//! Port of `cutsel_hybrid.c`: hybrid cut selector, a weighted sum of
//! efficacy, directed cutoff distance, objective parallelism and integral
//! support.

use std::any::Any;

use crate::{
    api,
    def::{INVALID, max},
    lp::{row_get_lp_efficacy, row_get_lp_sol_cutoff_distance, row_get_num_int_cols, row_get_obj_parallelism, row_get_parallelism},
    misc::RandNumGen,
    paramset::paramset_add_real,
    retcode::ScipResult,
    scip::Scip,
    types::{CutselId, RowId, ScipResultCode},
};

const CUTSEL_NAME: &str = "hybrid";
const CUTSEL_DESC: &str = "weighted sum of efficacy, dircutoffdist, objparal, and intsupport";
const CUTSEL_PRIORITY: i32 = 8000;

const RANDSEED: u32 = 0x5EED;
const GOODSCORE: f64 = 0.9;
const BADSCORE: f64 = 0.0;

/// Weight of efficacy in score calculation.
const DEFAULT_EFFICACYWEIGHT: f64 = 1.0;
/// Weight of directed cutoff distance in score calculation.
const DEFAULT_DIRCUTOFFDISTWEIGHT: f64 = 0.0;
/// Weight of objective parallelism in score calculation.
const DEFAULT_OBJPARALWEIGHT: f64 = 0.1;
/// Weight of integral support in cut score calculation.
const DEFAULT_INTSUPPORTWEIGHT: f64 = 0.1;
/// Minimal orthogonality for a cut to enter the LP.
const DEFAULT_MINORTHO: f64 = 0.90;
/// Minimal orthogonality for a cut to enter the LP in the root node.
const DEFAULT_MINORTHOROOT: f64 = 0.90;

/// Cut selector data.
#[derive(Default)]
struct CutselData {
    /// Random generator for tiebreaking.
    randnumgen: Option<RandNumGen>,
    /// Threshold for score of cut relative to best score to be considered
    /// good, so that less strict filtering is applied.
    goodscore: f64,
    /// Threshold for score of cut relative to best score to be discarded.
    badscore: f64,
    /// Weight of objective parallelism in cut score calculation.
    objparalweight: f64,
    /// Weight of efficacy in cut score calculation.
    efficacyweight: f64,
    /// Weight of directed cutoff distance in cut score calculation.
    dircutoffdistweight: f64,
    /// Weight of integral support in cut score calculation.
    intsupportweight: f64,
    /// Minimal orthogonality for a cut to enter the LP.
    minortho: f64,
    /// Minimal orthogonality for a cut to enter the LP in the root node.
    minorthoroot: f64,
}

fn data_mut(scip: &mut Scip, cutsel: CutselId) -> &mut CutselData {
    scip.cutsels[cutsel].cutseldata.as_mut().unwrap().downcast_mut::<CutselData>().unwrap()
}

/*
 * Local methods
 */

/// `scoring`: returns the maximum score of cuts; if scores is not NULL, then
/// stores the individual score of each cut in scores.
#[allow(clippy::too_many_arguments)]
fn scoring(
    scip: &mut Scip,
    cuts: &[RowId],
    mut randnumgen: Option<&mut RandNumGen>,
    dircutoffdistweight: f64,
    mut efficacyweight: f64,
    objparalweight: f64,
    intsupportweight: f64,
    mut scores: Option<&mut [f64]>,
) -> f64 {
    let mut maxscore = 0.0;

    let sol = api::get_best_sol(scip);

    // If there is an incumbent and the factor is not 0.0, compute directed
    // cutoff distances for the incumbent.
    if sol != crate::var::NONE && dircutoffdistweight > 0.0 {
        for (i, &cut) in cuts.iter().enumerate() {
            let intsupport = if intsupportweight > 0.0 {
                intsupportweight * row_get_num_int_cols(scip, cut) as f64 / scip.rows[cut].len as f64
            } else {
                0.0
            };

            let objparallelism = if objparalweight > 0.0 {
                objparalweight * row_get_obj_parallelism(scip, cut)
            } else {
                0.0
            };

            let mut efficacy = row_get_lp_efficacy(scip, cut);

            let mut score = if scip.rows[cut].local {
                dircutoffdistweight * efficacy
            } else {
                let s = row_get_lp_sol_cutoff_distance(scip, cut, sol);
                dircutoffdistweight * max(s, efficacy)
            };

            efficacy *= efficacyweight;
            score += objparallelism + intsupport + efficacy;

            // Add small term to prefer global pool cuts.
            if scip.rows[cut].inglobalcutpool {
                score += 1e-4;
            }

            if let Some(r) = randnumgen.as_deref_mut() {
                score += r.get_real(0.0, 1e-6);
            }

            maxscore = max(maxscore, score);

            if let Some(s) = scores.as_deref_mut() {
                s[i] = score;
            }
        }
    } else {
        // In case there is no solution add the directed cutoff distance weight
        // to the efficacy weight.
        efficacyweight += dircutoffdistweight;
        for (i, &cut) in cuts.iter().enumerate() {
            let intsupport = if intsupportweight > 0.0 {
                intsupportweight * row_get_num_int_cols(scip, cut) as f64 / scip.rows[cut].len as f64
            } else {
                0.0
            };

            let objparallelism = if objparalweight > 0.0 {
                objparalweight * row_get_obj_parallelism(scip, cut)
            } else {
                0.0
            };

            let efficacy = if efficacyweight > 0.0 { efficacyweight * row_get_lp_efficacy(scip, cut) } else { 0.0 };

            let mut score = objparallelism + intsupport + efficacy;

            // Add small term to prefer global pool cuts.
            if scip.rows[cut].inglobalcutpool {
                score += 1e-4;
            }

            if let Some(r) = randnumgen.as_deref_mut() {
                score += r.get_real(0.0, 1e-6);
            }

            maxscore = max(maxscore, score);

            if let Some(s) = scores.as_deref_mut() {
                s[i] = score;
            }
        }
    }
    maxscore
}

/// `selectBestCut`: move the cut with the highest score to the first
/// position in the array; there must be at least one cut.
fn select_best_cut(cuts: &mut [RowId], scores: &mut [f64]) {
    let mut bestscore = scores[0];
    let mut bestpos = 0;

    for i in 1..cuts.len() {
        if scores[i] > bestscore {
            bestpos = i;
            bestscore = scores[i];
        }
    }

    cuts.swap(bestpos, 0);
    scores.swap(bestpos, 0);
}

/// `filterWithParallelism`: filters the given array of cuts to enforce a
/// maximum parallelism constraint w.r.t the given cut; moves filtered cuts
/// to the end of the array and returns number of selected cuts.
#[allow(clippy::too_many_arguments)]
fn filter_with_parallelism(scip: &mut Scip, cut: RowId, cuts: &mut [RowId], scores: &mut [f64], mut ncuts: usize, goodscore: f64, goodmaxparall: f64, maxparall: f64) -> usize {
    for i in (0..ncuts).rev() {
        let thisparall = row_get_parallelism(scip, cut, cuts[i], b'e');
        let thismaxparall = if scores[i] >= goodscore { goodmaxparall } else { maxparall };

        if thisparall > thismaxparall {
            ncuts -= 1;
            cuts.swap(i, ncuts);
            scores.swap(i, ncuts);
        }
    }

    ncuts
}

/*
 * Callback methods of cut selector
 */

/// `cutselCopyHybrid`: copy method for cut selector plugin.
fn cutsel_copy_hybrid(scip: &mut Scip, _sourcescip: &Scip, _cutsel: CutselId) -> ScipResult {
    // Call inclusion method of cut selector.
    include_cutsel_hybrid(scip)
}

/// `cutselInitHybrid`: initialization method of cut selector.
fn cutsel_init_hybrid(scip: &mut Scip, cutsel: CutselId) -> ScipResult {
    let seed = scip.set.initialize_random_seed(RANDSEED);
    data_mut(scip, cutsel).randnumgen = Some(RandNumGen::new(seed));
    Ok(())
}

/// `cutselExitHybrid`: deinitialization method of cut selector.
fn cutsel_exit_hybrid(scip: &mut Scip, cutsel: CutselId) -> ScipResult {
    data_mut(scip, cutsel).randnumgen = None;
    Ok(())
}

/// `cutselSelectHybrid`: cut selection method of cut selector.
#[allow(clippy::too_many_arguments)]
fn cutsel_select_hybrid(
    scip: &mut Scip,
    cutsel: CutselId,
    cuts: &mut [RowId],
    forcedcuts: &[RowId],
    root: bool,
    maxnselectedcuts: i32,
    nselectedcuts: &mut i32,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::Success;

    let d = data_mut(scip, cutsel);
    let minortho = if root { d.minorthoroot } else { d.minortho };
    let maxparall = 1.0 - minortho;
    let goodmaxparall = max(0.5, 1.0 - minortho);
    let (goodscore, badscore, dircutoffdistweight, efficacyweight, objparalweight, intsupportweight) =
        (d.goodscore, d.badscore, d.dircutoffdistweight, d.efficacyweight, d.objparalweight, d.intsupportweight);

    // The random number generator lives in the selector's data; take it out
    // while the selection borrows the SCIP data.
    let mut randnumgen = data_mut(scip, cutsel).randnumgen.take();
    let r = select_cuts_hybrid(
        scip,
        cuts,
        forcedcuts,
        randnumgen.as_mut(),
        goodscore,
        badscore,
        goodmaxparall,
        maxparall,
        dircutoffdistweight,
        efficacyweight,
        objparalweight,
        intsupportweight,
        maxnselectedcuts,
        nselectedcuts,
    );
    data_mut(scip, cutsel).randnumgen = randnumgen;
    r
}

/*
 * cut selector specific interface methods
 */

/// `SCIPincludeCutselHybrid`: creates the hybrid cut selector and includes
/// it in SCIP.
pub fn include_cutsel_hybrid(scip: &mut Scip) -> ScipResult {
    // Create hybrid cut selector data.
    let cutseldata = CutselData {
        goodscore: GOODSCORE,
        badscore: BADSCORE,
        ..Default::default()
    };

    let methods = crate::cutsel::CutselMethods {
        cutselcopy: Some(cutsel_copy_hybrid),
        cutselinit: Some(cutsel_init_hybrid),
        cutselexit: Some(cutsel_exit_hybrid),
        cutselselect: Some(cutsel_select_hybrid),
        ..Default::default()
    };
    if scip.set.cutsels.0[..scip.set.ncutsels as usize].iter().any(|&c| scip.cutsels[c].name == CUTSEL_NAME) {
        // Cut selector already included.
        return Err(crate::retcode::Retcode::InvalidData);
    }
    let cutsel = crate::cutsel::cutsel_create(scip, CUTSEL_NAME, CUTSEL_DESC, CUTSEL_PRIORITY, methods, Some(Box::new(cutseldata) as Box<dyn Any>))?;
    crate::set::set_include_cutsel(scip, cutsel);

    // Add hybrid cut selector parameters.
    macro_rules! target {
        ($field:ident) => {
            crate::data_target!(f64, cutsels, cutsel, cutseldata, CutselData, $field)
        };
    }
    let name = |p: &str| format!("cutselection/{CUTSEL_NAME}/{p}");
    paramset_add_real(
        scip,
        &name("efficacyweight"),
        "weight of efficacy in cut score calculation",
        target!(efficacyweight),
        false,
        DEFAULT_EFFICACYWEIGHT,
        0.0,
        INVALID / 10.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("dircutoffdistweight"),
        "weight of directed cutoff distance in cut score calculation",
        target!(dircutoffdistweight),
        false,
        DEFAULT_DIRCUTOFFDISTWEIGHT,
        0.0,
        INVALID / 10.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("objparalweight"),
        "weight of objective parallelism in cut score calculation",
        target!(objparalweight),
        false,
        DEFAULT_OBJPARALWEIGHT,
        0.0,
        INVALID / 10.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("intsupportweight"),
        "weight of integral support in cut score calculation",
        target!(intsupportweight),
        false,
        DEFAULT_INTSUPPORTWEIGHT,
        0.0,
        INVALID / 10.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("minortho"),
        "minimal orthogonality for a cut to enter the LP",
        target!(minortho),
        false,
        DEFAULT_MINORTHO,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("minorthoroot"),
        "minimal orthogonality for a cut to enter the LP in the root node",
        target!(minorthoroot),
        false,
        DEFAULT_MINORTHOROOT,
        0.0,
        1.0,
        None,
    )?;

    Ok(())
}

/// `SCIPselectCutsHybrid`: perform a cut selection algorithm for the given
/// array of cuts; the selected cuts are moved to the front of `cuts`.
#[allow(clippy::too_many_arguments)]
pub fn select_cuts_hybrid(
    scip: &mut Scip,
    cuts: &mut [RowId],
    forcedcuts: &[RowId],
    mut randnumgen: Option<&mut RandNumGen>,
    goodscorefac: f64,
    badscorefac: f64,
    goodmaxparall: f64,
    maxparall: f64,
    dircutoffdistweight: f64,
    efficacyweight: f64,
    objparalweight: f64,
    intsupportweight: f64,
    maxselectedcuts: i32,
    nselectedcuts: &mut i32,
) -> ScipResult {
    let mut ncuts = cuts.len();
    *nselectedcuts = 0;

    let mut scores = vec![0.0; ncuts];

    // Compute scores of cuts and max score of cuts and forced cuts (used to
    // define goodscore).
    let maxforcedscores = scoring(
        scip,
        forcedcuts,
        randnumgen.as_deref_mut(),
        dircutoffdistweight,
        efficacyweight,
        objparalweight,
        intsupportweight,
        None,
    );
    let maxnonforcedscores = scoring(
        scip,
        cuts,
        randnumgen,
        dircutoffdistweight,
        efficacyweight,
        objparalweight,
        intsupportweight,
        Some(&mut scores),
    );

    let mut goodscore = max(maxforcedscores, maxnonforcedscores);

    // Compute values for filtering cuts.
    let badscore = goodscore * badscorefac;
    goodscore *= goodscorefac;

    // Perform cut selection algorithm for the cuts; forced cuts are going to
    // be selected so use them to filter cuts.
    let mut i = 0;
    while i < forcedcuts.len() && ncuts > 0 {
        ncuts = filter_with_parallelism(scip, forcedcuts[i], cuts, &mut scores, ncuts, goodscore, goodmaxparall, maxparall);
        i += 1;
    }

    // Now greedily select the remaining cuts.
    let mut start = 0;
    while ncuts > 0 {
        select_best_cut(&mut cuts[start..start + ncuts], &mut scores[start..start + ncuts]);
        let selectedcut = cuts[start];

        // If the best cut of the remaining cuts is considered bad, we discard it
        // and all remaining cuts.
        if scores[start] < badscore {
            break;
        }

        *nselectedcuts += 1;

        // If the maximal number of cuts was selected, we can stop here.
        if *nselectedcuts == maxselectedcuts {
            break;
        }

        // Move the pointers to the next position and filter the remaining cuts to
        // enforce the maximum parallelism constraint.
        start += 1;
        ncuts -= 1;
        ncuts = filter_with_parallelism(scip, selectedcut, &mut cuts[start..], &mut scores[start..], ncuts, goodscore, goodmaxparall, maxparall);
    }

    Ok(())
}
