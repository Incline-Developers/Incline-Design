//! Port of `sepa_aggregation.c`: flow cover, knapsack cover and complemented
//! mixed integer rounding cuts from aggregations of LP rows, with the dummy
//! child separators `flowcover`, `cmir` and `knapsackcover` whose
//! frequencies switch the three cut types.

use std::any::Any;

use crate::{
    api,
    cuts::{
        AggrRow, CutOut, aggr_row_add_objective_function, aggr_row_add_row, aggr_row_clear, aggr_row_create, aggr_row_get_probvar_value, aggr_row_has_row_been_added,
        aggr_row_remove_zeros, calc_flow_cover, calc_knapsack_cover, cut_generation_heuristic_cmir,
    },
    def::{REAL_MAX, max, min},
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_lp,
    types::{RowId, ScipResultCode, SepaId, SolId},
    var::NONE,
};

const SEPA_NAME: &str = "aggregation";
const SEPA_DESC: &str = "aggregation heuristic for complemented mixed integer rounding cuts and flowcover cuts";
const SEPA_PRIORITY: i32 = -3000;
const SEPA_FREQ: i32 = 10;
const SEPA_MAXBOUNDDIST: f64 = 1.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = false;

const DEFAULT_MAXROUNDS: i32 = -1;
const DEFAULT_MAXROUNDSROOT: i32 = -1;
const DEFAULT_MAXTRIES: i32 = 200;
const DEFAULT_MAXTRIESROOT: i32 = -1;
const DEFAULT_MAXFAILS: i32 = 20;
const DEFAULT_MAXFAILSROOT: i32 = 100;
const DEFAULT_MAXAGGRS: i32 = 3;
const DEFAULT_MAXAGGRSROOT: i32 = 6;
const DEFAULT_MAXSEPACUTS: i32 = 100;
const DEFAULT_MAXSEPACUTSROOT: i32 = 500;
const DEFAULT_MAXSLACK: f64 = 0.0;
const DEFAULT_MAXSLACKROOT: f64 = 0.1;
const DEFAULT_DENSITYSCORE: f64 = 1e-4;
const DEFAULT_SLACKSCORE: f64 = 1e-3;
const DEFAULT_MAXAGGDENSITY: f64 = 0.20;
const DEFAULT_MAXROWDENSITY: f64 = 0.05;
const DEFAULT_DENSITYOFFSET: i32 = 100;
const DEFAULT_MAXROWFAC: f64 = 1e+4;
const DEFAULT_MAXTESTDELTA: i32 = -1;
const DEFAULT_AGGRTOL: f64 = 1e-2;
const DEFAULT_TRYNEGSCALING: bool = true;
const DEFAULT_FIXINTEGRALRHS: bool = true;
const DEFAULT_DYNAMICCUTS: bool = true;

const BOUNDSWITCH: f64 = 0.5;
const POSTPROCESS: bool = true;
/// We allow variable bound substitution for variables with continuous
/// vartype only.
const VARTYPEUSEVBDS: i32 = 2;
const MINFRAC: f64 = 0.05;
const MAXFRAC: f64 = 0.999;

/// Separator data.
#[derive(Default)]
struct SepaData {
    maxslack: f64,
    maxslackroot: f64,
    densityscore: f64,
    slackscore: f64,
    maxaggdensity: f64,
    maxrowdensity: f64,
    maxrowfac: f64,
    aggrtol: f64,
    maxrounds: i32,
    maxroundsroot: i32,
    maxtries: i32,
    maxtriesroot: i32,
    maxfails: i32,
    maxfailsroot: i32,
    maxaggrs: i32,
    maxaggrsroot: i32,
    maxsepacuts: i32,
    maxsepacutsroot: i32,
    densityoffset: i32,
    maxtestdelta: i32,
    trynegscaling: bool,
    fixintegralrhs: bool,
    dynamiccuts: bool,
    sepflowcover: bool,
    sepknapsackcover: bool,
    sepcmir: bool,
    cmir: SepaId,
    flowcover: SepaId,
    knapsackcover: SepaId,
}

fn data(scip: &Scip, sepa: SepaId) -> &SepaData {
    scip.sepas[sepa].sepadata.as_ref().unwrap().downcast_ref::<SepaData>().unwrap()
}

fn data_mut(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/// Data used for aggregation of row (`AGGREGATIONDATA`).
struct AggregationData {
    /// Bound distance of continuous variables.
    bounddist: Vec<f64>,
    /// Problem indices of the continuous variables corresponding to the
    /// bounddistance value.
    bounddistinds: Vec<i32>,
    /// Number of continuous variables that are not at their bounds.
    nbounddistvars: usize,
    /// Array of rows suitable for substitution of continuous variable.
    aggrrows: Vec<RowId>,
    /// Coefficient of continuous variable in row that is suitable for
    /// substitution of that variable.
    aggrrowscoef: Vec<f64>,
    /// Array with start positions of suitable rows for substitution for each
    /// continuous variable with non-zero bound distance.
    aggrrowsstart: Vec<usize>,
    /// Array with number of rows suitable for substitution that only contain
    /// one continuous variable that is not at it's bound.
    ngoodaggrrows: Vec<usize>,
    /// Number of continuous variables that are not at their bounds for each
    /// row.
    nbadvarsinrow: Vec<i32>,
    /// Store aggregation row here so that it can be reused.
    aggrrow: AggrRow,
}

/*
 * Local methods
 */

/// The cut `addCut` adds.
struct Cut<'a> {
    coefs: &'a [f64],
    inds: &'a [i32],
    nnz: i32,
    rhs: f64,
    efficacy: f64,
    islocal: bool,
    removable: bool,
    rank: i32,
}

/// `addCut`: adds given cut to LP if violated; returns the cut if it was
/// added (captured, the caller releases it).
#[allow(clippy::too_many_arguments)]
fn add_cut(scip: &mut Scip, sol: SolId, sepa: SepaId, mut makeintegral: bool, cut: &Cut, cutclassname: &str, cutoff: &mut bool, ncuts: &mut i32) -> ScipResult<Option<RowId>> {
    *cutoff = false;

    if cut.nnz > 0 && api::is_efficacious(scip, cut.efficacy) {
        // Get active problem variables.
        let vars = api::get_vars(scip).to_vec();

        // Create cut name.
        let cutname = format!("{cutclassname}{}_{}", scip.stat.nlps, *ncuts);

        let inf = scip.set.infinity();
        loop {
            // tryagain:
            let mut row = scip_lp::create_empty_row_sepa(scip, sepa, &cutname, -inf, cut.rhs, cut.islocal, false, cut.removable)?;

            scip_lp::cache_row_extensions(scip, row);

            for i in 0..cut.nnz as usize {
                api::add_var_to_row(scip, row, vars[cut.inds[i] as usize], cut.coefs[i])?;
            }

            // Set cut rank.
            scip.rows[row].rank = cut.rank;

            // If requested, try to scale the cut to integral values but only if the
            // scaling is small; otherwise keep the fractional cut.
            let success = if makeintegral && crate::lp::row_get_num_int_cols(scip, row) == scip.rows[row].len {
                let (eps, sumeps) = (scip.set.epsilon(), scip.set.sumepsilon());
                let success = crate::lp::row_make_integral(scip, row, -eps, sumeps, 1000, 1000.0, false)?;

                if scip.set.is_infinity(scip.rows[row].rhs) {
                    // Release the row.
                    scip_lp::release_row(scip, &mut row)?;

                    // The scaling destroyed the cut, so try to add it again, but this
                    // time do not scale it.
                    makeintegral = false;
                    continue;
                }
                success
            } else {
                false
            };

            if success && !scip_lp::is_cut_efficacious(scip, sol, row) {
                scip_lp::release_row(scip, &mut row)?;

                // The cut is not efficacious anymore due to the scaling, so do not add
                // it.
                return Ok(None);
            }

            scip_lp::flush_row_extensions(scip, row)?;

            if crate::cutpool::cutpool_is_cut_new(scip, crate::cutpool::GLOBALCUTPOOL, row) {
                *ncuts += 1;

                if !cut.islocal {
                    api::add_pool_cut(scip, row)?;
                } else {
                    *cutoff = scip_lp::add_row(scip, row, false)?;
                }

                return Ok(Some(row));
            } else {
                // Release the row.
                scip_lp::release_row(scip, &mut row)?;
            }
            break;
        }
    }

    Ok(None)
}

/// `setupAggregationData`: setup data for aggregating rows.
fn setup_aggregation_data(scip: &mut Scip, sol: SolId, allowlocal: bool) -> AggregationData {
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    let nintvars = api::get_n_int_vars(scip) as usize;
    let nimplvars = api::get_n_impl_vars(scip) as usize;
    let ncontvars = api::get_n_cont_vars(scip) as usize;
    let nrows = lp_rows(scip).len();

    let mut aggrdata = AggregationData {
        bounddist: vec![0.0; ncontvars + nimplvars],
        bounddistinds: vec![0; ncontvars + nimplvars],
        nbounddistvars: 0,
        aggrrows: Vec::new(),
        aggrrowscoef: Vec::new(),
        aggrrowsstart: vec![0; ncontvars + nimplvars + 1],
        ngoodaggrrows: vec![0; ncontvars + nimplvars],
        nbadvarsinrow: vec![0; nrows],
        aggrrow: aggr_row_create(scip),
    };

    let firstcontvar = nvars - ncontvars;

    for i in nbinvars + nintvars..nvars {
        // Compute the bound distance of the variable.
        let x = &scip.vars[vars[i]];
        let (mut bestlb, mut bestub) = if allowlocal { (x.locdom.lb, x.locdom.ub) } else { (x.glbdom.lb, x.glbdom.ub) };

        let (mut bestvlb, mut bestvlbidx) = (0.0, 0);
        let (mut bestvub, mut bestvubidx) = (0.0, 0);
        crate::var::var_get_closest_vlb(scip, vars[i], sol, &mut bestvlb, &mut bestvlbidx);
        crate::var::var_get_closest_vub(scip, vars[i], sol, &mut bestvub, &mut bestvubidx);
        if bestvlbidx >= 0 {
            bestlb = max(bestlb, bestvlb);
        }
        if bestvubidx >= 0 {
            bestub = min(bestub, bestvub);
        }

        let primsol = api::get_sol_val(scip, sol, vars[i]);
        let distlb = primsol - bestlb;
        let distub = bestub - primsol;

        let mut bounddist = min(distlb, distub);
        bounddist = max(bounddist, 0.0);

        // Prefer continuous variables over implicit integers to be aggregated out.
        if i < firstcontvar {
            bounddist *= 0.1;
        }

        // When variable is not at its bound, we want to project it out, so add it
        // to the aggregation data.
        if !scip.set.is_zero(bounddist) {
            let k = aggrdata.nbounddistvars;
            aggrdata.nbounddistvars += 1;

            aggrdata.bounddist[k] = bounddist;
            aggrdata.bounddistinds[k] = i as i32;
            aggrdata.aggrrowsstart[k] = aggrdata.aggrrows.len();

            // The current variable is a bad variable (continuous, not at its bound):
            // increase the number of bad variable count on each row this variable
            // appears in; also each row this variable appears in is added to the
            // list of rows that can be used to aggregate this variable.
            if crate::var::var_is_in_lp(scip, vars[i]) {
                let col = scip.vars[vars[i]].data.col;
                let c = &scip.cols[col];
                let ncolnonzeros = c.nlprows as usize;
                let colrows = c.rows.0[..ncolnonzeros].to_vec();
                let colrowvals = c.vals.0[..ncolnonzeros].to_vec();

                for k in 0..ncolnonzeros {
                    // Ignore modifiable rows and local rows if those are not permitted.
                    let r = &scip.rows[colrows[k]];
                    if r.modifiable || (!allowlocal && r.local) {
                        continue;
                    }

                    aggrdata.nbadvarsinrow[r.lppos as usize] += 1;
                    aggrdata.aggrrows.push(colrows[k]);
                    aggrdata.aggrrowscoef.push(colrowvals[k]);
                }
            }
        }
    }

    // Add sentinel entry at the end.
    aggrdata.aggrrowsstart[aggrdata.nbounddistvars] = aggrdata.aggrrows.len();

    // For each continous variable that is not at its bounds check if there is
    // a row where it is the only such variable ("good" rows). In the array
    // with the rows that are suitable for substituting this variable move the
    // good rows to the beginning and store the number of good rows for each of
    // the variables. If a variable has at least one good row, then it is a
    // "better" variable and we make the value of the bounddistance for this
    // variable negative, to mark it. Note that better variables are continuous
    // variables that are not at their bounds and can be projected out without
    // introducing bad variables (by using a good row).
    let mut beg = aggrdata.aggrrowsstart[0];
    for i in 0..aggrdata.nbounddistvars {
        let end = aggrdata.aggrrowsstart[i + 1];
        let mut ngoodrows = 0;
        for k in beg..end {
            let r = &scip.rows[aggrdata.aggrrows[k]];
            let lppos = r.lppos as usize;
            if aggrdata.nbadvarsinrow[lppos] == 1 && scip.set.is_eq(r.lhs, r.rhs) {
                let nextgoodrowpos = beg + ngoodrows;
                if k > nextgoodrowpos {
                    aggrdata.aggrrows.swap(k, nextgoodrowpos);
                    aggrdata.aggrrowscoef.swap(k, nextgoodrowpos);
                }
                ngoodrows += 1;
            }
        }
        if ngoodrows > 0 {
            aggrdata.bounddist[i] = -aggrdata.bounddist[i];
        }
        aggrdata.ngoodaggrrows[i] = ngoodrows;
        beg = end;
    }

    aggrdata
}

/// The index of `probvaridx` in the aggregation data
/// (`SCIPsortedvecFindInt` on `bounddistinds`).
fn aggrdata_find(aggrdata: &AggregationData, probvaridx: i32) -> Option<usize> {
    crate::sort::sortedvec_find(&aggrdata.bounddistinds[..aggrdata.nbounddistvars], |a, b| (*a > *b) as i32 - (*a < *b) as i32, probvaridx).ok()
}

/// `aggrdataGetBoundDist`: find the bound distance value in the aggregation
/// data struct for the given variable problem index.
fn aggrdata_get_bound_dist(aggrdata: &AggregationData, probvaridx: i32) -> f64 {
    match aggrdata_find(aggrdata, probvaridx) {
        Some(i) => aggrdata.bounddist[i],
        None => 0.0,
    }
}

/// `aggregateNextRow`: aggregates the next row suitable for cancelling out
/// an active continuous variable; returns whether another row was added to
/// the aggregation row.
fn aggregate_next_row(scip: &mut Scip, sepa: SepaId, rowlhsscores: &[f64], rowrhsscores: &[f64], aggrdata: &mut AggregationData, naggrs: &mut i32) -> ScipResult<bool> {
    let mut success = false;
    let (aggrtol, maxrowfac) = {
        let d = data(scip, sepa);
        (d.aggrtol, d.maxrowfac)
    };
    let nnz = aggrdata.aggrrow.nnz as usize;
    let inds = aggrdata.aggrrow.inds[..nnz].to_vec();

    let firstcontvar = api::get_n_bin_vars(scip) + api::get_n_int_vars(scip);

    let mut badvarinds = Vec::new();
    let mut badvarbddist = Vec::new();

    for &ind in &inds {
        // Only consider continuous variables.
        if ind < firstcontvar {
            continue;
        }

        let bounddist = aggrdata_get_bound_dist(aggrdata, ind);
        if bounddist == 0.0 {
            continue;
        }

        badvarinds.push(ind);
        badvarbddist.push(bounddist);
    }
    let nbadvars = badvarinds.len();

    if nbadvars == 0 {
        return Ok(success);
    }

    crate::sort::sort_down(&mut badvarbddist[..], (&mut badvarinds[..],), crate::sort::cmp_real);

    let mut aggrfac = 0.0;
    let mut bestrowscore = 0.0;
    let mut bestrowside = 0;
    let mut minbddist = 0.0;
    let mut bestrow = NONE;

    let lp_pos = |scip: &Scip, r: RowId| scip.rows[r].lppos as usize;

    // Because the "good" bad variables have a negative bound distance, they
    // are at the end.
    for i in (0..nbadvars).rev() {
        // If the bound distance is not negative, there are no more good
        // variables so stop.
        if badvarbddist[i] > 0.0 {
            break;
        }

        // If no best row was found yet, this variable has the currently best
        // bound distance.
        if aggrfac == 0.0 {
            minbddist = -badvarbddist[i] * (1.0 - aggrtol);
        }

        // If the bound distance of the current variable is smaller than the
        // minimum bound distance stop looping.
        if -badvarbddist[i] < minbddist {
            break;
        }

        let probvaridx = badvarinds[i];

        let Some(idx) = aggrdata_find(aggrdata, probvaridx) else { return Err(Retcode::Error) };
        let start = aggrdata.aggrrowsstart[idx];
        let ngoodrows = aggrdata.ngoodaggrrows[idx];

        for k in 0..ngoodrows {
            let candrow = aggrdata.aggrrows[start + k];
            // Do not add rows twice.
            if aggr_row_has_row_been_added(scip, &aggrdata.aggrrow, candrow) {
                continue;
            }

            let rowaggrfac = -aggr_row_get_probvar_value(&aggrdata.aggrrow, probvaridx) / aggrdata.aggrrowscoef[start + k];

            // If factor is too extreme skip this row.
            if scip.set.is_feas_zero(rowaggrfac) || rowaggrfac.abs() > maxrowfac {
                continue;
            }

            let lppos = lp_pos(scip, candrow);

            // Row could be used and good rows are equalities, so ignore sidetype.
            let rowscore = max(rowlhsscores[lppos], rowrhsscores[lppos]);

            // If this rows score is better than the currently best score, remember
            // it.
            if aggrfac == 0.0 || rowscore > bestrowscore {
                bestrow = candrow;
                aggrfac = rowaggrfac;
                bestrowscore = rowscore;
                bestrowside = 0;
            }
        }
    }

    // Found a row among the good rows, so aggregate it and stop.
    if aggrfac != 0.0 {
        *naggrs += 1;
        aggr_row_add_row(scip, &mut aggrdata.aggrrow, bestrow, aggrfac, bestrowside);
        success = aggr_row_remove_zeros(scip, &mut aggrdata.aggrrow, false);
        return Ok(success);
    }

    for i in 0..nbadvars {
        // If the bound distance is negative, there are no more variables to be
        // tested, so stop.
        if badvarbddist[i] < 0.0 {
            break;
        }

        // If no best row was found yet, this variable has the currently best
        // bound distance.
        if aggrfac == 0.0 {
            minbddist = badvarbddist[i] * (1.0 - aggrtol);
        }

        // If the bound distance of the current variable is smaller than the
        // minimum bound distance stop looping.
        if badvarbddist[i] < minbddist {
            break;
        }

        let probvaridx = badvarinds[i];

        let Some(idx) = aggrdata_find(aggrdata, probvaridx) else { return Err(Retcode::Error) };
        let start = aggrdata.aggrrowsstart[idx];
        let nrows = aggrdata.aggrrowsstart[idx + 1] - start;

        // Bounddistance was positive for this variable, so it should not have good
        // rows.
        for k in 0..nrows {
            let candrow = aggrdata.aggrrows[start + k];
            // Do not add rows twice.
            if aggr_row_has_row_been_added(scip, &aggrdata.aggrrow, candrow) {
                continue;
            }

            let rowaggrfac = -aggr_row_get_probvar_value(&aggrdata.aggrrow, probvaridx) / aggrdata.aggrrowscoef[start + k];

            // If factor is too extreme skip this row.
            if scip.set.is_feas_zero(rowaggrfac) || rowaggrfac.abs() > maxrowfac {
                continue;
            }

            // Row could be used, decide which side.
            let lppos = lp_pos(scip, candrow);

            // Either both or none of the rowscores are 0.0 so use the one which
            // gives a positive slack.
            let (lhs, rhs) = (scip.rows[candrow].lhs, scip.rows[candrow].rhs);
            let (rowscore, rowside) = if (rowaggrfac < 0.0 && !scip.set.is_infinity(-lhs)) || scip.set.is_infinity(rhs) {
                (rowlhsscores[lppos], -1)
            } else {
                (rowrhsscores[lppos], 1)
            };

            // If this rows score is better than the currently best score, remember
            // it.
            if aggrfac == 0.0
                || scip.set.is_gt(rowscore, bestrowscore)
                || (scip.set.is_eq(rowscore, bestrowscore) && aggrdata.nbadvarsinrow[lppos] < aggrdata.nbadvarsinrow[lp_pos(scip, bestrow)])
            {
                bestrow = candrow;
                aggrfac = rowaggrfac;
                bestrowscore = rowscore;
                bestrowside = rowside;
            }
        }
    }

    // Found a row so aggregate it.
    if aggrfac != 0.0 {
        *naggrs += 1;
        aggr_row_add_row(scip, &mut aggrdata.aggrrow, bestrow, aggrfac, bestrowside);
        success = aggr_row_remove_zeros(scip, &mut aggrdata.aggrrow, false);
    }

    Ok(success)
}

/// The LP rows (`SCIPgetLPRowsData`).
fn lp_rows(scip: &Scip) -> Vec<RowId> {
    if scip.tree.as_ref().unwrap().focuslpconstructed {
        let lp = scip.lp.as_ref().unwrap();
        lp.rows.0[..lp.nrows as usize].to_vec()
    } else {
        Vec::new()
    }
}

/// `aggregation`: aggregates different single mixed integer constraints by
/// taking linear combinations of the rows of the LP; returns whether the
/// given startrow was actually tried.
#[allow(clippy::too_many_arguments)]
fn aggregation(
    scip: &mut Scip,
    aggrdata: &mut AggregationData,
    sepa: SepaId,
    sol: SolId,
    allowlocal: bool,
    rowlhsscores: &mut [f64],
    rowrhsscores: &mut [f64],
    startrow: i32,
    maxaggrs: i32,
    cutoff: &mut bool,
    cutinds: &mut [i32],
    cutcoefs: &mut [f64],
    negate: bool,
    ncuts: &mut i32,
) -> ScipResult<bool> {
    *cutoff = false;
    let mut wastried = false;

    let rows = lp_rows(scip);
    let d = data(scip, sepa);
    let maxtestdelta = if d.maxtestdelta == -1 { i32::MAX } else { d.maxtestdelta };

    // Calculate maximal number of non-zeros in aggregated row.
    let nlpcols = if scip.tree.as_ref().unwrap().focuslpconstructed {
        scip.lp.as_ref().unwrap().ncols
    } else {
        0
    };
    let maxaggrnonzs = (d.maxaggdensity * nlpcols as f64) as i32 + d.densityoffset;
    let (sepflowcover, sepknapsackcover, sepcmir, dynamiccuts) = (d.sepflowcover, d.sepknapsackcover, d.sepcmir, d.dynamiccuts);
    let (cmirsepa, knapsackcoversepa, flowcoversepa) = (d.cmir, d.knapsackcover, d.flowcover);

    // Add start row to the initially empty aggregation row (aggrrow).
    if startrow < 0 {
        let mut rhs = api::get_cutoffbound(scip);

        // If the objective is integral we round the right hand side of the cutoff
        // constraint. Therefore the constraint may not be valid for the problem
        // but it is valid for the set of all improving solutions. We refrain from
        // adding an epsilon cutoff for the case of a non-integral objective
        // function to avoid cutting of any improving solution even if the
        // improvement is below some epsilon value.
        if scip.probs[crate::prob::TRANSPROB].objisintegral {
            rhs = rhs.floor();
        }

        aggr_row_add_objective_function(scip, &mut aggrdata.aggrrow, rhs, 1.0);

        if aggrdata.aggrrow.nnz == 0 {
            aggr_row_clear(&mut aggrdata.aggrrow);
            return Ok(wastried);
        }
    } else {
        let startrowact = api::get_row_sol_activity(scip, rows[startrow as usize], sol);
        let r = &scip.rows[rows[startrow as usize]];
        let startweight = if startrowact <= 0.5 * r.lhs + 0.5 * r.rhs { -1.0 } else { 1.0 };

        aggr_row_add_row(scip, &mut aggrdata.aggrrow, rows[startrow as usize], if negate { -startweight } else { startweight }, 0);
    }

    // Try to generate cut from the current aggregated row; add cut if found,
    // otherwise add another row to aggrrow in order to get rid of a continuous
    // variable.
    let mut naggrs = 0;
    while naggrs <= maxaggrs {
        let mut cutrank = 0;
        let mut cutnnz = 0;
        let mut cmircutislocal = false;
        let mut flowcovercutislocal = false;
        let mut knapsackcovercutislocal = false;
        let mut cutrhs = crate::def::INVALID;

        wastried = true;

        // Step 1: try to generate a MIR cut out of the current aggregated row.

        let mut flowcoverefficacy = -scip.set.infinity();

        let flowcoversuccess = if sepflowcover {
            let out = CutOut {
                cutcoefs: &mut *cutcoefs,
                cutrhs: &mut cutrhs,
                cutinds: &mut *cutinds,
                cutnnz: &mut cutnnz,
                cutefficacy: Some(&mut flowcoverefficacy),
                cutrank: Some(&mut cutrank),
                cutislocal: &mut flowcovercutislocal,
            };
            calc_flow_cover(scip, sol, POSTPROCESS, BOUNDSWITCH, allowlocal, &mut aggrdata.aggrrow, out)?
        } else {
            false
        };

        // Initialize the knapsack cover cut efficacy variable with the flowcover
        // efficacy so that only knapsack cover cuts better than that efficacy are
        // returned.
        let mut knapsackcoverefficacy = flowcoverefficacy;

        let knapsackcoversuccess = if sepknapsackcover {
            let out = CutOut {
                cutcoefs: &mut *cutcoefs,
                cutrhs: &mut cutrhs,
                cutinds: &mut *cutinds,
                cutnnz: &mut cutnnz,
                cutefficacy: Some(&mut knapsackcoverefficacy),
                cutrank: Some(&mut cutrank),
                cutislocal: &mut knapsackcovercutislocal,
            };
            calc_knapsack_cover(scip, sol, allowlocal, &aggrdata.aggrrow, out)?
        } else {
            false
        };

        // Initialize the cutefficacy variable with the knapsackcoverefficacy, so
        // that only CMIR cuts that have a higher efficacy than that of a flowcover
        // or knapsack cover cut possibly found in the call above are returned
        // since the previous cut is overwritten in that case.
        let mut cutefficacy = knapsackcoverefficacy;

        let cmirsuccess = if sepcmir {
            let out = CutOut {
                cutcoefs: &mut *cutcoefs,
                cutrhs: &mut cutrhs,
                cutinds: &mut *cutinds,
                cutnnz: &mut cutnnz,
                cutefficacy: Some(&mut cutefficacy),
                cutrank: Some(&mut cutrank),
                cutislocal: &mut cmircutislocal,
            };
            cut_generation_heuristic_cmir(
                scip,
                sol,
                POSTPROCESS,
                BOUNDSWITCH,
                VARTYPEUSEVBDS,
                allowlocal,
                maxtestdelta,
                None,
                None,
                MINFRAC,
                MAXFRAC,
                &aggrdata.aggrrow,
                out,
            )?
        } else {
            false
        };

        let mut cut = None;
        let mk = |islocal: bool, cutcoefs: &'_ [f64], cutinds: &'_ [i32]| -> (bool, Vec<f64>, Vec<i32>) { (islocal, cutcoefs.to_vec(), cutinds.to_vec()) };
        if cmirsuccess || knapsackcoversuccess || flowcoversuccess {
            let (cutsepa, islocal, name) = if cmirsuccess {
                (cmirsepa, cmircutislocal, if startrow < 0 { "objcmir" } else { "cmir" })
            } else if knapsackcoversuccess {
                (knapsackcoversepa, knapsackcovercutislocal, if startrow < 0 { "objlci" } else { "lci" })
            } else {
                (flowcoversepa, flowcovercutislocal, if startrow < 0 { "objflowcover" } else { "flowcover" })
            };
            let (islocal, coefs, inds) = mk(islocal, cutcoefs, cutinds);
            let c = Cut {
                coefs: &coefs,
                inds: &inds,
                nnz: cutnnz,
                rhs: cutrhs,
                efficacy: cutefficacy,
                islocal,
                removable: dynamiccuts,
                rank: cutrank,
            };
            cut = add_cut(scip, sol, cutsepa, false, &c, name, cutoff, ncuts)?;
        }

        if *cutoff {
            if let Some(mut c) = cut {
                scip_lp::release_row(scip, &mut c)?;
            }
            break;
        }

        // If the cut was successfully added, decrease the score of the rows used
        // in the aggregation and clean the aggregation row (and call this function
        // again with a different start row for aggregation).
        if let Some(mut c) = cut {
            let nrows = aggrdata.aggrrow.nrows as usize;
            let rowinds = aggrdata.aggrrow.rowsinds[..nrows].to_vec();

            // Decrease row score of used rows slightly.
            for &ri in &rowinds {
                let fac = 1.0 - 0.999 * crate::lp::row_get_parallelism(scip, rows[ri as usize], c, b'e');
                rowlhsscores[ri as usize] *= fac;
                rowrhsscores[ri as usize] *= fac;
            }

            scip_lp::release_row(scip, &mut c)?;

            break;
        }

        // Step 2: aggregate an additional row in order to remove a continuous
        // variable.

        // Abort, if we reached the maximal number of aggregations.
        if naggrs == maxaggrs {
            break;
        }

        let aggrsuccess = aggregate_next_row(scip, sepa, rowlhsscores, rowrhsscores, aggrdata, &mut naggrs)?;

        // No suitable aggregation was found or number of non-zeros is now too
        // large so abort.
        if !aggrsuccess || aggrdata.aggrrow.nnz > maxaggrnonzs || aggrdata.aggrrow.nnz == 0 {
            break;
        }
    }

    aggr_row_clear(&mut aggrdata.aggrrow);

    Ok(wastried)
}

/// `getRowFracActivity`: gives an estimate of how much the activity of this
/// row is affected by fractionality in the current solution.
fn get_row_frac_activity(scip: &Scip, row: RowId, fractionalities: &[f64]) -> f64 {
    let r = &scip.rows[row];
    let mut fracsum = 0.0;
    for i in 0..r.nlpcols as usize {
        let var = scip.cols[r.cols[i]].var;
        fracsum += (r.vals[i] * fractionalities[scip.vars[var].probindex as usize]).abs();
    }
    fracsum
}

/// `separateCuts`: searches for and adds c-MIR cuts that separate the given
/// primal solution.
fn separate_cuts(scip: &mut Scip, sepa: SepaId, sol: SolId, allowlocal: bool, depth: i32, result: &mut ScipResultCode) -> ScipResult {
    let ncalls = scip.sepas[sepa].ncallsatnode;

    // Only call the cmir cut separator a given number of times at each node.
    let d = data(scip, sepa);
    if (depth == 0 && d.maxroundsroot >= 0 && ncalls >= d.maxroundsroot) || (depth > 0 && d.maxrounds >= 0 && ncalls >= d.maxrounds) {
        return Ok(());
    }

    // Check which cuts should be separated.
    {
        let cmirfreq = scip.sepas[d.cmir].freq;
        let flowcoverfreq = scip.sepas[d.flowcover].freq;
        let knapsackcoverfreq = scip.sepas[d.knapsackcover].freq;
        let at = |freq: i32| if freq > 0 { depth % freq == 0 } else { freq == depth };
        let (c, f, k) = (at(cmirfreq), at(flowcoverfreq), at(knapsackcoverfreq));

        let d = data_mut(scip, sepa);
        d.sepcmir = c;
        d.sepflowcover = f;
        d.sepknapsackcover = k;
    }

    let d = data(scip, sepa);
    if !d.sepcmir && !d.sepflowcover && !d.sepknapsackcover {
        return Ok(());
    }

    // Get all rows and number of columns.
    let rows = lp_rows(scip);
    let nrows = rows.len();

    // Nothing to do, if LP is empty.
    if nrows == 0 {
        return Ok(());
    }

    // Check whether SCIP was stopped in the meantime.
    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Get active problem variables.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    // IMPLINTSARECONT: also aggregate out implicit integers.
    let ncontvars = (api::get_n_cont_vars(scip) + api::get_n_cont_impl_vars(scip)) as usize;
    let nintvars = nvars - ncontvars;

    // Nothing to do, if problem has no variables.
    if nvars == 0 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Get data structure.
    let mut rowlhsscores = vec![0.0; nrows];
    let mut rowrhsscores = vec![0.0; nrows];
    let mut roworder = vec![0i32; nrows];
    let mut varsolvals = vec![0.0; nvars];
    let mut fractionalities = vec![0.0; nvars];
    let mut cutinds = vec![0i32; nvars];
    let mut cutcoefs = vec![0.0; nvars];
    let mut rowscores = vec![0.0; nrows];

    // Get the solution values for all active variables.
    api::get_sol_vals(scip, sol, &vars, &mut varsolvals);

    // Calculate the fractionality of the integer variables in the current
    // solution.
    for v in 0..nintvars {
        fractionalities[v] = scip.set.feas_frac(varsolvals[v]);
        fractionalities[v] = min(fractionalities[v], 1.0 - fractionalities[v]);
    }

    // Calculate the fractionality of the continuous variables in the current
    // solution; Continuous variables which are in a variable bound with a
    // binary variable are additionally assigned the fractionality of the
    // binary variable scaled by the variable bound coefficient.
    for v in nintvars..nvars {
        let (mut closestvlb, mut closestvlbidx) = (0.0, 0);
        let (mut closestvub, mut closestvubidx) = (0.0, 0);
        crate::var::var_get_closest_vlb(scip, vars[v], sol, &mut closestvlb, &mut closestvlbidx);
        crate::var::var_get_closest_vub(scip, vars[v], sol, &mut closestvub, &mut closestvubidx);

        fractionalities[v] = 0.0;
        if closestvlbidx != -1 && scip.set.is_eq(varsolvals[v], closestvlb) {
            let vb = scip.vars[vars[v]].vlbs.as_ref().unwrap();
            let (vlbvar, vlbcoef) = (vb.vars[closestvlbidx as usize], vb.coefs[closestvlbidx as usize]);
            let vlbvarprobidx = scip.vars[vlbvar].probindex as usize;
            let mut frac = scip.set.feas_frac(varsolvals[vlbvarprobidx]);

            if frac < 0.0 {
                frac = 0.0;
            }
            frac = min(frac, 1.0 - frac) * vlbcoef;
            fractionalities[v] += frac;
        }

        if closestvubidx != -1 && scip.set.is_eq(varsolvals[v], closestvub) {
            let vb = scip.vars[vars[v]].vubs.as_ref().unwrap();
            let (vubvar, vubcoef) = (vb.vars[closestvubidx as usize], vb.coefs[closestvubidx as usize]);
            let vubvarprobidx = scip.vars[vubvar].probindex as usize;
            let mut frac = scip.set.feas_frac(varsolvals[vubvarprobidx]);

            if frac < 0.0 {
                frac = 0.0;
            }
            frac = min(frac, 1.0 - frac) * vubcoef;
            fractionalities[v] += frac;
        }
    }

    // Get the maximal number of cuts allowed in a separation round.
    let d = data(scip, sepa);
    let (mut maxtries, mut maxfails, maxaggrs, maxsepacuts, maxslack) = if depth == 0 {
        (d.maxtriesroot, d.maxfailsroot, d.maxaggrsroot, d.maxsepacutsroot, d.maxslackroot)
    } else {
        (d.maxtries, d.maxfails, d.maxaggrs, d.maxsepacuts, d.maxslack)
    };
    let (densityoffset, maxrowdensity, maxaggdensity, densityscore, slackscore, trynegscaling) =
        (d.densityoffset, d.maxrowdensity, d.maxaggdensity, d.densityscore, d.slackscore, d.trynegscaling);

    // Calculate aggregation scores for both sides of all rows, and sort rows
    // by decreasing maximal score; TODO: document score definition.

    // Count the number of non-zero rows and zero rows. these values are used
    // for the sorting of the rowscores. only the non-zero rows need to be
    // sorted.
    let mut nnonzrows = 0;
    for r in 0..nrows {
        let row = &scip.rows[rows[r]];
        let nnonz = row.nlpcols;
        if nnonz == 0 || row.modifiable || (!allowlocal && row.local) {
            // Ignore empty rows, modifiable rows, and local rows if they are not
            // allowed.
            rowlhsscores[r] = 0.0;
            rowrhsscores[r] = 0.0;
        } else {
            if scip.lp.as_ref().unwrap().objsqrnormunreliable {
                crate::lp::lp_recalculate_obj_sqr_norm(scip);
            }
            let mut objnorm = scip.lp.as_ref().unwrap().objsqrnorm.sqrt();
            objnorm = max(objnorm, 1.0);

            let fracact = get_row_frac_activity(scip, rows[r], &fractionalities);
            let row = &scip.rows[rows[r]];
            let dualsol = if sol == NONE { if row.lppos >= 0 { row.dualsol } else { 0.0 } } else { 1.0 };
            let (lhs, rhs) = (row.lhs, row.rhs);
            let mut rownorm = crate::lp::row_get_norm(row);
            let activity = api::get_row_sol_activity(scip, rows[r], sol);
            rownorm = max(rownorm, 0.1);
            let rowdensity = (nnonz - densityoffset) as f64 / nvars as f64;
            let fracscore = fracact / rownorm;

            let mut slack = (activity - lhs) / rownorm;
            let mut dualscore = max(fracscore * dualsol / objnorm, 0.0001);
            if !scip.set.is_infinity(-lhs) && scip.set.is_le(slack, maxslack) && rowdensity <= maxrowdensity && rowdensity <= maxaggdensity {
                rowlhsscores[r] = dualscore + densityscore * (1.0 - rowdensity) + slackscore * max(1.0 - slack, 0.0);
            } else {
                rowlhsscores[r] = 0.0;
            }

            slack = (rhs - activity) / rownorm;
            dualscore = max(-fracscore * dualsol / objnorm, 0.0001);

            if !scip.set.is_infinity(rhs) && scip.set.is_le(slack, maxslack) && rowdensity <= maxrowdensity && rowdensity <= maxaggdensity {
                rowrhsscores[r] = dualscore + densityscore * (1.0 - rowdensity) + slackscore * max(1.0 - slack, 0.0);
            } else {
                rowrhsscores[r] = 0.0;
            }

            // For the row order only use the fractionality score since it best
            // indicates how likely it is to find a cut.
            if fracscore != 0.0 {
                roworder[nnonzrows] = r as i32;
                rowscores[nnonzrows] = fracscore;
                nnonzrows += 1;
            }
        }
    }

    crate::sort::sort_down(&mut rowscores[..nnonzrows], (&mut roworder[..nnonzrows],), crate::sort::cmp_real);

    // Calculate the data required for performing the row aggregation.
    let mut aggrdata = setup_aggregation_data(scip, sol, allowlocal);

    let mut ncuts = 0;
    if maxtries < 0 {
        maxtries = i32::MAX;
    }
    if maxfails < 0 {
        maxfails = i32::MAX;
    } else if depth == 0 && 2 * api::get_n_sepa_rounds(scip) < maxfails {
        // Allow up to double as many fails in early separounds of root node.
        maxfails += maxfails - 2 * api::get_n_sepa_rounds(scip);
    }

    // Start aggregation heuristic for each row in the LP and generate
    // resulting cuts.
    let mut ntries = 0;
    let mut nfails = 0;
    let mut cutoff = false;

    'terminate: {
        if !scip.set.is_infinity(api::get_cutoffbound(scip)) {
            // Try separating the objective function with the cutoff bound.
            aggregation(
                scip,
                &mut aggrdata,
                sepa,
                sol,
                allowlocal,
                &mut rowlhsscores,
                &mut rowrhsscores,
                -1,
                2 * maxaggrs,
                &mut cutoff,
                &mut cutinds,
                &mut cutcoefs,
                false,
                &mut ncuts,
            )?;

            if cutoff {
                break 'terminate;
            }
        }

        let mut r = 0;
        while r < nnonzrows && ntries < maxtries && ncuts < maxsepacuts && !crate::solve::solve_is_stopped(scip, false) {
            let oldncuts = ncuts;
            let mut wastried = aggregation(
                scip,
                &mut aggrdata,
                sepa,
                sol,
                allowlocal,
                &mut rowlhsscores,
                &mut rowrhsscores,
                roworder[r],
                maxaggrs,
                &mut cutoff,
                &mut cutinds,
                &mut cutcoefs,
                false,
                &mut ncuts,
            )?;

            // If trynegscaling is true we start the aggregation heuristic again for
            // this row, but multiply it by -1 first. This is done by calling the
            // aggregation function with the parameter negate equal to TRUE.
            if trynegscaling && !cutoff {
                wastried = aggregation(
                    scip,
                    &mut aggrdata,
                    sepa,
                    sol,
                    allowlocal,
                    &mut rowlhsscores,
                    &mut rowrhsscores,
                    roworder[r],
                    maxaggrs,
                    &mut cutoff,
                    &mut cutinds,
                    &mut cutcoefs,
                    true,
                    &mut ncuts,
                )?;
            }

            if cutoff {
                break;
            }

            if !wastried {
                r += 1;
                continue;
            }
            ntries += 1;

            if ncuts == oldncuts {
                nfails += 1;
                if nfails >= maxfails {
                    break;
                }
            } else {
                nfails = 0;
            }
            r += 1;
        }
    }

    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if ncuts > 0 {
        *result = ScipResultCode::Separated;
    }

    Ok(())
}

/*
 * Callback methods of separator
 */

/// `sepaCopyAggregation`: copy method for separator plugins.
fn sepa_copy_aggregation(scip: &mut Scip, _sourcescip: &Scip, _sepa: SepaId) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_sepa_aggregation(scip)
}

/// `sepaExeclpAggregation`: LP solution separation method of separator.
fn sepa_execlp_aggregation(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only call separator, if we are not close to terminating.
    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Only call separator, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != crate::types::LpSolStat::Optimal {
        return Ok(());
    }

    // Only call separator, if there are fractional variables.
    if api::get_n_lp_branch_cands(scip) == 0 {
        return Ok(());
    }

    separate_cuts(scip, sepa, NONE, allowlocal, depth, result)
}

/// `sepaExecsolAggregation`: arbitrary primal solution separation method of
/// separator.
fn sepa_execsol_aggregation(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    separate_cuts(scip, sepa, sol, allowlocal, depth, result)
}

/// `sepaExeclpDummy`: LP solution separation method of dummy separator.
fn sepa_execlp_dummy(_scip: &mut Scip, _sepa: SepaId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;
    Ok(())
}

/// `sepaExecsolDummy`: arbitrary primal solution separation method of dummy
/// separator.
fn sepa_execsol_dummy(_scip: &mut Scip, _sepa: SepaId, _sol: SolId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;
    Ok(())
}

/*
 * separator specific interface methods
 */

/// `SCIPincludeSepaAggregation`: creates the cmir separator and includes it
/// in SCIP.
pub fn include_sepa_aggregation(scip: &mut Scip) -> ScipResult {
    // Include dummy separators.
    let flowcover = api::include_sepa_basic(
        scip,
        "flowcover",
        "separator for flowcover cuts",
        -100000,
        SEPA_FREQ,
        0.0,
        SEPA_USESSUBSCIP,
        false,
        Some(sepa_execlp_dummy),
        Some(sepa_execsol_dummy),
        None,
    )?;
    let cmir = api::include_sepa_basic(
        scip,
        "cmir",
        "separator for cmir cuts",
        -100000,
        SEPA_FREQ,
        0.0,
        SEPA_USESSUBSCIP,
        false,
        Some(sepa_execlp_dummy),
        Some(sepa_execsol_dummy),
        None,
    )?;
    let knapsackcover = api::include_sepa_basic(
        scip,
        "knapsackcover",
        "separator for knapsack cover cuts",
        -100000,
        SEPA_FREQ,
        0.0,
        SEPA_USESSUBSCIP,
        false,
        Some(sepa_execlp_dummy),
        Some(sepa_execsol_dummy),
        None,
    )?;

    // Create cmir separator data (uninitialised block memory in C until the
    // parameters set it).
    let sepadata = SepaData {
        flowcover,
        cmir,
        knapsackcover,
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
        Some(sepa_execlp_aggregation),
        Some(sepa_execsol_aggregation),
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non-NULL pointers to callback methods.
    scip.sepas[sepa].sepacopy = Some(sepa_copy_aggregation);

    // Mark main separator as a parent.
    scip.sepas[sepa].isparentsepa = true;

    // Set pointer from child separators to main separator.
    scip.sepas[flowcover].parentsepa = sepa;
    scip.sepas[cmir].parentsepa = sepa;
    scip.sepas[knapsackcover].parentsepa = sepa;

    // Add cmir separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    let name = |p: &str| format!("separating/{SEPA_NAME}/{p}");
    paramset_add_int(
        scip,
        &name("maxrounds"),
        "maximal number of cmir separation rounds per node (-1: unlimited)",
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
        "maximal number of cmir separation rounds in the root node (-1: unlimited)",
        target!(i32, maxroundsroot),
        false,
        DEFAULT_MAXROUNDSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxtries"),
        "maximal number of rows to start aggregation with per separation round (-1: unlimited)",
        target!(i32, maxtries),
        true,
        DEFAULT_MAXTRIES,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxtriesroot"),
        "maximal number of rows to start aggregation with per separation round in the root node (-1: unlimited)",
        target!(i32, maxtriesroot),
        true,
        DEFAULT_MAXTRIESROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxfails"),
        "maximal number of consecutive unsuccessful aggregation tries (-1: unlimited)",
        target!(i32, maxfails),
        true,
        DEFAULT_MAXFAILS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxfailsroot"),
        "maximal number of consecutive unsuccessful aggregation tries in the root node (-1: unlimited)",
        target!(i32, maxfailsroot),
        true,
        DEFAULT_MAXFAILSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxaggrs"),
        "maximal number of aggregations for each row per separation round",
        target!(i32, maxaggrs),
        true,
        DEFAULT_MAXAGGRS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxaggrsroot"),
        "maximal number of aggregations for each row per separation round in the root node",
        target!(i32, maxaggrsroot),
        true,
        DEFAULT_MAXAGGRSROOT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxsepacuts"),
        "maximal number of cmir cuts separated per separation round",
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
        "maximal number of cmir cuts separated per separation round in the root node",
        target!(i32, maxsepacutsroot),
        false,
        DEFAULT_MAXSEPACUTSROOT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxslack"),
        "maximal slack of rows to be used in aggregation",
        target!(f64, maxslack),
        true,
        DEFAULT_MAXSLACK,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxslackroot"),
        "maximal slack of rows to be used in aggregation in the root node",
        target!(f64, maxslackroot),
        true,
        DEFAULT_MAXSLACKROOT,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("densityscore"),
        "weight of row density in the aggregation scoring of the rows",
        target!(f64, densityscore),
        true,
        DEFAULT_DENSITYSCORE,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("slackscore"),
        "weight of slack in the aggregation scoring of the rows",
        target!(f64, slackscore),
        true,
        DEFAULT_SLACKSCORE,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxaggdensity"),
        "maximal density of aggregated row",
        target!(f64, maxaggdensity),
        true,
        DEFAULT_MAXAGGDENSITY,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxrowdensity"),
        "maximal density of row to be used in aggregation",
        target!(f64, maxrowdensity),
        true,
        DEFAULT_MAXROWDENSITY,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("densityoffset"),
        "additional number of variables allowed in row on top of density",
        target!(i32, densityoffset),
        true,
        DEFAULT_DENSITYOFFSET,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("maxrowfac"),
        "maximal row aggregation factor",
        target!(f64, maxrowfac),
        true,
        DEFAULT_MAXROWFAC,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        &name("maxtestdelta"),
        "maximal number of different deltas to try (-1: unlimited)",
        target!(i32, maxtestdelta),
        true,
        DEFAULT_MAXTESTDELTA,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        &name("aggrtol"),
        "tolerance for bound distances used to select continuous variable in current aggregated constraint to be eliminated",
        target!(f64, aggrtol),
        true,
        DEFAULT_AGGRTOL,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("trynegscaling"),
        "should negative values also be tested in scaling?",
        target!(bool, trynegscaling),
        true,
        DEFAULT_TRYNEGSCALING,
        None,
    )?;
    paramset_add_bool(
        scip,
        &name("fixintegralrhs"),
        "should an additional variable be complemented if f0 = 0?",
        target!(bool, fixintegralrhs),
        true,
        DEFAULT_FIXINTEGRALRHS,
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

    Ok(())
}
