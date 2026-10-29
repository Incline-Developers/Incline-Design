//! SCIP's separation storage (`sepastore.c`, `struct_sepastore.h`): the
//! cuts found in a separation round, before they enter the LP.

use crate::{
    cvec::CVec,
    event, lp,
    misc::RandNumGen,
    retcode::ScipResult,
    scip::Scip,
    tree,
    types::{BoundType, EfficiacyChoice, RowId, RowOriginType, VarId},
};

/// The main separation storage in `Scip::sepastores`.
pub const SEPASTORE: usize = 0;
/// The separation storage used during probing (`scip->sepastoreprobing`).
pub const SEPASTOREPROBING: usize = 1;

/// `SCIP_SEPASTORE`.
#[derive(Clone, Debug)]
pub struct SepaStore {
    pub cuts: CVec<RowId>,
    pub randnumgen: RandNumGen,
    pub cutssize: i32,
    pub ncuts: i32,
    pub nforcedcuts: i32,
    pub ncutsadded: i32,
    pub ncutsaddedviapool: i32,
    pub ncutsaddeddirect: i32,
    pub ncutsfoundround: i32,
    pub ncutsapplied: i32,
    pub initiallp: bool,
    pub forcecuts: bool,
}

macro_rules! ss {
    ($scip:expr, $s:expr) => {
        $scip.sepastores[$s]
    };
}

/*
 * dynamic memory arrays
 */

/// `sepastoreEnsureCutsMem`: resizes cuts and score arrays to be able to
/// store at least `num` entries.
fn sepastore_ensure_cuts_mem(scip: &mut Scip, s: usize, num: i32) {
    if num > ss!(scip, s).cutssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        ss!(scip, s).cuts.0.resize(newsize as usize, crate::var::NONE);
        ss!(scip, s).cutssize = newsize;
    }
}

/// `SCIPsepastoreCreate`: creates separation storage.
pub fn sepastore_create(scip: &Scip) -> SepaStore {
    SepaStore {
        cuts: CVec::new(),
        cutssize: 0,
        ncuts: 0,
        nforcedcuts: 0,
        ncutsadded: 0,
        ncutsaddedviapool: 0,
        ncutsaddeddirect: 0,
        ncutsfoundround: 0,
        ncutsapplied: 0,
        initiallp: false,
        forcecuts: false,
        randnumgen: RandNumGen::new(scip.set.initialize_random_seed(0x5EED)),
    }
}

/// `SCIPsepastoreStartInitialLP`: informs separation storage that the
/// setup of the initial LP starts now.
pub fn sepastore_start_initial_lp(scip: &mut Scip, s: usize) {
    ss!(scip, s).initiallp = true;
}

/// `SCIPsepastoreEndInitialLP`: informs separation storage that the setup
/// of the initial LP is now finished.
pub fn sepastore_end_initial_lp(scip: &mut Scip, s: usize) {
    ss!(scip, s).initiallp = false;
}

/// `SCIPsepastoreStartForceCuts`: informs separation storage that the
/// following cuts should be used in any case.
pub fn sepastore_start_force_cuts(scip: &mut Scip, s: usize) {
    ss!(scip, s).forcecuts = true;
}

/// `SCIPsepastoreEndForceCuts`: informs separation storage that the
/// following cuts should no longer be used in any case.
pub fn sepastore_end_force_cuts(scip: &mut Scip, s: usize) {
    ss!(scip, s).forcecuts = false;
}

/// `sepastoreIsCutRedundant`: checks cut for redundancy due to activity
/// bounds.
fn sepastore_is_cut_redundant(scip: &mut Scip, cut: RowId) -> bool {
    // Modifiable cuts cannot be declared redundant, since we don't know all
    // coefficients.
    if scip.rows[cut].modifiable {
        return false;
    }

    // Check for activity redundancy.
    let lhs = scip.rows[cut].lhs;
    let rhs = scip.rows[cut].rhs;
    let minactivity = lp::row_get_min_activity(scip, cut);
    let maxactivity = lp::row_get_max_activity(scip, cut);

    (scip.set.is_infinity(-lhs) || scip.set.is_le(lhs, minactivity)) && (scip.set.is_infinity(rhs) || scip.set.is_le(maxactivity, rhs))
}

/// `sepastoreIsCutRedundantOrInfeasible`: checks cut for redundancy or
/// infeasibility due to activity bounds; returns `(redundant,
/// infeasible)`.
fn sepastore_is_cut_redundant_or_infeasible(scip: &mut Scip, cut: RowId) -> (bool, bool) {
    // Modifiable cuts cannot be declared redundant or infeasible, since we
    // don't know all coefficients.
    if scip.rows[cut].modifiable {
        return (false, false);
    }

    // Check for activity redundancy.
    let lhs = scip.rows[cut].lhs;
    let rhs = scip.rows[cut].rhs;
    let minactivity = lp::row_get_min_activity(scip, cut);
    let maxactivity = lp::row_get_max_activity(scip, cut);

    if (scip.set.is_infinity(-lhs) || scip.set.is_le(lhs, minactivity)) && (scip.set.is_infinity(rhs) || scip.set.is_le(maxactivity, rhs)) {
        return (true, false);
    }

    if (!scip.set.is_infinity(rhs) && scip.set.is_feas_positive(minactivity - rhs)) || (!scip.set.is_infinity(-lhs) && scip.set.is_feas_negative(maxactivity - lhs)) {
        return (true, true);
    }
    (false, false)
}

/// `sepastoreIsBdchgApplicable`: checks whether a cut with only one
/// variable can be applied as boundchange. This is the case if the bound
/// change would prove infeasibility (w.r.t feastol), or if the new bound is
/// at least epsilon better than the old bound. In the latter case, also
/// the opposite bound has to be taken into account.
fn sepastore_is_bdchg_applicable(scip: &Scip, cut: RowId) -> bool {
    let set = &scip.set;
    let row = &scip.rows[cut];

    // Get the single variable and its coefficient of the cut.
    let var = scip.cols[row.cols[0_usize]].var;
    let val = row.vals[0_usize];

    // If the coefficient is nearly zero, we better ignore this cut for
    // numerical reasons.
    if set.is_feas_zero(val) {
        return false;
    }

    let local = row.local;
    let v = &scip.vars[var];
    let oldlb = if local { v.locdom.lb } else { v.glbdom.lb };
    let oldub = if local { v.locdom.ub } else { v.glbdom.ub };

    let lb_applicable = |mut newlb: f64| {
        crate::var::var_adjust_lb(scip, var, &mut newlb);
        // Bound changes that improve the bound sufficiently are applicable.
        set.is_feas_gt(newlb, oldub) || set.is_gt(crate::def::min(newlb, oldub), oldlb)
    };
    let ub_applicable = |mut newub: f64| {
        crate::var::var_adjust_ub(scip, var, &mut newub);
        // Bound changes that improve the bound sufficiently are applicable.
        set.is_feas_lt(newub, oldlb) || set.is_lt(crate::def::max(newub, oldlb), oldub)
    };

    // Get the left hand side of the cut and convert it to a bound.
    let mut lhs = row.lhs;
    if !set.is_infinity(-lhs) {
        lhs -= row.constant;
        if val > 0.0 {
            // Coefficient is positive -> lhs corresponds to lower bound.
            if lb_applicable(lhs / val) {
                return true;
            }
        } else {
            // Coefficient is negative -> lhs corresponds to upper bound.
            if ub_applicable(lhs / val) {
                return true;
            }
        }
    }

    // Get the right hand side of the cut and convert it to a bound.
    let mut rhs = row.rhs;
    if !set.is_infinity(rhs) {
        rhs -= row.constant;
        if val > 0.0 {
            // Coefficient is positive -> rhs corresponds to upper bound.
            if ub_applicable(rhs / val) {
                return true;
            }
        } else {
            // Coefficient is negative -> rhs corresponds to lower bound.
            if lb_applicable(rhs / val) {
                return true;
            }
        }
    }
    false
}

/// Issues a ROWDELETEDSEPA event for `cut` if the filter tracks them.
fn issue_row_deleted_sepa(scip: &mut Scip, eventfilter: usize, cut: RowId) -> ScipResult {
    let f = &scip.eventfilters[eventfilter];
    if f.len > 0 && (f.eventmask & event::EVENTTYPE_ROWDELETEDSEPA) != 0 {
        let ev = event::event_create_row_deleted_sepa(cut);
        event::eventqueue_add_filter(scip, eventfilter, ev)?;
    }
    Ok(())
}

/// Undoes the found-cut statistics of `cut` when it leaves the storage.
fn uncount_cut(scip: &mut Scip, s: usize, cut: RowId) {
    if !ss!(scip, s).initiallp {
        let fromcutpool = scip.rows[cut].fromcutpool;
        let st = &mut ss!(scip, s);
        st.ncutsadded -= 1;
        if fromcutpool {
            st.ncutsaddedviapool -= 1;
        } else {
            st.ncutsaddeddirect -= 1;
        }

        if scip.rows[cut].origintype == RowOriginType::Sepa {
            let sepa = scip.rows[cut].origin;
            crate::sepa::sepa_dec_ncuts_added(scip, sepa, fromcutpool);
        }
    }
}

/// `sepastoreDelCut`: removes a non-forced cut from the separation
/// storage.
fn sepastore_del_cut(scip: &mut Scip, s: usize, eventfilter: usize, pos: i32) -> ScipResult {
    let cut = ss!(scip, s).cuts[pos];

    // Check, if the row deletions from separation storage events are tracked
    // if so, issue ROWDELETEDSEPA event.
    issue_row_deleted_sepa(scip, eventfilter, cut)?;

    // Release the row.
    uncount_cut(scip, s, cut);
    lp::row_release(scip, cut)?;

    // Move last cut to the empty position.
    let st = &mut ss!(scip, s);
    st.cuts[pos] = st.cuts[st.ncuts - 1];
    st.ncuts -= 1;
    Ok(())
}

/// `SCIPsepastoreAddCut`: adds cut to separation storage and captures it;
/// returns whether the cut was detected to be infeasible.
pub fn sepastore_add_cut(scip: &mut Scip, s: usize, eventfilter: usize, cut: RowId, forcecut: bool, root: bool) -> ScipResult<bool> {
    // The cut will be forced to enter the LP if the dual must be collected
    // and the initial LP is being constructed.
    let mut forcecut = forcecut || (scip.set.lp_alwaysgetduals && ss!(scip, s).initiallp);

    // In the root node, every local cut is a global cut, and global cuts are
    // nicer in many ways ...
    if root && scip.rows[cut].local {
        lp::row_chg_local(scip, cut, false);
    }

    // Check cut for redundancy or infeasibility.
    let (redundant, mut infeasible) = sepastore_is_cut_redundant_or_infeasible(scip, cut);

    // Note that we add infeasible rows in any case, since we cannot be sure
    // whether the return values are handled correctly. In this way, the LP
    // becomes infeasible.

    // In each separation round, make sure that at least one (even redundant)
    // cut enters the LP to avoid cycling.
    if !forcecut && ss!(scip, s).ncuts > 0 && redundant {
        return Ok(infeasible);
    }

    // If only one cut is currently present in sepastore, it could be
    // redundant; in this case, it can now be removed again, because now a
    // non redundant cut enters the sepastore.
    if ss!(scip, s).ncuts == 1 {
        let first = ss!(scip, s).cuts[0_usize];
        if sepastore_is_cut_redundant(scip, first) {
            // Check, if the row deletions from separation storage events are
            // tracked if so, issue ROWDELETEDSEPA event.
            issue_row_deleted_sepa(scip, eventfilter, first)?;

            // Update statistics of total number of found cuts.
            uncount_cut(scip, s, first);
            lp::row_release(scip, first)?;
            ss!(scip, s).ncuts = 0;
            ss!(scip, s).nforcedcuts = 0;
        }
    }

    // A cut is forced to enter the LP if
    //  - we construct the initial LP, or
    //  - it has infinite score factor, or
    //  - it is a bound change that can be applied
    // if it is a non-forced cut and no cuts should be added, abort.
    forcecut = forcecut || ss!(scip, s).initiallp || ss!(scip, s).forcecuts || (!scip.rows[cut].modifiable && scip.rows[cut].len == 1 && sepastore_is_bdchg_applicable(scip, cut));
    if !forcecut && scip.set.get_sepa_maxcuts(root) == 0 {
        return Ok(infeasible);
    }

    // Get enough memory to store the cut.
    let n = ss!(scip, s).ncuts;
    sepastore_ensure_cuts_mem(scip, s, n + 1);

    // Capture the cut.
    lp::row_capture(scip, cut);

    // Add cut to arrays.
    let st = &mut ss!(scip, s);
    let pos;
    if forcecut {
        // Make room at the beginning of the array for forced cut.
        pos = st.nforcedcuts;
        st.cuts[st.ncuts] = st.cuts[pos];
        st.nforcedcuts += 1;
    } else {
        pos = st.ncuts;
    }
    st.cuts[pos] = cut;

    // Update statistics of total number of found cuts.
    if !ss!(scip, s).initiallp {
        let fromcutpool = scip.rows[cut].fromcutpool;
        let st = &mut ss!(scip, s);
        st.ncutsadded += 1;
        st.ncutsfoundround += 1;
        if fromcutpool {
            st.ncutsaddedviapool += 1;
        } else {
            st.ncutsaddeddirect += 1;
        }

        if scip.rows[cut].origintype == RowOriginType::Sepa {
            let sepa = scip.rows[cut].origin;
            crate::sepa::sepa_inc_ncuts_added(scip, sepa, fromcutpool);
        }
    }
    ss!(scip, s).ncuts += 1;

    // Check, if the row addition to separation storage events are tracked if
    // so, issue ROWADDEDSEPA event.
    let f = &scip.eventfilters[eventfilter];
    if f.len > 0 && (f.eventmask & event::EVENTTYPE_ROWADDEDSEPA) != 0 {
        let ev = event::event_create_row_added_sepa(cut);
        event::eventqueue_add_filter(scip, eventfilter, ev)?;
    }

    // If the duals need to be collected, then the infeasible flag is set to
    // FALSE. This ensures that the LP is solved.
    if scip.set.lp_alwaysgetduals && ss!(scip, s).initiallp {
        infeasible = false;
    }
    Ok(infeasible)
}

/// `sepastoreApplyLb` (`boundtype` lower) and `sepastoreApplyUb`
/// (`boundtype` upper): applies a bound change; returns `(applied,
/// cutoff)`.
#[allow(clippy::too_many_arguments)]
fn sepastore_apply_bound(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    eventfilter: usize,
    var: VarId,
    mut bound: f64,
    boundtype: BoundType,
    local: bool,
    applied: &mut bool,
    cutoff: &mut bool,
) -> ScipResult {
    let lower = boundtype == BoundType::Lower;

    // Adjust bound to the one that would be applied, so the SCIPsetIsGT
    // check below is more reliable.
    if lower {
        crate::var::var_adjust_lb(scip, var, &mut bound);
    } else {
        crate::var::var_adjust_ub(scip, var, &mut bound);
    }

    let (domlb, domub) = {
        let v = &scip.vars[var];
        if local { (v.locdom.lb, v.locdom.ub) } else { (v.glbdom.lb, v.glbdom.ub) }
    };
    let improves = if lower { scip.set.is_gt(bound, domlb) } else { scip.set.is_lt(bound, domub) };

    // Apply the bound change or detect a cutoff; changing a lower bound to a
    // value >= SCIPinfinity (an upper bound to <= -SCIPinfinity) should
    // result in a cutoff, since "infinite" values in solutions are reserved
    // for another meaning.
    if improves {
        let feasible = if lower {
            !scip.set.is_infinity(bound) && scip.set.is_feas_le(bound, domub)
        } else {
            !scip.set.is_infinity(-bound) && scip.set.is_feas_ge(bound, domlb)
        };
        if local {
            if feasible {
                let node = tree::tree_get_current_node(scip);
                tree::node_add_boundchg(scip, node, transprob, origprob, eventfilter, var, bound, boundtype, false)?;
            } else {
                *cutoff = true;
            }
        } else {
            // Apply the global bound change or detect a global cutoff which
            // means we can cutoff the root node.
            let root = tree::tree_get_root_node(scip);
            if feasible {
                tree::node_add_boundchg(scip, root, transprob, origprob, eventfilter, var, bound, boundtype, false)?;
            } else {
                // We are done with solving since a global bound change is
                // infeasible.
                tree::node_cutoff(scip, root, eventfilter, transprob, origprob)?;
                *cutoff = true;
            }
        }
        *applied = true;
    }
    Ok(())
}

/// `sepastoreApplyBdchg`: applies a cut that is a bound change directly as
/// bound change instead of adding it as row to the LP; returns `(applied,
/// cutoff)`.
fn sepastore_apply_bdchg(scip: &mut Scip, s: usize, transprob: usize, origprob: usize, eventfilter: usize, cut: RowId) -> ScipResult<(bool, bool)> {
    let mut applied = false;
    let mut cutoff = false;

    // Get the single variable and its coefficient of the cut.
    let (var, val, local, lhs, rhs, constant) = {
        let row = &scip.rows[cut];
        (scip.cols[row.cols[0_usize]].var, row.vals[0_usize], row.local, row.lhs, row.rhs, row.constant)
    };

    // If the coefficient is nearly zero, we better ignore this cut for
    // numerical reasons.
    if scip.set.is_feas_zero(val) {
        return Ok((applied, cutoff));
    }

    // Get the left hand side of the cut and convert it to a bound.
    if !scip.set.is_infinity(-lhs) {
        let lhs = lhs - constant;
        // Coefficient is positive -> lhs corresponds to lower bound;
        // negative -> upper bound.
        let bt = if val > 0.0 { BoundType::Lower } else { BoundType::Upper };
        sepastore_apply_bound(scip, transprob, origprob, eventfilter, var, lhs / val, bt, local, &mut applied, &mut cutoff)?;
    }

    // Get the right hand side of the cut and convert it to a bound.
    if !scip.set.is_infinity(rhs) {
        let rhs = rhs - constant;
        // Coefficient is positive -> rhs corresponds to upper bound;
        // negative -> lower bound.
        let bt = if val > 0.0 { BoundType::Upper } else { BoundType::Lower };
        sepastore_apply_bound(scip, transprob, origprob, eventfilter, var, rhs / val, bt, local, &mut applied, &mut cutoff)?;
    }

    // Count the bound change as applied cut.
    if applied && !ss!(scip, s).initiallp {
        ss!(scip, s).ncutsapplied += 1;
    }
    Ok((applied, cutoff))
}

/// `sepastoreApplyCut`: applies the given cut to the LP and updates the
/// orthogonalities and scores of remaining cuts.
fn sepastore_apply_cut(scip: &mut Scip, s: usize, cut: RowId, depth: i32, ncutsapplied: &mut i32) -> ScipResult {
    // A row could have been added twice to the separation store; add it only
    // once!
    if scip.rows[cut].lppos < 0 {
        // Add cut to the LP and capture it.
        lp::lp_add_row(scip, cut, depth)?;

        // Update statistics -> only if we are not in the initial lp (cuts are
        // only counted if added during run).
        if !ss!(scip, s).initiallp {
            ss!(scip, s).ncutsapplied += 1;

            // Increase count of applied cuts for origins of row.
            let origin = scip.rows[cut].origin;
            match scip.rows[cut].origintype {
                RowOriginType::Conshdlr => scip.conshdlrs[origin].ncutsapplied += 1,
                RowOriginType::Cons => {
                    let h = scip.conss[origin].conshdlr;
                    scip.conshdlrs[h].ncutsapplied += 1;
                }
                RowOriginType::Sepa => {
                    let fromcutpool = scip.rows[cut].fromcutpool;
                    crate::sepa::sepa_inc_ncuts_applied(scip, origin, fromcutpool);
                }
                RowOriginType::Unspec | RowOriginType::Reopt => {
                    // Do nothing - cannot update statistics.
                }
            }
        }

        *ncutsapplied += 1;
    }
    Ok(())
}

/// `SCIPsepastoreApplyCuts`: adds cuts to the LP and clears separation
/// storage; returns whether a cutoff was detected.
pub fn sepastore_apply_cuts(scip: &mut Scip, s: usize, transprob: usize, origprob: usize, eventfilter: usize, root: bool) -> ScipResult<bool> {
    // Get maximal number of cuts to add to the LP.
    let maxsepacuts = scip.set.get_sepa_maxcuts(root);

    // If all cuts are forced cuts, no selection is required.

    let (ncuts, nforcedcuts, initiallp) = (ss!(scip, s).ncuts, ss!(scip, s).nforcedcuts, ss!(scip, s).initiallp);
    let nselectedcuts = if nforcedcuts >= ncuts.min(maxsepacuts) {
        nforcedcuts
    } else {
        // Call cut selection algorithms.
        let mut cuts = std::mem::take(&mut ss!(scip, s).cuts);
        let r = crate::cutsel::cutsels_select(scip, &mut cuts.0[..ncuts as usize], nforcedcuts, root, initiallp, maxsepacuts);
        ss!(scip, s).cuts = cuts;

        // Note that cut selector statistics are updated here also when in
        // probing mode; this may lead to an offset with
        // separator/constraint handler statistics.
        r? + nforcedcuts
    };

    // Apply all selected cuts.
    let mut ncutsapplied = 0;
    let mut cutoff = false;
    let node = tree::tree_get_current_node(scip);

    // Get depth of current node.
    let depth = scip.nodes[node].depth as i32;

    let mut i = 0;
    while i < nselectedcuts && !cutoff {
        let cut = ss!(scip, s).cuts[i];

        if i < ss!(scip, s).nforcedcuts || {
            let eff = lp::row_get_lp_efficacy(scip, cut);
            scip.set.is_feas_positive(eff)
        } {
            let mut applied = false;

            // If the cut is a bound change (i.e. a row with only one
            // variable), add it as bound change instead of LP row.
            if !scip.rows[cut].modifiable && scip.rows[cut].len == 1 {
                (applied, cutoff) = sepastore_apply_bdchg(scip, s, transprob, origprob, eventfilter, cut)?;
            }

            if !applied {
                // Add cut to the LP and update orthogonalities.
                sepastore_apply_cut(scip, s, cut, depth, &mut ncutsapplied)?;
            }
        }
        i += 1;
    }

    // Clear the separation storage and reset statistics for separation round.
    sepastore_clear_cuts(scip, s, eventfilter)?;
    Ok(cutoff)
}

/// `SCIPsepastoreClearCuts`: clears the separation storage without adding
/// the cuts to the LP.
pub fn sepastore_clear_cuts(scip: &mut Scip, s: usize, eventfilter: usize) -> ScipResult {
    // Release cuts.
    for c in 0..ss!(scip, s).ncuts {
        let cut = ss!(scip, s).cuts[c];

        // Check, if the row deletions from separation storage events are
        // tracked if so, issue ROWDELETEDSEPA event.
        issue_row_deleted_sepa(scip, eventfilter, cut)?;
        lp::row_release(scip, cut)?;
    }

    // Reset counters.
    let st = &mut ss!(scip, s);
    st.ncuts = 0;
    st.nforcedcuts = 0;
    st.ncutsfoundround = 0;

    // If we have just finished the initial LP construction, free the
    // (potentially large) cuts array.
    if st.initiallp {
        st.cuts = CVec::new();
        st.cutssize = 0;
    }
    Ok(())
}

/// `SCIPsepastoreRemoveInefficaciousCuts`: removes cuts that are
/// inefficacious w.r.t. the current LP solution from separation storage
/// without adding the cuts to the LP.
pub fn sepastore_remove_inefficacious_cuts(scip: &mut Scip, s: usize, eventfilter: usize, root: bool, efficiacychoice: EfficiacyChoice) -> ScipResult {
    // Check non-forced cuts only.
    let mut c = ss!(scip, s).nforcedcuts;
    while c < ss!(scip, s).ncuts {
        let cut = ss!(scip, s).cuts[c];

        // Calculate cut's efficacy.
        let cutefficacy = match efficiacychoice {
            EfficiacyChoice::Lp => lp::row_get_lp_efficacy(scip, cut),
            EfficiacyChoice::Relax => lp::row_get_relax_efficacy(scip, cut),
            EfficiacyChoice::Nlp => lp::row_get_nlp_efficacy(scip, cut),
        };

        if !scip.set.is_efficacious(root, cutefficacy) {
            sepastore_del_cut(scip, s, eventfilter, c)?;
        } else {
            c += 1;
        }
    }
    Ok(())
}

/// `SCIPsepastoreIsCutApplicable`: indicates whether a cut is applicable.
/// A cut is applicable if it is modifiable, not a bound change, or a bound
/// change that changes bounds by at least epsilon.
pub fn sepastore_is_cut_applicable(scip: &Scip, cut: RowId) -> bool {
    scip.rows[cut].modifiable || scip.rows[cut].len != 1 || sepastore_is_bdchg_applicable(scip, cut)
}

/// `SCIPsepastoreGetCuts`: get cuts in the separation storage.
pub fn sepastore_get_cuts(scip: &Scip, s: usize) -> &[RowId] {
    let st = &scip.sepastores[s];
    st.cuts.head(st.ncuts)
}

/// `SCIPsepastoreGetNCuts` of the main storage (`SCIPgetNCuts`).
pub fn sepastore_get_ncuts(scip: &Scip) -> i32 {
    scip.sepastores[SEPASTORE].ncuts
}

/// `SCIPsepastoreGetNCutsAdded`: gets the total number of cutting planes
/// added to the separation storage; this is equal to the sum of added cuts
/// directly and via the pool.
pub fn sepastore_get_ncuts_added(scip: &Scip, s: usize) -> i32 {
    scip.sepastores[s].ncutsadded
}

/// `SCIPsepastoreGetNCutsFoundRound`: get number of cuts found so far in
/// current separation round.
pub fn sepastore_get_ncuts_found_round(scip: &Scip, s: usize) -> i32 {
    scip.sepastores[s].ncutsfoundround
}

/// `SCIPsepastoreGetNCutsApplied`: gets the total number of cutting planes
/// applied to the LP.
pub fn sepastore_get_ncuts_applied(scip: &Scip, s: usize) -> i32 {
    scip.sepastores[s].ncutsapplied
}
