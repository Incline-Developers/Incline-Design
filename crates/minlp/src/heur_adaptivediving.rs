//! Port of `heur_adaptivediving.c`: diving heuristic that selects
//! adaptively between the existing, public divesets.

use std::any::Any;

use crate::{
    api,
    heur::{DiveContext, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    misc::RandNumGen,
    prob::ORIGPROB,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{HeurId, LpSolStat, ScipResultCode},
    var::NONE,
};

const HEUR_NAME: &str = "adaptivediving";
const HEUR_DESC: &str = "diving heuristic that selects adaptively between the existing, public divesets";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -70000;
const HEUR_FREQ: i32 = 5;
const HEUR_FREQOFS: i32 = 3;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

const DEFAULT_INITIALSEED: u32 = 13;

/*
 * Default parameter settings
 */

const DEFAULT_SELTYPE: u8 = b'w';
/// Score parameter for selection: minimize either average 'n'odes, LP
/// 'i'terations, backtrack/'c'onflict ratio, 'd'epth, 1 / 's'olutions, or
/// 1 / solutions'u'ccess.
const DEFAULT_SCORETYPE: u8 = b'c';
const DEFAULT_USEADAPTIVECONTEXT: bool = false;
/// Coefficient c to decrease initial confidence (calls + 1.0) / (calls + c)
/// in scores.
const DEFAULT_SELCONFIDENCECOEFF: f64 = 10.0;
/// Parameter that increases probability of exploration among divesets
/// (only active if seltype is 'e').
const DEFAULT_EPSILON: f64 = 1.0;
/// Maximal fraction of diving LP iterations compared to node LP iterations.
const DEFAULT_MAXLPITERQUOT: f64 = 0.15;
/// Additional number of allowed LP iterations.
const DEFAULT_MAXLPITEROFS: i64 = 1500;
/// Weight of incumbent solutions compared to other solutions in computation
/// of LP iteration limit.
const DEFAULT_BESTSOLWEIGHT: f64 = 10.0;

/// Locally defined heuristic data.
struct HeurData {
    /* data structures used internally */
    /// Working solution.
    sol: crate::types::SolId,
    /// Random number generator for selection.
    randnumgen: RandNumGen,
    /// Publicly available divesets from diving heuristics (None until they
    /// are collected).
    divesets: Option<Vec<DivesetId>>,
    /// Stores the last selected diveset when the heuristics was run.
    lastselection: i32,

    /* user parameters */
    /// Parameter that increases probability of exploration among divesets
    /// (only active if seltype is 'e').
    epsilon: f64,
    /// Coefficient c to decrease initial confidence (calls + 1.0) /
    /// (calls + c) in scores.
    selconfidencecoeff: f64,
    /// Maximal fraction of diving LP iterations compared to node LP
    /// iterations.
    maxlpiterquot: f64,
    /// Additional number of allowed LP iterations.
    maxlpiterofs: i64,
    /// Weight of incumbent solutions compared to other solutions in
    /// computation of LP iteration limit.
    bestsolweight: f64,
    /// Selection strategy: (e)psilon-greedy, (w)eighted distribution,
    /// (n)ext diving.
    seltype: u8,
    /// Score parameter for selection: minimize either average 'n'odes, LP
    /// 'i'terations, backtrack/'c'onflict ratio, 'd'epth, 1 / 's'olutions,
    /// or 1 / solutions'u'ccess.
    scoretype: u8,
    /// Should the heuristic use its own statistics, or shared statistics?
    useadaptivecontext: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

/*
 * local methods
 */

/// `divesetGetSelectionScore`: get the selection score for this dive set.
fn diveset_get_selection_score(scip: &Scip, diveset: DivesetId, heurdata: &HeurData, divecontext: DiveContext) -> ScipResult<f64> {
    let s = &scip.divesets[diveset].divesetstats[divecontext as usize];
    let ncalls = s.ncalls as f64;

    // Compute confidence scalar (converges towards 1 with increasing number
    // of calls).
    let confidence = (ncalls + 1.0) / (ncalls + heurdata.selconfidencecoeff);

    let score = match heurdata.scoretype {
        // Min average nodes.
        b'n' => confidence * s.totalnnodes as f64 / (ncalls + 1.0),
        // Min avg LP iterations.
        b'i' => confidence * s.nlpiterations as f64 / (ncalls + 1.0),
        // Min backtrack / conflict ratio (the current default).
        b'c' => confidence * (s.totalnbacktracks as f64) / (s.nconflictsfound as f64 + 10.0),
        // Minimum average depth.
        b'd' => crate::heur::diveset_get_avg_depth(scip, diveset, divecontext) * confidence,
        // Maximum number of solutions.
        b's' => confidence / (crate::heur::diveset_get_n_sols(scip, diveset, divecontext) as f64 + 1.0),
        // Maximum solution success (which weighs best solutions higher).
        b'u' => confidence / (crate::heur::diveset_get_sol_success(scip, diveset, divecontext) as f64 + 1.0),
        c => {
            log::error!("Unsupported scoring parameter '{}'", c as char);
            return Err(Retcode::ParameterWrongVal);
        }
    };

    Ok(score)
}

/*
 * Callback methods
 */

/// `heurCopyAdaptivediving`: copy method for primal heuristic plugins.
fn heur_copy_adaptivediving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_adaptivediving(scip)
}

/// `findAndStoreDivesets`: find publicly available divesets and store them.
fn find_and_store_divesets(scip: &mut Scip, heur: HeurId) {
    let mut divesets = Vec::new();

    for h in 0..scip.set.nheurs as usize {
        let hh = scip.set.heurs[h];

        // Loop over divesets of this heuristic and check whether they are
        // public.
        for &diveset in &scip.heurs[hh].divesets {
            if scip.divesets[diveset].ispublic {
                divesets.push(diveset);
            }
        }
    }

    hdm(scip, heur).divesets = Some(divesets);
}

/// `heurInitAdaptivediving`: initialization method of primal heuristic.
fn heur_init_adaptivediving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Get and reset heuristic data; we clear the list of collected divesets
    // to ensure reproducability and consistent state across multiple runs
    // within the same SCIP data structure.
    {
        let h = hdm(scip, heur);
        h.lastselection = -1;
        h.divesets = None;
    }

    // Create working solution.
    let sol = api::create_sol(scip, heur)?;

    // Initialize random seed; use problem dimensions to vary initial order
    // between different instances.
    let seed = DEFAULT_INITIALSEED
        .wrapping_add(scip.probs[ORIGPROB].nvars as u32)
        .wrapping_add(scip.probs[ORIGPROB].nconss as u32);
    let modifiedseed = scip.set.initialize_random_seed(seed);
    let h = hdm(scip, heur);
    h.sol = sol;
    h.randnumgen.set_seed(modifiedseed);

    Ok(())
}

/// `heurExitAdaptivediving`: deinitialization method of primal heuristic.
fn heur_exit_adaptivediving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `getLPIterlimit`: get LP iteration limit for diving.
fn get_lp_iterlimit(scip: &Scip, heur: HeurId) -> i64 {
    let h = hd(scip, heur);
    let nsolsfound = scip.heurs[heur].nsolsfound as f64 + h.bestsolweight * scip.heurs[heur].nbestsolsfound as f64;
    let nlpiterations = api::get_n_node_lp_iterations(scip);
    let ncalls = scip.heurs[heur].ncalls;
    let mut nlpiterationsdive: i64 = 0;

    // Loop over the divesets and collect their individual iterations.
    for &diveset in h.divesets.as_ref().unwrap() {
        nlpiterationsdive += scip.divesets[diveset].divesetstats[DiveContext::Adaptive as usize].nlpiterations;
    }

    // Compute the iteration limit.
    let mut lpiterlimit = (h.maxlpiterquot * (nsolsfound + 1.0) / (ncalls as f64 + 1.0) * nlpiterations as f64) as i64;
    lpiterlimit += h.maxlpiterofs;
    lpiterlimit -= nlpiterationsdive;

    lpiterlimit
}

/// `sampleWeighted`: sample from a distribution defined by weights.
fn sample_weighted(rng: &mut RandNumGen, weights: &[f64]) -> i32 {
    let nweights = weights.len();
    let mut weightsum = 0.0;

    // Collect sum of weights.
    for &w in weights {
        weightsum += w;
    }

    let randomnr = rng.get_real(0.0, weightsum);
    weightsum = 0.0;

    // Choose first element i such that the weight sum exceeds the random
    // number.
    let mut w = 0;
    while w + 1 < nweights {
        weightsum += weights[w];
        if weightsum >= randomnr {
            break;
        }
        w += 1;
    }

    w as i32
}

/// `selectDiving`: select the diving method to apply.
fn select_diving(scip: &mut Scip, heur: HeurId) -> ScipResult<i32> {
    let divesets = hd(scip, heur).divesets.clone().unwrap();
    let ndivesets = divesets.len();

    let divecontext = if hd(scip, heur).useadaptivecontext { DiveContext::Adaptive } else { DiveContext::Total };

    // Check availability of divesets.
    let mut methodunavailable = vec![false; ndivesets];
    for d in 0..ndivesets {
        let available = crate::heur::diveset_is_available(scip, divesets[d])?;
        methodunavailable[d] = !available;
    }

    let mut selection: i32 = -1;
    let ncalls = scip.heurs[heur].ncalls;

    match hd(scip, heur).seltype {
        b'e' => {
            let h = hd(scip, heur);
            let mut epsilon_t = h.epsilon * (ndivesets as f64 / (ncalls as f64 + 1.0)).sqrt();
            epsilon_t = if epsilon_t >= 0.05 { epsilon_t } else { 0.05 };

            // Select one of the available methods at random.
            let explore = epsilon_t >= 1.0 || hdm(scip, heur).randnumgen.get_real(0.0, 1.0) < epsilon_t;
            if explore {
                loop {
                    selection = hdm(scip, heur).randnumgen.get_int(0, ndivesets as i32 - 1);
                    if !methodunavailable[selection as usize] {
                        break;
                    }
                }
            } else {
                let mut bestscore = crate::def::REAL_MAX;
                for d in 0..ndivesets {
                    if methodunavailable[d] {
                        continue;
                    }

                    let score = diveset_get_selection_score(scip, divesets[d], hd(scip, heur), divecontext)?;
                    if score < bestscore {
                        bestscore = score;
                        selection = d as i32;
                    }
                }
            }
        }
        b'w' => {
            // Initialize weights as inverse of the score + a small positive
            // epsilon.
            let mut weights = vec![0.0f64; ndivesets];
            for d in 0..ndivesets {
                let score = diveset_get_selection_score(scip, divesets[d], hd(scip, heur), divecontext)?;
                weights[d] = if methodunavailable[d] { 0.0 } else { 1.0 / (score + 1e-4) };
            }

            selection = sample_weighted(&mut hdm(scip, heur).randnumgen, &weights);
        }
        b'n' => {
            // Continue from last selection and stop at the next available
            // method.
            selection = hd(scip, heur).lastselection;
            loop {
                selection = (selection + 1) % ndivesets as i32;
                if !methodunavailable[selection as usize] {
                    break;
                }
            }
            hdm(scip, heur).lastselection = selection;
        }
        c => {
            log::error!("Error: Unknown selection method {}", c as char);
            return Err(Retcode::InvalidData);
        }
    }

    Ok(selection)
}

/// `heurExecAdaptivediving`: execution method of primal heuristic.
fn heur_exec_adaptivediving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    if hd(scip, heur).divesets.is_none() {
        find_and_store_divesets(scip, heur);
    }

    *result = ScipResultCode::Delayed;

    // Do not call heuristic in node that was already detected to be
    // infeasible.
    if nodeinfeasible {
        return Ok(());
    }

    // Only call heuristic, if an optimal LP solution is at hand.
    if !api::has_current_node_lp(scip) || api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only call heuristic, if the LP objective value is smaller than the
    // cutoff bound.
    let lpobjval = api::get_lp_objval(scip);
    if scip.set.is_ge(lpobjval, api::get_cutoffbound(scip)) {
        return Ok(());
    }

    // Only call heuristic, if the LP solution is basic (which allows fast
    // resolve in diving).
    if !api::is_lp_sol_basic(scip) {
        return Ok(());
    }

    // Don't dive two times at the same node.
    if scip.stat.lastdivenode == api::get_n_nodes(scip) && api::get_depth(scip) > 0 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotRun;

    let lpiterlimit = get_lp_iterlimit(scip, heur);

    if lpiterlimit <= 0 {
        return Ok(());
    }

    // Select the next diving strategy based on previous success.
    let selection = select_diving(scip, heur)?;
    let diveset = hd(scip, heur).divesets.as_ref().unwrap()[selection as usize];
    let sol = hd(scip, heur).sol;

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, lpiterlimit, -1, -1.0, DiveContext::Adaptive)?;

    Ok(())
}

/// `SCIPincludeHeurAdaptivediving`: creates the adaptivediving heuristic
/// and includes it in SCIP.
pub fn include_heur_adaptivediving(scip: &mut Scip) -> ScipResult {
    // Create adaptivediving data.
    let seed = scip.set.initialize_random_seed(DEFAULT_INITIALSEED);

    // Include adaptive diving primal heuristic.
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
        heur_exec_adaptivediving,
        Some(Box::new(HeurData {
            sol: NONE,
            randnumgen: RandNumGen::new(seed),
            divesets: None,
            lastselection: -1,
            epsilon: DEFAULT_EPSILON,
            selconfidencecoeff: DEFAULT_SELCONFIDENCECOEFF,
            maxlpiterquot: DEFAULT_MAXLPITERQUOT,
            maxlpiterofs: DEFAULT_MAXLPITEROFS,
            bestsolweight: DEFAULT_BESTSOLWEIGHT,
            seltype: DEFAULT_SELTYPE,
            scoretype: DEFAULT_SCORETYPE,
            useadaptivecontext: DEFAULT_USEADAPTIVECONTEXT,
        }) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_adaptivediving);
    h.heurinit = Some(heur_init_adaptivediving);
    h.heurexit = Some(heur_exit_adaptivediving);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }

    // Add parameters.
    use crate::{
        def::REAL_MAX,
        paramset::{paramset_add_bool, paramset_add_char, paramset_add_longint, paramset_add_real},
    };
    paramset_add_real(
        scip,
        "heuristics/adaptivediving/epsilon",
        "parameter that increases probability of exploration among divesets (only active if seltype is 'e')",
        t!(f64, epsilon),
        false,
        DEFAULT_EPSILON,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/adaptivediving/scoretype",
        "score parameter for selection: minimize either average 'n'odes, LP 'i'terations,backtrack/'c'onflict ratio, 'd'epth, 1 / 's'olutions, or 1 / solutions'u'ccess",
        t!(u8, scoretype),
        false,
        DEFAULT_SCORETYPE,
        Some("cdinsu"),
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/adaptivediving/seltype",
        "selection strategy: (e)psilon-greedy, (w)eighted distribution, (n)ext diving",
        t!(u8, seltype),
        false,
        DEFAULT_SELTYPE,
        Some("enw"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/adaptivediving/useadaptivecontext",
        "should the heuristic use its own statistics, or shared statistics?",
        t!(bool, useadaptivecontext),
        true,
        DEFAULT_USEADAPTIVECONTEXT,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/adaptivediving/selconfidencecoeff",
        "coefficient c to decrease initial confidence (calls + 1.0) / (calls + c) in scores",
        t!(f64, selconfidencecoeff),
        false,
        DEFAULT_SELCONFIDENCECOEFF,
        1.0,
        i32::MAX as f64,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/adaptivediving/maxlpiterquot",
        "maximal fraction of diving LP iterations compared to node LP iterations",
        t!(f64, maxlpiterquot),
        false,
        DEFAULT_MAXLPITERQUOT,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/adaptivediving/maxlpiterofs",
        "additional number of allowed LP iterations",
        t!(i64, maxlpiterofs),
        false,
        DEFAULT_MAXLPITEROFS,
        0,
        i32::MAX as i64,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/adaptivediving/bestsolweight",
        "weight of incumbent solutions compared to other solutions in computation of LP iteration limit",
        t!(f64, bestsolweight),
        false,
        DEFAULT_BESTSOLWEIGHT,
        0.0,
        REAL_MAX,
        None,
    )?;

    Ok(())
}
