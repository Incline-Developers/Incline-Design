//! Port of `cons_components.c`: constraint handler that detects independent
//! components of the problem and, in presolving, solves the small ones in
//! a sub-SCIP and fixes their variables.
//!
//! Solving components during branch-and-bound (the propagation method,
//! `constraints/components/propfreq`, off by default) is not ported.

use std::any::Any;

use crate::{
    api,
    cons::{ConsFlags, PRESOLTIMING_FINAL, PROPTIMING_BEFORELP, PresolCounts, PresolTiming, PropTiming},
    copy::CopyMaps,
    misc::Digraph,
    paramset::{ParamSetting, paramset_set_bool, paramset_set_int, paramset_set_longint, paramset_set_real},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::{Scip, Stage},
    types::{ConsId, ConshdlrId, LockType, ScipResultCode, SolId, Status, VarId, VarType},
    var::NONE,
};

const CONSHDLR_NAME: &str = "components";
const CONSHDLR_DESC: &str = "independent components constraint handler";
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = 0;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -9999999;
/// Frequency for using all instead of only the useful constraints in
/// separation, propagation and enforcement, -1 for no eager evaluations, 0
/// for first only.
const CONSHDLR_EAGERFREQ: i32 = -1;
/// Should the constraint handler be skipped, if no constraints are
/// available?
const CONSHDLR_NEEDSCONS: bool = false;
/// Frequency for propagating domains; zero means only preprocessing
/// propagation.
const CONSHDLR_PROPFREQ: i32 = -1;
/// Maximal number of presolving rounds the constraint handler participates
/// in (-1: no limit).
const CONSHDLR_MAXPREROUNDS: i32 = -1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const CONSHDLR_DELAYPROP: bool = true;
/// Presolving timing of the constraint handler (fast, medium, or
/// exhaustive).
const CONSHDLR_PRESOLTIMING: PresolTiming = PRESOLTIMING_FINAL;
/// Propagation timing mask of the constraint handler.
const CONSHDLR_PROP_TIMING: PropTiming = PROPTIMING_BEFORELP;

/// Maximum depth of a node to run components detection (-1: disable
/// component detection during solving).
const DEFAULT_MAXDEPTH: i32 = i32::MAX;
/// Maximum number of integer (or binary) variables to solve a subproblem
/// directly in presolving (-1: no solving).
const DEFAULT_MAXINTVARS: i32 = 200;
/// Minimum absolute size (in terms of variables) to solve a component
/// individually during branch-and-bound.
const DEFAULT_MINSIZE: i32 = 50;
/// Minimum relative size (in terms of variables) to solve a component
/// individually during branch-and-bound.
const DEFAULT_MINRELSIZE: f64 = 0.1;
/// Maximum number of nodes to be solved in subproblems during presolving.
const DEFAULT_NODELIMIT: i64 = 10000;
/// The maximum weight of a component to be presolved.
const DEFAULT_MAXCOMPWEIGHT: f64 = 200.0;
/// The weight of an integer variable compared to binary variables.
const DEFAULT_INTFACTOR: f64 = 1.0;
/// The weight of a continuous variable compared to a binary variable.
const DEFAULT_CONTFACTOR: f64 = 0.2;
/// Default value for parameter to increase the feasibility tolerance in all
/// sub-SCIPs.
const DEFAULT_FEASTOLFACTOR: f64 = 1.0;

/// `SCIP_ConshdlrData`: constraint handler data.
#[derive(Default)]
struct ConshdlrData {
    nodelimit: i64,
    maxcompweight: f64,
    intfactor: f64,
    contfactor: f64,
    feastolfactor: f64,
    maxintvars: i32,
    maxdepth: i32,
    minsize: i32,
    minrelsize: f64,
    subscipdepth: i32,
}

fn chd(scip: &Scip, conshdlr: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[conshdlr].conshdlrdata.as_ref().unwrap().downcast_ref().unwrap()
}

/// `getMinsize`: the minimum size of components to solve individually.
fn get_minsize(scip: &Scip, d: &ConshdlrData) -> i32 {
    let minsize = (d.minrelsize * api::get_n_vars(scip) as f64) as i32;
    if minsize >= d.minsize { minsize } else { d.minsize }
}

/// `createSubscip`: creates the sub-SCIP the components are solved in, or
/// `None` if the plugins could not be copied.
fn create_subscip(scip: &Scip, d: &ConshdlrData) -> ScipResult<Option<Box<Scip>>> {
    // Create a new SCIP instance.
    let mut subscip = Scip::create()?;

    // Copy plugins, we omit pricers (because we do not run if there are
    // active pricers) and dialogs.
    let success = crate::copy::copy_plugins(scip, &mut subscip, true, true, true, true, true, true, true, true, true, true, true)?;

    // The plugins were successfully copied.
    if !success {
        return Ok(None);
    }

    // Copy parameter settings.
    crate::copy::copy_param_settings(scip, &mut subscip)?;

    // Some general settings should not be fixed; disable solution limits.
    paramset_set_int(&mut subscip, "limits/solutions", -1)?;
    paramset_set_int(&mut subscip, "limits/bestsol", -1)?;

    // Disable bound limits.
    paramset_set_real(&mut subscip, "limits/primal", crate::def::INVALID)?;
    paramset_set_real(&mut subscip, "limits/dual", crate::def::INVALID)?;

    // Reduce the effort spent for hash tables.
    paramset_set_bool(&mut subscip, "misc/usevartable", false)?;
    paramset_set_bool(&mut subscip, "misc/useconstable", false)?;

    // Disable presolving.
    crate::paramset::set_presolving(&mut subscip, ParamSetting::Off, true)?;

    // Disable component presolving and fix the parameter.
    paramset_set_int(&mut subscip, "constraints/components/maxprerounds", 0)?;
    crate::paramset::paramset_fix(&mut subscip, "constraints/components/maxprerounds", true)?;

    // Find the components constraint handler in the sub-SCIP and inform it
    // about the actual depth in the tree.
    let newconshdlr = crate::set::set_find_conshdlr(&subscip, CONSHDLR_NAME);
    let subscipdepth = d.subscipdepth + api::get_depth(scip);
    subscip.conshdlrs[newconshdlr]
        .conshdlrdata
        .as_mut()
        .unwrap()
        .downcast_mut::<ConshdlrData>()
        .unwrap()
        .subscipdepth = subscipdepth;

    // Disable output.
    paramset_set_int(&mut subscip, "display/verblevel", 0)?;

    Ok(Some(subscip))
}

/// `copyToSubscip`: copies the given variables and constraints into the
/// sub-SCIP; returns success.
#[allow(clippy::too_many_arguments)]
fn copy_to_subscip(scip: &mut Scip, subscip: &mut Scip, name: &str, vars: &[VarId], subvars: &mut [VarId], conss: &[ConsId], maps: &mut CopyMaps) -> ScipResult<bool> {
    // Create problem in sub-SCIP.
    crate::copy::copy_prob_transformed(scip, subscip, name)?;

    // Copy variables.
    for (i, &var) in vars.iter().enumerate() {
        let (subvar, success) = crate::copy::get_var_copy(scip, subscip, var, Some(maps), false)?;
        subvars[i] = subvar;

        // Abort if variable was not successfully copied.
        if !success {
            return Ok(false);
        }
    }

    // Copy constraints.
    for &cons in conss {
        let c = &scip.conss[cons];
        let flags = ConsFlags {
            initial: c.initial,
            separate: c.separate,
            enforce: c.enforce,
            check: c.check,
            propagate: c.propagate,
            local: false,
            modifiable: false,
            dynamic: c.dynamic,
            removable: c.removable,
            stickingatnode: false,
        };
        let conshdlr = c.conshdlr;

        // Copy the constraint.
        let (mut newcons, success) = crate::copy::get_cons_copy(scip, subscip, cons, conshdlr, maps, None, &flags, false)?;

        // Abort if constraint was not successfully copied.
        if !success {
            return Ok(false);
        }

        crate::scip_cons::add_cons(subscip, newcons)?;
        crate::scip_cons::release_cons(subscip, &mut newcons)?;
    }

    Ok(true)
}

/// `solveSubscip`: solves the sub-SCIP with the given node and gap limits
/// and the remaining time of the main SCIP.
fn solve_subscip(scip: &Scip, subscip: &mut Scip, nodelimit: i64, gaplimit: f64) -> ScipResult {
    // Set time limit.
    let mut timelimit = crate::paramset::paramset_get_real(scip, "limits/time")?;
    if !scip.set.is_infinity(timelimit) {
        timelimit -= api::get_solving_time(scip);
        timelimit += api::get_solving_time(subscip);
    }

    // Subtract the memory already used by the main SCIP and the estimated
    // memory usage of external software: SCIP's memory accounting is not
    // ported, both count as zero.
    let memorylimit = crate::paramset::paramset_get_real(scip, "limits/memory")?;

    // Check if mem limit needs to be avoided.
    let avoidmemout = crate::paramset::paramset_get_bool(scip, "misc/avoidmemout")?;

    // Abort if no time is left or not enough memory.
    if (avoidmemout && memorylimit <= 0.0) || timelimit <= 0.0 {
        return Ok(());
    }

    // SCIP copy limits will set wrong time limits since it does not take into
    // account time spent already in the sub-SCIP; nevertheless, we call it to
    // set the memory limit and unset all other limits, if set in the main
    // SCIP.
    crate::copy::copy_limits(scip, subscip)?;

    // Set time and memory limit for the subproblem.
    paramset_set_real(subscip, "limits/time", timelimit)?;

    // Only set soft time limit if it exists.
    if crate::paramset::paramset_get_param(scip, "limits/softtime").is_some() {
        // Set soft time limit, if specified in main SCIP and if it exists.
        let mut softtimelimit = crate::paramset::paramset_get_real(scip, "limits/softtime")?;
        if softtimelimit > -0.5 {
            softtimelimit -= api::get_solving_time(scip);
            softtimelimit += api::get_solving_time(subscip);
            softtimelimit = crate::def::max(softtimelimit, 0.0);
        }
        paramset_set_real(subscip, "limits/softtime", softtimelimit)?;
    }

    // Set gap limit.
    paramset_set_real(subscip, "limits/gap", gaplimit)?;

    // Set node limit.
    paramset_set_longint(subscip, "limits/nodes", nodelimit)?;

    // Solve the subproblem.
    crate::scip_solve::solve(subscip)
}

/// `solveAndEvalSubscip`: solves a connected component during presolving
/// and evaluates the result; returns whether it was solved to optimality.
#[allow(clippy::too_many_arguments)]
fn solve_and_eval_subscip(
    scip: &mut Scip,
    nodelimit: i64,
    subscip: &mut Scip,
    vars: &[VarId],
    subvars: &[VarId],
    conss: &[ConsId],
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult<bool> {
    let nvars = vars.len();
    let mut solved = false;

    solve_subscip(scip, subscip, nodelimit, 0.0)?;

    match api::get_status(subscip) {
        Status::Optimal => {
            let sol: SolId = api::get_best_sol(subscip);

            let mut feasible = api::check_sol_orig(subscip, sol, false, false)?;

            let mut fixvals = vec![0.0; nvars];
            if feasible {
                // Get values of variables in the optimal solution.
                for i in 0..nvars {
                    let var = vars[i];
                    let subvar = subvars[i];

                    // Get global bounds.
                    let glb = scip.vars[var].glbdom.lb;
                    let gub = scip.vars[var].glbdom.ub;

                    if subvar != NONE {
                        // Get solution value from optimal solution of the
                        // sub-SCIP.
                        fixvals[i] = api::get_sol_val(subscip, sol, subvar);

                        // Checking a solution is done with a relative
                        // tolerance of feasibility epsilon, if we really want
                        // to change the bounds of the variables by fixing
                        // them, the old bounds must not be violated by more
                        // than the absolute epsilon; therefore, we change the
                        // fixing values, if needed, and mark that the
                        // solution has to be checked again.
                        if scip.set.is_gt(fixvals[i], gub) {
                            fixvals[i] = gub;
                            feasible = false;
                        } else if scip.set.is_lt(fixvals[i], glb) {
                            fixvals[i] = glb;
                            feasible = false;
                        }
                    } else {
                        // The variable was not copied, so it was cancelled
                        // out of constraints during copying; thus, it is
                        // free in the subproblem and we can fix it to its
                        // best bound.
                        let obj = scip.vars[var].obj;
                        if scip.set.is_positive(obj) {
                            fixvals[i] = glb;
                        } else if scip.set.is_negative(obj) {
                            fixvals[i] = gub;
                        } else {
                            fixvals[i] = 0.0;
                            fixvals[i] = crate::def::min(fixvals[i], gub);
                            fixvals[i] = crate::def::max(fixvals[i], glb);
                        }
                    }
                }

                // The solution value of at least one variable is feasible with
                // a relative tolerance of feasibility epsilon, but infeasible
                // with an absolute tolerance of epsilon; try to set the
                // variables to the bounds and check solution again in the
                // original space (changed values might now violate
                // constraints).
                if !feasible {
                    let origobj = api::get_sol_orig_obj(subscip, api::get_best_sol(subscip));

                    crate::scip_solve::free_transform_public(subscip)?;
                    let sol = crate::sol::sol_create_original(subscip, crate::prob::ORIGPROB, crate::scip::ORIGPRIMAL, NONE)?;

                    // Set solution values of variables.
                    for i in 0..nvars {
                        if subvars[i] != NONE {
                            api::set_sol_val(subscip, sol, subvars[i], fixvals[i])?;
                        }
                    }

                    // Check the solution; integrality and bounds should be
                    // fulfilled and do not have to be checked.
                    feasible = api::check_sol(subscip, sol, false, false, false, false, true)?;

                    if !subscip.set.is_feas_eq(subscip.sols[sol].obj, origobj) {
                        feasible = false;
                    }

                    api::free_sol(subscip, sol)?;
                }

                // If the solution is feasible, fix variables and delete
                // constraints of the component.
                if feasible {
                    // Fix variables.
                    for i in 0..nvars {
                        crate::scip_var::fix_var(scip, vars[i], fixvals[i])?;
                        scip.vars[vars[i]].delglobalstructs = true;
                        counts.nfixedvars += 1;
                    }

                    // Delete constraints.
                    for &cons in conss {
                        crate::scip_cons::del_cons(scip, cons)?;
                        counts.ndelconss += 1;
                    }

                    *result = ScipResultCode::Success;
                    solved = true;
                }
            }
        }
        Status::Infeasible => *result = ScipResultCode::Cutoff,
        // TODO: store unbounded ray in original SCIP data structure.
        Status::Unbounded | Status::InfOrUnbd => *result = ScipResultCode::Unbounded,
        _ => {
            // Transfer global fixings to the original problem; we can only do
            // this, if we did not find a solution in the subproblem, because
            // otherwise, the primal bound might lead to dual reductions that
            // cannot be transferred to the original problem without also
            // transferring the possibly suboptimal solution (which is
            // currently not possible).
            if api::get_n_sols(subscip) == 0 {
                let mut ntightened = 0;

                for i in 0..nvars {
                    if subvars[i] == NONE {
                        continue;
                    }

                    let (_, tightened) = crate::scip_var::tighten_var_lb(scip, vars[i], subscip.vars[subvars[i]].glbdom.lb, false)?;
                    if tightened {
                        ntightened += 1;
                    }

                    let (_, tightened) = crate::scip_var::tighten_var_ub(scip, vars[i], subscip.vars[subvars[i]].glbdom.ub, false)?;
                    if tightened {
                        ntightened += 1;
                    }
                }

                *result = ScipResultCode::Success;
                counts.nchgbds += ntightened;
            }
        }
    }

    Ok(solved)
}

/// `fillDigraph`: fills the directed graph with one edge chain per
/// constraint; returns `(success, firstvaridxpercons)`.
fn fill_digraph(scip: &mut Scip, digraph: &mut Digraph, conss: &[ConsId], unfixedvarpos: &[i32]) -> ScipResult<(bool, Vec<i32>)> {
    let mut success = true;
    let mut firstvaridxpercons = vec![0i32; conss.len()];
    let mut nvars = api::get_n_vars(scip);

    for (c, &cons) in conss.iter().enumerate() {
        // Check for reached timelimit.
        if c % 1000 == 0 && crate::solve::solve_is_stopped(scip, false) {
            success = false;
            break;
        }

        // Get number of variables for this constraint.
        let nconsvars;
        (nconsvars, success) = crate::cons::cons_get_n_vars(scip, cons)?;
        if !success {
            break;
        }

        // Reallocate consvars array, if needed.
        if nconsvars > nvars {
            nvars = nconsvars;
        }

        // Get variables for this constraint.
        let consvarsvec;
        (consvarsvec, success) = crate::cons::cons_get_vars(scip, cons)?;
        if !success {
            break;
        }

        // Transform given variables to active variables.
        let mut nconsvars = consvarsvec.len() as i32;
        let mut consvars = crate::cvec::CVec(consvarsvec);
        consvars.realloc(nvars);
        let mut requiredsize = 0;
        crate::var::vars_get_active_vars(scip, &mut consvars, &mut nconsvars, nvars, &mut requiredsize)?;

        firstvaridxpercons[c] = -1;

        // Store the index of the first unfixed variable and add edges to the
        // directed graph.
        if nconsvars > 0 {
            let mut v = 0;
            let mut idx1 = -1;

            // Go through variables until the first unfixed one is reached
            // (which has unfixedvarpos >= 0).
            while idx1 == -1 && v < nconsvars {
                idx1 = scip.vars[consvars[v]].probindex;
                idx1 = unfixedvarpos[idx1 as usize];
                v += 1;
            }

            if idx1 >= 0 {
                // Save index of the first variable for later component
                // assignment.
                firstvaridxpercons[c] = idx1;

                // Create sparse directed graph; sparse means to add only
                // those edges necessary for component calculation, i.e., only
                // edges from the first variable to all others.
                while v < nconsvars {
                    let idx2 = unfixedvarpos[scip.vars[consvars[v]].probindex as usize];
                    v += 1;

                    // Variable is fixed.
                    if idx2 < 0 {
                        continue;
                    }

                    // We add only one directed edge, because the other
                    // direction is automatically added for component
                    // computation.
                    digraph.add_arc(idx1, idx2);
                }
            }
        }
    }

    Ok((success, firstvaridxpercons))
}

/// The components found by [`find_components`].
struct Components {
    sortedvars: Vec<VarId>,
    sortedconss: Vec<ConsId>,
    compstartsvars: Vec<i32>,
    compstartsconss: Vec<i32>,
    ncomponents: i32,
    ncompsmaxsize: i32,
}

/// `sortComponents`: sorts the components by size and the variables and
/// constraints by component number; returns `(ncompsminsize,
/// ncompsmaxsize)`.
#[allow(clippy::too_many_arguments)]
fn sort_components(
    scip: &Scip,
    d: &ConshdlrData,
    digraph: &Digraph,
    conss: &mut [ConsId],
    vars: &mut [VarId],
    varcomponent: &mut [i32],
    conscomponent: &mut [i32],
    firstvaridxpercons: &[i32],
) -> (i32, i32) {
    // Compute minimum size of components to solve individually.
    let minsize = get_minsize(scip, d);

    let ncomponents = digraph.get_n_components() as usize;
    let mut ncompsminsize = 0;
    let mut ncompsmaxsize = 0;

    // We want to sort the components in increasing complexity (number of
    // discrete variables, integer weighted with factor intfactor, continuous
    // used as tie-breaker through contfactor).
    let mut compsize = vec![0.0; ncomponents];
    let mut permu = vec![0i32; ncomponents];

    // Get number of variables in the components.
    for c in 0..ncomponents {
        let cvars = digraph.get_component(c as i32);
        let ncvars = cvars.len() as i32;
        permu[c] = c as i32;
        let mut nbinvars = 0;
        let mut nintvars = 0;

        for &cv in cvars {
            let var = vars[cv as usize];
            if crate::var::var_is_implied_integral(scip, var) {
                continue;
            }

            // Check whether variable is of binary or integer type.
            match scip.vars[var].vartype {
                VarType::Binary => nbinvars += 1,
                VarType::Integer => nintvars += 1,
                _ => {}
            }
        }
        let ncontvars = ncvars - nintvars - nbinvars;
        let ndiscvars = (nbinvars as f64 + d.intfactor * nintvars as f64) as i32;
        compsize[c] = nbinvars as f64 + d.intfactor * nintvars as f64 + d.contfactor * ncontvars as f64;

        // Component fulfills the maxsize and maxcompweight requirement.
        if ndiscvars <= d.maxintvars && compsize[c] <= d.maxcompweight {
            ncompsmaxsize += 1;
        }

        // Component fulfills the minsize requirement.
        if ncvars >= minsize {
            ncompsminsize += 1;
        }
    }

    // Get permutation of component numbers such that the size of the
    // components is increasing.
    crate::sort::sort(&mut compsize, (&mut permu[..],), crate::sort::cmp_real);

    // Now, we need the reverse direction, i.e., for each component number,
    // we store its new number such that the components are sorted; for this,
    // we abuse the conscomponent array.
    for c in 0..ncomponents {
        conscomponent[permu[c] as usize] = c as i32;
    }

    // For each variable, replace the old component number by the new one.
    for c in 0..vars.len() {
        varcomponent[c] = conscomponent[varcomponent[c] as usize];
    }

    // Do the mapping from calculated components per variable to
    // corresponding constraints and sort the component-arrays for faster
    // finding the actual variables and constraints belonging to one
    // component.
    for c in 0..conss.len() {
        conscomponent[c] = if firstvaridxpercons[c] == -1 {
            -1
        } else {
            varcomponent[firstvaridxpercons[c] as usize]
        };
    }

    let nvars = vars.len();
    crate::sort::sort(&mut varcomponent[..nvars], (vars,), crate::sort::cmp_int);
    let nconss = conss.len();
    crate::sort::sort(&mut conscomponent[..nconss], (conss,), crate::sort::cmp_int);

    (ncompsminsize, ncompsmaxsize)
}

/// `findComponents`: performs the components detection on the active
/// problem (the presolving case, without fixed variables).
fn find_components(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult<Components> {
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    let mut comps = Components {
        sortedvars: Vec::new(),
        sortedconss: Vec::new(),
        compstartsvars: vec![0; nvars + 1],
        compstartsconss: vec![0; nvars + 1],
        ncomponents: 0,
        ncompsmaxsize: 0,
    };

    // Collect checked constraints for component detection.
    let t = &scip.probs[TRANSPROB];
    comps.sortedconss = t.conss.0[..t.nconss as usize].to_vec();
    let nsortedconss = comps.sortedconss.len();

    if nvars > 1 && nsortedconss > 1 {
        // Count number of varlocks for each variable (up + down locks) and
        // multiply it by 2; that value is used as an estimate of the number of
        // arcs incident to the variable's node in the digraph to be safe, we
        // double this value.
        let mut varlocks = Vec::with_capacity(nvars);
        let mut unfixedvarpos = vec![0i32; nvars];
        for (v, &var) in vars.iter().enumerate() {
            // Variable is not fixed or we do not want to disregard fixed
            // variables.
            comps.sortedvars.push(var);
            varlocks.push(4 * (crate::var::var_get_n_locks_down_type(scip, var, LockType::Model) + crate::var::var_get_n_locks_up_type(scip, var, LockType::Model)));
            unfixedvarpos[v] = v as i32;
        }
        let nunfixedvars = comps.sortedvars.len();

        if nunfixedvars > 0 {
            // Create and fill directed graph.
            let mut digraph = Digraph::new(nunfixedvars as i32);
            digraph.set_sizes(&varlocks);
            let conss = comps.sortedconss.clone();
            let (success, firstvaridxpercons) = fill_digraph(scip, &mut digraph, &conss, &unfixedvarpos)?;

            if success {
                let mut varcomponent = vec![0i32; nunfixedvars];
                let mut conscomponent = vec![0i32; if nunfixedvars >= nsortedconss { nunfixedvars } else { nsortedconss }];

                // Compute independent components.
                comps.ncomponents = digraph.compute_undirected_components(1, Some(&mut varcomponent));

                if comps.ncomponents > 1 {
                    let nconss = nsortedconss;
                    let nvars = nunfixedvars;

                    // Sort components by size and sort variables and
                    // constraints by component number.
                    let d = chd(scip, conshdlr);
                    let (_, ncompsmaxsize) = sort_components(
                        scip,
                        d,
                        &digraph,
                        &mut comps.sortedconss,
                        &mut comps.sortedvars,
                        &mut varcomponent,
                        &mut conscomponent,
                        &firstvaridxpercons,
                    );
                    comps.ncompsmaxsize = ncompsmaxsize;

                    // Determine start indices of components in sortedvars and
                    // sortedconss array.
                    let mut i = 0;
                    while i < nconss && conscomponent[i] == -1 {
                        i += 1;
                    }
                    for c in 0..=comps.ncomponents {
                        comps.compstartsconss[c as usize] = i as i32;
                        while i < nconss && conscomponent[i] == c {
                            i += 1;
                        }
                    }

                    let mut i = 0;
                    for c in 0..=comps.ncomponents {
                        comps.compstartsvars[c as usize] = i as i32;
                        while i < nvars && varcomponent[i] == c {
                            i += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(comps)
}

/*
 * Callback methods of constraint handler
 */

/// `conshdlrCopyComponents`: copy method for constraint handler plugins
/// (called when SCIP copies plugins).
fn conshdlr_copy_components(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_components(scip)?;
    *valid = true;
    Ok(())
}

/// `consPropComponents`: domain propagation method of constraint handler,
/// which solves components during branch-and-bound.
fn cons_prop_components(
    _scip: &mut Scip,
    _conshdlr: ConshdlrId,
    _conss: &[ConsId],
    _nusefulconss: i32,
    _nmarkedconss: i32,
    _proptiming: PropTiming,
    _result: &mut ScipResultCode,
) -> ScipResult {
    unimplemented!("solving components during branch-and-bound (constraints/components/propfreq, off by default)");
}

/// `consPresolComponents`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_components(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    _conss: &[ConsId],
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if scip.set.stage != Stage::Presolving || api::in_probing(scip) {
        return Ok(());
    }

    // Do not run, if not all variables are explicitly known.
    if api::get_n_active_pricers(scip) > 0 {
        return Ok(());
    }

    // We do not want to run, if there are no variables left.
    if api::get_n_vars(scip) == 0 {
        return Ok(());
    }

    // Only call the components presolving, if presolving would be stopped
    // otherwise.
    if !api::is_presolve_finished_with(scip, counts) {
        return Ok(());
    }

    // The components constraint handler does kind of dual reductions.
    if !crate::scip_var::allow_strong_dual_reds(scip) || !crate::scip_var::allow_weak_dual_reds(scip) {
        return Ok(());
    }

    // Check for a reached timelimit.
    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Search for components.
    let comps = find_components(scip, conshdlr)?;

    if comps.ncompsmaxsize > 0 {
        // Build subscip.
        let d = chd(scip, conshdlr);
        let nodelimit = d.nodelimit;
        let Some(mut subscip) = create_subscip(scip, d)? else {
            return Ok(());
        };

        paramset_set_bool(&mut subscip, "misc/usesmalltables", true)?;
        paramset_set_int(&mut subscip, "constraints/components/propfreq", -1)?;

        // Hashmap mapping from original constraints to constraints in the
        // sub-SCIPs (for performance reasons).
        let mut consmap = crate::hash::HashMap::new(comps.sortedconss.len() as i32);
        let mut subvars = vec![NONE; comps.sortedvars.len()];
        let mut nsolved = 0;

        // Loop over all components.
        let mut comp = 0;
        while comp < comps.ncompsmaxsize && !crate::solve::solve_is_stopped(scip, false) {
            let c = comp as usize;
            comp += 1;

            // Get component variables.
            let compvars = &comps.sortedvars[comps.compstartsvars[c] as usize..comps.compstartsvars[c + 1] as usize];
            let ncompvars = compvars.len();

            // Get component constraints.
            let compconss = &comps.sortedconss[comps.compstartsconss[c] as usize..comps.compstartsconss[c + 1] as usize];

            // If we have an unlocked variable, let duality fixing do the job!
            if compconss.is_empty() {
                continue;
            }

            let mut maps = CopyMaps {
                varmap: crate::hash::HashMap::new(ncompvars as i32),
                consmap,
            };

            // Get name of the original problem and add "comp_nr".
            let name = format!("{}_comp_{}", scip.probs[crate::prob::ORIGPROB].name, c);

            let success = copy_to_subscip(scip, &mut subscip, &name, compvars, &mut subvars[..ncompvars], compconss, &mut maps)?;
            consmap = maps.consmap;

            if !success {
                continue;
            }

            // Solve the subproblem and evaluate the result, i.e. apply fixings
            // of variables and remove constraints.
            let solved = solve_and_eval_subscip(scip, nodelimit, &mut subscip, compvars, &subvars[..ncompvars], compconss, counts, result)?;

            if solved {
                nsolved += 1;
            }

            // If the component is unbounded or infeasible, this holds for the
            // complete problem as well. If there is only one component left,
            // let's solve this in the main SCIP.
            if *result == ScipResultCode::Unbounded || *result == ScipResultCode::Cutoff || nsolved == comps.ncomponents - 1 {
                break;
            }
        }
    }

    Ok(())
}

/// `consEnforelaxComponents`: no enforcement is performed, but the callback
/// is needed for all constraint handlers with needscons = FALSE.
fn cons_enforelax_components(
    _scip: &mut Scip,
    _sol: SolId,
    _conshdlr: ConshdlrId,
    _conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Feasible;
    Ok(())
}

/// `consLockComponents`: variable rounding lock method of constraint
/// handler.
fn cons_lock_components(_scip: &mut Scip, _conshdlr: ConshdlrId, _cons: ConsId, _locktype: LockType, _nlockspos: i32, _nlocksneg: i32) -> ScipResult {
    Ok(())
}

/*
 * constraint specific interface methods
 */

/// `SCIPincludeConshdlrComponents`: creates the components constraint
/// handler and includes it in SCIP.
pub fn include_conshdlr_components(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_int, paramset_add_longint, paramset_add_real};

    // Create components constraint data.
    let conshdlrdata = ConshdlrData {
        subscipdepth: 0,
        ..Default::default()
    };

    // Include constraint handler.
    let conshdlr = crate::cons::include_conshdlr_basic(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_NEEDSCONS,
        None,
        None,
        None,
        Some(cons_lock_components),
        Some(Box::new(conshdlrdata) as Box<dyn Any>),
    )?;

    crate::cons::set_conshdlr_prop(scip, conshdlr, Some(cons_prop_components), CONSHDLR_PROPFREQ, CONSHDLR_DELAYPROP, CONSHDLR_PROP_TIMING)?;
    crate::cons::set_conshdlr_presol(scip, conshdlr, Some(cons_presol_components), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_components);
    scip.conshdlrs[conshdlr].conshdlrcopy = Some(conshdlr_copy_components);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, $f)
        };
    }

    paramset_add_int(
        scip,
        "constraints/components/maxdepth",
        "maximum depth of a node to run components detection (-1: disable component detection during solving)",
        t!(i32, maxdepth),
        false,
        DEFAULT_MAXDEPTH,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/components/maxintvars",
        "maximum number of integer (or binary) variables to solve a subproblem during presolving (-1: unlimited)",
        t!(i32, maxintvars),
        true,
        DEFAULT_MAXINTVARS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/components/minsize",
        "minimum absolute size (in terms of variables) to solve a component individually during branch-and-bound",
        t!(i32, minsize),
        true,
        DEFAULT_MINSIZE,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/components/minrelsize",
        "minimum relative size (in terms of variables) to solve a component individually during branch-and-bound",
        t!(f64, minrelsize),
        true,
        DEFAULT_MINRELSIZE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_longint(
        scip,
        "constraints/components/nodelimit",
        "maximum number of nodes to be solved in subproblems during presolving",
        t!(i64, nodelimit),
        false,
        DEFAULT_NODELIMIT,
        -1,
        i64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/components/maxcompweight",
        "the maximum weight of a component, in terms of the used factors",
        t!(f64, maxcompweight),
        false,
        DEFAULT_MAXCOMPWEIGHT,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/components/intfactor",
        "the weight of an integer variable compared to binary variables",
        t!(f64, intfactor),
        false,
        DEFAULT_INTFACTOR,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/components/contfactor",
        "the weight of a continuous variable compared to binary variables",
        t!(f64, contfactor),
        false,
        DEFAULT_CONTFACTOR,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/components/feastolfactor",
        "factor to increase the feasibility tolerance of the main SCIP in all sub-SCIPs, default value 1.0",
        t!(f64, feastolfactor),
        true,
        DEFAULT_FEASTOLFACTOR,
        0.0,
        1000000.0,
        None,
    )?;

    Ok(())
}
