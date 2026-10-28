//! SCIP's problem variables (`var.c`, `struct_var.h`).
//!
//! C unions (`data`, `reason`) are structs holding every member; only the
//! one the status selects is meaningful, as in C. C arrays are `Vec`s the
//! size of the C allocation, with C's counts kept beside them. Domain holes
//! are carried but never created by what this port solves.

use crate::{
    cvec::CVec,
    def::{DEFAULT_INFINITY, INVALID, REAL_MAX, REAL_MIN, UNKNOWN, eps_eq, eps_z, max, min, sqr},
    history::{History, ValueHistory, branchdir_opposite},
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    types::{BoundType, BoundchgType, BranchDir, ColId, ConsId, DomchgType, ImplintType, LockType, NLOCKTYPES, NodeId, PropId, SolId, VarId, VarStatus, VarType},
};

/// The id meaning `NULL`.
pub const NONE: usize = usize::MAX;

/// `SCIP_HOLE`: the open interval (left, right) missing from a domain.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hole {
    pub left: f64,
    pub right: f64,
}

/// `SCIP_HOLELIST`.
#[derive(Clone, Debug, Default)]
pub struct HoleList {
    pub hole: Hole,
    pub next: Option<Box<HoleList>>,
}

/// `SCIP_BRANCHINGDATA`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BranchingData {
    pub lpsolval: f64,
}

/// `SCIP_INFERENCEDATA`; `reason` is C's union of `cons` and `prop`.
#[derive(Clone, Copy, Debug)]
pub struct InferenceData {
    pub var: VarId,
    pub reason: Reason,
    pub info: i32,
}

/// The `reason` union of `SCIP_INFERENCEDATA`.
#[derive(Clone, Copy, Debug)]
pub struct Reason {
    pub cons: ConsId,
    pub prop: PropId,
}

impl Default for InferenceData {
    fn default() -> Self {
        Self {
            var: NONE,
            reason: Reason { cons: NONE, prop: NONE },
            info: 0,
        }
    }
}

/// The `data` union of `SCIP_BOUNDCHG`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BoundChgData {
    pub branchingdata: BranchingData,
    pub inferencedata: InferenceData,
}

/// `SCIP_BOUNDCHG`.
#[derive(Clone, Copy, Debug)]
pub struct BoundChg {
    pub newbound: f64,
    pub data: BoundChgData,
    pub var: VarId,
    pub boundchgtype: BoundchgType,
    pub boundtype: BoundType,
    pub inferboundtype: BoundType,
    pub applied: bool,
    pub redundant: bool,
}

impl Default for BoundChg {
    fn default() -> Self {
        Self {
            newbound: 0.0,
            data: BoundChgData::default(),
            var: NONE,
            boundchgtype: BoundchgType::Branching,
            boundtype: BoundType::Lower,
            inferboundtype: BoundType::Lower,
            applied: false,
            redundant: false,
        }
    }
}

/// `SCIP_BDCHGIDX`: when a bound change happened on the path to the
/// current node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BdChgIdx {
    pub depth: i32,
    pub pos: i32,
}

/// `SCIP_BDCHGINFO`.
#[derive(Clone, Copy, Debug)]
pub struct BdChgInfo {
    pub oldbound: f64,
    pub newbound: f64,
    pub var: VarId,
    pub inferencedata: InferenceData,
    pub bdchgidx: BdChgIdx,
    pub pos: u32,
    pub boundchgtype: BoundchgType,
    pub boundtype: BoundType,
    pub inferboundtype: BoundType,
    pub redundant: bool,
}

impl Default for BdChgInfo {
    fn default() -> Self {
        Self {
            oldbound: 0.0,
            newbound: 0.0,
            var: NONE,
            inferencedata: InferenceData::default(),
            bdchgidx: BdChgIdx::default(),
            pos: 0,
            boundchgtype: BoundchgType::Branching,
            boundtype: BoundType::Lower,
            inferboundtype: BoundType::Lower,
            redundant: false,
        }
    }
}

/// `SCIP_HOLECHG`. C keeps a pointer to the changed list pointer; holes are
/// never created here, so only the record's shape is kept.
#[derive(Clone, Debug, Default)]
pub struct HoleChg {
    pub var: VarId,
    pub newlist: Option<Box<HoleList>>,
    pub oldlist: Option<Box<HoleList>>,
}

/// `SCIP_DOMCHG`: C's union of the bound-only, static and dynamic layouts,
/// which share their leading fields.
#[derive(Clone, Debug)]
pub struct DomChg {
    pub domchgtype: DomchgType,
    pub nboundchgs: u32,
    pub boundchgs: CVec<BoundChg>,
    pub holechgs: CVec<HoleChg>,
    pub nholechgs: i32,
    pub boundchgssize: i32,
    pub holechgssize: i32,
}

/// `SCIP_DOM`.
#[derive(Clone, Debug, Default)]
pub struct Dom {
    pub lb: f64,
    pub ub: f64,
    pub holelist: Option<Box<HoleList>>,
}

/// `SCIP_ORIGINAL`.
#[derive(Clone, Debug)]
pub struct Original {
    pub origdom: Dom,
    pub transvar: VarId,
}

/// `SCIP_LOOSE`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Loose {
    pub minaggrcoef: f64,
    pub maxaggrcoef: f64,
}

/// `SCIP_AGGREGATE`: x = scalar * var + constant.
#[derive(Clone, Copy, Debug)]
pub struct Aggregate {
    pub scalar: f64,
    pub constant: f64,
    pub var: VarId,
}

/// `SCIP_MULTAGGR`: x = sum scalars[i] * vars[i] + constant.
#[derive(Clone, Debug, Default)]
pub struct Multaggr {
    pub constant: f64,
    pub scalars: CVec<f64>,
    pub vars: CVec<VarId>,
    pub nvars: i32,
    pub varssize: i32,
}

/// `SCIP_NEGATE`: x' = constant - x.
#[derive(Clone, Copy, Debug, Default)]
pub struct Negate {
    pub constant: f64,
}

/// The `data` union of `SCIP_VAR`.
#[derive(Clone, Debug)]
pub struct VarData {
    pub original: Original,
    pub loose: Loose,
    pub negate: Negate,
    pub aggregate: Aggregate,
    pub multaggr: Multaggr,
    pub col: ColId,
}

impl Default for VarData {
    fn default() -> Self {
        Self {
            original: Original {
                origdom: Dom::default(),
                transvar: NONE,
            },
            loose: Loose::default(),
            negate: Negate::default(),
            aggregate: Aggregate {
                scalar: 0.0,
                constant: 0.0,
                var: NONE,
            },
            multaggr: Multaggr::default(),
            col: NONE,
        }
    }
}

/// `SCIP_VBOUNDS`: variable bounds x <= b*z + d (or >=).
#[derive(Clone, Debug, Default)]
pub struct VBounds {
    pub vars: CVec<VarId>,
    pub coefs: CVec<f64>,
    pub constants: CVec<f64>,
    pub len: i32,
    pub size: i32,
}

/// `SCIP_IMPLICS`: implications of x <= 0 (index 0) and x >= 1 (index 1).
#[derive(Clone, Debug, Default)]
pub struct Implics {
    pub vars: [CVec<VarId>; 2],
    pub types: [CVec<BoundType>; 2],
    pub bounds: [CVec<f64>; 2],
    pub ids: [CVec<i32>; 2],
    pub size: [i32; 2],
    pub nimpls: [i32; 2],
}

/// `SCIP_CLIQUELIST`: the cliques x = 0 (index 0) and x = 1 (index 1) is in.
#[derive(Clone, Debug, Default)]
pub struct CliqueList {
    pub cliques: [CVec<usize>; 2],
    pub ncliques: [i32; 2],
    pub size: [i32; 2],
}

/// `SCIP_VAR`.
#[derive(Clone, Debug)]
pub struct Var {
    pub obj: f64,
    pub unchangedobj: f64,
    pub branchfactor: f64,
    pub rootsol: f64,
    pub bestrootsol: f64,
    pub bestrootredcost: f64,
    pub bestrootlpobjval: f64,
    pub relaxsol: f64,
    pub nlpsol: f64,
    pub primsolavg: f64,
    pub conflictlb: f64,
    pub conflictub: f64,
    pub conflictrelaxedlb: f64,
    pub conflictrelaxedub: f64,
    pub lazylb: f64,
    pub lazyub: f64,
    pub glbdom: Dom,
    pub locdom: Dom,
    pub data: VarData,
    pub name: String,
    pub parentvars: CVec<VarId>,
    pub negatedvar: VarId,
    pub vlbs: Option<Box<VBounds>>,
    pub vubs: Option<Box<VBounds>>,
    pub implics: Option<Box<Implics>>,
    pub cliquelist: Option<Box<CliqueList>>,
    /// The variable's event filter (an index into `Scip::eventfilters`),
    /// or `NONE` for original variables.
    pub eventfilter: usize,
    pub lbchginfos: CVec<BdChgInfo>,
    pub ubchginfos: CVec<BdChgInfo>,
    pub history: Box<History>,
    pub historycrun: Box<History>,
    pub valuehistory: Option<Box<ValueHistory>>,
    pub closestvblpcount: i64,
    pub index: i32,
    pub probindex: i32,
    pub pseudocandindex: i32,
    pub eventqueueindexobj: i32,
    pub eventqueueindexlb: i32,
    pub eventqueueindexub: i32,
    pub parentvarssize: i32,
    pub nparentvars: i32,
    pub nuses: i32,
    pub nlocksdown: [i32; NLOCKTYPES],
    pub nlocksup: [i32; NLOCKTYPES],
    pub branchpriority: i32,
    pub lbchginfossize: i32,
    pub nlbchginfos: i32,
    pub ubchginfossize: i32,
    pub nubchginfos: i32,
    pub conflictlbcount: i32,
    pub conflictubcount: i32,
    pub closestvlbidx: i32,
    pub closestvubidx: i32,
    pub initial: bool,
    pub removable: bool,
    pub deletable: bool,
    pub deleted: bool,
    pub donotaggr: bool,
    pub donotmultaggr: bool,
    pub vartype: VarType,
    pub varimpltype: ImplintType,
    pub varstatus: VarStatus,
    pub pseudocostflag: u32,
    pub branchdirection: crate::types::BranchDir,
    pub eventqueueimpl: bool,
    pub delglobalstructs: bool,
    pub relaxationonly: bool,
}

/// `varEnsureLbchginfosSize`.
fn var_ensure_lbchginfos_size(scip: &mut Scip, var: VarId, num: i32) {
    if num > scip.vars[var].lbchginfossize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.vars[var].lbchginfos.realloc(newsize);
        scip.vars[var].lbchginfossize = newsize;
    }
}

/// `varEnsureUbchginfosSize`.
fn var_ensure_ubchginfos_size(scip: &mut Scip, var: VarId, num: i32) {
    if num > scip.vars[var].ubchginfossize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.vars[var].ubchginfos.realloc(newsize);
        scip.vars[var].ubchginfossize = newsize;
    }
}

/// `varAddLbchginfo` and `varAddUbchginfo`: records a bound change on the
/// path to the current node.
fn var_add_bdchginfo(
    scip: &mut Scip,
    var: VarId,
    boundtype: BoundType,
    oldbound: f64,
    newbound: f64,
    depth: i32,
    pos: i32,
    infervar: VarId,
    infercons: ConsId,
    inferprop: PropId,
    inferinfo: i32,
    inferboundtype: BoundType,
    boundchgtype: BoundchgType,
) {
    let v = &scip.vars[var];
    let n = if boundtype == BoundType::Lower { v.nlbchginfos } else { v.nubchginfos };
    if boundtype == BoundType::Lower {
        var_ensure_lbchginfos_size(scip, var, n + 1);
    } else {
        var_ensure_ubchginfos_size(scip, var, n + 1);
    }
    let v = &mut scip.vars[var];
    let info = if boundtype == BoundType::Lower { &mut v.lbchginfos[n] } else { &mut v.ubchginfos[n] };
    info.oldbound = oldbound;
    info.newbound = newbound;
    info.var = var;
    info.bdchgidx.depth = depth;
    info.bdchgidx.pos = pos;
    info.pos = n as u32;
    info.boundchgtype = boundchgtype;
    info.boundtype = boundtype;
    info.redundant = false;
    info.inferboundtype = inferboundtype;
    info.inferencedata.var = infervar;
    info.inferencedata.info = inferinfo;
    match boundchgtype {
        BoundchgType::Branching => {}
        BoundchgType::ConsInfer => info.inferencedata.reason.cons = infercons,
        BoundchgType::PropInfer => info.inferencedata.reason.prop = inferprop,
    }
    if boundtype == BoundType::Lower {
        v.nlbchginfos += 1;
    } else {
        v.nubchginfos += 1;
    }
}

/// `SCIPboundchgApply`: applies a single bound change of a node's domain
/// change at `depth`, position `pos`. The bound change is the caller's copy
/// of the node's, written back after.
pub fn boundchg_apply(scip: &mut Scip, boundchg: &mut BoundChg, depth: i32, pos: i32, cutoff: &mut bool) -> ScipResult {
    *cutoff = false;

    // Ignore redundant bound changes.
    if boundchg.redundant {
        return Ok(());
    }
    let var = boundchg.var;

    // Apply bound change.
    match boundchg.boundtype {
        BoundType::Lower => {
            // Check, if the bound change is still active (could be replaced
            // by inference due to repropagation of higher node).
            if scip.set.is_gt(boundchg.newbound, scip.vars[var].locdom.lb) {
                if scip.set.is_le(boundchg.newbound, scip.vars[var].locdom.ub) {
                    // Add the bound change info to the variable's bound
                    // change info array.
                    let oldlb = scip.vars[var].locdom.lb;
                    match boundchg.boundchgtype {
                        BoundchgType::Branching => {
                            var_add_bdchginfo(
                                scip,
                                var,
                                BoundType::Lower,
                                oldlb,
                                boundchg.newbound,
                                depth,
                                pos,
                                NONE,
                                NONE,
                                NONE,
                                0,
                                BoundType::Lower,
                                BoundchgType::Branching,
                            );
                            scip.stat.lastbranchvar = var;
                            scip.stat.lastbranchdir = BranchDir::Upwards;
                            scip.stat.lastbranchvalue = boundchg.newbound;
                        }
                        BoundchgType::ConsInfer => {
                            let d = boundchg.data.inferencedata;
                            var_add_bdchginfo(
                                scip,
                                var,
                                BoundType::Lower,
                                oldlb,
                                boundchg.newbound,
                                depth,
                                pos,
                                d.var,
                                d.reason.cons,
                                NONE,
                                d.info,
                                boundchg.inferboundtype,
                                BoundchgType::ConsInfer,
                            );
                        }
                        BoundchgType::PropInfer => {
                            let d = boundchg.data.inferencedata;
                            var_add_bdchginfo(
                                scip,
                                var,
                                BoundType::Lower,
                                oldlb,
                                boundchg.newbound,
                                depth,
                                pos,
                                d.var,
                                NONE,
                                d.reason.prop,
                                d.info,
                                boundchg.inferboundtype,
                                BoundchgType::PropInfer,
                            );
                        }
                    }

                    // Change local bound of variable.
                    var_chg_lb_local(scip, var, boundchg.newbound)?;
                } else {
                    *cutoff = true;
                    boundchg.redundant = true; // bound change has not entered the lbchginfos array of the variable!
                }
            } else {
                // Mark bound change to be inactive.
                boundchg.redundant = true;
            }
        }
        BoundType::Upper => {
            if scip.set.is_lt(boundchg.newbound, scip.vars[var].locdom.ub) {
                if scip.set.is_ge(boundchg.newbound, scip.vars[var].locdom.lb) {
                    let oldub = scip.vars[var].locdom.ub;
                    match boundchg.boundchgtype {
                        BoundchgType::Branching => {
                            var_add_bdchginfo(
                                scip,
                                var,
                                BoundType::Upper,
                                oldub,
                                boundchg.newbound,
                                depth,
                                pos,
                                NONE,
                                NONE,
                                NONE,
                                0,
                                BoundType::Upper,
                                BoundchgType::Branching,
                            );
                            scip.stat.lastbranchvar = var;
                            scip.stat.lastbranchdir = BranchDir::Downwards;
                            scip.stat.lastbranchvalue = boundchg.newbound;
                        }
                        BoundchgType::ConsInfer => {
                            let d = boundchg.data.inferencedata;
                            var_add_bdchginfo(
                                scip,
                                var,
                                BoundType::Upper,
                                oldub,
                                boundchg.newbound,
                                depth,
                                pos,
                                d.var,
                                d.reason.cons,
                                NONE,
                                d.info,
                                boundchg.inferboundtype,
                                BoundchgType::ConsInfer,
                            );
                        }
                        BoundchgType::PropInfer => {
                            let d = boundchg.data.inferencedata;
                            var_add_bdchginfo(
                                scip,
                                var,
                                BoundType::Upper,
                                oldub,
                                boundchg.newbound,
                                depth,
                                pos,
                                d.var,
                                NONE,
                                d.reason.prop,
                                d.info,
                                boundchg.inferboundtype,
                                BoundchgType::PropInfer,
                            );
                        }
                    }
                    var_chg_ub_local(scip, var, boundchg.newbound)?;
                } else {
                    *cutoff = true;
                    boundchg.redundant = true;
                }
            } else {
                boundchg.redundant = true;
            }
        }
    }

    // Update the branching and inference history.
    if !boundchg.applied && !boundchg.redundant {
        if boundchg.boundchgtype == BoundchgType::Branching {
            let dir = if boundchg.boundtype == BoundType::Lower {
                BranchDir::Upwards
            } else {
                BranchDir::Downwards
            };
            var_inc_nbranchings(scip, var, dir, boundchg.newbound, depth)?;
        } else if scip.stat.lastbranchvar != NONE {
            // If the variable is the last branching variable, inference is
            // assigned to it.
            let (lastvar, lastdir, lastvalue) = (scip.stat.lastbranchvar, scip.stat.lastbranchdir, scip.stat.lastbranchvalue);
            var_inc_inference_sum(scip, lastvar, lastdir, lastvalue, 1.0)?;
        }
        boundchg.applied = true;
    }
    Ok(())
}

/// `SCIPboundchgUndo`.
pub fn boundchg_undo(scip: &mut Scip, boundchg: &BoundChg) -> ScipResult {
    // Ignore redundant bound changes.
    if boundchg.redundant {
        return Ok(());
    }
    let var = boundchg.var;

    // Undo bound change: apply the previous bound change of variable.
    match boundchg.boundtype {
        BoundType::Lower => {
            scip.vars[var].nlbchginfos -= 1;
            let n = scip.vars[var].nlbchginfos;
            let oldbound = scip.vars[var].lbchginfos[n].oldbound;
            var_chg_lb_local(scip, var, oldbound)?;
        }
        BoundType::Upper => {
            scip.vars[var].nubchginfos -= 1;
            let n = scip.vars[var].nubchginfos;
            let oldbound = scip.vars[var].ubchginfos[n].oldbound;
            var_chg_ub_local(scip, var, oldbound)?;
        }
    }

    // Update last branching variable.
    if boundchg.boundchgtype == BoundchgType::Branching {
        scip.stat.lastbranchvar = NONE;
        scip.stat.lastbranchvalue = UNKNOWN;
    }
    Ok(())
}

/// `boundchgApplyGlobal`: applies a single bound change to the global
/// problem.
fn boundchg_apply_global(scip: &mut Scip, boundchg: &BoundChg, cutoff: &mut bool) -> ScipResult {
    *cutoff = false;

    // Ignore redundant bound changes.
    if boundchg.redundant {
        return Ok(());
    }
    let var = boundchg.var;
    let newbound = boundchg.newbound;
    let boundtype = boundchg.boundtype;

    // Check if the bound change is redundant which can happen due to a
    // (better) global bound change which was performed after the bound
    // change was applied.
    if (boundtype == BoundType::Lower && scip.set.is_feas_le(newbound, scip.vars[var].glbdom.lb))
        || (boundtype == BoundType::Upper && scip.set.is_feas_ge(newbound, scip.vars[var].glbdom.ub))
    {
        return Ok(());
    }

    // Check if the bound change would lead to an infeasible domain.
    if (boundtype == BoundType::Lower && scip.set.is_feas_gt(newbound, scip.vars[var].glbdom.ub))
        || (boundtype == BoundType::Upper && scip.set.is_feas_lt(newbound, scip.vars[var].glbdom.lb))
    {
        *cutoff = true;
        return Ok(());
    }
    var_chg_bd_global(scip, var, newbound, boundtype)?;
    Ok(())
}

/// `boundchgCaptureData`.
fn boundchg_capture_data(scip: &mut Scip, boundchg: &BoundChg) {
    var_capture(scip, boundchg.var);
    match boundchg.boundchgtype {
        BoundchgType::Branching | BoundchgType::PropInfer => {}
        BoundchgType::ConsInfer => crate::cons::cons_capture(scip, boundchg.data.inferencedata.reason.cons),
    }
}

/// `boundchgReleaseData`.
fn boundchg_release_data(scip: &mut Scip, boundchg: &mut BoundChg) -> ScipResult {
    match boundchg.boundchgtype {
        BoundchgType::Branching | BoundchgType::PropInfer => {}
        BoundchgType::ConsInfer => crate::cons::cons_release(scip, boundchg.data.inferencedata.reason.cons)?,
    }
    var_release(scip, &mut boundchg.var)?;
    Ok(())
}

/// `domchgCreate`: an empty dynamic domain change.
fn domchg_create() -> Box<DomChg> {
    Box::new(DomChg {
        domchgtype: DomchgType::Dynamic,
        nboundchgs: 0,
        boundchgs: CVec::new(),
        holechgs: CVec::new(),
        nholechgs: 0,
        boundchgssize: 0,
        holechgssize: 0,
    })
}

/// `SCIPdomchgFree`.
pub fn domchg_free(scip: &mut Scip, domchg: &mut Option<Box<DomChg>>) -> ScipResult {
    if let Some(mut d) = domchg.take() {
        // Release variable and constraint captures.
        for i in 0..d.nboundchgs as i32 {
            boundchg_release_data(scip, &mut d.boundchgs[i])?;
        }
    }
    Ok(())
}

/// `domchgMakeDynamic`.
fn domchg_make_dynamic(domchg: &mut Option<Box<DomChg>>) {
    match domchg {
        None => *domchg = Some(domchg_create()),
        Some(d) => match d.domchgtype {
            DomchgType::Bound => {
                d.nholechgs = 0;
                d.holechgs = CVec::new();
                d.boundchgssize = d.nboundchgs as i32;
                d.holechgssize = 0;
                d.domchgtype = DomchgType::Dynamic;
            }
            DomchgType::Both => {
                d.boundchgssize = d.nboundchgs as i32;
                d.holechgssize = d.nholechgs;
                d.domchgtype = DomchgType::Dynamic;
            }
            DomchgType::Dynamic => {}
        },
    }
}

/// `SCIPdomchgMakeStatic`: shrinks the domain change to its size, freeing
/// it if empty.
pub fn domchg_make_static(scip: &mut Scip, domchg: &mut Option<Box<DomChg>>) -> ScipResult {
    let Some(d) = domchg.as_mut() else { return Ok(()) };
    match d.domchgtype {
        DomchgType::Bound => {
            if d.nboundchgs == 0 {
                domchg_free(scip, domchg)?;
            }
        }
        DomchgType::Both => {
            if d.nholechgs == 0 {
                if d.nboundchgs == 0 {
                    domchg_free(scip, domchg)?;
                } else {
                    d.domchgtype = DomchgType::Bound;
                }
            }
        }
        DomchgType::Dynamic => {
            if d.nholechgs == 0 {
                if d.nboundchgs == 0 {
                    domchg_free(scip, domchg)?;
                } else {
                    let n = d.nboundchgs as i32;
                    d.boundchgs.realloc(n);
                    d.holechgs = CVec::new();
                    d.domchgtype = DomchgType::Bound;
                }
            } else {
                let (n, h) = (d.nboundchgs as i32, d.nholechgs);
                d.boundchgs.realloc(n);
                d.holechgs.realloc(h);
                d.domchgtype = DomchgType::Both;
            }
        }
    }
    Ok(())
}

/// `domchgEnsureBoundchgsSize`.
fn domchg_ensure_boundchgs_size(scip: &Scip, d: &mut DomChg, num: i32) {
    if num > d.boundchgssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        d.boundchgs.realloc(newsize);
        d.boundchgssize = newsize;
    }
}

/// The domain change of `node`.
#[inline]
pub(crate) fn node_domchg(scip: &mut Scip, node: NodeId) -> &mut Option<Box<DomChg>> {
    &mut scip.nodes[node].domchg
}

/// `SCIPdomchgApply`: applies the domain change of `node` at `depth`.
pub fn domchg_apply(scip: &mut Scip, node: NodeId, depth: i32, cutoff: &mut bool) -> ScipResult {
    *cutoff = false;
    let Some(n) = scip.nodes[node].domchg.as_ref().map(|d| d.nboundchgs as i32) else {
        return Ok(());
    };

    // Apply bound changes.
    let mut i = 0;
    while i < n {
        let mut boundchg = node_domchg(scip, node).as_ref().unwrap().boundchgs[i];
        boundchg_apply(scip, &mut boundchg, depth, i, cutoff)?;
        node_domchg(scip, node).as_mut().unwrap().boundchgs[i] = boundchg;
        if *cutoff {
            break;
        }
        i += 1;
    }

    // Mark all bound changes after a cutoff redundant.
    let d = node_domchg(scip, node).as_mut().unwrap();
    while i < n {
        d.boundchgs[i].redundant = true;
        i += 1;
    }
    Ok(())
}

/// `SCIPdomchgUndo`.
pub fn domchg_undo(scip: &mut Scip, node: NodeId) -> ScipResult {
    let Some(n) = scip.nodes[node].domchg.as_ref().map(|d| d.nboundchgs as i32) else {
        return Ok(());
    };

    // Undo bound changes.
    let mut i = n - 1;
    while i >= 0 {
        let boundchg = node_domchg(scip, node).as_ref().unwrap().boundchgs[i];
        boundchg_undo(scip, &boundchg)?;
        i -= 1;
    }
    Ok(())
}

/// `SCIPdomchgApplyGlobal`.
pub fn domchg_apply_global(scip: &mut Scip, node: NodeId, cutoff: &mut bool) -> ScipResult {
    *cutoff = false;
    let Some(n) = scip.nodes[node].domchg.as_ref().map(|d| d.nboundchgs as i32) else {
        return Ok(());
    };
    for i in 0..n {
        let boundchg = node_domchg(scip, node).as_ref().unwrap().boundchgs[i];
        boundchg_apply_global(scip, &boundchg, cutoff)?;
        if *cutoff {
            break;
        }
    }
    Ok(())
}

/// `SCIPdomchgAddBoundchg`: adds a bound change to the domain change of
/// `node`, creating it (dynamic) if needed.
pub fn domchg_add_boundchg(
    scip: &mut Scip,
    node: NodeId,
    var: VarId,
    newbound: f64,
    boundtype: BoundType,
    boundchgtype: BoundchgType,
    lpsolval: f64,
    infervar: VarId,
    infercons: ConsId,
    inferprop: PropId,
    inferinfo: i32,
    inferboundtype: BoundType,
) -> ScipResult {
    // If domain change data doesn't exist, create it; if domain change is
    // static, convert it into dynamic change.
    let mut domchg = node_domchg(scip, node).take();
    if domchg.as_ref().is_none_or(|d| d.domchgtype != DomchgType::Dynamic) {
        domchg_make_dynamic(&mut domchg);
    }
    let d = domchg.as_mut().unwrap();

    // Get memory for additional bound change.
    domchg_ensure_boundchgs_size(scip, d, d.nboundchgs as i32 + 1);

    // Fill in the bound change data.
    let boundchg = &mut d.boundchgs[d.nboundchgs as i32];
    boundchg.var = var;
    match boundchgtype {
        BoundchgType::Branching => boundchg.data.branchingdata.lpsolval = lpsolval,
        BoundchgType::ConsInfer => {
            boundchg.data.inferencedata.var = infervar;
            boundchg.data.inferencedata.reason.cons = infercons;
            boundchg.data.inferencedata.info = inferinfo;
        }
        BoundchgType::PropInfer => {
            boundchg.data.inferencedata.var = infervar;
            boundchg.data.inferencedata.reason.prop = inferprop;
            boundchg.data.inferencedata.info = inferinfo;
        }
    }
    boundchg.newbound = newbound;
    boundchg.boundchgtype = boundchgtype;
    boundchg.boundtype = boundtype;
    boundchg.inferboundtype = inferboundtype;
    boundchg.applied = false;
    boundchg.redundant = false;
    let added = *boundchg;
    d.nboundchgs += 1;
    *node_domchg(scip, node) = domchg;

    // Capture branching and inference data associated with the bound
    // changes.
    boundchg_capture_data(scip, &added);
    Ok(())
}

/// `adjustedLb`: the lower bound rounded as the variable's type requires.
fn adjusted_lb(scip: &Scip, isintegral: bool, lb: f64) -> f64 {
    if lb < 0.0 && scip.set.is_infinity(-lb) {
        -scip.set.infinity()
    } else if lb > 0.0 && scip.set.is_infinity(lb) {
        scip.set.infinity()
    } else if isintegral {
        scip.set.feas_ceil(lb)
    } else if lb > 0.0 && lb < scip.set.epsilon() {
        0.0
    } else {
        lb
    }
}

/// `adjustedUb`.
fn adjusted_ub(scip: &Scip, isintegral: bool, ub: f64) -> f64 {
    if ub > 0.0 && scip.set.is_infinity(ub) {
        scip.set.infinity()
    } else if ub < 0.0 && scip.set.is_infinity(-ub) {
        -scip.set.infinity()
    } else if isintegral {
        scip.set.feas_floor(ub)
    } else if ub < 0.0 && ub > -scip.set.epsilon() {
        0.0
    } else {
        ub
    }
}

/// `SCIPvarRemoveCliquesImplicsVbs`: removes (redundant) cliques,
/// implications and variable bounds of `var` from all other variables'
/// structures, and from `var` itself if `removefromvar`.
pub fn var_remove_cliques_implics_vbs(scip: &mut Scip, var: VarId, irrelevantvar: bool, onlyredundant: bool, removefromvar: bool) -> ScipResult {
    let lb = scip.vars[var].glbdom.lb;
    let ub = scip.vars[var].glbdom.ub;

    // Remove implications of (fixed) binary variable.
    if scip.vars[var].implics.is_some() && (!onlyredundant || lb > 0.5 || ub < 0.5) {
        let mut varfixing = false;
        loop {
            let implics = scip.vars[var].implics.as_ref().unwrap();
            let fix = varfixing as usize;
            let nimpls = implics.nimpls[fix];
            let implvars = implics.vars[fix].head(nimpls).to_vec();
            let impltypes = implics.types[fix].head(nimpls).to_vec();
            for i in 0..nimpls as usize {
                let implvar = implvars[i];
                let impltype = impltypes[i];

                // Remove for all implications z == 0 / 1 ==> x <= p / x >= p
                // (x not binary) the variable bound.
                if impltype == BoundType::Upper {
                    if scip.vars[implvar].vubs.is_some() {
                        crate::implics::vbounds_del(&mut scip.vars[implvar].vubs, var, varfixing);
                        scip.vars[implvar].closestvblpcount = -1;
                        scip.vars[var].closestvblpcount = -1;
                    }
                } else if scip.vars[implvar].vlbs.is_some() {
                    crate::implics::vbounds_del(&mut scip.vars[implvar].vlbs, var, !varfixing);
                    scip.vars[implvar].closestvblpcount = -1;
                    scip.vars[var].closestvblpcount = -1;
                }
            }
            varfixing = !varfixing;
            if !varfixing {
                break;
            }
        }
        if removefromvar {
            // Free the implications data structures.
            scip.vars[var].implics = None;
        }
    }

    // Remove the (redundant) variable lower bounds.
    if scip.vars[var].vlbs.is_some() {
        let nvbds = scip.vars[var].vlbs.as_ref().unwrap().len;
        let mut newnvbds = 0;
        for i in 0..nvbds {
            let vb = scip.vars[var].vlbs.as_ref().unwrap();
            let implvar = vb.vars[i];
            let coef = vb.coefs[i];
            let constant = vb.constants[i];

            // Variable bound is not redundant: keep it.
            if onlyredundant {
                let vbound = max(coef * scip.vars[implvar].glbdom.ub, coef * scip.vars[implvar].glbdom.lb) + constant;
                if scip.set.is_feas_gt(vbound, lb) {
                    if removefromvar {
                        if newnvbds < i {
                            let vb = scip.vars[var].vlbs.as_mut().unwrap();
                            vb.vars[newnvbds] = implvar;
                            vb.coefs[newnvbds] = coef;
                            vb.constants[newnvbds] = constant;
                        }
                        newnvbds += 1;
                    }
                    continue;
                }
            }

            // Remove the corresponding implication.
            if scip.vars[implvar].implics.is_some() {
                crate::implics::implics_del(&mut scip.vars[implvar].implics, coef > 0.0, var, BoundType::Lower);
            }
            if coef > 0.0 && scip.vars[implvar].vubs.is_some() {
                crate::implics::vbounds_del(&mut scip.vars[implvar].vubs, var, false);
                scip.vars[implvar].closestvblpcount = -1;
                scip.vars[var].closestvblpcount = -1;
            } else if coef < 0.0 && scip.vars[implvar].vlbs.is_some() {
                crate::implics::vbounds_del(&mut scip.vars[implvar].vlbs, var, true);
                scip.vars[implvar].closestvblpcount = -1;
                scip.vars[var].closestvblpcount = -1;
            }
        }
        if removefromvar {
            // Update the number of variable bounds.
            crate::implics::vbounds_shrink(&mut scip.vars[var].vlbs, newnvbds);
            scip.vars[var].closestvblpcount = -1;
        }
    }

    // Remove the (redundant) variable upper bounds.
    if scip.vars[var].vubs.is_some() {
        let nvbds = scip.vars[var].vubs.as_ref().unwrap().len;
        let mut newnvbds = 0;
        for i in 0..nvbds {
            let vb = scip.vars[var].vubs.as_ref().unwrap();
            let implvar = vb.vars[i];
            let coef = vb.coefs[i];
            let constant = vb.constants[i];
            if onlyredundant {
                let vbound = min(coef * scip.vars[implvar].glbdom.ub, coef * scip.vars[implvar].glbdom.lb) + constant;
                if scip.set.is_feas_lt(vbound, ub) {
                    if removefromvar {
                        if newnvbds < i {
                            let vb = scip.vars[var].vubs.as_mut().unwrap();
                            vb.vars[newnvbds] = implvar;
                            vb.coefs[newnvbds] = coef;
                            vb.constants[newnvbds] = constant;
                        }
                        newnvbds += 1;
                    }
                    continue;
                }
            }
            if scip.vars[implvar].implics.is_some() {
                crate::implics::implics_del(&mut scip.vars[implvar].implics, coef < 0.0, var, BoundType::Upper);
            }
            if coef < 0.0 && scip.vars[implvar].vubs.is_some() {
                crate::implics::vbounds_del(&mut scip.vars[implvar].vubs, var, true);
                scip.vars[implvar].closestvblpcount = -1;
                scip.vars[var].closestvblpcount = -1;
            } else if coef > 0.0 && scip.vars[implvar].vlbs.is_some() {
                crate::implics::vbounds_del(&mut scip.vars[implvar].vlbs, var, false);
                scip.vars[implvar].closestvblpcount = -1;
                scip.vars[var].closestvblpcount = -1;
            }
        }
        if removefromvar {
            crate::implics::vbounds_shrink(&mut scip.vars[var].vubs, newnvbds);
            scip.vars[var].closestvblpcount = -1;
        }
    }

    // Remove the variable from all cliques.
    if var_is_binary(scip, var) {
        crate::implics::cliquelist_remove_from_cliques(scip, var, irrelevantvar);
    }
    Ok(())
}

/// `varCreate`: a new variable with the next index, not yet in the
/// arena's status-specific state.
fn var_create(
    scip: &mut Scip,
    name: Option<&str>,
    mut lb: f64,
    mut ub: f64,
    obj: f64,
    mut vartype: VarType,
    impltype: ImplintType,
    initial: bool,
    removable: bool,
) -> ScipResult<VarId> {
    if scip.set.is_infinity(obj.abs()) {
        log::error!("invalid objective coefficient: value is infinite");
        return Err(Retcode::InvalidData);
    }

    // Adjust bounds of variable.
    let integral = vartype != VarType::Continuous || impltype != ImplintType::None;
    lb = adjusted_lb(scip, integral, lb);
    ub = adjusted_ub(scip, integral, ub);

    // Convert [0,1]-integers into binary variables and check that binary
    // variables have correct bounds.
    if (lb == 0.0 || lb == 1.0) && (ub == 0.0 || ub == 1.0) {
        if vartype == VarType::Integer {
            vartype = VarType::Binary;
        }
    } else if vartype == VarType::Binary {
        log::error!("invalid bounds [{lb:.2},{ub:.2}] for binary variable <{}>", name.unwrap_or(""));
        return Err(Retcode::InvalidData);
    }

    // `varSetName`.
    let name = match name {
        Some(name) => name.to_owned(),
        None => format!("_var{}_", scip.stat.nvaridx),
    };
    let index = scip.stat.nvaridx;
    let v = Var {
        obj,
        unchangedobj: obj,
        branchfactor: 1.0,
        rootsol: 0.0,
        bestrootsol: 0.0,
        bestrootredcost: 0.0,
        bestrootlpobjval: INVALID,
        relaxsol: 0.0,
        nlpsol: 0.0,
        primsolavg: 0.5 * (lb + ub),
        conflictlb: REAL_MIN,
        conflictub: REAL_MAX,
        conflictrelaxedlb: REAL_MIN,
        conflictrelaxedub: REAL_MAX,
        lazylb: -scip.set.infinity(),
        lazyub: scip.set.infinity(),
        glbdom: Dom { lb, ub, holelist: None },
        locdom: Dom { lb, ub, holelist: None },
        data: VarData::default(),
        name,
        parentvars: CVec::new(),
        negatedvar: NONE,
        vlbs: None,
        vubs: None,
        implics: None,
        cliquelist: None,
        eventfilter: NONE,
        lbchginfos: CVec::new(),
        ubchginfos: CVec::new(),
        history: Box::new(History::new()),
        historycrun: Box::new(History::new()),
        valuehistory: None,
        closestvblpcount: -1,
        index,
        probindex: -1,
        pseudocandindex: -1,
        eventqueueindexobj: -1,
        eventqueueindexlb: -1,
        eventqueueindexub: -1,
        parentvarssize: 0,
        nparentvars: 0,
        nuses: 0,
        nlocksdown: [0; NLOCKTYPES],
        nlocksup: [0; NLOCKTYPES],
        branchpriority: 0,
        lbchginfossize: 0,
        nlbchginfos: 0,
        ubchginfossize: 0,
        nubchginfos: 0,
        conflictlbcount: 0,
        conflictubcount: 0,
        closestvlbidx: -1,
        closestvubidx: -1,
        initial,
        removable,
        deletable: false,
        deleted: false,
        donotaggr: false,
        donotmultaggr: false,
        vartype,
        varimpltype: impltype,
        varstatus: VarStatus::Original,
        pseudocostflag: 0,
        branchdirection: BranchDir::Auto,
        eventqueueimpl: false,
        delglobalstructs: false,
        relaxationonly: false,
    };
    scip.vars.insert_at(index as usize, v);
    scip.stat.nvaridx += 1;
    Ok(index as usize)
}

/// `SCIPvarCreateOriginal`: an original variable, captured once.
pub fn var_create_original(
    scip: &mut Scip,
    name: Option<&str>,
    lb: f64,
    ub: f64,
    obj: f64,
    vartype: VarType,
    impltype: ImplintType,
    initial: bool,
    removable: bool,
) -> ScipResult<VarId> {
    let var = var_create(scip, name, lb, ub, obj, vartype, impltype, initial, removable)?;
    let v = &mut scip.vars[var];
    v.varstatus = VarStatus::Original;
    v.data.original.origdom.holelist = None;
    // The bounds as given, not as varCreate adjusted them.
    v.data.original.origdom.lb = lb;
    v.data.original.origdom.ub = ub;
    v.data.original.transvar = NONE;

    // Capture variable.
    var_capture(scip, var);
    Ok(var)
}

/// `SCIPvarCreateTransformed`: a loose transformed variable with its event
/// filter, captured once.
pub fn var_create_transformed(
    scip: &mut Scip,
    name: Option<&str>,
    lb: f64,
    ub: f64,
    obj: f64,
    vartype: VarType,
    impltype: ImplintType,
    initial: bool,
    removable: bool,
) -> ScipResult<VarId> {
    let var = var_create(scip, name, lb, ub, obj, vartype, impltype, initial, removable)?;

    // Create event filter for transformed variable.
    scip.vars[var].eventfilter = crate::event::eventfilter_create(scip);

    // Set variable status and data.
    let v = &mut scip.vars[var];
    v.varstatus = VarStatus::Loose;
    v.data.loose.minaggrcoef = 1.0;
    v.data.loose.maxaggrcoef = 1.0;

    // Capture variable.
    var_capture(scip, var);
    Ok(var)
}

/// `SCIPvarCopy`: an original variable in `scip` copying `sourcevar` of
/// `sourcescip`, recorded in `varmap`. Variable data is not ported.
pub fn var_copy(scip: &mut Scip, sourcescip: &Scip, sourcevar: VarId, varmap: &mut crate::hash::HashMap<usize>, global: bool) -> ScipResult<VarId> {
    let sv = &sourcescip.vars[sourcevar];

    // Get global or local bounds of source variable.
    let (lb, ub) = if sv.varstatus == VarStatus::Original {
        (var_get_lb_original(sourcescip, sourcevar), var_get_ub_original(sourcescip, sourcevar))
    } else if global {
        (sv.glbdom.lb, sv.glbdom.ub)
    } else {
        (sv.locdom.lb, sv.locdom.ub)
    };

    // Create var and copy its basic data.
    let var = var_create_original(scip, Some(&sv.name), lb, ub, sv.obj, sv.vartype, sv.varimpltype, sv.initial, sv.removable)?;
    scip.vars[var].donotaggr = sv.donotaggr;
    scip.vars[var].donotmultaggr = sv.donotmultaggr;

    // Insert variable into mapping between source SCIP and the target SCIP.
    let _ = varmap.insert(sourcevar as u64, var);

    // In case there exists variable data and the variable data copy
    // callback, try to copy variable data: not ported.

    // Copy history values.
    if scip.set.history_allowtransfer {
        var_merge_histories_from(scip, var, &sourcescip.vars[sourcevar].historycrun);
    }
    Ok(var)
}

/// `varEnsureParentvarsSize`.
fn var_ensure_parentvars_size(scip: &mut Scip, var: VarId, num: i32) {
    if num > scip.vars[var].parentvarssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.vars[var].parentvars.realloc(newsize);
        scip.vars[var].parentvarssize = newsize;
    }
}

/// `varAddParent`.
fn var_add_parent(scip: &mut Scip, var: VarId, parentvar: VarId) {
    let n = scip.vars[var].nparentvars + 1;
    var_ensure_parentvars_size(scip, var, n);
    let np = scip.vars[var].nparentvars;
    scip.vars[var].parentvars[np] = parentvar;
    scip.vars[var].nparentvars += 1;
    var_capture(scip, parentvar);
}

/// `varFreeParents`.
fn var_free_parents(scip: &mut Scip, var: VarId) -> ScipResult {
    // Release the parent variables and remove the link from the parent
    // variable to the child.
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        match scip.vars[parentvar].varstatus {
            VarStatus::Original => scip.vars[parentvar].data.original.transvar = NONE,
            VarStatus::Aggregated => scip.vars[parentvar].data.aggregate.var = NONE,
            VarStatus::Negated => {
                scip.vars[parentvar].negatedvar = NONE;
                scip.vars[var].negatedvar = NONE;
            }
            _ => {
                log::error!("parent variable is neither ORIGINAL, AGGREGATED nor NEGATED");
                return Err(Retcode::InvalidData);
            }
        }
        let mut released = parentvar;
        var_release(scip, &mut released)?;
        scip.vars[var].parentvars[i] = released;
    }
    scip.vars[var].parentvars = CVec::new();
    Ok(())
}

/// `varFree`.
fn var_free(scip: &mut Scip, var: VarId) -> ScipResult {
    // Free variable data structure.
    if scip.vars[var].varstatus == VarStatus::Column {
        let col = scip.vars[var].data.col;
        crate::lp::col_free(scip, col)?;
    }

    // Release all parent variables and free the parentvars array.
    var_free_parents(scip, var)?;

    // Free event filter.
    if scip.vars[var].eventfilter != NONE {
        let filter = scip.vars[var].eventfilter;
        crate::event::eventfilter_free(scip, filter)?;
    }
    scip.vars.remove(var);
    Ok(())
}

/// `SCIPvarCapture`.
#[inline]
pub fn var_capture(scip: &mut Scip, var: VarId) {
    scip.vars[var].nuses += 1;
}

/// `SCIPvarRelease`: frees the variable when its last use is released;
/// sets `*var` to `NONE`.
pub fn var_release(scip: &mut Scip, var: &mut VarId) -> ScipResult {
    scip.vars[*var].nuses -= 1;
    if scip.vars[*var].nuses == 0 {
        var_free(scip, *var)?;
    }
    *var = NONE;
    Ok(())
}

/// `SCIPvarChgName`.
pub fn var_chg_name(scip: &mut Scip, var: VarId, name: &str) {
    scip.vars[var].name = name.to_owned();
}

/// `SCIPvarInitSolve`.
pub fn var_init_solve(scip: &mut Scip, var: VarId) {
    scip.vars[var].historycrun.reset();
    scip.vars[var].conflictlbcount = 0;
    scip.vars[var].conflictubcount = 0;
}

/// `varEventVarUnlocked`.
fn var_event_var_unlocked(scip: &mut Scip, var: VarId) -> ScipResult {
    let event = crate::event::event_create_var_unlocked(var);
    crate::event::eventqueue_add(scip, event)
}

/// `SCIPvarAddLocks`.
pub fn var_add_locks(scip: &mut Scip, var: VarId, locktype: LockType, mut addnlocksdown: i32, mut addnlocksup: i32) -> ScipResult {
    // Nothing to do if nothing to add.
    if addnlocksdown == 0 && addnlocksup == 0 {
        return Ok(());
    }
    let lt = locktype as usize;
    let mut lockvar = var;
    loop {
        match scip.vars[lockvar].varstatus {
            VarStatus::Original => {
                if scip.vars[lockvar].data.original.transvar != NONE {
                    lockvar = scip.vars[lockvar].data.original.transvar;
                } else {
                    scip.vars[lockvar].nlocksdown[lt] += addnlocksdown;
                    scip.vars[lockvar].nlocksup[lt] += addnlocksup;
                    return Ok(());
                }
            }
            VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => {
                scip.vars[lockvar].nlocksdown[lt] += addnlocksdown;
                scip.vars[lockvar].nlocksup[lt] += addnlocksup;
                if locktype == LockType::Model && scip.vars[lockvar].nlocksdown[lt] <= 1 && scip.vars[lockvar].nlocksup[lt] <= 1 {
                    var_event_var_unlocked(scip, lockvar)?;
                }
                return Ok(());
            }
            VarStatus::Aggregated => {
                if scip.vars[lockvar].data.aggregate.scalar < 0.0 {
                    std::mem::swap(&mut addnlocksdown, &mut addnlocksup);
                }
                lockvar = scip.vars[lockvar].data.aggregate.var;
            }
            VarStatus::MultAggr => {
                scip.vars[lockvar].nlocksdown[lt] += addnlocksdown;
                scip.vars[lockvar].nlocksup[lt] += addnlocksup;
                let mut v = scip.vars[lockvar].data.multaggr.nvars - 1;
                while v >= 0 {
                    let (scalar, aggrvar) = (scip.vars[lockvar].data.multaggr.scalars[v], scip.vars[lockvar].data.multaggr.vars[v]);
                    if scalar > 0.0 {
                        var_add_locks(scip, aggrvar, locktype, addnlocksdown, addnlocksup)?;
                    } else {
                        var_add_locks(scip, aggrvar, locktype, addnlocksup, addnlocksdown)?;
                    }
                    v -= 1;
                }
                return Ok(());
            }
            VarStatus::Negated => {
                std::mem::swap(&mut addnlocksdown, &mut addnlocksup);
                lockvar = scip.vars[lockvar].negatedvar;
            }
        }
    }
}

/// `SCIPvarGetNLocksDownType`.
pub fn var_get_nlocks_down_type(scip: &Scip, var: VarId, locktype: LockType) -> i32 {
    let v = &scip.vars[var];
    let lt = locktype as usize;
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar != NONE {
                var_get_nlocks_down_type(scip, v.data.original.transvar, locktype)
            } else {
                v.nlocksdown[lt]
            }
        }
        VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => v.nlocksdown[lt],
        VarStatus::Aggregated => {
            if v.data.aggregate.scalar > 0.0 {
                var_get_nlocks_down_type(scip, v.data.aggregate.var, locktype)
            } else {
                var_get_nlocks_up_type(scip, v.data.aggregate.var, locktype)
            }
        }
        VarStatus::MultAggr => {
            let mut nlocks = 0;
            for i in 0..v.data.multaggr.nvars {
                if v.data.multaggr.scalars[i] > 0.0 {
                    nlocks += var_get_nlocks_down_type(scip, v.data.multaggr.vars[i], locktype);
                } else {
                    nlocks += var_get_nlocks_up_type(scip, v.data.multaggr.vars[i], locktype);
                }
            }
            nlocks
        }
        VarStatus::Negated => var_get_nlocks_up_type(scip, v.negatedvar, locktype),
    }
}

/// `SCIPvarGetNLocksUpType`.
pub fn var_get_nlocks_up_type(scip: &Scip, var: VarId, locktype: LockType) -> i32 {
    let v = &scip.vars[var];
    let lt = locktype as usize;
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar != NONE {
                var_get_nlocks_up_type(scip, v.data.original.transvar, locktype)
            } else {
                v.nlocksup[lt]
            }
        }
        VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => v.nlocksup[lt],
        VarStatus::Aggregated => {
            if v.data.aggregate.scalar > 0.0 {
                var_get_nlocks_up_type(scip, v.data.aggregate.var, locktype)
            } else {
                var_get_nlocks_down_type(scip, v.data.aggregate.var, locktype)
            }
        }
        VarStatus::MultAggr => {
            let mut nlocks = 0;
            for i in 0..v.data.multaggr.nvars {
                if v.data.multaggr.scalars[i] > 0.0 {
                    nlocks += var_get_nlocks_up_type(scip, v.data.multaggr.vars[i], locktype);
                } else {
                    nlocks += var_get_nlocks_down_type(scip, v.data.multaggr.vars[i], locktype);
                }
            }
            nlocks
        }
        VarStatus::Negated => var_get_nlocks_down_type(scip, v.negatedvar, locktype),
    }
}

/// `SCIPvarGetNLocksDown`.
pub fn var_get_nlocks_down(scip: &Scip, var: VarId) -> i32 {
    var_get_nlocks_down_type(scip, var, LockType::Model)
}

/// `SCIPvarGetNLocksUp`.
pub fn var_get_nlocks_up(scip: &Scip, var: VarId) -> i32 {
    var_get_nlocks_up_type(scip, var, LockType::Model)
}

/// `SCIPvarMayRoundDown`.
pub fn var_may_round_down(scip: &Scip, var: VarId) -> bool {
    var_get_nlocks_down_type(scip, var, LockType::Model) == 0
}

/// `SCIPvarMayRoundUp`.
pub fn var_may_round_up(scip: &Scip, var: VarId) -> bool {
    var_get_nlocks_up_type(scip, var, LockType::Model) == 0
}

/// `SCIPvarTransform`: the transformed variable of `origvar`, created if
/// it does not exist yet; captured either way.
pub fn var_transform(scip: &mut Scip, origvar: VarId, objsense: crate::types::ObjSense, transvar: &mut VarId) -> ScipResult {
    // Check if variable is already transformed.
    if scip.vars[origvar].data.original.transvar != NONE {
        *transvar = scip.vars[origvar].data.original.transvar;
        var_capture(scip, *transvar);
        return Ok(());
    }

    // Create transformed variable.
    let o = &scip.vars[origvar];
    let name = format!("t_{}", o.name);
    let (lb, ub, obj, vartype, impltype, initial, removable) = (o.glbdom.lb, o.glbdom.ub, objsense as i32 as f64 * o.obj, o.vartype, o.varimpltype, o.initial, o.removable);
    *transvar = var_create_transformed(scip, Some(&name), lb, ub, obj, vartype, impltype, initial, removable)?;

    // Copy the branch factor and priority.
    let o = scip.vars[origvar].clone_settings();
    let t = &mut scip.vars[*transvar];
    t.branchfactor = o.branchfactor;
    t.branchpriority = o.branchpriority;
    t.branchdirection = o.branchdirection;

    // Duplicate hole lists.
    t.glbdom.holelist = o.glbholes;
    t.locdom.holelist = o.locholes;

    // Link original and transformed variable.
    scip.vars[origvar].data.original.transvar = *transvar;
    var_add_parent(scip, *transvar, origvar);

    // Copy rounding locks.
    let t = &mut scip.vars[*transvar];
    t.nlocksdown = o.nlocksdown;
    t.nlocksup = o.nlocksup;

    // Copy donot(mult)aggr status.
    t.donotaggr = o.donotaggr;
    t.donotmultaggr = o.donotmultaggr;

    // Copy lazy bounds.
    t.lazylb = o.lazylb;
    t.lazyub = o.lazyub;

    // Transfer local and global statistics.
    t.history.unite(&o.history, false);
    Ok(())
}

/// What `SCIPvarTransform` copies from the original variable.
struct VarSettings {
    branchfactor: f64,
    branchpriority: i32,
    branchdirection: BranchDir,
    glbholes: Option<Box<HoleList>>,
    locholes: Option<Box<HoleList>>,
    nlocksdown: [i32; NLOCKTYPES],
    nlocksup: [i32; NLOCKTYPES],
    donotaggr: bool,
    donotmultaggr: bool,
    lazylb: f64,
    lazyub: f64,
    history: History,
}

impl Var {
    fn clone_settings(&self) -> VarSettings {
        VarSettings {
            branchfactor: self.branchfactor,
            branchpriority: self.branchpriority,
            branchdirection: self.branchdirection,
            glbholes: self.glbdom.holelist.clone(),
            locholes: self.locdom.holelist.clone(),
            nlocksdown: self.nlocksdown,
            nlocksup: self.nlocksup,
            donotaggr: self.donotaggr,
            donotmultaggr: self.donotmultaggr,
            lazylb: self.lazylb,
            lazyub: self.lazyub,
            history: (*self.history).clone(),
        }
    }
}

/// `SCIPvarGetTransformed`.
pub fn var_get_transformed(scip: &mut Scip, origvar: VarId, transvar: &mut VarId) -> ScipResult {
    if scip.vars[origvar].varstatus == VarStatus::Negated {
        let negorig = scip.vars[origvar].negatedvar;
        let t = scip.vars[negorig].data.original.transvar;
        if t == NONE {
            *transvar = NONE;
        } else {
            var_negate(scip, t, transvar)?;
        }
    } else {
        *transvar = scip.vars[origvar].data.original.transvar;
    }
    Ok(())
}

/// `SCIPvarColumn`: converts a loose variable into a column variable.
pub fn var_column(scip: &mut Scip, var: VarId, prob: usize) -> ScipResult {
    scip.vars[var].varstatus = VarStatus::Column;

    // Create column of variable.
    let removable = scip.vars[var].removable;
    let col = crate::lp::col_create(scip, var, &[], &[], removable)?;
    scip.vars[var].data.col = col;

    if scip.vars[var].probindex != -1 {
        // Inform problem about the variable's status change.
        crate::prob::prob_var_changed_status(scip, prob, var)?;

        // Inform LP, that problem variable is now a column variable and no
        // longer loose.
        crate::lp::lp_update_var_column(scip, var)?;
    }
    Ok(())
}

/// `SCIPvarLoose`: converts a column variable back into a loose variable,
/// freeing its column.
pub fn var_loose(scip: &mut Scip, var: VarId, prob: usize) -> ScipResult {
    // Free the column and set the status to LOOSE.
    let col = scip.vars[var].data.col;
    crate::lp::col_free(scip, col)?;
    scip.vars[var].data.col = NONE;
    scip.vars[var].varstatus = VarStatus::Loose;
    if scip.vars[var].probindex != -1 {
        // Inform problem about the variable's status change.
        crate::prob::prob_var_changed_status(scip, prob, var)?;

        // Inform LP, that problem variable is now a loose variable and no
        // longer a column.
        crate::lp::lp_update_var_loose(scip, var)?;
    }

    // Initialize minimal and maximal aggregation coefficients.
    scip.vars[var].data.loose.minaggrcoef = 1.0;
    scip.vars[var].data.loose.maxaggrcoef = 1.0;
    Ok(())
}

/// `varEventVarFixed`: issues a VARFIXED event on the variable and,
/// depending on how it was fixed (0: fixed, 1: aggregated, 2:
/// multi-aggregated), on its parents.
fn var_event_var_fixed(scip: &mut Scip, var: VarId, fixeventtype: i32) -> ScipResult {
    let event = crate::event::event_create_var_fixed(var);
    crate::event::eventqueue_add(scip, event)?;
    let mut i = scip.vars[var].nparentvars - 1;
    while i >= 0 {
        let parent = scip.vars[var].parentvars[i];
        let varstatus = scip.vars[parent].varstatus;
        match fixeventtype {
            0 | 2 => {
                if varstatus != VarStatus::Original {
                    var_event_var_fixed(scip, parent, fixeventtype)?;
                }
            }
            1 => {
                // An aggregated variable's parents that are aggregated do
                // not change.
                if varstatus != VarStatus::Aggregated && varstatus != VarStatus::Original {
                    var_event_var_fixed(scip, parent, fixeventtype)?;
                }
            }
            _ => {
                log::error!("unknown variable fixation event origin");
                return Err(Retcode::InvalidData);
            }
        }
        i -= 1;
    }
    Ok(())
}

/// `SCIPvarFix`: converts a loose variable into a fixed one, following
/// aggregations and negations to the active variable.
pub fn var_fix(scip: &mut Scip, var: VarId, transprob: usize, origprob: usize, mut fixedval: f64, infeasible: &mut bool, fixed: &mut bool) -> ScipResult {
    *infeasible = false;
    *fixed = false;
    if scip.vars[var].varstatus == VarStatus::Fixed {
        *infeasible = !scip.set.is_feas_eq(fixedval, scip.vars[var].locdom.lb);
        return Ok(());
    } else if (var_is_integral(scip, var) && !scip.set.is_feas_integral(fixedval))
        || scip.set.is_feas_lt(fixedval, scip.vars[var].locdom.lb)
        || scip.set.is_feas_gt(fixedval, scip.vars[var].locdom.ub)
    {
        *infeasible = true;
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == NONE {
                log::error!("cannot fix an untransformed original variable");
                return Err(Retcode::InvalidData);
            }
            var_fix(scip, transvar, transprob, origprob, fixedval, infeasible, fixed)?;
        }
        VarStatus::Loose => {
            // Set the objective value to zero, so the variable's objective
            // contribution can be moved into the problem's offset.
            let obj = scip.vars[var].obj;
            var_chg_obj(scip, var, transprob, 0.0)?;

            // Since we change the variable type form loose to fixed, we
            // have to adjust the number of loose variables in the LP data
            // structure; the loose objective value (looseobjval) in the LP
            // data structure, however, gets adjusted in
            // SCIPvarChgObj(); we also have to adjust the number of loose
            // variables with infinite best bound.
            crate::lp::lp_dec_nloosevars(scip);

            // Change variable's bounds to fixed value (thereby removing
            // redundant implications and variable bounds).
            scip.vars[var].glbdom.holelist = None;
            scip.vars[var].locdom.holelist = None;
            if var_is_integral(scip, var) {
                fixedval = scip.set.round(fixedval);
            }
            var_chg_lb_global(scip, var, fixedval)?;
            var_chg_ub_global(scip, var, fixedval)?;

            // Ensure local domain is fixed to same value as global domain.
            let v = &mut scip.vars[var];
            v.glbdom.lb = fixedval;
            v.glbdom.ub = fixedval;
            v.locdom.lb = fixedval;
            v.locdom.ub = fixedval;

            // Delete implications and variable bounds information.
            var_remove_cliques_implics_vbs(scip, var, false, false, true)?;

            // Clear the history of the variable.
            scip.vars[var].history.reset();
            scip.vars[var].historycrun.reset();

            // Convert variable into fixed variable.
            scip.vars[var].varstatus = VarStatus::Fixed;

            // Inform problem about the variable's status change.
            if scip.vars[var].probindex != -1 {
                crate::prob::prob_var_changed_status(scip, transprob, var)?;
            }

            // Reset the objective value of the fixed variable, thus
            // adjusting the problem's objective offset.
            var_add_obj(scip, var, transprob, origprob, obj)?;

            // Issue VARFIXED event.
            var_event_var_fixed(scip, var, 0)?;
            *fixed = true;
        }
        VarStatus::Column => {
            log::error!("cannot fix a column variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Fixed => {
            log::error!("cannot fix a fixed variable again");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Aggregated => {
            // Fix aggregation variable y in x = a*y + c, instead of fixing
            // x directly.
            let a = scip.vars[var].data.aggregate;
            let childfixedval = (fixedval - a.constant) / a.scalar;
            var_fix(scip, a.var, transprob, origprob, childfixedval, infeasible, fixed)?;
        }
        VarStatus::MultAggr => {
            log::error!("cannot fix a multiple aggregated variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            // Fix negation variable x in x' = offset - x, instead of fixing
            // x' directly.
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_fix(scip, negvar, transprob, origprob, constant - fixedval, infeasible, fixed)?;
        }
    }
    Ok(())
}

/// `SCIPvarGetActiveRepresentatives`: replaces the linear sum
/// `vars * scalars + constant` by one over active variables, merging
/// duplicates. If `varssize` is too small for the result, only
/// `requiredsize` is set.
pub fn var_get_active_representatives(
    scip: &mut Scip,
    vars: &mut CVec<VarId>,
    scalars: &mut CVec<f64>,
    nvars: &mut i32,
    varssize: i32,
    constant: &mut f64,
    requiredsize: &mut i32,
) -> ScipResult {
    *requiredsize = 0;
    if *nvars == 0 {
        return Ok(());
    }

    // Handle the "easy" case of just one variable and avoid memory
    // allocation if the variable is already active.
    if *nvars == 1 && (scip.vars[vars[0]].varstatus == VarStatus::Column || scip.vars[vars[0]].varstatus == VarStatus::Loose) {
        *requiredsize = 1;
        return Ok(());
    }

    // Allocate temporary memory.
    let mut tmpvarssize = *nvars;
    let mut tmpvars: CVec<VarId> = CVec::with_size(tmpvarssize);
    let mut activevarssize = max(10, 2 * *nvars);
    let mut activevars: CVec<VarId> = CVec::with_size(activevarssize);
    let ntotalvars = scip.stat.nvaridx;
    let mut tmpscalars: CVec<f64> = CVec::with_size(ntotalvars);

    // To avoid unnecessary expanding of variable arrays, we first replace
    // all variables by their active representatives.
    let mut activeconstant = 0.0;
    let mut nactivevars = 0;
    let mut ntmpvars = 0;
    for v in 0..*nvars {
        let mut var = vars[v];
        let mut scalar = scalars[v];

        // Transforms given variable, scalar and constant to the
        // corresponding active, fixed, or multi-aggregated variable,
        // scalar and constant; if the variable resolves to a fixed
        // variable, "scalar" will be 0.0 and the value of the sum will be
        // stored in "constant"; a multi-aggregation is treated like an
        // aggregation.
        var_get_probvar_sum(scip, &mut var, &mut scalar, &mut activeconstant)?;
        let varstatus = scip.vars[var].varstatus;
        if scalar != 0.0 {
            let idx = scip.vars[var].index;
            if tmpscalars[idx] == 0.0 {
                if varstatus == VarStatus::Loose || varstatus == VarStatus::Column {
                    activevars[nactivevars] = var;
                    nactivevars += 1;
                } else if varstatus == VarStatus::MultAggr {
                    tmpvars[ntmpvars] = var;
                    ntmpvars += 1;
                }
            }
            tmpscalars[idx] += scalar;
        }
    }
    let mut activeconstantinf = scip.set.is_infinity(activeconstant) || scip.set.is_infinity(-activeconstant);

    // Collect for each variable the representation in active variables.
    while ntmpvars >= 1 {
        ntmpvars -= 1;
        let var = tmpvars[ntmpvars];
        let scalar = tmpscalars[scip.vars[var].index];
        if scalar == 0.0 {
            continue;
        }
        let nmultvars = scip.vars[var].data.multaggr.nvars;
        let multvars = scip.vars[var].data.multaggr.vars.clone();
        let multscalars = scip.vars[var].data.multaggr.scalars.clone();
        tmpscalars[scip.vars[var].index] = 0.0;
        for v in 0..nmultvars {
            let mut multvar = multvars[v];
            let mut multscalar = multscalars[v];
            let mut multconstant = 0.0;
            var_get_probvar_sum(scip, &mut multvar, &mut multscalar, &mut multconstant)?;
            if !activeconstantinf {
                if scip.set.is_infinity(multconstant) || scip.set.is_infinity(-multconstant) {
                    if scalar * multconstant > 0.0 {
                        activeconstant = scip.set.infinity();
                        activeconstantinf = true;
                    } else {
                        activeconstant = -scip.set.infinity();
                        activeconstantinf = true;
                    }
                } else {
                    activeconstant += scalar * multconstant;
                }
            }
            if multscalar == 0.0 {
                continue;
            }
            let idx = scip.vars[multvar].index;
            if tmpscalars[idx] == 0.0 {
                let varstatus = scip.vars[multvar].varstatus;
                if varstatus == VarStatus::Loose || varstatus == VarStatus::Column {
                    if nactivevars >= activevarssize {
                        activevarssize *= 2;
                        activevars.realloc(activevarssize);
                    }
                    activevars[nactivevars] = multvar;
                    nactivevars += 1;
                } else if varstatus == VarStatus::MultAggr {
                    if ntmpvars >= tmpvarssize {
                        tmpvarssize *= 2;
                        tmpvars.realloc(tmpvarssize);
                    }
                    tmpvars[ntmpvars] = multvar;
                    ntmpvars += 1;
                }
            }
            tmpscalars[idx] += scalar * multscalar;
        }
        if !activeconstantinf {
            let multconstant = scip.vars[var].data.multaggr.constant;
            if scip.set.is_infinity(multconstant) || scip.set.is_infinity(-multconstant) {
                if scalar * multconstant > 0.0 {
                    activeconstant = scip.set.infinity();
                    activeconstantinf = true;
                } else {
                    activeconstant = -scip.set.infinity();
                    activeconstantinf = true;
                }
            } else {
                activeconstant += scalar * multconstant;
            }
        }
    }

    // Return results if enough memory was given.
    if varssize >= nactivevars {
        if !scip.set.is_infinity(*constant) && !scip.set.is_infinity(-*constant) {
            if activeconstantinf {
                *constant = activeconstant;
            } else {
                *constant += activeconstant;
            }
        }

        // Copy active variable and scalar array to the given arrays.
        *nvars = 0;
        for v in 0..nactivevars {
            let var = activevars[v];
            let idx = scip.vars[var].index;
            if !scip.set.is_zero(tmpscalars[idx]) {
                vars[*nvars] = var;
                scalars[*nvars] = tmpscalars[idx];
                *nvars += 1;
            }
        }
        *requiredsize = *nvars;
    } else {
        *requiredsize = nactivevars;
    }
    Ok(())
}

/// `SCIPvarFlattenAggregationGraph`: replaces a multi-aggregation by one
/// over active variables.
pub fn var_flatten_aggregation_graph(scip: &mut Scip, var: VarId) -> ScipResult {
    let mut nlocksup = [0; NLOCKTYPES];
    let mut nlocksdown = [0; NLOCKTYPES];

    // Remove the locks of the multi-aggregated variable; they are added
    // back after the representation changed.
    for i in 0..NLOCKTYPES {
        nlocksup[i] = scip.vars[var].nlocksup[i];
        nlocksdown[i] = scip.vars[var].nlocksdown[i];
        var_add_locks(scip, var, lock_type(i), -nlocksdown[i], -nlocksup[i])?;
    }
    let mut multconstant = scip.vars[var].data.multaggr.constant;
    let mut nmultvars = scip.vars[var].data.multaggr.nvars;
    let mut multvarssize = scip.vars[var].data.multaggr.varssize;
    let mut multrequiredsize = 0;
    let mut mvars = std::mem::take(&mut scip.vars[var].data.multaggr.vars);
    let mut mscalars = std::mem::take(&mut scip.vars[var].data.multaggr.scalars);
    var_get_active_representatives(scip, &mut mvars, &mut mscalars, &mut nmultvars, multvarssize, &mut multconstant, &mut multrequiredsize)?;
    if multrequiredsize > multvarssize {
        mvars.realloc(multrequiredsize);
        mscalars.realloc(multrequiredsize);
        multvarssize = multrequiredsize;
        var_get_active_representatives(scip, &mut mvars, &mut mscalars, &mut nmultvars, multvarssize, &mut multconstant, &mut multrequiredsize)?;
    }
    let m = &mut scip.vars[var].data.multaggr;
    m.vars = mvars;
    m.scalars = mscalars;
    m.constant = multconstant;
    m.nvars = nmultvars;
    m.varssize = multvarssize;
    for i in 0..NLOCKTYPES {
        var_add_locks(scip, var, lock_type(i), nlocksdown[i], nlocksup[i])?;
    }
    Ok(())
}

/// The `SCIP_LOCKTYPE` numbered `i`.
fn lock_type(i: usize) -> LockType {
    if i == 0 { LockType::Model } else { LockType::Conflict }
}

/// `SCIPvarMergeHistories`: adds the current-run history of `othervar` to
/// `targetvar`'s history and the global history.
pub fn var_merge_histories(scip: &mut Scip, targetvar: VarId, othervar: VarId) {
    let other = (*scip.vars[othervar].historycrun).clone();
    var_merge_histories_from(scip, targetvar, &other);
}

fn var_merge_histories_from(scip: &mut Scip, targetvar: VarId, othercrun: &History) {
    scip.vars[targetvar].history.unite(othercrun, false);
    scip.stat.glbhistory.unite(othercrun, false);
}

/// `SCIPvarSetHistory`.
pub fn var_set_history(scip: &mut Scip, var: VarId, history: &History) {
    scip.vars[var].history.unite(history, false);
    scip.stat.glbhistory.unite(history, false);
}

/// `varUpdateMinMaxAggrCoef`.
fn var_update_min_max_aggr_coef(scip: &mut Scip, var: VarId, aggvar: VarId, aggscalar: f64) {
    let mut minscalar = aggscalar.abs();
    let mut maxscalar = minscalar;
    minscalar *= scip.vars[aggvar].data.loose.minaggrcoef;
    maxscalar *= scip.vars[aggvar].data.loose.maxaggrcoef;
    if scip.vars[var].data.loose.minaggrcoef > minscalar {
        scip.vars[var].data.loose.minaggrcoef = minscalar;
    }
    if scip.vars[var].data.loose.maxaggrcoef < maxscalar {
        scip.vars[var].data.loose.maxaggrcoef = maxscalar;
    }
}

/// `varUpdateAggregationBounds`: tightens the global bounds of `var` and
/// `aggvar` against each other for the aggregation
/// `var = scalar * aggvar + constant`, until neither changes.
fn var_update_aggregation_bounds(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    aggvar: VarId,
    scalar: f64,
    constant: f64,
    infeasible: &mut bool,
    fixed: &mut bool,
) -> ScipResult {
    *infeasible = false;
    *fixed = false;

    // Tighten the bounds of aggregated variable var and aggregation
    // variable aggvar.
    loop {
        let mut aggvarbdschanged = false;
        let (alb, aub) = (scip.vars[aggvar].glbdom.lb, scip.vars[aggvar].glbdom.ub);
        let (mut varlb, mut varub);

        // Update the bounds of the aggregated variable x in x = a*y + c.
        if scalar > 0.0 {
            varlb = if scip.set.is_infinity(-alb) { -scip.set.infinity() } else { alb * scalar + constant };
            varub = if scip.set.is_infinity(aub) { scip.set.infinity() } else { aub * scalar + constant };
        } else {
            varub = if scip.set.is_infinity(-alb) { scip.set.infinity() } else { alb * scalar + constant };
            varlb = if scip.set.is_infinity(aub) { -scip.set.infinity() } else { aub * scalar + constant };
        }
        varlb = max(varlb, scip.vars[var].glbdom.lb);
        varub = min(varub, scip.vars[var].glbdom.ub);
        var_adjust_lb(scip, var, &mut varlb);
        var_adjust_ub(scip, var, &mut varub);

        // Check the new bounds.
        if scip.set.is_gt(varlb, varub) {
            // The aggregation is infeasible.
            *infeasible = true;
            return Ok(());
        } else if scip.set.is_eq(varlb, varub) {
            // The aggregated variable is fixed -> fix both variables.
            var_fix(scip, var, transprob, origprob, varlb, infeasible, fixed)?;
            if !*infeasible {
                let mut aggfixed = false;
                var_fix(scip, aggvar, transprob, origprob, (varlb - constant) / scalar, infeasible, &mut aggfixed)?;
            }
            return Ok(());
        } else {
            if scip.set.is_gt(varlb, scip.vars[var].glbdom.lb) {
                var_chg_lb_global(scip, var, varlb)?;
            }
            if scip.set.is_lt(varub, scip.vars[var].glbdom.ub) {
                var_chg_ub_global(scip, var, varub)?;
            }
            // Update the hole list of the aggregation variable: holes are
            // not ported.
        }

        // Update the bounds of the aggregation variable y in x = a*y + c
        // -> y = (x-c)/a.
        let (vlb, vub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
        let (mut aggvarlb, mut aggvarub);
        if scalar > 0.0 {
            aggvarlb = if scip.set.is_infinity(-vlb) { -scip.set.infinity() } else { (vlb - constant) / scalar };
            aggvarub = if scip.set.is_infinity(vub) { scip.set.infinity() } else { (vub - constant) / scalar };
        } else {
            aggvarub = if scip.set.is_infinity(-vlb) { scip.set.infinity() } else { (vlb - constant) / scalar };
            aggvarlb = if scip.set.is_infinity(vub) { -scip.set.infinity() } else { (vub - constant) / scalar };
        }
        aggvarlb = max(aggvarlb, scip.vars[aggvar].glbdom.lb);
        aggvarub = min(aggvarub, scip.vars[aggvar].glbdom.ub);
        var_adjust_lb(scip, aggvar, &mut aggvarlb);
        var_adjust_ub(scip, aggvar, &mut aggvarub);

        // Check the new bounds.
        if scip.set.is_gt(aggvarlb, aggvarub) {
            *infeasible = true;
            return Ok(());
        } else if scip.set.is_eq(aggvarlb, aggvarub) {
            // The aggregation variable is fixed -> fix both variables.
            var_fix(scip, aggvar, transprob, origprob, aggvarlb, infeasible, fixed)?;
            if !*infeasible {
                let mut varfixed = false;
                var_fix(scip, var, transprob, origprob, aggvarlb * scalar + constant, infeasible, &mut varfixed)?;
            }
            return Ok(());
        } else {
            if scip.set.is_gt(aggvarlb, scip.vars[aggvar].glbdom.lb) {
                let oldbd = scip.vars[aggvar].glbdom.lb;
                var_chg_lb_global(scip, aggvar, aggvarlb)?;
                aggvarbdschanged = !scip.set.is_eq(oldbd, scip.vars[aggvar].glbdom.lb);
            }
            if scip.set.is_lt(aggvarub, scip.vars[aggvar].glbdom.ub) {
                let oldbd = scip.vars[aggvar].glbdom.ub;
                var_chg_ub_global(scip, aggvar, aggvarub)?;
                aggvarbdschanged = aggvarbdschanged || !scip.set.is_eq(oldbd, scip.vars[aggvar].glbdom.ub);
            }
        }
        if !aggvarbdschanged {
            break;
        }
    }
    Ok(())
}

/// `SCIPvarAggregate`: converts the loose variable `var` into the
/// aggregation `var = scalar * aggvar + constant` (or a negation, for
/// binaries with x = 1 - y).
pub fn var_aggregate(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    mut aggvar: VarId,
    mut scalar: f64,
    mut constant: f64,
    infeasible: &mut bool,
    aggregated: &mut bool,
) -> ScipResult {
    *infeasible = false;
    *aggregated = false;

    // Get active problem variable of aggregation variable.
    var_get_probvar_sum(scip, &mut aggvar, &mut scalar, &mut constant)?;

    // Aggregation is a fixing, if the scalar is zero.
    if scip.set.is_zero(scalar) {
        var_fix(scip, var, transprob, origprob, constant, infeasible, aggregated)?;
        return Ok(());
    }

    // Don't perform the aggregation if the aggregation variable is
    // multi-aggregated itself.
    if scip.vars[aggvar].varstatus == VarStatus::MultAggr {
        return Ok(());
    }

    // If the variable is not allowed to be aggregated.
    if var_do_not_aggr(scip, var) {
        return Ok(());
    }

    // Aggregation is a fixing, if the scalar is zero.
    if var == aggvar {
        if scip.set.is_eq(scalar, 1.0) {
            *infeasible = !scip.set.is_zero(constant);
        } else {
            var_fix(scip, var, transprob, origprob, constant / (1.0 - scalar), infeasible, aggregated)?;
        }
        return Ok(());
    }

    // Tighten the bounds of aggregated and aggregation variable.
    let mut fixed = false;
    var_update_aggregation_bounds(scip, var, transprob, origprob, aggvar, scalar, constant, infeasible, &mut fixed)?;
    if *infeasible || fixed {
        *aggregated = fixed;
        return Ok(());
    }

    // Delete implications and variable bounds of the aggregated variable
    // from other variables, but keep them in the aggregated variable for
    // later usage.
    var_remove_cliques_implics_vbs(scip, var, false, false, false)?;

    // Set the aggregated variable's objective value to 0.0.
    let obj = scip.vars[var].obj;
    var_chg_obj(scip, var, transprob, 0.0)?;

    // Unlock all locks.
    let mut nlocksdown = [0; NLOCKTYPES];
    let mut nlocksup = [0; NLOCKTYPES];
    for i in 0..NLOCKTYPES {
        nlocksdown[i] = scip.vars[var].nlocksdown[i];
        nlocksup[i] = scip.vars[var].nlocksup[i];
        scip.vars[var].nlocksdown[i] = 0;
        scip.vars[var].nlocksup[i] = 0;
    }

    // Check, if variable should be used as NEGATED variable of the
    // aggregation variable.
    var_update_min_max_aggr_coef(scip, aggvar, var, scalar);
    if var_is_binary(scip, var)
        && var_is_binary(scip, aggvar)
        && scip.vars[var].negatedvar == NONE
        && scip.vars[aggvar].negatedvar == NONE
        && scip.set.is_eq(scalar, -1.0)
        && scip.set.is_eq(constant, 1.0)
    {
        // Set the aggregated variable's objective value to 0.0.
        scip.vars[var].varstatus = VarStatus::Negated;
        scip.vars[var].data.negate.constant = 1.0;
        scip.vars[var].negatedvar = aggvar;
        scip.vars[aggvar].negatedvar = var;

        // Copy donot(mult)aggr status.
        let (da, dm) = (scip.vars[var].donotaggr, scip.vars[var].donotmultaggr);
        scip.vars[aggvar].donotaggr |= da;
        scip.vars[aggvar].donotmultaggr |= dm;

        // Mark both variables to be non-deletable.
        var_mark_not_deletable(scip, var);
        var_mark_not_deletable(scip, aggvar);
    } else {
        // Convert variable into aggregated variable.
        scip.vars[var].varstatus = VarStatus::Aggregated;
        scip.vars[var].data.aggregate.var = aggvar;
        scip.vars[var].data.aggregate.scalar = scalar;
        scip.vars[var].data.aggregate.constant = constant;

        // Copy donot(mult)aggr status.
        let (da, dm) = (scip.vars[var].donotaggr, scip.vars[var].donotmultaggr);
        scip.vars[aggvar].donotaggr |= da;
        scip.vars[aggvar].donotmultaggr |= dm;

        // Mark both variables to be non-deletable.
        var_mark_not_deletable(scip, var);
        var_mark_not_deletable(scip, aggvar);
    }

    // Make aggregated variable a parent of the aggregation variable.
    var_add_parent(scip, aggvar, var);

    // Relock the variable, thus increasing the locks of the aggregation
    // variable.
    for i in 0..NLOCKTYPES {
        var_add_locks(scip, var, lock_type(i), nlocksdown[i], nlocksup[i])?;
    }

    // Move the variable bounds to the aggregation variable:
    //  - add all variable bounds again to the variable, thus adding it to
    //    the aggregation variable
    //  - free the variable bounds data structures
    if let Some(vlbs) = scip.vars[var].vlbs.clone() {
        let mut i = 0;
        while i < vlbs.len && !*infeasible {
            var_add_vlb(scip, var, transprob, origprob, vlbs.vars[i], vlbs.coefs[i], vlbs.constants[i], false, infeasible, None)?;
            i += 1;
        }
    }
    if let Some(vubs) = scip.vars[var].vubs.clone() {
        let mut i = 0;
        while i < vubs.len && !*infeasible {
            var_add_vub(scip, var, transprob, origprob, vubs.vars[i], vubs.coefs[i], vubs.constants[i], false, infeasible, None)?;
            i += 1;
        }
    }
    scip.vars[var].vlbs = None;
    scip.vars[var].vubs = None;

    // Move the implications to the aggregation variable:
    //  - add all implications again to the variable, thus adding it to the
    //    aggregation variable
    //  - free the implications data structures
    if scip.vars[var].implics.is_some() && scip.vars[aggvar].vartype == VarType::Binary && !var_is_implied_integral(scip, aggvar) {
        let implics = scip.vars[var].implics.clone().unwrap();
        for i in 0..2 {
            let mut j = 0;
            while j < implics.nimpls[i] && !*infeasible {
                var_add_implic(
                    scip,
                    var,
                    transprob,
                    origprob,
                    i == 1,
                    implics.vars[i][j],
                    implics.types[i][j],
                    implics.bounds[i][j],
                    false,
                    infeasible,
                    None,
                )?;
                j += 1;
            }
        }
    }
    scip.vars[var].implics = None;

    // Add the history entries to the aggregation variable and clear the
    // history of the aggregated variable.
    let (h, hc) = ((*scip.vars[var].history).clone(), (*scip.vars[var].historycrun).clone());
    scip.vars[aggvar].history.unite(&h, scalar < 0.0);
    scip.vars[aggvar].historycrun.unite(&hc, scalar < 0.0);
    scip.vars[var].history.reset();
    scip.vars[var].historycrun.reset();

    // Update flags of aggregation variable.
    let removable = scip.vars[var].removable;
    scip.vars[aggvar].removable &= removable;

    // Update branching factors and priorities of both variables to be the
    // maximum of both variables.
    let branchfactor = max(scip.vars[aggvar].branchfactor, scip.vars[var].branchfactor);
    let branchpriority = max(scip.vars[aggvar].branchpriority, scip.vars[var].branchpriority);
    var_chg_branch_factor(scip, aggvar, branchfactor)?;
    var_chg_branch_priority(scip, aggvar, branchpriority)?;
    var_chg_branch_factor(scip, var, branchfactor)?;
    var_chg_branch_priority(scip, var, branchpriority)?;

    // Update branching direction of both variables to agree to a single
    // direction.
    let (vdir, adir) = (scip.vars[var].branchdirection, scip.vars[aggvar].branchdirection);
    if scalar >= 0.0 {
        if vdir == BranchDir::Auto {
            var_chg_branch_direction(scip, var, adir)?;
        } else if adir == BranchDir::Auto {
            var_chg_branch_direction(scip, aggvar, vdir)?;
        } else if vdir != adir {
            var_chg_branch_direction(scip, var, BranchDir::Auto)?;
        }
    } else if vdir == BranchDir::Auto {
        var_chg_branch_direction(scip, var, branchdir_opposite(adir))?;
    } else if adir == BranchDir::Auto {
        var_chg_branch_direction(scip, aggvar, branchdir_opposite(vdir))?;
    } else if vdir != adir {
        var_chg_branch_direction(scip, var, BranchDir::Auto)?;
    }
    if scip.vars[var].probindex != -1 {
        // Inform problem about the variable's status change.
        crate::prob::prob_var_changed_status(scip, transprob, var)?;
    }

    // Reset the objective value of the aggregated variable, thus adjusting
    // the objective value of the aggregation variable and the problem
    // offset.
    var_add_obj(scip, var, transprob, origprob, obj)?;

    // Issue VARFIXED event.
    var_event_var_fixed(scip, var, 1)?;
    *aggregated = true;
    Ok(())
}

/// `tryAggregateIntVars`: aggregates `a*x + b*y == c` over integers
/// through a new integer variable when neither coefficient is a unit.
fn try_aggregate_int_vars(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    varx: VarId,
    vary: VarId,
    scalarx: f64,
    scalary: f64,
    mut rhs: f64,
    infeasible: &mut bool,
    aggregated: &mut bool,
) -> ScipResult {
    const MAXDNOM: i64 = 1000000;
    *infeasible = false;
    *aggregated = false;

    // If the variable is not allowed to be aggregated.
    if var_do_not_aggr(scip, varx) {
        return Ok(());
    }

    // Get rational representation of coefficients.
    let (mut scalarxn, mut scalarxd, mut scalaryn, mut scalaryd) = (0i64, 0i64, 0i64, 0i64);
    let mut success = crate::misc::real_to_rational(scalarx, -scip.set.epsilon(), scip.set.epsilon(), MAXDNOM, &mut scalarxn, &mut scalarxd);
    if success {
        success = crate::misc::real_to_rational(scalary, -scip.set.epsilon(), scip.set.epsilon(), MAXDNOM, &mut scalaryn, &mut scalaryd);
    }
    if !success {
        return Ok(());
    }

    // Multiply equality by smallest common denominator.
    let scm = crate::misc::calc_sma_com_mul(scalarxd, scalaryd);
    let mut a = (scm / scalarxd) * scalarxn;
    let mut b = (scm / scalaryd) * scalaryn;
    rhs *= scm as f64;

    // Divide equality by the greatest common divisor of a and b.
    let gcd = crate::misc::calc_gre_com_div(a.abs(), b.abs());
    a /= gcd;
    b /= gcd;
    rhs /= gcd as f64;

    // Check, if right hand side is integral.
    if !scip.set.is_feas_integral(rhs) {
        *infeasible = true;
        return Ok(());
    }
    let mut c = crate::misc::c_f64_to_i64(scip.set.feas_floor(rhs));

    // Check that the scalar and constant in the aggregation are not too
    // large to avoid numerical problems.
    let huge = scip.set.get_huge_value() * scip.set.feastol();
    if ((c / a) as f64).abs() > huge || (b as f64).abs() > huge || (a as f64).abs() > huge {
        return Ok(());
    }

    // Check, if we are in an easy case with either |a| = 1 or |b| = 1.
    if (a == 1 || a == -1) && !var_is_implied_integral(scip, vary) {
        // Aggregate x = - b/a*y + c/a.
        var_aggregate(scip, varx, transprob, origprob, vary, (-b / a) as f64, (c / a) as f64, infeasible, aggregated)?;
        return Ok(());
    }
    if (b == 1 || b == -1) && !var_is_implied_integral(scip, varx) {
        // Aggregate y = - a/b*x + c/b.
        var_aggregate(scip, vary, transprob, origprob, varx, (-a / b) as f64, (c / b) as f64, infeasible, aggregated)?;
        return Ok(());
    }

    // Both variables are integers, their coefficients are not multiples of
    // each other, and they don't have any common divisor. Let (x',y') be a
    // solution of a*x + b*y == c, then all solutions are
    // x = x' - b*z and y = y' + a*z with z integral.
    if a < 0 {
        a = -a;
        b = -b;
        c = -c;
    }

    // Search a solution to the equation.
    let mut ysol: i64 = 0;
    let mut currentclass = c % a;
    if currentclass < 0 {
        currentclass += a;
    }
    let mut classstep = (-b) % a;
    if classstep < 0 {
        classstep += a;
    }
    while currentclass != 0 {
        currentclass += classstep;
        if currentclass >= a {
            currentclass -= a;
        }
        ysol += 1;
    }
    let xsol = (c - b * ysol) / a;

    // Create new variable for the aggregation.
    let vartype = if scip.vars[varx].vartype == VarType::Continuous && scip.vars[vary].vartype == VarType::Continuous {
        VarType::Continuous
    } else {
        VarType::Integer
    };
    let impltype = min(scip.vars[varx].varimpltype, scip.vars[vary].varimpltype);
    let aggvarname = format!("agg{}", scip.stat.nvaridx);
    let initial = scip.vars[varx].initial || scip.vars[vary].initial;
    let removable = scip.vars[varx].removable && scip.vars[vary].removable;
    let inf = scip.set.infinity();
    let mut aggvar = var_create_transformed(scip, Some(&aggvarname), -inf, inf, 0.0, vartype, impltype, initial, removable)?;
    crate::prob::prob_add_var(scip, transprob, aggvar)?;

    // Aggregate x = x' - b*z.
    var_aggregate(scip, varx, transprob, origprob, aggvar, (-b) as f64, xsol as f64, infeasible, aggregated)?;

    // Aggregate y = y' + a*z.
    if !*infeasible {
        var_aggregate(scip, vary, transprob, origprob, aggvar, a as f64, ysol as f64, infeasible, aggregated)?;
    }

    // Release z.
    var_release(scip, &mut aggvar)?;
    Ok(())
}

/// The type `SCIPvarTryAggregateVars` ranks a variable by: its
/// `SCIP_VARTYPE`, implied integers counting as `IMPLINT`.
fn aggr_type(scip: &Scip, var: VarId) -> VarType {
    if var_is_implied_integral(scip, var) { VarType::ImplInt } else { scip.vars[var].vartype }
}

/// `SCIPvarTryAggregateVars`: aggregates `scalarx*x + scalary*y == rhs`
/// in the variable that keeps it integral, if any.
pub fn var_try_aggregate_vars(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    mut varx: VarId,
    mut vary: VarId,
    mut scalarx: f64,
    mut scalary: f64,
    rhs: f64,
    infeasible: &mut bool,
    aggregated: &mut bool,
) -> ScipResult {
    *infeasible = false;
    *aggregated = false;

    // Prefer aggregating the variable of more general type (preferred
    // aggregation variable is varx).
    let mut typex = aggr_type(scip, varx);
    let mut typey = aggr_type(scip, vary);
    if typex < typey || (typex == typey && var_is_binary(scip, varx) && !var_is_binary(scip, vary)) {
        std::mem::swap(&mut varx, &mut vary);
        std::mem::swap(&mut scalarx, &mut scalary);
        std::mem::swap(&mut typex, &mut typey);
    }

    // Don't aggregate if the aggregation would lead to a binary variable
    // aggregated to a non-binary variable.
    if var_is_binary(scip, varx) && !var_is_binary(scip, vary) {
        return Ok(());
    }

    // Figure out, which variable should be aggregated.
    let mut easyaggr = false;

    // If varx is continuous and vary is integral, the aggregation is easy.
    if typex == VarType::Continuous && typey != VarType::Continuous {
        easyaggr = true;
    } else if scip.set.is_feas_integral(scalary / scalarx) {
        // If the aggregation scalar for varx is integral, the aggregation
        // is easy.
        easyaggr = true;
    } else if typex == typey && scip.set.is_feas_integral(scalarx / scalary) {
        // If the aggregation scalar for vary is integral, the aggregation is
        // easy.
        std::mem::swap(&mut varx, &mut vary);
        std::mem::swap(&mut scalarx, &mut scalary);
        easyaggr = true;
    } else if typex == VarType::Continuous {
        // The aggregation is easy, if both variables are continuous.
        easyaggr = true;
    }

    // Calculate aggregation scalar and constant: a*x + b*y == c  =>
    // x == -b/a * y + c/a.
    let scalar = -scalary / scalarx;
    let constant = rhs / scalarx;
    if !var_is_aggr_coef_acceptable(scip, varx, scalar) {
        return Ok(());
    }

    // Did we find an "easy" aggregation?
    if easyaggr {
        if constant.abs() > scip.set.get_huge_value() * scip.set.feastol() {
            return Ok(());
        }

        // Check aggregation for integer feasibility.
        if typex != VarType::Continuous && typey != VarType::Continuous && scip.set.is_feas_integral(scalar) && !scip.set.is_feas_integral(constant) {
            *infeasible = true;
            return Ok(());
        }

        // If the aggregation scalar is fractional, we cannot (for
        // technical reasons) and do not want to aggregate implicit integer
        // variables, since we loose the "implicitness" of the aggregated
        // variable.
        var_aggregate(scip, varx, transprob, origprob, vary, scalar, constant, infeasible, aggregated)?;
    } else if (typex == VarType::Integer || typex == VarType::ImplInt) && (typey == VarType::Integer || typey == VarType::ImplInt) {
        // The variables are both integral: we have to try to find an
        // integer aggregation.
        try_aggregate_int_vars(scip, transprob, origprob, varx, vary, scalarx, scalary, rhs, infeasible, aggregated)?;
    }
    Ok(())
}

/// `SCIPvarMultiaggregate`: converts the loose variable `var` into
/// `var = sum scalars * aggvars + constant`, or a simpler fixing or
/// aggregation if the active representation allows.
pub fn var_multiaggregate(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    naggvars: i32,
    aggvars: &CVec<VarId>,
    scalars: &mut CVec<f64>,
    constant: f64,
    infeasible: &mut bool,
    aggregated: &mut bool,
) -> ScipResult {
    *infeasible = false;
    *aggregated = false;
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == NONE {
                log::error!("cannot multi-aggregate an untransformed original variable");
                return Err(Retcode::InvalidData);
            }
            var_multiaggregate(scip, transvar, transprob, origprob, naggvars, aggvars, scalars, constant, infeasible, aggregated)?;
        }
        VarStatus::Loose => 'terminate: {
            // Check if we would create a self-reference.
            let mut ntmpvars = naggvars;
            let mut tmpvarssize = naggvars;
            let mut tmpconstant = constant;
            let mut tmpvars: CVec<VarId> = CVec(aggvars.head(ntmpvars).to_vec());
            let mut tmpscalars: CVec<f64> = CVec(scalars.head(ntmpvars).to_vec());
            let mut tmprequiredsize = 0;

            // Get all active variables for multi-aggregation.
            var_get_active_representatives(scip, &mut tmpvars, &mut tmpscalars, &mut ntmpvars, tmpvarssize, &mut tmpconstant, &mut tmprequiredsize)?;
            if tmprequiredsize > tmpvarssize {
                tmpvars.realloc(tmprequiredsize);
                tmpscalars.realloc(tmprequiredsize);
                tmpvarssize = tmprequiredsize;
                var_get_active_representatives(scip, &mut tmpvars, &mut tmpscalars, &mut ntmpvars, tmpvarssize, &mut tmpconstant, &mut tmprequiredsize)?;
            }

            // Look for self-references.
            let mut tmpscalar = 0.0;
            let mut v = ntmpvars - 1;
            while v >= 0 {
                if scip.vars[tmpvars[v]].index == scip.vars[var].index {
                    tmpscalar += tmpscalars[v];
                    tmpvars[v] = tmpvars[ntmpvars - 1];
                    tmpscalars[v] = tmpscalars[ntmpvars - 1];
                    ntmpvars -= 1;
                }
                v -= 1;
            }

            // This means that x = x + a_1*y_1 + ... + a_n*y_n + c.
            if scip.set.is_eq(tmpscalar, 1.0) {
                if ntmpvars == 0 {
                    // Check if c = 0.
                    if !scip.set.is_feas_zero(tmpconstant) {
                        *infeasible = true;
                    }
                } else if ntmpvars == 1 {
                    // Fix y_1 = -c/a_1.
                    var_fix(scip, tmpvars[0], transprob, origprob, -constant / tmpscalars[0], infeasible, aggregated)?;
                } else if ntmpvars == 2 {
                    // Aggregate a_1*y_1 + a_2*y_2 = -c.
                    var_try_aggregate_vars(
                        scip,
                        transprob,
                        origprob,
                        tmpvars[0],
                        tmpvars[1],
                        tmpscalars[0],
                        tmpscalars[1],
                        -tmpconstant,
                        infeasible,
                        aggregated,
                    )?;
                }
                // Otherwise we cannot multi-aggregate.
                break 'terminate;
            } else if tmpscalar != 0.0 {
                // This means that x = b*x + a_1*y_1 + ... + a_n*y_n + c,
                // so x = a_1/(1-b)*y_1 + ... + a_n/(1-b)*y_n + c/(1-b).
                tmpscalar = 1.0 - tmpscalar;
                tmpconstant /= tmpscalar;
                for v in 0..ntmpvars {
                    tmpscalars[v] /= tmpscalar;
                }
            }

            // Check, if we are in one of the simple cases.
            if ntmpvars == 0 {
                var_fix(scip, var, transprob, origprob, tmpconstant, infeasible, aggregated)?;
                break 'terminate;
            }

            // If only one aggregation variable is left, we perform a
            // normal aggregation instead of a multi-aggregation.
            if ntmpvars == 1 {
                var_try_aggregate_vars(
                    scip,
                    transprob,
                    origprob,
                    var,
                    tmpvars[0_usize],
                    1.0,
                    -tmpscalars[0_usize],
                    tmpconstant,
                    infeasible,
                    aggregated,
                )?;
                break 'terminate;
            }

            // If the variable is not allowed to be multi-aggregated.
            if var_do_not_multaggr(scip, var) {
                break 'terminate;
            }

            // If the variable to be multi-aggregated has not the same type
            // and coefficients are not acceptable, we do not
            // multi-aggregate.
            for v in 0..ntmpvars {
                if !var_is_aggr_coef_acceptable(scip, var, tmpscalars[v]) {
                    break 'terminate;
                }
            }

            // Delete implications and variable bounds information (has to
            // be done before multi-aggregation, because the vars array is
            // used).
            var_remove_cliques_implics_vbs(scip, var, false, false, true)?;

            // Set the aggregated variable's objective value to 0.0.
            let obj = scip.vars[var].obj;
            var_chg_obj(scip, var, transprob, 0.0)?;

            // Since we change the variable type form loose to multaggr, we
            // have to adjust the number of loose variables in the LP data
            // structure.
            crate::lp::lp_dec_nloosevars(scip);

            // Unlock all rounding locks.
            let mut nlocksdown = [0; NLOCKTYPES];
            let mut nlocksup = [0; NLOCKTYPES];
            for i in 0..NLOCKTYPES {
                nlocksdown[i] = scip.vars[var].nlocksdown[i];
                nlocksup[i] = scip.vars[var].nlocksup[i];
                scip.vars[var].nlocksdown[i] = 0;
                scip.vars[var].nlocksup[i] = 0;
            }

            // Convert variable into multi-aggregated variable.
            for v in 0..ntmpvars {
                var_update_min_max_aggr_coef(scip, tmpvars[v], var, tmpscalars[v]);
            }
            let m = &mut scip.vars[var];
            m.varstatus = VarStatus::MultAggr;
            m.data.multaggr.vars = CVec(tmpvars.head(ntmpvars).to_vec());
            m.data.multaggr.scalars = CVec(tmpscalars.head(ntmpvars).to_vec());
            m.data.multaggr.constant = tmpconstant;
            m.data.multaggr.nvars = ntmpvars;
            m.data.multaggr.varssize = ntmpvars;

            // Mark variable to be non-deletable.
            var_mark_not_deletable(scip, var);

            // Relock the variable, thus increasing the locks of the
            // aggregation variables.
            for i in 0..NLOCKTYPES {
                var_add_locks(scip, var, lock_type(i), nlocksdown[i], nlocksup[i])?;
            }

            // Update flags and branching factors and priorities of
            // aggregation variables; update preferred branching direction
            // of all aggregation variables that don't have a preferred
            // direction yet.
            let mut branchfactor = scip.vars[var].branchfactor;
            let mut branchpriority = scip.vars[var].branchpriority;
            let branchdirection = scip.vars[var].branchdirection;
            for v in 0..ntmpvars {
                let removable = scip.vars[var].removable;
                scip.vars[tmpvars[v]].removable &= removable;
                branchfactor = max(scip.vars[tmpvars[v]].branchfactor, branchfactor);
                branchpriority = max(scip.vars[tmpvars[v]].branchpriority, branchpriority);

                // Mark variable to be non-deletable.
                var_mark_not_deletable(scip, tmpvars[v]);
            }
            for v in 0..ntmpvars {
                var_chg_branch_factor(scip, tmpvars[v], branchfactor)?;
                var_chg_branch_priority(scip, tmpvars[v], branchpriority)?;
                if scip.vars[tmpvars[v]].branchdirection == BranchDir::Auto {
                    if tmpscalars[v] >= 0.0 {
                        var_chg_branch_direction(scip, tmpvars[v], branchdirection)?;
                    } else {
                        var_chg_branch_direction(scip, tmpvars[v], branchdir_opposite(branchdirection))?;
                    }
                }
            }
            var_chg_branch_factor(scip, var, branchfactor)?;
            var_chg_branch_priority(scip, var, branchpriority)?;
            if scip.vars[var].probindex != -1 {
                // Inform problem about the variable's status change.
                crate::prob::prob_var_changed_status(scip, transprob, var)?;
            }

            // Issue VARFIXED event.
            var_event_var_fixed(scip, var, 2)?;

            // Reset the objective value of the aggregated variable, thus
            // adjusting the objective value of the aggregation variables
            // and the problem offset.
            var_add_obj(scip, var, transprob, origprob, obj)?;
            *aggregated = true;
        }
        VarStatus::Column => {
            log::error!("cannot multi-aggregate a column variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Fixed => {
            log::error!("cannot multi-aggregate a fixed variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Aggregated => {
            log::error!("cannot multi-aggregate an aggregated variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::MultAggr => {
            log::error!("cannot multi-aggregate a multiple aggregated variable again");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            // Aggregate negation variable x in x' = offset - x, instead of
            // aggregating x' directly: x' = a_1*y_1 + ... + a_n*y_n + c ->
            // x = offset - x' = offset - a_1*y_1 - ... - a_n*y_n - c.
            for v in 0..naggvars {
                scalars[v] *= -1.0;
            }
            let (negvar, negconst) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_multiaggregate(scip, negvar, transprob, origprob, naggvars, aggvars, scalars, negconst - constant, infeasible, aggregated)?;
            for v in 0..naggvars {
                scalars[v] *= -1.0;
            }
        }
    }
    Ok(())
}

/// `varGetActiveVar`: follows aggregations, negations and single-variable
/// multi-aggregations to the active variable.
fn var_get_active_var(scip: &Scip, var: VarId) -> VarId {
    let mut retvar = var;
    loop {
        let v = &scip.vars[retvar];
        match v.varstatus {
            VarStatus::Original | VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => return retvar,
            VarStatus::MultAggr => {
                // Handle multi-aggregated variables depending on one
                // variable only (possibly caused by SCIPvarFlattenAggregationGraph()).
                if v.data.multaggr.nvars == 1 {
                    retvar = v.data.multaggr.vars[0];
                } else {
                    return retvar;
                }
            }
            VarStatus::Aggregated => retvar = v.data.aggregate.var,
            VarStatus::Negated => retvar = v.negatedvar,
        }
    }
}

/// `SCIPvarDoNotAggr`.
pub fn var_do_not_aggr(scip: &Scip, var: VarId) -> bool {
    let retvar = var_get_active_var(scip, var);
    match scip.vars[retvar].varstatus {
        VarStatus::Original | VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => scip.vars[retvar].donotaggr,
        VarStatus::MultAggr => false,
        _ => panic!("wrong variable status"),
    }
}

/// `SCIPvarDoNotMultaggr`.
pub fn var_do_not_multaggr(scip: &Scip, var: VarId) -> bool {
    let retvar = var_get_active_var(scip, var);
    match scip.vars[retvar].varstatus {
        VarStatus::Original | VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => scip.vars[retvar].donotmultaggr,
        VarStatus::MultAggr => false,
        _ => panic!("wrong variable status"),
    }
}

/// `SCIPvarIsAggrCoefAcceptable`: whether aggregating with `scalar` keeps
/// the coefficients the variable appears with in a sane range.
pub fn var_is_aggr_coef_acceptable(scip: &Scip, var: VarId, scalar: f64) -> bool {
    // If the scalar is too small, then the aggregation would be too weak.
    if scip.set.is_sum_zero(var_get_min_aggr_coef(scip, var) * scalar) {
        return false;
    }
    // If the scalar is too large, then the aggregation coefficients could
    // be too small.
    if scip.set.is_sum_zero(1.0 / (var_get_max_aggr_coef(scip, var) * scalar)) {
        return false;
    }
    true
}

/// `SCIPvarNegate`: the negated variable x' = lb + ub - x (1 - x for
/// binaries), created once and kept.
pub fn var_negate(scip: &mut Scip, var: VarId, negvar: &mut VarId) -> ScipResult {
    // Check, if we already created the negated variable.
    if scip.vars[var].negatedvar == NONE {
        let v = &scip.vars[var];
        if scip.set.is_infinity(-v.glbdom.lb) || scip.set.is_infinity(v.glbdom.ub) {
            log::error!("cannot negate unbounded variable");
            return Err(Retcode::InvalidData);
        }
        let negvarname = format!("{}_neg", v.name);
        let (lb, ub, vartype, impltype, initial, removable) = (v.glbdom.lb, v.glbdom.ub, v.vartype, v.varimpltype, v.initial, v.removable);

        // Create negated variable.
        let nv = var_create(scip, Some(&negvarname), lb, ub, 0.0, vartype, impltype, initial, removable)?;
        scip.vars[nv].varstatus = VarStatus::Negated;
        scip.vars[nv].data.negate.constant = if var_is_binary(scip, var) {
            1.0
        } else {
            scip.vars[var].glbdom.lb + scip.vars[var].glbdom.ub
        };

        // Create event filter for transformed variable.
        if var_is_transformed(scip, var) {
            scip.vars[nv].eventfilter = crate::event::eventfilter_create(scip);
        }

        // Set the bounds corresponding to the negation variable.
        let o = scip.vars[var].clone_settings();
        let (oglb, ogub, olb, oub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub, scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);
        let n = &mut scip.vars[nv];
        let c = n.data.negate.constant;
        n.glbdom.lb = c - ogub;
        n.glbdom.ub = c - oglb;
        n.locdom.lb = c - oub;
        n.locdom.ub = c - olb;

        // Link the variables together.
        scip.vars[var].negatedvar = nv;
        scip.vars[nv].negatedvar = var;

        // Mark both variables to be non-deletable.
        var_mark_not_deletable(scip, var);
        var_mark_not_deletable(scip, nv);

        // Copy the branch factor and priority, and use the negative
        // preferred branching direction.
        let n = &mut scip.vars[nv];
        n.branchfactor = o.branchfactor;
        n.branchpriority = o.branchpriority;
        n.branchdirection = branchdir_opposite(o.branchdirection);

        // Copy donot(mult)aggr status.
        n.donotaggr = o.donotaggr;
        n.donotmultaggr = o.donotmultaggr;

        // Copy lazy bounds (they have to be flipped).
        n.lazylb = c - o.lazyub;
        n.lazyub = c - o.lazylb;

        // Make negated variable a parent of the negation variable (negated
        // variable is captured as a parent).
        var_add_parent(scip, var, nv);
    }
    *negvar = scip.vars[var].negatedvar;
    Ok(())
}

/// `SCIPvarSetProbindex`: `varSetProbindex`, keeping the column's copy.
pub fn var_set_probindex(scip: &mut Scip, var: VarId, probindex: i32) {
    scip.vars[var].probindex = probindex;
    if scip.vars[var].varstatus == VarStatus::Column {
        let col = scip.vars[var].data.col;
        scip.cols[col].var_probindex = probindex;
    }
}

/// `SCIPvarRemove`: the variable leaves the problem; a transformed active
/// variable drops (final) or removes its implications, variable bounds
/// and cliques.
pub fn var_remove(scip: &mut Scip, var: VarId, final_: bool, keepimplics: bool) -> ScipResult {
    // If the variable is active in the transformed problem, remove it from
    // the implication graph.
    if var_is_transformed(scip, var) && (scip.vars[var].varstatus == VarStatus::Loose || scip.vars[var].varstatus == VarStatus::Column) {
        if final_ {
            // Just destroy the data structures.
            scip.vars[var].vlbs = None;
            scip.vars[var].vubs = None;
            scip.vars[var].implics = None;
        } else if !keepimplics {
            // Unlink the variable from all other variables' lists and free
            // the data structures.
            var_remove_cliques_implics_vbs(scip, var, false, false, true)?;
        }
    }

    // Mark the variable to be no longer a member of the problem.
    var_set_probindex(scip, var, -1);
    Ok(())
}

/// `SCIPvarMarkDeleted`.
pub fn var_mark_deleted(scip: &mut Scip, var: VarId) {
    scip.vars[var].deleted = true;
}

/// `SCIPvarMarkDoNotAggr`.
pub fn var_mark_do_not_aggr(scip: &mut Scip, var: VarId) -> ScipResult {
    let retvar = var_get_active_var(scip, var);
    match scip.vars[retvar].varstatus {
        VarStatus::Original | VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => {
            scip.vars[retvar].donotaggr = true;
            Ok(())
        }
        VarStatus::MultAggr => {
            log::error!("cannot mark a multi-aggregated variable to not be aggregated.");
            Err(Retcode::InvalidData)
        }
        _ => {
            log::error!("wrong variable status");
            Err(Retcode::InvalidData)
        }
    }
}

/// `SCIPvarMarkDoNotMultaggr`.
pub fn var_mark_do_not_multaggr(scip: &mut Scip, var: VarId) -> ScipResult {
    let retvar = var_get_active_var(scip, var);
    match scip.vars[retvar].varstatus {
        VarStatus::Original | VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => {
            scip.vars[retvar].donotmultaggr = true;
            Ok(())
        }
        VarStatus::MultAggr => {
            log::error!("cannot mark a multi-aggregated variable to not be multi-aggregated.");
            Err(Retcode::InvalidData)
        }
        _ => {
            log::error!("wrong variable status");
            Err(Retcode::InvalidData)
        }
    }
}

/// `SCIPvarChgType`: only for variables not (yet) in a problem.
pub fn var_chg_type(scip: &mut Scip, var: VarId, vartype: VarType) -> ScipResult {
    if scip.vars[var].probindex >= 0 {
        log::error!("cannot change type of variable already in the problem");
        return Err(Retcode::InvalidData);
    }

    // Change deprecated implied integral type to weakly implied integral
    // flag.
    if vartype == VarType::ImplInt {
        if scip.vars[var].varimpltype != ImplintType::Weak {
            var_chg_impl_type(scip, var, ImplintType::Weak)?;
        }
        return Ok(());
    }
    let oldtype = scip.vars[var].vartype;
    scip.vars[var].vartype = vartype;
    if scip.set.stage > Stage::Transforming {
        let event = crate::event::event_create_type_changed(var, oldtype, vartype);
        crate::event::eventqueue_add(scip, event)?;
    }
    let negvar = scip.vars[var].negatedvar;
    if negvar != NONE {
        scip.vars[negvar].vartype = vartype;
        if scip.set.stage > Stage::Transforming {
            let event = crate::event::event_create_type_changed(negvar, oldtype, vartype);
            crate::event::eventqueue_add(scip, event)?;
        }
    }
    Ok(())
}

/// `SCIPvarChgImplType`.
pub fn var_chg_impl_type(scip: &mut Scip, var: VarId, impltype: ImplintType) -> ScipResult {
    if scip.vars[var].probindex >= 0 {
        log::error!("cannot change type of variable already in the problem");
        return Err(Retcode::InvalidData);
    }
    let oldtype = scip.vars[var].varimpltype;
    scip.vars[var].varimpltype = impltype;
    if scip.set.stage > Stage::Transforming {
        let event = crate::event::event_create_impl_type_changed(var, oldtype, impltype);
        crate::event::eventqueue_add(scip, event)?;
    }
    let negvar = scip.vars[var].negatedvar;
    if negvar != NONE {
        scip.vars[negvar].varimpltype = impltype;
        if scip.set.stage > Stage::Transforming {
            let event = crate::event::event_create_impl_type_changed(negvar, oldtype, impltype);
            crate::event::eventqueue_add(scip, event)?;
        }
    }
    Ok(())
}

/// `varEventObjChanged`.
fn var_event_obj_changed(scip: &mut Scip, var: VarId, oldobj: f64, newobj: f64) -> ScipResult {
    let event = crate::event::event_create_obj_changed(var, oldobj, newobj);
    crate::event::eventqueue_add(scip, event)
}

/// `SCIPvarChgObj`.
pub fn var_chg_obj(scip: &mut Scip, var: VarId, prob: usize, newobj: f64) -> ScipResult {
    if scip.set.is_eq(scip.vars[var].obj, newobj) {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                let p = &scip.probs[prob];
                let transobj = p.objsense as i32 as f64 * newobj / p.objscale;
                var_chg_obj(scip, transvar, prob, transobj)?;
            } else {
                scip.vars[var].obj = newobj;
            }
            scip.vars[var].unchangedobj = newobj;
        }
        VarStatus::Loose | VarStatus::Column => {
            let oldobj = scip.vars[var].obj;
            scip.vars[var].obj = newobj;

            // Update unchanged objective value of variable.
            if !scip.lp.as_ref().is_some_and(|lp| lp.divingobjchg) {
                scip.vars[var].unchangedobj = newobj;
            }

            // Update the number of variables with non-zero objective
            // coefficient; we only want to do the update, if the variable
            // is added to the problem; since the objective of inactive
            // variables cannot be changed, this corresponds to
            // probindex != -1.
            if var_is_active(scip, var) {
                crate::prob::prob_update_nobj_vars(scip, prob, oldobj, newobj);
            }
            var_event_obj_changed(scip, var, oldobj, newobj)?;
        }
        _ => {
            log::error!("cannot change objective value of a fixed, aggregated, multi-aggregated, or negated variable");
            return Err(Retcode::InvalidData);
        }
    }
    Ok(())
}

/// `SCIPvarAddObj`: adds to the objective coefficient, passing it through
/// fixings and aggregations into the problem's offset.
pub fn var_add_obj(scip: &mut Scip, var: VarId, transprob: usize, origprob: usize, addobj: f64) -> ScipResult {
    if scip.set.is_zero(addobj) {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                let p = &scip.probs[transprob];
                let transadd = p.objsense as i32 as f64 * addobj / p.objscale;
                var_add_obj(scip, transvar, transprob, origprob, transadd)?;
            } else {
                scip.vars[var].obj += addobj;
            }
            scip.vars[var].unchangedobj += addobj;
        }
        VarStatus::Loose | VarStatus::Column => {
            let oldobj = scip.vars[var].obj;
            scip.vars[var].obj += addobj;

            // Update unchanged objective value of variable.
            if !scip.lp.as_ref().is_some_and(|lp| lp.divingobjchg) {
                scip.vars[var].unchangedobj += addobj;
            }

            // Update the number of variables with non-zero objective
            // coefficient.
            if var_is_active(scip, var) {
                let newobj = scip.vars[var].obj;
                crate::prob::prob_update_nobj_vars(scip, transprob, oldobj, newobj);
            }
            let newobj = scip.vars[var].obj;
            var_event_obj_changed(scip, var, oldobj, newobj)?;
        }
        VarStatus::Fixed => {
            let lb = scip.vars[var].locdom.lb;
            crate::prob::prob_add_objoffset(scip, transprob, lb * addobj);
            crate::primal::primal_update_objoffset(scip, crate::scip::TRANSPRIMAL, transprob, origprob)?;
        }
        VarStatus::Aggregated => {
            // x = a*y + c  ->  add a*addobj to obj. val. of y, and c*addobj
            // to obj. offset of problem.
            let a = scip.vars[var].data.aggregate;
            crate::prob::prob_add_objoffset(scip, transprob, a.constant * addobj);
            crate::primal::primal_update_objoffset(scip, crate::scip::TRANSPRIMAL, transprob, origprob)?;
            var_add_obj(scip, a.var, transprob, origprob, a.scalar * addobj)?;
        }
        VarStatus::MultAggr => {
            // x = a_1*y_1 + ... + a_n*y_n  + c  ->  add a_i*addobj to obj.
            // val. of y_i, and c*addobj to obj. offset.
            let constant = scip.vars[var].data.multaggr.constant;
            crate::prob::prob_add_objoffset(scip, transprob, constant * addobj);
            crate::primal::primal_update_objoffset(scip, crate::scip::TRANSPRIMAL, transprob, origprob)?;
            for i in 0..scip.vars[var].data.multaggr.nvars {
                let (mvar, mscalar) = (scip.vars[var].data.multaggr.vars[i], scip.vars[var].data.multaggr.scalars[i]);
                var_add_obj(scip, mvar, transprob, origprob, mscalar * addobj)?;
            }
        }
        VarStatus::Negated => {
            // x' = offset - x  ->  add -addobj to obj. val. of x and
            // offset*addobj to obj. offset of problem.
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            crate::prob::prob_add_objoffset(scip, transprob, constant * addobj);
            crate::primal::primal_update_objoffset(scip, crate::scip::TRANSPRIMAL, transprob, origprob)?;
            var_add_obj(scip, negvar, transprob, origprob, -addobj)?;
        }
    }
    Ok(())
}

/// `SCIPvarChgObjDive`: changes the objective of the variable's column in
/// diving only.
pub fn var_chg_obj_dive(scip: &mut Scip, var: VarId, mut newobj: f64) -> ScipResult {
    if scip.set.is_zero(newobj) {
        newobj = 0.0;
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            var_chg_obj_dive(scip, transvar, newobj)?;
        }
        VarStatus::Column => {
            let col = scip.vars[var].data.col;
            crate::lp::col_chg_obj(scip, col, newobj)?;
        }
        VarStatus::Loose | VarStatus::Fixed => {
            // Nothing to do here: only the constant shift in the objective
            // function would be changed, which is irrelevant.
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            var_chg_obj_dive(scip, a.var, newobj / a.scalar)?;
        }
        VarStatus::MultAggr => {
            log::error!("cannot change diving objective value of a multi-aggregated variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            let negvar = scip.vars[var].negatedvar;
            var_chg_obj_dive(scip, negvar, -newobj)?;
        }
    }
    Ok(())
}

/// `SCIPvarAdjustLb`.
pub fn var_adjust_lb(scip: &Scip, var: VarId, lb: &mut f64) {
    *lb = adjusted_lb(scip, var_is_integral(scip, var), *lb);
}

/// `SCIPvarAdjustUb`.
pub fn var_adjust_ub(scip: &Scip, var: VarId, ub: &mut f64) {
    *ub = adjusted_ub(scip, var_is_integral(scip, var), *ub);
}

/// `SCIPvarAdjustBd`.
pub fn var_adjust_bd(scip: &Scip, var: VarId, boundtype: BoundType, bd: &mut f64) {
    if boundtype == BoundType::Lower {
        var_adjust_lb(scip, var, bd);
    } else {
        var_adjust_ub(scip, var, bd);
    }
}

/// `SCIPvarChgLbOriginal`.
pub fn var_chg_lb_original(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    newbound = adjusted_lb(scip, var_is_integral(scip, var), newbound);
    if scip.set.is_zero(newbound) {
        newbound = 0.0;
    }

    // Original domains are only stored for ORIGINAL variables, not for
    // NEGATED.
    if scip.vars[var].varstatus == VarStatus::Original {
        if scip.set.is_eq(scip.vars[var].data.original.origdom.lb, newbound) {
            return Ok(());
        }
        scip.vars[var].data.original.origdom.lb = newbound;
    } else if scip.vars[var].varstatus == VarStatus::Negated {
        let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
        var_chg_ub_original(scip, negvar, constant - newbound)?;
    }

    // Process parent variables.
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        let constant = scip.vars[parentvar].data.negate.constant;
        var_chg_ub_original(scip, parentvar, constant - newbound)?;
    }
    Ok(())
}

/// `SCIPvarChgUbOriginal`.
pub fn var_chg_ub_original(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    newbound = adjusted_ub(scip, var_is_integral(scip, var), newbound);
    if scip.set.is_zero(newbound) {
        newbound = 0.0;
    }
    if scip.vars[var].varstatus == VarStatus::Original {
        if scip.set.is_eq(scip.vars[var].data.original.origdom.ub, newbound) {
            return Ok(());
        }
        scip.vars[var].data.original.origdom.ub = newbound;
    } else if scip.vars[var].varstatus == VarStatus::Negated {
        let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
        var_chg_lb_original(scip, negvar, constant - newbound)?;
    }
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        let constant = scip.vars[parentvar].data.negate.constant;
        var_chg_lb_original(scip, parentvar, constant - newbound)?;
    }
    Ok(())
}

/// Whether a bound change event of type `eventtype` on `var` is caught by
/// anyone, or the variable is active (the LP and branching candidates
/// always need its bound changes).
fn var_bound_event_needed(scip: &Scip, var: VarId, eventtype: crate::event::EventType) -> bool {
    let f = &scip.eventfilters[scip.vars[var].eventfilter];
    (f.len > 0 && (f.eventmask & eventtype) != 0) || scip.vars[var].varstatus == VarStatus::Column || scip.vars[var].varstatus == VarStatus::Loose
}

/// `varEventGlbChanged`.
fn var_event_glb_changed(scip: &mut Scip, var: VarId, oldbound: f64, newbound: f64) -> ScipResult {
    // Check, if the variable is being tracked for bound changes; COLUMN
    // and LOOSE variables are tracked always, because global/root pseudo
    // objective value has to be updated.
    if var_bound_event_needed(scip, var, crate::event::EVENTTYPE_GLBCHANGED) {
        let event = crate::event::event_create_glb_changed(var, oldbound, newbound);
        crate::event::eventqueue_add(scip, event)?;
    }
    Ok(())
}

/// `varEventGubChanged`.
fn var_event_gub_changed(scip: &mut Scip, var: VarId, oldbound: f64, newbound: f64) -> ScipResult {
    if var_bound_event_needed(scip, var, crate::event::EVENTTYPE_GUBCHANGED) {
        let event = crate::event::event_create_gub_changed(var, oldbound, newbound);
        crate::event::eventqueue_add(scip, event)?;
    }
    Ok(())
}

/// `varIncRootboundchgs`.
fn var_inc_rootboundchgs(scip: &mut Scip, var: VarId) {
    if var_is_active(scip, var) && var_is_transformed(scip, var) && scip.set.stage == Stage::Solving {
        scip.stat.nrootboundchgs += 1;
        scip.stat.nrootboundchgsrun += 1;
        if var_is_integral(scip, var) && scip.vars[var].glbdom.lb + 0.5 > scip.vars[var].glbdom.ub {
            scip.stat.nrootintfixings += 1;
            scip.stat.nrootintfixingsrun += 1;
        }
    }
}

/// `varProcessChgLbGlobal`: performs the global lower bound change and
/// passes it to the parent variables.
fn var_process_chg_lb_global(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    // Adjust bound to integral value if variable is of integral type.
    newbound = adjusted_lb(scip, var_is_integral(scip, var), newbound);

    // Check that the bound is feasible.
    if scip.set.stage != Stage::Problem && newbound > scip.vars[var].glbdom.ub {
        // Due to numerics we only want to be feasible in feasibility
        // tolerance.
        newbound = scip.vars[var].glbdom.ub;
    }

    // Change the bound only if it is not equal (or a sign switch near
    // zero).
    let glb = scip.vars[var].glbdom.lb;
    if scip.set.is_eq(newbound, glb) && !(newbound != glb && newbound * glb <= 0.0) {
        return Ok(());
    }

    // Change the bound.
    let oldbound = glb;
    scip.vars[var].glbdom.lb = newbound;

    // Update statistic; during the update steps of the parent variables we
    // pass a NULL pointer to ensure that we only count the number of root
    // bound changes of the active variables.
    var_inc_rootboundchgs(scip, var);

    // Merge new global bound into the local bound change info arrays.
    let glb = scip.vars[var].glbdom.lb;
    for i in 0..scip.vars[var].nlbchginfos {
        let info = &mut scip.vars[var].lbchginfos[i];
        if info.oldbound < glb {
            info.oldbound = glb;
            if scip.set.is_le(info.newbound, glb) {
                info.newbound = glb;
                info.boundchgtype = BoundchgType::Branching;
                info.redundant = true;
            } else {
                break; // from now on, the remaining newbounds are also larger than lb
            }
        } else {
            break; // from now on, the remaining oldbounds are also larger than lb
        }
    }

    // Remove redundant implications and variable bounds.
    if (scip.vars[var].varstatus == VarStatus::Column || scip.vars[var].varstatus == VarStatus::Loose) && (!scip.set.reopt_enable || scip.set.stage == Stage::Presolving) {
        var_remove_cliques_implics_vbs(scip, var, false, true, true)?;
    }

    // Issue bound change event.
    if scip.set.stage != Stage::Problem && scip.vars[var].eventfilter != NONE {
        var_event_glb_changed(scip, var, oldbound, newbound)?;
    }

    // Process parent variables.
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        match scip.vars[parentvar].varstatus {
            VarStatus::Original => var_process_chg_lb_global(scip, parentvar, newbound)?,
            VarStatus::Column | VarStatus::Loose | VarStatus::Fixed | VarStatus::MultAggr => {
                log::error!("column, loose, fixed or multi-aggregated variable cannot be the parent of a variable");
                return Err(Retcode::InvalidData);
            }
            VarStatus::Aggregated => {
                // x = a*y + c.
                let a = scip.vars[parentvar].data.aggregate;
                if a.scalar > 0.0 {
                    let parentnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                        a.scalar * newbound + a.constant
                    } else {
                        newbound
                    };
                    var_process_chg_lb_global(scip, parentvar, parentnewbound)?;
                } else {
                    let parentnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                        a.scalar * newbound + a.constant
                    } else {
                        -newbound
                    };
                    var_process_chg_ub_global(scip, parentvar, parentnewbound)?;
                }
            }
            VarStatus::Negated => {
                // x = offset - x'.
                let constant = scip.vars[parentvar].data.negate.constant;
                var_process_chg_ub_global(scip, parentvar, constant - newbound)?;
            }
        }
    }
    Ok(())
}

/// `varProcessChgUbGlobal`.
fn var_process_chg_ub_global(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    newbound = adjusted_ub(scip, var_is_integral(scip, var), newbound);
    if scip.set.stage != Stage::Problem && newbound < scip.vars[var].glbdom.lb {
        newbound = scip.vars[var].glbdom.lb;
    }
    let gub = scip.vars[var].glbdom.ub;
    if scip.set.is_eq(newbound, gub) && !(newbound != gub && newbound * gub <= 0.0) {
        return Ok(());
    }
    let oldbound = gub;
    scip.vars[var].glbdom.ub = newbound;
    var_inc_rootboundchgs(scip, var);
    let gub = scip.vars[var].glbdom.ub;
    for i in 0..scip.vars[var].nubchginfos {
        let info = &mut scip.vars[var].ubchginfos[i];
        if info.oldbound > gub {
            info.oldbound = gub;
            if scip.set.is_ge(info.newbound, gub) {
                info.newbound = gub;
                info.boundchgtype = BoundchgType::Branching;
                info.redundant = true;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    if (scip.vars[var].varstatus == VarStatus::Column || scip.vars[var].varstatus == VarStatus::Loose) && (!scip.set.reopt_enable || scip.set.stage == Stage::Presolving) {
        var_remove_cliques_implics_vbs(scip, var, false, true, true)?;
    }
    if scip.set.stage != Stage::Problem && scip.vars[var].eventfilter != NONE {
        var_event_gub_changed(scip, var, oldbound, newbound)?;
    }
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        match scip.vars[parentvar].varstatus {
            VarStatus::Original => var_process_chg_ub_global(scip, parentvar, newbound)?,
            VarStatus::Column | VarStatus::Loose | VarStatus::Fixed | VarStatus::MultAggr => {
                log::error!("column, loose, fixed or multi-aggregated variable cannot be the parent of a variable");
                return Err(Retcode::InvalidData);
            }
            VarStatus::Aggregated => {
                let a = scip.vars[parentvar].data.aggregate;
                if a.scalar > 0.0 {
                    let parentnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                        a.scalar * newbound + a.constant
                    } else {
                        newbound
                    };
                    var_process_chg_ub_global(scip, parentvar, parentnewbound)?;
                } else {
                    let parentnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                        a.scalar * newbound + a.constant
                    } else {
                        -newbound
                    };
                    var_process_chg_lb_global(scip, parentvar, parentnewbound)?;
                }
            }
            VarStatus::Negated => {
                let constant = scip.vars[parentvar].data.negate.constant;
                var_process_chg_lb_global(scip, parentvar, constant - newbound)?;
            }
        }
    }
    Ok(())
}

/// `SCIPvarChgLbGlobal`.
pub fn var_chg_lb_global(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    // Adjust bound to integral value if variable is of integral type.
    newbound = adjusted_lb(scip, var_is_integral(scip, var), newbound);

    // Check that the bound is feasible; this must be w.r.t. feastol
    // because SCIPvarFix() allows fixings that are outside of the domain
    // within feastol.
    if scip.set.stage != Stage::Problem {
        newbound = min(newbound, scip.vars[var].glbdom.ub);
    }

    // The new global bound has to be tighter except we are in the original
    // problem; this must be w.r.t. feastol because SCIPvarFix() allows
    // fixings that are outside of the domain within feastol.
    let glb = scip.vars[var].glbdom.lb;
    if scip.set.is_eq(glb, newbound) && !(newbound != glb && newbound * glb <= 0.0) {
        return Ok(());
    }

    // Change bounds of attached variables.
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                var_chg_lb_global(scip, transvar, newbound)?;
            } else {
                if newbound > scip.vars[var].locdom.lb {
                    var_chg_lb_local(scip, var, newbound)?;
                }
                var_process_chg_lb_global(scip, var, newbound)?;
            }
        }
        VarStatus::Column | VarStatus::Loose => {
            if newbound > scip.vars[var].locdom.lb {
                // `newbound` may be larger than the local upper bound when
                // it was not tightened yet.
                if newbound > scip.vars[var].locdom.ub {
                    var_chg_ub_local(scip, var, newbound)?;
                }
                var_chg_lb_local(scip, var, newbound)?;
            }
            var_process_chg_lb_global(scip, var, newbound)?;
        }
        VarStatus::Fixed => {
            log::error!("cannot change the bounds of a fixed variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Aggregated => {
            // x = a*y + c  ->  y = (x-c)/a.
            let a = scip.vars[var].data.aggregate;
            if a.scalar > 0.0 {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    newbound
                };
                var_chg_lb_global(scip, a.var, childnewbound)?;
            } else {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    -newbound
                };
                var_chg_ub_global(scip, a.var, childnewbound)?;
            }
        }
        VarStatus::MultAggr => {
            log::error!("cannot change the bounds of a multi-aggregated variable.");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            // x' = offset - x  ->  x = offset - x'.
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_chg_ub_global(scip, negvar, constant - newbound)?;
        }
    }
    Ok(())
}

/// `SCIPvarChgUbGlobal`.
pub fn var_chg_ub_global(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    newbound = adjusted_ub(scip, var_is_integral(scip, var), newbound);
    if scip.set.stage != Stage::Problem {
        newbound = max(newbound, scip.vars[var].glbdom.lb);
    }
    let gub = scip.vars[var].glbdom.ub;
    if scip.set.is_eq(gub, newbound) && !(newbound != gub && newbound * gub <= 0.0) {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                var_chg_ub_global(scip, transvar, newbound)?;
            } else {
                if newbound < scip.vars[var].locdom.ub {
                    var_chg_ub_local(scip, var, newbound)?;
                }
                var_process_chg_ub_global(scip, var, newbound)?;
            }
        }
        VarStatus::Column | VarStatus::Loose => {
            if newbound < scip.vars[var].locdom.ub {
                if newbound < scip.vars[var].locdom.lb {
                    var_chg_lb_local(scip, var, newbound)?;
                }
                var_chg_ub_local(scip, var, newbound)?;
            }
            var_process_chg_ub_global(scip, var, newbound)?;
        }
        VarStatus::Fixed => {
            log::error!("cannot change the bounds of a fixed variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            if a.scalar > 0.0 {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    newbound
                };
                var_chg_ub_global(scip, a.var, childnewbound)?;
            } else {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    -newbound
                };
                var_chg_lb_global(scip, a.var, childnewbound)?;
            }
        }
        VarStatus::MultAggr => {
            log::error!("cannot change the bounds of a multi-aggregated variable.");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_chg_lb_global(scip, negvar, constant - newbound)?;
        }
    }
    Ok(())
}

/// `SCIPvarChgLbLazy`.
pub fn var_chg_lb_lazy(scip: &mut Scip, var: VarId, lazylb: f64) -> ScipResult {
    if scip.vars[var].varstatus == VarStatus::Column {
        return Err(Retcode::InvalidCall);
    }
    scip.vars[var].lazylb = lazylb;
    Ok(())
}

/// `SCIPvarChgUbLazy`.
pub fn var_chg_ub_lazy(scip: &mut Scip, var: VarId, lazyub: f64) -> ScipResult {
    if scip.vars[var].varstatus == VarStatus::Column {
        return Err(Retcode::InvalidCall);
    }
    scip.vars[var].lazyub = lazyub;
    Ok(())
}

/// `SCIPvarChgBdGlobal`.
pub fn var_chg_bd_global(scip: &mut Scip, var: VarId, newbound: f64, boundtype: BoundType) -> ScipResult {
    match boundtype {
        BoundType::Lower => var_chg_lb_global(scip, var, newbound),
        BoundType::Upper => var_chg_ub_global(scip, var, newbound),
    }
}

/// `varEventLbChanged`.
fn var_event_lb_changed(scip: &mut Scip, var: VarId, oldbound: f64, newbound: f64) -> ScipResult {
    if var_bound_event_needed(scip, var, crate::event::EVENTTYPE_LBCHANGED) {
        let event = crate::event::event_create_lb_changed(var, oldbound, newbound);
        crate::event::eventqueue_add(scip, event)?;
    }
    Ok(())
}

/// `varEventUbChanged`.
fn var_event_ub_changed(scip: &mut Scip, var: VarId, oldbound: f64, newbound: f64) -> ScipResult {
    if var_bound_event_needed(scip, var, crate::event::EVENTTYPE_UBCHANGED) {
        let event = crate::event::event_create_ub_changed(var, oldbound, newbound);
        crate::event::eventqueue_add(scip, event)?;
    }
    Ok(())
}

/// `varProcessChgLbLocal`: performs the local lower bound change and
/// passes it to the parents. `countdomchg` false is C's `stat == NULL`
/// (the parents' updates are not counted).
fn var_process_chg_lb_local(scip: &mut Scip, var: VarId, countdomchg: bool, mut newbound: f64) -> ScipResult {
    // Adjust bound to integral value if variable is of integral type.
    newbound = adjusted_lb(scip, var_is_integral(scip, var), newbound);

    // Check that the bound is feasible.
    if scip.set.stage != Stage::Problem {
        newbound = min(newbound, scip.vars[var].locdom.ub);

        // Check that the bound is feasible.
        newbound = max(newbound, scip.vars[var].glbdom.lb);
    }
    let v = &scip.vars[var];
    if scip.set.is_eq(newbound, v.glbdom.lb) && v.glbdom.lb != v.locdom.lb {
        newbound = v.glbdom.lb;
    } else if scip.set.is_eq(newbound, v.locdom.lb) && !(newbound != v.locdom.lb && newbound * v.locdom.lb <= 0.0) {
        return Ok(());
    }

    // Change the bound.
    let oldbound = scip.vars[var].locdom.lb;
    scip.vars[var].locdom.lb = newbound;

    // Update statistic.
    if countdomchg {
        scip.stat.domchgcount += 1;
    }

    // Issue bound change event.
    if scip.set.stage != Stage::Problem && scip.vars[var].eventfilter != NONE {
        var_event_lb_changed(scip, var, oldbound, newbound)?;
    }

    // Process parent variables.
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        match scip.vars[parentvar].varstatus {
            VarStatus::Original => var_process_chg_lb_local(scip, parentvar, false, newbound)?,
            VarStatus::Column | VarStatus::Loose | VarStatus::Fixed | VarStatus::MultAggr => {
                log::error!("column, loose, fixed or multi-aggregated variable cannot be the parent of a variable");
                return Err(Retcode::InvalidData);
            }
            VarStatus::Aggregated => {
                // x = a*y + c.
                let a = scip.vars[parentvar].data.aggregate;
                let mut parentnewbound = a.scalar * newbound + a.constant;
                if a.scalar > 0.0 {
                    if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                        // If parent's new lower bound exceeds its upper
                        // bound, then this could be due to numerical
                        // difficulties, e.g., if numbers are large; thus,
                        // at least for now we take the upper bound, since
                        // this is the tighter one.
                        if parentnewbound > scip.vars[parentvar].glbdom.ub {
                            parentnewbound = scip.vars[parentvar].glbdom.ub;
                        }
                    } else {
                        parentnewbound = newbound;
                    }
                    var_process_chg_lb_local(scip, parentvar, false, parentnewbound)?;
                } else {
                    if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                        if parentnewbound < scip.vars[parentvar].glbdom.lb {
                            parentnewbound = scip.vars[parentvar].glbdom.lb;
                        }
                    } else {
                        parentnewbound = -newbound;
                    }
                    var_process_chg_ub_local(scip, parentvar, false, parentnewbound)?;
                }
            }
            VarStatus::Negated => {
                // x = offset - x'.
                let constant = scip.vars[parentvar].data.negate.constant;
                var_process_chg_ub_local(scip, parentvar, false, constant - newbound)?;
            }
        }
    }
    Ok(())
}

/// `varProcessChgUbLocal`.
fn var_process_chg_ub_local(scip: &mut Scip, var: VarId, countdomchg: bool, mut newbound: f64) -> ScipResult {
    newbound = adjusted_ub(scip, var_is_integral(scip, var), newbound);
    if scip.set.stage != Stage::Problem {
        newbound = max(newbound, scip.vars[var].locdom.lb);
        newbound = min(newbound, scip.vars[var].glbdom.ub);
    }
    let v = &scip.vars[var];
    if scip.set.is_eq(newbound, v.glbdom.ub) && v.glbdom.ub != v.locdom.ub {
        newbound = v.glbdom.ub;
    } else if scip.set.is_eq(newbound, v.locdom.ub) && !(newbound != v.locdom.ub && newbound * v.locdom.ub <= 0.0) {
        return Ok(());
    }
    let oldbound = scip.vars[var].locdom.ub;
    scip.vars[var].locdom.ub = newbound;
    if countdomchg {
        scip.stat.domchgcount += 1;
    }
    if scip.set.stage != Stage::Problem && scip.vars[var].eventfilter != NONE {
        var_event_ub_changed(scip, var, oldbound, newbound)?;
    }
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        match scip.vars[parentvar].varstatus {
            VarStatus::Original => var_process_chg_ub_local(scip, parentvar, false, newbound)?,
            VarStatus::Column | VarStatus::Loose | VarStatus::Fixed | VarStatus::MultAggr => {
                log::error!("column, loose, fixed or multi-aggregated variable cannot be the parent of a variable");
                return Err(Retcode::InvalidData);
            }
            VarStatus::Aggregated => {
                let a = scip.vars[parentvar].data.aggregate;
                let mut parentnewbound = a.scalar * newbound + a.constant;
                if a.scalar > 0.0 {
                    if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                        if parentnewbound < scip.vars[parentvar].glbdom.lb {
                            parentnewbound = scip.vars[parentvar].glbdom.lb;
                        }
                    } else {
                        parentnewbound = newbound;
                    }
                    var_process_chg_ub_local(scip, parentvar, false, parentnewbound)?;
                } else {
                    if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                        if parentnewbound > scip.vars[parentvar].glbdom.ub {
                            parentnewbound = scip.vars[parentvar].glbdom.ub;
                        }
                    } else {
                        parentnewbound = -newbound;
                    }
                    var_process_chg_lb_local(scip, parentvar, false, parentnewbound)?;
                }
            }
            VarStatus::Negated => {
                let constant = scip.vars[parentvar].data.negate.constant;
                var_process_chg_lb_local(scip, parentvar, false, constant - newbound)?;
            }
        }
    }
    Ok(())
}

/// `SCIPvarChgLbLocal`.
pub fn var_chg_lb_local(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    // Adjust bound to integral value if variable is of integral type.
    newbound = adjusted_lb(scip, var_is_integral(scip, var), newbound);

    // Check that the bound is feasible.
    if scip.set.stage != Stage::Problem {
        newbound = min(newbound, scip.vars[var].locdom.ub);
    }

    // Change the bound only if it is not equal to the current one, or the
    // global bound (which a relaxation can return to exactly).
    let v = &scip.vars[var];
    if scip.set.is_eq(v.locdom.lb, newbound) && (!scip.set.is_eq(v.glbdom.lb, newbound) || v.locdom.lb == newbound) && !(newbound != v.locdom.lb && newbound * v.locdom.lb <= 0.0) {
        return Ok(());
    }

    // Change bounds of attached variables.
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                var_chg_lb_local(scip, transvar, newbound)?;
            } else {
                var_process_chg_lb_local(scip, var, true, newbound)?;
            }
        }
        VarStatus::Column | VarStatus::Loose => var_process_chg_lb_local(scip, var, true, newbound)?,
        VarStatus::Fixed => {
            log::error!("cannot change the bounds of a fixed variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Aggregated => {
            // x = a*y + c  ->  y = (x-c)/a.
            let a = scip.vars[var].data.aggregate;
            if scip.set.is_positive(a.scalar) {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    newbound
                };
                var_chg_lb_local(scip, a.var, childnewbound)?;
            } else if scip.set.is_negative(a.scalar) {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    -newbound
                };
                var_chg_ub_local(scip, a.var, childnewbound)?;
            } else {
                log::error!("scalar is zero in aggregation");
                return Err(Retcode::InvalidData);
            }
        }
        VarStatus::MultAggr => {
            log::error!("cannot change the bounds of a multi-aggregated variable.");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            // x' = offset - x  ->  x = offset - x'.
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_chg_ub_local(scip, negvar, constant - newbound)?;
        }
    }
    Ok(())
}

/// `SCIPvarChgUbLocal`.
pub fn var_chg_ub_local(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    newbound = adjusted_ub(scip, var_is_integral(scip, var), newbound);
    if scip.set.stage != Stage::Problem {
        newbound = max(newbound, scip.vars[var].locdom.lb);
    }
    let v = &scip.vars[var];
    if scip.set.is_eq(v.locdom.ub, newbound) && (!scip.set.is_eq(v.glbdom.ub, newbound) || v.locdom.ub == newbound) && !(newbound != v.locdom.ub && newbound * v.locdom.ub <= 0.0) {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                var_chg_ub_local(scip, transvar, newbound)?;
            } else {
                var_process_chg_ub_local(scip, var, true, newbound)?;
            }
        }
        VarStatus::Column | VarStatus::Loose => var_process_chg_ub_local(scip, var, true, newbound)?,
        VarStatus::Fixed => {
            log::error!("cannot change the bounds of a fixed variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            if scip.set.is_positive(a.scalar) {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    newbound
                };
                var_chg_ub_local(scip, a.var, childnewbound)?;
            } else if scip.set.is_negative(a.scalar) {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    -newbound
                };
                var_chg_lb_local(scip, a.var, childnewbound)?;
            } else {
                log::error!("scalar is zero in aggregation");
                return Err(Retcode::InvalidData);
            }
        }
        VarStatus::MultAggr => {
            log::error!("cannot change the bounds of a multi-aggregated variable.");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_chg_lb_local(scip, negvar, constant - newbound)?;
        }
    }
    Ok(())
}

/// `SCIPvarChgBdLocal`.
pub fn var_chg_bd_local(scip: &mut Scip, var: VarId, newbound: f64, boundtype: BoundType) -> ScipResult {
    match boundtype {
        BoundType::Lower => var_chg_lb_local(scip, var, newbound),
        BoundType::Upper => var_chg_ub_local(scip, var, newbound),
    }
}

/// `SCIPvarChgLbDive`: changes the bound of the variable's column in
/// diving only.
pub fn var_chg_lb_dive(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    // Adjust bound for integral variables.
    var_adjust_lb(scip, var, &mut newbound);

    // Change bounds of attached variables.
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            var_chg_lb_dive(scip, transvar, newbound)?;
        }
        VarStatus::Column => {
            let col = scip.vars[var].data.col;
            crate::lp::col_chg_lb(scip, col, newbound)?;
        }
        VarStatus::Loose => {
            log::error!("cannot change variable's bounds in dive for LOOSE variables");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Fixed => {
            log::error!("cannot change the bounds of a fixed variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Aggregated => {
            // x = a*y + c  ->  y = (x-c)/a.
            let a = scip.vars[var].data.aggregate;
            if scip.set.is_positive(a.scalar) {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    newbound
                };
                var_chg_lb_dive(scip, a.var, childnewbound)?;
            } else if scip.set.is_negative(a.scalar) {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    -newbound
                };
                var_chg_ub_dive(scip, a.var, childnewbound)?;
            } else {
                log::error!("scalar is zero in aggregation");
                return Err(Retcode::InvalidData);
            }
        }
        VarStatus::MultAggr => {
            log::error!("cannot change the bounds of a multi-aggregated variable.");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            // x' = offset - x  ->  x = offset - x'.
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_chg_ub_dive(scip, negvar, constant - newbound)?;
        }
    }
    Ok(())
}

/// `SCIPvarChgUbDive`.
pub fn var_chg_ub_dive(scip: &mut Scip, var: VarId, mut newbound: f64) -> ScipResult {
    var_adjust_ub(scip, var, &mut newbound);
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            var_chg_ub_dive(scip, transvar, newbound)?;
        }
        VarStatus::Column => {
            let col = scip.vars[var].data.col;
            crate::lp::col_chg_ub(scip, col, newbound)?;
        }
        VarStatus::Loose => {
            log::error!("cannot change variable's bounds in dive for LOOSE variables");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Fixed => {
            log::error!("cannot change the bounds of a fixed variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            if scip.set.is_positive(a.scalar) {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    newbound
                };
                var_chg_ub_dive(scip, a.var, childnewbound)?;
            } else if scip.set.is_negative(a.scalar) {
                let childnewbound = if !scip.set.is_infinity(-newbound) && !scip.set.is_infinity(newbound) {
                    (newbound - a.constant) / a.scalar
                } else {
                    -newbound
                };
                var_chg_lb_dive(scip, a.var, childnewbound)?;
            } else {
                log::error!("scalar is zero in aggregation");
                return Err(Retcode::InvalidData);
            }
        }
        VarStatus::MultAggr => {
            log::error!("cannot change the bounds of a multi-aggregated variable.");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_chg_lb_dive(scip, negvar, constant - newbound)?;
        }
    }
    Ok(())
}

/// The bound of a multi-aggregated variable implied by its aggregation
/// and its own bound: `SCIPvarGetMultaggrLbLocal`, `UbLocal`, `LbGlobal`
/// and `UbGlobal`.
fn var_get_multaggr_bound(scip: &Scip, var: VarId, upper: bool, global: bool) -> f64 {
    let bound = |v: VarId, up: bool| {
        if scip.vars[v].varstatus == VarStatus::MultAggr {
            var_get_multaggr_bound(scip, v, up, global)
        } else {
            let d = if global { &scip.vars[v].glbdom } else { &scip.vars[v].locdom };
            if up { d.ub } else { d.lb }
        }
    };
    let own = bound_of(scip, var, upper, global);
    let m = &scip.vars[var].data.multaggr;
    let mut posinf = false;
    let mut neginf = false;
    let mut sum = m.constant;
    let mut i = m.nvars - 1;
    while i >= 0 {
        let aggrvar = m.vars[i];
        if m.scalars[i] > 0.0 {
            let bnd = bound(aggrvar, upper);
            if scip.set.is_infinity(bnd) {
                posinf = true;
            } else if scip.set.is_infinity(-bnd) {
                neginf = true;
            } else {
                sum += m.scalars[i] * bnd;
            }
        } else {
            let bnd = bound(aggrvar, !upper);
            if scip.set.is_infinity(-bnd) {
                posinf = true;
            } else if scip.set.is_infinity(bnd) {
                neginf = true;
            } else {
                sum += m.scalars[i] * bnd;
            }
        }
        if !upper && neginf {
            return own;
        }
        if upper && posinf {
            return own;
        }
        i -= 1;
    }
    if upper {
        if neginf {
            return -scip.set.infinity();
        }
        min(sum, own)
    } else {
        if posinf {
            return scip.set.infinity();
        }
        max(sum, own)
    }
}

/// The variable's own local or global bound.
#[inline]
fn bound_of(scip: &Scip, var: VarId, upper: bool, global: bool) -> f64 {
    let d = if global { &scip.vars[var].glbdom } else { &scip.vars[var].locdom };
    if upper { d.ub } else { d.lb }
}

/// `SCIPvarGetMultaggrLbLocal`.
pub fn var_get_multaggr_lb_local(scip: &Scip, var: VarId) -> f64 {
    var_get_multaggr_bound(scip, var, false, false)
}

/// `SCIPvarGetMultaggrUbLocal`.
pub fn var_get_multaggr_ub_local(scip: &Scip, var: VarId) -> f64 {
    var_get_multaggr_bound(scip, var, true, false)
}

/// `SCIPvarGetMultaggrLbGlobal`.
pub fn var_get_multaggr_lb_global(scip: &Scip, var: VarId) -> f64 {
    var_get_multaggr_bound(scip, var, false, true)
}

/// `SCIPvarGetMultaggrUbGlobal`.
pub fn var_get_multaggr_ub_global(scip: &Scip, var: VarId) -> f64 {
    var_get_multaggr_bound(scip, var, true, true)
}

/// `SCIPvarResetBounds`: resets an original variable's bounds to its
/// original domain.
pub fn var_reset_bounds(scip: &mut Scip, var: VarId) -> ScipResult {
    let (lb, ub) = (scip.vars[var].data.original.origdom.lb, scip.vars[var].data.original.origdom.ub);
    var_chg_lb_global(scip, var, lb)?;
    var_chg_ub_global(scip, var, ub)?;
    var_chg_lb_local(scip, var, lb)?;
    var_chg_ub_local(scip, var, ub)?;
    Ok(())
}

/// `varEventImplAdded`.
fn var_event_impl_added(scip: &mut Scip, var: VarId) -> ScipResult {
    // Issue IMPLADDED event on variable.
    let event = crate::event::event_create_impl_added(var);
    crate::event::eventqueue_add(scip, event)
}

/// `varAddVbound`: adds a variable bound to the variable's list (not
/// checking it).
fn var_add_vbound(scip: &mut Scip, var: VarId, vbtype: BoundType, vbvar: VarId, vbcoef: f64, vbconstant: f64) -> ScipResult {
    // If the variable bound is on the variable itself, it is redundant or
    // a bound change: nothing to store.
    if var == vbvar {
        return Ok(());
    }
    let added = crate::implics::vbounds_add(scip, var, vbtype, vbvar, vbcoef, vbconstant);
    scip.vars[var].closestvblpcount = -1;
    if added {
        var_event_impl_added(scip, var)?;
    }
    Ok(())
}

const MAXIMPLSCLOSURE: i32 = 100;
const MAXABSVBCOEF: f64 = 1e+5;

/// `checkImplic`: whether the implication `implvar >= / <= implbound` is
/// redundant or infeasible for the global bounds.
fn check_implic(scip: &Scip, implvar: VarId, impltype: BoundType, implbound: f64, redundant: &mut bool, infeasible: &mut bool) {
    let impllb = scip.vars[implvar].glbdom.lb;
    let implub = scip.vars[implvar].glbdom.ub;
    if impltype == BoundType::Lower {
        *infeasible = scip.set.is_feas_gt(implbound, implub);
        *redundant = scip.set.is_feas_le(implbound, impllb);
    } else {
        *infeasible = scip.set.is_feas_lt(implbound, impllb);
        *redundant = scip.set.is_feas_ge(implbound, implub);
    }
}

/// `applyImplic`: applies the implication as a global bound change (at
/// the root node once solving).
fn apply_implic(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    implvar: VarId,
    impltype: BoundType,
    implbound: f64,
    infeasible: &mut bool,
    nbdchgs: Option<&mut i32>,
) -> ScipResult {
    *infeasible = false;
    let implub = scip.vars[implvar].glbdom.ub;
    let impllb = scip.vars[implvar].glbdom.lb;
    if impltype == BoundType::Lower {
        if scip.set.is_feas_gt(implbound, implub) {
            *infeasible = true;
        } else if scip.set.is_feas_gt(implbound, impllb) {
            // We are in the root node, or in the probing node
            if scip.set.stage >= Stage::Solving {
                let root = crate::tree::tree_get_root_node(scip);
                crate::tree::node_add_boundchg(scip, root, transprob, origprob, crate::scip::GLOBALEVENTFILTER, implvar, implbound, BoundType::Lower, false)?;
            } else {
                var_chg_lb_global(scip, implvar, implbound)?;
            }
            if let Some(n) = nbdchgs {
                *n += 1;
            }
        }
    } else if scip.set.is_feas_lt(implbound, impllb) {
        *infeasible = true;
    } else if scip.set.is_feas_lt(implbound, implub) {
        if scip.set.stage >= Stage::Solving {
            let root = crate::tree::tree_get_root_node(scip);
            crate::tree::node_add_boundchg(scip, root, transprob, origprob, crate::scip::GLOBALEVENTFILTER, implvar, implbound, BoundType::Upper, false)?;
        } else {
            var_chg_ub_global(scip, implvar, implbound)?;
        }
        if let Some(n) = nbdchgs {
            *n += 1;
        }
    }
    Ok(())
}

/// `varAddImplic`: adds `(var == varfixing) => implvar >=/<= implbound`
/// as a clique (binary implvar) or implication, fixing `var` on a
/// conflict.
fn var_add_implic_internal(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    varfixing: bool,
    implvar: VarId,
    impltype: BoundType,
    implbound: f64,
    isshortcut: bool,
    infeasible: &mut bool,
    mut nbdchgs: Option<&mut i32>,
    added: &mut bool,
) -> ScipResult {
    *infeasible = false;
    *added = false;

    // Check implication on debugging solution: not ported.
    let mut redundant = false;
    let mut conflict = false;
    check_implic(scip, implvar, impltype, implbound, &mut redundant, &mut conflict);

    // Check if variable is already fixed or the implication is redundant.
    if redundant {
        return Ok(());
    }

    // Check, if implication is added to the variable itself: x == varfixing
    // -> x <= / >= implbound.
    if var == implvar {
        if scip.set.is_lt(implbound, 0.0) || scip.set.is_gt(implbound, 1.0) {
            // x == varfixing -> x <= / >= implbound with implbound < 0 or
            // > 1: x must be !varfixing.
            conflict = true;
        } else {
            // x == varfixing -> x <= 0 or x >= 1: if varfixing == the
            // bound, redundant, else conflict.
            conflict = conflict || ((varfixing) == (impltype == BoundType::Upper));
            if !conflict {
                return Ok(());
            }
        }
    }

    // Check, if the variable is already fixed.
    if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
        // If the variable is fixed to the given value, perform the
        // implication; otherwise, ignore the implication.
        if varfixing == (scip.vars[var].glbdom.lb > 0.5) {
            apply_implic(scip, transprob, origprob, implvar, impltype, implbound, infeasible, nbdchgs)?;
        }
        return Ok(());
    }

    // Do not add implication over fixed variables, aggregated variables
    // are ok; the implied variable is a binary: add a clique.
    if !conflict {
        if var_is_binary(scip, implvar) {
            let vars = [var, implvar];
            let vals = [varfixing, impltype == BoundType::Upper];

            // Add the clique to the clique table.
            crate::implics::cliquetable_add(scip, transprob, origprob, &vars, &vals, false, &mut conflict, nbdchgs.as_deref_mut())?;
            if !conflict {
                return Ok(());
            }
        } else {
            // Add implication x == 0/1 -> y <= / >= b to the implications
            // list of x.
            crate::implics::implics_add(scip, var, varfixing, implvar, impltype, implbound, isshortcut, &mut conflict, added);
        }
    }

    // On conflict, fix the variable to the opposite value.
    if conflict {
        if scip.set.stage >= Stage::Solving {
            let root = crate::tree::tree_get_root_node(scip);
            if varfixing {
                crate::tree::node_add_boundchg(scip, root, transprob, origprob, crate::scip::GLOBALEVENTFILTER, var, 0.0, BoundType::Upper, false)?;
            } else {
                crate::tree::node_add_boundchg(scip, root, transprob, origprob, crate::scip::GLOBALEVENTFILTER, var, 1.0, BoundType::Lower, false)?;
            }
        } else if varfixing {
            var_chg_ub_global(scip, var, 0.0)?;
        } else {
            var_chg_lb_global(scip, var, 1.0)?;
        }
        if let Some(n) = nbdchgs {
            *n += 1;
        }
        return Ok(());
    } else if *added {
        // Issue IMPLADDED event.
        var_event_impl_added(scip, var)?;
    } else {
        // The implication was redundant.
        return Ok(());
    }

    // Check, whether implied variable is binary: no variable bounds needed
    // (cliques are stored instead).
    if !var_is_binary(scip, implvar) {
        let lb = scip.vars[implvar].glbdom.lb;
        let ub = scip.vars[implvar].glbdom.ub;

        // Add the variable bound x' <= b*y + d, or x' >= b*y + d,
        // respectively.
        if impltype == BoundType::Upper {
            // Variable bound coefficient: x <= (implbound - ub)*y + ub.
            if (implbound - ub).abs() <= MAXABSVBCOEF {
                let (coef, constant) = if varfixing { (implbound - ub, ub) } else { (ub - implbound, implbound) };
                var_add_vbound(scip, implvar, BoundType::Upper, var, coef, constant)?;
            }
        } else if (implbound - lb).abs() <= MAXABSVBCOEF {
            let (coef, constant) = if varfixing { (implbound - lb, lb) } else { (lb - implbound, implbound) };
            var_add_vbound(scip, implvar, BoundType::Lower, var, coef, constant)?;
        }
    }
    Ok(())
}

/// `varAddTransitiveBinaryClosureImplic`: adds the implications of
/// `implvar == implvarfixing` to `var == varfixing`.
fn var_add_transitive_binary_closure_implic(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    varfixing: bool,
    implvar: VarId,
    implvarfixing: bool,
    infeasible: &mut bool,
    mut nbdchgs: Option<&mut i32>,
) -> ScipResult {
    *infeasible = false;
    let fix = implvarfixing as usize;
    let nimpls = |scip: &Scip| scip.vars[implvar].implics.as_ref().map_or(0, |im| im.nimpls[fix]);

    // Add the implications of implvar to var; the implications list of
    // implvar may change, so only the count and live entries are read.
    let mut i = min(nimpls(scip), MAXIMPLSCLOSURE) - 1;
    while i >= 0 && !*infeasible {
        let im = scip.vars[implvar].implics.as_ref().unwrap();
        let (iv, it, ib) = (im.vars[fix][i], im.types[fix][i], im.bounds[fix][i]);

        // The implication var == varfixing -> implvar == implvarfixing ->
        // iv <= / >= ib holds.
        if var_is_active(scip, iv) {
            let mut added = false;
            var_add_implic_internal(scip, var, transprob, origprob, varfixing, iv, it, ib, true, infeasible, nbdchgs.as_deref_mut(), &mut added)?;
            i = min(i, nimpls(scip));
        }
        i -= 1;
    }
    Ok(())
}

/// `varAddTransitiveImplic`: adds the implication and, if `transitive`,
/// its consequences through `implvar`'s implications or variable bounds.
fn var_add_transitive_implic(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    varfixing: bool,
    implvar: VarId,
    impltype: BoundType,
    implbound: f64,
    transitive: bool,
    infeasible: &mut bool,
    mut nbdchgs: Option<&mut i32>,
) -> ScipResult {
    // Add implication x == 0/1 -> y <= b / y >= b to the implications list
    // of x.
    let mut added = false;
    var_add_implic_internal(
        scip,
        var,
        transprob,
        origprob,
        varfixing,
        implvar,
        impltype,
        implbound,
        false,
        infeasible,
        nbdchgs.as_deref_mut(),
        &mut added,
    )?;
    if *infeasible || var == implvar || !transitive || !added {
        return Ok(());
    }

    // Add transitive closure.
    if scip.vars[implvar].vartype == VarType::Binary && !var_is_implied_integral(scip, implvar) {
        // Binary variable: implications of implvar are added to var.
        let implvarfixing = impltype == BoundType::Lower;
        var_add_transitive_binary_closure_implic(scip, var, transprob, origprob, varfixing, implvar, implvarfixing, infeasible, nbdchgs.as_deref_mut())?;

        // Add implications to implvar == implvarfixing -> var.
        if !*infeasible {
            var_add_transitive_binary_closure_implic(scip, implvar, transprob, origprob, !implvarfixing, var, !varfixing, infeasible, nbdchgs.as_deref_mut())?;
        }
    } else {
        // Non-binary variable: add implications through variable bounds.
        // x == varfixing -> y <= b and y >= b_i*z_i + d_i ->
        // z_i <= (b - d_i)/b_i for b_i > 0.
        if impltype == BoundType::Upper && scip.vars[implvar].vlbs.is_some() {
            let nv = |scip: &Scip| scip.vars[implvar].vlbs.as_ref().map_or(0, |vb| vb.len);
            let mut i = nv(scip) - 1;
            while i >= 0 && !*infeasible {
                let vb = scip.vars[implvar].vlbs.as_ref().unwrap();
                let (vbvar, vbcoef, vbconst) = (vb.vars[i], vb.coefs[i], vb.constants[i]);

                // We have x == varfixing -> y <= b and y >= c*z + d:
                //  c > 0: x == varfixing -> z <= (b-d)/c
                //  c < 0: x == varfixing -> z >= (b-d)/c
                // If the variable bound variable is aggregated or negated,
                // the implication is not stored.
                if var_is_active(scip, vbvar) && scip.vars[vbvar].varstatus != VarStatus::Aggregated && scip.vars[vbvar].varstatus != VarStatus::Negated {
                    let mut vbimplbound = (implbound - vbconst) / vbcoef;
                    let mut added = false;
                    if vbcoef >= 0.0 {
                        vbimplbound = adjusted_ub(scip, var_is_integral(scip, vbvar), vbimplbound);
                        var_add_implic_internal(
                            scip,
                            var,
                            transprob,
                            origprob,
                            varfixing,
                            vbvar,
                            BoundType::Upper,
                            vbimplbound,
                            true,
                            infeasible,
                            nbdchgs.as_deref_mut(),
                            &mut added,
                        )?;
                    } else {
                        vbimplbound = adjusted_lb(scip, var_is_integral(scip, vbvar), vbimplbound);
                        var_add_implic_internal(
                            scip,
                            var,
                            transprob,
                            origprob,
                            varfixing,
                            vbvar,
                            BoundType::Lower,
                            vbimplbound,
                            true,
                            infeasible,
                            nbdchgs.as_deref_mut(),
                            &mut added,
                        )?;
                    }
                    i = min(i, nv(scip));
                }
                i -= 1;
            }
        }

        // x == varfixing -> y >= b and y <= b_i*z_i + d_i ->
        // z_i >= (b - d_i)/b_i for b_i > 0.
        if impltype == BoundType::Lower && scip.vars[implvar].vubs.is_some() {
            let nv = |scip: &Scip| scip.vars[implvar].vubs.as_ref().map_or(0, |vb| vb.len);
            let mut i = nv(scip) - 1;
            while i >= 0 && !*infeasible {
                let vb = scip.vars[implvar].vubs.as_ref().unwrap();
                let (vbvar, vbcoef, vbconst) = (vb.vars[i], vb.coefs[i], vb.constants[i]);
                if var_is_active(scip, vbvar) && scip.vars[vbvar].varstatus != VarStatus::Aggregated && scip.vars[vbvar].varstatus != VarStatus::Negated {
                    let mut vbimplbound = (implbound - vbconst) / vbcoef;
                    let mut added = false;
                    if vbcoef >= 0.0 {
                        vbimplbound = adjusted_lb(scip, var_is_integral(scip, vbvar), vbimplbound);
                        var_add_implic_internal(
                            scip,
                            var,
                            transprob,
                            origprob,
                            varfixing,
                            vbvar,
                            BoundType::Lower,
                            vbimplbound,
                            true,
                            infeasible,
                            nbdchgs.as_deref_mut(),
                            &mut added,
                        )?;
                    } else {
                        vbimplbound = adjusted_ub(scip, var_is_integral(scip, vbvar), vbimplbound);
                        var_add_implic_internal(
                            scip,
                            var,
                            transprob,
                            origprob,
                            varfixing,
                            vbvar,
                            BoundType::Upper,
                            vbimplbound,
                            true,
                            infeasible,
                            nbdchgs.as_deref_mut(),
                            &mut added,
                        )?;
                    }
                    i = min(i, nv(scip));
                }
                i -= 1;
            }
        }
    }
    Ok(())
}

/// The global bound change of variable-bound and implication processing:
/// at the root node once solving (`SCIPnodeAddBoundchg`), otherwise
/// `SCIPvarChg{Lb,Ub}Global`.
fn root_bound_change(scip: &mut Scip, transprob: usize, origprob: usize, var: VarId, newbound: f64, boundtype: BoundType) -> ScipResult {
    if scip.set.stage >= Stage::Solving {
        let root = crate::tree::tree_get_root_node(scip);
        crate::tree::node_add_boundchg(scip, root, transprob, origprob, crate::scip::GLOBALEVENTFILTER, var, newbound, boundtype, false)
    } else if boundtype == BoundType::Lower {
        var_chg_lb_global(scip, var, newbound)
    } else {
        var_chg_ub_global(scip, var, newbound)
    }
}

/// `SCIPvarAddVlb`: adds `var >= vlbcoef * vlbvar + vlbconstant`,
/// strengthening bounds with it and storing it as an implication or
/// variable bound.
pub fn var_add_vlb(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    mut vlbvar: VarId,
    mut vlbcoef: f64,
    mut vlbconstant: f64,
    transitive: bool,
    infeasible: &mut bool,
    mut nbdchgs: Option<&mut i32>,
) -> ScipResult {
    *infeasible = false;
    if let Some(n) = nbdchgs.as_deref_mut() {
        *n = 0;
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            var_add_vlb(scip, transvar, transprob, origprob, vlbvar, vlbcoef, vlbconstant, transitive, infeasible, nbdchgs)?;
        }
        VarStatus::Column | VarStatus::Loose | VarStatus::Fixed => {
            // Transform b*z + d into the corresponding sum after
            // transforming z to an active problem variable.
            var_get_probvar_sum(scip, &mut vlbvar, &mut vlbcoef, &mut vlbconstant)?;

            // If the vlb coefficient is zero, just update the lower bound
            // of the variable.
            if var == vlbvar {
                if scip.set.is_eq(vlbcoef, 1.0) {
                    // x >= x + c: c <= 0 or infeasible.
                    if scip.set.is_feas_positive(vlbconstant) {
                        *infeasible = true;
                    }
                } else {
                    let lb = scip.vars[var].glbdom.lb;
                    let ub = scip.vars[var].glbdom.ub;

                    // The variable bound constraint defines a new upper
                    // bound: x >= b*x + d  =>  (1-b)*x >= d
                    if scip.set.is_gt(vlbcoef, 1.0) {
                        let newub = adjusted_ub(scip, var_is_integral(scip, var), vlbconstant / (1.0 - vlbcoef));
                        if scip.set.is_feas_lt(newub, lb) {
                            *infeasible = true;
                            return Ok(());
                        }
                        if scip.set.is_feas_lt(newub, ub) {
                            // We are in the root node, or in the probing
                            // node, so we can tighten the global bound.
                            root_bound_change(scip, transprob, origprob, var, newub, BoundType::Upper)?;
                            if let Some(n) = nbdchgs.as_deref_mut() {
                                *n += 1;
                            }
                        }
                    } else {
                        // The variable bound constraint defines a new lower
                        // bound.
                        let newlb = adjusted_lb(scip, var_is_integral(scip, var), vlbconstant / (1.0 - vlbcoef));
                        if scip.set.is_feas_gt(newlb, ub) {
                            *infeasible = true;
                            return Ok(());
                        }
                        if scip.set.is_feas_gt(newlb, lb) {
                            root_bound_change(scip, transprob, origprob, var, newlb, BoundType::Lower)?;
                            if let Some(n) = nbdchgs.as_deref_mut() {
                                *n += 1;
                            }
                        }
                    }
                }
            } else if scip.set.is_zero(vlbcoef) {
                // The variable bound constraint is a bound: x >= d.
                vlbconstant = adjusted_lb(scip, var_is_integral(scip, var), vlbconstant);
                if scip.set.is_feas_gt(vlbconstant, scip.vars[var].glbdom.ub) {
                    *infeasible = true;
                } else if scip.set.is_feas_gt(vlbconstant, scip.vars[var].glbdom.lb) {
                    root_bound_change(scip, transprob, origprob, var, vlbconstant, BoundType::Lower)?;
                    if let Some(n) = nbdchgs.as_deref_mut() {
                        *n += 1;
                    }
                }
            } else if var_is_active(scip, vlbvar) {
                let mut minvlb = -scip.set.infinity();
                let mut maxvlb = scip.set.infinity();
                let mut xlb = scip.vars[var].glbdom.lb;
                let xub = scip.vars[var].glbdom.ub;
                let mut zlb = scip.vars[vlbvar].glbdom.lb;
                let mut zub = scip.vars[vlbvar].glbdom.ub;

                // Improve global bounds of vlb variable, and calculate
                // minimal and maximal value of variable bound.
                if vlbcoef >= 0.0 {
                    if !scip.set.is_infinity(xub) {
                        // x >= b*z + d  ->  z <= (x-d)/b.
                        let newzub = adjusted_ub(scip, var_is_integral(scip, vlbvar), (xub - vlbconstant) / vlbcoef);
                        if scip.set.is_feas_lt(newzub, zlb) {
                            *infeasible = true;
                            return Ok(());
                        }
                        if scip.set.is_feas_lt(newzub, zub) {
                            root_bound_change(scip, transprob, origprob, vlbvar, newzub, BoundType::Upper)?;
                            zub = newzub;
                            if let Some(n) = nbdchgs.as_deref_mut() {
                                *n += 1;
                            }
                        }
                        maxvlb = vlbcoef * zub + vlbconstant;
                        if !scip.set.is_infinity(-zlb) {
                            minvlb = vlbcoef * zlb + vlbconstant;
                        }
                    } else {
                        if !scip.set.is_infinity(zub) {
                            maxvlb = vlbcoef * zub + vlbconstant;
                        }
                        if !scip.set.is_infinity(-zlb) {
                            minvlb = vlbcoef * zlb + vlbconstant;
                        }
                    }
                } else if !scip.set.is_infinity(xub) {
                    // x >= b*z + d  ->  z >= (x-d)/b.
                    let newzlb = adjusted_lb(scip, var_is_integral(scip, vlbvar), (xub - vlbconstant) / vlbcoef);
                    if scip.set.is_feas_gt(newzlb, zub) {
                        *infeasible = true;
                        return Ok(());
                    }
                    if scip.set.is_feas_gt(newzlb, zlb) {
                        root_bound_change(scip, transprob, origprob, vlbvar, newzlb, BoundType::Lower)?;
                        zlb = newzlb;
                        if let Some(n) = nbdchgs.as_deref_mut() {
                            *n += 1;
                        }
                    }
                    maxvlb = vlbcoef * zlb + vlbconstant;
                    if !scip.set.is_infinity(zub) {
                        minvlb = vlbcoef * zub + vlbconstant;
                    }
                } else {
                    if !scip.set.is_infinity(-zlb) {
                        maxvlb = vlbcoef * zlb + vlbconstant;
                    }
                    if !scip.set.is_infinity(zub) {
                        minvlb = vlbcoef * zub + vlbconstant;
                    }
                }
                if maxvlb < minvlb {
                    maxvlb = minvlb;
                }

                // Adjust bounds due to integrality of variable.
                minvlb = adjusted_lb(scip, var_is_integral(scip, var), minvlb);
                maxvlb = adjusted_lb(scip, var_is_integral(scip, var), maxvlb);

                // Check bounds for feasibility.
                if scip.set.is_feas_gt(minvlb, xub) {
                    *infeasible = true;
                    return Ok(());
                }

                // Improve global lower bound of variable.
                if scip.set.is_feas_gt(minvlb, xlb) {
                    root_bound_change(scip, transprob, origprob, var, minvlb, BoundType::Lower)?;
                    xlb = minvlb;
                    if let Some(n) = nbdchgs.as_deref_mut() {
                        *n += 1;
                    }
                }
                minvlb = xlb;

                // Improve variable bound for binary z by moving the
                // variable's global bound to the vlb constant.
                let vlbvar_binary = scip.vars[vlbvar].vartype == VarType::Binary && !var_is_implied_integral(scip, vlbvar);
                if vlbvar_binary {
                    // b > 0: x >= (maxvlb - minvlb) * z + minvlb
                    // b < 0: x >= (minvlb - maxvlb) * z + maxvlb
                    if vlbcoef >= 0.0 {
                        vlbcoef = maxvlb - minvlb;
                        vlbconstant = minvlb;
                    } else {
                        vlbcoef = minvlb - maxvlb;
                        vlbconstant = maxvlb;
                    }
                }

                // Add variable bound to the variable bounds list.
                if scip.set.is_feas_gt(maxvlb, xlb) {
                    if vlbvar_binary {
                        // Add corresponding implication:
                        //   b > 0, x >= b*z + d  <->  z == 1 -> x >= b+d
                        //   b < 0, x >= b*z + d  <->  z == 0 -> x >= d
                        var_add_transitive_implic(
                            scip,
                            vlbvar,
                            transprob,
                            origprob,
                            vlbcoef >= 0.0,
                            var,
                            BoundType::Lower,
                            maxvlb,
                            transitive,
                            infeasible,
                            nbdchgs,
                        )?;
                    } else if scip.vars[var].vartype == VarType::Binary && !var_is_implied_integral(scip, var) {
                        // Add corresponding implication:
                        //   b > 0, x >= b*z + d  <->  x == 0 -> z <= -d/b
                        //   b < 0, x >= b*z + d  <->  x == 0 -> z >= -d/b
                        let mut implbound = -vlbconstant / vlbcoef;

                        // Tighten the implication bound if the variable is
                        // integer.
                        if var_is_integral(scip, vlbvar) {
                            if vlbcoef >= 0.0 {
                                implbound = scip.set.floor(implbound);
                            } else {
                                implbound = scip.set.ceil(implbound);
                            }
                        }
                        let impltype = if vlbcoef >= 0.0 { BoundType::Upper } else { BoundType::Lower };
                        var_add_transitive_implic(scip, var, transprob, origprob, false, vlbvar, impltype, implbound, transitive, infeasible, nbdchgs)?;
                    } else {
                        var_add_vbound(scip, var, BoundType::Lower, vlbvar, vlbcoef, vlbconstant)?;
                    }
                }
            }
        }
        VarStatus::Aggregated => {
            // x = a*y + c:  x >= b*z + d  <=>  a*y + c >= b*z + d  <=>
            // y >= b/a * z + (d-c)/a, if a > 0; y <= ..., if a < 0.
            var_get_probvar_sum(scip, &mut vlbvar, &mut vlbcoef, &mut vlbconstant)?;
            let a = scip.vars[var].data.aggregate;
            if a.var == vlbvar && scip.set.is_eq(a.scalar, vlbcoef) {
                if scip.set.is_feas_lt(a.constant, vlbconstant) {
                    *infeasible = true;
                }
            } else if scip.set.is_positive(a.scalar) {
                // a > 0 -> add variable lower bound.
                var_add_vlb(
                    scip,
                    a.var,
                    transprob,
                    origprob,
                    vlbvar,
                    vlbcoef / a.scalar,
                    (vlbconstant - a.constant) / a.scalar,
                    transitive,
                    infeasible,
                    nbdchgs,
                )?;
            } else if scip.set.is_negative(a.scalar) {
                // a < 0 -> add variable upper bound.
                var_add_vub(
                    scip,
                    a.var,
                    transprob,
                    origprob,
                    vlbvar,
                    vlbcoef / a.scalar,
                    (vlbconstant - a.constant) / a.scalar,
                    transitive,
                    infeasible,
                    nbdchgs,
                )?;
            } else {
                log::error!("scalar is zero in aggregation");
                return Err(Retcode::InvalidData);
            }
        }
        VarStatus::MultAggr => {
            // Nothing to do here.
        }
        VarStatus::Negated => {
            // x = offset - x':  x >= b*z + d  <=>  offset - x' >= b*z + d
            // <=>  x' <= -b*z + (offset-d).
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_add_vub(scip, negvar, transprob, origprob, vlbvar, -vlbcoef, constant - vlbconstant, transitive, infeasible, nbdchgs)?;
        }
    }
    Ok(())
}

/// `SCIPvarAddVub`: adds `var <= vubcoef * vubvar + vubconstant`.
pub fn var_add_vub(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    mut vubvar: VarId,
    mut vubcoef: f64,
    mut vubconstant: f64,
    transitive: bool,
    infeasible: &mut bool,
    mut nbdchgs: Option<&mut i32>,
) -> ScipResult {
    *infeasible = false;
    if let Some(n) = nbdchgs.as_deref_mut() {
        *n = 0;
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            var_add_vub(scip, transvar, transprob, origprob, vubvar, vubcoef, vubconstant, transitive, infeasible, nbdchgs)?;
        }
        VarStatus::Column | VarStatus::Loose | VarStatus::Fixed => {
            var_get_probvar_sum(scip, &mut vubvar, &mut vubcoef, &mut vubconstant)?;
            if var == vubvar {
                if scip.set.is_eq(vubcoef, 1.0) {
                    if scip.set.is_feas_negative(vubconstant) {
                        *infeasible = true;
                    }
                } else {
                    let lb = scip.vars[var].glbdom.lb;
                    let ub = scip.vars[var].glbdom.ub;

                    // The variable bound constraint defines a new lower
                    // bound: x <= b*x + d  =>  (1-b)*x <= d.
                    if scip.set.is_gt(vubcoef, 1.0) {
                        let newlb = adjusted_lb(scip, var_is_integral(scip, var), vubconstant / (1.0 - vubcoef));
                        if scip.set.is_feas_gt(newlb, ub) {
                            *infeasible = true;
                            return Ok(());
                        }
                        if scip.set.is_feas_gt(newlb, lb) {
                            root_bound_change(scip, transprob, origprob, var, newlb, BoundType::Lower)?;
                            if let Some(n) = nbdchgs.as_deref_mut() {
                                *n += 1;
                            }
                        }
                    } else {
                        // The variable bound constraint defines a new upper
                        // bound.
                        let newub = adjusted_ub(scip, var_is_integral(scip, var), vubconstant / (1.0 - vubcoef));
                        if scip.set.is_feas_lt(newub, lb) {
                            *infeasible = true;
                            return Ok(());
                        }
                        if scip.set.is_feas_lt(newub, ub) {
                            root_bound_change(scip, transprob, origprob, var, newub, BoundType::Upper)?;
                            if let Some(n) = nbdchgs.as_deref_mut() {
                                *n += 1;
                            }
                        }
                    }
                }
            } else if scip.set.is_zero(vubcoef) {
                // The variable bound constraint is a bound: x <= d.
                vubconstant = adjusted_ub(scip, var_is_integral(scip, var), vubconstant);
                if scip.set.is_feas_lt(vubconstant, scip.vars[var].glbdom.lb) {
                    *infeasible = true;
                } else if scip.set.is_feas_lt(vubconstant, scip.vars[var].glbdom.ub) {
                    root_bound_change(scip, transprob, origprob, var, vubconstant, BoundType::Upper)?;
                    if let Some(n) = nbdchgs.as_deref_mut() {
                        *n += 1;
                    }
                }
            } else if var_is_active(scip, vubvar) {
                let mut minvub = -scip.set.infinity();
                let mut maxvub = scip.set.infinity();
                let xlb = scip.vars[var].glbdom.lb;
                let mut xub = scip.vars[var].glbdom.ub;
                let mut zlb = scip.vars[vubvar].glbdom.lb;
                let mut zub = scip.vars[vubvar].glbdom.ub;

                // Improve global bounds of vub variable, and calculate
                // minimal and maximal value of variable bound.
                if vubcoef >= 0.0 {
                    if !scip.set.is_infinity(-xlb) {
                        // x <= b*z + d  ->  z >= (x-d)/b.
                        let newzlb = adjusted_lb(scip, var_is_integral(scip, vubvar), (xlb - vubconstant) / vubcoef);
                        if scip.set.is_feas_gt(newzlb, zub) {
                            *infeasible = true;
                            return Ok(());
                        }
                        if scip.set.is_feas_gt(newzlb, zlb) {
                            root_bound_change(scip, transprob, origprob, vubvar, newzlb, BoundType::Lower)?;
                            zlb = newzlb;
                            if let Some(n) = nbdchgs.as_deref_mut() {
                                *n += 1;
                            }
                        }
                        minvub = vubcoef * zlb + vubconstant;
                        if !scip.set.is_infinity(zub) {
                            maxvub = vubcoef * zub + vubconstant;
                        }
                    } else {
                        if !scip.set.is_infinity(zub) {
                            maxvub = vubcoef * zub + vubconstant;
                        }
                        if !scip.set.is_infinity(-zlb) {
                            minvub = vubcoef * zlb + vubconstant;
                        }
                    }
                } else if !scip.set.is_infinity(-xlb) {
                    // x <= b*z + d  ->  z <= (x-d)/b.
                    let newzub = adjusted_ub(scip, var_is_integral(scip, vubvar), (xlb - vubconstant) / vubcoef);
                    if scip.set.is_feas_lt(newzub, zlb) {
                        *infeasible = true;
                        return Ok(());
                    }
                    if scip.set.is_feas_lt(newzub, zub) {
                        root_bound_change(scip, transprob, origprob, vubvar, newzub, BoundType::Upper)?;
                        zub = newzub;
                        if let Some(n) = nbdchgs.as_deref_mut() {
                            *n += 1;
                        }
                    }
                    minvub = vubcoef * zub + vubconstant;
                    if !scip.set.is_infinity(-zlb) {
                        maxvub = vubcoef * zlb + vubconstant;
                    }
                } else {
                    if !scip.set.is_infinity(zub) {
                        minvub = vubcoef * zub + vubconstant;
                    }
                    if !scip.set.is_infinity(-zlb) {
                        maxvub = vubcoef * zlb + vubconstant;
                    }
                }
                if minvub > maxvub {
                    minvub = maxvub;
                }

                // Adjust bounds due to integrality of vub variable.
                minvub = adjusted_ub(scip, var_is_integral(scip, var), minvub);
                maxvub = adjusted_ub(scip, var_is_integral(scip, var), maxvub);

                // Check bounds for feasibility.
                if scip.set.is_feas_lt(maxvub, xlb) {
                    *infeasible = true;
                    return Ok(());
                }

                // Improve global upper bound of variable.
                if scip.set.is_feas_lt(maxvub, xub) {
                    root_bound_change(scip, transprob, origprob, var, maxvub, BoundType::Upper)?;
                    xub = maxvub;
                    if let Some(n) = nbdchgs.as_deref_mut() {
                        *n += 1;
                    }
                }
                maxvub = xub;

                // Improve variable bound for binary z by moving the
                // variable's global bound to the vub constant.
                if var_is_binary(scip, vubvar) {
                    // b > 0: x <= (maxvub - minvub) * z + minvub
                    // b < 0: x <= (minvub - maxvub) * z + maxvub
                    if vubcoef >= 0.0 {
                        vubcoef = maxvub - minvub;
                        vubconstant = minvub;
                    } else {
                        vubcoef = minvub - maxvub;
                        vubconstant = maxvub;
                    }
                }

                // Add variable bound to the variable bounds list.
                if scip.set.is_feas_lt(minvub, xub) {
                    if scip.vars[vubvar].vartype == VarType::Binary && !var_is_implied_integral(scip, vubvar) {
                        // Add corresponding implication:
                        //   b > 0, x <= b*z + d  <->  z == 0 -> x <= d
                        //   b < 0, x <= b*z + d  <->  z == 1 -> x <= b+d
                        var_add_transitive_implic(
                            scip,
                            vubvar,
                            transprob,
                            origprob,
                            vubcoef < 0.0,
                            var,
                            BoundType::Upper,
                            minvub,
                            transitive,
                            infeasible,
                            nbdchgs,
                        )?;
                    } else if scip.vars[var].vartype == VarType::Binary && !var_is_implied_integral(scip, var) {
                        // Add corresponding implication:
                        //   b > 0, x <= b*z + d  <->  x == 1 -> z >= (1-d)/b
                        //   b < 0, x <= b*z + d  <->  x == 1 -> z <= (1-d)/b
                        let impltype = if vubcoef >= 0.0 { BoundType::Lower } else { BoundType::Upper };
                        var_add_transitive_implic(
                            scip,
                            var,
                            transprob,
                            origprob,
                            true,
                            vubvar,
                            impltype,
                            (1.0 - vubconstant) / vubcoef,
                            transitive,
                            infeasible,
                            nbdchgs,
                        )?;
                    } else {
                        var_add_vbound(scip, var, BoundType::Upper, vubvar, vubcoef, vubconstant)?;
                    }
                }
            }
        }
        VarStatus::Aggregated => {
            // x = a*y + c:  x <= b*z + d  <=>  a*y + c <= b*z + d  <=>
            // y <= b/a * z + (d-c)/a, if a > 0; y >= ..., if a < 0.
            var_get_probvar_sum(scip, &mut vubvar, &mut vubcoef, &mut vubconstant)?;
            let a = scip.vars[var].data.aggregate;
            if a.var == vubvar && scip.set.is_eq(a.scalar, vubcoef) {
                if scip.set.is_feas_gt(a.constant, vubconstant) {
                    *infeasible = true;
                }
            } else if scip.set.is_positive(a.scalar) {
                var_add_vub(
                    scip,
                    a.var,
                    transprob,
                    origprob,
                    vubvar,
                    vubcoef / a.scalar,
                    (vubconstant - a.constant) / a.scalar,
                    transitive,
                    infeasible,
                    nbdchgs,
                )?;
            } else if scip.set.is_negative(a.scalar) {
                var_add_vlb(
                    scip,
                    a.var,
                    transprob,
                    origprob,
                    vubvar,
                    vubcoef / a.scalar,
                    (vubconstant - a.constant) / a.scalar,
                    transitive,
                    infeasible,
                    nbdchgs,
                )?;
            } else {
                log::error!("scalar is zero in aggregation");
                return Err(Retcode::InvalidData);
            }
        }
        VarStatus::MultAggr => {
            // Nothing to do here.
        }
        VarStatus::Negated => {
            // x = offset - x':  x <= b*z + d  <=>  offset - x' <= b*z + d
            // <=>  x' >= -b*z + (offset-d).
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_add_vlb(scip, negvar, transprob, origprob, vubvar, -vubcoef, constant - vubconstant, transitive, infeasible, nbdchgs)?;
        }
    }
    Ok(())
}

/// `SCIPvarAddImplic`: adds `(var == varfixing) => implvar >=/<= implbound`,
/// passing it through aggregations and negations of the binary `var`.
pub fn var_add_implic(
    scip: &mut Scip,
    var: VarId,
    transprob: usize,
    origprob: usize,
    varfixing: bool,
    mut implvar: VarId,
    mut impltype: BoundType,
    mut implbound: f64,
    transitive: bool,
    infeasible: &mut bool,
    mut nbdchgs: Option<&mut i32>,
) -> ScipResult {
    *infeasible = false;
    if let Some(n) = nbdchgs.as_deref_mut() {
        *n = 0;
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            var_add_implic(
                scip, transvar, transprob, origprob, varfixing, implvar, impltype, implbound, transitive, infeasible, nbdchgs,
            )?;
        }
        VarStatus::Column | VarStatus::Loose => {
            // If the variable is fixed (although it has no FIXED status),
            // and varfixing corresponds to the fixed value of the variable,
            // the implication can be applied directly (otherwise, the
            // implication is redundant).
            if scip.vars[var].glbdom.lb > 0.5 || scip.vars[var].glbdom.ub < 0.5 {
                if varfixing == (scip.vars[var].glbdom.lb > 0.5) {
                    apply_implic(scip, transprob, origprob, implvar, impltype, implbound, infeasible, nbdchgs)?;
                }
            } else {
                var_get_probvar_bound(scip, &mut implvar, &mut implbound, &mut impltype)?;
                var_adjust_bd(scip, implvar, impltype, &mut implbound);
                if var_is_active(scip, implvar) || scip.vars[implvar].varstatus == VarStatus::Fixed {
                    var_add_transitive_implic(scip, var, transprob, origprob, varfixing, implvar, impltype, implbound, transitive, infeasible, nbdchgs)?;
                }
            }
        }
        VarStatus::Fixed => {
            // If varfixing corresponds to the fixed value of the variable,
            // the implication can be applied directly (otherwise, the
            // implication is redundant).
            if varfixing == (scip.vars[var].glbdom.lb > 0.5) {
                apply_implic(scip, transprob, origprob, implvar, impltype, implbound, infeasible, nbdchgs)?;
            }
        }
        VarStatus::Aggregated => {
            // (x == 1 => y <= b)  <=>  (x' == 1 => y <= b) for x = x' + 0;
            // (x == 1 => y <= b)  <=>  (x' == 0 => y <= b) for x = 1 - x'.
            let a = scip.vars[var].data.aggregate;
            if var_is_binary(scip, a.var) {
                if a.scalar > 0.0 {
                    var_add_implic(scip, a.var, transprob, origprob, varfixing, implvar, impltype, implbound, transitive, infeasible, nbdchgs)?;
                } else {
                    var_add_implic(scip, a.var, transprob, origprob, !varfixing, implvar, impltype, implbound, transitive, infeasible, nbdchgs)?;
                }
            }
        }
        VarStatus::MultAggr => {
            // Nothing to do here.
        }
        VarStatus::Negated => {
            // (x == 1 => y <= b)  <=>  (x' == 0 => y <= b).
            let negvar = scip.vars[var].negatedvar;
            if scip.vars[negvar].vartype == VarType::Binary && !var_is_implied_integral(scip, negvar) {
                var_add_implic(scip, negvar, transprob, origprob, !varfixing, implvar, impltype, implbound, transitive, infeasible, nbdchgs)?;
            } else if scip.vars[implvar].vartype == VarType::Binary {
                // The negated variable is not binary: the implication is
                // stored on the implied variable, as y == 0/1 => x' >= / <= 1/0.
                let implfix = impltype == BoundType::Upper;
                var_add_implic(
                    scip,
                    implvar,
                    transprob,
                    origprob,
                    implfix,
                    negvar,
                    if varfixing { BoundType::Lower } else { BoundType::Upper },
                    if varfixing { 1.0 } else { 0.0 },
                    transitive,
                    infeasible,
                    nbdchgs,
                )?;
            } else {
                // (x = 1 => y <= b) <=> (x' = 0 => y <= b) <=> y <= b*x' +
                // ... as variable bounds on both sides.
                let up = impltype == BoundType::Upper;
                if varfixing {
                    var_add_vlb(
                        scip,
                        negvar,
                        transprob,
                        origprob,
                        implvar,
                        if up { 1.0 } else { -1.0 },
                        if up { 0.0 } else { 1.0 },
                        transitive,
                        infeasible,
                        nbdchgs.as_deref_mut(),
                    )?;
                } else {
                    var_add_vub(
                        scip,
                        negvar,
                        transprob,
                        origprob,
                        implvar,
                        if up { -1.0 } else { 1.0 },
                        if up { 1.0 } else { 0.0 },
                        transitive,
                        infeasible,
                        nbdchgs.as_deref_mut(),
                    )?;
                }
                if up {
                    var_add_vub(
                        scip,
                        implvar,
                        transprob,
                        origprob,
                        negvar,
                        if varfixing { 1.0 } else { -1.0 },
                        if varfixing { 0.0 } else { 1.0 },
                        transitive,
                        infeasible,
                        nbdchgs,
                    )?;
                } else {
                    var_add_vlb(
                        scip,
                        implvar,
                        transprob,
                        origprob,
                        negvar,
                        if varfixing { -1.0 } else { 1.0 },
                        if varfixing { 1.0 } else { 0.0 },
                        transitive,
                        infeasible,
                        nbdchgs,
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// `SCIPvarHasImplic`.
pub fn var_has_implic(scip: &Scip, var: VarId, varfixing: bool, implvar: VarId, impltype: BoundType) -> bool {
    scip.vars[var]
        .implics
        .as_ref()
        .is_some_and(|im| crate::implics::implics_contains_impl(im, varfixing, implvar, impltype))
}

/// `SCIPvarHasBinaryImplic`.
pub fn var_has_binary_implic(scip: &Scip, var: VarId, varfixing: bool, implvar: VarId, implvarfixing: bool) -> bool {
    var_has_implic(scip, var, varfixing, implvar, if implvarfixing { BoundType::Lower } else { BoundType::Upper })
}

/// `SCIPvarGetImplicVarBounds`: the bounds `var == varfixing` implies on
/// `implvar`, `INVALID` where none.
pub fn var_get_implic_var_bounds(scip: &Scip, var: VarId, varfixing: bool, implvar: VarId, lb: &mut f64, ub: &mut f64) {
    *lb = INVALID;
    *ub = INVALID;
    let Some(im) = scip.vars[var].implics.as_ref() else { return };
    let (mut lowerpos, mut upperpos) = (-1, -1);
    crate::implics::implics_get_var_implic_poss(im, varfixing, implvar, &mut lowerpos, &mut upperpos);
    let bounds = &im.bounds[varfixing as usize];
    if bounds.is_empty() {
        return;
    }
    if lowerpos >= 0 {
        *lb = bounds[lowerpos];
    }
    if upperpos >= 0 {
        *ub = bounds[upperpos];
    }
}

/// `SCIPvarFixBinary`: fixes a binary variable globally (at the root node
/// once solving).
pub fn var_fix_binary(scip: &mut Scip, var: VarId, transprob: usize, origprob: usize, value: bool, infeasible: &mut bool, nbdchgs: Option<&mut i32>) -> ScipResult {
    *infeasible = false;
    if !value {
        if scip.vars[var].glbdom.lb > 0.5 {
            *infeasible = true;
        } else if scip.vars[var].glbdom.ub > 0.5 {
            root_bound_change(scip, transprob, origprob, var, 0.0, BoundType::Upper)?;
            if let Some(n) = nbdchgs {
                *n += 1;
            }
        }
    } else if scip.vars[var].glbdom.ub < 0.5 {
        *infeasible = true;
    } else if scip.vars[var].glbdom.lb < 0.5 {
        root_bound_change(scip, transprob, origprob, var, 1.0, BoundType::Lower)?;
        if let Some(n) = nbdchgs {
            *n += 1;
        }
    }
    Ok(())
}

/// `SCIPvarAddClique`: adds the variable (as `value`) to `clique`, fixing
/// it or the clique's other variables when it appears twice.
pub fn var_add_clique(
    scip: &mut Scip,
    mut var: VarId,
    transprob: usize,
    origprob: usize,
    mut value: bool,
    clique: usize,
    infeasible: &mut bool,
    mut nbdchgs: Option<&mut i32>,
) -> ScipResult {
    *infeasible = false;

    // Get corresponding active problem variable.
    var_get_probvar_binary(scip, &mut var, &mut value)?;
    if scip.vars[var].varstatus == VarStatus::Column || scip.vars[var].varstatus == VarStatus::Loose {
        let mut doubleentry = false;
        let mut oppositeentry = false;

        // Add variable to clique.
        crate::implics::clique_add_var(scip, clique, var, value, &mut doubleentry, &mut oppositeentry);

        // Add clique to variable's clique list.
        crate::implics::cliquelist_add(scip, var, value, clique);

        // Check consistency of cliquelist.
        crate::implics::cliquelist_check(scip, var);

        // If the variable now appears twice in the clique with the same
        // value, it has to be fixed to the opposite value.
        if doubleentry {
            var_fix_binary(scip, var, transprob, origprob, !value, infeasible, nbdchgs.as_deref_mut())?;
        }

        // If the variable appears with both values in the clique, all
        // other variables of the clique have to be fixed to the opposite
        // of their values.
        if oppositeentry {
            let nvars = scip.cliques[clique].nvars;
            let mut i = 0;
            while i < nvars && !*infeasible {
                let (v, val) = (scip.cliques[clique].vars[i], scip.cliques[clique].values[i]);
                if v != var {
                    var_fix_binary(scip, v, transprob, origprob, !val, infeasible, nbdchgs.as_deref_mut())?;
                }
                i += 1;
            }
        }
    }
    Ok(())
}

/// `SCIPvarsAddClique`: adds `clique` to the clique lists of its variables.
pub fn vars_add_clique(scip: &mut Scip, vars: &[VarId], values: &[bool], clique: usize) {
    let mut v = vars.len() as i32 - 1;
    while v >= 0 {
        let var = vars[v as usize];
        crate::implics::cliquelist_add(scip, var, values[v as usize], clique);
        crate::implics::cliquelist_check(scip, var);
        v -= 1;
    }
}

/// `SCIPvarAddCliqueToList`.
pub fn var_add_clique_to_list(scip: &mut Scip, var: VarId, value: bool, clique: usize) {
    crate::implics::cliquelist_add(scip, var, value, clique);
}

/// `SCIPvarDelCliqueFromList`.
pub fn var_del_clique_from_list(scip: &mut Scip, var: VarId, value: bool, clique: usize) {
    crate::implics::cliquelist_del(scip, var, value, clique);
}

/// `SCIPvarDelClique`.
pub fn var_del_clique(scip: &mut Scip, mut var: VarId, mut value: bool, clique: usize) -> ScipResult {
    // Get corresponding active problem variable.
    var_get_probvar_binary(scip, &mut var, &mut value)?;
    if scip.vars[var].varstatus == VarStatus::Column || scip.vars[var].varstatus == VarStatus::Loose {
        // Delete clique from variable's clique list.
        crate::implics::cliquelist_del(scip, var, value, clique);

        // Delete variable from clique.
        crate::implics::clique_del_var(scip, clique, var, value);

        // Check consistency of cliquelist.
        crate::implics::cliquelist_check(scip, var);
    }
    Ok(())
}

/// `SCIPvarsHaveCommonClique`.
pub fn vars_have_common_clique(scip: &Scip, var1: VarId, value1: bool, var2: VarId, value2: bool, regardimplics: bool) -> bool {
    crate::implics::cliquelists_have_common_clique(scip, var1, value1, var2, value2)
        || (regardimplics && var_has_implic(scip, var1, value1, var2, if value2 { BoundType::Upper } else { BoundType::Lower }))
}

/// `varProcessChgBranchFactor`.
fn var_process_chg_branch_factor(scip: &mut Scip, var: VarId, mut branchfactor: f64) -> ScipResult {
    // Only use positive values.
    let eps = scip.set.epsilon();
    branchfactor = max(branchfactor, eps);
    if scip.set.is_eq(branchfactor, scip.vars[var].branchfactor) {
        return Ok(());
    }

    // Change the branch factor.
    scip.vars[var].branchfactor = branchfactor;

    // Process parent variables.
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        match scip.vars[parentvar].varstatus {
            VarStatus::Original => {
                // Do not change priorities across the border between
                // transformed and original problem.
            }
            VarStatus::Column | VarStatus::Loose | VarStatus::Fixed | VarStatus::MultAggr => {
                log::error!("column, loose, fixed or multi-aggregated variable cannot be the parent of a variable");
                return Err(Retcode::InvalidData);
            }
            VarStatus::Aggregated | VarStatus::Negated => var_process_chg_branch_factor(scip, parentvar, branchfactor)?,
        }
    }
    Ok(())
}

/// `SCIPvarChgBranchFactor`.
pub fn var_chg_branch_factor(scip: &mut Scip, var: VarId, branchfactor: f64) -> ScipResult {
    if scip.set.is_eq(scip.vars[var].branchfactor, branchfactor) {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                var_chg_branch_factor(scip, transvar, branchfactor)?;
            } else {
                scip.vars[var].branchfactor = branchfactor;
            }
        }
        VarStatus::Column | VarStatus::Loose | VarStatus::Fixed => var_process_chg_branch_factor(scip, var, branchfactor)?,
        VarStatus::Aggregated => {
            let aggrvar = scip.vars[var].data.aggregate.var;
            var_chg_branch_factor(scip, aggrvar, branchfactor)?;
        }
        VarStatus::MultAggr => {
            for v in 0..scip.vars[var].data.multaggr.nvars {
                let mvar = scip.vars[var].data.multaggr.vars[v];
                var_chg_branch_factor(scip, mvar, branchfactor)?;
            }
        }
        VarStatus::Negated => {
            let negvar = scip.vars[var].negatedvar;
            var_chg_branch_factor(scip, negvar, branchfactor)?;
        }
    }
    Ok(())
}

/// `varProcessChgBranchPriority`.
fn var_process_chg_branch_priority(scip: &mut Scip, var: VarId, branchpriority: i32) -> ScipResult {
    if branchpriority == scip.vars[var].branchpriority {
        return Ok(());
    }
    scip.vars[var].branchpriority = branchpriority;
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        match scip.vars[parentvar].varstatus {
            VarStatus::Original => {}
            VarStatus::Column | VarStatus::Loose | VarStatus::Fixed | VarStatus::MultAggr => {
                log::error!("column, loose, fixed or multi-aggregated variable cannot be the parent of a variable");
                return Err(Retcode::InvalidData);
            }
            VarStatus::Aggregated | VarStatus::Negated => var_process_chg_branch_priority(scip, parentvar, branchpriority)?,
        }
    }
    Ok(())
}

/// `SCIPvarChgBranchPriority`.
pub fn var_chg_branch_priority(scip: &mut Scip, var: VarId, branchpriority: i32) -> ScipResult {
    if scip.vars[var].branchpriority == branchpriority {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                var_chg_branch_priority(scip, transvar, branchpriority)?;
            } else {
                scip.vars[var].branchpriority = branchpriority;
            }
        }
        VarStatus::Column | VarStatus::Loose | VarStatus::Fixed => var_process_chg_branch_priority(scip, var, branchpriority)?,
        VarStatus::Aggregated => {
            let aggrvar = scip.vars[var].data.aggregate.var;
            var_chg_branch_priority(scip, aggrvar, branchpriority)?;
        }
        VarStatus::MultAggr => {
            for v in 0..scip.vars[var].data.multaggr.nvars {
                let mvar = scip.vars[var].data.multaggr.vars[v];
                var_chg_branch_priority(scip, mvar, branchpriority)?;
            }
        }
        VarStatus::Negated => {
            let negvar = scip.vars[var].negatedvar;
            var_chg_branch_priority(scip, negvar, branchpriority)?;
        }
    }
    Ok(())
}

/// `varProcessChgBranchDirection`.
fn var_process_chg_branch_direction(scip: &mut Scip, var: VarId, branchdirection: BranchDir) -> ScipResult {
    if branchdirection == scip.vars[var].branchdirection {
        return Ok(());
    }
    scip.vars[var].branchdirection = branchdirection;
    for i in 0..scip.vars[var].nparentvars {
        let parentvar = scip.vars[var].parentvars[i];
        match scip.vars[parentvar].varstatus {
            VarStatus::Original => {}
            VarStatus::Column | VarStatus::Loose | VarStatus::Fixed | VarStatus::MultAggr => {
                log::error!("column, loose, fixed or multi-aggregated variable cannot be the parent of a variable");
                return Err(Retcode::InvalidData);
            }
            VarStatus::Aggregated => {
                if scip.vars[parentvar].data.aggregate.scalar > 0.0 {
                    var_process_chg_branch_direction(scip, parentvar, branchdirection)?;
                } else {
                    var_process_chg_branch_direction(scip, parentvar, branchdir_opposite(branchdirection))?;
                }
            }
            VarStatus::Negated => var_process_chg_branch_direction(scip, parentvar, branchdir_opposite(branchdirection))?,
        }
    }
    Ok(())
}

/// `SCIPvarChgBranchDirection`.
pub fn var_chg_branch_direction(scip: &mut Scip, var: VarId, branchdirection: BranchDir) -> ScipResult {
    if scip.vars[var].branchdirection == branchdirection {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar != NONE {
                var_chg_branch_direction(scip, transvar, branchdirection)?;
            } else {
                scip.vars[var].branchdirection = branchdirection;
            }
        }
        VarStatus::Column | VarStatus::Loose | VarStatus::Fixed => var_process_chg_branch_direction(scip, var, branchdirection)?,
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            if a.scalar > 0.0 {
                var_chg_branch_direction(scip, a.var, branchdirection)?;
            } else {
                var_chg_branch_direction(scip, a.var, branchdir_opposite(branchdirection))?;
            }
        }
        VarStatus::MultAggr => {
            for v in 0..scip.vars[var].data.multaggr.nvars {
                // Only update branching direction of aggregation variables,
                // if they don't have a preferred direction yet.
                let (mvar, scalar) = (scip.vars[var].data.multaggr.vars[v], scip.vars[var].data.multaggr.scalars[v]);
                if scip.vars[mvar].branchdirection == BranchDir::Auto {
                    if scalar > 0.0 {
                        var_chg_branch_direction(scip, mvar, branchdirection)?;
                    } else {
                        var_chg_branch_direction(scip, mvar, branchdir_opposite(branchdirection))?;
                    }
                }
            }
        }
        VarStatus::Negated => {
            let negvar = scip.vars[var].negatedvar;
            var_chg_branch_direction(scip, negvar, branchdir_opposite(branchdirection))?;
        }
    }
    Ok(())
}

/// `SCIPvarCompareActiveAndNegated`: compares by index, negated variables
/// as their negation.
pub fn var_compare_active_and_negated(scip: &Scip, mut var1: VarId, mut var2: VarId) -> i32 {
    if scip.vars[var1].varstatus == VarStatus::Negated {
        var1 = scip.vars[var1].negatedvar;
    }
    if scip.vars[var2].varstatus == VarStatus::Negated {
        var2 = scip.vars[var2].negatedvar;
    }
    let (i1, i2) = (scip.vars[var1].index, scip.vars[var2].index);
    if i1 < i2 {
        -1
    } else if i1 > i2 {
        1
    } else {
        0
    }
}

/// `SCIPvarCompare` (and the `SCIPvarComp` sort comparator): by index.
pub fn var_compare(scip: &Scip, var1: VarId, var2: VarId) -> i32 {
    let (i1, i2) = (scip.vars[var1].index, scip.vars[var2].index);
    if i1 < i2 {
        -1
    } else if i1 > i2 {
        1
    } else {
        0
    }
}

/// `SCIPvarComp`.
pub fn var_comp(scip: &Scip, var1: VarId, var2: VarId) -> i32 {
    var_compare(scip, var1, var2)
}

/// Sorts `vars` by index, as `SCIPsortPtr(vars, SCIPvarComp, n)`.
fn sort_vars(scip: &Scip, vars: &mut [VarId]) {
    crate::sort::sort(vars, (), |a, b| var_compare(scip, *a, *b));
}

/// `SCIPvarsGetActiveVars`: the distinct active variables the given
/// variables depend on, sorted by index; `requiredsize` alone if
/// `varssize` is too small.
pub fn vars_get_active_vars(scip: &Scip, vars: &mut CVec<VarId>, nvars: &mut i32, varssize: i32, requiredsize: &mut i32) -> ScipResult {
    *requiredsize = 0;
    if *nvars == 0 {
        return Ok(());
    }
    let mut nactivevars = 0;
    let mut activevarssize = *nvars;
    let mut ntmpvars = *nvars;
    let mut tmpvarssize = *nvars;
    let mut activevars: CVec<VarId> = CVec::with_size(activevarssize);
    let mut tmpvars: CVec<VarId> = CVec(vars.head(ntmpvars).to_vec());
    let mut noldtmpvars = ntmpvars;

    // Sort all variables to remove duplicates.
    sort_vars(scip, tmpvars.head_mut(ntmpvars));
    let mut v = ntmpvars - 1;
    while v > 0 {
        // Remove duplicates.
        if var_compare(scip, tmpvars[v], tmpvars[v - 1]) == 0 {
            ntmpvars -= 1;
            tmpvars[v] = tmpvars[ntmpvars];
        }
        v -= 1;
    }

    // Sort the variables again if duplicates were removed.
    if noldtmpvars > ntmpvars {
        sort_vars(scip, tmpvars.head_mut(ntmpvars));
    }

    // Collect for each variable the representation in active variables.
    while ntmpvars >= 1 {
        ntmpvars -= 1;
        let var = tmpvars[ntmpvars];
        match scip.vars[var].varstatus {
            VarStatus::Original => {
                let transvar = scip.vars[var].data.original.transvar;
                if transvar == NONE {
                    log::error!("original variable has no transformed variable attached");
                    return Err(Retcode::InvalidData);
                }
                tmpvars[ntmpvars] = transvar;
                ntmpvars += 1;
            }
            VarStatus::Aggregated => {
                tmpvars[ntmpvars] = scip.vars[var].data.aggregate.var;
                ntmpvars += 1;
            }
            VarStatus::Negated => {
                tmpvars[ntmpvars] = scip.vars[var].negatedvar;
                ntmpvars += 1;
            }
            VarStatus::Loose | VarStatus::Column => {
                // Check for space in the activevars array.
                if nactivevars >= activevarssize {
                    activevarssize *= 2;
                    activevars.realloc(activevarssize);
                }
                activevars[nactivevars] = var;
                nactivevars += 1;
            }
            VarStatus::MultAggr => {
                // Append all multi-aggregated variables to the tmpvars
                // array.
                let nmultvars = scip.vars[var].data.multaggr.nvars;
                if nmultvars + ntmpvars > tmpvarssize {
                    while nmultvars + ntmpvars > tmpvarssize {
                        tmpvarssize *= 2;
                    }
                    tmpvars.realloc(tmpvarssize);
                }

                // Copy all multi-aggregation variables into our working
                // array and replace them by their active representatives.
                for k in 0..nmultvars {
                    tmpvars[ntmpvars + k] = var_get_probvar(scip, scip.vars[var].data.multaggr.vars[k]);
                }
                ntmpvars += nmultvars;
                noldtmpvars = ntmpvars;

                // Sort all variables to remove duplicates.
                sort_vars(scip, tmpvars.head_mut(ntmpvars));
                let mut v = ntmpvars - 1;
                while v > 0 {
                    if var_compare(scip, tmpvars[v], tmpvars[v - 1]) == 0 {
                        ntmpvars -= 1;
                        tmpvars[v] = tmpvars[ntmpvars];
                    }
                    v -= 1;
                }
                if noldtmpvars > ntmpvars {
                    sort_vars(scip, tmpvars.head_mut(ntmpvars));
                }
            }
            VarStatus::Fixed => {
                // No need for memorizing fixed variables.
            }
        }
    }

    // Sort variable array by variable index.
    sort_vars(scip, activevars.head_mut(nactivevars));

    // Eliminate duplicates and count required size.
    let mut v = nactivevars - 1;
    while v > 0 {
        if var_compare(scip, activevars[v - 1], activevars[v]) == 0 {
            nactivevars -= 1;
            activevars[v] = activevars[nactivevars];
        }
        v -= 1;
    }
    *requiredsize = nactivevars;
    if varssize >= *requiredsize {
        *nvars = *requiredsize;
        for k in 0..nactivevars {
            vars[k] = activevars[k];
        }
    }
    Ok(())
}

/// `SCIPvarsGetProbvar`.
pub fn vars_get_probvar(scip: &Scip, vars: &mut [VarId]) {
    for v in vars.iter_mut().rev() {
        *v = var_get_probvar(scip, *v);
    }
}

/// `SCIPvarGetProbvar`: the active, fixed or multi-aggregated variable the
/// variable resolves to.
pub fn var_get_probvar(scip: &Scip, var: VarId) -> VarId {
    let mut retvar = var;
    loop {
        let v = &scip.vars[retvar];
        match v.varstatus {
            VarStatus::Original => {
                if v.data.original.transvar == NONE {
                    log::error!("original variable has no transformed variable attached");
                    return NONE;
                }
                retvar = v.data.original.transvar;
            }
            VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => return retvar,
            VarStatus::MultAggr => {
                // Handle multi-aggregated variables depending on one
                // variable only (possibly caused by SCIPvarFlattenAggregationGraph()).
                if v.data.multaggr.nvars == 1 {
                    retvar = v.data.multaggr.vars[0];
                } else {
                    return retvar;
                }
            }
            VarStatus::Aggregated => retvar = v.data.aggregate.var,
            VarStatus::Negated => retvar = v.negatedvar,
        }
    }
}

/// `SCIPvarsGetProbvarBinary`.
pub fn vars_get_probvar_binary(scip: &Scip, vars: &mut [VarId], negated: &mut [bool]) -> ScipResult {
    for v in (0..vars.len()).rev() {
        var_get_probvar_binary(scip, &mut vars[v], &mut negated[v])?;
    }
    Ok(())
}

/// `SCIPvarGetProbvarBinary`: the active binary variable and negation
/// status a binary variable resolves to.
pub fn var_get_probvar_binary(scip: &Scip, var: &mut VarId, negated: &mut bool) -> ScipResult {
    let mut active = false;
    while !active && *var != NONE {
        let v = &scip.vars[*var];
        match v.varstatus {
            VarStatus::Original => {
                if v.data.original.transvar == NONE {
                    return Ok(());
                }
                *var = v.data.original.transvar;
            }
            VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => active = true,
            VarStatus::MultAggr => {
                // Handle multi-aggregated variables depending on one
                // variable only (possibly caused by SCIPvarFlattenAggregationGraph()).
                let m = &v.data.multaggr;
                if m.nvars == 1 {
                    // If not all variables were fully propagated, a variable
                    // can be multi-aggregated to another that needs fixing
                    // (x = y - 1 or x = y + 1): return the multi-aggregation.
                    if eps_eq(m.constant, -1.0, 1e-06) || (eps_eq(m.constant, 1.0, 1e-06) && eps_eq(m.scalars[0], 1.0, 1e-06)) {
                        active = true;
                    } else if !eps_eq(m.scalars[0_usize].abs(), 1.0, 1e-06) {
                        // A scalar of absolute value other than one: the
                        // aggregated variable needs fixing elsewhere.
                        active = true;
                    } else if !eps_z(m.constant, 1e-06) && !eps_eq(m.constant, 1.0, 1e-06) {
                        // A constant other than 0 or 1: likewise.
                        active = true;
                    } else {
                        if !eps_z(m.constant, 1e-06) {
                            *negated = !*negated;
                        }
                        *var = m.vars[0];
                    }
                } else {
                    active = true;
                }
            }
            VarStatus::Aggregated => {
                // x = a*y + c, binary: a = 1, c = 0 or a = -1, c = 1.
                *negated = if v.data.aggregate.scalar > 0.0 { *negated } else { !*negated };
                *var = v.data.aggregate.var;
            }
            VarStatus::Negated => {
                *negated = !*negated;
                *var = v.negatedvar;
            }
        }
    }
    if active {
        Ok(())
    } else {
        log::error!("active variable path leads to NULL pointer");
        Err(Retcode::InvalidData)
    }
}

/// `SCIPvarGetProbvarBound`: transforms the bound `var >=/<= bound` into
/// one on the active variable.
pub fn var_get_probvar_bound(scip: &Scip, var: &mut VarId, bound: &mut f64, boundtype: &mut BoundType) -> ScipResult {
    let flip = |t: &mut BoundType| *t = if *t == BoundType::Lower { BoundType::Upper } else { BoundType::Lower };
    let v = &scip.vars[*var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                log::error!("original variable has no transformed variable attached");
                return Err(Retcode::InvalidData);
            }
            *var = v.data.original.transvar;
            var_get_probvar_bound(scip, var, bound, boundtype)?;
        }
        VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => {}
        VarStatus::MultAggr => {
            // Handle multi-aggregated variables depending on one variable
            // only (possibly caused by SCIPvarFlattenAggregationGraph()).
            let m = &v.data.multaggr;
            if m.nvars == 1 {
                *bound /= m.scalars[0];
                *bound -= m.constant / m.scalars[0];
                if m.scalars[0] < 0.0 {
                    flip(boundtype);
                }
                *var = m.vars[0];
                var_get_probvar_bound(scip, var, bound, boundtype)?;
            }
        }
        VarStatus::Aggregated => {
            // x = a*y + c  ->  y = x/a - c/a.
            let a = v.data.aggregate;
            *bound /= a.scalar;
            *bound -= a.constant / a.scalar;
            if a.scalar < 0.0 {
                flip(boundtype);
            }
            *var = a.var;
            var_get_probvar_bound(scip, var, bound, boundtype)?;
        }
        VarStatus::Negated => {
            // x = offset - x'  ->  x' = offset - x.
            *bound = v.data.negate.constant - *bound;
            flip(boundtype);
            *var = v.negatedvar;
            var_get_probvar_bound(scip, var, bound, boundtype)?;
        }
    }
    Ok(())
}

/// `SCIPvarGetProbvarSum`: transforms `scalar * var + constant` into the
/// sum over the active (or fixed or multi-aggregated) variable; for a
/// fixed variable, `scalar` becomes 0 and its value goes into `constant`.
pub fn var_get_probvar_sum(scip: &Scip, var: &mut VarId, scalar: &mut f64, constant: &mut f64) -> ScipResult {
    while *var != NONE {
        let v = &scip.vars[*var];
        match v.varstatus {
            VarStatus::Original => {
                if v.data.original.transvar == NONE {
                    log::error!("original variable has no transformed variable attached");
                    return Err(Retcode::InvalidData);
                }
                *var = v.data.original.transvar;
            }
            VarStatus::Loose | VarStatus::Column => return Ok(()),
            VarStatus::Fixed => {
                // x = c'  =>  a*x + c ==             (a*c' + c)
                if !scip.set.is_infinity(*constant) && !scip.set.is_infinity(-*constant) {
                    let glb = v.glbdom.lb;
                    if scip.set.is_infinity(glb) || scip.set.is_infinity(-glb) {
                        *constant = if *scalar * glb > 0.0 { scip.set.infinity() } else { -scip.set.infinity() };
                    } else {
                        *constant += *scalar * glb;
                    }
                }
                *scalar = 0.0;
                return Ok(());
            }
            VarStatus::MultAggr => {
                // Handle multi-aggregated variables depending on one
                // variable only (possibly caused by SCIPvarFlattenAggregationGraph()).
                let m = &v.data.multaggr;
                if m.nvars == 1 {
                    if !scip.set.is_infinity(*constant) && !scip.set.is_infinity(-*constant) {
                        if scip.set.is_infinity(m.constant) || scip.set.is_infinity(-m.constant) {
                            *constant = if *scalar * m.constant > 0.0 { scip.set.infinity() } else { -scip.set.infinity() };
                            *scalar = 0.0;
                        } else {
                            *constant += *scalar * m.constant;
                        }
                    }
                    *scalar *= m.scalars[0];
                    *var = m.vars[0];
                } else {
                    return Ok(());
                }
            }
            VarStatus::Aggregated => {
                // x = a'*x' + c'  =>  a*x + c == (a*a')*x' + (a*c' + c).
                let a = v.data.aggregate;
                if !scip.set.is_infinity(*constant) && !scip.set.is_infinity(-*constant) {
                    *constant += *scalar * a.constant;
                }
                *scalar *= a.scalar;
                *var = a.var;
            }
            VarStatus::Negated => {
                // x = - x' + c'  =>  a*x + c ==   (-a)*x' + (a*c' + c).
                if !scip.set.is_infinity(*constant) && !scip.set.is_infinity(-*constant) {
                    *constant += *scalar * v.data.negate.constant;
                }
                *scalar *= -1.0;
                *var = v.negatedvar;
            }
        }
    }
    *scalar = 0.0;
    Ok(())
}

/// `SCIPvarGetOrigvarSum`: transforms `scalar * var + constant` of a
/// transformed variable into one over the original variable it came from,
/// or sets `var` to `NONE` if there is none.
pub fn var_get_origvar_sum(scip: &Scip, var: &mut VarId, scalar: &mut f64, constant: &mut f64) -> ScipResult {
    while !var_is_original(scip, *var) {
        let v = &scip.vars[*var];

        // If the variable has no parent variables, it was generated during
        // solving and has no corresponding original variable.
        if v.nparentvars == 0 {
            // The variable is a negated one whose negation has no parents:
            // follow the negation.
            if v.varstatus == VarStatus::Negated && (scip.vars[v.negatedvar].nparentvars == 0 || scip.vars[v.negatedvar].parentvars[0] != *var) {
                *scalar *= -1.0;
                *constant -= v.data.negate.constant * *scalar;
                *var = v.negatedvar;
                continue;
            } else {
                // Original variable is not existing.
                *var = NONE;
                return Ok(());
            }
        }

        // Follow the link to the first parent variable.
        let parentvar = v.parentvars[0];
        let p = &scip.vars[parentvar];
        match p.varstatus {
            VarStatus::Original => {}
            VarStatus::Column | VarStatus::Loose | VarStatus::Fixed | VarStatus::MultAggr => {
                log::error!("column, loose, fixed or multi-aggregated variable cannot be the parent of a variable");
                return Err(Retcode::InvalidData);
            }
            VarStatus::Aggregated => {
                // x = a*y + c  ->  y = (x-c)/a.
                *scalar /= p.data.aggregate.scalar;
                *constant -= p.data.aggregate.constant * *scalar;
            }
            VarStatus::Negated => {
                // x = b - y  ->  y = b - x.
                *scalar *= -1.0;
                *constant -= p.data.negate.constant * *scalar;
            }
        }
        *var = parentvar;
    }
    Ok(())
}

/// `SCIPvarIsTransformedOrigvar`: whether the variable is the direct
/// transformation of an original variable.
pub fn var_is_transformed_origvar(scip: &Scip, var: VarId) -> bool {
    if !var_is_transformed(scip, var) || scip.vars[var].nparentvars < 1 {
        return false;
    }
    let mut parentvar = scip.vars[var].parentvars[0];

    // We follow the aggregation tree to the root unless an original
    // variable has been found - the first entries in the parentlist are
    // candidates.
    while scip.vars[parentvar].nparentvars >= 1 && scip.vars[parentvar].varstatus != VarStatus::Original {
        parentvar = scip.vars[parentvar].parentvars[0];
    }
    scip.vars[parentvar].varstatus == VarStatus::Original
}

/// `SCIPvarGetObjLP`: the objective coefficient in the current (diving)
/// LP.
pub fn var_get_obj_lp(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => var_get_obj_lp(scip, v.data.original.transvar),
        VarStatus::Column => scip.cols[v.data.col].obj,
        VarStatus::Loose | VarStatus::Fixed => v.obj,
        VarStatus::Aggregated => v.data.aggregate.scalar * var_get_obj_lp(scip, v.data.aggregate.var),
        VarStatus::MultAggr => panic!("cannot get the objective value of a multiple aggregated variable"),
        VarStatus::Negated => -var_get_obj_lp(scip, v.negatedvar),
    }
}

/// `SCIPvarGetLbLP`.
pub fn var_get_lb_lp(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => var_get_lb_lp(scip, v.data.original.transvar),
        VarStatus::Column => scip.cols[v.data.col].lb,
        VarStatus::Loose | VarStatus::Fixed => v.locdom.lb,
        VarStatus::Aggregated => {
            let a = v.data.aggregate;
            if (a.scalar > 0.0 && scip.set.is_infinity(-var_get_lb_lp(scip, a.var))) || (a.scalar < 0.0 && scip.set.is_infinity(var_get_ub_lp(scip, a.var))) {
                -scip.set.infinity()
            } else if a.scalar > 0.0 {
                a.scalar * var_get_lb_lp(scip, a.var) + a.constant
            } else if a.scalar < 0.0 {
                a.scalar * var_get_ub_lp(scip, a.var) + a.constant
            } else {
                panic!("scalar is zero in aggregation");
            }
        }
        VarStatus::MultAggr => panic!("getting the bounds of a multiple aggregated variable is not implemented yet"),
        VarStatus::Negated => v.data.negate.constant - var_get_ub_lp(scip, v.negatedvar),
    }
}

/// `SCIPvarGetUbLP`.
pub fn var_get_ub_lp(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => var_get_ub_lp(scip, v.data.original.transvar),
        VarStatus::Column => scip.cols[v.data.col].ub,
        VarStatus::Loose | VarStatus::Fixed => v.locdom.ub,
        VarStatus::Aggregated => {
            let a = v.data.aggregate;
            if (a.scalar > 0.0 && scip.set.is_infinity(var_get_ub_lp(scip, a.var))) || (a.scalar < 0.0 && scip.set.is_infinity(-var_get_lb_lp(scip, a.var))) {
                return scip.set.infinity();
            }
            if a.scalar > 0.0 {
                a.scalar * var_get_ub_lp(scip, a.var) + a.constant
            } else if a.scalar < 0.0 {
                a.scalar * var_get_lb_lp(scip, a.var) + a.constant
            } else {
                panic!("scalar is zero in aggregation");
            }
        }
        VarStatus::MultAggr => panic!("cannot get the bounds of a multi-aggregated variable."),
        VarStatus::Negated => v.data.negate.constant - var_get_lb_lp(scip, v.negatedvar),
    }
}

/// `SCIPvarGetBestBoundLocal`.
#[inline]
pub fn var_get_best_bound_local(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.obj >= 0.0 { v.locdom.lb } else { v.locdom.ub }
}

/// `SCIPvarGetWorstBoundLocal`.
#[inline]
pub fn var_get_worst_bound_local(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.obj >= 0.0 { v.locdom.ub } else { v.locdom.lb }
}

/// `SCIPvarGetBestBoundGlobal`.
#[inline]
pub fn var_get_best_bound_global(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.obj >= 0.0 { v.glbdom.lb } else { v.glbdom.ub }
}

/// `SCIPvarGetWorstBoundGlobal`.
#[inline]
pub fn var_get_worst_bound_global(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.obj >= 0.0 { v.glbdom.ub } else { v.glbdom.lb }
}

/// `SCIPvarGetBestBoundType`.
#[inline]
pub fn var_get_best_bound_type(scip: &Scip, var: VarId) -> BoundType {
    if scip.vars[var].obj >= 0.0 { BoundType::Lower } else { BoundType::Upper }
}

/// `SCIPvarGetWorstBoundType`.
#[inline]
pub fn var_get_worst_bound_type(scip: &Scip, var: VarId) -> BoundType {
    if scip.vars[var].obj >= 0.0 { BoundType::Upper } else { BoundType::Lower }
}

/// `SCIPcolGetPrimsol` (a macro): 0 for columns not in the LP.
#[inline]
fn col_primsol(scip: &Scip, col: ColId) -> f64 {
    let c = &scip.cols[col];
    if c.lppos >= 0 { c.primsol } else { 0.0 }
}

/// `SCIPvarGetLPSol`.
#[inline]
pub fn var_get_lp_sol(scip: &Scip, var: VarId) -> f64 {
    if scip.vars[var].varstatus == VarStatus::Column {
        col_primsol(scip, scip.vars[var].data.col)
    } else {
        var_get_lp_sol_rec(scip, var)
    }
}

/// `SCIPvarGetLPSol_rec`.
pub fn var_get_lp_sol_rec(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                return INVALID;
            }
            var_get_lp_sol(scip, v.data.original.transvar)
        }
        VarStatus::Loose => var_get_best_bound_local(scip, var),
        VarStatus::Column => col_primsol(scip, v.data.col),
        VarStatus::Fixed => v.locdom.lb,
        VarStatus::Aggregated => {
            let a = v.data.aggregate;
            let lpsolval = var_get_lp_sol(scip, a.var);
            if lpsolval >= DEFAULT_INFINITY {
                return if a.scalar > 0.0 { DEFAULT_INFINITY } else { -DEFAULT_INFINITY };
            } else if lpsolval <= -DEFAULT_INFINITY {
                return if a.scalar > 0.0 { -DEFAULT_INFINITY } else { DEFAULT_INFINITY };
            }
            a.scalar * lpsolval + a.constant
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut primsol = m.constant;
            for i in 0..m.nvars {
                primsol += m.scalars[i] * var_get_lp_sol(scip, m.vars[i]);
            }
            primsol
        }
        VarStatus::Negated => v.data.negate.constant - var_get_lp_sol(scip, v.negatedvar),
    }
}

/// `SCIPvarGetNLPSol`.
#[inline]
pub fn var_get_nlp_sol(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.varstatus == VarStatus::Column || v.varstatus == VarStatus::Loose {
        v.nlpsol
    } else {
        var_get_nlp_sol_rec(scip, var)
    }
}

/// `SCIPvarGetNLPSol_rec`.
pub fn var_get_nlp_sol_rec(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => var_get_nlp_sol(scip, v.data.original.transvar),
        VarStatus::Loose | VarStatus::Column => v.nlpsol,
        VarStatus::Fixed => v.glbdom.lb,
        VarStatus::Aggregated => {
            let solval = var_get_nlp_sol(scip, v.data.aggregate.var);
            v.data.aggregate.scalar * solval + v.data.aggregate.constant
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut solval = m.constant;
            for i in 0..m.nvars {
                solval += m.scalars[i] * var_get_nlp_sol(scip, m.vars[i]);
            }
            solval
        }
        VarStatus::Negated => {
            let solval = var_get_nlp_sol(scip, v.negatedvar);
            v.data.negate.constant - solval
        }
    }
}

/// `SCIPvarGetPseudoSol`.
#[inline]
pub fn var_get_pseudo_sol(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.varstatus == VarStatus::Loose || v.varstatus == VarStatus::Column {
        var_get_best_bound_local(scip, var)
    } else {
        var_get_pseudo_sol_rec(scip, var)
    }
}

/// `SCIPvarGetPseudoSol_rec`.
pub fn var_get_pseudo_sol_rec(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                return INVALID;
            }
            var_get_pseudo_sol(scip, v.data.original.transvar)
        }
        VarStatus::Loose | VarStatus::Column => var_get_best_bound_local(scip, var),
        VarStatus::Fixed => v.locdom.lb,
        VarStatus::Aggregated => {
            let pseudosolval = var_get_pseudo_sol(scip, v.data.aggregate.var);
            v.data.aggregate.scalar * pseudosolval + v.data.aggregate.constant
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut pseudosol = m.constant;
            for i in 0..m.nvars {
                pseudosol += m.scalars[i] * var_get_pseudo_sol(scip, m.vars[i]);
            }
            pseudosol
        }
        VarStatus::Negated => v.data.negate.constant - var_get_pseudo_sol(scip, v.negatedvar),
    }
}

/// `SCIPvarGetSol`.
pub fn var_get_sol(scip: &Scip, var: VarId, getlpval: bool) -> f64 {
    if getlpval { var_get_lp_sol(scip, var) } else { var_get_pseudo_sol(scip, var) }
}

/// `SCIPvarStoreRootSol`.
pub fn var_store_root_sol(scip: &mut Scip, var: VarId, roothaslp: bool) {
    scip.vars[var].rootsol = var_get_sol(scip, var, roothaslp);
}

/// `SCIPvarUpdateBestRootSol`: keeps the root solution and reduced cost
/// that give the best cutoff bound.
pub fn var_update_best_root_sol(scip: &mut Scip, var: VarId, rootsol: f64, rootredcost: f64, rootlpobjval: f64) {
    // If reduced cost are zero nothing to update.
    if scip.set.is_dualfeas_zero(rootredcost) {
        return;
    }

    // Check if we have already a best combination stored.
    let v = &scip.vars[var];
    if !scip.set.is_dualfeas_zero(v.bestrootredcost) {
        // Compute the cutoff bound which would improve the corresponding
        // bound with the current best root LP solution.
        let bound = if v.bestrootredcost > 0.0 { v.glbdom.ub } else { v.glbdom.lb };
        let currcutoffbound = (bound - v.bestrootsol) * v.bestrootredcost + v.bestrootlpobjval;

        // Compute the cutoff bound which would improve the corresponding
        // bound with new root LP solution.
        let bound = if rootredcost > 0.0 { v.glbdom.ub } else { v.glbdom.lb };
        let cutoffbound = (bound - rootsol) * rootredcost + rootlpobjval;

        // Check if an improving root solution, root reduced cost, and root
        // LP objective value is at hand.
        if cutoffbound > currcutoffbound {
            let v = &mut scip.vars[var];
            v.bestrootsol = rootsol;
            v.bestrootredcost = rootredcost;
            v.bestrootlpobjval = rootlpobjval;
        }
    } else {
        // Store current root solution, root reduced cost, and root LP
        // objective value.
        let v = &mut scip.vars[var];
        v.bestrootsol = rootsol;
        v.bestrootredcost = rootredcost;
        v.bestrootlpobjval = rootlpobjval;
    }
}

/// `SCIPvarGetRootSol`.
pub fn var_get_root_sol(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                return 0.0;
            }
            var_get_root_sol(scip, v.data.original.transvar)
        }
        VarStatus::Loose | VarStatus::Column => v.rootsol,
        VarStatus::Fixed => v.locdom.lb,
        VarStatus::Aggregated => v.data.aggregate.scalar * var_get_root_sol(scip, v.data.aggregate.var) + v.data.aggregate.constant,
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut rootsol = m.constant;
            for i in 0..m.nvars {
                rootsol += m.scalars[i] * var_get_root_sol(scip, m.vars[i]);
            }
            rootsol
        }
        VarStatus::Negated => v.data.negate.constant - var_get_root_sol(scip, v.negatedvar),
    }
}

/// `getImplVarRedcost`: the reduced cost of the binary column at the bound
/// `varfixing` selects, 0 if it is not at that bound.
fn get_impl_var_redcost(scip: &mut Scip, var: VarId, varfixing: bool) -> f64 {
    if scip.vars[var].varstatus == VarStatus::Column {
        let col = scip.vars[var].data.col;
        let basestat = scip.cols[col].basisstatus;
        let lpissolbasic = crate::lp::lp_is_sol_basic(scip);
        let primsol = col_primsol(scip, col);
        let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);
        if (lpissolbasic && (basestat == crate::lpi::BASESTAT_LOWER || basestat == crate::lpi::BASESTAT_UPPER))
            || (!lpissolbasic && (scip.set.is_feas_eq(lb, primsol) || scip.set.is_feas_eq(ub, primsol)))
        {
            let redcost = crate::lp::col_get_redcost(scip, col);
            if (varfixing && ((lpissolbasic && basestat == crate::lpi::BASESTAT_LOWER) || (!lpissolbasic && scip.set.is_feas_eq(lb, primsol))))
                || (!varfixing && ((lpissolbasic && basestat == crate::lpi::BASESTAT_UPPER) || (!lpissolbasic && scip.set.is_feas_eq(ub, primsol))))
            {
                return redcost;
            } else {
                return 0.0;
            }
        }
        return 0.0;
    }
    0.0
}

const MAX_CLIQUELENGTH: i32 = 50;

/// `SCIPvarGetImplRedcost`: the reduced cost of fixing the binary `var`
/// to `varfixing`, including what its cliques and implications imply.
pub fn var_get_impl_redcost(scip: &mut Scip, var: VarId, varfixing: bool, prob: usize) -> f64 {
    let mut implredcost = get_impl_var_redcost(scip, var, varfixing);

    // Search for cliques the variable is in.
    let fix = varfixing as usize;
    let ncliques = scip.vars[var].cliquelist.as_ref().map_or(0, |cl| cl.ncliques[fix]);
    if ncliques > 0 {
        let nentries = scip.probs[prob].nvars - scip.probs[prob].ncontvars + 1;
        let mut ids: CVec<i32> = CVec::with_size(nentries);
        let mut nids = 0;
        let mut entries: CVec<i32> = CVec::with_size(nentries);
        let cliques = &scip.vars[var].cliquelist.as_ref().unwrap().cliques[fix];

        // Iterate over all cliques and determine all importantimplications.
        let mut c = ncliques - 1;
        while c >= 0 {
            let clique = &scip.cliques[cliques[c]];
            let nclqvars = clique.nvars;

            // Skip cliques that are too large.
            if nclqvars > MAX_CLIQUELENGTH {
                c -= 1;
                continue;
            }
            let cleanedup = crate::implics::clique_is_cleaned_up(clique);
            let mut v = nclqvars - 1;
            while v >= 0 {
                let clqvar = clique.vars[v];

                // We need to consider the active representation of each
                // variable, and only unfixed ones.
                if clqvar != var && (cleanedup || var_is_active(scip, clqvar)) && (scip.vars[clqvar].locdom.lb < 0.5 && scip.vars[clqvar].locdom.ub > 0.5) {
                    let probindex = scip.vars[clqvar].probindex + 1;

                    // Check that we only count the reduced cost of each
                    // variable once.
                    if entries[probindex] == 0 {
                        ids[nids] = probindex;
                        nids += 1;

                        // Mark variable as visited.
                        entries[probindex] = if clique.values[v] { probindex } else { -probindex };
                    }
                }
                v -= 1;
            }
            c -= 1;
        }
        // Add all implied reduced cost.
        let mut v = nids - 1;
        while v >= 0 {
            let id = ids[v];
            let probvar = scip.probs[prob].vars[id - 1];
            let redcost = if (entries[id] > 0) != varfixing {
                get_impl_var_redcost(scip, probvar, entries[id] < 0)
            } else {
                -get_impl_var_redcost(scip, probvar, entries[id] < 0)
            };
            if (varfixing && scip.set.is_dualfeas_positive(redcost)) || (!varfixing && scip.set.is_dualfeas_negative(redcost)) {
                implredcost += redcost;
            }

            // Reset entries clear buffer array.
            entries[id] = 0;
            v -= 1;
        }
    }

    // Collect non-binary implication information.
    let nvars = scip.vars[var].implics.as_ref().map_or(0, |im| im.nimpls[fix]);
    if nvars > 0 {
        let im = scip.vars[var].implics.as_ref().unwrap();
        let (vars, boundtypes, bounds) = (
            CVec(im.vars[fix].head(nvars).to_vec()),
            CVec(im.types[fix].head(nvars).to_vec()),
            CVec(im.bounds[fix].head(nvars).to_vec()),
        );
        let lpissolbasic = crate::lp::lp_is_sol_basic(scip);
        let mut v = nvars - 1;
        while v >= 0 {
            let implvar = vars[v];
            let lb = scip.vars[implvar].locdom.lb;
            let ub = scip.vars[implvar].locdom.ub;

            // Ignore binary variable which are fixed or not of column
            // status.
            if scip.vars[implvar].varstatus != VarStatus::Column || scip.set.is_feas_eq(lb, ub) {
                v -= 1;
                continue;
            }
            let col = scip.vars[implvar].data.col;
            let mut redcost = 0.0;

            // Solution is a basic LP solution.
            if lpissolbasic {
                let basestat = scip.cols[col].basisstatus;
                if basestat == crate::lpi::BASESTAT_LOWER && boundtypes[v] == BoundType::Lower && scip.set.is_feas_gt(bounds[v], lb) {
                    redcost = crate::lp::col_get_redcost(scip, col);
                    if !varfixing {
                        redcost *= lb - bounds[v];
                    } else {
                        redcost *= bounds[v] - lb;
                    }
                } else if basestat == crate::lpi::BASESTAT_UPPER && boundtypes[v] == BoundType::Upper && scip.set.is_feas_lt(bounds[v], ub) {
                    redcost = crate::lp::col_get_redcost(scip, col);
                    if varfixing {
                        redcost *= bounds[v] - ub;
                    } else {
                        redcost *= ub - bounds[v];
                    }
                }
            } else {
                let primsol = col_primsol(scip, col);
                if boundtypes[v] == BoundType::Lower && scip.set.is_feas_eq(lb, primsol) && scip.set.is_feas_gt(bounds[v], lb) {
                    redcost = crate::lp::col_get_redcost(scip, col);
                    if varfixing {
                        redcost *= lb - bounds[v];
                    } else {
                        redcost *= bounds[v] - lb;
                    }
                } else if boundtypes[v] == BoundType::Upper && scip.set.is_feas_eq(ub, primsol) && scip.set.is_feas_lt(bounds[v], ub) {
                    redcost = crate::lp::col_get_redcost(scip, col);
                    if varfixing {
                        redcost *= bounds[v] - ub;
                    } else {
                        redcost *= ub - bounds[v];
                    }
                }
            }
            if (varfixing && scip.set.is_dualfeas_positive(redcost)) || (!varfixing && scip.set.is_dualfeas_negative(redcost)) {
                implredcost += redcost;
            }
            v -= 1;
        }
    }
    implredcost
}

/// `SCIPvarGetBestRootSol`.
pub fn var_get_best_root_sol(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                return 0.0;
            }
            var_get_best_root_sol(scip, v.data.original.transvar)
        }
        VarStatus::Loose | VarStatus::Column => v.bestrootsol,
        VarStatus::Fixed => v.locdom.lb,
        VarStatus::Aggregated => v.data.aggregate.scalar * var_get_best_root_sol(scip, v.data.aggregate.var) + v.data.aggregate.constant,
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut rootsol = m.constant;
            for i in 0..m.nvars {
                rootsol += m.scalars[i] * var_get_best_root_sol(scip, m.vars[i]);
            }
            rootsol
        }
        VarStatus::Negated => v.data.negate.constant - var_get_best_root_sol(scip, v.negatedvar),
    }
}

/// `SCIPvarGetBestRootRedcost`.
pub fn var_get_best_root_redcost(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                return INVALID;
            }
            var_get_best_root_redcost(scip, v.data.original.transvar)
        }
        VarStatus::Loose | VarStatus::Column => v.bestrootredcost,
        _ => 0.0,
    }
}

/// `SCIPvarGetBestRootLPObjval`.
pub fn var_get_best_root_lp_objval(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                return INVALID;
            }
            var_get_best_root_lp_objval(scip, v.data.original.transvar)
        }
        VarStatus::Loose | VarStatus::Column => v.bestrootlpobjval,
        _ => INVALID,
    }
}

/// `SCIPvarSetBestRootSol`.
pub fn var_set_best_root_sol(scip: &mut Scip, var: VarId, rootsol: f64, rootredcost: f64, rootlpobjval: f64) {
    let v = &mut scip.vars[var];
    v.bestrootsol = rootsol;
    v.bestrootredcost = rootredcost;
    v.bestrootlpobjval = rootlpobjval;
}

/// `SCIPvarSetRelaxSol`.
pub fn var_set_relax_sol(scip: &mut Scip, var: VarId, solval: f64, updateobj: bool) -> ScipResult {
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            var_set_relax_sol(scip, transvar, solval, updateobj)?;
        }
        VarStatus::Loose | VarStatus::Column => {
            if updateobj {
                let delta = scip.vars[var].obj * (solval - scip.vars[var].relaxsol);
                crate::relax::relaxation_sol_obj_add(scip, delta);
            }
            scip.vars[var].relaxsol = solval;
        }
        VarStatus::Fixed => {
            if !scip.set.is_eq(solval, scip.vars[var].glbdom.lb) {
                log::error!(
                    "cannot set relaxation solution value for variable <{}> fixed to {:.15} to different value {:.15}",
                    scip.vars[var].name,
                    scip.vars[var].glbdom.lb,
                    solval
                );
                return Err(Retcode::InvalidData);
            }
        }
        VarStatus::Aggregated => {
            // x = a*y + c  =>  y = (x-c)/a.
            let a = scip.vars[var].data.aggregate;
            var_set_relax_sol(scip, a.var, (solval - a.constant) / a.scalar, updateobj)?;
        }
        VarStatus::MultAggr => {
            log::error!("cannot set solution value for multiple aggregated variable");
            return Err(Retcode::InvalidData);
        }
        VarStatus::Negated => {
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_set_relax_sol(scip, negvar, constant - solval, updateobj)?;
        }
    }
    Ok(())
}

/// `SCIPvarGetRelaxSol`.
pub fn var_get_relax_sol(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => var_get_relax_sol(scip, v.data.original.transvar),
        VarStatus::Loose | VarStatus::Column => v.relaxsol,
        VarStatus::Fixed => v.glbdom.lb,
        VarStatus::Aggregated => {
            let a = v.data.aggregate;
            let solval = var_get_relax_sol(scip, a.var);
            if scip.set.is_infinity(solval) || scip.set.is_infinity(-solval) {
                if a.scalar * solval > 0.0 {
                    return scip.set.infinity();
                }
                if a.scalar * solval < 0.0 {
                    return -scip.set.infinity();
                }
            }
            a.scalar * solval + a.constant
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut solvalsum = m.constant;
            for i in 0..m.nvars {
                let solval = var_get_relax_sol(scip, m.vars[i]);
                if scip.set.is_infinity(solval) || scip.set.is_infinity(-solval) {
                    if m.scalars[i] * solval > 0.0 {
                        return scip.set.infinity();
                    }
                    if m.scalars[i] * solval < 0.0 {
                        return -scip.set.infinity();
                    }
                }
                solvalsum += m.scalars[i] * solval;
            }
            solvalsum
        }
        VarStatus::Negated => {
            let solval = var_get_relax_sol(scip, v.negatedvar);
            if scip.set.is_infinity(solval) {
                return -scip.set.infinity();
            }
            if scip.set.is_infinity(-solval) {
                return scip.set.infinity();
            }
            v.data.negate.constant - solval
        }
    }
}

/// `SCIPvarGetRelaxSolTransVar`.
pub fn var_get_relax_sol_trans_var(scip: &Scip, var: VarId) -> f64 {
    scip.vars[var].relaxsol
}

/// `SCIPvarSetNLPSol`.
pub fn var_set_nlp_sol(scip: &mut Scip, var: VarId, solval: f64) -> ScipResult {
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            var_set_nlp_sol(scip, transvar, solval)?;
        }
        VarStatus::Loose | VarStatus::Column => scip.vars[var].nlpsol = solval,
        VarStatus::Fixed => {
            if !scip.set.is_eq(solval, scip.vars[var].glbdom.lb) {
                log::error!(
                    "cannot set NLP solution value for variable <{}> fixed to {:.15} to different value {:.15}",
                    scip.vars[var].name,
                    scip.vars[var].glbdom.lb,
                    solval
                );
                return Err(Retcode::InvalidCall);
            }
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            var_set_nlp_sol(scip, a.var, (solval - a.constant) / a.scalar)?;
        }
        VarStatus::MultAggr => {
            log::error!("cannot set solution value for multiple aggregated variable");
            return Err(Retcode::InvalidCall);
        }
        VarStatus::Negated => {
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_set_nlp_sol(scip, negvar, constant - solval)?;
        }
    }
    Ok(())
}

/// `SCIPvarGetAvgSol`: the average of the variable over the primal
/// solutions found, within its global bounds.
pub fn var_get_avg_sol(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                return 0.0;
            }
            var_get_avg_sol(scip, v.data.original.transvar)
        }
        VarStatus::Loose | VarStatus::Column => {
            let mut avgsol = v.primsolavg;
            avgsol = max(avgsol, v.glbdom.lb);
            avgsol = min(avgsol, v.glbdom.ub);
            avgsol
        }
        VarStatus::Fixed => v.locdom.lb,
        VarStatus::Aggregated => v.data.aggregate.scalar * var_get_avg_sol(scip, v.data.aggregate.var) + v.data.aggregate.constant,
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut avgsol = m.constant;
            for i in 0..m.nvars {
                avgsol += m.scalars[i] * var_get_avg_sol(scip, m.vars[i]);
            }
            avgsol
        }
        VarStatus::Negated => v.data.negate.constant - var_get_avg_sol(scip, v.negatedvar),
    }
}

/// `SCIPvarGetClosestVlb` and `SCIPvarGetClosestVub`: the variable bound
/// closest to `sol` (the LP solution if `NONE`, cached per LP).
fn var_get_closest_vbound(scip: &mut Scip, var: VarId, sol: SolId, upper: bool, closest: &mut f64, closestidx: &mut i32) {
    *closestidx = -1;
    *closest = if upper { REAL_MAX } else { REAL_MIN };
    let vb = if upper { &scip.vars[var].vubs } else { &scip.vars[var].vlbs };
    let nvbs = vb.as_ref().map_or(0, |vb| vb.len);
    if nvbs > 0 {
        let vb = vb.as_ref().unwrap();
        let cachedidx = if upper { scip.vars[var].closestvubidx } else { scip.vars[var].closestvlbidx };
        if scip.vars[var].closestvblpcount == scip.stat.lpcount && cachedidx != -1 && sol == NONE {
            let i = cachedidx;
            *closestidx = i;
            *closest = vb.coefs[i] * var_get_lp_sol(scip, vb.vars[i]) + vb.constants[i];
        } else {
            // Search for the closest variable bound.
            for i in 0..nvbs {
                // Use only active variables for the variable bound.
                if var_is_active(scip, vb.vars[i]) {
                    let x = if sol == NONE {
                        var_get_lp_sol(scip, vb.vars[i])
                    } else {
                        crate::sol::sol_get_val(scip, sol, vb.vars[i])
                    };
                    let vbsol = vb.coefs[i] * x + vb.constants[i];
                    if (upper && vbsol < *closest) || (!upper && vbsol > *closest) {
                        *closest = vbsol;
                        *closestidx = i;
                    }
                }
            }
            if sol == NONE {
                // Update cached value.
                let v = &mut scip.vars[var];
                if v.closestvblpcount != scip.stat.lpcount {
                    if upper {
                        v.closestvlbidx = -1;
                    } else {
                        v.closestvubidx = -1;
                    }
                }
                if upper {
                    v.closestvubidx = *closestidx;
                } else {
                    v.closestvlbidx = *closestidx;
                }
                v.closestvblpcount = scip.stat.lpcount;
            }
        }
    }
}

/// `SCIPvarGetClosestVlb`.
pub fn var_get_closest_vlb(scip: &mut Scip, var: VarId, sol: SolId, closestvlb: &mut f64, closestvlbidx: &mut i32) {
    var_get_closest_vbound(scip, var, sol, false, closestvlb, closestvlbidx);
}

/// `SCIPvarGetClosestVub`.
pub fn var_get_closest_vub(scip: &mut Scip, var: VarId, sol: SolId, closestvub: &mut f64, closestvubidx: &mut i32) {
    var_get_closest_vbound(scip, var, sol, true, closestvub, closestvubidx);
}

/// `SCIPvarAddToRow`: adds `val * var` to the row, turning a loose
/// variable into a column first.
pub fn var_add_to_row(scip: &mut Scip, var: VarId, prob: usize, row: crate::types::RowId, val: f64) -> ScipResult {
    if scip.set.is_zero(val) {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == NONE {
                log::error!("cannot add untransformed original variable <{}> to LP row <{}>", scip.vars[var].name, scip.rows[row].name);
                return Err(Retcode::InvalidData);
            }
            var_add_to_row(scip, transvar, prob, row, val)
        }
        VarStatus::Loose => {
            // Add globally fixed variables as constant.
            if scip.set.is_eq(scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub) {
                let constant = val * scip.vars[var].glbdom.lb;
                return crate::lp::row_add_constant(scip, row, constant);
            }

            // Convert loose variable into column.
            var_column(scip, var, prob)?;

            // Add column to row.
            let col = scip.vars[var].data.col;
            crate::lp::row_inc_coef(scip, row, col, val)
        }
        VarStatus::Column => {
            let col = scip.vars[var].data.col;
            crate::lp::row_inc_coef(scip, row, col, val)
        }
        VarStatus::Fixed => {
            let constant = val * scip.vars[var].locdom.lb;
            crate::lp::row_add_constant(scip, row, constant)
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            var_add_to_row(scip, a.var, prob, row, a.scalar * val)?;
            crate::lp::row_add_constant(scip, row, a.constant * val)
        }
        VarStatus::MultAggr => {
            for i in 0..scip.vars[var].data.multaggr.nvars {
                let (mvar, mscalar) = (scip.vars[var].data.multaggr.vars[i], scip.vars[var].data.multaggr.scalars[i]);
                var_add_to_row(scip, mvar, prob, row, mscalar * val)?;
            }
            let constant = scip.vars[var].data.multaggr.constant;
            crate::lp::row_add_constant(scip, row, constant * val)
        }
        VarStatus::Negated => {
            let (negvar, constant) = (scip.vars[var].negatedvar, scip.vars[var].data.negate.constant);
            var_add_to_row(scip, negvar, prob, row, -val)?;
            crate::lp::row_add_constant(scip, row, constant * val)
        }
    }
}

/// `SCIPvarUpdatePseudocost`.
pub fn var_update_pseudocost(scip: &mut Scip, var: VarId, solvaldelta: f64, objdelta: f64, weight: f64) -> ScipResult {
    // Check if history statistics should be collected for a variable.
    if !scip.stat.collectvarhistory {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == NONE {
                log::error!("cannot update pseudo costs of original untransformed variable");
                return Err(Retcode::InvalidData);
            }
            var_update_pseudocost(scip, transvar, solvaldelta, objdelta, weight)
        }
        VarStatus::Loose | VarStatus::Column => {
            let rootsol = var_get_root_sol(scip, var);
            let oldrootpseudocosts = var_get_min_pseudocost_score(scip, var, rootsol);

            // Update histories.
            let Scip { vars, stat, set, .. } = scip;
            vars[var].history.update_pseudocost(set, solvaldelta, objdelta, weight);
            vars[var].historycrun.update_pseudocost(set, solvaldelta, objdelta, weight);
            stat.glbhistory.update_pseudocost(set, solvaldelta, objdelta, weight);
            stat.glbhistorycrun.update_pseudocost(set, solvaldelta, objdelta, weight);

            // Update root LP best-estimate.
            crate::stat::stat_update_var_root_lp_best_estimate(scip, var, oldrootpseudocosts)
        }
        VarStatus::Fixed => {
            log::error!("cannot update pseudo cost values of a fixed variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            var_update_pseudocost(scip, a.var, solvaldelta / a.scalar, objdelta, weight)
        }
        VarStatus::MultAggr => {
            log::error!("cannot update pseudo cost values of a multi-aggregated variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Negated => {
            let negvar = scip.vars[var].negatedvar;
            var_update_pseudocost(scip, negvar, -solvaldelta, objdelta, weight)
        }
    }
}

/// `SCIPvarUpdateAncPseudocost`.
pub fn var_update_anc_pseudocost(scip: &mut Scip, var: VarId, solvaldelta: f64, objdelta: f64, weight: f64) -> ScipResult {
    if !scip.stat.collectvarhistory {
        return Ok(());
    }
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == NONE {
                log::error!("cannot update ancestral pseudo costs of original untransformed variable");
                return Err(Retcode::InvalidData);
            }
            var_update_anc_pseudocost(scip, transvar, solvaldelta, objdelta, weight)
        }
        VarStatus::Loose | VarStatus::Column => {
            let Scip { vars, stat, set, .. } = scip;
            vars[var].history.update_anc_pseudocost(set, solvaldelta, objdelta, weight);
            vars[var].historycrun.update_anc_pseudocost(set, solvaldelta, objdelta, weight);
            stat.glbhistory.update_anc_pseudocost(set, solvaldelta, objdelta, weight);
            stat.glbhistorycrun.update_anc_pseudocost(set, solvaldelta, objdelta, weight);
            Ok(())
        }
        VarStatus::Fixed => {
            log::error!("cannot update ancestral pseudo cost values of a fixed variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            var_update_anc_pseudocost(scip, a.var, solvaldelta / a.scalar, objdelta, weight)
        }
        VarStatus::MultAggr => {
            log::error!("cannot update ancestral pseudo cost values of a multi-aggregated variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Negated => {
            let negvar = scip.vars[var].negatedvar;
            var_update_anc_pseudocost(scip, negvar, -solvaldelta, objdelta, weight)
        }
    }
}

/// The pseudocost getters (`SCIPvarGetPseudocost`, `...CurrentRun`,
/// `SCIPvarGetAncPseudocost`): the variable's own value if it has an
/// observation in that direction, else the global one; through
/// aggregations the solution delta is scaled.
fn var_get_pseudocost_impl(scip: &Scip, var: VarId, solvaldelta: f64, crun: bool, anc: bool) -> f64 {
    let v = &scip.vars[var];
    let glb = if crun { &scip.stat.glbhistorycrun } else { &scip.stat.glbhistory };
    let get = |h: &History, d: f64| if anc { h.get_anc_pseudocost(d) } else { h.get_pseudocost(d) };
    let count = |h: &History, dir: BranchDir| if anc { h.get_anc_pseudocost_count(dir) } else { h.get_pseudocost_count(dir) };
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                get(glb, solvaldelta)
            } else {
                var_get_pseudocost_impl(scip, v.data.original.transvar, solvaldelta, crun, anc)
            }
        }
        VarStatus::Loose | VarStatus::Column => {
            let dir = if solvaldelta >= 0.0 { BranchDir::Upwards } else { BranchDir::Downwards };
            let h: &History = if crun { &v.historycrun } else { &v.history };
            if count(h, dir) > 0.0 { get(h, solvaldelta) } else { get(glb, solvaldelta) }
        }
        VarStatus::Fixed => 0.0,
        VarStatus::Aggregated => var_get_pseudocost_impl(scip, v.data.aggregate.var, v.data.aggregate.scalar * solvaldelta, crun, anc),
        VarStatus::MultAggr => 0.0,
        VarStatus::Negated => var_get_pseudocost_impl(scip, v.negatedvar, -solvaldelta, crun, anc),
    }
}

/// `SCIPvarGetPseudocost`.
pub fn var_get_pseudocost(scip: &Scip, var: VarId, solvaldelta: f64) -> f64 {
    var_get_pseudocost_impl(scip, var, solvaldelta, false, false)
}

/// `SCIPvarGetAncPseudocost`.
pub fn var_get_anc_pseudocost(scip: &Scip, var: VarId, solvaldelta: f64) -> f64 {
    var_get_pseudocost_impl(scip, var, solvaldelta, false, true)
}

/// `SCIPvarGetPseudocostCurrentRun`.
pub fn var_get_pseudocost_current_run(scip: &Scip, var: VarId, solvaldelta: f64) -> f64 {
    var_get_pseudocost_impl(scip, var, solvaldelta, true, false)
}

/// The direction-indexed history getters: the variable's history (or
/// current-run history) value, `origdefault` for an untransformed original
/// variable, 0 for fixed and multi-aggregated ones, and the opposite
/// direction through negative aggregations and negations.
fn var_history_get<T: Copy>(scip: &Scip, var: VarId, dir: BranchDir, crun: bool, origdefault: T, zero: T, f: &dyn Fn(&History, BranchDir) -> T) -> T {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                origdefault
            } else {
                var_history_get(scip, v.data.original.transvar, dir, crun, origdefault, zero, f)
            }
        }
        VarStatus::Loose | VarStatus::Column => f(if crun { &v.historycrun } else { &v.history }, dir),
        VarStatus::Fixed | VarStatus::MultAggr => zero,
        VarStatus::Aggregated => {
            let d = if v.data.aggregate.scalar > 0.0 { dir } else { branchdir_opposite(dir) };
            var_history_get(scip, v.data.aggregate.var, d, crun, origdefault, zero, f)
        }
        VarStatus::Negated => var_history_get(scip, v.negatedvar, branchdir_opposite(dir), crun, origdefault, zero, f),
    }
}

/// `SCIPvarGetPseudocostCount`.
pub fn var_get_pseudocost_count(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, false, 0.0, 0.0, &|h, d| h.get_pseudocost_count(d))
}

/// `SCIPvarGetPseudocostCountCurrentRun`.
pub fn var_get_pseudocost_count_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, true, 0.0, 0.0, &|h, d| h.get_pseudocost_count(d))
}

/// `SCIPvarGetAncPseudocostCountCurrentRun`.
pub fn var_get_anc_pseudocost_count_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, true, 0.0, 0.0, &|h, d| h.get_anc_pseudocost_count(d))
}

/// `SCIPvarGetMinPseudocostScore`: the smaller pseudocost of rounding
/// `solval` up or down, 0 without observations in both directions.
pub fn var_get_min_pseudocost_score(scip: &Scip, var: VarId, solval: f64) -> f64 {
    if scip.set.is_feas_integral(solval) {
        return 0.0;
    }
    if var_get_pseudocost_count(scip, var, BranchDir::Downwards) < 1.0 || var_get_pseudocost_count(scip, var, BranchDir::Upwards) < 1.0 {
        return 0.0;
    }
    let solvaldeltaup = scip.set.ceil(solval) - solval;
    let solvaldeltadown = scip.set.floor(solval) - solval;
    let upscore = var_get_pseudocost(scip, var, solvaldeltaup);
    let downscore = var_get_pseudocost(scip, var, solvaldeltadown);
    min(upscore, downscore)
}

/// `SCIPvarGetPseudocostVariance`.
pub fn var_get_pseudocost_variance(scip: &Scip, var: VarId, dir: BranchDir, onlycurrentrun: bool) -> f64 {
    var_history_get(scip, var, dir, onlycurrentrun, 0.0, 0.0, &|h, d| h.get_pseudocost_variance(d))
}

/// `SCIPvarCalcPscostConfidenceBound`.
pub fn var_calc_pscost_confidence_bound(scip: &Scip, var: VarId, dir: BranchDir, onlycurrentrun: bool, clevel: crate::types::ConfidenceLevel) -> f64 {
    let mut confidencebound = var_get_pseudocost_variance(scip, var, dir, onlycurrentrun);
    if scip.set.is_feas_positive(confidencebound) {
        let count = if onlycurrentrun {
            var_get_pseudocost_count_current_run(scip, var, dir)
        } else {
            var_get_pseudocost_count(scip, var, dir)
        };

        // Assertion is valid because variance is positive.
        confidencebound /= count;
        confidencebound = confidencebound.sqrt();

        // Scale by the critical value of the Student-T distribution with
        // count - 1 degrees of freedom.
        confidencebound *= crate::misc::student_t_get_critical_value(clevel, scip.set.floor(count) as i32 - 1);
    } else {
        confidencebound = 0.0;
    }
    confidencebound
}

/// `SCIPvarIsPscostRelerrorReliable`.
pub fn var_is_pscost_relerror_reliable(scip: &Scip, var: VarId, threshold: f64, clevel: crate::types::ConfidenceLevel) -> bool {
    let downsize = var_get_pseudocost_count_current_run(scip, var, BranchDir::Downwards);
    let upsize = var_get_pseudocost_count_current_run(scip, var, BranchDir::Upwards);

    // Consider only the minimum number of observations.
    let size = min(downsize, upsize);

    // Not enough observations: return false.
    if size <= 1.9 {
        return false;
    }

    // Use the current run pseudo cost variances and counts.
    let relerrordown = if downsize >= 1.9 {
        let normval = max(1.0, var_get_pseudocost_current_run(scip, var, -1.0));
        var_calc_pscost_confidence_bound(scip, var, BranchDir::Downwards, true, clevel) / normval
    } else {
        0.0
    };
    let relerrorup = if upsize >= 1.9 {
        let normval = max(1.0, var_get_pseudocost_current_run(scip, var, 1.0));
        var_calc_pscost_confidence_bound(scip, var, BranchDir::Upwards, true, clevel) / normval
    } else {
        0.0
    };

    // Consider the maximum of the relative errors.
    let relerror = max(relerrorup, relerrordown);
    relerror <= threshold
}

/// `SCIPvarSignificantPscostDifference`.
pub fn var_significant_pscost_difference(
    scip: &Scip,
    varx: VarId,
    fracx: f64,
    vary: VarId,
    fracy: f64,
    dir: BranchDir,
    clevel: crate::types::ConfidenceLevel,
    onesided: bool,
) -> bool {
    // Statistical tests are only valid for different variables.
    if varx == vary {
        return false;
    }
    let countx = var_get_pseudocost_count(scip, varx, dir);
    let county = var_get_pseudocost_count(scip, vary, dir);

    // If not at least 2 measurements were taken, return false.
    if countx <= 1.9 || county <= 1.9 {
        return false;
    }
    let realdirection = if dir == BranchDir::Downwards { -1.0 } else { 1.0 };
    let meanx = fracx * var_get_pseudocost(scip, varx, realdirection);
    let meany = fracy * var_get_pseudocost(scip, vary, realdirection);
    let variancex = sqr(fracx) * var_get_pseudocost_variance(scip, varx, dir, false);
    let variancey = sqr(fracy) * var_get_pseudocost_variance(scip, vary, dir, false);

    // If there is no variance, the means are taken from a constant
    // distribution.
    if scip.set.is_feas_eq(variancex + variancey, 0.0) {
        return if onesided {
            scip.set.is_feas_gt(meanx, meany)
        } else {
            !scip.set.is_feas_eq(meanx, meany)
        };
    }
    let mut tresult = crate::misc::compute_two_sample_t_test_value(meanx, meany, variancex, variancey, countx, county);

    // For the two-sided hypothesis, just take the absolute of t.
    if !onesided {
        tresult = tresult.abs();
    }

    // The desired confidence level is reached when the t-value is larger
    // than the critical value.
    tresult >= crate::misc::student_t_get_critical_value(clevel, (countx + county - 2.0) as i32)
}

/// `SCIPvarPscostThresholdProbabilityTest`.
pub fn var_pscost_threshold_probability_test(scip: &Scip, var: VarId, frac: f64, threshold: f64, dir: BranchDir, clevel: crate::types::ConfidenceLevel) -> bool {
    use crate::types::ConfidenceLevel;
    let count = var_get_pseudocost_count(scip, var, dir);

    // Need at least 2 observations.
    if count <= 1.9 {
        return false;
    }
    let realdirection = if dir == BranchDir::Downwards { -1.0 } else { 1.0 };
    let mean = frac * var_get_pseudocost(scip, var, realdirection);
    let variance = sqr(frac) * var_get_pseudocost_variance(scip, var, dir, false);

    // Nothing can be said if the mean is already larger than the
    // threshold.
    if scip.set.is_feas_ge(mean, threshold) {
        return false;
    }

    // If the variance is zero, the probability is either 0 or 1.
    if scip.set.is_feas_eq(variance, 0.0) {
        return scip.set.is_feas_lt(mean, threshold);
    }

    // Compute the probability that a value below the threshold is
    // observed.
    let probability = crate::misc::normal_cdf(mean, variance, threshold);

    // Return true if the probability exceeds the confidence level.
    let problimit = match clevel {
        ConfidenceLevel::Min => 0.75,
        ConfidenceLevel::Low => 0.875,
        ConfidenceLevel::Medium => 0.9,
        ConfidenceLevel::High => 0.95,
        ConfidenceLevel::Max => 0.975,
    };
    probability >= problimit
}

/// `useValuehistory`.
fn use_valuehistory(scip: &Scip, var: VarId, value: f64) -> bool {
    // Check if the domain value is unknown (not specific).
    if value == UNKNOWN {
        return false;
    }
    if !scip.set.history_valuebased {
        return false;
    }

    // Value based history information is only collected for integer
    // variables (binaries and continuous variables have no domain values
    // worth recording).
    let t = scip.vars[var].vartype;
    t != VarType::Binary && t != VarType::Continuous
}

/// The history updates (`SCIPvarIncVSIDS`, `IncNActiveConflicts`,
/// `IncNBranchings`, `IncInferenceSum`, `IncCutoffSum`): applied to the
/// active variable's histories (`glb`: also the global ones) and its value
/// history, with the domain value mapped through aggregations.
fn var_history_inc(scip: &mut Scip, var: VarId, dir: BranchDir, mut value: f64, glb: bool, what: &str, op: &dyn Fn(&mut History, BranchDir)) -> ScipResult {
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == NONE {
                log::error!("cannot update {what} of original untransformed variable");
                return Err(Retcode::InvalidData);
            }
            var_history_inc(scip, transvar, dir, value, glb, what, op)
        }
        VarStatus::Loose | VarStatus::Column => {
            op(&mut scip.vars[var].history, dir);
            op(&mut scip.vars[var].historycrun, dir);
            if glb {
                op(&mut scip.stat.glbhistory, dir);
                op(&mut scip.stat.glbhistorycrun, dir);
            }
            if use_valuehistory(scip, var, value) {
                let vh = scip.vars[var].valuehistory.get_or_insert_with(Default::default);
                op(vh.find(value), dir);
            }
            Ok(())
        }
        VarStatus::Fixed => {
            log::error!("cannot update {what} of a fixed variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Aggregated => {
            let a = scip.vars[var].data.aggregate;
            value = (value - a.constant) / a.scalar;
            let d = if a.scalar > 0.0 { dir } else { branchdir_opposite(dir) };
            var_history_inc(scip, a.var, d, value, glb, what, op)
        }
        VarStatus::MultAggr => {
            log::error!("cannot update {what} of a multi-aggregated variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Negated => {
            value = 1.0 - value;
            let negvar = scip.vars[var].negatedvar;
            var_history_inc(scip, negvar, branchdir_opposite(dir), value, glb, what, op)
        }
    }
}

/// `SCIPvarIncVSIDS`.
pub fn var_inc_vsids(scip: &mut Scip, var: VarId, dir: BranchDir, value: f64, weight: f64) -> ScipResult {
    // Check if history statistics should be collected for a variable.
    if !scip.stat.collectvarhistory {
        return Ok(());
    }
    if scip.set.is_zero(weight) {
        return Ok(());
    }
    var_history_inc(scip, var, dir, value, false, "VSIDS", &|h, d| h.inc_vsids(d, weight))
}

/// `SCIPvarScaleVSIDS`.
pub fn var_scale_vsids(scip: &mut Scip, var: VarId, scalar: f64) -> ScipResult {
    match scip.vars[var].varstatus {
        VarStatus::Original => {
            let transvar = scip.vars[var].data.original.transvar;
            if transvar == NONE {
                log::error!("cannot update VSIDS of original untransformed variable");
                return Err(Retcode::InvalidData);
            }
            var_scale_vsids(scip, transvar, scalar)
        }
        VarStatus::Loose | VarStatus::Column => {
            let v = &mut scip.vars[var];
            v.history.scale_vsids(scalar);
            v.historycrun.scale_vsids(scalar);
            if let Some(vh) = v.valuehistory.as_mut() {
                vh.scale_vsids(scalar);
            }
            Ok(())
        }
        VarStatus::Fixed => {
            log::error!("cannot update VSIDS of a fixed variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Aggregated => {
            let aggrvar = scip.vars[var].data.aggregate.var;
            var_scale_vsids(scip, aggrvar, scalar)
        }
        VarStatus::MultAggr => {
            log::error!("cannot update VSIDS of a multi-aggregated variable");
            Err(Retcode::InvalidData)
        }
        VarStatus::Negated => {
            let negvar = scip.vars[var].negatedvar;
            var_scale_vsids(scip, negvar, scalar)
        }
    }
}

/// `SCIPvarIncNActiveConflicts`.
pub fn var_inc_nactive_conflicts(scip: &mut Scip, var: VarId, dir: BranchDir, value: f64, length: f64) -> ScipResult {
    if !scip.stat.collectvarhistory {
        return Ok(());
    }
    var_history_inc(scip, var, dir, value, false, "conflict score", &|h, d| h.inc_n_active_conflicts(d, length))
}

/// `SCIPvarIncNBranchings`.
pub fn var_inc_nbranchings(scip: &mut Scip, var: VarId, dir: BranchDir, value: f64, depth: i32) -> ScipResult {
    if !scip.stat.collectvarhistory {
        return Ok(());
    }
    var_history_inc(scip, var, dir, value, true, "branching counter", &|h, d| h.inc_n_branchings(d, depth))
}

/// `SCIPvarIncInferenceSum`.
pub fn var_inc_inference_sum(scip: &mut Scip, var: VarId, dir: BranchDir, value: f64, weight: f64) -> ScipResult {
    if !scip.stat.collectvarhistory {
        return Ok(());
    }
    var_history_inc(scip, var, dir, value, true, "inference counter", &|h, d| h.inc_inference_sum(d, weight))
}

/// `SCIPvarIncCutoffSum`.
pub fn var_inc_cutoff_sum(scip: &mut Scip, var: VarId, dir: BranchDir, value: f64, weight: f64) -> ScipResult {
    if !scip.stat.collectvarhistory {
        return Ok(());
    }
    var_history_inc(scip, var, dir, value, true, "cutoff sum", &|h, d| h.inc_cutoff_sum(d, weight))
}

/// `SCIPvarGetNActiveConflicts`.
pub fn var_get_nactive_conflicts(scip: &Scip, var: VarId, dir: BranchDir) -> i64 {
    var_history_get(scip, var, dir, false, 0, 0, &|h, d| h.get_n_active_conflicts(d))
}

/// `SCIPvarGetNActiveConflictsCurrentRun`.
pub fn var_get_nactive_conflicts_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> i64 {
    var_history_get(scip, var, dir, true, 0, 0, &|h, d| h.get_n_active_conflicts(d))
}

/// `SCIPvarGetAvgConflictlength`.
pub fn var_get_avg_conflictlength(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, false, 0.0, 0.0, &|h, d| h.get_avg_conflictlength(d))
}

/// `SCIPvarGetAvgConflictlengthCurrentRun`.
pub fn var_get_avg_conflictlength_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, true, 0.0, 0.0, &|h, d| h.get_avg_conflictlength(d))
}

/// `SCIPvarGetNBranchings`.
pub fn var_get_nbranchings(scip: &Scip, var: VarId, dir: BranchDir) -> i64 {
    var_history_get(scip, var, dir, false, 0, 0, &|h, d| h.get_n_branchings(d))
}

/// `SCIPvarGetNBranchingsCurrentRun`.
pub fn var_get_nbranchings_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> i64 {
    var_history_get(scip, var, dir, true, 0, 0, &|h, d| h.get_n_branchings(d))
}

/// `SCIPvarGetAvgBranchdepth`.
pub fn var_get_avg_branchdepth(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, false, 0.0, 0.0, &|h, d| h.get_avg_branchdepth(d))
}

/// `SCIPvarGetAvgBranchdepthCurrentRun`.
pub fn var_get_avg_branchdepth_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, true, 0.0, 0.0, &|h, d| h.get_avg_branchdepth(d))
}

/// `SCIPvarGetVSIDS`.
pub fn var_get_vsids(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    let v = &scip.vars[var];
    if v.varstatus == VarStatus::Loose || v.varstatus == VarStatus::Column {
        v.history.get_vsids(dir) / scip.stat.vsidsweight
    } else {
        var_get_vsids_rec(scip, var, dir)
    }
}

/// `SCIPvarGetVSIDS_rec`.
pub fn var_get_vsids_rec(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    let w = scip.stat.vsidsweight;
    var_history_get(scip, var, dir, false, 0.0, 0.0, &|h, d| h.get_vsids(d) / w)
}

/// `SCIPvarGetVSIDSCurrentRun`.
pub fn var_get_vsids_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    if dir != BranchDir::Downwards && dir != BranchDir::Upwards {
        return INVALID;
    }
    let w = scip.stat.vsidsweight;
    var_history_get(scip, var, dir, true, 0.0, 0.0, &|h, d| h.get_vsids(d) / w)
}

/// `SCIPvarGetInferenceSum`.
pub fn var_get_inference_sum(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, false, 0.0, 0.0, &|h, d| h.get_inference_sum(d))
}

/// `SCIPvarGetInferenceSumCurrentRun`.
pub fn var_get_inference_sum_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, true, 0.0, 0.0, &|h, d| h.get_inference_sum(d))
}

/// `SCIPvarGetAvgInferences` and its current-run version: the average
/// number of inferences per branching, or (without branchings) the number
/// of implications plus twice the number of cliques, or the global average.
fn var_get_avg_inferences_impl(scip: &Scip, var: VarId, dir: BranchDir, crun: bool) -> f64 {
    let v = &scip.vars[var];
    let glb = if crun { &scip.stat.glbhistorycrun } else { &scip.stat.glbhistory };
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                glb.get_avg_inferences(dir)
            } else {
                var_get_avg_inferences_impl(scip, v.data.original.transvar, dir, crun)
            }
        }
        VarStatus::Loose | VarStatus::Column => {
            let h: &History = if crun { &v.historycrun } else { &v.history };
            if h.get_n_branchings(dir) > 0 {
                h.get_avg_inferences(dir)
            } else {
                let up = (dir == BranchDir::Upwards) as usize;
                let nimpls = v.implics.as_ref().map_or(0, |im| im.nimpls[up]);
                let ncliques = v.cliquelist.as_ref().map_or(0, |cl| cl.ncliques[up]);
                if nimpls + ncliques > 0 {
                    (nimpls + 2 * ncliques) as f64
                } else {
                    glb.get_avg_inferences(dir)
                }
            }
        }
        VarStatus::Fixed | VarStatus::MultAggr => 0.0,
        VarStatus::Aggregated => {
            let d = if v.data.aggregate.scalar > 0.0 { dir } else { branchdir_opposite(dir) };
            var_get_avg_inferences_impl(scip, v.data.aggregate.var, d, crun)
        }
        VarStatus::Negated => var_get_avg_inferences_impl(scip, v.negatedvar, branchdir_opposite(dir), crun),
    }
}

/// `SCIPvarGetAvgInferences`.
pub fn var_get_avg_inferences(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_get_avg_inferences_impl(scip, var, dir, false)
}

/// `SCIPvarGetAvgInferencesCurrentRun`.
pub fn var_get_avg_inferences_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_get_avg_inferences_impl(scip, var, dir, true)
}

/// `SCIPvarGetCutoffSum`.
pub fn var_get_cutoff_sum(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, false, 0.0, 0.0, &|h, d| h.get_cutoff_sum(d))
}

/// `SCIPvarGetCutoffSumCurrentRun`.
pub fn var_get_cutoff_sum_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_history_get(scip, var, dir, true, 0.0, 0.0, &|h, d| h.get_cutoff_sum(d))
}

/// `SCIPvarGetAvgCutoffs` and its current-run version: the variable's
/// average if it was branched on, else the global one.
fn var_get_avg_cutoffs_impl(scip: &Scip, var: VarId, dir: BranchDir, crun: bool) -> f64 {
    let v = &scip.vars[var];
    let glb = if crun { &scip.stat.glbhistorycrun } else { &scip.stat.glbhistory };
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                glb.get_avg_cutoffs(dir)
            } else {
                var_get_avg_cutoffs_impl(scip, v.data.original.transvar, dir, crun)
            }
        }
        VarStatus::Loose | VarStatus::Column => {
            let h: &History = if crun { &v.historycrun } else { &v.history };
            if h.get_n_branchings(dir) > 0 {
                h.get_avg_cutoffs(dir)
            } else {
                glb.get_avg_cutoffs(dir)
            }
        }
        VarStatus::Fixed | VarStatus::MultAggr => 0.0,
        VarStatus::Aggregated => {
            let d = if v.data.aggregate.scalar > 0.0 { dir } else { branchdir_opposite(dir) };
            var_get_avg_cutoffs_impl(scip, v.data.aggregate.var, d, crun)
        }
        VarStatus::Negated => var_get_avg_cutoffs_impl(scip, v.negatedvar, branchdir_opposite(dir), crun),
    }
}

/// `SCIPvarGetAvgCutoffs`.
pub fn var_get_avg_cutoffs(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_get_avg_cutoffs_impl(scip, var, dir, false)
}

/// `SCIPvarGetAvgCutoffsCurrentRun`.
pub fn var_get_avg_cutoffs_current_run(scip: &Scip, var: VarId, dir: BranchDir) -> f64 {
    var_get_avg_cutoffs_impl(scip, var, dir, true)
}

/// The GMI score functions follow aggregations without flipping.
fn var_gmi_target(scip: &Scip, var: VarId) -> Option<VarId> {
    let mut v = var;
    loop {
        let x = &scip.vars[v];
        match x.varstatus {
            VarStatus::Original => {
                if x.data.original.transvar == NONE {
                    return None;
                }
                v = x.data.original.transvar;
            }
            VarStatus::Loose | VarStatus::Column => return Some(v),
            VarStatus::Fixed | VarStatus::MultAggr => return None,
            VarStatus::Aggregated => v = x.data.aggregate.var,
            VarStatus::Negated => v = x.negatedvar,
        }
    }
}

/// `SCIPvarGetAvgGMIScore`.
pub fn var_get_avg_gmiscore(scip: &Scip, var: VarId) -> f64 {
    var_gmi_target(scip, var).map_or(0.0, |v| scip.vars[v].history.get_avg_gmieff())
}

/// `SCIPvarIncGMIeffSum`.
pub fn var_inc_gmieff_sum(scip: &mut Scip, var: VarId, gmieff: f64) {
    if let Some(v) = var_gmi_target(scip, var) {
        scip.vars[v].history.inc_gmieff_sum(gmieff);
    }
}

/// `SCIPvarGetLastGMIScore`.
pub fn var_get_last_gmiscore(scip: &Scip, var: VarId) -> f64 {
    var_gmi_target(scip, var).map_or(0.0, |v| scip.vars[v].history.get_last_gmieff())
}

/// `SCIPvarSetLastGMIScore`.
pub fn var_set_last_gmiscore(scip: &mut Scip, var: VarId, gmieff: f64) {
    if let Some(v) = var_gmi_target(scip, var) {
        scip.vars[v].history.set_last_gmieff(gmieff);
    }
}

/// `initbdchgidx`: before any bound change.
pub const INITBDCHGIDX: BdChgIdx = BdChgIdx { depth: -2, pos: 0 };
/// `presolvebdchgidx`: in presolving.
pub const PRESOLVEBDCHGIDX: BdChgIdx = BdChgIdx { depth: -1, pos: 0 };

/// `SCIPbdchgidxIsEarlierNonNull`.
#[inline]
pub fn bdchgidx_is_earlier_non_null(idx1: &BdChgIdx, idx2: &BdChgIdx) -> bool {
    idx1.depth < idx2.depth || (idx1.depth == idx2.depth && idx1.pos < idx2.pos)
}

/// `SCIPbdchgidxIsEarlier`: `None` is "now", after everything.
#[inline]
pub fn bdchgidx_is_earlier(idx1: Option<&BdChgIdx>, idx2: Option<&BdChgIdx>) -> bool {
    match (idx1, idx2) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(a), Some(b)) => bdchgidx_is_earlier_non_null(a, b),
    }
}

/// `SCIPvarGetLbchgInfo` and `SCIPvarGetUbchgInfo`: the position of the
/// last bound change before (or, `after`, at or before) `bdchgidx` in the
/// variable's lower or upper bound change array, or `None`.
pub fn var_get_bdchg_info(scip: &Scip, var: VarId, boundtype: BoundType, bdchgidx: Option<&BdChgIdx>, after: bool) -> Option<i32> {
    let v = &scip.vars[var];
    let (infos, n) = if boundtype == BoundType::Lower {
        (&v.lbchginfos, v.nlbchginfos)
    } else {
        (&v.ubchginfos, v.nubchginfos)
    };
    let mut i = n - 1;
    while i >= 0 {
        if infos[i].redundant {
            return None;
        }
        let hit = if after {
            !bdchgidx_is_earlier(bdchgidx, Some(&infos[i].bdchgidx))
        } else {
            bdchgidx_is_earlier(Some(&infos[i].bdchgidx), bdchgidx)
        };
        if hit {
            return Some(i);
        }
        i -= 1;
    }
    None
}

/// The bound change info `pos` of the variable's lower or upper bounds.
#[inline]
pub fn var_bdchginfo(scip: &Scip, var: VarId, boundtype: BoundType, pos: i32) -> &BdChgInfo {
    if boundtype == BoundType::Lower {
        &scip.vars[var].lbchginfos[pos]
    } else {
        &scip.vars[var].ubchginfos[pos]
    }
}

/// `SCIPvarGetLastBdchgIndex`.
pub fn var_get_last_bdchg_index(scip: &Scip, var: VarId) -> BdChgIdx {
    let var = var_get_probvar(scip, var);
    if var == NONE {
        return INITBDCHGIDX;
    }
    if !var_is_active(scip, var) {
        return PRESOLVEBDCHGIDX;
    }
    let v = &scip.vars[var];
    let lbchgidx = if v.nlbchginfos > 0 && !v.lbchginfos[v.nlbchginfos - 1].redundant {
        v.lbchginfos[v.nlbchginfos - 1].bdchgidx
    } else {
        INITBDCHGIDX
    };
    let ubchgidx = if v.nubchginfos > 0 && !v.ubchginfos[v.nubchginfos - 1].redundant {
        v.ubchginfos[v.nubchginfos - 1].bdchgidx
    } else {
        INITBDCHGIDX
    };
    if bdchgidx_is_earlier_non_null(&lbchgidx, &ubchgidx) { ubchgidx } else { lbchgidx }
}

/// `SCIPvarGetLastBdchgDepth`.
pub fn var_get_last_bdchg_depth(scip: &Scip, var: VarId) -> i32 {
    var_get_last_bdchg_index(scip, var).depth
}

/// `SCIPvarGetConflictingBdchgDepth`: the depth at which the variable's
/// opposite bound first conflicted with `bound`, -1 if it does not, 0 if
/// globally.
pub fn var_get_conflicting_bdchg_depth(scip: &Scip, var: VarId, boundtype: BoundType, bound: f64) -> i32 {
    let v = &scip.vars[var];
    if boundtype == BoundType::Lower {
        // Check if the bound is in conflict with the current local bounds.
        if scip.set.is_le(bound, v.locdom.ub) {
            return -1;
        }

        // Check if the bound is in conflict with the global bound.
        if scip.set.is_gt(bound, v.glbdom.ub) {
            return 0;
        }

        // Local bounds are in conflict with the given bound -> there must be
        // at least one conflicting change!
        let mut i = v.nubchginfos - 1;
        while i > 0 && scip.set.is_gt(bound, v.ubchginfos[i - 1].newbound) {
            i -= 1;
        }
        v.ubchginfos[i].bdchgidx.depth
    } else {
        if scip.set.is_ge(bound, v.locdom.lb) {
            return -1;
        }
        if scip.set.is_lt(bound, v.glbdom.lb) {
            return 0;
        }
        let mut i = v.nlbchginfos - 1;
        while i > 0 && scip.set.is_lt(bound, v.lbchginfos[i - 1].newbound) {
            i -= 1;
        }
        v.lbchginfos[i].bdchgidx.depth
    }
}

/// `SCIPvarWasFixedEarlier`.
pub fn var_was_fixed_earlier(scip: &Scip, var1: VarId, var2: VarId) -> bool {
    let var1 = var_get_probvar(scip, var1);
    let var2 = var_get_probvar(scip, var2);
    let (v1, v2) = (&scip.vars[var1], &scip.vars[var2]);
    if !var_is_active(scip, var2) || v2.glbdom.lb > 0.5 || v2.glbdom.ub < 0.5 {
        return false;
    }
    if !var_is_active(scip, var1) || v1.glbdom.lb > 0.5 || v1.glbdom.ub < 0.5 {
        return true;
    }
    let bdchgidx1 = if v1.nlbchginfos == 1 {
        Some(&v1.lbchginfos[0_usize].bdchgidx)
    } else if v1.nubchginfos == 1 {
        Some(&v1.ubchginfos[0_usize].bdchgidx)
    } else {
        None
    };
    let bdchgidx2 = if v2.nlbchginfos == 1 {
        Some(&v2.lbchginfos[0_usize].bdchgidx)
    } else if v2.nubchginfos == 1 {
        Some(&v2.ubchginfos[0_usize].bdchgidx)
    } else {
        None
    };
    bdchgidx_is_earlier(bdchgidx1, bdchgidx2)
}

/// `SCIPbdchginfoGetRelaxedBound`.
pub fn bdchginfo_get_relaxed_bound(scip: &Scip, info: &BdChgInfo) -> f64 {
    if info.boundtype == BoundType::Lower {
        scip.vars[info.var].conflictrelaxedlb
    } else {
        scip.vars[info.var].conflictrelaxedub
    }
}

/// `SCIPbdchginfoHasInferenceReason`.
pub fn bdchginfo_has_inference_reason(info: &BdChgInfo) -> bool {
    info.boundchgtype == BoundchgType::ConsInfer || (info.boundchgtype == BoundchgType::PropInfer && info.inferencedata.reason.prop != NONE)
}

/// `SCIPbdchginfoIsTighter`.
pub fn bdchginfo_is_tighter(info1: &BdChgInfo, info2: &BdChgInfo) -> bool {
    if info1.boundtype == BoundType::Lower {
        info1.newbound > info2.newbound
    } else {
        info1.newbound < info2.newbound
    }
}

/// `SCIPvarsCountTypes`: `[bin, int, binimpl, intimpl, contimpl, cont]`.
pub fn vars_count_types(scip: &Scip, vars: &[VarId]) -> [i32; 6] {
    let mut counts = [0; 6];
    for &v in vars {
        let implied = var_is_implied_integral(scip, v);
        let k = match scip.vars[v].vartype {
            VarType::Binary => {
                if implied {
                    2
                } else {
                    0
                }
            }
            VarType::Integer => {
                if implied {
                    3
                } else {
                    1
                }
            }
            VarType::Continuous => {
                if implied {
                    4
                } else {
                    5
                }
            }
            VarType::ImplInt => panic!("unknown variable type"),
        };
        counts[k] += 1;
    }
    counts
}

/// `SCIPvarGetNUses`.
#[inline]
pub fn var_get_nuses(scip: &Scip, var: VarId) -> i32 {
    scip.vars[var].nuses
}

/// `SCIPvarSetInitial`: only before solving.
pub fn var_set_initial(scip: &mut Scip, var: VarId, initial: bool) -> ScipResult {
    if scip.vars[var].varstatus != VarStatus::Original && scip.vars[var].varstatus != VarStatus::Loose {
        return Err(Retcode::InvalidCall);
    }
    scip.vars[var].initial = initial;
    Ok(())
}

/// `SCIPvarSetRemovable`.
pub fn var_set_removable(scip: &mut Scip, var: VarId, removable: bool) -> ScipResult {
    if scip.vars[var].varstatus != VarStatus::Original && scip.vars[var].varstatus != VarStatus::Loose {
        return Err(Retcode::InvalidCall);
    }
    scip.vars[var].removable = removable;
    Ok(())
}

/// `SCIPvarIsOriginal`.
#[inline]
pub fn var_is_original(scip: &Scip, var: VarId) -> bool {
    let v = &scip.vars[var];
    v.varstatus == VarStatus::Original || (v.varstatus == VarStatus::Negated && scip.vars[v.negatedvar].varstatus == VarStatus::Original)
}

/// `SCIPvarIsTransformed`.
#[inline]
pub fn var_is_transformed(scip: &Scip, var: VarId) -> bool {
    let v = &scip.vars[var];
    v.varstatus != VarStatus::Original && (v.varstatus != VarStatus::Negated || scip.vars[v.negatedvar].varstatus != VarStatus::Original)
}

/// `SCIPvarIsNegated`.
#[inline]
pub fn var_is_negated(scip: &Scip, var: VarId) -> bool {
    scip.vars[var].varstatus == VarStatus::Negated
}

/// `SCIPvarIsBinary`: binary type, or integral with bounds (and lazy
/// bounds) within [0,1].
#[inline]
pub fn var_is_binary(scip: &Scip, var: VarId) -> bool {
    let v = &scip.vars[var];
    v.vartype == VarType::Binary
        || ((v.vartype != VarType::Continuous || v.varimpltype != ImplintType::None) && max(v.glbdom.lb, v.lazylb) >= 0.0 && min(v.glbdom.ub, v.lazyub) <= 1.0)
}

/// `SCIPvarIsIntegral`.
#[inline]
pub fn var_is_integral(scip: &Scip, var: VarId) -> bool {
    let v = &scip.vars[var];
    v.vartype != VarType::Continuous || v.varimpltype != ImplintType::None
}

/// `SCIPvarIsImpliedIntegral`.
#[inline]
pub fn var_is_implied_integral(scip: &Scip, var: VarId) -> bool {
    scip.vars[var].varimpltype != ImplintType::None
}

/// `SCIPvarIsNonimpliedIntegral`.
#[inline]
pub fn var_is_nonimplied_integral(scip: &Scip, var: VarId) -> bool {
    let v = &scip.vars[var];
    v.vartype != VarType::Continuous && v.varimpltype == ImplintType::None
}

/// `SCIPvarMarkDeletable`.
pub fn var_mark_deletable(scip: &mut Scip, var: VarId) {
    scip.vars[var].deletable = true;
}

/// `SCIPvarMarkNotDeletable`.
pub fn var_mark_not_deletable(scip: &mut Scip, var: VarId) {
    scip.vars[var].deletable = false;
}

/// `SCIPvarMarkDeleteGlobalStructures`.
pub fn var_mark_delete_global_structures(scip: &mut Scip, var: VarId) {
    scip.vars[var].delglobalstructs = true;
}

/// `SCIPvarMarkRelaxationOnly`.
pub fn var_mark_relaxation_only(scip: &mut Scip, var: VarId) {
    scip.vars[var].relaxationonly = true;
    var_mark_deletable(scip, var);
}

/// `SCIPvarIsActive`.
#[inline]
pub fn var_is_active(scip: &Scip, var: VarId) -> bool {
    scip.vars[var].probindex >= 0
}

/// `SCIPvarIsInLP`.
#[inline]
pub fn var_is_in_lp(scip: &Scip, var: VarId) -> bool {
    scip.vars[var].varstatus == VarStatus::Column && scip.cols[scip.vars[var].data.col].lppos >= 0
}

/// `SCIPvarGetMinAggrCoef`.
#[inline]
pub fn var_get_min_aggr_coef(scip: &Scip, var: VarId) -> f64 {
    scip.vars[var].data.loose.minaggrcoef
}

/// `SCIPvarGetMaxAggrCoef`.
#[inline]
pub fn var_get_max_aggr_coef(scip: &Scip, var: VarId) -> f64 {
    scip.vars[var].data.loose.maxaggrcoef
}

/// `SCIPvarGetLbOriginal`: also of negated original variables.
pub fn var_get_lb_original(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.varstatus == VarStatus::Original {
        v.data.original.origdom.lb
    } else {
        v.data.negate.constant - scip.vars[v.negatedvar].data.original.origdom.ub
    }
}

/// `SCIPvarGetUbOriginal`.
pub fn var_get_ub_original(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.varstatus == VarStatus::Original {
        v.data.original.origdom.ub
    } else {
        v.data.negate.constant - scip.vars[v.negatedvar].data.original.origdom.lb
    }
}

/// `SCIPvarGetAggregatedObj`: the objective of the variable expressed
/// through the active variables.
pub fn var_get_aggregated_obj(scip: &Scip, var: VarId, aggrobj: &mut f64) -> ScipResult {
    let mut probvar = var;
    let mut mult = 1.0;
    while probvar != NONE {
        let v = &scip.vars[probvar];
        match v.varstatus {
            VarStatus::Original | VarStatus::Loose | VarStatus::Column => {
                *aggrobj = mult * v.obj;
                return Ok(());
            }
            VarStatus::Fixed => {
                *aggrobj = 0.0;
                return Ok(());
            }
            VarStatus::MultAggr => {
                if v.data.multaggr.nvars == 1 {
                    mult *= v.data.multaggr.scalars[0];
                    probvar = v.data.multaggr.vars[0];
                } else {
                    *aggrobj = 0.0;
                    let mut k = v.data.multaggr.nvars - 1;
                    while k >= 0 {
                        let mut tmpobj = 0.0;
                        var_get_aggregated_obj(scip, v.data.multaggr.vars[k], &mut tmpobj)?;
                        *aggrobj += v.data.multaggr.scalars[k] * tmpobj;
                        k -= 1;
                    }
                    return Ok(());
                }
            }
            VarStatus::Aggregated => {
                mult *= v.data.aggregate.scalar;
                probvar = v.data.aggregate.var;
            }
            VarStatus::Negated => {
                mult *= -1.0;
                probvar = v.negatedvar;
            }
        }
    }
    Err(Retcode::InvalidData)
}

/// `SCIPvarCatchEvent`.
pub fn var_catch_event(
    scip: &mut Scip,
    var: VarId,
    eventtype: crate::event::EventType,
    eventhdlr: crate::types::EventhdlrId,
    eventdata: usize,
    filterpos: Option<&mut i32>,
) -> ScipResult {
    let filter = scip.vars[var].eventfilter;
    crate::event::eventfilter_add(scip, filter, eventtype, eventhdlr, eventdata, filterpos)
}

/// `SCIPvarDropEvent`.
pub fn var_drop_event(scip: &mut Scip, var: VarId, eventtype: crate::event::EventType, eventhdlr: crate::types::EventhdlrId, eventdata: usize, filterpos: i32) -> ScipResult {
    let filter = scip.vars[var].eventfilter;
    crate::event::eventfilter_del(scip, filter, eventtype, eventhdlr, eventdata, filterpos)
}

/// `SCIPvarGetNLocksDownType`: gets number of locks for rounding down of a
/// special type.
pub fn var_get_n_locks_down_type(scip: &Scip, var: VarId, locktype: LockType) -> i32 {
    let v = &scip.vars[var];
    let lt = locktype as usize;
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar != NONE {
                var_get_n_locks_down_type(scip, v.data.original.transvar, locktype)
            } else {
                v.nlocksdown[lt]
            }
        }
        VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => v.nlocksdown[lt],
        VarStatus::Aggregated => {
            if v.data.aggregate.scalar > 0.0 {
                var_get_n_locks_down_type(scip, v.data.aggregate.var, locktype)
            } else {
                var_get_n_locks_up_type(scip, v.data.aggregate.var, locktype)
            }
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut nlocks = 0;
            for i in 0..m.nvars {
                if m.scalars[i] > 0.0 {
                    nlocks += var_get_n_locks_down_type(scip, m.vars[i], locktype);
                } else {
                    nlocks += var_get_n_locks_up_type(scip, m.vars[i], locktype);
                }
            }
            nlocks
        }
        VarStatus::Negated => var_get_n_locks_up_type(scip, v.negatedvar, locktype),
    }
}

/// `SCIPvarGetNLocksUpType`: gets number of locks for rounding up of a
/// special type.
pub fn var_get_n_locks_up_type(scip: &Scip, var: VarId, locktype: LockType) -> i32 {
    let v = &scip.vars[var];
    let lt = locktype as usize;
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar != NONE {
                var_get_n_locks_up_type(scip, v.data.original.transvar, locktype)
            } else {
                v.nlocksup[lt]
            }
        }
        VarStatus::Loose | VarStatus::Column | VarStatus::Fixed => v.nlocksup[lt],
        VarStatus::Aggregated => {
            if v.data.aggregate.scalar > 0.0 {
                var_get_n_locks_up_type(scip, v.data.aggregate.var, locktype)
            } else {
                var_get_n_locks_down_type(scip, v.data.aggregate.var, locktype)
            }
        }
        VarStatus::MultAggr => {
            let m = &v.data.multaggr;
            let mut nlocks = 0;
            for i in 0..m.nvars {
                if m.scalars[i] > 0.0 {
                    nlocks += var_get_n_locks_up_type(scip, m.vars[i], locktype);
                } else {
                    nlocks += var_get_n_locks_down_type(scip, m.vars[i], locktype);
                }
            }
            nlocks
        }
        VarStatus::Negated => var_get_n_locks_down_type(scip, v.negatedvar, locktype),
    }
}

/// `SCIPvarIncNActiveConflicts`: increases the number of active conflicts
/// by one and the overall length of the variable by the given length.
pub fn var_inc_n_active_conflicts(scip: &mut Scip, var: VarId, dir: BranchDir, value: f64, length: f64) -> ScipResult {
    // Check if history statistics should be collected for a variable.
    if !scip.stat.collectvarhistory {
        return Ok(());
    }
    var_history_inc(scip, var, dir, value, false, "conflict score", &|h, d| h.inc_n_active_conflicts(d, length))
}

/// `SCIPvarGetNBranchingsCurrentRun`: returns the number of times, a bound
/// of the variable was changed in given direction due to branching in the
/// current run.
pub fn var_get_n_branchings_current_run(scip: &Scip, var: VarId, dir: crate::types::BranchDir) -> i64 {
    let v = &scip.vars[var];
    match v.varstatus {
        VarStatus::Original => {
            if v.data.original.transvar == NONE {
                0
            } else {
                var_get_n_branchings_current_run(scip, v.data.original.transvar, dir)
            }
        }
        VarStatus::Loose | VarStatus::Column => v.historycrun.get_n_branchings(dir),
        VarStatus::Fixed => 0,
        VarStatus::Aggregated => {
            if v.data.aggregate.scalar > 0.0 {
                var_get_n_branchings_current_run(scip, v.data.aggregate.var, dir)
            } else {
                var_get_n_branchings_current_run(scip, v.data.aggregate.var, crate::history::branchdir_opposite(dir))
            }
        }
        VarStatus::MultAggr => 0,
        VarStatus::Negated => var_get_n_branchings_current_run(scip, v.negatedvar, crate::history::branchdir_opposite(dir)),
    }
}

/// `SCIPvarGetNCliques`: gets number of cliques, the active variable is
/// contained in (`varfixing`: FALSE for cliques containing x == 0, TRUE for
/// x == 1).
pub fn var_get_n_cliques(scip: &Scip, var: VarId, varfixing: bool) -> i32 {
    scip.vars[var].cliquelist.as_ref().map_or(0, |l| l.ncliques[varfixing as usize])
}

/// `SCIPvarGetNImpls`: gets number of implications y <= b or y >= b for
/// x == 0 or x == 1 of given active problem variable x.
pub fn var_get_n_impls(scip: &Scip, var: VarId, varfixing: bool) -> i32 {
    scip.vars[var].implics.as_ref().map_or(0, |i| i.nimpls[varfixing as usize])
}
