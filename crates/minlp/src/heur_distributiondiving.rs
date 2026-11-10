//! Port of `heur_distributiondiving.c`: diving heuristic that chooses
//! fixings w.r.t. changes in the solution density.

use std::any::Any;

use crate::{
    api,
    branch_distribution::{row_calc_probability, update_distribution_score, var_calc_distribution_parameters},
    def::INVALID,
    event::{EVENTTYPE_BOUNDCHANGED, Event},
    heur::{DIVETYPE_INTEGRALITY, DiveContext, DiveType, DivesetId, HEURTIMING_AFTERLPPLUNGE, HeurTiming},
    retcode::ScipResult,
    scip::Scip,
    types::{EventhdlrId, HeurId, RowId, ScipResultCode, VarId},
    var::{NONE, var_is_binary},
};

const HEUR_NAME: &str = "distributiondiving";
const HEUR_DESC: &str = "Diving heuristic that chooses fixings w.r.t. changes in the solution density";
const HEUR_DISPCHAR: u8 = b'd';
const HEUR_PRIORITY: i32 = -1003300;
const HEUR_FREQ: i32 = 10;
const HEUR_FREQOFS: i32 = 3;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERLPPLUNGE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = false;
/// The event type to be handled by this event handler.
const EVENT_DISTRIBUTION: crate::event::EventType = EVENTTYPE_BOUNDCHANGED;
const EVENTHDLR_NAME: &str = "eventhdlr_distributiondiving";
/// Bit mask that represents all supported dive types.
const DIVESET_DIVETYPES: DiveType = DIVETYPE_INTEGRALITY;
/// Is this dive set publicly available (ie., can be used by other primal
/// heuristics?)
const DIVESET_ISPUBLIC: bool = false;

/*
 * Default parameter settings
 */

const DEFAULT_MINRELDEPTH: f64 = 0.0;
const DEFAULT_MAXRELDEPTH: f64 = 1.0;
const DEFAULT_MAXLPITERQUOT: f64 = 0.05;
const DEFAULT_MAXLPITEROFS: i32 = 1000;
const DEFAULT_MAXDIVEUBQUOT: f64 = 0.8;
const DEFAULT_MAXDIVEAVGQUOT: f64 = 0.0;
const DEFAULT_MAXDIVEUBQUOTNOSOL: f64 = 0.1;
const DEFAULT_MAXDIVEAVGQUOTNOSOL: f64 = 0.0;
const DEFAULT_BACKTRACK: bool = true;
const DEFAULT_LPRESOLVEDOMCHGQUOT: f64 = 0.15;
const DEFAULT_LPSOLVEFREQ: i32 = 0;
const DEFAULT_ONLYLPBRANCHCANDS: bool = true;

/// The score; largest 'd'ifference, 'l'owest cumulative probability,
/// 'h'ighest c.p., 'v'otes lowest c.p., votes highest c.p. ('w'),
/// 'r'evolving.
const SCOREPARAM_VALUES: &[u8] = b"lhwvd";
const SCOREPARAM_VALUESLEN: i64 = 5;
/// Default scoring parameter to guide the diving.
const DEFAULT_SCOREPARAM: u8 = b'r';
/// Initial seed for random number generation.
const DEFAULT_RANDSEED: u32 = 117;

/// Locally defined heuristic data.
struct HeurData {
    /// Working solution.
    sol: crate::types::SolId,
    /// Event handler pointer.
    eventhdlr: EventhdlrId,
    /// Variables to process bound change events for.
    updatedvars: Vec<VarId>,
    /// Row activity mean values for all rows.
    rowmeans: Vec<f64>,
    /// Row activity variances for all rows.
    rowvariances: Vec<f64>,
    /// Variable upper bounds as currently saved in the row activities.
    currentubs: Vec<f64>,
    /// Variable lower bounds as currently saved in the row activities.
    currentlbs: Vec<f64>,
    /// Count the number of variables with infinite bounds which allow for
    /// always repairing the constraint right hand side.
    rowinfinitiesdown: Vec<i32>,
    /// Count the number of variables with infinite bounds which allow for
    /// always repairing the constraint left hand side.
    rowinfinitiesup: Vec<i32>,
    /// Array of variable positions in the updated variables array.
    varposs: Vec<i32>,
    /// Array of event filter positions for variable events.
    varfilterposs: Vec<i32>,
    /// The current number of variables with pending bound changes.
    nupdatedvars: i32,
    /// Memory size of current arrays, needed for dynamic reallocation.
    memsize: i32,
    /// Memory size of updated vars and varposs array.
    varpossmemsize: i32,

    /// Score user parameter.
    scoreparam: u8,
    /// Score to be used depending on user parameter to use fixed score or
    /// revolve.
    score: u8,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

/*
 * local methods
 */

/// `heurdataEnsureArraySize`: ensure that maxindex + 1 rows can be
/// represented in data arrays; memory gets reallocated with 10% extra space
/// to save some time for future allocations.
fn heurdata_ensure_array_size(scip: &mut Scip, heur: HeurId, maxindex: i32) -> ScipResult {
    let memsize = hd(scip, heur).memsize;

    // maxindex fits in current array -> nothing to do.
    if maxindex < memsize {
        return Ok(());
    }

    // New memory size is the max index + 1 plus 10% additional space.
    let newsize = scip.set.feas_ceil((maxindex + 1) as f64 * 1.1) as i32;
    debug_assert!(newsize > memsize);

    // Alloc memory arrays for row information.
    if memsize == 0 {
        debug_assert!(scip.set.stage == crate::scip::Stage::Solving);

        let vars = api::get_vars(scip).to_vec();
        let nvars = api::get_n_vars(scip) as usize;
        debug_assert!(nvars > 0);

        // Allocate variable update event processing array storage.
        let eventhdlr = {
            let h = hdm(scip, heur);
            h.varfilterposs = vec![0; nvars];
            h.varposs = vec![0; nvars];
            h.updatedvars = vec![NONE; nvars];
            h.currentubs = vec![0.0; nvars];
            h.currentlbs = vec![0.0; nvars];
            h.varpossmemsize = nvars as i32;
            h.nupdatedvars = 0;
            h.eventhdlr
        };

        // Init variable event processing data.
        for (v, &var) in vars.iter().enumerate().take(nvars) {
            debug_assert!(scip.vars[var].probindex == v as i32);

            // Set up variable events to catch bound changes.
            let mut filterpos = -1;
            api::catch_var_event(scip, var, EVENT_DISTRIBUTION, eventhdlr, 0, Some(&mut filterpos))?;
            debug_assert!(filterpos >= 0);

            let h = hdm(scip, heur);
            h.varfilterposs[v] = filterpos;
            h.varposs[v] = -1;
            h.updatedvars[v] = NONE;
            h.currentlbs[v] = INVALID;
            h.currentubs[v] = INVALID;
        }
    }

    // Loop over extended arrays and invalidate data to trigger initialization
    // of this row when necessary.
    let h = hdm(scip, heur);
    h.rowmeans.resize(newsize as usize, INVALID);
    h.rowvariances.resize(newsize as usize, INVALID);
    h.rowinfinitiesdown.resize(newsize as usize, 0);
    h.rowinfinitiesup.resize(newsize as usize, 0);
    for r in memsize as usize..newsize as usize {
        h.rowmeans[r] = INVALID;
        h.rowvariances[r] = INVALID;
        h.rowinfinitiesdown[r] = 0;
        h.rowinfinitiesup[r] = 0;
    }

    // Adjust memsize.
    h.memsize = newsize;

    Ok(())
}

/// `heurdataUpdateCurrentBounds`: update the variables current lower and
/// upper bound.
fn heurdata_update_current_bounds(scip: &mut Scip, heur: HeurId, var: VarId) {
    let varindex = scip.vars[var].probindex as usize;
    let lblocal = scip.vars[var].locdom.lb;
    let ublocal = scip.vars[var].locdom.ub;

    debug_assert!(scip.set.is_feas_le(lblocal, ublocal));

    let h = hdm(scip, heur);
    h.currentlbs[varindex] = lblocal;
    h.currentubs[varindex] = ublocal;
}

/// `rowCalculateGauss`: calculates the initial mean and variance of the row
/// activity normal distribution; returns (mu, sigma2, rowinfinitiesdown,
/// rowinfinitiesup).
fn row_calculate_gauss(scip: &mut Scip, heur: HeurId, row: RowId) -> (f64, f64, i32, i32) {
    let nrowvals = scip.rows[row].len as usize;

    let mut mu = scip.rows[row].constant;
    let mut sigma2 = 0.0;
    let mut rowinfinitiesdown = 0;
    let mut rowinfinitiesup = 0;

    // Loop over nonzero row coefficients and sum up the variable
    // contributions to mu and sigma2.
    for c in 0..nrowvals {
        let colvar = scip.cols[scip.rows[row].cols[c]].var;

        let colval = scip.rows[row].vals[c];
        let colvarlb = scip.vars[colvar].locdom.lb;
        let colvarub = scip.vars[colvar].locdom.ub;

        let varindex = scip.vars[colvar].probindex as usize;

        // Variable bounds need to be watched from now on.
        if hd(scip, heur).currentlbs[varindex] == INVALID {
            heurdata_update_current_bounds(scip, heur, colvar);
        }

        // Variables with infinite bounds are skipped for the calculation of
        // the variance; they need to be accounted for by the counters for
        // infinite row activity decrease and increase and they are used to
        // shift the row activity mean in case they have one nonzero, but
        // finite bound.
        if scip.set.is_infinity(-colvarlb) || scip.set.is_infinity(colvarub) {
            if scip.set.is_infinity(colvarub) {
                // An infinite upper bound gives the row an infinite maximum
                // activity or minimum activity, if the coefficient is
                // positive or negative, resp.
                if colval < 0.0 {
                    rowinfinitiesdown += 1;
                } else {
                    rowinfinitiesup += 1;
                }
            }

            // An infinite lower bound gives the row an infinite maximum
            // activity or minimum activity, if the coefficient is negative
            // or positive, resp.
            if scip.set.is_infinity(-colvarlb) {
                if colval > 0.0 {
                    rowinfinitiesdown += 1;
                } else {
                    rowinfinitiesup += 1;
                }
            }
        }
        let (varmean, varvariance) = var_calc_distribution_parameters(scip, colvarlb, colvarub, scip.vars[colvar].vartype, scip.vars[colvar].varimpltype);

        // Actual values are updated; the contribution of the variable to mu
        // is the arithmetic mean of its bounds.
        mu += colval * varmean;

        // The variance contribution of a variable is c^2 * (u - l)^2 / 12.0
        // for continuous and c^2 * ((u - l + 1)^2 - 1) / 12.0 for integer.
        let squarecoeff = colval * colval;
        sigma2 += squarecoeff * varvariance;

        debug_assert!(!scip.set.is_feas_negative(sigma2));
    }

    (mu, sigma2, rowinfinitiesdown, rowinfinitiesup)
}

/// `calcBranchScore`: calculate the branching score of a variable,
/// depending on the chosen score parameter.
fn calc_branch_score(scip: &mut Scip, heur: HeurId, var: VarId, lpsolval: f64, upscore: &mut f64, downscore: &mut f64, scoreparam: u8) -> ScipResult {
    debug_assert!(!scip.set.is_integral(lpsolval) || var_is_binary(scip, var));

    let varcol = scip.vars[var].data.col;

    let ncolrows = scip.cols[varcol].len as usize;
    let varlb = scip.vars[var].locdom.lb;
    let varub = scip.vars[var].locdom.ub;
    debug_assert!(varub - varlb > 0.5);
    let vartype = scip.vars[var].vartype;
    let impltype = scip.vars[var].varimpltype;

    // Calculate mean and variance of variable uniform distribution before and
    // after branching.
    let (currentmean, squaredbounddiff) = var_calc_distribution_parameters(scip, varlb, varub, vartype, impltype);

    // Unfixed binary variables may have an integer solution value in the LP
    // solution, eg, at the presence of indicator constraints.
    let (newlb, newub) = if !var_is_binary(scip, var) {
        (scip.set.feas_ceil(lpsolval), scip.set.feas_floor(lpsolval))
    } else {
        (1.0, 0.0)
    };

    // Calculate the variable's uniform distribution after branching up and
    // down, respectively.
    let (meanup, squaredbounddiffup) = var_calc_distribution_parameters(scip, newlb, varub, vartype, impltype);

    // Calculate the distribution mean and variance for a variable with finite
    // lower bound.
    let (meandown, squaredbounddiffdown) = var_calc_distribution_parameters(scip, varlb, newub, vartype, impltype);

    // Initialize the variable's up and down score.
    *upscore = 0.0;
    *downscore = 0.0;

    // Loop over the variable rows and calculate the up and down score.
    for i in 0..ncolrows {
        let row = scip.cols[varcol].rows[i];
        let rowval = scip.cols[varcol].vals[i];

        // We access the rows by their index.
        let rowpos = scip.rows[row].index;

        // Call method to ensure sufficient data capacity.
        heurdata_ensure_array_size(scip, heur, rowpos)?;
        let rowpos = rowpos as usize;

        // Calculate row activity distribution if this is the first candidate
        // to appear in this row.
        if hd(scip, heur).rowmeans[rowpos] == INVALID {
            let (mu, sigma2, down, up) = row_calculate_gauss(scip, heur, row);
            let h = hdm(scip, heur);
            h.rowmeans[rowpos] = mu;
            h.rowvariances[rowpos] = sigma2;
            h.rowinfinitiesdown[rowpos] = down;
            h.rowinfinitiesup[rowpos] = up;
        }

        // Retrieve the row distribution parameters from the branch rule data.
        let h = hd(scip, heur);
        let rowmean = h.rowmeans[rowpos];
        let rowvariance = h.rowvariances[rowpos];
        let rowinfinitiesdown = h.rowinfinitiesdown[rowpos];
        let rowinfinitiesup = h.rowinfinitiesup[rowpos];
        debug_assert!(!scip.set.is_negative(rowvariance));

        let currentrowprob = row_calc_probability(scip, row, rowmean, rowvariance, rowinfinitiesdown, rowinfinitiesup);

        // Get variable's current expected contribution to row activity.
        let squaredcoeff = rowval * rowval;

        // First, get the probability change for the row if the variable is
        // branched on upwards. The probability can only be affected if the
        // variable upper bound is finite.
        let newrowprobup = if !scip.set.is_infinity(varub) {
            // Calculate how branching would affect the row parameters.
            let changedrowmean = rowmean + rowval * (meanup - currentmean);
            let mut changedrowvariance = rowvariance + squaredcoeff * (squaredbounddiffup - squaredbounddiff);
            changedrowvariance = if 0.0 >= changedrowvariance { 0.0 } else { changedrowvariance };

            let mut rowinftiesdownafterbranch = rowinfinitiesdown;
            let mut rowinftiesupafterbranch = rowinfinitiesup;

            // Account for changes of the row's infinite bound contributions.
            if scip.set.is_infinity(-varlb) && rowval < 0.0 {
                rowinftiesupafterbranch -= 1;
            }
            if scip.set.is_infinity(-varlb) && rowval > 0.0 {
                rowinftiesdownafterbranch -= 1;
            }

            debug_assert!(rowinftiesupafterbranch >= 0);
            debug_assert!(rowinftiesdownafterbranch >= 0);
            row_calc_probability(scip, row, changedrowmean, changedrowvariance, rowinftiesdownafterbranch, rowinftiesupafterbranch)
        } else {
            currentrowprob
        };

        // Do the same for the other branching direction.
        let newrowprobdown = if !scip.set.is_infinity(varlb) {
            let changedrowmean = rowmean + rowval * (meandown - currentmean);
            let mut changedrowvariance = rowvariance + squaredcoeff * (squaredbounddiffdown - squaredbounddiff);
            changedrowvariance = if 0.0 >= changedrowvariance { 0.0 } else { changedrowvariance };

            let mut rowinftiesdownafterbranch = rowinfinitiesdown;
            let mut rowinftiesupafterbranch = rowinfinitiesup;

            // Account for changes of the row's infinite bound contributions.
            if scip.set.is_infinity(varub) && rowval > 0.0 {
                rowinftiesupafterbranch -= 1;
            }
            if scip.set.is_infinity(varub) && rowval < 0.0 {
                rowinftiesdownafterbranch -= 1;
            }

            debug_assert!(rowinftiesdownafterbranch >= 0);
            debug_assert!(rowinftiesupafterbranch >= 0);
            row_calc_probability(scip, row, changedrowmean, changedrowvariance, rowinftiesdownafterbranch, rowinftiesupafterbranch)
        } else {
            currentrowprob
        };

        // Update the up and down score depending on the chosen scoring
        // parameter.
        update_distribution_score(scip, currentrowprob, newrowprobup, newrowprobdown, upscore, downscore, scoreparam)?;
    }

    Ok(())
}

/// `heurdataFreeArrays`: free heuristic data.
fn heurdata_free_arrays(scip: &mut Scip, heur: HeurId) -> ScipResult {
    let varpossmemsize = hd(scip, heur).varpossmemsize;
    if varpossmemsize > 0 {
        debug_assert!(varpossmemsize == api::get_n_vars(scip));

        let vars = api::get_vars(scip).to_vec();
        let eventhdlr = hd(scip, heur).eventhdlr;
        for v in (0..varpossmemsize as usize).rev() {
            let var = vars[v];
            debug_assert!(v as i32 == scip.vars[var].probindex);
            let filterpos = hd(scip, heur).varfilterposs[v];
            api::drop_var_event(scip, var, EVENT_DISTRIBUTION, eventhdlr, 0, filterpos)?;
        }
        let h = hdm(scip, heur);
        h.currentlbs = Vec::new();
        h.currentubs = Vec::new();
        h.updatedvars = Vec::new();
        h.varposs = Vec::new();
        h.varfilterposs = Vec::new();
    }

    let h = hdm(scip, heur);
    if h.memsize > 0 {
        h.rowvariances = Vec::new();
        h.rowmeans = Vec::new();
        h.rowinfinitiesup = Vec::new();
        h.rowinfinitiesdown = Vec::new();

        h.memsize = 0;
    }

    h.varpossmemsize = 0;
    h.nupdatedvars = 0;

    Ok(())
}

/// `heurdataAddBoundChangeVar`: add variable to the bound change event
/// queue; skipped if variable is already in there, or if variable has no
/// row currently watched.
fn heurdata_add_bound_change_var(scip: &mut Scip, heur: HeurId, var: VarId) {
    let varindex = scip.vars[var].probindex;

    // If variable is not active, it should not be watched.
    if varindex == -1 {
        return;
    }
    let varindex = varindex as usize;
    let h = hdm(scip, heur);
    let varpos = h.varposs[varindex];
    debug_assert!(varpos < h.nupdatedvars);

    // Nothing to do if variable is already in the queue.
    if varpos >= 0 {
        debug_assert!(h.updatedvars[varpos as usize] == var);
        return;
    }

    // We don't need to enqueue the variable if it hasn't been watched so far.
    if h.currentlbs[varindex] == INVALID {
        return;
    }

    // Add the variable to the heuristic data of variables to process updates
    // for.
    debug_assert!(h.varpossmemsize > h.nupdatedvars);
    let varpos = h.nupdatedvars;
    h.updatedvars[varpos as usize] = var;
    h.varposs[varindex] = varpos;
    h.nupdatedvars += 1;
}

/// `heurdataPopBoundChangeVar`: returns the next unprocessed variable (last
/// in, first out) with pending bound changes, or NONE.
fn heurdata_pop_bound_change_var(scip: &mut Scip, heur: HeurId) -> VarId {
    let nupdatedvars = hd(scip, heur).nupdatedvars;

    // Return if no variable is currently pending.
    if nupdatedvars == 0 {
        return NONE;
    }

    let varpos = (nupdatedvars - 1) as usize;
    let var = hd(scip, heur).updatedvars[varpos];
    let varindex = scip.vars[var].probindex as usize;

    let h = hdm(scip, heur);
    debug_assert!(varpos as i32 == h.varposs[varindex]);
    h.varposs[varindex] = -1;
    h.nupdatedvars -= 1;

    var
}

/// `varProcessBoundChanges`: process a variable from the queue of changed
/// variables.
fn var_process_bound_changes(scip: &mut Scip, heur: HeurId, var: VarId) -> ScipResult {
    // Ensure that this is a probing bound change.
    debug_assert!(api::in_probing(scip));

    let varcol = scip.vars[var].data.col;
    let ncolrows = scip.cols[varcol].len as usize;

    let varindex = scip.vars[var].probindex as usize;

    let oldlb = hd(scip, heur).currentlbs[varindex];
    let oldub = hd(scip, heur).currentubs[varindex];

    // Skip update if the variable has never been subject of previously
    // calculated row activities.
    if oldlb == INVALID {
        return Ok(());
    }

    let newlb = scip.vars[var].locdom.lb;
    let newub = scip.vars[var].locdom.ub;

    // Skip update if the bound change events have cancelled out.
    if scip.set.is_feas_eq(oldlb, newlb) && scip.set.is_feas_eq(oldub, newub) {
        return Ok(());
    }

    // Calculate old and new variable distribution mean and variance.
    let vartype = scip.vars[var].vartype;
    let impltype = scip.vars[var].varimpltype;
    let (oldmean, oldvariance) = var_calc_distribution_parameters(scip, oldlb, oldub, vartype, impltype);
    let (newmean, newvariance) = var_calc_distribution_parameters(scip, newlb, newub, vartype, impltype);

    // Loop over all rows of this variable and update activity distribution.
    for r in 0..ncolrows {
        let rowpos = scip.rows[scip.cols[varcol].rows[r]].index;
        debug_assert!(rowpos >= 0);

        heurdata_ensure_array_size(scip, heur, rowpos)?;
        let rowpos = rowpos as usize;

        let newubinf = scip.set.is_infinity(newub);
        let oldubinf = scip.set.is_infinity(oldub);
        let newlbinf = scip.set.is_infinity(newlb);
        let oldlbinf = scip.set.is_infinity(oldlb);
        let coeff = scip.cols[varcol].vals[r];
        let h = hdm(scip, heur);

        // Only consider rows for which activity distribution was already
        // calculated.
        if h.rowmeans[rowpos] != INVALID {
            let coeffsquared = coeff * coeff;

            // Update variable contribution to row activity distribution.
            h.rowmeans[rowpos] += coeff * (newmean - oldmean);
            h.rowvariances[rowpos] += coeffsquared * (newvariance - oldvariance);
            h.rowvariances[rowpos] = if 0.0 >= h.rowvariances[rowpos] { 0.0 } else { h.rowvariances[rowpos] };

            // Account for changes of the infinite contributions to row
            // activities.
            if coeff > 0.0 {
                // If the coefficient is positive, upper bounds affect activity
                // up.
                if newubinf && !oldubinf {
                    h.rowinfinitiesup[rowpos] += 1;
                } else if !newubinf && oldubinf {
                    h.rowinfinitiesup[rowpos] -= 1;
                }

                if newlbinf && !oldlbinf {
                    h.rowinfinitiesdown[rowpos] += 1;
                } else if !newlbinf && oldlbinf {
                    h.rowinfinitiesdown[rowpos] -= 1;
                }
            } else if coeff < 0.0 {
                if newubinf && !oldubinf {
                    h.rowinfinitiesdown[rowpos] += 1;
                } else if !newubinf && oldubinf {
                    h.rowinfinitiesdown[rowpos] -= 1;
                }

                if newlbinf && !oldlbinf {
                    h.rowinfinitiesup[rowpos] += 1;
                } else if !newlbinf && oldlbinf {
                    h.rowinfinitiesup[rowpos] -= 1;
                }
            }
            debug_assert!(h.rowinfinitiesdown[rowpos] >= 0);
            debug_assert!(h.rowinfinitiesup[rowpos] >= 0);
        }
    }

    // Store the new local bounds in the data.
    heurdata_update_current_bounds(scip, heur, var);

    Ok(())
}

/*
 * Callback methods
 */

/// `heurCopyDistributiondiving`: copy method for primal heuristic plugins.
fn heur_copy_distributiondiving(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_distributiondiving(scip)
}

/// `heurInitDistributiondiving`: initialization method of primal heuristic.
fn heur_init_distributiondiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create working solution.
    let sol = api::create_sol(scip, heur)?;
    hdm(scip, heur).sol = sol;
    Ok(())
}

/// `heurExitDistributiondiving`: deinitialization method of primal
/// heuristic.
fn heur_exit_distributiondiving(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free working solution.
    let sol = hd(scip, heur).sol;
    api::free_sol(scip, sol)?;
    hdm(scip, heur).sol = NONE;
    Ok(())
}

/// `divesetGetScoreDistributiondiving`: scoring callback for distribution
/// diving. Best candidate maximizes the distribution score.
#[allow(clippy::too_many_arguments)]
fn diveset_get_score_distributiondiving(
    scip: &mut Scip,
    diveset: DivesetId,
    _divetype: DiveType,
    cand: VarId,
    candsol: f64,
    _candsfrac: f64,
    score: &mut f64,
    roundup: &mut bool,
) -> ScipResult {
    let heur = scip.divesets[diveset].heur;

    // Process pending bound change events.
    while hd(scip, heur).nupdatedvars > 0 {
        // Pop the next variable from the queue and process its bound changes.
        let nextvar = heurdata_pop_bound_change_var(scip, heur);
        debug_assert!(nextvar != NONE);
        var_process_bound_changes(scip, heur, nextvar)?;
    }

    let varindex = scip.vars[cand].probindex;

    // Terminate with a penalty for inactive variables, which the plugin can
    // currently not score this should never happen with default settings
    // where only LP branching candidates are iterated, but might occur if
    // other constraint handlers try to score an inactive variable that was
    // (multi-)aggregated or negated.
    if varindex == -1 {
        *score = -1.0;
        *roundup = false;

        return Ok(());
    }

    // If the heuristic has not captured the variable bounds yet, this can be
    // done now.
    if hd(scip, heur).currentlbs[varindex as usize] == INVALID {
        heurdata_update_current_bounds(scip, heur, cand);
    }

    let mut upscore = 0.0;
    let mut downscore = 0.0;

    // Loop over candidate rows and determine the candidate up- and down-
    // branching score w.r.t. the score parameter.
    let scoreparam = hd(scip, heur).score;
    calc_branch_score(scip, heur, cand, candsol, &mut upscore, &mut downscore, scoreparam)?;

    // Score is simply the maximum of the two individual scores.
    *roundup = upscore > downscore;
    *score = if upscore >= downscore { upscore } else { downscore };

    Ok(())
}

/// `heurExecDistributiondiving`: execution method of primal heuristic.
fn heur_exec_distributiondiving(scip: &mut Scip, heur: HeurId, _heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Get heuristic's data.
    let nlprows = api::get_n_lp_rows(scip);
    if nlprows == 0 {
        return Ok(());
    }

    // Terminate if there are no integer variables (note that, e.g., SOS1
    // variables may be present).
    if api::get_n_cont_vars(scip) + api::get_n_cont_impl_vars(scip) == api::get_n_vars(scip) {
        return Ok(());
    }

    // Select and store the scoring parameter for this call of the heuristic.
    let ncalls = scip.heurs[heur].ncalls;
    let h = hdm(scip, heur);
    if h.scoreparam == b'r' {
        h.score = SCOREPARAM_VALUES[(ncalls % SCOREPARAM_VALUESLEN) as usize];
    } else {
        h.score = h.scoreparam;
    }

    heurdata_ensure_array_size(scip, heur, nlprows)?;
    let diveset = scip.heurs[heur].divesets[0];
    let sol = hd(scip, heur).sol;

    crate::heuristics::perform_generic_diving_algorithm(scip, diveset, sol, heur, result, nodeinfeasible, -1, -1, -1.0, DiveContext::Single)?;

    heurdata_free_arrays(scip, heur)?;

    Ok(())
}

/// `eventExecDistribution`: event execution method of distribution
/// branching which handles bound change events of variables.
fn event_exec_distribution(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, _eventdata: usize) -> ScipResult {
    let heur: HeurId = *scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref::<HeurId>().unwrap();
    let var = event.data.var;

    // Add the variable to the queue of unprocessed variables; method itself
    // ensures that every variable is added at most once.
    heurdata_add_bound_change_var(scip, heur, var);

    Ok(())
}

/*
 * heuristic specific interface methods
 */

/// `SCIPincludeHeurDistributiondiving`: creates the distributiondiving
/// heuristic and includes it in SCIP.
pub fn include_heur_distributiondiving(scip: &mut Scip) -> ScipResult {
    // Create event handler first to finish heuristic data; its data (the
    // heuristic) is set once the heuristic is included.
    let eventhdlr = api::include_eventhdlr_basic(
        scip,
        EVENTHDLR_NAME,
        "event handler for dynamic acitivity distribution updating",
        event_exec_distribution,
        None,
    )?;

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
        heur_exec_distributiondiving,
        Some(Box::new(HeurData {
            sol: NONE,
            eventhdlr,
            updatedvars: Vec::new(),
            rowmeans: Vec::new(),
            rowvariances: Vec::new(),
            currentubs: Vec::new(),
            currentlbs: Vec::new(),
            rowinfinitiesdown: Vec::new(),
            rowinfinitiesup: Vec::new(),
            varposs: Vec::new(),
            varfilterposs: Vec::new(),
            nupdatedvars: 0,
            memsize: 0,
            varpossmemsize: 0,
            scoreparam: DEFAULT_SCOREPARAM,
            score: DEFAULT_SCOREPARAM,
        }) as Box<dyn Any>),
    )?;
    scip.eventhdlrs[eventhdlr].eventhdlrdata = Some(Box::new(heur) as Box<dyn Any>);

    // Primal heuristic is safe to use in exact solving mode.
    scip.heurs[heur].exact = true;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_distributiondiving);
    h.heurinit = Some(heur_init_distributiondiving);
    h.heurexit = Some(heur_exit_distributiondiving);

    // Add diveset with the defined scoring function.
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
        diveset_get_score_distributiondiving,
        None,
    )?;

    crate::paramset::paramset_add_char(
        scip,
        "heuristics/distributiondiving/scoreparam",
        "the score;largest 'd'ifference, 'l'owest cumulative probability,'h'ighest c.p., 'v'otes lowest c.p., votes highest c.p.('w'), 'r'evolving",
        crate::data_target!(u8, heurs, heur, heurdata, HeurData, scoreparam),
        true,
        DEFAULT_SCOREPARAM,
        Some("lvdhwr"),
        None,
    )?;

    Ok(())
}
