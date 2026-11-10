//! Port of `relax_benders.c`: a relaxator that applies the default Benders'
//! decomposition to a user decomposition and solves the decomposed
//! problem. User decompositions are not ported (`SCIPgetDecomps` finds
//! none), so the decomposition is never applied and the relaxator never
//! runs; it is included because its presence alone changes the node
//! solving loop (`relaxcalled`).

use std::any::Any;

use crate::{
    relax::RelaxMethods,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{RelaxId, ScipResultCode},
};

const RELAX_NAME: &str = "benders";
const RELAX_DESC: &str = "applies default Benders' decomposition and solves the problem";
const RELAX_PRIORITY: i32 = 1;
const RELAX_FREQ: i32 = 0;

/// Continue solving the original SCIP instance if optimal solution is not
/// found.
const DEFAULT_CONTORIG: bool = false;
/// Node limit for the Benders' decomposition solve, -1 indicates original
/// SCIP limit is used.
const DEFAULT_NODELIMIT: i64 = -1;

/// `SCIP_RelaxData` (the master problem, subproblems and variable maps exist
/// only once a decomposition is applied).
struct RelaxData {
    /// Indicates whether the decomposition was applied.
    decompapplied: bool,
    /// The node limit for the Benders' decomposition solve.
    nodelimit: i64,
    /// Continue solving the original SCIP instance if optimal solution is
    /// not found.
    contorig: bool,
}

fn rd(scip: &Scip, relax: RelaxId) -> &RelaxData {
    scip.relaxs[relax].relaxdata.as_ref().unwrap().downcast_ref().unwrap()
}

/// `freeDecomposition`: frees the master problem and subproblems if the
/// decomposition was applied.
fn free_decomposition(scip: &mut Scip, relax: RelaxId) -> ScipResult {
    // If the decomposition has been applied, then the corresponding SCIP
    // instances need to be freed.
    if rd(scip, relax).decompapplied {
        unimplemented!("relax_benders with an applied decomposition");
    }
    Ok(())
}

/// `relaxFreeBenders`: destructor of relaxator to free user data (called
/// when SCIP is exiting).
fn relax_free_benders(scip: &mut Scip, relax: RelaxId) -> ScipResult {
    // Freeing the decomposition information.
    free_decomposition(scip, relax)?;
    scip.relaxs[relax].relaxdata = None;
    Ok(())
}

/// `relaxInitsolBenders`: solving process initialization method of
/// relaxator (called when branch and bound process is about to begin).
fn relax_initsol_benders(scip: &mut Scip, relax: RelaxId) -> ScipResult {
    // If a decomposition has been previously applied, then it must be freed
    // first.
    free_decomposition(scip, relax)?;

    let applybenders = crate::paramset::paramset_get_bool(scip, "decomposition/applybenders")?;
    if !applybenders {
        return Ok(());
    }

    // If there are no decompositions, then Benders' decomposition can't be
    // applied (user decompositions are not ported: there are none).
    Ok(())
}

/// `relaxExecBenders`: execution method of relaxator.
fn relax_exec_benders(scip: &mut Scip, relax: RelaxId, _lowerbound: &mut f64, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // The relaxator is only executed if the Benders decomposition is applied.
    if !rd(scip, relax).decompapplied {
        return Ok(());
    }

    unimplemented!("relax_benders with an applied decomposition")
}

/// `SCIPincludeRelaxBenders`: creates the Benders' decomposition relaxator
/// and includes it in SCIP.
pub fn include_relax_benders(scip: &mut Scip) -> ScipResult {
    // Create benders relaxator data.
    let relaxdata = RelaxData {
        decompapplied: false,
        nodelimit: 0,
        contorig: false,
    };

    // Include relaxator (`SCIPincludeRelax`: check whether relaxation handler
    // is already present).
    if crate::set::set_find_relax(scip, RELAX_NAME) != crate::var::NONE {
        log::error!("relaxation handler <{RELAX_NAME}> already included.");
        return Err(Retcode::InvalidData);
    }
    let methods = RelaxMethods {
        relaxfree: Some(relax_free_benders),
        relaxinitsol: Some(relax_initsol_benders),
        relaxexec: Some(relax_exec_benders),
        ..Default::default()
    };
    let relax = crate::relax::relax_create(scip, RELAX_NAME, RELAX_DESC, RELAX_PRIORITY, RELAX_FREQ, methods, Some(Box::new(relaxdata) as Box<dyn Any>))?;
    crate::set::set_include_relax(scip, relax);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, relaxs, relax, relaxdata, RelaxData, $f)
        };
    }

    // Adding parameters for the Benders' relaxator.
    crate::paramset::paramset_add_bool(
        scip,
        "relaxing/benders/continueorig",
        "continue solving the original SCIP instance if the optimal solution is not found by Benders' decomposition",
        t!(bool, contorig),
        false,
        DEFAULT_CONTORIG,
        None,
    )?;
    crate::paramset::paramset_add_longint(
        scip,
        "relaxing/benders/nodelimit",
        "the node limit applied only to the Benders' decomposition solve (-1 indicates that the original SCIP node limit is used).",
        t!(i64, nodelimit),
        false,
        DEFAULT_NODELIMIT,
        -1,
        i64::MAX,
        None,
    )?;

    Ok(())
}
