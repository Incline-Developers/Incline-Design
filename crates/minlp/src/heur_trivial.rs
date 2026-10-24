//! Port of `heur_trivial.c`: start heuristic which tries some trivial
//! solutions (all variables at zero, at their lower bounds, at their upper
//! bounds, and at the bound with fewer locks).

use crate::{
    api,
    heur::{HEURTIMING_BEFORENODE, HEURTIMING_BEFOREPRESOL, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{HeurId, LockType, ScipResultCode},
    var::{var_get_n_locks_down_type, var_get_n_locks_up_type, var_is_integral},
};

const HEUR_NAME: &str = "trivial";
const HEUR_DESC: &str = "start heuristic which tries some trivial solutions";
const HEUR_DISPCHAR: u8 = b't';
const HEUR_PRIORITY: i32 = 10000;
const HEUR_FREQ: i32 = 0;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_BEFOREPRESOL | HEURTIMING_BEFORENODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;

/// `heurCopyTrivial`: copy method for primal heuristic plugins.
fn heur_copy_trivial(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_trivial(scip)
}

/// `heurExecTrivial`: execution method of primal heuristic.
fn heur_exec_trivial(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, _nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    // Initialize data structure.
    let mut zerosol = api::create_sol(scip, heur)?;
    let mut lbsol = api::create_sol(scip, heur)?;
    let mut ubsol = api::create_sol(scip, heur)?;
    let mut locksol = api::create_sol(scip, heur)?;

    let ret = (|| -> ScipResult {
        // Determine large value to set variables to.
        let large = scip.set.round(crate::def::min(1.0 / scip.set.feastol(), scip.set.get_huge_value()) / 10.0);

        // Check zero solution once.
        let mut difflb = false;
        let mut diffub = false;
        let mut difflock = false;

        let vars = api::get_vars(scip).to_vec();

        for (i, &var) in vars.iter().enumerate() {
            let mut lb = scip.vars[var].locdom.lb;
            let mut ub = scip.vars[var].locdom.ub;

            // If problem is obviously infeasible due to empty domain, stop.
            if scip.set.is_feas_gt(lb, ub) {
                return Ok(());
            }

            // Set bounds to sufficient large value.
            if scip.set.is_infinity(-lb) {
                lb = crate::def::min(-large, ub);
            }
            if scip.set.is_infinity(ub) {
                ub = crate::def::max(large, lb);
            }

            // Set value next to zero within bounds.
            let zeroval = crate::def::max(crate::def::min(0.0, ub), lb);

            // Set value to the bound with fewer locks, if tie choose an
            // average value.
            let nlocksdown = var_get_n_locks_down_type(scip, var, LockType::Model);
            let nlocksup = var_get_n_locks_up_type(scip, var, LockType::Model);
            let solval = if nlocksdown < nlocksup {
                lb
            } else if nlocksdown > nlocksup {
                ub
            } else {
                let mut solval = (lb + ub) / 2.0;

                // If a tie occurs, roughly every third integer variable will
                // be rounded up.
                if var_is_integral(scip, var) {
                    solval = if i % 3 == 0 { scip.set.ceil(solval) } else { scip.set.floor(solval) };
                }
                solval
            };

            if !scip.set.is_eq(lb, zeroval) {
                difflb = true;
            }

            if !scip.set.is_eq(ub, zeroval) {
                diffub = true;
            }

            if !scip.set.is_eq(solval, zeroval) {
                difflock = true;
            }

            // Set variable to values.
            api::set_sol_val(scip, zerosol, var, zeroval)?;
            api::set_sol_val(scip, lbsol, var, lb)?;
            api::set_sol_val(scip, ubsol, var, ub)?;
            api::set_sol_val(scip, locksol, var, solval)?;
        }

        // Try zero solution.
        if api::try_sol(scip, zerosol, false, false, false, false, true)? {
            *result = ScipResultCode::FoundSol;
        }

        // Try lower bound solution.
        if difflb && api::try_sol(scip, lbsol, false, false, false, false, true)? {
            *result = ScipResultCode::FoundSol;
        }

        // Try upper bound solution.
        if diffub && api::try_sol(scip, ubsol, false, false, false, false, true)? {
            *result = ScipResultCode::FoundSol;
        }

        // Try lock solution.
        if difflock && api::try_sol(scip, locksol, false, false, false, false, true)? {
            *result = ScipResultCode::FoundSol;
        }

        Ok(())
    })();

    // TERMINATE: free solutions.
    api::free_sol(scip, locksol)?;
    api::free_sol(scip, ubsol)?;
    api::free_sol(scip, lbsol)?;
    api::free_sol(scip, zerosol)?;
    let _ = (&mut zerosol, &mut lbsol, &mut ubsol, &mut locksol);

    ret
}

/// `SCIPincludeHeurTrivial`: creates the trivial primal heuristic and
/// includes it in SCIP.
pub fn include_heur_trivial(scip: &mut Scip) -> ScipResult {
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
        heur_exec_trivial,
        None,
    )?;

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    scip.heurs[heur].heurcopy = Some(heur_copy_trivial);

    Ok(())
}
