//! Port of `sepa_minor.c`: principal minor separator.
//!
//! The separator computes eigenvalues with Lapack, which the reference
//! build lacks (`SCIPlapackIsAvailable()` is false), so its LP and solution
//! callbacks return before detecting or separating anything; the minor
//! detection (`detectMinors`) and cut generation are therefore not ported.

use std::any::Any;

use crate::{
    api,
    misc::RandNumGen,
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    retcode::ScipResult,
    scip::Scip,
    types::{ScipResultCode, SepaId, SolId},
};

const SEPA_NAME: &str = "minor";
const SEPA_DESC: &str = "separator to ensure that 2x2 principal minors of X - xx' are positive semi-definite";
const SEPA_PRIORITY: i32 = 0;
const SEPA_FREQ: i32 = 10;
const SEPA_MAXBOUNDDIST: f64 = 1.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = false;

/// Default constant for the maximum number of minors, i.e., max(const, fac *
/// # quadratic terms).
const DEFAULT_MAXMINORSCONST: i32 = 3000;
/// Default factor for the maximum number of minors.
const DEFAULT_MAXMINORSFAC: f64 = 10.0;
/// Default minimum required violation of a cut.
const DEFAULT_MINCUTVIOL: f64 = 1e-4;
/// Default random seed.
const DEFAULT_RANDSEED: u32 = 157;
/// Maximal number of separation rounds per node (-1: unlimited).
const DEFAULT_MAXROUNDS: i32 = 10;
/// Maximal number of separation rounds in the root node (-1: unlimited).
const DEFAULT_MAXROUNDSROOT: i32 = -1;
/// Default for ignoring circle packing constraints during minor detection.
const DEFAULT_IGNOREPACKINGCONSS: bool = true;

/// `SCIP_SepaData`: separator data.
#[derive(Default)]
struct SepaData {
    /// Constant for the maximum number of minors.
    maxminorsconst: i32,
    /// Factor for the maximum number of minors.
    maxminorsfac: f64,
    /// Maximal number of separation rounds per node (-1: unlimited).
    maxrounds: i32,
    /// Maximal number of separation rounds in the root node (-1: unlimited).
    maxroundsroot: i32,
    /// Minimum required violation of a cut.
    mincutviol: f64,
    /// Random number generation.
    randnumgen: Option<RandNumGen>,
    /// Whether to ignore circle packing constraints during minor detection.
    ignorepackingconss: bool,
}

/// `SCIPlapackIsAvailable()` in the reference build.
const LAPACK_IS_AVAILABLE: bool = false;

/*
 * Callback methods of separator
 */

/// `sepaCopyMinor`: copy method for separator plugins.
fn sepa_copy_minor(scip: &mut Scip, _source: &Scip, _sepa: SepaId) -> ScipResult {
    include_sepa_minor(scip)
}

/// `sepaFreeMinor`: destructor of separator to free user data.
fn sepa_free_minor(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    scip.sepas[sepa].sepadata = None;
    Ok(())
}

fn sd(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/// `sepaInitMinor`: initialization method of separator.
fn sepa_init_minor(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    sd(scip, sepa).randnumgen = Some(RandNumGen::new(seed));
    Ok(())
}

/// `sepaExitMinor`: deinitialization method of separator.
fn sepa_exit_minor(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Free random number generator.
    sd(scip, sepa).randnumgen = None;
    Ok(())
}

/// `sepaExeclpMinor`: LP solution separation method of separator.
fn sepa_execlp_minor(_scip: &mut Scip, _sepa: SepaId, _result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    // Need routine to compute eigenvalues/eigenvectors.
    if !LAPACK_IS_AVAILABLE {
        return Ok(());
    }
    unreachable!("minor detection and separation need Lapack");
}

/// `sepaExecsolMinor`: arbitrary primal solution separation method of
/// separator.
fn sepa_execsol_minor(_scip: &mut Scip, _sepa: SepaId, _sol: SolId, _result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    // Need routine to compute eigenvalues/eigenvectors.
    if !LAPACK_IS_AVAILABLE {
        return Ok(());
    }
    unreachable!("minor detection and separation need Lapack");
}

/*
 * separator specific interface methods
 */

/// `SCIPincludeSepaMinor`: creates the minor separator and includes it in
/// SCIP.
pub fn include_sepa_minor(scip: &mut Scip) -> ScipResult {
    // Create minor separator data.
    let sepadata = SepaData::default();

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
        Some(sepa_execlp_minor),
        Some(sepa_execsol_minor),
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non fundamental callbacks via setter functions (the exitsol
    // callback only releases detected minors, of which there are none).
    let s = &mut scip.sepas[sepa];
    s.sepacopy = Some(sepa_copy_minor);
    s.sepafree = Some(sepa_free_minor);
    s.sepainit = Some(sepa_init_minor);
    s.sepaexit = Some(sepa_exit_minor);

    // Add minor separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    paramset_add_int(
        scip,
        "separating/minor/maxminorsconst",
        "constant for the maximum number of minors, i.e., max(const, fac * # quadratic terms)",
        target!(i32, maxminorsconst),
        false,
        DEFAULT_MAXMINORSCONST,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/minor/maxminorsfac",
        "factor for the maximum number of minors, i.e., max(const, fac * # quadratic terms)",
        target!(f64, maxminorsfac),
        false,
        DEFAULT_MAXMINORSFAC,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/minor/mincutviol",
        "minimum required violation of a cut",
        target!(f64, mincutviol),
        false,
        DEFAULT_MINCUTVIOL,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/minor/maxrounds",
        "maximal number of separation rounds per node (-1: unlimited)",
        target!(i32, maxrounds),
        false,
        DEFAULT_MAXROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/minor/maxroundsroot",
        "maximal number of separation rounds in the root node (-1: unlimited)",
        target!(i32, maxroundsroot),
        false,
        DEFAULT_MAXROUNDSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/minor/ignorepackingconss",
        "whether to ignore circle packing constraints during minor detection",
        target!(bool, ignorepackingconss),
        false,
        DEFAULT_IGNOREPACKINGCONSS,
        None,
    )?;

    Ok(())
}
