//! SCIP's pricing storage (`pricestore.c`, `struct_pricestore.h`): the
//! variables found in a pricing round, before they enter the LP.

use crate::{
    clock::Clock,
    cvec::CVec,
    lp,
    retcode::ScipResult,
    scip::Scip,
    types::{LpSolStat, VarId, VarStatus},
    var::{self, NONE},
};

/// `SCIP_PRICESTORE`.
#[derive(Clone, Debug, Default)]
pub struct PriceStore {
    pub probpricingtime: Clock,
    pub vars: CVec<VarId>,
    pub scores: CVec<f64>,
    pub bdviolvars: CVec<VarId>,
    pub bdviolvarslb: CVec<f64>,
    pub bdviolvarsub: CVec<f64>,
    pub varssize: i32,
    pub nvars: i32,
    pub bdviolvarssize: i32,
    pub nbdviolvars: i32,
    pub naddedbdviolvars: i32,
    pub nprobpricings: i32,
    pub nprobvarsfound: i32,
    pub nvarsfound: i32,
    pub nvarsapplied: i32,
    pub initiallp: bool,
}

macro_rules! ps {
    ($scip:expr) => {
        (**$scip.pricestore.as_mut().unwrap())
    };
}

/*
 * dynamic memory arrays
 */

/// `pricestoreEnsureVarsMem`: resizes vars and score arrays to be able to
/// store at least `num` entries.
fn pricestore_ensure_vars_mem(scip: &mut Scip, num: i32) {
    if num > ps!(scip).varssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let p = &mut ps!(scip);
        p.vars.0.resize(newsize as usize, NONE);
        p.scores.realloc(newsize);
        p.varssize = newsize;
    }
}

/// `pricestoreEnsureBdviolvarsMem`: resizes bdviolvars arrays to be able
/// to store at least `num` entries.
fn pricestore_ensure_bdviolvars_mem(scip: &mut Scip, num: i32) {
    if num > ps!(scip).bdviolvarssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let p = &mut ps!(scip);
        p.bdviolvars.0.resize(newsize as usize, NONE);
        p.bdviolvarslb.realloc(newsize);
        p.bdviolvarsub.realloc(newsize);
        p.bdviolvarssize = newsize;
    }
}

/// `SCIPpricestoreCreate`: creates pricing storage.
pub fn pricestore_create(scip: &mut Scip) {
    scip.pricestore = Some(Box::new(PriceStore {
        probpricingtime: Clock::new(),
        ..Default::default()
    }));
}

/// `SCIPpricestoreStartInitialLP`: informs pricing storage, that the setup
/// of the initial LP starts now.
pub fn pricestore_start_initial_lp(scip: &mut Scip) {
    ps!(scip).initiallp = true;
}

/// `SCIPpricestoreEndInitialLP`: informs pricing storage, that the setup of
/// the initial LP is now finished.
pub fn pricestore_end_initial_lp(scip: &mut Scip) {
    ps!(scip).initiallp = false;
}

/// `SCIPpricestoreAddVar`: adds variable to pricing storage and capture
/// it.
pub fn pricestore_add_var(scip: &mut Scip, var: VarId, score: f64, root: bool) -> ScipResult {
    let maxpricevars = if ps!(scip).initiallp {
        i32::MAX
    } else {
        ps!(scip).nvarsfound += 1;
        scip.set.get_price_maxvars(root)
    };

    // Check, if variable belongs to the best "maxpricevars" pricing
    // variables.
    let nvars = ps!(scip).nvars;
    if nvars < maxpricevars || score > ps!(scip).scores[maxpricevars - 1] {
        // Capture variable.
        var::var_capture(scip, var);

        // If the array consists of "maxpricevars" variables, release the
        // worst variables.
        if ps!(scip).nvars == maxpricevars {
            let last = ps!(scip).nvars - 1;
            let mut worst = ps!(scip).vars[last];
            var::var_release(scip, &mut worst)?;
            ps!(scip).nvars -= 1;
        }

        // Get enough memory to store additional variable.
        let n = ps!(scip).nvars;
        pricestore_ensure_vars_mem(scip, n + 1);

        // Insert the variable at the correct position in sorted arrays.
        let p = &mut ps!(scip);
        let mut v = p.nvars;
        while v > 0 && score > p.scores[v - 1] {
            p.vars[v] = p.vars[v - 1];
            p.scores[v] = p.scores[v - 1];
            v -= 1;
        }
        p.vars[v] = var;
        p.scores[v] = score;
        p.nvars += 1;
    }
    Ok(())
}

/// `SCIPpricestoreAddBdviolvar`: adds variable where zero violates the
/// bounds to pricing storage, capture it.
pub fn pricestore_add_bdviolvar(scip: &mut Scip, var: VarId) -> ScipResult {
    if !ps!(scip).initiallp {
        ps!(scip).nvarsfound += 1;
    }

    // Get enough memory to store additional variable.
    let n = ps!(scip).nbdviolvars;
    pricestore_ensure_bdviolvars_mem(scip, n + 1);

    // Capture variable.
    var::var_capture(scip, var);

    // Insert variable in bdviolvars arrays.
    let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);
    let p = &mut ps!(scip);
    p.bdviolvars[n] = var;
    p.bdviolvarslb[n] = lb;
    p.bdviolvarsub[n] = ub;
    p.nbdviolvars += 1;

    // Temporarily set bounds, such that zero is feasible, because we don't
    // want to destroy dual feasibility (by adding columns) and primal
    // feasibility (by introducing violated bounds) at the same time. The
    // correct bounds must be reset with a call to
    // SCIPpricestoreResetBounds(). The inference information is unimportant
    // for this temporary bound change.
    if scip.set.is_positive(lb) {
        var::var_chg_lb_local(scip, var, 0.0)?;
    } else {
        var::var_chg_ub_local(scip, var, 0.0)?;
    }
    Ok(())
}

/// `addBoundViolated`: adds given problem variable to pricing storage, if
/// zero is not best bound w.r.t. objective function; returns whether it
/// was added.
fn add_bound_violated(scip: &mut Scip, var: VarId) -> ScipResult<bool> {
    let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

    // Add variable, if zero is not feasible within the bounds.
    if scip.set.is_positive(lb) || scip.set.is_negative(ub) {
        pricestore_add_bdviolvar(scip, var)?;
        Ok(true)
    } else {
        // Add variable, if zero is not best bound w.r.t. objective function.
        let bestbound = var::var_get_best_bound_local(scip, var);
        if !scip.set.is_zero(bestbound) {
            let root = crate::tree::tree_get_current_depth(scip) == 0;
            let score = -scip.vars[var].obj * bestbound;
            pricestore_add_var(scip, var, score, root)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// `SCIPpricestoreAddProbVars`: adds problem variables with negative
/// reduced costs to pricing storage.
pub fn pricestore_add_prob_vars(scip: &mut Scip, prob: usize) -> ScipResult {
    // If all problem variables of status COLUMN are already in the LP,
    // nothing has to be done.
    if scip.probs[prob].ncolvars == lp::lp_get_n_cols(scip) {
        return Ok(());
    }

    let root = crate::tree::tree_get_current_depth(scip) == 0;
    let maxpricevars = scip.set.get_price_maxvars(root);
    let abortpricevars = (scip.set.price_abortfac * maxpricevars as f64) as i32;

    // Test pricing: is abortpricevars a good idea? -> like strong
    // branching, lookahead, ...

    ps!(scip).nprobpricings += 1;

    // Start timing.
    let enabled = scip.set.time_enabled;
    ps!(scip).probpricingtime.start(enabled);

    // Price already existing problem variables.
    let mut nfoundvars = 0;
    let mut v = 0;
    while v < scip.probs[prob].nvars && nfoundvars < abortpricevars {
        let var = scip.probs[prob].vars[v];
        v += 1;
        if scip.vars[var].varstatus == VarStatus::Column {
            let col = scip.vars[var].data.col;
            if scip.cols[col].lppos < 0 {
                // Add variable to pricing storage, if zero is not best bound
                // w.r.t. objective function.
                let added = add_bound_violated(scip, var)?;

                if added {
                    ps!(scip).nprobvarsfound += 1;
                    nfoundvars += 1;
                } else if scip.cols[col].len > 0 {
                    // A column not in LP that doesn't have zero in its bounds
                    // was added by bound checking above.
                    let feasibility = if lp::lp_get_solstat(scip) == LpSolStat::Infeasible {
                        // The LP was proven infeasible, so we have an
                        // infeasibility proof by the dual Farkas multipliers
                        // y. The valid inequality y^T A x >= y^T b is violated
                        // by all x, especially by the (for this inequality
                        // most feasible solution) x' defined by
                        //   x'_i = ub_i, if y^T A_i > 0
                        //   x'_i = lb_i, if y^T A_i <= 0.
                        // Pricing in this case means to add variables i with
                        // positive Farkas value, i.e. y^T A_i x'_i > 0.
                        -lp::col_get_farkas_value(scip, col)
                    } else {
                        // The dual LP is feasible, and we have a feasible dual
                        // solution. Pricing in this case means to add
                        // variables with negative feasibility, that is
                        //  - positive reduced costs for variables with
                        //    negative lower bound
                        //  - negative reduced costs for variables with
                        //    positive upper bound
                        lp::col_get_feasibility(scip, col)
                    };

                    // Add variable if feasibility is negative, i.e., the
                    // reduced costs are negative.
                    if !scip.set.is_positive(feasibility) {
                        let len = scip.cols[col].len;
                        pricestore_add_var(scip, var, -feasibility / (len + 1) as f64, root)?;
                        ps!(scip).nprobvarsfound += 1;
                        nfoundvars += 1;
                    }
                }
            }
        }
    }

    // Stop timing.
    ps!(scip).probpricingtime.stop(enabled);
    Ok(())
}

/// `SCIPpricestoreApplyVars`: adds priced variables to the LP.
pub fn pricestore_apply_vars(scip: &mut Scip, prob: usize) -> ScipResult {
    let depth = crate::tree::tree_get_current_depth(scip);

    // Add the variables with violated bounds to LP.
    for v in ps!(scip).naddedbdviolvars..ps!(scip).nbdviolvars {
        let var = ps!(scip).bdviolvars[v];
        if scip.vars[var].varstatus == VarStatus::Loose {
            // Transform loose variable into column variable.
            var::var_column(scip, var, prob)?;
        }
        let col = scip.vars[var].data.col;

        lp::lp_add_col(scip, col, depth)?;

        if !ps!(scip).initiallp {
            ps!(scip).nvarsapplied += 1;
        }
    }
    ps!(scip).naddedbdviolvars = ps!(scip).nbdviolvars;

    // Add the selected pricing variables to LP.
    for v in 0..ps!(scip).nvars {
        let var = ps!(scip).vars[v];

        // Transform variable into column variable, if needed.
        if scip.vars[var].varstatus == VarStatus::Loose {
            // Transform loose variable into column variable.
            var::var_column(scip, var, prob)?;
        }
        let col = scip.vars[var].data.col;

        lp::lp_add_col(scip, col, depth)?;

        // Release the variable.
        let mut released = var;
        var::var_release(scip, &mut released)?;
        ps!(scip).vars[v] = released;

        if !ps!(scip).initiallp {
            ps!(scip).nvarsapplied += 1;
        }
    }

    // Clear the pricing storage.
    ps!(scip).nvars = 0;
    Ok(())
}

/// `SCIPpricestoreResetBounds`: reset variables' bounds violated by zero
/// to its original value.
pub fn pricestore_reset_bounds(scip: &mut Scip) -> ScipResult {
    // Reset variables' bounds, release them, and clear the boundviolation
    // storage; the inference information is unimportant in these removals
    // of temporary bound changes.
    for v in 0..ps!(scip).nbdviolvars {
        let var = ps!(scip).bdviolvars[v];
        let (lb, ub) = (ps!(scip).bdviolvarslb[v], ps!(scip).bdviolvarsub[v]);
        var::var_chg_lb_local(scip, var, lb)?;
        var::var_chg_ub_local(scip, var, ub)?;
        let mut released = var;
        var::var_release(scip, &mut released)?;
        ps!(scip).bdviolvars[v] = released;
    }
    ps!(scip).naddedbdviolvars = 0;
    ps!(scip).nbdviolvars = 0;
    Ok(())
}

/// `SCIPpricestoreGetNVars`: gets number of variables in pricing storage.
pub fn pricestore_get_n_vars(scip: &Scip) -> i32 {
    let p = scip.pricestore.as_ref().unwrap();
    p.nvars + p.nbdviolvars - p.naddedbdviolvars
}

/// `SCIPpricestoreGetNBoundResets`: gets number of variables in pricing
/// storage whose bounds must be reset.
pub fn pricestore_get_n_bound_resets(scip: &Scip) -> i32 {
    let p = scip.pricestore.as_ref().unwrap();
    p.nbdviolvars - p.naddedbdviolvars
}
