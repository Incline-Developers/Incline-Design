//! Port of `prop_probing.c`: probing propagator on binary variables.
//!
//! In presolving every binary variable is fixed tentatively to 0 and to 1;
//! fixings, aggregations, implications and bound changes valid in both
//! branches are applied (`SCIPapplyProbingVar`,
//! `SCIPanalyzeDeductionsProbing`).

use std::any::Any;

use crate::{
    api,
    cons::*,
    misc::RandNumGen,
    retcode::ScipResult,
    scip::{Scip, Stage},
    scip_probing, scip_var,
    types::{BoundType, LockType, PropId, ScipResultCode, VarId, VarType},
    var::{var_get_n_cliques, var_get_n_impls, var_get_n_locks_down_type, var_get_n_locks_up_type, var_is_active, var_is_binary, var_is_implied_integral, var_is_integral},
};

const PROP_NAME: &str = "probing";
const PROP_DESC: &str = "probing propagator on binary variables";
const PROP_TIMING: PropTiming = PROPTIMING_AFTERLPLOOP;
/// Propagation priority.
const PROP_PRIORITY: i32 = -100000;
/// Propagation frequency.
const PROP_FREQ: i32 = -1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const PROP_DELAY: bool = true;
/// Priority of the presolving method (>= 0: before, < 0: after constraint
/// handlers); combined with presolvers.
const PROP_PRESOL_PRIORITY: i32 = -100000;
/// Timing of the presolving method (fast, medium, or exhaustive).
const PROP_PRESOLTIMING: PresolTiming = PRESOLTIMING_EXHAUSTIVE;
/// Maximal number of presolving rounds the presolver participates in (-1: no
/// limit).
const PROP_PRESOL_MAXROUNDS: i32 = -1;
/// Maximal denominator for simple rational fixed values.
const MAXDNOM: i64 = 10000;

/* Default parameter settings */

/// Maximal number of runs, probing participates in (-1: no limit).
const DEFAULT_MAXRUNS: i32 = 1;
/// Maximal number of propagation rounds in probing subproblems.
const DEFAULT_PROPROUNDS: i32 = -1;
/// Maximal number of fixings found, until probing is interrupted (0: don't
/// interrupt).
const DEFAULT_MAXFIXINGS: i32 = 25;
/// Maximal number of successive probings without fixings, until probing is
/// aborted (0: don't abort).
const DEFAULT_MAXUSELESS: i32 = 1000;
/// Maximal number of successive probings without fixings, bound changes,
/// and implications, until probing is aborted (0: don't abort).
const DEFAULT_MAXTOTALUSELESS: i32 = 50;
/// Maximal number of probings without fixings, until probing is aborted (0:
/// don't abort).
const DEFAULT_MAXSUMUSELESS: i32 = 0;
/// Maximal depth until propagation is executed (-1: no limit).
const DEFAULT_MAXDEPTH: i32 = -1;
/// Random initial seed.
const DEFAULT_RANDSEED: u32 = 59;

/// Propagator data.
#[derive(Default)]
struct PropData {
    /// Problem variables sorted by number of rounding locks, used in
    /// presolving (`None`: not created).
    sortedvars: Option<Vec<VarId>>,
    /// How often we already probed on each variable (by variable index).
    nprobed: Vec<i32>,
    noldtotalvars: i32,
    nsortedvars: i32,
    nsortedbinvars: i32,
    maxruns: i32,
    proprounds: i32,
    maxfixings: i32,
    maxuseless: i32,
    maxtotaluseless: i32,
    maxsumuseless: i32,
    startidx: i32,
    lastsortstartidx: i32,
    nfixings: i32,
    naggregations: i32,
    nimplications: i32,
    nbdchgs: i32,
    nuseless: i32,
    ntotaluseless: i32,
    nsumuseless: i32,
    maxdepth: i32,
    /// Last node where probing was applied, or -1 for presolving, and -2 for
    /// not applied yet.
    lastnode: i64,
    randnumgen: Option<RandNumGen>,
}

fn pd(scip: &Scip, prop: PropId) -> &PropData {
    scip.props[prop].propdata.as_ref().unwrap().downcast_ref::<PropData>().unwrap()
}

fn pdm(scip: &mut Scip, prop: PropId) -> &mut PropData {
    scip.props[prop].propdata.as_mut().unwrap().downcast_mut::<PropData>().unwrap()
}

/*
 * Local methods
 */

/// `initPropdata`: initializes the propagator data.
fn init_propdata(propdata: &mut PropData) {
    propdata.sortedvars = None;
    propdata.nprobed = Vec::new();
    propdata.noldtotalvars = 0;
    propdata.nsortedvars = 0;
    propdata.nsortedbinvars = 0;
    propdata.startidx = 0;
    propdata.lastsortstartidx = -1;
    propdata.nfixings = 0;
    propdata.naggregations = 0;
    propdata.nimplications = 0;
    propdata.nbdchgs = 0;
    propdata.nuseless = 0;
    propdata.ntotaluseless = 0;
    propdata.nsumuseless = 0;
    propdata.lastnode = -2;
    propdata.randnumgen = None;
}

/// `freeSortedvars`: frees the sorted vars array.
fn free_sortedvars(scip: &mut Scip, prop: PropId) -> ScipResult {
    if let Some(mut sortedvars) = pdm(scip, prop).sortedvars.take() {
        // Release variables.
        let n = pd(scip, prop).nsortedvars as usize;
        for v in sortedvars.iter_mut().take(n) {
            api::release_var(scip, v)?;
        }
        let p = pdm(scip, prop);
        p.nsortedvars = 0;
        p.nsortedbinvars = 0;
    }

    let p = pdm(scip, prop);
    p.nprobed = Vec::new();
    p.noldtotalvars = 0;

    Ok(())
}

/// The variable's lock and implication/clique counts used for its probing
/// score.
fn var_counts(scip: &Scip, var: VarId) -> (i32, i32, i32, i32, i32, i32) {
    (
        var_get_n_locks_down_type(scip, var, LockType::Model),
        var_get_n_locks_up_type(scip, var, LockType::Model),
        var_get_n_impls(scip, var, false),
        var_get_n_impls(scip, var, true),
        var_get_n_cliques(scip, var, false),
        var_get_n_cliques(scip, var, true),
    )
}

/// `sortVariables`: sorts the binary variables starting with the given index
/// by rounding locks and implications.
fn sort_variables(scip: &mut Scip, prop: PropId, vars: &mut [VarId], nvars: usize, firstidx: usize) {
    if nvars <= firstidx {
        return;
    }
    let nsortedvars = nvars - firstidx;

    // Sort the variables by number of rounding locks and implications.
    let mut scores = vec![0.0f64; nsortedvars];

    let mut maxscore = -1.0;
    let mut minnprobings = i32::MAX;

    // Determine maximal possible score and minimal number of probings over
    // all variables.
    for &var in &vars[..nvars] {
        if var_is_active(scip, var) {
            let (nlocksdown, nlocksup, nimplzero, nimplone, nclqzero, nclqone) = var_counts(scip, var);

            let tmp = -crate::def::max(nlocksdown, nlocksup) as f64 + 10.0 * crate::def::min(nimplzero, nimplone) as f64 + 100.0 * crate::def::min(nclqzero, nclqone) as f64;

            if tmp > maxscore {
                maxscore = tmp;
            }
            let idx = scip.vars[var].index as usize;
            if pd(scip, prop).nprobed[idx] < minnprobings {
                minnprobings = pd(scip, prop).nprobed[idx];
            }
        }
    }

    // Correct number of probings on each variable by minimal number of
    // probings.
    if minnprobings > 0 {
        for &var in &vars[..nvars] {
            if var_is_active(scip, var) {
                let idx = scip.vars[var].index as usize;
                pdm(scip, prop).nprobed[idx] -= minnprobings;
            }
        }
    }

    for i in 0..nsortedvars {
        let var = vars[firstidx + i];

        // Prefer variables that we did not already probe on.
        if var_is_active(scip, var) {
            let (nlocksdown, nlocksup, nimplzero, nimplone, nclqzero, nclqone) = var_counts(scip, var);

            // Use a random offset to break possible ties arbitrarily.
            let randomoffset = pdm(scip, prop).randnumgen.as_mut().unwrap().get_real(0.0, 0.5);

            let nprobed = pd(scip, prop).nprobed[scip.vars[var].index as usize];
            scores[i] = -maxscore * nprobed as f64 - crate::def::max(nlocksdown, nlocksup) as f64
                + 10.0 * crate::def::min(nimplzero, nimplone) as f64
                + 100.0 * crate::def::min(nclqzero, nclqone) as f64
                - randomoffset; // to break ties randomly
        } else {
            scores[i] = -scip.set.infinity();
        }
    }

    crate::sort::sort_down(&mut scores[..], (&mut vars[firstidx..nvars],), crate::sort::cmp_real);
}

/// Bounds stored by `SCIPapplyProbingVar`.
struct ProbeBounds {
    impllbs: Vec<f64>,
    implubs: Vec<f64>,
    proplbs: Vec<f64>,
    propubs: Vec<f64>,
}

impl ProbeBounds {
    fn new(n: usize) -> Self {
        Self {
            impllbs: vec![0.0; n],
            implubs: vec![0.0; n],
            proplbs: vec![0.0; n],
            propubs: vec![0.0; n],
        }
    }
}

/// `applyProbing`: the main probing loop.
#[allow(clippy::too_many_arguments)]
fn apply_probing(
    scip: &mut Scip,
    prop: PropId,
    mut vars: Vec<VarId>,
    mut nvars: usize,
    mut nbinvars: usize,
    startidx: &mut i32,
    nfixedvars: &mut i32,
    naggrvars: &mut i32,
    nchgbds: &mut i32,
    oldnfixedvars: i32,
    oldnaggrvars: i32,
    delay: &mut bool,
    cutoff: &mut bool,
) -> ScipResult {
    let (maxfixings, maxuseless, maxtotaluseless, maxsumuseless, proprounds) = {
        let p = pd(scip, prop);
        let lim = |v: i32| if v > 0 { v } else { i32::MAX };
        (lim(p.maxfixings), lim(p.maxuseless), lim(p.maxtotaluseless), lim(p.maxsumuseless), p.proprounds)
    };
    let mut aborted = false;
    let mut looped = false;
    let mut oldstartidx = *startidx as usize;
    let mut i = *startidx as usize;

    // Temporary memory for storing probing results.
    let mut zero = ProbeBounds::new(nvars);
    let mut one = ProbeBounds::new(nvars);

    // For each binary variable, probe fixing the variable to zero and one.
    *delay = false;
    *cutoff = false;
    loop {
        while i < nbinvars && !*cutoff {
            // Check whether probing should be aborted.
            {
                let p = pd(scip, prop);
                if p.nuseless >= maxuseless || p.ntotaluseless >= maxtotaluseless || p.nsumuseless >= maxsumuseless || crate::solve::solve_is_stopped(scip, false) {
                    aborted = true;
                    break;
                }
            }

            // Check if we already fixed enough variables for this round, or
            // probed on all variables.
            if *nfixedvars - oldnfixedvars + *naggrvars - oldnaggrvars >= maxfixings || (looped && oldstartidx == i) {
                if *nfixedvars - oldnfixedvars + *naggrvars - oldnaggrvars > 0 {
                    *delay = true;
                } else {
                    aborted = true;
                }
                break;
            }

            let var = vars[i];

            // Ignore variables, that were fixed, aggregated, or deleted in
            // prior probings.
            let skip = |scip: &Scip| !var_is_active(scip, var) || scip.vars[var].deleted || scip.vars[var].locdom.lb > 0.5 || scip.vars[var].locdom.ub < 0.5;
            if skip(scip) {
                i += 1;
                continue;
            }

            {
                let p = pdm(scip, prop);
                if p.nuseless > 0 {
                    p.nsumuseless += 1;
                } else {
                    p.nsumuseless = crate::def::max(p.nsumuseless - 1, 0);
                }
                p.nuseless += 1;
                p.ntotaluseless += 1;
            }

            // Determine whether one probing should happen.
            let probingone = var_get_n_locks_up_type(scip, var, LockType::Model) != 0;

            if probingone {
                // Apply probing for fixing the variable to one.
                let localcutoff = apply_probing_var(scip, &vars, nvars, i, BoundType::Lower, 1.0, proprounds, &mut one)?;

                if localcutoff {
                    let (c, fixed) = if scip.set.stage != Stage::Solving || scip.nodes[crate::tree::tree_get_current_node(scip)].depth == 0 {
                        // The variable can be fixed to FALSE.
                        scip_var::fix_var(scip, var, 0.0)?
                    } else {
                        scip_var::tighten_var_ub(scip, var, 0.0, true)?
                    };
                    *cutoff = c;

                    if fixed {
                        *nfixedvars += 1;
                        let p = pdm(scip, prop);
                        p.nfixings += 1;
                        p.nuseless = 0;
                        p.ntotaluseless = 0;
                    }
                    // Don't try downwards direction, because the variable is
                    // already fixed.
                    i += 1;
                    continue;
                }

                // Ignore variables, that were fixed, aggregated, or deleted in
                // prior probings (propagators in one-probe might have found
                // global fixings but did not trigger the localcutoff).
                if skip(scip) {
                    i += 1;
                    continue;
                }
            }

            // Determine whether zero probing should happen.
            let probingzero = var_get_n_locks_down_type(scip, var, LockType::Model) != 0;

            if probingzero {
                // Apply probing for fixing the variable to zero.
                let localcutoff = apply_probing_var(scip, &vars, nvars, i, BoundType::Upper, 0.0, proprounds, &mut zero)?;

                if localcutoff {
                    let (c, fixed) = if scip.set.stage != Stage::Solving || scip.nodes[crate::tree::tree_get_current_node(scip)].depth == 0 {
                        // The variable can be fixed to TRUE.
                        scip_var::fix_var(scip, var, 1.0)?
                    } else {
                        scip_var::tighten_var_lb(scip, var, 1.0, true)?
                    };
                    *cutoff = c;

                    if fixed {
                        *nfixedvars += 1;
                        let p = pdm(scip, prop);
                        p.nfixings += 1;
                        p.nuseless = 0;
                        p.ntotaluseless = 0;
                    }
                    // Don't analyze probing deductions, because the variable
                    // is already fixed.
                    i += 1;
                    continue;
                }
            }

            // Not have to check deductions if only one probing direction has
            // been checked.
            if !probingzero || !probingone {
                i += 1;
                continue;
            }

            // Count number of probings on each variable.
            let idx = scip.vars[var].index as usize;
            pdm(scip, prop).nprobed[idx] += 1;

            // Analyze probing deductions.
            let mut localnfixedvars = 0;
            let mut localnaggrvars = 0;
            let mut localnimplications = 0;
            let mut localnchgbds = 0;
            analyze_deductions_probing(
                scip,
                var,
                0.0,
                1.0,
                nvars,
                &vars,
                Some((&zero.impllbs, &zero.implubs)),
                &zero.proplbs,
                &zero.propubs,
                Some((&one.impllbs, &one.implubs)),
                &one.proplbs,
                &one.propubs,
                &mut localnfixedvars,
                &mut localnaggrvars,
                &mut localnimplications,
                &mut localnchgbds,
                cutoff,
            )?;

            *nfixedvars += localnfixedvars;
            *naggrvars += localnaggrvars;
            *nchgbds += localnchgbds;
            let p = pdm(scip, prop);
            p.nfixings += localnfixedvars;
            p.naggregations += localnaggrvars;
            p.nbdchgs += localnchgbds;
            p.nimplications += localnimplications;

            if localnfixedvars > 0 || localnaggrvars > 0 {
                p.nuseless = 0;
                p.ntotaluseless = 0;
            }
            if localnimplications > 0 || localnchgbds > 0 {
                p.ntotaluseless = 0;
            }
            i += 1;
        }

        looped = true;

        // Check if we reached the end of all binary variables but did not
        // stop, so we start from the beginning.
        if i == nbinvars && !*cutoff && !*delay && !aborted {
            i = 0;

            if scip.set.stage == Stage::Presolving {
                // Release old variables and free memory.
                let mut old = pdm(scip, prop).sortedvars.take().unwrap();
                let n = pd(scip, prop).nsortedvars as usize;
                for v in (0..n).rev() {
                    api::release_var(scip, &mut old[v])?;
                }
                {
                    let p = pdm(scip, prop);
                    p.nsortedvars = 0;
                    p.nsortedbinvars = 0;
                }

                // Get new variables.
                let mut newvars = api::get_vars(scip).to_vec();
                let nnewvars = newvars.len();

                let mut nnewbinvars = api::get_n_bin_vars(scip) as usize;
                let nnewintvars = api::get_n_int_vars(scip) as usize;
                let nnewimplvars = api::get_n_impl_vars(scip) as usize;

                // Determine implicit binary variables.
                let lastidx = nnewbinvars + nnewintvars + nnewimplvars;
                let firstnonbin = nnewbinvars;
                for v in firstnonbin..lastidx {
                    if var_is_binary(scip, newvars[v]) {
                        newvars.swap(nnewbinvars, v);
                        nnewbinvars += 1;
                    }
                }

                nbinvars = nnewbinvars;
                nvars = nnewvars;

                zero = ProbeBounds::new(nvars);
                one = ProbeBounds::new(nvars);

                // Correct oldstartidx which is used for early termination.
                if oldstartidx >= nbinvars {
                    oldstartidx = nbinvars.wrapping_sub(1);
                }

                // Capture variables to make sure, the variables are not
                // deleted.
                for v in (0..nnewvars).rev() {
                    crate::var::var_capture(scip, newvars[v]);
                }

                {
                    let p = pdm(scip, prop);
                    p.nsortedvars = nnewvars as i32;
                    p.nsortedbinvars = nnewbinvars as i32;
                }

                if nnewbinvars == 0 {
                    pdm(scip, prop).sortedvars = Some(newvars);
                    *startidx = 0;
                    let p = pdm(scip, prop);
                    p.lastsortstartidx = -1;
                    p.nuseless = 0;
                    p.ntotaluseless = 0;

                    return Ok(());
                }

                // Resorting here might lead to probing a second time on the
                // same variable.
                sort_variables(scip, prop, &mut newvars, nnewbinvars, 0);
                pdm(scip, prop).lastsortstartidx = 0;

                pdm(scip, prop).sortedvars = Some(newvars.clone());
                vars = newvars;
            }
        }

        if !(i == 0 && !*cutoff && !*delay && !aborted) {
            break;
        }
    }

    *startidx = i as i32;

    Ok(())
}

/*
 * Callback methods of propagator
 */

/// `propCopyProbing`: copy method for propagator plugins.
fn prop_copy_probing(scip: &mut Scip, _sourcescip: &Scip, _prop: PropId) -> ScipResult {
    include_prop_probing(scip)
}

/// `propInitProbing`: initialization method of propagator.
fn prop_init_probing(scip: &mut Scip, prop: PropId) -> ScipResult {
    init_propdata(pdm(scip, prop));

    // Create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    pdm(scip, prop).randnumgen = Some(RandNumGen::new(seed));

    Ok(())
}

/// `propExitProbing`: deinitialization method of propagator.
fn prop_exit_probing(scip: &mut Scip, prop: PropId) -> ScipResult {
    free_sortedvars(scip, prop)?;

    // Free random number generator.
    pdm(scip, prop).randnumgen = None;

    Ok(())
}

/// `propInitpreProbing`: presolving initialization method of propagator.
fn prop_initpre_probing(scip: &mut Scip, prop: PropId) -> ScipResult {
    pdm(scip, prop).lastnode = -2;
    Ok(())
}

/// `propExitpreProbing`: presolving deinitialization method of propagator.
fn prop_exitpre_probing(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Delete the vars array, if the maximal number of runs are exceeded.
    let maxruns = pd(scip, prop).maxruns;
    if maxruns >= 0 && api::get_n_runs(scip) >= maxruns {
        free_sortedvars(scip, prop)?;
    }

    Ok(())
}

/// `propInitsolProbing`: solving process initialization method of
/// propagator.
fn prop_initsol_probing(scip: &mut Scip, prop: PropId) -> ScipResult {
    // Reset all propdata elements for stopping propagation earlier.
    let p = pdm(scip, prop);
    p.nuseless = 0;
    p.ntotaluseless = 0;
    p.nsumuseless = 0;

    Ok(())
}

/// Grows `nprobed` to the number of total variables (indices never
/// decrease, so every variable is identified by its index).
fn grow_nprobed(scip: &mut Scip, prop: PropId) {
    let ntotalvars = scip.stat.nvaridx;
    let p = pdm(scip, prop);
    if p.noldtotalvars < ntotalvars {
        p.nprobed.resize(ntotalvars as usize, 0);
        p.noldtotalvars = ntotalvars;
    }
}

/// `propPresolProbing`: presolve method of propagator.
fn prop_presol_probing(
    scip: &mut Scip,
    prop: PropId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    let mut nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;
    let nimplvars = api::get_n_impl_vars(scip) as usize;

    // If we have no binary variable anymore, we stop probing.
    if nbinvars + nintvars + nimplvars == 0 {
        return Ok(());
    }

    // Check, if probing should be applied in the current run.
    let maxruns = pd(scip, prop).maxruns;
    if maxruns >= 0 && api::get_n_runs(scip) > maxruns {
        return Ok(());
    }

    // If no domains changed since the last call, we don't need to probe.
    if pd(scip, prop).lastnode == -1 && newcounts.nfixedvars == 0 && newcounts.naggrvars == 0 && newcounts.nchgbds == 0 && newcounts.naddholes == 0 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Allow some additional probings.
    {
        let p = pdm(scip, prop);
        p.nuseless -= p.nuseless / 10;
        p.ntotaluseless -= p.ntotaluseless / 10;
    }

    // Get variable data.
    if pd(scip, prop).sortedvars.is_none() {
        let mut sortedvars = api::get_vars(scip).to_vec();
        let nvars = sortedvars.len();

        // Determine implicit binary variables.
        let lastidx = nbinvars + nintvars + nimplvars;
        let firstnonbin = nbinvars;
        for v in firstnonbin..lastidx {
            if var_is_binary(scip, sortedvars[v]) {
                sortedvars.swap(nbinvars, v);
                nbinvars += 1;
            }
        }

        // Capture variables to make sure, the variables are not deleted.
        for v in (0..nvars).rev() {
            crate::var::var_capture(scip, sortedvars[v]);
        }

        let p = pdm(scip, prop);
        p.sortedvars = Some(sortedvars);
        p.nsortedvars = nvars as i32;
        p.nsortedbinvars = nbinvars as i32;
    }

    if pd(scip, prop).nsortedbinvars == 0 {
        return Ok(());
    }

    grow_nprobed(scip, prop);

    pdm(scip, prop).lastnode = -1;

    // Sort the binary variables by number of rounding locks, if at least 100
    // variables were probed since last sort.
    let (lastsortstartidx, startidx, nsortedvars, nsortedbinvars) = {
        let p = pd(scip, prop);
        (p.lastsortstartidx, p.startidx, p.nsortedvars as usize, p.nsortedbinvars as usize)
    };
    if lastsortstartidx < 0 || startidx - lastsortstartidx >= 100 {
        let mut sortedvars = pdm(scip, prop).sortedvars.take().unwrap();
        sort_variables(scip, prop, &mut sortedvars, nsortedbinvars, startidx as usize);
        let p = pdm(scip, prop);
        p.sortedvars = Some(sortedvars);
        p.lastsortstartidx = p.startidx;
    }

    let oldnfixedvars = counts.nfixedvars;
    let oldnaggrvars = counts.naggrvars;
    let oldnchgbds = counts.nchgbds;
    let oldnimplications = pd(scip, prop).nimplications;

    // Start probing on variables.
    let mut delay = false;
    let mut cutoff = false;
    let vars = pd(scip, prop).sortedvars.clone().unwrap();
    let mut startidx = pd(scip, prop).startidx;
    apply_probing(
        scip,
        prop,
        vars,
        nsortedvars,
        nsortedbinvars,
        &mut startidx,
        &mut counts.nfixedvars,
        &mut counts.naggrvars,
        &mut counts.nchgbds,
        oldnfixedvars,
        oldnaggrvars,
        &mut delay,
        &mut cutoff,
    )?;
    pdm(scip, prop).startidx = startidx;

    // Adjust result code.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else {
        if delay {
            // Probing was interrupted because it reached the maximal fixings
            // parameter, so we want to rerun it at the next call.
            pdm(scip, prop).lastnode = -2;
        }

        if counts.nfixedvars > oldnfixedvars || counts.naggrvars > oldnaggrvars || counts.nchgbds > oldnchgbds || pd(scip, prop).nimplications > oldnimplications {
            *result = ScipResultCode::Success;
        }
    }

    Ok(())
}

/// `propExecProbing`: execution method of propagator.
fn prop_exec_probing(scip: &mut Scip, prop: PropId, _proptiming: PropTiming, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Avoid recursive infinity loop.
    if api::in_probing(scip) {
        return Ok(());
    }

    // Only call propagation on branching candidates, if an optimal LP
    // solution is at hand.
    if api::get_lp_solstat(scip) != crate::types::LpSolStat::Optimal {
        return Ok(());
    }

    // If already called stop.
    let nodenumber = scip.nodes[crate::tree::tree_get_current_node(scip)].number;
    if pd(scip, prop).lastnode == nodenumber {
        return Ok(());
    }

    // If maximal depth for propagation is reached, stop.
    let maxdepth = pd(scip, prop).maxdepth;
    if maxdepth >= 0 && maxdepth < api::get_depth(scip) {
        return Ok(());
    }

    pdm(scip, prop).lastnode = nodenumber;

    // Get (number of) fractional variables that should be integral.
    let (vars, _, _, nvars, _, _) = api::get_lp_branch_cands(scip)?;

    // Copy binary variables to array binvars.
    let mut binvars: Vec<VarId> = vars[..nvars as usize].iter().copied().filter(|&v| var_is_binary(scip, v)).collect();
    let nbinvars = binvars.len();

    if nbinvars == 0 {
        *result = ScipResultCode::DidNotFind;
        return Ok(());
    }

    grow_nprobed(scip, prop);

    // Sort binary variables.
    sort_variables(scip, prop, &mut binvars, nbinvars, 0);

    let oldnfixedvars = 0;
    let oldnaggrvars = 0;
    let oldnchgbds = 0;
    let mut nfixedvars = 0;
    let mut naggrvars = 0;
    let mut nchgbds = 0;
    let mut startidx = 0;
    let mut delay = false;
    let mut cutoff = false;

    // Start probing on found variables.
    apply_probing(
        scip,
        prop,
        binvars,
        nbinvars,
        nbinvars,
        &mut startidx,
        &mut nfixedvars,
        &mut naggrvars,
        &mut nchgbds,
        oldnfixedvars,
        oldnaggrvars,
        &mut delay,
        &mut cutoff,
    )?;

    if delay {
        // Probing was interrupted because it reached the maximal fixings
        // parameter, so we want to rerun it at the next call.
        pdm(scip, prop).lastnode = -2;
    }

    // Adjust result code.
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if nfixedvars > oldnfixedvars || naggrvars > oldnaggrvars || nchgbds > oldnchgbds {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/// `propRespropProbing`: propagation conflict resolving method of
/// propagator.
#[allow(clippy::too_many_arguments)]
fn prop_resprop_probing(
    _scip: &mut Scip,
    _prop: PropId,
    _infervar: VarId,
    _inferinfo: i32,
    _boundtype: BoundType,
    _bdchgidx: Option<crate::var::BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;
    Ok(())
}

/*
 * propagator specific interface methods
 */

/// `SCIPincludePropProbing`: creates the probing propagator and includes it
/// in SCIP.
pub fn include_prop_probing(scip: &mut Scip) -> ScipResult {
    use crate::paramset::paramset_add_int;

    // Create probing propagator data.
    let mut propdata = PropData::default();
    init_propdata(&mut propdata);

    // Include propagator.
    let prop = api::include_prop_basic(
        scip,
        PROP_NAME,
        PROP_DESC,
        PROP_PRIORITY,
        PROP_FREQ,
        PROP_DELAY,
        PROP_TIMING,
        prop_exec_probing,
        Some(Box::new(propdata) as Box<dyn Any>),
    )?;

    // Set optional callbacks via setter functions.
    scip.props[prop].propcopy = Some(prop_copy_probing);
    scip.props[prop].propinit = Some(prop_init_probing);
    scip.props[prop].propexit = Some(prop_exit_probing);
    scip.props[prop].propinitsol = Some(prop_initsol_probing);
    scip.props[prop].propinitpre = Some(prop_initpre_probing);
    scip.props[prop].propexitpre = Some(prop_exitpre_probing);
    crate::prop::prop_set_presol(scip, prop, prop_presol_probing, PROP_PRESOL_PRIORITY, PROP_PRESOL_MAXROUNDS, PROP_PRESOLTIMING)?;
    scip.props[prop].propresprop = Some(prop_resprop_probing);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, props, prop, propdata, PropData, $f)
        };
    }

    // Add probing propagator parameters.
    paramset_add_int(
        scip,
        "propagating/probing/maxruns",
        "maximal number of runs, probing participates in (-1: no limit)",
        t!(i32, maxruns),
        false,
        DEFAULT_MAXRUNS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/probing/proprounds",
        "maximal number of propagation rounds in probing subproblems (-1: no limit, 0: auto)",
        t!(i32, proprounds),
        true,
        DEFAULT_PROPROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/probing/maxfixings",
        "maximal number of fixings found, until probing is interrupted (0: don't iterrupt)",
        t!(i32, maxfixings),
        true,
        DEFAULT_MAXFIXINGS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/probing/maxuseless",
        "maximal number of successive probings without fixings, until probing is aborted (0: don't abort)",
        t!(i32, maxuseless),
        true,
        DEFAULT_MAXUSELESS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/probing/maxtotaluseless",
        "maximal number of successive probings without fixings, bound changes, and implications, until probing is aborted (0: don't abort)",
        t!(i32, maxtotaluseless),
        true,
        DEFAULT_MAXTOTALUSELESS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/probing/maxsumuseless",
        "maximal number of probings without fixings, until probing is aborted (0: don't abort)",
        t!(i32, maxsumuseless),
        true,
        DEFAULT_MAXSUMUSELESS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/probing/maxdepth",
        "maximal depth until propagation is executed(-1: no limit)",
        t!(i32, maxdepth),
        true,
        DEFAULT_MAXDEPTH,
        -1,
        i32::MAX,
        None,
    )?;

    Ok(())
}

/// `SCIPapplyProbingVar`: applies and evaluates probing of a single variable
/// in the given direction and bound; returns whether the probing direction
/// is infeasible.
#[allow(clippy::too_many_arguments)]
fn apply_probing_var(
    scip: &mut Scip,
    vars: &[VarId],
    nvars: usize,
    probingpos: usize,
    boundtype: BoundType,
    bound: f64,
    maxproprounds: i32,
    out: &mut ProbeBounds,
) -> ScipResult<bool> {
    let pvar = vars[probingpos];

    // In debug mode C asserts that this trivial infeasibility does not occur,
    // but in optimized mode it returns safely.
    if scip.set.is_lt(bound, scip.vars[pvar].locdom.lb) || scip.set.is_gt(bound, scip.vars[pvar].locdom.ub) {
        return Ok(true);
    }

    // Start probing mode.
    scip_probing::start_probing(scip)?;

    // Enables collection of variable statistics during probing.
    crate::stat::stat_enable_var_history(scip);

    // Fix variable.
    if boundtype == BoundType::Upper {
        scip_probing::chg_var_ub_probing(scip, pvar, bound)?;
    } else {
        scip_probing::chg_var_lb_probing(scip, pvar, bound)?;
    }

    // Apply propagation of implication graph and clique table.
    let mut cutoff = scip_probing::propagate_probing_implications(scip)?;
    if !cutoff {
        for i in 0..nvars {
            out.impllbs[i] = scip.vars[vars[i]].locdom.lb;
            out.implubs[i] = scip.vars[vars[i]].locdom.ub;
        }

        // Apply propagation.
        cutoff = scip_probing::propagate_probing(scip, maxproprounds)?.0;
    }

    // Evaluate propagation.
    if !cutoff {
        for i in 0..nvars {
            out.proplbs[i] = scip.vars[vars[i]].locdom.lb;
            out.propubs[i] = scip.vars[vars[i]].locdom.ub;
        }
    }

    // Exit probing mode.
    scip_probing::end_probing(scip)?;

    Ok(cutoff)
}

/// `SCIPanalyzeDeductionsProbing`: analyses boundchanges resulting from
/// probing on a variable and performs deduced fixations, aggregations, and
/// domain tightenings. Given a variable probingvar with domain [l,u] and
/// bound tightening results from reducing the domain once to [l,leftub] and
/// once to [rightlb,u], the method computes and applies resulting variable
/// fixations, aggregations, implications, and bound changes.
#[allow(clippy::too_many_arguments)]
pub fn analyze_deductions_probing(
    scip: &mut Scip,
    probingvar: VarId,
    mut leftub: f64,
    mut rightlb: f64,
    nvars: usize,
    vars: &[VarId],
    leftimpl: Option<(&[f64], &[f64])>,
    leftproplbs: &[f64],
    leftpropubs: &[f64],
    rightimpl: Option<(&[f64], &[f64])>,
    rightproplbs: &[f64],
    rightpropubs: &[f64],
    nfixedvars: &mut i32,
    naggrvars: &mut i32,
    nimplications: &mut i32,
    nchgbds: &mut i32,
    cutoff: &mut bool,
) -> ScipResult {
    let probingvarisbinary;
    let probingvarisinteger;

    if var_is_integral(scip, probingvar) {
        // Adjust bounds to actually used ones.
        leftub = scip.set.floor(leftub);
        rightlb = scip.set.ceil(rightlb);

        probingvarisinteger = true;
        probingvarisbinary = var_is_binary(scip, probingvar);
    } else {
        probingvarisbinary = false;
        probingvarisinteger = false;
    }

    // Check if probing variable was fixed in the branches.
    let fixedleft = scip.set.is_eq(scip.vars[probingvar].locdom.lb, leftub);
    let fixedright = scip.set.is_eq(scip.vars[probingvar].locdom.ub, rightlb);

    let (leftimpllbs, leftimplubs) = (leftimpl.map(|x| x.0), leftimpl.map(|x| x.1));
    let (rightimpllbs, rightimplubs) = (rightimpl.map(|x| x.0), rightimpl.map(|x| x.1));

    *cutoff = false;

    let mut j = 0;
    while j < nvars && !*cutoff {
        let var = vars[j];

        // If probing variable is binary, then there is nothing we could
        // deduce here (variable should be fixed in both branches).
        if var == probingvar && probingvarisbinary {
            j += 1;
            continue;
        }

        // New bounds of the variable is the union of the propagated bounds of
        // the left and right case.
        let newlb = crate::def::min(leftproplbs[j], rightproplbs[j]);
        let newub = crate::def::max(leftpropubs[j], rightpropubs[j]);
        let varisinteger = var_is_integral(scip, var);

        let at_root = |scip: &Scip| scip.set.stage != Stage::Solving || scip.nodes[crate::tree::tree_get_current_node(scip)].depth == 0;

        // Check for fixed variables.
        if scip.set.is_eq(newlb, newub) {
            let fixval = if !varisinteger {
                // In both probings, variable j is deduced to the same value:
                // fix variable to this value.
                crate::misc::select_simple_value(newlb - 0.9 * scip.set.epsilon(), newub + 0.9 * scip.set.epsilon(), MAXDNOM)
            } else {
                newlb
            };

            let fixed = if at_root(scip) {
                let (c, f) = scip_var::fix_var(scip, var, fixval)?;
                *cutoff = c;
                f
            } else {
                let (c, f) = scip_var::tighten_var_lb(scip, var, fixval, true)?;
                *cutoff = c;
                let mut f = f;
                if !*cutoff {
                    let (c, tightened) = scip_var::tighten_var_ub(scip, var, fixval, true)?;
                    *cutoff = c;
                    f &= tightened;
                }
                f
            };

            if fixed {
                *nfixedvars += 1;
            }

            j += 1;
            continue;
        } else {
            // Check for bound tightenings.
            let oldlb = scip.vars[var].locdom.lb;
            let oldub = scip.vars[var].locdom.ub;

            let force = true;
            let (tightenlb, tightenub) = if varisinteger {
                (newlb > oldlb + 0.5, newub < oldub - 0.5)
            } else {
                (scip.set.is_lb_better(newlb, oldlb, oldub), scip.set.is_ub_better(newub, oldlb, oldub))
            };

            if tightenlb {
                // In both probings, variable j is deduced to be at least
                // newlb: tighten lower bound.
                let (c, tightened) = scip_var::tighten_var_lb(scip, var, newlb, force)?;
                *cutoff = c;
                if tightened {
                    *nchgbds += 1;
                }
            }

            if tightenub && !*cutoff {
                // In both probings, variable j is deduced to be at most newub:
                // tighten upper bound.
                let (c, tightened) = scip_var::tighten_var_ub(scip, var, newub, force)?;
                *cutoff = c;
                if tightened {
                    *nchgbds += 1;
                }
            }
            if *cutoff {
                break;
            }
        }

        // Below we add aggregations and implications between probingvar and
        // var, we don't want this if both variables are the same.
        if var == probingvar {
            j += 1;
            continue;
        }

        // Check for aggregations and implications.
        if fixedleft && fixedright && scip.set.is_eq(leftproplbs[j], leftpropubs[j]) && scip.set.is_eq(rightproplbs[j], rightpropubs[j]) {
            // var is fixed whenever probingvar is fixed -> both variables can
            // be aggregated:
            // (rightlb - leftub) * var - (rightproplbs[j] - leftproplbs[j]) * probingvar = leftproplbs[j] * rightlb - rightproplbs[j] * leftub
            if scip.set.stage == Stage::Presolving {
                let (c, _redundant, aggregated) = scip_var::aggregate_vars(
                    scip,
                    var,
                    probingvar,
                    rightlb - leftub,
                    -(rightproplbs[j] - leftproplbs[j]),
                    leftproplbs[j] * rightlb - rightproplbs[j] * leftub,
                )?;
                *cutoff = c;

                if aggregated {
                    *naggrvars += 1;
                }
            } else if probingvarisinteger && scip.nodes[crate::tree::tree_get_current_node(scip)].depth == 0 {
                // If we are not in presolving, then we cannot do aggregations
                // but we can use variable bounds to code the same equality.
                let coef = (rightproplbs[j] - leftproplbs[j]) / (rightlb - leftub);
                let constant = (leftproplbs[j] * rightlb - rightproplbs[j] * leftub) / (rightlb - leftub);
                let (c, nboundchanges) = scip_var::add_var_vlb(scip, var, probingvar, coef, constant)?;
                *cutoff = c;
                *nchgbds += nboundchanges;

                if !*cutoff {
                    let (c, nboundchanges) = scip_var::add_var_vub(scip, var, probingvar, coef, constant)?;
                    *cutoff = c;
                    *nchgbds += nboundchanges;
                }
                *nimplications += 1;
            }
            // If probingvar is continuous and we are in solving stage, then
            // we do nothing.
        }
        // Can only add implications on binary variables which are globally
        // valid.
        else if probingvarisbinary && at_root(scip) {
            // Implications can be added only for binary variables; probing
            // should have fixed the variable to 0.0 in the left branch and to
            // 1.0 in the right branch.
            let add = |scip: &mut Scip, varfixing: bool, impltype: BoundType, bound: f64, cutoff: &mut bool, nimplications: &mut i32, nchgbds: &mut i32| -> ScipResult {
                let (c, nboundchanges) = scip_var::add_var_implication(scip, probingvar, varfixing, var, impltype, bound)?;
                *cutoff = c;
                *nimplications += 1;
                *nchgbds += nboundchanges;
                Ok(())
            };

            if scip.set.is_eq(newlb, leftpropubs[j]) && leftimplubs.is_none_or(|a| a[j] > leftpropubs[j]) {
                // var is fixed to lower bound whenever probingvar is fixed to
                // 0.0 and implication is not already known -> insert
                // implication: probingvar == 0 => var <= leftpropubs[j].
                add(scip, false, BoundType::Upper, leftpropubs[j], cutoff, nimplications, nchgbds)?;
            } else if scip.set.is_eq(newub, leftproplbs[j]) && leftimpllbs.is_none_or(|a| a[j] < leftproplbs[j]) {
                // probingvar == 0 => var >= leftproplbs[j].
                add(scip, false, BoundType::Lower, leftproplbs[j], cutoff, nimplications, nchgbds)?;
            }
            // We can do an else here, since the case where var is fixed for
            // both fixings of probingvar had been handled as aggregation.
            else if scip.set.is_eq(newlb, rightpropubs[j]) && rightimplubs.is_none_or(|a| a[j] > rightpropubs[j]) {
                // probingvar == 1 => var <= rightpropubs[j].
                add(scip, true, BoundType::Upper, rightpropubs[j], cutoff, nimplications, nchgbds)?;
            } else if scip.set.is_eq(newub, rightproplbs[j]) && rightimpllbs.is_none_or(|a| a[j] < rightproplbs[j]) {
                // probingvar == 1 => var >= rightproplbs[j].
                add(scip, true, BoundType::Lower, rightproplbs[j], cutoff, nimplications, nchgbds)?;
            } else if scip.vars[var].vartype != VarType::Binary || var_is_implied_integral(scip, var) {
                // Check for implications for lower or upper bounds (only store
                // implications with bounds tightened at least by 0.5).
                if leftpropubs[j] < newub - 0.5 && leftimplubs.is_none_or(|a| leftpropubs[j] < a[j]) {
                    // probingvar == 0 => var <= leftpropubs[j].
                    add(scip, false, BoundType::Upper, leftpropubs[j], cutoff, nimplications, nchgbds)?;
                }
                if leftproplbs[j] > newlb + 0.5 && leftimpllbs.is_none_or(|a| leftproplbs[j] > a[j]) && !*cutoff {
                    // probingvar == 0 => var >= leftproplbs[j].
                    add(scip, false, BoundType::Lower, leftproplbs[j], cutoff, nimplications, nchgbds)?;
                }
                if rightpropubs[j] < newub - 0.5 && rightimplubs.is_none_or(|a| rightpropubs[j] < a[j]) && !*cutoff {
                    // probingvar == 1 => var <= rightpropubs[j].
                    add(scip, true, BoundType::Upper, rightpropubs[j], cutoff, nimplications, nchgbds)?;
                }
                if rightproplbs[j] > newlb + 0.5 && rightimpllbs.is_none_or(|a| rightproplbs[j] > a[j]) && !*cutoff {
                    // probingvar == 1 => var >= rightproplbs[j].
                    add(scip, true, BoundType::Lower, rightproplbs[j], cutoff, nimplications, nchgbds)?;
                }
            }
        }
        j += 1;
    }

    Ok(())
}
