//! SCIP's cut selectors (`cutsel.c`, `struct_cutsel.h`).

use std::any::Any;

use crate::{
    clock::Clock,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{CutselId, RowId, ScipResultCode},
};

pub type CutselMethod = fn(&mut Scip, CutselId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type CutselCopy = fn(&mut Scip, &Scip, CutselId) -> ScipResult;
/// `SCIP_DECL_CUTSELSELECT`: `(cuts, forcedcuts, root, maxnselectedcuts,
/// nselectedcuts, result)`; the selected cuts are moved to the front of
/// `cuts`.
pub type CutselSelect = fn(&mut Scip, CutselId, &mut [RowId], &[RowId], bool, i32, &mut i32, &mut ScipResultCode) -> ScipResult;

/// `SCIP_CUTSEL`.
pub struct Cutsel {
    pub ncalls: i64,
    pub nrootcalls: i64,
    pub nrootcutsselected: i64,
    pub nrootcutsforced: i64,
    pub nrootcutsfiltered: i64,
    pub nlocalcutsselected: i64,
    pub nlocalcutsforced: i64,
    pub nlocalcutsfiltered: i64,
    pub name: String,
    pub desc: String,
    pub cutselcopy: Option<CutselCopy>,
    pub cutselfree: Option<CutselMethod>,
    pub cutselinit: Option<CutselMethod>,
    pub cutselexit: Option<CutselMethod>,
    pub cutselinitsol: Option<CutselMethod>,
    pub cutselexitsol: Option<CutselMethod>,
    pub cutselselect: Option<CutselSelect>,
    pub cutseldata: Option<Box<dyn Any>>,
    pub setuptime: Clock,
    pub cutseltime: Clock,
    pub priority: i32,
    pub initialized: bool,
}

/// `paramChgdCutselPriority`: the parameter already wrote the priority;
/// mark the cut selectors unsorted (`SCIPsetCutselPriority`).
fn param_chgd_cutsel_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.cutselssorted = false;
    Ok(())
}

/// The callbacks `SCIPcutselCreate` takes.
#[derive(Default)]
pub struct CutselMethods {
    pub cutselcopy: Option<CutselCopy>,
    pub cutselfree: Option<CutselMethod>,
    pub cutselinit: Option<CutselMethod>,
    pub cutselexit: Option<CutselMethod>,
    pub cutselinitsol: Option<CutselMethod>,
    pub cutselexitsol: Option<CutselMethod>,
    pub cutselselect: Option<CutselSelect>,
}

/// `SCIPcutselCreate`: creates a cut selector.
pub fn cutsel_create(scip: &mut Scip, name: &str, desc: &str, priority: i32, methods: CutselMethods, cutseldata: Option<Box<dyn Any>>) -> ScipResult<CutselId> {
    let cutsel = scip.cutsels.push(Cutsel {
        name: name.to_string(),
        desc: desc.to_string(),
        priority,
        cutselcopy: methods.cutselcopy,
        cutselfree: methods.cutselfree,
        cutselinit: methods.cutselinit,
        cutselexit: methods.cutselexit,
        cutselinitsol: methods.cutselinitsol,
        cutselexitsol: methods.cutselexitsol,
        cutselselect: methods.cutselselect,
        cutseldata,
        ncalls: 0,
        nrootcalls: 0,
        nrootcutsselected: 0,
        nrootcutsforced: 0,
        nrootcutsfiltered: 0,
        nlocalcutsselected: 0,
        nlocalcutsforced: 0,
        nlocalcutsfiltered: 0,
        initialized: false,
        // Create clocks.
        setuptime: Clock::new(),
        cutseltime: Clock::new(),
    });

    // Add parameters.
    crate::paramset::paramset_add_int(
        scip,
        &format!("cutselection/{name}/priority"),
        &format!("priority of cut selection rule <{name}>"),
        crate::field_target!(i32, cutsels, cutsel, priority),
        false,
        priority,
        i32::MIN / 4,
        i32::MAX / 2,
        Some(param_chgd_cutsel_priority),
    )?;
    Ok(cutsel)
}

/// `SCIPcutselComp`: compares two cut selectors w. r. to their priority.
pub fn cutsel_comp(scip: &Scip, elem1: CutselId, elem2: CutselId) -> i32 {
    scip.cutsels[elem2].priority - scip.cutsels[elem1].priority
}

/// `SCIPsetSortCutsels`: sorts cut selectors by priorities.
pub fn set_sort_cutsels(scip: &mut Scip) {
    if !scip.set.cutselssorted {
        let n = scip.set.ncutsels as usize;
        let mut cutsels = std::mem::take(&mut scip.set.cutsels);
        crate::sort::sort(&mut cutsels.0[..n], (), |&a, &b| cutsel_comp(scip, a, b));
        scip.set.cutsels = cutsels;
        scip.set.cutselssorted = true;
    }
}

/// `SCIPcutselsSelect`: calls cut selectors to select cuts; the first
/// `nforcedcuts` of `cuts` are forced; returns the number of selected
/// optional cuts, which are moved right after the forced ones.
pub fn cutsels_select(scip: &mut Scip, cuts: &mut [RowId], nforcedcuts: i32, root: bool, initiallp: bool, maxnselectedcuts: i32) -> ScipResult<i32> {
    let mut result = ScipResultCode::DidNotFind;
    let ncuts = cuts.len() as i32;

    // Sort the cut selectors by priority.
    set_sort_cutsels(scip);

    // Redefine maxnselectedcuts to be w.r.t the optional cuts.
    let maxnselectedcuts = (maxnselectedcuts - nforcedcuts).min(ncuts);

    // Try all cut selectors until one succeeds.
    let mut nselectedcuts = 0;
    let mut i = 0;
    while i < scip.set.ncutsels && result == ScipResultCode::DidNotFind {
        let cutsel = scip.set.cutsels[i];

        // Start timing.
        let enabled = scip.set.time_enabled;
        scip.cutsels[cutsel].cutseltime.start(enabled);

        let (forced, optional) = cuts.split_at_mut(nforcedcuts as usize);
        let select = scip.cutsels[cutsel].cutselselect.expect("a selection method");
        select(scip, cutsel, optional, forced, root, maxnselectedcuts, &mut nselectedcuts, &mut result)?;

        // Stop timing.
        scip.cutsels[cutsel].cutseltime.stop(enabled);

        let c = &mut scip.cutsels[cutsel];
        c.ncalls += 1;
        if root {
            c.nrootcalls += 1;
        }

        if result != ScipResultCode::DidNotFind && !initiallp {
            if root {
                c.nrootcutsselected += nselectedcuts as i64;
                c.nrootcutsforced += nforcedcuts as i64;
                c.nrootcutsfiltered += (ncuts - nselectedcuts) as i64;
            } else {
                c.nlocalcutsselected += nselectedcuts as i64;
                c.nlocalcutsforced += nforcedcuts as i64;
                c.nlocalcutsfiltered += (ncuts - nselectedcuts) as i64;
            }
        }
        i += 1;
    }
    Ok(nselectedcuts)
}

/// `SCIPcutselCopyInclude`: copies the given cut selector to a new scip.
pub fn cutsel_copy_include(scip: &mut Scip, sourcescip: &Scip, cutsel: CutselId) -> ScipResult {
    if let Some(copy) = sourcescip.cutsels[cutsel].cutselcopy {
        copy(scip, sourcescip, cutsel)?;
    }
    Ok(())
}

/// `SCIPcutselFree`: frees memory of cut selector.
pub fn cutsel_free(scip: &mut Scip, cutsel: CutselId) -> ScipResult {
    // Call destructor of cut selector.
    if let Some(free) = scip.cutsels[cutsel].cutselfree {
        free(scip, cutsel)?;
    }
    scip.cutsels.remove(cutsel);
    Ok(())
}

/// Runs one of the cut selector's setup callbacks under its setup clock.
fn cutsel_setup_call(scip: &mut Scip, cutsel: CutselId, method: Option<CutselMethod>) -> ScipResult {
    if let Some(method) = method {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.cutsels[cutsel].setuptime.start(enabled);
        method(scip, cutsel)?;
        // Stop timing.
        scip.cutsels[cutsel].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPcutselInit`: initializes cut selector.
pub fn cutsel_init(scip: &mut Scip, cutsel: CutselId) -> ScipResult {
    if scip.cutsels[cutsel].initialized {
        log::error!("cut selector <{}> already initialized", scip.cutsels[cutsel].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let c = &mut scip.cutsels[cutsel];
        c.setuptime.reset();
        c.cutseltime.reset();
    }

    let init = scip.cutsels[cutsel].cutselinit;
    cutsel_setup_call(scip, cutsel, init)?;
    scip.cutsels[cutsel].initialized = true;
    Ok(())
}

/// `SCIPcutselExit`: deinitializes cut selector.
pub fn cutsel_exit(scip: &mut Scip, cutsel: CutselId) -> ScipResult {
    if !scip.cutsels[cutsel].initialized {
        log::error!("cut selector <{}> not initialized", scip.cutsels[cutsel].name);
        return Err(Retcode::InvalidCall);
    }

    let exit = scip.cutsels[cutsel].cutselexit;
    cutsel_setup_call(scip, cutsel, exit)?;
    scip.cutsels[cutsel].initialized = false;
    Ok(())
}

/// `SCIPcutselInitsol`: informs cut selector that the branch and bound
/// process is being started.
pub fn cutsel_initsol(scip: &mut Scip, cutsel: CutselId) -> ScipResult {
    let initsol = scip.cutsels[cutsel].cutselinitsol;
    cutsel_setup_call(scip, cutsel, initsol)
}

/// `SCIPcutselExitsol`: informs cut selector that the branch and bound
/// process data is being freed.
pub fn cutsel_exitsol(scip: &mut Scip, cutsel: CutselId) -> ScipResult {
    let exitsol = scip.cutsels[cutsel].cutselexitsol;
    cutsel_setup_call(scip, cutsel, exitsol)
}

/// `SCIPcutselSetPriority`: sets priority of cut selector.
pub fn cutsel_set_priority(scip: &mut Scip, cutsel: CutselId, priority: i32) {
    scip.cutsels[cutsel].priority = priority;
    scip.set.cutselssorted = false;
}

/// `SCIPcutselEnableOrDisableClocks`.
pub fn cutsel_enable_or_disable_clocks(scip: &mut Scip, cutsel: CutselId, enable: bool) {
    let c = &mut scip.cutsels[cutsel];
    c.setuptime.enable_or_disable(enable);
    c.cutseltime.enable_or_disable(enable);
}
