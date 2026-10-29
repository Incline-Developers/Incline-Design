//! Port of `heur_indicatordiving.c`: LP diving heuristic that fixes
//! indicator variables controlling semicontinuous variables.
//!
//! A variable \f$x\f$ is semicontinuous if its domain is \f$\{c\} \cup
//! [lb_1, ub_1]\f$ depending on a binary variable, here an indicator
//! variable (or a variable bounding variable of a varbound constraint).

use std::{any::Any, collections::HashMap};

use crate::{
    api,
    heur::{DIVETYPE_INTEGRALITY, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ConsId, ConshdlrId, HeurId, ScipResultCode, SolId, VarId, VarType},
    var::NONE,
};

const HEUR_NAME: &str = "indicatordiving";
const HEUR_DESC: &str = "LP diving heuristic that fixes indicator variables controlling semicontinuous variables";
const HEUR_DISPCHAR: u8 = b'I';
const HEUR_PRIORITY: i32 = -150000;
const HEUR_FREQ: i32 = 0;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;
/// Bit mask that represents all supported dive types.
const DIVESET_DIVETYPES: DiveType = DIVETYPE_INTEGRALITY;
/// Is this dive set publicly available (ie., can be used by other primal
/// heuristics?)
const DIVESET_ISPUBLIC: bool = false;

/*
 * Default parameter settings
 */

/// Minimal relative depth to start diving.
const DEFAULT_MINRELDEPTH: f64 = 0.0;
/// Maximal relative depth to start diving.
const DEFAULT_MAXRELDEPTH: f64 = 1.0;
/// Maximal fraction of diving LP iterations compared to node LP iterations.
const DEFAULT_MAXLPITERQUOT: f64 = 0.05;
/// Additional number of allowed LP iterations.
const DEFAULT_MAXLPITEROFS: i32 = 1000;
/// Maximal quotient (curlowerbound - lowerbound)/(cutoffbound - lowerbound)
/// where diving is performed (0.0: no limit).
const DEFAULT_MAXDIVEUBQUOT: f64 = 0.8;
/// Maximal quotient (curlowerbound - lowerbound)/(avglowerbound -
/// lowerbound) where diving is performed (0.0: no limit).
const DEFAULT_MAXDIVEAVGQUOT: f64 = 0.0;
/// Maximal UBQUOT when no solution was found yet (0.0: no limit).
const DEFAULT_MAXDIVEUBQUOTNOSOL: f64 = 0.1;
/// Maximal AVGQUOT when no solution was found yet (0.0: no limit).
const DEFAULT_MAXDIVEAVGQUOTNOSOL: f64 = 0.0;
/// Use one level of backtracking if infeasibility is encountered?
const DEFAULT_BACKTRACK: bool = true;
/// Percentage of immediate domain changes during probing to trigger LP
/// resolve.
const DEFAULT_LPRESOLVEDOMCHGQUOT: f64 = 0.15;
/// LP solve frequency for diving heuristics.
const DEFAULT_LPSOLVEFREQ: i32 = 30;
/// Should only LP branching candidates be considered instead of the slower
/// but more general constraint handler diving variable selection?
const DEFAULT_ONLYLPBRANCHCANDS: bool = false;
/// Initial seed for random number generation.
const DEFAULT_RANDSEED: u32 = 11;

/*
 * Heuristic specific parameters
 */

/// Default setting for parameter roundingfrac.
const DEFAULT_ROUNDINGFRAC: f64 = 0.5;
/// Default setting for parameter roundingmode.
const DEFAULT_ROUNDINGMODE: i32 = 0;
/// Default setting for parameter semicontscoremode.
const DEFAULT_SEMICONTSCOREMODE: i32 = 0;
/// Default setting for parameter usevarbounds.
const DEFAULT_USEVARBOUNDS: bool = true;
/// Default setting for parameter runwithoutscinds.
const DEFAULT_RUNWITHOUTSCINDS: bool = false;

/// `ROUNDING_CONSERVATIVE`.
const ROUNDING_CONSERVATIVE: i32 = 0;
/// `ROUNDING_AGGRESSIVE`.
const ROUNDING_AGGRESSIVE: i32 = 1;

const MIN_RAND: f64 = 1e-06;
const MAX_RAND: f64 = 1e-05;

/// `SCVARDATA`: data structure to store information of a semicontinuous
/// variable.
///
/// For a variable x (not stored in the struct), this stores the data of
/// nbnds implications bvars[i] = 0 -> x = vals[i] and bvars[i] = 1 -> lbs[i]
/// <= x <= ubs[i], where bvars[i] are binary variables.
#[derive(Default)]
struct ScVarData {
    /// Values of the variable when the corresponding bvars[i] = 0.
    vals0: Vec<f64>,
    /// Global lower bounds of the variable when the corresponding bvars[i] =
    /// 1.
    lbs1: Vec<f64>,
    /// Global upper bounds of the variable when the corresponding bvars[i] =
    /// 1.
    ubs1: Vec<f64>,
    /// The binary variables on which the variable domain depends.
    bvars: Vec<VarId>,
    /// Number of suitable on/off bounds the var has.
    nbnds: i32,
}

/// `SCIP_HeurData`: locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: SolId,
    /// Indicator constraint handler.
    indicatorconshdlr: ConshdlrId,
    /// Varbound constraint handler.
    varboundconshdlr: ConshdlrId,
    /// Hashmap to store semicontinuous variables.
    scvars: Option<HashMap<VarId, ScVarData>>,
    /// Hashmap to store indicator constraints of binary variables.
    indicatormap: Option<HashMap<VarId, ConsId>>,
    /// Hashmap to store varbound constraints of binary variables.
    varboundmap: Option<HashMap<VarId, ConsId>>,
    /// In violation case all fractional below this value are fixed to
    /// constant.
    roundingfrac: f64,
    /// Decides which roundingmode is selected (0: conservative (default), 1:
    /// aggressive).
    roundingmode: i32,
    /// Which values of semi-continuous variables should get a high score?
    /// (0: low (default), 1: middle, 2: high).
    semicontscoremode: i32,
    /// Should varbound constraints be considered?
    usevarbounds: bool,
    /// Should heur run if there are no indicator constraints modeling
    /// semicont. vars?
    runwithoutscinds: bool,
    /// Can we skip the candidate var until indicator conss handler determines
    /// the candidate var?
    gotoindconss: bool,
    /// Contains current solution violated indicator constraints? (only
    /// unbounded)
    containsviolindconss: bool,
    /// Are we at a new probing node?
    newnode: bool,
    /// Current probing depth.
    probingdepth: i32,
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

/// `isViolatedAndNotFixed`: checks if constraint is violated but can be
/// fixed by the indicator constraint handler.
fn is_violated_and_not_fixed(scip: &Scip, sol: SolId, cons: ConsId) -> bool {
    if !crate::cons_indicator::is_violated_indicator(scip, cons, sol) {
        return false;
    }

    let binvar = crate::cons_indicator::get_binary_var_indicator(scip, cons);
    let solval = api::get_sol_val(scip, sol, binvar);

    scip.set.is_feas_integral(solval) && scip.vars[binvar].locdom.lb < scip.vars[binvar].locdom.ub - 0.5
}

/// `checkAndGetIndicator`: checks if variable is indicator variable and
/// stores corresponding indicator constraint; returns `(cons,
/// isindicator)`. Additionally, if we are at a new probing node, checks if
/// there are violated and not fixed (unbounded) indicator constraints.
fn check_and_get_indicator(scip: &mut Scip, heur: HeurId, cand: VarId) -> (ConsId, bool) {
    let (newnode, sol, conshdlr) = (hd(scip, heur).newnode, hd(scip, heur).sol, hd(scip, heur).indicatorconshdlr);

    let cons = hd(scip, heur).indicatormap.as_ref().unwrap().get(&cand).copied().unwrap_or(NONE);
    let isindicator = cons != NONE;

    // Since we are at a new probing node, check if there are violated and not
    // fixed indicator constraints.
    if newnode {
        let h = &scip.conshdlrs[conshdlr];
        let indicatorconss = h.conss.0[..h.nactiveconss as usize].to_vec();
        let mut containsviolindconss = false;

        for &c in &indicatorconss {
            containsviolindconss = containsviolindconss || is_violated_and_not_fixed(scip, sol, c);

            if containsviolindconss {
                break;
            }
        }
        hdm(scip, heur).containsviolindconss = containsviolindconss;
    }

    (cons, isindicator)
}

/// `checkAndGetVarbound`: checks if variable is binary variable of varbound
/// constraint and stores corresponding varbound constraint; returns `(cons,
/// isvarbound)`.
fn check_and_get_varbound(scip: &Scip, heur: HeurId, cand: VarId) -> (ConsId, bool) {
    if scip.vars[cand].vartype != VarType::Binary || crate::var::var_is_implied_integral(scip, cand) {
        return (NONE, false);
    }

    let cons = hd(scip, heur).varboundmap.as_ref().unwrap().get(&cand).copied().unwrap_or(NONE);
    (cons, cons != NONE)
}

/// `addSCVarIndicator`: adds an indicator to the data of a semicontinuous
/// variable.
fn add_sc_var_indicator(scip: &Scip, scvdata: &mut ScVarData, indicator: VarId, val0: f64, lb1: f64, ub1: f64) {
    // Find the position where to insert.
    let (found, pos) = if scvdata.bvars.is_empty() {
        (false, 0)
    } else {
        match crate::sort::sortedvec_find(&scvdata.bvars[..scvdata.nbnds as usize], |&a, &b| crate::var::var_compare(scip, a, b), indicator) {
            Ok(p) => (true, p),
            Err(p) => (false, p),
        }
    };

    if found {
        return;
    }

    // Move entries if needed.
    scvdata.bvars.insert(pos, indicator);
    scvdata.vals0.insert(pos, val0);
    scvdata.lbs1.insert(pos, lb1);
    scvdata.ubs1.insert(pos, ub1);
    scvdata.nbnds += 1;
}

/// The variable bounds of a variable: `(vars, coefs, constants)` (empty
/// when there are none).
fn var_vbounds(vb: &Option<Box<crate::var::VBounds>>) -> (Vec<VarId>, Vec<f64>, Vec<f64>) {
    match vb {
        Some(v) => {
            let n = v.len as usize;
            (v.vars.0[..n].to_vec(), v.coefs.0[..n].to_vec(), v.constants.0[..n].to_vec())
        }
        None => (Vec::new(), Vec::new(), Vec::new()),
    }
}

/// `varIsSemicontinuous`: checks if a variable is semicontinuous and, if
/// needed, updates the scvars hashmap; returns whether it is.
///
/// A variable \f$x\f$ is semicontinuous if its bounds depend on the binary
/// variable bvar and bvar == 0 => \f$x = x^\prime\f$. Currently, we only
/// consider semicontinuous variables with constant \f$x^\prime\f$ that is
/// equal to the given constant.
fn var_is_semicontinuous(scip: &Scip, heur_scvars: &mut HashMap<VarId, ScVarData>, var: VarId, constant: f64) -> bool {
    if heur_scvars.contains_key(&var) {
        return true;
    }

    let v = &scip.vars[var];
    let (vlbvars, vlbcoefs, vlbconstants) = var_vbounds(&v.vlbs);
    let (vubvars, vubcoefs, vubconstants) = var_vbounds(&v.vubs);
    let nvlbs = vlbvars.len();
    let nvubs = vubvars.len();
    let glb = v.glbdom.lb;
    let gub = v.glbdom.ub;
    let mut scvdata: Option<ScVarData> = None;

    let find = |vec: &[VarId], bvar: VarId| crate::sort::sortedvec_find(vec, |&a, &b| crate::var::var_compare(scip, a, b), bvar);

    // Scan through lower bounds; for each binary vlbvar save the corresponding
    // lb0 and lb1. Then check if there is an upper bound with this vlbvar and
    // save ub0 and ub1. If the found bounds imply that the var value is fixed
    // to some val0 when vlbvar = 0, save vlbvar and val0 to scvdata.
    for c in 0..nvlbs {
        if scip.vars[vlbvars[c]].vartype != VarType::Binary || crate::var::var_is_implied_integral(scip, vlbvars[c]) {
            continue;
        }

        let bvar = vlbvars[c];

        let lb0 = crate::def::max(vlbconstants[c], glb);
        let lb1 = crate::def::max(vlbconstants[c] + vlbcoefs[c], glb);

        // Look for bvar in vubvars.
        let found = if v.vubs.is_some() { find(&vubvars[..nvubs], bvar).ok() } else { None };

        let (ub0, ub1) = match found {
            // Save the upper bounds.
            Some(pos) => (crate::def::min(vubconstants[pos], gub), crate::def::min(vubconstants[pos] + vubcoefs[pos], gub)),
            // If there is no upper bound with vubvar = bvar, use global var
            // bounds.
            None => (gub, gub),
        };

        // The 'off' domain of a semicontinuous var should reduce to a single
        // point (constant) and be different from the 'on' domain.
        if scip.set.is_eq(lb0, constant) && (!scip.set.is_eq(lb0, lb1) || !scip.set.is_eq(ub0, ub1)) {
            add_sc_var_indicator(scip, scvdata.get_or_insert_with(ScVarData::default), bvar, lb0, lb1, ub1);
        }
    }

    // Look for vubvars that have not been processed yet.
    for c in 0..nvubs {
        if scip.vars[vubvars[c]].vartype != VarType::Binary || crate::var::var_is_implied_integral(scip, vubvars[c]) {
            continue;
        }

        let bvar = vubvars[c];

        // Skip vars that are in vlbvars.
        if v.vlbs.is_some() && find(&vlbvars[..nvlbs], bvar).is_ok() {
            continue;
        }

        let lb0 = glb;
        let lb1 = glb;
        let ub0 = crate::def::min(vubconstants[c], gub);
        let ub1 = crate::def::min(vubconstants[c] + vubcoefs[c], gub);

        // The 'off' domain of a semicontinuous var should reduce to a single
        // point (constant) and be different from the 'on' domain.
        if scip.set.is_eq(lb0, constant) && (!scip.set.is_eq(lb0, lb1) || !scip.set.is_eq(ub0, ub1)) {
            add_sc_var_indicator(scip, scvdata.get_or_insert_with(ScVarData::default), bvar, lb0, lb1, ub1);
        }
    }

    match scvdata {
        Some(d) => {
            heur_scvars.insert(var, d);
            true
        }
        None => false,
    }
}

/// `varIsSemicontinuous` on the heuristic's scvars map.
fn heur_var_is_semicontinuous(scip: &mut Scip, heur: HeurId, var: VarId, constant: f64) -> bool {
    let mut scvars = hdm(scip, heur).scvars.take().unwrap();
    let result = var_is_semicontinuous(scip, &mut scvars, var, constant);
    hdm(scip, heur).scvars = Some(scvars);
    result
}

/// `hasUnfixedSCIndicator`: checks if there are unfixed indicator variables
/// modeling semicont. vars.
fn has_unfixed_sc_indicator(scip: &mut Scip, heur: HeurId, conshdlr: ConshdlrId) -> ScipResult<bool> {
    let mut hasunfixedscindconss = false;

    let h = &scip.conshdlrs[conshdlr];
    let indicatorconss = h.conss.0[..h.nconss as usize].to_vec();

    for &cons in &indicatorconss {
        let binvar = crate::cons_indicator::get_binary_var_indicator(scip, cons);

        // Check if indicator variable is unfixed.
        if scip.vars[binvar].locdom.lb < scip.vars[binvar].locdom.ub - 0.5 {
            let lincons = crate::cons_indicator::get_linear_cons_indicator(scip, cons);
            let (rhs, _) = crate::misc_linear::cons_get_rhs(scip, lincons);
            let (nconsvars, _) = crate::cons::cons_get_n_vars(scip, lincons)?;

            // Check if constraint contains only two variables with finite rhs.
            // TODO: allow also indicators for lower bounds.
            if nconsvars == 2 && !scip.set.is_infinity(rhs) {
                let (consvars, _) = crate::cons::cons_get_vars(scip, lincons)?;
                let (_consvals, _) = crate::misc_linear::get_cons_vals(scip, lincons)?;

                for &consvar in &consvars[..nconsvars as usize] {
                    // Note that we have exact two variables.
                    if consvar == crate::cons_indicator::get_slack_var_indicator(scip, cons) {
                        continue;
                    }

                    let semicontinuousvar = consvar;

                    // Check if semicontinuous variable.
                    if heur_var_is_semicontinuous(scip, heur, semicontinuousvar, rhs) {
                        hasunfixedscindconss = true;
                        break;
                    }
                }
                if hasunfixedscindconss {
                    break;
                }
            }
        }
    }

    Ok(hasunfixedscindconss)
}

/// `createMaps`: creates and initializes hashmaps.
///
/// indicatormap: binary var -> indicator constraint
/// varboundmap: binary var -> varbound constraint
///
/// Currently exactly one constraint is assigned to a binary variable (per
/// hashmap), but a binary variable can also control more than one
/// constraint.
fn create_maps(scip: &mut Scip, heur: HeurId) {
    let (indicatorconshdlr, varboundconshdlr, usevarbounds) = {
        let d = hd(scip, heur);
        (d.indicatorconshdlr, d.varboundconshdlr, d.usevarbounds)
    };

    // Indicator constraints.
    let h = &scip.conshdlrs[indicatorconshdlr];
    let conss = h.conss.0[..h.nconss as usize].to_vec();
    let mut indicatormap = HashMap::with_capacity(conss.len());
    for &cons in &conss {
        indicatormap.entry(crate::cons_indicator::get_binary_var_indicator(scip, cons)).or_insert(cons);
    }

    // Varbound constraints.
    let mut varboundmap = None;
    if usevarbounds {
        let h = &scip.conshdlrs[varboundconshdlr];
        let conss = h.conss.0[..h.nconss as usize].to_vec();
        let mut map = HashMap::with_capacity(conss.len());
        for &cons in &conss {
            map.entry(crate::cons_varbound::get_vbdvar_varbound(scip, cons)).or_insert(cons);
        }
        varboundmap = Some(map);
    }

    let d = hdm(scip, heur);
    d.indicatormap = Some(indicatormap);
    d.varboundmap = varboundmap;
}

/// `getScoreOfFarkasDiving`: calculate score and preferred rounding
/// direction for the candidate variable; returns `(roundup, score)`.
fn get_score_of_farkas_diving(scip: &mut Scip, diveset: DivesetId, cand: VarId, candsfrac: f64) -> (bool, f64) {
    let obj = scip.vars[cand].obj;

    // Dive towards the pseudosolution, at the same time approximate the
    // contribution to a potential Farkas-proof (infeasibility proof) by y^TA_i =
    // c_i.
    let roundup = if scip.set.is_negative(obj) {
        true
    } else if scip.set.is_positive(obj) {
        false
    } else if scip.set.is_eq(candsfrac, 0.5) {
        scip.divesets[diveset].randnumgen.get_int(0, 1) == 0
    } else {
        candsfrac > 0.5
    };

    // Larger score is better.
    let mut score = obj.abs() + scip.divesets[diveset].randnumgen.get_real(MIN_RAND, MAX_RAND);

    // Prefer decisions on binary variables.
    if scip.vars[cand].vartype != VarType::Binary || crate::var::var_is_implied_integral(scip, cand) {
        score = -1.0 / score;
    }

    (roundup, score)
}

/*
 * Callback methods
 */

/// `heurCopyIndicatordiving`: copy method for primal heuristic plugins
/// (called when SCIP copies plugins).
fn heur_copy_indicatordiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    // Call inclusion method of primal heuristic.
    include_heur_indicatordiving(scip)
}

/// `heurFreeIndicatordiving`: destructor of primal heuristic to free user
/// data (called when SCIP is exiting).
fn heur_free_indicatordiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    scip.heurs[heur].heurdata = None;
    Ok(())
}

/// `heurInitIndicatordiving`: initialization method of primal heuristic
/// (called after problem was transformed).
fn heur_init_indicatordiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working data.
    let sol = api::create_sol(scip, heur)?;
    let nvars = api::get_n_vars(scip) as usize;
    let indicatorconshdlr = crate::set::set_find_conshdlr(scip, "indicator");
    let varboundconshdlr = crate::set::set_find_conshdlr(scip, "varbound");

    let d = hdm(scip, heur);
    d.sol = sol;
    d.scvars = Some(HashMap::with_capacity(nvars));
    d.indicatorconshdlr = indicatorconshdlr;
    d.varboundconshdlr = varboundconshdlr;

    Ok(())
}

/// `heurExitIndicatordiving`: deinitialization method of primal heuristic
/// (called before transformed problem is freed).
fn heur_exit_indicatordiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working data.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    let d = hdm(scip, heur);
    d.sol = NONE;
    d.scvars = None;

    Ok(())
}

/// `heurExecIndicatordiving`: execution method of primal heuristic.
fn heur_exec_indicatordiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    let diveset = scip.heurs[heur].divesets[0];

    *result = ScipResultCode::DidNotRun;

    // Check if there are unfixed indicator variables modeling semicont. vars.
    let indicatorconshdlr = hd(scip, heur).indicatorconshdlr;
    let hasunfixedscindconss = has_unfixed_sc_indicator(scip, heur, indicatorconshdlr)?;

    // Skip heuristic if problem doesn't contain unfixed indicator variables, or
    // if there are no varbound constraints which should be considered.
    let (runwithoutscinds, usevarbounds, varboundconshdlr) = (hd(scip, heur).runwithoutscinds, hd(scip, heur).usevarbounds, hd(scip, heur).varboundconshdlr);
    if !hasunfixedscindconss && (!runwithoutscinds || !usevarbounds || scip.conshdlrs[varboundconshdlr].nconss == 0) {
        return Ok(());
    }

    // Create and initialize hashmaps.
    create_maps(scip, heur);

    // (Re-)set flags.
    let d = hdm(scip, heur);
    d.gotoindconss = false;
    d.containsviolindconss = false;
    d.newnode = true;
    d.probingdepth = -1;

    let sol = d.sol;
    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)?;

    // Free hashmaps since constraints can get removed/modified till the next
    // call.
    let d = hdm(scip, heur);
    d.varboundmap = None;
    d.indicatormap = None;

    Ok(())
}

/// `divesetGetScoreIndicatordiving`: calculate score and preferred rounding
/// direction for the candidate variable.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_indicatordiving(
    scip: &mut Scip,
    diveset: DivesetId,
    _divetype: DiveType,
    cand: VarId,
    candsol: f64,
    candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let mut varboundcons = NONE;
    let mut semicontinuousvar;
    let mut lpsolsemicontinuous = 0.0;
    let mut idxbvars: i32 = -1;
    let mut isvbdvar = false;
    let mut issemicont = true;
    let fixconstant;

    let heur = scip.divesets[diveset].heur;

    // Check if we are at a new probing node; since diving heuristics backtrack
    // at most one probing node, we are at a new node iff the probing depth
    // increased.
    let probingdepth = crate::scip_probing::get_probing_depth(scip);
    {
        let d = hdm(scip, heur);
        d.newnode = d.probingdepth < probingdepth;
        d.probingdepth = probingdepth;
    }

    // Skip if current candidate can not be determined by the indicator
    // constraint handler and violated indicator constraints still exists.
    let determined = scip.set.is_feas_integral(candsol) && scip.vars[cand].locdom.lb < scip.vars[cand].locdom.ub - 0.5;
    if !determined && hd(scip, heur).gotoindconss {
        *score = crate::def::REAL_MIN;
        *roundup = false;
        return Ok(());
    } else {
        hdm(scip, heur).gotoindconss = false;
    }

    // Check if candidate variable is indicator variable.
    let (indicatorcons, isindicatorvar) = check_and_get_indicator(scip, heur, cand);

    // Skip candidate in next calls since we have violated indicator constraints
    // but current candidate is not determined by the indicator constraint
    // handler.
    let containsviolindconss = hd(scip, heur).containsviolindconss;
    if containsviolindconss && !(determined && isindicatorvar) {
        hdm(scip, heur).gotoindconss = true;
        *score = crate::def::REAL_MIN;
        *roundup = false;
        return Ok(());
    }

    // Check if candidate variable is bounding variable.
    let usevarbounds = hd(scip, heur).usevarbounds;
    if usevarbounds && !isindicatorvar {
        (varboundcons, isvbdvar) = check_and_get_varbound(scip, heur, cand);
    }

    // Return
    // - if candidate variable is neither a indicator variable nor a variable
    //   bounding variable
    // - or if candidate variable is not an indicator variable but there will be
    //   indicator variables as candidates
    // - or if candidate variable is not an indicator variable and varbound
    //   constraints are not considered.
    if !isindicatorvar && (!isvbdvar || containsviolindconss || !usevarbounds) {
        *score = crate::def::REAL_MIN;
        *roundup = false;

        if !containsviolindconss && !isvbdvar {
            (*roundup, *score) = get_score_of_farkas_diving(scip, diveset, cand, candsfrac);
            *score = (*score / (100.0 + score.abs())) * 100.0 - 200.0; // scale to [-300,-100]
        }
        return Ok(());
    }

    let lincons;
    let nonoptionvar;
    let side;
    if isindicatorvar {
        // Prefer indicator constraint.
        lincons = crate::cons_indicator::get_linear_cons_indicator(scip, indicatorcons);
        nonoptionvar = crate::cons_indicator::get_slack_var_indicator(scip, indicatorcons);
        let (rhs, _) = crate::misc_linear::cons_get_rhs(scip, lincons);
        // TODO: allow also indicators for lower bounds.
        issemicont = scip.set.is_infinity(-crate::misc_linear::cons_get_lhs(scip, lincons).0);
        side = rhs;
    } else {
        lincons = varboundcons;
        nonoptionvar = crate::cons_varbound::get_vbdvar_varbound(scip, varboundcons);
        let (rhs, _) = crate::misc_linear::cons_get_rhs(scip, lincons);
        let (lhs, _) = crate::misc_linear::cons_get_lhs(scip, lincons);
        side = if scip.set.is_infinity(rhs) { lhs } else { rhs };
    }

    let (nconsvars, _) = crate::cons::cons_get_n_vars(scip, lincons)?;

    if nconsvars != 2 || !issemicont {
        (*roundup, *score) = get_score_of_farkas_diving(scip, diveset, cand, candsfrac);
        *score = (*score / (100.0 + score.abs())) * 100.0 - 200.0; // scale to [-300,-100]
        return Ok(());
    }

    let (consvars, _) = crate::cons::cons_get_vars(scip, lincons)?;
    let (_consvals, _) = crate::misc_linear::get_cons_vals(scip, lincons)?;

    issemicont = false;
    let mut scvar = NONE;
    for &consvar in &consvars[..nconsvars as usize] {
        // Note that we have exact two variables.
        if consvar == nonoptionvar {
            continue;
        }

        semicontinuousvar = consvar;
        lpsolsemicontinuous = crate::var::var_get_lp_sol(scip, semicontinuousvar);

        // Only allow semicontinuous variables.
        if heur_var_is_semicontinuous(scip, heur, semicontinuousvar, side) {
            scvar = semicontinuousvar;
            let scdata = &hd(scip, heur).scvars.as_ref().unwrap()[&semicontinuousvar];
            for b in 0..scdata.nbnds as usize {
                if (scdata.bvars[b] == cand || (crate::var::var_is_negated(scip, cand) && scdata.bvars[0] == scip.vars[cand].negatedvar)) && scip.set.is_eq(side, scdata.vals0[b]) {
                    // TODO: handle also more general variables; currently we handle
                    // only variables with domain vals0 < lb1 <= ub1.
                    if scip.set.is_ge(lpsolsemicontinuous, scdata.vals0[b]) && scip.set.is_le(lpsolsemicontinuous, scdata.ubs1[b]) {
                        issemicont = true;
                        idxbvars = b as i32;
                        break;
                    }
                }
            }
        }
    }

    // Only continue if semicontinuous variable.
    if !issemicont {
        (*roundup, *score) = get_score_of_farkas_diving(scip, diveset, cand, candsfrac);
        *score = (*score / (100.0 + score.abs())) * 100.0 - 200.0; // scale to [-300,-100]
        return Ok(());
    }

    // The scdata of the variable that left the loop (C keeps the pointer of
    // the last semicontinuous variable).
    let (vals0, lbs1, ubs1) = {
        let scdata = &hd(scip, heur).scvars.as_ref().unwrap()[&scvar];
        let b = idxbvars as usize;
        (scdata.vals0[b], scdata.lbs1[b], scdata.ubs1[b])
    };
    let (roundingfrac, roundingmode, semicontscoremode) = (hd(scip, heur).roundingfrac, hd(scip, heur).roundingmode, hd(scip, heur).semicontscoremode);

    // Case: Variable is in range [lb1,ub1].
    if scip.set.is_ge(lpsolsemicontinuous, lbs1) && scip.set.is_le(lpsolsemicontinuous, ubs1) {
        *score = scip.divesets[diveset].randnumgen.get_real(-1.0, 0.0);
        fixconstant = false;
    }
    // Case: Variable is equal to constant.
    else if scip.set.is_eq(lpsolsemicontinuous, vals0) {
        *score = scip.divesets[diveset].randnumgen.get_real(-1.0, 0.0);
        fixconstant = true;
    }
    // Case: Variable is between constant and lb1.
    else {
        let mut shiftedlpsolsemicontinuous = lpsolsemicontinuous;
        let mut shiftedlbs1 = lbs1;

        // Handle case if constant of semicont. var is not zero -> shift values.
        if !scip.set.is_zero(vals0) {
            shiftedlpsolsemicontinuous -= vals0;
            shiftedlbs1 -= vals0;
        }

        *score = 100.0 * (shiftedlbs1 - shiftedlpsolsemicontinuous) / shiftedlbs1;

        fixconstant = match roundingmode {
            ROUNDING_CONSERVATIVE => *score > (1.0 - roundingfrac) * 100.0,
            ROUNDING_AGGRESSIVE => *score <= (1.0 - roundingfrac) * 100.0,
            _ => return Err(Retcode::InvalidData),
        };

        match semicontscoremode {
            0 => {}
            1 => {
                if shiftedlpsolsemicontinuous < shiftedlbs1 * roundingfrac {
                    *score = 100.0 * (shiftedlpsolsemicontinuous / (roundingfrac * shiftedlbs1));
                } else {
                    *score = 100.0 * (-shiftedlpsolsemicontinuous / ((1.0 - roundingfrac) * shiftedlbs1) + (1.0 / (1.0 - roundingfrac)));
                }
            }
            2 => *score = 100.0 - *score,
            _ => return Err(Retcode::InvalidData),
        }
    }

    // Set roundup depending on whether we have an indicator constraint or a
    // varbound constraint:
    // - indicator constraint: roundup == fix to constant
    // - varbound constraint: roundup == push to range
    *roundup = if isindicatorvar { fixconstant } else { !fixconstant };

    Ok(())
}

/// `divesetAvailableIndicatordiving`: callback to check preconditions for
/// diving, e.g., if an incumbent solution is available.
fn diveset_available_indicatordiving(scip: &mut Scip, diveset: DivesetId, available: &mut bool) -> ScipResult {
    // Skip if problem doesn't contain indicator constraints. If varbound
    // constraints should be considered, skip only if there are also no
    // varbound constraints.
    let indicator = crate::set::set_find_conshdlr(scip, "indicator");
    *available = scip.conshdlrs[indicator].nactiveconss == 0;

    if !*available {
        let heur = scip.divesets[diveset].heur;
        if hd(scip, heur).runwithoutscinds && hd(scip, heur).usevarbounds {
            let varbound = crate::set::set_find_conshdlr(scip, "varbound");
            *available = scip.conshdlrs[varbound].nactiveconss == 0;
        }
    }

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurIndicatordiving`: creates the indicatordiving heuristic
/// and includes it in SCIP.
pub fn include_heur_indicatordiving(scip: &mut Scip) -> ScipResult {
    // Create indicatordiving primal heuristic data (C allocates it
    // uninitialized; the parameters set the rest).
    let heurdata = HeurData {
        sol: NONE,
        indicatorconshdlr: NONE,
        varboundconshdlr: NONE,
        scvars: None,
        indicatormap: None,
        varboundmap: None,
        roundingfrac: 0.0,
        roundingmode: 0,
        semicontscoremode: 0,
        usevarbounds: false,
        runwithoutscinds: false,
        gotoindconss: false,
        containsviolindconss: false,
        newnode: false,
        probingdepth: 0,
    };

    // Include primal heuristic.
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
        heur_exec_indicatordiving,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non fundamental callbacks via setter functions.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_indicatordiving);
    h.heurfree = Some(heur_free_indicatordiving);
    h.heurinit = Some(heur_init_indicatordiving);
    h.heurexit = Some(heur_exit_indicatordiving);

    // Create a diveset (this will automatically install some additional
    // parameters for the heuristic).
    crate::heur::diveset_create(
        scip,
        heur,
        Some(HEUR_NAME),
        DEFAULT_MINRELDEPTH,
        DEFAULT_MAXRELDEPTH,
        DEFAULT_MAXLPITERQUOT,
        DEFAULT_MAXDIVEUBQUOT,
        DEFAULT_MAXDIVEAVGQUOT,
        DEFAULT_MAXDIVEUBQUOTNOSOL,
        DEFAULT_MAXDIVEAVGQUOTNOSOL,
        DEFAULT_LPRESOLVEDOMCHGQUOT,
        DEFAULT_LPSOLVEFREQ,
        DEFAULT_MAXLPITEROFS,
        DEFAULT_RANDSEED,
        DEFAULT_BACKTRACK,
        DEFAULT_ONLYLPBRANCHCANDS,
        DIVESET_ISPUBLIC,
        DIVESET_DIVETYPES,
        diveset_get_score_indicatordiving,
        Some(diveset_available_indicatordiving),
    )?;

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};
    paramset_add_real(
        scip,
        &format!("heuristics/{HEUR_NAME}/roundingfrac"),
        "in violation case all fractional below this value are fixed to constant",
        t!(f64, roundingfrac),
        false,
        DEFAULT_ROUNDINGFRAC,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("heuristics/{HEUR_NAME}/roundingmode"),
        "decides which roundingmode is selected (0: conservative, 1: aggressive)",
        t!(i32, roundingmode),
        false,
        DEFAULT_ROUNDINGMODE,
        0,
        1,
        None,
    )?;
    paramset_add_int(
        scip,
        &format!("heuristics/{HEUR_NAME}/semicontscoremode"),
        "which values of semi-continuous variables should get a high score? (0: low, 1: middle, 2: high)",
        t!(i32, semicontscoremode),
        false,
        DEFAULT_SEMICONTSCOREMODE,
        0,
        2,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("heuristics/{HEUR_NAME}/usevarbounds"),
        "should varbound constraints be considered?",
        t!(bool, usevarbounds),
        false,
        DEFAULT_USEVARBOUNDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        &format!("heuristics/{HEUR_NAME}/runwithoutscinds"),
        "should heur run if there are no indicator constraints modeling semicont. vars?",
        t!(bool, runwithoutscinds),
        false,
        DEFAULT_RUNWITHOUTSCINDS,
        None,
    )?;

    Ok(())
}
