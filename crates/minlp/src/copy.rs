//! Copying a SCIP into a sub-SCIP (`scip_copy.c`), as LNS heuristics do,
//! and `SCIPcopyLargeNeighborhoodSearch` (`heuristics.c`).
//!
//! A sub-SCIP is another [`Scip`]: the source and the target are two
//! separate values, so a copy function takes both mutably. C reaches the
//! source's plugins' copy callbacks with the target SCIP; here the source
//! plugin's function pointer is called on the target.
//!
//! Not ported: original-problem copies other than of a SCIP in
//! `SCIP_STAGE_PROBLEM`, problem and variable data copies (no plugin has
//! any), Benders, pricers, readers, dialogs, displays, tables, exact
//! solving, the message handler and the concurrent sync store.

use crate::{
    api,
    cons::ConsFlags,
    hash::HashMap,
    prob::TRANSPROB,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    types::{ColId, ConsId, ConshdlrId, HeurId, SolId, VarId, VarStatus},
    var::NONE,
};

/// The variable and constraint maps of a copy: C's `varmap` and `consmap`
/// from source pointers to target pointers, here from source ids.
#[derive(Clone, Debug)]
pub struct CopyMaps {
    pub varmap: HashMap<usize>,
    pub consmap: HashMap<usize>,
}

impl CopyMaps {
    /// Two empty maps, sized as `SCIPhashmapCreate` would size them.
    pub fn new(nvars: i32, nconss: i32) -> Self {
        Self {
            varmap: HashMap::new(nvars),
            consmap: HashMap::new(nconss),
        }
    }

    /// `SCIPhashmapGetImage(varmap, sourcevar)`, `NONE` if not copied.
    pub fn var_image(&self, sourcevar: VarId) -> VarId {
        self.varmap.get_image(sourcevar as u64).unwrap_or(NONE)
    }
}

/// `takeCut`: whether a cut of the pool is copied.
fn take_cut(scip: &Scip, cut: usize, cutsel: u8) -> bool {
    if scip.rows[scip.cuts[cut].row].lppos < 0 {
        return false;
    }
    match cutsel {
        b'a' => scip.cuts[cut].age == 0,
        b'q' => crate::cutpool::cut_get_lp_activity_quot(scip, cut) >= scip.set.sepa_minactivityquot,
        _ => panic!("unknown cut selection strategy {}, must be either 'a' or 'q'", cutsel as char),
    }
}

/// `copyCuts` for a target other than the source: linear constraints in
/// `target` from the selected cuts of the source's pool `pool`.
fn copy_cuts(source: &mut Scip, target: &mut Scip, pool: usize, maps: &mut CopyMaps, global: bool, ncutsadded: &mut i32) -> ScipResult {
    let ncuts = source.cutpools[pool].ncuts as usize;
    let cuts = source.cutpools[pool].cuts.0[..ncuts].to_vec();
    for cut in cuts {
        let row = source.cuts[cut].row;
        let mut takecut = take_cut(source, cut, source.set.sepa_cutselsubscip);

        // Create a linear constraint out of the cut.
        if takecut {
            let ncols = source.rows[row].len as usize;
            let mut vars = Vec::with_capacity(ncols);
            let mut i = 0;
            while i < ncols && takecut {
                let var = source.cols[source.rows[row].cols[i]].var;
                vars.push(var);
                takecut = !source.vars[var].relaxationonly;
                i += 1;
            }

            // Discard cut if it contains a variable which is invalid after
            // a restart.
            if !takecut {
                continue;
            }

            // Get corresponding variables in the target.
            for v in vars.iter_mut() {
                let (targetvar, success) = get_var_copy(source, target, *v, Some(maps), global)?;
                if !success {
                    return Ok(());
                }
                *v = targetvar;
            }

            let r = &source.rows[row];
            let name = format!("{}_{}", r.name, source.stat.nruns);
            let vals = r.vals.0[..ncols].to_vec();
            let (lhs, rhs) = (r.lhs - r.constant, r.rhs - r.constant);
            let mut cons = crate::cons_linear::create_cons_linear(target, &name, &vars, &vals, lhs, rhs, true, true, false, false, true, false, false, false, true, false)?;
            crate::scip_cons::add_cons(target, cons)?;
            crate::scip_cons::release_cons(target, &mut cons)?;

            *ncutsadded += 1;
        }
    }
    Ok(())
}

/// `SCIPcopyPlugins` (`SCIPsetCopyPlugins`): includes in `target` the
/// plugins of `source` that have a copy method, in the source's array
/// order; returns whether every constraint handler was validly copied.
#[allow(clippy::too_many_arguments)]
pub fn copy_plugins(
    source: &Scip,
    target: &mut Scip,
    copyconshdlrs: bool,
    copyconflicthdlrs: bool,
    copypresolvers: bool,
    copyrelaxators: bool,
    copyseparators: bool,
    copycutselectors: bool,
    copypropagators: bool,
    copyheuristics: bool,
    copyeventhdlrs: bool,
    copynodeselectors: bool,
    copybranchrules: bool,
) -> ScipResult<bool> {
    let set = &source.set;
    let mut allvalid = true;

    // Copy all constraint handler plugins, in the order they were added.
    if copyconshdlrs {
        for p in 0..set.nconshdlrs {
            let h = set.conshdlrs_include[p];
            if source.conshdlrs[h].conshdlrcopy.is_some() {
                let mut valid = false;
                crate::cons::conshdlr_copy_include(target, source, h, &mut valid)?;
                allvalid = allvalid && valid;
            } else if !source.conshdlrs[h].needscons {
                allvalid = false;
            }
        }
    }

    if copyconflicthdlrs {
        for p in 0..set.nconflicthdlrs {
            let h = set.conflicthdlrs[p];
            crate::conflict::conflicthdlr_copy_include(target, source, h)?;
        }
    }

    if copypresolvers {
        for p in 0..set.npresols {
            let h = set.presols[p];
            crate::presol::presol_copy_include(target, source, h)?;
        }
    }

    if copynodeselectors {
        for p in 0..set.nnodesels {
            let h = set.nodesels[p];
            crate::nodesel::nodesel_copy_include(target, source, h)?;
        }
    }

    if copybranchrules {
        for p in 0..set.nbranchrules {
            let h = set.branchrules[p];
            crate::branch::branchrule_copy_include(target, source, h)?;
        }
    }

    if copyeventhdlrs {
        for p in 0..set.neventhdlrs {
            let h = set.eventhdlrs[p];
            crate::event::eventhdlr_copy_include(target, source, h)?;
        }
    }

    if copyrelaxators {
        for p in 0..set.nrelaxs {
            let h = set.relaxs[p];
            crate::relax::relax_copy_include(target, source, h)?;
        }
    }

    if copyheuristics {
        for p in 0..set.nheurs {
            let h = set.heurs[p];
            crate::heur::heur_copy_include(target, source, h)?;
        }
    }

    if copypropagators {
        for p in 0..set.nprops {
            let h = set.props[p];
            crate::prop::prop_copy_include(target, source, h)?;
        }
    }

    if copyseparators {
        for p in 0..set.nsepas {
            let h = set.sepas[p];
            crate::sepa::sepa_copy_include(target, source, h)?;
        }
    }

    if copycutselectors {
        for p in 0..set.ncutsels {
            let h = set.cutsels[p];
            crate::cutsel::cutsel_copy_include(target, source, h)?;
        }
    }

    // Copy all expression handlers (every caller copies them).
    for exprhdlr in &set.exprhdlrs {
        let h = *exprhdlr;
        crate::expr::exprhdlr_copy_include(target, source, h)?;
    }

    Ok(allvalid)
}

/// `copyProb` of the transformed problem: a fresh problem in `target`.
fn copy_prob(source: &Scip, target: &mut Scip, name: &str) -> ScipResult {
    // Free old problem.
    crate::scip_solve::free_prob(target)?;

    // Switch stage to PROBLEM.
    target.set.stage = Stage::Problem;

    // Create the statistics data structure.
    crate::stat::stat_create(target);
    target.stat.subscipdepth = source.stat.subscipdepth + 1;

    // Create the problem by copying the source problem (no problem data).
    crate::prob::prob_create(target, crate::prob::ORIGPROB, name, false);

    // Creating the solution candidates storage.
    target.primals.insert_at(crate::scip::ORIGPRIMAL, crate::primal::primal_create());

    // Create conflict store to store conflict constraints.
    crate::conflictstore::conflictstore_create(target)?;
    Ok(())
}

/// `SCIPcopyProb`: creates the target problem from the source's
/// transformed problem.
pub fn copy_prob_transformed(source: &Scip, target: &mut Scip, name: &str) -> ScipResult {
    copy_prob(source, target, name)
}

/// `SCIPenableConsCompression`.
pub fn enable_cons_compression(scip: &mut Scip) {
    scip.probs[crate::prob::ORIGPROB].conscompression = true;
}

/// `SCIPisConsCompressionEnabled`.
pub fn is_cons_compression_enabled(scip: &Scip) -> bool {
    scip.probs[crate::prob::ORIGPROB].conscompression && scip.set.stage == Stage::Problem
}

/// `SCIPgetVarCopy`: the copy of `sourcevar` in `target`, created and added
/// to the target problem if it does not exist yet; returns `(targetvar,
/// success)`. `maps` `None` is C's `varmap == NULL` (fresh local maps).
pub fn get_var_copy(source: &mut Scip, target: &mut Scip, sourcevar: VarId, maps: Option<&mut CopyMaps>, global: bool) -> ScipResult<(VarId, bool)> {
    let mut success = true;

    // Try to retrieve copied variable from hashmap.
    if let Some(m) = &maps {
        let targetvar = m.var_image(sourcevar);
        if targetvar != NONE {
            return Ok((targetvar, true));
        }
    }

    // Reject copying of relaxation-only variables.
    if source.vars[sourcevar].relaxationonly {
        success = false;
    }

    // If the target SCIP is already in solving stage we currently are not
    // copying the variable.
    if target.set.stage > Stage::Problem {
        return Ok((NONE, false));
    }

    let mut localmaps;
    let maps = match maps {
        Some(m) => m,
        None => {
            localmaps = CopyMaps::new(api::get_n_vars(source), api::get_n_conss(source));
            &mut localmaps
        }
    };

    // If variable does not exist yet in target SCIP, create it.
    let mut var = match source.vars[sourcevar].varstatus {
        VarStatus::Original | VarStatus::Column | VarStatus::Loose | VarStatus::Fixed => crate::var::var_copy(target, source, sourcevar, &mut maps.varmap, global)?,
        VarStatus::Aggregated => {
            // Get aggregation data.
            let a = &source.vars[sourcevar].data.aggregate;
            let (sourceaggrvar, aggrcoef, constant) = (a.var, a.scalar, a.constant);

            // Get copy of the aggregation variable.
            let (targetaggrvar, s) = get_var_copy(source, target, sourceaggrvar, Some(maps), global)?;
            success = s;

            // Create copy of the aggregated variable.
            let var = crate::var::var_copy(target, source, sourcevar, &mut maps.varmap, global)?;

            let name = format!("{}_aggr", source.vars[sourcevar].name);

            // Add aggregation x = a*y + c as linear constraint x - a*y = c.
            let mut cons = crate::cons_linear::create_cons_linear(target, &name, &[], &[], constant, constant, true, true, true, true, true, false, false, false, false, false)?;
            crate::cons_linear::add_coef_linear(target, cons, var, 1.0)?;
            crate::cons_linear::add_coef_linear(target, cons, targetaggrvar, -aggrcoef)?;
            crate::scip_cons::add_cons(target, cons)?;
            crate::scip_cons::release_cons(target, &mut cons)?;
            var
        }
        VarStatus::MultAggr => {
            // Get the active representation.
            crate::var::var_flatten_aggregation_graph(source, sourcevar)?;

            // Get multi-aggregation data.
            let m = &source.vars[sourcevar].data.multaggr;
            let naggrvars = m.nvars as usize;
            let sourceaggrvars = m.vars.0[..naggrvars].to_vec();
            let aggrcoefs = m.scalars.0[..naggrvars].to_vec();
            let constant = m.constant;

            // Get copies of the active variables of the multi-aggregation.
            let mut targetaggrvars = vec![NONE; naggrvars];
            for i in 0..naggrvars {
                let (t, s) = get_var_copy(source, target, sourceaggrvars[i], Some(maps), global)?;
                targetaggrvars[i] = t;
                success = s;
            }

            // Create copy of the multi-aggregated variable.
            let var = crate::var::var_copy(target, source, sourcevar, &mut maps.varmap, global)?;

            let name = format!("{}_multaggr", source.vars[sourcevar].name);

            // Add multi-aggregation x = a^T y + c as linear constraint
            // a^T y - x = -c.
            let mut cons = crate::cons_linear::create_cons_linear(
                target,
                &name,
                &targetaggrvars,
                &aggrcoefs,
                -constant,
                -constant,
                true,
                true,
                true,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
            )?;
            crate::cons_linear::add_coef_linear(target, cons, var, -1.0)?;
            crate::scip_cons::add_cons(target, cons)?;
            crate::scip_cons::release_cons(target, &mut cons)?;
            var
        }
        VarStatus::Negated => {
            // Get negated source variable.
            let sourcenegatedvar = source.vars[sourcevar].negatedvar;

            // Get copy of negated source variable.
            let (targetnegatedvar, s) = get_var_copy(source, target, sourcenegatedvar, Some(maps), global)?;

            // Get negation of copied negated source variable, this is the
            // target variable; it is not added to the problem.
            let targetvar = crate::scip_var::get_negated_var(target, targetnegatedvar)?;
            return Ok((targetvar, s));
        }
    };

    // Add the (new) target variable to the target problem.
    crate::scip_var::add_var(target, var)?;
    let targetvar = var;

    // Remove the variable capture which was done due to the creation of the
    // variable.
    api::release_var(target, &mut var)?;

    Ok((targetvar, success))
}

/// `copyVars` of the transformed problem: copies the active variables of
/// `source` (but relaxation-only ones) and fixes the copies of
/// `fixedvars` to `fixedvals`.
fn copy_vars(source: &mut Scip, target: &mut Scip, maps: &mut CopyMaps, fixedvars: &[VarId], fixedvals: &[f64], global: bool) -> ScipResult {
    let sourcevars = api::get_vars(source).to_vec();

    // Create the variables of the target SCIP.
    for &sourcevar in &sourcevars {
        if source.vars[sourcevar].relaxationonly {
            continue;
        }

        // Copy variable and add this copy to the target SCIP if the copying
        // was valid.
        get_var_copy(source, target, sourcevar, Some(maps), global)?;
    }

    // Fix the variables that should be fixed right away.
    for (i, &fixedvar) in fixedvars.iter().enumerate() {
        if source.vars[fixedvar].relaxationonly {
            continue;
        }

        // Retrieve target variable as image of the source variable.
        let targetvar = maps.var_image(fixedvar);

        // Fix the variable to the specified value.
        crate::scip_var::fix_var(target, targetvar, fixedvals[i])?;
    }
    Ok(())
}

/// `SCIPcopyVars`: copies the active variables of `source` into `target`.
pub fn copy_vars_public(source: &mut Scip, target: &mut Scip, maps: &mut CopyMaps, fixedvars: &[VarId], fixedvals: &[f64], global: bool) -> ScipResult {
    copy_vars(source, target, maps, fixedvars, fixedvals, global)
}

/// `SCIPmergeVariableStatistics`: merges the histories of the `source`
/// variables into the `target` ones (`NONE` entries are skipped).
pub fn merge_variable_statistics(source: &Scip, target: &mut Scip, _sourcevars: &[VarId], _targetvars: &[VarId]) -> ScipResult {
    // Check if target scip has been set to allow merging variable
    // statistics.
    if !target.set.history_allowmerge {
        return Ok(());
    }

    // We do not want to copy statistics from a scip that has not really
    // started solving.
    if source.set.stage < Stage::Solving {
        return Ok(());
    }

    // If the transformation of the source was subject to scaling, the
    // history information cannot be just copied.
    let origprob = &source.probs[crate::prob::ORIGPROB];
    let objscale = origprob.objscale;
    let objoffset = origprob.objoffset;
    if !target.set.is_eq(1.0, objscale) || !target.set.is_eq(0.0, objoffset) {
        return Ok(());
    }

    unimplemented!("SCIPmergeVariableStatistics with history/allowmerge (off by default)");
}

/// `translateSubSol`: the values of `subsol` for the variables of the main
/// SCIP (`subvars` in the order of its variables, `NONE` for those
/// without a copy).
fn translate_sub_sol_vals(scip: &Scip, subscip: &Scip, subsol: SolId, subvars: &[VarId]) -> Vec<f64> {
    let vars = api::get_vars(scip);
    vars.iter()
        .enumerate()
        .map(|(i, &var)| {
            if subvars[i] != NONE {
                api::get_sol_val(subscip, subsol, subvars[i])
            } else {
                let v = &scip.vars[var];
                crate::def::min(crate::def::max(0.0, v.locdom.lb), v.locdom.ub)
            }
        })
        .collect()
}

/// `SCIPtranslateSubSol`: a new solution of `scip`, found by `heur`, with
/// the values of `subsol`.
pub fn translate_sub_sol(scip: &mut Scip, subscip: &Scip, subsol: SolId, heur: HeurId, subvars: &[VarId]) -> ScipResult<SolId> {
    let vars = api::get_vars(scip).to_vec();

    // Get the solution values.
    let subsolvals = translate_sub_sol_vals(scip, subscip, subsol, subvars);

    // Create new solution for the original problem.
    let newsol = api::create_sol(scip, heur)?;
    api::set_sol_vals(scip, newsol, &vars, &subsolvals)?;
    Ok(newsol)
}

/// `SCIPtranslateSubSols`: adds the first feasible solution of `subscip`
/// to `scip`; returns `(success, solindex)`.
pub fn translate_sub_sols(scip: &mut Scip, subscip: &Scip, heur: HeurId, subvars: &[VarId]) -> ScipResult<(bool, i32)> {
    let mut success = false;
    let mut solindex = -1;

    // Check, whether a solution was found.
    if api::get_n_sols(subscip) == 0 {
        return Ok((false, solindex));
    }

    let vars = api::get_vars(scip).to_vec();
    let subsols = api::get_sols(subscip).to_vec();
    let mut newsol = NONE;
    for &subsol in &subsols {
        // Create or clear main solution.
        if newsol == NONE {
            newsol = api::create_sol(scip, heur)?;
            solindex = scip.sols[newsol].index;
        } else {
            api::clear_sol(scip, newsol)?;
        }

        // Get values from subsol and put them into newsol.
        let solvals = translate_sub_sol_vals(scip, subscip, subsol, subvars);
        api::set_sol_vals(scip, newsol, &vars, &solvals)?;

        // Reject solution with invalid objective value.
        if api::get_sol_trans_obj(scip, newsol) == crate::def::INVALID {
            continue;
        }

        // Check whether feasible.
        success = api::check_sol(scip, newsol, false, false, true, true, true)?;
        if success {
            // If feasible, then there is a good chance that we can add it;
            // we use SCIPaddSolFree to make sure that newsol is reset to
            // NULL.
            success = api::add_sol_free(scip, newsol)?;
            newsol = NONE;
            if success {
                break;
            }
        }
    }

    if newsol != NONE {
        api::free_sol(scip, newsol)?;
    }
    Ok((success, solindex))
}

/// `SCIPgetConsCopy`: the copy of `sourcecons` in `target` (captured), or
/// `NONE`; returns `(targetcons, valid)`.
#[allow(clippy::too_many_arguments)]
pub fn get_cons_copy(
    source: &mut Scip,
    target: &mut Scip,
    sourcecons: ConsId,
    sourceconshdlr: ConshdlrId,
    maps: &mut CopyMaps,
    name: Option<&str>,
    flags: &ConsFlags,
    global: bool,
) -> ScipResult<(ConsId, bool)> {
    // Use global map and try to retrieve copied constraint.
    let mut targetcons = maps.consmap.get_image(sourcecons as u64).unwrap_or(NONE);

    if targetcons != NONE {
        // If found capture existing copy of the constraint.
        crate::scip_cons::capture_cons(target, targetcons);
        return Ok((targetcons, true));
    }

    // Otherwise create a copy of the constraint (SCIPconsCopy): if the
    // handler does not support copying, valid stays false.
    let mut valid = false;
    if let Some(conscopy) = source.conshdlrs[sourceconshdlr].conscopy {
        conscopy(target, &mut targetcons, name, source, sourceconshdlr, sourcecons, maps, flags, global, &mut valid)?;
    }

    // If a target constraint was created, insert it into the mapping.
    if targetcons != NONE {
        let _ = maps.consmap.insert(sourcecons as u64, targetcons);
    }
    Ok((targetcons, valid))
}

/// `SCIPcopyConss`: copies the active constraints of every handler of
/// `source` into `target`; returns whether all checked or enforced ones
/// were validly copied.
pub fn copy_conss(source: &mut Scip, target: &mut Scip, maps: &mut CopyMaps, global: bool, enablepricing: bool) -> ScipResult<bool> {
    let mut valid = true;

    // Copy constraints: loop through all (source) constraint handlers.
    for i in 0..source.set.nconshdlrs {
        let h = source.set.conshdlrs[i];

        // Get all active constraints for copying.
        let nsourceconss = source.conshdlrs[h].nactiveconss as usize;
        let sourceconss = source.conshdlrs[h].conss.0[..nsourceconss].to_vec();

        // Copy all constraints of one constraint handler.
        for &sc in &sourceconss {
            // In case of copying the global problem we have to ignore the
            // local constraints which are active.
            if global && source.conss[sc].local {
                continue;
            }

            let c = &source.conss[sc];
            let flags = ConsFlags {
                initial: c.initial,
                separate: c.separate,
                enforce: c.enforce,
                check: c.check,
                propagate: c.propagate,
                local: false,
                modifiable: c.modifiable,
                dynamic: c.dynamic,
                removable: c.removable,
                stickingatnode: false,
            };

            // Use the copy constructor of the constraint handler and
            // creates and captures the constraint if possible.
            let (mut targetcons, _singlevalid) = get_cons_copy(source, target, sc, h, maps, None, &flags, global)?;

            // Add the copied constraint to target SCIP if the copying
            // process created a constraint.
            if targetcons != NONE {
                if !enablepricing {
                    target.conss[targetcons].modifiable = false;
                }

                // Add constraint to target SCIP.
                crate::scip_cons::add_cons(target, targetcons)?;

                // Add the conflict constraint to the store of targetscip.
                if source.conss[sc].conflict {
                    let inf = target.set.infinity();
                    crate::conflictstore::conflictstore_add_conflict(target, TRANSPROB, targetcons, crate::types::ConfType::Unknown, false, -inf)?;
                }

                // Release constraint once for the creation capture.
                crate::scip_cons::release_cons(target, &mut targetcons)?;
            } else {
                // If an enforced or checked constraint could not be copied,
                // then the copy is not valid.
                if source.conss[sc].enforce || source.conss[sc].check {
                    valid = false;
                }
            }
        }
    }
    Ok(valid)
}

/// `SCIPcopyCuts`: copies the active cuts of the source's cut pools into
/// `target` as linear constraints; returns how many.
pub fn copy_cuts_public(source: &mut Scip, target: &mut Scip, maps: &mut CopyMaps, global: bool) -> ScipResult<i32> {
    let mut nlocalcutsadded = 0;

    // If we do not have any cuts, nothing can be converted.
    if source.set.stage < Stage::Solving {
        return Ok(0);
    }

    if crate::set::set_find_conshdlr(target, crate::cons_linear::CONSHDLR_NAME) == NONE {
        return Ok(0);
    }

    // Convert cut from global cut pool.
    copy_cuts(source, target, crate::cutpool::GLOBALCUTPOOL, maps, global, &mut nlocalcutsadded)?;

    // Convert delayed cuts from global delayed cut pool.
    copy_cuts(source, target, crate::cutpool::DELAYEDCUTPOOL, maps, global, &mut nlocalcutsadded)?;

    Ok(nlocalcutsadded)
}

/// `SCIPcopyImplicationsCliques`: copies the cliques and binary
/// implications of `source` into `target`; returns `(infeasible, nbdchgs,
/// ncopied)`.
pub fn copy_implications_cliques(source: &mut Scip, target: &mut Scip, maps: &mut CopyMaps, global: bool) -> ScipResult<(bool, i32, i32)> {
    let mut ncopied = 0;
    let mut nbdchgs = 0;

    // Get all active variables.
    let sourcevars = api::get_vars(source).to_vec();
    let nbinvars = api::get_n_bin_vars(source) as usize;

    // Stop if no possible variables for cliques exist.
    if nbinvars == 0 {
        return Ok((false, 0, 0));
    }

    // Get cliques.
    let ncliques = crate::scip_var::get_n_cliques(source) as usize;
    if ncliques > 0 {
        let cliques = crate::scip_var::get_cliques(source).to_vec();

        // Loop through all cliques.
        for &clique in &cliques {
            let cl = &source.cliques[clique];
            let cliquesize = cl.nvars as usize;
            let cliquevars = cl.vars.0[..cliquesize].to_vec();
            let cliquevals = cl.values.0[..cliquesize].to_vec();
            let isequation = cl.equation;

            // Get target variables of clique.
            let mut targetclique = Vec::with_capacity(cliquesize);
            for &cv in &cliquevars {
                let (t, success) = get_var_copy(source, target, cv, Some(maps), global)?;
                if !success {
                    return Ok((false, nbdchgs, ncopied));
                }
                targetclique.push(t);
            }

            // Create clique.
            let (infeasible, nboundchg) = crate::scip_var::add_clique(target, &targetclique, &cliquevals, isequation)?;
            if infeasible {
                return Ok((true, nbdchgs, ncopied));
            }
            ncopied += 1;
            nbdchgs += nboundchg;
        }
    }

    // Create binary implications.
    for &sourcevar in &sourcevars[..nbinvars] {
        let (targetvar, success) = get_var_copy(source, target, sourcevar, Some(maps), global)?;
        if !success {
            return Ok((false, nbdchgs, ncopied));
        }

        // Consider both possible implications.
        for d in [false, true] {
            let nimpls = crate::var::var_get_n_impls(source, sourcevar, d) as usize;
            if nimpls == 0 {
                continue;
            }
            let im = source.vars[sourcevar].implics.as_ref().unwrap();
            let impltypes = im.types[d as usize].0[..nimpls].to_vec();
            let implvars = im.vars[d as usize].0[..nimpls].to_vec();
            let implbounds = im.bounds[d as usize].0[..nimpls].to_vec();

            // Create implications.
            for l in 0..nimpls {
                let (impltype, implbound) = (impltypes[l], implbounds[l]);
                let (implvar, success) = get_var_copy(source, target, implvars[l], Some(maps), global)?;
                if !success {
                    return Ok((false, nbdchgs, ncopied));
                }
                let (infeasible, nboundchg) = crate::scip_var::add_var_implication(target, targetvar, d, implvar, impltype, implbound)?;
                if infeasible {
                    return Ok((true, nbdchgs, ncopied));
                }
                ncopied += 1;
                nbdchgs += nboundchg;
            }
        }
    }
    Ok((false, nbdchgs, ncopied))
}

/// `SCIPcopyParamSettings` (`SCIPparamsetCopyParams`): sets every
/// parameter of `target` that `source` also has to the source's value and
/// fixing status.
pub fn copy_param_settings(source: &Scip, target: &mut Scip) -> ScipResult {
    use crate::paramset::{ParamData, param_get_bool, param_get_char, param_get_int, param_get_longint, param_get_real, param_get_string};
    for i in 0..source.set.paramset.params.len() {
        let sourceparam = &source.set.paramset.params[i];

        // Find parameter of same name in target scip; if a plugin was not
        // copied, the parameter does not exist in the target SCIP.
        let Some(t) = crate::paramset::paramset_get_param(target, &sourceparam.name) else {
            continue;
        };

        // Set value of target parameter to value of source parameter.
        match &sourceparam.data {
            ParamData::Bool(..) => crate::paramset::param_set_bool(target, t, param_get_bool(source, i), false, true)?,
            ParamData::Int(..) => crate::paramset::param_set_int(target, t, param_get_int(source, i), false, true)?,
            ParamData::Longint(..) => crate::paramset::param_set_longint(target, t, param_get_longint(source, i), false, true)?,
            ParamData::Real(..) => crate::paramset::param_set_real(target, t, param_get_real(source, i), false, true)?,
            ParamData::Char(..) => crate::paramset::param_set_char(target, t, param_get_char(source, i), false, true)?,
            ParamData::String(..) => {
                // The visualization and certificate parameters are
                // explicitly not copied.
                if !sourceparam.name.starts_with("visual/") && !sourceparam.name.starts_with("certificate/") {
                    crate::paramset::param_set_string(target, t, &param_get_string(source, i), false, true)?;
                }
            }
        }

        // Copy fixing status of parameter.
        target.set.paramset.params[t].isfixed = source.set.paramset.params[i].isfixed;
    }

    // Reoptimization is not ported, so there is nothing to disable.
    if target.set.reopt_enable {
        return Err(Retcode::InvalidCall);
    }
    Ok(())
}

/// `SCIPgetSubscipDepth`.
pub fn get_subscip_depth(scip: &Scip) -> i32 {
    scip.stat.subscipdepth
}

/// `doCopy` of the transformed problem (`original` FALSE, which every
/// sub-SCIP heuristic asks for); returns whether the copy is valid.
#[allow(clippy::too_many_arguments)]
fn do_copy(
    source: &mut Scip,
    target: &mut Scip,
    maps: &mut CopyMaps,
    suffix: &str,
    fixedvars: &[VarId],
    fixedvals: &[f64],
    useconscompression: bool,
    global: bool,
    enablepricing: bool,
) -> ScipResult<bool> {
    if source.set.stage == Stage::Problem {
        unimplemented!("copying the original problem (SCIPcopyOrigProb)");
    }

    // Start time measuring.
    let startcopytime = source.stat.copyclock.get_time();
    let enabled = source.set.time_enabled;
    source.stat.copyclock.start(enabled);

    // Copy all plugins.
    let mut localvalid = copy_plugins(source, target, true, true, true, true, true, true, true, true, true, true, true)?;

    // Pricers are not ported, so none is active.

    // Construct name for the target SCIP using the source problem name and
    // the given suffix string.
    let name = format!("{}_{}", source.probs[crate::prob::ORIGPROB].name, suffix);

    // Copy all settings.
    copy_param_settings(source, target)?;

    // Create problem in the target SCIP copying the source transformed
    // problem data.
    copy_prob(source, target, &name)?;

    // Copy transformed variables and perform fixings if needed.
    copy_vars(source, target, maps, fixedvars, fixedvals, global)?;

    // If fixed variables are directly specified or inferred from local
    // bounds, enable constraint compression.
    if useconscompression && (!fixedvars.is_empty() || !global) {
        enable_cons_compression(target);

        // Domain reductions yield a copy that is no longer guaranteed to be
        // valid.
        localvalid = false;
    }

    // Copy all constraints.
    let consscopyvalid = copy_conss(source, target, maps, global, enablepricing)?;
    localvalid = localvalid && consscopyvalid;

    // No Benders' decompositions to copy.

    // Stop time measuring.
    source.stat.copyclock.stop(enabled);

    // Get time after copying procedure.
    let copytime = source.stat.copyclock.get_time() - startcopytime;
    if copytime > source.stat.maxcopytime {
        source.stat.maxcopytime = copytime;
    }
    if copytime < source.stat.mincopytime {
        source.stat.mincopytime = copytime;
    }

    // Increase copy counter.
    source.stat.ncopies += 1;

    Ok(localvalid)
}

/// `SCIPcopy`: copies the transformed problem of `source` into `target`;
/// returns whether the copy is valid.
pub fn copy(source: &mut Scip, target: &mut Scip, maps: &mut CopyMaps, suffix: &str, global: bool, enablepricing: bool) -> ScipResult<bool> {
    do_copy(source, target, maps, suffix, &[], &[], false, global, enablepricing)
}

/// `SCIPcopyConsCompression`: as [`copy`], with constraint compression and
/// the given variables fixed.
#[allow(clippy::too_many_arguments)]
pub fn copy_cons_compression(
    source: &mut Scip,
    target: &mut Scip,
    maps: &mut CopyMaps,
    suffix: &str,
    fixedvars: &[VarId],
    fixedvals: &[f64],
    global: bool,
    enablepricing: bool,
) -> ScipResult<bool> {
    do_copy(source, target, maps, suffix, fixedvars, fixedvals, true, global, enablepricing)
}

/// `getCopyTimelimit`.
fn get_copy_timelimit(source: &Scip) -> ScipResult<f64> {
    let mut timelimit = crate::paramset::paramset_get_real(source, "limits/time")?;
    if !source.set.is_infinity(timelimit) {
        timelimit -= api::get_solving_time(source);
    }
    Ok(timelimit)
}

/// `getCopyMemlimit`. SCIP's block memory accounting is not ported: the
/// memory in use counts as zero, so the limit stays as set.
fn get_copy_memlimit(source: &Scip) -> ScipResult<f64> {
    crate::paramset::paramset_get_real(source, "limits/memory")
}

/// `SCIPcheckCopyLimits`: whether there is time (and memory) left to copy
/// and solve a sub-SCIP.
pub fn check_copy_limits(source: &Scip) -> ScipResult<bool> {
    let timelimit = get_copy_timelimit(source)?;
    if source.set.misc_avoidmemout {
        // Try to avoid running into memory limit; external software uses no
        // memory here.
        let memorylimit = get_copy_memlimit(source)?;
        Ok(timelimit > 0.0 && memorylimit > 0.0)
    } else {
        Ok(timelimit > 0.0)
    }
}

/// `SCIPcopyLimits`: copies the time and memory limits of `source` into
/// `target` and disables all other limits.
pub fn copy_limits(source: &Scip, target: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_set_int, paramset_set_longint, paramset_set_real};
    let mut timelimit = get_copy_timelimit(source)?;
    let mut memorylimit = get_copy_memlimit(source)?;

    // Avoid negative limits.
    if timelimit < 0.0 {
        timelimit = 0.0;
    }
    if memorylimit < 0.0 {
        memorylimit = 0.0;
    }

    // Set time and memory limit to the adjusted values.
    paramset_set_real(target, "limits/time", timelimit)?;
    paramset_set_real(target, "limits/memory", memorylimit)?;

    // Copy and adjust soft time limit (or disable it).
    if crate::paramset::paramset_get_param(target, "limits/softtime").is_some() {
        let mut softtime = crate::paramset::paramset_get_real(source, "limits/softtime")?;
        if !source.set.is_negative(softtime) {
            softtime -= api::get_solving_time(source);
            softtime = crate::def::max(0.0, softtime);
        }
        paramset_set_real(target, "limits/softtime", softtime)?;
    }

    // Disable all other limits.
    paramset_set_real(target, "limits/absgap", 0.0)?;
    paramset_set_int(target, "limits/bestsol", -1)?;
    paramset_set_real(target, "limits/gap", 0.0)?;
    paramset_set_longint(target, "limits/nodes", -1)?;
    paramset_set_int(target, "limits/restarts", -1)?;
    paramset_set_int(target, "limits/solutions", -1)?;
    paramset_set_longint(target, "limits/stallnodes", -1)?;
    paramset_set_longint(target, "limits/totalnodes", -1)?;
    paramset_set_real(target, "limits/primal", crate::def::INVALID)?;
    paramset_set_real(target, "limits/dual", crate::def::INVALID)?;
    Ok(())
}

/// `SCIPsetCommonSubscipParams`: the limits and search settings most
/// sub-SCIP heuristics use.
pub fn set_common_subscip_params(source: &Scip, subscip: &mut Scip, nsubnodes: i64, nstallnodes: i64, bestsollimit: i32) -> ScipResult {
    use crate::paramset::{ParamSetting, paramset_set_bool, paramset_set_char, paramset_set_int, paramset_set_longint};

    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(subscip, "misc/catchctrlc", false)?;

    // Disable statistic timing inside sub SCIP and output to console.
    paramset_set_int(subscip, "display/verblevel", 0)?;
    paramset_set_bool(subscip, "timing/statistictiming", false)?;

    // Set limits for the subproblem.
    copy_limits(source, subscip)?;
    paramset_set_longint(subscip, "limits/nodes", nsubnodes)?;
    paramset_set_longint(subscip, "limits/stallnodes", nstallnodes)?;
    paramset_set_int(subscip, "limits/bestsol", bestsollimit)?;

    // Forbid recursive call of heuristics and separators solving subMIPs.
    crate::paramset::set_subscips_off(subscip, true)?;

    // Disable cutting plane separation.
    crate::paramset::set_separating(subscip, ParamSetting::Off, true)?;

    // Disable expensive presolving.
    crate::paramset::set_presolving(subscip, ParamSetting::Fast, true)?;

    // Use best estimate node selection.
    if crate::set::set_find_nodesel(subscip, "estimate") != NONE && !crate::paramset::paramset_is_fixed(subscip, "nodeselection/estimate/stdpriority")? {
        paramset_set_int(subscip, "nodeselection/estimate/stdpriority", i32::MAX / 4)?;
    }

    // Activate uct node selection at the top of the tree.
    let useuct = crate::paramset::paramset_get_bool(source, "heuristics/useuctsubscip")?;
    if useuct && crate::set::set_find_nodesel(subscip, "uct") != NONE && !crate::paramset::paramset_is_fixed(subscip, "nodeselection/uct/stdpriority")? {
        paramset_set_int(subscip, "nodeselection/uct/stdpriority", i32::MAX / 2)?;
    }

    // Use inference branching.
    if crate::set::set_find_branchrule(subscip, "inference") != NONE && !crate::paramset::paramset_is_fixed(subscip, "branching/inference/priority")? {
        paramset_set_int(subscip, "branching/inference/priority", i32::MAX / 4)?;
    }

    // Enable conflict analysis, disable analysis of boundexceeding LPs, and
    // restrict conflict pool.
    if !crate::paramset::paramset_is_fixed(subscip, "conflict/enable")? {
        paramset_set_bool(subscip, "conflict/enable", true)?;
    }
    if !crate::paramset::paramset_is_fixed(subscip, "conflict/useboundlp")? {
        paramset_set_char(subscip, "conflict/useboundlp", b'o')?;
    }
    if !crate::paramset::paramset_is_fixed(subscip, "conflict/maxstoresize")? {
        paramset_set_int(subscip, "conflict/maxstoresize", 100)?;
    }

    // Speed up sub-SCIP by not checking dual LP feasibility.
    paramset_set_bool(subscip, "lp/checkdualfeas", false)?;
    Ok(())
}

/// `createRows` (`heuristics.c`): linear constraints in `subscip` from the
/// LP rows of `scip`.
fn create_rows(scip: &mut Scip, subscip: &mut Scip, maps: &CopyMaps) -> ScipResult {
    // Get the rows and their number.
    let rows = api::get_lp_rows(scip);

    // Copy all rows to linear constraints.
    for row in rows {
        // Ignore rows that are only locally valid.
        if scip.rows[row].local {
            continue;
        }

        // Get the row's data.
        let r = &scip.rows[row];
        let constant = r.constant;
        let lhs = r.lhs - constant;
        let rhs = r.rhs - constant;
        let nnonz = r.len as usize;
        let vals = r.vals.0[..nnonz].to_vec();
        let cols: &[ColId] = &r.cols.0[..nnonz];
        let name = r.name.clone();

        // The subproblem variables of the row.
        let consvars: Vec<VarId> = cols.iter().map(|&col| maps.var_image(scip.cols[col].var)).collect();

        // Create a new linear constraint and add it to the subproblem.
        let mut cons = crate::cons_linear::create_cons_linear(subscip, &name, &consvars, &vals, lhs, rhs, true, true, true, true, true, false, false, true, true, false)?;
        crate::scip_cons::add_cons(subscip, cons)?;
        crate::scip_cons::release_cons(subscip, &mut cons)?;
    }
    Ok(())
}

/// `SCIPcopyLargeNeighborhoodSearch` (`heuristics.c`): creates the sub-SCIP
/// of an LNS heuristic, either a copy of the problem with the given
/// variables fixed or the LP rows of `sourcescip`.
#[allow(clippy::too_many_arguments)]
pub fn copy_large_neighborhood_search(
    source: &mut Scip,
    subscip: &mut Scip,
    maps: &mut CopyMaps,
    suffix: &str,
    fixedvars: &[VarId],
    fixedvals: &[f64],
    uselprows: bool,
    copycuts: bool,
) -> ScipResult<(bool, bool)> {
    let mut valid = false;
    if uselprows {
        // Copy all plugins.
        crate::plugins::include_default_plugins(subscip)?;

        copy_param_settings(source, subscip)?;

        // Set name to the original problem name and possibly add a suffix.
        let probname = format!("{}_{}", source.probs[crate::prob::ORIGPROB].name, suffix);

        // Create the subproblem.
        crate::scip_solve::create_prob(subscip, &probname)?;

        // Copy all variables.
        copy_vars(source, subscip, maps, fixedvars, fixedvals, true)?;

        // Create linear constraints from LP rows of the source problem.
        create_rows(source, subscip, maps)?;
    } else {
        valid = copy_cons_compression(source, subscip, maps, suffix, fixedvars, fixedvals, true, false)?;

        if copycuts {
            // Copies all active cuts from cutpool of sourcescip to linear
            // constraints in targetscip.
            copy_cuts_public(source, subscip, maps, true)?;
        }
    }

    // Disable bound limits in subscip since objective might be changed.
    crate::paramset::paramset_set_real(subscip, "limits/primal", crate::def::INVALID)?;
    crate::paramset::paramset_set_real(subscip, "limits/dual", crate::def::INVALID)?;

    Ok((true, valid))
}
