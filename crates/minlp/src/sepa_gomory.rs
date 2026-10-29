//! Port of `sepa_gomory.c`: Gomory MIR cuts and strong CG cuts from LP
//! tableau rows, with the dummy child separators `strongcg` and `gomorymi`
//! whose frequencies switch the two cut types.
//!
//! Exact mode (`SCIPisExact`) and certificates are not ported.

use std::any::Any;

use crate::{
    api,
    cuts::{CutOut, aggr_row_create, aggr_row_sum_rows, calc_mir, calc_strong_cg},
    def::min,
    misc::RandNumGen,
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    types::{RowId, ScipResultCode, SepaId, VarId, VarType},
    var::NONE,
};

const SEPA_NAME: &str = "gomory";
const SEPA_DESC: &str = "separator for Gomory mixed-integer and strong CG cuts from LP tableau rows";
const SEPA_PRIORITY: i32 = -1000;
const SEPA_FREQ: i32 = 10;
const SEPA_MAXBOUNDDIST: f64 = 1.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = false;

/// Maximal number of gomory separation rounds per node (-1: unlimited).
const DEFAULT_MAXROUNDS: i32 = 5;
/// Maximal number of gomory separation rounds in the root node (-1:
/// unlimited).
const DEFAULT_MAXROUNDSROOT: i32 = 10;
/// Maximal number of gomory cuts separated per separation round.
const DEFAULT_MAXSEPACUTS: i32 = 50;
/// Maximal number of gomory cuts separated per separation round in root
/// node.
const DEFAULT_MAXSEPACUTSROOT: i32 = 200;
/// Maximal rank of a gomory cut that could not be scaled to integral
/// coefficients (-1: unlimited).
const DEFAULT_MAXRANK: i32 = -1;
/// Maximal rank of a gomory cut that could be scaled to integral
/// coefficients (-1: unlimited).
const DEFAULT_MAXRANKINTEGRAL: i32 = -1;
/// Should generated cuts be removed from the LP if they are no longer
/// tight?
const DEFAULT_DYNAMICCUTS: bool = true;
/// Minimal integrality violation of a basis variable in order to try
/// Gomory cut.
const DEFAULT_AWAY: f64 = 0.01;
/// Try to scale all cuts to integral coefficients.
const DEFAULT_MAKEINTEGRAL: bool = false;
/// If conversion to integral coefficients failed still consider the cut.
const DEFAULT_FORCECUTS: bool = true;
/// Separate rows with integral slack.
const DEFAULT_SEPARATEROWS: bool = true;
/// Should cuts be added to the delayed cut pool?
const DEFAULT_DELAYEDCUTS: bool = false;
/// Choose side types of row (lhs/rhs) based on basis information?
const DEFAULT_SIDETYPEBASIS: bool = true;
/// Try to generate strengthened Chvatal-Gomory cuts?
const DEFAULT_TRYSTRONGCG: bool = true;
/// Should both Gomory and strong CG cuts be generated (otherwise take
/// best)?
const DEFAULT_GENBOTHGOMSCG: bool = true;
/// Initial random seed.
const DEFAULT_RANDSEED: u32 = 53;

/// Threshold for bound switching - see SCIPcalcMIR().
const BOUNDSWITCH: f64 = 0.9999;
/// Apply postprocessing after MIR calculation - see SCIPcalcMIR().
const POSTPROCESS: bool = true;
/// We allow variable bound substitution for variables with continuous
/// vartype only.
const VARTYPEUSEVBDS: i32 = 2;
/// Try to generate an integral rhs - see SCIPcalcMIR().
const FIXINTEGRALRHS: bool = false;
/// Convert continuous variable to integral variables in
/// SCIPmakeRowIntegral().
const MAKECONTINTEGRAL: bool = false;

/// `MAXAGGRLEN(nvars)`: maximal length of base inequality.
fn maxaggrlen(nvars: usize) -> f64 {
    0.1 * nvars as f64 + 1000.0
}

/// Separator data.
#[derive(Default)]
struct SepaData {
    /// Random number generator.
    randnumgen: Option<RandNumGen>,
    /// Strong CG cut separator.
    strongcg: SepaId,
    /// Gomory cut separator.
    gomory: SepaId,
    away: f64,
    maxrounds: i32,
    maxroundsroot: i32,
    maxsepacuts: i32,
    maxsepacutsroot: i32,
    maxrank: i32,
    maxrankintegral: i32,
    /// Total number of cuts found after last call of separator.
    lastncutsfound: i32,
    dynamiccuts: bool,
    makeintegral: bool,
    forcecuts: bool,
    separaterows: bool,
    delayedcuts: bool,
    sidetypebasis: bool,
    #[allow(dead_code)]
    trystrongcg: bool,
    genbothgomscg: bool,
}

fn data(scip: &Scip, sepa: SepaId) -> &SepaData {
    scip.sepas[sepa].sepadata.as_ref().unwrap().downcast_ref::<SepaData>().unwrap()
}

fn data_mut(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/// `evaluateCutNumerics`: returns TRUE if the cut can be taken, otherwise
/// FALSE if there some numerical evidences.
fn evaluate_cut_numerics(scip: &mut Scip, sepa: SepaId, cut: RowId, maxdnom: i64, maxscale: f64) -> ScipResult<bool> {
    let mut madeintegral = false;
    let (makeintegral, forcecuts, maxrankintegral, maxrank) = {
        let d = data(scip, sepa);
        (d.makeintegral, d.forcecuts, d.maxrankintegral, d.maxrank)
    };

    if makeintegral && crate::lp::row_get_num_int_cols(scip, cut) == scip.rows[cut].len {
        // Try to scale the cut to integral values.
        let (eps, sumeps) = (scip.set.epsilon(), scip.set.sumepsilon());
        madeintegral = crate::lp::row_make_integral(scip, cut, -eps, sumeps, maxdnom, maxscale, MAKECONTINTEGRAL)?;

        if !madeintegral && !forcecuts {
            return Ok(false);
        }

        // In case the right hand side is plus infinity (due to scaling) the cut
        // is useless so we are not taking it at all.
        if madeintegral && scip.set.is_infinity(scip.rows[cut].rhs) {
            return Ok(false);
        }
    }

    // Discard integral cut if the rank is too high.
    if madeintegral && maxrankintegral != -1 && scip.rows[cut].rank > maxrankintegral {
        return Ok(false);
    }

    // Discard cut if the rank is too high.
    if !madeintegral && maxrank != -1 && scip.rows[cut].rank > maxrank {
        return Ok(false);
    }

    Ok(true)
}

/// The cut `addCut` adds.
struct Cut<'a> {
    nnz: usize,
    inds: &'a [i32],
    coefs: &'a [f64],
    efficacy: f64,
    rhs: f64,
    islocal: bool,
    rank: i32,
}

/// `addCut`: add cut.
#[allow(clippy::too_many_arguments)]
fn add_cut(scip: &mut Scip, sepa: SepaId, vars: &[VarId], c: i32, maxdnom: i64, maxscale: f64, cut: &Cut, strongcg: bool, cutoff: &mut bool, naddedcuts: &mut i32) -> ScipResult {
    if cut.nnz == 0 && scip.set.is_feas_negative(cut.rhs) {
        *cutoff = true;
        return Ok(());
    }

    // Only take efficient cuts, except for cuts with one non-zero coefficient
    // (= bound changes); the latter cuts will be handled internally in
    // sepastore.
    if api::is_efficacious(scip, cut.efficacy) || (cut.nnz == 1 && scip.set.is_feas_positive(cut.efficacy)) {
        // Construct cut name.
        let (cutsepa, prefix) = if strongcg {
            (data(scip, sepa).strongcg, "scg")
        } else {
            (data(scip, sepa).gomory, "gom")
        };
        let cutname = if c >= 0 {
            format!("{prefix}{}_x{c}", scip.stat.nlps)
        } else {
            format!("{prefix}{}_s{}", scip.stat.nlps, -c - 1)
        };

        // Create empty cut.
        let inf = scip.set.infinity();
        let dynamiccuts = data(scip, sepa).dynamiccuts;
        let mut row = scip_lp::create_empty_row_sepa(scip, cutsepa, &cutname, -inf, cut.rhs, cut.islocal, false, dynamiccuts)?;

        // Set cut rank.
        scip.rows[row].rank = cut.rank;

        // Cache the row extension and only flush them if the cut gets added.
        scip_lp::cache_row_extensions(scip, row);

        // Collect all non-zero coefficients.
        for v in 0..cut.nnz {
            api::add_var_to_row(scip, row, vars[cut.inds[v] as usize], cut.coefs[v])?;
        }

        // Flush all changes before adding the cut.
        scip_lp::flush_row_extensions(scip, row)?;

        if scip.rows[row].len == 0 {
            // C returns here without releasing the row.
            *cutoff = true;
            return Ok(());
        } else if scip.rows[row].len == 1 {
            // Add the bound change as cut to avoid that the LP gets modified. This
            // would mean that the LP is not flushed and the method
            // SCIPgetLPBInvRow() fails; SCIP internally will apply this bound
            // change automatically.
            *cutoff = scip_lp::add_row(scip, row, true)?;
            *naddedcuts += 1;
        } else {
            let useful = evaluate_cut_numerics(scip, sepa, row, maxdnom, maxscale)?;

            if useful && crate::cutpool::cutpool_is_cut_new(scip, crate::cutpool::GLOBALCUTPOOL, row) {
                // Add global cuts which are not implicit bound changes to the cut
                // pool.
                if !cut.islocal {
                    if data(scip, sepa).delayedcuts {
                        crate::cutpool::cutpool_add_row(scip, crate::cutpool::DELAYEDCUTPOOL, row)?;
                    } else {
                        api::add_pool_cut(scip, row)?;
                    }
                } else {
                    // Local cuts we add to the sepastore.
                    *cutoff = scip_lp::add_row(scip, row, false)?;
                }

                *naddedcuts += 1;
            }
        }

        // Release the row.
        scip_lp::release_row(scip, &mut row)?;
    }

    Ok(())
}

/*
 * Callback methods
 */

/// `sepaCopyGomory`: copy method for separator plugins.
fn sepa_copy_gomory(scip: &mut Scip, _sourcescip: &Scip, _sepa: SepaId) -> ScipResult {
    // Call inclusion method of separator.
    include_sepa_gomory(scip)
}

/// `sepaInitGomory`: initialization method of separator.
fn sepa_init_gomory(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Create and initialize random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    data_mut(scip, sepa).randnumgen = Some(RandNumGen::new(seed));
    Ok(())
}

/// `sepaExitGomory`: deinitialization method of separator.
fn sepa_exit_gomory(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    data_mut(scip, sepa).randnumgen = None;
    Ok(())
}

/// Whether a child separator of frequency `freq` separates at `depth`.
fn separates_at(freq: i32, depth: i32) -> bool {
    if freq > 0 { depth % freq == 0 } else { freq == depth }
}

/// `sepaExeclpGomory`: LP solution separation method of separator.
fn sepa_execlp_gomory(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    let ncalls = scip.sepas[sepa].ncallsatnode;
    let d = data(scip, sepa);
    let minfrac = d.away;
    let maxfrac = 1.0 - d.away;
    let (maxroundsroot, maxrounds) = (d.maxroundsroot, d.maxrounds);

    // Only call separator, if we are not close to terminating.
    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Only call the gomory cut separator a given number of times at each node.
    if (depth == 0 && maxroundsroot >= 0 && ncalls >= maxroundsroot) || (depth > 0 && maxrounds >= 0 && ncalls >= maxrounds) {
        return Ok(());
    }

    // Only call separator, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != crate::types::LpSolStat::Optimal {
        return Ok(());
    }

    // Only call separator, if the LP solution is basic.
    if !api::is_lp_sol_basic(scip) {
        return Ok(());
    }

    // Only call separator, if there are fractional variables.
    if api::get_n_lp_branch_cands(scip) == 0 {
        return Ok(());
    }

    // Check whether strong CG cuts should be separated; check whether Gomory
    // MI cuts should be separated.
    let d = data(scip, sepa);
    let separatescg = separates_at(scip.sepas[d.strongcg].freq, depth);
    let separategmi = separates_at(scip.sepas[d.gomory].freq, depth);

    if !separatescg && !separategmi {
        return Ok(());
    }

    // Get variables data.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    // Get LP data.
    let (cols, rows) = if scip.tree.as_ref().unwrap().focuslpconstructed {
        let lp = scip.lp.as_ref().unwrap();
        (lp.cols.0[..lp.ncols as usize].to_vec(), lp.rows.0[..lp.nrows as usize].to_vec())
    } else {
        (Vec::new(), Vec::new())
    };
    let (ncols, nrows) = (cols.len(), rows.len());
    if ncols == 0 || nrows == 0 {
        return Ok(());
    }

    // Set the maximal denominator in rational representation of gomory cut and
    // the maximal scale factor to scale resulting cut to integral values to
    // avoid numerical instabilities.
    let maxdepth = api::get_max_depth(scip);
    let (maxdnom, maxscale): (i64, f64) = if depth == 0 || depth <= maxdepth / 4 {
        (1000, 1000.0)
    } else if depth <= maxdepth / 2 {
        (100, 100.0)
    } else {
        (10, 10.0)
    };

    // Allocate temporary memory.
    let mut cutcoefs = vec![0.0; nvars];
    let mut cutinds = vec![0i32; nvars];
    let mut basisind = vec![0i32; nrows];
    let mut basisperm = vec![0i32; nrows];
    let mut basisfrac = vec![0.0; nrows];
    let mut binvrow = vec![0.0; nrows];
    let mut inds = vec![0i32; nrows];
    let mut cutefficacies = vec![0.0; nrows];
    let mut colindsproducedcut = vec![-1i32; nrows];
    let mut aggrrow = aggr_row_create(scip);

    // Get basis indices.
    if !crate::lp::lp_is_sol_basic(scip) {
        log::error!("current LP solution is not basic");
        return Err(crate::retcode::Retcode::InvalidCall);
    }
    crate::lp::lp_get_basis_ind(scip, &mut basisind)?;

    let separaterows = data(scip, sepa).separaterows;
    let mut randnumgen = data_mut(scip, sepa).randnumgen.take().unwrap();
    for i in 0..nrows {
        let mut frac = 0.0;

        let c = basisind[i];

        cutefficacies[i] = 0.0;
        basisperm[i] = i as i32;
        colindsproducedcut[i] = -1;

        if c >= 0 {
            let var = scip.cols[cols[c as usize]].var;

            // Check if we have an integral variable or a variable that is
            // implicitly integral.
            if scip.vars[var].vartype != VarType::Continuous {
                frac = scip.set.feas_frac(crate::lp::col_get_primsol(scip, cols[c as usize]));
                frac = min(frac, 1.0 - frac);
            }
        } else if separaterows {
            let row = rows[(-c - 1) as usize];

            // We allow separating on rows with a 'slack' implied integral variable.
            if scip.rows[row].integral && !scip.rows[row].modifiable && crate::lp::row_get_num_implied_int_cols(scip, row) == 0 {
                let activity = if crate::tree::tree_has_current_node_lp(scip) {
                    crate::lp::row_get_lp_activity(scip, row)
                } else {
                    crate::lp::row_get_pseudo_activity(scip, row)
                };
                frac = scip.set.feas_frac(activity);
                frac = min(frac, 1.0 - frac);
            }
        }

        if frac >= minfrac {
            // Slightly change fractionality to have random order for equal
            // fractions.
            basisfrac[i] = frac + randnumgen.get_real(-1e-6, 1e-6);
        } else {
            basisfrac[i] = 0.0;
        }
    }
    data_mut(scip, sepa).randnumgen = Some(randnumgen);

    // Sort basis indices by fractionality.
    crate::sort::sort_down(&mut basisfrac[..], (&mut basisperm[..],), crate::sort::cmp_real);

    // Get the maximal number of cuts allowed in a separation round.
    let d = data(scip, sepa);
    let maxsepacuts = if depth == 0 { d.maxsepacutsroot } else { d.maxsepacuts };
    let (sidetypebasis, genbothgomscg) = (d.sidetypebasis, d.genbothgomscg);

    let mut cutoff = false;
    let mut naddedcuts = 0;

    // For all basic columns belonging to integer variables, try to generate a
    // gomory cut.
    let mut i = 0;
    while i < nrows && naddedcuts < maxsepacuts && !crate::solve::solve_is_stopped(scip, false) && !cutoff {
        let mut cutrhs = 0.0;
        let mut cutefficacy = 0.0;
        let mut cutislocal = false;
        let mut strongcgsuccess = false;
        let mut ninds = -1;
        let mut cutnnz = 0;
        let mut cutrank = 0;

        if basisfrac[i] == 0.0 {
            break;
        }

        let j = basisperm[i] as usize;
        let c = basisind[j];

        // Get the row of B^-1 for this basic integer variable with fractional
        // solution value.
        if !crate::lp::lp_is_sol_basic(scip) {
            log::error!("current LP solution is not basic");
            return Err(crate::retcode::Retcode::InvalidCall);
        }
        crate::lp::lp_get_binv_row(scip, j as i32, &mut binvrow, Some(&mut inds), Some(&mut ninds))?;

        let negslack = if !scip.set.exact_enable || scip.set.exact_allownegslack { 2 } else { 0 };
        let rowinds: Option<&[i32]> = if ninds >= 0 { Some(&inds[..ninds as usize]) } else { None };
        let success = aggr_row_sum_rows(scip, &mut aggrrow, &binvrow, rowinds, sidetypebasis, allowlocal, negslack, maxaggrlen(nvars) as i32);

        if !success {
            i += 1;
            continue;
        }

        // Try to create a strong CG cut out of the aggregation row.
        if separatescg && !scip.set.exact_enable {
            let out = CutOut {
                cutcoefs: &mut cutcoefs,
                cutrhs: &mut cutrhs,
                cutinds: &mut cutinds,
                cutnnz: &mut cutnnz,
                cutefficacy: Some(&mut cutefficacy),
                cutrank: Some(&mut cutrank),
                cutislocal: &mut cutislocal,
            };
            strongcgsuccess = calc_strong_cg(scip, NONE, POSTPROCESS, BOUNDSWITCH, VARTYPEUSEVBDS, allowlocal, minfrac, maxfrac, 1.0, &aggrrow, out)?;

            // If we want to generate both cuts, add cut and reset cutefficacy and
            // strongcgsuccess.
            if strongcgsuccess && genbothgomscg {
                let cut = Cut {
                    nnz: cutnnz as usize,
                    inds: &cutinds,
                    coefs: &cutcoefs,
                    efficacy: cutefficacy,
                    rhs: cutrhs,
                    islocal: cutislocal,
                    rank: cutrank,
                };
                add_cut(scip, sepa, &vars, c, maxdnom, maxscale, &cut, true, &mut cutoff, &mut naddedcuts)?;
                if c >= 0 {
                    cutefficacies[i] = cutefficacy;
                    colindsproducedcut[i] = c;
                }
                cutefficacy = 0.0;
                strongcgsuccess = false;
                if cutoff {
                    break;
                }
            }
        }

        // Try to create Gomory cut out of the aggregation row.
        if separategmi {
            // SCIPcalcMIR will only override the cut if its efficacy is larger
            // than the one of the strongcg cut.
            let out = CutOut {
                cutcoefs: &mut cutcoefs,
                cutrhs: &mut cutrhs,
                cutinds: &mut cutinds,
                cutnnz: &mut cutnnz,
                cutefficacy: Some(&mut cutefficacy),
                cutrank: Some(&mut cutrank),
                cutislocal: &mut cutislocal,
            };
            let success = calc_mir(
                scip,
                NONE,
                POSTPROCESS,
                BOUNDSWITCH,
                VARTYPEUSEVBDS,
                allowlocal,
                FIXINTEGRALRHS,
                None,
                None,
                minfrac,
                maxfrac,
                1.0,
                &aggrrow,
                out,
            )?;

            if success || strongcgsuccess {
                if success {
                    // Set strongcgsuccess to FALSE, since the MIR cut has overriden the
                    // strongcg cut.
                    strongcgsuccess = false;
                }

                let cut = Cut {
                    nnz: cutnnz as usize,
                    inds: &cutinds,
                    coefs: &cutcoefs,
                    efficacy: cutefficacy,
                    rhs: cutrhs,
                    islocal: cutislocal,
                    rank: cutrank,
                };
                add_cut(scip, sepa, &vars, c, maxdnom, maxscale, &cut, strongcgsuccess, &mut cutoff, &mut naddedcuts)?;
                if c >= 0 {
                    cutefficacies[i] = cutefficacy;
                    colindsproducedcut[i] = c;
                }
            }
        }

        i += 1;
    }

    // Add normalized efficacy GMI statistics to history.
    let mut maxcutefficacy = 0.0;
    for i in 0..nrows {
        if cutefficacies[i] > maxcutefficacy && colindsproducedcut[i] >= 0 {
            maxcutefficacy = cutefficacies[i];
        }
    }

    for i in 0..nrows {
        if colindsproducedcut[i] >= 0 && api::is_efficacious(scip, cutefficacies[i]) {
            let var = scip.cols[cols[colindsproducedcut[i] as usize]].var;
            crate::var::var_set_last_gmiscore(scip, var, cutefficacies[i] / maxcutefficacy);
            crate::var::var_inc_gmieff_sum(scip, var, cutefficacies[i] / maxcutefficacy);
        }
    }

    let ncutsadded = scip.sepastores[crate::sepastore::SEPASTORE].ncutsadded;
    data_mut(scip, sepa).lastncutsfound = ncutsadded;

    // Evaluate the result of the separation.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if naddedcuts > 0 {
        ScipResultCode::Separated
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `sepaExeclpDummy`: LP solution separation method of dummy separator.
fn sepa_execlp_dummy(_scip: &mut Scip, _sepa: SepaId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;
    Ok(())
}

/// `sepaExecsolDummy`: arbitrary primal solution separation method of dummy
/// separator.
fn sepa_execsol_dummy(_scip: &mut Scip, _sepa: SepaId, _sol: crate::types::SolId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;
    Ok(())
}

/// `SCIPincludeSepaGomory`: creates the Gomory MIR cut separator and
/// includes it in SCIP.
pub fn include_sepa_gomory(scip: &mut Scip) -> ScipResult {
    // Create separator data.
    let sepadata = SepaData {
        lastncutsfound: 0,
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
        Some(sepa_execlp_gomory),
        None,
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Gomory is safe to use in exact solving mode (not ported).

    let strongcg = api::include_sepa_basic(
        scip,
        "strongcg",
        "separator for strong CG cuts",
        -100000,
        SEPA_FREQ,
        0.0,
        SEPA_USESSUBSCIP,
        false,
        Some(sepa_execlp_dummy),
        Some(sepa_execsol_dummy),
        None,
    )?;
    let gomory = api::include_sepa_basic(
        scip,
        "gomorymi",
        "separator for Gomory mixed-integer cuts",
        -100000,
        SEPA_FREQ,
        0.0,
        SEPA_USESSUBSCIP,
        false,
        Some(sepa_execlp_dummy),
        Some(sepa_execsol_dummy),
        None,
    )?;
    {
        let d = data_mut(scip, sepa);
        d.strongcg = strongcg;
        d.gomory = gomory;
    }

    // Set non-NULL pointers to callback methods.
    let s = &mut scip.sepas[sepa];
    s.sepacopy = Some(sepa_copy_gomory);
    s.sepainit = Some(sepa_init_gomory);
    s.sepaexit = Some(sepa_exit_gomory);

    // Mark main separator as a parent.
    s.isparentsepa = true;

    // Set pointer from child separators to main separator.
    scip.sepas[strongcg].parentsepa = sepa;
    scip.sepas[gomory].parentsepa = sepa;

    // Add separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    let name = |p: &str| format!("separating/gomory/{p}");
    paramset_add_int(
        scip,
        &name("maxrounds"),
        "maximal number of gomory separation rounds per node (-1: unlimited)",
        target!(i32, maxrounds),
        false,
        DEFAULT_MAXROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxroundsroot"),
        "maximal number of gomory separation rounds in the root node (-1: unlimited)",
        target!(i32, maxroundsroot),
        false,
        DEFAULT_MAXROUNDSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxsepacuts"),
        "maximal number of gomory cuts separated per separation round",
        target!(i32, maxsepacuts),
        false,
        DEFAULT_MAXSEPACUTS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxsepacutsroot"),
        "maximal number of gomory cuts separated per separation round in the root node",
        target!(i32, maxsepacutsroot),
        false,
        DEFAULT_MAXSEPACUTSROOT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxrank"),
        "maximal rank of a gomory cut that could not be scaled to integral coefficients (-1: unlimited)",
        target!(i32, maxrank),
        false,
        DEFAULT_MAXRANK,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxrankintegral"),
        "maximal rank of a gomory cut that could be scaled to integral coefficients (-1: unlimited)",
        target!(i32, maxrankintegral),
        false,
        DEFAULT_MAXRANKINTEGRAL,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("away"),
        "minimal integrality violation of a basis variable in order to try Gomory cut",
        target!(f64, away),
        false,
        DEFAULT_AWAY,
        1e-4,
        0.5,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("dynamiccuts"),
        "should generated cuts be removed from the LP if they are no longer tight?",
        target!(bool, dynamiccuts),
        false,
        DEFAULT_DYNAMICCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("makeintegral"),
        "try to scale cuts to integral coefficients",
        target!(bool, makeintegral),
        true,
        DEFAULT_MAKEINTEGRAL,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("forcecuts"),
        "if conversion to integral coefficients failed still consider the cut",
        target!(bool, forcecuts),
        true,
        DEFAULT_FORCECUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("separaterows"),
        "separate rows with integral slack",
        target!(bool, separaterows),
        true,
        DEFAULT_SEPARATEROWS,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("delayedcuts"),
        "should cuts be added to the delayed cut pool?",
        target!(bool, delayedcuts),
        true,
        DEFAULT_DELAYEDCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("sidetypebasis"),
        "choose side types of row (lhs/rhs) based on basis information?",
        target!(bool, sidetypebasis),
        true,
        DEFAULT_SIDETYPEBASIS,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("trystrongcg"),
        "try to generate strengthened Chvatal-Gomory cuts?",
        target!(bool, trystrongcg),
        true,
        DEFAULT_TRYSTRONGCG,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("genbothgomscg"),
        "Should both Gomory and strong CG cuts be generated (otherwise take best)?",
        target!(bool, genbothgomscg),
        true,
        DEFAULT_GENBOTHGOMSCG,
        None,
    )?;

    Ok(())
}
