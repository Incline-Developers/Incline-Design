//! SCIP's primal heuristics and dive sets (`heur.c`, `struct_heur.h`).

use std::any::Any;

use crate::{
    clock::Clock,
    misc::RandNumGen,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{HeurId, ScipResultCode, SolId, VarId},
    var::NONE,
};

pub type HeurTiming = u32;
pub const HEURTIMING_NONE: HeurTiming = 0x000;
pub const HEURTIMING_BEFORENODE: HeurTiming = 0x001;
pub const HEURTIMING_DURINGLPLOOP: HeurTiming = 0x002;
pub const HEURTIMING_AFTERLPLOOP: HeurTiming = 0x004;
pub const HEURTIMING_AFTERLPNODE: HeurTiming = 0x008;
pub const HEURTIMING_AFTERPSEUDONODE: HeurTiming = 0x010;
pub const HEURTIMING_AFTERLPPLUNGE: HeurTiming = 0x020;
pub const HEURTIMING_AFTERPSEUDOPLUNGE: HeurTiming = 0x040;
pub const HEURTIMING_DURINGPRICINGLOOP: HeurTiming = 0x080;
pub const HEURTIMING_BEFOREPRESOL: HeurTiming = 0x100;
pub const HEURTIMING_DURINGPRESOLLOOP: HeurTiming = 0x200;
pub const HEURTIMING_AFTERPROPLOOP: HeurTiming = 0x400;
pub const HEURTIMING_AFTERNODE: HeurTiming = HEURTIMING_AFTERLPNODE | HEURTIMING_AFTERPSEUDONODE;
pub const HEURTIMING_AFTERPLUNGE: HeurTiming = HEURTIMING_AFTERLPPLUNGE | HEURTIMING_AFTERPSEUDOPLUNGE;

pub type DiveType = u32;
pub const DIVETYPE_NONE: DiveType = 0x000;
pub const DIVETYPE_INTEGRALITY: DiveType = 0x001;
pub const DIVETYPE_SOS1VARIABLE: DiveType = 0x002;

/// `SCIP_DIVECONTEXT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiveContext {
    Total = 0,
    Single = 1,
    Adaptive = 2,
    Scheduler = 3,
}

pub type DivesetId = usize;

pub type HeurMethod = fn(&mut Scip, HeurId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type HeurCopy = fn(&mut Scip, &Scip, HeurId) -> ScipResult;
/// `SCIP_DECL_HEUREXEC`: `(heurtiming, nodeinfeasible, result)`.
pub type HeurExec = fn(&mut Scip, HeurId, HeurTiming, bool, &mut ScipResultCode) -> ScipResult;
/// `SCIP_DECL_DIVESETGETSCORE`: `(divetype, cand, candsol, candsfrac,
/// score, roundup)`.
pub type DivesetGetScore = fn(&mut Scip, DivesetId, DiveType, VarId, f64, f64, &mut f64, &mut bool) -> ScipResult;
/// `SCIP_DECL_DIVESETAVAILABLE`.
pub type DivesetAvailable = fn(&mut Scip, DivesetId, &mut bool) -> ScipResult;

/// `SCIP_DIVESETSTATS`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DivesetStats {
    pub nlpiterations: i64,
    pub nlps: i64,
    pub totaldepth: i64,
    pub totalsoldepth: i64,
    pub totalnnodes: i64,
    pub totalnbacktracks: i64,
    pub nsolsfound: i64,
    pub nbestsolsfound: i64,
    pub nconflictsfound: i64,
    pub mindepth: i32,
    pub maxdepth: i32,
    pub minsoldepth: i32,
    pub maxsoldepth: i32,
    pub ncalls: i32,
    pub nsolcalls: i32,
}

/// `SCIP_DIVESET`.
pub struct Diveset {
    pub heur: HeurId,
    pub name: String,
    pub sol: SolId,
    pub randnumgen: RandNumGen,
    pub divesetstats: [DivesetStats; 4],
    pub minreldepth: f64,
    pub maxreldepth: f64,
    pub maxlpiterquot: f64,
    pub maxdiveubquot: f64,
    pub maxdiveavgquot: f64,
    pub maxdiveubquotnosol: f64,
    pub maxdiveavgquotnosol: f64,
    pub lpresolvedomchgquot: f64,
    pub lpsolvefreq: i32,
    pub maxlpiterofs: i32,
    pub initialseed: u32,
    pub backtrack: bool,
    pub onlylpbranchcands: bool,
    pub ispublic: bool,
    pub divetypemask: DiveType,
    pub divesetgetscore: Option<DivesetGetScore>,
    pub divesetavailable: Option<DivesetAvailable>,
}

/// `SCIP_HEUR`.
pub struct Heur {
    pub ncalls: i64,
    pub nsolsfound: i64,
    pub nbestsolsfound: i64,
    pub name: String,
    pub desc: String,
    pub heurcopy: Option<HeurCopy>,
    pub heurfree: Option<HeurMethod>,
    pub heurinit: Option<HeurMethod>,
    pub heurexit: Option<HeurMethod>,
    pub heurinitsol: Option<HeurMethod>,
    pub heurexitsol: Option<HeurMethod>,
    pub heurexec: Option<HeurExec>,
    pub heurdata: Option<Box<dyn Any>>,
    pub divesets: Vec<DivesetId>,
    pub setuptime: Clock,
    pub heurclock: Clock,
    pub priority: i32,
    pub freq: i32,
    pub freqofs: i32,
    pub maxdepth: i32,
    pub delaypos: i32,
    pub timingmask: HeurTiming,
    pub usessubscip: bool,
    pub exact: bool,
    pub initialized: bool,
    pub dispchar: u8,
}

/// `SCIPheurComp`: compares two heuristics w.r.t. to their delay positions
/// and priorities.
pub fn heur_comp(scip: &Scip, elem1: HeurId, elem2: HeurId) -> i32 {
    let heur1 = &scip.heurs[elem1];
    let heur2 = &scip.heurs[elem2];

    if heur1.delaypos == heur2.delaypos {
        if heur1.priority != heur2.priority {
            // Prefer higher priorities.
            heur2.priority - heur1.priority
        } else {
            // Tiebreaker.
            match heur1.name.as_bytes().cmp(heur2.name.as_bytes()) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }
        }
    } else if heur1.delaypos == -1 {
        // Prefer delayed heuristics.
        1
    } else if heur2.delaypos == -1 {
        // Prefer delayed heuristics.
        -1
    } else if heur1.ncalls * heur1.freq as i64 > heur2.ncalls * heur2.freq as i64 {
        1
    } else if heur1.ncalls * (heur1.freq as i64) < heur2.ncalls * heur2.freq as i64 {
        -1
    } else {
        // Prefer lower delay positions.
        heur1.delaypos - heur2.delaypos
    }
}

/// `SCIPheurCompPriority`: compares two heuristics w.r.t. to their
/// priority values.
pub fn heur_comp_priority(scip: &Scip, elem1: HeurId, elem2: HeurId) -> i32 {
    scip.heurs[elem2].priority - scip.heurs[elem1].priority
}

/// `SCIPsetSortHeurs`: sorts heuristics by their delay positions and
/// priorities.
pub fn set_sort_heurs(scip: &mut Scip) {
    if !scip.set.heurssorted {
        let n = scip.set.nheurs as usize;
        let mut heurs = std::mem::take(&mut scip.set.heurs);
        crate::sort::sort(&mut heurs.0[..n], (), |&a, &b| heur_comp(scip, a, b));
        scip.set.heurs = heurs;
        scip.set.heurssorted = true;
        scip.set.heursnamesorted = false;
    }
}

/// `paramChgdHeurPriority`: the parameter already wrote the priority; mark
/// the heuristics unsorted (`SCIPsetHeurPriority`).
fn param_chgd_heur_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.heurssorted = false;
    Ok(())
}

/// `resetDivesetStats`: resets diving statistics.
fn reset_diveset_stats(divesetstats: &mut DivesetStats) {
    *divesetstats = DivesetStats {
        minsoldepth: i32::MAX,
        maxsoldepth: -1,
        mindepth: i32::MAX,
        maxdepth: -1,
        ..Default::default()
    };
}

/// `SCIPdivesetReset`: resets diving settings counters.
pub fn diveset_reset(scip: &mut Scip, diveset: DivesetId) {
    let seed = scip.set.initialize_random_seed(scip.divesets[diveset].initialseed);
    let d = &mut scip.divesets[diveset];

    // Reset diveset statistics for all contexts.
    for s in d.divesetstats.iter_mut() {
        reset_diveset_stats(s);
    }

    // Reset the random number generator.
    d.randnumgen.set_seed(seed);
}

/// `updateDivesetstats`: update dive set statistics.
#[allow(clippy::too_many_arguments)]
fn update_divesetstats(
    divesetstats: &mut DivesetStats,
    depth: i32,
    nprobingnodes: i32,
    nbacktracks: i32,
    nsolsfound: i64,
    nbestsolsfound: i64,
    nconflictsfound: i64,
    leavesol: bool,
) {
    divesetstats.totaldepth += depth as i64;
    divesetstats.mindepth = divesetstats.mindepth.min(depth);
    divesetstats.maxdepth = divesetstats.maxdepth.max(depth);
    divesetstats.totalnnodes += nprobingnodes as i64;
    divesetstats.totalnbacktracks += nbacktracks as i64;
    divesetstats.ncalls += 1;

    // Update solution statistics only if a solution was found.
    if leavesol {
        divesetstats.totalsoldepth += depth as i64;
        divesetstats.minsoldepth = divesetstats.minsoldepth.min(depth);
        divesetstats.maxsoldepth = divesetstats.maxsoldepth.max(depth);
        divesetstats.nsolcalls += 1;
    }

    divesetstats.nsolsfound += nsolsfound;
    divesetstats.nbestsolsfound += nbestsolsfound;
    divesetstats.nconflictsfound += nconflictsfound;
}

/// `SCIPdivesetUpdateStats`: update diveset statistics and global diveset
/// statistics.
#[allow(clippy::too_many_arguments)]
pub fn diveset_update_stats(
    scip: &mut Scip,
    diveset: DivesetId,
    depth: i32,
    nprobingnodes: i32,
    nbacktracks: i32,
    nsolsfound: i64,
    nbestsolsfound: i64,
    nconflictsfound: i64,
    leavesol: bool,
    divecontext: DiveContext,
) {
    let updatecontexts = [DiveContext::Total, divecontext];

    // Update statistics for total context and given context.
    for c in updatecontexts {
        let s = &mut scip.divesets[diveset].divesetstats[c as usize];
        update_divesetstats(s, depth, nprobingnodes, nbacktracks, nsolsfound, nbestsolsfound, nconflictsfound, leavesol);
    }

    scip.stat.totaldivesetdepth += depth as i64;
    scip.stat.ndivesetcalls += 1;
}

/// `SCIPdivesetCreate`: create a set of diving heuristic settings.
#[allow(clippy::too_many_arguments)]
pub fn diveset_create(
    scip: &mut Scip,
    heur: HeurId,
    name: Option<&str>,
    minreldepth: f64,
    maxreldepth: f64,
    maxlpiterquot: f64,
    maxdiveubquot: f64,
    maxdiveavgquot: f64,
    maxdiveubquotnosol: f64,
    maxdiveavgquotnosol: f64,
    lpresolvedomchgquot: f64,
    lpsolvefreq: i32,
    maxlpiterofs: i32,
    initialseed: u32,
    backtrack: bool,
    onlylpbranchcands: bool,
    ispublic: bool,
    divetypemask: DiveType,
    divesetgetscore: DivesetGetScore,
    divesetavailable: Option<DivesetAvailable>,
) -> ScipResult<DivesetId> {
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};

    // For convenience, the name gets inferred from the heuristic to which
    // the diveset is added if no name is provided.
    let divesetname = name.map(str::to_string).unwrap_or_else(|| scip.heurs[heur].name.clone());

    let diveset = scip.divesets.push(Diveset {
        initialseed,
        // Simply use 0 as initial seed, the seed is reset in
        // SCIPdivesetReset, anyway.
        randnumgen: RandNumGen::new(0),
        name: divesetname.clone(),
        heur: NONE,
        divesetgetscore: Some(divesetgetscore),
        divesetavailable,
        sol: NONE,
        divetypemask,
        ispublic,
        divesetstats: [DivesetStats::default(); 4],
        minreldepth,
        maxreldepth,
        maxlpiterquot,
        maxdiveubquot,
        maxdiveavgquot,
        maxdiveubquotnosol,
        maxdiveavgquotnosol,
        lpresolvedomchgquot,
        lpsolvefreq,
        maxlpiterofs,
        backtrack,
        onlylpbranchcands,
    });

    // Append diveset to heuristic array of divesets (heurAddDiveset).
    scip.divesets[diveset].heur = heur;
    scip.heurs[heur].divesets.push(diveset);

    diveset_reset(scip, diveset);

    // Add collection of diving heuristic specific parameters.
    let n = &divesetname;
    let rmax = crate::def::REAL_MAX;
    paramset_add_real(
        scip,
        &format!("heuristics/{n}/minreldepth"),
        "minimal relative depth to start diving",
        crate::field_target!(f64, divesets, diveset, minreldepth),
        true,
        minreldepth,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("heuristics/{n}/maxreldepth"),
        "maximal relative depth to start diving",
        crate::field_target!(f64, divesets, diveset, maxreldepth),
        true,
        maxreldepth,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("heuristics/{n}/maxlpiterquot"),
        "maximal fraction of diving LP iterations compared to node LP iterations",
        crate::field_target!(f64, divesets, diveset, maxlpiterquot),
        false,
        maxlpiterquot,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("heuristics/{n}/maxlpiterofs"),
        "additional number of allowed LP iterations",
        crate::field_target!(i32, divesets, diveset, maxlpiterofs),
        false,
        maxlpiterofs,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("heuristics/{n}/maxdiveubquot"),
        "maximal quotient (curlowerbound - lowerbound)/(cutoffbound - lowerbound) where diving is performed (0.0: no limit)",
        crate::field_target!(f64, divesets, diveset, maxdiveubquot),
        true,
        maxdiveubquot,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("heuristics/{n}/maxdiveavgquot"),
        "maximal quotient (curlowerbound - lowerbound)/(avglowerbound - lowerbound) where diving is performed (0.0: no limit)",
        crate::field_target!(f64, divesets, diveset, maxdiveavgquot),
        true,
        maxdiveavgquot,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("heuristics/{n}/maxdiveubquotnosol"),
        "maximal UBQUOT when no solution was found yet (0.0: no limit)",
        crate::field_target!(f64, divesets, diveset, maxdiveubquotnosol),
        true,
        maxdiveubquotnosol,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("heuristics/{n}/maxdiveavgquotnosol"),
        "maximal AVGQUOT when no solution was found yet (0.0: no limit)",
        crate::field_target!(f64, divesets, diveset, maxdiveavgquotnosol),
        true,
        maxdiveavgquotnosol,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("heuristics/{n}/backtrack"),
        "use one level of backtracking if infeasibility is encountered?",
        crate::field_target!(bool, divesets, diveset, backtrack),
        false,
        backtrack,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("heuristics/{n}/lpresolvedomchgquot"),
        "percentage of immediate domain changes during probing to trigger LP resolve",
        crate::field_target!(f64, divesets, diveset, lpresolvedomchgquot),
        false,
        lpresolvedomchgquot,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("heuristics/{n}/lpsolvefreq"),
        "LP solve frequency for diving heuristics (0: only after enough domain changes have been found)",
        crate::field_target!(i32, divesets, diveset, lpsolvefreq),
        false,
        lpsolvefreq,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("heuristics/{n}/onlylpbranchcands"),
        "should only LP branching candidates be considered instead of the slower but more general constraint handler diving variable selection?",
        crate::field_target!(bool, divesets, diveset, onlylpbranchcands),
        false,
        onlylpbranchcands,
        None,
    )?;
    Ok(diveset)
}

/// `SCIPdivesetGetNSols`: get the total number of solutions (leaf and
/// rounding solutions) found by the diving settings.
pub fn diveset_get_n_sols(scip: &Scip, diveset: DivesetId, divecontext: DiveContext) -> i64 {
    scip.divesets[diveset].divesetstats[divecontext as usize].nsolsfound
}

/// `SCIPdivesetGetSolSuccess`: get the number of successful runs of the
/// diving settings.
pub fn diveset_get_sol_success(scip: &Scip, diveset: DivesetId, divecontext: DiveContext) -> i64 {
    let s = &scip.divesets[diveset].divesetstats[divecontext as usize];
    10 * s.nbestsolsfound + s.nsolsfound
}

/// `SCIPdivesetGetAvgDepth`: get the average depth this dive set reached
/// during execution.
pub fn diveset_get_avg_depth(scip: &Scip, diveset: DivesetId, divecontext: DiveContext) -> f64 {
    let s = &scip.divesets[diveset].divesetstats[divecontext as usize];
    if s.ncalls == 0 { 0.0 } else { s.totaldepth as f64 / s.ncalls as f64 }
}

/// `SCIPdivesetGetAvgSolutionDepth`: get the average depth at which this
/// dive set found a solution.
pub fn diveset_get_avg_solution_depth(scip: &Scip, diveset: DivesetId, divecontext: DiveContext) -> f64 {
    let s = &scip.divesets[diveset].divesetstats[divecontext as usize];
    if s.nsolcalls == 0 { 0.0 } else { s.totalsoldepth as f64 / s.nsolcalls as f64 }
}

/// `SCIPdivesetSupportsType`: returns TRUE if dive set supports diving of
/// the specified type.
pub fn diveset_supports_type(scip: &Scip, diveset: DivesetId, divetype: DiveType) -> bool {
    divetype & scip.divesets[diveset].divetypemask != 0
}

/// `SCIPdivesetUpdateLPStats`: update diveset LP statistics, should be
/// called after every LP solved by this diving heuristic.
pub fn diveset_update_lp_stats(scip: &mut Scip, diveset: DivesetId, niterstoadd: i64, divecontext: DiveContext) {
    // Update statistics for total context and given context.
    for c in [divecontext, DiveContext::Total] {
        let s = &mut scip.divesets[diveset].divesetstats[c as usize];
        s.nlpiterations += niterstoadd;
        s.nlps += 1;
    }

    scip.stat.ndivesetlpiterations += niterstoadd;
    scip.stat.ndivesetlps += 1;
}

/// `SCIPdivesetGetScore`: get the candidate score and preferred rounding
/// direction for a candidate variable; returns `(score, roundup)`.
pub fn diveset_get_score(scip: &mut Scip, diveset: DivesetId, divetype: DiveType, divecand: VarId, divecandsol: f64, divecandfrac: f64) -> ScipResult<(f64, bool)> {
    let getscore = scip.divesets[diveset].divesetgetscore.expect("a scoring method");
    let mut candscore = 0.0;
    let mut roundup = false;
    getscore(scip, diveset, divetype, divecand, divecandsol, divecandfrac, &mut candscore, &mut roundup)?;
    Ok((candscore, roundup))
}

/// `SCIPdivesetIsAvailable`: check specific preconditions for diving,
/// e.g., if an incumbent solution is available.
pub fn diveset_is_available(scip: &mut Scip, diveset: DivesetId) -> ScipResult<bool> {
    match scip.divesets[diveset].divesetavailable {
        None => Ok(true),
        Some(available) => {
            let mut a = false;
            available(scip, diveset, &mut a)?;
            Ok(a)
        }
    }
}

/// `SCIPheurCopyInclude`: copies the given primal heuristic to a new scip.
pub fn heur_copy_include(scip: &mut Scip, sourcescip: &Scip, heur: HeurId) -> ScipResult {
    if let Some(copy) = sourcescip.heurs[heur].heurcopy {
        copy(scip, sourcescip, heur)?;
    }
    Ok(())
}

/// The callbacks `SCIPheurCreate` takes.
#[derive(Default)]
pub struct HeurMethods {
    pub heurcopy: Option<HeurCopy>,
    pub heurfree: Option<HeurMethod>,
    pub heurinit: Option<HeurMethod>,
    pub heurexit: Option<HeurMethod>,
    pub heurinitsol: Option<HeurMethod>,
    pub heurexitsol: Option<HeurMethod>,
    pub heurexec: Option<HeurExec>,
}

/// `SCIPheurCreate`: creates a primal heuristic.
#[allow(clippy::too_many_arguments)]
pub fn heur_create(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    dispchar: u8,
    priority: i32,
    freq: i32,
    freqofs: i32,
    maxdepth: i32,
    timingmask: HeurTiming,
    usessubscip: bool,
    methods: HeurMethods,
    heurdata: Option<Box<dyn Any>>,
) -> ScipResult<HeurId> {
    use crate::paramset::paramset_add_int;

    let heur = scip.heurs.push(Heur {
        name: name.to_string(),
        desc: desc.to_string(),
        dispchar,
        priority,
        freq,
        freqofs,
        maxdepth,
        delaypos: -1,
        timingmask,
        usessubscip,
        heurcopy: methods.heurcopy,
        heurfree: methods.heurfree,
        heurinit: methods.heurinit,
        heurexit: methods.heurexit,
        heurinitsol: methods.heurinitsol,
        heurexitsol: methods.heurexitsol,
        heurexec: methods.heurexec,
        heurdata,
        setuptime: Clock::new(),
        heurclock: Clock::new(),
        ncalls: 0,
        nsolsfound: 0,
        nbestsolsfound: 0,
        exact: false,
        initialized: false,
        divesets: Vec::new(),
    });

    // Add parameters.
    let maxd = crate::def::MAXTREEDEPTH;
    paramset_add_int(
        scip,
        &format!("heuristics/{name}/priority"),
        &format!("priority of heuristic <{name}>"),
        crate::field_target!(i32, heurs, heur, priority),
        true,
        priority,
        i32::MIN / 4,
        i32::MAX / 4,
        Some(param_chgd_heur_priority),
    )?;
    paramset_add_int(
        scip,
        &format!("heuristics/{name}/freq"),
        &format!("frequency for calling primal heuristic <{name}> (-1: never, 0: only at depth freqofs)"),
        crate::field_target!(i32, heurs, heur, freq),
        false,
        freq,
        -1,
        maxd,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("heuristics/{name}/freqofs"),
        &format!("frequency offset for calling primal heuristic <{name}>"),
        crate::field_target!(i32, heurs, heur, freqofs),
        false,
        freqofs,
        0,
        maxd,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("heuristics/{name}/maxdepth"),
        &format!("maximal depth level to call primal heuristic <{name}> (-1: no limit)"),
        crate::field_target!(i32, heurs, heur, maxdepth),
        true,
        maxdepth,
        -1,
        maxd,
        None,
    )?;
    Ok(heur)
}

/// `SCIPheurFree`: calls destructor and frees memory of primal heuristic.
pub fn heur_free(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Call destructor of primal heuristic.
    if let Some(free) = scip.heurs[heur].heurfree {
        free(scip, heur)?;
    }

    for d in std::mem::take(&mut scip.heurs[heur].divesets) {
        scip.divesets.remove(d);
    }
    scip.heurs.remove(heur);
    Ok(())
}

/// Runs one of the heuristic's setup callbacks under its setup clock.
fn heur_setup_call(scip: &mut Scip, heur: HeurId, method: Option<HeurMethod>) -> ScipResult {
    if let Some(method) = method {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.heurs[heur].setuptime.start(enabled);
        method(scip, heur)?;
        // Stop timing.
        scip.heurs[heur].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPheurInit`: initializes primal heuristic.
pub fn heur_init(scip: &mut Scip, heur: HeurId) -> ScipResult {
    if scip.heurs[heur].initialized {
        log::error!("primal heuristic <{}> already initialized", scip.heurs[heur].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let h = &mut scip.heurs[heur];
        h.setuptime.reset();
        h.heurclock.reset();

        h.delaypos = -1;
        h.ncalls = 0;
        h.nsolsfound = 0;
        h.nbestsolsfound = 0;

        scip.set.heurssorted = false;
        scip.set.heursnamesorted = false;
    }

    let init = scip.heurs[heur].heurinit;
    heur_setup_call(scip, heur, init)?;

    // Reset dive sets.
    for d in scip.heurs[heur].divesets.clone() {
        diveset_reset(scip, d);
    }

    scip.heurs[heur].initialized = true;
    Ok(())
}

/// `SCIPheurExit`: calls exit method of primal heuristic.
pub fn heur_exit(scip: &mut Scip, heur: HeurId) -> ScipResult {
    if !scip.heurs[heur].initialized {
        log::error!("primal heuristic <{}> not initialized", scip.heurs[heur].name);
        return Err(Retcode::InvalidCall);
    }

    let exit = scip.heurs[heur].heurexit;
    heur_setup_call(scip, heur, exit)?;
    scip.heurs[heur].initialized = false;
    Ok(())
}

/// `SCIPheurInitsol`: informs primal heuristic that the branch and bound
/// process is being started.
pub fn heur_initsol(scip: &mut Scip, heur: HeurId) -> ScipResult {
    if scip.heurs[heur].delaypos != -1 {
        scip.heurs[heur].delaypos = -1;
        scip.set.heurssorted = false;
    }

    // Call solving process initialization method of primal heuristic.
    let initsol = scip.heurs[heur].heurinitsol;
    heur_setup_call(scip, heur, initsol)
}

/// `SCIPheurExitsol`: informs primal heuristic that the branch and bound
/// process data is being freed.
pub fn heur_exitsol(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Call solving process deinitialization method of primal heuristic.
    let exitsol = scip.heurs[heur].heurexitsol;
    heur_setup_call(scip, heur, exitsol)
}

/// `SCIPheurShouldBeExecuted`: should the heuristic be executed at the
/// given depth, frequency, timing, ...; returns `(execute, delayed)`.
pub fn heur_should_be_executed(scip: &Scip, heur: HeurId, depth: i32, lpstateforkdepth: i32, heurtiming: HeurTiming) -> (bool, bool) {
    let h = &scip.heurs[heur];
    let mut delayed = false;

    let mut execute = if ((h.timingmask & HEURTIMING_BEFOREPRESOL) != 0 && heurtiming == HEURTIMING_BEFOREPRESOL)
        || ((h.timingmask & HEURTIMING_DURINGPRESOLLOOP) != 0 && heurtiming == HEURTIMING_DURINGPRESOLLOOP)
    {
        // Heuristic may be executed before/during presolving. Do so, if it
        // was not disabled by setting the frequency to -1.
        h.freq >= 0
    } else if (h.timingmask & HEURTIMING_AFTERPSEUDONODE) == 0 && (heurtiming == HEURTIMING_AFTERLPNODE || heurtiming == HEURTIMING_AFTERLPPLUNGE) {
        // Heuristic was skipped on intermediate pseudo nodes: check, if a node
        // matching the execution frequency lies between the current node and
        // the last LP node of the path.
        h.freq > 0 && depth >= h.freqofs && ((depth + h.freq - h.freqofs) / h.freq != (lpstateforkdepth + h.freq - h.freqofs) / h.freq)
    } else {
        // Heuristic may be executed on every node: check, if the current depth
        // matches the execution frequency and offset.
        h.freq > 0 && depth >= h.freqofs && (depth - h.freqofs) % h.freq == 0
    };

    // If frequency is zero, execute heuristic only at the depth level of the
    // frequency offset.
    execute = execute || (depth == h.freqofs && h.freq == 0);

    // Compare current depth against heuristic's maximal depth level.
    execute = execute && (h.maxdepth == -1 || depth <= h.maxdepth);

    // If the heuristic was delayed, execute it anyway.
    execute = execute || h.delaypos >= 0;

    // If the heuristic should be called after plunging but not during
    // plunging, delay it if we are in plunging.
    if execute
        && ((heurtiming == HEURTIMING_AFTERLPNODE && (h.timingmask & HEURTIMING_AFTERLPNODE) == 0 && (h.timingmask & HEURTIMING_AFTERLPPLUNGE) > 0)
            || (heurtiming == HEURTIMING_AFTERPSEUDONODE && (h.timingmask & HEURTIMING_AFTERPSEUDONODE) == 0 && (h.timingmask & HEURTIMING_AFTERPSEUDOPLUNGE) > 0))
    {
        // The heuristic should be delayed until plunging is finished.
        execute = false;
        delayed = true;
    }

    // Execute heuristic only if its timing mask fits the current point in
    // the node solving process.
    execute = execute && (h.timingmask & heurtiming) > 0;

    (execute, delayed)
}

/// `SCIPheurExec`: calls execution method of primal heuristic.
#[allow(clippy::too_many_arguments)]
pub fn heur_exec(
    scip: &mut Scip,
    heur: HeurId,
    primal: usize,
    depth: i32,
    lpstateforkdepth: i32,
    heurtiming: HeurTiming,
    nodeinfeasible: bool,
    ndelayedheurs: &mut i32,
) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    let (execute, delayed) = heur_should_be_executed(scip, heur, depth, lpstateforkdepth, heurtiming);

    if delayed {
        result = R::Delayed;
    }

    if execute {
        let oldnsolsfound = scip.primals[primal].nsolsfound;
        let oldnbestsolsfound = scip.primals[primal].nbestsolsfound;

        // Start timing.
        let enabled = scip.set.time_enabled;
        scip.heurs[heur].heurclock.start(enabled);

        // Call external method.
        let exec = scip.heurs[heur].heurexec.expect("an execution method");
        exec(scip, heur, heurtiming, nodeinfeasible, &mut result)?;

        // Stop timing.
        scip.heurs[heur].heurclock.stop(enabled);

        // Evaluate result.
        if !matches!(result, R::FoundSol | R::DidNotFind | R::DidNotRun | R::Delayed | R::Unbounded) {
            log::error!(
                "execution method of primal heuristic <{}> returned invalid result <{}>",
                scip.heurs[heur].name,
                result as i32
            );
            return Err(Retcode::InvalidResult);
        }
        let (nsols, nbest) = (scip.primals[primal].nsolsfound, scip.primals[primal].nbestsolsfound);
        let h = &mut scip.heurs[heur];
        if result != R::DidNotRun && result != R::Delayed {
            h.ncalls += 1;
        }
        h.nsolsfound += nsols - oldnsolsfound;
        h.nbestsolsfound += nbest - oldnbestsolsfound;

        // Update delay position of heuristic.
        if result != R::Delayed && h.delaypos != -1 {
            h.delaypos = -1;
            scip.set.heurssorted = false;
        }
    }

    // Check if the heuristic was (still) delayed.
    if result == R::Delayed || scip.heurs[heur].delaypos >= 0 {
        // Mark the heuristic delayed.
        if scip.heurs[heur].delaypos != *ndelayedheurs {
            scip.heurs[heur].delaypos = *ndelayedheurs;
            scip.set.heurssorted = false;
        }
        *ndelayedheurs += 1;
    }
    Ok(result)
}

/// `SCIPheurSetPriority`: sets priority of primal heuristic.
pub fn heur_set_priority(scip: &mut Scip, heur: HeurId, priority: i32) {
    scip.heurs[heur].priority = priority;
    scip.set.heurssorted = false;
}

/// `SCIPheurEnableOrDisableClocks`.
pub fn heur_enable_or_disable_clocks(scip: &mut Scip, heur: HeurId, enable: bool) {
    let h = &mut scip.heurs[heur];
    h.setuptime.enable_or_disable(enable);
    h.heurclock.enable_or_disable(enable);
}

/// `SCIP_VGRAPH`: the variable-constraint graph for breadth-first search
/// neighborhoods (`SCIPvariableGraphCreate` and its siblings).
pub struct VGraph {
    /// Constraints of each variable.
    varconss: Vec<Vec<crate::types::ConsId>>,
    /// The constraints visited during breadth-first search (C's hash
    /// table, of which only membership is used).
    visitedconss: std::collections::HashSet<crate::types::ConsId>,
}

impl VGraph {
    /// `SCIPvariableGraphCreate` (with `fillVariableGraph`): returns the
    /// graph (`None` if there are no variables) and the number of relaxed
    /// dense constraints.
    pub fn create(scip: &mut Scip, relaxdenseconss: bool, relaxdensity: f64) -> ScipResult<(Option<VGraph>, i32)> {
        let nvars = crate::api::get_n_vars(scip) as usize;
        let mut nrelaxedconstraints = 0;
        if nvars == 0 {
            return Ok((None, 0));
        }

        let mut vargraph = VGraph {
            varconss: vec![Vec::new(); nvars],
            visitedconss: Default::default(),
        };

        let t = &scip.probs[crate::prob::TRANSPROB];
        let conss = t.conss.0[..t.nconss as usize].to_vec();
        let relaxlimit = (relaxdensity * nvars as f64) as i32;

        for cons in conss {
            // We only consider constraints that are checkable.
            if !scip.conss[cons].check {
                continue;
            }

            // Request number of variables.
            let (nconsvars, success) = crate::cons::cons_get_n_vars(scip, cons)?;
            if !success {
                continue;
            }

            // Relax constraints with density above the allowed number of free
            // variables.
            if relaxdenseconss && nconsvars >= relaxlimit {
                nrelaxedconstraints += 1;
                continue;
            }

            // Collect constraint variables in buffer.
            let (varbuffer, success) = crate::cons::cons_get_vars(scip, cons)?;
            if !success {
                continue;
            }

            // Loop over constraint variables and add this constraint to them if
            // they are active.
            for &var in &varbuffer[..nconsvars as usize] {
                let varpos = scip.vars[var].probindex;

                // Skip inactive variables.
                if varpos == -1 {
                    continue;
                }

                // Add constraint to constraint array for this variable.
                vargraph.varconss[varpos as usize].push(cons);
            }
        }

        Ok((Some(vargraph), nrelaxedconstraints))
    }

    /// `SCIPvariablegraphBreadthFirst`: the distances in the variable graph
    /// from the start variables (-1 for unreached ones). `vargraph` `None`
    /// creates a local graph.
    pub fn breadth_first(
        scip: &mut Scip,
        vargraph: Option<&mut VGraph>,
        startvars: &[crate::types::VarId],
        distances: &mut [i32],
        maxdistance: i32,
        maxvars: i32,
        maxbinintvars: i32,
    ) -> ScipResult {
        // Get variable data.
        let vars = crate::api::get_vars(scip).to_vec();
        let nvars = vars.len();
        if nvars == 0 {
            return Ok(());
        }

        let nbinintvars = nvars as i32 - crate::api::get_n_cont_vars(scip) - crate::api::get_n_cont_impl_vars(scip);
        let mut queue = vec![-1i32; nvars];
        let mut increment: isize = 1;

        // Create a variable graph locally for this method, if none is
        // provided.
        let mut localvargraph;
        let vargraph = match vargraph {
            Some(g) => g,
            None => {
                localvargraph = VGraph::create(scip, false, 1.0)?.0.unwrap();
                &mut localvargraph
            }
        };

        vargraph.visitedconss.clear();

        // Initialize distances to -1.
        for d in distances[..nvars].iter_mut() {
            *d = -1;
        }

        let mut nvarshit = 0;
        let mut nbinintvarshit = 0;

        // Initialize distances for starting variables and add them to the
        // queue.
        for (i, &sv) in startvars.iter().enumerate() {
            let probindex = scip.vars[sv].probindex;

            // Start variables have a distance of 0.
            distances[probindex as usize] = 0;
            queue[i] = probindex;
            nvarshit += 1;

            if probindex < nbinintvars {
                nbinintvarshit += 1;
            }
        }

        let mut currlvlidx: isize = 0;
        let mut nextlvlidx: isize = nvars as isize - 1;
        let mut currentdistance;

        // Loop over the queue and pop the next variable, starting with start
        // variables.
        loop {
            let varpos = scip.vars[vars[queue[currlvlidx as usize] as usize]].probindex as usize;
            currentdistance = distances[varpos];

            // Check for termination because maximum distance has been reached.
            if currentdistance == maxdistance {
                break;
            }

            // Loop over variable constraints and enqueue variables that were not
            // visited yet.
            for c in 0..vargraph.varconss[varpos].len() {
                let cons = vargraph.varconss[varpos][c];

                // Check first if this constraint has already been visited.
                if vargraph.visitedconss.contains(&cons) {
                    continue;
                }

                // Request number of constraint variables.
                let (nconsvars, success) = crate::cons::cons_get_n_vars(scip, cons)?;
                if !success {
                    continue;
                }

                // Collect constraint variables in buffer.
                let (varbuffer, success) = crate::cons::cons_get_vars(scip, cons)?;
                if !success {
                    continue;
                }

                // Collect previously unvisited variables of the constraint and
                // enqueue them for breadth-first search.
                for &nextvar in &varbuffer[..nconsvars as usize] {
                    if !crate::var::var_is_active(scip, nextvar) {
                        continue;
                    }

                    let nextvarpos = scip.vars[nextvar].probindex;

                    // Insert variables that were not considered yet into the
                    // next level queue.
                    if distances[nextvarpos as usize] == -1 {
                        distances[nextvarpos as usize] = currentdistance + 1;
                        queue[nextlvlidx as usize] = nextvarpos;
                        nextlvlidx -= increment;
                        nvarshit += 1;

                        if nextvarpos < nbinintvars {
                            nbinintvarshit += 1;
                        }
                    }
                }

                // Mark the constraint as visited.
                vargraph.visitedconss.insert(cons);
            }

            queue[currlvlidx as usize] = -1;
            currlvlidx += increment;

            // Check if we need to swap current and next level index and reverse
            // the increment.
            if currlvlidx == nvars as isize || currlvlidx == 0 || queue[currlvlidx as usize] == -1 || currlvlidx == nextlvlidx {
                // Break early if the distance has been determined for enough
                // variables.
                if nvarshit >= maxvars || nbinintvarshit >= maxbinintvars {
                    break;
                }

                // Increment knows whether we are currently looping upwards (all
                // variables with odd distance) or downwards the queue.
                if increment == 1 {
                    currlvlidx = nvars as isize - 1;
                    nextlvlidx = 0;
                    increment = -1;
                } else {
                    currlvlidx = 0;
                    nextlvlidx = nvars as isize - 1;
                    increment = 1;
                }
            }

            if !(queue[currlvlidx as usize] != -1 && distances[queue[currlvlidx as usize] as usize] >= currentdistance) {
                break;
            }
        }

        Ok(())
    }
}
