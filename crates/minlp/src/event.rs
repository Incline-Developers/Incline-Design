//! SCIP's events (`event.c`, `struct_event.h`): the event queue, event
//! filters and event handlers.
//!
//! Event data, C's `SCIP_EVENTDATA*`, is an opaque `usize` the handler
//! chose when it caught the event (an id, or an index into its own table).

use std::any::Any;

use crate::{
    clock::Clock,
    cvec::CVec,
    retcode::ScipResult,
    scip::Scip,
    types::{ColId, EventhdlrId, ImplintType, NodeId, RowId, SideType, SolId, VarId, VarType},
    var::NONE,
};

/// `SCIP_EVENTTYPE`.
pub type EventType = u64;
pub const EVENTTYPE_DISABLED: EventType = 0x0000000000;
pub const EVENTTYPE_VARADDED: EventType = 0x0000000001;
pub const EVENTTYPE_VARDELETED: EventType = 0x0000000002;
pub const EVENTTYPE_VARFIXED: EventType = 0x0000000004;
pub const EVENTTYPE_VARUNLOCKED: EventType = 0x0000000008;
pub const EVENTTYPE_OBJCHANGED: EventType = 0x0000000010;
pub const EVENTTYPE_GLBCHANGED: EventType = 0x0000000020;
pub const EVENTTYPE_GUBCHANGED: EventType = 0x0000000040;
pub const EVENTTYPE_LBTIGHTENED: EventType = 0x0000000080;
pub const EVENTTYPE_LBRELAXED: EventType = 0x0000000100;
pub const EVENTTYPE_UBTIGHTENED: EventType = 0x0000000200;
pub const EVENTTYPE_UBRELAXED: EventType = 0x0000000400;
pub const EVENTTYPE_GHOLEADDED: EventType = 0x0000000800;
pub const EVENTTYPE_GHOLEREMOVED: EventType = 0x0000001000;
pub const EVENTTYPE_LHOLEADDED: EventType = 0x0000002000;
pub const EVENTTYPE_LHOLEREMOVED: EventType = 0x0000004000;
pub const EVENTTYPE_IMPLADDED: EventType = 0x0000008000;
pub const EVENTTYPE_TYPECHANGED: EventType = 0x0000010000;
pub const EVENTTYPE_IMPLTYPECHANGED: EventType = 0x0000020000;
pub const EVENTTYPE_PRESOLVEROUND: EventType = 0x0000040000;
pub const EVENTTYPE_NODEFOCUSED: EventType = 0x0000080000;
pub const EVENTTYPE_NODEFEASIBLE: EventType = 0x0000100000;
pub const EVENTTYPE_NODEINFEASIBLE: EventType = 0x0000200000;
pub const EVENTTYPE_NODEBRANCHED: EventType = 0x0000400000;
pub const EVENTTYPE_NODEDELETE: EventType = 0x0000800000;
pub const EVENTTYPE_DUALBOUNDIMPROVED: EventType = 0x0001000000;
pub const EVENTTYPE_FIRSTLPSOLVED: EventType = 0x0002000000;
pub const EVENTTYPE_LPSOLVED: EventType = 0x0004000000;
pub const EVENTTYPE_POORSOLFOUND: EventType = 0x0008000000;
pub const EVENTTYPE_BESTSOLFOUND: EventType = 0x0010000000;
pub const EVENTTYPE_ROWADDEDSEPA: EventType = 0x0020000000;
pub const EVENTTYPE_ROWDELETEDSEPA: EventType = 0x0040000000;
pub const EVENTTYPE_ROWADDEDLP: EventType = 0x0080000000;
pub const EVENTTYPE_ROWDELETEDLP: EventType = 0x0100000000;
pub const EVENTTYPE_ROWCOEFCHANGED: EventType = 0x0200000000;
pub const EVENTTYPE_ROWCONSTCHANGED: EventType = 0x0400000000;
pub const EVENTTYPE_ROWSIDECHANGED: EventType = 0x0800000000;
pub const EVENTTYPE_SYNC: EventType = 0x1000000000;
pub const EVENTTYPE_GBDCHANGED: EventType = EVENTTYPE_GLBCHANGED | EVENTTYPE_GUBCHANGED;
pub const EVENTTYPE_LBCHANGED: EventType = EVENTTYPE_LBTIGHTENED | EVENTTYPE_LBRELAXED;
pub const EVENTTYPE_UBCHANGED: EventType = EVENTTYPE_UBTIGHTENED | EVENTTYPE_UBRELAXED;
pub const EVENTTYPE_BOUNDTIGHTENED: EventType = EVENTTYPE_LBTIGHTENED | EVENTTYPE_UBTIGHTENED;
pub const EVENTTYPE_BOUNDRELAXED: EventType = EVENTTYPE_LBRELAXED | EVENTTYPE_UBRELAXED;
pub const EVENTTYPE_BOUNDCHANGED: EventType = EVENTTYPE_LBCHANGED | EVENTTYPE_UBCHANGED;
pub const EVENTTYPE_GHOLECHANGED: EventType = EVENTTYPE_GHOLEADDED | EVENTTYPE_GHOLEREMOVED;
pub const EVENTTYPE_LHOLECHANGED: EventType = EVENTTYPE_LHOLEADDED | EVENTTYPE_LHOLEREMOVED;
pub const EVENTTYPE_HOLECHANGED: EventType = EVENTTYPE_GHOLECHANGED | EVENTTYPE_LHOLECHANGED;
pub const EVENTTYPE_DOMCHANGED: EventType = EVENTTYPE_BOUNDCHANGED | EVENTTYPE_HOLECHANGED;
pub const EVENTTYPE_VARCHANGED: EventType = EVENTTYPE_VARFIXED
    | EVENTTYPE_VARUNLOCKED
    | EVENTTYPE_OBJCHANGED
    | EVENTTYPE_GBDCHANGED
    | EVENTTYPE_DOMCHANGED
    | EVENTTYPE_IMPLADDED
    | EVENTTYPE_VARDELETED
    | EVENTTYPE_TYPECHANGED
    | EVENTTYPE_IMPLTYPECHANGED;
pub const EVENTTYPE_VAREVENT: EventType = EVENTTYPE_VARADDED | EVENTTYPE_VARCHANGED;
pub const EVENTTYPE_NODESOLVED: EventType = EVENTTYPE_NODEFEASIBLE | EVENTTYPE_NODEINFEASIBLE | EVENTTYPE_NODEBRANCHED;
pub const EVENTTYPE_NODEEVENT: EventType = EVENTTYPE_NODEFOCUSED | EVENTTYPE_NODESOLVED;
pub const EVENTTYPE_LPEVENT: EventType = EVENTTYPE_FIRSTLPSOLVED | EVENTTYPE_LPSOLVED;
pub const EVENTTYPE_SOLFOUND: EventType = EVENTTYPE_POORSOLFOUND | EVENTTYPE_BESTSOLFOUND;
pub const EVENTTYPE_SOLEVENT: EventType = EVENTTYPE_SOLFOUND;
pub const EVENTTYPE_GAPUPDATED: EventType = EVENTTYPE_BESTSOLFOUND | EVENTTYPE_DUALBOUNDIMPROVED;
pub const EVENTTYPE_ROWCHANGED: EventType = EVENTTYPE_ROWCOEFCHANGED | EVENTTYPE_ROWCONSTCHANGED | EVENTTYPE_ROWSIDECHANGED;
pub const EVENTTYPE_ROWEVENT: EventType = EVENTTYPE_ROWADDEDSEPA | EVENTTYPE_ROWDELETEDSEPA | EVENTTYPE_ROWADDEDLP | EVENTTYPE_ROWDELETEDLP | EVENTTYPE_ROWCHANGED;

/// The `data` union of `SCIP_EVENT`: every variant's fields, only those of
/// the event's type meaningful.
#[derive(Clone, Copy, Debug)]
pub struct EventData {
    pub var: VarId,
    pub oldobj: f64,
    pub newobj: f64,
    pub oldbound: f64,
    pub newbound: f64,
    pub left: f64,
    pub right: f64,
    pub oldtype: VarType,
    pub newtype: VarType,
    pub oldimpltype: ImplintType,
    pub newimpltype: ImplintType,
    pub row: RowId,
    pub col: ColId,
    pub side: SideType,
    pub oldval: f64,
    pub newval: f64,
    pub node: NodeId,
    pub sol: SolId,
}

impl Default for EventData {
    fn default() -> Self {
        Self {
            var: NONE,
            oldobj: 0.0,
            newobj: 0.0,
            oldbound: 0.0,
            newbound: 0.0,
            left: 0.0,
            right: 0.0,
            oldtype: VarType::Continuous,
            newtype: VarType::Continuous,
            oldimpltype: ImplintType::None,
            newimpltype: ImplintType::None,
            row: NONE,
            col: NONE,
            side: SideType::Left,
            oldval: 0.0,
            newval: 0.0,
            node: NONE,
            sol: NONE,
        }
    }
}

/// `SCIP_EVENT`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Event {
    pub data: EventData,
    pub eventtype: EventType,
}

/// `SCIP_EVENTFILTER`: the handlers catching events of one object, as a
/// list with a free list threaded through `nextpos`.
#[derive(Clone, Debug, Default)]
pub struct EventFilter {
    pub eventtypes: CVec<EventType>,
    pub eventhdlrs: CVec<EventhdlrId>,
    pub eventdata: CVec<usize>,
    pub nextpos: CVec<i32>,
    pub size: i32,
    pub len: i32,
    pub firstfreepos: i32,
    pub firstdeletedpos: i32,
    pub eventmask: EventType,
    pub delayedeventmask: EventType,
    pub delayupdates: bool,
}

/// `SCIP_EVENTQUEUE`.
#[derive(Clone, Debug, Default)]
pub struct EventQueue {
    pub events: CVec<Event>,
    pub eventssize: i32,
    pub nevents: i32,
    pub delayevents: bool,
}

/// `SCIP_DECL_EVENTCOPY` and the other handler-only callbacks.
pub type EventhdlrMethod = fn(&mut Scip, EventhdlrId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type EventhdlrCopy = fn(&mut Scip, &Scip, EventhdlrId) -> ScipResult;
pub type EventDelete = fn(&mut Scip, EventhdlrId, &mut usize) -> ScipResult;
pub type EventExec = fn(&mut Scip, EventhdlrId, &Event, usize) -> ScipResult;

/// `SCIP_EVENTHDLR`.
pub struct Eventhdlr {
    pub name: String,
    pub desc: String,
    pub eventcopy: Option<EventhdlrCopy>,
    pub eventfree: Option<EventhdlrMethod>,
    pub eventinit: Option<EventhdlrMethod>,
    pub eventexit: Option<EventhdlrMethod>,
    pub eventinitsol: Option<EventhdlrMethod>,
    pub eventexitsol: Option<EventhdlrMethod>,
    pub eventdelete: Option<EventDelete>,
    pub eventexec: Option<EventExec>,
    pub eventhdlrdata: Option<Box<dyn Any>>,
    pub setuptime: Clock,
    pub eventtime: Clock,
    pub initialized: bool,
}

/*
 * Event handler methods
 */

/// `SCIPeventhdlrCopyInclude`.
pub fn eventhdlr_copy_include(scip: &mut Scip, sourcescip: &Scip, eventhdlr: EventhdlrId) -> ScipResult {
    if let Some(copy) = sourcescip.eventhdlrs[eventhdlr].eventcopy {
        copy(scip, sourcescip, eventhdlr)?;
    }
    Ok(())
}

/// `SCIPeventhdlrCreate` (without the parameter, which event handlers
/// don't have).
#[allow(clippy::too_many_arguments)]
pub fn eventhdlr_create(
    name: &str,
    desc: &str,
    eventcopy: Option<EventhdlrCopy>,
    eventfree: Option<EventhdlrMethod>,
    eventinit: Option<EventhdlrMethod>,
    eventexit: Option<EventhdlrMethod>,
    eventinitsol: Option<EventhdlrMethod>,
    eventexitsol: Option<EventhdlrMethod>,
    eventdelete: Option<EventDelete>,
    eventexec: Option<EventExec>,
    eventhdlrdata: Option<Box<dyn Any>>,
) -> Eventhdlr {
    Eventhdlr {
        name: name.to_string(),
        desc: desc.to_string(),
        eventcopy,
        eventfree,
        eventinit,
        eventexit,
        eventinitsol,
        eventexitsol,
        eventdelete,
        eventexec,
        eventhdlrdata,
        setuptime: Clock::new(),
        eventtime: Clock::new(),
        initialized: false,
    }
}

/// `SCIPeventhdlrFree`: calls the destructor; the caller drops the handler.
pub fn eventhdlr_free(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    if let Some(eventfree) = scip.eventhdlrs[eventhdlr].eventfree {
        eventfree(scip, eventhdlr)?;
    }
    Ok(())
}

/// Runs one of the handler's setup callbacks under its setup clock.
fn eventhdlr_timed(scip: &mut Scip, eventhdlr: EventhdlrId, method: EventhdlrMethod) -> ScipResult {
    let time_enabled = scip.set.time_enabled;
    scip.eventhdlrs[eventhdlr].setuptime.start(time_enabled);
    method(scip, eventhdlr)?;
    scip.eventhdlrs[eventhdlr].setuptime.stop(time_enabled);
    Ok(())
}

/// `SCIPeventhdlrInit`.
pub fn eventhdlr_init(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    if scip.eventhdlrs[eventhdlr].initialized {
        log::error!("event handler <{}> already initialized", scip.eventhdlrs[eventhdlr].name);
        return Err(crate::retcode::Retcode::InvalidCall);
    }
    if scip.set.misc_resetstat {
        let h = &mut scip.eventhdlrs[eventhdlr];
        h.setuptime.reset();
        h.eventtime.reset();
    }
    if let Some(eventinit) = scip.eventhdlrs[eventhdlr].eventinit {
        eventhdlr_timed(scip, eventhdlr, eventinit)?;
    }
    scip.eventhdlrs[eventhdlr].initialized = true;
    Ok(())
}

/// `SCIPeventhdlrExit`.
pub fn eventhdlr_exit(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    if !scip.eventhdlrs[eventhdlr].initialized {
        log::error!("event handler <{}> not initialized", scip.eventhdlrs[eventhdlr].name);
        return Err(crate::retcode::Retcode::InvalidCall);
    }
    if let Some(eventexit) = scip.eventhdlrs[eventhdlr].eventexit {
        eventhdlr_timed(scip, eventhdlr, eventexit)?;
    }
    scip.eventhdlrs[eventhdlr].initialized = false;
    Ok(())
}

/// `SCIPeventhdlrInitsol`.
pub fn eventhdlr_initsol(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    if let Some(eventinitsol) = scip.eventhdlrs[eventhdlr].eventinitsol {
        eventhdlr_timed(scip, eventhdlr, eventinitsol)?;
    }
    Ok(())
}

/// `SCIPeventhdlrExitsol`.
pub fn eventhdlr_exitsol(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    if let Some(eventexitsol) = scip.eventhdlrs[eventhdlr].eventexitsol {
        eventhdlr_timed(scip, eventhdlr, eventexitsol)?;
    }
    Ok(())
}

/// `SCIPeventhdlrExec` (`TIMEEVENTEXEC` is off).
pub fn eventhdlr_exec(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let eventexec = scip.eventhdlrs[eventhdlr].eventexec.expect("event handler without exec");
    eventexec(scip, eventhdlr, event, eventdata)
}

/// `SCIPeventhdlrEnableOrDisableClocks`.
pub fn eventhdlr_enable_or_disable_clocks(eventhdlr: &mut Eventhdlr, enable: bool) {
    for clock in [&mut eventhdlr.setuptime, &mut eventhdlr.eventtime] {
        if enable {
            clock.enable();
        } else {
            clock.disable();
        }
    }
}

/*
 * Event methods
 */

fn event_of(eventtype: EventType, data: EventData) -> Event {
    Event { data, eventtype }
}

/// `SCIPeventCreateSync`.
pub fn event_create_sync() -> Event {
    event_of(EVENTTYPE_SYNC, EventData::default())
}

fn event_var(eventtype: EventType, var: VarId) -> Event {
    event_of(eventtype, EventData { var, ..Default::default() })
}

/// `SCIPeventCreateVarAdded`.
pub fn event_create_var_added(var: VarId) -> Event {
    event_var(EVENTTYPE_VARADDED, var)
}

/// `SCIPeventCreateVarDeleted`.
pub fn event_create_var_deleted(var: VarId) -> Event {
    event_var(EVENTTYPE_VARDELETED, var)
}

/// `SCIPeventCreateVarFixed`.
pub fn event_create_var_fixed(var: VarId) -> Event {
    event_var(EVENTTYPE_VARFIXED, var)
}

/// `SCIPeventCreateVarUnlocked`.
pub fn event_create_var_unlocked(var: VarId) -> Event {
    event_var(EVENTTYPE_VARUNLOCKED, var)
}

/// `SCIPeventCreateObjChanged`.
pub fn event_create_obj_changed(var: VarId, oldobj: f64, newobj: f64) -> Event {
    event_of(
        EVENTTYPE_OBJCHANGED,
        EventData {
            var,
            oldobj,
            newobj,
            ..Default::default()
        },
    )
}

fn event_bdchg(eventtype: EventType, var: VarId, oldbound: f64, newbound: f64) -> Event {
    event_of(
        eventtype,
        EventData {
            var,
            oldbound,
            newbound,
            ..Default::default()
        },
    )
}

/// `SCIPeventCreateGlbChanged`.
pub fn event_create_glb_changed(var: VarId, oldbound: f64, newbound: f64) -> Event {
    event_bdchg(EVENTTYPE_GLBCHANGED, var, oldbound, newbound)
}

/// `SCIPeventCreateGubChanged`.
pub fn event_create_gub_changed(var: VarId, oldbound: f64, newbound: f64) -> Event {
    event_bdchg(EVENTTYPE_GUBCHANGED, var, oldbound, newbound)
}

/// `SCIPeventCreateLbChanged`.
pub fn event_create_lb_changed(var: VarId, oldbound: f64, newbound: f64) -> Event {
    event_bdchg(if newbound > oldbound { EVENTTYPE_LBTIGHTENED } else { EVENTTYPE_LBRELAXED }, var, oldbound, newbound)
}

/// `SCIPeventCreateUbChanged`.
pub fn event_create_ub_changed(var: VarId, oldbound: f64, newbound: f64) -> Event {
    event_bdchg(if newbound < oldbound { EVENTTYPE_UBTIGHTENED } else { EVENTTYPE_UBRELAXED }, var, oldbound, newbound)
}

/// `SCIPeventCreateImplAdded`.
pub fn event_create_impl_added(var: VarId) -> Event {
    event_var(EVENTTYPE_IMPLADDED, var)
}

/// `SCIPeventCreateTypeChanged`.
pub fn event_create_type_changed(var: VarId, oldtype: VarType, newtype: VarType) -> Event {
    event_of(
        EVENTTYPE_TYPECHANGED,
        EventData {
            var,
            oldtype,
            newtype,
            ..Default::default()
        },
    )
}

/// `SCIPeventCreateImplTypeChanged`.
pub fn event_create_impl_type_changed(var: VarId, oldimpltype: ImplintType, newimpltype: ImplintType) -> Event {
    event_of(
        EVENTTYPE_IMPLTYPECHANGED,
        EventData {
            var,
            oldimpltype,
            newimpltype,
            ..Default::default()
        },
    )
}

fn event_row(eventtype: EventType, row: RowId) -> Event {
    event_of(eventtype, EventData { row, ..Default::default() })
}

/// `SCIPeventCreateRowAddedSepa`.
pub fn event_create_row_added_sepa(row: RowId) -> Event {
    event_row(EVENTTYPE_ROWADDEDSEPA, row)
}

/// `SCIPeventCreateRowDeletedSepa`.
pub fn event_create_row_deleted_sepa(row: RowId) -> Event {
    event_row(EVENTTYPE_ROWDELETEDSEPA, row)
}

/// `SCIPeventCreateRowAddedLP`.
pub fn event_create_row_added_lp(row: RowId) -> Event {
    event_row(EVENTTYPE_ROWADDEDLP, row)
}

/// `SCIPeventCreateRowDeletedLP`.
pub fn event_create_row_deleted_lp(row: RowId) -> Event {
    event_row(EVENTTYPE_ROWDELETEDLP, row)
}

/// `SCIPeventCreateRowCoefChanged`.
pub fn event_create_row_coef_changed(row: RowId, col: ColId, oldval: f64, newval: f64) -> Event {
    event_of(
        EVENTTYPE_ROWCOEFCHANGED,
        EventData {
            row,
            col,
            oldval,
            newval,
            ..Default::default()
        },
    )
}

/// `SCIPeventCreateRowConstChanged`.
pub fn event_create_row_const_changed(row: RowId, oldval: f64, newval: f64) -> Event {
    event_of(
        EVENTTYPE_ROWCONSTCHANGED,
        EventData {
            row,
            oldval,
            newval,
            ..Default::default()
        },
    )
}

/// `SCIPeventCreateRowSideChanged`.
pub fn event_create_row_side_changed(row: RowId, side: SideType, oldval: f64, newval: f64) -> Event {
    event_of(
        EVENTTYPE_ROWSIDECHANGED,
        EventData {
            row,
            side,
            oldval,
            newval,
            ..Default::default()
        },
    )
}

/// An event of the given node or LP type for `node`, set by the caller
/// with `SCIPeventChgType` and `SCIPeventChgNode`.
pub fn event_create_node(eventtype: EventType, node: NodeId) -> Event {
    event_of(eventtype, EventData { node, ..Default::default() })
}

/// An event of the given solution type for `sol` (`SCIPeventChgType` and
/// `SCIPeventChgSol`).
pub fn event_create_sol(eventtype: EventType, sol: SolId) -> Event {
    event_of(eventtype, EventData { sol, ..Default::default() })
}

/// `SCIPeventProcess`: processes the event; `eventfilter` takes the
/// events not tied to a variable (the global filter, or a row's). The C
/// callers' optional primal, LP and branching candidates are those of the
/// transformed problem: the C never reaches one it passed as NULL.
pub fn event_process(scip: &mut Scip, event: &Event, eventfilter: usize) -> ScipResult {
    use crate::{lp, types::VarStatus};
    let var = event.data.var;
    match event.eventtype {
        EVENTTYPE_DISABLED => {}
        EVENTTYPE_VARADDED
        | EVENTTYPE_PRESOLVEROUND
        | EVENTTYPE_NODEFOCUSED
        | EVENTTYPE_NODEFEASIBLE
        | EVENTTYPE_NODEINFEASIBLE
        | EVENTTYPE_NODEBRANCHED
        | EVENTTYPE_NODEDELETE
        | EVENTTYPE_DUALBOUNDIMPROVED
        | EVENTTYPE_FIRSTLPSOLVED
        | EVENTTYPE_LPSOLVED
        | EVENTTYPE_POORSOLFOUND
        | EVENTTYPE_BESTSOLFOUND
        | EVENTTYPE_ROWADDEDSEPA
        | EVENTTYPE_ROWDELETEDSEPA
        | EVENTTYPE_ROWADDEDLP
        | EVENTTYPE_ROWDELETEDLP
        | EVENTTYPE_ROWCOEFCHANGED
        | EVENTTYPE_ROWCONSTCHANGED
        | EVENTTYPE_ROWSIDECHANGED
        | EVENTTYPE_SYNC => eventfilter_process(scip, eventfilter, event)?,
        EVENTTYPE_OBJCHANGED => {
            if scip.vars[var].probindex >= 0 {
                if scip.vars[var].varstatus == VarStatus::Column {
                    lp::col_chg_obj(scip, scip.vars[var].data.col, event.data.newobj)?;
                }
                lp::lp_update_var_obj(scip, var, event.data.oldobj, event.data.newobj)?;
            }
            if !scip.lp.as_ref().unwrap().divingobjchg {
                crate::primal::primal_update_var_obj(scip, crate::scip::TRANSPRIMAL, var, event.data.oldobj, event.data.newobj);
            }
            eventfilter_process(scip, scip.vars[var].eventfilter, event)?;
        }
        EVENTTYPE_GLBCHANGED | EVENTTYPE_GUBCHANGED => {
            let status = scip.vars[var].varstatus;
            if status == VarStatus::Column || status == VarStatus::Loose {
                if event.eventtype == EVENTTYPE_GLBCHANGED {
                    lp::lp_update_var_lb_global(scip, var, event.data.oldbound, event.data.newbound)?;
                } else {
                    lp::lp_update_var_ub_global(scip, var, event.data.oldbound, event.data.newbound)?;
                }
            }
            eventfilter_process(scip, scip.vars[var].eventfilter, event)?;
        }
        EVENTTYPE_LBTIGHTENED | EVENTTYPE_LBRELAXED | EVENTTYPE_UBTIGHTENED | EVENTTYPE_UBRELAXED => {
            let status = scip.vars[var].varstatus;
            let lower = event.eventtype & EVENTTYPE_LBCHANGED != 0;
            if status == VarStatus::Column || status == VarStatus::Loose {
                if status == VarStatus::Column {
                    let col = scip.vars[var].data.col;
                    if lower {
                        lp::col_chg_lb(scip, col, event.data.newbound)?;
                    } else {
                        lp::col_chg_ub(scip, col, event.data.newbound)?;
                    }
                }
                if lower {
                    lp::lp_update_var_lb(scip, var, event.data.oldbound, event.data.newbound)?;
                } else {
                    lp::lp_update_var_ub(scip, var, event.data.oldbound, event.data.newbound)?;
                }
                crate::branch::branchcand_update_var(scip, var)?;
            }
            eventfilter_process(scip, scip.vars[var].eventfilter, event)?;
        }
        EVENTTYPE_GHOLEADDED
        | EVENTTYPE_GHOLEREMOVED
        | EVENTTYPE_LHOLEADDED
        | EVENTTYPE_LHOLEREMOVED
        | EVENTTYPE_IMPLADDED
        | EVENTTYPE_TYPECHANGED
        | EVENTTYPE_IMPLTYPECHANGED
        | EVENTTYPE_VARDELETED
        | EVENTTYPE_VARFIXED
        | EVENTTYPE_VARUNLOCKED => eventfilter_process(scip, scip.vars[var].eventfilter, event)?,
        _ => {
            log::error!("unknown event type <{:#x}>", event.eventtype);
            return Err(crate::retcode::Retcode::InvalidData);
        }
    }
    Ok(())
}

/*
 * Event filter methods
 */

/// `eventfilterEnsureMem`.
fn eventfilter_ensure_mem(scip: &mut Scip, filter: usize, num: i32) {
    if num > scip.eventfilters[filter].size {
        let newsize = scip.set.calc_mem_grow_size(num);
        let f = &mut scip.eventfilters[filter];
        f.eventtypes.realloc(newsize);
        f.eventhdlrs.realloc(newsize);
        f.eventdata.realloc(newsize);
        f.nextpos.realloc(newsize);
        f.size = newsize;
    }
}

/// `SCIPeventfilterCreate`.
pub fn eventfilter_create(scip: &mut Scip) -> usize {
    scip.eventfilters.push(EventFilter {
        eventtypes: CVec::new(),
        eventhdlrs: CVec::new(),
        eventdata: CVec::new(),
        nextpos: CVec::new(),
        size: 0,
        len: 0,
        firstfreepos: -1,
        firstdeletedpos: -1,
        eventmask: EVENTTYPE_DISABLED,
        delayedeventmask: EVENTTYPE_DISABLED,
        delayupdates: false,
    })
}

/// `SCIPeventfilterFree`: calls the handlers' delete callbacks for the
/// events still caught, then frees the filter.
pub fn eventfilter_free(scip: &mut Scip, filter: usize) -> ScipResult {
    let mut i = 0;
    while i < scip.eventfilters[filter].len {
        let f = &scip.eventfilters[filter];
        if f.eventtypes[i] != EVENTTYPE_DISABLED {
            let eventhdlr = f.eventhdlrs[i];
            if let Some(eventdelete) = scip.eventhdlrs[eventhdlr].eventdelete {
                let mut eventdata = scip.eventfilters[filter].eventdata[i];
                eventdelete(scip, eventhdlr, &mut eventdata)?;
                scip.eventfilters[filter].eventdata[i] = eventdata;
            }
        }
        i += 1;
    }
    scip.eventfilters.remove(filter);
    Ok(())
}

/// `SCIPeventfilterAdd`: returns the event's position in the filter.
pub fn eventfilter_add(scip: &mut Scip, filter: usize, eventtype: EventType, eventhdlr: EventhdlrId, eventdata: usize, filterpos: Option<&mut i32>) -> ScipResult {
    let pos;
    if scip.eventfilters[filter].delayupdates {
        // Updates are delayed: append the event to the filter's list.
        let len = scip.eventfilters[filter].len;
        eventfilter_ensure_mem(scip, filter, len + 1);
        let f = &mut scip.eventfilters[filter];
        pos = f.len;
        f.len += 1;
        f.delayedeventmask |= eventtype;
    } else {
        if scip.eventfilters[filter].firstfreepos == -1 {
            // Append the event to the filter's list.
            let len = scip.eventfilters[filter].len;
            eventfilter_ensure_mem(scip, filter, len + 1);
            let f = &mut scip.eventfilters[filter];
            pos = f.len;
            f.len += 1;
        } else {
            // Take the first free slot.
            let f = &mut scip.eventfilters[filter];
            pos = f.firstfreepos;
            f.firstfreepos = f.nextpos[pos];
        }
        scip.eventfilters[filter].eventmask |= eventtype;
    }
    let f = &mut scip.eventfilters[filter];
    f.eventtypes[pos] = eventtype;
    f.eventhdlrs[pos] = eventhdlr;
    f.eventdata[pos] = eventdata;
    f.nextpos[pos] = -2;
    if let Some(filterpos) = filterpos {
        *filterpos = pos;
    }
    Ok(())
}

/// `eventfilterSearch`: the position of the event in the filter, or -1.
fn eventfilter_search(f: &EventFilter, eventtype: EventType, eventhdlr: EventhdlrId, eventdata: usize) -> i32 {
    let mut i = f.len - 1;
    while i >= 0 {
        if eventdata == f.eventdata[i] && eventhdlr == f.eventhdlrs[i] && eventtype == f.eventtypes[i] && f.nextpos[i] == -2 {
            return i;
        }
        i -= 1;
    }
    -1
}

/// `SCIPeventfilterDel`: `filterpos` -1 searches for the event.
pub fn eventfilter_del(scip: &mut Scip, filter: usize, eventtype: EventType, eventhdlr: EventhdlrId, eventdata: usize, mut filterpos: i32) -> ScipResult {
    let f = &mut scip.eventfilters[filter];
    if filterpos == -1 {
        filterpos = eventfilter_search(f, eventtype, eventhdlr, eventdata);
    }
    if filterpos == -1 {
        log::error!("no event for event handler {eventhdlr} with data {eventdata} and event mask {eventtype:#x} found in event filter {filter}");
        return Err(crate::retcode::Retcode::InvalidData);
    }
    if f.delayupdates {
        // Updates are delayed: mark the entry deleted.
        f.nextpos[filterpos] = f.firstdeletedpos;
        f.firstdeletedpos = filterpos;
    } else {
        // Free the slot.
        f.eventtypes[filterpos] = EVENTTYPE_DISABLED;
        f.nextpos[filterpos] = f.firstfreepos;
        f.firstfreepos = filterpos;
    }
    Ok(())
}

/// `eventfilterProcessUpdates`: frees the slots deleted while updates were
/// delayed and enables the delayed event types.
fn eventfilter_process_updates(f: &mut EventFilter) {
    let mut pos = f.firstdeletedpos;
    while pos != -1 {
        let nextpos = f.nextpos[pos];
        f.nextpos[pos] = f.firstfreepos;
        f.firstfreepos = pos;
        f.eventtypes[pos] = EVENTTYPE_DISABLED;
        pos = nextpos;
    }
    f.firstdeletedpos = -1;
    f.eventmask |= f.delayedeventmask;
    f.delayedeventmask = EVENTTYPE_DISABLED;
    f.delayupdates = false;
}

/// `SCIPeventfilterProcess`: calls the handlers catching the event.
pub fn eventfilter_process(scip: &mut Scip, filter: usize, event: &Event) -> ScipResult {
    let eventtype = event.eventtype;

    // Check if there may be any event handler for specific event.
    if eventtype & scip.eventfilters[filter].eventmask == 0 {
        return Ok(());
    }

    // Delay the updates on this eventfilter, such that changes during event
    // processing to the event filter will not affect the current event
    // processing.
    scip.eventfilters[filter].delayupdates = true;

    // Process the event by calling the event handlers. C reads the event
    // types through a pointer taken before the loop; entries below `len`
    // don't change while updates are delayed.
    let mut processed = false;
    let len = scip.eventfilters[filter].len;
    for i in 0..len {
        let f = &scip.eventfilters[filter];
        if eventtype & f.eventtypes[i] != 0 {
            let (eventhdlr, eventdata) = (f.eventhdlrs[i], f.eventdata[i]);
            eventhdlr_exec(scip, eventhdlr, event, eventdata)?;
            processed = true;
        }
    }

    // If no handler of this filter wanted the event, remove it from the
    // mask.
    if !processed {
        scip.eventfilters[filter].eventmask &= !event.eventtype;
    }

    // Process delayed eventfilter changes.
    eventfilter_process_updates(&mut scip.eventfilters[filter]);
    Ok(())
}

/*
 * Event queue methods
 */

/// `eventqueueAppend`.
fn eventqueue_append(scip: &mut Scip, event: Event) {
    let n = scip.eventqueue.nevents;
    if n + 1 > scip.eventqueue.eventssize {
        let newsize = scip.set.calc_mem_grow_size(n + 1);
        scip.eventqueue.events.realloc(newsize);
        scip.eventqueue.eventssize = newsize;
    }
    let q = &mut scip.eventqueue;
    q.events[q.nevents] = event;
    q.nevents += 1;
}

/// `SCIPeventqueueAdd` with the global event filter.
pub fn eventqueue_add(scip: &mut Scip, event: Event) -> ScipResult {
    eventqueue_add_filter(scip, crate::scip::GLOBALEVENTFILTER, event)
}

/// `SCIPeventqueueAdd`: processes the event at once (against
/// `eventfilter`), or queues it while events are delayed, merging bound and
/// objective changes of a variable into the queued one. Queued events are
/// processed against the filter of the `eventqueue_process_filter` call.
pub fn eventqueue_add_filter(scip: &mut Scip, eventfilter: usize, event: Event) -> ScipResult {
    if !scip.eventqueue.delayevents {
        // Process event immediately, and the events it triggered.
        eventqueue_delay(scip);
        event_process(scip, &event, eventfilter)?;
        return eventqueue_process_filter(scip, eventfilter);
    }
    let var = event.data.var;
    match event.eventtype {
        EVENTTYPE_DISABLED => {
            log::error!("cannot add a disabled event to the event queue");
            return Err(crate::retcode::Retcode::InvalidData);
        }
        EVENTTYPE_VARADDED
        | EVENTTYPE_VARDELETED
        | EVENTTYPE_VARFIXED
        | EVENTTYPE_VARUNLOCKED
        | EVENTTYPE_GLBCHANGED
        | EVENTTYPE_GUBCHANGED
        | EVENTTYPE_PRESOLVEROUND
        | EVENTTYPE_NODEFOCUSED
        | EVENTTYPE_NODEFEASIBLE
        | EVENTTYPE_NODEINFEASIBLE
        | EVENTTYPE_NODEBRANCHED
        | EVENTTYPE_NODEDELETE
        | EVENTTYPE_DUALBOUNDIMPROVED
        | EVENTTYPE_FIRSTLPSOLVED
        | EVENTTYPE_LPSOLVED
        | EVENTTYPE_POORSOLFOUND
        | EVENTTYPE_BESTSOLFOUND
        | EVENTTYPE_GHOLEADDED
        | EVENTTYPE_GHOLEREMOVED
        | EVENTTYPE_LHOLEADDED
        | EVENTTYPE_LHOLEREMOVED
        | EVENTTYPE_ROWADDEDSEPA
        | EVENTTYPE_ROWDELETEDSEPA
        | EVENTTYPE_ROWADDEDLP
        | EVENTTYPE_ROWDELETEDLP
        | EVENTTYPE_ROWCOEFCHANGED
        | EVENTTYPE_ROWCONSTCHANGED
        | EVENTTYPE_ROWSIDECHANGED
        | EVENTTYPE_SYNC => eventqueue_append(scip, event),
        EVENTTYPE_OBJCHANGED => {
            let pos = scip.vars[var].eventqueueindexobj;
            if pos >= 0 {
                // Update last objective change event.
                let qevent = &mut scip.eventqueue.events[pos];
                qevent.data.newobj = event.data.newobj;
                if qevent.data.newobj == qevent.data.oldobj {
                    // The queued event became redundant.
                    qevent.eventtype = EVENTTYPE_DISABLED;
                    scip.vars[var].eventqueueindexobj = -1;
                }
            } else {
                // Objective value of variable was not yet changed.
                scip.vars[var].eventqueueindexobj = scip.eventqueue.nevents;
                eventqueue_append(scip, event);
            }
        }
        EVENTTYPE_LBTIGHTENED | EVENTTYPE_LBRELAXED => {
            let pos = scip.vars[var].eventqueueindexlb;
            if pos >= 0 {
                // Update last lower bound change event.
                let qevent = &mut scip.eventqueue.events[pos];
                qevent.data.newbound = event.data.newbound;
                if qevent.data.newbound < qevent.data.oldbound {
                    qevent.eventtype = EVENTTYPE_LBRELAXED;
                } else if qevent.data.newbound > qevent.data.oldbound {
                    qevent.eventtype = EVENTTYPE_LBTIGHTENED;
                } else {
                    // The queued bound change was reversed.
                    qevent.eventtype = EVENTTYPE_DISABLED;
                    scip.vars[var].eventqueueindexlb = -1;
                }
            } else {
                // Lower bound of variable was not yet changed.
                scip.vars[var].eventqueueindexlb = scip.eventqueue.nevents;
                eventqueue_append(scip, event);
            }
        }
        EVENTTYPE_UBTIGHTENED | EVENTTYPE_UBRELAXED => {
            let pos = scip.vars[var].eventqueueindexub;
            if pos >= 0 {
                // Update last upper bound change event.
                let qevent = &mut scip.eventqueue.events[pos];
                qevent.data.newbound = event.data.newbound;
                if qevent.data.newbound < qevent.data.oldbound {
                    qevent.eventtype = EVENTTYPE_UBTIGHTENED;
                } else if qevent.data.newbound > qevent.data.oldbound {
                    qevent.eventtype = EVENTTYPE_UBRELAXED;
                } else {
                    // The queued bound change was reversed.
                    qevent.eventtype = EVENTTYPE_DISABLED;
                    scip.vars[var].eventqueueindexub = -1;
                }
            } else {
                // Upper bound of variable was not yet changed.
                scip.vars[var].eventqueueindexub = scip.eventqueue.nevents;
                eventqueue_append(scip, event);
            }
        }
        EVENTTYPE_IMPLADDED => {
            if !scip.vars[var].eventqueueimpl {
                scip.vars[var].eventqueueimpl = true;
                eventqueue_append(scip, event);
            }
        }
        _ => {
            log::error!("unknown event type <{:#x}>", event.eventtype);
            return Err(crate::retcode::Retcode::InvalidData);
        }
    }
    Ok(())
}

/// `SCIPeventqueueDelay`.
pub fn eventqueue_delay(scip: &mut Scip) {
    scip.eventqueue.delayevents = true;
}

/// `SCIPeventqueueProcess` with the global event filter.
pub fn eventqueue_process(scip: &mut Scip) -> ScipResult {
    eventqueue_process_filter(scip, crate::scip::GLOBALEVENTFILTER)
}

/// `SCIPeventqueueProcess`: processes the queued events, including those
/// they queue, against `eventfilter`, then stops delaying.
pub fn eventqueue_process_filter(scip: &mut Scip, eventfilter: usize) -> ScipResult {
    let mut i = 0;
    while i < scip.eventqueue.nevents {
        let event = scip.eventqueue.events[i];

        // Unmark variables with events in the queue.
        let var = event.data.var;
        if event.eventtype & EVENTTYPE_OBJCHANGED != 0 {
            scip.vars[var].eventqueueindexobj = -1;
        } else if event.eventtype & EVENTTYPE_LBCHANGED != 0 {
            scip.vars[var].eventqueueindexlb = -1;
        } else if event.eventtype & EVENTTYPE_UBCHANGED != 0 {
            scip.vars[var].eventqueueindexub = -1;
        } else if event.eventtype & EVENTTYPE_IMPLADDED != 0 {
            scip.vars[var].eventqueueimpl = false;
        }

        // Process event.
        event_process(scip, &event, eventfilter)?;
        i += 1;
    }
    let q = &mut scip.eventqueue;
    q.nevents = 0;
    q.delayevents = false;
    Ok(())
}

/// `SCIPeventqueueIsDelayed`.
pub fn eventqueue_is_delayed(scip: &Scip) -> bool {
    scip.eventqueue.delayevents
}
