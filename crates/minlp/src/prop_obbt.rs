//! Port of `prop_obbt.c`: optimization-based bound tightening propagator.
//!
//! Separating the OBBT LP solutions (`propagating/obbt/separatesol`) and
//! linear constraints from bilinear inequalities
//! (`propagating/obbt/createlincons`) are not ported; both are off by
//! default.

use std::any::Any;

use crate::{
    api,
    cons::{PROPTIMING_AFTERLPLOOP, PropTiming},
    def::INVALID,
    lpi::BASESTAT_BASIC,
    misc::RandNumGen,
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_longint, paramset_add_real, paramset_get_int, paramset_get_real, paramset_set_int, paramset_set_real},
    retcode::ScipResult,
    scip::{Scip, Stage},
    scip_lp, scip_probing, scip_var,
    types::{BoundType, ExprId, LpSolStat, PropId, RowId, ScipResultCode, VarId, VarStatus, VarType},
    var::{NONE, var_get_lp_sol, var_is_binary, var_is_implied_integral, var_is_integral},
};

const PROP_NAME: &str = "obbt";
const PROP_DESC: &str = "optimization-based bound tightening propagator";
const PROP_TIMING: PropTiming = PROPTIMING_AFTERLPLOOP;
/// Propagator priority.
const PROP_PRIORITY: i32 = -1000000;
/// Propagator frequency.
const PROP_FREQ: i32 = 0;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const PROP_DELAY: bool = true;

/// Should obbt try to provide genvbounds if possible?
const DEFAULT_CREATE_GENVBOUNDS: bool = true;
/// Should coefficients in filtering be normalized w.r.t. the domains sizes?
const DEFAULT_FILTERING_NORM: bool = true;
/// Try to filter bounds in so-called filter rounds by solving auxiliary LPs?
const DEFAULT_APPLY_FILTERROUNDS: bool = false;
/// Should obbt try to use the LP solution to filter some bounds?
const DEFAULT_APPLY_TRIVIALFITLERING: bool = true;
/// Try to genrate genvbounds during trivial and aggressive filtering?
const DEFAULT_GENVBDSDURINGFILTER: bool = true;
/// Feasibility tolerance for reduced costs used in obbt; this value is used
/// if SCIP's dual feastol is greater.
const DEFAULT_DUALFEASTOL: f64 = 1e-9;
/// Maximum condition limit used in LP solver (-1.0: no limit).
const DEFAULT_CONDITIONLIMIT: f64 = -1.0;
/// Minimal relative improve for strengthening bounds.
const DEFAULT_BOUNDSTREPS: f64 = 0.001;
/// Minimal number of filtered bounds to apply another filter round.
const DEFAULT_FILTERING_MIN: i32 = 2;
/// Multiple of root node LP iterations used as total LP iteration limit for
/// obbt (<= 0: no limit ).
const DEFAULT_ITLIMITFACTOR: f64 = 10.0;
/// Minimum LP iteration limit.
const DEFAULT_MINITLIMIT: i64 = 5000;
/// Only apply obbt on non-convex variables.
const DEFAULT_ONLYNONCONVEXVARS: bool = true;
/// Apply obbt on variables of indicator constraints? (independent of
/// convexity)
const DEFAULT_INDICATORS: bool = false;
/// Variables of indicator constraints with smaller upper bound are not
/// considered and upper bound is tightened only if new bound is smaller.
const DEFAULT_INDICATORTHRESHOLD: f64 = 1e6;
/// Should bounds of integral variables be tightened during the probing mode?
const DEFAULT_TIGHTINTBOUNDSPROBING: bool = true;
/// Should bounds of continuous variables be tightened during the probing
/// mode?
const DEFAULT_TIGHTCONTBOUNDSPROBING: bool = false;
/// Which type of ordering algorithm should we use? (0: no, 1: greedy, 2:
/// greedy reverse)
const DEFAULT_ORDERINGALGO: i32 = 1;
/// Base that is used to calculate a bounds score value.
const OBBT_SCOREBASE: u32 = 5;
const GENVBOUND_PROP_NAME: &str = "genvbounds";

/// Should the obbt LP solution be separated? note that that by separating
/// solution OBBT will apply all bound tightenings immediatly.
const DEFAULT_SEPARATESOL: bool = false;
/// Minimum number of iteration spend to separate an obbt LP solution.
const DEFAULT_SEPAMINITER: i32 = 0;
/// Maximum number of iteration spend to separate an obbt LP solution.
const DEFAULT_SEPAMAXITER: i32 = 10;
/// Try to create genvbounds during separation process?
const DEFAULT_GENVBDSDURINGSEPA: bool = true;
/// Trigger a propagation round after that many bound tightenings (0: no
/// propagation).
const DEFAULT_PROPAGATEFREQ: i32 = 0;
/// Solve auxiliary LPs in order to find valid inequalities for bilinear
/// terms?
const DEFAULT_CREATE_BILININEQS: bool = true;
/// Create linear constraints from inequalities for bilinear terms?
const DEFAULT_CREATE_LINCONS: bool = false;
/// Multiple of OBBT LP limit used as total LP iteration limit for solving
/// bilinear inequality LPs (< 0 for no limit).
const DEFAULT_ITLIMITFAC_BILININEQS: f64 = 3.0;
/// Minimum nonconvexity for choosing a bilinear term.
const DEFAULT_MINNONCONVEXITY: f64 = 1e-1;
/// Initial random seed.
const DEFAULT_RANDSEED: u32 = 149;

/*
 * Data structures
 */

/// `BOUND`: bound data.
#[derive(Clone, Copy, Debug)]
struct Bound {
    /// Variable.
    var: VarId,
    /// Stores a probably tighter value for this bound.
    newval: f64,
    /// Type of bound.
    boundtype: BoundType,
    /// Score value that is used to group bounds.
    score: u32,
    /// Thrown out during pre-filtering step.
    filtered: bool,
    /// Stores whether a probably tighter value for this bound was found.
    found: bool,
    /// Has this bound been processed already?
    done: bool,
    /// Is this bound affecting a nonconvex term?
    nonconvex: bool,
    /// Is this bound affecting an indicator constraint?
    indicator: bool,
    /// Unique index.
    index: i32,
}

/// `CORNER`: all possible corners of a rectangular domain.
type Corner = i32;
const LEFTBOTTOM: Corner = 1;
const RIGHTBOTTOM: Corner = 2;
const RIGHTTOP: Corner = 4;
const LEFTTOP: Corner = 8;
const FILTERED: Corner = 15;

/// `BILINBOUND`: bilinear bound data.
#[derive(Clone, Copy, Debug)]
struct BilinBound {
    /// Product expression.
    expr: ExprId,
    /// Corners that could be thrown out during pre-filtering step.
    filtered: i32,
    /// Has this bilinear term been processed already?
    done: bool,
    /// Score value that is used to group bilinear term bounds.
    score: f64,
}

/// `SCIP_PropData`: propagator data.
#[derive(Default)]
struct PropData {
    /// Array of interesting bounds (`NULL`: empty).
    bounds: Vec<Bound>,
    /// Array of interesting bilinear bounds.
    bilinbounds: Vec<BilinBound>,
    /// Current objective cutoff row (`NONE` if there is none).
    cutoffrow: RowId,
    /// Genvbound propagator (`NONE` if not available).
    genvboundprop: PropId,
    /// Random number generator.
    randnumgen: Option<RandNumGen>,
    /// Number of last node where obbt was performed.
    lastnode: i64,
    /// Number of domain reductions found during propagation.
    npropagatedomreds: i64,
    /// Number of LP iterations during the probing mode.
    nprobingiterations: i64,
    /// Number of LP iterations spend for filtering.
    nfilterlpiters: i64,
    /// Minimum LP iteration limit.
    minitlimit: i64,
    /// Total LP iterations limit for solving bilinear inequality LPs.
    itlimitbilin: i64,
    /// Total LP iterations used for solving bilinear inequality LPs.
    itusedbilin: i64,
    /// Feasibility tolerance for reduced costs used in obbt; this value is
    /// used if SCIP's dual feastol is greater.
    dualfeastol: f64,
    /// Maximum condition limit used in LP solver (-1.0: no limit).
    conditionlimit: f64,
    /// Minimal relative improve for strengthening bounds.
    boundstreps: f64,
    /// LP iteration limit for obbt will be this factor times total LP
    /// iterations in root node.
    itlimitfactor: f64,
    /// Multiple of OBBT LP limit used as total LP iteration limit for solving
    /// bilinear inequality LPs (< 0 for no limit).
    itlimitfactorbilin: f64,
    /// Lower bound on minimum absolute value of nonconvex eigenvalues for a
    /// bilinear term.
    minnonconvexity: f64,
    /// Threshold whether upper bounds of vars of indicator conss are
    /// considered or tightened.
    indicatorthreshold: f64,
    /// Apply filter rounds?
    applyfilterrounds: bool,
    /// Should obbt try to use the LP solution to filter some bounds?
    applytrivialfilter: bool,
    /// Should we try to generate genvbounds during trivial and aggressive
    /// filtering?
    genvbdsduringfilter: bool,
    /// Try to create genvbounds during separation process?
    genvbdsduringsepa: bool,
    /// Should obbt try to provide genvbounds if possible?
    creategenvbounds: bool,
    /// Should coefficients in filtering be normalized w.r.t. the domains
    /// sizes?
    normalize: bool,
    /// Only apply obbt on non-convex variables.
    onlynonconvexvars: bool,
    /// Apply obbt on variables of indicator constraints? (independent of
    /// convexity)
    indicators: bool,
    /// Should bounds of integral variables be tightened during the probing
    /// mode?
    tightintboundsprobing: bool,
    /// Should bounds of continuous variables be tightened during the probing
    /// mode?
    tightcontboundsprobing: bool,
    /// Should the obbt LP solution be separated?
    separatesol: bool,
    /// Solve auxiliary LPs in order to find valid inequalities for bilinear
    /// terms?
    createbilinineqs: bool,
    /// Create linear constraints from inequalities for bilinear terms?
    createlincons: bool,
    /// Which type of ordering algorithm should we use? (0: no, 1: greedy, 2:
    /// greedy reverse)
    orderingalgo: i32,
    /// Length of interesting bounds array (-1: not initialized).
    nbounds: i32,
    /// Size of bilinear bounds array.
    bilinboundssize: i32,
    /// Minimal number of filtered bounds to apply another filter round.
    nminfilter: i32,
    /// Number of filtered bounds by solving auxiliary variables.
    nfiltered: i32,
    /// Number of filtered bounds because the LP value was equal to the bound.
    ntrivialfiltered: i32,
    /// Number of solved bounds during the loop in applyObbt().
    nsolvedbounds: i32,
    /// Number of non-trivial genvbounds generated and added during obbt.
    ngenvboundsprobing: i32,
    /// Number of non-trivial genvbounds found during aggressive filtering.
    ngenvboundsaggrfil: i32,
    /// Number of non-trivial genvbounds found during trivial filtering.
    ngenvboundstrivfil: i32,
    /// Index to store the last undone and unfiltered bound.
    lastidx: i32,
    /// Index to store the last undone and unfiltered bilinear bound.
    lastbilinidx: i32,
    /// Minimum number of iteration spend to separate an obbt LP solution.
    sepaminiter: i32,
    /// Maximum number of iteration spend to separate an obbt LP solution.
    sepamaxiter: i32,
    /// Trigger a propagation round after that many bound tightenings (0: no
    /// propagation).
    propagatefreq: i32,
    /// Number of bound tightenings since the last propagation round.
    propagatecounter: i32,
}

fn pd(scip: &Scip, prop: PropId) -> &PropData {
    scip.props[prop].propdata.as_ref().unwrap().downcast_ref::<PropData>().unwrap()
}

fn pdm(scip: &mut Scip, prop: PropId) -> &mut PropData {
    scip.props[prop].propdata.as_mut().unwrap().downcast_mut::<PropData>().unwrap()
}

#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

#[inline]
fn cmin(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}

/*
 * Local methods
 */

/// `solveLP`: solves the LP and handles errors; returns `(error, optimal)`.
fn solve_lp(scip: &mut Scip, itlimit: i32) -> ScipResult<(bool, bool)> {
    let mut optimal = false;

    let retcode = scip_probing::solve_probing_lp(scip, itlimit);

    let lpsolstat = api::get_lp_solstat(scip);

    // An error should not kill the overall solving process.
    let error = match retcode {
        Err(retcode) => {
            log::warn!("   error while solving LP in obbt propagator; LP solve terminated with code <{retcode:?}>");
            log::warn!("   this does not affect the remaining solution procedure --> continue");

            return Ok((true, optimal));
        }
        Ok((lperror, _)) => lperror,
    };

    if lpsolstat == LpSolStat::Optimal {
        debug_assert!(!error);
        optimal = true;
    }

    Ok((error, optimal))
}

/// `addObjCutoff`: adds the objective cutoff to the LP; must be in probing
/// mode.
fn add_obj_cutoff(scip: &mut Scip, propdata: &mut PropData) -> ScipResult {
    debug_assert!(propdata.cutoffrow == NONE);

    if scip.set.is_infinity(api::get_cutoffbound(scip)) {
        return Ok(());
    }

    // Get variables data.
    let vars = api::get_vars(scip).to_vec();

    // Create objective cutoff row; set local flag to FALSE since primal cutoff
    // is globally valid.
    let infinity = scip.set.infinity();
    let cutoffbound = api::get_cutoffbound(scip);
    let row = scip_lp::create_empty_row_unspec(scip, "obbt_objcutoff", -infinity, cutoffbound, false, false, false)?;
    scip_lp::cache_row_extensions(scip, row);

    for &v in &vars {
        let obj = scip.vars[v].obj;
        api::add_var_to_row(scip, row, v, obj)?;
    }
    scip_lp::flush_row_extensions(scip, row)?;

    // Add row to the LP.
    scip_probing::add_row_probing(scip, row)?;

    propdata.cutoffrow = row;

    Ok(())
}

/// `varIsFixedLocal`: determines, whether a variable is already locally
/// fixed.
fn var_is_fixed_local(scip: &Scip, var: VarId) -> bool {
    scip.set.is_feas_eq(scip.vars[var].locdom.lb, scip.vars[var].locdom.ub)
}

/// `setObjProbing`: sets objective to minimize or maximize a single
/// variable.
fn set_obj_probing(scip: &mut Scip, bound: &Bound, coef: f64) -> ScipResult {
    // Set the objective for bound->var.
    if bound.boundtype == BoundType::Lower {
        scip_probing::chg_var_obj_probing(scip, bound.var, coef)
    } else {
        scip_probing::chg_var_obj_probing(scip, bound.var, -coef)
    }
}

/// `includeVarGenVBound`: determines whether variable should be included in
/// the right-hand side of the generalized variable bound.
fn include_var_gen_vbound(scip: &mut Scip, var: VarId) -> bool {
    if scip.vars[var].varstatus != VarStatus::Column {
        return false;
    }

    let redcost = api::get_var_redcost(scip, var);

    if redcost == INVALID {
        return false;
    }

    let dualfeastol = scip.set.dualfeastol();
    !(redcost < dualfeastol && redcost > -dualfeastol)
}

/// `getIterationsLeft`: returns number of LP iterations left (-1: no limit).
fn get_iterations_left(scip: &Scip, nolditerations: i64, itlimit: i64) -> i32 {
    if itlimit == -1 {
        -1
    } else {
        let mut itsleft = itlimit - (api::get_n_lp_iterations(scip) - nolditerations);
        itsleft = if itsleft >= 0 { itsleft } else { 0 };
        itsleft = if itsleft <= i32::MAX as i64 { itsleft } else { i32::MAX as i64 };

        itsleft as i32
    }
}

/// `getFilterCoef`: returns the objective coefficient for a variable's bound
/// that will be chosen during filtering.
fn get_filter_coef(scip: &Scip, propdata: &PropData, var: VarId, boundtype: BoundType) -> f64 {
    let lb = scip.vars[var].locdom.lb;
    let ub = scip.vars[var].locdom.ub;

    // Infinite bounds will not be reached.
    if boundtype == BoundType::Lower && scip.set.is_infinity(-lb) {
        return 0.0;
    }
    if boundtype == BoundType::Upper && scip.set.is_infinity(ub) {
        return 0.0;
    }

    if propdata.normalize {
        // If the length of the domain is too large then the coefficient should
        // be set to +/- 1.0.
        if boundtype == BoundType::Lower && scip.set.is_infinity(ub) {
            return 1.0;
        }
        if boundtype == BoundType::Upper && scip.set.is_infinity(-lb) {
            return -1.0;
        }

        // Otherwise the coefficient is +/- 1.0 / ( ub - lb ).
        if boundtype == BoundType::Lower { 1.0 / (ub - lb) } else { -1.0 / (ub - lb) }
    } else if boundtype == BoundType::Lower {
        1.0
    } else {
        -1.0
    }
}

/// `createGenVBound`: creates a genvbound if the dual LP solution provides
/// such information; returns whether a non-trivial genvbound was found.
///
/// Consider the problem
///
///    min { +/- x_i : obj * x <= z, lb <= Ax <= ub, l <= x <= u },
///
/// where z is the current cutoff bound. Let (mu, nu, gamma, alpha, beta) >= 0
/// be the optimal solution of the dual of problem (P), where the variables
/// correspond to the primal inequalities in the following way:
///
///          Ax >=  lb    <->   mu
///         -Ax >= -ub    <->   nu
///    -obj * x >=  -z    <->   gamma
///           x >=   l    <->   alpha
///          -x >=  -u    <->   beta
///
/// Fixing these multipliers, by weak duality, we obtain the inequality
///
///    +/- x_i >= lb*mu - ub*nu - z*gamma + l*alpha - u*beta
///
/// that holds for all primal feasible points x with objective value at least
/// z. Setting
///
///    c = lb*mu - ub*nu, redcost_k = alpha_k - beta_k
///
/// we obtain the inequality
///
///    +/- x_i >= sum ( redcost_k * x_k ) + (-gamma) * cutoff_bound + c,
///
/// that holds for all primal feasible points with objective value at least
/// cutoff_bound. Therefore, the latter inequality can be added as a
/// generalized variable bound.
fn create_gen_vbound(scip: &mut Scip, propdata: &PropData, bound: &Bound) -> ScipResult<bool> {
    let mut found = false;

    // A genvbound with a multiplier for x_i would not help us.
    let redcostxi = api::get_var_redcost(scip, bound.var);
    if scip.set.is_zero(redcostxi) {
        // Set x_i.
        let xi = bound.var;

        // Get variable data.
        let vars = api::get_vars(scip).to_vec();

        // Count nonzero coefficients in genvbound.
        let mut ncoefs = 0;
        for &v in &vars {
            if include_var_gen_vbound(scip, v) {
                ncoefs += 1;
            }
        }

        let dualfeastol = scip.set.dualfeastol();

        // Get dual multiplier for the objective cutoff (set to zero if there
        // is no).
        let mut gamma_dual = if propdata.cutoffrow == NONE {
            0.0
        } else {
            // Note that the objective cutoff is of the form
            //    -inf <= obj * x <= cutoff_bound
            // but we want the positive dual multiplier!
            -scip_lp::row_get_dualsol(scip, propdata.cutoffrow)
        };

        // We need to treat gamma to be exactly 0 if it is below the dual
        // feasibility tolerance, see #2914.
        if propdata.cutoffrow != NONE && gamma_dual.abs() <= dualfeastol {
            gamma_dual = 0.0;
        }

        // We need at least one nonzero coefficient or a nonzero dual multiplier
        // for the objective cutoff.
        if ncoefs > 0 || gamma_dual != 0.0 {
            // Add the bound if the bool is still TRUE after the loop.
            let mut addgenvbound = true;

            // Allocate memory for storing the genvbounds right-hand side
            // variables and coefficients.
            let mut genvboundvars = Vec::with_capacity(ncoefs);
            let mut genvboundcoefs = Vec::with_capacity(ncoefs);

            // Set c = lb*mu - ub*nu - z*gamma + l*alpha - u*beta.
            let mut c = api::get_lp_objval(scip);

            // Subtract ( - z * gamma ) from c.
            c += api::get_cutoffbound(scip) * gamma_dual;

            // Subtract ( l*alpha - u*beta ) from c and set the coefficients of
            // the variables.
            for &xk in &vars {
                if include_var_gen_vbound(scip, xk) {
                    let redcost = api::get_var_redcost(scip, xk);

                    // In this case dont add a genvbound.
                    if (redcost > dualfeastol && scip.set.is_infinity(-scip.vars[xk].locdom.lb)) || (redcost < -dualfeastol && scip.set.is_infinity(scip.vars[xk].locdom.ub)) {
                        addgenvbound = false;
                        break;
                    }

                    // Store coefficients.
                    genvboundvars.push(xk);
                    genvboundcoefs.push(redcost);

                    // If redcost > 0, then redcost = alpha_k, otherwise redcost
                    // = - beta_k.
                    c -= if redcost > 0.0 {
                        redcost * scip.vars[xk].locdom.lb
                    } else {
                        redcost * scip.vars[xk].locdom.ub
                    };
                }
            }

            // Add genvbound.
            if addgenvbound && !scip.set.is_infinity(-c) {
                let coefcutoffbound = if gamma_dual < dualfeastol { 0.0 } else { -gamma_dual };
                crate::prop_genvbounds::gen_vbound_add(scip, propdata.genvboundprop, &genvboundvars, xi, &genvboundcoefs, coefcutoffbound, c, bound.boundtype)?;
                found = true;
            }
        }
    }

    Ok(found)
}

/// `exchangeBounds`: exchange a bound which has been processed and updates
/// the last undone and unfiltered bound index; has to be called after
/// filtering or processing a bound.
fn exchange_bounds(propdata: &mut PropData, i: i32) {
    // Exchange the bounds.
    if propdata.lastidx != i {
        propdata.bounds.swap(i as usize, propdata.lastidx as usize);
    }

    propdata.lastidx -= 1;
}

/// `getCorner`: helper function to return a corner of the domain of two
/// variables.
fn get_corner(scip: &Scip, x: VarId, y: VarId, corner: Corner) -> (f64, f64) {
    let (vx, vy) = (&scip.vars[x].glbdom, &scip.vars[y].glbdom);
    match corner {
        LEFTBOTTOM => (vx.lb, vy.lb),
        RIGHTBOTTOM => (vx.ub, vy.lb),
        LEFTTOP => (vx.lb, vy.ub),
        RIGHTTOP => (vx.ub, vy.ub),
        _ => panic!("invalid corner"),
    }
}

/// `getCorners`: helper function to return the two end points of a
/// diagonal; returns `(xs, ys, xt, yt)`.
fn get_corners(scip: &Scip, x: VarId, y: VarId, corner: Corner) -> (f64, f64, f64, f64) {
    // Get end point.
    let (xt, yt) = get_corner(scip, x, y, corner);

    // Get start point.
    let (xs, ys) = match corner {
        LEFTBOTTOM => get_corner(scip, x, y, RIGHTTOP),
        RIGHTBOTTOM => get_corner(scip, x, y, LEFTTOP),
        LEFTTOP => get_corner(scip, x, y, RIGHTBOTTOM),
        RIGHTTOP => get_corner(scip, x, y, LEFTBOTTOM),
        _ => panic!("invalid corner"),
    };

    (xs, ys, xt, yt)
}

/// `bilinboundGetX`: returns the first variable of a bilinear bound.
fn bilinbound_get_x(scip: &Scip, bilinbound: &BilinBound) -> VarId {
    crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[bilinbound.expr].children[0])
}

/// `bilinboundGetY`: returns the second variable of a bilinear bound.
fn bilinbound_get_y(scip: &Scip, bilinbound: &BilinBound) -> VarId {
    crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[bilinbound.expr].children[1])
}

/// `bilinboundGetLocksNeg`: returns the negative locks of the expression in
/// a bilinear bound.
fn bilinbound_get_locks_neg(scip: &Scip, bilinbound: &BilinBound) -> i32 {
    crate::cons_nonlinear::get_expr_n_locks_neg_nonlinear(scip, bilinbound.expr)
}

/// `bilinboundGetLocksPos`: returns the positive locks of the expression in
/// a bilinear bound.
fn bilinbound_get_locks_pos(scip: &Scip, bilinbound: &BilinBound) -> i32 {
    crate::cons_nonlinear::get_expr_n_locks_pos_nonlinear(scip, bilinbound.expr)
}

/// `bilinboundGetScore`: computes the score of a bilinear term bound.
fn bilinbound_get_score(scip: &Scip, randnumgen: &mut RandNumGen, bilinbound: &BilinBound) -> f64 {
    let x = bilinbound_get_x(scip, bilinbound);
    let y = bilinbound_get_y(scip, bilinbound);
    let lbx = scip.vars[x].locdom.lb;
    let ubx = scip.vars[x].locdom.ub;
    let lby = scip.vars[y].locdom.lb;
    let uby = scip.vars[y].locdom.ub;
    let eps = scip.set.epsilon();

    // Consider how often a bilinear term is present in the problem.
    let mut score = (bilinbound_get_locks_neg(scip, bilinbound) + bilinbound_get_locks_pos(scip, bilinbound)) as f64;

    // Penalize small variable domains; TODO tune the factor in the logarithm,
    // maybe add a parameter for it.
    if ubx - lbx < 0.5 {
        score += (2.0 * (ubx - lbx) + eps).ln();
    }
    if uby - lby < 0.5 {
        score += (2.0 * (uby - lby) + eps).ln();
    }

    // Consider interiority of variables in the LP solution.
    if api::get_lp_solstat(scip) == LpSolStat::Optimal {
        let solx = var_get_lp_sol(scip, x);
        let soly = var_get_lp_sol(scip, y);
        let interiorityx = cmin(solx - lbx, ubx - solx) / cmax(ubx - lbx, eps);
        let interiorityy = cmin(soly - lby, uby - soly) / cmax(uby - lby, eps);

        score += interiorityx + interiorityy;
    }

    // Randomize score.
    score *= 1.0 + randnumgen.get_real(-eps, eps);

    score
}

/// `indicatorVarIsInteresting`: determines whether a variable of an
/// indicator constraint is (still) interesting.
///
/// A variable is interesting if it is not only part of indicator constraints
/// or if the upper bound is greater than the given threshold.
fn indicator_var_is_interesting(scip: &Scip, var: VarId, nlcount: i32, nindcount: i32, threshold: f64) -> bool {
    // If variable is only part of indicator constraints, consider current
    // upper bound.
    !(nlcount == 0 && nindcount > 0 && scip.set.is_le(scip.vars[var].locdom.ub, threshold))
}

/// `filterExistingLP`: trying to filter some bounds using the existing LP
/// solution; returns how many bounds were filtered this round.
fn filter_existing_lp(scip: &mut Scip, propdata: &mut PropData, currbound: Option<&Bound>) -> ScipResult<i32> {
    let mut nfiltered = 0;

    // Only apply filtering if an LP solution is at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(nfiltered);
    }

    // Check if a bound is tight.
    let mut i = propdata.bounds.len() as i32 - 1;
    while i >= 0 {
        let bound = propdata.bounds[i as usize];
        if bound.filtered || bound.done {
            i -= 1;
            continue;
        }

        let boundval = if bound.boundtype == BoundType::Upper {
            scip.vars[bound.var].locdom.ub
        } else {
            scip.vars[bound.var].locdom.lb
        };
        let solval = var_get_lp_sol(scip, bound.var);

        // Bound is tight; since this holds for all fixed variables, those are
        // filtered here automatically; if the lp solution is infinity, then
        // also the bound is tight.
        if (bound.boundtype == BoundType::Upper && (scip.set.is_infinity(solval) || scip.set.is_feas_ge(solval, boundval)))
            || (bound.boundtype == BoundType::Lower && (scip.set.is_infinity(-solval) || scip.set.is_feas_le(solval, boundval)))
        {
            // Mark bound as filtered.
            propdata.bounds[i as usize].filtered = true;

            // Get the basis status of the variable.
            let basestat = crate::lp::col_get_basis_status(scip, scip.vars[bound.var].data.col);

            // Solve corresponding OBBT LP and try to generate a nontrivial
            // genvbound.
            if let Some(currbound) = currbound
                && propdata.genvbdsduringfilter
                && basestat == BASESTAT_BASIC
            {
                // Set objective coefficient of the bound.
                scip_probing::chg_var_obj_probing(scip, currbound.var, 0.0)?;
                set_obj_probing(scip, &bound, 1.0)?;

                // Solve the OBBT LP.
                propdata.nprobingiterations -= api::get_n_lp_iterations(scip);
                let (_error, optimal) = solve_lp(scip, -1)?;
                propdata.nprobingiterations += api::get_n_lp_iterations(scip);

                // Try to generate a genvbound if we have solved the OBBT LP.
                if optimal && propdata.genvboundprop != NONE && (api::get_depth(scip) == 0 || (api::in_probing(scip) && api::get_depth(scip) == 1)) {
                    let found = create_gen_vbound(scip, propdata, &bound)?;

                    if found {
                        propdata.ngenvboundstrivfil += 1;
                    }
                }

                // Restore objective function.
                set_obj_probing(scip, &bound, 0.0)?;
                set_obj_probing(scip, currbound, 1.0)?;
            }

            // Exchange bound i with propdata->bounds[propdata->lastidx].
            if propdata.lastidx >= 0 {
                exchange_bounds(propdata, i);
            }

            // Increase number of filtered variables.
            nfiltered += 1;
        }
        i -= 1;
    }

    // Try to filter bilinear bounds.
    for i in propdata.lastbilinidx as usize..propdata.bilinbounds.len() {
        let corners = [LEFTTOP, LEFTBOTTOM, RIGHTTOP, RIGHTBOTTOM];
        let bilinbound = propdata.bilinbounds[i];

        // Skip processed and filtered bounds.
        if bilinbound.done || bilinbound.filtered == FILTERED {
            continue;
        }

        let x = bilinbound_get_x(scip, &bilinbound);
        let y = bilinbound_get_y(scip, &bilinbound);
        let solx = var_get_lp_sol(scip, x);
        let soly = var_get_lp_sol(scip, y);
        let mut filtered = bilinbound.filtered;

        // Check cases of unbounded solution values.
        if scip.set.is_infinity(solx) {
            filtered = filtered | RIGHTTOP | RIGHTBOTTOM;
        } else if scip.set.is_infinity(-solx) {
            filtered = filtered | LEFTTOP | LEFTBOTTOM;
        }

        if scip.set.is_infinity(soly) {
            filtered = filtered | RIGHTTOP | LEFTTOP;
        } else if scip.set.is_infinity(-soly) {
            filtered = filtered | RIGHTBOTTOM | LEFTBOTTOM;
        }

        // Check all corners.
        for &corner in &corners {
            let (xt, yt) = get_corner(scip, x, y, corner);

            if (scip.set.is_infinity(solx.abs()) || scip.set.is_feas_eq(xt, solx)) && (scip.set.is_infinity(soly.abs()) || scip.set.is_feas_eq(yt, soly)) {
                filtered |= corner;
            }
        }

        propdata.bilinbounds[i].filtered = filtered;
    }

    Ok(nfiltered)
}

/// `filterRound`: enforces one round of filtering; returns how many bounds
/// were filtered this round.
fn filter_round(scip: &mut Scip, propdata: &mut PropData, itlimit: i32, objcoefs: &[f64], objcoefsinds: &[usize]) -> ScipResult<i32> {
    let mut nfiltered = 0;

    // Solve LP.
    propdata.nfilterlpiters -= api::get_n_lp_iterations(scip) as i32 as i64;
    let (_error, optimal) = solve_lp(scip, itlimit)?;
    propdata.nfilterlpiters += api::get_n_lp_iterations(scip) as i32 as i64;

    if !optimal {
        return Ok(nfiltered);
    }

    // Check if a bound is tight.
    for i in 0..propdata.bounds.len() {
        let bound = propdata.bounds[i];

        // If bound is filtered it was handled already before.
        if bound.filtered {
            continue;
        }

        let boundval = if bound.boundtype == BoundType::Upper {
            scip.vars[bound.var].locdom.ub
        } else {
            scip.vars[bound.var].locdom.lb
        };
        let solval = var_get_lp_sol(scip, bound.var);

        // Bound is tight.
        if (bound.boundtype == BoundType::Upper && scip.set.is_feas_ge(solval, boundval)) || (bound.boundtype == BoundType::Lower && scip.set.is_feas_le(solval, boundval)) {
            // Mark bound as filtered.
            propdata.bounds[i].filtered = true;

            // Get the basis status of the variable.
            let basestat = crate::lp::col_get_basis_status(scip, scip.vars[bound.var].data.col);

            // Increase number of filtered variables.
            nfiltered += 1;

            // Solve corresponding OBBT LP and try to generate a nontrivial
            // genvbound.
            if propdata.genvbdsduringfilter && basestat == BASESTAT_BASIC {
                // Set all objective coefficients to zero.
                for &ind in objcoefsinds {
                    let filtervar = propdata.bounds[ind].var;
                    scip_probing::chg_var_obj_probing(scip, filtervar, 0.0)?;
                }

                // Set objective coefficient of the bound.
                set_obj_probing(scip, &bound, 1.0)?;

                // Solve the OBBT LP.
                propdata.nfilterlpiters -= api::get_n_lp_iterations(scip) as i32 as i64;
                let (_error, optimal) = solve_lp(scip, -1)?;
                propdata.nfilterlpiters += api::get_n_lp_iterations(scip) as i32 as i64;

                // Try to generate a genvbound if we have solved the OBBT LP.
                if optimal && propdata.genvboundprop != NONE && (api::get_depth(scip) == 0 || (api::in_probing(scip) && api::get_depth(scip) == 1)) {
                    let found = create_gen_vbound(scip, propdata, &bound)?;

                    if found {
                        propdata.ngenvboundsaggrfil += 1;
                    }
                }

                // Restore objective function.
                for (j, &ind) in objcoefsinds.iter().enumerate() {
                    // NOTE: only restore coefficients of nonfiltered bounds.
                    if !propdata.bounds[ind].filtered {
                        let filtervar = propdata.bounds[ind].var;
                        scip_probing::chg_var_obj_probing(scip, filtervar, objcoefs[j])?;
                    }
                }
            }

            // Get the corresponding variable's objective coefficient.
            let objcoef = scip_probing::get_var_obj_probing(scip, bound.var);

            // Change objective coefficient if it was set up for this bound.
            if (bound.boundtype == BoundType::Upper && scip.set.is_negative(objcoef)) || (bound.boundtype == BoundType::Lower && scip.set.is_positive(objcoef)) {
                scip_probing::chg_var_obj_probing(scip, bound.var, 0.0)?;
            }
        }
    }

    Ok(nfiltered)
}

/// `filterBounds`: filter some bounds that are not improvable by solving
/// auxiliary LPs.
fn filter_bounds(scip: &mut Scip, propdata: &mut PropData, itlimit: i64) -> ScipResult {
    let mut ntotalfiltered = 0;
    let nolditerations = api::get_n_lp_iterations(scip);
    let mut nleftiterations = get_iterations_left(scip, nolditerations, itlimit);

    // Get variable data.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    let mut objcoefs = Vec::with_capacity(propdata.bounds.len());
    let mut objcoefsinds = Vec::with_capacity(propdata.bounds.len());

    // 1.) Filter bounds of variables that are only part of indicator
    // constraints if they are not interesting any more.
    for i in 0..propdata.bounds.len() {
        let b = propdata.bounds[i];
        if !b.filtered
            && !b.done
            && b.indicator
            && !b.nonconvex
            && !indicator_var_is_interesting(scip, vars[i], b.nonconvex as i32, b.indicator as i32, propdata.indicatorthreshold)
        {
            // Mark bound as filtered.
            propdata.bounds[i].filtered = true;

            // Increase number of filtered variables.
            ntotalfiltered += 1;
        }
    }

    // 2.) Try first to filter lower bounds of interesting variables, whose
    // bounds are not already filtered.
    for &v in vars.iter().take(nvars) {
        scip_probing::chg_var_obj_probing(scip, v, 0.0)?;
    }

    for i in 0..propdata.bounds.len() {
        let b = propdata.bounds[i];
        if b.boundtype == BoundType::Lower && !b.filtered && !b.done {
            let objcoef = get_filter_coef(scip, propdata, b.var, BoundType::Lower);

            if !scip.set.is_zero(objcoef) {
                scip_probing::chg_var_obj_probing(scip, b.var, objcoef)?;

                // Store nontrivial objective coefficients.
                objcoefs.push(objcoef);
                objcoefsinds.push(i);
            }
        }
    }

    loop {
        let nfiltered = filter_round(scip, propdata, nleftiterations, &objcoefs, &objcoefsinds)?;
        ntotalfiltered += nfiltered;

        // Update iterations left.
        nleftiterations = get_iterations_left(scip, nolditerations, itlimit);
        if !(nfiltered >= propdata.nminfilter && (nleftiterations == -1 || nleftiterations > 0)) {
            break;
        }
    }

    // 3.) Now try to filter the remaining upper bounds of interesting
    // variables, whose bounds are not already filtered.

    // Set all objective coefficients to zero.
    for &ind in &objcoefsinds {
        let var = propdata.bounds[ind].var;
        scip_probing::chg_var_obj_probing(scip, var, 0.0)?;
    }

    // Reset number of nontrivial objective coefficients.
    objcoefs.clear();
    objcoefsinds.clear();

    for i in 0..propdata.bounds.len() {
        let b = propdata.bounds[i];
        if b.boundtype == BoundType::Upper && !b.filtered {
            let objcoef = get_filter_coef(scip, propdata, b.var, BoundType::Upper);

            if !scip.set.is_zero(objcoef) {
                scip_probing::chg_var_obj_probing(scip, b.var, objcoef)?;

                // Store nontrivial objective coefficients.
                objcoefs.push(objcoef);
                objcoefsinds.push(i);
            }
        }
    }

    loop {
        let nfiltered = filter_round(scip, propdata, nleftiterations, &objcoefs, &objcoefsinds)?;
        ntotalfiltered += nfiltered;

        // Update iterations left.
        nleftiterations = get_iterations_left(scip, nolditerations, itlimit);
        if !(nfiltered >= propdata.nminfilter && (nleftiterations == -1 || nleftiterations > 0)) {
            break;
        }
    }

    propdata.nfiltered += ntotalfiltered;

    Ok(())
}

/// `applyBoundChgs`: applies possible bound changes that were found.
fn apply_bound_chgs(scip: &mut Scip, propdata: &PropData, result: &mut ScipResultCode) -> ScipResult {
    for bound in &propdata.bounds {
        if bound.found {
            let (infeas, tightened) = if bound.boundtype == BoundType::Lower {
                scip_var::tighten_var_lb(scip, bound.var, bound.newval, false)?
            }
            // Tighten only if new bound is small enough due to numerical
            // reasons.
            else if scip.set.is_le(bound.newval, propdata.indicatorthreshold) {
                scip_var::tighten_var_ub(scip, bound.var, bound.newval, false)?
            } else {
                (false, false)
            };

            // Handle information about the success.
            if infeas {
                *result = ScipResultCode::Cutoff;
                break;
            }

            if tightened {
                *result = ScipResultCode::ReducedDom;
            }
        }
    }

    Ok(())
}

/// `tightenBoundProbing`: tries to tighten a bound in probing mode; returns
/// whether tightening was successful.
fn tighten_bound_probing(scip: &mut Scip, bound: &Bound, mut newval: f64) -> ScipResult<bool> {
    let mut tightened = false;

    // Get old bounds.
    let lb = scip.vars[bound.var].locdom.lb;
    let ub = scip.vars[bound.var].locdom.ub;

    if bound.boundtype == BoundType::Lower {
        // Round bounds new value if variable is integral.
        if var_is_integral(scip, bound.var) {
            newval = scip.set.ceil(newval);
        }

        // Ensure that we give consistent bounds to the LP solver.
        if newval > ub {
            newval = ub;
        }

        // Tighten if really better.
        if scip.set.is_lb_better(newval, lb, ub) {
            scip_probing::chg_var_lb_probing(scip, bound.var, newval)?;
            tightened = true;
        }
    } else {
        // Round bounds new value if variable is integral.
        if var_is_integral(scip, bound.var) {
            newval = scip.set.floor(newval);
        }

        // Ensure that we give consistent bounds to the LP solver.
        if newval < lb {
            newval = lb;
        }

        // Tighten if really better.
        if scip.set.is_ub_better(newval, lb, ub) {
            scip_probing::chg_var_ub_probing(scip, bound.var, newval)?;
            tightened = true;
        }
    }

    Ok(tightened)
}

/// `compBoundsScore`: comparison method for two bounds w.r.t. their scores.
fn comp_bounds_score(bound1: &Bound, bound2: &Bound) -> i32 {
    if bound1.score == bound2.score {
        0
    } else if bound1.score > bound2.score {
        1
    } else {
        -1
    }
}

/// `compBilinboundsScore`: comparison method for two bilinear term bounds
/// w.r.t. their scores.
fn comp_bilinbounds_score(bound1: &BilinBound, bound2: &BilinBound) -> i32 {
    if bound1.score == bound2.score {
        0
    } else if bound1.score > bound2.score {
        1
    } else {
        -1
    }
}

/// `compBoundsBoundtype`: comparison method for two bounds w.r.t. their
/// boundtype.
fn comp_bounds_boundtype(bound1: &Bound, bound2: &Bound) -> i32 {
    // Prioritize undone bounds.
    let mut diff = i32::from(!bound1.done) - i32::from(!bound2.done);
    if diff != 0 {
        return diff;
    }

    // Prioritize unfiltered bounds.
    diff = i32::from(!bound1.filtered) - i32::from(!bound2.filtered);
    if diff != 0 {
        return diff;
    }

    diff = i32::from(bound1.boundtype == BoundType::Lower) - i32::from(bound2.boundtype == BoundType::Lower);
    if diff != 0 {
        return diff;
    }

    if bound1.score == bound2.score {
        0
    } else if bound1.score > bound2.score {
        1
    } else {
        -1
    }
}

/// `sortBounds`: sort the propdata->bounds array with their distance or their
/// boundtype key.
fn sort_bounds(propdata: &mut PropData) {
    crate::sort::sort_down(&mut propdata.bounds[..], (), comp_bounds_boundtype);
}

/// `evalBound`: evaluates a bound for the current LP solution.
fn eval_bound(scip: &Scip, bound: &Bound) -> f64 {
    if bound.boundtype == BoundType::Lower {
        (var_get_lp_sol(scip, bound.var) - scip.vars[bound.var].locdom.lb).abs()
    } else {
        (scip.vars[bound.var].locdom.ub - var_get_lp_sol(scip, bound.var)).abs()
    }
}

/// `nextBound`: returns the index of the next undone and unfiltered bound
/// with the smallest distance.
fn next_bound(scip: &Scip, propdata: &PropData, convexphase: bool) -> i32 {
    let mut bestidx = -1;
    let mut bestval = scip.set.infinity();

    for k in 0..=propdata.lastidx {
        let tmpbound = &propdata.bounds[k as usize];

        // Variables of indicator constraints are considered as nonconvex.
        if !tmpbound.filtered && !tmpbound.done && (tmpbound.nonconvex == !convexphase || tmpbound.indicator == !convexphase) {
            // Return the next bound which is not done or unfiltered yet.
            if propdata.orderingalgo == 0 {
                return k;
            }

            let mut boundval = eval_bound(scip, tmpbound);

            // Negate boundval if we use the reverse greedy algorithm.
            boundval = if propdata.orderingalgo == 2 { -boundval } else { boundval };

            if bestidx == -1 || boundval < bestval {
                bestidx = k;
                bestval = boundval;
            }
        }
    }

    bestidx
}

/// `findNewBounds`: finds new variable bounds until no iterations left or
/// all bounds have been checked.
fn find_new_bounds(scip: &mut Scip, propdata: &mut PropData, nleftiterations: &mut i64, convexphase: bool) -> ScipResult {
    // Update the number of left iterations.
    let nolditerations = api::get_n_lp_iterations(scip);
    let itlimit = *nleftiterations;
    let mut iterationsleft = (*nleftiterations == -1) || (*nleftiterations > 0);

    // To improve the performance we sort the bound in such a way that the
    // undone and unfiltered bounds are at the end of propdata->bounds. We
    // calculate and update the position of the last unfiltered and undone
    // bound in propdata->lastidx.
    if !convexphase {
        // Sort bounds.
        sort_bounds(propdata);

        // If the first bound is filtered or done then there is no bound left.
        if propdata.bounds[0].done || propdata.bounds[0].filtered {
            return Ok(());
        }

        // Compute the last undone and unfiltered node.
        propdata.lastidx = 0;
        while propdata.lastidx < propdata.bounds.len() as i32 - 1 && !propdata.bounds[propdata.lastidx as usize].done && !propdata.bounds[propdata.lastidx as usize].filtered {
            propdata.lastidx += 1;
        }
    }

    // Find the first unprocessed bound.
    let mut nextboundidx = next_bound(scip, propdata, convexphase);

    // Skip if there is no bound left.
    if nextboundidx == -1 {
        return Ok(());
    }

    // The current bound is followed through its exchanges by its index.
    let mut currboundidx = nextboundidx;

    // Main loop.
    while iterationsleft && !api::is_stopped(scip) {
        // Do not visit currbound more than once.
        propdata.bounds[currboundidx as usize].done = true;
        exchange_bounds(propdata, nextboundidx);
        currboundidx = propdata.lastidx + 1;
        let mut currbound = propdata.bounds[currboundidx as usize];

        // Set objective for curr.
        set_obj_probing(scip, &currbound, 1.0)?;

        propdata.nprobingiterations -= api::get_n_lp_iterations(scip);

        // Now solve the LP.
        let (error, optimal) = solve_lp(scip, *nleftiterations as i32)?;

        propdata.nprobingiterations += api::get_n_lp_iterations(scip);
        propdata.nsolvedbounds += 1;

        // Update nleftiterations.
        *nleftiterations = get_iterations_left(scip, nolditerations, itlimit) as i64;
        iterationsleft = (*nleftiterations == -1) || (*nleftiterations > 0);

        if error {
            // Set the objective of currbound to zero to null the whole
            // objective; otherwise the objective is wrong when we call
            // findNewBounds() for the convex phase.
            scip_probing::chg_var_obj_probing(scip, currbound.var, 0.0)?;

            return Ok(());
        }

        if optimal {
            currbound.newval = var_get_lp_sol(scip, currbound.var);
            currbound.found = true;
            propdata.bounds[currboundidx as usize] = currbound;

            // In root node we may want to create a genvbound (independent of
            // tightening success).
            if (api::get_depth(scip) == 0 || (api::in_probing(scip) && api::get_depth(scip) == 1)) && propdata.genvboundprop != NONE {
                let found = create_gen_vbound(scip, propdata, &currbound)?;

                if found {
                    propdata.ngenvboundsprobing += 1;
                }
            }

            // Try to tighten bound in probing mode.
            let mut success = false;
            if (propdata.tightintboundsprobing && var_is_integral(scip, currbound.var)) || (propdata.tightcontboundsprobing && !var_is_integral(scip, currbound.var)) {
                success = tighten_bound_probing(scip, &currbound, currbound.newval)?;
            }

            // Separate current OBBT LP solution.
            if iterationsleft && propdata.separatesol {
                unimplemented!("propagating/obbt/separatesol (applySeparation)");
            }

            // Filter bound candidates by using the current LP solution.
            if propdata.applytrivialfilter {
                let nfiltered = filter_existing_lp(scip, propdata, Some(&currbound))?;
                propdata.ntrivialfiltered += nfiltered;
            }

            propdata.propagatecounter += i32::from(success);

            // Propagate if we have found enough bound tightenings.
            if propdata.propagatefreq != 0 && propdata.propagatecounter >= propdata.propagatefreq {
                let (_cutoff, ndomredsfound) = scip_probing::propagate_probing(scip, 0)?;

                propdata.npropagatedomreds += ndomredsfound;
                propdata.propagatecounter = 0;
            }
        }

        // Set objective to zero.
        set_obj_probing(scip, &currbound, 0.0)?;

        // Find the first unprocessed bound.
        nextboundidx = next_bound(scip, propdata, convexphase);

        // Check if there is no unprocessed and unfiltered node left.
        if nextboundidx == -1 {
            break;
        }

        currboundidx = nextboundidx;
    }

    Ok(())
}

/// `applyObbt`: main function of obbt.
fn apply_obbt(scip: &mut Scip, propdata: &mut PropData, itlimit: i64, result: &mut ScipResultCode) -> ScipResult {
    let mut oldlbs: Option<Vec<f64>> = None;
    let mut oldubs: Option<Vec<f64>> = None;
    let lastnpropagatedomreds = propdata.npropagatedomreds;
    let mut nleftiterations = itlimit;
    let continuenode = scip.nodes[crate::tree::tree_get_current_node(scip)].number == propdata.lastnode;
    propdata.lastidx = -1;
    let mut boundleft = false;
    *result = ScipResultCode::DidNotFind;

    // Store old variable bounds if we use propagation during obbt.
    if propdata.propagatefreq > 0 {
        oldlbs = Some(vec![0.0; propdata.bounds.len()]);
        oldubs = Some(vec![0.0; propdata.bounds.len()]);
    }

    // Reset bound data structure flags; fixed variables are marked as filtered.
    for i in 0..propdata.bounds.len() {
        let mut bound = propdata.bounds[i];
        bound.found = false;

        // Store old variable bounds.
        if let (Some(oldlbs), Some(oldubs)) = (oldlbs.as_mut(), oldubs.as_mut()) {
            oldlbs[bound.index as usize] = scip.vars[bound.var].locdom.lb;
            oldubs[bound.index as usize] = scip.vars[bound.var].locdom.ub;
        }

        // Reset 'done' and 'filtered' flag in a new B&B node.
        if !continuenode {
            bound.done = false;
            bound.filtered = false;
        }

        // Mark fixed variables as filtered.
        bound.filtered |= var_is_fixed_local(scip, bound.var);

        // Check for an unprocessed bound.
        if !bound.filtered && !bound.done {
            boundleft = true;
        }
        propdata.bounds[i] = bound;
    }

    // No bound left to check.
    if !boundleft {
        return Ok(());
    }

    // Filter variables via inspecting present LP solution.
    if propdata.applytrivialfilter && !continuenode {
        let nfiltered = filter_existing_lp(scip, propdata, None)?;
        propdata.ntrivialfiltered += nfiltered;
    }

    // Store old dualfeasibletol.
    let olddualfeastol = scip.set.dualfeastol();

    // Start probing.
    scip_probing::start_probing(scip)?;

    // Tighten dual feastol.
    if propdata.dualfeastol < olddualfeastol {
        api::chg_dualfeastol(scip, propdata.dualfeastol)?;
    }

    // Tighten condition limit.
    let conditionlimit = paramset_get_real(scip, "lp/conditionlimit");
    let hasconditionlimit = conditionlimit.is_ok();
    let oldconditionlimit = conditionlimit.unwrap_or(0.0);
    if !hasconditionlimit {
        log::warn!("obbt propagator could not set condition limit in LP solver - running without");
    } else if propdata.conditionlimit > 0.0 && (oldconditionlimit < 0.0 || propdata.conditionlimit < oldconditionlimit) {
        paramset_set_real(scip, "lp/conditionlimit", propdata.conditionlimit)?;
    }

    // Tighten relative bound improvement limit.
    let oldboundstreps = paramset_get_real(scip, "numerics/boundstreps")?;
    if !scip.set.is_eq(oldboundstreps, propdata.boundstreps) {
        paramset_set_real(scip, "numerics/boundstreps", propdata.boundstreps)?;
    }

    // Add objective cutoff.
    add_obj_cutoff(scip, propdata)?;

    // Deactivate LP polishing.
    let oldpolishing = paramset_get_int(scip, "lp/solutionpolishing")?;
    paramset_set_int(scip, "lp/solutionpolishing", 0)?;

    // Apply filtering.
    if propdata.applyfilterrounds {
        filter_bounds(scip, propdata, nleftiterations)?;
    }

    // Set objective coefficients to zero.
    let vars = api::get_vars(scip).to_vec();
    for &v in &vars {
        // Note that it is not possible to change the objective of non-column
        // variables during probing; we have to take care of the objective
        // contribution of loose variables in createGenVBound().
        if scip.vars[v].obj != 0.0 && scip.vars[v].varstatus == VarStatus::Column {
            scip_probing::chg_var_obj_probing(scip, v, 0.0)?;
        }
    }

    // Find new bounds for the variables.
    find_new_bounds(scip, propdata, &mut nleftiterations, false)?;

    if nleftiterations > 0 || itlimit < 0 {
        find_new_bounds(scip, propdata, &mut nleftiterations, true)?;
    }

    // Reset dual feastol and condition limit.
    api::chg_dualfeastol(scip, olddualfeastol)?;
    if hasconditionlimit {
        paramset_set_real(scip, "lp/conditionlimit", oldconditionlimit)?;
    }

    // Update bound->newval if we have learned additional bound tightenings
    // during SCIPpropagateProbing().
    if let (Some(oldlbs), Some(oldubs)) = (oldlbs.as_ref(), oldubs.as_ref())
        && propdata.npropagatedomreds - lastnpropagatedomreds > 0
    {
        for i in 0..propdata.bounds.len() {
            let bound = &mut propdata.bounds[i];
            let (lblocal, ublocal) = (scip.vars[bound.var].locdom.lb, scip.vars[bound.var].locdom.ub);

            // It might be the case that a bound found by the additional
            // propagation is better than the bound found after solving an
            // OBBT LP.
            if bound.found {
                if bound.boundtype == BoundType::Lower {
                    bound.newval = cmax(bound.newval, lblocal);
                } else {
                    bound.newval = cmin(bound.newval, ublocal);
                }
            } else {
                let oldlb = oldlbs[bound.index as usize];
                let oldub = oldubs[bound.index as usize];

                if bound.boundtype == BoundType::Lower && scip.set.is_lb_better(lblocal, oldlb, oldub) {
                    bound.newval = lblocal;
                    bound.found = true;
                }

                if bound.boundtype == BoundType::Upper && scip.set.is_ub_better(ublocal, oldlb, oldub) {
                    bound.newval = ublocal;
                    bound.found = true;
                }
            }
        }
    }

    // Reset relative bound improvement limit.
    paramset_set_real(scip, "numerics/boundstreps", oldboundstreps)?;

    // Reset original LP polishing setting.
    paramset_set_int(scip, "lp/solutionpolishing", oldpolishing)?;

    // End probing.
    scip_probing::end_probing(scip)?;

    // Release cutoff row if there is one.
    if propdata.cutoffrow != NONE {
        scip_lp::release_row(scip, &mut propdata.cutoffrow)?;
    }

    // Apply buffered bound changes.
    apply_bound_chgs(scip, propdata, result)
}

/// `solveBilinearLP`: computes a valid inequality from the current LP
/// relaxation for a bilinear term xy only involving x and y; the inequality
/// is found by optimizing along the line connecting the points (xs,ys) and
/// (xt,yt) over the currently given linear relaxation of the problem; this
/// optimization problem is an LP
///
/// max lambda
/// s.t. Ax <= b
///      (x,y) = (xs,ys) + lambda ((xt,yt) - (xs,ys))
///      lambda in [0,1]
///
/// which is equivalent to
///
/// max x
/// s.t. (1) Ax <= b
///      (2) (x - xs) / (xt - xs) = (y - ys) / (yt - ys)
///
/// Let x* be the optimal primal and (mu,theta) be the optimal dual solution
/// of this LP. The KKT conditions imply that the aggregation of the linear
/// constraints mu*Ax <= mu*b can be written as
///
/// x * (1 - theta) / (xt - xs) + y * theta / (yt - ys) = mu * Ax <= mu * b
///
/// <=> alpha * x + beta * y <= mu * b = alpha * (x*) + beta * (y*)
///
/// which is a valid inequality in the (x,y)-space; in order to avoid
/// numerical difficulties when (xs,ys) is too close to (xt,yt), we scale
/// constraint (1) by max{1,|xt-xs|,|yt-ys|} beforehand.
///
/// Returns `(xcoef, ycoef, constant)` and the number of non-zero dual
/// multipliers except for the auxiliary row when `countnonzduals`.
#[allow(clippy::too_many_arguments)]
fn solve_bilinear_lp(scip: &mut Scip, x: VarId, y: VarId, xs: f64, ys: f64, xt: f64, yt: f64, iterlim: i64, countnonzduals: bool) -> ScipResult<(f64, f64, f64, i32)> {
    let mut xcoef = INVALID;
    let mut ycoef = INVALID;
    let mut constant = INVALID;
    let mut nnonzduals = 0;

    // Skip computations if (xs,ys) and (xt,yt) are too close to each other or
    // contain too large values.
    if scip.set.is_feas_eq(xs, xt)
        || scip.set.is_feas_eq(ys, yt)
        || scip.set.is_huge_value(xs.abs())
        || scip.set.is_huge_value(xt.abs())
        || scip.set.is_huge_value(ys.abs())
        || scip.set.is_huge_value(yt.abs())
    {
        return Ok((xcoef, ycoef, constant, nnonzduals));
    }

    // Compute scaler for the additional linear constraint.
    let max3 = cmax(cmax(1.0, (xt - xs).abs()), (yt - ys).abs());
    let scale = cmin(max3, 100.0);

    // Set objective function.
    let signx = if xs > xt { 1.0 } else { -1.0 };
    scip_probing::chg_var_obj_probing(scip, x, signx)?;

    // Create new probing node to remove the added LP row afterwards.
    scip_probing::new_probing_node(scip)?;

    // Create row.
    let side = scale * (xs / (xt - xs) - ys / (yt - ys));
    let mut row = scip_lp::create_empty_row_unspec(scip, "bilinrow", side, side, false, false, true)?;
    api::add_var_to_row(scip, row, x, scale / (xt - xs))?;
    api::add_var_to_row(scip, row, y, -scale / (yt - ys))?;
    scip_probing::add_row_probing(scip, row)?;

    // Solve probing LP.
    let (lperror, _) = scip_probing::solve_probing_lp(scip, iterlim as i32)?;

    // Collect dual and primal solution entries.
    if !lperror && api::get_lp_solstat(scip) == LpSolStat::Optimal {
        let xval = var_get_lp_sol(scip, x);
        let yval = var_get_lp_sol(scip, y);
        let mu = -scip_lp::row_get_dualsol(scip, row);

        // xcoef x + ycoef y <= constant
        xcoef = -signx - (mu * scale) / (xt - xs);
        ycoef = (mu * scale) / (yt - ys);
        constant = xcoef * xval + ycoef * yval;

        // xcoef x <= -ycoef y + constant
        ycoef = -ycoef;

        // Inequality is only useful when both coefficients are different from
        // zero; normalize inequality if possible.
        if !scip.set.is_feas_zero(xcoef) && !scip.set.is_feas_zero(ycoef) {
            let val = xcoef.abs();

            xcoef /= val;
            ycoef /= val;
            constant /= val;

            if scip.set.is_zero(constant) {
                constant = 0.0;
            }

            if countnonzduals {
                // Count the number of non-zero dual multipliers except for the
                // added row.
                for r in api::get_lp_rows(scip) {
                    if r != row && !scip.set.is_feas_zero(scip_lp::row_get_dualsol(scip, r)) {
                        nnonzduals += 1;
                    }
                }
            }
        } else {
            xcoef = INVALID;
            ycoef = INVALID;
            constant = INVALID;
        }
    }

    // Release row and backtrack probing node.
    scip_lp::release_row(scip, &mut row)?;
    scip_probing::backtrack_probing(scip, 0)?;

    // Reset objective function.
    scip_probing::chg_var_obj_probing(scip, x, 0.0)?;

    Ok((xcoef, ycoef, constant, nnonzduals))
}

/// `applyObbtBilinear`: applies obbt for finding valid inequalities for
/// bilinear terms; function works as follows:
///
///  1. start probing mode
///  2. add objective cutoff (if possible)
///  3. set objective function to zero
///  4. set feasibility, optimality, and relative bound improvement
///     tolerances of SCIP
///  5. main loop
///  6. restore old tolerances
fn apply_obbt_bilinear(scip: &mut Scip, propdata: &mut PropData, itlimit: i64, result: &mut ScipResultCode) -> ScipResult {
    let nbilinbounds = propdata.bilinbounds.len() as i32;
    if nbilinbounds <= 0 || api::get_depth(scip) != 0 || propdata.lastbilinidx >= nbilinbounds {
        return Ok(());
    }

    // Find nonlinear handler for bilinear terms.
    let conshdlr = crate::set::set_find_conshdlr(scip, "nonlinear");
    let bilinearnlhdlr = if conshdlr != NONE {
        crate::cons_nonlinear::find_nlhdlr_nonlinear(scip, conshdlr, "bilinear")
    } else {
        NONE
    };

    // No nonlinear handler available -> skip.
    if bilinearnlhdlr == NONE {
        return Ok(());
    }

    let vars = api::get_vars(scip).to_vec();

    let nolditerations = api::get_n_lp_iterations(scip);
    let mut nleftiterations = get_iterations_left(scip, nolditerations, itlimit) as i64;

    // 1. start probing
    scip_probing::start_probing(scip)?;

    // 2. add objective cutoff
    add_obj_cutoff(scip, propdata)?;

    // 3. set objective function to zero
    for &v in &vars {
        scip_probing::chg_var_obj_probing(scip, v, 0.0)?;
    }

    // 4. tighten LP feasibility tolerance to be at most feastol/10.0
    let feastol = scip.set.feastol();
    let oldfeastol = api::chg_relaxfeastol(scip, feastol / 10.0);

    // We need to solve the probing LP before creating new probing nodes in
    // solveBilinearLP().
    let (lperror, _) = scip_probing::solve_probing_lp(scip, nleftiterations as i32)?;

    if !lperror {
        // 5. main loop
        let mut i = propdata.lastbilinidx;
        while i < nbilinbounds
            && (nleftiterations > 0 || nleftiterations == -1)
            && (propdata.itlimitbilin < 0 || propdata.itlimitbilin > propdata.itusedbilin)
            && !api::is_stopped(scip)
        {
            let corners = [LEFTBOTTOM, LEFTTOP, RIGHTTOP, RIGHTBOTTOM];
            let bilinbound = propdata.bilinbounds[i as usize];

            // We already solved LPs for this bilinear term.
            if bilinbound.done || bilinbound.filtered == FILTERED {
                i += 1;
                continue;
            }

            // Iterate through all corners
            //
            // 0: (xs,ys)=(ubx,lby) (xt,yt)=(lbx,uby) -> underestimate
            // 1: (xs,ys)=(ubx,uby) (xt,yt)=(lbx,lby) -> overestimate
            // 2: (xs,ys)=(lbx,uby) (xt,yt)=(ubx,lby) -> underestimate
            // 3: (xs,ys)=(lbx,lby) (xt,yt)=(ubx,uby) -> overestimate
            for &corner in &corners {
                let x = bilinbound_get_x(scip, &bilinbound);
                let y = bilinbound_get_y(scip, &bilinbound);

                // Skip corners that lead to an under- or overestimate that is
                // not needed.
                if ((corner == LEFTTOP || corner == RIGHTBOTTOM) && bilinbound_get_locks_pos(scip, &bilinbound) == 0)
                    || ((corner == LEFTBOTTOM || corner == RIGHTTOP) && bilinbound_get_locks_neg(scip, &bilinbound) == 0)
                {
                    continue;
                }

                // Check whether corner has been filtered already.
                if (bilinbound.filtered & corner) != 0 {
                    continue;
                }

                // Get corners (xs,ys) and (xt,yt).
                let (xs, ys, xt, yt) = get_corners(scip, x, y, corner);

                // Skip target corner points with too large values.
                if scip.set.is_huge_value(xt.abs()) || scip.set.is_huge_value(yt.abs()) {
                    continue;
                }

                // Compute inequality.
                propdata.itusedbilin -= api::get_n_lp_iterations(scip);
                let (xcoef, ycoef, constant, nnonzduals) = solve_bilinear_lp(scip, x, y, xs, ys, xt, yt, -1, propdata.createlincons)?;
                propdata.itusedbilin += api::get_n_lp_iterations(scip);

                // Update number of LP iterations.
                nleftiterations = get_iterations_left(scip, nolditerations, itlimit) as i64;

                // Add inequality to quadratic constraint handler if it
                // separates (xt,yt).
                if !scip.set.is_huge_value(xcoef)
                    && !scip.set.is_feas_zero(xcoef)
                    && ycoef.abs() < 1e+3
                    && ycoef.abs() > 1e-3
                    && scip
                        .set
                        .is_feas_gt((xcoef * xt - ycoef * yt - constant) / (xcoef * xcoef + ycoef * ycoef + constant * constant).sqrt(), 1e-2)
                {
                    // Add inequality to the associated product expression.
                    let success = crate::nlhdlr_bilinear::add_ineq_bilinear(scip, bilinearnlhdlr, bilinbound.expr, xcoef, ycoef, constant)?;

                    // Check whether the inequality has been accepted.
                    if success {
                        *result = ScipResultCode::ReducedDom;

                        // Create a linear constraint that is only used for
                        // propagation.
                        if propdata.createlincons && nnonzduals > 1 {
                            unimplemented!("propagating/obbt/createlincons");
                        }
                    }
                }
            }

            // Mark the bound as processed.
            propdata.bilinbounds[i as usize].done = true;
            i += 1;
        }

        // Remember last unprocessed bilinear term.
        propdata.lastbilinidx = i;
    }

    // End probing.
    scip_probing::end_probing(scip)?;

    // Release cutoff row if there is one.
    if propdata.cutoffrow != NONE {
        scip_lp::release_row(scip, &mut propdata.cutoffrow)?;
    }

    // 6. restore old tolerance
    api::chg_relaxfeastol(scip, oldfeastol);

    Ok(())
}

/// `getScore`: computes the score of a bound.
fn get_score(scip: &Scip, bound: &Bound, nlcount: i32, nindcount: i32, maxnlcount: i32, smallub: f64) -> u32 {
    let mut counter = nlcount as f64;
    if nindcount > 0 {
        // Variables with small upper bound are counted in half since the
        // probability is high that the corresponding indicator constraint is
        // already reformulated as bigM constraint.
        if scip.vars[bound.var].locdom.ub <= smallub {
            counter += 0.5 * nindcount as f64;
        } else {
            counter += nindcount as f64;
        }
    }

    // score = ( nlcount * ( BASE - 1 ) / maxnlcount ) * BASE^2 + vartype * BASE + boundtype
    let mut score = if counter > 0.0 {
        ((OBBT_SCOREBASE as f64 * counter * (OBBT_SCOREBASE - 1) as f64) / maxnlcount as f64) as u32
    } else {
        0
    };
    if var_is_implied_integral(scip, bound.var) {
        score += 2;
    } else {
        match scip.vars[bound.var].vartype {
            VarType::Binary => score += 4,
            VarType::Integer => score += 1,
            VarType::Continuous => score += 3,
            #[allow(unreachable_patterns)]
            _ => panic!("invalid variable type"),
        }
    }

    score *= OBBT_SCOREBASE;
    if bound.boundtype == BoundType::Upper {
        score += 1;
    }

    score
}

/// `getNLPVarsNonConvexity`: count how often each variable is used in a
/// nonconvex term.
fn get_nlp_vars_non_convexity(scip: &Scip) -> Vec<u32> {
    let nvars = api::get_n_vars(scip) as usize;

    // Initialize nccounts to zero.
    let mut nccounts = vec![0u32; nvars];

    // Get nonlinear constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, "nonlinear");
    if conshdlr == NONE || scip.conshdlrs[conshdlr].nconss == 0 {
        return nccounts;
    }

    let var2expr = crate::cons_nonlinear::get_var_expr_hashmap_nonlinear(scip, conshdlr);

    for &var in api::get_vars(scip) {
        if let Some(&e) = var2expr.get(&var) {
            nccounts[scip.vars[var].probindex as usize] = crate::cons_nonlinear::get_expr_n_sepa_uses_activity_nonlinear(scip, e);
        }
    }

    nccounts
}

/// `getNVarsIndicators`: computes for each variable the number of indicator
/// constraints in which the variable appears.
fn get_n_vars_indicators(scip: &Scip) -> Vec<i32> {
    let nvars = api::get_n_vars(scip) as usize;

    // Initialize nindcount to zero.
    let mut nindcount = vec![0i32; nvars];

    // Get indicator constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, "indicator");
    if conshdlr == NONE || scip.conshdlrs[conshdlr].nconss == 0 {
        return nindcount;
    }

    for indcons in crate::cons::conshdlr_get_conss(scip, conshdlr) {
        let lincons = crate::cons_indicator::get_linear_cons_indicator(scip, indcons);

        let consvars = crate::cons_linear::get_vars_linear(scip, lincons);
        let slackvar = crate::cons_indicator::get_slack_var_indicator(scip, indcons);

        for &v in consvars {
            // We should skip the slackvariable.
            if v == slackvar {
                continue;
            }

            // Consider only active variables.
            if scip.vars[v].varstatus == VarStatus::Column {
                nindcount[scip.vars[v].probindex as usize] += 1;
            }
        }
    }

    nindcount
}

/// `varIsInteresting`: determines whether a variable is interesting.
fn var_is_interesting(scip: &Scip, var: VarId, nlcount: i32, nindcount: i32) -> bool {
    !var_is_binary(scip, var) && scip.vars[var].varstatus == VarStatus::Column && (nlcount > 0 || nindcount > 0) && !var_is_fixed_local(scip, var)
}

/// `initBounds`: initializes interesting bounds.
fn init_bounds(scip: &mut Scip, propdata: &mut PropData) -> ScipResult {
    // Get variable data.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    // Count nonlinearities.
    let (nlcount, nccount) = if crate::nlp::is_nlp_constructed(scip) {
        (crate::nlp::get_nlp_vars_nonlinearity(scip)?, get_nlp_vars_non_convexity(scip))
    } else {
        // Initialize to zero.
        (vec![0i32; nvars], vec![0u32; nvars])
    };

    // Count indicators.
    let nindcount = if propdata.indicators { get_n_vars_indicators(scip) } else { vec![0i32; nvars] };

    let mut maxnlcount = 0;
    for i in 0..nvars {
        if maxnlcount < nlcount[i] + nindcount[i] {
            maxnlcount = nlcount[i] + nindcount[i];
        }
    }

    // Allocate interesting bounds array.
    propdata.bounds = Vec::with_capacity(2 * nvars);

    let maxcouplingvalue = paramset_get_real(scip, "constraints/indicator/maxcouplingvalue")?;
    let sepacouplingvalue = paramset_get_real(scip, "constraints/indicator/sepacouplingvalue")?;

    let smallub = cmin(maxcouplingvalue, sepacouplingvalue);

    // Get all interesting variables and their bounds.
    for i in 0..nvars {
        let nl = if propdata.onlynonconvexvars { nccount[i] as i32 } else { nlcount[i] };
        let nind = if propdata.indicators { nindcount[i] } else { 0 };
        if var_is_interesting(scip, vars[i], nl, nind) && indicator_var_is_interesting(scip, vars[i], nl, nind, propdata.indicatorthreshold) {
            for boundtype in [BoundType::Lower, BoundType::Upper] {
                // Create lower bound, then upper bound.
                let mut bound = Bound {
                    var: vars[i],
                    newval: 0.0,
                    boundtype,
                    score: 0,
                    filtered: false,
                    found: false,
                    done: false,
                    nonconvex: nccount[i] > 0,
                    indicator: nindcount[i] > 0,
                    index: propdata.bounds.len() as i32,
                };
                bound.score = get_score(scip, &bound, nlcount[i], nindcount[i], maxnlcount, smallub);
                propdata.bounds.push(bound);
            }
        }
    }

    // Set number of interesting bounds.
    propdata.nbounds = propdata.bounds.len() as i32;

    let conshdlr = crate::set::set_find_conshdlr(scip, "nonlinear");

    // Get all product expressions from nonlinear constraint handler.
    if propdata.nbounds > 0 && conshdlr != NONE && propdata.createbilinineqs {
        // Find nonlinear handler for bilinear terms.
        let bilinnlhdlr = crate::cons_nonlinear::find_nlhdlr_nonlinear(scip, conshdlr, "bilinear");
        debug_assert!(bilinnlhdlr != NONE);

        // Collect all bilinear product in all nonlinear constraints.
        let exprs = crate::nlhdlr_bilinear::get_exprs_bilinear(scip, bilinnlhdlr).to_vec();
        let nexprs = exprs.len();

        if nexprs > 0 {
            propdata.bilinbounds = Vec::with_capacity(nexprs);
            propdata.bilinboundssize = nexprs as i32;

            // Store candidates as bilinear bounds.
            for &e in &exprs {
                let x = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[0]);
                let y = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[1]);

                // Skip almost fixed variables.
                if !var_is_interesting(scip, x, 1, 0) || !var_is_interesting(scip, y, 1, 0) {
                    continue;
                }

                // Create bilinear bound; store and capture expression.
                let mut bilinbound = BilinBound {
                    expr: e,
                    filtered: 0,
                    done: false,
                    score: 0.0,
                };
                crate::expr::expr_capture(scip, e);

                // Compute a descent score.
                let mut randnumgen = propdata.randnumgen.take().unwrap();
                bilinbound.score = bilinbound_get_score(scip, &mut randnumgen, &bilinbound);
                propdata.randnumgen = Some(randnumgen);

                // Increase the number of bilinear bounds.
                propdata.bilinbounds.push(bilinbound);
            }
        }

        // Sort bounds according to decreasing score.
        if propdata.bilinbounds.len() > 1 {
            crate::sort::sort_down(&mut propdata.bilinbounds[..], (), comp_bilinbounds_score);
        }
    }

    if propdata.nbounds > 0 {
        // Sort bounds according to decreasing score; although this initial
        // order will be overruled by the distance criterion later, gives a
        // more well-defined starting situation for OBBT and might help to
        // reduce solver variability.
        crate::sort::sort_down(&mut propdata.bounds[..], (), comp_bounds_score);
    }

    Ok(())
}

/*
 * Callback methods of propagator
 */

/// `propCopyObbt`: copy method for propagator plugins.
fn prop_copy_obbt(scip: &mut Scip, _source: &Scip, _prop: PropId) -> ScipResult {
    include_prop_obbt(scip)
}

/// `propInitsolObbt`: solving process initialization method of propagator.
fn prop_initsol_obbt(scip: &mut Scip, prop: PropId) -> ScipResult {
    let genvboundprop = if pd(scip, prop).creategenvbounds {
        crate::set::set_find_prop(scip, GENVBOUND_PROP_NAME)
    } else {
        NONE
    };
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);

    let p = pdm(scip, prop);
    p.bounds = Vec::new();
    p.nbounds = -1;
    p.cutoffrow = NONE;
    p.lastnode = -1;

    // If genvbounds propagator is not available, we cannot create genvbounds.
    p.genvboundprop = genvboundprop;

    // Create random number generator.
    p.randnumgen = Some(RandNumGen::new(seed));

    Ok(())
}

/// `propExecObbt`: execution method of propagator.
fn prop_exec_obbt(scip: &mut Scip, prop: PropId, _proptiming: PropTiming, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Do not run in: presolving, repropagation, probing mode, if no objective
    // propagation is allowed.
    if scip.set.stage != Stage::Solving || api::in_repropagation(scip) || api::in_probing(scip) || !scip_var::allow_weak_dual_reds(scip) {
        return Ok(());
    }

    // Do not run propagator in a sub-SCIP.
    if crate::copy::get_subscip_depth(scip) > 0 {
        return Ok(());
    }

    // Only run for nonlinear problems, i.e., if NLP is constructed or if
    // indicator constraints exists and should be considered.
    if !crate::nlp::is_nlp_constructed(scip) {
        let indhdlr = crate::set::set_find_conshdlr(scip, "indicator");
        if !pd(scip, prop).indicators || indhdlr == NONE || scip.conshdlrs[indhdlr].nconss == 0 {
            return Ok(());
        }
    }

    // Only run if LP all columns are in the LP, i.e., the LP is a relaxation;
    // e.g., do not run if pricers are active since pricing is not performed
    // in probing mode.
    if !api::all_cols_in_lp(scip) {
        return Ok(());
    }

    // The propagator data is taken out while obbt runs.
    let mut propdata = scip.props[prop].propdata.take().unwrap();
    let ret = exec_obbt(scip, propdata.downcast_mut::<PropData>().unwrap(), result);
    scip.props[prop].propdata = Some(propdata);
    ret
}

/// The body of `propExecObbt` once its data is at hand.
fn exec_obbt(scip: &mut Scip, propdata: &mut PropData, result: &mut ScipResultCode) -> ScipResult {
    // Ensure that bounds are initialized.
    if propdata.nbounds == -1 {
        // Bounds must be initialized at root node.
        if api::get_depth(scip) == 0 {
            init_bounds(scip, propdata)?;
        } else {
            return Ok(());
        }
    }

    // Do not run if there are no interesting bounds.
    // @todo disable
    if propdata.nbounds <= 0 {
        return Ok(());
    }

    // Only run once in a node != root.
    if api::get_depth(scip) > 0 && scip.nodes[crate::tree::tree_get_current_node(scip)].number == propdata.lastnode {
        return Ok(());
    }

    // Without an optimal LP solution we don't want to run; this may be because
    // propagators with higher priority have already found reductions or
    // numerical troubles occured during LP solving.
    let lpsolstat = api::get_lp_solstat(scip);
    if lpsolstat != LpSolStat::Optimal && lpsolstat != LpSolStat::UnboundedRay {
        return Ok(());
    }

    // Compute iteration limit.
    let itlimit = if propdata.itlimitfactor > 0.0 {
        cmax(propdata.itlimitfactor * api::get_n_root_lp_iterations(scip) as f64, propdata.minitlimit as f64) as i64
    } else {
        -1
    };

    // Apply obbt.
    apply_obbt(scip, propdata, itlimit, result)?;

    // Compute globally inequalities for bilinear terms.
    if propdata.createbilinineqs {
        // Set LP iteration limit.
        if propdata.itlimitbilin == 0 {
            // No iteration limit if itlimit < 0 or itlimitfactorbilin < 0.
            propdata.itlimitbilin = if itlimit < 0 || propdata.itlimitfactorbilin < 0.0 {
                -1
            } else {
                (itlimit as f64 * propdata.itlimitfactorbilin) as i64
            };
        }

        apply_obbt_bilinear(scip, propdata, itlimit, result)?;
    }

    // Set current node as last node.
    propdata.lastnode = scip.nodes[crate::tree::tree_get_current_node(scip)].number;

    Ok(())
}

/// `propRespropObbt`: propagation conflict resolving method of propagator.
#[allow(clippy::too_many_arguments)]
fn prop_resprop_obbt(
    _scip: &mut Scip,
    _prop: PropId,
    _infervar: VarId,
    _inferinfo: i32,
    _boundtype: BoundType,
    _bdchgidx: Option<crate::var::BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotFind;
    Ok(())
}

/// `propExitsolObbt`: solving process deinitialization method of propagator.
fn prop_exitsol_obbt(scip: &mut Scip, prop: PropId, _restart: bool) -> ScipResult {
    let p = pdm(scip, prop);

    // Free random number generator.
    p.randnumgen = None;

    // Free bilinear bounds.
    if p.bilinboundssize > 0 {
        let bilinbounds = std::mem::take(&mut p.bilinbounds);
        p.bilinboundssize = 0;
        for b in bilinbounds.into_iter().rev() {
            // Release expression.
            let mut e = b.expr;
            crate::expr::expr_release(scip, &mut e)?;
        }
    }

    // Free memory allocated for the bounds.
    let p = pdm(scip, prop);
    p.bounds = Vec::new();

    p.nbounds = -1;
    p.itlimitbilin = 0;
    p.itusedbilin = 0;

    Ok(())
}

/// `propFreeObbt`: destructor of propagator to free user data.
fn prop_free_obbt(scip: &mut Scip, prop: PropId) -> ScipResult {
    scip.props[prop].propdata = None;
    Ok(())
}

/*
 * propagator specific interface methods
 */

/// `SCIPincludePropObbt`: creates the obbt propagator and includes it in
/// SCIP.
pub fn include_prop_obbt(scip: &mut Scip) -> ScipResult {
    // Create obbt propagator data and initialize statistic variables.
    let propdata = PropData {
        cutoffrow: NONE,
        genvboundprop: NONE,
        lastidx: -1,
        ..Default::default()
    };

    // Include propagator.
    let prop = api::include_prop_basic(
        scip,
        PROP_NAME,
        PROP_DESC,
        PROP_PRIORITY,
        PROP_FREQ,
        PROP_DELAY,
        PROP_TIMING,
        prop_exec_obbt,
        Some(Box::new(propdata) as Box<dyn Any>),
    )?;

    let p = &mut scip.props[prop];
    p.propcopy = Some(prop_copy_obbt);
    p.propfree = Some(prop_free_obbt);
    p.propexitsol = Some(prop_exitsol_obbt);
    p.propinitsol = Some(prop_initsol_obbt);
    p.propresprop = Some(prop_resprop_obbt);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, props, prop, propdata, PropData, $f)
        };
    }
    let rmax = crate::def::REAL_MAX;
    let rmin = crate::def::REAL_MIN;
    paramset_add_bool(
        scip,
        "propagating/obbt/creategenvbounds",
        "should obbt try to provide genvbounds if possible?",
        t!(bool, creategenvbounds),
        true,
        DEFAULT_CREATE_GENVBOUNDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/normalize",
        "should coefficients in filtering be normalized w.r.t. the domains sizes?",
        t!(bool, normalize),
        true,
        DEFAULT_FILTERING_NORM,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/applyfilterrounds",
        "try to filter bounds in so-called filter rounds by solving auxiliary LPs?",
        t!(bool, applyfilterrounds),
        true,
        DEFAULT_APPLY_FILTERROUNDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/applytrivialfilter",
        "try to filter bounds with the LP solution after each solve?",
        t!(bool, applytrivialfilter),
        true,
        DEFAULT_APPLY_TRIVIALFITLERING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/genvbdsduringfilter",
        "should we try to generate genvbounds during trivial and aggressive filtering?",
        t!(bool, genvbdsduringfilter),
        true,
        DEFAULT_GENVBDSDURINGFILTER,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/genvbdsduringsepa",
        "try to create genvbounds during separation process?",
        t!(bool, genvbdsduringsepa),
        true,
        DEFAULT_GENVBDSDURINGSEPA,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/obbt/minfilter",
        "minimal number of filtered bounds to apply another filter round",
        t!(i32, nminfilter),
        true,
        DEFAULT_FILTERING_MIN,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/obbt/itlimitfactor",
        "multiple of root node LP iterations used as total LP iteration limit for obbt (<= 0: no limit )",
        t!(f64, itlimitfactor),
        false,
        DEFAULT_ITLIMITFACTOR,
        rmin,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/obbt/itlimitfactorbilin",
        "multiple of OBBT LP limit used as total LP iteration limit for solving bilinear inequality LPs (< 0 for no limit)",
        t!(f64, itlimitfactorbilin),
        false,
        DEFAULT_ITLIMITFAC_BILININEQS,
        rmin,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/obbt/minnonconvexity",
        "minimum absolute value of nonconvex eigenvalues for a bilinear term",
        t!(f64, minnonconvexity),
        false,
        DEFAULT_MINNONCONVEXITY,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_longint(
        scip,
        "propagating/obbt/minitlimit",
        "minimum LP iteration limit",
        t!(i64, minitlimit),
        false,
        DEFAULT_MINITLIMIT,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/obbt/dualfeastol",
        "feasibility tolerance for reduced costs used in obbt; this value is used if SCIP's dual feastol is greater",
        t!(f64, dualfeastol),
        false,
        DEFAULT_DUALFEASTOL,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/obbt/conditionlimit",
        "maximum condition limit used in LP solver (-1.0: no limit)",
        t!(f64, conditionlimit),
        false,
        DEFAULT_CONDITIONLIMIT,
        -1.0,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/obbt/boundstreps",
        "minimal relative improve for strengthening bounds",
        t!(f64, boundstreps),
        false,
        DEFAULT_BOUNDSTREPS,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/obbt/indicatorthreshold",
        "threshold whether upper bounds of vars of indicator conss are considered or tightened",
        t!(f64, indicatorthreshold),
        true,
        DEFAULT_INDICATORTHRESHOLD,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/onlynonconvexvars",
        "only apply obbt on non-convex variables",
        t!(bool, onlynonconvexvars),
        true,
        DEFAULT_ONLYNONCONVEXVARS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/indicators",
        "apply obbt on variables of indicator constraints? (independent of convexity)",
        t!(bool, indicators),
        true,
        DEFAULT_INDICATORS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/tightintboundsprobing",
        "should integral bounds be tightened during the probing mode?",
        t!(bool, tightintboundsprobing),
        true,
        DEFAULT_TIGHTINTBOUNDSPROBING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/tightcontboundsprobing",
        "should continuous bounds be tightened during the probing mode?",
        t!(bool, tightcontboundsprobing),
        true,
        DEFAULT_TIGHTCONTBOUNDSPROBING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/createbilinineqs",
        "solve auxiliary LPs in order to find valid inequalities for bilinear terms?",
        t!(bool, createbilinineqs),
        true,
        DEFAULT_CREATE_BILININEQS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/createlincons",
        "create linear constraints from inequalities for bilinear terms?",
        t!(bool, createlincons),
        true,
        DEFAULT_CREATE_LINCONS,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/obbt/orderingalgo",
        "select the type of ordering algorithm which should be used (0: no special ordering, 1: greedy, 2: greedy reverse)",
        t!(i32, orderingalgo),
        true,
        DEFAULT_ORDERINGALGO,
        0,
        2,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/obbt/separatesol",
        "should the obbt LP solution be separated?",
        t!(bool, separatesol),
        true,
        DEFAULT_SEPARATESOL,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/obbt/sepaminiter",
        "minimum number of iteration spend to separate an obbt LP solution",
        t!(i32, sepaminiter),
        true,
        DEFAULT_SEPAMINITER,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/obbt/sepamaxiter",
        "maximum number of iteration spend to separate an obbt LP solution",
        t!(i32, sepamaxiter),
        true,
        DEFAULT_SEPAMAXITER,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/obbt/propagatefreq",
        "trigger a propagation round after that many bound tightenings (0: no propagation)",
        t!(i32, propagatefreq),
        true,
        DEFAULT_PROPAGATEFREQ,
        0,
        i32::MAX,
        None,
    )?;

    Ok(())
}
