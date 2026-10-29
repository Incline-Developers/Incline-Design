//! Port of `sepa_mixing.c`: mixing inequality separator.

use std::any::Any;

use crate::{
    api,
    paramset::{paramset_add_bool, paramset_add_int},
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    types::{ScipResultCode, SepaId, SolId},
    var::{NONE, var_is_binary},
};

const SEPA_NAME: &str = "mixing";
const SEPA_DESC: &str = "mixing inequality separator";
/// Maximal number of mixing separation rounds per node (-1: unlimited).
const DEFAULT_MAXROUNDS: i32 = -1;
/// Maximal number of mixing separation rounds in the root node (-1:
/// unlimited).
const DEFAULT_MAXROUNDSROOT: i32 = -1;
const SEPA_PRIORITY: i32 = -50;
const SEPA_FREQ: i32 = 10;
const SEPA_MAXBOUNDDIST: f64 = 1.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = false;
/// Should local bounds be used?
const DEFAULT_USELOCALBOUNDS: bool = false;
/// Should general/implicit integer variables be used to generate cuts?
const DEFAULT_ISCUTSONINTS: bool = false;
/// Maximal number of consecutive unsuccessful iterations.
const DEFAULT_MAXNUNSUCCESSFUL: i32 = 10;

/// Separator-specific data for the mixing separator.
#[derive(Default)]
struct SepaData {
    uselocalbounds: bool,
    iscutsonints: bool,
    maxrounds: i32,
    maxroundsroot: i32,
    /// Number of consecutive unsuccessful iterations.
    nunsuccessful: i32,
    maxnunsuccessful: i32,
}

fn data(scip: &Scip, sepa: SepaId) -> &SepaData {
    scip.sepas[sepa].sepadata.as_ref().unwrap().downcast_ref::<SepaData>().unwrap()
}

fn data_mut(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/*
 * local methods
 */

/// `addCut`: adds the given cut.
#[allow(clippy::too_many_arguments)]
fn add_cut(
    scip: &mut Scip,
    sepa: SepaId,
    sol: SolId,
    cutcoefs: &[f64],
    cutinds: &[i32],
    cutnnz: usize,
    cutrhs: f64,
    cutislocal: bool,
    cutoff: &mut bool,
    ncuts: &mut i32,
) -> ScipResult {
    *cutoff = false;

    // Get active problem variables.
    let vars = api::get_vars(scip).to_vec();

    // Construct cut name.
    let cutname = format!("mix{}_x{}", scip.stat.nlps, *ncuts);

    // Create empty cut.
    let inf = scip.set.infinity();
    let mut cut = scip_lp::create_empty_row_sepa(scip, sepa, &cutname, -inf, cutrhs, cutislocal, false, true)?;

    // Cache the row extension and only flush them if the cut gets added.
    scip_lp::cache_row_extensions(scip, cut);

    // Collect all non-zero coefficients.
    for v in 0..cutnnz {
        api::add_var_to_row(scip, cut, vars[cutinds[v] as usize], cutcoefs[v])?;
    }

    // Flush all changes before adding the cut.
    scip_lp::flush_row_extensions(scip, cut)?;

    if scip_lp::is_cut_efficacious(scip, sol, cut) {
        // Set cut rank.
        scip.rows[cut].rank = 1;

        if cutislocal {
            // Local cuts are added to the sepastore.
            *cutoff = scip_lp::add_row(scip, cut, false)?;
        } else {
            api::add_pool_cut(scip, cut)?;
        }
        *ncuts += 1;
    }

    // Release the row.
    scip_lp::release_row(scip, &mut cut)
}

/// The variable bound information `separateCuts` collects for one side.
#[derive(Default)]
struct Mix {
    coefs: Vec<f64>,
    sols: Vec<f64>,
    inds: Vec<i32>,
    signs: Vec<i32>,
}

impl Mix {
    fn clear(&mut self) {
        self.coefs.clear();
        self.sols.clear();
        self.inds.clear();
        self.signs.clear();
    }

    fn len(&self) -> usize {
        self.coefs.len()
    }

    /// `SCIPsortDownRealRealIntInt(sols, coefs, inds, signs)`.
    fn sort(&mut self) {
        crate::sort::sort_down(&mut self.sols[..], (&mut self.coefs[..], &mut self.inds[..], &mut self.signs[..]), crate::sort::cmp_real);
    }
}

/// `separateCuts`: searches and adds mixing cuts that are violated by the
/// given solution value array.
///
/// This function implements the separation algorithm for the mixing
/// inequalities on one continuous variable and its variable bounds
/// (Marchand, Martin, Weismantel and Wolsey: cutting planes in integer and
/// mixed integer programming), and the conflict inequalities that
/// combine a variable lower and a variable upper bound.
fn separate_cuts(scip: &mut Scip, sepa: SepaId, sol: SolId, cutoff: &mut bool, ncuts: &mut i32) -> ScipResult {
    *cutoff = false;
    *ncuts = 0;

    // Exit if there are no binary variables - ignore integer variables that
    // have 0/1 bounds.
    let nmaxvars = api::get_n_bin_vars(scip) as usize;
    if nmaxvars == 0 {
        return Ok(());
    }

    let d = data(scip, sepa);
    let uselocalbounds = d.uselocalbounds;

    // Get the index of the first considered variable.
    let firstvar = if d.iscutsonints {
        // Generate cuts based on all nonbinary variabels.
        api::get_n_bin_vars(scip)
    } else {
        // Only generate cuts based on continuous variables.
        api::get_n_bin_vars(scip) + api::get_n_int_vars(scip) + api::get_n_bin_impl_vars(scip) + api::get_n_int_impl_vars(scip)
    } as usize;
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    if firstvar == nvars {
        return Ok(());
    }

    // Allocate temporary memory.
    let mut vlbmix = Mix::default();
    let mut vubmix = Mix::default();
    let mut cutcoefs = vec![0.0; nmaxvars + 1];
    let mut cutinds = vec![0i32; nmaxvars + 1];

    for &var in &vars[firstvar..nvars] {
        let mut lb = crate::def::INVALID;
        let mut ub = crate::def::INVALID;
        // Is it a local lower bound or global lower bound?
        let mut islocallb = false;
        // Is it a local upper bound or global upper bound?
        let mut islocalub = false;
        let mut cutnnz;
        let mut cutrhs;
        vlbmix.clear();
        vubmix.clear();

        if scip.vars[var].probindex < 0 {
            continue;
        }

        let nvlb = scip.vars[var].vlbs.as_ref().map_or(0, |v| v.len) as usize;
        let nvub = scip.vars[var].vubs.as_ref().map_or(0, |v| v.len) as usize;

        if nvlb == 0 && nvub == 0 {
            continue;
        }

        // Skip lower bound if the LP solution value is equal to the upper bound
        // of the continuous variable.
        let varsolval = api::get_sol_val(scip, sol, var);

        'vlb: {
            if scip.set.is_feas_eq(scip.vars[var].locdom.ub, varsolval) {
                break 'vlb;
            }

            if nvlb == 0 {
                break 'vlb;
            }

            // Get variable lower variable bounds information.
            let (vlbvars, vlbcoefs, vlbconsts) = {
                let v = scip.vars[var].vlbs.as_ref().unwrap();
                (v.vars.0[..nvlb].to_vec(), v.coefs.0[..nvlb].to_vec(), v.constants.0[..nvlb].to_vec())
            };
            let mut maxabscoef = 0.0;
            let mut maxabsind = -1;
            let mut maxabssign = 0;

            lb = scip.vars[var].glbdom.lb;
            if uselocalbounds && scip.set.is_lt(lb, scip.vars[var].locdom.lb) {
                // This is a lcoal cut.
                islocallb = true;
                lb = scip.vars[var].locdom.lb;
            }

            // Extract the useful variable bounds information (binary and
            // nonredundant).
            for j in 0..nvlb {
                // Consider only active and binary variables.
                if var_is_binary(scip, vlbvars[j]) && scip.vars[vlbvars[j]].probindex >= 0 {
                    let maxactivity = if vlbcoefs[j] > 0.0 { vlbconsts[j] + vlbcoefs[j] } else { vlbconsts[j] };

                    // Skip redundant variable bound constraints.
                    if scip.set.is_feas_le(maxactivity, lb) {
                        continue;
                    }

                    let (coef, sign) = if vlbcoefs[j] > 0.0 { (maxactivity - lb, 0) } else { (lb - maxactivity, 1) };

                    let solval = api::get_sol_val(scip, sol, vlbvars[j]);
                    vlbmix.signs.push(sign);
                    vlbmix.coefs.push(coef.abs());
                    vlbmix.inds.push(scip.vars[vlbvars[j]].probindex);
                    vlbmix.sols.push(if sign == 0 { solval } else { 1.0 - solval });

                    // Update the maximal coefficient if needed.
                    let k = vlbmix.len() - 1;
                    if maxabscoef < vlbmix.coefs[k] {
                        maxabscoef = vlbmix.coefs[k];
                        maxabsind = vlbmix.inds[k];
                        maxabssign = vlbmix.signs[k];
                    }

                    // Stop if size is exceeded; possibly ignore redundant variable
                    // bounds.
                    if vlbmix.len() >= nmaxvars {
                        break;
                    }
                }
            }

            // Stop if no variable lower bounds information exists.
            if vlbmix.len() == 0 {
                break 'vlb;
            }

            // Stop if the current solution value of the transformed continuous
            // variable is larger than the maximal coefficient.
            if scip.set.is_feas_gt(varsolval - lb, maxabscoef) {
                break 'vlb;
            }

            // Sort the solution values in non-increasing order.
            vlbmix.sort();

            // Add the continuous variable.
            cutnnz = 0;
            cutcoefs[cutnnz] = -1.0;
            cutinds[cutnnz] = scip.vars[var].probindex;
            cutrhs = -lb;
            cutnnz += 1;

            let mut activity = -(varsolval - lb);
            let mut lastcoef = 0.0;

            // Loop over the variables and add the variable to the cut if its
            // coefficient is larger than that of the last variable.
            for j in 0..vlbmix.len() {
                let solval = vlbmix.sols[j];

                // Stop if we can not find a violated cut or we reached 0 solution
                // values.
                if activity + solval * (maxabscoef - lastcoef) < 0.0 || scip.set.is_feas_zero(solval) {
                    break;
                } else {
                    // Skip if we have already added a variable with bigger coefficient.
                    if scip.set.is_le(vlbmix.coefs[j], lastcoef) {
                        continue;
                    } else {
                        activity += (vlbmix.coefs[j] - lastcoef) * solval;
                        if vlbmix.signs[j] != 0 {
                            cutcoefs[cutnnz] = lastcoef - vlbmix.coefs[j];
                            cutrhs -= vlbmix.coefs[j] - lastcoef;
                        } else {
                            cutcoefs[cutnnz] = vlbmix.coefs[j] - lastcoef;
                        }
                        cutinds[cutnnz] = vlbmix.inds[j];
                        cutnnz += 1;
                        lastcoef = vlbmix.coefs[j];
                    }
                }
            }

            // Add the variable with maximal coefficient to make sure the cut is
            // strong enough.
            if scip.set.is_gt(maxabscoef, lastcoef) {
                // Do not update activity: either the corresponding solval is 0.0 or the
                // cut will not become efficious.
                if maxabssign != 0 {
                    cutcoefs[cutnnz] = lastcoef - maxabscoef;
                    cutrhs -= maxabscoef - lastcoef;
                } else {
                    cutcoefs[cutnnz] = maxabscoef - lastcoef;
                }
                cutinds[cutnnz] = maxabsind;
                cutnnz += 1;
            }

            // Add the cut if the violtion is good enough and the number of nonzero
            // coefficients is larger than 2.
            if api::is_efficacious(scip, activity) && cutnnz > 2 {
                add_cut(scip, sepa, sol, &cutcoefs, &cutinds, cutnnz, cutrhs, islocallb, cutoff, ncuts)?;
            }
        }

        // VUB:
        'vub: {
            if nvub == 0 {
                break 'vub;
            }

            // Get variable upper bounds information.
            let (vubvars, vubcoefs, vubconsts) = {
                let v = scip.vars[var].vubs.as_ref().unwrap();
                (v.vars.0[..nvub].to_vec(), v.coefs.0[..nvub].to_vec(), v.constants.0[..nvub].to_vec())
            };
            let mut maxabscoef = 0.0;
            let mut maxabsind = -1;
            let mut maxabssign = 0;

            // Stop if the lower bound is equal to the solution value of the
            // continuous variable.
            if scip.set.is_feas_eq(scip.vars[var].locdom.lb, varsolval) {
                break 'vub;
            }

            ub = scip.vars[var].glbdom.ub;
            if uselocalbounds && scip.set.is_gt(ub, scip.vars[var].locdom.ub) {
                // This is a lcoal cut.
                islocalub = true;
                ub = scip.vars[var].locdom.ub;
            }

            // Extract the useful variable bounds information (binary and
            // nonredundant).
            for j in 0..nvub {
                // Consider only active and binary variables.
                if var_is_binary(scip, vubvars[j]) && scip.vars[vubvars[j]].probindex >= 0 {
                    let minactivity = if vubcoefs[j] < 0.0 { vubconsts[j] + vubcoefs[j] } else { vubconsts[j] };

                    // Skip redundant variable bound constraints.
                    if scip.set.is_feas_le(ub, minactivity) {
                        continue;
                    }

                    let (coef, sign) = if vubcoefs[j] > 0.0 { (ub - minactivity, 1) } else { (minactivity - ub, 0) };

                    let solval = api::get_sol_val(scip, sol, vubvars[j]);
                    vubmix.signs.push(sign);
                    vubmix.coefs.push(coef.abs());
                    vubmix.inds.push(scip.vars[vubvars[j]].probindex);
                    vubmix.sols.push(if sign == 0 { solval } else { 1.0 - solval });

                    // Update the maximal coefficient if needed.
                    let k = vubmix.len() - 1;
                    if maxabscoef < vubmix.coefs[k] {
                        maxabscoef = vubmix.coefs[k];
                        maxabsind = vubmix.inds[k];
                        maxabssign = vubmix.signs[k];
                    }

                    // Stop if size is exceeded; possibly ignore redundant variable
                    // bounds.
                    if vubmix.len() >= nmaxvars {
                        break;
                    }
                }
            }

            // Stop if no variable upper bounds information exists.
            if vubmix.len() == 0 {
                break 'vub;
            }

            // Stop if the current solution value of transformed continuous variable
            // is larger than the maximal coefficient.
            if scip.set.is_feas_gt(ub - varsolval, maxabscoef) {
                break 'vub;
            }

            // Sort the solution values in non-increasing order.
            vubmix.sort();

            // Add the continuous variables.
            cutnnz = 0;
            cutcoefs[cutnnz] = 1.0;
            cutinds[cutnnz] = scip.vars[var].probindex;
            cutrhs = ub;
            cutnnz += 1;

            let mut activity = varsolval - ub;
            let mut lastcoef = 0.0;

            for j in 0..vubmix.len() {
                let solval = vubmix.sols[j];

                // Stop if we can not find a violated cut or we reached 0 solution
                // values.
                if activity + solval * (maxabscoef - lastcoef) < 0.0 || scip.set.is_feas_zero(solval) {
                    break;
                } else {
                    // Skip if we have already added a variable with bigger coefficient.
                    if scip.set.is_le(vubmix.coefs[j], lastcoef) {
                        continue;
                    } else {
                        activity += (vubmix.coefs[j] - lastcoef) * solval;
                        if vubmix.signs[j] != 0 {
                            cutcoefs[cutnnz] = lastcoef - vubmix.coefs[j];
                            cutrhs -= vubmix.coefs[j] - lastcoef;
                        } else {
                            cutcoefs[cutnnz] = vubmix.coefs[j] - lastcoef;
                        }
                        cutinds[cutnnz] = vubmix.inds[j];
                        cutnnz += 1;
                        lastcoef = vubmix.coefs[j];
                    }
                }
            }

            // Add the variable with maximal coefficient if needed.
            if scip.set.is_gt(maxabscoef, lastcoef) {
                // Do not update activity: either the corresponding solval is 0.0 or the
                // cut will not become efficious.
                if maxabssign != 0 {
                    cutcoefs[cutnnz] = lastcoef - maxabscoef;
                    cutrhs -= maxabscoef - lastcoef;
                } else {
                    cutcoefs[cutnnz] = maxabscoef - lastcoef;
                }
                cutinds[cutnnz] = maxabsind;
                cutnnz += 1;
            }

            // Add the cut if the violtion is good enough and the number of nonzero
            // coefficients is larger than 2.
            if api::is_efficacious(scip, activity) && cutnnz > 2 {
                add_cut(scip, sepa, sol, &cutcoefs, &cutinds, cutnnz, cutrhs, islocalub, cutoff, ncuts)?;
            }
        }

        // CONFLICT: combine the variable lower bounds information and upper
        // bounds information together to generate cuts; stop if no useful
        // variable lower (or upper) bounds information exists.
        if vlbmix.len() == 0 || vubmix.len() == 0 {
            continue;
        }

        let cutislocal = islocallb || islocalub;
        for j in 0..vlbmix.len() {
            let solval = vlbmix.sols[j];

            // Stop if no violated cut exists.
            if !api::is_efficacious(scip, solval + vubmix.sols[0] - 1.0) {
                break;
            }

            for k in 0..vubmix.len() {
                // Only consider the inequality if its violation is good enough.
                if api::is_efficacious(scip, solval + vubmix.sols[k] - 1.0) {
                    let tmp = lb + vlbmix.coefs[j] + vubmix.coefs[k] - ub;

                    // Add the cut if it is valid.
                    if api::is_efficacious(scip, tmp) {
                        cutnnz = 2;
                        cutrhs = 1.0;
                        cutcoefs[0] = if vlbmix.signs[j] != 0 { -1.0 } else { 1.0 };
                        cutcoefs[1] = if vubmix.signs[k] != 0 { -1.0 } else { 1.0 };
                        cutinds[0] = vlbmix.inds[j];
                        cutinds[1] = vubmix.inds[k];
                        cutrhs = if vlbmix.signs[j] != 0 { cutrhs - 1.0 } else { cutrhs };
                        cutrhs = if vubmix.signs[k] != 0 { cutrhs - 1.0 } else { cutrhs };
                        add_cut(scip, sepa, sol, &cutcoefs, &cutinds, cutnnz, cutrhs, cutislocal, cutoff, ncuts)?;
                    }
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}

/*
 * Callback methods of separator
 */

/// `sepaCopyMixing`: copy method for separator plugins.
fn sepa_copy_mixing(scip: &mut Scip, _sourcescip: &Scip, _sepa: SepaId) -> ScipResult {
    // Call inclusion method of separator.
    include_sepa_mixing(scip)
}

/// Whether the separator was already called its maximal number of times at
/// the current node.
fn rounds_exceeded(scip: &Scip, sepa: SepaId, depth: i32) -> bool {
    let ncalls = scip.sepas[sepa].ncallsatnode;
    let d = data(scip, sepa);
    (depth == 0 && d.maxroundsroot >= 0 && ncalls >= d.maxroundsroot) || (depth > 0 && d.maxrounds >= 0 && ncalls >= d.maxrounds)
}

/// `sepaExeclpMixing`: LP solution separation method of separator.
fn sepa_execlp_mixing(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, _allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only call the mixing cut separator a given number of times at each node.
    if rounds_exceeded(scip, sepa, depth) {
        return Ok(());
    }

    // If all the active problem variables are binary, stop.
    if api::get_n_vars(scip) == api::get_n_bin_vars(scip) {
        return Ok(());
    }

    // Call the cut separation.
    let mut cutoff = false;
    let mut ncuts = 0;
    separate_cuts(scip, sepa, NONE, &mut cutoff, &mut ncuts)?;

    // Adjust result code.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if ncuts > 0 {
        ScipResultCode::Separated
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `sepaExecSolMixing`: arbitrary primal solution separation method of
/// separator.
fn sepa_execsol_mixing(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode, _allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Do not run if we have reached the maximal number of consecutive
    // unsuccessful calls.
    let d = data(scip, sepa);
    if d.nunsuccessful >= d.maxnunsuccessful {
        return Ok(());
    }

    // Only call the mixing cut separator a given number of times at each node.
    if rounds_exceeded(scip, sepa, depth) {
        return Ok(());
    }

    // If all the active problem variables are binary, stop.
    if api::get_n_vars(scip) == api::get_n_bin_vars(scip) {
        return Ok(());
    }

    // Call the cut separation.
    let mut cutoff = false;
    let mut ncuts = 0;
    separate_cuts(scip, sepa, sol, &mut cutoff, &mut ncuts)?;

    // Adjust result code.
    let d = data_mut(scip, sepa);
    if cutoff {
        d.nunsuccessful = 0;
        *result = ScipResultCode::Cutoff;
    } else if ncuts > 0 {
        d.nunsuccessful = 0;
        *result = ScipResultCode::Separated;
    } else {
        d.nunsuccessful += 1;
        *result = ScipResultCode::DidNotFind;
    }

    Ok(())
}

/*
 * separator specific interface methods
 */

/// `SCIPincludeSepaMixing`: creates the mixing separator and includes it in
/// SCIP.
pub fn include_sepa_mixing(scip: &mut Scip) -> ScipResult {
    // Create mixing separator data.
    let sepadata = SepaData {
        nunsuccessful: 0,
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
        Some(sepa_execlp_mixing),
        Some(sepa_execsol_mixing),
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non-NULL pointers to callback methods.
    scip.sepas[sepa].sepacopy = Some(sepa_copy_mixing);

    // Add separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    paramset_add_bool(
        scip,
        "separating/mixing/uselocalbounds",
        "Should local bounds be used?",
        target!(bool, uselocalbounds),
        true,
        DEFAULT_USELOCALBOUNDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/mixing/iscutsonints",
        "Should general integer variables be used to generate cuts?",
        target!(bool, iscutsonints),
        true,
        DEFAULT_ISCUTSONINTS,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/mixing/maxrounds",
        "maximal number of mixing separation rounds per node (-1: unlimited)",
        target!(i32, maxrounds),
        false,
        DEFAULT_MAXROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/mixing/maxroundsroot",
        "maximal number of mixing separation rounds in the root node (-1: unlimited)",
        target!(i32, maxroundsroot),
        false,
        DEFAULT_MAXROUNDSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/mixing/maxnunsuccessful",
        "maximal number of consecutive unsuccessful iterations",
        target!(i32, maxnunsuccessful),
        false,
        DEFAULT_MAXNUNSUCCESSFUL,
        -1,
        i32::MAX,
        None,
    )?;

    Ok(())
}
