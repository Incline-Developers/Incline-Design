//! Port of `nlhdlr_soc.c`: nonlinear handler for second order cone
//! constraints.

use std::collections::{HashMap, HashSet};

use crate::{
    def::INVALID,
    expr::{self, ExprId},
    nlhdlr::{self, NLHDLR_METHOD_SEPAABOVE, NLHDLR_METHOD_SEPABELOW, NLHDLR_METHOD_SEPABOTH, NlhdlrId, NlhdlrMethod},
    retcode::ScipResult,
    rowprep::RowPrep,
    scip::Scip,
    types::{ConsId, ConshdlrId, LockType, RowId, ScipResultCode, SideType, SolId, VarId},
    var::NONE,
};

/* fundamental nonlinear handler properties */
const NLHDLR_NAME: &str = "soc";
const NLHDLR_DESC: &str = "nonlinear handler for second-order cone structures";
/// Priority of the nonlinear handler for detection.
const NLHDLR_DETECTPRIORITY: i32 = 100;
/// Priority of the nonlinear handler for enforcement.
const NLHDLR_ENFOPRIORITY: i32 = 100;
/// Default value for parameter mincutefficacy.
const DEFAULT_MINCUTEFFICACY: f64 = 1e-5;
/// Default value for parameter compeigenvalues.
const DEFAULT_COMPEIGENVALUES: bool = true;

/*
 * Data structures
 */

/// `SCIP_NLHDLREXPRDATA`: nonlinear handler expression data.
///
/// A 'term' is one of the arguments of the quadratic terms, i.e. v_i^T x +
/// beta_i. The last term is always the one on the right-hand side. This
/// means that nterms is equal to n+1.
///
/// - vars contains a list of all expressions which are treated as variables
///   (no duplicates)
/// - offsets contains the constants beta_i of each term
/// - transcoefs contains the non-zero values of the transformation vectors
///   v_i of each term
/// - transcoefsidx contains for each entry of transcoefs the position of the
///   respective variable in vars
/// - termbegins contains the index at which the transcoefs of each term
///   start, with a sentinel value
/// - nterms is the total number of terms appearing on both sides
/// - nvars is the total number of unique variables appearing (length of
///   vars)
///
/// The disaggregation is implicitly stored in the variables disvars and
/// disrow. An SOC is replaced by n smaller SOCs (v_i^T x + beta_i)^2 <=
/// disvar_i * (v_{n+1}^T x + beta_{n+1}) and the row sum_i disvar_i <=
/// v_{n+1}^T x + beta_{n+1}. The disaggregation only happens if we have more
/// than 3 terms.
struct ExprData {
    /// Expressions which (aux)variables appear on both sides (x).
    vars: Vec<ExprId>,
    /// Offsets of both sides (beta_i).
    offsets: Vec<f64>,
    /// Non-zeros of linear transformation vectors (v_i).
    transcoefs: Vec<f64>,
    /// Mapping of transformation coefficients to variable indices in vars.
    transcoefsidx: Vec<i32>,
    /// Starting indices of transcoefs for each term.
    termbegins: Vec<i32>,
    /// Total number of variables appearing.
    nvars: usize,
    /// Number of summands in the SQRT +1 for RHS (n+1).
    nterms: usize,

    /* variables for cone disaggregation */
    /// Disaggregation variables for each term in lhs (empty if none).
    disvars: Vec<VarId>,
    /// Disaggregation row (`NONE` if none).
    disrow: RowId,

    /* separation data */
    /// Current values for vars (empty while not allocated).
    varvals: Vec<f64>,
    /// Current values for disvars (empty while not allocated).
    disvarvals: Vec<f64>,
}

/// `SCIP_NLHDLRDATA`.
struct NlhdlrData {
    /// Minimum efficacy a cut need to be added.
    mincutefficacy: f64,
    /// Whether Eigenvalue computations should be done to detect complex
    /// cases.
    compeigenvalues: bool,
}

#[inline]
fn ed(scip: &Scip, nlhdlrexprdata: usize) -> &ExprData {
    nlhdlr::exprdata::<ExprData>(scip, nlhdlrexprdata)
}

#[inline]
fn edm(scip: &mut Scip, nlhdlrexprdata: usize) -> &mut ExprData {
    nlhdlr::exprdata_mut::<ExprData>(scip, nlhdlrexprdata)
}

#[inline]
fn hd(scip: &Scip, nlhdlr: NlhdlrId) -> &NlhdlrData {
    nlhdlr::hdlrdata::<NlhdlrData>(scip, nlhdlr)
}

/*
 * Local methods
 */

/// `createDisaggrVars`: helper method to create variables for the cone
/// disaggregation.
fn create_disaggr_vars(scip: &mut Scip, e: ExprId, nlhdlrexprdata: usize) -> ScipResult {
    let ndisvars = ed(scip, nlhdlrexprdata).nterms - 1;

    // Allocate memory.
    let mut disvars = Vec::with_capacity(ndisvars);

    // Create disaggregation variables representing the epigraph of (v_i^T x +
    // beta_i)^2 / (v_{n+1}^T x + beta_{n+1}).
    let infinity = scip.set.infinity();
    for i in 0..ndisvars {
        let name = format!("conedis_{e}_{i}");
        let var = crate::scip_var::create_var_impl(
            scip,
            &name,
            0.0,
            infinity,
            0.0,
            crate::types::VarType::Continuous,
            crate::types::ImplintType::None,
            true,
            false,
        )?;
        crate::var::var_mark_relaxation_only(scip, var);
        crate::scip_var::add_var(scip, var)?;
        crate::api::add_var_locks_type(scip, var, LockType::Model, 1, 1)?;
        disvars.push(var);
    }

    let d = edm(scip, nlhdlrexprdata);
    d.disvars = disvars;
    d.disvarvals = vec![0.0; ndisvars];

    Ok(())
}

/// `freeDisaggrVars`: helper method to free variables for the cone
/// disaggregation.
fn free_disaggr_vars(scip: &mut Scip, d: &mut ExprData) -> ScipResult {
    if d.disvars.is_empty() {
        return Ok(());
    }

    // Release variables.
    for v in d.disvars.iter_mut() {
        crate::api::add_var_locks_type(scip, *v, LockType::Model, -1, -1)?;
        crate::api::release_var(scip, v)?;
    }

    // Free memory.
    d.disvars = Vec::new();
    d.disvarvals = Vec::new();

    Ok(())
}

/// `createDisaggrRow`: helper method to create the disaggregation row
/// disvars_i <= v_{n+1}^T x + beta_{n+1}.
fn create_disaggr_row(scip: &mut Scip, conshdlr: ConshdlrId, e: ExprId, nlhdlrexprdata: usize) -> ScipResult {
    let nterms = ed(scip, nlhdlrexprdata).nterms;
    let beta = ed(scip, nlhdlrexprdata).offsets[nterms - 1];
    let ndisvars = nterms - 1;

    // Create row 0 <= beta_{n+1}.
    let name = format!("conedis_{e}_row");
    let ninf = -scip.set.infinity();
    let disrow = crate::scip_lp::create_empty_row_conshdlr(scip, conshdlr, &name, ninf, beta, false, false, true)?;
    edm(scip, nlhdlrexprdata).disrow = disrow;

    // Add disvars to row.
    for i in 0..ndisvars {
        let v = ed(scip, nlhdlrexprdata).disvars[i];
        crate::api::add_var_to_row(scip, disrow, v, 1.0)?;
    }

    // Add rhs vars to row.
    let (b, en) = (
        ed(scip, nlhdlrexprdata).termbegins[nterms - 1] as usize,
        ed(scip, nlhdlrexprdata).termbegins[nterms] as usize,
    );
    for i in b..en {
        let d = ed(scip, nlhdlrexprdata);
        let ve = d.vars[d.transcoefsidx[i] as usize];
        let coef = -d.transcoefs[i];
        let var = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, ve);

        crate::api::add_var_to_row(scip, disrow, var, coef)?;
    }

    Ok(())
}

/// `createNlhdlrExprData`: helper method to create nonlinear handler
/// expression data.
#[allow(clippy::too_many_arguments)]
fn create_nlhdlr_expr_data(scip: &mut Scip, vars: &[ExprId], offsets: &[f64], transcoefs: &[f64], transcoefsidx: &[i32], termbegins: &[i32], nvars: usize, nterms: usize) -> usize {
    let ntranscoefs = termbegins[nterms] as usize;

    scip.nlhdlrexprdatas.push(Box::new(ExprData {
        vars: vars[..nvars].to_vec(),
        offsets: offsets[..nterms].to_vec(),
        transcoefs: transcoefs[..ntranscoefs].to_vec(),
        transcoefsidx: transcoefsidx[..ntranscoefs].to_vec(),
        termbegins: termbegins[..nterms + 1].to_vec(),
        nvars,
        nterms,
        disvars: Vec::new(),
        disrow: NONE,
        varvals: Vec::new(),
        disvarvals: Vec::new(),
    }))
}

/// `freeNlhdlrExprData`: helper method to free nonlinear handler expression
/// data.
fn free_nlhdlr_expr_data(scip: &mut Scip, nlhdlrexprdata: &mut usize) -> ScipResult {
    let mut d = *scip.nlhdlrexprdatas.remove(*nlhdlrexprdata).downcast::<ExprData>().unwrap();

    // Free variables and row for cone disaggregation.
    free_disaggr_vars(scip, &mut d)?;

    *nlhdlrexprdata = NONE;

    Ok(())
}

/// `updateVarVals`: set varvals in nlhdlrexprdata to values from given SCIP
/// solution.
fn update_var_vals(scip: &mut Scip, nlhdlrexprdata: usize, sol: SolId, roundtinyfrac: bool) {
    // Update varvals.
    for i in 0..ed(scip, nlhdlrexprdata).nvars {
        let ve = ed(scip, nlhdlrexprdata).vars[i];
        let var = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, ve);
        let mut val = crate::api::get_sol_val(scip, sol, var);
        if roundtinyfrac && scip.set.is_integral(val) {
            val = scip.set.round(val);
        }
        edm(scip, nlhdlrexprdata).varvals[i] = val;
    }

    // Update disvarvals (in unittests, this may be NULL even though nterms >
    // 1).
    if !ed(scip, nlhdlrexprdata).disvarvals.is_empty() {
        for i in 0..ed(scip, nlhdlrexprdata).nterms - 1 {
            let var = ed(scip, nlhdlrexprdata).disvars[i];
            let mut val = crate::api::get_sol_val(scip, sol, var);
            if roundtinyfrac && scip.set.is_integral(val) {
                val = scip.set.round(val);
            }
            edm(scip, nlhdlrexprdata).disvarvals[i] = val;
        }
    }
}

/// `evalSingleTerm`: evaluate a single term of the form v_i^T x + beta_i.
fn eval_single_term(d: &ExprData, k: usize) -> f64 {
    let mut result = d.offsets[k];

    for i in d.termbegins[k] as usize..d.termbegins[k + 1] as usize {
        result += d.transcoefs[i] * d.varvals[d.transcoefsidx[i] as usize];
    }

    result
}

/// `generateCutSolSOC`: computes gradient cut for a 2D or 3D SOC.
///
/// A 3D SOC looks like sqrt((v_1^T x + beta_1)^2 + (v_2^T x + beta_2)^2) <=
/// v_3^T x + beta_3. Let f(x) be the left-hand-side; the gradient cut is
/// f(x*) + grad f(x*)(x - x*) <= v_3^T x + beta_3. If beta_1 = beta_2 = 0,
/// then the constant on the left-hand-side of the cut becomes zero. A 2D SOC
/// is |v_1^T x + beta_1| <= v_2^T x + beta_2, but we build the cut using the
/// same procedure as for 3D.
fn generate_cut_sol_soc(scip: &mut Scip, e: ExprId, nlhdlrexprdata: usize, mincutviolation: f64, rhsval: f64) -> Option<RowPrep> {
    let d = ed(scip, nlhdlrexprdata);
    let nterms = d.nterms;
    let mut valterms = [0.0, 0.0];

    // Evaluate lhs terms and compute f(x*), check whether both beta_1 and
    // beta_2 are zero.
    let mut fvalue = 0.0;
    let mut offsetzero = true;
    for i in 0..nterms - 1 {
        valterms[i] = eval_single_term(d, i);
        fvalue += valterms[i] * valterms[i];
        if d.offsets[i] != 0.0 {
            offsetzero = false;
        }
    }
    fvalue = fvalue.sqrt();

    // Don't generate cut if we are not violated @todo: remove this once core
    // detects better when a nlhdlr's cons is violated.
    if fvalue - rhsval <= mincutviolation {
        return None;
    }

    // If f(x*) = 0 then we are at top of cone, where we cannot generate cut.
    if scip.set.is_zero(fvalue) {
        return None;
    }

    // Create cut.
    let mut rowprep = RowPrep::new(SideType::Right, false);
    rowprep.ensure_size(d.termbegins[nterms] as usize);

    // Cut is f(x*) + \nabla f(x*)^T (x - x*) \leq v_n^T x + \beta_n, i.e.,
    // \nabla f(x*)^T x - v_n^T x \leq \beta_n + \nabla f(x*)^T x* - f(x*); thus
    // cutrhs is \beta_n - f(x*) + \nabla f(x*)^T x*; if offsetzero, then we
    // make sure that cutrhs is exactly \beta_n.
    let mut cutrhs = d.offsets[nterms - 1];
    if !offsetzero {
        cutrhs -= fvalue;
    }

    // Add cut coefficients from lhs terms and compute cut's rhs.
    for j in 0..nterms - 1 {
        for i in d.termbegins[j] as usize..d.termbegins[j + 1] as usize {
            let cutvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, d.vars[d.transcoefsidx[i] as usize]);

            // Cutcoef is (the first part of) the partial derivative w.r.t cutvar.
            let cutcoef = d.transcoefs[i] * valterms[j] / fvalue;

            rowprep.add_term(cutvar, cutcoef);

            if !offsetzero {
                cutrhs += cutcoef * d.varvals[d.transcoefsidx[i] as usize];
            }
        }
    }

    // Add terms for v_n.
    for i in d.termbegins[nterms - 1] as usize..d.termbegins[nterms] as usize {
        let cutvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, d.vars[d.transcoefsidx[i] as usize]);
        rowprep.add_term(cutvar, -d.transcoefs[i]);
    }

    // Add side.
    rowprep.add_side(cutrhs);

    // Set name.
    rowprep.name = format!("soc{nterms}_{e}_{}", crate::api::get_n_lps(scip));

    Some(rowprep)
}

/// `generateCutSolDisagg`: helper method to compute and add a gradient cut
/// for the k-th cone disaggregation.
///
/// After the SOC constraint has been disaggregated into the row sum_i y_i <=
/// v_n^T x + beta_n and the smaller SOC constraints (v_i^T x + beta_i)^2 <=
/// (v_n^T x + beta_n) y_i, we want to separate one of the small rotated
/// cones, in standard form sqrt(4(v_i^T x + beta_i)^2 + (v_n^T x + beta_n -
/// y_i)^2) - v_n^T x - beta_n - y_i <= 0, by its gradient cut.
fn generate_cut_sol_disagg(scip: &mut Scip, e: ExprId, nlhdlrexprdata: usize, disaggidx: usize, mincutviolation: f64, rhsval: f64) -> Option<RowPrep> {
    let d = ed(scip, nlhdlrexprdata);
    let nterms = d.nterms;
    let termbegins = &d.termbegins;

    // nterms is equal to n in the description and disaggidx is in {0, ..., n
    // - 1}.

    let disvarval = d.disvarvals[disaggidx];

    let lhsval = eval_single_term(d, disaggidx);

    let t = rhsval - disvarval;
    let denominator = (4.0 * (lhsval * lhsval) + t * t).sqrt();

    // Compute value of function to be separated (f(x*,y*)).
    let fvalue = denominator - rhsval - disvarval;

    // If the disagg soc is not violated don't compute cut.
    if fvalue <= mincutviolation {
        return None;
    }

    // If the denominator is 0 -> the constraint can't be violated, and the
    // gradient is infinite.
    if scip.set.is_zero(denominator) {
        return None;
    }

    // Compute upper bound on the number of variables in cut: vars in rhs + vars
    // in term + disagg var.
    let ncutvars = (termbegins[nterms] - termbegins[nterms - 1]) + (termbegins[disaggidx + 1] - termbegins[disaggidx]) + 1;

    // Create cut.
    let mut rowprep = RowPrep::new(SideType::Right, false);
    rowprep.ensure_size(ncutvars as usize);

    // Check whether offsets (beta) are zero, so we can know cut constant will
    // be zero.
    let offsetzero = d.offsets[disaggidx] == 0.0 && d.offsets[nterms - 1] == 0.0;

    // Constant will be grad_f(x*,y*)^T (x*, y*).
    let mut constant = 0.0;

    // A variable could appear on the lhs and rhs, but we add the coefficients
    // separately.

    // Add terms for v_disaggidx.
    for i in termbegins[disaggidx] as usize..termbegins[disaggidx + 1] as usize {
        let cutvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, d.vars[d.transcoefsidx[i] as usize]);

        // Cutcoef is (the first part of) the partial derivative w.r.t cutvar.
        let cutcoef = 4.0 * lhsval * d.transcoefs[i] / denominator;

        rowprep.add_term(cutvar, cutcoef);

        if !offsetzero {
            constant += cutcoef * d.varvals[d.transcoefsidx[i] as usize];
        }
    }

    // Add terms for v_n.
    for i in termbegins[nterms - 1] as usize..termbegins[nterms] as usize {
        let cutvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, d.vars[d.transcoefsidx[i] as usize]);

        // Cutcoef is the (second part of) the partial derivative w.r.t cutvar.
        let cutcoef = (rhsval - disvarval) * d.transcoefs[i] / denominator - d.transcoefs[i];

        rowprep.add_term(cutvar, cutcoef);

        if !offsetzero {
            constant += cutcoef * d.varvals[d.transcoefsidx[i] as usize];
        }
    }

    // Add term for disvar: cutcoef is the the partial derivative w.r.t. the
    // disaggregation variable.
    let cutcoef = (disvarval - rhsval) / denominator - 1.0;
    let cutvar = d.disvars[disaggidx];

    rowprep.add_term(cutvar, cutcoef);

    if !offsetzero {
        constant += cutcoef * d.disvarvals[disaggidx];

        // Add side.
        rowprep.add_side(constant - fvalue);
    }

    // Set name.
    rowprep.name = format!("soc_{e}_{disaggidx}_{}", crate::api::get_n_lps(scip));

    Some(rowprep)
}

/// `SCIPgetCutEfficacy` (`scip_cut.c`).
fn get_cut_efficacy(scip: &mut Scip, sol: SolId, cut: RowId) -> f64 {
    if sol == NONE {
        crate::lp::row_get_lp_efficacy(scip, cut)
    } else {
        crate::lp::row_get_sol_efficacy(scip, cut, sol)
    }
}

/// `addCut`: given a rowprep, does a number of cleanup and checks and, if
/// successful, generate a cut to be added to the sepastorage.
fn add_cut(scip: &mut Scip, mincutefficacy: f64, rowprep: &mut RowPrep, sol: SolId, cons: ConsId, allowweakcuts: bool, result: &mut ScipResultCode) -> ScipResult {
    let mut success = false;
    let huge = scip.set.get_huge_value();
    rowprep.cleanup2(scip, sol, huge, Some(&mut success));

    if !success {
        return Ok(());
    }

    if rowprep.get_violation(scip, sol, None) <= crate::lp::lp_get_feastol(scip) {
        return Ok(());
    }

    let mut cut = rowprep.get_row_cons(scip, cons)?;

    let cutefficacy = get_cut_efficacy(scip, sol, cut);

    // Check whether cut is applicable.
    if crate::sepastore::sepastore_is_cut_applicable(scip, cut) && (allowweakcuts || cutefficacy >= mincutefficacy) {
        let infeasible = crate::scip_lp::add_row(scip, cut, false)?;

        if infeasible {
            *result = ScipResultCode::Cutoff;
        } else {
            *result = ScipResultCode::Separated;
        }
    }

    // Release row.
    crate::scip_lp::release_row(scip, &mut cut)
}

/// `addCutPool`: given a rowprep, does a number of cleanup and checks and,
/// if successful, generate a cut to be added to the cutpool.
fn add_cut_pool(scip: &mut Scip, rowprep: &mut RowPrep, sol: SolId, cons: ConsId) -> ScipResult {
    let mut success = false;
    let huge = scip.set.get_huge_value();
    rowprep.cleanup2(scip, sol, huge, Some(&mut success));

    // If failed or cut is only locally valid now, then skip.
    if !success || rowprep.local {
        return Ok(());
    }

    // If row after cleanup is just a boundchange, then skip.
    if rowprep.get_n_vars() <= 1 {
        return Ok(());
    }

    // Generate row and add to cutpool.
    let mut cut = rowprep.get_row_cons(scip, cons)?;
    crate::api::add_pool_cut(scip, cut)?;
    crate::scip_lp::release_row(scip, &mut cut)
}

/// `detectSocNorm`: detects if expr <= auxvar is of the form sqrt(sum_i
/// coef_i (expr_i + shift_i)^2 + const) <= auxvar.
///
/// If a user inputs the above expression with `const` = -epsilon, then
/// `const` is going to be set to 0.
fn detect_soc_norm(scip: &mut Scip, e: ExprId, nlhdlrexprdata: &mut usize, success: &mut bool) -> ScipResult {
    *success = false;
    let mut issoc = true;

    // Relation is not "<=" -> skip.
    if crate::cons_nonlinear::get_expr_n_locks_pos_nonlinear(scip, e) == 0 {
        return Ok(());
    }

    // Expression is a leaf (variable or constant).
    if scip.exprs[e].children.is_empty() {
        return Ok(());
    }

    let child = scip.exprs[e].children[0];

    // Check whether expression is a sqrt and has a sum as child with at least 2
    // children and a non-negative constant.
    if !expr::expr_is_power(scip, e)
        || crate::expr_pow::get_exponent_expr_pow(scip, e) != 0.5
        || !expr::expr_is_sum(scip, child)
        || scip.exprs[child].children.len() < 2
        || crate::expr_sum::get_constant_expr_sum(scip, child) < 0.0
    {
        return Ok(());
    }

    // Get children of the sum.
    let children = scip.exprs[child].children.clone();
    let nchildren = children.len();
    let childcoefs = crate::expr_sum::get_coefs_expr_sum(scip, child).to_vec();

    // TODO: should we initialize the hashmap with size SCIPgetNVars() so that
    // it never has to be resized?
    let mut expr2idx: HashMap<ExprId, usize> = HashMap::with_capacity(nchildren);
    let mut linexprs: HashSet<ExprId> = HashSet::with_capacity(nchildren);

    // We create coefs array here already, since we have to fill it in first
    // loop in case of success; +1 for auxvar.
    let mut transcoefs = vec![0.0; nchildren + 1];

    let mut nterms = 0;

    // Check if all children are squares or linear terms with matching square
    // term: if the i-th child is (pow, expr, 2) we store the association
    // <|expr -> i|> in expr2idx and if expr was in linexprs, we remove it from
    // there. If the i-th child is expr' (different from (pow, expr, 2)) and
    // expr' is not a key of expr2idx, we add it to linexprs. If at the end
    // there is any expr in linexpr -> we do not have a separable quadratic
    // function.
    let is_sqr = |scip: &Scip, x: ExprId| expr::expr_is_power(scip, x) && crate::expr_pow::get_exponent_expr_pow(scip, x) == 2.0;
    let is_binvar = |scip: &Scip, x: ExprId| expr::expr_is_var(scip, x) && crate::var::var_is_binary(scip, crate::expr_var::get_var_expr_var(scip, x));
    for i in 0..nchildren {
        // Handle quadratic expressions children.
        if is_sqr(scip, children[i]) {
            let squarearg = scip.exprs[children[i]].children[0];

            expr2idx.entry(squarearg).or_insert(nterms);

            if childcoefs[i] < 0.0 {
                issoc = false;
                break;
            }
            transcoefs[nterms] = childcoefs[i].sqrt();

            linexprs.remove(&squarearg);
            nterms += 1;
        }
        // Handle binary variable children.
        else if is_binvar(scip, children[i]) {
            expr2idx.insert(children[i], nterms);

            if childcoefs[i] < 0.0 {
                issoc = false;
                break;
            }
            transcoefs[nterms] = childcoefs[i].sqrt();

            nterms += 1;
        } else if !expr2idx.contains_key(&children[i]) {
            linexprs.insert(children[i]);
        }
    }

    // There are linear terms without corresponding quadratic terms or it was
    // detected not to be soc.
    if !linexprs.is_empty() || !issoc {
        return Ok(());
    }

    // Add one to terms counter for auxvar.
    nterms += 1;

    let mut constant = crate::expr_sum::get_constant_expr_sum(scip, child);

    // Compute constant of possible soc expression to check its sign.
    for i in 0..nchildren {
        if !is_sqr(scip, children[i]) {
            let auxvarpos = expr2idx[&children[i]];
            let t = 0.5 * childcoefs[i] / transcoefs[auxvarpos];
            constant -= t * t;
        }
    }

    // If the constant is negative -> no SOC.
    if scip.set.is_negative(constant) {
        return Ok(());
    } else if scip.set.is_zero(constant) {
        constant = 0.0;
    }

    // At this point, we have found an SOC structure.
    *success = true;

    let nvars = nterms;

    // Add one to terms counter for constant term.
    if constant > 0.0 {
        nterms += 1;
    }

    // Allocate temporary memory to collect data.
    let mut vars = vec![NONE; nvars];
    let mut offsets = vec![0.0; nterms];
    let mut transcoefsidx = vec![0i32; nvars];
    let mut termbegins = vec![0i32; nterms + 1];

    // Fill in data for non constant terms of lhs; initialize their offsets.
    for i in 0..nvars - 1 {
        transcoefsidx[i] = i as i32;
        termbegins[i] = i as i32;
        offsets[i] = 0.0;
    }

    // Add constant term and rhs.
    vars[nvars - 1] = e;
    if constant > 0.0 {
        // Constant term.
        termbegins[nterms - 2] = (nterms - 2) as i32;
        offsets[nterms - 2] = constant.sqrt();

        // Rhs.
        termbegins[nterms - 1] = (nterms - 2) as i32;
        offsets[nterms - 1] = 0.0;
        transcoefsidx[nterms - 2] = (nvars - 1) as i32;
        transcoefs[nterms - 2] = 1.0;

        // Sentinel value.
        termbegins[nterms] = (nterms - 1) as i32;
    } else {
        // Rhs.
        termbegins[nterms - 1] = (nterms - 1) as i32;
        offsets[nterms - 1] = 0.0;
        transcoefsidx[nterms - 1] = (nvars - 1) as i32;
        transcoefs[nterms - 1] = 1.0;

        // Sentinel value.
        termbegins[nterms] = nterms as i32;
    }

    // Request required auxiliary variables and fill vars and offsets array.
    let mut nextentry = 0;
    for i in 0..nchildren {
        if is_sqr(scip, children[i]) {
            let squarearg = scip.exprs[children[i]].children[0];

            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, squarearg, true, false, false, false)?;

            vars[nextentry] = squarearg;
            nextentry += 1;
        } else if is_binvar(scip, children[i]) {
            // Handle binary variable children: no need to request auxvar.
            vars[nextentry] = children[i];
            nextentry += 1;
        } else {
            let auxvarpos = expr2idx[&children[i]];

            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, children[i], true, false, false, false)?;

            offsets[auxvarpos] = 0.5 * childcoefs[i] / transcoefs[auxvarpos];
        }
    }

    // Create and store nonlinear handler expression data.
    *nlhdlrexprdata = create_nlhdlr_expr_data(scip, &vars, &offsets, &transcoefs, &transcoefsidx, &termbegins, nvars, nterms);

    Ok(())
}

/// `detectSocQuadraticSimple`: helper method to detect c + sum_i coef_i
/// expr_i^2 - coef_k expr_k^2 <= 0 and c + sum_i coef_i expr_i^2 - coef_k
/// expr_k expr_l <= 0; binary linear variables are interpreted as quadratic
/// terms.
#[allow(clippy::too_many_arguments)]
fn detect_soc_quadratic_simple(scip: &mut Scip, e: ExprId, conslhs: f64, consrhs: f64, nlhdlrexprdata: &mut usize, enforcebelow: &mut bool, success: &mut bool) -> ScipResult {
    *success = false;

    // Check whether expression is a sum.
    let (children, mut childcoefs, constant) = if expr::expr_is_sum(scip, e) {
        // Get children of the sum; we duplicate the child coefficients since we
        // have to manipulate them.
        (
            scip.exprs[e].children.clone(),
            crate::expr_sum::get_coefs_expr_sum(scip, e).to_vec(),
            crate::expr_sum::get_constant_expr_sum(scip, e),
        )
    } else if expr::expr_is_product(scip, e) && scip.exprs[e].children.len() == 2 && conslhs != INVALID {
        // Handle bilinear term as SOC, if we have a constraint like x*y >=
        // constant (if conslhs is SCIP_INVALID, then we have not a constraint, but
        // a subexpression).
        (vec![e], vec![1.0], 0.0)
    } else {
        return Ok(());
    };
    let nchildren = children.len();

    // Initialize data.
    let mut lhsidx: i32 = -1;
    let mut rhsidx: i32 = -1;
    let mut nposquadterms = 0;
    let mut nnegquadterms = 0;
    let mut nposbilinterms = 0;
    let mut nnegbilinterms = 0;

    // Check if all children are quadratic or binary linear and count number of
    // positive and negative terms.
    for i in 0..nchildren {
        let c = children[i];
        if (expr::expr_is_power(scip, c) && crate::expr_pow::get_exponent_expr_pow(scip, c) == 2.0)
            || (expr::expr_is_var(scip, c) && crate::var::var_is_binary(scip, crate::expr_var::get_var_expr_var(scip, c)))
        {
            if childcoefs[i] > 0.0 {
                nposquadterms += 1;
                lhsidx = i as i32;
            } else {
                nnegquadterms += 1;
                rhsidx = i as i32;
            }
        } else if expr::expr_is_product(scip, c) && scip.exprs[c].children.len() == 2 {
            if childcoefs[i] > 0.0 {
                nposbilinterms += 1;
                lhsidx = i as i32;
            } else {
                nnegbilinterms += 1;
                rhsidx = i as i32;
            }
        } else {
            return Ok(());
        }

        // More than one positive eigenvalue and more than one negative eigenvalue
        // -> can't be convex.
        if nposquadterms > 1 && nnegquadterms > 1 {
            return Ok(());
        }

        // More than one bilinear term -> can't be handled by this method.
        if nposbilinterms + nnegbilinterms > 1 {
            return Ok(());
        }

        // One positive bilinear term and also at least one positive quadratic
        // term -> not a simple SOC.
        if nposbilinterms > 0 && nposquadterms > 0 {
            return Ok(());
        }

        // One negative bilinear term and also at least one negative quadratic
        // term -> not a simple SOC.
        if nnegbilinterms > 0 && nnegquadterms > 0 {
            return Ok(());
        }
    }

    if nposquadterms == nchildren || nnegquadterms == nchildren {
        return Ok(());
    }

    // If a bilinear term is involved, it is a hyperbolic expression.
    let ishyperbolic = nposbilinterms + nnegbilinterms > 0;

    let (lhs, rhs) = if conslhs == INVALID || consrhs == INVALID {
        expr::expr_eval_activity(scip, e)?;
        let expractivity = scip.exprs[e].activity;
        (
            if conslhs == INVALID { expractivity.inf } else { conslhs },
            if consrhs == INVALID { expractivity.sup } else { consrhs },
        )
    } else {
        (conslhs, consrhs)
    };

    // Detect case and store lhs/rhs information.
    let specialtermidx;
    let mut lhsconstant;
    if (ishyperbolic && nnegbilinterms > 0) || (!ishyperbolic && nnegquadterms < 2) {
        // We have -x*y + z^2 ... -> we want to write z^2 ... <= x*y; or we have
        // -x^2 + y^2 ... -> we want to write y^2 ... <= x^2; in any case, we need
        // a finite rhs.

        // If rhs is infinity, it can't be soc.
        // TODO: if it can't be soc, then we should enforce the caller so that we
        // do not try the more complex quadratic method.
        if scip.set.is_infinity(rhs) {
            return Ok(());
        }

        specialtermidx = rhsidx as usize;
        lhsconstant = constant - rhs;
        // Enforce expr <= rhs.
        *enforcebelow = true;
    } else {
        // We have x*y - z^2 ... -> we want to write x*y >= z^2 ... or we have x^2
        // - y^2 - z^2 ... -> we want to write x^2 >= y^2 + z^2 ...; in any case,
        // we need a finite lhs.

        // If lhs is infinity, it can't be soc.
        if scip.set.is_infinity(-lhs) {
            return Ok(());
        }

        specialtermidx = lhsidx as usize;
        lhsconstant = lhs - constant;

        // Negate all coefficients.
        for c in childcoefs.iter_mut() {
            *c = -*c;
        }
        // Enforce lhs <= expr.
        *enforcebelow = false;
    }

    let rhssign;
    let special = children[specialtermidx];
    if ishyperbolic {
        let sc0 = scip.exprs[special].children[0];
        let sc1 = scip.exprs[special].children[1];
        expr::expr_eval_activity(scip, sc0)?;
        let yactivity = scip.exprs[sc0].activity;

        expr::expr_eval_activity(scip, sc1)?;
        let zactivity = scip.exprs[sc1].activity;

        if scip.set.is_negative(yactivity.inf + zactivity.inf) {
            // The sum of the expressions in the bilinear term changes sign -> no SOC.
            if scip.set.is_positive(yactivity.sup + zactivity.sup) {
                return Ok(());
            }

            rhssign = -1.0;
        } else {
            rhssign = 1.0;
        }

        lhsconstant *= 4.0 / -childcoefs[specialtermidx];
    } else if expr::expr_is_var(scip, special) {
        // children[specialtermidx] can be a variable, in which case we treat it as
        // if it is squared.
        rhssign = 1.0;
    } else {
        let sc0 = scip.exprs[special].children[0];
        expr::expr_eval_activity(scip, sc0)?;
        let rhsactivity = scip.exprs[sc0].activity;

        if rhsactivity.inf < 0.0 {
            // Rhs variable changes sign -> no SOC.
            if rhsactivity.sup > 0.0 {
                return Ok(());
            }

            rhssign = -1.0;
        } else {
            rhssign = 1.0;
        }
    }

    if scip.set.is_negative(lhsconstant) {
        return Ok(());
    }

    if scip.set.is_zero(lhsconstant) {
        lhsconstant = 0.0;
    }

    // We have found an SOC-representable expression. Now build the
    // nlhdlrexprdata.
    //
    // In the non-hyperbolic case, c + sum_i coef_i expr_i^2 - coef_k expr_k^2 <=
    // 0 is transformed to sqrt( c + sum_i coef_i expr_i^2 ) <= coef_k expr_k, so
    // there are nchildren many vars, nchildren (+ 1 if c != 0) many terms,
    // nchildren many coefficients in the vs in SOC representation.
    //
    // In the hyperbolic case, c + sum_i coef_i expr_i^2 - coef_k expr_k expr_l
    // <= 0 is transformed to sqrt( 4(c + sum_i coef_i expr_i^2) + (expr_k -
    // expr_l)^2 ) <= expr_k + expr_l, so there are nchildren + 1many vars,
    // nchildren + 1(+ 1 if c != 0) many terms, nchildren + 3 many coefficients
    // in the vs in SOC representation.
    let ntranscoefs = if ishyperbolic { nchildren + 3 } else { nchildren };
    let nvars = if ishyperbolic { nchildren + 1 } else { nchildren };
    let mut nterms = nvars;

    // Constant term.
    if lhsconstant > 0.0 {
        nterms += 1;
    }

    // SOC was detected, allocate temporary memory for data to collect.
    let mut vars = vec![NONE; nvars];
    let mut offsets = vec![0.0; nterms];
    let mut transcoefs = vec![0.0; ntranscoefs];
    let mut transcoefsidx = vec![0i32; ntranscoefs];
    let mut termbegins = vec![0i32; nterms + 1];

    *success = true;
    let mut nextentry = 0;

    // Collect all the v_i and beta_i.
    let mut nnzinterms = 0;
    for i in 0..nchildren {
        // Variable and coef for rhs have to be set to the last entry.
        if i == specialtermidx {
            continue;
        }

        // Extract (unique) variable appearing in term.
        if expr::expr_is_var(scip, children[i]) {
            vars[nextentry] = children[i];
        } else {
            // Notify that we will require auxiliary variable.
            let c0 = scip.exprs[children[i]].children[0];
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, c0, true, false, false, false)?;
            vars[nextentry] = c0;
        }

        // Store v_i and beta_i.
        termbegins[nextentry] = nnzinterms as i32;
        offsets[nextentry] = 0.0;

        transcoefsidx[nnzinterms] = nextentry as i32;
        if ishyperbolic {
            // We eliminate the coefficient of the bilinear term to arrive at standard
            // form.
            transcoefs[nnzinterms] = (4.0 * childcoefs[i] / -childcoefs[specialtermidx]).sqrt();
        } else {
            transcoefs[nnzinterms] = childcoefs[i].sqrt();
        }

        // Finish adding nonzeros.
        nnzinterms += 1;

        // Finish processing term.
        nextentry += 1;
    }

    // Store term for constant (v_i = 0).
    if lhsconstant > 0.0 {
        termbegins[nextentry] = nnzinterms as i32;
        offsets[nextentry] = lhsconstant.sqrt();

        // Finish processing term; this term has 0 nonzero thus we do not increase
        // nnzinterms.
        nextentry += 1;
    }

    if !ishyperbolic {
        // Store rhs term.
        if expr::expr_is_var(scip, special) {
            // This should be the "children[specialtermidx] can be a variable, in
            // which case we treat it as if it is squared" case.
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, special, true, false, false, false)?;
            vars[nvars - 1] = special;
        } else {
            let c0 = scip.exprs[special].children[0];
            crate::cons_nonlinear::register_expr_usage_nonlinear(scip, c0, true, false, false, false)?;
            vars[nvars - 1] = c0;
        }

        termbegins[nextentry] = nnzinterms as i32;
        offsets[nextentry] = 0.0;

        transcoefs[nnzinterms] = rhssign * (-childcoefs[specialtermidx]).sqrt();
        transcoefsidx[nnzinterms] = (nvars - 1) as i32;

        // Finish adding nonzeros.
        nnzinterms += 1;

        // Finish processing term.
        nextentry += 1;
    } else {
        // Store last lhs term and rhs term coming from the bilinear term.
        let c0 = scip.exprs[special].children[0];
        crate::cons_nonlinear::register_expr_usage_nonlinear(scip, c0, true, false, false, false)?;
        vars[nvars - 2] = c0;

        let c1 = scip.exprs[special].children[1];
        crate::cons_nonlinear::register_expr_usage_nonlinear(scip, c1, true, false, false, false)?;
        vars[nvars - 1] = c1;

        // At this point, vars[nvars - 2] = expr_k and vars[nvars - 1] = expr_l; on
        // the lhs we have the term (expr_k - expr_l)^2.
        termbegins[nextentry] = nnzinterms as i32;
        offsets[nextentry] = 0.0;

        // expr_k
        transcoefsidx[nnzinterms] = (nvars - 2) as i32;
        transcoefs[nnzinterms] = 1.0;
        nnzinterms += 1;

        // - expr_l
        transcoefsidx[nnzinterms] = (nvars - 1) as i32;
        transcoefs[nnzinterms] = -1.0;
        nnzinterms += 1;

        // Finish processing term.
        nextentry += 1;

        // On rhs we have +/-(expr_k + expr_l).
        termbegins[nextentry] = nnzinterms as i32;
        offsets[nextentry] = 0.0;

        // rhssing * expr_k
        transcoefsidx[nnzinterms] = (nvars - 2) as i32;
        transcoefs[nnzinterms] = rhssign;
        nnzinterms += 1;

        // rhssing * expr_l
        transcoefsidx[nnzinterms] = (nvars - 1) as i32;
        transcoefs[nnzinterms] = rhssign;
        nnzinterms += 1;

        // Finish processing term.
        nextentry += 1;
    }

    // Sentinel value.
    termbegins[nextentry] = nnzinterms as i32;

    // Create and store nonlinear handler expression data.
    *nlhdlrexprdata = create_nlhdlr_expr_data(scip, &vars, &offsets, &transcoefs, &transcoefsidx, &termbegins, nvars, nterms);

    Ok(())
}

/// `detectSocQuadraticComplex`: detects complex quadratic expressions that
/// can be represented as SOC constraints (by an eigen-decomposition computed
/// using Lapack).
///
/// Lapack is not available in the reference build, so after the first check
/// this never detects anything.
fn detect_soc_quadratic_complex(scip: &mut Scip, e: ExprId, success: &mut bool) -> ScipResult {
    *success = false;

    // Check whether expression is a sum with at least 2 children.
    if !expr::expr_is_sum(scip, e) || scip.exprs[e].children.len() < 2 {
        return Ok(());
    }

    // We need Lapack to compute eigenvalues/vectors below (SCIPlapackIsAvailable()
    // is false in the reference).
    Ok(())
}

/// `detectSOC`: helper method to detect SOC structures.
///
/// The detection runs in 3 steps: 1. check if expression is a norm of the
/// form sqrt(sum_i (sqrcoef_i expr_i^2 + lincoef_i expr_i) + c), 2. check if
/// expression represents a simple soc-representable quadratic function, 3.
/// check if expression represents a quadratic inequality with exactly one
/// negative eigenvalue (only if parameter `compeigenvalues` is set).
#[allow(clippy::too_many_arguments)]
fn detect_soc(
    scip: &mut Scip,
    compeigenvalues: bool,
    e: ExprId,
    conslhs: f64,
    consrhs: f64,
    nlhdlrexprdata: &mut usize,
    enforcebelow: &mut bool,
    success: &mut bool,
) -> ScipResult {
    *success = false;

    // Check whether expression is given as norm as described in case 1 above:
    // if we have a constraint sqrt(sum x_i^2) <= constant, then it might be
    // better not to handle this here; thus, we only call detectSocNorm when the
    // expr is _not_ the root of a constraint.
    if conslhs == INVALID && consrhs == INVALID {
        detect_soc_norm(scip, e, nlhdlrexprdata, success)?;
        *enforcebelow = *success;
    }

    if !*success {
        // Check whether expression is a simple soc-respresentable quadratic
        // expression as described in case 2 above.
        detect_soc_quadratic_simple(scip, e, conslhs, consrhs, nlhdlrexprdata, enforcebelow, success)?;
    }

    if !*success && compeigenvalues {
        // Check whether expression is a more complex soc-respresentable quadratic
        // expression as described in case 3.
        detect_soc_quadratic_complex(scip, e, success)?;
    }

    Ok(())
}

/*
 * Callback methods of nonlinear handler
 */

/// `nlhdlrCopyhdlrSoc`: nonlinear handler copy callback.
fn nlhdlr_copyhdlr_soc(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_soc(targetscip)
}

/// `nlhdlrFreehdlrdataSoc`: callback to free data of handler.
fn nlhdlr_freehdlrdata_soc(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    scip.nlhdlrs[nlhdlr].data = None;
    Ok(())
}

/// `nlhdlrFreeExprDataSoc`: callback to free expression specific data.
fn nlhdlr_free_expr_data_soc(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    free_nlhdlr_expr_data(scip, nlhdlrexprdata)
}

/// `nlhdlrDetectSoc`: callback to detect structure in expression tree.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_soc(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    cons: ConsId,
    enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    // Don't try if no sepa is required.
    // TODO implement some bound strengthening.
    if (*enforcing & NLHDLR_METHOD_SEPABOTH) == NLHDLR_METHOD_SEPABOTH {
        return Ok(());
    }

    let compeigenvalues = hd(scip, nlhdlr).compeigenvalues;

    let conslhs = if cons == NONE { INVALID } else { crate::cons_nonlinear::get_lhs_nonlinear(scip, cons) };
    let consrhs = if cons == NONE { INVALID } else { crate::cons_nonlinear::get_rhs_nonlinear(scip, cons) };

    let mut enforcebelow = false;
    let mut success = false;
    detect_soc(scip, compeigenvalues, e, conslhs, consrhs, nlhdlrexprdata, &mut enforcebelow, &mut success)?;

    if !success {
        return Ok(());
    }

    // Inform what we can do.
    *participating = if enforcebelow { NLHDLR_METHOD_SEPABELOW } else { NLHDLR_METHOD_SEPAABOVE };

    if expr::expr_is_power(scip, e) && crate::expr_pow::get_exponent_expr_pow(scip, e) == 0.5 {
        // If we have been successful on sqrt(...) <= auxvar, then we enforce.
        *enforcing |= *participating;
    } else if cons != NONE {
        // Expr is quadratic (product or sum) and we separate for expr <= ub(auxvar)
        // or expr >= lb(auxvar) only; in that case, we enforce only if expr is the
        // root of a constraint, since then replacing auxvar by up(auxvar) does not
        // relax anything (auxvar <= ub(auxvar) is the only constraint on auxvar);
        // however, if the constraint has both lhs and rhs and has only one
        // bilinear term (x*y=constant), then it seems that handling both sides by
        // nlhdlr_bilinear can still be beneficial (the latter means lhs or rhs
        // disappear, or expr is not a product and not a sum with only 1 term).
        if scip.set.is_infinity(-crate::cons_nonlinear::get_lhs_nonlinear(scip, cons)) || scip.set.is_infinity(crate::cons_nonlinear::get_rhs_nonlinear(scip, cons)) {
            *enforcing |= *participating;
        } else if !expr::expr_is_product(scip, e) && scip.exprs[e].children.len() > 1 {
            *enforcing |= *participating;
        }
    }

    Ok(())
}

/// `nlhdlrEvalauxSoc`: auxiliary evaluation callback of nonlinear handler.
/// @todo: remember if we are in the original variables and avoid
/// reevaluating.
fn nlhdlr_evalaux_soc(scip: &mut Scip, _nlhdlr: NlhdlrId, e: ExprId, nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    // If the original expression is a norm, evaluate w.r.t. the auxiliary
    // variables.
    if expr::expr_is_power(scip, e) {
        update_var_vals(scip, nlhdlrexprdata, sol, false);

        // Compute sum_i coef_i expr_i^2.
        *auxvalue = 0.0;
        let d = ed(scip, nlhdlrexprdata);
        for i in 0..d.nterms - 1 {
            let termval = eval_single_term(d, i);
            *auxvalue += termval * termval;
        }

        // Compute sqrt(sum_i coef_i expr_i^2).
        *auxvalue = auxvalue.sqrt();
    }
    // Otherwise, evaluate the original quadratic expression w.r.t. the created
    // auxvars of the children.
    else if expr::expr_is_sum(scip, e) {
        let children = scip.exprs[e].children.clone();
        let childcoefs = crate::expr_sum::get_coefs_expr_sum(scip, e).to_vec();

        *auxvalue = crate::expr_sum::get_constant_expr_sum(scip, e);

        for (i, &c) in children.iter().enumerate() {
            if expr::expr_is_power(scip, c) {
                let argauxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[c].children[0]);
                let solval = crate::api::get_sol_val(scip, sol, argauxvar);
                *auxvalue += childcoefs[i] * (solval * solval);
            } else if expr::expr_is_product(scip, c) {
                let argauxvar1 = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[c].children[0]);
                let argauxvar2 = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[c].children[1]);
                *auxvalue += childcoefs[i] * crate::api::get_sol_val(scip, sol, argauxvar1) * crate::api::get_sol_val(scip, sol, argauxvar2);
            } else {
                let argauxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, c);
                *auxvalue += childcoefs[i] * crate::api::get_sol_val(scip, sol, argauxvar);
            }
        }
    } else {
        let argauxvar1 = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[0]);
        let argauxvar2 = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, scip.exprs[e].children[1]);
        *auxvalue = crate::api::get_sol_val(scip, sol, argauxvar1) * crate::api::get_sol_val(scip, sol, argauxvar2);
    }

    Ok(())
}

/// Turns a rowprep from the initial cut generation into a row added to the
/// LP (cleanup, row, add); returns whether that is infeasible.
fn add_init_cut(scip: &mut Scip, rowprep: Option<RowPrep>, cons: ConsId, infeasible: &mut bool) -> ScipResult {
    if let Some(mut rowprep) = rowprep {
        let mut success = false;
        let huge = scip.set.get_huge_value();
        rowprep.cleanup2(scip, NONE, huge, Some(&mut success));

        if success {
            let mut cut = rowprep.get_row_cons(scip, cons)?;
            *infeasible = crate::scip_lp::add_row(scip, cut, false)?;
            crate::scip_lp::release_row(scip, &mut cut)?;
        }
    }

    Ok(())
}

/// `nlhdlrInitSepaSoc`: separation initialization method of the nonlinear
/// handler (called during CONSINITLP).
#[allow(clippy::too_many_arguments)]
fn nlhdlr_init_sepa_soc(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    _nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    _overestimate: bool,
    _underestimate: bool,
    infeasible: &mut bool,
) -> ScipResult {
    // Already needed for debug solution.
    let nvars = ed(scip, nlhdlrexprdata).nvars;
    edm(scip, nlhdlrexprdata).varvals = vec![0.0; nvars];

    let nterms = ed(scip, nlhdlrexprdata).nterms;

    // If we have 3 or more terms in lhs create variable and row for
    // disaggregation.
    if nterms > 3 {
        // Create variables for cone disaggregation.
        create_disaggr_vars(scip, e, nlhdlrexprdata)?;

        // Create the disaggregation row and store it in nlhdlrexprdata.
        create_disaggr_row(scip, conshdlr, e, nlhdlrexprdata)?;
    }

    // Add some initial cuts on well-selected coordinates.
    let ninf = -scip.set.infinity();
    if nterms == 2 {
        // We have |v_1^T x + \beta_1| \leq v_2^T x + \beta_2; we should linearize
        // at points where the first term is -1 or 1, so we can take x = v_1 /
        // ||v_1||^2 (+/-1 - beta_1).

        // Calculate ||v_1||^2.
        let mut norm = 0.0;
        {
            let d = ed(scip, nlhdlrexprdata);
            for i in d.termbegins[0] as usize..d.termbegins[1] as usize {
                norm += d.transcoefs[i] * d.transcoefs[i];
            }
        }

        edm(scip, nlhdlrexprdata).varvals.iter_mut().for_each(|v| *v = 0.0);

        let mut plusminus1 = -1.0;
        while plusminus1 <= 1.0 {
            // Set x = v_1 / ||v_1||^2 (plusminus1 - beta_1).
            {
                let d = edm(scip, nlhdlrexprdata);
                for i in d.termbegins[0] as usize..d.termbegins[1] as usize {
                    let idx = d.transcoefsidx[i] as usize;
                    d.varvals[idx] = d.transcoefs[i] / norm * (plusminus1 - d.offsets[0]);
                }
            }

            // Compute gradient cut.
            let rhsval = eval_single_term(ed(scip, nlhdlrexprdata), 1);
            let rowprep = generate_cut_sol_soc(scip, e, nlhdlrexprdata, ninf, rhsval);

            add_init_cut(scip, rowprep, cons, infeasible)?;

            if *infeasible {
                return Ok(());
            }

            plusminus1 += 2.0;
        }
    } else if nterms == 3 && ed(scip, nlhdlrexprdata).termbegins[0] != ed(scip, nlhdlrexprdata).termbegins[1] {
        // We have \sqrt{ (v_1^T x + \beta_1)^2 + (v_2^T x + \beta_2)^2 } \leq
        // v_3^T x + \beta_3 with v_1 != 0; we should linearize at points where the
        // first and second term are (-1,0), (1,1), (1,-1) (see the C for the
        // derivation of x_i and x_j).
        const REFPOINTS: [[f64; 2]; 3] = [[-1.0, 0.0], [1.0, 1.0], [1.0, -1.0]];
        let mut v1j = 0.0;
        let mut v2j = 0.0;
        let mut j: i32 = -1;

        let (i, v1i, v2zero, v2i) = {
            let d = ed(scip, nlhdlrexprdata);
            let tb = &d.termbegins;
            let i = d.transcoefsidx[tb[0] as usize];
            let v1i = d.transcoefs[tb[0] as usize];
            let v2zero = tb[1] == tb[2];

            // Get (v_2)_i; as it is a sparse vector, we need to search for i in
            // transcoefsidx.
            let mut v2i = 0.0;
            if !v2zero && let Ok(pos) = crate::sort::sortedvec_find(&d.transcoefsidx[tb[1] as usize..tb[2] as usize], crate::sort::cmp_int, i) {
                v2i = d.transcoefs[tb[1] as usize + pos];
            }
            (i, v1i, v2zero, v2i)
        };

        // Find a j, for now look only into indices with (v_2)_j != 0.
        {
            let d = ed(scip, nlhdlrexprdata);
            let tb = &d.termbegins;
            for k in tb[1] as usize..tb[2] as usize {
                // Check whether transcoefsidx[k] could be a good j; i == j is not good.
                if d.transcoefsidx[k] == i {
                    continue;
                }

                // Get (v_1)_j; as it is a sparse vector, we need to search for j in
                // transcoefsidx.
                v1j = 0.0;
                if let Ok(pos) = crate::sort::sortedvec_find(&d.transcoefsidx[tb[0] as usize..tb[1] as usize], crate::sort::cmp_int, d.transcoefsidx[k]) {
                    v1j = d.transcoefs[tb[0] as usize + pos];
                }

                v2j = d.transcoefs[k];

                // (v_2)_j - (v_2)_i (v_1)_j / (v_1)_i = 0 is also not good.
                if scip.set.is_zero(v2j - v2i * v1j / v1i) {
                    continue;
                }

                j = d.transcoefsidx[k];
                break;
            }
        }

        if v2zero {
            j = 0;
            v1j = 0.0;
            v2j = 0.0;
        }

        if j != -1 {
            edm(scip, nlhdlrexprdata).varvals.iter_mut().for_each(|v| *v = 0.0);

            let npoints = if v2zero { 2 } else { 3 };
            for point in REFPOINTS.iter().take(npoints) {
                {
                    let d = edm(scip, nlhdlrexprdata);
                    let c = point[0] - d.offsets[0];

                    if !v2zero {
                        // Set x_j and x_i.
                        let dd = point[1] - d.offsets[1];
                        d.varvals[j as usize] = (dd - v2i / v1i * c) / (v2j - v2i * v1j / v1i);
                        d.varvals[i as usize] = (c - v1j * d.varvals[j as usize]) / v1i;
                    } else {
                        // Set x_i.
                        d.varvals[i as usize] = c / v1i;
                    }
                }

                // Compute gradient cut.
                let rhsval = eval_single_term(ed(scip, nlhdlrexprdata), 2);
                let rowprep = generate_cut_sol_soc(scip, e, nlhdlrexprdata, ninf, rhsval);

                add_init_cut(scip, rowprep, cons, infeasible)?;

                if *infeasible {
                    return Ok(());
                }
            }
        }
    } else if nterms == 3 {
        // We have \sqrt{ \beta_1^2 + (v_2^T x + \beta_2)^2 } \leq v_3^T x + \beta_3
        // with v_2 != 0; we should linearize at points where the second term is -1
        // or 1; set x = v_2 / ||v_2||^2 (+/-1 - beta_2).

        // Calculate ||v_2||^2.
        let mut norm = 0.0;
        {
            let d = ed(scip, nlhdlrexprdata);
            for i in d.termbegins[1] as usize..d.termbegins[2] as usize {
                norm += d.transcoefs[i] * d.transcoefs[i];
            }
        }

        edm(scip, nlhdlrexprdata).varvals.iter_mut().for_each(|v| *v = 0.0);

        let mut plusminus1 = -1.0;
        while plusminus1 <= 1.0 {
            // Set x = v_2 / ||v_2||^2 (plusminus1 - beta_2).
            {
                let d = edm(scip, nlhdlrexprdata);
                for i in d.termbegins[1] as usize..d.termbegins[2] as usize {
                    let idx = d.transcoefsidx[i] as usize;
                    d.varvals[idx] = d.transcoefs[i] / norm * (plusminus1 - d.offsets[1]);
                }
            }

            // Compute gradient cut.
            let rhsval = eval_single_term(ed(scip, nlhdlrexprdata), 2);
            let rowprep = generate_cut_sol_soc(scip, e, nlhdlrexprdata, ninf, rhsval);

            add_init_cut(scip, rowprep, cons, infeasible)?;

            if *infeasible {
                return Ok(());
            }

            plusminus1 += 2.0;
        }
    } else {
        // Generate gradient cuts for the small rotated cones sqrt(4(v_k^T x +
        // beta_k)^2 + (v_n^T x + beta_n - y_k)^2) - v_n^T x - beta_n - y_k <= 0;
        // we should linearize again at points where the first and second term
        // (inside sqr) are (-1/2,0), (1/2,1), (1/2,-1). Since we have y_k, we can
        // achieve this more easily here via x = v_k/||v_k||^2 (+/-0.5 - beta_k),
        // y_k = v_n^T x + beta_n + 0/1/-1. If v_k = 0, then we use x = 0 and
        // linearize for second term being 1 and -1 only.
        const REFPOINTS: [[f64; 2]; 3] = [[-0.5, 0.0], [0.5, 1.0], [0.5, -1.0]];

        // Add disaggregation row to LP.
        let disrow = ed(scip, nlhdlrexprdata).disrow;
        *infeasible = crate::scip_lp::add_row(scip, disrow, false)?;

        if *infeasible {
            return Ok(());
        }

        for k in 0..nterms - 1 {
            let (vkzero, norm) = {
                let d = ed(scip, nlhdlrexprdata);
                let vkzero = d.termbegins[k + 1] == d.termbegins[k];

                // Calculate ||v_k||^2.
                let mut norm = 0.0;
                for i in d.termbegins[k] as usize..d.termbegins[k + 1] as usize {
                    norm += d.transcoefs[i] * d.transcoefs[i];
                }
                (vkzero, norm)
            };

            edm(scip, nlhdlrexprdata).varvals.iter_mut().for_each(|v| *v = 0.0);

            for point in if vkzero { 1 } else { 0 }..3 {
                // Set x = v_k / ||v_k||^2 (refpoints[point][0] - beta_k) / 2.
                {
                    let d = edm(scip, nlhdlrexprdata);
                    for i in d.termbegins[k] as usize..d.termbegins[k + 1] as usize {
                        let idx = d.transcoefsidx[i] as usize;
                        d.varvals[idx] = d.transcoefs[i] / norm * (REFPOINTS[point][0] - d.offsets[k]);
                    }
                }

                // Set y_k = v_n^T x + beta_n + 0/1/-1.
                let rhsval = eval_single_term(ed(scip, nlhdlrexprdata), nterms - 1);
                edm(scip, nlhdlrexprdata).disvarvals[k] = rhsval + REFPOINTS[point][1];

                // Compute gradient cut.
                let rowprep = generate_cut_sol_disagg(scip, e, nlhdlrexprdata, k, ninf, rhsval);

                add_init_cut(scip, rowprep, cons, infeasible)?;

                if *infeasible {
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// `nlhdlrExitSepaSoc`: separation deinitialization method of a nonlinear
/// handler (called during CONSEXITSOL).
fn nlhdlr_exit_sepa_soc(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: usize) -> ScipResult {
    // Free disaggreagation row.
    let mut disrow = ed(scip, nlhdlrexprdata).disrow;
    if disrow != NONE {
        crate::scip_lp::release_row(scip, &mut disrow)?;
        edm(scip, nlhdlrexprdata).disrow = NONE;
    }

    edm(scip, nlhdlrexprdata).varvals = Vec::new();

    Ok(())
}

/// `nlhdlrEnfoSoc`: nonlinear handler separation callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_enfo_soc(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    _auxvalue: f64,
    _overestimate: bool,
    allowweakcuts: bool,
    _separated: bool,
    _addbranchscores: bool,
    branchcandonly: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    if branchcandonly {
        return Ok(());
    }

    let mincutefficacy = hd(scip, nlhdlr).mincutefficacy;

    // Update varvals; set variables close to integer to integer, in particular
    // when close to zero; for simple soc's (no large v_i, no offsets), variables
    // close to zero would give coefficients close to zero in the cut, which the
    // cut cleanup may have problems to relax (and we end up with local or much
    // relaxed cuts); also when close to other integers, rounding now may
    // prevent some relaxation in cut cleanup.
    update_var_vals(scip, nlhdlrexprdata, sol, true);

    let nterms = ed(scip, nlhdlrexprdata).nterms;
    let rhsval = eval_single_term(ed(scip, nlhdlrexprdata), nterms - 1);
    let lpfeastol = crate::lp::lp_get_feastol(scip);

    // If there are three or two terms just compute gradient cut.
    if nterms < 4 {
        // Compute gradient cut.
        if let Some(mut rowprep) = generate_cut_sol_soc(scip, e, nlhdlrexprdata, lpfeastol, rhsval) {
            add_cut(scip, mincutefficacy, &mut rowprep, sol, cons, allowweakcuts, result)?;
        }

        return Ok(());
    }

    let ndisaggrs = nterms - 1;

    // Check whether the aggregation row is in the LP.
    let disrow = ed(scip, nlhdlrexprdata).disrow;
    if scip.rows[disrow].lppos < 0 && -crate::scip_lp::get_row_sol_feasibility(scip, disrow, sol) > crate::lp::lp_get_feastol(scip) {
        let infeasible = crate::scip_lp::add_row(scip, disrow, true)?;

        if infeasible {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }

        *result = ScipResultCode::Separated;
    }

    let mut k = 0;
    while k < ndisaggrs && *result != ScipResultCode::Cutoff {
        // Compute gradient cut.
        if let Some(mut rowprep) = generate_cut_sol_disagg(scip, e, nlhdlrexprdata, k, lpfeastol, rhsval) {
            add_cut(scip, mincutefficacy, &mut rowprep, sol, cons, allowweakcuts, result)?;
        }

        k += 1;
    }

    Ok(())
}

/// `nlhdlrSollinearizeSoc`: solution linearization callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_sollinearize_soc(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    _nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    _solisbest: bool,
    _overestimate: bool,
    _underestimate: bool,
) -> ScipResult {
    // Update varvals.
    update_var_vals(scip, nlhdlrexprdata, sol, true);

    let nterms = ed(scip, nlhdlrexprdata).nterms;
    let rhsval = eval_single_term(ed(scip, nlhdlrexprdata), nterms - 1);
    let ninf = -scip.set.infinity();

    // If there are three or two terms just compute gradient cut.
    if nterms < 4 {
        // Compute gradient cut.
        if let Some(mut rowprep) = generate_cut_sol_soc(scip, e, nlhdlrexprdata, ninf, rhsval) {
            add_cut_pool(scip, &mut rowprep, sol, cons)?;
        }

        return Ok(());
    }

    for k in 0..nterms - 1 {
        // Compute gradient cut.
        if let Some(mut rowprep) = generate_cut_sol_disagg(scip, e, nlhdlrexprdata, k, ninf, rhsval) {
            add_cut_pool(scip, &mut rowprep, sol, cons)?;
        }
    }

    Ok(())
}

/*
 * nonlinear handler specific interface methods
 */

/// `SCIPincludeNlhdlrSoc`: includes SOC nonlinear handler in nonlinear
/// constraint handler.
pub fn include_nlhdlr_soc(scip: &mut Scip) -> ScipResult {
    // Create nonlinear handler data.
    let nlhdlrdata = NlhdlrData {
        mincutefficacy: 0.0,
        compeigenvalues: false,
    };

    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        NLHDLR_NAME,
        Some(NLHDLR_DESC),
        NLHDLR_DETECTPRIORITY,
        NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_soc,
        nlhdlr_evalaux_soc,
        Some(Box::new(nlhdlrdata)),
    )?;

    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_soc));
    nlhdlr::nlhdlr_set_free_hdlr_data(scip, nlhdlr, Some(nlhdlr_freehdlrdata_soc));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_free_expr_data_soc));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, Some(nlhdlr_init_sepa_soc), Some(nlhdlr_enfo_soc), None, Some(nlhdlr_exit_sepa_soc));
    nlhdlr::nlhdlr_set_sollinearize(scip, nlhdlr, Some(nlhdlr_sollinearize_soc));

    // Add soc nlhdlr parameters.
    // TODO should we get rid of this and use separating/mineffiacy(root)
    // instead, which is 1e-4?
    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, nlhdlrs, nlhdlr, data, NlhdlrData, $f)
        };
    }
    let infinity = scip.set.infinity();
    crate::paramset::paramset_add_real(
        scip,
        "nlhdlr/soc/mincutefficacy",
        "Minimum efficacy which a cut needs in order to be added.",
        t!(f64, mincutefficacy),
        false,
        DEFAULT_MINCUTEFFICACY,
        0.0,
        infinity,
        None,
    )?;
    crate::paramset::paramset_add_bool(
        scip,
        "nlhdlr/soc/compeigenvalues",
        "Should Eigenvalue computations be done to detect complex cases in quadratic constraints?",
        t!(bool, compeigenvalues),
        false,
        DEFAULT_COMPEIGENVALUES,
        None,
    )?;

    Ok(())
}
