//! Port of `heur_gins.c`: graph induced neighborhood search, an LNS
//! heuristic that keeps a k-neighborhood of a variable in the
//! variable-constraint graph free, fixes the rest to the incumbent, and
//! solves the sub-SCIP, rolling the neighborhood on while it finds better
//! solutions.
//!
//! User decompositions (`SCIPgetDecomps`) are not ported; without them C
//! uses the rolling horizon, as here.

use std::any::Any;

use crate::{
    api,
    copy::CopyMaps,
    heur::{HEURTIMING_AFTERNODE, HeurTiming, VGraph},
    misc::RandNumGen,
    paramset::{ParamSetting, paramset_set_bool, paramset_set_char, paramset_set_int, paramset_set_longint, paramset_set_real},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, ScipResultCode, SolId, Status, VarId},
    var::NONE,
};

const HEUR_NAME: &str = "gins";
const HEUR_DESC: &str = "gins works on k-neighborhood in a variable-constraint graph";
const HEUR_DISPCHAR: u8 = b'L';
const HEUR_PRIORITY: i32 = -1103000;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 8;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERNODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

/// Number of nodes added to the contingent of the total nodes.
const DEFAULT_NODESOFS: i32 = 500;
/// Maximum number of nodes to regard in the subproblem.
const DEFAULT_MAXNODES: i32 = 5000;
/// Factor by which Gins should at least improve the incumbent.
const DEFAULT_MINIMPROVE: f64 = 0.01;
/// Minimum number of nodes to regard in the subproblem.
const DEFAULT_MINNODES: i32 = 50;
/// Minimum percentage of integer variables that have to be fixed.
const DEFAULT_MINFIXINGRATE: f64 = 0.66;
/// Subproblem nodes in relation to nodes of the original problem.
const DEFAULT_NODESQUOT: f64 = 0.15;
/// Number of nodes without incumbent change that heuristic should wait.
const DEFAULT_NWAITINGNODES: i32 = 100;
/// Should subproblem be created out of the rows in the LP rows?
const DEFAULT_USELPROWS: bool = false;
/// If DEFAULT_USELPROWS is FALSE, then should all active cuts from the
/// cutpool of the original scip be copied to constraints of the subscip?
const DEFAULT_COPYCUTS: bool = true;
/// Limit on number of improving incumbent solutions in sub-CIP.
const DEFAULT_BESTSOLLIMIT: i32 = 3;
/// Should continuous variables outside the neighborhoods get fixed?
const DEFAULT_FIXCONTVARS: bool = false;
/// The reference point to compute the neighborhood potential: (r)oot,
/// (l)ocal lp, or (p)seudo solution.
const DEFAULT_POTENTIAL: u8 = b'r';
/// Maximum distance to selected variable to enter the subproblem, or -1 to
/// select the distance that best approximates the minimum fixing rate from
/// below.
const DEFAULT_MAXDISTANCE: i32 = 3;
const DEFAULT_RANDSEED: u32 = 71;
/// Should dense constraints (at least as dense as 1 - minfixingrate) be
/// ignored by connectivity graph?
const DEFAULT_RELAXDENSECONSS: bool = false;
/// Should the heuristic solve a sequence of sub-MIP's around the first
/// selected variable.
const DEFAULT_USEROLLINGHORIZON: bool = true;
/// Limiting percentage for variables already used in sub-SCIPs to terminate
/// rolling horizon approach.
const DEFAULT_ROLLHORIZONLIMFAC: f64 = 0.4;
/// Should user decompositions be considered, if available?
const DEFAULT_USEDECOMP: bool = true;
/// Should user decompositions be considered for initial selection in
/// rolling horizon, if available?
const DEFAULT_USEDECOMPROLLHORIZON: bool = false;
/// Should random initial variable selection be used if decomposition was
/// not successful?
const DEFAULT_USESELFALLBACK: bool = true;
/// Overlap of blocks between runs - 0.0: no overlap, 1.0: shift by only 1
/// block.
const DEFAULT_OVERLAP: f64 = 0.0;
/// Should blocks be treated consecutively (sorted by ascending label?).
const DEFAULT_CONSECUTIVEBLOCKS: bool = true;

/// `ROLLINGHORIZON`: rolling horizon data structure to control multiple LNS
/// heuristic runs away from an original source variable.
struct RollingHorizon {
    /// Variable graph data structure for breadth-first-search
    /// neighborhoods.
    variablegraph: Option<VGraph>,
    /// Distances of the heuristic rolling horizon from the original source
    /// variable indexed by probindex.
    distances: Vec<i32>,
    /// Array that represents for every variable whether it has been used in
    /// a neighborhood indexed by probindex.
    used: Vec<bool>,
    /// The last distance k for a neighborhood, will be decreased during the
    /// rolling horizon if the selected variables are already used.
    lastmaxdistance: i32,
    /// Last distance from originally selected variable in iteration zero.
    lastdistance: i32,
    /// Counter for the number of rolling horizon iterations.
    niterations: i32,
    /// Counts the number variables that have been part of any neighborhood
    /// during the rolling horizon approach.
    nused: i32,
    /// Counter for the number of nonreachable variables (distance -1) from
    /// the initially selected variable.
    nnonreachable: i32,
}

/// `SCIP_HeurData`.
struct HeurData {
    nodesofs: i32,
    maxnodes: i32,
    minnodes: i32,
    minfixingrate: f64,
    overlap: f64,
    nwaitingnodes: i32,
    minimprove: f64,
    usednodes: i64,
    nodesquot: f64,
    rollhorizonlimfac: f64,
    randnumgen: Option<RandNumGen>,
    lastinitsol: SolId,
    uselprows: bool,
    copycuts: bool,
    allblocksunsuitable: bool,
    fixcontvars: bool,
    bestsollimit: i32,
    maxdistance: i32,
    sumneighborhoodvars: i32,
    sumdiscneighborhoodvars: i32,
    nneighborhoods: i32,
    nsubmips: i32,
    consecutiveblocks: bool,
    relaxdenseconss: bool,
    userollinghorizon: bool,
    usedecomp: bool,
    usedecomprollhorizon: bool,
    useselfallback: bool,
    potential: u8,
    maxseendistance: i32,
    nrelaxedconstraints: i32,
    nfailures: i32,
    nextnodenumber: i64,
    targetnodes: i64,
}

/// `SOLVELIMITS`: limits for the sub-SCIP solving process.
#[derive(Default)]
struct SolveLimits {
    nodelimit: i64,
    stallnodelimit: i64,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut().unwrap()
}

/*
 * Local methods
 */

/// `checkFixingrate`: whether enough fixings have been found.
fn check_fixingrate(scip: &Scip, d: &HeurData, nfixings: i32) -> bool {
    let nvars = api::get_n_vars(scip);
    let nbinvars = api::get_n_bin_vars(scip);
    let nintvars = api::get_n_int_vars(scip);
    let fixthreshold = (d.minfixingrate * (if d.fixcontvars { nvars } else { nbinvars + nintvars }) as f64) as i32;

    // Compare actual number of fixings to limit; if we fixed not enough
    // variables we terminate here.
    nfixings >= fixthreshold
}

/// `getPotential`: the potential of a subset of variables (distance to a
/// reference point such as the pseudo-solution or root LP solution).
fn get_potential(scip: &Scip, potentialref: u8, sol: SolId, vars: &[VarId]) -> f64 {
    if vars.is_empty() {
        return 0.0;
    }

    let mut potential = 0.0;
    for &var in vars {
        let varobj = scip.vars[var].obj;

        if scip.set.is_zero(varobj) {
            continue;
        }

        // Determine the reference point for potential computation.
        let referencepoint = match potentialref {
            // Use difference to pseudo solution using the bound in the
            // objective direction.
            b'p' => {
                if varobj > 0.0 {
                    scip.vars[var].glbdom.lb
                } else {
                    scip.vars[var].glbdom.ub
                }
            }
            // Use root LP solution difference.
            b'r' => crate::var::var_get_root_sol(scip, var),
            // Use local LP solution.
            b'l' => api::get_sol_val(scip, NONE, var),
            _ => {
                log::error!("Unknown potential computation {} specified", potentialref as char);
                0.0
            }
        };

        if scip.set.is_infinity(referencepoint.abs()) {
            continue;
        }

        // Calculate the delta to the variables best bound.
        let objdelta = (api::get_sol_val(scip, sol, var) - referencepoint) * varobj;
        potential += objdelta;
    }

    potential
}

/// `rollingHorizonCreate`: creates a rolling horizon data structure.
fn rolling_horizon_create(scip: &Scip) -> RollingHorizon {
    let size = (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip)) as usize;
    RollingHorizon {
        variablegraph: None,
        distances: vec![0; size],
        used: vec![false; size],
        lastmaxdistance: 0,
        lastdistance: -1,
        niterations: 0,
        nused: 0,
        nnonreachable: 0,
    }
}

/// `rollingHorizonRunAgain`: is there potential to run another iteration of
/// the rolling horizon approach?
fn rolling_horizon_run_again(scip: &Scip, rollinghorizon: &RollingHorizon, d: &HeurData) -> bool {
    let maxnused = (d.rollhorizonlimfac * (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip) - rollinghorizon.nnonreachable) as f64) as i32;

    // Run again if a certain percentage of the reachable variables (in the
    // same connected component) was not used in a previous neighborhood.
    rollinghorizon.nused < maxnused
}

/// `rollingHorizonStoreDistances`: stores the distances from the selected
/// variable permanently for the rolling horizon approach.
fn rolling_horizon_store_distances(rollinghorizon: &mut RollingHorizon, distances: &[i32]) {
    let n = rollinghorizon.distances.len();
    rollinghorizon.distances.copy_from_slice(&distances[..n]);
    rollinghorizon.lastdistance = 0;
    rollinghorizon.nnonreachable = 0;

    // Collect number of nonreachable variables.
    for &dist in &distances[..n] {
        if dist == -1 {
            rollinghorizon.nnonreachable += 1;
        }
    }
}

/// `isVariableInNeighborhood`: is the variable in the current neighborhood
/// which is given by the breadth-first distances from a central variable?
fn is_variable_in_neighborhood(scip: &Scip, var: VarId, distances: &[i32], maxdistance: i32) -> bool {
    let p = scip.vars[var].probindex as usize;
    distances[p] != -1 && distances[p] <= maxdistance
}

/// `getFixVal`: the fixing value of a variable.
fn get_fix_val(scip: &Scip, sol: SolId, var: VarId) -> f64 {
    let mut fixval = api::get_sol_val(scip, sol, var);
    let lb = scip.vars[var].glbdom.lb;
    let ub = scip.vars[var].glbdom.ub;

    // Due to dual reductions, it may happen that the solution value is not
    // in the variable's domain anymore.
    if scip.set.is_lt(fixval, lb) {
        fixval = lb;
    } else if scip.set.is_gt(fixval, ub) {
        fixval = ub;
    }

    fixval
}

/// `fixNonNeighborhoodVariables`: fixes variables in subproblem based on
/// long breadth-first distances in variable graph.
#[allow(clippy::too_many_arguments)]
fn fix_non_neighborhood_variables(
    scip: &Scip,
    d: &HeurData,
    mut rollinghorizon: Option<&mut RollingHorizon>,
    sol: SolId,
    vars: &[VarId],
    fixedvars: &mut Vec<VarId>,
    fixedvals: &mut Vec<f64>,
    distances: &[i32],
    maxdistance: i32,
) {
    let nvars = api::get_n_vars(scip);
    let nbinvars = api::get_n_bin_vars(scip);
    let nintvars = api::get_n_int_vars(scip);
    let nvarstofix = if d.fixcontvars { nvars } else { nbinvars + nintvars };

    fixedvars.clear();
    fixedvals.clear();

    // Change bounds of variables of the subproblem.
    for i in 0..nvarstofix as usize {
        // Fix all variables that are too far away from this variable according
        // to maxdistance.
        if distances[i] == -1 || distances[i] > maxdistance {
            let fixval = get_fix_val(scip, sol, vars[i]);

            // Store variable and value of this fixing.
            if !scip.set.is_infinity(fixval.abs()) {
                fixedvars.push(vars[i]);
                fixedvals.push(fixval);
            }
        } else if let Some(rh) = rollinghorizon.as_deref_mut()
            && (i as i32) < nbinvars + nintvars
            && !rh.used[i]
        {
            rh.nused += 1;
            rh.used[i] = true;
        }
    }

    if let Some(rh) = rollinghorizon {
        rh.lastmaxdistance = maxdistance;
        rh.niterations += 1;
    }
}

/// `determineMaxDistance`: determines the maximum allowed distance to stay
/// within the restriction to fix at least minfixingrate many non
/// neighborhood variables.
fn determine_max_distance(scip: &Scip, d: &mut HeurData, distances: &[i32]) -> i32 {
    let nvars = api::get_n_vars(scip);
    let nbinvars = api::get_n_bin_vars(scip);
    let nintvars = api::get_n_int_vars(scip);
    let nrelevantdistances = (if d.fixcontvars { nvars } else { nbinvars + nintvars }) as usize;

    // Copy the relevant distances of either the discrete or all problem
    // variables and sort them.
    let mut distancescopy = distances[..nrelevantdistances].to_vec();
    crate::sort::sort(&mut distancescopy, (), crate::sort::cmp_int);

    // Distances can be infinite in the case of multiple connected components;
    // therefore, search for the index of the zero entry, which is the unique
    // variable with distance 0 to itself.
    let zeropos = match crate::sort::sortedvec_find(&distancescopy, crate::sort::cmp_int, 0) {
        Ok(p) | Err(p) => p,
    };

    // Determine the critical index to look for an appropriate neighborhood
    // distance, starting from the zero position.
    let mut criticalidx = zeropos as i32 + ((1.0 - d.minfixingrate) * nrelevantdistances as f64) as i32;
    criticalidx = if criticalidx < nrelevantdistances as i32 {
        criticalidx
    } else {
        nrelevantdistances as i32 - 1
    };
    let criticalidx = criticalidx as usize;

    // Determine the maximum breadth-first distance such that the neighborhood
    // size stays small enough (below 1-minfixingrate).
    let mut choosevardistance = distancescopy[criticalidx];

    // We set the distance to exactly the distance at the critical index. If
    // the distance at critical index is not the last one, we check if there
    // are many variables with this exact distance. We decrement the distance
    // such that the neighborhood is small enough.
    if criticalidx != nrelevantdistances - 1 && distancescopy[criticalidx + 1] == choosevardistance {
        choosevardistance -= 1;
    }

    // Update the maximum distance.
    let last = distancescopy[nrelevantdistances - 1];
    d.maxseendistance = if d.maxseendistance >= last { d.maxseendistance } else { last };

    choosevardistance
}

/// `heurdataAvgDiscreteNeighborhoodSize`: the average size of a discrete
/// neighborhood over all variables tested.
fn heurdata_avg_discrete_neighborhood_size(d: &HeurData) -> f64 {
    d.sumdiscneighborhoodvars as f64 / crate::def::max(1.0, d.nneighborhoods as f64)
}

/// `selectInitialVariableRandomly`: chooses a variable at random among
/// disjoint neighborhoods of best potential; returns `(selvar,
/// selvarmaxdistance)`.
fn select_initial_variable_randomly(scip: &mut Scip, heur: HeurId, vargraph: &mut VGraph, distances: &mut [i32]) -> ScipResult<(VarId, i32)> {
    let sol = api::get_best_sol(scip);

    // Get required data of the original problem.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;

    // Copy SCIP variables.
    let mut varscopy = vars[..nbinvars + nintvars].to_vec();
    let mut nsearched = 0;
    let mut maxpotential = crate::def::REAL_MIN;
    let mut selvar = NONE;
    let mut selvarmaxdistance = 0;

    let d = hd(scip, heur);

    // Determine upper bound on neighborhood size.
    let nintegralvarsbound = ((1.0 - d.minfixingrate) * (nbinvars + nintvars) as f64) as i32;

    // Maximum distance from selected variable for breadth-first search (if
    // set to -1, we compute an exhaustive breadth-first search and sort the
    // variables by their distance).
    let maxdistance = if d.maxdistance == -1 { i32::MAX } else { d.maxdistance };

    let mut nintegralvarsleft = nbinvars + nintvars;

    // Sort inactive variables to the end of the array.
    for v in (0..nintegralvarsleft).rev() {
        if !crate::var::var_is_active(scip, varscopy[v]) {
            varscopy[v] = varscopy[nintegralvarsleft - 1];
            nintegralvarsleft -= 1;
        }
    }

    // Adjust the search limit.
    let mut searchlimit = if d.nneighborhoods < 10 {
        5
    } else {
        (nintegralvarsleft as f64 / heurdata_avg_discrete_neighborhood_size(d)) as i32
    };
    searchlimit = if searchlimit <= 10 { searchlimit } else { 10 };

    // Multi variable potential: choose different disjoint neighborhoods,
    // compare their potential.
    while nsearched < searchlimit && nintegralvarsleft > 0 {
        nsearched += 1;

        // Select a variable to start with randomly, but make sure it is
        // active.
        let mut choosevar;
        loop {
            let idx = hdm(scip, heur).randnumgen.as_mut().unwrap().get_int(0, nintegralvarsleft as i32 - 1) as usize;
            choosevar = varscopy[idx];

            // Sort inactive variables to the end.
            if scip.vars[choosevar].probindex < 0 {
                varscopy[idx] = varscopy[nintegralvarsleft - 1];
                nintegralvarsleft -= 1;
            }

            if !(scip.vars[choosevar].probindex < 0 && nintegralvarsleft > 0) {
                break;
            }
        }

        // If there was no variable chosen, there are no active variables
        // left.
        if scip.vars[choosevar].probindex < 0 {
            break;
        }

        // Determine breadth-first distances from the chosen variable.
        VGraph::breadth_first(scip, Some(&mut *vargraph), &[choosevar], distances, maxdistance, i32::MAX, i32::MAX)?;

        // Use either automatic or user-defined max-distance for neighborhood
        // in variable constraint graph.
        let choosevardistance = if hd(scip, heur).maxdistance != -1 {
            hd(scip, heur).maxdistance
        } else {
            determine_max_distance_heur(scip, heur, distances)
        };

        let mut ndiscvarsneighborhood = 0;
        let mut neighborhood = Vec::with_capacity(nvars);

        // Loop over variables and determine neighborhood.
        for &currvar in vars.iter().rev() {
            // Put variable in the neighborhood.
            if is_variable_in_neighborhood(scip, currvar, distances, choosevardistance) {
                neighborhood.push(currvar);

                // Increase discrete variables counter.
                if (scip.vars[currvar].vartype as i32) <= crate::types::VarType::Integer as i32 {
                    ndiscvarsneighborhood += 1;
                }
            }
        }

        // Check if neighborhood contains too many integer variables in order
        // to satisfy the minimum fixing rate.
        if ndiscvarsneighborhood >= nintegralvarsbound || ndiscvarsneighborhood <= 1 {
            // Too many or too few discrete variables in this neighborhood.
        } else {
            // Compare the neighborhood potential to the best potential found
            // so far.
            let potential = get_potential(scip, hd(scip, heur).potential, sol, &neighborhood);

            // Big potential, take this variable.
            if potential > maxpotential {
                maxpotential = potential;
                selvar = choosevar;
                selvarmaxdistance = choosevardistance;
            }
        }

        // Sort neighborhood variables to the end so that this neighborhood is
        // not considered in further samples (C tests the problem variable at
        // each position, not the copy there).
        for v in (0..nintegralvarsleft).rev() {
            let currvar = vars[v];
            if is_variable_in_neighborhood(scip, currvar, distances, choosevardistance) {
                varscopy[v] = varscopy[nintegralvarsleft - 1];
                nintegralvarsleft -= 1;
            }
        }

        let d = hdm(scip, heur);
        d.sumdiscneighborhoodvars += ndiscvarsneighborhood;
        d.sumneighborhoodvars += neighborhood.len() as i32;
        d.nneighborhoods += 1;
    }

    // Maybe no variable has a positive delta.
    if !scip.set.is_positive(maxpotential) || selvar == NONE {
        selvar = NONE;
    }

    Ok((selvar, selvarmaxdistance))
}

/// A heuristic data value to swap in while the real one is borrowed out.
fn placeholder_data() -> HeurData {
    HeurData {
        nodesofs: 0,
        maxnodes: 0,
        minnodes: 0,
        minfixingrate: 0.0,
        overlap: 0.0,
        nwaitingnodes: 0,
        minimprove: 0.0,
        usednodes: 0,
        nodesquot: 0.0,
        rollhorizonlimfac: 0.0,
        randnumgen: None,
        lastinitsol: NONE,
        uselprows: false,
        copycuts: false,
        allblocksunsuitable: false,
        fixcontvars: false,
        bestsollimit: 0,
        maxdistance: 0,
        sumneighborhoodvars: 0,
        sumdiscneighborhoodvars: 0,
        nneighborhoods: 0,
        nsubmips: 0,
        consecutiveblocks: false,
        relaxdenseconss: false,
        userollinghorizon: false,
        usedecomp: false,
        usedecomprollhorizon: false,
        useselfallback: false,
        potential: 0,
        maxseendistance: 0,
        nrelaxedconstraints: 0,
        nfailures: 0,
        nextnodenumber: 0,
        targetnodes: 0,
    }
}

/// [`determine_max_distance`] on the heuristic's data.
fn determine_max_distance_heur(scip: &mut Scip, heur: HeurId, distances: &[i32]) -> i32 {
    let mut d = std::mem::replace(hdm(scip, heur), placeholder_data());
    let r = determine_max_distance(scip, &mut d, distances);
    *hdm(scip, heur) = d;
    r
}

/// `selectNextVariable`: selects the next variable using the rolling
/// horizon; returns `(selvar, selvarmaxdistance)`.
fn select_next_variable(scip: &mut Scip, heur: HeurId, rollinghorizon: &mut RollingHorizon, distances: &mut [i32]) -> ScipResult<(VarId, i32)> {
    let vars = api::get_vars(scip).to_vec();
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;

    let mut selvar;
    let mut selvarmaxdistance;
    let mut minunuseddistance;

    // Loop over the variables that are left and pick the variable with
    //  - the smallest, always nondecreasing distance
    //  - that was not used before in a neighborhood.
    loop {
        minunuseddistance = i32::MAX;
        selvarmaxdistance = rollinghorizon.lastmaxdistance;
        selvar = NONE;

        let mut i = 0;
        while i < nbinvars + nintvars && minunuseddistance > rollinghorizon.lastdistance {
            if rollinghorizon.distances[i] >= rollinghorizon.lastdistance && rollinghorizon.distances[i] < minunuseddistance && !rollinghorizon.used[i] {
                minunuseddistance = rollinghorizon.distances[i];
                selvar = vars[i];
            }
            i += 1;
        }

        // If no variable could be selected, we can stop.
        if selvar == NONE {
            return Ok((NONE, 0));
        }

        // Determine the distances to other variables from this variable.
        VGraph::breadth_first(scip, rollinghorizon.variablegraph.as_mut(), &[selvar], distances, selvarmaxdistance, i32::MAX, i32::MAX)?;

        selvarmaxdistance = determine_max_distance_heur(scip, heur, distances);

        // Mark this variable as used in order to not find it again.
        if selvarmaxdistance == 0 {
            rollinghorizon.used[scip.vars[selvar].probindex as usize] = true;
            rollinghorizon.nused += 1;
            selvar = NONE;
        }

        if !(rolling_horizon_run_again(scip, rollinghorizon, hd(scip, heur)) && (selvar == NONE || selvarmaxdistance == 0)) {
            break;
        }
    }

    // Breadth-first search determines the distances of all variables that
    // are no more than maxdistance away from the start variable.
    selvarmaxdistance = if selvarmaxdistance <= rollinghorizon.lastmaxdistance {
        selvarmaxdistance
    } else {
        rollinghorizon.lastmaxdistance
    };
    rollinghorizon.lastdistance = minunuseddistance;
    rollinghorizon.lastmaxdistance = selvarmaxdistance;

    Ok((selvar, selvarmaxdistance))
}

/// `chooseDecomp`: the last nontrivial user decomposition. Decompositions
/// are not ported, so there is none.
fn choose_decomp(_scip: &Scip) -> bool {
    false
}

/// `determineVariableFixings`: determines the graph-induced variable
/// fixings; returns success.
fn determine_variable_fixings(
    scip: &mut Scip,
    heur: HeurId,
    fixedvars: &mut Vec<VarId>,
    fixedvals: &mut Vec<f64>,
    mut rollinghorizon: Option<&mut RollingHorizon>,
) -> ScipResult<bool> {
    fixedvars.clear();
    fixedvals.clear();

    let sol = api::get_best_sol(scip);

    // Get required data of the source problem.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    // Get the saved variable graph, or create a new one.
    let (relaxdenseconss, minfixingrate) = (hd(scip, heur).relaxdenseconss, hd(scip, heur).minfixingrate);
    let mut localgraph = None;
    match rollinghorizon.as_deref_mut() {
        Some(rh) => {
            if rh.niterations == 0 {
                // Create variable graph.
                let (g, nrelaxed) = VGraph::create(scip, relaxdenseconss, 1.0 - minfixingrate)?;
                rh.variablegraph = g;
                hdm(scip, heur).nrelaxedconstraints = nrelaxed;
            }
        }
        None => {
            // Create variable graph.
            let (g, nrelaxed) = VGraph::create(scip, relaxdenseconss, 1.0 - minfixingrate)?;
            localgraph = g;
            hdm(scip, heur).nrelaxedconstraints = nrelaxed;
        }
    }

    // Allocate buffer memory to hold distances.
    let mut distances = vec![0i32; nvars];

    let (selvar, selvarmaxdistance);

    // In the first iteration of the rolling horizon approach, we select an
    // initial variable.
    if rollinghorizon.as_ref().is_none_or(|rh| rh.niterations == 0) {
        let userandomselection = true;

        // Choose the initial variable based on a user decomposition, if
        // available.
        if hd(scip, heur).usedecomprollhorizon && choose_decomp(scip) {
            unreachable!("user decompositions are not ported");
        }

        // Use random variable selection as fallback strategy, if no user
        // decomposition is available, or the heuristic should not use
        // decomposition.
        let vargraph = match rollinghorizon.as_deref_mut() {
            Some(rh) => rh.variablegraph.as_mut().unwrap(),
            None => localgraph.as_mut().unwrap(),
        };
        (selvar, selvarmaxdistance) = if userandomselection {
            select_initial_variable_randomly(scip, heur, vargraph, &mut distances)?
        } else {
            (NONE, 0)
        };

        // Collect and save the distances in the rolling horizon data
        // structure.
        if selvar != NONE
            && let Some(rh) = rollinghorizon.as_deref_mut()
        {
            // Collect distances in the variable graph of all variables to the
            // selected variable.
            VGraph::breadth_first(scip, rh.variablegraph.as_mut(), &[selvar], &mut distances, i32::MAX, i32::MAX, i32::MAX)?;
            rolling_horizon_store_distances(rh, &distances);
            rh.lastmaxdistance = selvarmaxdistance;
        }
    } else {
        // Select the next variable, if variables are left.
        (selvar, selvarmaxdistance) = select_next_variable(scip, heur, rollinghorizon.as_deref_mut().unwrap(), &mut distances)?;
    }

    let success = if selvar == NONE {
        false
    } else {
        // Fix variables that are not in the neighborhood around the selected
        // variable.
        let d = hd(scip, heur);
        fix_non_neighborhood_variables(scip, d, rollinghorizon, sol, &vars, fixedvars, fixedvals, &distances, selvarmaxdistance);
        check_fixingrate(scip, hd(scip, heur), fixedvars.len() as i32)
    };

    Ok(success)
}

/// `setLimits`: sets sub-SCIP solving limits.
fn set_limits(sourcescip: &Scip, subscip: &mut Scip, solvelimits: &SolveLimits) -> ScipResult {
    crate::copy::copy_limits(sourcescip, subscip)?;
    paramset_set_longint(subscip, "limits/nodes", solvelimits.nodelimit)?;
    paramset_set_longint(subscip, "limits/stallnodes", solvelimits.stallnodelimit)?;

    // Safe long running times caused by lack of dual convergence.
    paramset_set_real(subscip, "limits/gap", 0.01)?;
    Ok(())
}

/// `setupSubScip`: sets up the sub-SCIP.
fn setup_sub_scip(scip: &Scip, subscip: &mut Scip, solvelimits: &SolveLimits, heur: HeurId) -> ScipResult {
    let d = hd(scip, heur);

    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(subscip, "misc/catchctrlc", false)?;

    // Disable output to console.
    paramset_set_int(subscip, "display/verblevel", 0)?;

    // Disable statistic timing inside sub SCIP.
    paramset_set_bool(subscip, "timing/statistictiming", false)?;

    paramset_set_int(subscip, "limits/bestsol", d.bestsollimit)?;

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

    // Add an objective cutoff.
    let upperbound = api::get_upperbound(scip) - scip.set.sumepsilon();
    let cutoff = if !scip.set.is_infinity(-api::get_lowerbound(scip)) {
        (1.0 - d.minimprove) * api::get_upperbound(scip) + d.minimprove * api::get_lowerbound(scip)
    } else if api::get_upperbound(scip) >= 0.0 {
        (1.0 - d.minimprove) * api::get_upperbound(scip)
    } else {
        (1.0 + d.minimprove) * api::get_upperbound(scip)
    };
    let cutoff = crate::def::min(upperbound, cutoff);
    api::set_objlimit(subscip, cutoff)?;

    // Set solve limits for sub-SCIP.
    set_limits(scip, subscip, solvelimits)
}

/// `determineLimits`: determines limits for a sub-SCIP; returns whether
/// another sub-SCIP can be solved with them.
fn determine_limits(scip: &Scip, heur: HeurId, solvelimits: &mut SolveLimits, mut runagain: bool) -> ScipResult<bool> {
    let d = hd(scip, heur);
    let h = &scip.heurs[heur];

    // Check whether there is enough time and memory left.
    if !crate::copy::check_copy_limits(scip)? {
        runagain = false;
    }

    // Calculate the maximal number of branching nodes until heuristic is
    // aborted.
    let mut maxnnodesr = d.nodesquot * api::get_n_nodes(scip) as f64;

    // Reward gins if it succeeded often, count the setup costs for the
    // sub-MIP as 100 nodes.
    let mut confidence = h.ncalls as f64;
    confidence /= confidence + 5.0;
    maxnnodesr *= 1.0 + confidence * 2.0 * (h.nbestsolsfound as f64 + 1.0) / (d.nsubmips as f64 + 1.0);
    let mut maxnnodes = (maxnnodesr - 100.0 * d.nsubmips as f64) as i64;
    maxnnodes += d.nodesofs as i64;

    // Determine the node limit for the current process.
    solvelimits.nodelimit = maxnnodes - d.usednodes;
    solvelimits.nodelimit = if solvelimits.nodelimit <= d.maxnodes as i64 {
        solvelimits.nodelimit
    } else {
        d.maxnodes as i64
    };

    // Check whether we have enough nodes left to call subproblem solving.
    if solvelimits.nodelimit < d.targetnodes {
        runagain = false;
    }

    solvelimits.stallnodelimit = d.targetnodes;

    Ok(runagain)
}

/// `updateFailureStatistic`: updates heurdata after a run of GINS.
fn update_failure_statistic(scip: &mut Scip, heur: HeurId) {
    let nnodes = api::get_n_nodes(scip);
    let d = hdm(scip, heur);

    // Increase number of failures, calculate next node at which GINS should
    // be called and update actual solutions.
    d.nfailures += 1;
    d.nextnodenumber = if d.nfailures <= 25 { nnodes + 100 * (2i64 << d.nfailures) } else { i64::MAX };
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyGins`: copy method for primal heuristic plugins (called when
/// SCIP copies plugins).
fn heur_copy_gins(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_gins(scip)
}

/// `heurInitGins`: initialization method of primal heuristic (called after
/// problem was transformed).
fn heur_init_gins(scip: &mut Scip, heur: HeurId) -> ScipResult {
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);

    // Initialize data.
    let d = hdm(scip, heur);
    d.usednodes = 0;
    d.randnumgen = Some(RandNumGen::new(seed));
    d.sumdiscneighborhoodvars = 0;
    d.sumneighborhoodvars = 0;
    d.nneighborhoods = 0;
    d.maxseendistance = 0;
    d.nsubmips = 0;
    d.nfailures = 0;
    d.nextnodenumber = 0;
    d.lastinitsol = NONE;
    d.allblocksunsuitable = false;
    d.targetnodes = d.minnodes as i64;
    Ok(())
}

/// `heurExitGins`: deinitialization method of primal heuristic.
fn heur_exit_gins(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free some data structures that must be reset for a new problem.
    hdm(scip, heur).randnumgen = None;
    Ok(())
}

/// `heurExecGins`: execution method of primal heuristic.
fn heur_exec_gins(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::Delayed;

    // Only call heuristic if feasible solution is available.
    if api::get_n_sols(scip) == 0 {
        return Ok(());
    }

    // In case of many unsuccessful calls, the nextnodenumber is increased to
    // prevent us from running too often.
    if api::get_n_nodes(scip) < hd(scip, heur).nextnodenumber {
        return Ok(());
    }

    // Get best solution.
    let mut newincumbent = api::get_best_sol(scip);

    // Only call heuristic if enough nodes were processed since last
    // incumbent.
    if api::get_n_nodes(scip) - scip.sols[newincumbent].nodenum < hd(scip, heur).nwaitingnodes as i64 {
        return Ok(());
    }

    // Only call heuristic if the best solution comes from transformed
    // problem.
    if crate::sol::sol_is_original(scip, newincumbent) {
        return Ok(());
    }

    *result = ScipResultCode::DidNotRun;

    // Only call heuristic if discrete variables are present.
    if api::get_n_bin_vars(scip) == 0 && api::get_n_int_vars(scip) == 0 {
        return Ok(());
    }

    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Determine solving limits for the sub-SCIP for the first time.
    let mut solvelimits = SolveLimits::default();
    let mut runagain = determine_limits(scip, heur, &mut solvelimits, true)?;

    if !runagain {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    let mut fixedvars = Vec::with_capacity(nvars);
    let mut fixedvals = Vec::with_capacity(nvars);

    if choose_decomp(scip) && hd(scip, heur).usedecomp {
        unreachable!("user decompositions are not ported");
    }

    // Only create a rolling horizon data structure if we need it.
    let mut rollinghorizon = if hd(scip, heur).userollinghorizon { Some(rolling_horizon_create(scip)) } else { None };

    loop {
        // Create a new problem, by fixing all variables except for a small
        // neighborhood.
        let success = determine_variable_fixings(scip, heur, &mut fixedvars, &mut fixedvals, rollinghorizon.as_mut())?;

        // Terminate if it was not possible to create the subproblem.
        if !success {
            // Do not count this as a call to the heuristic.
            *result = ScipResultCode::DidNotRun;

            // Count this as a failure and increase the number of waiting nodes
            // until the next call.
            update_failure_statistic(scip, heur);
            return Ok(());
        }

        // Initializing the subproblem.
        let mut subscip = Scip::create()?;
        hdm(scip, heur).nsubmips += 1;

        // Create the variable mapping hash map.
        let mut maps = CopyMaps::new(nvars as i32, api::get_n_conss(scip));

        // Create a problem copy as sub SCIP.
        let (uselprows, copycuts) = (hd(scip, heur).uselprows, hd(scip, heur).copycuts);
        crate::copy::copy_large_neighborhood_search(scip, &mut subscip, &mut maps, "gins", &fixedvars, &fixedvals, uselprows, copycuts)?;

        let subvars: Vec<VarId> = vars.iter().map(|&v| maps.var_image(v)).collect();

        // Set up limits for the subproblem.
        setup_sub_scip(scip, &mut subscip, &solvelimits, heur)?;

        // Solve the subproblem.
        crate::scip_solve::solve(&mut subscip)?;

        // Increase target nodes if a (stall) node limit was reached; this will
        // immediately affect the next run.
        let status = api::get_status(&subscip);
        if status == Status::NodeLimit || status == Status::StallNodeLimit {
            let d = hdm(scip, heur);
            d.targetnodes = (1.05 * d.targetnodes as f64) as i64 + 5;

            // But not too far.
            let half = d.maxnodes as i64 / 2;
            d.targetnodes = if d.targetnodes <= half { d.targetnodes } else { half };
        }

        // Transfer variable statistics from sub-SCIP.
        crate::copy::merge_variable_statistics(&subscip, scip, &subvars, &vars)?;

        hdm(scip, heur).usednodes += api::get_n_nodes(&subscip);

        // Check, whether a solution was found; due to numerics, it might
        // happen that not all solutions are feasible -> try all solutions
        // until one was accepted.
        let (success, _) = crate::copy::translate_sub_sols(scip, &subscip, heur, &subvars)?;
        if success {
            *result = ScipResultCode::FoundSol;
        }

        let oldincumbent = newincumbent;
        newincumbent = api::get_best_sol(scip);

        // Check if we want to run another rolling horizon iteration.
        runagain = success && newincumbent != oldincumbent && hd(scip, heur).userollinghorizon;
        if runagain {
            runagain = determine_limits(scip, heur, &mut solvelimits, runagain)?;
            if let Some(rh) = rollinghorizon.as_ref() {
                runagain = runagain && rolling_horizon_run_again(scip, rh, hd(scip, heur)) && success;
            }
        }

        if !runagain {
            break;
        }
    }

    // Delay the heuristic in case it was not successful.
    if *result != ScipResultCode::FoundSol {
        update_failure_statistic(scip, heur);
    }

    Ok(())
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurGins`: creates the gins primal heuristic and includes it
/// in SCIP.
pub fn include_heur_gins(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_real};

    // Include primal heuristic.
    let mut data = placeholder_data();
    data.nodesofs = DEFAULT_NODESOFS;
    data.maxnodes = DEFAULT_MAXNODES;
    data.minnodes = DEFAULT_MINNODES;
    data.minfixingrate = DEFAULT_MINFIXINGRATE;
    data.overlap = DEFAULT_OVERLAP;
    data.nwaitingnodes = DEFAULT_NWAITINGNODES;
    data.minimprove = DEFAULT_MINIMPROVE;
    data.nodesquot = DEFAULT_NODESQUOT;
    data.rollhorizonlimfac = DEFAULT_ROLLHORIZONLIMFAC;
    data.uselprows = DEFAULT_USELPROWS;
    data.copycuts = DEFAULT_COPYCUTS;
    data.fixcontvars = DEFAULT_FIXCONTVARS;
    data.bestsollimit = DEFAULT_BESTSOLLIMIT;
    data.maxdistance = DEFAULT_MAXDISTANCE;
    data.consecutiveblocks = DEFAULT_CONSECUTIVEBLOCKS;
    data.relaxdenseconss = DEFAULT_RELAXDENSECONSS;
    data.userollinghorizon = DEFAULT_USEROLLINGHORIZON;
    data.usedecomp = DEFAULT_USEDECOMP;
    data.usedecomprollhorizon = DEFAULT_USEDECOMPROLLHORIZON;
    data.useselfallback = DEFAULT_USESELFALLBACK;
    data.potential = DEFAULT_POTENTIAL;
    let heur = api::include_heur_basic(
        scip,
        HEUR_NAME,
        HEUR_DESC,
        HEUR_DISPCHAR,
        HEUR_PRIORITY,
        HEUR_FREQ,
        HEUR_FREQOFS,
        HEUR_MAXDEPTH,
        HEUR_TIMING,
        HEUR_USESSUBSCIP,
        heur_exec_gins,
        Some(Box::new(data) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_gins);
    h.heurinit = Some(heur_init_gins);
    h.heurexit = Some(heur_exit_gins);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add gins primal heuristic parameters.
    paramset_add_int(
        scip,
        "heuristics/gins/nodesofs",
        "number of nodes added to the contingent of the total nodes",
        t!(i32, nodesofs),
        false,
        DEFAULT_NODESOFS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/gins/maxnodes",
        "maximum number of nodes to regard in the subproblem",
        t!(i32, maxnodes),
        true,
        DEFAULT_MAXNODES,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/gins/minnodes",
        "minimum number of nodes required to start the subproblem",
        t!(i32, minnodes),
        true,
        DEFAULT_MINNODES,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/gins/nwaitingnodes",
        "number of nodes without incumbent change that heuristic should wait",
        t!(i32, nwaitingnodes),
        true,
        DEFAULT_NWAITINGNODES,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/gins/nodesquot",
        "contingent of sub problem nodes in relation to the number of nodes of the original problem",
        t!(f64, nodesquot),
        false,
        DEFAULT_NODESQUOT,
        0.0,
        1.0,
        None,
    )?;
    let sumepsilon = scip.set.sumepsilon();
    paramset_add_real(
        scip,
        "heuristics/gins/minfixingrate",
        "percentage of integer variables that have to be fixed",
        t!(f64, minfixingrate),
        false,
        DEFAULT_MINFIXINGRATE,
        sumepsilon,
        1.0 - sumepsilon,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/gins/minimprove",
        "factor by which gins should at least improve the incumbent",
        t!(f64, minimprove),
        true,
        DEFAULT_MINIMPROVE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/uselprows",
        "should subproblem be created out of the rows in the LP rows?",
        t!(bool, uselprows),
        true,
        DEFAULT_USELPROWS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/copycuts",
        "if uselprows == FALSE, should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/fixcontvars",
        "should continuous variables outside the neighborhoods be fixed?",
        t!(bool, fixcontvars),
        true,
        DEFAULT_FIXCONTVARS,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/gins/bestsollimit",
        "limit on number of improving incumbent solutions in sub-CIP",
        t!(i32, bestsollimit),
        false,
        DEFAULT_BESTSOLLIMIT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/gins/maxdistance",
        "maximum distance to selected variable to enter the subproblem, or -1 to select the distance that best approximates the minimum fixing rate from below",
        t!(i32, maxdistance),
        false,
        DEFAULT_MAXDISTANCE,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/gins/potential",
        "the reference point to compute the neighborhood potential: (r)oot, (l)ocal lp, or (p)seudo solution",
        t!(u8, potential),
        true,
        DEFAULT_POTENTIAL,
        Some("lpr"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/userollinghorizon",
        "should the heuristic solve a sequence of sub-MIP's around the first selected variable",
        t!(bool, userollinghorizon),
        true,
        DEFAULT_USEROLLINGHORIZON,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/relaxdenseconss",
        "should dense constraints (at least as dense as 1 - minfixingrate) be ignored by connectivity graph?",
        t!(bool, relaxdenseconss),
        true,
        DEFAULT_RELAXDENSECONSS,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/gins/rollhorizonlimfac",
        "limiting percentage for variables already used in sub-SCIPs to terminate rolling horizon approach",
        t!(f64, rollhorizonlimfac),
        true,
        DEFAULT_ROLLHORIZONLIMFAC,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/gins/overlap",
        "overlap of blocks between runs - 0.0: no overlap, 1.0: shift by only 1 block",
        t!(f64, overlap),
        true,
        DEFAULT_OVERLAP,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/usedecomp",
        "should user decompositions be considered, if available?",
        t!(bool, usedecomp),
        true,
        DEFAULT_USEDECOMP,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/usedecomprollhorizon",
        "should user decompositions be considered for initial selection in rolling horizon, if available?",
        t!(bool, usedecomprollhorizon),
        true,
        DEFAULT_USEDECOMPROLLHORIZON,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/useselfallback",
        "should random initial variable selection be used if decomposition was not successful?",
        t!(bool, useselfallback),
        true,
        DEFAULT_USESELFALLBACK,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/gins/consecutiveblocks",
        "should blocks be treated consecutively (sorted by ascending label?)",
        t!(bool, consecutiveblocks),
        true,
        DEFAULT_CONSECUTIVEBLOCKS,
        None,
    )?;

    Ok(())
}
