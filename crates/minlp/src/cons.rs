//! SCIP's constraints and constraint handlers (`cons.c`, `struct_cons.h`).
//!
//! A handler's callbacks are function pointers, as in C. A constraint's
//! data and a handler's data are `Box<dyn Any>`, which the handler
//! downcasts to its own type.

use std::any::Any;

use crate::{
    clock::Clock,
    cvec::CVec,
    retcode::ScipResult,
    scip::Scip,
    types::{BoundType, ConsId, ConshdlrId, LockType, NLOCKTYPES, ScipResultCode, SolId, VarId},
    var::{BdChgIdx, NONE},
};

/// `SCIP_PROPTIMING`: bitmask of when a propagator runs.
pub type PropTiming = i32;
pub const PROPTIMING_BEFORELP: PropTiming = 0x001;
pub const PROPTIMING_DURINGLPLOOP: PropTiming = 0x002;
pub const PROPTIMING_AFTERLPLOOP: PropTiming = 0x004;
pub const PROPTIMING_AFTERLPNODE: PropTiming = 0x008;
pub const PROPTIMING_ALWAYS: PropTiming = PROPTIMING_BEFORELP | PROPTIMING_DURINGLPLOOP | PROPTIMING_AFTERLPLOOP | PROPTIMING_AFTERLPNODE;

/// `SCIP_PRESOLTIMING`: bitmask of the presolving rounds a presolver runs in.
pub type PresolTiming = i32;
pub const PRESOLTIMING_NONE: PresolTiming = 0x002;
pub const PRESOLTIMING_FAST: PresolTiming = 0x004;
pub const PRESOLTIMING_MEDIUM: PresolTiming = 0x008;
pub const PRESOLTIMING_EXHAUSTIVE: PresolTiming = 0x010;
pub const PRESOLTIMING_FINAL: PresolTiming = 0x020;
pub const PRESOLTIMING_ALWAYS: PresolTiming = PRESOLTIMING_FAST | PRESOLTIMING_MEDIUM | PRESOLTIMING_EXHAUSTIVE;
pub const PRESOLTIMING_MAX: PresolTiming = PRESOLTIMING_FAST | PRESOLTIMING_MEDIUM | PRESOLTIMING_EXHAUSTIVE | PRESOLTIMING_FINAL;

/// The presolving counters C passes as ten `int*` arguments (and their
/// "new since the last call" values as ten `int`s).
#[derive(Clone, Copy, Debug, Default)]
pub struct PresolCounts {
    pub nfixedvars: i32,
    pub naggrvars: i32,
    pub nchgvartypes: i32,
    pub nchgbds: i32,
    pub naddholes: i32,
    pub ndelconss: i32,
    pub naddconss: i32,
    pub nupgdconss: i32,
    pub nchgcoefs: i32,
    pub nchgsides: i32,
}

impl PresolCounts {
    /// The counts minus `b`, field by field (the changes since `b`).
    pub fn minus(&self, b: &PresolCounts) -> PresolCounts {
        PresolCounts {
            nfixedvars: self.nfixedvars - b.nfixedvars,
            naggrvars: self.naggrvars - b.naggrvars,
            nchgvartypes: self.nchgvartypes - b.nchgvartypes,
            nchgbds: self.nchgbds - b.nchgbds,
            naddholes: self.naddholes - b.naddholes,
            ndelconss: self.ndelconss - b.ndelconss,
            naddconss: self.naddconss - b.naddconss,
            nupgdconss: self.nupgdconss - b.nupgdconss,
            nchgcoefs: self.nchgcoefs - b.nchgcoefs,
            nchgsides: self.nchgsides - b.nchgsides,
        }
    }

    /// Adds `d`, field by field.
    pub fn add(&mut self, d: &PresolCounts) {
        self.nfixedvars += d.nfixedvars;
        self.naggrvars += d.naggrvars;
        self.nchgvartypes += d.nchgvartypes;
        self.nchgbds += d.nchgbds;
        self.naddholes += d.naddholes;
        self.ndelconss += d.ndelconss;
        self.naddconss += d.naddconss;
        self.nupgdconss += d.nupgdconss;
        self.nchgcoefs += d.nchgcoefs;
        self.nchgsides += d.nchgsides;
    }
}

/// `SCIP_DECL_CONSHDLRCOPY`: `(target scip, source scip, source handler,
/// valid)`.
pub type ConshdlrCopy = fn(&mut Scip, &Scip, ConshdlrId, &mut bool) -> ScipResult;
pub type ConsFree = fn(&mut Scip, ConshdlrId) -> ScipResult;
/// `SCIP_DECL_CONSINIT`, and the `INITPRE`, `EXITPRE`, `INITSOL`, `EXIT` and
/// `DELVARS` methods of the same shape.
pub type ConsInit = fn(&mut Scip, ConshdlrId, &[ConsId]) -> ScipResult;
pub type ConsExitsol = fn(&mut Scip, ConshdlrId, &[ConsId], bool) -> ScipResult;
pub type ConsDelete = fn(&mut Scip, ConshdlrId, ConsId, &mut Option<Box<dyn Any>>) -> ScipResult;
pub type ConsTrans = fn(&mut Scip, ConshdlrId, ConsId, &mut ConsId) -> ScipResult;
pub type ConsInitlp = fn(&mut Scip, ConshdlrId, &[ConsId], &mut bool) -> ScipResult;
/// `(conss, nusefulconss, result)`.
pub type ConsSepalp = fn(&mut Scip, ConshdlrId, &[ConsId], i32, &mut ScipResultCode) -> ScipResult;
pub type ConsSepasol = fn(&mut Scip, ConshdlrId, &[ConsId], i32, SolId, &mut ScipResultCode) -> ScipResult;
/// `(conss, nusefulconss, solinfeasible, result)`.
pub type ConsEnfolp = fn(&mut Scip, ConshdlrId, &[ConsId], i32, bool, &mut ScipResultCode) -> ScipResult;
pub type ConsEnforelax = fn(&mut Scip, SolId, ConshdlrId, &[ConsId], i32, bool, &mut ScipResultCode) -> ScipResult;
/// `(conss, nusefulconss, solinfeasible, objinfeasible, result)`.
pub type ConsEnfops = fn(&mut Scip, ConshdlrId, &[ConsId], i32, bool, bool, &mut ScipResultCode) -> ScipResult;
/// `(conss, sol, checkintegrality, checklprows, printreason, completely, result)`.
pub type ConsCheck = fn(&mut Scip, ConshdlrId, &[ConsId], SolId, bool, bool, bool, bool, &mut ScipResultCode) -> ScipResult;
/// `(conss, nusefulconss, nmarkedconss, proptiming, result)`.
pub type ConsProp = fn(&mut Scip, ConshdlrId, &[ConsId], i32, i32, PropTiming, &mut ScipResultCode) -> ScipResult;
/// `(conss, nrounds, presoltiming, new counts, counts, result)`.
pub type ConsPresol = fn(&mut Scip, ConshdlrId, &[ConsId], i32, PresolTiming, &PresolCounts, &mut PresolCounts, &mut ScipResultCode) -> ScipResult;
/// `(cons, infervar, inferinfo, boundtype, bdchgidx, relaxedbd, result)`.
pub type ConsResprop = fn(&mut Scip, ConshdlrId, ConsId, VarId, i32, BoundType, Option<BdChgIdx>, f64, &mut ScipResultCode) -> ScipResult;
pub type ConsLock = fn(&mut Scip, ConshdlrId, ConsId, LockType, i32, i32) -> ScipResult;
/// `SCIP_DECL_CONSACTIVE`, and `DEACTIVE`, `ENABLE`, `DISABLE`.
pub type ConsActive = fn(&mut Scip, ConshdlrId, ConsId) -> ScipResult;
/// `SCIP_DECL_CONSGETVARS`: fills `vars`, or reports it is too small.
pub type ConsGetvars = fn(&mut Scip, ConshdlrId, ConsId, &mut Vec<VarId>, &mut bool) -> ScipResult;
pub type ConsGetnvars = fn(&mut Scip, ConshdlrId, ConsId, &mut i32, &mut bool) -> ScipResult;
pub type ConsGetdivebdchgs = fn(&mut Scip, ConshdlrId, usize, SolId, &mut bool, &mut bool) -> ScipResult;
/// `SCIP_DECL_CONSCOPY`: `(target scip, target cons, name, source scip,
/// source handler, source cons, var and cons maps, flags, global, valid)`.
pub type ConsCopy = fn(&mut Scip, &mut ConsId, Option<&str>, &mut Scip, ConshdlrId, ConsId, &mut crate::copy::CopyMaps, &ConsFlags, bool, &mut bool) -> ScipResult;

/// The flags every `SCIPcreateCons...` takes.
#[derive(Clone, Copy, Debug)]
pub struct ConsFlags {
    pub initial: bool,
    pub separate: bool,
    pub enforce: bool,
    pub check: bool,
    pub propagate: bool,
    pub local: bool,
    pub modifiable: bool,
    pub dynamic: bool,
    pub removable: bool,
    pub stickingatnode: bool,
}

impl Default for ConsFlags {
    /// The flags of `SCIPcreateConsBasic...`.
    fn default() -> Self {
        Self {
            initial: true,
            separate: true,
            enforce: true,
            check: true,
            propagate: true,
            local: false,
            modifiable: false,
            dynamic: false,
            removable: false,
            stickingatnode: false,
        }
    }
}

/// `SCIP_CONS`.
pub struct Cons {
    pub age: f64,
    pub name: String,
    pub conshdlr: ConshdlrId,
    pub consdata: Option<Box<dyn Any>>,
    pub transorigcons: ConsId,
    /// The node whose constraint set change added this constraint, or
    /// `NONE` (C points at the set change itself, which the node owns).
    pub addconssetchg: usize,
    pub addarraypos: i32,
    pub consspos: i32,
    pub initconsspos: i32,
    pub sepaconsspos: i32,
    pub enfoconsspos: i32,
    pub checkconsspos: i32,
    pub propconsspos: i32,
    pub confconsspos: i32,
    pub nlockspos: [i32; NLOCKTYPES],
    pub nlocksneg: [i32; NLOCKTYPES],
    pub activedepth: i32,
    pub validdepth: i32,
    pub nuses: i32,
    pub nupgradelocks: i32,
    pub initial: bool,
    pub separate: bool,
    pub enforce: bool,
    pub check: bool,
    pub propagate: bool,
    pub sepaenabled: bool,
    pub propenabled: bool,
    pub local: bool,
    pub modifiable: bool,
    pub dynamic: bool,
    pub removable: bool,
    pub stickingatnode: bool,
    pub original: bool,
    pub deleteconsdata: bool,
    pub active: bool,
    pub conflict: bool,
    pub enabled: bool,
    pub obsolete: bool,
    pub markpropagate: bool,
    pub deleted: bool,
    pub update: bool,
    pub updateinsert: bool,
    pub updateactivate: bool,
    pub updatedeactivate: bool,
    pub updateenable: bool,
    pub updatedisable: bool,
    pub updatesepaenable: bool,
    pub updatesepadisable: bool,
    pub updatepropenable: bool,
    pub updatepropdisable: bool,
    pub updateobsolete: bool,
    pub updatefree: bool,
    pub updateactfocus: bool,
    pub updatemarkpropagate: bool,
    pub updateunmarkpropagate: bool,
}

/// `SCIP_CONSSETCHG`.
#[derive(Clone, Debug, Default)]
pub struct ConsSetChg {
    pub addedconss: CVec<ConsId>,
    pub disabledconss: CVec<ConsId>,
    pub addedconsssize: i32,
    pub naddedconss: i32,
    pub disabledconsssize: i32,
    pub ndisabledconss: i32,
}

/// `SCIP_CONSHDLR`.
pub struct Conshdlr {
    pub nsepacalls: i64,
    pub nenfolpcalls: i64,
    pub nenfopscalls: i64,
    pub nenforelaxcalls: i64,
    pub npropcalls: i64,
    pub ncheckcalls: i64,
    pub nrespropcalls: i64,
    pub ncutoffs: i64,
    pub ncutsfound: i64,
    pub ncutsapplied: i64,
    pub nconssfound: i64,
    pub ndomredsfound: i64,
    pub nchildren: i64,
    pub lastpropdomchgcount: i64,
    pub storedpropdomchgcount: i64,
    pub lastenfolpdomchgcount: i64,
    pub lastenfopsdomchgcount: i64,
    pub lastenforelaxdomchgcount: i64,
    pub lastenfolpnode: i64,
    pub lastenfopsnode: i64,
    pub lastenforelaxnode: i64,
    pub lastenfolpresult: ScipResultCode,
    pub lastenfopsresult: ScipResultCode,
    pub lastenforelaxresult: ScipResultCode,
    pub ageresetavg: f64,
    pub name: String,
    pub desc: String,
    pub conshdlrcopy: Option<ConshdlrCopy>,
    pub consfree: Option<ConsFree>,
    pub consinit: Option<ConsInit>,
    pub consexit: Option<ConsInit>,
    pub consinitpre: Option<ConsInit>,
    pub consexitpre: Option<ConsInit>,
    pub consinitsol: Option<ConsInit>,
    pub consexitsol: Option<ConsExitsol>,
    pub consdelete: Option<ConsDelete>,
    pub constrans: Option<ConsTrans>,
    pub consinitlp: Option<ConsInitlp>,
    pub conssepalp: Option<ConsSepalp>,
    pub conssepasol: Option<ConsSepasol>,
    pub consenfolp: Option<ConsEnfolp>,
    pub consenforelax: Option<ConsEnforelax>,
    pub consenfops: Option<ConsEnfops>,
    pub conscheck: Option<ConsCheck>,
    pub consprop: Option<ConsProp>,
    pub conspresol: Option<ConsPresol>,
    pub consresprop: Option<ConsResprop>,
    pub conslock: Option<ConsLock>,
    pub consactive: Option<ConsActive>,
    pub consdeactive: Option<ConsActive>,
    pub consenable: Option<ConsActive>,
    pub consdisable: Option<ConsActive>,
    pub consdelvars: Option<ConsInit>,
    pub conscopy: Option<ConsCopy>,
    pub consgetvars: Option<ConsGetvars>,
    pub consgetnvars: Option<ConsGetnvars>,
    pub consgetdivebdchgs: Option<ConsGetdivebdchgs>,
    pub conshdlrdata: Option<Box<dyn Any>>,
    pub conss: CVec<ConsId>,
    pub initconss: CVec<ConsId>,
    pub sepaconss: CVec<ConsId>,
    pub enfoconss: CVec<ConsId>,
    pub checkconss: CVec<ConsId>,
    pub propconss: CVec<ConsId>,
    pub storedpropconss: CVec<ConsId>,
    pub updateconss: CVec<ConsId>,
    pub setuptime: Clock,
    pub presoltime: Clock,
    pub sepatime: Clock,
    pub enfolptime: Clock,
    pub enfopstime: Clock,
    pub enforelaxtime: Clock,
    pub proptime: Clock,
    pub sbproptime: Clock,
    pub checktime: Clock,
    pub resproptime: Clock,
    pub lastsepalpcount: i64,
    pub lastenfolplpcount: i64,
    pub lastenforelaxrelaxcount: i64,
    pub sepapriority: i32,
    pub enfopriority: i32,
    pub checkpriority: i32,
    pub sepafreq: i32,
    pub propfreq: i32,
    pub eagerfreq: i32,
    pub maxprerounds: i32,
    pub consssize: i32,
    pub nconss: i32,
    pub nactiveconss: i32,
    pub maxnactiveconss: i32,
    pub startnactiveconss: i32,
    pub initconsssize: i32,
    pub ninitconss: i32,
    pub ninitconsskept: i32,
    pub sepaconsssize: i32,
    pub nsepaconss: i32,
    pub nusefulsepaconss: i32,
    pub enfoconsssize: i32,
    pub nenfoconss: i32,
    pub nusefulenfoconss: i32,
    pub checkconsssize: i32,
    pub ncheckconss: i32,
    pub nusefulcheckconss: i32,
    pub propconsssize: i32,
    pub npropconss: i32,
    pub nmarkedpropconss: i32,
    pub nusefulpropconss: i32,
    pub storedpropconsssize: i32,
    pub storednmarkedpropconss: i32,
    pub updateconsssize: i32,
    pub nupdateconss: i32,
    pub nenabledconss: i32,
    pub lastnusefulpropconss: i32,
    pub lastnusefulsepaconss: i32,
    pub lastnusefulenfoconss: i32,
    pub lastnfixedvars: i32,
    pub lastnaggrvars: i32,
    pub lastnchgvartypes: i32,
    pub lastnchgbds: i32,
    pub lastnaddholes: i32,
    pub lastndelconss: i32,
    pub lastnaddconss: i32,
    pub lastnupgdconss: i32,
    pub lastnchgcoefs: i32,
    pub lastnchgsides: i32,
    pub nfixedvars: i32,
    pub naggrvars: i32,
    pub nchgvartypes: i32,
    pub nchgbds: i32,
    pub naddholes: i32,
    pub ndelconss: i32,
    pub naddconss: i32,
    pub nupgdconss: i32,
    pub nchgcoefs: i32,
    pub nchgsides: i32,
    pub npresolcalls: i32,
    pub delayupdatecount: i32,
    pub delaysepa: bool,
    pub delayprop: bool,
    pub needscons: bool,
    pub sepalpwasdelayed: bool,
    pub sepasolwasdelayed: bool,
    pub propwasdelayed: bool,
    pub exact: bool,
    pub initialized: bool,
    pub duringsepa: bool,
    pub duringprop: bool,
    pub proptiming: PropTiming,
    pub presoltiming: PresolTiming,
}

/// `AGERESETAVG_INIT`: initial value of the exponentially decaying weighted
/// sum for ages.
pub const AGERESETAVG_INIT: f64 = 1000.0;
/// `AGERESETAVG_MIN`: minimal value to use for weighted sum of ages.
const AGERESETAVG_MIN: f64 = 100.0;
/// `AGERESETAVG_DECAY`: weight of a new addend in the exponentially
/// decaying sum.
const AGERESETAVG_DECAY: f64 = 0.0005;
/// `AGERESETAVG_AGELIMIT`: in dynamic setting, a constraint is deleted if
/// its age exceeds the average reset age by this factor.
const AGERESETAVG_AGELIMIT: f64 = 2.0;
/// `AGERESETAVG_OBSOLETEAGE`: in dynamic setting, a constraint is marked
/// obsolete if its age exceeds the average reset age by this factor.
const AGERESETAVG_OBSOLETEAGE: f64 = 1.8;

/// Moves `cons` to position `k` of the handler's array `arr`, swapping the
/// constraint there into its old position (`pos` is the constraints'
/// position field for that array).
macro_rules! cons_swap {
    ($scip:expr, $h:expr, $cons:expr, $arr:ident, $pos:ident, $k:expr) => {{
        let k = $k;
        let tmpcons = $scip.conshdlrs[$h].$arr[k];
        let oldpos = $scip.conss[$cons].$pos;
        $scip.conshdlrs[$h].$arr[k] = $cons;
        $scip.conshdlrs[$h].$arr[oldpos] = tmpcons;
        $scip.conss[tmpcons].$pos = oldpos;
        $scip.conss[$cons].$pos = k;
    }};
}

/// Grows one of a handler's constraint arrays like
/// `conshdlrEnsureConssMem` and its siblings.
macro_rules! conshdlr_ensure {
    ($name:ident, $arr:ident, $size:ident) => {
        fn $name(scip: &mut Scip, h: ConshdlrId, num: i32) {
            if num > scip.conshdlrs[h].$size {
                let newsize = scip.set.calc_mem_grow_size(num);
                let c = &mut scip.conshdlrs[h];
                c.$arr.realloc(newsize);
                c.$size = newsize;
            }
        }
    };
}

conshdlr_ensure!(conshdlr_ensure_conss_mem, conss, consssize);
conshdlr_ensure!(conshdlr_ensure_initconss_mem, initconss, initconsssize);
conshdlr_ensure!(conshdlr_ensure_sepaconss_mem, sepaconss, sepaconsssize);
conshdlr_ensure!(conshdlr_ensure_enfoconss_mem, enfoconss, enfoconsssize);
conshdlr_ensure!(conshdlr_ensure_checkconss_mem, checkconss, checkconsssize);
conshdlr_ensure!(conshdlr_ensure_propconss_mem, propconss, propconsssize);
conshdlr_ensure!(conshdlr_ensure_updateconss_mem, updateconss, updateconsssize);

/// `conshdlrAreUpdatesDelayed`.
fn conshdlr_are_updates_delayed(c: &Conshdlr) -> bool {
    c.delayupdatecount > 0
}

/// `conshdlrGetAgeresetavg`.
fn conshdlr_get_ageresetavg(c: &Conshdlr) -> f64 {
    crate::def::max(c.ageresetavg, AGERESETAVG_MIN)
}

/// `conshdlrUpdateAgeresetavg`: updates the exponentially decaying weighted
/// age average for age resets.
fn conshdlr_update_ageresetavg(c: &mut Conshdlr, age: f64) {
    c.ageresetavg *= 1.0 - AGERESETAVG_DECAY;
    c.ageresetavg += AGERESETAVG_DECAY * age;
}

/// `consExceedsAgelimit`: returns whether the constraint's age exceeds the
/// age limit.
fn cons_exceeds_agelimit(scip: &Scip, cons: ConsId) -> bool {
    let c = &scip.conss[cons];
    let agelimit = scip.set.cons_agelimit;
    c.dynamic && ((agelimit > 0 && c.age > agelimit as f64) || (agelimit == 0 && c.age > AGERESETAVG_AGELIMIT * conshdlr_get_ageresetavg(&scip.conshdlrs[c.conshdlr])))
}

/// `consExceedsObsoleteage`: returns whether the constraint's age exceeds
/// the obsolete age limit.
fn cons_exceeds_obsoleteage(scip: &Scip, cons: ConsId) -> bool {
    let c = &scip.conss[cons];
    let obsoleteage = scip.set.cons_obsoleteage;
    c.dynamic && ((obsoleteage > 0 && c.age > obsoleteage as f64) || (obsoleteage == 0 && c.age > AGERESETAVG_OBSOLETEAGE * conshdlr_get_ageresetavg(&scip.conshdlrs[c.conshdlr])))
}

/// `conshdlrMarkConsObsolete`: marks constraint to be obsolete; it will be
/// moved to the last part of the constraint arrays, such that it is
/// checked, enforced, separated, and propagated after the useful
/// constraints.
fn conshdlr_mark_cons_obsolete(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    scip.conss[cons].obsolete = true;
    if scip.conss[cons].active && scip.conss[cons].check {
        // Switch the last useful (non-obsolete) check constraint with this
        // constraint.
        let k = scip.conshdlrs[h].nusefulcheckconss - 1;
        cons_swap!(scip, h, cons, checkconss, checkconsspos, k);
        scip.conshdlrs[h].nusefulcheckconss -= 1;
    }
    if scip.conss[cons].enabled {
        let c = &scip.conss[cons];
        if c.separate && c.sepaenabled {
            if c.sepaconsspos < scip.conshdlrs[h].lastnusefulsepaconss {
                scip.conshdlrs[h].lastnusefulsepaconss -= 1;
            }

            // Switch the last useful (non-obsolete) sepa constraint with
            // this constraint.
            let k = scip.conshdlrs[h].nusefulsepaconss - 1;
            cons_swap!(scip, h, cons, sepaconss, sepaconsspos, k);
            scip.conshdlrs[h].nusefulsepaconss -= 1;
        }
        if scip.conss[cons].enforce {
            let ch = &mut scip.conshdlrs[h];
            if scip.conss[cons].enfoconsspos < ch.lastnusefulenfoconss {
                ch.lastnusefulenfoconss -= 1;
            } else {
                // The constraint that becomes obsolete is not yet enforced
                // on the current solution: reset the enforcement counters
                // in order to enforce all constraints again.
                ch.lastenfolplpcount = -1;
                ch.lastenfolpdomchgcount = -1;
                ch.lastenfopsdomchgcount = -1;
                ch.lastenforelaxdomchgcount = -1;
                ch.lastenforelaxrelaxcount = -1;
                ch.lastenfolpnode = -1;
                ch.lastenfopsnode = -1;
            }

            // Switch the last useful (non-obsolete) enfo constraint with
            // this constraint.
            let k = scip.conshdlrs[h].nusefulenfoconss - 1;
            cons_swap!(scip, h, cons, enfoconss, enfoconsspos, k);
            scip.conshdlrs[h].nusefulenfoconss -= 1;
        }

        // In case the constraint is marked to be propagated, we do not move
        // it in the propconss array since the first part of the array
        // contains all marked constraints independently of their age.
        let c = &scip.conss[cons];
        if c.propagate && c.propenabled && !c.markpropagate {
            if c.propconsspos < scip.conshdlrs[h].lastnusefulpropconss {
                scip.conshdlrs[h].lastnusefulpropconss -= 1;
            }

            // Switch the last useful (non-obsolete) prop constraint with
            // this constraint.
            let k = scip.conshdlrs[h].nusefulpropconss - 1;
            cons_swap!(scip, h, cons, propconss, propconsspos, k);
            scip.conshdlrs[h].nusefulpropconss -= 1;
        }
    }
}

/// `conshdlrMarkConsUseful`: marks obsolete constraint to be not obsolete
/// anymore; it will be moved to the first part of the constraint arrays.
fn conshdlr_mark_cons_useful(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    scip.conss[cons].obsolete = false;
    if scip.conss[cons].active && scip.conss[cons].check {
        // Switch the first obsolete check constraint with this constraint.
        let k = scip.conshdlrs[h].nusefulcheckconss;
        cons_swap!(scip, h, cons, checkconss, checkconsspos, k);
        scip.conshdlrs[h].nusefulcheckconss += 1;
    }
    if scip.conss[cons].enabled {
        let c = &scip.conss[cons];
        if c.separate && c.sepaenabled {
            // Switch the first obsolete sepa constraint with this
            // constraint.
            let k = scip.conshdlrs[h].nusefulsepaconss;
            cons_swap!(scip, h, cons, sepaconss, sepaconsspos, k);
            scip.conshdlrs[h].nusefulsepaconss += 1;
        }
        if scip.conss[cons].enforce {
            // Switch the first obsolete enfo constraint with this
            // constraint.
            let k = scip.conshdlrs[h].nusefulenfoconss;
            cons_swap!(scip, h, cons, enfoconss, enfoconsspos, k);
            scip.conshdlrs[h].nusefulenfoconss += 1;
        }

        // In case the constraint is marked to be propagated, we do not move
        // it in the propconss array since the first part of the array
        // contains all marked constraints independently of their age.
        let c = &scip.conss[cons];
        if c.propagate && c.propenabled && !c.markpropagate {
            // Switch the first obsolete prop constraint with this
            // constraint.
            let k = scip.conshdlrs[h].nusefulpropconss;
            cons_swap!(scip, h, cons, propconss, propconsspos, k);
            scip.conshdlrs[h].nusefulpropconss += 1;
        }
    }
}

/// `conshdlrMarkConsPropagate`: marks constraint to be propagated in the
/// next propagation round; it is moved to the first part of the
/// propagation array.
fn conshdlr_mark_cons_propagate(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let c = &scip.conss[cons];
    if !c.enabled || c.markpropagate {
        return;
    }
    scip.conss[cons].markpropagate = true;

    // Propagation of the constraint is globally or locally disabled, so we
    // do not have to move the constraint in the propconss array.
    let c = &scip.conss[cons];
    if !c.propagate || !c.propenabled {
        return;
    }

    // If the constraint is obsolete, we need to move it first to the
    // non-obsolete part of the array.
    if c.obsolete {
        let k = scip.conshdlrs[h].nusefulpropconss;
        cons_swap!(scip, h, cons, propconss, propconsspos, k);
        scip.conshdlrs[h].nusefulpropconss += 1;
    }

    // Move the constraint to the first part of the array.
    let k = scip.conshdlrs[h].nmarkedpropconss;
    cons_swap!(scip, h, cons, propconss, propconsspos, k);
    scip.conshdlrs[h].nmarkedpropconss += 1;
}

/// `conshdlrUnmarkConsPropagate`: unmarks constraint to be propagated in
/// the next propagation round; it is moved behind the marked part of the
/// propagation array.
fn conshdlr_unmark_cons_propagate(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let c = &scip.conss[cons];
    if !c.enabled || !c.markpropagate {
        return;
    }
    scip.conss[cons].markpropagate = false;

    // Propagation of the constraint is globally or locally disabled, so we
    // do not have to move the constraint in the propconss array.
    let c = &scip.conss[cons];
    if !c.propagate || !c.propenabled {
        return;
    }

    // First, move the constraint out of the first part to the second part
    // of the constraint array.
    let nmarked = scip.conshdlrs[h].nmarkedpropconss;
    if c.propconsspos < nmarked - 1 {
        scip.conshdlrs[h].nmarkedpropconss -= 1;
        let k = scip.conshdlrs[h].nmarkedpropconss;
        cons_swap!(scip, h, cons, propconss, propconsspos, k);
    } else if c.propconsspos == nmarked - 1 {
        scip.conshdlrs[h].nmarkedpropconss -= 1;
    }

    // If the constraint is obsolete, move it to the last part of the
    // array.
    if scip.conss[cons].obsolete {
        scip.conshdlrs[h].nusefulpropconss -= 1;
        let k = scip.conshdlrs[h].nusefulpropconss;
        cons_swap!(scip, h, cons, propconss, propconsspos, k);
    }
}

/// Moves the constraint at position `from` of the handler's array `arr`
/// to position `to`, updating its position field `pos`.
macro_rules! cons_move {
    ($scip:expr, $h:expr, $arr:ident, $pos:ident, $from:expr, $to:expr) => {{
        let (from, to) = ($from, $to);
        let moved = $scip.conshdlrs[$h].$arr[from];
        $scip.conshdlrs[$h].$arr[to] = moved;
        $scip.conss[moved].$pos = to;
    }};
}

/// `conshdlrAddCons`: adds constraint to conshdlr's constraint array.
fn conshdlr_add_cons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let n = scip.conshdlrs[h].nconss;
    conshdlr_ensure_conss_mem(scip, h, n + 1);
    let c = &mut scip.conshdlrs[h];
    c.conss[c.nconss] = cons;
    scip.conss[cons].consspos = c.nconss;
    c.nconss += 1;
}

/// `conshdlrDelCons`: deletes constraint from conshdlr's constraint array.
fn conshdlr_del_cons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let pos = scip.conss[cons].consspos;
    let last = scip.conshdlrs[h].nconss - 1;
    cons_move!(scip, h, conss, consspos, last, pos);
    scip.conshdlrs[h].nconss -= 1;
    scip.conss[cons].consspos = -1;
}

/// `conshdlrAddInitcons`: adds constraint to the initconss array of
/// constraint handler.
fn conshdlr_add_initcons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let n = scip.conshdlrs[h].ninitconss;
    conshdlr_ensure_initconss_mem(scip, h, n + 1);
    let c = &mut scip.conshdlrs[h];
    let insertpos = c.ninitconss;
    c.initconss[insertpos] = cons;
    c.ninitconss += 1;
    scip.stat.ninitconssadded += 1;

    // If the constraint is kept, we keep the stored position at the
    // beginning of the array.
    if scip.conss[cons].initconsspos == -1 {
        scip.conss[cons].initconsspos = insertpos;
    }
}

/// `conshdlrDelInitcons`: deletes constraint from the initconss array of
/// constraint handler.
fn conshdlr_del_initcons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let mut delpos = scip.conss[cons].initconsspos;
    if delpos < scip.conshdlrs[h].ninitconsskept {
        scip.conshdlrs[h].ninitconsskept -= 1;
        let kept = scip.conshdlrs[h].ninitconsskept;
        cons_move!(scip, h, initconss, initconsspos, kept, delpos);
        delpos = kept;
    }
    let n = scip.conshdlrs[h].ninitconss;
    if delpos < n - 1 {
        cons_move!(scip, h, initconss, initconsspos, n - 1, delpos);
    }
    scip.conshdlrs[h].ninitconss -= 1;
    scip.conss[cons].initconsspos = -1;
}

/// `conshdlrAddSepacons`: adds constraint to the sepaconss array of
/// constraint handler.
fn conshdlr_add_sepacons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let n = scip.conshdlrs[h].nsepaconss;
    conshdlr_ensure_sepaconss_mem(scip, h, n + 1);
    let mut insertpos = n;
    if !scip.conss[cons].obsolete {
        let nuseful = scip.conshdlrs[h].nusefulsepaconss;
        if nuseful < n {
            cons_move!(scip, h, sepaconss, sepaconsspos, nuseful, n);
            insertpos = nuseful;
        }
        scip.conshdlrs[h].nusefulsepaconss += 1;
    }
    scip.conshdlrs[h].sepaconss[insertpos] = cons;
    scip.conss[cons].sepaconsspos = insertpos;
    scip.conshdlrs[h].nsepaconss += 1;
}

/// `conshdlrDelSepacons`: deletes constraint from the sepaconss array of
/// constraint handler.
fn conshdlr_del_sepacons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let mut delpos = scip.conss[cons].sepaconsspos;
    if !scip.conss[cons].obsolete {
        if delpos < scip.conshdlrs[h].lastnusefulsepaconss {
            scip.conshdlrs[h].lastnusefulsepaconss -= 1;
        }
        let last = scip.conshdlrs[h].nusefulsepaconss - 1;
        cons_move!(scip, h, sepaconss, sepaconsspos, last, delpos);
        delpos = last;
        scip.conshdlrs[h].nusefulsepaconss -= 1;
    }
    let n = scip.conshdlrs[h].nsepaconss;
    if delpos < n - 1 {
        cons_move!(scip, h, sepaconss, sepaconsspos, n - 1, delpos);
    }
    scip.conshdlrs[h].nsepaconss -= 1;
    scip.conss[cons].sepaconsspos = -1;
}

/// Resets a handler's enforcement counters, so that all constraints are
/// enforced again on the next call.
fn conshdlr_reset_enforcement(c: &mut Conshdlr) {
    c.lastenfolplpcount = -1;
    c.lastenfolpdomchgcount = -1;
    c.lastenfopsdomchgcount = -1;
    c.lastenforelaxdomchgcount = -1;
    c.lastenforelaxrelaxcount = -1;
    c.lastenfolpnode = -1;
    c.lastenfopsnode = -1;
}

/// `conshdlrAddEnfocons`: adds constraint to the enfoconss array of
/// constraint handler.
fn conshdlr_add_enfocons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let n = scip.conshdlrs[h].nenfoconss;
    conshdlr_ensure_enfoconss_mem(scip, h, n + 1);
    let mut insertpos = n;
    if !scip.conss[cons].obsolete {
        let nuseful = scip.conshdlrs[h].nusefulenfoconss;
        if nuseful < n {
            cons_move!(scip, h, enfoconss, enfoconsspos, nuseful, n);
            insertpos = nuseful;
        }
        scip.conshdlrs[h].nusefulenfoconss += 1;
    } else {
        // We have to make sure that even this obsolete constraint is
        // enforced in the next enforcement call.
        conshdlr_reset_enforcement(&mut scip.conshdlrs[h]);
    }
    scip.conshdlrs[h].enfoconss[insertpos] = cons;
    scip.conss[cons].enfoconsspos = insertpos;
    scip.conshdlrs[h].nenfoconss += 1;
}

/// `conshdlrDelEnfocons`: deletes constraint from the enfoconss array of
/// constraint handler.
fn conshdlr_del_enfocons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let mut delpos = scip.conss[cons].enfoconsspos;
    if !scip.conss[cons].obsolete {
        if delpos < scip.conshdlrs[h].lastnusefulenfoconss {
            scip.conshdlrs[h].lastnusefulenfoconss -= 1;
        }
        let last = scip.conshdlrs[h].nusefulenfoconss - 1;
        cons_move!(scip, h, enfoconss, enfoconsspos, last, delpos);
        delpos = last;
        scip.conshdlrs[h].nusefulenfoconss -= 1;

        // If the constraint that moved to the free position was a newly
        // added constraint and not enforced in the last enforcement, we
        // have to make sure it will be enforced in the next run.
        if delpos >= scip.conshdlrs[h].lastnusefulenfoconss {
            scip.conshdlrs[h].lastnusefulenfoconss = scip.conss[cons].enfoconsspos;
        }
    }
    let n = scip.conshdlrs[h].nenfoconss;
    if delpos < n - 1 {
        cons_move!(scip, h, enfoconss, enfoconsspos, n - 1, delpos);
    }
    scip.conshdlrs[h].nenfoconss -= 1;
    scip.conss[cons].enfoconsspos = -1;
}

/// `conshdlrAddCheckcons`: adds constraint to the checkconss array of
/// constraint handler.
fn conshdlr_add_checkcons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let n = scip.conshdlrs[h].ncheckconss;
    conshdlr_ensure_checkconss_mem(scip, h, n + 1);
    let mut insertpos = n;
    if !scip.conss[cons].obsolete {
        let nuseful = scip.conshdlrs[h].nusefulcheckconss;
        if nuseful < n {
            cons_move!(scip, h, checkconss, checkconsspos, nuseful, n);
            insertpos = nuseful;
        }
        scip.conshdlrs[h].nusefulcheckconss += 1;
    }
    scip.conshdlrs[h].checkconss[insertpos] = cons;
    scip.conss[cons].checkconsspos = insertpos;
    scip.conshdlrs[h].ncheckconss += 1;
}

/// `conshdlrDelCheckcons`: deletes constraint from the checkconss array of
/// constraint handler.
fn conshdlr_del_checkcons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let mut delpos = scip.conss[cons].checkconsspos;
    if !scip.conss[cons].obsolete {
        let last = scip.conshdlrs[h].nusefulcheckconss - 1;
        cons_move!(scip, h, checkconss, checkconsspos, last, delpos);
        delpos = last;
        scip.conshdlrs[h].nusefulcheckconss -= 1;
    }
    let n = scip.conshdlrs[h].ncheckconss;
    if delpos < n - 1 {
        cons_move!(scip, h, checkconss, checkconsspos, n - 1, delpos);
    }
    scip.conshdlrs[h].ncheckconss -= 1;
    scip.conss[cons].checkconsspos = -1;
}

/// `conshdlrAddPropcons`: adds constraint to the propconss array of
/// constraint handler.
fn conshdlr_add_propcons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    let n = scip.conshdlrs[h].npropconss;
    conshdlr_ensure_propconss_mem(scip, h, n + 1);
    let mut insertpos = n;
    if !scip.conss[cons].obsolete {
        let nuseful = scip.conshdlrs[h].nusefulpropconss;
        if nuseful < n {
            cons_move!(scip, h, propconss, propconsspos, nuseful, n);
            insertpos = nuseful;
        }
        scip.conshdlrs[h].nusefulpropconss += 1;
    }
    scip.conshdlrs[h].propconss[insertpos] = cons;
    scip.conss[cons].propconsspos = insertpos;
    scip.conshdlrs[h].npropconss += 1;

    // If the constraint is marked to be propagated, we have to move it to
    // the first part of the array.
    if scip.conss[cons].markpropagate {
        scip.conss[cons].markpropagate = false;
        let hc = scip.conss[cons].conshdlr;
        conshdlr_mark_cons_propagate(scip, hc, cons);
    }
}

/// `conshdlrDelPropcons`: deletes constraint from the propconss array of
/// constraint handler.
fn conshdlr_del_propcons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    // Unmark constraint to be propagated; this will move the constraint to
    // the obsolete or non-obsolete part of the array, depending on its age.
    if scip.conss[cons].markpropagate {
        let hc = scip.conss[cons].conshdlr;
        conshdlr_unmark_cons_propagate(scip, hc, cons);
    }
    let mut delpos = scip.conss[cons].propconsspos;
    if !scip.conss[cons].obsolete {
        if delpos < scip.conshdlrs[h].lastnusefulpropconss {
            scip.conshdlrs[h].lastnusefulpropconss -= 1;
        }
        let last = scip.conshdlrs[h].nusefulpropconss - 1;
        cons_move!(scip, h, propconss, propconsspos, last, delpos);
        delpos = last;
        scip.conshdlrs[h].nusefulpropconss -= 1;
    }
    let n = scip.conshdlrs[h].npropconss;
    if delpos < n - 1 {
        cons_move!(scip, h, propconss, propconsspos, n - 1, delpos);
    }
    scip.conshdlrs[h].npropconss -= 1;
    scip.conss[cons].propconsspos = -1;
}

/// `conshdlrEnableConsSeparation`: enables separation of constraint.
fn conshdlr_enable_cons_separation(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    scip.conss[cons].sepaenabled = true;
    if scip.conss[cons].enabled && scip.conss[cons].separate {
        conshdlr_add_sepacons(scip, h, cons);
    }
}

/// `conshdlrDisableConsSeparation`: disables separation of constraint.
fn conshdlr_disable_cons_separation(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    if scip.conss[cons].separate && scip.conss[cons].enabled {
        conshdlr_del_sepacons(scip, h, cons);
    }
    scip.conss[cons].sepaenabled = false;
}

/// `conshdlrEnableConsPropagation`: enables propagation of constraint.
fn conshdlr_enable_cons_propagation(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    scip.conss[cons].propenabled = true;
    if scip.conss[cons].enabled && scip.conss[cons].propagate {
        conshdlr_add_propcons(scip, h, cons);
    }
}

/// `conshdlrDisableConsPropagation`: disables propagation of constraint.
fn conshdlr_disable_cons_propagation(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    if scip.conss[cons].propagate && scip.conss[cons].enabled {
        conshdlr_del_propcons(scip, h, cons);
    }
    scip.conss[cons].propenabled = false;
}

/// `conshdlrEnableCons`: enables separation, enforcement, and propagation
/// of constraint.
fn conshdlr_enable_cons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) -> ScipResult {
    scip.conss[cons].enabled = true;
    scip.conshdlrs[h].nenabledconss += 1;
    scip.stat.nenabledconss += 1;
    let c = &scip.conss[cons];
    if c.separate && c.sepaenabled {
        conshdlr_add_sepacons(scip, h, cons);
    }
    if scip.conss[cons].enforce {
        conshdlr_add_enfocons(scip, h, cons);
    }
    let c = &scip.conss[cons];
    if c.propagate && c.propenabled {
        conshdlr_add_propcons(scip, h, cons);
    }

    // Call the activation method of the constraint handler.
    if let Some(consenable) = scip.conshdlrs[h].consenable {
        consenable(scip, h, cons)?;
    }
    Ok(())
}

/// `conshdlrDisableCons`: disables separation, enforcement, and
/// propagation of constraint.
fn conshdlr_disable_cons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) -> ScipResult {
    // Call the deactivation method of the constraint handler.
    if let Some(consdisable) = scip.conshdlrs[h].consdisable {
        consdisable(scip, h, cons)?;
    }
    let c = &scip.conss[cons];
    if c.separate && c.sepaenabled {
        conshdlr_del_sepacons(scip, h, cons);
    }
    if scip.conss[cons].enforce {
        conshdlr_del_enfocons(scip, h, cons);
    }
    let c = &scip.conss[cons];
    if c.propagate && c.propenabled {
        conshdlr_del_propcons(scip, h, cons);
    }
    scip.conss[cons].enabled = false;
    scip.conshdlrs[h].nenabledconss -= 1;
    scip.stat.nenabledconss -= 1;
    Ok(())
}

/// `conshdlrActivateCons`: activates and adds constraint to constraint
/// handler's constraint arrays; `depth` is -1 for the global problem.
fn conshdlr_activate_cons(scip: &mut Scip, h: ConshdlrId, cons: ConsId, depth: i32, _focusnode: bool) -> ScipResult {
    scip.conss[cons].active = true;
    scip.conss[cons].activedepth = depth;

    // Move the constraint to the active part of the conss array.
    let k = scip.conshdlrs[h].nactiveconss;
    cons_swap!(scip, h, cons, conss, consspos, k);
    let c = &mut scip.conshdlrs[h];
    c.nactiveconss += 1;
    c.maxnactiveconss = crate::def::max(c.maxnactiveconss, c.nactiveconss);
    scip.stat.nactiveconss += 1;

    // Add constraint to the check array.
    if scip.conss[cons].check {
        conshdlr_add_checkcons(scip, h, cons);
    }

    // Add constraint to the initconss array if the constraint is initial
    // and added to the focus node.
    if scip.conss[cons].initial {
        conshdlr_add_initcons(scip, h, cons);
    }

    // Call the activation method of the constraint handler.
    if let Some(consactive) = scip.conshdlrs[h].consactive {
        consactive(scip, h, cons)?;
    }

    // Enable separation, enforcement, and propagation of constraint.
    conshdlr_enable_cons(scip, h, cons)
}

/// `conshdlrDeactivateCons`: deactivates and removes constraint from
/// constraint handler's conss array.
fn conshdlr_deactivate_cons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) -> ScipResult {
    // Disable constraint.
    if scip.conss[cons].enabled {
        conshdlr_disable_cons(scip, h, cons)?;
    }

    // Call the deactivation method of the constraint handler.
    if let Some(consdeactive) = scip.conshdlrs[h].consdeactive {
        consdeactive(scip, h, cons)?;
    }

    // Delete constraint from the initconss array.
    if scip.conss[cons].initconsspos >= 0 {
        conshdlr_del_initcons(scip, h, cons);
    }

    // Delete constraint from the check array.
    if scip.conss[cons].check {
        conshdlr_del_checkcons(scip, h, cons);
    }

    // Switch the last active constraint with the deactivated constraint.
    let k = scip.conshdlrs[h].nactiveconss - 1;
    cons_swap!(scip, h, cons, conss, consspos, k);
    scip.conshdlrs[h].nactiveconss -= 1;
    scip.conss[cons].active = false;
    scip.conss[cons].activedepth = -2;
    scip.stat.nactiveconss -= 1;
    Ok(())
}

/// `conshdlrProcessUpdates`: processes all delayed updates of constraints:
/// recently (de)activated constraints will be (de)activated; recently
/// en/disabled constraints will be en/disabled; recent obsolete non-check
/// constraints will be globally deleted; recent obsolete check constraints
/// will be moved to the last positions in the sepa-, enfo-, check-, and
/// prop-arrays; recent useful constraints will be moved to the first
/// positions in the sepa-, enfo-, check-, and prop-arrays; constraints
/// which were recently marked to be propagated are moved to the first
/// positions in the prop-array.
fn conshdlr_process_updates(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    let mut i = scip.conshdlrs[h].nupdateconss - 1;
    while i >= 0 {
        let cons = scip.conshdlrs[h].updateconss[i];
        if scip.conss[cons].updateinsert {
            conshdlr_add_cons(scip, h, cons);
            scip.conss[cons].updateinsert = false;
        }
        let c = &scip.conss[cons];
        if c.updateactivate {
            let (depth, focus) = (c.activedepth, c.updateactfocus);
            conshdlr_activate_cons(scip, h, cons, depth, focus)?;
            scip.conss[cons].updateactivate = false;
        } else if c.updatedeactivate {
            conshdlr_deactivate_cons(scip, h, cons)?;
            let obsolete = cons_exceeds_obsoleteage(scip, cons);
            let c = &mut scip.conss[cons];
            c.updatedeactivate = false;
            c.updateenable = false;
            c.updatedisable = false;
            c.obsolete = obsolete;
            c.updateobsolete = false;
        } else if c.updateenable {
            conshdlr_enable_cons(scip, h, cons)?;
            scip.conss[cons].updateenable = false;
        } else if c.updatedisable {
            conshdlr_disable_cons(scip, h, cons)?;
            scip.conss[cons].updatedisable = false;
        }
        let c = &scip.conss[cons];
        if c.updatesepaenable {
            if !c.sepaenabled {
                conshdlr_enable_cons_separation(scip, h, cons);
            }
            scip.conss[cons].updatesepaenable = false;
        } else if c.updatesepadisable {
            if c.sepaenabled {
                conshdlr_disable_cons_separation(scip, h, cons);
            }
            scip.conss[cons].updatesepadisable = false;
        }
        let c = &scip.conss[cons];
        if c.updatepropenable {
            if !c.propenabled {
                conshdlr_enable_cons_propagation(scip, h, cons);
            }
            scip.conss[cons].updatepropenable = false;
        } else if c.updatepropdisable {
            if c.propenabled {
                conshdlr_disable_cons_propagation(scip, h, cons);
            }
            scip.conss[cons].updatepropdisable = false;
        }
        if scip.conss[cons].updatefree {
            // Nothing to do here: the constraint is freed, when it is
            // released from the updateconss array.
            let c = &mut scip.conss[cons];
            c.updatefree = false;
            c.updateobsolete = false;
        } else {
            if scip.conss[cons].updateobsolete {
                let exceeds = cons_exceeds_obsoleteage(scip, cons);
                if !scip.conss[cons].obsolete && exceeds {
                    // The constraint's status must be switched to obsolete.
                    conshdlr_mark_cons_obsolete(scip, h, cons);
                } else if scip.conss[cons].obsolete && !exceeds {
                    // The constraint's status must be switched to useful.
                    conshdlr_mark_cons_useful(scip, h, cons);
                }
                scip.conss[cons].updateobsolete = false;
            }
            if scip.conss[cons].updatemarkpropagate {
                // The constraint must be marked to be propagated.
                conshdlr_mark_cons_propagate(scip, h, cons);
                scip.conss[cons].updatemarkpropagate = false;
            } else if scip.conss[cons].updateunmarkpropagate {
                // The constraint must be unmarked to be propagated.
                conshdlr_unmark_cons_propagate(scip, h, cons);
                scip.conss[cons].updateunmarkpropagate = false;
            }
        }
        scip.conss[cons].update = false;

        // Release the constraint.
        cons_release(scip, cons)?;
        scip.conshdlrs[h].updateconss[i] = crate::var::NONE;
        i -= 1;
    }
    scip.conshdlrs[h].nupdateconss = 0;
    Ok(())
}

/// `conshdlrDelayUpdates`: marks constraint handler to delay all
/// constraint updates until the next conshdlrProcessUpdates() call.
fn conshdlr_delay_updates(scip: &mut Scip, h: ConshdlrId) {
    scip.conshdlrs[h].delayupdatecount += 1;
}

/// `conshdlrForceUpdates`: marks constraint handler to perform all
/// constraint updates immediately; all delayed constraint updates will be
/// processed.
fn conshdlr_force_updates(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    scip.conshdlrs[h].delayupdatecount -= 1;

    // Only run the update if all delays are taken away (reference counting).
    if !conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        conshdlr_process_updates(scip, h)?;
    }
    Ok(())
}

/// `conshdlrAddUpdateCons`: adds constraint to constraint handler's update
/// constraint array and captures it.
fn conshdlr_add_update_cons(scip: &mut Scip, h: ConshdlrId, cons: ConsId) {
    if !scip.conss[cons].update {
        let n = scip.conshdlrs[h].nupdateconss;
        conshdlr_ensure_updateconss_mem(scip, h, n + 1);
        let c = &mut scip.conshdlrs[h];
        c.updateconss[c.nupdateconss] = cons;
        c.nupdateconss += 1;
        cons_capture(scip, cons);
        scip.conss[cons].update = true;
    }
}

/// `SCIPconshdlrCompSepa`: compares two constraint handlers w.r.t. their
/// separation priority.
pub fn conshdlr_comp_sepa(scip: &Scip, h1: ConshdlrId, h2: ConshdlrId) -> i32 {
    scip.conshdlrs[h2].sepapriority - scip.conshdlrs[h1].sepapriority
}

/// `SCIPconshdlrCompEnfo`: compares two constraint handlers w.r.t. their
/// enforcing priority.
pub fn conshdlr_comp_enfo(scip: &Scip, h1: ConshdlrId, h2: ConshdlrId) -> i32 {
    scip.conshdlrs[h2].enfopriority - scip.conshdlrs[h1].enfopriority
}

/// `SCIPconshdlrCompCheck`: compares two constraint handlers w.r.t. their
/// feasibility check priority.
pub fn conshdlr_comp_check(scip: &Scip, h1: ConshdlrId, h2: ConshdlrId) -> i32 {
    scip.conshdlrs[h2].checkpriority - scip.conshdlrs[h1].checkpriority
}

/// `SCIPconsCompCheck`: compares two constraints w.r.t. their feasibility
/// check priority.
pub fn cons_comp_check(scip: &Scip, cons1: ConsId, cons2: ConsId) -> i32 {
    scip.conshdlrs[scip.conss[cons2].conshdlr].checkpriority - scip.conshdlrs[scip.conss[cons1].conshdlr].checkpriority
}

/// The callbacks of a constraint handler, all optional (`NULL` in C), for
/// [`conshdlr_create`].
#[derive(Default)]
pub struct ConshdlrMethods {
    pub conshdlrcopy: Option<ConshdlrCopy>,
    pub consfree: Option<ConsFree>,
    pub consinit: Option<ConsInit>,
    pub consexit: Option<ConsInit>,
    pub consinitpre: Option<ConsInit>,
    pub consexitpre: Option<ConsInit>,
    pub consinitsol: Option<ConsInit>,
    pub consexitsol: Option<ConsExitsol>,
    pub consdelete: Option<ConsDelete>,
    pub constrans: Option<ConsTrans>,
    pub consinitlp: Option<ConsInitlp>,
    pub conssepalp: Option<ConsSepalp>,
    pub conssepasol: Option<ConsSepasol>,
    pub consenfolp: Option<ConsEnfolp>,
    pub consenforelax: Option<ConsEnforelax>,
    pub consenfops: Option<ConsEnfops>,
    pub conscheck: Option<ConsCheck>,
    pub consprop: Option<ConsProp>,
    pub conspresol: Option<ConsPresol>,
    pub consresprop: Option<ConsResprop>,
    pub conslock: Option<ConsLock>,
    pub consactive: Option<ConsActive>,
    pub consdeactive: Option<ConsActive>,
    pub consenable: Option<ConsActive>,
    pub consdisable: Option<ConsActive>,
    pub consdelvars: Option<ConsInit>,
    pub conscopy: Option<ConsCopy>,
    pub consgetvars: Option<ConsGetvars>,
    pub consgetnvars: Option<ConsGetnvars>,
    pub consgetdivebdchgs: Option<ConsGetdivebdchgs>,
}

/// `SCIPconshdlrCopyInclude`: copies the given constraint handler to a new
/// scip.
pub fn conshdlr_copy_include(scip: &mut Scip, sourcescip: &Scip, h: ConshdlrId, valid: &mut bool) -> ScipResult {
    if let Some(conshdlrcopy) = sourcescip.conshdlrs[h].conshdlrcopy {
        conshdlrcopy(scip, sourcescip, h, valid)?;
    }
    Ok(())
}

/// `SCIPconshdlrCreate`: creates a constraint handler, adds it to
/// `Scip::conshdlrs`, and adds its parameters.
#[allow(clippy::too_many_arguments)]
pub fn conshdlr_create(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    sepapriority: i32,
    enfopriority: i32,
    checkpriority: i32,
    sepafreq: i32,
    propfreq: i32,
    eagerfreq: i32,
    maxprerounds: i32,
    delaysepa: bool,
    delayprop: bool,
    needscons: bool,
    proptiming: PropTiming,
    presoltiming: PresolTiming,
    methods: ConshdlrMethods,
    conshdlrdata: Option<Box<dyn Any>>,
) -> ScipResult<ConshdlrId> {
    // The interface change from delay flags to timings cannot be recognized
    // at compile time: exit with an appropriate error message.
    if !(PRESOLTIMING_NONE..=PRESOLTIMING_MAX).contains(&presoltiming) {
        log::error!("ERROR: 'PRESOLDELAY'-flag no longer available since SCIP 3.2, use an appropriate 'SCIP_PRESOLTIMING' for <{name}> constraint handler instead.");
        return Err(crate::retcode::Retcode::ParameterWrongVal);
    }
    let m = methods;
    let conshdlr = Conshdlr {
        name: name.to_string(),
        desc: desc.to_string(),
        sepapriority,
        enfopriority,
        checkpriority,
        sepafreq,
        propfreq,
        eagerfreq,
        maxprerounds,
        conshdlrcopy: m.conshdlrcopy,
        consfree: m.consfree,
        consinit: m.consinit,
        consexit: m.consexit,
        consinitpre: m.consinitpre,
        consexitpre: m.consexitpre,
        consinitsol: m.consinitsol,
        consexitsol: m.consexitsol,
        consdelete: m.consdelete,
        constrans: m.constrans,
        consinitlp: m.consinitlp,
        conssepalp: m.conssepalp,
        conssepasol: m.conssepasol,
        consenfolp: m.consenfolp,
        consenforelax: m.consenforelax,
        consenfops: m.consenfops,
        conscheck: m.conscheck,
        consprop: m.consprop,
        conspresol: m.conspresol,
        consresprop: m.consresprop,
        conslock: m.conslock,
        consactive: m.consactive,
        consdeactive: m.consdeactive,
        consenable: m.consenable,
        consdisable: m.consdisable,
        consdelvars: m.consdelvars,
        conscopy: m.conscopy,
        consgetvars: m.consgetvars,
        consgetnvars: m.consgetnvars,
        consgetdivebdchgs: m.consgetdivebdchgs,
        conshdlrdata,
        conss: CVec::new(),
        consssize: 0,
        nconss: 0,
        nactiveconss: 0,
        maxnactiveconss: 0,
        startnactiveconss: 0,
        initconss: CVec::new(),
        initconsssize: 0,
        ninitconss: 0,
        ninitconsskept: 0,
        sepaconss: CVec::new(),
        sepaconsssize: 0,
        nsepaconss: 0,
        nusefulsepaconss: 0,
        enfoconss: CVec::new(),
        enfoconsssize: 0,
        nenfoconss: 0,
        nusefulenfoconss: 0,
        checkconss: CVec::new(),
        checkconsssize: 0,
        ncheckconss: 0,
        nusefulcheckconss: 0,
        propconss: CVec::new(),
        propconsssize: 0,
        npropconss: 0,
        nusefulpropconss: 0,
        nmarkedpropconss: 0,
        updateconss: CVec::new(),
        updateconsssize: 0,
        nupdateconss: 0,
        nenabledconss: 0,
        lastnusefulpropconss: 0,
        lastnusefulsepaconss: 0,
        lastnusefulenfoconss: 0,
        storedpropconss: CVec::new(),
        storedpropconsssize: 0,
        storednmarkedpropconss: 0,
        storedpropdomchgcount: 0,
        setuptime: Clock::new(),
        presoltime: Clock::new(),
        sepatime: Clock::new(),
        enfolptime: Clock::new(),
        enfopstime: Clock::new(),
        enforelaxtime: Clock::new(),
        proptime: Clock::new(),
        sbproptime: Clock::new(),
        checktime: Clock::new(),
        resproptime: Clock::new(),
        nsepacalls: 0,
        nenfolpcalls: 0,
        nenfopscalls: 0,
        nenforelaxcalls: 0,
        npropcalls: 0,
        ncheckcalls: 0,
        nrespropcalls: 0,
        ncutoffs: 0,
        ncutsfound: 0,
        ncutsapplied: 0,
        nconssfound: 0,
        ndomredsfound: 0,
        nchildren: 0,
        lastpropdomchgcount: -1,
        lastsepalpcount: -1,
        lastenfolplpcount: -1,
        lastenfolpdomchgcount: -1,
        lastenfopsdomchgcount: -1,
        lastenforelaxdomchgcount: -1,
        lastenforelaxrelaxcount: -1,
        lastenfolpnode: -1,
        lastenfopsnode: -1,
        lastenforelaxnode: 0,
        lastenfolpresult: ScipResultCode::DidNotRun,
        lastenfopsresult: ScipResultCode::DidNotRun,
        lastenforelaxresult: ScipResultCode::DidNotRun,
        lastnfixedvars: 0,
        lastnaggrvars: 0,
        lastnchgvartypes: 0,
        lastnchgbds: 0,
        lastnaddholes: 0,
        lastndelconss: 0,
        lastnaddconss: 0,
        lastnupgdconss: 0,
        lastnchgcoefs: 0,
        lastnchgsides: 0,
        nfixedvars: 0,
        naggrvars: 0,
        nchgvartypes: 0,
        nchgbds: 0,
        naddholes: 0,
        ndelconss: 0,
        naddconss: 0,
        nupgdconss: 0,
        nchgcoefs: 0,
        nchgsides: 0,
        npresolcalls: 0,
        delayupdatecount: 0,
        ageresetavg: AGERESETAVG_INIT,
        needscons,
        sepalpwasdelayed: false,
        sepasolwasdelayed: false,
        propwasdelayed: false,
        duringsepa: false,
        duringprop: false,
        exact: false,
        initialized: false,
        delaysepa,
        delayprop,
        proptiming,
        presoltiming,
    };
    let h = scip.conshdlrs.push(conshdlr);

    // Add parameters.
    use crate::paramset::{paramset_add_bool, paramset_add_int};
    let maxdepth = crate::def::MAXTREEDEPTH;
    paramset_add_int(
        scip,
        &format!("constraints/{name}/sepafreq"),
        "frequency for separating cuts (-1: never, 0: only in root node)",
        crate::field_target!(i32, conshdlrs, h, sepafreq),
        false,
        sepafreq,
        -1,
        maxdepth,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("constraints/{name}/propfreq"),
        "frequency for propagating domains (-1: never, 0: only in root node)",
        crate::field_target!(i32, conshdlrs, h, propfreq),
        false,
        propfreq,
        -1,
        maxdepth,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("constraints/{name}/proptiming"),
        &format!(
            "timing when constraint propagation should be called ({}:BEFORELP, {}:DURINGLPLOOP, {}:AFTERLPLOOP, {}:ALWAYS)",
            PROPTIMING_BEFORELP, PROPTIMING_DURINGLPLOOP, PROPTIMING_AFTERLPLOOP, PROPTIMING_ALWAYS
        ),
        crate::field_target!(i32, conshdlrs, h, proptiming),
        true,
        proptiming,
        PROPTIMING_BEFORELP,
        PROPTIMING_ALWAYS,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("constraints/{name}/eagerfreq"),
        "frequency for using all instead of only the useful constraints in separation, propagation and enforcement (-1: never, 0: only in first evaluation)",
        crate::field_target!(i32, conshdlrs, h, eagerfreq),
        true,
        eagerfreq,
        -1,
        maxdepth,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("constraints/{name}/maxprerounds"),
        "maximal number of presolving rounds the constraint handler participates in (-1: no limit)",
        crate::field_target!(i32, conshdlrs, h, maxprerounds),
        true,
        maxprerounds,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("constraints/{name}/delaysepa"),
        "should separation method be delayed, if other separators found cuts?",
        crate::field_target!(bool, conshdlrs, h, delaysepa),
        true,
        delaysepa,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("constraints/{name}/delayprop"),
        "should propagation method be delayed, if other propagators found reductions?",
        crate::field_target!(bool, conshdlrs, h, delayprop),
        true,
        delayprop,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("constraints/{name}/presoltiming"),
        &format!(
            "timing mask of the constraint handler's presolving method ({}:FAST, {}:MEDIUM, {}:EXHAUSTIVE, {}:FINAL)",
            PRESOLTIMING_FAST, PRESOLTIMING_MEDIUM, PRESOLTIMING_EXHAUSTIVE, PRESOLTIMING_FINAL
        ),
        crate::field_target!(i32, conshdlrs, h, presoltiming),
        true,
        presoltiming,
        PRESOLTIMING_FAST,
        PRESOLTIMING_MAX,
        None,
    )?;
    Ok(h)
}

/// `SCIPconshdlrFree`: calls destructor and frees memory of constraint
/// handler.
pub fn conshdlr_free(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    // Call destructor of constraint handler.
    if let Some(consfree) = scip.conshdlrs[h].consfree {
        consfree(scip, h)?;
    }
    scip.conshdlrs.remove(h);
    Ok(())
}

/// Runs one of the handler's setup callbacks on its constraints under the
/// setup clock, with constraint updates delayed meanwhile.
fn conshdlr_setup_call(scip: &mut Scip, h: ConshdlrId, call: impl FnOnce(&mut Scip, &[ConsId]) -> ScipResult) -> ScipResult {
    // Because during constraint processing, constraints of this handler
    // may be deleted, activated, deactivated, enabled, disabled, marked
    // obsolete or useful, which would change the conss array given to the
    // external method; to avoid this, these changes will be buffered and
    // processed after the method call.
    conshdlr_delay_updates(scip, h);
    let time_enabled = scip.set.time_enabled;
    scip.conshdlrs[h].setuptime.start(time_enabled);
    let c = &scip.conshdlrs[h];
    let conss = c.conss.head(c.nconss).to_vec();
    call(scip, &conss)?;
    scip.conshdlrs[h].setuptime.stop(time_enabled);
    conshdlr_force_updates(scip, h)
}

/// `SCIPconshdlrInit`: calls init method of constraint handler.
pub fn conshdlr_init(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }
    if scip.conshdlrs[h].initialized {
        log::error!("constraint handler <{}> already initialized", scip.conshdlrs[h].name);
        return Err(crate::retcode::Retcode::InvalidCall);
    }
    if scip.set.misc_resetstat {
        let c = &mut scip.conshdlrs[h];
        c.setuptime.reset();
        c.presoltime.reset();
        c.sepatime.reset();
        c.enfolptime.reset();
        c.enfopstime.reset();
        c.enforelaxtime.reset();
        c.proptime.reset();
        c.sbproptime.reset();
        c.checktime.reset();
        c.resproptime.reset();
        c.nsepacalls = 0;
        c.nenfolpcalls = 0;
        c.nenfopscalls = 0;
        c.nenforelaxcalls = 0;
        c.npropcalls = 0;
        c.ncheckcalls = 0;
        c.nrespropcalls = 0;
        c.ncutoffs = 0;
        c.ncutsfound = 0;
        c.ncutsapplied = 0;
        c.nconssfound = 0;
        c.ndomredsfound = 0;
        c.nchildren = 0;
        c.lastpropdomchgcount = -1;
        c.lastenfolpdomchgcount = -1;
        c.lastenfopsdomchgcount = -1;
        c.lastenforelaxdomchgcount = -1;
        c.lastenforelaxrelaxcount = -1;
        c.lastenfolpnode = -1;
        c.lastenfopsnode = -1;
        c.lastenfolpresult = ScipResultCode::DidNotRun;
        c.lastenfopsresult = ScipResultCode::DidNotRun;
        c.maxnactiveconss = c.nactiveconss;
        c.startnactiveconss = 0;
        c.lastsepalpcount = -1;
        c.lastenfolplpcount = -1;
        c.lastnusefulpropconss = 0;
        c.lastnusefulsepaconss = 0;
        c.lastnusefulenfoconss = 0;
        c.lastnfixedvars = 0;
        c.lastnaggrvars = 0;
        c.lastnchgvartypes = 0;
        c.lastnchgbds = 0;
        c.lastnaddholes = 0;
        c.lastndelconss = 0;
        c.lastnaddconss = 0;
        c.lastnupgdconss = 0;
        c.lastnchgcoefs = 0;
        c.lastnchgsides = 0;
        c.nfixedvars = 0;
        c.naggrvars = 0;
        c.nchgvartypes = 0;
        c.nchgbds = 0;
        c.naddholes = 0;
        c.ndelconss = 0;
        c.naddconss = 0;
        c.nupgdconss = 0;
        c.nchgcoefs = 0;
        c.nchgsides = 0;
        c.npresolcalls = 0;
        c.ageresetavg = AGERESETAVG_INIT;
        c.sepalpwasdelayed = false;
        c.sepasolwasdelayed = false;
        c.propwasdelayed = false;
    }

    // Call initialization method of constraint handler.
    if let Some(consinit) = scip.conshdlrs[h].consinit {
        conshdlr_setup_call(scip, h, |scip, conss| consinit(scip, h, conss))?;
    }
    scip.conshdlrs[h].initialized = true;
    Ok(())
}

/// `SCIPconshdlrExit`: calls exit method of constraint handler.
pub fn conshdlr_exit(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }
    if !scip.conshdlrs[h].initialized {
        log::error!("constraint handler <{}> not initialized", scip.conshdlrs[h].name);
        return Err(crate::retcode::Retcode::InvalidCall);
    }

    // Call deinitialization method of constraint handler.
    if let Some(consexit) = scip.conshdlrs[h].consexit {
        conshdlr_setup_call(scip, h, |scip, conss| consexit(scip, h, conss))?;
    }
    scip.conshdlrs[h].initialized = false;
    Ok(())
}

/// `SCIPconshdlrInitpre`: informs constraint handler that the presolving
/// process is being started.
pub fn conshdlr_initpre(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }

    // Reset conshdlr last presolved data in case of a restart.
    let c = &mut scip.conshdlrs[h];
    c.lastpropdomchgcount = -1;
    c.lastenfolpdomchgcount = -1;
    c.lastenfopsdomchgcount = -1;
    c.lastenforelaxdomchgcount = -1;
    c.lastenforelaxrelaxcount = -1;
    c.lastenfolpnode = -1;
    c.lastenfopsnode = -1;
    c.lastenfolpresult = ScipResultCode::DidNotRun;
    c.lastenfopsresult = ScipResultCode::DidNotRun;
    c.lastenforelaxresult = ScipResultCode::DidNotRun;
    c.maxnactiveconss = c.nactiveconss;
    c.startnactiveconss = 0;
    c.lastsepalpcount = -1;
    c.lastenfolplpcount = -1;
    c.lastnusefulpropconss = 0;
    c.lastnusefulsepaconss = 0;
    c.lastnusefulenfoconss = 0;
    c.lastnfixedvars = 0;
    c.lastnaggrvars = 0;
    c.lastnchgvartypes = 0;
    c.lastnchgbds = 0;
    c.lastnaddholes = 0;
    c.lastndelconss = 0;
    c.lastnaddconss = 0;
    c.lastnupgdconss = 0;
    c.lastnchgcoefs = 0;
    c.lastnchgsides = 0;
    c.propwasdelayed = false;

    // Call presolving initialization method of constraint handler.
    if let Some(consinitpre) = scip.conshdlrs[h].consinitpre {
        conshdlr_setup_call(scip, h, |scip, conss| consinitpre(scip, h, conss))?;
    }

    // After a restart the LP is empty but the initial constraints are not
    // included in the initialconss array anymore; we have to put them back
    // into this array in order to obtain the correct initial root
    // relaxation.
    if scip.stat.nruns >= 2 {
        for c in 0..scip.conshdlrs[h].nconss {
            let cons = scip.conshdlrs[h].conss[c];
            let co = &scip.conss[cons];
            if co.addarraypos >= 0 && !co.deleted && co.initial && co.initconsspos == -1 {
                conshdlr_add_initcons(scip, h, cons);
            }
        }
    }
    Ok(())
}

/// `SCIPconshdlrExitpre`: informs constraint handler that the presolving
/// is finished.
pub fn conshdlr_exitpre(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }

    // Call presolving deinitialization method of constraint handler.
    if let Some(consexitpre) = scip.conshdlrs[h].consexitpre {
        conshdlr_setup_call(scip, h, |scip, conss| consexitpre(scip, h, conss))?;
    }

    // Update statistics.
    let c = &mut scip.conshdlrs[h];
    c.maxnactiveconss = c.nactiveconss;
    c.startnactiveconss = c.nactiveconss;
    Ok(())
}

/// `SCIPconshdlrInitsol`: informs constraint handler that the branch and
/// bound process is being started.
pub fn conshdlr_initsol(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }
    scip.conshdlrs[h].sepalpwasdelayed = false;
    scip.conshdlrs[h].sepasolwasdelayed = false;

    // Call solving process initialization method of constraint handler.
    if let Some(consinitsol) = scip.conshdlrs[h].consinitsol {
        conshdlr_setup_call(scip, h, |scip, conss| consinitsol(scip, h, conss))?;
    }
    Ok(())
}

/// `SCIPconshdlrExitsol`: informs constraint handler that the branch and
/// bound process data is being freed.
pub fn conshdlr_exitsol(scip: &mut Scip, h: ConshdlrId, restart: bool) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }

    // Call solving process deinitialization method of constraint handler.
    if let Some(consexitsol) = scip.conshdlrs[h].consexitsol {
        conshdlr_setup_call(scip, h, |scip, conss| consexitsol(scip, h, conss, restart))?;
    }
    Ok(())
}

/// `SCIPconshdlrInitLP`: calls LP initialization method of constraint
/// handler to separate all initial active constraints; returns whether
/// infeasibility was detected while building the LP.
pub fn conshdlr_init_lp(scip: &mut Scip, h: ConshdlrId, initkeptconss: bool) -> ScipResult<bool> {
    let mut cutoff = false;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(cutoff);
    }
    let Some(consinitlp) = scip.conshdlrs[h].consinitlp else {
        return Ok(cutoff);
    };
    let c = &scip.conshdlrs[h];
    if c.needscons && (c.ninitconss == 0 || (!initkeptconss && c.ninitconss == c.ninitconsskept)) {
        return Ok(cutoff);
    }

    // Because during constraint processing, constraints of this handler
    // may be deleted, activated, deactivated, enabled, disabled, marked
    // obsolete or useful, which would change the conss array given to the
    // external method; to avoid this, these changes will be buffered and
    // processed after the method call.
    conshdlr_delay_updates(scip, h);
    let oldninitconss = scip.conshdlrs[h].ninitconss;
    let time_enabled = scip.set.time_enabled;
    scip.conshdlrs[h].sepatime.start(time_enabled);
    if initkeptconss {
        // Add all kept initial constraints which are currently active to
        // the second part of the initconss array; we have to iterate over
        // all initial constraints, since the initconss array may also be
        // changed during this loop.
        for c in 0..scip.conshdlrs[h].ninitconsskept {
            let cons = scip.conshdlrs[h].initconss[c];
            if scip.conss[cons].active {
                conshdlr_add_initcons(scip, h, cons);
            }
        }
    }

    // Call LP initialization method of constraint handler.
    let c = &scip.conshdlrs[h];
    let conss = c.initconss.0[c.ninitconsskept as usize..c.ninitconss as usize].to_vec();
    consinitlp(scip, h, &conss, &mut cutoff)?;
    scip.conshdlrs[h].sepatime.stop(time_enabled);

    // Perform the cached constraint updates.
    conshdlr_force_updates(scip, h)?;

    // Clear the initconss array: keep only the initial constraints which
    // were activated at a more global node than the current one.
    let currentdepth = crate::tree::tree_get_current_depth(scip);
    let mut c = scip.conshdlrs[h].ninitconsskept;
    while c < oldninitconss {
        let cons = scip.conshdlrs[h].initconss[c];
        let activedepth = scip.conss[cons].activedepth;
        if if currentdepth > 0 { activedepth != currentdepth } else { activedepth > 0 } {
            let kept = scip.conshdlrs[h].ninitconsskept;
            cons_move!(scip, h, initconss, initconsspos, c, kept);
            scip.conshdlrs[h].ninitconsskept += 1;
        } else {
            scip.conss[cons].initconsspos = -1;
        }
        c += 1;
    }
    let ch = &mut scip.conshdlrs[h];
    ch.ninitconss = ch.ninitconsskept;
    if ch.ninitconss == 0 {
        ch.initconss = CVec::new();
        ch.initconsssize = 0;
    }
    Ok(cutoff)
}

/// What a handler call is measured against: domain changes, cuts and
/// active constraints before it.
/// Cuts in the main separation storage; 0 before `initSolve` creates it
/// (presolve probing propagates, and propagation never reads cuts in C).
fn main_ncuts(scip: &Scip) -> i32 {
    scip.sepastores.get(crate::sepastore::SEPASTORE).map_or(0, |s| s.ncuts)
}

struct CallCounts {
    ndomchgs: i64,
    nprobdomchgs: i64,
    ncuts: i32,
    nactiveconss: i32,
}

impl CallCounts {
    fn take(scip: &Scip) -> Self {
        let stat = &scip.stat;
        CallCounts {
            ndomchgs: stat.nboundchgs + stat.nholechgs,
            nprobdomchgs: stat.nprobboundchgs + stat.nprobholechgs,
            ncuts: main_ncuts(scip),
            nactiveconss: stat.nactiveconss,
        }
    }

    /// Adds the cuts and constraints found since to the handler's
    /// statistics.
    fn add_found(&self, scip: &mut Scip, h: ConshdlrId) {
        let ncuts = main_ncuts(scip);
        let nactiveconss = scip.stat.nactiveconss;
        let c = &mut scip.conshdlrs[h];
        c.ncutsfound += (ncuts - self.ncuts) as i64;
        c.nconssfound += crate::def::max(nactiveconss - self.nactiveconss, 0) as i64;
    }

    /// Updates domain reductions; therefore removes the domain reduction
    /// counts which were generated in probing mode.
    fn add_domreds(&self, scip: &mut Scip, h: ConshdlrId) {
        let stat = &scip.stat;
        let c = &mut scip.conshdlrs[h];
        c.ndomredsfound += stat.nboundchgs + stat.nholechgs - self.ndomchgs;
        c.ndomredsfound -= stat.nprobboundchgs + stat.nprobholechgs - self.nprobdomchgs;
    }
}

/// Whether the handler evaluates all instead of only the useful
/// constraints on its `ncalls`-th call (its eager frequency).
fn conshdlr_is_eager(c: &Conshdlr, ncalls: i64) -> bool {
    (c.eagerfreq == 0 && ncalls == 0) || (c.eagerfreq > 0 && ncalls % c.eagerfreq as i64 == 0)
}

/// `SCIPconshdlrSeparateLP`: calls separator method of constraint handler
/// to separate LP solution.
pub fn conshdlr_separate_lp(scip: &mut Scip, h: ConshdlrId, depth: i32, execdelayed: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode::*;
    let mut result = DidNotRun;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(result);
    }
    let c = &scip.conshdlrs[h];
    let Some(conssepalp) = c.conssepalp else {
        return Ok(result);
    };
    if !((depth == 0 && c.sepafreq == 0) || (c.sepafreq > 0 && depth % c.sepafreq == 0) || c.sepalpwasdelayed) {
        return Ok(result);
    }

    // Check, if separation method should be delayed.
    if !c.delaysepa || execdelayed {
        let lpcount = scip.stat.lpcount;
        let (nconss, mut nusefulconss, firstcons) = if c.lastsepalpcount == lpcount {
            // All constraints that were not yet separated on the new LP
            // solution must be useful constraints, which means, that the
            // new constraints are the last constraints of the useful ones.
            let n = c.nusefulsepaconss - c.lastnusefulsepaconss;
            (n, n, c.lastnusefulsepaconss)
        } else {
            // On a new LP solution, we want to separate all constraints.
            (c.nsepaconss, c.nusefulsepaconss, 0)
        };

        // Constraint handlers without constraints should only be called
        // once.
        if nconss > 0 || (!c.needscons && c.lastsepalpcount != lpcount) {
            // Remember the number of processed constraints on the current
            // LP solution.
            let lastsepalpcount = lpcount;
            let lastnusefulsepaconss = c.nusefulsepaconss;

            // Get the array of the constraints to be processed.
            let conss = c.sepaconss.0[firstcons as usize..(firstcons + nconss) as usize].to_vec();
            let counts = CallCounts::take(scip);

            // Check, if we want to use eager evaluation.
            if conshdlr_is_eager(&scip.conshdlrs[h], scip.conshdlrs[h].nsepacalls) {
                nusefulconss = nconss;
            }

            // Because during constraint processing, constraints of this
            // handler may be deleted, activated, deactivated, enabled,
            // disabled, marked obsolete or useful, which would change the
            // conss array given to the external method; to avoid this,
            // these changes will be buffered and processed after the
            // method call.
            conshdlr_delay_updates(scip, h);
            scip.conshdlrs[h].duringsepa = true;
            let time_enabled = scip.set.time_enabled;
            scip.conshdlrs[h].sepatime.start(time_enabled);
            conssepalp(scip, h, &conss, nusefulconss, &mut result)?;
            scip.conshdlrs[h].sepatime.stop(time_enabled);
            scip.conshdlrs[h].duringsepa = false;

            // Perform the cached constraint updates.
            conshdlr_force_updates(scip, h)?;

            // Update statistics.
            if result != DidNotRun && result != Delayed {
                let c = &mut scip.conshdlrs[h];
                c.lastsepalpcount = lastsepalpcount;
                c.lastnusefulsepaconss = crate::def::min(lastnusefulsepaconss, c.nusefulsepaconss);
                c.nsepacalls += 1;
            }
            if result == Cutoff {
                scip.conshdlrs[h].ncutoffs += 1;
            }
            counts.add_found(scip, h);
            counts.add_domreds(scip, h);

            // Evaluate result.
            if !matches!(result, Cutoff | ConsAdded | ReducedDom | Separated | NewRound | DidNotFind | DidNotRun | Delayed) {
                log::error!(
                    "LP separation method of constraint handler <{}> returned invalid result <{}>",
                    scip.conshdlrs[h].name,
                    result as i32
                );
                return Err(crate::retcode::Retcode::InvalidResult);
            }
        }
    } else {
        result = Delayed;
    }

    // Remember whether separation method was delayed.
    scip.conshdlrs[h].sepalpwasdelayed = result == Delayed;
    Ok(result)
}

/// `SCIPconshdlrSeparateSol`: calls separator method of constraint handler
/// to separate given primal solution.
pub fn conshdlr_separate_sol(scip: &mut Scip, h: ConshdlrId, sol: SolId, depth: i32, execdelayed: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode::*;
    let mut result = DidNotRun;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(result);
    }
    let c = &scip.conshdlrs[h];
    let Some(conssepasol) = c.conssepasol else {
        return Ok(result);
    };
    if !((depth == 0 && c.sepafreq == 0) || (c.sepafreq > 0 && depth % c.sepafreq == 0) || c.sepasolwasdelayed) {
        return Ok(result);
    }

    // Check, if separation method should be delayed.
    if !c.delaysepa || execdelayed {
        // Always separate all constraints.
        let nconss = c.nsepaconss;
        let mut nusefulconss = c.nusefulsepaconss;
        if nconss > 0 || !c.needscons {
            // Get the array of the constraints to be processed.
            let conss = c.sepaconss.head(nconss).to_vec();
            let counts = CallCounts::take(scip);

            // Check, if we want to use eager evaluation.
            if conshdlr_is_eager(&scip.conshdlrs[h], scip.conshdlrs[h].nsepacalls) {
                nusefulconss = nconss;
            }

            // Constraint updates are buffered during the call.
            conshdlr_delay_updates(scip, h);
            scip.conshdlrs[h].duringsepa = true;
            let time_enabled = scip.set.time_enabled;
            scip.conshdlrs[h].sepatime.start(time_enabled);
            conssepasol(scip, h, &conss, nusefulconss, sol, &mut result)?;
            scip.conshdlrs[h].sepatime.stop(time_enabled);
            scip.conshdlrs[h].duringsepa = false;

            // Perform the cached constraint updates.
            conshdlr_force_updates(scip, h)?;

            // Update statistics.
            if result != DidNotRun && result != Delayed {
                scip.conshdlrs[h].nsepacalls += 1;
            }
            if result == Cutoff {
                scip.conshdlrs[h].ncutoffs += 1;
            }
            counts.add_found(scip, h);
            counts.add_domreds(scip, h);

            // Evaluate result.
            if !matches!(result, Cutoff | ConsAdded | ReducedDom | Separated | NewRound | DidNotFind | DidNotRun | Delayed) {
                log::error!(
                    "SOL separation method of constraint handler <{}> returned invalid result <{}>",
                    scip.conshdlrs[h].name,
                    result as i32
                );
                return Err(crate::retcode::Retcode::InvalidResult);
            }
        }
    } else {
        result = Delayed;
    }

    // Remember whether separation method was delayed.
    scip.conshdlrs[h].sepasolwasdelayed = result == Delayed;
    Ok(result)
}

/// `SCIPconshdlrEnforceRelaxSol`: calls enforcing method of constraint
/// handler for a relaxation solution; only relaxators produce those, and
/// none is included.
pub fn conshdlr_enforce_relax_sol(_scip: &mut Scip, _h: ConshdlrId, _relaxsol: SolId, _solinfeasible: bool) -> ScipResult<ScipResultCode> {
    todo!("SCIPconshdlrEnforceRelaxSol: no relaxator is included")
}

/// `SCIPconshdlrEnforceLPSol`: calls enforcing method of constraint
/// handler for LP solution for all constraints added after last
/// conshdlrResetEnfo() call.
pub fn conshdlr_enforce_lp_sol(scip: &mut Scip, h: ConshdlrId, solinfeasible: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode::*;
    let mut result = Feasible;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(result);
    }
    let c = &scip.conshdlrs[h];
    let Some(consenfolp) = c.consenfolp else {
        return Ok(result);
    };
    let stat = &scip.stat;

    // Check, if this LP solution was already enforced at this node.
    let (nconss, mut nusefulconss, firstcons, lpchanged, lastinfeasible);
    if c.lastenfolplpcount == stat.lpcount && c.lastenfolpdomchgcount == stat.domchgcount && c.lastenfolpnode == stat.nnodes && c.lastenfolpresult != ConsAdded {
        // If we already enforced the same pseudo solution at this node, we
        // will only enforce new constraints in the following; however, the
        // result of the last call for the old constraint is still valid
        // and we have to ensure that an infeasibility in the last call is
        // not lost because we only enforce new constraints.
        if c.lastenfolpresult == Feasible {
            lastinfeasible = false;
        } else {
            result = Infeasible;
            lastinfeasible = true;
        }

        // All constraints that were not yet enforced on the new LP solution
        // must be useful constraints, which means, that the new constraints
        // are the last constraints of the useful ones.
        nconss = c.nusefulenfoconss - c.lastnusefulenfoconss;
        nusefulconss = nconss;
        firstcons = c.lastnusefulenfoconss;
        lpchanged = false;
    } else {
        // On a new LP solution, we want to enforce all constraints.
        nconss = c.nenfoconss;
        nusefulconss = c.nusefulenfoconss;
        firstcons = 0;
        lpchanged = true;
        lastinfeasible = false;
    }

    // Constraint handlers without constraints should only be called once.
    if nconss > 0 || (!c.needscons && lpchanged) {
        // Remember the number of processed constraints on the current LP
        // solution.
        let (lpcount, domchgcount, nnodes) = (stat.lpcount, stat.domchgcount, stat.nnodes);
        let c = &mut scip.conshdlrs[h];
        c.lastenfolplpcount = lpcount;
        c.lastenfolpdomchgcount = domchgcount;
        c.lastenfolpnode = nnodes;
        c.lastnusefulenfoconss = c.nusefulenfoconss;

        // Get the array of the constraints to be processed.
        let conss = if nconss > 0 {
            c.enfoconss.0[firstcons as usize..(firstcons + nconss) as usize].to_vec()
        } else {
            Vec::new()
        };
        let counts = CallCounts::take(scip);

        // Check, if we want to use eager evaluation.
        if conshdlr_is_eager(&scip.conshdlrs[h], scip.conshdlrs[h].nenfolpcalls) {
            nusefulconss = nconss;
        }

        // Constraint updates are buffered during the call.
        conshdlr_delay_updates(scip, h);
        let time_enabled = scip.set.time_enabled;
        scip.conshdlrs[h].enfolptime.start(time_enabled);
        consenfolp(scip, h, &conss, nusefulconss, solinfeasible, &mut result)?;
        scip.conshdlrs[h].enfolptime.stop(time_enabled);

        // Perform the cached constraint updates.
        conshdlr_force_updates(scip, h)?;

        // Remember the result of the enforcement call.
        scip.conshdlrs[h].lastenfolpresult = result;

        // Update statistics.
        scip.conshdlrs[h].nenfolpcalls += 1;
        if result == Cutoff {
            scip.conshdlrs[h].ncutoffs += 1;
        }
        counts.add_found(scip, h);
        if result != Branched {
            counts.add_domreds(scip, h);
        } else {
            scip.conshdlrs[h].nchildren += tree!(scip).nchildren as i64;
        }

        // Evaluate result.
        if !matches!(result, Cutoff | ConsAdded | ReducedDom | Separated | SolveLp | Branched | Infeasible | Feasible) {
            log::error!(
                "enforcing method of constraint handler <{}> for LP solutions returned invalid result <{}>",
                scip.conshdlrs[h].name,
                result as i32
            );
            return Err(crate::retcode::Retcode::InvalidResult);
        }

        // If the same LP solution was already enforced at this node, we only
        // enforced new constraints this time; if the enfolp call returns
        // feasible now, the solution is only feasible w.r.t. the new
        // constraints, if the last call detected infeasibility for the old
        // constraints, we have to change the result to infeasible.
        if lastinfeasible && result == Feasible {
            result = Infeasible;
        }
    }
    Ok(result)
}

/// `SCIPconshdlrGetDiveBoundChanges`: calls diving solution enforcement
/// callback of constraint handler, if it exists; returns (success,
/// infeasible).
pub fn conshdlr_get_dive_bound_changes(scip: &mut Scip, h: ConshdlrId, diveset: usize, sol: SolId) -> ScipResult<(bool, bool)> {
    let mut success = false;
    let mut infeasible = false;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok((success, infeasible));
    }
    if let Some(consgetdivebdchgs) = scip.conshdlrs[h].consgetdivebdchgs {
        consgetdivebdchgs(scip, h, diveset, sol, &mut success, &mut infeasible)?;
    }
    Ok((success, infeasible))
}

/// `SCIPconshdlrEnforcePseudoSol`: calls enforcing method of constraint
/// handler for pseudo solution for all constraints added after last
/// conshdlrResetEnfo() call.
pub fn conshdlr_enforce_pseudo_sol(scip: &mut Scip, h: ConshdlrId, solinfeasible: bool, objinfeasible: bool, forced: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode::*;

    // No enforcing of pseudo solution.
    if scip.set.cons_disableenfops && crate::branch::branchcand_get_n_pseudo_cands(scip) > 0 {
        return Ok(Infeasible);
    }
    let mut result = Feasible;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(result);
    }
    let c = &scip.conshdlrs[h];
    let Some(consenfops) = c.consenfops else {
        return Ok(result);
    };
    let stat = &scip.stat;

    // Check, if this pseudo solution was already enforced at this node.
    let (nconss, mut nusefulconss, firstcons, pschanged, lastinfeasible);
    if !forced && c.lastenfopsdomchgcount == stat.domchgcount && c.lastenfopsnode == stat.nnodes && c.lastenfopsresult != ConsAdded && c.lastenfopsresult != SolveLp {
        // If we already enforced the same pseudo solution at this node, we
        // will only enforce new constraints in the following; however, the
        // result of the last call for the old constraint is still valid and
        // we have to ensure that an infeasibility in the last call is not
        // lost because we only enforce new constraints.
        if c.lastenfopsresult == Infeasible {
            result = Infeasible;
            lastinfeasible = true;
        } else {
            lastinfeasible = false;
        }

        // All constraints that were not yet enforced on the new LP solution
        // must be useful constraints, which means, that the new constraints
        // are the last constraints of the useful ones.
        nconss = c.nusefulenfoconss - c.lastnusefulenfoconss;
        nusefulconss = nconss;
        firstcons = c.lastnusefulenfoconss;
        pschanged = false;
    } else {
        // On a new pseudo solution, we want to enforce all constraints.
        nconss = c.nenfoconss;
        nusefulconss = c.nusefulenfoconss;
        firstcons = 0;
        pschanged = true;
        lastinfeasible = false;
    }

    // Constraint handlers without constraints should only be called once.
    if nconss > 0 || (!c.needscons && pschanged) {
        // Remember the number of processed constraints on the current
        // pseudo solution.
        let (domchgcount, nnodes) = (stat.domchgcount, stat.nnodes);
        let c = &mut scip.conshdlrs[h];
        c.lastenfopsdomchgcount = domchgcount;
        c.lastenfopsnode = nnodes;
        c.lastnusefulenfoconss = c.nusefulenfoconss;

        // Get the array of the constraints to be processed.
        let conss = if c.needscons {
            c.enfoconss.0[firstcons as usize..(firstcons + nconss) as usize].to_vec()
        } else {
            Vec::new()
        };
        let counts = CallCounts::take(scip);

        // Check, if we want to use eager evaluation.
        if conshdlr_is_eager(&scip.conshdlrs[h], scip.conshdlrs[h].nenfopscalls) {
            nusefulconss = nconss;
        }

        // Constraint updates are buffered during the call.
        conshdlr_delay_updates(scip, h);
        let time_enabled = scip.set.time_enabled;
        scip.conshdlrs[h].enfopstime.start(time_enabled);
        consenfops(scip, h, &conss, nusefulconss, solinfeasible, objinfeasible, &mut result)?;
        scip.conshdlrs[h].enfopstime.stop(time_enabled);

        // Perform the cached constraint updates.
        conshdlr_force_updates(scip, h)?;

        // Update statistics.
        if result != DidNotRun {
            scip.conshdlrs[h].nenfopscalls += 1;
        } else if !objinfeasible {
            log::error!(
                "enforcing method of constraint handler <{}> for pseudo solutions was skipped, even though the solution was not objective-infeasible",
                scip.conshdlrs[h].name
            );
            scip.conshdlrs[h].lastenfopsresult = result;
            return Err(crate::retcode::Retcode::InvalidResult);
        }
        // A constraint handler might return SCIP_DIDNOTRUN and not check any
        // constraints in case objinfeasible was TRUE; we change the result
        // to SCIP_INFEASIBLE in this case.
        else {
            result = Infeasible;
        }
        if result == Cutoff {
            scip.conshdlrs[h].ncutoffs += 1;
        }
        if result != Branched {
            counts.add_domreds(scip, h);
        } else {
            scip.conshdlrs[h].nchildren += tree!(scip).nchildren as i64;
        }

        // Remember the result of the enforcement call.
        scip.conshdlrs[h].lastenfopsresult = result;

        // Evaluate result.
        if !matches!(result, Cutoff | ConsAdded | ReducedDom | Branched | SolveLp | Infeasible | Feasible | DidNotRun) {
            log::error!(
                "enforcing method of constraint handler <{}> for pseudo solutions returned invalid result <{}>",
                scip.conshdlrs[h].name,
                result as i32
            );
            return Err(crate::retcode::Retcode::InvalidResult);
        }

        // If the same pseudo solution was already enforced at this node, we
        // only enforced new constraints this time; a feasible result then
        // keeps the last call's infeasibility.
        if lastinfeasible && result == Feasible {
            result = Infeasible;
        }
    } else if objinfeasible {
        // Even if nothing is enforced, the solution might still be
        // infeasible due to violating lower bound.
        result = Infeasible;
    }
    Ok(result)
}

/// `SCIPconshdlrCheck`: calls feasibility check method of constraint
/// handler.
#[allow(clippy::too_many_arguments)]
pub fn conshdlr_check(scip: &mut Scip, h: ConshdlrId, sol: SolId, checkintegrality: bool, checklprows: bool, printreason: bool, completely: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode::*;
    let mut result = Feasible;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(result);
    }
    let c = &scip.conshdlrs[h];
    if let Some(conscheck) = c.conscheck
        && (!c.needscons || c.ncheckconss > 0)
    {
        let conss = c.checkconss.head(c.ncheckconss).to_vec();

        // Constraint updates are buffered during the call.
        conshdlr_delay_updates(scip, h);
        let time_enabled = scip.set.time_enabled;
        scip.conshdlrs[h].checktime.start(time_enabled);
        conscheck(scip, h, &conss, sol, checkintegrality, checklprows, printreason, completely, &mut result)?;
        scip.conshdlrs[h].checktime.stop(time_enabled);

        // Update statistics.
        scip.conshdlrs[h].ncheckcalls += 1;

        // Perform the cached constraint updates.
        conshdlr_force_updates(scip, h)?;

        // Evaluate result.
        if result != Infeasible && result != Feasible {
            log::error!(
                "feasibility check of constraint handler <{}> returned invalid result <{}>",
                scip.conshdlrs[h].name,
                result as i32
            );
            return Err(crate::retcode::Retcode::InvalidResult);
        }
    }
    Ok(result)
}

/// `SCIPconshdlrPropagate`: calls propagation method of constraint handler.
#[allow(clippy::too_many_arguments)]
pub fn conshdlr_propagate(
    scip: &mut Scip,
    h: ConshdlrId,
    depth: i32,
    fullpropagation: bool,
    execdelayed: bool,
    instrongbranching: bool,
    proptiming: PropTiming,
) -> ScipResult<ScipResultCode> {
    use ScipResultCode::*;
    let mut result = DidNotRun;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(result);
    }
    let c = &scip.conshdlrs[h];
    let Some(consprop) = c.consprop else {
        return Ok(result);
    };
    if !((!c.needscons || c.npropconss > 0) && ((depth == 0 && c.propfreq == 0) || (c.propfreq > 0 && depth % c.propfreq == 0) || c.propwasdelayed)) {
        return Ok(result);
    }

    // Check, if propagation method should be delayed.
    if !c.delayprop || execdelayed {
        let domchgcount = scip.stat.domchgcount;
        let (nconss, mut nusefulconss, firstcons) = if !fullpropagation && c.lastpropdomchgcount == domchgcount && c.nmarkedpropconss == 0 {
            // All constraints that were not yet propagated on the new
            // domains must be useful constraints, which means, that the new
            // constraints are the last constraints of the useful ones.
            let n = c.nusefulpropconss - c.lastnusefulpropconss;
            (n, n, c.lastnusefulpropconss)
        } else {
            // On new domains, we want to propagate all constraints.
            (c.npropconss, c.nusefulpropconss, 0)
        };
        let nmarkedpropconss = c.nmarkedpropconss;

        // Constraint handlers without constraints should only be called
        // once.
        if nconss > 0 || fullpropagation || (!c.needscons && c.lastpropdomchgcount != domchgcount) {
            // Remember the number of processed constraints on the current
            // domains.
            let lastpropdomchgcount = domchgcount;
            let lastnusefulpropconss = c.nusefulpropconss;

            // Get the array of the constraints to be processed.
            let conss = if nconss > 0 {
                c.propconss.0[firstcons as usize..(firstcons + nconss) as usize].to_vec()
            } else {
                Vec::new()
            };
            let counts = CallCounts::take(scip);

            // Check, if we want to use eager evaluation.
            if conshdlr_is_eager(&scip.conshdlrs[h], scip.conshdlrs[h].npropcalls) {
                nusefulconss = nconss;
            }

            // Constraint updates are buffered during the call.
            conshdlr_delay_updates(scip, h);
            scip.conshdlrs[h].duringprop = true;
            let time_enabled = scip.set.time_enabled;
            if instrongbranching {
                scip.conshdlrs[h].sbproptime.start(time_enabled);
            } else {
                scip.conshdlrs[h].proptime.start(time_enabled);
            }
            consprop(scip, h, &conss, nusefulconss, nmarkedpropconss, proptiming, &mut result)?;
            if instrongbranching {
                scip.conshdlrs[h].sbproptime.stop(time_enabled);
            } else {
                scip.conshdlrs[h].proptime.stop(time_enabled);
            }
            scip.conshdlrs[h].duringprop = false;

            // Perform the cached constraint updates.
            conshdlr_force_updates(scip, h)?;

            // Update statistics.
            if result != DidNotRun && result != Delayed {
                let c = &mut scip.conshdlrs[h];
                c.lastpropdomchgcount = lastpropdomchgcount;
                c.lastnusefulpropconss = crate::def::min(c.nusefulpropconss, lastnusefulpropconss);
                c.npropcalls += 1;
            }
            if result == Cutoff {
                scip.conshdlrs[h].ncutoffs += 1;
            }
            counts.add_domreds(scip, h);

            // Check result code of callback method.
            if !matches!(result, Cutoff | ReducedDom | DidNotFind | DidNotRun | Delayed | DelayNode) {
                log::error!(
                    "propagation method of constraint handler <{}> returned invalid result <{}>",
                    scip.conshdlrs[h].name,
                    result as i32
                );
                return Err(crate::retcode::Retcode::InvalidResult);
            }
        }
    } else {
        result = Delayed;
    }

    // Remember whether propagation method was delayed.
    scip.conshdlrs[h].propwasdelayed = result == Delayed;
    Ok(result)
}

/// `SCIPconshdlrPresolve`: calls presolving method of constraint handler;
/// `totals` are all presolvers' counts, updated by the call.
pub fn conshdlr_presolve(scip: &mut Scip, h: ConshdlrId, timing: PresolTiming, nrounds: i32, totals: &mut PresolCounts) -> ScipResult<ScipResultCode> {
    use ScipResultCode::*;
    let mut result = DidNotRun;
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(result);
    }
    let c = &scip.conshdlrs[h];
    let Some(conspresol) = c.conspresol else {
        return Ok(result);
    };
    if !((!c.needscons || c.nactiveconss > 0) && (c.maxprerounds == -1 || c.npresolcalls < c.maxprerounds)) {
        return Ok(result);
    }

    // Check, if presolving method should be executed for the current
    // timing.
    if timing & c.presoltiming != 0 {
        // Calculate the number of changes since last call.
        let c = &mut scip.conshdlrs[h];
        let new = PresolCounts {
            nfixedvars: totals.nfixedvars - c.lastnfixedvars,
            naggrvars: totals.naggrvars - c.lastnaggrvars,
            nchgvartypes: totals.nchgvartypes - c.lastnchgvartypes,
            nchgbds: totals.nchgbds - c.lastnchgbds,
            naddholes: totals.naddholes - c.lastnaddholes,
            ndelconss: totals.ndelconss - c.lastndelconss,
            naddconss: totals.naddconss - c.lastnaddconss,
            nupgdconss: totals.nupgdconss - c.lastnupgdconss,
            nchgcoefs: totals.nchgcoefs - c.lastnchgcoefs,
            nchgsides: totals.nchgsides - c.lastnchgsides,
        };

        // Remember the old number of changes.
        c.lastnfixedvars = totals.nfixedvars;
        c.lastnaggrvars = totals.naggrvars;
        c.lastnchgvartypes = totals.nchgvartypes;
        c.lastnchgbds = totals.nchgbds;
        c.lastnaddholes = totals.naddholes;
        c.lastndelconss = totals.ndelconss;
        c.lastnaddconss = totals.naddconss;
        c.lastnupgdconss = totals.nupgdconss;
        c.lastnchgcoefs = totals.nchgcoefs;
        c.lastnchgsides = totals.nchgsides;
        let conss = c.conss.head(c.nactiveconss).to_vec();

        // Constraint updates are buffered during the call.
        conshdlr_delay_updates(scip, h);

        // Start timing.
        let time_enabled = scip.set.time_enabled;
        scip.conshdlrs[h].presoltime.start(time_enabled);

        // Call external method.
        conspresol(scip, h, &conss, nrounds, timing, &new, totals, &mut result)?;

        // Stop timing.
        scip.conshdlrs[h].presoltime.stop(time_enabled);

        // Perform the cached constraint updates.
        conshdlr_force_updates(scip, h)?;

        // Count the new changes.
        let c = &mut scip.conshdlrs[h];
        c.nfixedvars += totals.nfixedvars - c.lastnfixedvars;
        c.naggrvars += totals.naggrvars - c.lastnaggrvars;
        c.nchgvartypes += totals.nchgvartypes - c.lastnchgvartypes;
        c.nchgbds += totals.nchgbds - c.lastnchgbds;
        c.naddholes += totals.naddholes - c.lastnaddholes;
        c.ndelconss += totals.ndelconss - c.lastndelconss;
        c.naddconss += totals.naddconss - c.lastnaddconss;
        c.nupgdconss += totals.nupgdconss - c.lastnupgdconss;
        c.nchgcoefs += totals.nchgcoefs - c.lastnchgcoefs;
        c.nchgsides += totals.nchgsides - c.lastnchgsides;

        // Check result code of callback method.
        if !matches!(result, Cutoff | Unbounded | Success | DidNotFind | DidNotRun | Delayed) {
            log::error!("presolving method of constraint handler <{}> returned invalid result <{}>", c.name, result as i32);
            return Err(crate::retcode::Retcode::InvalidResult);
        }

        // Increase the number of calls, if the presolving method tried to
        // find reductions.
        if result != DidNotRun {
            c.npresolcalls += 1;
        }
    }
    Ok(result)
}

/// `SCIPconshdlrDelVars`: calls variable deletion method of constraint
/// handler.
pub fn conshdlr_del_vars(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }
    if let Some(consdelvars) = scip.conshdlrs[h].consdelvars {
        // Constraint updates are buffered during the call.
        conshdlr_delay_updates(scip, h);
        let c = &scip.conshdlrs[h];
        let conss = c.conss.head(c.nconss).to_vec();
        consdelvars(scip, h, &conss)?;

        // Perform the cached constraint updates.
        conshdlr_force_updates(scip, h)?;
    }
    Ok(())
}

/// `SCIPconshdlrLockVars`: locks rounding of variables involved in the
/// given constraint constraint handler that doesn't need constraints.
pub fn conshdlr_lock_vars(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }
    let conslock = scip.conshdlrs[h].conslock.expect("a lock method");
    conslock(scip, h, crate::var::NONE, LockType::Model, 1, 0)
}

/// `SCIPconshdlrUnlockVars`: unlocks rounding of variables involved in the
/// given constraint constraint handler that doesn't need constraints.
pub fn conshdlr_unlock_vars(scip: &mut Scip, h: ConshdlrId) -> ScipResult {
    if scip.set.exact_enable && !scip.conshdlrs[h].exact {
        return Ok(());
    }
    let conslock = scip.conshdlrs[h].conslock.expect("a lock method");
    conslock(scip, h, crate::var::NONE, LockType::Model, -1, 0)
}

/// `SCIPconshdlrNeedsCons`: does the constraint handler need a constraint
/// in order to be called?
pub fn conshdlr_needs_cons(scip: &Scip, h: ConshdlrId) -> bool {
    scip.conshdlrs[h].needscons
}

/// `SCIPconshdlrSetNeedsCons`.
pub fn conshdlr_set_needs_cons(scip: &mut Scip, h: ConshdlrId, needscons: bool) {
    scip.conshdlrs[h].needscons = needscons;
}

/// `SCIPconshdlrDoesPresolve`: does the constraint handler perform
/// presolving?
pub fn conshdlr_does_presolve(scip: &Scip, h: ConshdlrId) -> bool {
    scip.conshdlrs[h].conspresol.is_some()
}

/// `SCIPconshdlrSetSepa`: sets all separation related callbacks/parameters
/// of the constraint handler.
#[allow(clippy::too_many_arguments)]
pub fn conshdlr_set_sepa(scip: &mut Scip, h: ConshdlrId, conssepalp: Option<ConsSepalp>, conssepasol: Option<ConsSepasol>, sepafreq: i32, sepapriority: i32, delaysepa: bool) {
    let c = &mut scip.conshdlrs[h];
    c.conssepalp = conssepalp;
    c.conssepasol = conssepasol;
    c.sepafreq = sepafreq;
    c.sepapriority = sepapriority;
    c.delaysepa = delaysepa;
}

/// `SCIPconshdlrSetProp`: sets both the propagation callback and the
/// propagation frequency of the constraint handler.
pub fn conshdlr_set_prop(scip: &mut Scip, h: ConshdlrId, consprop: Option<ConsProp>, propfreq: i32, delayprop: bool, timingmask: PropTiming) {
    let c = &mut scip.conshdlrs[h];
    c.consprop = consprop;
    c.propfreq = propfreq;
    c.delayprop = delayprop;
    c.proptiming = timingmask;
}

/// `SCIPconshdlrSetPresol`: sets presolving method of constraint handler.
pub fn conshdlr_set_presol(scip: &mut Scip, h: ConshdlrId, conspresol: Option<ConsPresol>, maxprerounds: i32, presoltiming: PresolTiming) -> ScipResult {
    let c = &mut scip.conshdlrs[h];
    c.conspresol = conspresol;
    c.maxprerounds = maxprerounds;

    // The interface change from delay flags to timings cannot be recognized
    // at compile time: exit with an appropriate error message.
    if !(PRESOLTIMING_FAST..=PRESOLTIMING_MAX).contains(&presoltiming) {
        log::error!(
            "ERROR: 'PRESOLDELAY'-flag no longer available since SCIP 3.2, use an appropriate 'SCIP_PRESOLTIMING' for <{}> constraint handler instead.",
            c.name
        );
        return Err(crate::retcode::Retcode::ParameterWrongVal);
    }
    c.presoltiming = presoltiming;
    Ok(())
}

/*
 * Constraint set change methods: a set change is that of node `node`
 * (`scip.nodes[node].conssetchg`).
 */

/// The constraint set change of `node`, which must exist.
macro_rules! csc {
    ($scip:expr, $node:expr) => {
        (**$scip.nodes[$node].conssetchg.as_mut().unwrap())
    };
}

/// `conssetchgRelease`: releases all constraints of the constraint set
/// change data.
fn conssetchg_release(scip: &mut Scip, node: crate::types::NodeId) -> ScipResult {
    for i in 0..csc!(scip, node).naddedconss {
        let cons = csc!(scip, node).addedconss[i];
        if cons != crate::var::NONE {
            cons_release(scip, cons)?;
            csc!(scip, node).addedconss[i] = crate::var::NONE;
        }
    }
    for i in 0..csc!(scip, node).ndisabledconss {
        let cons = csc!(scip, node).disabledconss[i];
        if cons != crate::var::NONE {
            cons_release(scip, cons)?;
            csc!(scip, node).disabledconss[i] = crate::var::NONE;
        }
    }
    Ok(())
}

/// `SCIPconssetchgFree`: frees fixed size constraint set change data and
/// releases all included constraints.
pub fn conssetchg_free(scip: &mut Scip, node: crate::types::NodeId) -> ScipResult {
    if scip.nodes[node].conssetchg.is_some() {
        // Release constraints.
        conssetchg_release(scip, node)?;
        scip.nodes[node].conssetchg = None;
    }
    Ok(())
}

/// `conssetchgEnsureAddedconssSize`.
fn conssetchg_ensure_addedconss_size(scip: &mut Scip, node: crate::types::NodeId, num: i32) {
    if num > csc!(scip, node).addedconsssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let c = &mut csc!(scip, node);
        c.addedconss.realloc(newsize);
        c.addedconsssize = newsize;
    }
}

/// `conssetchgEnsureDisabledconssSize`.
fn conssetchg_ensure_disabledconss_size(scip: &mut Scip, node: crate::types::NodeId, num: i32) {
    if num > csc!(scip, node).disabledconsssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let c = &mut csc!(scip, node);
        c.disabledconss.realloc(newsize);
        c.disabledconsssize = newsize;
    }
}

/// `SCIPconssetchgAddAddedCons`: adds constraint addition to constraint set
/// changes, and captures constraint; activates constraint if the
/// constraint set change data is currently active.
pub fn conssetchg_add_added_cons(scip: &mut Scip, node: crate::types::NodeId, cons: ConsId, depth: i32, focusnode: bool, active: bool) -> ScipResult {
    // If constraint set change doesn't exist, create it.
    if scip.nodes[node].conssetchg.is_none() {
        scip.nodes[node].conssetchg = Some(Box::default());
    }

    // Add constraint to the addedconss array.
    let n = csc!(scip, node).naddedconss;
    conssetchg_ensure_addedconss_size(scip, node, n + 1);
    let c = &mut csc!(scip, node);
    c.addedconss[c.naddedconss] = cons;
    c.naddedconss += 1;

    // Undelete constraint, if it was globally deleted in the past.
    scip.conss[cons].deleted = false;

    // Capture constraint.
    cons_capture(scip, cons);

    // Activate constraint, if node is active.
    if active && !scip.conss[cons].active {
        cons_activate(scip, cons, depth, focusnode)?;

        // Remember, that this constraint set change data was responsible
        // for the constraint's addition.
        scip.conss[cons].addconssetchg = node;
        scip.conss[cons].addarraypos = csc!(scip, node).naddedconss - 1;
    }
    Ok(())
}

/// `SCIPconssetchgAddDisabledCons`: adds constraint disabling to constraint
/// set changes, and captures constraint.
pub fn conssetchg_add_disabled_cons(scip: &mut Scip, node: crate::types::NodeId, cons: ConsId) {
    // If constraint set change doesn't exist, create it.
    if scip.nodes[node].conssetchg.is_none() {
        scip.nodes[node].conssetchg = Some(Box::default());
    }

    // Add constraint to the disabledconss array.
    let n = csc!(scip, node).ndisabledconss;
    conssetchg_ensure_disabledconss_size(scip, node, n + 1);
    let c = &mut csc!(scip, node);
    c.disabledconss[c.ndisabledconss] = cons;
    c.ndisabledconss += 1;

    // Capture constraint.
    cons_capture(scip, cons);
}

/// `conssetchgDelAddedCons`: deactivates, deletes, and releases constraint
/// from the addedconss array of the constraint set change data.
fn conssetchg_del_added_cons(scip: &mut Scip, node: crate::types::NodeId, mut arraypos: i32) -> ScipResult {
    let cons = csc!(scip, node).addedconss[arraypos];

    // Remove the link to the constraint set change data.
    if scip.conss[cons].addconssetchg == node {
        scip.conss[cons].addconssetchg = crate::var::NONE;
        scip.conss[cons].addarraypos = -1;
    }

    // Release constraint.
    cons_release(scip, cons)?;
    csc!(scip, node).addedconss[arraypos] = crate::var::NONE;

    // We want to keep the order of the constraint additions: move all
    // subsequent constraints one slot to the front.
    while arraypos < csc!(scip, node).naddedconss - 1 {
        let next = csc!(scip, node).addedconss[arraypos + 1];
        csc!(scip, node).addedconss[arraypos] = next;
        if scip.conss[next].addconssetchg == node {
            scip.conss[next].addarraypos = arraypos;
        }
        arraypos += 1;
    }
    csc!(scip, node).naddedconss -= 1;
    Ok(())
}

/// `conssetchgDelDisabledCons`: deletes and releases deactivated
/// constraint from the disabledconss array of the constraint set change
/// data.
fn conssetchg_del_disabled_cons(scip: &mut Scip, node: crate::types::NodeId, mut arraypos: i32) -> ScipResult {
    // Release constraint.
    let cons = csc!(scip, node).disabledconss[arraypos];
    cons_release(scip, cons)?;
    csc!(scip, node).disabledconss[arraypos] = crate::var::NONE;

    // We want to keep the order of the constraint disablings: move all
    // subsequent constraints one slot to the front.
    let c = &mut csc!(scip, node);
    while arraypos < c.ndisabledconss - 1 {
        c.disabledconss[arraypos] = c.disabledconss[arraypos + 1];
        arraypos += 1;
    }
    c.ndisabledconss -= 1;
    Ok(())
}

/// `SCIPconssetchgApply`: applies constraint set change.
pub fn conssetchg_apply(scip: &mut Scip, node: crate::types::NodeId, depth: i32, focusnode: bool) -> ScipResult {
    if scip.nodes[node].conssetchg.is_none() {
        return Ok(());
    }

    // Apply constraint additions.
    let mut i = 0;
    while i < csc!(scip, node).naddedconss {
        let cons = csc!(scip, node).addedconss[i];
        if scip.conss[cons].active || scip.conss[cons].deleted {
            // Delete constraint from addedcons array, the empty slot is
            // now used by the next constraint, and naddedconss was
            // decreased, so do not increase i.
            conssetchg_del_added_cons(scip, node, i)?;
        } else {
            // Activate constraint.
            cons_activate(scip, cons, depth, focusnode)?;

            // Remember, that this constraint set change data was
            // responsible for the constraint's addition.
            scip.conss[cons].addconssetchg = node;
            scip.conss[cons].addarraypos = i;

            // Handle the next constraint.
            i += 1;
        }
    }

    // Apply constraint disablings.
    let mut i = 0;
    while i < csc!(scip, node).ndisabledconss {
        let cons = csc!(scip, node).disabledconss[i];
        if !scip.conss[cons].enabled {
            // Release and remove constraint from the disabledconss array,
            // the empty slot is now used by the next constraint and
            // ndisabledconss was decreased, so do not increase i.
            conssetchg_del_disabled_cons(scip, node, i)?;
        } else {
            cons_disable(scip, cons)?;

            // Handle the next constraint.
            i += 1;
        }
    }
    Ok(())
}

/// `SCIPconssetchgUndo`: undoes constraint set change.
pub fn conssetchg_undo(scip: &mut Scip, node: crate::types::NodeId) -> ScipResult {
    if scip.nodes[node].conssetchg.is_none() {
        return Ok(());
    }

    // Undo constraint disablings.
    let mut i = csc!(scip, node).ndisabledconss - 1;
    while i >= 0 {
        let cons = csc!(scip, node).disabledconss[i];

        // If the constraint is inactive, we can permanently remove it from
        // the disabledconss array: the deactivation was more global than
        // the current node. If the constraint is already enabled, we need
        // not to do anything.
        if !scip.conss[cons].active {
            conssetchg_del_disabled_cons(scip, node, i)?;
        } else if !scip.conss[cons].enabled {
            cons_enable(scip, cons)?;
        }
        i -= 1;
    }

    // Undo constraint additions.
    let mut i = csc!(scip, node).naddedconss - 1;
    while i >= 0 {
        let cons = csc!(scip, node).addedconss[i];

        // If the constraint is already deactivated, we need not to do
        // anything (see the C for the path where this happens).
        if scip.conss[cons].active {
            cons_deactivate(scip, cons)?;
            scip.conss[cons].addconssetchg = crate::var::NONE;
            scip.conss[cons].addarraypos = -1;
        }
        i -= 1;
    }
    Ok(())
}

/// `SCIPconssetchgMakeGlobal`: applies constraint set change to the global
/// problem and deletes the constraint set change data.
pub fn conssetchg_make_global(scip: &mut Scip, node: crate::types::NodeId, prob: usize) -> ScipResult {
    if scip.nodes[node].conssetchg.is_none() {
        return Ok(());
    }

    // Apply constraint additions to the global problem (loop backwards,
    // because then conssetchgDelAddedCons() is more efficient).
    let mut i = csc!(scip, node).naddedconss - 1;
    while i >= 0 {
        let cons = csc!(scip, node).addedconss[i];

        // Only unlock and remove constraint if it is not sticking at the
        // node.
        if !scip.conss[cons].stickingatnode {
            // Because we first have to delete the constraint, we have to
            // capture it in order to not loose it.
            cons_capture(scip, cons);

            // Delete constraint addition from constraint set change data.
            conssetchg_del_added_cons(scip, node, i)?;

            // Don't add deleted (i.e. globally obsolete) constraints to
            // the global problem.
            if !scip.conss[cons].deleted {
                crate::prob::prob_add_cons(scip, prob, cons)?;
            }

            // Release constraint.
            cons_release(scip, cons)?;
        }
        i -= 1;
    }

    // Apply constraint disablings to the global problem (loop backwards,
    // because then conssetchgDelDisabledCons() is more efficient).
    let mut i = csc!(scip, node).ndisabledconss - 1;
    while i >= 0 {
        let cons = csc!(scip, node).disabledconss[i];

        // Only delete constraint if it is not sticking at the node.
        if !scip.conss[cons].stickingatnode {
            // Globally delete constraint.
            if !scip.conss[cons].deleted {
                cons_delete(scip, cons, prob)?;
            }

            // Release and remove constraint from the disabledconss array.
            conssetchg_del_disabled_cons(scip, node, i)?;
        }
        i -= 1;
    }
    let c = &csc!(scip, node);
    if c.naddedconss == 0 && c.ndisabledconss == 0 {
        // Free empty constraint set change data.
        conssetchg_free(scip, node)?;
    }
    Ok(())
}

/*
 * Constraint methods
 */

/// `SCIPconsCreate`: creates and captures a constraint, and inserts it
/// into the conss array of its constraint handler.
#[allow(clippy::too_many_arguments)]
pub fn cons_create(
    scip: &mut Scip,
    name: &str,
    conshdlr: ConshdlrId,
    consdata: Option<Box<dyn Any>>,
    flags: &ConsFlags,
    original: bool,
    deleteconsdata: bool,
) -> ScipResult<ConsId> {
    let c = Cons {
        name: name.to_string(),
        conshdlr,
        consdata,
        transorigcons: crate::var::NONE,
        addconssetchg: crate::var::NONE,
        addarraypos: -1,
        consspos: -1,
        initconsspos: -1,
        sepaconsspos: -1,
        enfoconsspos: -1,
        checkconsspos: -1,
        propconsspos: -1,
        confconsspos: -1,
        activedepth: -2,
        validdepth: if flags.local { -1 } else { 0 },
        age: 0.0,
        nuses: 0,
        nupgradelocks: 0,
        initial: flags.initial,
        separate: flags.separate,
        enforce: flags.enforce,
        check: flags.check,
        propagate: flags.propagate,
        sepaenabled: flags.separate,
        propenabled: flags.propagate,
        local: flags.local,
        modifiable: flags.modifiable,
        dynamic: flags.dynamic,
        removable: flags.removable,
        stickingatnode: flags.stickingatnode,
        original,
        deleteconsdata,
        active: false,
        conflict: false,
        enabled: false,
        obsolete: false,
        markpropagate: true,
        deleted: false,
        update: false,
        updateinsert: false,
        updateactivate: false,
        updatedeactivate: false,
        updateenable: false,
        updatedisable: false,
        updatesepaenable: false,
        updatesepadisable: false,
        updatepropenable: false,
        updatepropdisable: false,
        updateobsolete: false,
        updatemarkpropagate: false,
        updateunmarkpropagate: false,
        updatefree: false,
        updateactfocus: false,
        nlockspos: [0; NLOCKTYPES],
        nlocksneg: [0; NLOCKTYPES],
    };
    let cons = scip.conss.push(c);

    // Capture constraint.
    cons_capture(scip, cons);

    // Insert the constraint as inactive constraint into the transformed
    // constraints array.
    if !original {
        // Check, if inserting constraint should be delayed.
        if conshdlr_are_updates_delayed(&scip.conshdlrs[conshdlr]) {
            scip.conss[cons].updateinsert = true;
            conshdlr_add_update_cons(scip, conshdlr, cons);
        } else {
            conshdlr_add_cons(scip, conshdlr, cons);
        }
    }
    Ok(cons)
}

/// `SCIPconsChgName`: change name of given constraint.
pub fn cons_chg_name(scip: &mut Scip, cons: ConsId, name: &str) {
    scip.conss[cons].name = name.to_string();
}

/// `SCIPconsFree`: frees a constraint and removes it from the conss array
/// of its constraint handler.
fn cons_free(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let h = scip.conss[cons].conshdlr;

    // Free constraint data.
    if let Some(consdelete) = scip.conshdlrs[h].consdelete
        && scip.conss[cons].consdata.is_some()
        && scip.conss[cons].deleteconsdata
    {
        let mut consdata = scip.conss[cons].consdata.take();
        consdelete(scip, h, cons, &mut consdata)?;
        scip.conss[cons].consdata = consdata;
    } else if !scip.conss[cons].deleteconsdata {
        scip.conss[cons].consdata = None;
    }

    // Unlink transformed and original constraint.
    let transorigcons = scip.conss[cons].transorigcons;
    if transorigcons != crate::var::NONE {
        scip.conss[transorigcons].transorigcons = crate::var::NONE;
    }

    // Remove constraint from the transformed constraints array.
    if !scip.conss[cons].original {
        conshdlr_del_cons(scip, h, cons);
    }
    scip.conss.remove(cons);
    Ok(())
}

/// `SCIPconsCapture`: increases usage counter of constraint.
pub fn cons_capture(scip: &mut Scip, cons: ConsId) {
    scip.conss[cons].nuses += 1;
}

/// `SCIPconsRelease`: decreases usage counter of constraint, and frees
/// memory if necessary.
pub fn cons_release(scip: &mut Scip, cons: ConsId) -> ScipResult {
    scip.conss[cons].nuses -= 1;
    if scip.conss[cons].nuses == 0 {
        let h = scip.conss[cons].conshdlr;

        // Check, if freeing constraint should be delayed.
        if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
            scip.conss[cons].updatefree = true;
            conshdlr_add_update_cons(scip, h, cons);
        } else {
            cons_free(scip, cons)?;
        }
    }
    Ok(())
}

/// `SCIPconsGetVars`: the variables of the constraint, if its handler can
/// give them (`success`).
pub fn cons_get_vars(scip: &mut Scip, cons: ConsId) -> ScipResult<(Vec<VarId>, bool)> {
    let h = scip.conss[cons].conshdlr;
    let mut vars = Vec::new();
    let mut success = false;
    if let Some(consgetvars) = scip.conshdlrs[h].consgetvars {
        consgetvars(scip, h, cons, &mut vars, &mut success)?;
    }
    Ok((vars, success))
}

/// `SCIPconsGetNVars`: the number of variables of the constraint, if its
/// handler can give it (`success`).
pub fn cons_get_n_vars(scip: &mut Scip, cons: ConsId) -> ScipResult<(i32, bool)> {
    let h = scip.conss[cons].conshdlr;
    let mut nvars = 0;
    let mut success = false;
    if let Some(consgetnvars) = scip.conshdlrs[h].consgetnvars {
        consgetnvars(scip, h, cons, &mut nvars, &mut success)?;
    }
    Ok((nvars, success))
}

/// `SCIPconsDelete`: globally removes constraint from all subproblems;
/// removes constraint from the constraint set change data of the node,
/// where it was added, or from the problem, if it was a problem constraint
/// (reoptimization is off).
pub fn cons_delete(scip: &mut Scip, cons: ConsId, prob: usize) -> ScipResult {
    // Mark constraint deleted.
    scip.conss[cons].deleted = true;

    // Deactivate constraint, if it is currently active.
    if scip.conss[cons].active && !scip.conss[cons].updatedeactivate {
        cons_deactivate(scip, cons)?;
    } else {
        scip.conss[cons].updateactivate = false;
    }

    // Remove formerly active constraint from the conssetchg's addedconss /
    // prob's conss array.
    if scip.conss[cons].addarraypos >= 0 {
        let addconssetchg = scip.conss[cons].addconssetchg;
        if addconssetchg == crate::var::NONE {
            // Remove problem constraint from the problem.
            crate::prob::prob_del_cons(scip, prob, cons)?;
        } else {
            // Remove constraint from the constraint set change addedconss
            // array.
            let pos = scip.conss[cons].addarraypos;
            conssetchg_del_added_cons(scip, addconssetchg, pos)?;
        }
    }
    Ok(())
}

/// `SCIPconsTransform`: gets and captures transformed constraint of a
/// given original constraint; if the constraint is not yet transformed, a
/// new transformed constraint for this constraint is created.
pub fn cons_transform(scip: &mut Scip, origcons: ConsId) -> ScipResult<ConsId> {
    // Check, if the constraint is already transformed.
    let transorigcons = scip.conss[origcons].transorigcons;
    if transorigcons != crate::var::NONE {
        cons_capture(scip, transorigcons);
        return Ok(transorigcons);
    }

    // Create transformed constraint.
    let h = scip.conss[origcons].conshdlr;
    let Some(constrans) = scip.conshdlrs[h].constrans else {
        // Without a transform method the transformed constraint shares the
        // original's data, which a `Box` cannot.
        todo!("SCIPconsTransform without a CONSTRANS method: constraint handler <{}>", scip.conshdlrs[h].name);
    };
    let mut transcons = crate::var::NONE;
    constrans(scip, h, origcons, &mut transcons)?;

    // Link original and transformed constraint.
    scip.conss[origcons].transorigcons = transcons;
    scip.conss[transcons].transorigcons = origcons;

    // Copy the number of upgradelocks.
    scip.conss[transcons].nupgradelocks = scip.conss[origcons].nupgradelocks;
    Ok(transcons)
}

/// `SCIPconsSetInitial`: sets the initial flag of the given constraint.
pub fn cons_set_initial(scip: &mut Scip, cons: ConsId, initial: bool) {
    if scip.conss[cons].initial != initial {
        scip.conss[cons].initial = initial;
        if !scip.conss[cons].original {
            let h = scip.conss[cons].conshdlr;
            if initial {
                conshdlr_add_initcons(scip, h, cons);
            } else if scip.conss[cons].initconsspos >= 0 {
                conshdlr_del_initcons(scip, h, cons);
            }
        }
    }
}

/// `SCIPconsSetSeparated`: sets the separate flag of the given constraint.
pub fn cons_set_separated(scip: &mut Scip, cons: ConsId, separate: bool) {
    let c = &scip.conss[cons];
    if c.separate != separate {
        if scip.set.stage == crate::scip::Stage::Problem {
            scip.conss[cons].separate = separate;
        } else if c.enabled && c.sepaenabled {
            let h = c.conshdlr;
            if separate {
                scip.conss[cons].separate = separate;
                conshdlr_add_sepacons(scip, h, cons);
            } else {
                conshdlr_del_sepacons(scip, h, cons);
                scip.conss[cons].separate = separate;
            }
        }
    }
}

/// `SCIPconsSetEnforced`: sets the enforce flag of the given constraint.
pub fn cons_set_enforced(scip: &mut Scip, cons: ConsId, enforce: bool) {
    let c = &scip.conss[cons];
    if c.enforce != enforce {
        if scip.set.stage == crate::scip::Stage::Problem {
            scip.conss[cons].enforce = enforce;
        } else if c.enabled {
            let h = c.conshdlr;
            if enforce {
                scip.conss[cons].enforce = enforce;
                conshdlr_add_enfocons(scip, h, cons);
            } else {
                conshdlr_del_enfocons(scip, h, cons);
                scip.conss[cons].enforce = enforce;
            }
        }
    }
}

/// `SCIPconsSetChecked`: sets the check flag of the given constraint.
pub fn cons_set_checked(scip: &mut Scip, cons: ConsId, check: bool) -> ScipResult {
    if scip.conss[cons].check != check {
        scip.conss[cons].check = check;
        if !scip.conss[cons].original {
            // If constraint is a problem constraint, update variable
            // roundings locks.
            if scip.conss[cons].addconssetchg == crate::var::NONE && scip.conss[cons].addarraypos >= 0 {
                if check {
                    cons_add_locks(scip, cons, LockType::Model, 1, 0)?;
                } else {
                    cons_add_locks(scip, cons, LockType::Model, -1, 0)?;
                }
            }

            // If constraint is active, update the checkconss array of the
            // constraint handler.
            if scip.conss[cons].active {
                let h = scip.conss[cons].conshdlr;
                if check {
                    conshdlr_add_checkcons(scip, h, cons);
                } else {
                    conshdlr_del_checkcons(scip, h, cons);
                }
            }
        }
    }
    Ok(())
}

/// `SCIPconsSetPropagated`: sets the propagate flag of the given
/// constraint.
pub fn cons_set_propagated(scip: &mut Scip, cons: ConsId, propagate: bool) {
    let c = &scip.conss[cons];
    if c.propagate != propagate {
        if scip.set.stage == crate::scip::Stage::Problem {
            scip.conss[cons].propagate = propagate;
        } else if c.enabled && c.propenabled {
            let h = c.conshdlr;
            if propagate {
                scip.conss[cons].propagate = propagate;
                conshdlr_add_propcons(scip, h, cons);
            } else {
                conshdlr_del_propcons(scip, h, cons);
                scip.conss[cons].propagate = propagate;
            }
        }
    }
}

/// `SCIPconsSetLocal`: sets the local flag of the given constraint.
pub fn cons_set_local(scip: &mut Scip, cons: ConsId, local: bool) {
    scip.conss[cons].local = local;
    if !local {
        scip.conss[cons].validdepth = 0;
    }
}

/// `SCIPconsActivate`: activates constraint or marks constraint to be
/// activated in next update; `depth` is -1 for the global problem.
pub fn cons_activate(scip: &mut Scip, cons: ConsId, depth: i32, focusnode: bool) -> ScipResult {
    let h = scip.conss[cons].conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        let c = &mut scip.conss[cons];
        c.updateactivate = true;
        c.activedepth = depth;
        c.updateactfocus = focusnode;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_activate_cons(scip, h, cons, depth, focusnode)?;
    }
    Ok(())
}

/// `SCIPconsDeactivate`: deactivates constraint or marks constraint to be
/// deactivated in next update.
pub fn cons_deactivate(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let h = scip.conss[cons].conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updatedeactivate = true;
        scip.conss[cons].activedepth = -2;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_deactivate_cons(scip, h, cons)?;
    }
    Ok(())
}

/// `SCIPconsEnable`: enables constraint's separation, enforcing, and
/// propagation capabilities or marks them to be enabled in next update.
pub fn cons_enable(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let c = &scip.conss[cons];
    if !c.active || c.updatedeactivate || c.updateenable || (c.enabled && !c.updatedisable) {
        return Ok(());
    }
    let h = c.conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updateenable = true;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_enable_cons(scip, h, cons)?;
    }
    Ok(())
}

/// `SCIPconsDisable`: disables constraint's separation, enforcing, and
/// propagation capabilities or marks them to be disabled in next update.
pub fn cons_disable(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let c = &scip.conss[cons];
    if c.updatedisable || (!c.enabled && !c.updateenable) {
        return Ok(());
    }
    let h = c.conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updatedisable = true;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_disable_cons(scip, h, cons)?;
    }
    Ok(())
}

/// `SCIPconsEnableSeparation`: enables constraint's separation
/// capabilities or marks them to be enabled in next update.
pub fn cons_enable_separation(scip: &mut Scip, cons: ConsId) {
    let c = &scip.conss[cons];
    if c.updatesepaenable || (c.sepaenabled && !c.updatesepadisable) {
        return;
    }
    let h = c.conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updatesepadisable = false;
        scip.conss[cons].updatesepaenable = true;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_enable_cons_separation(scip, h, cons);
    }
}

/// `SCIPconsDisableSeparation`: disables constraint's separation
/// capabilities or marks them to be disabled in next update.
pub fn cons_disable_separation(scip: &mut Scip, cons: ConsId) {
    let c = &scip.conss[cons];
    if c.updatesepadisable || (!c.sepaenabled && !c.updatesepaenable) {
        return;
    }
    let h = c.conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updatesepaenable = false;
        scip.conss[cons].updatesepadisable = true;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_disable_cons_separation(scip, h, cons);
    }
}

/// `SCIPconsEnablePropagation`: enables constraint's propagation
/// capabilities or marks them to be enabled in next update.
pub fn cons_enable_propagation(scip: &mut Scip, cons: ConsId) {
    let c = &scip.conss[cons];
    if c.updatepropenable || (c.propenabled && !c.updatepropdisable) {
        return;
    }
    let h = c.conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updatepropdisable = false;
        scip.conss[cons].updatepropenable = true;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_enable_cons_propagation(scip, h, cons);
    }
}

/// `SCIPconsDisablePropagation`: disables constraint's propagation
/// capabilities or marks them to be disabled in next update.
pub fn cons_disable_propagation(scip: &mut Scip, cons: ConsId) {
    let c = &scip.conss[cons];
    if c.updatepropdisable || (!c.propenabled && !c.updatepropenable) {
        return;
    }
    let h = c.conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updatepropenable = false;
        scip.conss[cons].updatepropdisable = true;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_disable_cons_propagation(scip, h, cons);
    }
}

/// `SCIPconsMarkConflict`: marks the constraint to be a conflict.
pub fn cons_mark_conflict(scip: &mut Scip, cons: ConsId) {
    scip.conss[cons].conflict = true;
}

/// `SCIPconsMarkPropagate`: marks the constraint to be propagated (update
/// might be delayed).
pub fn cons_mark_propagate(scip: &mut Scip, cons: ConsId) {
    let c = &scip.conss[cons];
    if c.updatemarkpropagate || (c.markpropagate && !c.updateunmarkpropagate) {
        return;
    }
    let h = c.conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updateunmarkpropagate = false;
        scip.conss[cons].updatemarkpropagate = true;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_mark_cons_propagate(scip, h, cons);
    }
}

/// `SCIPconsUnmarkPropagate`: unmarks the constraint to be propagated
/// (update might be delayed).
pub fn cons_unmark_propagate(scip: &mut Scip, cons: ConsId) {
    let c = &scip.conss[cons];
    if c.updateunmarkpropagate || (!c.markpropagate && !c.updatemarkpropagate) {
        return;
    }
    let h = c.conshdlr;
    if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
        scip.conss[cons].updatemarkpropagate = false;
        scip.conss[cons].updateunmarkpropagate = true;
        conshdlr_add_update_cons(scip, h, cons);
    } else {
        conshdlr_unmark_cons_propagate(scip, h, cons);
    }
}

/// `SCIPconsAddAge`: adds given value to age of constraint, but age can
/// never become negative; should be called with `deltaage` > 0, if
/// constraint is completely useless; if the age exceeds the obsolete or
/// age limits, the constraint is marked obsolete or deleted.
pub fn cons_add_age(scip: &mut Scip, cons: ConsId, prob: usize, deltaage: f64) -> ScipResult {
    // Possible Ages:
    //  0 <= age < infinity
    // In presolving, the age is not updated.
    if scip.set.stage == crate::scip::Stage::Presolving {
        return Ok(());
    }
    let c = &mut scip.conss[cons];
    c.age += deltaage;
    c.age = crate::def::max(c.age, 0.0);
    if !c.original {
        if !c.check && cons_exceeds_agelimit(scip, cons) {
            cons_delete(scip, cons, prob)?;
        } else if !scip.conss[cons].obsolete && cons_exceeds_obsoleteage(scip, cons) {
            let h = scip.conss[cons].conshdlr;
            if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
                scip.conss[cons].updateobsolete = true;
                conshdlr_add_update_cons(scip, h, cons);
            } else {
                conshdlr_mark_cons_obsolete(scip, h, cons);
            }
        }
    }
    Ok(())
}

/// `SCIPconsIncAge`: increases age of constraint by 1.0.
pub fn cons_inc_age(scip: &mut Scip, cons: ConsId, prob: usize) -> ScipResult {
    cons_add_age(scip, cons, prob, 1.0)
}

/// `SCIPconsResetAge`: resets age of constraint to zero; should be called,
/// if the constraint was useful.
pub fn cons_reset_age(scip: &mut Scip, cons: ConsId) {
    let h = scip.conss[cons].conshdlr;
    let age = scip.conss[cons].age;
    conshdlr_update_ageresetavg(&mut scip.conshdlrs[h], age);
    scip.conss[cons].age = 0.0;
    if scip.conss[cons].obsolete {
        if conshdlr_are_updates_delayed(&scip.conshdlrs[h]) {
            scip.conss[cons].updateobsolete = true;
            conshdlr_add_update_cons(scip, h, cons);
        } else {
            conshdlr_mark_cons_useful(scip, h, cons);
        }
    }
}

/// `SCIPconsResolvePropagation`: resolves the given conflicting bound,
/// that was deduced by the given constraint, by putting all "reason"
/// bounds leading to the deduction into the conflict queue with calls to
/// SCIPaddConflictLb(), SCIPaddConflictUb(), SCIPaddConflictBd(),
/// SCIPaddConflictRelaxedLb(), SCIPaddConflictRelaxedUb(),
/// SCIPaddConflictRelaxedBd(), or SCIPaddConflictBinvar().
#[allow(clippy::too_many_arguments)]
pub fn cons_resolve_propagation(
    scip: &mut Scip,
    cons: ConsId,
    infervar: VarId,
    inferinfo: i32,
    inferboundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    relaxedbd: f64,
) -> ScipResult<ScipResultCode> {
    let mut result = ScipResultCode::DidNotRun;
    let h = scip.conss[cons].conshdlr;
    let Some(consresprop) = scip.conshdlrs[h].consresprop else {
        log::error!(
            "propagation conflict resolving method of constraint handler <{}> is not implemented",
            scip.conshdlrs[h].name
        );
        return Err(crate::retcode::Retcode::PluginNotFound);
    };
    let time_enabled = scip.set.time_enabled;
    scip.conshdlrs[h].resproptime.start(time_enabled);
    consresprop(scip, h, cons, infervar, inferinfo, inferboundtype, bdchgidx, relaxedbd, &mut result)?;
    scip.conshdlrs[h].resproptime.stop(time_enabled);

    // Update statistics.
    scip.conshdlrs[h].nrespropcalls += 1;

    // Check result code.
    if result != ScipResultCode::Success && result != ScipResultCode::DidNotFind {
        log::error!(
            "propagation conflict resolving method of constraint handler <{}> returned invalid result <{}>",
            scip.conshdlrs[h].name,
            result as i32
        );
        return Err(crate::retcode::Retcode::InvalidResult);
    }
    Ok(result)
}

/// `SCIPconsAddLocks`: adds given values to lock status of the constraint
/// and updates the rounding locks of the involved variables.
pub fn cons_add_locks(scip: &mut Scip, cons: ConsId, locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    let lt = locktype as usize;
    let c = &mut scip.conss[cons];

    // Update the rounding locks.
    let oldnlockspos = c.nlockspos[lt];
    let oldnlocksneg = c.nlocksneg[lt];
    c.nlockspos[lt] += nlockspos;
    c.nlocksneg[lt] += nlocksneg;

    // Check, if the constraint switched from unlocked to locked, or from
    // locked to unlocked.
    let updlockpos = (c.nlockspos[lt] > 0) as i32 - (oldnlockspos > 0) as i32;
    let updlockneg = (c.nlocksneg[lt] > 0) as i32 - (oldnlocksneg > 0) as i32;

    // Lock the variables, if the constraint switched from unlocked to
    // locked or from locked to unlocked.
    if updlockpos != 0 || updlockneg != 0 {
        let h = c.conshdlr;
        let conslock = scip.conshdlrs[h].conslock.expect("a lock method");
        conslock(scip, h, cons, locktype, updlockpos, updlockneg)?;
    }
    Ok(())
}

/// `SCIPconsCheck`: checks single constraint for feasibility of the given
/// solution.
pub fn cons_check(scip: &mut Scip, cons: ConsId, sol: SolId, checkintegrality: bool, checklprows: bool, printreason: bool) -> ScipResult<ScipResultCode> {
    let h = scip.conss[cons].conshdlr;
    let mut result = ScipResultCode::DidNotRun;
    let conscheck = scip.conshdlrs[h].conscheck.expect("a check method");
    conscheck(scip, h, &[cons], sol, checkintegrality, checklprows, printreason, false, &mut result)?;
    if result != ScipResultCode::Infeasible && result != ScipResultCode::Feasible {
        log::error!(
            "feasibility check of constraint handler <{}> on constraint <{}> returned invalid result <{}>",
            scip.conshdlrs[h].name,
            scip.conss[cons].name,
            result as i32
        );
        return Err(crate::retcode::Retcode::InvalidResult);
    }
    Ok(result)
}

/// `SCIPconsEnfops`: enforces single constraint for a given pseudo
/// solution.
pub fn cons_enfops(scip: &mut Scip, cons: ConsId, solinfeasible: bool, objinfeasible: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let h = scip.conss[cons].conshdlr;
    let mut result = R::DidNotRun;
    let consenfops = scip.conshdlrs[h].consenfops.expect("a pseudo enforcement method");
    consenfops(scip, h, &[cons], 1, solinfeasible, objinfeasible, &mut result)?;
    if !matches!(
        result,
        R::Cutoff | R::ConsAdded | R::ReducedDom | R::Branched | R::SolveLp | R::Infeasible | R::Feasible | R::DidNotRun
    ) {
        log::error!(
            "enforcing method of constraint handler <{}> for pseudo solutions returned invalid result <{}>",
            scip.conshdlrs[h].name,
            result as i32
        );
        return Err(crate::retcode::Retcode::InvalidResult);
    }
    Ok(result)
}

/// `SCIPconsEnfolp`: enforces single constraint for a given LP solution.
pub fn cons_enfolp(scip: &mut Scip, cons: ConsId, solinfeasible: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let h = scip.conss[cons].conshdlr;
    let mut result = R::DidNotRun;
    let consenfolp = scip.conshdlrs[h].consenfolp.expect("an LP enforcement method");
    consenfolp(scip, h, &[cons], 1, solinfeasible, &mut result)?;
    if !matches!(result, R::Cutoff | R::ConsAdded | R::ReducedDom | R::Branched | R::Separated | R::Infeasible | R::Feasible) {
        log::error!(
            "enforcing method of constraint handler <{}> for LP returned invalid result <{}>",
            scip.conshdlrs[h].name,
            result as i32
        );
        return Err(crate::retcode::Retcode::InvalidResult);
    }
    Ok(result)
}

/// `SCIPconsEnforelax`: enforces single constraint for a given relaxation
/// solution.
pub fn cons_enforelax(scip: &mut Scip, cons: ConsId, sol: SolId, solinfeasible: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let h = scip.conss[cons].conshdlr;
    let mut result = R::DidNotRun;
    let consenforelax = scip.conshdlrs[h].consenforelax.expect("a relaxation enforcement method");
    consenforelax(scip, sol, h, &[cons], 1, solinfeasible, &mut result)?;
    if !matches!(result, R::Cutoff | R::ConsAdded | R::ReducedDom | R::Branched | R::Separated | R::Infeasible | R::Feasible) {
        log::error!(
            "enforcing method of constraint handler <{}> for relaxation returned invalid result <{}>",
            scip.conshdlrs[h].name,
            result as i32
        );
        return Err(crate::retcode::Retcode::InvalidResult);
    }
    Ok(result)
}

/// `SCIPconsInitlp`: calls LP initialization method for single constraint;
/// returns whether infeasibility was detected while building the LP.
pub fn cons_initlp(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    let h = scip.conss[cons].conshdlr;
    let mut infeasible = false;
    if let Some(consinitlp) = scip.conshdlrs[h].consinitlp {
        consinitlp(scip, h, &[cons], &mut infeasible)?;
    }
    Ok(infeasible)
}

/// `SCIPconsSepalp`: calls separation method of single constraint for LP
/// solution.
pub fn cons_sepalp(scip: &mut Scip, cons: ConsId) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let h = scip.conss[cons].conshdlr;
    let mut result = R::DidNotRun;
    if let Some(conssepalp) = scip.conshdlrs[h].conssepalp {
        conssepalp(scip, h, &[cons], 1, &mut result)?;
        if !matches!(
            result,
            R::Cutoff | R::ConsAdded | R::ReducedDom | R::Separated | R::NewRound | R::DidNotFind | R::DidNotRun | R::Delayed
        ) {
            log::error!(
                "separation method of constraint handler <{}> returned invalid result <{}>",
                scip.conshdlrs[h].name,
                result as i32
            );
            return Err(crate::retcode::Retcode::InvalidResult);
        }
    }
    Ok(result)
}

/// `SCIPconsSepasol`: calls separation method of single constraint for
/// given primal solution.
pub fn cons_sepasol(scip: &mut Scip, cons: ConsId, sol: SolId) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let h = scip.conss[cons].conshdlr;
    let mut result = R::DidNotRun;
    if let Some(conssepasol) = scip.conshdlrs[h].conssepasol {
        conssepasol(scip, h, &[cons], 1, sol, &mut result)?;
        if !matches!(
            result,
            R::Cutoff | R::ConsAdded | R::ReducedDom | R::Separated | R::NewRound | R::DidNotFind | R::DidNotRun | R::Delayed
        ) {
            log::error!(
                "separation method of constraint handler for arbitrary primal solution <{}> returned invalid result <{}>",
                scip.conshdlrs[h].name,
                result as i32
            );
            return Err(crate::retcode::Retcode::InvalidResult);
        }
    }
    Ok(result)
}

/// `SCIPconsProp`: calls domain propagation method of single constraint.
pub fn cons_prop(scip: &mut Scip, cons: ConsId, proptiming: PropTiming) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let h = scip.conss[cons].conshdlr;
    let mut result = R::DidNotRun;
    if let Some(consprop) = scip.conshdlrs[h].consprop {
        consprop(scip, h, &[cons], 1, 1, proptiming, &mut result)?;
        if !matches!(result, R::Cutoff | R::ConsAdded | R::ReducedDom | R::DidNotFind | R::DidNotRun | R::Delayed) {
            log::error!(
                "propagation method of constraint handler <{}> returned invalid result <{}>",
                scip.conshdlrs[h].name,
                result as i32
            );
            return Err(crate::retcode::Retcode::InvalidResult);
        }
    }
    Ok(result)
}

/// `SCIPconsResprop`: resolves propagation conflict of single constraint.
#[allow(clippy::too_many_arguments)]
pub fn cons_resprop(
    scip: &mut Scip,
    cons: ConsId,
    infervar: VarId,
    inferinfo: i32,
    boundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    relaxedbd: f64,
) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let h = scip.conss[cons].conshdlr;
    let mut result = R::DidNotRun;
    if let Some(consresprop) = scip.conshdlrs[h].consresprop {
        consresprop(scip, h, cons, infervar, inferinfo, boundtype, bdchgidx, relaxedbd, &mut result)?;
        if result != R::Success && result != R::DidNotFind {
            log::error!(
                "propagation conflict resolving method of constraint handler <{}> returned invalid result <{}>",
                scip.conshdlrs[h].name,
                result as i32
            );
            return Err(crate::retcode::Retcode::InvalidResult);
        }
    }
    Ok(result)
}

/// `SCIPconsPresol`: presolves single constraint; `newcounts` are the
/// changes since the last call, `counts` the running totals.
pub fn cons_presol(scip: &mut Scip, cons: ConsId, nrounds: i32, timing: PresolTiming, newcounts: &PresolCounts, counts: &mut PresolCounts) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let h = scip.conss[cons].conshdlr;
    let mut result = R::DidNotRun;
    if let Some(conspresol) = scip.conshdlrs[h].conspresol {
        conspresol(scip, h, &[cons], nrounds, timing, newcounts, counts, &mut result)?;
        if !matches!(result, R::Unbounded | R::Cutoff | R::Success | R::DidNotFind | R::DidNotRun | R::Delayed) {
            log::error!(
                "presolving method of constraint handler <{}> returned invalid result <{}>",
                scip.conshdlrs[h].name,
                result as i32
            );
            return Err(crate::retcode::Retcode::InvalidResult);
        }
    }
    Ok(result)
}

/// `SCIPconsActive`: calls constraint activation notification method of
/// single constraint.
pub fn cons_active(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let h = scip.conss[cons].conshdlr;
    if let Some(consactive) = scip.conshdlrs[h].consactive {
        consactive(scip, h, cons)?;
    }
    Ok(())
}

/// `SCIPconsDeactive`: calls constraint deactivation notification method
/// of single constraint.
pub fn cons_deactive(scip: &mut Scip, cons: ConsId) -> ScipResult {
    let h = scip.conss[cons].conshdlr;
    if let Some(consdeactive) = scip.conshdlrs[h].consdeactive {
        consdeactive(scip, h, cons)?;
    }
    Ok(())
}

/*
 * Hash functions
 */

/// `SCIPhashGetKeyCons`: gets the key (i.e. the name) of the given
/// constraint.
pub fn hash_get_key_cons(scip: &Scip, cons: ConsId) -> &str {
    &scip.conss[cons].name
}

/*
 * method for arrays of contraint handlers
 */

/// `ensurePropagationStorage`: ensures size of storage for propagable
/// constraints with a minimum size of `num`.
fn ensure_propagation_storage(scip: &mut Scip, h: ConshdlrId, num: i32) {
    if num > scip.conshdlrs[h].storedpropconsssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let c = &mut scip.conshdlrs[h];
        c.storedpropconss.realloc(newsize);
        c.storedpropconsssize = newsize;
    }
}

/// `SCIPconshdlrsStorePropagationStatus`: stores all constraints marked
/// for propagation away when probing is started.
pub fn conshdlrs_store_propagation_status(scip: &mut Scip, conshdlrs: &[ConshdlrId]) {
    for &h in conshdlrs.iter().rev() {
        let nmarked = scip.conshdlrs[h].nmarkedpropconss;
        if nmarked > 0 {
            ensure_propagation_storage(scip, h, nmarked);
            let c = &mut scip.conshdlrs[h];
            for v in 0..nmarked as usize {
                c.storedpropconss[v] = c.propconss[v];
            }
            c.storednmarkedpropconss = c.nmarkedpropconss;
            c.storedpropdomchgcount = c.lastpropdomchgcount;

            for v in (0..nmarked as usize).rev() {
                let cons = scip.conshdlrs[h].storedpropconss[v];
                cons_capture(scip, cons);
            }
        }
    }
}

/// `SCIPconshdlrsResetPropagationStatus`: reset all constraints marked for
/// propagation when probing was finished.
pub fn conshdlrs_reset_propagation_status(scip: &mut Scip, conshdlrs: &[ConshdlrId]) -> ScipResult {
    for &h in conshdlrs.iter().rev() {
        if scip.conshdlrs[h].storednmarkedpropconss > 0 {
            // Mark all previously marked constraint, which were marked
            // before probing.
            let mut v = scip.conshdlrs[h].nmarkedpropconss - 1;
            while v >= 0 {
                let cons = scip.conshdlrs[h].propconss[v as usize];
                cons_unmark_propagate(scip, cons);
                v -= 1;
            }

            for v in 0..scip.conshdlrs[h].storednmarkedpropconss as usize {
                let cons = scip.conshdlrs[h].storedpropconss[v];
                let c = &scip.conss[cons];
                if c.enabled && c.propagate && c.propenabled {
                    cons_mark_propagate(scip, cons);
                }
                cons_release(scip, cons)?;
            }
            let c = &mut scip.conshdlrs[h];
            c.lastpropdomchgcount = c.storedpropdomchgcount;
            c.storednmarkedpropconss = 0;
        }
    }
    Ok(())
}

/*
 * simple functions implemented as defines
 */

impl Cons {
    /// `SCIPconsGetActiveDepth`: returns the depth in the tree at which the
    /// constraint is added (-1 for the global problem).
    pub fn get_active_depth(&self) -> i32 {
        self.activedepth
    }

    /// `SCIPconsIsActive`: returns TRUE iff constraint is active in the
    /// current node.
    pub fn is_active(&self) -> bool {
        self.updateactivate || (self.active && !self.updatedeactivate)
    }

    /// `SCIPconsIsUpdatedeactivate`.
    pub fn is_updatedeactivate(&self) -> bool {
        self.updatedeactivate
    }

    /// `SCIPconsGetValidDepth`: returns the depth in the tree at which the
    /// constraint is valid; returns `i32::MAX`, if the constraint is local
    /// and currently not valid.
    pub fn get_valid_depth(&self) -> i32 {
        if !self.local {
            0
        } else if !self.is_active() {
            i32::MAX
        } else if self.validdepth == -1 {
            self.get_active_depth()
        } else {
            self.validdepth
        }
    }

    /// `SCIPconsIsEnabled`: returns TRUE iff constraint is enabled in the
    /// current node.
    pub fn is_enabled(&self) -> bool {
        self.updateenable || (self.enabled && !self.updatedisable)
    }

    /// `SCIPconsIsSeparationEnabled`.
    pub fn is_separation_enabled(&self) -> bool {
        self.is_enabled() && (self.updatesepaenable || (self.sepaenabled && !self.updatesepadisable))
    }

    /// `SCIPconsIsPropagationEnabled`.
    pub fn is_propagation_enabled(&self) -> bool {
        self.is_enabled() && (self.updatepropenable || (self.propenabled && !self.updatepropdisable))
    }

    /// `SCIPconsIsDeleted`.
    pub fn is_deleted(&self) -> bool {
        self.deleted
    }

    /// `SCIPconsIsObsolete`.
    pub fn is_obsolete(&self) -> bool {
        self.updateobsolete || self.obsolete
    }

    /// `SCIPconsIsConflict`.
    pub fn is_conflict(&self) -> bool {
        self.conflict
    }

    /// `SCIPconsGetAge`.
    pub fn get_age(&self) -> f64 {
        self.age
    }

    /// `SCIPconsIsInitial`.
    pub fn is_initial(&self) -> bool {
        self.initial
    }

    /// `SCIPconsIsSeparated`.
    pub fn is_separated(&self) -> bool {
        self.separate
    }

    /// `SCIPconsIsEnforced`.
    pub fn is_enforced(&self) -> bool {
        self.enforce
    }

    /// `SCIPconsIsChecked`.
    pub fn is_checked(&self) -> bool {
        self.check
    }

    /// `SCIPconsIsMarkedPropagate`.
    pub fn is_marked_propagate(&self) -> bool {
        self.updatemarkpropagate || (self.markpropagate && !self.updateunmarkpropagate)
    }

    /// `SCIPconsIsPropagated`.
    pub fn is_propagated(&self) -> bool {
        self.propagate
    }

    /// `SCIPconsIsGlobal`.
    pub fn is_global(&self) -> bool {
        !self.local
    }

    /// `SCIPconsIsLocal`.
    pub fn is_local(&self) -> bool {
        self.local
    }

    /// `SCIPconsIsModifiable`.
    pub fn is_modifiable(&self) -> bool {
        self.modifiable
    }

    /// `SCIPconsIsDynamic`.
    pub fn is_dynamic(&self) -> bool {
        self.dynamic
    }

    /// `SCIPconsIsRemovable`.
    pub fn is_removable(&self) -> bool {
        self.removable
    }

    /// `SCIPconsIsStickingAtNode`.
    pub fn is_sticking_at_node(&self) -> bool {
        self.stickingatnode
    }

    /// `SCIPconsIsInProb`: returns TRUE iff constraint belongs to the
    /// global problem.
    pub fn is_in_prob(&self) -> bool {
        self.addconssetchg == crate::var::NONE && self.addarraypos >= 0
    }

    /// `SCIPconsIsOriginal`.
    pub fn is_original(&self) -> bool {
        self.original
    }

    /// `SCIPconsIsTransformed`.
    pub fn is_transformed(&self) -> bool {
        !self.original
    }

    /// `SCIPconsIsLockedPos`.
    pub fn is_locked_pos(&self) -> bool {
        self.nlockspos[LockType::Model as usize] > 0
    }

    /// `SCIPconsIsLockedNeg`.
    pub fn is_locked_neg(&self) -> bool {
        self.nlocksneg[LockType::Model as usize] > 0
    }

    /// `SCIPconsIsLocked`.
    pub fn is_locked(&self) -> bool {
        self.is_locked_pos() || self.is_locked_neg()
    }

    /// `SCIPconsGetNLocksPos`.
    pub fn get_n_locks_pos(&self) -> i32 {
        self.nlockspos[LockType::Model as usize]
    }

    /// `SCIPconsGetNLocksNeg`.
    pub fn get_n_locks_neg(&self) -> i32 {
        self.nlocksneg[LockType::Model as usize]
    }

    /// `SCIPconsIsLockedTypePos`.
    pub fn is_locked_type_pos(&self, locktype: LockType) -> bool {
        self.nlockspos[locktype as usize] > 0
    }

    /// `SCIPconsIsLockedTypeNeg`.
    pub fn is_locked_type_neg(&self, locktype: LockType) -> bool {
        self.nlocksneg[locktype as usize] > 0
    }

    /// `SCIPconsIsLockedType`.
    pub fn is_locked_type(&self, locktype: LockType) -> bool {
        self.nlockspos[locktype as usize] > 0 || self.nlocksneg[locktype as usize] > 0
    }

    /// `SCIPconsGetNLocksTypePos`.
    pub fn get_n_locks_type_pos(&self, locktype: LockType) -> i32 {
        self.nlockspos[locktype as usize]
    }

    /// `SCIPconsGetNLocksTypeNeg`.
    pub fn get_n_locks_type_neg(&self, locktype: LockType) -> i32 {
        self.nlocksneg[locktype as usize]
    }

    /// `SCIPconsIsAdded`: returns if the constraint was already added to a
    /// SCIP instance.
    pub fn is_added(&self) -> bool {
        self.addarraypos >= 0
    }

    /// `SCIPconsAddUpgradeLocks`: adds locks to (dis-)allow upgrading of
    /// constraint.
    pub fn add_upgrade_locks(&mut self, nlocks: i32) {
        self.nupgradelocks += nlocks;
    }

    /// `SCIPconsGetNUpgradeLocks`.
    pub fn get_n_upgrade_locks(&self) -> i32 {
        self.nupgradelocks
    }
}

/*
 * Inclusion (`scip_cons.c`)
 */

/// `SCIPincludeConshdlrBasic`: creates a constraint handler and includes it
/// in SCIP with its most fundamental callbacks; all non-fundamental (or
/// optional) callbacks as, e.g., init and exit callbacks, will be set to
/// `None`; the remaining ones are set with the `set_conshdlr_*` functions.
#[allow(clippy::too_many_arguments)]
pub fn include_conshdlr_basic(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    enfopriority: i32,
    chckpriority: i32,
    eagerfreq: i32,
    needscons: bool,
    consenfolp: Option<ConsEnfolp>,
    consenfops: Option<ConsEnfops>,
    conscheck: Option<ConsCheck>,
    conslock: Option<ConsLock>,
    conshdlrdata: Option<Box<dyn Any>>,
) -> ScipResult<ConshdlrId> {
    // Check whether constraint handler is already present.
    if crate::set::set_find_conshdlr(scip, name) != NONE {
        log::error!("constraint handler <{name}> already included.");
        return Err(crate::retcode::Retcode::InvalidData);
    }

    let methods = ConshdlrMethods {
        consenfolp,
        consenfops,
        conscheck,
        conslock,
        ..ConshdlrMethods::default()
    };
    let conshdlr = conshdlr_create(
        scip,
        name,
        desc,
        0,
        enfopriority,
        chckpriority,
        -1,
        -1,
        eagerfreq,
        0,
        false,
        false,
        needscons,
        PROPTIMING_BEFORELP,
        PRESOLTIMING_ALWAYS,
        methods,
        conshdlrdata,
    )?;
    crate::set::set_include_conshdlr(scip, conshdlr);

    Ok(conshdlr)
}

/// `SCIPsetConshdlrSepa`: sets all separation related callbacks/parameters
/// of the constraint handler.
pub fn set_conshdlr_sepa(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conssepalp: Option<ConsSepalp>,
    conssepasol: Option<ConsSepasol>,
    sepafreq: i32,
    sepapriority: i32,
    delaysepa: bool,
) -> ScipResult {
    let oldsepapriority = scip.conshdlrs[conshdlr].sepapriority;

    // SCIPconshdlrSetSepa.
    let c = &mut scip.conshdlrs[conshdlr];
    c.conssepalp = conssepalp;
    c.conssepasol = conssepasol;
    c.sepafreq = sepafreq;
    c.sepapriority = sepapriority;
    c.delaysepa = delaysepa;

    // Change the position of the constraint handler in the constraint handler
    // array w.r.t. its new sepa priority.
    if oldsepapriority != sepapriority {
        crate::set::set_reinsert_conshdlr_sepa_prio(scip, conshdlr, oldsepapriority);
    }

    let name = scip.conshdlrs[conshdlr].name.clone();
    crate::paramset::paramset_set_default_int(scip, &format!("constraints/{name}/sepafreq"), sepafreq)?;
    crate::paramset::paramset_set_default_bool(scip, &format!("constraints/{name}/delaysepa"), delaysepa)
}

/// `SCIPsetConshdlrProp`: sets all propagation related callbacks/parameters
/// of the constraint handler.
pub fn set_conshdlr_prop(scip: &mut Scip, conshdlr: ConshdlrId, consprop: Option<ConsProp>, propfreq: i32, delayprop: bool, proptiming: PropTiming) -> ScipResult {
    // SCIPconshdlrSetProp.
    let c = &mut scip.conshdlrs[conshdlr];
    c.consprop = consprop;
    c.propfreq = propfreq;
    c.delayprop = delayprop;
    c.proptiming = proptiming;

    let name = scip.conshdlrs[conshdlr].name.clone();
    crate::paramset::paramset_set_default_int(scip, &format!("constraints/{name}/propfreq"), propfreq)?;
    crate::paramset::paramset_set_default_int(scip, &format!("constraints/{name}/proptiming"), proptiming)?;
    crate::paramset::paramset_set_default_bool(scip, &format!("constraints/{name}/delayprop"), delayprop)
}

/// `SCIPsetConshdlrPresol`: sets presolving method of the constraint
/// handler.
pub fn set_conshdlr_presol(scip: &mut Scip, conshdlr: ConshdlrId, conspresol: Option<ConsPresol>, maxprerounds: i32, presoltiming: PresolTiming) -> ScipResult {
    // SCIPconshdlrSetPresol.
    let c = &mut scip.conshdlrs[conshdlr];
    c.conspresol = conspresol;
    c.maxprerounds = maxprerounds;

    // The interface change from delay flags to timings cannot be recognized
    // at compile time: Exit with an appropriate error message.
    if !(PRESOLTIMING_FAST..=PRESOLTIMING_MAX).contains(&presoltiming) {
        log::error!(
            "ERROR: 'PRESOLDELAY'-flag no longer available since SCIP 3.2, use an appropriate 'SCIP_PRESOLTIMING' for <{}> constraint handler instead.",
            c.name
        );
        return Err(crate::retcode::Retcode::ParameterWrongVal);
    }
    c.presoltiming = presoltiming;

    let name = scip.conshdlrs[conshdlr].name.clone();
    crate::paramset::paramset_set_default_int(scip, &format!("constraints/{name}/maxprerounds"), maxprerounds)?;
    crate::paramset::paramset_set_default_int(scip, &format!("constraints/{name}/presoltiming"), presoltiming)
}

/// The flags of constraint `cons` (`SCIPconsIsInitial(cons)`, ...,
/// `SCIPconsIsStickingAtNode(cons)`), as the `SCIPcreateCons...` calls that
/// copy them pass them.
pub fn cons_flags(scip: &Scip, cons: ConsId) -> ConsFlags {
    let c = &scip.conss[cons];
    ConsFlags {
        initial: c.is_initial(),
        separate: c.is_separated(),
        enforce: c.is_enforced(),
        check: c.is_checked(),
        propagate: c.is_propagated(),
        local: c.is_local(),
        modifiable: c.modifiable,
        dynamic: c.is_dynamic(),
        removable: c.is_removable(),
        stickingatnode: c.is_sticking_at_node(),
    }
}

/// `SCIPconshdlrGetConss`: the active constraints of the handler (a copy).
pub fn conshdlr_get_conss(scip: &Scip, conshdlr: ConshdlrId) -> Vec<ConsId> {
    let h = &scip.conshdlrs[conshdlr];
    h.conss.0[..h.nconss as usize].to_vec()
}
