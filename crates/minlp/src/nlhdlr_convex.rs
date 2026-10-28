//! Port of `nlhdlr_convex.c`: nonlinear handlers for convex and concave
//! expressions.

use std::collections::{HashMap, HashSet};

use crate::{
    def::INVALID,
    expr::{self, ExprCurv, ExprId, Monotone},
    exprcurv,
    misc::RandNumGen,
    nlhdlr::{self, NLHDLR_METHOD_SEPAABOVE, NLHDLR_METHOD_SEPABELOW, NLHDLR_METHOD_SEPABOTH, NlhdlrId, NlhdlrMethod},
    quad,
    retcode::ScipResult,
    rowprep::RowPrep,
    scip::Scip,
    types::{BoundType, ConsId, ConshdlrId, ImplintType, SideType, SolId, VarId},
    var::NONE,
};

/* fundamental nonlinear handler properties */
const CONVEX_NLHDLR_NAME: &str = "convex";
const CONVEX_NLHDLR_DESC: &str = "handler that identifies and estimates convex expressions";
const CONVEX_NLHDLR_DETECTPRIORITY: i32 = 50;
const CONVEX_NLHDLR_ENFOPRIORITY: i32 = 50;

const CONCAVE_NLHDLR_NAME: &str = "concave";
const CONCAVE_NLHDLR_DESC: &str = "handler that identifies and estimates concave expressions";
const CONCAVE_NLHDLR_DETECTPRIORITY: i32 = 40;
const CONCAVE_NLHDLR_ENFOPRIORITY: i32 = 40;

const DEFAULT_DETECTSUM: bool = false;
const DEFAULT_EXTENDEDFORM: bool = true;
const DEFAULT_CVXQUADRATIC_CONVEX: bool = true;
const DEFAULT_CVXQUADRATIC_CONCAVE: bool = false;
const DEFAULT_CVXSIGNOMIAL: bool = true;
const DEFAULT_CVXPRODCOMP: bool = true;
const DEFAULT_HANDLETRIVIAL: bool = false;
const DEFAULT_MAXPERTURB: f64 = 0.01;

/// Maximal absolute value of variable for still generating a linearization
/// cut at that point in initlp.
const INITLPMAXVARVAL: f64 = 1000.0;
/// Initial seed for random number generator for point perturbation.
const RANDNUMINITSEED: u32 = 220802;

/*
 * Data structures
 */

/// `SCIP_NLHDLREXPRDATA`: nonlinear handler expression data.
struct ExprData {
    /// Expression (copy) for which this nlhdlr estimates.
    nlexpr: ExprId,
    /// Mapping of our copied expression to original expression.
    nlexpr2origexpr: HashMap<ExprId, ExprId>,
    /// Number of distinct leafs of nlexpr, i.e., number of distinct
    /// (auxiliary) variables handled.
    nleafs: i32,
    /// Distinct leaf expressions (excluding value-expressions), thus
    /// variables.
    leafexprs: Vec<ExprId>,
}

/// `SCIP_NLHDLRDATA`: nonlinear handler data.
struct NlhdlrData {
    /// Whether this data is used for the convex nlhdlr (TRUE) or the concave
    /// one (FALSE).
    isnlhdlrconvex: bool,
    /// Solution used for evaluating expression in a different point, e.g., for
    /// facet computation of vertex-polyhedral function.
    evalsol: SolId,
    /// Random number generator used to perturb reference point in
    /// estimateGradient().
    randnumgen: Option<RandNumGen>,

    /* parameters */
    /// Whether to run detection when the root of an expression is a
    /// non-quadratic sum.
    detectsum: bool,
    /// Whether to create extended formulations instead of looking for maximal
    /// possible subexpression.
    extendedform: bool,
    /// Maximal perturbation of non-differentiable reference point.
    maxperturb: f64,

    /* advanced parameters (maybe remove some day) */
    /// Whether to use convexity check on quadratics.
    cvxquadratic: bool,
    /// Whether to use convexity check on signomials.
    cvxsignomial: bool,
    /// Whether to use convexity check on product composition f(h)*h.
    cvxprodcomp: bool,
    /// Whether to handle trivial expressions, i.e., those where all children
    /// are variables.
    handletrivial: bool,
}

/// The parts of [`NlhdlrData`] the detection reads (C passes the whole
/// data, and `SCIPhasExprCurvature` one on the stack).
#[derive(Clone, Copy)]
struct DetectData {
    isnlhdlrconvex: bool,
    detectsum: bool,
    extendedform: bool,
    cvxquadratic: bool,
    cvxsignomial: bool,
    cvxprodcomp: bool,
    handletrivial: bool,
}

impl NlhdlrData {
    fn detect_data(&self) -> DetectData {
        DetectData {
            isnlhdlrconvex: self.isnlhdlrconvex,
            detectsum: self.detectsum,
            extendedform: self.extendedform,
            cvxquadratic: self.cvxquadratic,
            cvxsignomial: self.cvxsignomial,
            cvxprodcomp: self.cvxprodcomp,
            handletrivial: self.handletrivial,
        }
    }
}

/// `DECL_CURVCHECK`: (nlexpr, isrootexpr, stack, nlexpr2origexpr,
/// nlhdlrdata, assumevarfixed, success).
type CurvCheck = fn(&mut Scip, ExprId, bool, &mut Vec<ExprId>, &mut HashMap<ExprId, ExprId>, &DetectData, Option<&HashMap<VarId, ()>>, &mut bool) -> ScipResult;

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

#[inline]
fn hdm(scip: &mut Scip, nlhdlr: NlhdlrId) -> &mut NlhdlrData {
    nlhdlr::hdlrdata_mut::<NlhdlrData>(scip, nlhdlr)
}

/// C's `MAX(a, b)`: `a >= b ? a : b`.
#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// C's `MIN(a, b)`: `a <= b ? a : b`.
#[inline]
fn cmin(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}

/// Whether `expr` belongs to the expression handler `name` (for handlers
/// the port does not include).
fn is_expr_named(scip: &Scip, expr: ExprId, name: &str) -> bool {
    scip.exprhdlrs[scip.exprs[expr].exprhdlr].name == name
}

/*
 * static methods
 */

/// `nlhdlrExprCreate`: create nlhdlr-expression; does not create children,
/// i.e., assumes that this will be a leaf.
fn nlhdlr_expr_create(scip: &mut Scip, nlexpr2origexpr: &mut HashMap<ExprId, ExprId>, origexpr: ExprId, curv: ExprCurv) -> ScipResult<ExprId> {
    if scip.exprs[origexpr].children.is_empty() {
        // For leaves, do not copy.
        let nlhdlrexpr = origexpr;
        expr::expr_capture(scip, nlhdlrexpr);
        nlexpr2origexpr.entry(nlhdlrexpr).or_insert(origexpr);
        return Ok(nlhdlrexpr);
    }

    // Create copy of expression, but without children.
    let nlhdlrexpr = expr::expr_duplicate_shallow(scip, origexpr, None, 0)?;

    // Store the curvature we want to get in the curvature flag of the copied
    // expression; it's a bit of a misuse, but once we are done with
    // everything, this is actually correct.
    scip.exprs[nlhdlrexpr].curvature = curv;

    // Remember which the original expression was.
    nlexpr2origexpr.insert(nlhdlrexpr, origexpr);

    Ok(nlhdlrexpr)
}

/// `nlhdlrExprGrowChildren`: expand nlhdlr-expression by adding children
/// according to original expression.
fn nlhdlr_expr_grow_children(scip: &mut Scip, nlexpr2origexpr: &mut HashMap<ExprId, ExprId>, nlhdlrexpr: ExprId, childrencurv: Option<&[ExprCurv]>) -> ScipResult {
    let origexpr = nlexpr2origexpr[&nlhdlrexpr];

    let origchildren = scip.exprs[origexpr].children.clone();
    if origchildren.is_empty() {
        return Ok(());
    }

    for (i, &origchild) in origchildren.iter().enumerate() {
        let mut child = nlhdlr_expr_create(scip, nlexpr2origexpr, origchild, childrencurv.map_or(ExprCurv::Unknown, |c| c[i]))?;
        expr::expr_append_child(scip, nlhdlrexpr, child);
        // Append captures child, so we can release the capture from
        // nlhdlrExprCreate.
        expr::expr_release(scip, &mut child)?;
    }

    Ok(())
}

/// `nlhdlrExprEvalConcave`: evaluate expression at solution w.r.t.
/// auxiliary variables.
fn nlhdlr_expr_eval_concave(scip: &mut Scip, nlhdlrexprdata: usize, evalsol: SolId, args: &[f64]) -> f64 {
    for (i, &a) in args.iter().enumerate() {
        let leaf = ed(scip, nlhdlrexprdata).leafexprs[i];
        let var = crate::expr_var::get_var_expr_var(scip, leaf);
        crate::api::set_sol_val(scip, evalsol, var, a).expect("SCIPsetSolVal");
    }

    let nlexpr = ed(scip, nlhdlrexprdata).nlexpr;
    expr::expr_eval(scip, nlexpr, evalsol, 0).expect("SCIPevalExpr");

    scip.exprs[nlexpr].evalvalue
}

/// `curvCheckQuadratic`: looks whether given expression is (proper)
/// quadratic and has a given curvature.
///
/// If having a given curvature, currently require all arguments of
/// quadratic to be linear. Hence, not using this for a simple square term,
/// as curvCheckExprhdlr may provide a better condition on argument
/// curvature then. Also we wouldn't do anything useful for a single
/// bilinear term. Thus, run on sum's only.
#[allow(clippy::too_many_arguments)]
fn curv_check_quadratic(
    scip: &mut Scip,
    nlexpr: ExprId,
    isrootexpr: bool,
    stack: &mut Vec<ExprId>,
    nlexpr2origexpr: &mut HashMap<ExprId, ExprId>,
    nlhdlrdata: &DetectData,
    assumevarfixed: Option<&HashMap<VarId, ()>>,
    success: &mut bool,
) -> ScipResult {
    *success = false;

    if !nlhdlrdata.cvxquadratic {
        return Ok(());
    }

    if !expr::expr_is_sum(scip, nlexpr) {
        return Ok(());
    }

    let wantedcurv = scip.exprs[nlexpr].curvature;
    if wantedcurv == ExprCurv::Linear {
        return Ok(());
    }

    let e = nlexpr2origexpr[&nlexpr];

    // Check whether quadratic.
    let isquadratic = expr::expr_check_quadratic(scip, e)?;

    // If not quadratic, then give up here.
    if !isquadratic {
        return Ok(());
    }

    let (nquadexprs, nbilinexprs) = {
        let q = expr::expr_get_quadratic_data(scip, e);
        (q.quadexprterms.len(), q.bilinexprterms.len())
    };

    // If only single square term (+linear), then give up here (let
    // curvCheckExprhdlr handle this).
    if nquadexprs <= 1 {
        return Ok(());
    }

    // If root expression is only sum of squares (+linear) and detectsum is
    // disabled, then give up here, too.
    if isrootexpr && !nlhdlrdata.detectsum && nbilinexprs == 0 {
        return Ok(());
    }

    // Get curvature of quadratic.
    // TODO as we know what curvature we want, we could first do some simple
    // checks like computing xQx for a random x.
    let presentcurv = expr::expr_compute_quadratic_curvature(scip, e, assumevarfixed, false)?;

    // If not having desired curvature, return.
    if presentcurv != wantedcurv {
        return Ok(());
    }

    *success = true;

    let mut lonelysquares: Option<HashSet<ExprId>> = None;
    if !nlhdlrdata.detectsum {
        // First step towards block-decomposition of quadratic term: collect all
        // square-expressions (in original expr) which have no adjacent bilinear
        // term; we will treat these x^2 as linear, i.e., add an auxvar for them,
        // so x^2 maybe linearized more efficiently (in particular if x is
        // discrete).
        let mut ls = HashSet::with_capacity(nquadexprs);
        for t in &expr::expr_get_quadratic_data(scip, e).quadexprterms {
            if t.nadjbilin == 0 {
                ls.insert(t.sqrexpr);
            }
        }
        lonelysquares = Some(ls);
    }

    // Add immediate children to nlexpr.
    nlhdlr_expr_grow_children(scip, nlexpr2origexpr, nlexpr, None)?;

    // Put children that are not square or product on stack; grow child for
    // children that are square or product and put this child on stack;
    // require all children to be linear.
    for i in 0..scip.exprs[nlexpr].children.len() {
        let curvlinear = [ExprCurv::Linear, ExprCurv::Linear];

        let child = scip.exprs[nlexpr].children[i];
        let origchild = scip.exprs[e].children[i];

        if expr::expr_is_power(scip, child) && crate::expr_pow::get_exponent_expr_pow(scip, child) == 2.0 && lonelysquares.as_ref().is_none_or(|ls| !ls.contains(&origchild)) {
            // Square term that isn't lonely, i.e., orig-version of child is a
            // square-expr and nadjbilin>0.
            nlhdlr_expr_grow_children(scip, nlexpr2origexpr, child, Some(&curvlinear))?;
            stack.extend_from_slice(&scip.exprs[child].children.clone());
        } else if expr::expr_is_product(scip, child) && scip.exprs[origchild].children.len() == 2 {
            // Using original version of child here as NChildren(child)==0 atm.
            // Bilinear term.
            nlhdlr_expr_grow_children(scip, nlexpr2origexpr, child, Some(&curvlinear))?;
            stack.extend_from_slice(&scip.exprs[child].children.clone());
        } else {
            // Linear term (or term to be considered as linear) or lonely square
            // term; if we want extended formulations, then require linearity, so
            // an auxvar will be introduced if it is nonlinear; if we do not want
            // extended formulations, then the term needs to have curvature
            // "wantedcurv"; thus, if the coef is negative, then the child needs to
            // have the curvature opposite to "wantedcurv".
            if nlhdlrdata.extendedform {
                scip.exprs[child].curvature = ExprCurv::Linear;
            } else {
                let coef = crate::expr_sum::get_coefs_expr_sum(scip, nlexpr)[i];
                scip.exprs[child].curvature = exprcurv::exprcurv_multiply(coef, wantedcurv);
            }
            stack.push(child);
        }
    }

    Ok(())
}

/// `curvCheckSignomial`: looks whether top of given expression looks like a
/// signomial that can have a given curvature, e.g., sqrt(x)*sqrt(y) is
/// convex if x,y >= 0 and x and y are convex.
#[allow(clippy::too_many_arguments)]
fn curv_check_signomial(
    scip: &mut Scip,
    nlexpr: ExprId,
    _isrootexpr: bool,
    stack: &mut Vec<ExprId>,
    nlexpr2origexpr: &mut HashMap<ExprId, ExprId>,
    nlhdlrdata: &DetectData,
    _assumevarfixed: Option<&HashMap<VarId, ()>>,
    success: &mut bool,
) -> ScipResult {
    *success = false;

    if !nlhdlrdata.cvxsignomial {
        return Ok(());
    }

    if !expr::expr_is_product(scip, nlexpr) {
        return Ok(());
    }

    let e = nlexpr2origexpr[&nlexpr];
    let nfactors = scip.exprs[e].children.len();
    // Boooring.
    if nfactors <= 1 {
        return Ok(());
    }

    let mut exponents = vec![0.0; nfactors];
    let mut bounds = vec![crate::interval::Interval::default(); nfactors];
    let mut curv = vec![ExprCurv::Unknown; nfactors];

    for i in 0..nfactors {
        let child = scip.exprs[e].children[i];

        if !expr::expr_is_power(scip, child) {
            exponents[i] = 1.0;
            expr::expr_eval_activity(scip, child)?;
            bounds[i] = scip.exprs[child].activity;
        } else {
            exponents[i] = crate::expr_pow::get_exponent_expr_pow(scip, child);
            let c0 = scip.exprs[child].children[0];
            expr::expr_eval_activity(scip, c0)?;
            bounds[i] = scip.exprs[c0].activity;
        }
    }

    let coef = crate::expr_product::get_coef_expr_product(scip, e);
    if !exprcurv::exprcurv_monomial_inv(
        exprcurv::exprcurv_multiply(coef, scip.exprs[nlexpr].curvature),
        nfactors,
        Some(&exponents),
        &bounds,
        &mut curv,
    ) {
        return Ok(());
    }

    // Add immediate children to nlexpr; some entries in curv actually apply to
    // arguments of pow's, will correct this next.
    nlhdlr_expr_grow_children(scip, nlexpr2origexpr, nlexpr, Some(&curv))?;

    // Put children that are not power on stack; grow child for children that
    // are power and put this child on stack; if extendedform, then require
    // children to be linear; unless they are linear, an auxvar will be
    // introduced for them and thus they will be handled as var here.
    for i in 0..nfactors {
        let mut child = scip.exprs[nlexpr].children[i];

        if expr::expr_is_power(scip, child) {
            nlhdlr_expr_grow_children(scip, nlexpr2origexpr, child, Some(&curv[i..i + 1]))?;
            child = scip.exprs[child].children[0];
        }

        if nlhdlrdata.extendedform {
            scip.exprs[child].curvature = ExprCurv::Linear;
        }

        stack.push(child);
    }

    *success = true;

    Ok(())
}

/// `curvCheckProductComposite`: looks for f(c h(x)+d) h(x) * constant and
/// tries to conclude conditions on curvature (see the C for the
/// derivation).
#[allow(clippy::too_many_arguments)]
fn curv_check_product_composite(
    scip: &mut Scip,
    nlexpr: ExprId,
    _isrootexpr: bool,
    stack: &mut Vec<ExprId>,
    nlexpr2origexpr: &mut HashMap<ExprId, ExprId>,
    nlhdlrdata: &DetectData,
    _assumevarfixed: Option<&HashMap<VarId, ()>>,
    success: &mut bool,
) -> ScipResult {
    *success = false;

    if !nlhdlrdata.cvxprodcomp {
        return Ok(());
    }

    if !expr::expr_is_product(scip, nlexpr) {
        return Ok(());
    }

    let e = nlexpr2origexpr[&nlexpr];
    if scip.exprs[e].children.len() != 2 {
        return Ok(());
    }

    // Check whether we have f(c * h(x)) * h(x) or h(x) * f(c * h(x)).
    let mut f = NONE;
    let mut h = NONE;
    let mut c = 0.0;
    let mut ch = NONE;
    let mut fidx = 0;
    while fidx <= 1 {
        f = scip.exprs[e].children[fidx];
        if scip.exprs[f].children.len() != 1 {
            fidx += 1;
            continue;
        }

        ch = scip.exprs[f].children[0];
        c = 1.0;
        h = ch;

        // Check whether ch is of the form c*h(x), then switch h to child ch.
        if expr::expr_is_sum(scip, ch) && scip.exprs[ch].children.len() == 1 {
            c = crate::expr_sum::get_coefs_expr_sum(scip, ch)[0];
            h = scip.exprs[ch].children[0];
        }

        // Can assume that duplicate subexpressions have been identified and
        // comparing pointer is sufficient.
        if scip.exprs[e].children[1 - fidx] == h {
            break;
        }
        fidx += 1;
    }
    if fidx == 2 {
        return Ok(());
    }

    // Constant of c*h(x)+d.
    let d = if h != ch { crate::expr_sum::get_constant_expr_sum(scip, ch) } else { 0.0 };

    expr::expr_eval_activity(scip, f)?;
    expr::expr_eval_activity(scip, h)?;
    let fbounds = scip.exprs[f].activity;
    let hbounds = scip.exprs[h].activity;

    // If h has mixed sign, then cannot conclude anything.
    if hbounds.inf < 0.0 && hbounds.sup > 0.0 {
        return Ok(());
    }

    // If we have some convex or concave x*abs(c*x+d), then gradients at x=-d/c
    // may be very wrong due to rounding errors and non-differentiability of
    // abs() at zero (#3411). Therefore, we skip handling such expression in
    // this nonlinear handler when one of the bounds of c*x+d is very close to
    // zero. (If zero is in between the bounds of c*x+d, then the composition
    // wouldn't be regarded as convex/concave anyway.)
    if is_expr_named(scip, f, "abs") && (scip.set.is_zero(c * hbounds.inf + d) || scip.set.is_zero(c * hbounds.sup + d)) {
        return Ok(());
    }

    let fh = scip.exprs[f].exprhdlr;
    let mut fmonotonicity = Monotone::Unknown;
    expr::exprhdlr_monotonicity_expr(scip, fh, f, 0, &mut fmonotonicity)?;

    // If f is not monotone, then cannot conclude anything.
    if fmonotonicity == Monotone::Unknown {
        return Ok(());
    }

    // Curvature we want to achieve (negate if product has negative coef).
    let desiredcurv = exprcurv::exprcurv_multiply(crate::expr_product::get_coef_expr_product(scip, nlexpr), scip.exprs[nlexpr].curvature);

    // Now check the conditions as stated above.
    let mut dummy = [ExprCurv::Unknown];
    let mut hcurv;
    if desiredcurv == ExprCurv::Convex {
        // f(c h(x)+d)h(x) is convex if c*f is monotonically increasing (c f' >=
        // 0) and either
        //   - f is convex (f'' >= 0) and h is nonnegative (h >= 0) and h is
        //     convex (h'' >= 0) and [f is nonnegative (f >= 0) or h is linear
        //     (h''=0)], or
        //   - f is concave (f'' <= 0) and h is nonpositive (h <= 0) and h is
        //     concave (h'' <= 0) and [f is nonpositive (f <= 0) or h is linear
        //     (h''=0)]
        // as the curvature requirements on f are on f only and not the
        // composition f(h), we can ignore the requirements returned by
        // SCIPcallExprCurvature (last arg).
        if (c > 0.0 && fmonotonicity != Monotone::Inc) || (c < 0.0 && fmonotonicity != Monotone::Dec) {
            return Ok(());
        }

        // Check whether f can be convex (h>=0) or concave (h<=0), resp., and
        // derive requirements for h.
        if hbounds.inf >= 0.0 {
            expr::exprhdlr_curvature_expr(scip, fh, f, ExprCurv::Convex, success, &mut dummy)?;

            // Now h also needs to be convex; and if f < 0, then h actually needs
            // to be linear.
            hcurv = if fbounds.inf < 0.0 { ExprCurv::Linear } else { ExprCurv::Convex };
        } else {
            expr::exprhdlr_curvature_expr(scip, fh, f, ExprCurv::Concave, success, &mut dummy)?;

            // Now h also needs to be concave; and if f > 0, then h actually needs
            // to be linear.
            hcurv = if fbounds.sup > 0.0 { ExprCurv::Linear } else { ExprCurv::Concave };
        }
    } else {
        // f(c h(x)+d)*h(x) is concave if c*f is monotonically decreasing (c f'
        // <= 0) and either
        //   - f is convex (f'' >= 0) and h is nonpositive (h <= 0) and h is
        //     concave (h'' <= 0) and [f is nonnegative (f >= 0) or h is linear
        //     (h''=0)], or
        //   - f is concave (f'' <= 0) and h is nonnegative (h >= 0) and h is
        //     convex (h'' >= 0) and [f is nonpositive (f <= 0) or h is linear
        //     (h''=0)]
        // as the curvature requirements on f are on f only and not the
        // composition f(h), we can ignore the requirements returned by
        // SCIPcallExprCurvature (last arg).
        if (c > 0.0 && fmonotonicity != Monotone::Dec) || (c < 0.0 && fmonotonicity != Monotone::Inc) {
            return Ok(());
        }

        // Check whether f can be convex (h<=0) or concave (h>=0), resp., and
        // derive requirements for h.
        if hbounds.sup <= 0.0 {
            expr::exprhdlr_curvature_expr(scip, fh, f, ExprCurv::Convex, success, &mut dummy)?;

            // Now h also needs to be concave; and if f < 0, then h actually needs
            // to be linear.
            hcurv = if fbounds.inf < 0.0 { ExprCurv::Linear } else { ExprCurv::Concave };
        } else {
            expr::exprhdlr_curvature_expr(scip, fh, f, ExprCurv::Concave, success, &mut dummy)?;

            // Now h also needs to be convex; and if f > 0, then h actually needs
            // to be linear.
            hcurv = if fbounds.sup > 0.0 { ExprCurv::Linear } else { ExprCurv::Convex };
        }
    }

    if !*success {
        return Ok(());
    }

    // Add immediate children (f and ch) to nlexpr; we set required curvature
    // for h further below.
    nlhdlr_expr_grow_children(scip, nlexpr2origexpr, nlexpr, None)?;

    // Push this h onto stack for further checking.
    let hcopy = scip.exprs[nlexpr].children[1 - fidx];
    stack.push(hcopy);

    // If we prefer extended formulations, then we always want h() to be
    // linear.
    if nlhdlrdata.extendedform {
        hcurv = ExprCurv::Linear;
    }

    // h-child of product should have curvature hcurv.
    scip.exprs[hcopy].curvature = hcurv;

    let fcopy = scip.exprs[nlexpr].children[fidx];
    if h != ch {
        // Add copy of ch as child to copy of f.
        nlhdlr_expr_grow_children(scip, nlexpr2origexpr, fcopy, None)?;

        // Add copy of h (created above as child of product) as child in copy of
        // ch.
        let chcopy = scip.exprs[fcopy].children[0];
        expr::expr_append_child(scip, chcopy, hcopy);
    } else {
        // Add copy of h (created above as child of product) as child in copy of
        // f.
        expr::expr_append_child(scip, fcopy, hcopy);
    }

    Ok(())
}

/// `curvCheckExprhdlr`: use expression handlers curvature callback to check
/// whether given curvature can be achieved.
#[allow(clippy::too_many_arguments)]
fn curv_check_exprhdlr(
    scip: &mut Scip,
    nlexpr: ExprId,
    isrootexpr: bool,
    stack: &mut Vec<ExprId>,
    nlexpr2origexpr: &mut HashMap<ExprId, ExprId>,
    nlhdlrdata: &DetectData,
    _assumevarfixed: Option<&HashMap<VarId, ()>>,
    success: &mut bool,
) -> ScipResult {
    let origexpr = nlexpr2origexpr[&nlexpr];
    let nchildren = scip.exprs[origexpr].children.len();
    let oh = scip.exprs[origexpr].exprhdlr;
    let wanted = scip.exprs[nlexpr].curvature;

    if nchildren == 0 {
        // If originally no children, then should be var or value, which should
        // have every curvature, so should always be success.
        expr::exprhdlr_curvature_expr(scip, oh, origexpr, wanted, success, &mut [])?;
        return Ok(());
    }

    *success = false;

    // Ignore sums if > 1 children.
    // NOTE: this means that for something like 1+f(x), even if f is a trivial
    // convex expression, we would handle 1+f(x) with this nlhdlr, instead of
    // formulating this as 1+z and handling z=f(x) with the default nlhdlr,
    // i.e., the exprhdlr; today, I prefer handling this here, as it avoids
    // introducing an extra auxiliary variable.
    if isrootexpr && !nlhdlrdata.detectsum && expr::expr_is_sum(scip, nlexpr) && nchildren > 1 {
        return Ok(());
    }

    let mut childcurv = vec![ExprCurv::Unknown; nchildren];

    // Check whether and under which conditions origexpr can have desired
    // curvature.
    expr::exprhdlr_curvature_expr(scip, oh, origexpr, wanted, success, &mut childcurv)?;

    if !*success {
        return Ok(());
    }

    // If origexpr can have curvature curv, then don't treat it as leaf, but
    // include its children.
    nlhdlr_expr_grow_children(scip, nlexpr2origexpr, nlexpr, Some(&childcurv))?;

    // If we prefer extended formulations, then require all children to be
    // linear. Unless they are, auxvars will be introduced and they will be
    // handles as variables, which can be an advantage in the context of
    // extended formulations.
    if nlhdlrdata.extendedform {
        for i in 0..nchildren {
            let c = scip.exprs[nlexpr].children[i];
            scip.exprs[c].curvature = ExprCurv::Linear;
        }
    }

    // Add children expressions to to-do list (stack).
    stack.extend_from_slice(&scip.exprs[nlexpr].children.clone());

    Ok(())
}

/// `CURVCHECKS`: curvature check and expression-growing methods
/// (curvCheckExprhdlr should be last).
const CURVCHECKS: [CurvCheck; 4] = [curv_check_product_composite, curv_check_signomial, curv_check_quadratic, curv_check_exprhdlr];

/// `exprIsMultivarLinear`: checks whether expression is a sum with more
/// than one child and each child being a variable or going to be a variable
/// if `expr` is a nlhdlr-specific copy.
///
/// Within constructExpr(), we can have an expression of any type which is a
/// copy of an original expression, but without children. At the end of
/// constructExpr() (after the loop with the stack), these expressions will
/// remain as leafs and will eventually be turned into variables in
/// collectLeafs(). Thus, we treat every child that has no children as if it
/// were a variable. Theoretically, there is still the possibility that it
/// could be a constant (value-expression), but simplify should have removed
/// these.
fn expr_is_multivar_linear(scip: &Scip, e: ExprId) -> bool {
    if !expr::expr_is_sum(scip, e) {
        return false;
    }

    let children = &scip.exprs[e].children;
    if children.len() <= 1 {
        return false;
    }

    for &c in children {
        if !scip.exprs[c].children.is_empty() {
            return false;
        }
    }

    true
}

/// `constructExpr`: constructs a subexpression (as nlhdlr-expression) of
/// maximal size that has a given curvature.
///
/// If the curvature cannot be achieved for an expression in the original
/// expression graph, then this expression becomes a leaf in the
/// nlhdlr-expression. Returns `NONE` if failed.
#[allow(clippy::too_many_arguments)]
fn construct_expr(
    scip: &mut Scip,
    nlhdlrdata: &DetectData,
    nlexpr2origexpr: &mut HashMap<ExprId, ExprId>,
    nleafs: &mut i32,
    rootexpr: ExprId,
    curv: ExprCurv,
    assumevarfixed: Option<&HashMap<VarId, ()>>,
    assumecurvature: bool,
    mut curvsuccess: Option<&mut bool>,
) -> ScipResult<ExprId> {
    let mut isrootexpr = true;

    // Create root expression.
    let mut rootnlexpr = nlhdlr_expr_create(scip, nlexpr2origexpr, rootexpr, curv)?;

    *nleafs = 0;
    if let Some(s) = curvsuccess.as_deref_mut() {
        *s = true;
    }

    // To do list: expressions where to check whether they can have the
    // desired curvature when taking their children into account.
    let mut stack: Vec<ExprId> = Vec::with_capacity(20);
    stack.push(rootnlexpr);
    while let Some(nlexpr) = stack.pop() {
        // Take expression from stack.
        let oldstackpos = stack.len();

        let nh = scip.exprs[nlexpr].exprhdlr;
        if nlhdlrdata.isnlhdlrconvex && scip.exprhdlrs[nh].bwdiff.is_none() {
            // If bwdiff is not implemented, then we could not generate cuts in the
            // convex nlhdlr, so "stop" (treat nlexpr as variable).
        } else if !nlhdlrdata.isnlhdlrconvex && expr_is_multivar_linear(scip, nlexpr2origexpr[&nlexpr]) {
            // If we are in the concave handler, we would like to treat linear
            // multivariate subexpressions by a new auxvar always, e.g., handle
            // log(x+y) as log(z), z=x+y, because the estimation problem will be
            // smaller then without making the estimator worse (cons_nonlinear does
            // this, too); this check takes care of this when x and y are original
            // variables; however, it isn't unlikely that we will have sums that
            // become linear after we add auxvars for some children; this will be
            // handled in a postprocessing below; for now, the check is performed
            // on the original expression since there is not enough information in
            // nlexpr yet.
        } else if scip.exprs[nlexpr].curvature != ExprCurv::Unknown && !assumecurvature {
            // If we are here, either convexity or concavity is required; try to
            // check for this curvature.
            let mut success = false;

            // Try through curvature check methods until one succeeds.
            for method in CURVCHECKS {
                method(scip, nlexpr, isrootexpr, &mut stack, nlexpr2origexpr, nlhdlrdata, assumevarfixed, &mut success)?;
                if success {
                    break;
                }
            }
        } else {
            // If we don't care about curvature in this subtree anymore (very
            // unlikely), or we are told to assume that the desired curvature is
            // present (assumecurvature==TRUE), then only continue iterating this
            // subtree to assemble leaf expressions.
            nlhdlr_expr_grow_children(scip, nlexpr2origexpr, nlexpr, None)?;

            // Add children expressions, if any, to to-do list (stack).
            stack.extend_from_slice(&scip.exprs[nlexpr].children.clone());
        }

        isrootexpr = false;

        // If nothing was added, then none of the successors of nlexpr were added
        // to the stack; this is either because nlexpr was already a variable or
        // value expressions, thus a leaf, or because the desired curvature could
        // not be achieved, so it will be handled as variables, thus a leaf.
        if stack.len() == oldstackpos {
            *nleafs += 1;

            // Check whether the new leaf is not an original variable (or
            // constant).
            if let Some(s) = curvsuccess.as_deref_mut()
                && !expr::expr_is_var(scip, nlexpr)
                && !expr::expr_is_value(scip, nlexpr)
            {
                *s = false;
            }
        }
    }

    if !nlhdlrdata.isnlhdlrconvex && rootnlexpr != NONE {
        // Remove multivariate linear subexpressions, that is, change some
        // f(z1+z2) into f(z3) (z3=z1+z2 will be done by nlhdlr_default); this
        // handles the case that was not covered by the above check, which could
        // recognize f(x+y) for x, y original variables.
        let mut it = expr::ExprIter::create();
        it.init(scip, rootnlexpr, expr::ExprIterType::Dfs, false)?;
        it.set_stages_dfs(scip, expr::EXPRITER_VISITINGCHILD);

        while !it.is_end() {
            let child = it.get_child_expr_dfs(scip);

            // We want to change some f(x+y+z) into just f(), where f is the
            // expression the iterator points to and x+y+z is child. A child of a
            // child, e.g., z, may not be a variable yet (these are added in
            // collectLeafs later), but an expression of some nonlinear type
            // without children.
            if expr_is_multivar_linear(scip, child) {
                // Turn child (x+y+z) into a sum without children; collectLeafs()
                // should then replace this by an auxvar.

                // TODO remove children from nlexpr2origexpr? should also do this if
                // they are not used somewhere else; we could check nuses for this;
                // however, it shouldn't matter to have some stray entries in the
                // hashmap either.
                expr::expr_remove_children(scip, child)?;

                it.skip_dfs(scip);
            } else {
                it.get_next(scip);
            }
        }
        it.free(scip);
    }

    if rootnlexpr != NONE {
        let mut istrivial = true;

        // If handletrivial is enabled, then only require that rootnlexpr itself
        // has required curvature (so has children; see below) and that we are not
        // a trivial sum (because the previous implementation of this nlhdlr
        // didn't allow this, either).
        if !nlhdlrdata.handletrivial || expr::expr_is_sum(scip, rootnlexpr) {
            // If all children do not have children, i.e., are variables, or will be
            // replaced by auxvars, then free; also if rootnlexpr has no children,
            // then free.
            for &c in &scip.exprs[rootnlexpr].children {
                if !scip.exprs[c].children.is_empty() {
                    istrivial = false;
                    break;
                }
            }
        } else if !scip.exprs[rootnlexpr].children.is_empty() {
            // If handletrivial, then just require children.
            istrivial = false;
        }

        if istrivial {
            expr::expr_release(scip, &mut rootnlexpr)?;
            rootnlexpr = NONE;
        }
    }

    Ok(rootnlexpr)
}

/// `collectLeafs`: collects (non-value) leaf expressions and ensure that
/// they correspond to a variable (original or auxiliary).
///
/// For children where we could not achieve the desired curvature, get the
/// auxvar and replace the child by a var-expression that points to this
/// auxvar. Collect all leaf expressions (if not a value-expression) and
/// index them.
fn collect_leafs(scip: &mut Scip, nlhdlrexprdata: usize) -> ScipResult {
    // Collect all auxvars and collect all variables.
    let mut leaf2index: HashMap<ExprId, i32> = HashMap::with_capacity(ed(scip, nlhdlrexprdata).nleafs.max(0) as usize);
    // We start a new count, this time skipping value-expressions.
    edm(scip, nlhdlrexprdata).nleafs = 0;

    let root = ed(scip, nlhdlrexprdata).nlexpr;
    let mut it = expr::ExprIter::create();
    it.init(scip, root, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_VISITINGCHILD);

    let mut nlexpr = it.get_current();
    while !it.is_end() {
        let mut child = it.get_child_expr_dfs(scip);

        // If the to-be-visited child has children, then it doesn't need to be
        // replaced by a new expression (representing the auxvar).
        if !scip.exprs[child].children.is_empty() {
            nlexpr = it.get_next(scip);
            continue;
        }

        let origexpr = ed(scip, nlhdlrexprdata).nlexpr2origexpr[&child];

        if !scip.exprs[origexpr].children.is_empty() {
            // Having a child that had children in original but not in copy means
            // that we could not achieve the desired curvature; thus, replace by a
            // new child that points to the auxvar of the original expression; we
            // registered in createNlhdlrExprData that we need an auxvar, so it
            // should exist now.
            let var = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, origexpr);

            // This captures newchild once.
            let mut newchild = crate::expr_var::create_expr_var(scip, var, None, 0)?;

            // This captures newchild again.
            let childidx = it.get_child_idx_dfs(scip) as usize;
            expr::expr_replace_child(scip, nlexpr, childidx, newchild)?;

            // Do not remove child->origexpr from hashmap, as child may appear again
            // due to common subexprs (created by curvCheckProductComposite, for
            // example); if it doesn't reappear, though, but the memory address is
            // reused, we need to make sure it points to the right origexpr.
            edm(scip, nlhdlrexprdata).nlexpr2origexpr.insert(newchild, origexpr);

            if let std::collections::hash_map::Entry::Vacant(e) = leaf2index.entry(newchild) {
                // New leaf -> new index and remember in hashmap.
                let d = edm(scip, nlhdlrexprdata);
                e.insert(d.nleafs);
                d.nleafs += 1;
            }

            child = newchild;
            // Because it was captured by both create and replace.
            expr::expr_release(scip, &mut newchild)?;
        } else if expr::expr_is_var(scip, child) {
            // If variable, then add to hashmap, if not already there.
            if let std::collections::hash_map::Entry::Vacant(e) = leaf2index.entry(child) {
                let d = edm(scip, nlhdlrexprdata);
                e.insert(d.nleafs);
                d.nleafs += 1;
            }
        }
        // Else: it's probably a value-expression, nothing to do.

        // Update integrality flag for future leaf expressions: convex nlhdlr may
        // use this information.
        crate::scip_expr::compute_expr_integrality(scip, child)?;

        nlexpr = it.get_next(scip);
    }
    it.free(scip);

    // Assemble auxvars array.
    let nleafs = ed(scip, nlhdlrexprdata).nleafs as usize;
    let mut leafexprs = vec![NONE; nleafs];
    for (&leaf, &idx) in &leaf2index {
        leafexprs[idx as usize] = leaf;
    }
    edm(scip, nlhdlrexprdata).leafexprs = leafexprs;

    Ok(())
}

/// `createNlhdlrExprData`: creates nonlinear handler expression data
/// structure and registers expr usage.
#[allow(clippy::too_many_arguments)]
fn create_nlhdlr_expr_data(
    scip: &mut Scip,
    nlhdlrdata: &DetectData,
    e: ExprId,
    nlexpr: ExprId,
    nlexpr2origexpr: HashMap<ExprId, ExprId>,
    nleafs: i32,
    participating: NlhdlrMethod,
) -> ScipResult<usize> {
    let mut usingaux = false;

    let mut it = expr::ExprIter::create();
    it.init(scip, nlexpr, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_VISITINGCHILD);

    while !it.is_end() {
        // Check whether to-be-visited child needs to be replaced by a new
        // expression (representing the auxvar); if child has children, then that
        // is not the case; if child has no children, but also corresponding
        // origexpr has no chilren, then this is also not the case.
        let child = it.get_child_expr_dfs(scip);
        if !scip.exprs[child].children.is_empty() {
            it.get_next(scip);
            continue;
        }

        let origexpr = nlexpr2origexpr[&child];

        // If child had children in original but not in copy means that we could
        // not achieve the desired curvature; thus, we will later replace by a new
        // child that points to the auxvar of the original expression; as we do
        // not have the auxvar now, we will only register that we will need the
        // auxvar later (if origexpr isn't a variable or constant); if we are
        // working for the concave nlhdlr, then we also indicate interest on the
        // exprs activity for estimate (distinguish below or above).
        let haschildren = !scip.exprs[origexpr].children.is_empty();
        crate::cons_nonlinear::register_expr_usage_nonlinear(
            scip,
            origexpr,
            haschildren,
            false,
            !nlhdlrdata.isnlhdlrconvex && (participating & NLHDLR_METHOD_SEPABELOW) != 0,
            !nlhdlrdata.isnlhdlrconvex && (participating & NLHDLR_METHOD_SEPAABOVE) != 0,
        )?;

        // Remember that we use an auxvar.
        if haschildren {
            usingaux = true;
        }

        it.get_next(scip);
    }
    it.free(scip);

    // If we don't work on the extended formulation, then set curvature also in
    // original expression (in case someone wants to pick this up; this might
    // be removed again). This doesn't ensure that every convex or concave
    // original expression is actually marked here. Not only because our tests
    // are incomprehensive, but also because we may not detect on sums, prefer
    // extended formulations (in nlhdlr_convex), or introduce auxvars for linear
    // subexpressions on purpose (in nlhdlr_concave).
    if !usingaux {
        scip.exprs[e].curvature = scip.exprs[nlexpr].curvature;
    }

    Ok(scip.nlhdlrexprdatas.push(Box::new(ExprData {
        nlexpr,
        nlexpr2origexpr,
        nleafs,
        leafexprs: Vec::new(),
    })))
}

/// `estimateVertexPolyhedral`: adds an estimator for a vertex-polyhedral
/// (e.g., concave) function to a given rowprep.
///
/// Calls `SCIPcomputeFacetVertexPolyhedralNonlinear()` for given function
/// and box set to local bounds of auxiliary variables.
#[allow(clippy::too_many_arguments)]
fn estimate_vertex_polyhedral(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    nlhdlrexprdata: usize,
    sol: SolId,
    usemidpoint: bool,
    overestimate: bool,
    targetvalue: f64,
    rowprep: &mut RowPrep,
    success: &mut bool,
) -> ScipResult {
    *success = false;

    // Caller is responsible to have checked whether we can estimate, i.e.,
    // expression curvature and overestimate flag match.

    if hd(scip, nlhdlr).evalsol == NONE {
        let s = crate::api::create_sol(scip, NONE)?;
        hdm(scip, nlhdlr).evalsol = s;
    }
    let evalsol = hd(scip, nlhdlr).evalsol;

    let nleafs = ed(scip, nlhdlrexprdata).nleafs as usize;
    let mut xstar = vec![0.0; nleafs];
    let mut boxv = vec![0.0; 2 * nleafs];

    let mut allfixed = true;
    for i in 0..nleafs {
        let var = crate::expr_var::get_var_expr_var(scip, ed(scip, nlhdlrexprdata).leafexprs[i]);

        boxv[2 * i] = scip.vars[var].locdom.lb;
        if scip.set.is_infinity(-boxv[2 * i]) {
            return Ok(());
        }

        boxv[2 * i + 1] = scip.vars[var].locdom.ub;
        if scip.set.is_infinity(boxv[2 * i + 1]) {
            return Ok(());
        }

        if !scip.set.is_rel_eq(boxv[2 * i], boxv[2 * i + 1]) {
            allfixed = false;
        }

        if usemidpoint {
            xstar[i] = 0.5 * (boxv[2 * i] + boxv[2 * i + 1]);
        } else {
            xstar[i] = crate::api::get_sol_val(scip, sol, var);
        }
    }

    if allfixed {
        // SCIPcomputeFacetVertexPolyhedralNonlinear prints a warning and does not
        // succeed if all is fixed.
        return Ok(());
    }

    rowprep.ensure_size(nleafs + 1);

    let mut coefs = vec![0.0; nleafs];
    let mut facetconstant = 0.0;
    let mut evalfun = |s: &mut Scip, args: &[f64]| nlhdlr_expr_eval_concave(s, nlhdlrexprdata, evalsol, args);
    *success = crate::cons_nonlinear::compute_facet_vertex_polyhedral_nonlinear(
        scip,
        conshdlr,
        overestimate,
        &mut evalfun,
        &xstar,
        &boxv,
        nleafs,
        targetvalue,
        &mut coefs,
        &mut facetconstant,
    )?;

    if !*success {
        return Ok(());
    }

    rowprep.local = true;
    rowprep.add_constant(facetconstant);
    for i in 0..nleafs {
        let var = crate::expr_var::get_var_expr_var(scip, ed(scip, nlhdlrexprdata).leafexprs[i]);
        rowprep.add_term(var, coefs[i]);
    }

    Ok(())
}

/// `estimateGradientInner`: adds an estimator computed via a gradient to a
/// given rowprep.
fn estimate_gradient_inner(scip: &mut Scip, nlhdlrexprdata: usize, sol: SolId, rowprep: &mut RowPrep, success: &mut bool) -> ScipResult {
    let nlexpr = ed(scip, nlhdlrexprdata).nlexpr;

    // Compute gradient (TODO: this also re-evaluates (soltag=0), which
    // shouldn't be necessary unless we tried ConvexSecant before or are called
    // from Sollinearize callback).
    expr::expr_eval_gradient(scip, nlexpr, sol, 0)?;

    // If gradient evaluation error, then return.
    if scip.exprs[nlexpr].derivative == INVALID {
        return Ok(());
    }

    // Add gradient underestimator to rowprep: f(sol) + (x - sol) \nabla
    // f(sol); constant will store f(sol) - sol * \nabla f(sol); to avoid some
    // cancellation errors when linear variables take huge values (like 1e20),
    // we use double-double arithmetic here.
    // f(sol)
    let mut constant = quad::Quad::new(scip.exprs[nlexpr].evalvalue);
    let nleafs = ed(scip, nlhdlrexprdata).nleafs as usize;
    for i in 0..nleafs {
        let leaf = ed(scip, nlhdlrexprdata).leafexprs[i];

        let deriv = scip.exprs[leaf].derivative;
        if deriv == INVALID {
            return Ok(());
        }

        let var = crate::expr_var::get_var_expr_var(scip, leaf);
        let varval = crate::api::get_sol_val(scip, sol, var);

        // Add deriv * var to rowprep and deriv * (-varval) to constant.
        rowprep.add_term(var, deriv);
        constant = quad::sum_qd(constant, -deriv * varval);
    }

    rowprep.add_constant(constant.to_dbl());
    rowprep.local = false;

    *success = true;

    Ok(())
}

/// `estimateGradient`: adds an estimator computed via a gradient to a given
/// rowprep, possibly perturbing solution.
fn estimate_gradient(scip: &mut Scip, nlhdlr: NlhdlrId, nlhdlrexprdata: usize, sol: SolId, rowprep: &mut RowPrep, success: &mut bool) -> ScipResult {
    *success = false;

    estimate_gradient_inner(scip, nlhdlrexprdata, sol, rowprep, success)?;

    // If succeeded, then there was no gradient evaluation error, so we are
    // done.
    if *success {
        return Ok(());
    }

    // We take maxperturb == 0 as signal to not try perturbation.
    if hd(scip, nlhdlr).maxperturb == 0.0 {
        return Ok(());
    }

    if hd(scip, nlhdlr).evalsol == NONE {
        let s = crate::api::create_sol(scip, NONE)?;
        hdm(scip, nlhdlr).evalsol = s;
    }

    if hd(scip, nlhdlr).randnumgen.is_none() {
        let seed = scip.set.initialize_random_seed(RANDNUMINITSEED);
        hdm(scip, nlhdlr).randnumgen = Some(RandNumGen::new(seed));
    }

    let evalsol = hd(scip, nlhdlr).evalsol;
    let maxperturb = hd(scip, nlhdlr).maxperturb;
    let nleafs = ed(scip, nlhdlrexprdata).nleafs as usize;
    for i in 0..nleafs {
        let var = crate::expr_var::get_var_expr_var(scip, ed(scip, nlhdlrexprdata).leafexprs[i]);
        let lb = scip.vars[var].glbdom.lb;
        let ub = scip.vars[var].glbdom.ub;
        let mut val = crate::api::get_sol_val(scip, sol, var);
        val = cmin(ub, cmax(lb, val));

        let mut p = hdm(scip, nlhdlr).randnumgen.as_mut().unwrap().get_real(-maxperturb, maxperturb);
        if !scip.set.is_zero(val) {
            p *= val.abs();
        }

        // If perturbation to left underruns lower bound, then try perturb to
        // right; if perturbation to right exceeds lower bound, then try perturb
        // to left.
        if val + p <= lb || val + p >= ub {
            val -= p;
        } else {
            val += p;
        }

        // If still exceeding bound, then |ub-lb| < 2*maxperturb and we pick a
        // random value between bounds (if ub-lb < 2eps, then SCIPrandomGetReal()
        // still gives a reasonable number in [lb,ub]).
        if val <= lb || val >= ub {
            let eps = scip.set.epsilon();
            val = hdm(scip, nlhdlr).randnumgen.as_mut().unwrap().get_real(lb + eps, ub - eps);
        }

        crate::api::set_sol_val(scip, evalsol, var, val)?;
    }

    // Try again with perturbed point.
    estimate_gradient_inner(scip, nlhdlrexprdata, evalsol, rowprep, success)
}

/// `estimateConvexSecant`: adds an estimator generated by putting a secant
/// through the coordinates given by the two closest integer points.
fn estimate_convex_secant(scip: &mut Scip, nlhdlr: NlhdlrId, nlhdlrexprdata: usize, sol: SolId, rowprep: &mut RowPrep, success: &mut bool) -> ScipResult {
    let nlexpr = ed(scip, nlhdlrexprdata).nlexpr;

    *success = false;

    let var = crate::expr_var::get_var_expr_var(scip, ed(scip, nlhdlrexprdata).leafexprs[0]);

    let mut x = crate::api::get_sol_val(scip, sol, var);

    // Find out coordinates of var left and right to sol.
    let left;
    let right;
    if scip.set.is_integral(x) {
        x = scip.set.round(x);
        if scip.set.is_eq(x, scip.vars[var].glbdom.lb) {
            left = x;
            right = left + 1.0;
        } else {
            right = x;
            left = right - 1.0;
        }
    } else {
        left = scip.set.floor(x);
        right = scip.set.ceil(x);
    }

    // Now evaluate at left and right.
    if hd(scip, nlhdlr).evalsol == NONE {
        let s = crate::api::create_sol(scip, NONE)?;
        hdm(scip, nlhdlr).evalsol = s;
    }
    let evalsol = hd(scip, nlhdlr).evalsol;

    crate::api::set_sol_val(scip, evalsol, var, left)?;
    expr::expr_eval(scip, nlexpr, evalsol, 0)?;

    // Evaluation error or a too large constant -> skip.
    let fleft = scip.exprs[nlexpr].evalvalue;
    if scip.set.is_infinity(fleft.abs()) {
        return Ok(());
    }

    crate::api::set_sol_val(scip, evalsol, var, right)?;
    expr::expr_eval(scip, nlexpr, evalsol, 0)?;

    // Evaluation error or a too large constant -> skip.
    let fright = scip.exprs[nlexpr].evalvalue;
    if scip.set.is_infinity(fright.abs()) {
        return Ok(());
    }

    // Skip if too steep; for clay0204h, this resulted in a wrong cut from
    // f(0)=1e12 f(1)=0.99998, since due to limited precision, this was handled
    // as if f(1)=1.
    let eps = scip.set.epsilon();
    if (!scip.set.is_zero(fleft) && (fright / fleft).abs() * eps > 1.0) || (!scip.set.is_zero(fright) && (fleft / fright).abs() * eps > 1.0) {
        return Ok(());
    }

    // Now add f(left) + (f(right) - f(left)) * (x - left) as estimator to
    // rowprep.
    rowprep.add_term(var, fright - fleft);
    rowprep.add_constant(fleft - (fright - fleft) * left);
    rowprep.local = false;

    *success = true;

    Ok(())
}

/// The name C gives an estimator: `prefix`, the expression, and the solution
/// (`%s%p_%s%lld`).
fn estimate_name(scip: &Scip, prefix: &str, overestimate: bool, e: ExprId, sol: SolId) -> String {
    format!(
        "{}{prefix}{e}_{}{}",
        if overestimate { "over" } else { "under" },
        if sol != NONE { "sol" } else { "lp" },
        if sol != NONE { scip.sols[sol].index as i64 } else { crate::api::get_n_lps(scip) }
    )
}

/*
 * Callback methods of convex nonlinear handler
 */

/// `nlhdlrfreeHdlrDataConvexConcave`: free handler data of convex or
/// concave nlhdlr.
fn nlhdlr_free_hdlr_data_convex_concave(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    scip.nlhdlrs[nlhdlr].data = None;
    Ok(())
}

/// `nlhdlrfreeExprDataConvexConcave`: callback to free expression specific
/// data.
fn nlhdlr_free_expr_data_convex_concave(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: &mut usize) -> ScipResult {
    let d = scip.nlhdlrexprdatas.remove(*nlhdlrexprdata).downcast::<ExprData>().unwrap();
    let mut nlexpr = d.nlexpr;
    expr::expr_release(scip, &mut nlexpr)?;
    *nlhdlrexprdata = NONE;
    Ok(())
}

/// `nlhdlrExitConvex`: deinitialization of problem-specific data.
fn nlhdlr_exit_convex(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    let evalsol = hd(scip, nlhdlr).evalsol;
    if evalsol != NONE {
        crate::api::free_sol(scip, evalsol)?;
        hdm(scip, nlhdlr).evalsol = NONE;
    }

    hdm(scip, nlhdlr).randnumgen = None;

    Ok(())
}

/// `nlhdlrDetectConvex`: checks whether expression (or -expression) is
/// convex, possibly after introducing auxiliary variables.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_convex(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    _cons: ConsId,
    enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    let mut nlexpr = NONE;
    let mut nleafs = 0;

    // We currently do not participate if only activity computation is
    // required.
    if (*enforcing & NLHDLR_METHOD_SEPABOTH) == NLHDLR_METHOD_SEPABOTH {
        return Ok(());
    }

    // Ignore pure constants and variables.
    if scip.exprs[e].children.is_empty() {
        return Ok(());
    }

    let nlhdlrdata = hd(scip, nlhdlr).detect_data();

    // Initialize mapping from copied expression to original one; 20 is not a
    // bad estimate for the size of convex subexpressions that we can usually
    // discover; when expressions will be allowed to store "user"data, we could
    // get rid of this hashmap (TODO).
    let mut nlexpr2origexpr: HashMap<ExprId, ExprId> = HashMap::with_capacity(20);

    // If no separation below yet.
    if *enforcing & NLHDLR_METHOD_SEPABELOW == 0 {
        let assume = crate::cons_nonlinear::assume_convex_nonlinear(scip, conshdlr);
        nlexpr = construct_expr(scip, &nlhdlrdata, &mut nlexpr2origexpr, &mut nleafs, e, ExprCurv::Convex, None, assume, None)?;
        if nlexpr != NONE {
            *participating |= NLHDLR_METHOD_SEPABELOW;
        } else {
            nlexpr2origexpr.clear();
        }
    }

    // If no separation above and not convex.
    if *enforcing & NLHDLR_METHOD_SEPAABOVE == 0 && nlexpr == NONE {
        let assume = crate::cons_nonlinear::assume_convex_nonlinear(scip, conshdlr);
        nlexpr = construct_expr(scip, &nlhdlrdata, &mut nlexpr2origexpr, &mut nleafs, e, ExprCurv::Concave, None, assume, None)?;
        if nlexpr != NONE {
            *participating |= NLHDLR_METHOD_SEPAABOVE;
        }
    }

    // Everything we participate in we also enforce.
    *enforcing |= *participating;

    if *participating == 0 {
        return Ok(());
    }

    // Create the expression data of the nonlinear handler; notify conshdlr
    // about expr for which we will require auxiliary variables.
    *nlhdlrexprdata = create_nlhdlr_expr_data(scip, &nlhdlrdata, e, nlexpr, nlexpr2origexpr, nleafs, *participating)?;

    Ok(())
}

/// `nlhdlrEvalAuxConvexConcave`: auxiliary evaluation callback.
fn nlhdlr_eval_aux_convex_concave(scip: &mut Scip, _nlhdlr: NlhdlrId, _expr: ExprId, nlhdlrexprdata: usize, auxvalue: &mut f64, sol: SolId) -> ScipResult {
    let nlexpr = ed(scip, nlhdlrexprdata).nlexpr;
    expr::expr_eval(scip, nlexpr, sol, 0)?;
    *auxvalue = scip.exprs[nlexpr].evalvalue;
    Ok(())
}

/// `nlhdlrInitSepaConvex`: init sepa callback that initializes LP.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_init_sepa_convex(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    mut overestimate: bool,
    mut underestimate: bool,
    infeasible: &mut bool,
) -> ScipResult {
    // Setup nlhdlrexprdata->leafexprs.
    collect_leafs(scip, nlhdlrexprdata)?;

    let nlexpr = ed(scip, nlhdlrexprdata).nlexpr;
    let curvature = scip.exprs[nlexpr].curvature;

    // We can only be estimating on the convex side.
    if curvature == ExprCurv::Convex {
        overestimate = false;
    } else if curvature == ExprCurv::Concave {
        underestimate = false;
    }
    if !overestimate && !underestimate {
        return Ok(());
    }

    // Linearizes at 5 different points obtained as convex combination of the
    // lower and upper bound of the variables present in the convex expression;
    // whether more weight is given to the lower or upper bound of a variable
    // depends on whether the fixing of the variable to that value is better for
    // the objective function.
    let sol = crate::api::create_sol(scip, NONE)?;

    *infeasible = false;

    for k in 0..5 {
        // lambda = 0.1, 0.2, 0.3, 0.4, 0.5
        let lambda = 0.1 * (k + 1) as f64;

        let nleafs = ed(scip, nlhdlrexprdata).nleafs as usize;
        for i in 0..nleafs {
            let var = crate::expr_var::get_var_expr_var(scip, ed(scip, nlhdlrexprdata).leafexprs[i]);

            let mut lb = scip.vars[var].glbdom.lb;
            let mut ub = scip.vars[var].glbdom.ub;

            // Make sure the absolute values of bounds are not too large.
            if ub > -INITLPMAXVARVAL {
                lb = cmax(lb, -INITLPMAXVARVAL);
            }
            if lb < INITLPMAXVARVAL {
                ub = cmin(ub, INITLPMAXVARVAL);
            }

            // In the case when ub < -maxabsbnd or lb > maxabsbnd, we still want to at
            // least make bounds finite.
            if scip.set.is_infinity(-lb) {
                lb = cmin(-10.0, ub - 0.1 * ub.abs());
            }
            if scip.set.is_infinity(ub) {
                ub = cmax(10.0, lb + 0.1 * lb.abs());
            }

            if crate::var::var_get_best_bound_type(scip, var) == BoundType::Lower {
                crate::api::set_sol_val(scip, sol, var, lambda * ub + (1.0 - lambda) * lb)?;
            } else {
                crate::api::set_sol_val(scip, sol, var, lambda * lb + (1.0 - lambda) * ub)?;
            }
        }

        let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);
        let mut success = false;
        estimate_gradient(scip, nlhdlr, nlhdlrexprdata, sol, &mut rowprep, &mut success)?;
        if !success {
            continue;
        }

        // Add auxiliary variable.
        rowprep.add_term(crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e), -1.0);

        // Straighten out numerics.
        let huge = scip.set.get_huge_value();
        rowprep.cleanup2(scip, NONE, huge, Some(&mut success));
        if !success {
            continue;
        }

        rowprep.name = format!("{}estimate_gradient{e}_initsepa_{k}", if overestimate { "over" } else { "under" });
        let mut row = rowprep.get_row_cons(scip, cons)?;

        *infeasible = crate::scip_lp::add_row(scip, row, false)?;
        crate::scip_lp::release_row(scip, &mut row)?;

        if *infeasible {
            break;
        }
    }

    crate::api::free_sol(scip, sol)?;

    Ok(())
}

/// `nlhdlrEstimateConvex`: estimator callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_estimate_convex(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    auxvalue: f64,
    overestimate: bool,
    _targetvalue: f64,
    _addbranchscores: bool,
    rowpreps: &mut Vec<RowPrep>,
    success: &mut bool,
    addedbranchscores: &mut bool,
) -> ScipResult {
    // We must be called only for the side that we indicated to participate in
    // during DETECT.

    *success = false;
    *addedbranchscores = false;

    // We can skip eval as nlhdlrEvalAux should have been called for same
    // solution before.

    let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);

    let (nleafs, leaf0) = {
        let d = ed(scip, nlhdlrexprdata);
        (d.nleafs, d.leafexprs.first().copied().unwrap_or(NONE))
    };
    if nleafs == 1 && scip.exprs[leaf0].integrality != ImplintType::None {
        estimate_convex_secant(scip, nlhdlr, nlhdlrexprdata, sol, &mut rowprep, success)?;

        rowprep.name = estimate_name(scip, "estimate_convexsecant", overestimate, e, sol);
    }

    // If secant method was not used or failed, then try with gradient (unless
    // we had an evaluation error in sol before).
    if !*success && auxvalue != INVALID {
        estimate_gradient(scip, nlhdlr, nlhdlrexprdata, sol, &mut rowprep, success)?;

        rowprep.name = estimate_name(scip, "estimate_convexgradient", overestimate, e, sol);
    }

    if *success {
        rowpreps.push(rowprep);
    }

    Ok(())
}

/// `nlhdlrSollinearizeConvex`: solution notification callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_sollinearize_convex(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    _solisbest: bool,
    overestimate: bool,
    _underestimate: bool,
) -> ScipResult {
    let mut success = false;

    // We must be called only for the side that we indicated to participate in
    // during DETECT.

    // Evaluate nlexpr in solution.
    let nlexpr = ed(scip, nlhdlrexprdata).nlexpr;
    expr::expr_eval(scip, nlexpr, sol, 0)?;

    let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);

    let (nleafs, leaf0) = {
        let d = ed(scip, nlhdlrexprdata);
        (d.nleafs, d.leafexprs.first().copied().unwrap_or(NONE))
    };
    let solindex = scip.sols[sol].index;
    if nleafs == 1 && scip.exprs[leaf0].integrality != ImplintType::None {
        estimate_convex_secant(scip, nlhdlr, nlhdlrexprdata, sol, &mut rowprep, &mut success)?;

        rowprep.name = format!("{}estimate_convexsecant{e}_sol{solindex}notify", if overestimate { "over" } else { "under" });
    }

    // If secant method was not used or failed, then try with gradient.
    if !success {
        estimate_gradient(scip, nlhdlr, nlhdlrexprdata, sol, &mut rowprep, &mut success)?;

        rowprep.name = format!("{}estimate_convexgradient{e}_sol{solindex}notify", if overestimate { "over" } else { "under" });
    }

    if success {
        // Complete estimator to cut and clean it up.
        rowprep.add_term(crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e), -1.0);
        let huge = scip.set.get_huge_value();
        rowprep.cleanup2(scip, sol, huge, Some(&mut success));
    }

    // If cleanup succeeded and rowprep is still global, add to cutpool.
    if success && !rowprep.local {
        let mut row = rowprep.get_row_cons(scip, cons)?;
        crate::api::add_pool_cut(scip, row)?;
        crate::scip_lp::release_row(scip, &mut row)?;
    }

    Ok(())
}

/// `nlhdlrCopyhdlrConvex`: include nlhdlr in another scip instance.
fn nlhdlr_copyhdlr_convex(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_convex(targetscip)
}

macro_rules! t {
    ($ty:ty, $nlhdlr:expr, $f:ident) => {
        crate::data_target!($ty, nlhdlrs, $nlhdlr, data, NlhdlrData, $f)
    };
}

/// `SCIPincludeNlhdlrConvex`: includes convex nonlinear handler in
/// nonlinear constraint handler.
pub fn include_nlhdlr_convex(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_real};

    let nlhdlrdata = NlhdlrData {
        isnlhdlrconvex: true,
        evalsol: NONE,
        randnumgen: None,
        detectsum: false,
        extendedform: false,
        maxperturb: 0.0,
        cvxquadratic: false,
        cvxsignomial: false,
        cvxprodcomp: false,
        handletrivial: false,
    };

    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        CONVEX_NLHDLR_NAME,
        Some(CONVEX_NLHDLR_DESC),
        CONVEX_NLHDLR_DETECTPRIORITY,
        CONVEX_NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_convex,
        nlhdlr_eval_aux_convex_concave,
        Some(Box::new(nlhdlrdata)),
    )?;

    paramset_add_bool(
        scip,
        "nlhdlr/convex/detectsum",
        "whether to run convexity detection when the root of an expression is a non-quadratic sum",
        t!(bool, nlhdlr, detectsum),
        false,
        DEFAULT_DETECTSUM,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/convex/extendedform",
        "whether to create extended formulations instead of looking for maximal convex expressions",
        t!(bool, nlhdlr, extendedform),
        false,
        DEFAULT_EXTENDEDFORM,
        None,
    )?;
    paramset_add_real(
        scip,
        "nlhdlr/convex/maxperturb",
        "maximal relative perturbation of non-differentiable reference point",
        t!(f64, nlhdlr, maxperturb),
        false,
        DEFAULT_MAXPERTURB,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/convex/cvxquadratic",
        "whether to use convexity check on quadratics",
        t!(bool, nlhdlr, cvxquadratic),
        true,
        DEFAULT_CVXQUADRATIC_CONVEX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/convex/cvxsignomial",
        "whether to use convexity check on signomials",
        t!(bool, nlhdlr, cvxsignomial),
        true,
        DEFAULT_CVXSIGNOMIAL,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/convex/cvxprodcomp",
        "whether to use convexity check on product composition f(h)*h",
        t!(bool, nlhdlr, cvxprodcomp),
        true,
        DEFAULT_CVXPRODCOMP,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/convex/handletrivial",
        "whether to also handle trivial convex expressions",
        t!(bool, nlhdlr, handletrivial),
        true,
        DEFAULT_HANDLETRIVIAL,
        None,
    )?;

    nlhdlr::nlhdlr_set_free_hdlr_data(scip, nlhdlr, Some(nlhdlr_free_hdlr_data_convex_concave));
    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_convex));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_free_expr_data_convex_concave));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, Some(nlhdlr_init_sepa_convex), None, Some(nlhdlr_estimate_convex), None);
    nlhdlr::nlhdlr_set_sollinearize(scip, nlhdlr, Some(nlhdlr_sollinearize_convex));
    nlhdlr::nlhdlr_set_init_exit(scip, nlhdlr, None, Some(nlhdlr_exit_convex));

    Ok(())
}

/*
 * Callback methods of concave nonlinear handler
 */

/// `nlhdlrExitConcave`: deinitialization of problem-specific data.
fn nlhdlr_exit_concave(scip: &mut Scip, nlhdlr: NlhdlrId) -> ScipResult {
    let evalsol = hd(scip, nlhdlr).evalsol;
    if evalsol != NONE {
        crate::api::free_sol(scip, evalsol)?;
        hdm(scip, nlhdlr).evalsol = NONE;
    }

    Ok(())
}

/// `nlhdlrDetectConcave`: checks whether expression (or -expression) is
/// concave, possibly after introducing auxiliary variables.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_detect_concave(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    _cons: ConsId,
    enforcing: &mut NlhdlrMethod,
    participating: &mut NlhdlrMethod,
    nlhdlrexprdata: &mut usize,
) -> ScipResult {
    let mut nlexpr = NONE;
    let mut nleafs = 0;

    // We currently do not participate if only activity computation is
    // required.
    if (*enforcing & NLHDLR_METHOD_SEPABOTH) == NLHDLR_METHOD_SEPABOTH {
        return Ok(());
    }

    // Ignore pure constants and variables.
    if scip.exprs[e].children.is_empty() {
        return Ok(());
    }

    let nlhdlrdata = hd(scip, nlhdlr).detect_data();

    // Initialize mapping from copied expression to original one; 20 is not a
    // bad estimate for the size of concave subexpressions that we can usually
    // discover; when expressions will be allowed to store "user"data, we could
    // get rid of this hashmap (TODO).
    let mut nlexpr2origexpr: HashMap<ExprId, ExprId> = HashMap::with_capacity(20);

    // If no separation below yet.
    if *enforcing & NLHDLR_METHOD_SEPABELOW == 0 {
        nlexpr = construct_expr(scip, &nlhdlrdata, &mut nlexpr2origexpr, &mut nleafs, e, ExprCurv::Concave, None, false, None)?;

        if nlexpr != NONE && nleafs as usize > crate::cons_nonlinear::MAXVERTEXPOLYDIM {
            expr::expr_release(scip, &mut nlexpr)?;
            nlexpr = NONE;
        }

        if nlexpr != NONE {
            *participating |= NLHDLR_METHOD_SEPABELOW;
        } else {
            nlexpr2origexpr.clear();
        }
    }

    // If no separation above and not concave.
    if *enforcing & NLHDLR_METHOD_SEPAABOVE == 0 && nlexpr == NONE {
        nlexpr = construct_expr(scip, &nlhdlrdata, &mut nlexpr2origexpr, &mut nleafs, e, ExprCurv::Convex, None, false, None)?;

        if nlexpr != NONE && nleafs as usize > crate::cons_nonlinear::MAXVERTEXPOLYDIM {
            expr::expr_release(scip, &mut nlexpr)?;
            nlexpr = NONE;
        }

        if nlexpr != NONE {
            *participating |= NLHDLR_METHOD_SEPAABOVE;
        }
    }

    // Everything we participate in we also enforce (at the moment).
    *enforcing |= *participating;

    if *participating == 0 {
        return Ok(());
    }

    // Create the expression data of the nonlinear handler; notify conshdlr
    // about expr for which we will require auxiliary variables and use
    // activity.
    *nlhdlrexprdata = create_nlhdlr_expr_data(scip, &nlhdlrdata, e, nlexpr, nlexpr2origexpr, nleafs, *participating)?;

    Ok(())
}

/// `nlhdlrInitSepaConcave`: init sepa callback that initializes LP.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_init_sepa_concave(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    mut overestimate: bool,
    mut underestimate: bool,
    infeasible: &mut bool,
) -> ScipResult {
    let nlexpr = ed(scip, nlhdlrexprdata).nlexpr;

    // Setup nlhdlrexprdata->leafexprs.
    collect_leafs(scip, nlhdlrexprdata)?;

    let curvature = scip.exprs[nlexpr].curvature;

    // We can only be estimating on non-convex side.
    if curvature == ExprCurv::Concave {
        overestimate = false;
    } else if curvature == ExprCurv::Convex {
        underestimate = false;
    }
    if !overestimate && !underestimate {
        return Ok(());
    }

    // Compute estimator and store in rowprep.
    let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);
    let mut success = false;
    let target = if overestimate { scip.set.infinity() } else { -scip.set.infinity() };
    estimate_vertex_polyhedral(scip, conshdlr, nlhdlr, nlhdlrexprdata, NONE, true, overestimate, target, &mut rowprep, &mut success)?;
    if !success {
        return Ok(());
    }

    // Add auxiliary variable.
    rowprep.add_term(crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e), -1.0);

    // Straighten out numerics.
    let huge = scip.set.get_huge_value();
    rowprep.cleanup2(scip, NONE, huge, Some(&mut success));
    if !success {
        return Ok(());
    }

    rowprep.name = format!("{}estimate_concave{e}_initsepa", if overestimate { "over" } else { "under" });
    let mut row = rowprep.get_row_cons(scip, cons)?;

    *infeasible = crate::scip_lp::add_row(scip, row, false)?;
    crate::scip_lp::release_row(scip, &mut row)?;

    Ok(())
}

/// `nlhdlrEstimateConcave`: estimator callback.
#[allow(clippy::too_many_arguments)]
fn nlhdlr_estimate_concave(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    nlhdlr: NlhdlrId,
    e: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    auxvalue: f64,
    overestimate: bool,
    targetvalue: f64,
    addbranchscores: bool,
    rowpreps: &mut Vec<RowPrep>,
    success: &mut bool,
    addedbranchscores: &mut bool,
) -> ScipResult {
    // We must be called only for the side that we indicated to participate in
    // during DETECT.

    *success = false;
    *addedbranchscores = false;

    let mut rowprep = RowPrep::new(if overestimate { SideType::Left } else { SideType::Right }, true);

    estimate_vertex_polyhedral(scip, conshdlr, nlhdlr, nlhdlrexprdata, sol, false, overestimate, targetvalue, &mut rowprep, success)?;

    if *success {
        rowprep.name = estimate_name(scip, "estimate_concave", overestimate, e, sol);
        rowpreps.push(rowprep);
    }

    if addbranchscores {
        // Check how much is the violation on the side that we estimate.
        let violation = if auxvalue == INVALID {
            // If cannot evaluate, then always branch.
            scip.set.infinity()
        } else {
            // Get value of auxiliary variable of this expression.
            let auxval = crate::api::get_sol_val(scip, sol, crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, e));

            // Compute the violation: if we underestimate, then we enforce expr <=
            // auxval, so violation is (positive part of) auxvalue - auxval; if we
            // overestimate, then we enforce expr >= auxval, so violation is
            // (positive part of) auxval - auxvalue.
            if !overestimate { cmax(0.0, auxvalue - auxval) } else { cmax(0.0, auxval - auxvalue) }
        };

        // Add violation as branching-score to expressions; the core will take
        // care distributing this onto variables; map leaf expressions back to
        // original expressions.
        // TODO do this once at end of detect and store in nlhdlrexprdata.
        let exprs: Vec<ExprId> = {
            let d = ed(scip, nlhdlrexprdata);
            d.leafexprs[..d.nleafs as usize].iter().map(|l| d.nlexpr2origexpr[l]).collect()
        };
        *addedbranchscores = crate::cons_nonlinear::add_exprs_viol_score_nonlinear(scip, &exprs, violation, sol)?;
    }

    Ok(())
}

/// `nlhdlrCopyhdlrConcave`: includes nonlinear handler in another scip
/// instance.
fn nlhdlr_copyhdlr_concave(targetscip: &mut Scip, _targetconshdlr: ConshdlrId, _sourcescip: &Scip, _sourceconshdlr: ConshdlrId, _sourcenlhdlr: NlhdlrId) -> ScipResult {
    include_nlhdlr_concave(targetscip)
}

/// `SCIPincludeNlhdlrConcave`: includes concave nonlinear handler in
/// nonlinear constraint handler.
pub fn include_nlhdlr_concave(scip: &mut Scip) -> ScipResult {
    use crate::paramset::paramset_add_bool;

    let nlhdlrdata = NlhdlrData {
        isnlhdlrconvex: false,
        evalsol: NONE,
        randnumgen: None,
        detectsum: false,
        // "extended" formulations of a concave expressions can give worse
        // estimators.
        extendedform: false,
        maxperturb: 0.0,
        cvxquadratic: false,
        cvxsignomial: false,
        cvxprodcomp: false,
        handletrivial: false,
    };

    let nlhdlr = crate::cons_nonlinear::include_nlhdlr_nonlinear(
        scip,
        CONCAVE_NLHDLR_NAME,
        Some(CONCAVE_NLHDLR_DESC),
        CONCAVE_NLHDLR_DETECTPRIORITY,
        CONCAVE_NLHDLR_ENFOPRIORITY,
        nlhdlr_detect_concave,
        nlhdlr_eval_aux_convex_concave,
        Some(Box::new(nlhdlrdata)),
    )?;

    paramset_add_bool(
        scip,
        "nlhdlr/concave/detectsum",
        "whether to run convexity detection when the root of an expression is a sum",
        t!(bool, nlhdlr, detectsum),
        false,
        DEFAULT_DETECTSUM,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/concave/cvxquadratic",
        "whether to use convexity check on quadratics",
        t!(bool, nlhdlr, cvxquadratic),
        true,
        DEFAULT_CVXQUADRATIC_CONCAVE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/concave/cvxsignomial",
        "whether to use convexity check on signomials",
        t!(bool, nlhdlr, cvxsignomial),
        true,
        DEFAULT_CVXSIGNOMIAL,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/concave/cvxprodcomp",
        "whether to use convexity check on product composition f(h)*h",
        t!(bool, nlhdlr, cvxprodcomp),
        true,
        DEFAULT_CVXPRODCOMP,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlhdlr/concave/handletrivial",
        "whether to also handle trivial convex expressions",
        t!(bool, nlhdlr, handletrivial),
        true,
        DEFAULT_HANDLETRIVIAL,
        None,
    )?;

    nlhdlr::nlhdlr_set_free_hdlr_data(scip, nlhdlr, Some(nlhdlr_free_hdlr_data_convex_concave));
    nlhdlr::nlhdlr_set_copy_hdlr(scip, nlhdlr, Some(nlhdlr_copyhdlr_concave));
    nlhdlr::nlhdlr_set_free_expr_data(scip, nlhdlr, Some(nlhdlr_free_expr_data_convex_concave));
    nlhdlr::nlhdlr_set_sepa(scip, nlhdlr, Some(nlhdlr_init_sepa_concave), None, Some(nlhdlr_estimate_concave), None);
    nlhdlr::nlhdlr_set_init_exit(scip, nlhdlr, None, Some(nlhdlr_exit_concave));

    Ok(())
}

/// `SCIPhasExprCurvature`: checks whether a given expression is convex or
/// concave w.r.t. the original variables.
///
/// This function uses the methods that are used in the detection algorithm
/// of the convex nonlinear handler.
pub fn has_expr_curvature(scip: &mut Scip, e: ExprId, curv: ExprCurv, assumevarfixed: Option<&HashMap<VarId, ()>>) -> ScipResult<bool> {
    // Create temporary hashmap.
    let mut nlexpr2origexpr: HashMap<ExprId, ExprId> = HashMap::with_capacity(20);

    // Prepare nonlinear handler data.
    let nlhdlrdata = DetectData {
        isnlhdlrconvex: true,
        detectsum: true,
        extendedform: false,
        cvxquadratic: true,
        cvxsignomial: true,
        cvxprodcomp: true,
        handletrivial: true,
    };

    let mut success = false;
    let mut nleafs = 0;
    let mut rootnlexpr = construct_expr(scip, &nlhdlrdata, &mut nlexpr2origexpr, &mut nleafs, e, curv, assumevarfixed, false, Some(&mut success))?;

    // Free created expression.
    if rootnlexpr != NONE {
        expr::expr_release(scip, &mut rootnlexpr)?;
    }

    Ok(success)
}
