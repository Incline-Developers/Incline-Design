//! Port of `sepa_disjunctive.c`: disjunctive cut separator.
//!
//! It separates disjunctive cuts on the conflict graph of SOS1
//! constraints, which this port does not have: the SOS1 handler is never
//! found, so the separator returns before its cuts. It is included
//! nonetheless because it is delayed, and a delayed separator changes the
//! separation loop whether or not it finds anything.

use std::any::Any;

use crate::{
    api,
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    retcode::ScipResult,
    scip::Scip,
    types::{ConshdlrId, LpSolStat, ScipResultCode, SepaId},
    var::NONE,
};

const SEPA_NAME: &str = "disjunctive";
const SEPA_DESC: &str = "disjunctive cut separator";
/// Priority for separation.
const SEPA_PRIORITY: i32 = 10;
/// Frequency for separating cuts; zero means to separate only in the root
/// node.
const SEPA_FREQ: i32 = 0;
/// Maximal relative distance from the current node's dual bound to primal
/// bound compared to best node's dual bound for applying separation.
const SEPA_MAXBOUNDDIST: f64 = 0.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = true;

/// Maximal rank of a cut that could not be scaled to integral coefficients
/// (-1: unlimited).
const DEFAULT_MAXRANK: i32 = 20;
/// Maximal rank of a cut that could be scaled to integral coefficients (-1:
/// unlimited).
const DEFAULT_MAXRANKINTEGRAL: i32 = -1;
/// Maximal valid range max(|weights|)/min(|weights|) of row weights.
const DEFAULT_MAXWEIGHTRANGE: f64 = 1e+03;
/// Strengthen cut if integer variables are present.
const DEFAULT_STRENGTHEN: bool = true;
/// Node depth of separating cuts (-1: no limit).
const DEFAULT_MAXDEPTH: i32 = -1;
/// Maximal number of separation rounds in a branching node (-1: no limit).
const DEFAULT_MAXROUNDS: i32 = 25;
/// Maximal number of separation rounds in the root node (-1: no limit).
const DEFAULT_MAXROUNDSROOT: i32 = 100;
/// Maximal number of cuts investigated per iteration in a branching node.
const DEFAULT_MAXINVCUTS: i32 = 50;
/// Maximal number of cuts investigated per iteration in the root node.
const DEFAULT_MAXINVCUTSROOT: i32 = 250;
/// Delay separation if number of conflict graph edges is larger than
/// predefined value (-1: no limit).
const DEFAULT_MAXCONFSDELAY: i32 = 100000;

/// Separator data.
#[derive(Default)]
struct SepaData {
    /// Strengthen cut if integer variables are present.
    strengthen: bool,
    /// SOS1 constraint handler.
    conshdlr: ConshdlrId,
    /// Maximal valid range max(|weights|)/min(|weights|) of row weights.
    maxweightrange: f64,
    /// Maximal rank of a cut that could not be scaled to integral
    /// coefficients (-1: unlimited).
    maxrank: i32,
    /// Maximal rank of a cut that could be scaled to integral coefficients
    /// (-1: unlimited).
    maxrankintegral: i32,
    /// Node depth of separating cuts (-1: no limit).
    maxdepth: i32,
    /// Maximal number of separation rounds in a branching node (-1: no
    /// limit).
    maxrounds: i32,
    /// Maximal number of separation rounds in the root node (-1: no limit).
    maxroundsroot: i32,
    /// Maximal number of cuts separated per iteration in a branching node.
    maxinvcuts: i32,
    /// Maximal number of cuts separated per iteration in the root node.
    maxinvcutsroot: i32,
    /// Delay separation if number of conflict graph edges is larger than
    /// predefined value (-1: no limit).
    maxconfsdelay: i32,
}

fn sd(scip: &Scip, sepa: SepaId) -> &SepaData {
    scip.sepas[sepa].sepadata.as_ref().unwrap().downcast_ref::<SepaData>().unwrap()
}

fn sdm(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/*
 * Callback methods
 */

/// Copy method for separator plugins (called when SCIP copies plugins).
fn sepa_copy_disjunctive(scip: &mut Scip, _sourcescip: &Scip, _sepa: SepaId) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_sepa_disjunctive(scip)
}

/// Solving process initialization method of separator.
fn sepa_initsol_disjunctive(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    let conshdlr = crate::set::set_find_conshdlr(scip, "SOS1");
    sdm(scip, sepa).conshdlr = conshdlr;
    Ok(())
}

/// LP solution separation method for disjunctive cuts.
fn sepa_execlp_disjunctive(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if !allowlocal {
        return Ok(());
    }

    // Only generate disjunctive cuts if we are not close to terminating.
    if api::is_stopped(scip) {
        return Ok(());
    }

    // Only generate disjunctive cuts if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Only generate disjunctive cuts if the LP solution is basic.
    if !api::is_lp_sol_basic(scip) {
        return Ok(());
    }

    // Get LP data.
    let ncols = api::get_lp_cols(scip).len();
    let nrows = api::get_n_lp_rows(scip);

    // Return if LP has no columns or no rows.
    if ncols == 0 || nrows == 0 {
        return Ok(());
    }

    // Get constraint handler.
    let sepadata = sd(scip, sepa);
    let conshdlr = sepadata.conshdlr;
    if conshdlr == NONE {
        return Ok(());
    }

    // Get number of constraints.
    let nconss = scip.conshdlrs[conshdlr].nconss;
    if nconss == 0 {
        return Ok(());
    }

    // Check for maxdepth < depth, maxinvcutsroot = 0 and maxinvcuts = 0.
    if (sepadata.maxdepth >= 0 && sepadata.maxdepth < depth) || (depth == 0 && sepadata.maxinvcutsroot == 0) || (depth > 0 && sepadata.maxinvcuts == 0) {
        return Ok(());
    }

    // Only call the cut separator a given number of times at each node.
    let ncalls = scip.sepas[sepa].ncallsatnode;
    if (depth == 0 && sepadata.maxroundsroot >= 0 && ncalls >= sepadata.maxroundsroot) || (depth > 0 && sepadata.maxrounds >= 0 && ncalls >= sepadata.maxrounds) {
        return Ok(());
    }

    // The conflict graph of the SOS1 constraints (SCIPgetConflictgraphSOS1).
    unimplemented!("cons_sos1")
}

/*
 * Separator specific interface methods
 */

/// `SCIPincludeSepaDisjunctive`: creates the disjunctive cut separator and
/// includes it in SCIP.
pub fn include_sepa_disjunctive(scip: &mut Scip) -> ScipResult {
    // Create separator data.
    let sepadata = SepaData {
        conshdlr: NONE,
        ..Default::default()
    };

    // Include separator.
    let sepa = api::include_sepa_basic(
        scip,
        SEPA_NAME,
        SEPA_DESC,
        SEPA_PRIORITY,
        SEPA_FREQ,
        SEPA_MAXBOUNDDIST,
        SEPA_USESSUBSCIP,
        SEPA_DELAY,
        Some(sepa_execlp_disjunctive),
        None,
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non fundamental callbacks via setter functions.
    scip.sepas[sepa].sepacopy = Some(sepa_copy_disjunctive);
    scip.sepas[sepa].sepainitsol = Some(sepa_initsol_disjunctive);

    // Add separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    paramset_add_bool(
        scip,
        "separating/disjunctive/strengthen",
        "strengthen cut if integer variables are present.",
        target!(bool, strengthen),
        true,
        DEFAULT_STRENGTHEN,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/disjunctive/maxdepth",
        "node depth of separating bipartite disjunctive cuts (-1: no limit)",
        target!(i32, maxdepth),
        true,
        DEFAULT_MAXDEPTH,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/disjunctive/maxrounds",
        "maximal number of separation rounds per iteration in a branching node (-1: no limit)",
        target!(i32, maxrounds),
        true,
        DEFAULT_MAXROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/disjunctive/maxroundsroot",
        "maximal number of separation rounds in the root node (-1: no limit)",
        target!(i32, maxroundsroot),
        true,
        DEFAULT_MAXROUNDSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/disjunctive/maxinvcuts",
        "maximal number of cuts investigated per iteration in a branching node",
        target!(i32, maxinvcuts),
        true,
        DEFAULT_MAXINVCUTS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/disjunctive/maxinvcutsroot",
        "maximal number of cuts investigated per iteration in the root node",
        target!(i32, maxinvcutsroot),
        true,
        DEFAULT_MAXINVCUTSROOT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/disjunctive/maxconfsdelay",
        "delay separation if number of conflict graph edges is larger than predefined value (-1: no limit)",
        target!(i32, maxconfsdelay),
        true,
        DEFAULT_MAXCONFSDELAY,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/disjunctive/maxrank",
        "maximal rank of a disj. cut that could not be scaled to integral coefficients (-1: unlimited)",
        target!(i32, maxrank),
        false,
        DEFAULT_MAXRANK,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/disjunctive/maxrankintegral",
        "maximal rank of a disj. cut that could be scaled to integral coefficients (-1: unlimited)",
        target!(i32, maxrankintegral),
        false,
        DEFAULT_MAXRANKINTEGRAL,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/disjunctive/maxweightrange",
        "maximal valid range max(|weights|)/min(|weights|) of row weights",
        target!(f64, maxweightrange),
        true,
        DEFAULT_MAXWEIGHTRANGE,
        1.0,
        crate::def::REAL_MAX,
        None,
    )?;

    Ok(())
}
