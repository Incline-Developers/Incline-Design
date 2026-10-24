//! SCIP's conflict store (`conflictstore.c`, `struct_conflictstore.h`):
//! the conflict constraints and dual proofs conflict analysis found.

use crate::{
    cons,
    cvec::CVec,
    retcode::ScipResult,
    scip::{Scip, Stage},
    types::{ConfType, ConsId, EventhdlrId, LockType},
    var::NONE,
};

/// Default size of the dual ray proof store.
const CONFLICTSTORE_DUALRAYSIZE: i32 = 100;
/// Default size of the dual solution proof store.
const CONFLICTSTORE_DUALSOLSIZE: i32 = 75;
/// Default minimal size of a dynamic conflict store.
const CONFLICTSTORE_MINSIZE: i32 = 2000;
/// Maximal size of a dynamic conflict store (multiplied by 3).
const CONFLICTSTORE_MAXSIZE: i32 = 60000;
/// Default size of conflict store.
const CONFLICTSTORE_SIZE: i32 = 10000;
/// Frequency to resort the conflict array.
const CONFLICTSTORE_SORTFREQ: i64 = 20;

/* event handler properties */
const EVENTHDLR_NAME: &str = "ConflictStore";
const EVENTHDLR_DESC: &str = "Solution event handler for conflict store.";

/// `SCIP_CONFLICTSTORE`.
#[derive(Clone, Debug, Default)]
pub struct ConflictStore {
    pub eventhdlr: EventhdlrId,
    pub conflicts: CVec<ConsId>,
    pub dualrayconfs: CVec<ConsId>,
    pub dualsolconfs: CVec<ConsId>,
    pub origconfs: CVec<ConsId>,
    pub confprimalbnds: CVec<f64>,
    pub dualprimalbnds: CVec<f64>,
    pub scalefactors: CVec<f64>,
    pub updateside: CVec<bool>,
    pub drayrelaxonly: CVec<bool>,
    pub dsolrelaxonly: CVec<bool>,
    pub avgswitchlength: f64,
    pub lastcutoffbound: f64,
    pub lastnodenum: i64,
    pub ncleanups: i64,
    pub nnzdualrays: i64,
    pub nnzdualsols: i64,
    pub conflictsize: i32,
    pub origconflictsize: i32,
    pub nconflicts: i32,
    pub ndualrayconfs: i32,
    pub ndualsolconfs: i32,
    pub norigconfs: i32,
    pub ncbconflicts: i32,
    pub nconflictsfound: i32,
    pub cleanupfreq: i32,
    pub nswitches: i32,
    pub initstoresize: i32,
    pub storesize: i32,
    pub maxstoresize: i32,
}

macro_rules! cs {
    ($scip:expr) => {
        (**$scip.conflictstore.as_mut().unwrap())
    };
}

/// `eventExecConflictstore`: exec the event handler.
fn event_exec_conflictstore(scip: &mut Scip, _eventhdlr: EventhdlrId, _event: &crate::event::Event, _eventdata: usize) -> ScipResult {
    if scip.set.stage == Stage::Presolving || scip.set.stage == Stage::Solving {
        // SCIPclearConflictStore.
        let cutoffbound = scip.primals[crate::scip::TRANSPRIMAL].cutoffbound;
        conflictstore_clean_new_incumbent(scip, crate::prob::TRANSPROB, cutoffbound)?;
    }
    Ok(())
}

/// `eventInitsolConflictstore`: solving process initialization method of
/// event handler (called when branch and bound process is about to
/// begin).
fn event_initsol_conflictstore(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    let cleanboundexceeding = crate::paramset::paramset_get_bool(scip, "conflict/cleanboundexceedings")?;

    if !cleanboundexceeding {
        return Ok(());
    }

    crate::api::catch_event(scip, crate::event::EVENTTYPE_BESTSOLFOUND, eventhdlr, NONE, None)
}

/// `eventExitsolConflictstore`: solving process deinitialization method
/// of event handler (called before branch and bound process data is
/// freed).
fn event_exitsol_conflictstore(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    let cleanboundexceeding = crate::paramset::paramset_get_bool(scip, "conflict/cleanboundexceedings")?;

    if !cleanboundexceeding {
        return Ok(());
    }

    crate::api::drop_event(scip, crate::event::EVENTTYPE_BESTSOLFOUND, eventhdlr, NONE, -1)
}

/// `compareConss`: comparison method for constraints.
fn compare_conss(scip: &Scip, cons1: ConsId, cons2: ConsId) -> i32 {
    let (a1, a2) = (scip.conss[cons1].age, scip.conss[cons2].age);
    if a1 > a2 + 1e-09 {
        -1
    } else if a1 < a2 - 1e-09 {
        1
    } else {
        0
    }
}

/// `initConflictstore`: initializes the conflict store.
fn init_conflictstore(scip: &mut Scip, transprob: usize) -> ScipResult {
    // Calculate the maximal size of the conflict store.
    if cs!(scip).maxstoresize == -1 {
        let maxstoresize = scip.set.conf_maxstoresize;
        let nconss = scip.probs[transprob].nconss;
        let nvars = scip.probs[transprob].nvars;
        let c = &mut cs!(scip);
        c.maxstoresize = maxstoresize;

        // The size should be dynamic wrt number of variables after
        // presolving.
        if c.maxstoresize == -1 {
            c.initstoresize = CONFLICTSTORE_MINSIZE;
            c.initstoresize += 2 * nconss;

            if nvars / 2 <= 500 {
                c.initstoresize += CONFLICTSTORE_MAXSIZE / 100;
            } else if nvars / 2 <= 5000 {
                c.initstoresize += CONFLICTSTORE_MAXSIZE / 10;
            } else {
                c.initstoresize += CONFLICTSTORE_MAXSIZE / 2;
            }

            c.initstoresize = c.initstoresize.min(CONFLICTSTORE_MAXSIZE);
            c.storesize = c.initstoresize;
            c.maxstoresize = crate::def::min(3.0 * c.initstoresize as f64, CONFLICTSTORE_MAXSIZE as f64) as i32;
        } else {
            c.initstoresize = c.maxstoresize;
            c.storesize = c.maxstoresize;
        }
    }
    Ok(())
}

/// `conflictstoreEnsureMem`: resizes conflict and primal bound arrays to
/// be able to store at least `num` entries.
fn conflictstore_ensure_mem(scip: &mut Scip, num: i32) {
    // We do not allocate more memory as allowed.
    if cs!(scip).conflictsize == cs!(scip).maxstoresize {
        return;
    }

    if num > cs!(scip).conflictsize {
        let newsize;

        // Initialize the complete data structure.
        if cs!(scip).conflictsize == 0 {
            newsize = cs!(scip).storesize.min(CONFLICTSTORE_SIZE).max(num);
        } else {
            newsize = scip.set.calc_mem_grow_size(num).min(cs!(scip).maxstoresize);
        }
        let c = &mut cs!(scip);
        c.conflicts.0.resize(newsize as usize, NONE);
        c.confprimalbnds.realloc(newsize);
        c.conflictsize = newsize;
    }
}

/// `adjustStorageSize`: increase the dynamic storage if we could not
/// delete enough conflicts.
///
/// We want to have at least set->conf_maxconss free slots in the conflict
/// array, because this is the maximal number of conflicts generated at a
/// node. We increase the size by the minimum of set->conf_maxconss and 1%
/// of the current store size. Nevertheless, we don't exceed
/// conflictstore->maxstoresize.
fn adjust_storage_size(scip: &mut Scip) {
    let maxconss = scip.set.conf_maxconss;
    let c = &mut cs!(scip);

    // Increase storage.
    if c.storesize - c.nconflicts <= maxconss && c.storesize < c.maxstoresize {
        let increase = (0.01 * c.storesize as f64).ceil();
        c.storesize += maxconss.min(increase as i32);
        c.storesize = c.storesize.min(c.maxstoresize);
    }
}

/// `delPosConflict`: removes conflict at position `pos`.
fn del_pos_conflict(scip: &mut Scip, transprob: usize, pos: i32, deleteconflict: bool) -> ScipResult {
    let lastpos = cs!(scip).nconflicts - 1;
    let conflict = cs!(scip).conflicts[pos];

    // Decrease number of conflicts depending an a cutoff bound.
    let isinf = scip.set.is_infinity(cs!(scip).confprimalbnds[pos].abs());
    cs!(scip).ncbconflicts -= if isinf { 0 } else { 1 };

    // Remove conflict locks.
    cons::cons_add_locks(scip, conflict, LockType::Conflict, -1, 0)?;

    // Invalidate conflict position.
    scip.conss[conflict].confconsspos = -1;

    // Mark the constraint as deleted.
    if deleteconflict && !scip.conss[conflict].deleted {
        cons::cons_delete(scip, conflict, transprob)?;
    }
    cons::cons_release(scip, conflict)?;

    // Replace with conflict at the last position.
    if pos < lastpos {
        let c = &mut cs!(scip);
        let moved = c.conflicts[lastpos];
        c.conflicts[pos] = moved;
        c.confprimalbnds[pos] = c.confprimalbnds[lastpos];
        scip.conss[moved].confconsspos = pos;
    }

    // Decrease number of conflicts.
    cs!(scip).nconflicts -= 1;
    Ok(())
}

/// `delPosDualray`: removes proof based on a dual ray at position `pos`.
fn del_pos_dualray(scip: &mut Scip, transprob: usize, pos: i32, deleteconflict: bool) -> ScipResult {
    let lastpos = cs!(scip).ndualrayconfs - 1;
    let dualproof = cs!(scip).dualrayconfs[pos];

    // Decrease the number of non-zeros.
    let (nvars, _success) = cons::cons_get_n_vars(scip, dualproof)?;
    cs!(scip).nnzdualrays -= nvars as i64;

    // Implement dualraycons upgrade.
    scip.conss[dualproof].add_upgrade_locks(-1);

    // Remove conflict locks.
    cons::cons_add_locks(scip, dualproof, LockType::Conflict, -1, 0)?;

    // Mark the constraint as deleted.
    if deleteconflict && !scip.conss[dualproof].deleted {
        cons::cons_delete(scip, dualproof, transprob)?;
    }
    cons::cons_release(scip, dualproof)?;

    // Replace with dual ray at the last position.
    if pos < lastpos {
        let c = &mut cs!(scip);
        c.dualrayconfs[pos] = c.dualrayconfs[lastpos];
        c.drayrelaxonly[pos] = c.drayrelaxonly[lastpos];
    }

    // Decrease number of dual rays.
    cs!(scip).ndualrayconfs -= 1;
    Ok(())
}

/// `delPosDualsol`: removes proof based on a dual solution at position
/// `pos`.
fn del_pos_dualsol(scip: &mut Scip, transprob: usize, pos: i32, deleteconflict: bool) -> ScipResult {
    let lastpos = cs!(scip).ndualsolconfs - 1;
    let dualproof = cs!(scip).dualsolconfs[pos];

    // Decrease the number of non-zeros.
    let (nvars, _success) = cons::cons_get_n_vars(scip, dualproof)?;
    cs!(scip).nnzdualsols -= nvars as i64;

    // Implement dualsolcons upgrade.
    scip.conss[dualproof].add_upgrade_locks(-1);

    // Remove conflict locks.
    cons::cons_add_locks(scip, dualproof, LockType::Conflict, -1, 0)?;

    // Mark the constraint as deleted.
    if deleteconflict && !scip.conss[dualproof].deleted {
        cons::cons_delete(scip, dualproof, transprob)?;
    }
    cons::cons_release(scip, dualproof)?;

    // Replace with dual ray at the last position.
    if pos < lastpos {
        let c = &mut cs!(scip);
        c.dualsolconfs[pos] = c.dualsolconfs[lastpos];
        c.dualprimalbnds[pos] = c.dualprimalbnds[lastpos];
        c.scalefactors[pos] = c.scalefactors[lastpos];
        c.updateside[pos] = c.updateside[lastpos];
        c.dsolrelaxonly[pos] = c.dsolrelaxonly[lastpos];
    }

    // Decrease number of dual rays.
    cs!(scip).ndualsolconfs -= 1;
    Ok(())
}

/// `cleanDeletedAndCheckedConflicts`: removes all deleted conflicts from
/// the storage; returns how many.
fn clean_deleted_and_checked_conflicts(scip: &mut Scip) -> ScipResult<i32> {
    let mut ndelconfs = 0;

    // We traverse backwards to avoid swapping of pointers.
    let mut i = cs!(scip).nconflicts - 1;
    while i >= 0 {
        let c = cs!(scip).conflicts[i];
        // Check whether the constraint is already marked as deleted.
        if scip.conss[c].deleted || scip.conss[c].check {
            // Remove conflict at current position.
            del_pos_conflict(scip, NONE, i, false)?;
            ndelconfs += 1;
        }
        i -= 1;
    }
    Ok(ndelconfs)
}

/// `cleanDeletedAndCheckedDualrayCons`: removes all deleted dual proofs of
/// infeasible LP relaxations from the storage; returns how many.
fn clean_deleted_and_checked_dualray_cons(scip: &mut Scip) -> ScipResult<i32> {
    let mut ndelproofs = 0;

    // We traverse backwards to avoid swapping of pointers.
    let mut i = cs!(scip).ndualrayconfs - 1;
    while i >= 0 {
        let c = cs!(scip).dualrayconfs[i];
        // Check whether the constraint is already marked as deleted.
        if scip.conss[c].deleted || scip.conss[c].check {
            // Remove proof at current position.
            del_pos_dualray(scip, NONE, i, false)?;
            ndelproofs += 1;
        }
        i -= 1;
    }
    Ok(ndelproofs)
}

/// `cleanDeletedAndCheckedDualsolCons`: removes all deleted dual proofs of
/// bound exceeding LP relaxations from the storage; returns how many.
fn clean_deleted_and_checked_dualsol_cons(scip: &mut Scip) -> ScipResult<i32> {
    let mut ndelproofs = 0;

    // We traverse backwards to avoid swapping of pointers.
    let mut i = cs!(scip).ndualsolconfs - 1;
    while i >= 0 {
        let c = cs!(scip).dualsolconfs[i];
        // Check whether the constraint is already marked as deleted.
        if scip.conss[c].deleted || scip.conss[c].check {
            // Remove proof at current position.
            del_pos_dualsol(scip, NONE, i, false)?;
            ndelproofs += 1;
        }
        i -= 1;
    }
    Ok(ndelproofs)
}

/// `conflictstoreCleanUpStorage`: cleans up the storage.
fn conflictstore_clean_up_storage(scip: &mut Scip, transprob: usize) -> ScipResult {
    // The storage is empty.
    if cs!(scip).nconflicts == 0 {
        return Ok(());
    }

    // Remove all as deleted marked conflicts.
    let ndelconfs = clean_deleted_and_checked_conflicts(scip)?;

    // Return if at least one conflict could be deleted; only clean up the
    // storage if it is filled enough.
    if ndelconfs == 0 && cs!(scip).nconflicts >= cs!(scip).conflictsize {
        // Resort the array regularly.
        if cs!(scip).ncleanups % CONFLICTSTORE_SORTFREQ == 0 {
            // Sort conflict constraints.
            let n = cs!(scip).nconflicts as usize;
            let mut conflicts = std::mem::take(&mut cs!(scip).conflicts);
            let mut bnds = std::mem::take(&mut cs!(scip).confprimalbnds);
            crate::sort::sort(&mut conflicts.0[..n], (&mut bnds.0[..n],), |&a, &b| compare_conss(scip, a, b));
            cs!(scip).conflicts = conflicts;
            cs!(scip).confprimalbnds = bnds;

            // Update conflict positions.
            for i in 0..cs!(scip).nconflicts {
                let c = cs!(scip).conflicts[i];
                scip.conss[c].confconsspos = i;
            }
        }

        if cs!(scip).ncleanups % CONFLICTSTORE_SORTFREQ == 0 {
            // Remove conflict at first position (array is sorted).
            del_pos_conflict(scip, transprob, 0, true)?;
        } else {
            let mut maxage = scip.conss[cs!(scip).conflicts[0_usize]].age;
            let mut oldest_i = 0;

            // Check the first 10% of conflicts and find the oldest.
            let mut i = 1;
            while (i as f64) < 0.1 * cs!(scip).nconflicts as f64 {
                let age = scip.conss[cs!(scip).conflicts[i]].age;
                if age > maxage {
                    maxage = age;
                    oldest_i = i;
                }
                i += 1;
            }

            // Remove conflict at position oldest_i.
            del_pos_conflict(scip, transprob, oldest_i, true)?;
        }

        // Adjust size of the storage if we use a dynamic store.
        if scip.set.conf_maxstoresize == -1 {
            adjust_storage_size(scip);
        }
    }

    // Increase the number of clean ups.
    cs!(scip).ncleanups += 1;
    Ok(())
}

/// `conflictstoreAddOrigConflict`: adds an original conflict constraint to
/// the store; the constraint will be only transfered to the storage of the
/// transformed problem after calling SCIPconflictstoreTransform().
fn conflictstore_add_orig_conflict(scip: &mut Scip, cons: ConsId) {
    if cs!(scip).origconflictsize == 0 {
        cs!(scip).origconfs.0.resize(CONFLICTSTORE_MINSIZE as usize, NONE);
        cs!(scip).origconflictsize = CONFLICTSTORE_MINSIZE;
    } else if cs!(scip).norigconfs == cs!(scip).origconflictsize {
        let newsize = scip.set.calc_mem_grow_size(cs!(scip).origconflictsize + 1);
        cs!(scip).origconfs.0.resize(newsize as usize, NONE);
        cs!(scip).origconflictsize = newsize;
    }

    cons::cons_capture(scip, cons);
    let c = &mut cs!(scip);
    c.origconfs[c.norigconfs] = cons;
    c.norigconfs += 1;
}

/// `SCIPconflictstoreCreate`: creates conflict store.
pub fn conflictstore_create(scip: &mut Scip) -> ScipResult {
    let mut c = ConflictStore {
        initstoresize: -1,
        storesize: -1,
        maxstoresize: -1,
        lastcutoffbound: crate::def::INVALID,
        lastnodenum: -1,
        ..Default::default()
    };
    c.eventhdlr = crate::set::set_find_eventhdlr(scip, EVENTHDLR_NAME);

    // Create event handler for LP events.
    if c.eventhdlr == NONE {
        let h = crate::event::eventhdlr_create(
            EVENTHDLR_NAME,
            EVENTHDLR_DESC,
            None,
            None,
            None,
            None,
            Some(event_initsol_conflictstore),
            Some(event_exitsol_conflictstore),
            None,
            Some(event_exec_conflictstore),
            None,
        );
        c.eventhdlr = scip.eventhdlrs.push(h);
        crate::set::set_include_eventhdlr(scip, c.eventhdlr);
    }
    scip.conflictstore = Some(Box::new(c));
    Ok(())
}

/// `SCIPconflictstoreFree`: frees conflict store.
pub fn conflictstore_free(scip: &mut Scip) -> ScipResult {
    // Clear the storage.
    conflictstore_clear(scip)?;
    scip.conflictstore = None;
    Ok(())
}

/// `SCIPconflictstoreClear`: clears conflict store.
pub fn conflictstore_clear(scip: &mut Scip) -> ScipResult {
    // Remove original constraints (if present).
    for i in 0..cs!(scip).norigconfs {
        let conflict = cs!(scip).origconfs[i];
        cons::cons_release(scip, conflict)?;
    }
    cs!(scip).norigconfs = 0;

    // Clean up storage of conflict constraints; we traverse in reverse order
    // to avoid swapping of pointers.
    let mut i = cs!(scip).nconflicts - 1;
    while i >= 0 {
        del_pos_conflict(scip, NONE, i, false)?;
        i -= 1;
    }

    // Clean up storage of proof constraints based on dual rays.
    let mut i = cs!(scip).ndualrayconfs - 1;
    while i >= 0 {
        del_pos_dualray(scip, NONE, i, false)?;
        i -= 1;
    }

    // Clean up storage of proof constraints based on dual solutions.
    let mut i = cs!(scip).ndualsolconfs - 1;
    while i >= 0 {
        del_pos_dualsol(scip, NONE, i, false)?;
        i -= 1;
    }
    Ok(())
}

/// `SCIPconflictstoreClean`: cleans up conflict store.
pub fn conflictstore_clean(scip: &mut Scip, transprob: usize) -> ScipResult {
    // Remove all conflicts that are marked as deleted or where the check flag
    // has changed to TRUE.
    //
    // The latter case might happen during processing parallel constraints,
    // e.g., cons_knapsack.c:detectRedundantConstraints(). In that case, the
    // conflict constraint should stay, e.g., it has the stricter capacity,
    // and replaces a model constraint. Hence, the conflict is now a
    // constraint that is needed to stay correct. Therefore, the conflictpool
    // is not allowed to delete this constraint from the entire problem.
    clean_deleted_and_checked_conflicts(scip)?;

    // Remove all dual infeasibility proofs that are marked as deleted or
    // where the check flag has changed to TRUE.
    clean_deleted_and_checked_dualray_cons(scip)?;

    // Remove all dual bound exceeding proofs that are marked as deleted or
    // where the check flag has changed to TRUE.
    clean_deleted_and_checked_dualsol_cons(scip)?;

    // Don't update bound exceeding proofs after a restart.
    if scip.stat.inrestart {
        let mut i = cs!(scip).ndualrayconfs - 1;
        while i >= 0 {
            if cs!(scip).drayrelaxonly[i] {
                del_pos_dualray(scip, transprob, i, true)?;
            }
            i -= 1;
        }

        let mut i = cs!(scip).ndualsolconfs - 1;
        while i >= 0 {
            if cs!(scip).dsolrelaxonly[i] {
                del_pos_dualsol(scip, transprob, i, true)?;
            } else {
                cs!(scip).updateside[i] = false;
            }
            i -= 1;
        }
    }
    Ok(())
}

/// Removes the oldest stored proof of the same locality as `dualproof` to
/// make room (the full-store branch of the two add-proof functions);
/// returns false if the new proof was deleted instead.
fn make_room_for_proof(scip: &mut Scip, transprob: usize, dualproof: ConsId, dualray: bool) -> ScipResult<bool> {
    let local = scip.conss[dualproof].local;
    let mut pos = 0;
    let n = if dualray { cs!(scip).ndualrayconfs } else { cs!(scip).ndualsolconfs };

    // Sort dual rays.
    if dualray {
        let mut confs = std::mem::take(&mut cs!(scip).dualrayconfs);
        let mut relax = std::mem::take(&mut cs!(scip).drayrelaxonly);
        crate::sort::sort(&mut confs.0[..n as usize], (&mut relax.0[..n as usize],), |&a, &b| compare_conss(scip, a, b));
        cs!(scip).dualrayconfs = confs;
        cs!(scip).drayrelaxonly = relax;
    } else {
        let c = &mut cs!(scip);
        let mut confs = std::mem::take(&mut c.dualsolconfs);
        let mut bnds = std::mem::take(&mut c.dualprimalbnds);
        let mut scales = std::mem::take(&mut c.scalefactors);
        let mut upd = std::mem::take(&mut c.updateside);
        let mut relax = std::mem::take(&mut c.dsolrelaxonly);
        let k = n as usize;
        crate::sort::sort(&mut confs.0[..k], (&mut bnds.0[..k], &mut scales.0[..k], &mut upd.0[..k], &mut relax.0[..k]), |&a, &b| {
            compare_conss(scip, a, b)
        });
        let c = &mut cs!(scip);
        c.dualsolconfs = confs;
        c.dualprimalbnds = bnds;
        c.scalefactors = scales;
        c.updateside = upd;
        c.dsolrelaxonly = relax;
    }

    let confs = if dualray { &cs!(scip).dualrayconfs } else { &cs!(scip).dualsolconfs };
    while pos < n - 1 && local != scip.conss[confs[pos]].local {
        pos += 1;
    }

    // We don't want to keep the dual proof.
    if pos >= n {
        cons::cons_delete(scip, dualproof, transprob)?;
        return Ok(false);
    }

    if dualray {
        del_pos_dualray(scip, transprob, pos, true)?;
    } else {
        del_pos_dualsol(scip, transprob, pos, true)?;
    }
    Ok(true)
}

/// `SCIPconflictstoreAddDualraycons`: adds a constraint to the pool of
/// proof constraints based on dual rays; this methods captures the
/// constraint. `hasrelaxvar` says whether the dual proof contains at least
/// one variable that exists in the current relaxation only.
pub fn conflictstore_add_dualraycons(scip: &mut Scip, dualproof: ConsId, transprob: usize, hasrelaxvar: bool) -> ScipResult {
    // Mark the constraint to be a conflict.
    cons::cons_mark_conflict(scip, dualproof);

    // Create an array to store constraints based on dual rays.
    if cs!(scip).dualrayconfs.is_empty() {
        cs!(scip).dualrayconfs = crate::cvec![NONE; CONFLICTSTORE_DUALRAYSIZE as usize];
        cs!(scip).drayrelaxonly = CVec::with_size(CONFLICTSTORE_DUALRAYSIZE);
    }

    // The store is full, we proceed as follows:
    //
    // 1. check whether some constraints are marked as deleted and remove
    //    those
    // 2. if no constraint is marked as deleted: remove the oldest
    if cs!(scip).ndualrayconfs == CONFLICTSTORE_DUALRAYSIZE {
        // Remove all as deleted marked dual infeasibility proofs.
        let ndeleted = clean_deleted_and_checked_dualray_cons(scip)?;

        // If we could not remove a dual ray that is already marked as
        // deleted we need to remove the oldest active one.
        if ndeleted == 0 && !make_room_for_proof(scip, transprob, dualproof, true)? {
            return Ok(());
        }
    }

    // Add the new constraint based on a dual ray at the last position.
    cons::cons_capture(scip, dualproof);
    let c = &mut cs!(scip);
    c.dualrayconfs[c.ndualrayconfs] = dualproof;
    c.drayrelaxonly[c.ndualrayconfs] = hasrelaxvar;
    c.ndualrayconfs += 1;

    // Add conflict locks.
    cons::cons_add_locks(scip, dualproof, LockType::Conflict, 1, 0)?;

    // Implement dualraycons upgrade.
    scip.conss[dualproof].add_upgrade_locks(1);

    // Increase the number of non-zeros.
    let (nvars, _success) = cons::cons_get_n_vars(scip, dualproof)?;
    cs!(scip).nnzdualrays += nvars as i64;
    Ok(())
}

/// `SCIPconflictstoreAddDualsolcons`: adds a constraint to the pool of
/// proof constraints based on dual solutions; this methods captures the
/// constraint.
pub fn conflictstore_add_dualsolcons(scip: &mut Scip, dualproof: ConsId, transprob: usize, scale: f64, updateside: bool, hasrelaxvar: bool) -> ScipResult {
    // Mark the constraint to be a conflict.
    cons::cons_mark_conflict(scip, dualproof);

    // Create an array to store constraints based on dual rays.
    if cs!(scip).dualsolconfs.is_empty() {
        let c = &mut cs!(scip);
        c.dualsolconfs = crate::cvec![NONE; CONFLICTSTORE_DUALSOLSIZE as usize];
        c.dualprimalbnds = CVec::with_size(CONFLICTSTORE_DUALSOLSIZE);
        c.scalefactors = CVec::with_size(CONFLICTSTORE_DUALSOLSIZE);
        c.updateside = CVec::with_size(CONFLICTSTORE_DUALSOLSIZE);
        c.dsolrelaxonly = CVec::with_size(CONFLICTSTORE_DUALSOLSIZE);
    }

    // The store is full, we proceed as follows:
    //
    // 1. check whether some constraints are marked as deleted and remove
    //    those
    // 2. if no constraint is marked as deleted: remove the oldest
    if cs!(scip).ndualsolconfs == CONFLICTSTORE_DUALSOLSIZE {
        // Remove all as deleted marked dual bound exceeding proofs.
        let ndeleted = clean_deleted_and_checked_dualsol_cons(scip)?;

        // If we could not remove a dual proof that is already marked as
        // deleted we need to remove the oldest active one.
        if ndeleted == 0 && !make_room_for_proof(scip, transprob, dualproof, false)? {
            return Ok(());
        }
    }

    // Add the new constraint based on a dual solution at the last position.
    cons::cons_capture(scip, dualproof);
    let cutoffbound = scip.primals[crate::scip::TRANSPRIMAL].cutoffbound;
    let delta = if scip.probs[transprob].objisintegral { scip.set.cutoffbounddelta() } else { 0.0 };
    let c = &mut cs!(scip);
    let n = c.ndualsolconfs;
    c.dualsolconfs[n] = dualproof;
    c.dualprimalbnds[n] = cutoffbound - delta;
    c.scalefactors[n] = scale;
    c.updateside[n] = updateside;
    c.dsolrelaxonly[n] = hasrelaxvar;
    c.ndualsolconfs += 1;

    // Add conflict locks.
    cons::cons_add_locks(scip, dualproof, LockType::Conflict, 1, 0)?;

    // Implement dualsolcons upgrade.
    scip.conss[dualproof].add_upgrade_locks(1);

    // Increase the number of non-zeros.
    let (nvars, _success) = cons::cons_get_n_vars(scip, dualproof)?;
    cs!(scip).nnzdualsols += nvars as i64;
    Ok(())
}

/// `SCIPconflictstoreAddConflict`: adds a conflict to the conflict store;
/// this method captures the constraint.
pub fn conflictstore_add_conflict(scip: &mut Scip, transprob: usize, cons: ConsId, _conftype: ConfType, _cutoffinvolved: bool, primalbound: f64) -> ScipResult {
    // Mark the constraint to be a conflict.
    cons::cons_mark_conflict(scip, cons);

    // Add the constraint to a special store.
    if scip.conss[cons].original {
        conflictstore_add_orig_conflict(scip, cons);
        return Ok(());
    }

    let nconflicts = cs!(scip).nconflicts;

    // Initialize the storage.
    if cs!(scip).maxstoresize == -1 {
        init_conflictstore(scip, transprob)?;
    }

    // Return if conflict pool is disabled.
    if cs!(scip).maxstoresize <= 0 {
        return Ok(());
    }

    conflictstore_ensure_mem(scip, nconflicts + 1);

    // Return if the store has size zero.
    if cs!(scip).conflictsize == 0 {
        return Ok(());
    }

    let focusnode = crate::tree::tree_get_focus_node(scip);
    let curnodenum = if focusnode == NONE { -1 } else { scip.nodes[focusnode].number };

    // Clean up the storage if we are at a new node or the storage is full.
    if cs!(scip).lastnodenum != curnodenum || cs!(scip).nconflicts == cs!(scip).conflictsize {
        conflictstore_clean_up_storage(scip, transprob)?;
    }

    // Update the last seen node.
    cs!(scip).lastnodenum = curnodenum;

    // Validate conflict position.
    scip.conss[cons].confconsspos = cs!(scip).nconflicts;

    cons::cons_capture(scip, cons);
    let isinf = scip.set.is_infinity(primalbound.abs());
    let c = &mut cs!(scip);
    c.conflicts[c.nconflicts] = cons;
    c.confprimalbnds[c.nconflicts] = primalbound;
    c.ncbconflicts += if isinf { 0 } else { 1 };

    c.nconflicts += 1;
    c.nconflictsfound += 1;

    // Add conflict locks.
    cons::cons_add_locks(scip, cons, LockType::Conflict, 1, 0)?;
    Ok(())
}

/// `SCIPconflictstoreUpgradeConflict`: upgrades an unchecked conflict in
/// the conflict store; this method releases oldcons and captures newcons.
pub fn conflictstore_upgrade_conflict(scip: &mut Scip, oldcons: ConsId, newcons: ConsId) -> ScipResult {
    // Multiple upgrades unsupported.
    let pos = scip.conss[oldcons].confconsspos;
    if pos == -1 || scip.conss[oldcons].check {
        return Ok(());
    }

    // Mark constraint conflict.
    cons::cons_mark_conflict(scip, newcons);

    // Replace conflict constraint.
    cs!(scip).conflicts[pos] = newcons;
    scip.conss[newcons].confconsspos = pos;
    scip.conss[oldcons].confconsspos = -1;

    // Transfer conflict locks.
    cons::cons_add_locks(scip, newcons, LockType::Conflict, 1, 0)?;
    cons::cons_add_locks(scip, oldcons, LockType::Conflict, -1, 0)?;

    // Update conflict usage.
    cons::cons_capture(scip, newcons);
    cons::cons_release(scip, oldcons)?;
    Ok(())
}

/// `SCIPconflictstoreCleanNewIncumbent`: deletes all conflicts depending on
/// a cutoff bound larger than the given bound.
pub fn conflictstore_clean_new_incumbent(scip: &mut Scip, transprob: usize, cutoffbound: f64) -> ScipResult {
    // Return if we do not want to use the storage.
    if scip.set.conf_maxstoresize == 0 {
        return Ok(());
    }

    // Return if we do not want to remove conflicts related to an older
    // cutoff bound.
    if !scip.set.conf_cleanbnddepend {
        return Ok(());
    }

    // There is nothing to clean.
    if cs!(scip).ndualsolconfs == 0 && cs!(scip).nconflicts == 0 {
        return Ok(());
    }

    // We can stop whenever we have found a new incumbent but the cutoff bound
    // has not changed.
    let last = cs!(scip).lastcutoffbound;
    if last != crate::def::INVALID && scip.set.is_ge(cutoffbound, last) {
        return Ok(());
    }

    cs!(scip).lastcutoffbound = cutoffbound;

    // Calculate scalar to determine whether the old primal bound is worse
    // enough to remove the conflict.
    let improvement = if scip.set.is_positive(cutoffbound) {
        1.0 - scip.set.conf_minimprove
    } else {
        1.0 + scip.set.conf_minimprove
    };

    // Remove all conflicts depending on a primalbound*improvement >
    // cutoffbound.
    //
    // Note: we cannot remove conflicts that are marked as deleted because at
    // this point in time we would destroy the internal data structure.
    let mut i = 0;
    while i < cs!(scip).nconflicts {
        // Check if the conflict depends on the cutoff bound.
        if scip.set.is_gt(improvement * cs!(scip).confprimalbnds[i], cutoffbound) {
            // Remove conflict at current position; don't increase i because
            // delPosConflict will swap the last pointer to the i-th position.
            del_pos_conflict(scip, transprob, i, true)?;
        } else {
            // Increase i.
            i += 1;
        }
    }

    // Update all proof constraints based on a dual solution.
    let mut i = 0;
    while i < cs!(scip).ndualsolconfs {
        let dualproof = cs!(scip).dualsolconfs[i];

        if scip.conss[dualproof].deleted {
            i += 1;
            continue;
        }

        if !cs!(scip).updateside[i] || scip.set.is_le(improvement * cs!(scip).dualprimalbnds[i], cutoffbound) {
            i += 1;
            continue;
        }

        let conshdlr = scip.conss[dualproof].conshdlr;
        let delta = if scip.probs[transprob].objisintegral { scip.set.cutoffbounddelta() } else { 0.0 };

        if scip.conshdlrs[conshdlr].name == "linear" {
            let rhs = crate::cons_linear::get_rhs_linear(scip, dualproof);
            let scale = cs!(scip).scalefactors[i];
            let dualprimalbnd = cs!(scip).dualprimalbnds[i];

            if !scip.set.is_infinity(rhs) {
                // Get unscaled rhs.
                let mut newside = rhs * scale;
                newside -= dualprimalbnd;
                newside += cutoffbound - delta;

                // Scale rhs.
                newside /= scale;

                crate::cons_linear::chg_rhs_linear(scip, dualproof, newside)?;
            } else {
                let lhs = crate::cons_linear::get_lhs_linear(scip, dualproof);

                // Get unscaled lhs.
                let mut newside = lhs * scale;
                newside += dualprimalbnd;
                newside -= cutoffbound - delta;

                // Scale lhs.
                newside /= scale;

                crate::cons_linear::chg_lhs_linear(scip, dualproof, newside)?;
            }

            cs!(scip).dualprimalbnds[i] = cutoffbound - delta;

            i += 1;
        } else if scip.set.is_gt(improvement * cs!(scip).dualprimalbnds[i], cutoffbound) {
            // Remove conflict at current position; don't increase i because
            // delPosDualsol will swap the last pointer to the i-th position.
            del_pos_dualsol(scip, transprob, i, true)?;
        } else {
            // Increase i.
            i += 1;
        }
    }
    Ok(())
}

/// `SCIPconflictstoreGetMaxPoolSize`: returns the maximal size of the
/// conflict pool.
pub fn conflictstore_get_max_pool_size(scip: &Scip) -> i32 {
    let c = scip.conflictstore.as_ref().unwrap();
    c.storesize.min(c.maxstoresize)
}

/// `SCIPconflictstoreGetConflicts`: returns all active conflicts stored in
/// the conflict store.
pub fn conflictstore_get_conflicts(scip: &Scip) -> Vec<ConsId> {
    let c = scip.conflictstore.as_ref().unwrap();
    c.conflicts
        .head(c.nconflicts)
        .iter()
        .copied()
        .filter(|&k| scip.conss[k].is_active() && !scip.conss[k].deleted)
        .collect()
}

/// `SCIPconflictstoreTransform`: transformes all original conflicts into
/// transformed conflicts.
pub fn conflictstore_transform(scip: &mut Scip, transprob: usize) -> ScipResult {
    // Return if no original constraints are stored.
    if cs!(scip).norigconfs == 0 {
        return Ok(());
    }

    for i in 0..cs!(scip).norigconfs {
        let orig = cs!(scip).origconfs[i];
        let transcons = scip.conss[orig].transorigcons;

        if transcons != NONE {
            let inf = scip.set.infinity();
            conflictstore_add_conflict(scip, transprob, transcons, ConfType::Unknown, false, -inf)?;
        }

        cons::cons_release(scip, orig)?;
    }
    cs!(scip).norigconfs = 0;
    Ok(())
}

/// `SCIPconflictstoreGetNDualInfProofs`: returns the number of all stored
/// dual ray constraints.
pub fn conflictstore_get_n_dual_inf_proofs(scip: &Scip) -> i32 {
    scip.conflictstore.as_ref().unwrap().ndualrayconfs
}

/// `SCIPconflictstoreGetAvgNnzDualInfProofs`: returns the average number
/// of non-zeros over all stored dual ray constraints.
pub fn conflictstore_get_avg_nnz_dual_inf_proofs(scip: &Scip) -> f64 {
    let s = scip.conflictstore.as_ref().unwrap();
    if s.ndualrayconfs == 0 { 0.0 } else { s.nnzdualrays as f64 / s.ndualrayconfs as f64 }
}

/// `SCIPconflictstoreGetNDualBndProofs`: returns the number of all stored
/// dual solution constraints.
pub fn conflictstore_get_n_dual_bnd_proofs(scip: &Scip) -> i32 {
    scip.conflictstore.as_ref().unwrap().ndualsolconfs
}

/// `SCIPconflictstoreGetAvgNnzDualBndProofs`: returns the average number of
/// non-zeros over all stored boundexceeding proofs.
pub fn conflictstore_get_avg_nnz_dual_bnd_proofs(scip: &Scip) -> f64 {
    let s = scip.conflictstore.as_ref().unwrap();
    if s.ndualsolconfs == 0 { 0.0 } else { s.nnzdualsols as f64 / s.ndualsolconfs as f64 }
}

/// `SCIPconflictstoreGetNConflictsInStore`: returns the number of stored
/// conflicts on the conflict pool.
pub fn conflictstore_get_n_conflicts_in_store(scip: &Scip) -> i32 {
    scip.conflictstore.as_ref().unwrap().nconflicts
}
