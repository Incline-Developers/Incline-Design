//! Port of `heur_undercover.c`: undercover primal heuristic for MINLPs.
//!
//! The reference build has no NLP solver (`SCIPgetNNlpis()` is 0), so the
//! NLP relaxation is never solved for fixing values ('n' in
//! `heuristics/undercover/fixingalts`), and the NLP local search of the
//! `subnlp` heuristic (`SCIPapplyHeurSubNlp`), which gives up for want of an
//! NLP solver, only counts as a failure. SCIP's block memory accounting is not
//! ported: the memory in use counts as zero.

use std::any::Any;

use web_time::Instant;

use crate::{
    api,
    cons::ConsFlags,
    heur::{HEURTIMING_AFTERNODE, HEURTIMING_DURINGLPLOOP, HeurTiming},
    paramset::{
        ParamEmphasis, ParamSetting, paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_longint, paramset_add_real, paramset_add_string, paramset_get_bool,
        paramset_get_int, paramset_get_real, paramset_is_fixed, paramset_set_bool, paramset_set_int, paramset_set_longint, paramset_set_real,
    },
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_probing, scip_var,
    types::{BoundType, ConshdlrId, HeurId, LpSolStat, ScipResultCode, Status, VarId, VarStatus},
    var::NONE,
};

const HEUR_NAME: &str = "undercover";
const HEUR_DESC: &str = "solves a sub-CIP determined by a set covering approach";
const HEUR_DISPCHAR: u8 = b'L';
const HEUR_PRIORITY: i32 = -1110000;
const HEUR_FREQ: i32 = 0;
const HEUR_FREQOFS: i32 = 0;
const HEUR_MAXDEPTH: i32 = -1;
const HEUR_TIMING: HeurTiming = HEURTIMING_AFTERNODE;
/// Does the heuristic use a secondary SCIP instance?
const HEUR_USESSUBSCIP: bool = true;

/* default values for user parameters, grouped by parameter type */
/// Sequence of fixing values used: 'l'p relaxation, 'n'lp relaxation,
/// 'i'ncumbent solution.
const DEFAULT_FIXINGALTS: &str = "li";

/// Maximum number of nodes to regard in the subproblem.
const DEFAULT_MAXNODES: i64 = 500;
/// Minimum number of nodes to regard in the subproblem.
const DEFAULT_MINNODES: i64 = 500;
/// Number of nodes added to the contingent of the total nodes.
const DEFAULT_NODESOFS: i64 = 500;

/// Weight for conflict score in fixing order.
const DEFAULT_CONFLICTWEIGHT: f64 = 1000.0;
/// Weight for cutoff score in fixing order.
const DEFAULT_CUTOFFWEIGHT: f64 = 1.0;
/// Weight for inference score in fixing order.
const DEFAULT_INFERENCEWEIGHT: f64 = 1.0;
/// Maximum coversize (as fraction of total number of variables).
const DEFAULT_MAXCOVERSIZEVARS: f64 = 1.0;
/// Maximum coversize (as ratio to the percentage of non-affected
/// constraints).
const DEFAULT_MAXCOVERSIZECONSS: f64 = crate::def::REAL_MAX;
/// Minimum percentage of nonlinear constraints in the original problem.
const DEFAULT_MINCOVEREDREL: f64 = 0.15;
/// Minimum number of nonlinear constraints in the original problem.
const DEFAULT_MINCOVEREDABS: i32 = 5;
/// Factor by which heuristic should at least improve the incumbent.
const DEFAULT_MINIMPROVE: f64 = 0.0;
/// Subproblem nodes in relation to nodes of the original problem.
const DEFAULT_NODESQUOT: f64 = 0.1;
/// Fraction of covering variables in the last cover which need to change
/// their value when recovering.
const DEFAULT_RECOVERDIV: f64 = 0.9;

/// Maximum number of backtracks.
const DEFAULT_MAXBACKTRACKS: i32 = 6;
/// Maximum number of recoverings.
const DEFAULT_MAXRECOVERS: i32 = 0;
/// Maximum number of reorderings of the fixing order.
const DEFAULT_MAXREORDERS: i32 = 1;

/// Objective function of the covering problem.
const DEFAULT_COVERINGOBJ: u8 = b'u';
/// Order in which variables should be fixed.
const DEFAULT_FIXINGORDER: u8 = b'v';

/// Should undercover called at root node before cut separation?
const DEFAULT_BEFORECUTS: bool = true;
/// Should integer variables in the cover be fixed first?
const DEFAULT_FIXINTFIRST: bool = false;
/// Shall LP values for integer vars be rounded according to locks?
const DEFAULT_LOCKSROUNDING: bool = true;
/// Should we only fix/dom.red. variables creating nonconvexity?
const DEFAULT_ONLYCONVEXIFY: bool = false;
/// Should the NLP heuristic be called to polish a feasible solution?
const DEFAULT_POSTNLP: bool = true;
/// Should and constraints be covered (or just copied)?
const DEFAULT_COVERAND: bool = true;
/// Should bounddisjunction constraints be covered (or just copied)?
const DEFAULT_COVERBD: bool = false;
/// Should indicator constraints be covered (or just copied)?
const DEFAULT_COVERIND: bool = false;
/// Should nonlinear constraints be covered (or just copied)?
const DEFAULT_COVERNL: bool = true;
/// Shall the cover be re-used if a conflict was added after an infeasible
/// subproblem?
const DEFAULT_REUSECOVER: bool = false;
/// Should all active cuts from the cutpool of the original scip be copied to
/// constraints of the subscip?
const DEFAULT_COPYCUTS: bool = true;
/// Initial random seed.
const DEFAULT_RANDSEED: u32 = 43;

/* local defines */
/// List of objective functions of the covering problem.
const COVERINGOBJS: &str = "cdlmtu";
/// List of orders in which variables can be fixed.
const FIXINGORDERS: &str = "CcVv";
/// Maximum number of fails after which we give up solving the NLP
/// relaxation.
const MAXNLPFAILS: i32 = 1;
/// Maximum number of fails after which we give up calling NLP local search.
const MAXPOSTNLPFAILS: i32 = 1;
/// Don't start expensive parts of the heuristics if less than this amount of
/// time left.
const MINTIMELEFT: f64 = 1.0;
/// Number of nodes equivalent for the costs for setting up the sub-CIP.
const SUBMIPSETUPCOSTS: i64 = 200;

/// `SCIPgetNNlpis()` in the reference build.
const NNLPIS: i32 = 0;

/*
 * Data structures
 */

/// `SCIP_HeurData`: primal heuristic data.
#[derive(Default)]
struct HeurData {
    /// Array of nonlinear constraint handlers.
    nlconshdlrs: Vec<ConshdlrId>,
    /// NLP local search heuristics (`NONE` if not available).
    nlpheur: HeurId,
    /// Random number generator.
    randnumgen: Option<crate::misc::RandNumGen>,
    /// Sequence of fixing values used: 'l'p relaxation, 'n'lp relaxation,
    /// 'i'ncumbent solution.
    fixingalts: String,
    /// Maximum number of nodes to regard in the subproblem.
    maxnodes: i64,
    /// Minimum number of nodes to regard in the subproblem.
    minnodes: i64,
    /// Number of nodes added to the contingent of the total nodes.
    nodesofs: i64,
    /// Nodes already used by heuristic in earlier calls.
    nusednodes: i64,
    /// Weight for conflict score in fixing order.
    conflictweight: f64,
    /// Weight for cutoff score in fixing order.
    cutoffweight: f64,
    /// Weight for inference score in foxing order.
    inferenceweight: f64,
    /// Maximum coversize (as fraction of total number of variables).
    maxcoversizevars: f64,
    /// Maximum coversize (as ratio to the percentage of non-affected
    /// constraints).
    maxcoversizeconss: f64,
    /// Minimum percentage of nonlinear constraints in the original problem.
    mincoveredrel: f64,
    /// Factor by which heuristic should at least improve the incumbent.
    minimprove: f64,
    /// Subproblem nodes in relation to nodes of the original problem.
    nodesquot: f64,
    /// Fraction of covering variables in the last cover which need to change
    /// their value when recovering.
    recoverdiv: f64,
    /// Minimum number of nonlinear constraints in the original problem.
    mincoveredabs: i32,
    /// Maximum number of backtracks.
    maxbacktracks: i32,
    /// Maximum number of recoverings.
    maxrecovers: i32,
    /// Maximum number of reorderings of the fixing order.
    maxreorders: i32,
    /// Number of fixing alternatives.
    nfixingalts: i32,
    /// Number of fails when solving the NLP relaxation after last success.
    nnlpfails: i32,
    /// Number of fails of the NLP local search after last success.
    npostnlpfails: i32,
    /// Objective function of the covering problem.
    coveringobj: u8,
    /// Order in which variables should be fixed.
    fixingorder: u8,
    /// Should undercover be called at root node before cut separation?
    beforecuts: bool,
    /// Should integer variables in the cover be fixed first?
    fixintfirst: bool,
    /// Should global bounds on variables be used instead of local bounds at
    /// focus node?
    globalbounds: bool,
    /// Shall LP values for integer vars be rounded according to locks?
    locksrounding: bool,
    /// Has current NLP relaxation already been solved successfully?
    nlpsolved: bool,
    /// Has solving the NLP relaxation failed?
    nlpfailed: bool,
    /// Should we only fix/dom.red. variables creating nonconvexity?
    onlyconvexify: bool,
    /// Should the NLP heuristic be called to polish a feasible solution?
    postnlp: bool,
    /// Should and constraints be covered (or just copied)?
    coverand: bool,
    /// Should bounddisjunction constraints be covered (or just copied)?
    coverbd: bool,
    /// Should indicator constraints be covered (or just copied)?
    coverind: bool,
    /// Should nonlinear constraints be covered (or just copied)?
    covernl: bool,
    /// Shall the cover be re-used if a conflict was added after an infeasible
    /// subproblem?
    reusecover: bool,
    /// Should all active cuts from cutpool be copied to constraints in
    /// subproblem?
    copycuts: bool,
}

fn hd(scip: &Scip, heur: HeurId) -> &HeurData {
    scip.heurs[heur].heurdata.as_ref().unwrap().downcast_ref::<HeurData>().unwrap()
}

fn hdm(scip: &mut Scip, heur: HeurId) -> &mut HeurData {
    scip.heurs[heur].heurdata.as_mut().unwrap().downcast_mut::<HeurData>().unwrap()
}

#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

#[inline]
fn cmin(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}

/// `SCIP_CLOCK` as undercover uses it: wall time since the last reset.
struct Clock(Instant);

impl Clock {
    /// `updateTimelimit`: update time limit.
    fn update_timelimit(&mut self, timelimit: &mut f64) {
        *timelimit -= self.0.elapsed().as_secs_f64();
        self.0 = Instant::now();
    }
}

/*
 * Local methods
 */

/// `varIsFixed`: determines, whether a variable is fixed to the given value.
fn var_is_fixed(scip: &Scip, var: VarId, val: f64, global: bool) -> bool {
    let v = &scip.vars[var];
    if global {
        scip.set.is_feas_eq(val, v.glbdom.lb) && scip.set.is_feas_eq(val, v.glbdom.ub)
    } else {
        scip.set.is_feas_eq(val, v.locdom.lb) && scip.set.is_feas_eq(val, v.locdom.ub)
    }
}

/// `termIsConstant`: determines, whether a term is already constant, because
/// the variable is fixed or the coefficient is zero.
fn term_is_constant(scip: &Scip, var: VarId, coeff: f64, global: bool) -> bool {
    // If the variable has zero coefficient in the original problem, the term
    // is linear.
    if scip.set.is_zero(coeff) {
        return true;
    }

    // If the variable is fixed in the original problem, the term is linear.
    let v = &scip.vars[var];
    if global {
        scip.set.is_feas_eq(v.glbdom.lb, v.glbdom.ub)
    } else {
        scip.set.is_feas_eq(v.locdom.lb, v.locdom.ub)
    }
}

/// `termIsConvex`: determines, whether a term is convex.
fn term_is_convex(scip: &Scip, lhs: f64, rhs: f64, sign: bool) -> bool {
    if sign { scip.set.is_infinity(-lhs) } else { scip.set.is_infinity(rhs) }
}

/// `incCounters`: increases counters.
fn inc_counters(termcounter: &mut [i32], conscounter: &mut [i32], consmarker: &mut [bool], idx: usize) {
    termcounter[idx] += 1;
    if !consmarker[idx] {
        conscounter[idx] += 1;
        consmarker[idx] = true;
    }
}

/// The problem index of `var`'s binary representative and whether it is
/// negated, or `None` for a multi-aggregated representative (the
/// `SCIPgetBinvarRepresentative` block of `createCoveringProblem`).
fn active_probindex(scip: &mut Scip, var: VarId) -> ScipResult<Option<(usize, bool)>> {
    let probindex = scip.vars[var].probindex;
    if probindex != -1 {
        return Ok(Some((probindex as usize, false)));
    }

    // Get binary representative of variable.
    let (repvar, _) = scip_var::get_binvar_representative(scip, var)?;

    if scip.vars[repvar].varstatus == VarStatus::MultAggr {
        return Ok(None);
    }

    // Check for negation.
    if scip.vars[repvar].varstatus == VarStatus::Negated {
        Ok(Some((scip.vars[scip.vars[repvar].negatedvar].probindex as usize, true)))
    } else {
        Ok(Some((scip.vars[repvar].probindex as usize, false)))
    }
}

/// `processNlRow`: analyzes a nonlinear row and adds constraints and
/// fixings to the covering problem; returns whether the row was processed
/// successfully.
#[allow(clippy::too_many_arguments)]
fn process_nl_row(
    scip: &mut Scip,
    nlrow: usize,
    coveringscip: &mut Scip,
    coveringvars: &[VarId],
    termcounter: &mut [i32],
    conscounter: &mut [i32],
    consmarker: &mut [bool],
    globalbounds: bool,
    onlyconvexify: bool,
) -> ScipResult<bool> {
    let (lhs, rhs, curvature, rowexpr, rowname) = {
        let r = &scip.nlrows[nlrow];
        (r.lhs, r.rhs, r.curvature, r.expr, r.name.clone())
    };

    // If we only want to convexify and curvature and bounds prove already
    // convexity, nothing to do.
    use crate::expr::ExprCurv;
    if onlyconvexify
        && (curvature == ExprCurv::Linear || (scip.set.is_infinity(-lhs) && curvature == ExprCurv::Convex) || (scip.set.is_infinity(rhs) && curvature == ExprCurv::Concave))
    {
        return Ok(true);
    }

    consmarker.fill(false);

    // Go through expression.
    if rowexpr != NONE {
        let isquadratic = crate::expr::expr_check_quadratic(scip, rowexpr)?;
        if isquadratic && crate::expr::expr_are_quadratic_exprs_variables(scip, rowexpr) {
            let quaddata = crate::expr::expr_get_quadratic_data(scip, rowexpr);
            let quadterms: Vec<(ExprIdT, f64)> = quaddata.quadexprterms.iter().map(|t| (t.expr, t.sqrcoef)).collect();
            let bilinterms: Vec<(ExprIdT, ExprIdT, f64)> = quaddata.bilinexprterms.iter().map(|t| (t.expr1, t.expr2, t.coef)).collect();

            // Go through all quadratic terms.
            for &(varexpr, sqrcoef) in &quadterms {
                let var = crate::expr_var::get_var_expr_var(scip, varexpr);

                // Term is constant, nothing to do.
                if term_is_constant(scip, var, sqrcoef, globalbounds) {
                    continue;
                }

                // If we only convexify and term is convex considering the bounds
                // of the nlrow, nothing to do.
                if onlyconvexify && term_is_convex(scip, lhs, rhs, sqrcoef >= 0.0) {
                    continue;
                }

                let probidx = scip.vars[var].probindex;
                if probidx == -1 {
                    return Ok(false);
                }

                // Otherwise variable has to be in the cover.
                scip_var::fix_var(coveringscip, coveringvars[probidx as usize], 1.0)?;

                // Update counters.
                inc_counters(termcounter, conscounter, consmarker, probidx as usize);
            }

            // Go through all bilinear terms.
            for (t, &(varexpr1, varexpr2, bilincoef)) in bilinterms.iter().enumerate() {
                let var1 = crate::expr_var::get_var_expr_var(scip, varexpr1);
                let var2 = crate::expr_var::get_var_expr_var(scip, varexpr2);

                // If the term is linear because one of the variables is fixed or
                // the coefficient is zero, nothing to do.
                if term_is_constant(scip, var1, bilincoef, globalbounds) || term_is_constant(scip, var2, bilincoef, globalbounds) {
                    continue;
                }

                let probidx1 = scip.vars[var1].probindex;
                let probidx2 = scip.vars[var2].probindex;
                if probidx1 == -1 || probidx2 == -1 {
                    return Ok(false);
                }

                // Create covering constraint.
                let name = format!("{rowname}_covering{t}");
                let coveringconsvars = [coveringvars[probidx1 as usize], coveringvars[probidx2 as usize]];
                let flags = ConsFlags {
                    initial: true,
                    separate: true,
                    enforce: true,
                    check: true,
                    propagate: true,
                    local: false,
                    modifiable: false,
                    dynamic: false,
                    removable: false,
                    stickingatnode: false,
                };
                let mut coveringcons = crate::cons_setppc::create_cons_setcover(coveringscip, &name, &coveringconsvars, &flags)?;

                // Add and release covering constraint.
                crate::scip_cons::add_cons(coveringscip, coveringcons)?;
                crate::scip_cons::release_cons(coveringscip, &mut coveringcons)?;

                // Update counters for both variables.
                inc_counters(termcounter, conscounter, consmarker, probidx1 as usize);
                inc_counters(termcounter, conscounter, consmarker, probidx2 as usize);
            }
        } else {
            // Fix all variables contained in the expression.
            let mut it = crate::expr::ExprIter::create();
            it.init(scip, rowexpr, crate::expr::ExprIterType::Dfs, false)?;
            let mut e = it.get_current();
            while !it.is_end() {
                if crate::expr::expr_is_var(scip, e) {
                    let var = crate::expr_var::get_var_expr_var(scip, e);

                    // If constraints with inactive variables are present, we will
                    // have difficulties creating the sub-CIP later.
                    let probidx = scip.vars[var].probindex;
                    if probidx == -1 {
                        it.free(scip);
                        return Ok(false);
                    }

                    // Term is constant, nothing to do; otherwise fix variable.
                    if !term_is_constant(scip, var, 1.0, globalbounds) {
                        scip_var::fix_var(coveringscip, coveringvars[probidx as usize], 1.0)?;

                        // Update counters.
                        inc_counters(termcounter, conscounter, consmarker, probidx as usize);
                    }
                }
                e = it.get_next(scip);
            }
            it.free(scip);
        }
    }

    Ok(true)
}

type ExprIdT = crate::expr::ExprId;

/// `createCoveringProblem`: creates the covering problem to determine a
/// number of variables to be fixed; returns whether the problem was created
/// successfully.
#[allow(clippy::too_many_arguments)]
fn create_covering_problem(
    scip: &mut Scip,
    coveringscip: &mut Scip,
    coveringvars: &mut [VarId],
    globalbounds: bool,
    onlyconvexify: bool,
    coverand: bool,
    coverbd: bool,
    coverind: bool,
    covernl: bool,
    coveringobj: u8,
) -> ScipResult<bool> {
    // Get required data of the original problem.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    // Create problem data structure.
    let name = format!("{}_covering", scip.probs[crate::prob::TRANSPROB].name);
    crate::scip_solve::create_prob(coveringscip, &name)?;
    coveringscip.stat.subscipdepth = crate::copy::get_subscip_depth(scip) + 1;

    // Allocate and initialize to zero counter arrays for weighted objectives.
    let mut consmarker = vec![false; nvars];
    let mut conscounter = vec![0i32; nvars];
    let mut termcounter = vec![0i32; nvars];

    // Create covering variable for each variable in the original problem (fix
    // it or not?) in the same order as in the original problem.
    for i in 0..nvars {
        let mut ub = 1.0;

        if scip.vars[vars[i]].relaxationonly {
            // Skip relaxation-only variables; they cannot appear in
            // constraints.
            coveringvars[i] = NONE;
            continue;
        }

        // If the variable in the original problem is fixed, then the
        // corresponding cover variable cannot be 1 in any optimal solution of
        // the covering problem (see special termIsConstant treatment below)
        // since some calling code may assume that no fixed variables will
        // appear in the cover (see #1845), but we might not compute an
        // optimal cover here, we fix these variable to 0 here.
        let v = &scip.vars[vars[i]];
        let fixed = if globalbounds {
            scip.set.is_feas_eq(v.glbdom.lb, v.glbdom.ub)
        } else {
            scip.set.is_feas_eq(v.locdom.lb, v.locdom.ub)
        };
        if fixed {
            ub = 0.0;
        }

        let name = format!("{}_covering", scip.vars[vars[i]].name);
        coveringvars[i] = scip_var::create_var_impl(
            coveringscip,
            &name,
            0.0,
            ub,
            1.0,
            crate::types::VarType::Binary,
            crate::types::ImplintType::None,
            true,
            false,
        )?;
        scip_var::add_var(coveringscip, coveringvars[i])?;
    }

    let linflags = ConsFlags {
        initial: true,
        separate: true,
        enforce: true,
        check: true,
        propagate: true,
        local: false,
        modifiable: false,
        dynamic: false,
        removable: false,
        stickingatnode: false,
    };

    // Go through all AND constraints in the original problem.
    let conshdlr = crate::set::set_find_conshdlr(scip, "and");
    if conshdlr != NONE && coverand {
        let nactive = scip.conshdlrs[conshdlr].nactiveconss as usize;
        for c in (0..nactive).rev() {
            // Get constraint and variables.
            let andcons = scip.conshdlrs[conshdlr].conss[c];
            let andvars = crate::cons_and::get_vars_and(scip, andcons).to_vec();

            // Allocate memory for covering constraint.
            let mut coveringconsvars = Vec::with_capacity(andvars.len() + 1);
            let mut coveringconsvals = Vec::with_capacity(andvars.len() + 1);

            // Collect unfixed variables.
            consmarker.fill(false);
            for v in (0..andvars.len()).rev() {
                // If variable is fixed to 0, entire constraint can be
                // linearized.
                if var_is_fixed(scip, andvars[v], 0.0, globalbounds) {
                    coveringconsvars.clear();
                    coveringconsvals.clear();
                    break;
                }

                // If variable is fixed, nothing to do.
                if term_is_constant(scip, andvars[v], 1.0, globalbounds) {
                    continue;
                }

                // If constraints with inactive variables are present, we have to
                // find the corresponding active variable.
                let Some((probindex, negated)) = active_probindex(scip, andvars[v])? else {
                    return Ok(false);
                };

                // Add covering variable for unfixed original variable.
                if negated {
                    coveringconsvars.push(scip_var::get_negated_var(coveringscip, coveringvars[probindex])?);
                } else {
                    coveringconsvars.push(coveringvars[probindex]);
                }
                coveringconsvals.push(1.0);
            }

            // If constraints with inactive variables are present, we have to find
            // the corresponding active variable.
            let andres = crate::cons_and::get_resultant_and(scip, andcons);

            // If resultant is fixed this constraint can be either linearized or
            // is redundant because all operands can be fixed.
            if term_is_constant(scip, andres, 1.0, globalbounds) {
                continue;
            }

            let (probindex, negated) = if scip.vars[andres].probindex == -1 {
                let Some(pn) = active_probindex(scip, andres)? else {
                    return Ok(false);
                };
                pn
            } else if scip.vars[andres].glbdom.lb > 0.5 || scip.vars[andres].glbdom.ub < 0.5 {
                continue;
            } else {
                (scip.vars[andres].probindex as usize, false)
            };

            // If less than two variables are unfixed or the resultant variable is
            // fixed, the entire constraint can be linearized anyway.
            let mut ntofix = coveringconsvars.len();
            if ntofix >= 2 {
                // Add covering variable for unfixed resultant.
                if negated {
                    coveringconsvars.push(scip_var::get_negated_var(coveringscip, coveringvars[probindex])?);
                } else {
                    coveringconsvars.push(coveringvars[probindex]);
                }
                coveringconsvals.push((ntofix - 1) as f64);
                ntofix += 1;

                // Create covering constraint.
                let name = format!("{}_covering", scip.conss[andcons].name);
                let infinity = coveringscip.set.infinity();
                let mut coveringcons = crate::cons_linear::create_cons_linear(
                    coveringscip,
                    &name,
                    &coveringconsvars,
                    &coveringconsvals,
                    (ntofix - 1) as f64,
                    infinity,
                    linflags.initial,
                    linflags.separate,
                    linflags.enforce,
                    linflags.check,
                    linflags.propagate,
                    linflags.local,
                    linflags.modifiable,
                    linflags.dynamic,
                    linflags.removable,
                    linflags.stickingatnode,
                )?;

                // Add and release covering constraint.
                crate::scip_cons::add_cons(coveringscip, coveringcons)?;
                crate::scip_cons::release_cons(coveringscip, &mut coveringcons)?;

                // Update counters.
                for v in (0..ntofix).rev() {
                    let cv = coveringconsvars[v];
                    let idx = if coveringscip.vars[cv].varstatus == VarStatus::Negated {
                        coveringscip.vars[coveringscip.vars[cv].negatedvar].probindex
                    } else {
                        coveringscip.vars[cv].probindex
                    };
                    inc_counters(&mut termcounter, &mut conscounter, &mut consmarker, idx as usize);
                }
            }
        }
    }

    // Go through all bounddisjunction constraints in the original problem.
    let conshdlr = crate::set::set_find_conshdlr(scip, "bounddisjunction");
    if conshdlr != NONE && coverbd {
        let nactive = scip.conshdlrs[conshdlr].nactiveconss as usize;
        for c in (0..nactive).rev() {
            // Get constraint and variables.
            let bdcons = scip.conshdlrs[conshdlr].conss[c];
            let bdvars = crate::cons_bounddisjunction::get_vars_bounddisjunction(scip, bdcons).to_vec();
            let nbdvars = bdvars.len();

            // Bounddisjunction constraints are not passed to the NLP, hence
            // nothing to store in the hash map.

            // Allocate memory for covering constraint.
            let mut coveringconsvars = Vec::with_capacity(nbdvars);
            let mut coveringconsvals = Vec::with_capacity(nbdvars);

            // Collect unfixed variables.
            consmarker.fill(false);
            for v in (0..nbdvars).rev() {
                // If variable is fixed, nothing to do.
                let lb = if globalbounds { scip.vars[bdvars[v]].glbdom.lb } else { scip.vars[bdvars[v]].locdom.lb };
                if var_is_fixed(scip, bdvars[v], lb, globalbounds) {
                    continue;
                }

                // If constraints with inactive variables are present, we have to
                // find the corresponding active variable.
                let Some((probindex, negated)) = active_probindex(scip, bdvars[v])? else {
                    return Ok(false);
                };

                // Add covering variable for unfixed original variable.
                if negated {
                    coveringconsvars.push(scip_var::get_negated_var(coveringscip, coveringvars[probindex])?);
                } else {
                    coveringconsvars.push(coveringvars[probindex]);
                }
                coveringconsvals.push(1.0);
            }

            // If less than two variables are unfixed, the entire constraint can
            // be linearized anyway.
            let ntofix = coveringconsvars.len();
            if ntofix >= 2 {
                // Create covering constraint.
                let name = format!("{}_covering", scip.conss[bdcons].name);
                let infinity = coveringscip.set.infinity();
                let mut coveringcons = crate::cons_linear::create_cons_linear(
                    coveringscip,
                    &name,
                    &coveringconsvars,
                    &coveringconsvals,
                    (ntofix - 1) as f64,
                    infinity,
                    linflags.initial,
                    linflags.separate,
                    linflags.enforce,
                    linflags.check,
                    linflags.propagate,
                    linflags.local,
                    linflags.modifiable,
                    linflags.dynamic,
                    linflags.removable,
                    linflags.stickingatnode,
                )?;

                // Add and release covering constraint.
                crate::scip_cons::add_cons(coveringscip, coveringcons)?;
                crate::scip_cons::release_cons(coveringscip, &mut coveringcons)?;

                // Update counters.
                for v in (0..ntofix).rev() {
                    let cv = coveringconsvars[v];
                    let idx = if coveringscip.vars[cv].varstatus == VarStatus::Negated {
                        coveringscip.vars[coveringscip.vars[cv].negatedvar].probindex
                    } else {
                        coveringscip.vars[cv].probindex
                    };
                    inc_counters(&mut termcounter, &mut conscounter, &mut consmarker, idx as usize);
                }
            }
        }
    }

    // Go through all indicator constraints in the original problem; fix the
    // binary variable.
    let conshdlr = crate::set::set_find_conshdlr(scip, "indicator");
    if conshdlr != NONE && coverind {
        let nactive = scip.conshdlrs[conshdlr].nactiveconss as usize;
        for c in (0..nactive).rev() {
            // Get constraint and variables.
            let indcons = scip.conshdlrs[conshdlr].conss[c];
            let binvar = crate::cons_indicator::get_binary_var_indicator(scip, indcons);

            // Indicator constraints are not passed to the NLP, hence nothing to
            // store in the hash map.

            // If variable is fixed, nothing to do.
            let lb = if globalbounds { scip.vars[binvar].glbdom.lb } else { scip.vars[binvar].locdom.lb };
            if var_is_fixed(scip, binvar, lb, globalbounds) {
                continue;
            }

            // If constraints with inactive variables are present, we have to find
            // the corresponding active variable.
            let Some((probindex, _)) = active_probindex(scip, binvar)? else {
                return Ok(false);
            };

            // Get covering variable for unfixed binary variable in indicator
            // constraint.
            let coveringvar = coveringvars[probindex];

            // Require covering variable to be fixed such that indicator is
            // linearized.
            scip_var::chg_var_lb(coveringscip, coveringvar, 1.0)?;

            // Update counters.
            consmarker.fill(false);
            inc_counters(&mut termcounter, &mut conscounter, &mut consmarker, probindex);
        }
    }

    // Go through all nonlinear constraints in the original problem.
    //
    // @todo: some expr constraints might be SOC and these only need to have
    // all but one variable fixed in order to be linear; however, by just
    // looking at the nlrow representation of a soc constraint, processNlRow
    // doesn't realize this. if more specific information is accessible from
    // expr constrains, then this can be improved
    let conshdlr = crate::set::set_find_conshdlr(scip, "nonlinear");
    if conshdlr != NONE && covernl {
        let nactive = scip.conshdlrs[conshdlr].nactiveconss as usize;
        for c in (0..nactive).rev() {
            // Get constraint.
            let exprcons = scip.conshdlrs[conshdlr].conss[c];

            // Get nlrow representation and store it in hash map.
            let nlrow = crate::cons_nonlinear::get_nl_row_nonlinear(scip, exprcons)?;

            // Process nlrow.
            if !process_nl_row(
                scip,
                nlrow,
                coveringscip,
                coveringvars,
                &mut termcounter,
                &mut conscounter,
                &mut consmarker,
                globalbounds,
                onlyconvexify,
            )? {
                return Ok(false);
            }
        }
    }

    // Set objective function of covering problem.
    for i in (0..nvars).rev() {
        if coveringvars[i] == NONE {
            continue;
        }
        let v = &scip.vars[vars[i]];
        let obj = match coveringobj {
            // Number of influenced nonlinear constraints.
            b'c' => conscounter[i] as f64,
            // Domain size.
            b'd' => {
                if globalbounds {
                    v.glbdom.ub - v.glbdom.lb
                } else {
                    v.locdom.ub - v.locdom.lb
                }
            }
            // Number of locks.
            b'l' => {
                let nlocksup = crate::var::var_get_n_locks_up_type(scip, vars[i], crate::types::LockType::Model);
                let nlocksdown = crate::var::var_get_n_locks_down_type(scip, vars[i], crate::types::LockType::Model);
                (nlocksup + nlocksdown + 1) as f64
            }
            // min(up locks, down locks)+1
            b'm' => {
                let nlocksup = crate::var::var_get_n_locks_up_type(scip, vars[i], crate::types::LockType::Model);
                let nlocksdown = crate::var::var_get_n_locks_down_type(scip, vars[i], crate::types::LockType::Model);
                ((if nlocksup <= nlocksdown { nlocksup } else { nlocksdown }) + 1) as f64
            }
            // Number of influenced nonlinear terms.
            b't' => termcounter[i] as f64,
            // Unit penalties.
            b'u' => 1.0,
            _ => {
                log::error!("invalid choice <{}> for covering objective", coveringobj as char);
                return Ok(false);
            }
        };
        scip_var::chg_var_obj(coveringscip, coveringvars[i], obj)?;
    }

    // Covering problem successfully set up.
    Ok(true)
}

/// `forbidCover`: adds a constraint to the covering problem to forbid the
/// given cover; returns `(success, infeas)`.
fn forbid_cover(scip: &mut Scip, vars: &[VarId], cover: &[usize], diversification: i32) -> ScipResult<(bool, bool)> {
    let mut infeas = false;
    let mut consvars = Vec::with_capacity(cover.len());
    let mut cons = NONE;

    // Create constraint name.
    let name = "forbid_cover_assignment";
    let flags = ConsFlags {
        initial: true,
        separate: true,
        enforce: true,
        check: true,
        propagate: true,
        local: false,
        modifiable: false,
        dynamic: false,
        removable: false,
        stickingatnode: false,
    };

    // If all variables in the cover are binary and we require only one variable
    // to change its value, then we create a set covering constraint.
    if diversification == 1 {
        // Build up constraint.
        for &c in cover.iter().rev() {
            if vars[c] != NONE && !scip.set.is_feas_ge(scip.vars[vars[c]].locdom.lb, 1.0) {
                consvars.push(scip_var::get_negated_var(scip, vars[c])?);
            }
        }

        // If all covering variables are fixed to one, the constraint cannot be
        // satisfied.
        if consvars.is_empty() {
            infeas = true;
        } else {
            // Create constraint.
            cons = crate::cons_setppc::create_cons_setcover(scip, name, &consvars, &flags)?;
        }
    }
    // If all variables in the cover are binary and we require more variables to
    // change their value, then we create a linear constraint.
    else {
        // Build up constraint.
        let mut consvals = Vec::with_capacity(cover.len());
        for &c in cover.iter().rev() {
            if vars[c] != NONE && !scip.set.is_feas_ge(scip.vars[vars[c]].locdom.lb, 1.0) {
                consvars.push(vars[c]);
                consvals.push(1.0);
            }
        }
        let rhs = (consvars.len() as i32 - diversification) as f64;

        // If too many covering variables are fixed to 1, the constraint cannot
        // be satisfied.
        if rhs < 0.0 {
            infeas = true;
        } else {
            // Create constraint.
            let infinity = scip.set.infinity();
            cons = crate::cons_linear::create_cons_linear(
                scip,
                name,
                &consvars,
                &consvals,
                -infinity,
                rhs,
                flags.initial,
                flags.separate,
                flags.enforce,
                flags.check,
                flags.propagate,
                flags.local,
                flags.modifiable,
                flags.dynamic,
                flags.removable,
                flags.stickingatnode,
            )?;
        }
    }

    // If proven infeasible, we do not even add the constraint; otherwise we add
    // and release the constraint if created successfully.
    if !infeas && cons != NONE {
        crate::scip_cons::add_cons(scip, cons)?;
        crate::scip_cons::release_cons(scip, &mut cons)?;
        return Ok((true, infeas));
    }

    Ok((false, infeas))
}

/// `createConflict`: adds a set covering or bound disjunction constraint to
/// the original problem; returns whether the cutoff constraint was created
/// successfully.
fn create_conflict(scip: &mut Scip, bdvars: &[VarId], bdtypes: &[BoundType], bdbounds: &[f64], local: bool, dynamic: bool, removable: bool) -> ScipResult<bool> {
    let bdlen = bdvars.len();

    // Create constraint name.
    let name = "undercover_cutoff";

    // Check if all variables in the cover are binary.
    let mut isbinary = true;
    let mut i = bdlen as isize - 1;
    while i >= 0 && isbinary {
        isbinary = crate::var::var_is_binary(scip, bdvars[i as usize]);
        i -= 1;
    }

    let flags = ConsFlags {
        initial: false,
        separate: true,
        enforce: false,
        check: false,
        propagate: true,
        local,
        modifiable: false,
        dynamic,
        removable,
        stickingatnode: false,
    };

    let mut cons = if isbinary {
        // If all variables in the cover are binary, then we create a logicor
        // constraint; build up constraint.
        let mut consvars = vec![NONE; bdlen];
        for i in (0..bdlen).rev() {
            if bdtypes[i] == BoundType::Lower {
                consvars[i] = bdvars[i];
            } else {
                consvars[i] = scip_var::get_negated_var(scip, bdvars[i])?;
            }
        }

        // Create conflict constraint.
        crate::cons_logicor::create_cons_logicor(scip, name, &consvars, &flags)?
    } else {
        // Otherwise we create a bound disjunction constraint as given.
        crate::cons_bounddisjunction::create_cons_bounddisjunction(scip, name, bdvars, bdtypes, bdbounds, &flags)?
    };

    // Add and release constraint if created successfully.
    if local {
        crate::scip_cons::add_cons_local(scip, cons, NONE)?;
    } else {
        crate::scip_cons::add_cons(scip, cons)?;
    }

    crate::scip_cons::release_cons(scip, &mut cons)?;

    Ok(true)
}

/// `solveCoveringProblem`: solve covering problem; returns whether a
/// feasible cover was found, with the problem indices of its variables in
/// `cover`.
fn solve_covering_problem(coveringscip: &mut Scip, coveringvars: &[VarId], cover: &mut Vec<usize>, timelimit: f64, memorylimit: f64, objlimit: f64) -> ScipResult<bool> {
    // Forbid call of heuristics and separators solving sub-CIPs.
    crate::paramset::set_subscips_off(coveringscip, true)?;

    // Set presolving and separation to fast.
    crate::paramset::set_separating(coveringscip, ParamSetting::Fast, true)?;
    crate::paramset::set_presolving(coveringscip, ParamSetting::Fast, true)?;

    // Use inference branching.
    if crate::set::set_find_branchrule(coveringscip, "inference") != NONE && !paramset_is_fixed(coveringscip, "branching/inference/priority")? {
        paramset_set_int(coveringscip, "branching/inference/priority", i32::MAX / 4)?;
    }

    // Only solve root.
    paramset_set_longint(coveringscip, "limits/nodes", 1)?;

    // Set time, memory, and objective limit.
    paramset_set_real(coveringscip, "limits/time", timelimit)?;
    paramset_set_real(coveringscip, "limits/memory", memorylimit)?;
    api::set_objlimit(coveringscip, objlimit)?;

    // Do not abort on CTRL-C.
    paramset_set_bool(coveringscip, "misc/catchctrlc", false)?;

    // Disable output to console in optimized mode, enable in SCIP's debug mode.
    paramset_set_int(coveringscip, "display/verblevel", 0)?;

    // Solve covering problem (errors are passed on, as in the reference's
    // debug build).
    crate::scip_solve::solve(coveringscip)?;

    // Check, whether a feasible cover was found.
    if api::get_n_sols(coveringscip) == 0 {
        return Ok(false);
    }

    // Store solution.
    cover.clear();
    let bestsol = api::get_best_sol(coveringscip);
    for (i, &cv) in coveringvars.iter().enumerate() {
        if cv == NONE {
            continue;
        }

        if api::get_sol_val(coveringscip, bestsol, cv) > 0.5 {
            cover.push(i);
        }
    }

    Ok(true)
}

/// `computeFixingOrder`: computes fixing order and returns whether order has
/// really been changed.
fn compute_fixing_order(scip: &mut Scip, heur: HeurId, vars: &[VarId], cover: &mut [usize], lastfailed: usize) -> ScipResult<bool> {
    let coversize = cover.len();

    // If fixing the first variable had failed, do not try with another order.
    if lastfailed == 0 {
        return Ok(false);
    }

    // Allocate buffer array for score values.
    let mut scores = vec![0.0; coversize];

    // Initialize best score to infinite value.
    let fixingorder = hd(scip, heur).fixingorder;
    let sortdown = fixingorder == b'c' || fixingorder == b'v';
    let mut bestscore = if sortdown { -scip.set.infinity() } else { scip.set.infinity() };

    // Compute score values.
    for i in (0..coversize).rev() {
        // Get variable in the cover.
        let var = vars[cover[i]];

        if fixingorder == b'C' || fixingorder == b'c' {
            // Add a small pertubation value to the score to reduce performance
            // variability.
            let (conflictweight, inferenceweight, cutoffweight) = {
                let h = hd(scip, heur);
                (h.conflictweight, h.inferenceweight, h.cutoffweight)
            };
            let eps = scip.set.epsilon();
            let conflictscore = scip_var::get_var_conflict_score(scip, var);
            let inferencescore = crate::branch_inference::get_var_avg_inference_cutoff_score(scip, var, cutoffweight);
            let pert = hdm(scip, heur).randnumgen.as_mut().unwrap().get_real(0.0, eps);
            scores[i] = conflictweight * conflictscore + inferenceweight * inferencescore + pert;
        } else if fixingorder == b'V' || fixingorder == b'v' {
            scores[i] = cover[i] as f64;
        } else {
            return Err(Retcode::ParameterWrongVal);
        }

        // Update best score.
        bestscore = if sortdown { cmax(bestscore, scores[i]) } else { cmin(bestscore, scores[i]) };
    }

    // Put integers to the front.
    if hd(scip, heur).fixintfirst {
        for i in (0..coversize).rev() {
            if crate::var::var_is_integral(scip, vars[cover[i]]) {
                if sortdown {
                    scores[i] += bestscore + 1.0;
                } else {
                    scores[i] = bestscore - 1.0 / (scores[i] + 1.0);
                }
            }
        }
    }

    // Put last failed variable to the front.
    if lastfailed < coversize {
        if sortdown {
            scores[lastfailed] += bestscore + 2.0;
        } else {
            scores[lastfailed] = bestscore - 2.0 / (scores[lastfailed] + 1.0);
        }
    }

    // Sort by non-decreasing (negative) score.
    if sortdown {
        crate::sort::sort_down(&mut scores[..], (&mut cover[..],), crate::sort::cmp_real);
    } else {
        crate::sort::sort(&mut scores[..], (&mut cover[..],), crate::sort::cmp_real);
    }

    Ok(true)
}

/// `getFixingValue`: gets fixing value; returns whether it could be
/// retrieved successfully.
fn get_fixing_value(scip: &Scip, var: VarId, val: &mut f64, fixalt: u8) -> bool {
    let mut success = false;

    match fixalt {
        b'l' => {
            // Get the last LP solution value.
            *val = crate::var::var_get_lp_sol(scip, var);
            success = true;
        }
        b'n' => {
            // Only called if the NLP relaxation is available, which needs an NLP
            // solver.
            unreachable!("NLP fixing values need an NLP solver");
        }
        b'i' => {
            // Get value in the incumbent solution.
            *val = api::get_sol_val(scip, api::get_best_sol(scip), var);
            success = true;
        }
        _ => {}
    }

    // Due to propagation (during probing) it might happen that the LP and NLP
    // solution value of var might be outside of its bounds.
    *val = cmax(*val, scip.vars[var].locdom.lb);
    *val = cmin(*val, scip.vars[var].locdom.ub);

    success
}

/// `calculateAlternatives`: calculates up to four alternative values for
/// backtracking, if fixing the variable failed. The alternatives are the two
/// bounds of the variable, and the averages of the bounds and the fixing
/// value. For infinite bounds, fixval +/- abs(fixval) will be used instead.
fn calculate_alternatives(scip: &Scip, var: VarId, fixval: f64) -> Vec<f64> {
    let mut alternatives = Vec::with_capacity(4);

    // For binary variables, there is only two possible fixing values.
    if crate::var::var_is_binary(scip, var) {
        if scip.set.is_feas_eq(fixval, 0.0) || scip.set.is_feas_eq(fixval, 1.0) {
            alternatives.push(1.0 - fixval);
        } else {
            alternatives.push(0.0);
            alternatives.push(1.0);
        }
        return alternatives;
    }

    // Get bounds.
    let mut lb = scip.vars[var].locdom.lb;
    let mut ub = scip.vars[var].locdom.ub;

    // If lower bound is infinite, use x'-|x'|; if x' is zero, use -1.0 instead.
    if scip.set.is_infinity(-lb) {
        lb = if scip.set.is_feas_zero(fixval) { -1.0 } else { fixval - fixval.abs() };
    }

    // If upper bound is infinite, use x'+|x'|; if x' is zero, use 1.0 instead.
    if scip.set.is_infinity(ub) {
        ub = if scip.set.is_feas_zero(fixval) { 1.0 } else { fixval + fixval.abs() };
    }

    // Use lower bound if not equal to x'.
    if !scip.set.is_feas_eq(lb, fixval) {
        alternatives.push(lb);
    }

    // Use upper bound if not equal to x'.
    if !scip.set.is_feas_eq(ub, fixval) {
        alternatives.push(ub);
    }

    let isintegral = crate::var::var_is_integral(scip, var);

    // Use the average of x' and lower bound as alternative value, if this is
    // not equal to any of the other values.
    if !scip.set.is_feas_eq(lb, fixval) && (!isintegral || !scip.set.is_feas_eq(lb, fixval - 1.0)) {
        alternatives.push((lb + fixval) / 2.0);
    }

    // Use the average of x' and upper bound as alternative value, if this is
    // not equal to any of the other values.
    if !scip.set.is_feas_eq(ub, fixval) && (!isintegral || !scip.set.is_feas_eq(ub, fixval + 1.0)) {
        alternatives.push((ub + fixval) / 2.0);
    }

    alternatives
}

/// `roundFixingValue`: rounds the given fixing value.
fn round_fixing_value(scip: &Scip, val: &mut f64, var: VarId, locksrounding: bool) {
    let set = &scip.set;
    let mut x = *val;
    let nlocksdown = crate::var::var_get_n_locks_down_type(scip, var, crate::types::LockType::Model);
    let nlocksup = crate::var::var_get_n_locks_up_type(scip, var, crate::types::LockType::Model);

    // If integral within feasibility tolerance, only shift to nearest integer.
    if set.is_feas_integral(x) {
        x = if set.feas_frac(x) < 0.5 { set.feas_floor(x) } else { set.feas_ceil(x) };
    }
    // Round in the direction of least locks with fractionality as tie breaker.
    else if locksrounding {
        if nlocksdown < nlocksup {
            x = set.feas_floor(x);
        } else if nlocksdown > nlocksup {
            x = set.feas_ceil(x);
        } else {
            x = if set.feas_frac(x) < 0.5 { set.feas_floor(x) } else { set.feas_ceil(x) };
        }
    }
    // Round in the direction of least fractionality with locks as tie breaker.
    else if set.feas_frac(x) < 0.5 {
        x = set.feas_floor(x);
    } else if set.feas_frac(x) > 0.5 {
        x = set.feas_ceil(x);
    } else {
        x = if nlocksdown < nlocksup { set.feas_floor(x) } else { set.feas_ceil(x) };
    }

    // Return rounded fixing value.
    *val = x;
}

/// `solveSubproblem`: solve subproblem and pass best feasible solution to
/// original SCIP instance; returns `(validsolved, sol, nusednodes)`.
#[allow(clippy::too_many_arguments)]
fn solve_subproblem(
    scip: &mut Scip,
    heur: HeurId,
    cover: &[usize],
    fixedvals: &[f64],
    timelimit: f64,
    memorylimit: f64,
    nodelimit: i64,
    nstallnodes: i64,
) -> ScipResult<(bool, crate::types::SolId, i64)> {
    let mut sol = NONE;

    // Get required data of the original problem.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    // Fix subproblem variables in the cover.
    let fixedvars: Vec<VarId> = cover.iter().map(|&c| vars[c]).collect();

    // Create subproblem.
    let mut subscip = Scip::create()?;

    // Create the variable mapping hash map.
    let mut maps = crate::copy::CopyMaps::new(nvars as i32, 0);

    // Copy original problem to subproblem; do not copy pricers.
    let (globalbounds, copycuts, minimprove) = {
        let h = hd(scip, heur);
        (h.globalbounds, h.copycuts, h.minimprove)
    };
    let mut validsolved = crate::copy::copy_cons_compression(scip, &mut subscip, &mut maps, "undercoversub", &fixedvars, &fixedvals[..cover.len()], globalbounds, false)?;

    if copycuts {
        // Copies all active cuts from cutpool of sourcescip to linear
        // constraints in targetscip.
        crate::copy::copy_cuts_public(scip, &mut subscip, &mut maps, globalbounds)?;
    }

    // Store subproblem variables.
    let subvars: Vec<VarId> = vars.iter().map(|&v| maps.var_image(v)).collect();

    // Set the parameters such that good solutions are found fast.
    crate::paramset::set_emphasis(&mut subscip, ParamEmphasis::Feasibility, true)?;
    crate::paramset::set_presolving(&mut subscip, ParamSetting::Fast, true)?;
    crate::paramset::set_heuristics(&mut subscip, ParamSetting::Aggressive, true)?;

    // Deactivate expensive pre-root heuristics, since it may happen that the
    // lp relaxation of the subproblem is already infeasible; in this case, we
    // do not want to waste time on heuristics before solving the root lp.
    if !paramset_is_fixed(&subscip, "heuristics/shiftandpropagate/freq")? {
        paramset_set_int(&mut subscip, "heuristics/shiftandpropagate/freq", -1)?;
    }
    // The zeroobj heuristic is not ported (its default frequency is -1).
    if crate::paramset::paramset_get_param(&subscip, "heuristics/zeroobj/freq").is_some() {
        unimplemented!("heuristics/zeroobj");
    }

    // Forbid recursive call of undercover heuristic.
    if paramset_is_fixed(&subscip, "heuristics/undercover/freq")? {
        log::warn!("unfixing parameter heuristics/undercover/freq in subscip of undercover heuristic to avoid recursive calls");
        crate::paramset::paramset_fix(&mut subscip, "heuristics/undercover/freq", false)?;
    }
    paramset_set_int(&mut subscip, "heuristics/undercover/freq", -1)?;

    // Disable statistic timing inside sub SCIP.
    paramset_set_bool(&mut subscip, "timing/statistictiming", false)?;

    // Set time, memory and node limits.
    paramset_set_real(&mut subscip, "limits/time", timelimit)?;
    paramset_set_real(&mut subscip, "limits/memory", memorylimit)?;
    paramset_set_longint(&mut subscip, "limits/nodes", nodelimit)?;
    paramset_set_longint(&mut subscip, "limits/stallnodes", nstallnodes)?;

    // Do not abort subproblem on CTRL-C.
    paramset_set_bool(&mut subscip, "misc/catchctrlc", false)?;

    // Disable output to console in optimized mode, enable in SCIP's debug mode.
    paramset_set_int(&mut subscip, "display/verblevel", 0)?;

    // If there is already a solution, add an objective cutoff; note: this does
    // not affect the validity of the subproblem if we find solutions later,
    // thus we do not set *validsolved to FALSE.
    if api::get_n_sols(scip) > 0 {
        let upperbound = api::get_upperbound(scip);

        let mut cutoff = if scip.set.is_infinity(-api::get_lowerbound(scip)) {
            (if upperbound >= 0.0 { 1.0 - minimprove } else { 1.0 + minimprove }) * upperbound
        } else {
            (1.0 - minimprove) * upperbound + minimprove * api::get_lowerbound(scip)
        };

        cutoff = cmin(upperbound, cutoff);
        api::set_objlimit(&mut subscip, cutoff)?;
    }

    // Solve subproblem (errors are passed on, as in the reference's debug
    // build).
    crate::scip_solve::solve(&mut subscip)?;

    // Store solving status; note: if we proved infeasibility in presence of an
    // objective cutoff beyond the primal bound, the subproblem was not a valid
    // copy.
    let substatus = api::get_status(&subscip);
    validsolved = validsolved && (substatus == Status::Optimal || (substatus == Status::Infeasible && (api::get_n_sols(scip) == 0 || minimprove <= 0.0)));
    let nusednodes = api::get_n_nodes(&subscip);

    // If a solution was found for the subproblem, create corresponding
    // solution in the original problem.
    if api::get_n_sols(&subscip) > 0 && (substatus != Status::Infeasible || minimprove > 0.0) {
        let mut success = false;

        // Check, whether a solution was found; due to numerics, it might happen
        // that not all solutions are feasible -> try all solutions until one
        // was accepted.
        let subsols = api::get_sols(&subscip).to_vec();

        let mut i = 0;
        while i < subsols.len() {
            // Transform solution to original problem.
            sol = crate::copy::translate_sub_sol(scip, &subscip, subsols[i], heur, &subvars)?;

            // Try to add new solution to scip.
            success = api::try_sol(scip, sol, false, false, true, true, true)?;

            if success {
                break;
            } else {
                // Free solution structure, since SCIPtranslateSubSol would
                // recreate in the next round.
                api::free_sol(scip, sol)?;
                sol = NONE;
            }
            i += 1;
        }

        // If the best subproblem solution was not accepted in the original
        // problem, then we do not trust the solving status.
        if !success || i > 0 {
            validsolved = false;
        }
    }

    if validsolved {
        crate::copy::merge_variable_statistics(&subscip, scip, &subvars, &vars)?;
    }

    Ok((validsolved, sol, nusednodes))
}

/// The bound disjunction information along the probing path. C also keeps
/// the bounds before fixing (`oldbounds`), which only `getFixingValue` reads
/// for NLP fixing values; those need an NLP solver.
#[derive(Default)]
struct BdInfo {
    /// Variables with bound changes along probing path.
    vars: Vec<VarId>,
    /// Bound types in bound disjunction.
    types: Vec<BoundType>,
    /// Bounds in bound disjunction.
    bounds: Vec<f64>,
}

impl BdInfo {
    fn truncate(&mut self, len: usize) {
        self.vars.truncate(len);
        self.types.truncate(len);
        self.bounds.truncate(len);
    }
}

/// `performFixing`: perform fixing of a variable and record bound
/// disjunction information; returns whether the fixing lead to
/// infeasibility.
fn perform_fixing(scip: &mut Scip, var: VarId, mut val: f64, bd: &mut BdInfo) -> ScipResult<bool> {
    // Remember length of probing path.
    let oldbdlen = bd.vars.len();

    // Get bounds of the variable to fix.
    let mut oldlb = scip.vars[var].locdom.lb;
    let mut oldub = scip.vars[var].locdom.ub;

    // Decrease upper bound to fixing value.
    let mut infeas;
    if scip.set.is_ub_better(val, oldlb, oldub) {
        // We only want to open a new probing node if we do not exceed the
        // maximal tree depth.
        if api::get_depth(scip) < crate::def::MAXTREEDEPTH {
            // Create next probing node.
            scip_probing::new_probing_node(scip)?;
        }
        scip_probing::chg_var_ub_probing(scip, var, val)?;

        // Store bound disjunction information.
        bd.vars.push(var);
        bd.types.push(BoundType::Lower);
        bd.bounds.push(if crate::var::var_is_integral(scip, var) { scip.set.feas_ceil(val) + 1.0 } else { val });

        // Propagate the bound change; conflict analysis is performed
        // automatically.
        (infeas, _) = scip_probing::propagate_probing(scip, 0)?;

        // If propagation led to a cutoff, we backtrack immediately.
        if infeas {
            bd.truncate(oldbdlen);
            return Ok(infeas);
        }

        // Move fixing value into the new domain, since it may be outside due
        // to numerical issues or previous propagation.
        oldlb = scip.vars[var].locdom.lb;
        oldub = scip.vars[var].locdom.ub;
        val = cmin(val, oldub);
        val = cmax(val, oldlb);
    }

    // Update lower bound to fixing value.
    infeas = false;
    if scip.set.is_lb_better(val, oldlb, oldub) {
        // We only want to open a new probing node if we do not exceed the
        // maximal tree depth.
        if api::get_depth(scip) < crate::def::MAXTREEDEPTH {
            // Create next probing node.
            scip_probing::new_probing_node(scip)?;
        }
        scip_probing::chg_var_lb_probing(scip, var, val)?;

        // Store bound disjunction information.
        bd.vars.push(var);
        bd.types.push(BoundType::Upper);
        bd.bounds.push(if crate::var::var_is_integral(scip, var) { scip.set.feas_ceil(val) - 1.0 } else { val });

        // Propagate the bound change.
        (infeas, _) = scip_probing::propagate_probing(scip, 0)?;

        // If propagation led to a cutoff, we backtrack immediately.
        if infeas {
            bd.truncate(oldbdlen);
            return Ok(infeas);
        }
    }

    Ok(infeas)
}

/// `fixAndPropagate`: round-fix-propagate-analyze-backtrack for each
/// variable in the cover; returns `(infeas, lastfailed, nfixedints,
/// nfixedconts)`, with `lastfailed` the position in cover of the variable the
/// fixing of which yielded infeasibility.
fn fix_and_propagate(scip: &mut Scip, heur: HeurId, cover: &[usize], fixingvals: &mut [f64], bd: &mut BdInfo) -> ScipResult<(bool, usize, i32, i32)> {
    let coversize = cover.len();

    // Start probing in original problem.
    let lpsolved = api::get_lp_solstat(scip) == LpSolStat::Optimal;
    scip_probing::start_probing(scip)?;

    // Initialize data.
    let mut nfixedints = 0;
    let mut nfixedconts = 0;
    bd.truncate(0);
    let vars = api::get_vars(scip).to_vec();
    let mut infeas = false;

    let (maxbacktracks, nfixingalts, fixingalts, nlpfailed, locksrounding) = {
        let h = hd(scip, heur);
        (h.maxbacktracks, h.nfixingalts, h.fixingalts.clone().into_bytes(), h.nlpfailed, h.locksrounding)
    };

    // Round-fix-propagate-analyze-backtrack for each variable in the cover.
    //
    // TODO doing a fix-and-propagate for one variable at a time can be very
    // expensive for large covers (try, e.g., junkturn with
    // maxcoversizevars=1) consider splitting the cover into at most, say, 100
    // batches, and fix a complete batch before propagating
    let mut i = 0;
    while i < coversize && !infeas {
        // Get probindex of next variable in the cover.
        let idx = cover[i];

        // Nothing to do if the variable was already fixed, e.g., by
        // propagation.
        if scip.set.is_eq(scip.vars[vars[idx]].locdom.lb, scip.vars[vars[idx]].locdom.ub) {
            fixingvals[i] = scip.vars[vars[idx]].locdom.lb;
            i += 1;
            continue;
        }

        // We will store the fixing values already used to avoid try the same
        // value twice.
        let mut usedvals: Vec<f64> = Vec::with_capacity(maxbacktracks as usize + 1);

        // Backtracking loop.
        infeas = true;
        let mut nfailedvals = 0;
        let mut boundalts: Option<Vec<f64>> = None;
        let mut val = 0.0;
        let mut nbacktracks = 0;
        while nbacktracks <= maxbacktracks + nfailedvals && infeas {
            let probingdepth = scip_probing::get_probing_depth(scip);

            // Get fixing value.
            if nbacktracks < nfixingalts {
                let fixalt = fixingalts[nbacktracks as usize];

                // If the lp relaxation is not solved, we do not even try to
                // retrieve the lp solution value; if the NLP relaxation is not
                // constructed, we do not even try to retrieve the NLP solution
                // value; if there is no feasible solution yet, we do not even try
                // to obtain the value in the incumbent.
                let mut success = false;
                if (fixalt != b'l' || lpsolved) && (fixalt != b'n' || !nlpfailed) && (fixalt != b'i' || api::get_best_sol(scip) != NONE) {
                    success = get_fixing_value(scip, vars[idx], &mut val, fixalt);
                }

                if !success {
                    nfailedvals += 1;
                    nbacktracks += 1;
                    continue;
                }

                // For the first (successfully retrieved) fixing value, compute
                // (at most 4) bound dependent alternative fixing values.
                if boundalts.is_none() {
                    boundalts = Some(calculate_alternatives(scip, vars[idx], val));
                }
            }
            // Get alternative fixing value.
            else if let Some(alts) = boundalts.as_ref()
                && nbacktracks < nfixingalts + alts.len() as i32
            {
                val = alts[(nbacktracks - nfixingalts) as usize];
            } else {
                break;
            }

            // Round fixing value.
            if crate::var::var_is_integral(scip, vars[idx]) && !scip.set.is_integral(val) {
                round_fixing_value(scip, &mut val, vars[idx], locksrounding);
            }

            // Move value into the domain, since it may be outside due to
            // numerical issues or previous propagation.
            let oldlb = scip.vars[vars[idx]].locdom.lb;
            let oldub = scip.vars[vars[idx]].locdom.ub;
            val = cmin(val, oldub);
            val = cmax(val, oldlb);

            // Check if this fixing value was already used.
            let mut usedbefore = false;
            let mut j = usedvals.len() as isize - 1;
            while j >= 0 && !usedbefore {
                usedbefore = scip.set.is_feas_eq(val, usedvals[j as usize]);
                j -= 1;
            }

            if usedbefore {
                nfailedvals += 1;
                nbacktracks += 1;
                continue;
            }

            // Store fixing value.
            usedvals.push(val);

            // Fix-propagate-analyze.
            infeas = perform_fixing(scip, vars[idx], val, bd)?;

            // If infeasible, backtrack and try alternative fixing value.
            if infeas {
                scip_probing::backtrack_probing(scip, probingdepth)?;
            }
            nbacktracks += 1;
        }

        // Backtracking loop unsuccessful.
        if infeas {
            break;
        }
        // Fixing successful.
        else {
            // Store successful fixing value.
            fixingvals[i] = val;

            // Statistics.
            if !crate::var::var_is_integral(scip, vars[idx]) {
                nfixedconts += 1;
            } else {
                nfixedints += 1;
            }
        }
        i += 1;
    }

    // End of dive.
    scip_probing::end_probing(scip)?;

    Ok((infeas, i, nfixedints, nfixedconts))
}

/// `SCIPapplyUndercover`: main procedure of the undercover heuristic.
fn apply_undercover(scip: &mut Scip, heur: HeurId, result: &mut ScipResultCode, mut timelimit: f64, mut memorylimit: f64, nstallnodes: i64) -> ScipResult {
    // Create and start timing.
    let mut clock = Clock(Instant::now());

    // NLP relaxation has not been solved yet (only solve once, not again for
    // each cover or dive, because it is expensive).
    hdm(scip, heur).nlpsolved = false;

    // If solving the NLP relaxation has failed too often in previous runs, or
    // NLP and NLP solver is not available, we do not even try.
    let nlpfailed = hd(scip, heur).nnlpfails >= MAXNLPFAILS || !crate::nlp::is_nlp_constructed(scip) || NNLPIS == 0;
    hdm(scip, heur).nlpfailed = nlpfailed;

    // Get variable data of original problem.
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    // Get number of nonlinear constraints.
    let nnlconss: i32 = hd(scip, heur).nlconshdlrs.iter().map(|&h| scip.conshdlrs[h].nactiveconss).sum();

    // Run only if problem is sufficiently nonlinear.
    let nconss = api::get_n_conss(scip);
    if (nnlconss as f64) < nconss as f64 * hd(scip, heur).mincoveredrel || nnlconss < hd(scip, heur).mincoveredabs {
        return Ok(());
    }

    // Calculate upper bound for cover size.
    let (maxcoversizevars, maxcoversizeconss) = (hd(scip, heur).maxcoversizevars, hd(scip, heur).maxcoversizeconss);
    let mut maxcoversize = if maxcoversizevars < 1.0 {
        let mut maxcoversize = 0.0;
        for &v in &vars {
            if !scip.vars[v].relaxationonly {
                maxcoversize += 1.0;
            }
        }
        maxcoversize * maxcoversizevars
    } else {
        // If maxcoversizevars == 1.0, then there is no limit derived from
        // number of variables.
        nvars as f64
    };
    if maxcoversizeconss < crate::def::REAL_MAX {
        let maxcoversizeconss = maxcoversizeconss * nnlconss as f64 / nconss as f64;
        maxcoversize = cmin(maxcoversize, maxcoversizeconss);
    }

    // Create covering problem.
    let mut coveringscip = Scip::create()?;
    crate::plugins::include_default_plugins(&mut coveringscip)?;
    let mut coveringvars = vec![NONE; nvars];
    let ret = apply_with_covering(
        scip,
        heur,
        result,
        &mut coveringscip,
        &mut coveringvars,
        &mut clock,
        &mut timelimit,
        &mut memorylimit,
        nstallnodes,
        maxcoversize,
        nnlconss,
    );

    // TERMINATE:
    // We must remain in NLP diving mode until here to be able to retrieve NLP
    // solution values easily (the NLP is never solved without an NLP solver).

    // Free covering problem.
    for i in (0..nvars).rev() {
        if coveringvars[i] == NONE {
            continue;
        }
        api::release_var(&mut coveringscip, &mut coveringvars[i])?;
    }

    ret
}

/// The part of `SCIPapplyUndercover` that runs with the covering problem at
/// hand; returning ends at its `TERMINATE` label.
#[allow(clippy::too_many_arguments)]
fn apply_with_covering(
    scip: &mut Scip,
    heur: HeurId,
    result: &mut ScipResultCode,
    coveringscip: &mut Scip,
    coveringvars: &mut [VarId],
    clock: &mut Clock,
    timelimit: &mut f64,
    memorylimit: &mut f64,
    nstallnodes: i64,
    maxcoversize: f64,
    nnlconss: i32,
) -> ScipResult {
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    let (globalbounds, onlyconvexify, coverand, coverbd, coverind, covernl, coveringobj) = {
        let h = hd(scip, heur);
        (h.globalbounds, h.onlyconvexify, h.coverand, h.coverbd, h.coverind, h.covernl, h.coveringobj)
    };
    let success = create_covering_problem(
        scip,
        coveringscip,
        coveringvars,
        globalbounds,
        onlyconvexify,
        coverand,
        coverbd,
        coverind,
        covernl,
        coveringobj,
    )?;

    if !success {
        return Ok(());
    }

    // Count number of unfixed covering variables.
    let mut nunfixeds = 0;
    for i in (0..nvars).rev() {
        if coveringvars[i] != NONE && coveringscip.set.is_feas_eq(coveringscip.vars[coveringvars[i]].glbdom.lb, 1.0) {
            nunfixeds += 1;
        }
    }

    // Update time limit.
    clock.update_timelimit(timelimit);

    if *timelimit <= MINTIMELEFT {
        return Ok(());
    }

    // Update memory left (SCIPgetMemUsed() and SCIPgetMemExternEstim() of the
    // covering problem count as zero).
    *memorylimit -= 0.0;

    // Allocate memory for storing bound disjunction information along probing
    // path.
    let mut bd = BdInfo::default();

    // Initialize data for recovering loop.
    let mut cover: Vec<usize> = Vec::with_capacity(nvars);
    let mut fixingvals = vec![0.0; nvars];
    let mut ncovers = 0;
    let mut success = false;
    let mut reusecover = false;

    let nfixingalts = hd(scip, heur).fixingalts.len() as i32;
    hdm(scip, heur).nfixingalts = nfixingalts;

    let (maxrecovers, maxreorders, maxcoversizeconss) = {
        let h = hd(scip, heur);
        (h.maxrecovers, h.maxreorders, h.maxcoversizeconss)
    };
    let nconss = api::get_n_conss(scip);

    // Recovering loop.
    while (ncovers <= maxrecovers || reusecover) && !success {
        let mut conflictcreated = false;
        bd.truncate(0);

        // Solve covering problem.
        if !reusecover {
            success = solve_covering_problem(coveringscip, coveringvars, &mut cover, *timelimit, *memorylimit, maxcoversize)?;

            ncovers += 1;

            // Free transformed covering problem immediately.
            crate::scip_solve::free_transform_public(coveringscip)?;

            // Terminate if no feasible cover was found.
            if !success {
                return Ok(());
            }

            // Terminate, if cover is empty or too large.
            let coversize = cover.len();
            if coversize == 0 || coversize as f64 > maxcoversize {
                return Ok(());
            }

            // Terminate, if cover too large for the ratio of nonlinear
            // constraints.
            if maxcoversizeconss < crate::def::REAL_MAX && coversize as f64 > maxcoversizeconss * nnlconss as f64 / nconss as f64 {
                return Ok(());
            }
        }
        let coversize = cover.len();

        // Data setup.
        let mut ndives = 0;
        let mut nfixedconts = 0;
        success = false;
        let mut lastfailed = if reusecover {
            let c = coversize as i32 - 1;
            (if 1 >= c { 1 } else { c }) as usize
        } else {
            coversize
        };

        // Round-fix-propagate-analyze-backtrack-reorder loop.
        while ndives <= maxreorders && !success {
            // Compute fixing order.
            let mut reordered = compute_fixing_order(scip, heur, &vars, &mut cover, lastfailed)?;
            reordered = reordered || ndives == 0;

            // Stop if order has not changed.
            if !reordered {
                break;
            }

            let (infeas, failed, _nfixedints, fixedconts) = fix_and_propagate(scip, heur, &cover, &mut fixingvals, &mut bd)?;
            lastfailed = failed;
            nfixedconts = fixedconts;
            ndives += 1;
            success = !infeas;
        }

        // Update time limit.
        clock.update_timelimit(timelimit);

        if *timelimit <= MINTIMELEFT {
            return Ok(());
        }

        // No feasible fixing could be found for the current cover.
        if success {
            // Solve sub-CIP and pass feasible solutions to original problem.
            let maxnodes = hd(scip, heur).maxnodes;
            let (validsolved, mut sol, nsubnodes) = solve_subproblem(scip, heur, &cover, &fixingvals, *timelimit, *memorylimit, maxnodes, nstallnodes)?;

            // Update number of sub-CIP nodes used by heuristic so far.
            hdm(scip, heur).nusednodes += nsubnodes;

            // If the subproblem was constructed from a valid copy and solved,
            // try to forbid the assignment of fixing values to variables in the
            // cover.
            if validsolved {
                let minmaxvars = paramset_get_int(scip, "conflict/minmaxvars")?;
                let maxvarsfac = paramset_get_real(scip, "conflict/maxvarsfac")?;

                let bdlen = bd.vars.len() as i32;
                let useconf = bdlen > 0 && (bdlen <= minmaxvars || (bdlen as f64) < maxvarsfac * nvars as f64);

                if useconf {
                    // Even if we had reset the global bounds at start of probing,
                    // the constraint might be only locally valid due to local
                    // constraints/cuts.
                    let local = api::get_depth(scip) > 0;
                    success = create_conflict(scip, &bd.vars, &bd.types, &bd.bounds, local, true, true)?;
                    conflictcreated = success;
                }
            }

            // Heuristic succeeded.
            success = sol != NONE;
            if success {
                *result = ScipResultCode::FoundSol;

                // Call NLP local search heuristic unless it has failed too often.
                let (postnlp, npostnlpfails, nlpheur) = {
                    let h = hd(scip, heur);
                    (h.postnlp, h.npostnlpfails, h.nlpheur)
                };
                if postnlp && npostnlpfails < MAXPOSTNLPFAILS && !(nfixedconts == 0 && validsolved) && nlpheur != NONE {
                    // SCIPapplyHeurSubNlp gives up without an NLP solver
                    // (SCIP_DIDNOTRUN).
                    hdm(scip, heur).npostnlpfails += 1;
                }

                // Free solution.
                api::free_sol(scip, sol)?;
                sol = NONE;
            }
            let _ = sol;
        }

        // Heuristic failed but we have another recovering try, hence we forbid
        // the current cover in the covering problem.
        if !success && ncovers <= maxrecovers {
            // Compute minimal number of unfixed covering variables (in the
            // cover) which have to change their value.
            let recoverdiv = hd(scip, heur).recoverdiv;
            let mut diversification = scip.set.feas_ceil(recoverdiv * nunfixeds as f64) as i32;
            diversification = if diversification >= 1 { diversification } else { 1 };

            // Forbid unsuccessful cover globally in covering problem.
            let (forbidden, infeas) = forbid_cover(coveringscip, coveringvars, &cover, diversification)?;

            if infeas || !forbidden {
                return Ok(());
            }
            success = false;
        }

        // Try to re-use the same cover at most once.
        reusecover = hd(scip, heur).reusecover && !reusecover && conflictcreated;
    }

    Ok(())
}

/*
 * Callback methods of primal heuristic
 */

/// `heurCopyUndercover`: copy method for primal heuristic plugins.
fn heur_copy_undercover(scip: &mut Scip, _sourcescip: &Scip, _heur: HeurId) -> ScipResult {
    include_heur_undercover(scip)
}

/// `heurFreeUndercover`: destructor of primal heuristic to free user data.
fn heur_free_undercover(scip: &mut Scip, heur: HeurId) -> ScipResult {
    scip.heurs[heur].heurdata = None;
    Ok(())
}

/// `heurInitUndercover`: initialization method of primal heuristic.
fn heur_init_undercover(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Create random number generator.
    let seed = scip.set.initialize_random_seed(DEFAULT_RANDSEED);
    hdm(scip, heur).randnumgen = Some(crate::misc::RandNumGen::new(seed));
    Ok(())
}

/// `heurExitUndercover`: deinitialization method of primal heuristic.
fn heur_exit_undercover(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free random number generator.
    hdm(scip, heur).randnumgen = None;
    Ok(())
}

/// `heurInitsolUndercover`: solving process initialization method of primal
/// heuristic.
fn heur_initsol_undercover(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Initialize counters to zero.
    {
        let h = hdm(scip, heur);
        h.nusednodes = 0;
        h.npostnlpfails = 0;
        h.nnlpfails = 0;
    }

    // If the heuristic is called at the root node, we may want to be called
    // directly after the initial root LP solve.
    if hd(scip, heur).beforecuts && scip.heurs[heur].freqofs == 0 {
        scip.heurs[heur].timingmask = HEURTIMING_DURINGLPLOOP;
    }

    // Find nonlinear constraint handlers.
    let mut nlconshdlrs = Vec::with_capacity(4);
    let (coverand, coverbd, coverind, covernl) = {
        let h = hd(scip, heur);
        (h.coverand, h.coverbd, h.coverind, h.covernl)
    };
    for (cover, name) in [(coverand, "and"), (coverbd, "bounddisjunction"), (coverind, "indicator"), (covernl, "nonlinear")] {
        if cover {
            let h = crate::set::set_find_conshdlr(scip, name);
            if h != NONE {
                nlconshdlrs.push(h);
            }
        }
    }

    // Find NLP local search heuristic.
    let nlpheur = crate::set::set_find_heur(scip, "subnlp");

    let h = hdm(scip, heur);
    h.nlconshdlrs = nlconshdlrs;
    h.nlpheur = nlpheur;

    Ok(())
}

/// `heurExitsolUndercover`: solving process deinitialization method of
/// primal heuristic.
fn heur_exitsol_undercover(scip: &mut Scip, heur: HeurId) -> ScipResult {
    // Free array of nonlinear constraint handlers.
    hdm(scip, heur).nlconshdlrs = Vec::new();

    // Reset timing, if it was changed temporary (at the root node).
    scip.heurs[heur].timingmask = HEUR_TIMING;

    Ok(())
}

/// `heurExecUndercover`: execution method of primal heuristic.
fn heur_exec_undercover(scip: &mut Scip, heur: HeurId, heurtiming: HeurTiming, nodeinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Do not call heuristic of node was already detected to be infeasible.
    if nodeinfeasible {
        return Ok(());
    }

    // Only call heuristic once at the root.
    let ncalls = scip.heurs[heur].ncalls;
    if api::get_depth(scip) == 0 && ncalls > 0 {
        return Ok(());
    }

    // If we want to use NLP fixing values exclusively and no NLP solver is
    // available, we cannot run.
    if hd(scip, heur).fixingalts == "n" && NNLPIS == 0 {
        return Ok(());
    }

    // Calculate stallnode limit.
    let (nodesquot, nodesofs, nusednodes, maxnodes, minnodes) = {
        let h = hd(scip, heur);
        (h.nodesquot, h.nodesofs, h.nusednodes, h.maxnodes, h.minnodes)
    };
    let mut nstallnodes = (nodesquot * api::get_n_nodes(scip) as f64) as i64;

    // Reward heuristic if it succeeded often.
    let nbestsolsfound = scip.heurs[heur].nbestsolsfound;
    nstallnodes = (nstallnodes as f64 * 3.0 * (nbestsolsfound as f64 + 1.0) / (ncalls as f64 + 1.0)) as i64;
    // Account for the setup costs of the sub-CIP.
    nstallnodes -= SUBMIPSETUPCOSTS * ncalls;
    nstallnodes += nodesofs;

    // Determine the node limit for the current process.
    nstallnodes -= nusednodes;
    nstallnodes = if nstallnodes <= maxnodes { nstallnodes } else { maxnodes };
    nstallnodes = if nstallnodes >= 1 { nstallnodes } else { 1 };

    // Only call heuristics if we have enough nodes left to call sub-CIP
    // solving.
    if nstallnodes < minnodes {
        return Ok(());
    }

    // Only call heuristics if we have enough time left.
    let mut timelimit = paramset_get_real(scip, "limits/time")?;
    if !scip.set.is_infinity(timelimit) {
        timelimit -= api::get_solving_time(scip);
    }
    if timelimit <= 2.0 * MINTIMELEFT {
        return Ok(());
    }

    // Only call heuristics if we have enough memory left (SCIPgetMemUsed() and
    // SCIPgetMemExternEstim() count as zero).
    let memorylimit = paramset_get_real(scip, "limits/memory")?;
    let avoidmemout = paramset_get_bool(scip, "misc/avoidmemout")?;

    if avoidmemout && memorylimit <= 0.0 {
        return Ok(());
    }

    // Only call heuristic if nonlinear constraints are present.
    let mut run = false;
    for &h in hd(scip, heur).nlconshdlrs.iter().rev() {
        if run {
            break;
        }
        run = scip.conshdlrs[h].nactiveconss > 0;
    }

    // Go through all nlrows and check for general nonlinearities.
    if !run && crate::nlp::is_nlp_constructed(scip) {
        // Check for a nonlinear nlrow; start from the end since we expect the
        // linear nlrows at the end.
        let nlrows = scip.nlp.as_ref().unwrap().nlrows.clone();
        for &nlrow in nlrows.iter().rev() {
            if run {
                break;
            }
            run = scip.nlrows[nlrow].expr != NONE;
        }
    }

    if !run {
        return Ok(());
    }

    // Only call heuristics if solving has not stopped yet.
    if api::is_stopped(scip) {
        return Ok(());
    }

    // Reset timing, if it was changed temporary (at the root node).
    if heurtiming != HEUR_TIMING {
        scip.heurs[heur].timingmask = HEUR_TIMING;
    }

    // Call heuristic.
    *result = ScipResultCode::DidNotFind;

    apply_undercover(scip, heur, result, timelimit, memorylimit, nstallnodes)
}

/*
 * primal heuristic specific interface methods
 */

/// `SCIPincludeHeurUndercover`: creates the undercover primal heuristic and
/// includes it in SCIP.
pub fn include_heur_undercover(scip: &mut Scip) -> ScipResult {
    // Create undercover primal heuristic data; always use local bounds.
    let heurdata = HeurData {
        globalbounds: false,
        nlpheur: NONE,
        ..Default::default()
    };

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
        heur_exec_undercover,
        Some(Box::new(heurdata) as Box<dyn Any>),
    )?;

    // Set non-NULL pointers to callback methods.
    let h = &mut scip.heurs[heur];
    h.heurcopy = Some(heur_copy_undercover);
    h.heurfree = Some(heur_free_undercover);
    h.heurinit = Some(heur_init_undercover);
    h.heurexit = Some(heur_exit_undercover);
    h.heurinitsol = Some(heur_initsol_undercover);
    h.heurexitsol = Some(heur_exitsol_undercover);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, heurs, heur, heurdata, HeurData, $f)
        };
    }
    let (rmin, rmax) = (crate::def::REAL_MIN, crate::def::REAL_MAX);

    // Add string parameters.
    paramset_add_string(
        scip,
        "heuristics/undercover/fixingalts",
        "prioritized sequence of fixing values used ('l'p relaxation, 'n'lp relaxation, 'i'ncumbent solution)",
        t!(String, fixingalts),
        false,
        DEFAULT_FIXINGALTS,
        None,
    )?;

    // Add longint parameters.
    paramset_add_longint(
        scip,
        "heuristics/undercover/maxnodes",
        "maximum number of nodes to regard in the subproblem",
        t!(i64, maxnodes),
        true,
        DEFAULT_MAXNODES,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/undercover/minnodes",
        "minimum number of nodes required to start the subproblem",
        t!(i64, minnodes),
        true,
        DEFAULT_MINNODES,
        0,
        i64::MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "heuristics/undercover/nodesofs",
        "number of nodes added to the contingent of the total nodes",
        t!(i64, nodesofs),
        false,
        DEFAULT_NODESOFS,
        0,
        i64::MAX,
        None,
    )?;

    // Add real parameters.
    paramset_add_real(
        scip,
        "heuristics/undercover/conflictweight",
        "weight for conflict score in fixing order",
        t!(f64, conflictweight),
        true,
        DEFAULT_CONFLICTWEIGHT,
        rmin,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/undercover/cutoffweight",
        "weight for cutoff score in fixing order",
        t!(f64, cutoffweight),
        true,
        DEFAULT_CUTOFFWEIGHT,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/undercover/inferenceweight",
        "weight for inference score in fixing order",
        t!(f64, inferenceweight),
        true,
        DEFAULT_INFERENCEWEIGHT,
        rmin,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/undercover/maxcoversizevars",
        "maximum coversize (as fraction of total number of variables)",
        t!(f64, maxcoversizevars),
        true,
        DEFAULT_MAXCOVERSIZEVARS,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/undercover/maxcoversizeconss",
        "maximum coversize (as ratio to the percentage of non-affected constraints)",
        t!(f64, maxcoversizeconss),
        true,
        DEFAULT_MAXCOVERSIZECONSS,
        0.0,
        rmax,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/undercover/mincoveredrel",
        "minimum percentage of nonlinear constraints in the original problem",
        t!(f64, mincoveredrel),
        true,
        DEFAULT_MINCOVEREDREL,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/undercover/minimprove",
        "factor by which the heuristic should at least improve the incumbent",
        t!(f64, minimprove),
        true,
        DEFAULT_MINIMPROVE,
        -1.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/undercover/nodesquot",
        "contingent of sub problem nodes in relation to the number of nodes of the original problem",
        t!(f64, nodesquot),
        false,
        DEFAULT_NODESQUOT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "heuristics/undercover/recoverdiv",
        "fraction of covering variables in the last cover which need to change their value when recovering",
        t!(f64, recoverdiv),
        true,
        DEFAULT_RECOVERDIV,
        0.0,
        1.0,
        None,
    )?;

    // Add int parameters.
    paramset_add_int(
        scip,
        "heuristics/undercover/mincoveredabs",
        "minimum number of nonlinear constraints in the original problem",
        t!(i32, mincoveredabs),
        true,
        DEFAULT_MINCOVEREDABS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/undercover/maxbacktracks",
        "maximum number of backtracks in fix-and-propagate",
        t!(i32, maxbacktracks),
        true,
        DEFAULT_MAXBACKTRACKS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/undercover/maxrecovers",
        "maximum number of recoverings",
        t!(i32, maxrecovers),
        true,
        DEFAULT_MAXRECOVERS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "heuristics/undercover/maxreorders",
        "maximum number of reorderings of the fixing order",
        t!(i32, maxreorders),
        true,
        DEFAULT_MAXREORDERS,
        0,
        i32::MAX,
        None,
    )?;

    // Add char parameters.
    paramset_add_char(
        scip,
        "heuristics/undercover/coveringobj",
        "objective function of the covering problem (influenced nonlinear 'c'onstraints/'t'erms, 'd'omain size, 'l'ocks, 'm'in of up/down locks, 'u'nit penalties)",
        t!(u8, coveringobj),
        true,
        DEFAULT_COVERINGOBJ,
        Some(COVERINGOBJS),
        None,
    )?;
    paramset_add_char(
        scip,
        "heuristics/undercover/fixingorder",
        "order in which variables should be fixed (increasing 'C'onflict score, decreasing 'c'onflict score, increasing 'V'ariable index, decreasing 'v'ariable index",
        t!(u8, fixingorder),
        true,
        DEFAULT_FIXINGORDER,
        Some(FIXINGORDERS),
        None,
    )?;

    // Add bool parameters.
    paramset_add_bool(
        scip,
        "heuristics/undercover/beforecuts",
        "should the heuristic be called at root node before cut separation?",
        t!(bool, beforecuts),
        true,
        DEFAULT_BEFORECUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/fixintfirst",
        "should integer variables in the cover be fixed first?",
        t!(bool, fixintfirst),
        true,
        DEFAULT_FIXINTFIRST,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/locksrounding",
        "shall LP values for integer vars be rounded according to locks?",
        t!(bool, locksrounding),
        true,
        DEFAULT_LOCKSROUNDING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/onlyconvexify",
        "should we only fix variables in order to obtain a convex problem?",
        t!(bool, onlyconvexify),
        false,
        DEFAULT_ONLYCONVEXIFY,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/postnlp",
        "should the NLP heuristic be called to polish a feasible solution?",
        t!(bool, postnlp),
        false,
        DEFAULT_POSTNLP,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/coverand",
        "should and constraints be covered (or just copied)?",
        t!(bool, coverand),
        true,
        DEFAULT_COVERAND,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/coverbd",
        "should bounddisjunction constraints be covered (or just copied)?",
        t!(bool, coverbd),
        true,
        DEFAULT_COVERBD,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/coverind",
        "should indicator constraints be covered (or just copied)?",
        t!(bool, coverind),
        true,
        DEFAULT_COVERIND,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/covernl",
        "should nonlinear constraints be covered (or just copied)?",
        t!(bool, covernl),
        true,
        DEFAULT_COVERNL,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/copycuts",
        "should all active cuts from cutpool be copied to constraints in subproblem?",
        t!(bool, copycuts),
        true,
        DEFAULT_COPYCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/undercover/reusecover",
        "shall the cover be reused if a conflict was added after an infeasible subproblem?",
        t!(bool, reusecover),
        true,
        DEFAULT_REUSECOVER,
        None,
    )?;

    Ok(())
}
