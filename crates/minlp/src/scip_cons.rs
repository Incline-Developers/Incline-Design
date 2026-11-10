//! SCIP's constraint methods of the public API (`scip_cons.c`, and the
//! constraint parts of `scip_prob.c`): adding constraints to the problem or
//! to nodes, capturing, releasing and deleting them.
//!
//! Exact mode and reoptimization are not ported.

use crate::{
    cons::{cons_capture, cons_delete, cons_release, cons_set_local},
    prob::{ORIGPROB, TRANSPROB, prob_add_cons},
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    tree::{node_add_cons, tree_get_current_depth, tree_get_current_node},
    types::{ConsId, NodeId},
    var::NONE,
};

/// `SCIPaddCons` (`scip_prob.c`): adds constraint to the problem; if
/// constraint is only valid locally, it is added to the local subproblem
/// of the current node (and all of its subnodes); otherwise it is added
/// to the global problem.
pub fn add_cons(scip: &mut Scip, cons: ConsId) -> ScipResult {
    match scip.set.stage {
        Stage::Problem => prob_add_cons(scip, ORIGPROB, cons),
        Stage::Transformed => prob_add_cons(scip, TRANSPROB, cons),
        Stage::InitPresolve | Stage::Presolving | Stage::ExitPresolve | Stage::Presolved | Stage::InitSolve | Stage::Solving => {
            if tree_get_current_depth(scip) <= treer!(scip).effectiverootdepth {
                cons_set_local(scip, cons, false);
            }
            if scip.conss[cons].is_global() {
                prob_add_cons(scip, TRANSPROB, cons)
            } else {
                let node = tree_get_current_node(scip);
                node_add_cons(scip, node, cons)
            }
        }
        Stage::ExitSolve => prob_add_cons(scip, TRANSPROB, cons),
        _ => Err(Retcode::InvalidCall),
    }
}

/// `SCIPdelCons` (`scip_prob.c`): globally removes constraint from all
/// subproblems; removes constraint from the constraint set change data of
/// the node, where it was added, or from the problem, if it was a
/// problem constraint.
pub fn del_cons(scip: &mut Scip, cons: ConsId) -> ScipResult {
    match scip.set.stage {
        Stage::Problem => cons_delete(scip, cons, ORIGPROB),
        Stage::Transformed | Stage::InitPresolve | Stage::ExitPresolve | Stage::Presolving | Stage::InitSolve | Stage::Solving | Stage::ExitSolve => {
            cons_delete(scip, cons, TRANSPROB)
        }
        _ => Err(Retcode::InvalidCall),
    }
}

/// `SCIPaddConsNode` (`scip_prob.c`): adds constraint to the given node
/// (and all of its subnodes), even if it is a global constraint; it is
/// valid in `validnode` (`NONE` for the node itself).
pub fn add_cons_node(scip: &mut Scip, node: NodeId, cons: ConsId, validnode: NodeId) -> ScipResult {
    let nodedepth = scip.nodes[node].depth as i32;
    if validnode != NONE {
        let validdepth = scip.nodes[validnode].depth as i32;
        if validdepth > nodedepth {
            // Cannot add constraint valid in a deeper depth to a node.
            return Err(Retcode::InvalidData);
        }
        if scip.conss[cons].validdepth != -1 && scip.conss[cons].validdepth != validdepth {
            // Constraint is already marked to be valid in another depth.
            return Err(Retcode::InvalidData);
        }
        if validdepth <= treer!(scip).effectiverootdepth {
            cons_set_local(scip, cons, false);
        } else {
            scip.conss[cons].validdepth = validdepth;
        }
    }

    if nodedepth <= treer!(scip).effectiverootdepth {
        cons_set_local(scip, cons, false);
        prob_add_cons(scip, TRANSPROB, cons)
    } else {
        node_add_cons(scip, node, cons)
    }
}

/// `SCIPaddConsLocal` (`scip_prob.c`): adds constraint locally to the
/// current node (and all of its subnodes), even if it is a global
/// constraint.
pub fn add_cons_local(scip: &mut Scip, cons: ConsId, validnode: NodeId) -> ScipResult {
    let node = tree_get_current_node(scip);
    add_cons_node(scip, node, cons, validnode)
}

/// `SCIPcaptureCons` (`scip_cons.c`): increases usage counter of
/// constraint.
pub fn capture_cons(scip: &mut Scip, cons: ConsId) {
    cons_capture(scip, cons);
}

/// `SCIPreleaseCons` (`scip_cons.c`): decreases usage counter of
/// constraint, if the usage pointer reaches zero the constraint gets
/// freed; sets `*cons` to `NONE`.
pub fn release_cons(scip: &mut Scip, cons: &mut ConsId) -> ScipResult {
    match scip.set.stage {
        Stage::Problem => {}
        Stage::Transforming
        | Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::InitSolve
        | Stage::Solving
        | Stage::Solved
        | Stage::ExitSolve
        | Stage::FreeTrans => {
            let c = &scip.conss[*cons];
            if c.original && c.nuses == 1 && c.transorigcons != NONE {
                // Cannot release last use of original constraint while an
                // associated transformed constraint exists.
                return Err(Retcode::InvalidCall);
            }
        }
        _ => return Err(Retcode::InvalidCall),
    }
    cons_release(scip, *cons)?;
    *cons = NONE;
    Ok(())
}

/// `SCIPdelConsLocal` (`scip_prob.c`): disables constraint's separation,
/// enforcing, and propagation capabilities at the current node (and all
/// subnodes); if the current node is the root node, or if the method is
/// called during problem modification or presolving, the constraint is
/// globally deleted from the problem.
pub fn del_cons_local(scip: &mut Scip, cons: ConsId) -> ScipResult {
    match scip.set.stage {
        Stage::Problem => cons_delete(scip, cons, ORIGPROB),
        Stage::InitPresolve | Stage::ExitPresolve | Stage::Presolving | Stage::Solving => {
            let node = tree_get_current_node(scip);
            if scip.nodes[node].depth as i32 <= treer!(scip).effectiverootdepth {
                cons_delete(scip, cons, TRANSPROB)
            } else {
                crate::tree::node_del_cons(scip, node, cons)
            }
        }
        _ => Err(Retcode::InvalidCall),
    }
}

/// `SCIPsetConsChecked` (`scip_cons.c`): sets the check flag of the given
/// constraint.
pub fn set_cons_checked(scip: &mut Scip, cons: ConsId, check: bool) -> ScipResult {
    crate::cons::cons_set_checked(scip, cons, check)
}

/// `SCIPsetConsEnforced` (`scip_cons.c`): sets the enforce flag of the
/// given constraint.
pub fn set_cons_enforced(scip: &mut Scip, cons: ConsId, enforce: bool) {
    crate::cons::cons_set_enforced(scip, cons, enforce);
}

/// `SCIPsetConsPropagated` (`scip_cons.c`): sets the propagate flag of the
/// given constraint.
pub fn set_cons_propagated(scip: &mut Scip, cons: ConsId, propagate: bool) {
    crate::cons::cons_set_propagated(scip, cons, propagate);
}

/// `SCIPsetConsInitial` (`scip_cons.c`): sets the initial flag of the
/// given constraint.
pub fn set_cons_initial(scip: &mut Scip, cons: ConsId, initial: bool) {
    crate::cons::cons_set_initial(scip, cons, initial);
}

/// `SCIPsetConsSeparated` (`scip_cons.c`): sets the separate flag of the
/// given constraint.
pub fn set_cons_separated(scip: &mut Scip, cons: ConsId, separate: bool) {
    crate::cons::cons_set_separated(scip, cons, separate);
}

/// `SCIPaddConsUpgrade` (`scip_prob.c`): adds a constraint upgraded from
/// `oldcons` to the problem the same way (globally or locally), carries
/// over its conflict store entry, and releases `newcons`.
pub fn add_cons_upgrade(scip: &mut Scip, oldcons: ConsId, newcons: &mut ConsId) -> ScipResult {
    if scip.conss[oldcons].is_global() {
        add_cons(scip, *newcons)?;
    } else {
        add_cons_local(scip, *newcons, NONE)?;
    }

    if scip.conss[oldcons].conflict {
        crate::conflictstore::conflictstore_upgrade_conflict(scip, oldcons, *newcons)?;
    }

    release_cons(scip, newcons)
}

/// `SCIPupdateConsFlags` (`scip_cons.c`): updates the flags of the first
/// constraint according to the ones of the second constraint.
pub fn update_cons_flags(scip: &mut Scip, cons0: ConsId, cons1: ConsId) -> ScipResult {
    let c1 = &scip.conss[cons1];
    let (initial, separated, enforced, checked, propagated, dynamic, removable, sticking) = (
        c1.is_initial(),
        c1.is_separated(),
        c1.is_enforced(),
        c1.is_checked(),
        c1.is_propagated(),
        c1.is_dynamic(),
        c1.is_removable(),
        c1.is_sticking_at_node(),
    );

    if initial {
        set_cons_initial(scip, cons0, true);
    }
    if separated {
        set_cons_separated(scip, cons0, true);
    }
    if enforced {
        set_cons_enforced(scip, cons0, true);
    }
    if checked {
        set_cons_checked(scip, cons0, true)?;
    }
    if propagated {
        set_cons_propagated(scip, cons0, true);
    }
    if !dynamic {
        scip.conss[cons0].dynamic = false;
    }
    if !removable {
        scip.conss[cons0].removable = false;
    }
    if sticking {
        scip.conss[cons0].stickingatnode = true;
    }
    Ok(())
}

/// `SCIPcreateCons` (`scip_cons.c`): creates and captures a constraint of
/// the given constraint handler.
pub fn create_cons(
    scip: &mut Scip,
    name: &str,
    conshdlr: crate::types::ConshdlrId,
    consdata: Option<Box<dyn std::any::Any>>,
    flags: &crate::cons::ConsFlags,
) -> ScipResult<ConsId> {
    match scip.set.stage {
        Stage::Problem => crate::cons::cons_create(scip, name, conshdlr, consdata, flags, true, true),
        Stage::Transforming
        | Stage::Transformed
        | Stage::InitPresolve
        | Stage::Presolving
        | Stage::ExitPresolve
        | Stage::Presolved
        | Stage::InitSolve
        | Stage::Solving
        | Stage::ExitSolve => crate::cons::cons_create(scip, name, conshdlr, consdata, flags, false, true),
        _ => Err(Retcode::InvalidCall),
    }
}

/// `takeCut` (`scip_copy.c`): returns whether a cut of the pool is taken
/// when cuts are converted to constraints.
fn take_cut(scip: &Scip, cut: usize, cutsel: u8) -> bool {
    let row = scip.cuts[cut].row;

    // If the cut is not in the LP, it is not taken.
    if scip.rows[row].lppos < 0 {
        return false;
    }

    match cutsel {
        b'a' => scip.cuts[cut].age == 0,
        b'q' => crate::cutpool::cut_get_lp_activity_quot(scip, cut) >= scip.set.sepa_minactivityquot,
        _ => panic!("unknown cut selection strategy"),
    }
}

/// `copyCuts` (`scip_copy.c`) within one SCIP: turns the selected cuts of
/// a pool into linear constraints.
fn copy_cuts(scip: &mut Scip, pool: usize, ncutsadded: &mut i32) -> ScipResult {
    let ncuts = scip.cutpools[pool].ncuts as usize;
    let cuts = scip.cutpools[pool].cuts.0[..ncuts].to_vec();
    for cut in cuts {
        let row = scip.cuts[cut].row;

        let mut takecut = take_cut(scip, cut, scip.set.sepa_cutselrestart);

        // Create a linear constraint out of the cut.
        if takecut {
            let r = &scip.rows[row];
            let ncols = r.len as usize;
            let mut vars = Vec::with_capacity(ncols);
            let mut i = 0;
            while i < ncols && takecut {
                let var = scip.cols[r.cols[i]].var;
                vars.push(var);
                takecut = !scip.vars[var].relaxationonly;
                i += 1;
            }
            if !takecut {
                continue;
            }

            let name = format!("{}_{}", r.name, scip.stat.nruns);
            let vals = r.vals.0[..ncols].to_vec();
            let (lhs, rhs) = (r.lhs - r.constant, r.rhs - r.constant);
            let mut cons = crate::cons_linear::create_cons_linear(scip, &name, &vars, &vals, lhs, rhs, true, true, false, false, true, false, false, false, true, false)?;
            add_cons(scip, cons)?;
            release_cons(scip, &mut cons)?;

            *ncutsadded += 1;
        }
    }
    Ok(())
}

/// `SCIPconvertCutsToConss` (`scip_copy.c`): converts the active cuts of
/// the global and delayed cut pools into linear constraints of the same
/// SCIP (the restart case; copying into sub-SCIPs is not ported); returns
/// the number of cuts added.
pub fn convert_cuts_to_conss(scip: &mut Scip) -> ScipResult<i32> {
    let mut ncutsadded = 0;

    if scip.set.stage < Stage::Solving {
        return Ok(0);
    }

    // If we do not have any cuts, nothing can be converted.
    if crate::set::set_find_conshdlr(scip, crate::cons_linear::CONSHDLR_NAME) == NONE {
        return Ok(0);
    }

    // Convert cut from global cut pool.
    copy_cuts(scip, crate::cutpool::GLOBALCUTPOOL, &mut ncutsadded)?;

    // Convert delayed cuts from global delayed cut pool.
    copy_cuts(scip, crate::cutpool::DELAYEDCUTPOOL, &mut ncutsadded)?;

    Ok(ncutsadded)
}

/// `SCIPaddConflict` (`scip_prob.c`): adds a conflict to a given node or
/// globally (`node == NONE`) to the problem and the conflict store, and
/// releases the constraint.
pub fn add_conflict(scip: &mut Scip, node: NodeId, cons: &mut ConsId, validnode: NodeId, conftype: crate::types::ConfType, iscutoffinvolved: bool) -> ScipResult {
    // Mark constraint to be a conflict.
    crate::cons::cons_mark_conflict(scip, *cons);

    // Add the conflict to the problem or node.
    let primalbound = if iscutoffinvolved { crate::api::get_cutoffbound(scip) } else { -scip.set.infinity() };

    if node == NONE {
        add_cons(scip, *cons)?;
    } else {
        add_cons_node(scip, node, *cons, validnode)?;
    }

    // Add the conflict to the conflict store.
    if node == NONE || scip.nodes[node].nodetype != crate::types::NodeType::ProbingNode {
        crate::conflictstore::conflictstore_add_conflict(scip, TRANSPROB, *cons, conftype, iscutoffinvolved, primalbound)?;
    }

    // Release the constraint.
    release_cons(scip, cons)
}

/// `SCIPdelConsNode` (`scip_prob.c`): disables constraint's separation,
/// enforcing, and propagation capabilities at the given node (and all
/// subnodes); at or above the effective root depth the constraint is deleted
/// globally.
pub fn del_cons_node(scip: &mut Scip, node: NodeId, cons: ConsId) -> ScipResult {
    if (scip.nodes[node].depth as i32) <= crate::tree::tree_get_effective_root_depth(scip) {
        cons_delete(scip, cons, TRANSPROB)
    } else {
        crate::tree::node_del_cons(scip, node, cons)
    }
}
