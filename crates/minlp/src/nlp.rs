//! Port of `nlp.c` (and the parts of `scip_nlp.c` that wrap it): the NLP
//! relaxation and nonlinear rows.
//!
//! No NLP solver (NLPI) is available, as in the reference build (IPOPT is
//! off), so the NLPI problem never exists: every branch that updates it is
//! unreachable and left out. What remains is the bookkeeping the solving
//! process can observe: the NLP's variables and nonlinear rows, the event
//! handler that catches variable events, and the solution status.

use std::collections::HashMap;

use crate::{
    def::INVALID,
    event::{self, Event},
    expr::{self, ExprCurv, ExprId},
    interval::{self, Interval},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{EventhdlrId, VarId},
    var::NONE,
};

const EVENTHDLR_NAME: &str = "nlpEventHdlr";
const EVENTHDLR_DESC: &str = "handles all events necessary for maintaining NLP data";

/// `SCIP_NLPSOLSTAT`: NLP solution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NlpSolStat {
    /// Solved to global optimality.
    GlobOpt = 0,
    /// Solved to local optimality.
    LocOpt = 1,
    /// Feasible solution found.
    Feasible = 2,
    /// Solution found is local infeasible.
    LocInfeasible = 3,
    /// Problem is proven infeasible.
    GlobInfeasible = 4,
    /// Problem is unbounded.
    Unbounded = 5,
    /// Unknown solution status (e.g., problem not solved yet).
    Unknown = 6,
}

/// `SCIP_NLPTERMSTAT`: NLP solver termination status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NlpTermStat {
    /// Terminated successfully.
    Okay = 0,
    /// Time limit exceeded.
    TimeLimit = 1,
    /// Iteration limit exceeded.
    IterLimit = 2,
    /// Lower objective limit reached.
    LObjLimit = 3,
    /// Interrupted.
    Interrupt = 4,
    /// Stopped on numerical error.
    NumericError = 5,
    /// Evaluation error.
    EvalError = 6,
    /// Memory exceeded.
    OutOfMemory = 7,
    /// Problems with license of NLP solver.
    LicenseError = 8,
    /// Other error (= this should never happen).
    Other = 9,
}

/// `SCIP_NLROW`: a nonlinear row.
#[derive(Clone, Debug)]
pub struct NlRow {
    /* sides */
    /// Left hand side.
    pub lhs: f64,
    /// Right hand side.
    pub rhs: f64,

    /* constant part */
    /// Constant value.
    pub constant: f64,

    /* linear part */
    /// Linear variables (C's `nlinvars` is their number).
    pub linvars: Vec<VarId>,
    /// Coefficients of linear variables.
    pub lincoefs: Vec<f64>,
    /// Are the linear coefficients sorted (by variable indices?).
    pub linvarssorted: bool,

    /* nonlinear part */
    /// Expression representing nonlinear part (`NONE` for NULL).
    pub expr: ExprId,

    /* miscellaneous */
    /// Name.
    pub name: String,
    /// Number of times, this row is referenced.
    pub nuses: i32,
    /// Row activity value in NLP, or SCIP_INVALID if not yet calculated.
    pub activity: f64,
    /// NLP number for which activity value is valid.
    pub validactivitynlp: i64,
    /// Row activity value in pseudo solution, or SCIP_INVALID if not yet
    /// calculated.
    pub pseudoactivity: f64,
    /// Domain change number for which pseudo activity value is valid.
    pub validpsactivitydomchg: i64,
    /// Minimal activity value w.r.t. the variables' bounds, or SCIP_INVALID.
    pub minactivity: f64,
    /// Maximal activity value w.r.t. the variables' bounds, or SCIP_INVALID.
    pub maxactivity: f64,
    /// Domain change number for which activity bound values are valid.
    pub validactivitybdsdomchg: i64,
    /// Index of this row in NLP, or -1 if not added.
    pub nlpindex: i32,
    /// Index of this row in NLPI problem, or -1 if not in there.
    pub nlpiindex: i32,
    /// Dual value associated with row in last NLP solve.
    pub dualsol: f64,
    /// Curvature of the nonlinear row.
    pub curvature: ExprCurv,
}

/// `SCIP_NLP`: current NLP data.
#[derive(Clone, Debug)]
pub struct Nlp {
    /* status */
    /// Number of variable additions not flushed to NLPI problem yet.
    pub nunflushedvaradd: i32,
    /// Number of variable deletions not flushed to NLPI problem yet.
    pub nunflushedvardel: i32,
    /// Number of nonlinear row additions not flushed to NLPI problem yet.
    pub nunflushednlrowadd: i32,
    /// Number of nonlinear row deletions not flushed to NLPI problem yet.
    pub nunflushednlrowdel: i32,
    /// Are we currently in diving mode?
    pub indiving: bool,

    /* variables in problem */
    /// Variables.
    pub vars: Vec<VarId>,
    /// Variable hash: map SCIP_VAR* to index of variable in NLP.
    pub varhash: HashMap<VarId, i32>,
    /// Index of variables in NLPI problem, or -1 if variable has not been
    /// added to NLPI problem yet.
    pub varmap_nlp2nlpi: Vec<i32>,

    /* nonlinear rows in problem */
    /// Nonlinear rows.
    pub nlrows: Vec<usize>,
    /// Number of linear rows in NLP.
    pub nnlrowlinear: i32,
    /// Number of convex one-sided inequalities in NLP.
    pub nnlrowconvexineq: i32,
    /// Number of nonconvex one-sided inequalities in NLP.
    pub nnlrownonconvexineq: i32,
    /// Number of nonlinear equalities and two-sided inequalities in NLP.
    pub nnlrownonlineareq: i32,

    /* objective function */
    /// Is the objective in the NLPI up to date?
    pub objflushed: bool,

    /* initial guess */
    /// Is an initial guess available?
    pub haveinitguess: bool,

    /* solution of NLP */
    /// Objective function value of primal solution.
    pub primalsolobjval: f64,
    /// Status of NLP solution (feasible, optimal, unknown...).
    pub solstat: NlpSolStat,
    /// Termination status of NLP (normal, some limit reached, ...).
    pub termstat: NlpTermStat,
    /// Dual values associated with variable lower bounds.
    pub varlbdualvals: Vec<f64>,
    /// Dual values associated with variable upper bounds.
    pub varubdualvals: Vec<f64>,

    /* event handling */
    /// Event handler for bound change events.
    pub eventhdlr: EventhdlrId,
    /// Position of event handler in event handler filter.
    pub globalfilterpos: i32,

    /* miscellaneous */
    /// Problem name.
    pub name: String,
}

/// The event types the NLP catches on its variables.
const VAREVENTTYPE: event::EventType = event::EVENTTYPE_VARFIXED | event::EVENTTYPE_BOUNDCHANGED | event::EVENTTYPE_OBJCHANGED;

/// The NLP's event data (`(SCIP_EVENTDATA*)nlp`); there is only one NLP.
const NLPEVENTDATA: usize = 0;

/* private NLP nonlinear row methods */

/// Invalidates the cached activities of a row.
fn nlrow_invalidate_activities(r: &mut NlRow) {
    r.activity = INVALID;
    r.validactivitynlp = -1;
    r.pseudoactivity = INVALID;
    r.validpsactivitydomchg = -1;
    r.minactivity = INVALID;
    r.maxactivity = INVALID;
    r.validactivitybdsdomchg = -1;
}

/// `nlrowLinearCoefChanged`: announces, that the given linear coefficient
/// in the constraint matrix changed.
fn nlrow_linear_coef_changed(scip: &mut Scip, nlrow: usize) -> ScipResult {
    nlrow_invalidate_activities(&mut scip.nlrows[nlrow]);

    if scip.nlrows[nlrow].nlpindex >= 0 {
        // Notify NLP that row has changed.
        nlp_row_changed(scip);

        // (The row is never in an NLPI problem.)
    }

    Ok(())
}

/// `nlrowExprChanged`: announces, that an expression changed.
fn nlrow_expr_changed(scip: &mut Scip, nlrow: usize) -> ScipResult {
    nlrow_invalidate_activities(&mut scip.nlrows[nlrow]);

    if scip.nlrows[nlrow].nlpindex >= 0 {
        // Notify NLP that row has changed.
        nlp_row_changed(scip);
    }

    Ok(())
}

/// `nlrowSideChanged`: notifies nonlinear row, that its sides were changed.
fn nlrow_side_changed(scip: &mut Scip, nlrow: usize) -> ScipResult {
    if scip.nlrows[nlrow].nlpindex >= 0 {
        // Notify NLP that row has changed.
        nlp_row_changed(scip);
    }

    Ok(())
}

/// `nlrowConstantChanged`: notifies nonlinear row, that its constant was
/// changed.
fn nlrow_constant_changed(scip: &mut Scip, nlrow: usize) -> ScipResult {
    nlrow_invalidate_activities(&mut scip.nlrows[nlrow]);

    if scip.nlrows[nlrow].nlpindex >= 0 {
        // Notify NLP that row has changed.
        nlp_row_changed(scip);
    }

    Ok(())
}

/// `nlrowAddToStat`: increments or decrements count of NLROW in NLP
/// statistics.
///
/// Updates count on linear/convex/nonconvex NLP rows w.r.t. given NLROW.
fn nlrow_add_to_stat(scip: &mut Scip, nlrow: usize, incr: i32) {
    let r = &scip.nlrows[nlrow];
    let isinflhs = scip.set.is_infinity(-r.lhs);
    let isinfrhs = scip.set.is_infinity(r.rhs);
    let (hasexpr, curv) = (r.expr != NONE, r.curvature.bits());
    let nlp = scip.nlp.as_mut().unwrap();

    if !hasexpr {
        nlp.nnlrowlinear += incr;
        return;
    }

    if !isinflhs && !isinfrhs {
        nlp.nnlrownonlineareq += incr;
        return;
    }

    if (isinflhs && curv & ExprCurv::Convex.bits() != 0) ||  // g(x) <= rhs with g(x) convex
       (isinfrhs && curv & ExprCurv::Concave.bits() != 0)
    {
        // g(x) >= lhs with g(x) concave
        nlp.nnlrowconvexineq += incr;
        return;
    }

    nlp.nnlrownonconvexineq += incr;
}

/// `nlrowSortLinear`: sorts linear part of row entries such that lower
/// variable indices precede higher ones.
fn nlrow_sort_linear(scip: &mut Scip, nlrow: usize) {
    // Check, if row is already sorted in the LP part, or if the sorting should
    // be delayed.
    if scip.nlrows[nlrow].linvarssorted {
        return;
    }

    // Sort linear coefficients.
    let mut linvars = std::mem::take(&mut scip.nlrows[nlrow].linvars);
    let mut lincoefs = std::mem::take(&mut scip.nlrows[nlrow].lincoefs);
    crate::sort::sort(&mut linvars, (&mut lincoefs[..],), |&a, &b| crate::var::var_comp(scip, a, b));
    let r = &mut scip.nlrows[nlrow];
    r.linvars = linvars;
    r.lincoefs = lincoefs;
    r.linvarssorted = true;
}

/// `nlrowSearchLinearCoef`: searches linear variable in nonlinear row,
/// returns position in linvars vector or -1 if not found.
fn nlrow_search_linear_coef(scip: &mut Scip, nlrow: usize, var: VarId) -> i32 {
    if scip.nlrows[nlrow].linvars.is_empty() {
        return -1;
    }

    nlrow_sort_linear(scip, nlrow);
    match crate::sort::sortedvec_find(&scip.nlrows[nlrow].linvars, |&a, &b| crate::var::var_comp(scip, a, b), var) {
        Ok(pos) => pos as i32,
        Err(_) => -1,
    }
}

/// `nlrowMoveLinearCoef`: moves a coefficient in a nonlinear row to a
/// different place, and updates all corresponding data structures.
fn nlrow_move_linear_coef(r: &mut NlRow, oldpos: usize, newpos: usize) {
    if oldpos == newpos {
        return;
    }

    r.linvars[newpos] = r.linvars[oldpos];
    r.lincoefs[newpos] = r.lincoefs[oldpos];

    // Update sorted flags.
    r.linvarssorted = false;
}

/// `nlrowAddLinearCoef`: adds a previously non existing linear coefficient
/// to a nonlinear row.
fn nlrow_add_linear_coef(scip: &mut Scip, nlrow: usize, var: VarId, coef: f64) -> ScipResult {
    // Assert that only active variables are added once the row is in the NLP.

    let r = &mut scip.nlrows[nlrow];
    let pos = r.linvars.len();

    // Insert the variable.
    r.linvars.push(var);
    r.lincoefs.push(coef);

    nlrow_linear_coef_changed(scip, nlrow)?;

    // Update sorted flag.
    let r = &scip.nlrows[nlrow];
    if pos > 0 && crate::var::var_compare(scip, r.linvars[pos - 1], r.linvars[pos]) > 0 {
        scip.nlrows[nlrow].linvarssorted = false;
    }

    Ok(())
}

/// `nlrowDelLinearCoefPos`: deletes coefficient at given position from row.
fn nlrow_del_linear_coef_pos(scip: &mut Scip, nlrow: usize, pos: usize) -> ScipResult {
    let r = &mut scip.nlrows[nlrow];

    // Move last coefficient to position of empty slot (should set sorted flag
    // to FALSE, if not last variable was deleted).
    let last = r.linvars.len() - 1;
    nlrow_move_linear_coef(r, last, pos);
    r.linvars.pop();
    r.lincoefs.pop();

    nlrow_linear_coef_changed(scip, nlrow)
}

/// `nlrowRemoveFixedLinearCoefPos`: makes sure that there is no fixed
/// variable at position pos of the linear part of a nonlinear row.
///
/// A fixed variable is replaced with the corresponding constant or
/// disaggregated term.
fn nlrow_remove_fixed_linear_coef_pos(scip: &mut Scip, nlrow: usize, pos: usize) -> ScipResult {
    let var = scip.nlrows[nlrow].linvars[pos];

    if crate::var::var_is_active(scip, var) {
        return Ok(());
    }

    let oldconstant = scip.nlrows[nlrow].constant;

    // Replace fixed, aggregated, or negated variable.
    {
        let (mut v, mut c, mut k) = {
            let r = &scip.nlrows[nlrow];
            (r.linvars[pos], r.lincoefs[pos], r.constant)
        };
        crate::var::var_get_probvar_sum(scip, &mut v, &mut c, &mut k)?;
        let r = &mut scip.nlrows[nlrow];
        r.linvars[pos] = v;
        r.lincoefs[pos] = c;
        r.constant = k;
    }

    // If var had been fixed, entry should be removed from row.
    if scip.nlrows[nlrow].lincoefs[pos] == 0.0 {
        let r = &mut scip.nlrows[nlrow];
        let last = r.linvars.len() - 1;
        nlrow_move_linear_coef(r, last, pos);
        r.linvars.pop();
        r.lincoefs.pop();

        if pos < scip.nlrows[nlrow].linvars.len() {
            nlrow_remove_fixed_linear_coef_pos(scip, nlrow, pos)?;
        }

        return Ok(());
    }
    scip.nlrows[nlrow].linvarssorted = false;

    // Notify nlrow that coefficient of var is now 0.0 in row.
    nlrow_linear_coef_changed(scip, nlrow)?;

    // Notify nlrow that constant of row has changed.
    if oldconstant != scip.nlrows[nlrow].constant {
        nlrow_constant_changed(scip, nlrow)?;
    }

    if crate::var::var_is_active(scip, scip.nlrows[nlrow].linvars[pos]) {
        // If var was aggregated or negated, notify nlrow about new coefficient.
        nlrow_linear_coef_changed(scip, nlrow)?;
    } else {
        // If not removed or active, the new variable should be multi-aggregated.
        let var = scip.nlrows[nlrow].linvars[pos];
        let coef = scip.nlrows[nlrow].lincoefs[pos];

        // Remove the variable from the row.
        nlrow_del_linear_coef_pos(scip, nlrow, pos)?;

        // Add multi-aggregated term to row.
        let ma = scip.vars[var].data.multaggr.clone();
        if ma.constant != 0.0 {
            scip.nlrows[nlrow].constant += coef * ma.constant;
            nlrow_constant_changed(scip, nlrow)?;
        }

        for i in 0..ma.nvars as usize {
            if scip.set.is_zero(coef * ma.scalars[i]) {
                continue;
            }
            nlrow_add_linear_coef(scip, nlrow, ma.vars[i], coef * ma.scalars[i])?;
            if !crate::var::var_is_active(scip, ma.vars[i]) {
                // If newly added variable is fixed, replace it now.
                let last = scip.nlrows[nlrow].linvars.len() - 1;
                nlrow_remove_fixed_linear_coef_pos(scip, nlrow, last)?;
            }
        }

        // Due to nlrowDelLinearCoefPos, an inactive variable may have moved to
        // position pos; if that is the case, call ourself recursively.
        if pos < scip.nlrows[nlrow].linvars.len() && !crate::var::var_is_active(scip, scip.nlrows[nlrow].linvars[pos]) {
            nlrow_remove_fixed_linear_coef_pos(scip, nlrow, pos)?;
        }
    }

    Ok(())
}

/// `nlrowRemoveFixedLinearCoefs`: removes fixed variables from the linear
/// part of a nonlinear row.
fn nlrow_remove_fixed_linear_coefs(scip: &mut Scip, nlrow: usize) -> ScipResult {
    let oldlen = scip.nlrows[nlrow].linvars.len();
    let mut i = 0;
    while i < oldlen.min(scip.nlrows[nlrow].linvars.len()) {
        nlrow_remove_fixed_linear_coef_pos(scip, nlrow, i)?;
        i += 1;
    }

    Ok(())
}

/// `nlrowSimplifyExpr`: removes fixed variables from expression of a
/// nonlinear row.
fn nlrow_simplify_expr(scip: &mut Scip, nlrow: usize) -> ScipResult {
    if scip.nlrows[nlrow].expr == NONE {
        return Ok(());
    }

    let (mut simplified, changed, _infeasible) = expr::expr_simplify(scip, scip.nlrows[nlrow].expr, None, 0)?;

    if !changed {
        expr::expr_release(scip, &mut simplified)?;
        return Ok(());
    }

    let mut old = scip.nlrows[nlrow].expr;
    expr::expr_release(scip, &mut old)?;
    scip.nlrows[nlrow].expr = simplified;

    if expr::expr_is_value(scip, simplified) {
        // If expression tree is constant, remove it.
        let c = scip.nlrows[nlrow].constant + crate::expr_value::get_value_expr_value(scip, simplified);
        nlrow_chg_constant(scip, nlrow, c)?;

        // Removing the expression changes statistics on rows in stat, if row is
        // already in NLP; first remove current nlrow from stat, then add again
        // after releasing expression.
        let inlp = scip.nlrows[nlrow].nlpindex >= 0;
        if inlp {
            nlrow_add_to_stat(scip, nlrow, -1);
        }

        let mut e = scip.nlrows[nlrow].expr;
        expr::expr_release(scip, &mut e)?;
        scip.nlrows[nlrow].expr = NONE;
        scip.nlrows[nlrow].curvature = ExprCurv::Linear;

        if inlp {
            nlrow_add_to_stat(scip, nlrow, 1);
        }
    }

    nlrow_expr_changed(scip, nlrow)
}

/// `nlrowRemoveFixedVar`: removes fixed variable from nonlinear row.
fn nlrow_remove_fixed_var(scip: &mut Scip, nlrow: usize, var: VarId) -> ScipResult {
    // Search for variable in linear part and remove if existing.
    let pos = nlrow_search_linear_coef(scip, nlrow, var);
    if pos >= 0 {
        nlrow_remove_fixed_linear_coef_pos(scip, nlrow, pos as usize)?;
    }

    // Search for variable in nonlinear part and remove all fixed variables in
    // expression if existing.
    // TODO only call simplify if var appears in expr, but currently we don't
    // store the vars in a separate array.
    if scip.nlrows[nlrow].expr != NONE {
        nlrow_simplify_expr(scip, nlrow)?;
    }

    Ok(())
}

/* public NLP nonlinear row methods */

/// `SCIPnlrowCreate` (`SCIPcreateNlRow`): create a new nonlinear row; the
/// new row is already captured.
#[allow(clippy::too_many_arguments)]
pub fn create_nl_row(scip: &mut Scip, name: &str, constant: f64, linvars: &[VarId], lincoefs: &[f64], expr: ExprId, lhs: f64, rhs: f64, curvature: ExprCurv) -> ScipResult<usize> {
    // Nonlinear part.
    let rowexpr = if expr != NONE {
        // TODO preserve common subexpressions, or at least use only one varexpr per
        // var.
        expr::expr_copy(scip, expr, None, 0, None, 0)?
    } else {
        NONE
    };

    let nlrow = scip.nlrows.push(NlRow {
        lhs: if lhs <= rhs { lhs } else { rhs },
        rhs: if lhs >= rhs { lhs } else { rhs },
        constant,
        linvars: linvars.to_vec(),
        lincoefs: lincoefs[..linvars.len()].to_vec(),
        linvarssorted: linvars.is_empty(),
        expr: rowexpr,
        name: name.to_string(),
        nuses: 0,
        activity: INVALID,
        validactivitynlp: 0,
        pseudoactivity: INVALID,
        validpsactivitydomchg: 0,
        minactivity: INVALID,
        maxactivity: INVALID,
        validactivitybdsdomchg: 0,
        nlpindex: -1,
        nlpiindex: -1,
        dualsol: 0.0,
        curvature,
    });

    // Capture the nonlinear row.
    capture_nl_row(scip, nlrow);

    Ok(nlrow)
}

/// `SCIPnlrowCapture`: increases usage counter of nonlinear row.
pub fn capture_nl_row(scip: &mut Scip, nlrow: usize) {
    scip.nlrows[nlrow].nuses += 1;
}

/// `SCIPnlrowRelease` (`SCIPreleaseNlRow`): decreases usage counter of
/// nonlinear row.
pub fn release_nl_row(scip: &mut Scip, nlrow: &mut usize) -> ScipResult {
    scip.nlrows[*nlrow].nuses -= 1;
    if scip.nlrows[*nlrow].nuses > 0 {
        *nlrow = NONE;
        return Ok(());
    }

    // Free row; nonlinear part.
    let mut e = scip.nlrows[*nlrow].expr;
    if e != NONE {
        expr::expr_release(scip, &mut e)?;
    }

    scip.nlrows.remove(*nlrow);
    *nlrow = NONE;

    Ok(())
}

/// `SCIPnlrowAddLinearCoef` (`SCIPaddLinearCoefToNlRow`): adds a previously
/// non existing linear coefficient to a nonlinear row.
pub fn add_linear_coef_to_nl_row(scip: &mut Scip, nlrow: usize, mut var: VarId, mut val: f64) -> ScipResult {
    // If row is in NLP already, make sure that only active variables are
    // added.
    if scip.nlrows[nlrow].nlpindex >= 0 {
        // Get corresponding active or multi-aggregated variable.
        let mut constant = 0.0;
        crate::var::var_get_probvar_sum(scip, &mut var, &mut val, &mut constant)?;

        // Add constant.
        let c = scip.nlrows[nlrow].constant + constant;
        nlrow_chg_constant(scip, nlrow, c)?;

        if val == 0.0 {
            // Var has been fixed.
            return Ok(());
        }

        if !crate::var::var_is_active(scip, var) {
            // Var should be multi-aggregated, so call this function recursively.
            let ma = scip.vars[var].data.multaggr.clone();
            for i in 0..ma.nvars as usize {
                add_linear_coef_to_nl_row(scip, nlrow, ma.vars[i], ma.scalars[i] * val)?;
            }
            return Ok(());
        }

        // Var is active, so can go on like normal.
    }

    nlrow_add_linear_coef(scip, nlrow, var, val)
}

/// `SCIPnlrowChgExpr` (`SCIPsetNlRowExpr`): replaces or deletes an
/// expression in a nonlinear row.
pub fn set_nl_row_expr(scip: &mut Scip, nlrow: usize, newexpr: ExprId) -> ScipResult {
    let inlp = scip.nlrows[nlrow].nlpindex >= 0;

    // If row in NLP, then remove it from statistics on NLP rows.
    if inlp {
        nlrow_add_to_stat(scip, nlrow, -1);
    }

    // Free previous expression tree.
    let mut old = scip.nlrows[nlrow].expr;
    if old != NONE {
        expr::expr_release(scip, &mut old)?;
        scip.nlrows[nlrow].expr = NONE;
    }

    // Adds new expression tree.
    if newexpr != NONE {
        // TODO preserve common subexpressions, or at least use only one varexpr per
        // var.
        let copy = expr::expr_copy(scip, newexpr, None, 0, None, 0)?;
        scip.nlrows[nlrow].expr = copy;

        // If row is already in NLP, ensure that expr has only active variables.
        if inlp {
            let (simplified, _changed, _infeasible) = expr::expr_simplify(scip, copy, None, 0)?;
            let mut c = copy;
            expr::expr_release(scip, &mut c)?;
            scip.nlrows[nlrow].expr = simplified;
        }
    }

    // Notify row about the change.
    nlrow_expr_changed(scip, nlrow)?;

    // If row in NLP, then add it again to statistics on NLP rows.
    if inlp {
        nlrow_add_to_stat(scip, nlrow, 1);
    }

    Ok(())
}

/// `SCIPnlrowChgConstant`: changes constant of nonlinear row.
fn nlrow_chg_constant(scip: &mut Scip, nlrow: usize, constant: f64) -> ScipResult {
    if !scip.set.is_eq(scip.nlrows[nlrow].constant, constant) {
        scip.nlrows[nlrow].constant = constant;
        nlrow_constant_changed(scip, nlrow)?;
    }

    Ok(())
}

/// `SCIPchgNlRowConstant`: changes constant of nonlinear row.
pub fn chg_nl_row_constant(scip: &mut Scip, nlrow: usize, constant: f64) -> ScipResult {
    nlrow_chg_constant(scip, nlrow, constant)
}

/// `SCIPnlrowChgLhs` (`SCIPchgNlRowLhs`): changes left hand side of
/// nonlinear row.
pub fn chg_nl_row_lhs(scip: &mut Scip, nlrow: usize, lhs: f64) -> ScipResult {
    if !scip.set.is_eq(scip.nlrows[nlrow].lhs, lhs) {
        let inlp = scip.nlrows[nlrow].nlpindex >= 0;
        if inlp {
            nlrow_add_to_stat(scip, nlrow, -1);
        }

        scip.nlrows[nlrow].lhs = lhs;
        nlrow_side_changed(scip, nlrow)?;

        if inlp {
            nlrow_add_to_stat(scip, nlrow, 1);
        }
    }

    Ok(())
}

/// `SCIPnlrowChgRhs` (`SCIPchgNlRowRhs`): changes right hand side of
/// nonlinear row.
pub fn chg_nl_row_rhs(scip: &mut Scip, nlrow: usize, rhs: f64) -> ScipResult {
    if !scip.set.is_eq(scip.nlrows[nlrow].rhs, rhs) {
        let inlp = scip.nlrows[nlrow].nlpindex >= 0;
        if inlp {
            nlrow_add_to_stat(scip, nlrow, -1);
        }

        scip.nlrows[nlrow].rhs = rhs;
        nlrow_side_changed(scip, nlrow)?;

        if inlp {
            nlrow_add_to_stat(scip, nlrow, 1);
        }
    }

    Ok(())
}

/// `SCIPnlrowSetCurvature` (`SCIPsetNlRowCurvature`): sets the curvature of
/// a nonlinear row.
pub fn set_nl_row_curvature(scip: &mut Scip, nlrow: usize, curvature: ExprCurv) {
    let inlp = scip.nlrows[nlrow].nlpindex >= 0;
    if inlp {
        nlrow_add_to_stat(scip, nlrow, -1);
    }

    scip.nlrows[nlrow].curvature = curvature;

    if inlp {
        nlrow_add_to_stat(scip, nlrow, 1);
    }
}

/// `SCIPnlrowSimplify`: removes (or substitutes) all fixed, negated,
/// aggregated, multi-aggregated variables from the linear and nonlinear part
/// of a nonlinear row and simplifies its expression.
fn nlrow_simplify(scip: &mut Scip, nlrow: usize) -> ScipResult {
    nlrow_remove_fixed_linear_coefs(scip, nlrow)?;
    nlrow_simplify_expr(scip, nlrow)
}

/// `nlrowCalcActivityBounds` with `SCIPnlrowGetActivityBounds`: minimal and
/// maximal activity of row w.r.t. the variable's bounds.
pub fn nlrow_get_activity_bounds(scip: &mut Scip, nlrow: usize) -> ScipResult<(f64, f64)> {
    if scip.nlrows[nlrow].validactivitybdsdomchg != scip.stat.domchgcount {
        let inf = scip.set.infinity();

        // Calculate activity bounds.
        let mut activity = Interval::default();
        activity.set(scip.nlrows[nlrow].constant);
        let n = scip.nlrows[nlrow].linvars.len();
        let mut i = 0;
        while i < n && !interval::is_entire(inf, activity) {
            let (var, coef) = (scip.nlrows[nlrow].linvars[i], scip.nlrows[nlrow].lincoefs[i]);
            let mut bounds = Interval::default();
            bounds.set_bounds(scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);
            let b = bounds;
            interval::mul_scalar(inf, &mut bounds, b, coef);
            let a = activity;
            interval::add(inf, &mut activity, a, bounds);
            i += 1;
        }

        let e = scip.nlrows[nlrow].expr;
        if e != NONE && !interval::is_entire(inf, activity) {
            expr::expr_eval_activity(scip, e)?;
            let a = activity;
            interval::add(inf, &mut activity, a, scip.exprs[e].activity);
        }

        let domchgcount = scip.stat.domchgcount;
        let r = &mut scip.nlrows[nlrow];
        r.minactivity = activity.inf;
        r.maxactivity = activity.sup;
        r.validactivitybdsdomchg = domchgcount;
    }

    let r = &scip.nlrows[nlrow];
    Ok((r.minactivity, r.maxactivity))
}

/* private NLP methods */

/// `nlpRowChanged`: notifies NLP that a row has been changed.
fn nlp_row_changed(scip: &mut Scip) {
    let nlp = scip.nlp.as_mut().unwrap();

    // nlrow is a row in the NLP, so changes effect feasibility; if we have a
    // feasible NLP solution and it satisfies the modified row, then it is still
    // feasible; if the NLP was globally or locally infeasible or unbounded, then
    // this may not be the case anymore.
    nlp.solstat = if nlp.solstat <= NlpSolStat::Feasible {
        NlpSolStat::LocInfeasible
    } else {
        NlpSolStat::Unknown
    };
}

/// `nlpAddNlRows`: adds a set of nonlinear rows to the NLP and captures them.
fn nlp_add_nl_rows(scip: &mut Scip, nlrows: &[usize]) -> ScipResult {
    for &nlrow in nlrows {
        // Assert that row is not in NLP (or even NLPI) yet.

        // Make sure there are only active variables in row.
        nlrow_simplify(scip, nlrow)?;

        // Add row to NLP and capture it.
        let nlp = scip.nlp.as_mut().unwrap();
        let pos = nlp.nlrows.len();
        nlp.nlrows.push(nlrow);
        scip.nlrows[nlrow].nlpindex = pos as i32;

        nlrow_add_to_stat(scip, nlrow, 1);
        capture_nl_row(scip, nlrow);

        // If we have a feasible NLP solution and it satisfies the new solution,
        // then it is still feasible; if the NLP was globally or locally
        // infeasible, then it stays that way; if the NLP was unbounded, then this
        // may not be the case anymore.
        let nlp = scip.nlp.as_mut().unwrap();
        if nlp.solstat <= NlpSolStat::Feasible {
            nlp.solstat = NlpSolStat::LocInfeasible;
        } else if nlp.solstat == NlpSolStat::Unbounded {
            nlp.solstat = NlpSolStat::Unknown;
        }
    }

    scip.nlp.as_mut().unwrap().nunflushednlrowadd += nlrows.len() as i32;

    Ok(())
}

/// `nlpMoveNlrow`: moves a nonlinear row to a different place, and updates
/// all corresponding data structures.
fn nlp_move_nlrow(scip: &mut Scip, oldpos: usize, newpos: usize) {
    if oldpos == newpos {
        return;
    }

    let nlp = scip.nlp.as_mut().unwrap();
    let r = nlp.nlrows[oldpos];
    nlp.nlrows[newpos] = r;
    scip.nlrows[r].nlpindex = newpos as i32;
}

/// `nlpDelNlRowPos`: deletes nonlinear row with given position from NLP.
fn nlp_del_nl_row_pos(scip: &mut Scip, pos: usize) -> ScipResult {
    let mut nlrow = scip.nlp.as_ref().unwrap().nlrows[pos];

    // If row was not in NLPI yet, then we have one unflushed nlrow addition
    // less (rows are never in an NLPI problem).
    scip.nlp.as_mut().unwrap().nunflushednlrowadd -= 1;

    // Move NLP row from the end to pos and mark nlrow to be not in NLP anymore.
    let last = scip.nlp.as_ref().unwrap().nlrows.len() - 1;
    nlp_move_nlrow(scip, last, pos);
    scip.nlrows[nlrow].nlpindex = -1;

    // Do no longer count nlrow in NLP row statistics.
    nlrow_add_to_stat(scip, nlrow, -1);

    // Forget about restriction.
    release_nl_row(scip, &mut nlrow)?;
    let nlp = scip.nlp.as_mut().unwrap();
    nlp.nlrows.pop();

    if nlp.solstat < NlpSolStat::LocOpt {
        nlp.solstat = NlpSolStat::Feasible;
    } else if nlp.solstat == NlpSolStat::GlobInfeasible {
        nlp.solstat = NlpSolStat::LocInfeasible;
    }

    Ok(())
}

/// `nlpUpdateVarBounds`: updates bounds on a variable in the NLPI problem.
fn nlp_update_var_bounds(scip: &mut Scip, var: VarId) {
    let nlp = scip.nlp.as_ref().unwrap();

    // Original variable bounds are ignored during diving (all variable bounds
    // are reset to their current value in exitDiving).
    if nlp.indiving {
        return;
    }

    // Get position of variable in NLP; if variable not in NLPI yet, nothing to
    // do (variables are never in an NLPI problem).
    let pos = nlp.varhash[&var] as usize;
    debug_assert!(nlp.varmap_nlp2nlpi[pos] == -1);
}

/// `nlpUpdateObjCoef`: updates coefficient of a variable in the objective.
fn nlp_update_obj_coef(scip: &mut Scip, _var: VarId) {
    let nlp = scip.nlp.as_mut().unwrap();

    // If the objective in the NLPI is not up to date, then we do not need to do
    // something here.
    if !nlp.objflushed {
        return;
    }

    // Original objective is ignored during diving; we just need to remember
    // that at end of diving we have to flush the objective.
    if nlp.indiving {
        nlp.objflushed = false;
    }

    // Actually we only need to remember flushing the objective if we also have
    // an NLPI (and there is none).
}

/// `nlpAddVars`: adds new variables to the NLP.
fn nlp_add_vars(scip: &mut Scip, vars: &[VarId]) -> ScipResult {
    if vars.is_empty() {
        return Ok(());
    }

    for &var in vars {
        crate::var::var_capture(scip, var);

        let nlp = scip.nlp.as_mut().unwrap();
        let pos = nlp.vars.len();
        nlp.vars.push(var);
        nlp.varmap_nlp2nlpi.push(-1);
        nlp.varhash.insert(var, pos as i32);
        nlp.varlbdualvals.push(0.0);
        nlp.varubdualvals.push(0.0);

        // Update objective, if necessary (new variables have coefficient 0.0
        // anyway).
        if scip.vars[var].obj != 0.0 {
            nlp_update_obj_coef(scip, var);
        }

        // (There is never an initial guess.)

        // If we have a feasible NLP solution, then it remains feasible but we have
        // to update the objective function.
        if scip.nlp.as_ref().unwrap().solstat <= NlpSolStat::Feasible {
            let bb = crate::var::var_get_best_bound_local(scip, var);
            crate::var::var_set_nlp_sol(scip, var, bb)?;
            let obj = scip.vars[var].obj;
            let nlp = scip.nlp.as_mut().unwrap();
            nlp.primalsolobjval += obj * bb;
            nlp.solstat = NlpSolStat::Feasible;
        }

        // Catch events on variable.
        // @todo should store event filter position in nlp?
        let eventhdlr = scip.nlp.as_ref().unwrap().eventhdlr;
        crate::var::var_catch_event(scip, var, VAREVENTTYPE, eventhdlr, NLPEVENTDATA, None)?;
    }

    let nlp = scip.nlp.as_mut().unwrap();
    nlp.nunflushedvaradd += vars.len() as i32;

    Ok(())
}

/// `nlpMoveVar`: moves a variable to a different place, and updates all
/// corresponding data structures.
fn nlp_move_var(nlp: &mut Nlp, oldpos: usize, newpos: usize) {
    if oldpos == newpos {
        return;
    }

    let v = nlp.vars[oldpos];
    nlp.varhash.insert(v, newpos as i32);
    nlp.vars[newpos] = v;
    nlp.varmap_nlp2nlpi[newpos] = nlp.varmap_nlp2nlpi[oldpos];
    nlp.varlbdualvals[newpos] = nlp.varlbdualvals[oldpos];
    nlp.varubdualvals[newpos] = nlp.varubdualvals[oldpos];
}

/// `nlpDelVarPos`: deletes variable with given position from NLP.
fn nlp_del_var_pos(scip: &mut Scip, pos: usize) -> ScipResult {
    let mut var = scip.nlp.as_ref().unwrap().vars[pos];

    // (The reference is built without NDEBUG and asserts that the variable is
    // not used by any nonlinear row, sorting the linear parts that are sorted
    // already only.)

    // If we had a feasible solution, then adjust objective function value; if
    // NLP was unbounded before, then maybe it is not anymore.
    let solstat = scip.nlp.as_ref().unwrap().solstat;
    if solstat <= NlpSolStat::Feasible {
        let d = scip.vars[var].obj * crate::var::var_get_nlp_sol(scip, var);
        scip.nlp.as_mut().unwrap().primalsolobjval -= d;
    } else if solstat == NlpSolStat::Unbounded {
        scip.nlp.as_mut().unwrap().solstat = NlpSolStat::Unknown;
    }

    // If it was not in the NLPI yet, then we have one unflushed var addition
    // less now (variables are never in an NLPI problem).
    scip.nlp.as_mut().unwrap().nunflushedvaradd -= 1;

    // Drop events on variable.
    let eventhdlr = scip.nlp.as_ref().unwrap().eventhdlr;
    crate::var::var_drop_event(scip, var, VAREVENTTYPE, eventhdlr, NLPEVENTDATA, -1)?;

    // Move variable from end to pos.
    let nlp = scip.nlp.as_mut().unwrap();
    let last = nlp.vars.len() - 1;
    nlp_move_var(nlp, last, pos);

    // Forget about variable.
    nlp.varhash.remove(&var);
    nlp.vars.pop();
    nlp.varmap_nlp2nlpi.pop();
    nlp.varlbdualvals.pop();
    nlp.varubdualvals.pop();
    crate::var::var_release(scip, &mut var)?;

    Ok(())
}

/// `nlpRemoveFixedVar`: notifies NLP that a variable was fixed, so it is
/// removed from objective, all rows, and the NLP variables.
fn nlp_remove_fixed_var(scip: &mut Scip, var: VarId) -> ScipResult {
    // Remove var from all rows.
    let nlrows = scip.nlp.as_ref().unwrap().nlrows.clone();
    for nlrow in nlrows {
        nlrow_remove_fixed_var(scip, nlrow, var)?;
    }

    // Remove variable from NLP.
    nlp_del_var(scip, var)
}

/// `eventExecNlp`: handles all events necessary for maintaining NLP data.
fn event_exec_nlp(scip: &mut Scip, _eventhdlr: EventhdlrId, ev: &Event, _eventdata: usize) -> ScipResult {
    let etype = ev.eventtype;
    let var = ev.data.var;

    if event::EVENTTYPE_VARADDED & etype != 0 {
        nlp_add_var(scip, var)?;
    } else if event::EVENTTYPE_VARDELETED & etype != 0 {
        nlp_del_var(scip, var)?;
    } else if event::EVENTTYPE_VARFIXED & etype != 0 {
        // Variable was fixed, aggregated, or multi-aggregated.
        // TODO is this ever happening? that is, can we have changes in a variable
        // status during solve?
        nlp_remove_fixed_var(scip, var)?;
    } else if event::EVENTTYPE_BOUNDCHANGED & etype != 0 {
        nlp_update_var_bounds(scip, var);
    } else if event::EVENTTYPE_OBJCHANGED & etype != 0 {
        nlp_update_obj_coef(scip, var);
    } else {
        log::error!("unexpected event {etype:#x} on variable <{}>", scip.vars[var].name);
        return Err(Retcode::Error);
    }

    Ok(())
}

/* public NLP methods */

/// `SCIPnlpInclude`: includes event handler that is used by NLP.
pub fn nlp_include(scip: &mut Scip) -> ScipResult {
    // Check whether event handler is already present.
    if crate::set::set_find_eventhdlr(scip, EVENTHDLR_NAME) != NONE {
        log::error!("event handler <{EVENTHDLR_NAME}> already included.");
        return Err(Retcode::InvalidData);
    }

    let h = event::eventhdlr_create(EVENTHDLR_NAME, EVENTHDLR_DESC, None, None, None, None, None, None, None, Some(event_exec_nlp), None);
    let id = scip.eventhdlrs.push(h);
    crate::set::set_include_eventhdlr(scip, id);

    Ok(())
}

/// `SCIPnlpCreate`: construct a new empty NLP.
pub fn nlp_create(scip: &mut Scip, name: &str) -> ScipResult {
    // Select NLP solver (if any available) and setup problem: there are no NLP
    // solvers; maybe someone wanna use the NLP just to collect nonlinearities,
    // but is not necessarily interesting on solving, so we allow this and just
    // continue.

    // Event handling: catch variable addition and deletion events.
    let eventhdlr = crate::set::set_find_eventhdlr(scip, EVENTHDLR_NAME);
    if eventhdlr == NONE {
        log::error!("NLP eventhandler <{EVENTHDLR_NAME}> not found.");
        return Err(Retcode::PluginNotFound);
    }

    scip.nlp = Some(Box::new(Nlp {
        nunflushedvaradd: 0,
        nunflushedvardel: 0,
        nunflushednlrowadd: 0,
        nunflushednlrowdel: 0,
        indiving: false,
        vars: Vec::new(),
        varhash: HashMap::new(),
        varmap_nlp2nlpi: Vec::new(),
        nlrows: Vec::new(),
        nnlrowlinear: 0,
        nnlrowconvexineq: 0,
        nnlrownonconvexineq: 0,
        nnlrownonlineareq: 0,
        objflushed: true,
        haveinitguess: false,
        primalsolobjval: INVALID,
        solstat: NlpSolStat::Unknown,
        termstat: NlpTermStat::Other,
        varlbdualvals: Vec::new(),
        varubdualvals: Vec::new(),
        eventhdlr,
        globalfilterpos: -1,
        name: name.to_string(),
    }));

    let mut globalfilterpos = -1;
    event::eventfilter_add(
        scip,
        crate::scip::GLOBALEVENTFILTER,
        event::EVENTTYPE_VARADDED | event::EVENTTYPE_VARDELETED,
        eventhdlr,
        NLPEVENTDATA,
        Some(&mut globalfilterpos),
    )?;
    scip.nlp.as_mut().unwrap().globalfilterpos = globalfilterpos;

    Ok(())
}

/// `SCIPnlpFree`: frees NLP data object.
pub fn nlp_free(scip: &mut Scip) -> ScipResult {
    // Drop global events (variable addition and deletion).
    let (eventhdlr, globalfilterpos) = {
        let nlp = scip.nlp.as_ref().unwrap();
        (nlp.eventhdlr, nlp.globalfilterpos)
    };
    event::eventfilter_del(
        scip,
        crate::scip::GLOBALEVENTFILTER,
        event::EVENTTYPE_VARADDED | event::EVENTTYPE_VARDELETED,
        eventhdlr,
        NLPEVENTDATA,
        globalfilterpos,
    )?;

    nlp_reset(scip)?;

    scip.nlp = None;

    Ok(())
}

/// `SCIPnlpReset`: resets the NLP to the empty NLP by removing all variables
/// and rows from NLP, releasing all rows, and flushing the changes to the
/// NLP solver.
pub fn nlp_reset(scip: &mut Scip) -> ScipResult {
    // (The NLP is never in diving mode: diving needs an NLP solver.)

    {
        let nlp = scip.nlp.as_mut().unwrap();
        nlp.solstat = NlpSolStat::Unknown;
        nlp.termstat = NlpTermStat::Other;
        nlp.haveinitguess = false;
    }

    for i in (0..scip.nlp.as_ref().unwrap().nlrows.len()).rev() {
        nlp_del_nl_row_pos(scip, i)?;
    }

    for i in (0..scip.nlp.as_ref().unwrap().vars.len()).rev() {
        nlp_del_var_pos(scip, i)?;
    }

    // SCIPnlpFlush: without an NLP solver, there is nothing to flush.

    Ok(())
}

/// `SCIPnlpAddVar`: adds a variable to the NLP and captures the variable.
fn nlp_add_var(scip: &mut Scip, var: VarId) -> ScipResult {
    if scip.nlp.as_ref().unwrap().indiving {
        log::error!("cannot add variable during NLP diving");
        return Err(Retcode::Error);
    }

    nlp_add_vars(scip, &[var])
}

/// `SCIPnlpAddVars`: adds a set of variables to the NLP and captures the
/// variables.
pub fn nlp_add_vars_public(scip: &mut Scip, vars: &[VarId]) -> ScipResult {
    if scip.nlp.as_ref().unwrap().indiving && !vars.is_empty() {
        log::error!("cannot add variables during NLP diving");
        return Err(Retcode::Error);
    }

    nlp_add_vars(scip, vars)
}

/// `SCIPnlpDelVar`: deletes a variable from the NLP and releases the
/// variable.
fn nlp_del_var(scip: &mut Scip, var: VarId) -> ScipResult {
    let nlp = scip.nlp.as_ref().unwrap();
    let Some(&varpos) = nlp.varhash.get(&var) else {
        log::error!("variable <{}> not found in NLP, cannot delete", scip.vars[var].name);
        return Err(Retcode::Error);
    };

    if nlp.indiving {
        log::error!("cannot delete variable during NLP diving");
        return Err(Retcode::Error);
    }

    nlp_del_var_pos(scip, varpos as usize)
}

/* scip_nlp.c */

/// `SCIPisNLPConstructed`: returns, whether an NLP has been constructed.
pub fn is_nlp_constructed(scip: &Scip) -> bool {
    scip.nlp.is_some()
}

/// `SCIPgetNLPVarsNonlinearity` (with `SCIPnlpGetVarsNonlinearity`): the
/// number of times each NLP variable appears in the nonlinear part of an
/// NLP row, by NLP variable position.
pub fn get_nlp_vars_nonlinearity(scip: &mut Scip) -> ScipResult<Vec<i32>> {
    if scip.nlp.is_none() {
        log::error!("NLP has not been constructed.");
        return Err(Retcode::InvalidCall);
    }

    let nlrows = scip.nlp.as_ref().unwrap().nlrows.clone();
    let mut nlcount = vec![0i32; scip.nlp.as_ref().unwrap().vars.len()];

    let mut it = expr::ExprIter::create();

    for nlrow in nlrows {
        let rowexpr = scip.nlrows[nlrow].expr;
        if rowexpr == NONE {
            continue;
        }

        it.init(scip, rowexpr, expr::ExprIterType::Dfs, false)?;
        let mut e = it.get_current();
        while !it.is_end() {
            if expr::expr_is_var(scip, e) {
                let var = crate::expr_var::get_var_expr_var(scip, e);
                let varidx = scip.nlp.as_ref().unwrap().varhash[&var];
                nlcount[varidx as usize] += 1;
            }
            e = it.get_next(scip);
        }
    }

    it.free(scip);

    Ok(nlcount)
}

/// `SCIPenableNLP`: notifies SCIP that the NLP relaxation should be
/// initialized in INITSOLVE.
pub fn enable_nlp(scip: &mut Scip) {
    scip.probs[crate::prob::TRANSPROB].nlpenabled = true;
}

/// `SCIPaddNlRow`: adds a nonlinear row to the NLP; this row is captured by
/// the NLP.
pub fn add_nl_row(scip: &mut Scip, nlrow: usize) -> ScipResult {
    if scip.nlp.is_none() {
        log::error!("NLP has not been constructed.");
        return Err(Retcode::InvalidCall);
    }

    if scip.nlp.as_ref().unwrap().indiving {
        log::error!("cannot add row during NLP diving");
        return Err(Retcode::Error);
    }

    nlp_add_nl_rows(scip, &[nlrow])
}

/// `SCIPdelNlRow`: removes a nonlinear row from the NLP; does nothing if
/// nonlinear row is not in NLP.
pub fn del_nl_row(scip: &mut Scip, nlrow: usize) -> ScipResult {
    if scip.nlp.is_none() {
        log::error!("NLP has not been constructed.");
        return Err(Retcode::InvalidCall);
    }

    // If row not in NLP, nothing to do.
    if scip.nlrows[nlrow].nlpindex == -1 {
        return Ok(());
    }

    if scip.nlp.as_ref().unwrap().indiving {
        log::error!("cannot delete row during NLP diving");
        return Err(Retcode::Error);
    }

    let pos = scip.nlrows[nlrow].nlpindex as usize;
    nlp_del_nl_row_pos(scip, pos)
}
