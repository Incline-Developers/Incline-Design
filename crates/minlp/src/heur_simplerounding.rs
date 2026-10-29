//! Port of `heur_simplerounding.c`: simple and fast LP rounding heuristic.
//!
//! Rounds every fractional variable in a direction that cannot violate a
//! constraint (fewer locks); also rounds relaxation solutions if available.

use std::any::Any;

use crate::{
    api,
    heur::{HEURTIMING_AFTERLPNODE, HEURTIMING_DURINGLPLOOP, HEURTIMING_DURINGPRICINGLOOP, HeurTiming},
    prob::TRANSPROB,
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, LpSolStat, ScipResultCode, SolId, VarId},
    var::{NONE, var_is_implied_integral, var_may_round_down, var_may_round_up},
};

const HEUR_NAME: &str = "simplerounding";
const HEUR_DESC: &str = "simple and fast LP rounding heuristic";
const HEUR_DISPCHAR: u8 = b'r';
const HEUR_PRIORITY: i32 = -30;
const HEUR_FREQ: i32 = 1;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_DURINGLPLOOP | HEURTIMING_DURINGPRICINGLOOP;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// Should the heuristic only be called once per node?
const DEFAULT_ONCEPERNODE: bool = false;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: SolId,
    /// Last LP number where the heuristic was applied.
    lastlp: i64,
    /// Number of variables that can be rounded (-1 if not yet calculated).
    nroundablevars: i32,
    /// Should the heuristic only be called once per node?
    oncepernode: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

/*
 * Local methods
 */

/// `performSimpleRounding`: perform rounding.
fn perform_simple_rounding(scip: &mut Scip, sol: SolId, cands: &[VarId], candssol: &[f64], ncands: usize, result: &mut ScipResultCode) -> ScipResult {
    let mut nunroundableimplints = 0;

    // Round all roundable fractional variables as long as no unroundable
    // variable was found.
    let mut c = 0;
    while c < ncands {
        let oldsolval = candssol[c];
        let var = cands[c];
        let mayrounddown = var_may_round_down(scip, var);
        let mayroundup = var_may_round_up(scip, var);

        // Choose rounding direction.
        let newsolval = if mayrounddown && mayroundup {
            // We can round in both directions: round in objective function
            // direction.
            if scip.vars[var].obj >= 0.0 {
                scip.set.feas_floor(oldsolval)
            } else {
                scip.set.feas_ceil(oldsolval)
            }
        } else if mayrounddown {
            scip.set.feas_floor(oldsolval)
        } else if mayroundup {
            scip.set.feas_ceil(oldsolval)
        } else if var_is_implied_integral(scip, var) {
            nunroundableimplints += 1;
            c += 1;
            continue;
        } else {
            break;
        };

        // Store new solution value.
        api::set_sol_val(scip, sol, var, newsolval)?;
        c += 1;
    }

    // Check, if rounding was successful.
    if c == ncands {
        // Unroundable implied integral variables are adjusted, LP rows must
        // be checked afterwards.
        let checklprows = if nunroundableimplints > 0 {
            crate::sol::sol_adjust_implicit_sol_vals(scip, sol, TRANSPROB, true)?;
            true
        } else {
            false
        };

        let stored = if crate::prob::prob_all_cols_in_lp(scip, TRANSPROB) {
            // Check solution for feasibility, and add it to solution store if
            // possible; integrality need not be checked, because all
            // fractional variables were already moved in feasible direction
            // to the next integer.
            api::try_sol(scip, sol, false, false, false, false, checklprows)?
        } else {
            // If there are variables which are not present in the LP, e.g.,
            // for column generation, we need to check their bounds.
            api::try_sol(scip, sol, false, false, true, false, checklprows)?
        };

        if stored {
            *result = ScipResultCode::FoundSol;
        }
    }
    Ok(())
}

/// `performLPSimpleRounding`: perform LP-rounding.
fn perform_lp_simple_rounding(scip: &mut Scip, heur: HeurId, heurtiming: HeurTiming, result: &mut ScipResultCode) -> ScipResult {
    // Only call heuristic, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only call heuristic, if the LP objective value is smaller than the
    // cutoff bound.
    let lpobj = api::get_lp_objval(scip);
    if scip.set.is_ge(lpobj, api::get_cutoffbound(scip)) {
        return Ok(());
    }

    // Get fractional variables, that should be integral.
    let (lpcands, lpcandssol, _, nlpcands, _, nfracimplvars) = api::get_lp_branch_cands(scip)?;

    // Only call heuristic, if LP solution is fractional; except we are called
    // during pricing, in this case we want to detect a (mixed) integer (LP)
    // solution which is primal feasible.
    if nlpcands == 0 && heurtiming != HEURTIMING_DURINGPRICINGLOOP {
        return Ok(());
    }

    // Don't call heuristic, if there are more fractional variables than
    // roundable ones. We do not consider fractional continuous implied
    // integer variables here, because simple rounding may adjust those
    // separately, even if they aren't roundable.
    if nlpcands > hd(scip, heur).nroundablevars {
        return Ok(());
    }

    // Get the working solution from heuristic's local data.
    let sol = hd(scip, heur).sol;

    // Copy the current LP solution to the working solution.
    crate::sol::sol_link_lp_sol(scip, sol, TRANSPROB)?;

    // Don't call heuristic, if we have already processed the current LP
    // solution.
    let nlps = scip.stat.nlps;
    if nlps == hd(scip, heur).lastlp {
        return Ok(());
    }
    hdm(scip, heur).lastlp = nlps;

    // Perform simple rounding.
    perform_simple_rounding(scip, sol, &lpcands, &lpcandssol, (nlpcands + nfracimplvars) as usize, result)
}

/// `performRelaxSimpleRounding`: perform relaxation solution rounding.
fn perform_relax_simple_rounding(scip: &mut Scip, heur: HeurId, result: &mut ScipResultCode) -> ScipResult {
    // Do not call heuristic if no relaxation solution is available.
    if !crate::relax::relaxation_is_sol_valid(scip) {
        return Ok(());
    }

    // Get integral variables.
    let vars = api::get_vars(scip).to_vec();
    let nintegervars = (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip)) as usize;
    let ndiscretevars = nintegervars + api::get_n_impl_vars(scip) as usize;

    let mut relaxcands: Vec<VarId> = Vec::with_capacity(ndiscretevars);
    let mut relaxcandssol: Vec<f64> = Vec::with_capacity(ndiscretevars);

    // Get fractional non-implied integral variables.
    for &var in &vars[..nintegervars] {
        let val = crate::var::var_get_relax_sol(scip, var);
        if !scip.set.is_feas_integral(val) {
            relaxcands.push(var);
            relaxcandssol.push(val);
        }
    }

    // Don't call heuristic, if there are more fractional variables than
    // roundable ones. We explicitly do not consider implied integral
    // variables with fractional relaxation solution here because they may be
    // feasibly adjusted, although they are not roundable.
    if relaxcands.len() as i32 > hd(scip, heur).nroundablevars {
        return Ok(());
    }

    // Collect implied integral variables with fractional solution value.
    for &var in &vars[nintegervars..ndiscretevars] {
        let val = crate::var::var_get_relax_sol(scip, var);
        if !scip.set.is_feas_integral(val) {
            relaxcands.push(var);
            relaxcandssol.push(val);
        }
    }

    // Get the working solution from heuristic's local data.
    let sol = hd(scip, heur).sol;

    // Copy the current relaxation solution to the working solution.
    crate::sol::sol_link_relax_sol(scip, sol);

    // Perform simple rounding.
    let n = relaxcands.len();
    perform_simple_rounding(scip, sol, &relaxcands, &relaxcandssol, n, result)
}

/*
 * Callback methods
 */

/// `heurCopySimplerounding`: copy method for primal heuristic plugins.
fn heur_copy_simplerounding(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_simplerounding(scip)
}

/// `heurInitSimplerounding`: initialization method of primal heuristic.
fn heur_init_simplerounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create heuristic data.
    let sol = api::create_sol(scip, heur)?;
    let h = hdm(scip, heur);
    h.sol = sol;
    h.lastlp = -1;
    h.nroundablevars = -1;
    Ok(())
}

/// `heurExitSimplerounding`: deinitialization method of primal heuristic.
fn heur_exit_simplerounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free heuristic data.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `heurInitsolSimplerounding`: solving process initialization method of
/// primal heuristic.
fn heur_initsol_simplerounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    hdm(scip, heur).lastlp = -1;

    // Change the heuristic's timingmask, if it should be called only once per
    // node.
    if hd(scip, heur).oncepernode {
        scip.heurs[heur].timingmask = HEURTIMING_AFTERLPNODE;
    }
    Ok(())
}

/// `heurExitsolSimplerounding`: solving process deinitialization method of
/// primal heuristic.
fn heur_exitsol_simplerounding(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Reset the timing mask to its default value.
    scip.heurs[heur].timingmask = HEUR_TIMING;
    Ok(())
}

/// `heurExecSimplerounding`: execution method of primal heuristic.
fn heur_exec_simplerounding(scip: &mut Scip, heur: HeurId, heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Get nonimplied integral variable number.
    let nbinintvars = (api::get_n_bin_vars(scip) + api::get_n_int_vars(scip)) as usize;

    // Only call heuristic if there are integral variables.
    if nbinintvars == 0 {
        return Ok(());
    }

    // Only call heuristic if there are roundable variables; except we are
    // called during pricing, in this case we want to detect a (mixed) integer
    // (LP) solution which is primal feasible.
    if heurtiming != HEURTIMING_DURINGPRICINGLOOP && hd(scip, heur).nroundablevars == 0 {
        return Ok(());
    }

    // Only call heuristic if an optimal LP solution or a relaxation solution
    // is available.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal && !crate::relax::relaxation_is_sol_valid(scip) {
        return Ok(());
    }

    // Only call heuristic if the LP objective value is smaller than the
    // cutoff bound.
    let lpobj = api::get_lp_objval(scip);
    if scip.set.is_ge(lpobj, api::get_cutoffbound(scip)) {
        return Ok(());
    }

    // Only call heuristic if the current LP solution is not already processed
    // or a relaxation solution is available.
    if scip.stat.nlps == hd(scip, heur).lastlp && !crate::relax::relaxation_is_sol_valid(scip) {
        return Ok(());
    }

    // On our first call or after each pricing round, calculate the number of
    // roundable variables.
    if heurtiming == HEURTIMING_DURINGPRICINGLOOP || hd(scip, heur).nroundablevars == -1 {
        let vars = api::get_vars(scip);
        let n = vars[..nbinintvars].iter().filter(|&&v| var_may_round_down(scip, v) || var_may_round_up(scip, v)).count() as i32;
        hdm(scip, heur).nroundablevars = n;

        // Only call heuristic if there are roundable variables; except we are
        // called during pricing.
        if heurtiming != HEURTIMING_DURINGPRICINGLOOP && n == 0 {
            return Ok(());
        }
    }

    *result = ScipResultCode::DidNotFind;

    // Try to round LP solution.
    perform_lp_simple_rounding(scip, heur, heurtiming, result)?;

    // Try to round relaxation solution.
    perform_relax_simple_rounding(scip, heur, result)
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurSimplerounding`: creates the simple rounding heuristic
/// and includes it in SCIP.
pub fn include_heur_simplerounding(scip: &mut Scip) -> ScipResult {
    // Create heuristic data.
    let heurdata = HeurData {
        sol: NONE,
        lastlp: -1,
        nroundablevars: -1,
        oncepernode: DEFAULT_ONCEPERNODE,
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
        heur_exec_simplerounding,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_simplerounding);
    h.heurinit = Some(heur_init_simplerounding);
    h.heurexit = Some(heur_exit_simplerounding);
    h.heurinitsol = Some(heur_initsol_simplerounding);
    h.heurexitsol = Some(heur_exitsol_simplerounding);

    crate::paramset::paramset_add_bool(
        scip,
        "heuristics/simplerounding/oncepernode",
        "should the heuristic only be called once per node?",
        crate::data_target!(bool, heurs, heur, heurdata, HeurData, oncepernode),
        true,
        DEFAULT_ONCEPERNODE,
        None,
    )?;

    Ok(())
}
