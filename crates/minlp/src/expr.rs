//! Port of `expr.c`, `expriter.c` and the expression parts of `set.c`:
//! expressions and expression handlers.
//!
//! Expressions are reference-counted nodes of a DAG; they live in
//! `Scip::exprs` and are referred to by [`ExprId`] (C's `SCIP_EXPR*`).
//! Handlers live in `Scip::exprhdlrs`. Expression data and owner data are
//! boxed `Any`, as constraint data is.
//!
//! Not ported: printing (`SCIPexprPrint`, dot output), `SCIPexprDismantle`,
//! symmetry data, and the eigen decomposition of quadratic forms (the
//! reference is built without LAPACK, so `SCIPlapackIsAvailable()` is false
//! and `SCIPexprComputeQuadraticCurvature` never computes it).

use std::{
    any::Any,
    collections::{HashMap, VecDeque},
};

use crate::{
    clock::Clock,
    interval::Interval,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ImplintType, SolId, VarId},
    var::NONE,
};

/// `SCIP_EXPR*`.
pub type ExprId = usize;
/// `SCIP_EXPRHDLR*`.
pub type ExprhdlrId = usize;

/// `SCIP_INTERVAL_INFINITY` (`SCIP_DEFAULT_INFINITY`).
pub const INTERVAL_INFINITY: f64 = 1e20;

/// `SCIP_EXPRCURV`: curvature types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum ExprCurv {
    /// Unknown or indefinite curvature.
    #[default]
    Unknown = 0,
    /// Convex.
    Convex = 1,
    /// Concave.
    Concave = 2,
    /// Linear = convex and concave.
    Linear = 3,
}

impl ExprCurv {
    /// The curvature with the given bits (`SCIP_EXPRCURV` is a bit set).
    pub fn from_bits(bits: u32) -> ExprCurv {
        match bits & 3 {
            0 => ExprCurv::Unknown,
            1 => ExprCurv::Convex,
            2 => ExprCurv::Concave,
            _ => ExprCurv::Linear,
        }
    }

    /// The bits of the curvature.
    pub fn bits(self) -> u32 {
        self as u32
    }
}

/// `SCIP_MONOTONE`: monotonicity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum Monotone {
    /// Unknown or non-monotone.
    #[default]
    Unknown = 0,
    /// Increasing.
    Inc = 1,
    /// Decreasing.
    Dec = 2,
    /// Constant = increasing and decreasing.
    Const = 3,
}

/// `SCIP_EXPRITER_STAGE` values.
pub type ExprIterStage = u32;
/// An expression is visited the first time (before any of its children are
/// visited).
pub const EXPRITER_ENTEREXPR: ExprIterStage = 1;
/// A child of an expression is to be visited.
pub const EXPRITER_VISITINGCHILD: ExprIterStage = 2;
/// A child of an expression has been visited.
pub const EXPRITER_VISITEDCHILD: ExprIterStage = 4;
/// An expression is to be left (all of its children have been processed).
pub const EXPRITER_LEAVEEXPR: ExprIterStage = 8;
/// All stages.
pub const EXPRITER_ALLSTAGES: ExprIterStage = EXPRITER_ENTEREXPR | EXPRITER_VISITINGCHILD | EXPRITER_VISITEDCHILD | EXPRITER_LEAVEEXPR;

/// `SCIP_EXPRITER_MAXNACTIVE`: maximal number of iterators that can be
/// active on an expression graph concurrently.
pub const EXPRITER_MAXNACTIVE: usize = 5;

/// `SCIP_EXPR_MAXINITESTIMATES`.
pub const EXPR_MAXINITESTIMATES: usize = 10;

/// `SCIP_EXPRITER_TYPE`: type of expression iterator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExprIterType {
    /// Reverse topological order.
    #[default]
    Rtopologic,
    /// Breadth-first search.
    Bfs,
    /// Depth-first search.
    Dfs,
}

/// `SCIP_EXPRITER_USERDATA`: the union `{ realval, intval, intvals[2],
/// uintval, ptrval }`, as its 8 bytes (little-endian, as on the reference's
/// platform).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExprIterUserData(pub u64);

impl ExprIterUserData {
    pub fn realval(self) -> f64 {
        f64::from_bits(self.0)
    }
    pub fn from_real(v: f64) -> Self {
        Self(v.to_bits())
    }
    pub fn intval(self) -> i32 {
        self.0 as u32 as i32
    }
    pub fn set_intval(&mut self, v: i32) {
        self.0 = (self.0 & !0xffff_ffff) | (v as u32 as u64);
    }
    pub fn intvals(self) -> [i32; 2] {
        [self.0 as u32 as i32, (self.0 >> 32) as u32 as i32]
    }
    pub fn from_intvals(v: [i32; 2]) -> Self {
        Self((v[0] as u32 as u64) | ((v[1] as u32 as u64) << 32))
    }
    pub fn uintval(self) -> u32 {
        self.0 as u32
    }
    /// `ptrval` as an id (`NONE` for NULL).
    pub fn ptrval(self) -> usize {
        if self.0 == 0 { NONE } else { (self.0 - 1) as usize }
    }
    pub fn from_ptr(id: usize) -> Self {
        Self(if id == NONE { 0 } else { id as u64 + 1 })
    }
}

/// `SCIP_EXPRITERDATA`: expression iterator data of a specific expression.
#[derive(Clone, Copy, Debug)]
pub struct ExprIterData {
    /// Parent expression in DFS iteration.
    pub parent: ExprId,
    /// Child that is currently visited (or will be visited next) by DFS
    /// iteration.
    pub currentchild: i32,
    /// Tag to identify whether an expression has been visited already.
    pub visitedtag: i64,
    /// Space for iterator user to store some (temporary) data.
    pub userdata: ExprIterUserData,
}

impl Default for ExprIterData {
    fn default() -> Self {
        Self {
            parent: NONE,
            currentchild: 0,
            visitedtag: 0,
            userdata: ExprIterUserData(0),
        }
    }
}

/// `SCIP_DECL_EXPR_OWNERFREE`.
pub type ExprOwnerFree = fn(&mut Scip, ExprId) -> ScipResult;
/// `SCIP_DECL_EXPR_OWNEREVALACTIVITY`.
pub type ExprOwnerEvalActivity = fn(&mut Scip, ExprId) -> ScipResult;
/// `SCIP_DECL_EXPR_MAPEXPR` within one SCIP: returns the expression to
/// use as the copy of the given one, or `NONE` to copy it normally.
pub type ExprMapExpr = fn(&mut Scip, ExprId, Option<ExprOwnerCreate>, usize, usize) -> ScipResult<ExprId>;
/// `SCIP_DECL_EXPR_OWNERCREATE`: sets the expression's ownerdata and owner
/// callbacks; the `usize` is `ownercreatedata`.
pub type ExprOwnerCreate = fn(&mut Scip, ExprId, usize) -> ScipResult;
/// `SCIP_DECL_EXPR_INTEVALVAR`: the `usize` is `intevalvardata`.
pub type ExprIntEvalVar = fn(&mut Scip, VarId, usize) -> Interval;

/// `SCIP_DECL_EXPRCOPYHDLR`: (target, source, source handler).
pub type ExprCopyHdlr = fn(&mut Scip, &Scip, ExprhdlrId) -> ScipResult;
/// `SCIP_DECL_EXPRFREEHDLR`.
pub type ExprFreeHdlr = fn(&mut Scip, ExprhdlrId) -> ScipResult;
/// `SCIP_DECL_EXPRCOPYDATA`: the copy of the source expression's data. The
/// SCIP is the source's: a ported handler's copy involves the target only
/// for variables, whose expressions are copied across SCIPs by the
/// `mapexpr` callback instead.
pub type ExprCopyData = fn(&mut Scip, ExprId) -> ScipResult<Box<dyn Any>>;
/// `SCIP_DECL_EXPRFREEDATA`.
pub type ExprFreeData = fn(&mut Scip, ExprId) -> ScipResult;
/// `SCIP_DECL_EXPRSIMPLIFY`: (expr, simplifiedexpr, ownercreate,
/// ownercreatedata).
pub type ExprSimplify = fn(&mut Scip, ExprId, &mut ExprId, Option<ExprOwnerCreate>, usize) -> ScipResult;
/// `SCIP_DECL_EXPRCOMPARE`.
pub type ExprCompare = fn(&Scip, ExprId, ExprId) -> i32;
/// `SCIP_DECL_EXPRPARSE`: (handler, string, start position, end position
/// in string, expr, success, ownercreate, ownercreatedata).
pub type ExprParse = fn(&mut Scip, ExprhdlrId, &[u8], usize, &mut usize, &mut ExprId, &mut bool, Option<ExprOwnerCreate>, usize) -> ScipResult;
/// `SCIP_DECL_EXPRCURVATURE`: (expr, exprcurvature, success, childcurv).
pub type ExprCurvature = fn(&mut Scip, ExprId, ExprCurv, &mut bool, &mut [ExprCurv]) -> ScipResult;
/// `SCIP_DECL_EXPRMONOTONICITY`.
pub type ExprMonotonicity = fn(&mut Scip, ExprId, i32, &mut Monotone) -> ScipResult;
/// `SCIP_DECL_EXPRINTEGRALITY`.
pub type ExprIntegrality = fn(&mut Scip, ExprId, &mut ImplintType) -> ScipResult;
/// `SCIP_DECL_EXPRHASH`: (expr, hashkey, childrenhashes).
pub type ExprHash = fn(&mut Scip, ExprId, &mut u32, &[u32]) -> ScipResult;
/// `SCIP_DECL_EXPREVAL`.
pub type ExprEval = fn(&mut Scip, ExprId, &mut f64, SolId) -> ScipResult;
/// `SCIP_DECL_EXPRBWDIFF`.
pub type ExprBwDiff = fn(&mut Scip, ExprId, i32, &mut f64) -> ScipResult;
/// `SCIP_DECL_EXPRFWDIFF`.
pub type ExprFwDiff = fn(&mut Scip, ExprId, &mut f64, SolId) -> ScipResult;
/// `SCIP_DECL_EXPRBWFWDIFF`.
pub type ExprBwFwDiff = fn(&mut Scip, ExprId, i32, &mut f64, SolId) -> ScipResult;
/// `SCIP_DECL_EXPRINTEVAL`.
pub type ExprIntEval = fn(&mut Scip, ExprId, &mut Interval, Option<ExprIntEvalVar>, usize) -> ScipResult;
/// `SCIP_DECL_EXPRESTIMATE`: (expr, localbounds, globalbounds, refpoint,
/// overestimate, targetvalue, coefs, constant, islocal, success,
/// branchcand).
pub type ExprEstimate = fn(&mut Scip, ExprId, &[Interval], &[Interval], &[f64], bool, f64, &mut [f64], &mut f64, &mut bool, &mut bool, &mut [bool]) -> ScipResult;
/// `SCIP_DECL_EXPRINITESTIMATES`: (expr, bounds, overestimate, coefs,
/// constant, nreturned).
pub type ExprInitEstimates = fn(&mut Scip, ExprId, &[Interval], bool, &mut [Vec<f64>], &mut [f64], &mut i32) -> ScipResult;
/// `SCIP_DECL_EXPRREVERSEPROP`: (expr, bounds, childrenbounds,
/// infeasible).
pub type ExprReverseProp = fn(&mut Scip, ExprId, Interval, &mut [Interval], &mut bool) -> ScipResult;

/// `SCIP_EXPRHDLR`: expression handler.
pub struct Exprhdlr {
    /// Expression handler name.
    pub name: String,
    /// Expression handler description (can be NULL).
    pub desc: Option<String>,
    /// Data of handler.
    pub data: Option<Box<dyn Any>>,
    /// Precedence of expression operation relative to other expression (used
    /// for printing).
    pub precedence: u32,

    pub copyhdlr: Option<ExprCopyHdlr>,
    pub freehdlr: Option<ExprFreeHdlr>,
    pub copydata: Option<ExprCopyData>,
    pub freedata: Option<ExprFreeData>,
    pub simplify: Option<ExprSimplify>,
    pub compare: Option<ExprCompare>,
    /// The print callback exists (printing is not ported).
    pub print: bool,
    pub parse: Option<ExprParse>,
    pub eval: ExprEval,
    pub bwdiff: Option<ExprBwDiff>,
    pub fwdiff: Option<ExprFwDiff>,
    pub bwfwdiff: Option<ExprBwFwDiff>,
    pub inteval: Option<ExprIntEval>,
    pub estimate: Option<ExprEstimate>,
    pub initestimates: Option<ExprInitEstimates>,
    pub reverseprop: Option<ExprReverseProp>,
    pub hash: Option<ExprHash>,
    pub curvature: Option<ExprCurvature>,
    pub monotonicity: Option<ExprMonotonicity>,
    pub integrality: Option<ExprIntegrality>,
    /// The symmetry data callback exists (symmetry is not ported).
    pub getsymdata: bool,

    /* statistics */
    pub ncreated: u32,
    pub nestimatecalls: i64,
    pub nintevalcalls: i64,
    pub npropcalls: i64,
    pub ncutoffs: i64,
    pub ndomreds: i64,
    pub nsimplifycalls: i64,
    pub nsimplified: i64,
    pub nbranchscores: i64,
    pub estimatetime: Clock,
    pub intevaltime: Clock,
    pub proptime: Clock,
    pub simplifytime: Clock,
}

/// `SCIP_QUADEXPR_QUADTERM`: a single term associated to a quadratic
/// variable.
#[derive(Clone, Debug, Default)]
pub struct QuadExprQuadTerm {
    /// Expression of quadratic term.
    pub expr: ExprId,
    /// Linear coefficient of expression.
    pub lincoef: f64,
    /// Square coefficient of expression.
    pub sqrcoef: f64,
    /// Number of bilinear terms this expression is involved in.
    pub nadjbilin: i32,
    /// Size of adjacent bilinear terms array.
    pub adjbilinsize: i32,
    /// Indices of associated bilinear terms.
    pub adjbilin: Vec<i32>,
    /// Expression that was found to be the square of expr, or NULL if no
    /// square term (sqrcoef==0).
    pub sqrexpr: ExprId,
}

/// `SCIP_QUADEXPR_BILINTERM`: a single bilinear term.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadExprBilinTerm {
    /// First factor of bilinear term.
    pub expr1: ExprId,
    /// Second factor of bilinear term.
    pub expr2: ExprId,
    /// Coefficient of bilinear term.
    pub coef: f64,
    /// Position of expr2's quadexprterm in quadexprterms.
    pub pos2: i32,
    /// Expression that was found to be the product of expr1 and expr2.
    pub prodexpr: ExprId,
}

/// `SCIP_QUADEXPR`: representation of expression as quadratic.
#[derive(Clone, Debug, Default)]
pub struct QuadExpr {
    /// A constant term.
    pub constant: f64,
    /// Expressions of linear terms.
    pub linexprs: Vec<ExprId>,
    /// Coefficients of linear terms.
    pub lincoefs: Vec<f64>,
    /// Array with quadratic expression terms.
    pub quadexprterms: Vec<QuadExprQuadTerm>,
    /// Bilinear expression terms array.
    pub bilinexprterms: Vec<QuadExprBilinTerm>,
    /// Whether all arguments (linexprs, quadexprterms[.].expr) are variable
    /// expressions.
    pub allexprsarevars: bool,
    /// Curvature of the quadratic representation of the expression.
    pub curvature: ExprCurv,
    /// Whether curvature has been checked.
    pub curvaturechecked: bool,
    /// Whether the eigen information is stored.
    pub eigeninfostored: bool,
    /// Eigenvalues of the Q matrix: size of nquadexprs.
    pub eigenvalues: Option<Vec<f64>>,
    /// Eigenvalues of the Q matrix: size of nquadexprs^2.
    pub eigenvectors: Option<Vec<f64>>,
}

/// `SCIP_EXPR`: expression.
pub struct Expr {
    /// Expression type (as pointer to its handler).
    pub exprhdlr: ExprhdlrId,
    /// Expression data.
    pub exprdata: Option<Box<dyn Any>>,
    /// Children expressions (`nchildren` is the length; a released child is
    /// `NONE`).
    pub children: Vec<ExprId>,
    /// Reference counter.
    pub nuses: i32,
    /// Data for expression iterators.
    pub iterdata: [ExprIterData; EXPRITER_MAXNACTIVE],

    /* owner data */
    /// Data stored by owner of expression.
    pub ownerdata: Option<Box<dyn Any>>,
    /// Callback for freeing ownerdata.
    pub ownerfree: Option<ExprOwnerFree>,
    /// Callback for evaluating activity.
    pub ownerevalactivity: Option<ExprOwnerEvalActivity>,

    /* point-evaluation and differentiation */
    /// Value of expression from last evaluation (corresponding to evaltag).
    pub evalvalue: f64,
    /// Partial derivative of a "root path" w.r.t. this expression.
    pub derivative: f64,
    /// Directional derivative of this expr.
    pub dot: f64,
    /// Directional derivative of derivative of root (strictly speaking, a
    /// path) w.r.t this expr.
    pub bardot: f64,
    /// Tag of point for which the expression has been evaluated last, or 0.
    pub evaltag: i64,
    /// When computing partial derivatives of an expression w.r.t. a
    /// variable, the tag allows us to decide whether the expression depends
    /// on the variable.
    pub difftag: i64,

    /* interval-evaluation (activity) */
    /// Activity of expression with respect to variable bounds.
    pub activity: Interval,
    /// Tag of variable bounds for which activity is valid.
    pub activitytag: i64,

    /// Curvature of the expression w.r.t. bounds that have been used in the
    /// last curvature detection.
    pub curvature: ExprCurv,

    /// Indicates whether the expression is integral.
    pub integrality: ImplintType,

    /// Representation of expression as a quadratic, if checked and being
    /// quadratic.
    pub quaddata: Option<Box<QuadExpr>>,
    /// Whether it has been checked whether the expression is quadratic.
    pub quadchecked: bool,
}

/// `strcmp` of two names, as -1, 0 or 1.
fn strcmp(a: &str, b: &str) -> i32 {
    a.cmp(b) as i32
}

/* ---------------- expression handler (set.c) ---------------- */

/// `SCIPexprhdlrCreate`: creates an expression handler.
pub fn exprhdlr_create(scip: &mut Scip, name: &str, desc: Option<&str>, precedence: u32, eval: ExprEval, data: Option<Box<dyn Any>>) -> ExprhdlrId {
    scip.exprhdlrs.push(Exprhdlr {
        name: name.to_string(),
        desc: desc.map(str::to_string),
        data,
        precedence,
        copyhdlr: None,
        freehdlr: None,
        copydata: None,
        freedata: None,
        simplify: None,
        compare: None,
        print: false,
        parse: None,
        eval,
        bwdiff: None,
        fwdiff: None,
        bwfwdiff: None,
        inteval: None,
        estimate: None,
        initestimates: None,
        reverseprop: None,
        hash: None,
        curvature: None,
        monotonicity: None,
        integrality: None,
        getsymdata: false,
        ncreated: 0,
        nestimatecalls: 0,
        nintevalcalls: 0,
        npropcalls: 0,
        ncutoffs: 0,
        ndomreds: 0,
        nsimplifycalls: 0,
        nsimplified: 0,
        nbranchscores: 0,
        estimatetime: Clock::new(),
        intevaltime: Clock::new(),
        proptime: Clock::new(),
        simplifytime: Clock::new(),
    })
}

/// `SCIPexprhdlrFree`: frees expression handler.
pub fn exprhdlr_free(scip: &mut Scip, exprhdlr: ExprhdlrId) -> ScipResult {
    if let Some(freehdlr) = scip.exprhdlrs[exprhdlr].freehdlr {
        freehdlr(scip, exprhdlr)?;
    }
    Ok(())
}

/// `SCIPsetIncludeExprhdlr`: inserts an expression handler in expression
/// handler list.
pub fn set_include_exprhdlr(scip: &mut Scip, exprhdlr: ExprhdlrId) {
    let s = &mut scip.set;
    s.exprhdlrs.push(exprhdlr);
    s.exprhdlrssorted = false;

    let name = scip.exprhdlrs[exprhdlr].name.as_str();
    let s = &mut scip.set;
    if s.exprhdlrvar.is_none() && name == "var" {
        s.exprhdlrvar = Some(exprhdlr);
    } else if s.exprhdlrval.is_none() && name == "val" {
        s.exprhdlrval = Some(exprhdlr);
    } else if s.exprhdlrsum.is_none() && name == "sum" {
        s.exprhdlrsum = Some(exprhdlr);
    } else if s.exprhdlrproduct.is_none() && name == "prod" {
        s.exprhdlrproduct = Some(exprhdlr);
    } else if s.exprhdlrpow.is_none() && name == "pow" {
        s.exprhdlrpow = Some(exprhdlr);
    }
}

/// `SCIPsetFindExprhdlr`: returns the expression handler of the given name,
/// or `NONE` if not existing.
pub fn set_find_exprhdlr(scip: &Scip, name: &str) -> ExprhdlrId {
    for &h in &scip.set.exprhdlrs {
        if scip.exprhdlrs[h].name == name {
            return h;
        }
    }
    NONE
}

/// `SCIPsetSortExprhdlrs`: sorts expression handlers by name.
pub fn set_sort_exprhdlrs(scip: &mut Scip) {
    if !scip.set.exprhdlrssorted {
        let mut hs = std::mem::take(&mut scip.set.exprhdlrs);
        crate::sort::sort(&mut hs, (), |&a, &b| strcmp(&scip.exprhdlrs[a].name, &scip.exprhdlrs[b].name));
        scip.set.exprhdlrs = hs;
        scip.set.exprhdlrssorted = true;
    }
}

/// `SCIPexprhdlrCopyInclude`: calls the copy callback of an expression
/// handler.
pub fn exprhdlr_copy_include(target: &mut Scip, source: &Scip, exprhdlr: ExprhdlrId) -> ScipResult {
    if let Some(copyhdlr) = source.exprhdlrs[exprhdlr].copyhdlr {
        copyhdlr(target, source, exprhdlr)?;
    }
    Ok(())
}

/// `SCIPexprhdlrInit`: initialization of expression handler (resets
/// statistics).
pub fn exprhdlr_init(scip: &mut Scip, exprhdlr: ExprhdlrId) {
    if scip.set.misc_resetstat {
        let h = &mut scip.exprhdlrs[exprhdlr];
        h.ncreated = 0;
        h.nestimatecalls = 0;
        h.nintevalcalls = 0;
        h.npropcalls = 0;
        h.ncutoffs = 0;
        h.ndomreds = 0;
        h.nbranchscores = 0;
        h.nsimplifycalls = 0;
        h.nsimplified = 0;
        h.estimatetime.reset();
        h.intevaltime.reset();
        h.proptime.reset();
        h.simplifytime.reset();
    }
}

/// `SCIPexprhdlrComp`: comparison of expression handlers w.r.t. their name.
pub fn exprhdlr_comp(scip: &Scip, elem1: ExprhdlrId, elem2: ExprhdlrId) -> i32 {
    strcmp(&scip.exprhdlrs[elem1].name, &scip.exprhdlrs[elem2].name)
}

/// `SCIPexprhdlrParseExpr`: calls the parse callback of an expression
/// handler.
#[allow(clippy::too_many_arguments)]
pub fn exprhdlr_parse_expr(
    scip: &mut Scip,
    exprhdlr: ExprhdlrId,
    string: &[u8],
    pos: usize,
    endpos: &mut usize,
    expr: &mut ExprId,
    success: &mut bool,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    *expr = NONE;

    let Some(parse) = scip.exprhdlrs[exprhdlr].parse else {
        // TODO we could just look for a comma separated list of operands and
        // try to initialize the expr with this one? That would be sufficient
        // for sin, cos, exp, log, abs, for example.
        *success = false;
        return Ok(());
    };

    // Give control to exprhdlr's parser.
    parse(scip, exprhdlr, string, pos, endpos, expr, success, ownercreate, ownercreatedata)
}

/// `SCIPexprhdlrCurvatureExpr`: calls the curvature check callback of an
/// expression handler.
pub fn exprhdlr_curvature_expr(scip: &mut Scip, exprhdlr: ExprhdlrId, expr: ExprId, exprcurvature: ExprCurv, success: &mut bool, childcurv: &mut [ExprCurv]) -> ScipResult {
    *success = false;

    if let Some(curvature) = scip.exprhdlrs[exprhdlr].curvature {
        curvature(scip, expr, exprcurvature, success, childcurv)?;
    }

    Ok(())
}

/// `SCIPexprhdlrMonotonicityExpr`: calls the monotonicity check callback of
/// an expression handler.
pub fn exprhdlr_monotonicity_expr(scip: &mut Scip, exprhdlr: ExprhdlrId, expr: ExprId, childidx: i32, result: &mut Monotone) -> ScipResult {
    *result = Monotone::Unknown;

    // Check whether the expression handler implements the monotonicity
    // callback.
    if let Some(monotonicity) = scip.exprhdlrs[exprhdlr].monotonicity {
        monotonicity(scip, expr, childidx, result)?;
    }

    Ok(())
}

/// `SCIPexprhdlrIntegralityExpr`: calls the integrality check callback of
/// an expression handler.
pub fn exprhdlr_integrality_expr(scip: &mut Scip, exprhdlr: ExprhdlrId, expr: ExprId, integrality: &mut ImplintType) -> ScipResult {
    *integrality = ImplintType::None;

    // Check whether the expression handler implements the integrality
    // callback.
    if let Some(cb) = scip.exprhdlrs[exprhdlr].integrality {
        cb(scip, expr, integrality)?;
    }

    Ok(())
}

/// `SCIPexprhdlrHashExpr`: calls the hash callback of an expression
/// handler.
///
/// The method hashes an expression by taking the hashes of its children
/// into account.
pub fn exprhdlr_hash_expr(scip: &mut Scip, expr: ExprId, hashkey: &mut u32, childrenhashes: &[u32]) -> ScipResult {
    let h = scip.exprs[expr].exprhdlr;
    if let Some(hash) = scip.exprhdlrs[h].hash {
        hash(scip, expr, hashkey, childrenhashes)?;
    } else {
        // Compute initial hash from expression handler name if callback is not
        // implemented; this can lead to more collisions and thus a larger number
        // of expensive expression compare calls.
        *hashkey = 0;
        for &b in scip.exprhdlrs[h].name.as_bytes() {
            // C adds `(unsigned int) name[i]` with `char` signed.
            *hashkey = hashkey.wrapping_add(b as i8 as i32 as u32);
        }
        *hashkey = crate::misc::calc_fib_hash(*hashkey as f64);

        // Now make use of the hashkeys of the children.
        for childhash in &childrenhashes[..scip.exprs[expr].children.len()] {
            *hashkey ^= *childhash;
        }
    }

    Ok(())
}

/// `SCIPexprhdlrCompareExpr`: calls the compare callback of an expression
/// handler.
///
/// The method receives two expressions, expr1 and expr2, that must be of
/// the same expression handler.
pub fn exprhdlr_compare_expr(scip: &Scip, expr1: ExprId, expr2: ExprId) -> i32 {
    let h = scip.exprs[expr1].exprhdlr;
    if let Some(compare) = scip.exprhdlrs[h].compare {
        // Enforces OR1-OR4.
        return compare(scip, expr1, expr2);
    }

    // Enforces OR5: default comparison method of expressions of the same type:
    // expr1 < expr2 if and only if expr1_i = expr2_i for all i < k and expr1_k <
    // expr2_k. If there is no such k, use number of children to decide. If
    // number of children is equal, both expressions are equal.
    let (c1, c2) = (&scip.exprs[expr1].children, &scip.exprs[expr2].children);
    let mut i = 0;
    while i < c1.len() && i < c2.len() {
        let compareresult = expr_compare(scip, c1[i], c2[i]);
        if compareresult != 0 {
            return compareresult;
        }
        i += 1;
    }

    if c1.len() == c2.len() {
        0
    } else if c1.len() < c2.len() {
        -1
    } else {
        1
    }
}

/// `SCIPexprhdlrEvalExpr`: calls the evaluation callback of an expression
/// handler.
///
/// The method evaluates an expression by taking the values of its children
/// into account. Further, allows to evaluate w.r.t. given expression and
/// children values.
pub fn exprhdlr_eval_expr(scip: &mut Scip, exprhdlr: ExprhdlrId, expr: ExprId, val: &mut f64, childrenvals: Option<&[f64]>, sol: SolId) -> ScipResult {
    let mut origvals: Option<Vec<f64>> = None;

    // Temporarily overwrite the evalvalue in all children with values from
    // childrenvals.
    let nchildren = scip.exprs[expr].children.len();
    if let Some(cv) = childrenvals
        && nchildren > 0
    {
        let mut o = Vec::with_capacity(nchildren);
        for c in 0..nchildren {
            let child = scip.exprs[expr].children[c];
            o.push(scip.exprs[child].evalvalue);
            scip.exprs[child].evalvalue = cv[c];
        }
        origvals = Some(o);
    }

    // Call expression eval callback.
    (scip.exprhdlrs[exprhdlr].eval)(scip, expr, val, sol)?;

    // If there was some evaluation error (e.g., overflow) that hasn't been
    // caught yet, then do so now.
    if !val.is_finite() {
        *val = crate::def::INVALID;
    }

    // Restore original evalvalues in children.
    if let Some(o) = origvals {
        for (c, v) in o.into_iter().enumerate() {
            let child = scip.exprs[expr].children[c];
            scip.exprs[child].evalvalue = v;
        }
    }

    Ok(())
}

/// `SCIPexprhdlrBwDiffExpr`: calls the backward-differentiation callback of
/// an expression handler.
///
/// Further, allows to differentiate w.r.t. given expression and children
/// values.
#[allow(clippy::too_many_arguments)]
pub fn exprhdlr_bw_diff_expr(scip: &mut Scip, exprhdlr: ExprhdlrId, expr: ExprId, childidx: i32, derivative: &mut f64, childrenvals: Option<&[f64]>, exprval: f64) -> ScipResult {
    let Some(bwdiff) = scip.exprhdlrs[exprhdlr].bwdiff else {
        *derivative = crate::def::INVALID;
        return Ok(());
    };

    let mut origchildrenvals = Vec::new();
    let mut origexprval = crate::def::INVALID;
    if let Some(cv) = childrenvals {
        // Temporarily overwrite the evalvalue in all children and expr with
        // values from childrenvals and exprval, resp.
        for c in 0..scip.exprs[expr].children.len() {
            let child = scip.exprs[expr].children[c];
            origchildrenvals.push(scip.exprs[child].evalvalue);
            scip.exprs[child].evalvalue = cv[c];
        }

        origexprval = scip.exprs[expr].evalvalue;
        scip.exprs[expr].evalvalue = exprval;
    }

    // C calls the callback of expr's own handler here.
    let _ = bwdiff;
    let own = scip.exprhdlrs[scip.exprs[expr].exprhdlr].bwdiff.unwrap();
    own(scip, expr, childidx, derivative)?;

    // If there was some evaluation error (e.g., overflow) that hasn't been
    // caught yet, then do so now.
    if !derivative.is_finite() {
        *derivative = crate::def::INVALID;
    }

    // Restore original evalvalues in children.
    if childrenvals.is_some() {
        for (c, v) in origchildrenvals.into_iter().enumerate() {
            let child = scip.exprs[expr].children[c];
            scip.exprs[child].evalvalue = v;
        }

        scip.exprs[expr].evalvalue = origexprval;
    }

    Ok(())
}

/// `SCIPexprhdlrFwDiffExpr`: calls the forward differentiation callback of
/// an expression handler.
pub fn exprhdlr_fw_diff_expr(scip: &mut Scip, exprhdlr: ExprhdlrId, expr: ExprId, dot: &mut f64, direction: SolId) -> ScipResult {
    let Some(fwdiff) = scip.exprhdlrs[exprhdlr].fwdiff else {
        *dot = crate::def::INVALID;
        return Ok(());
    };

    fwdiff(scip, expr, dot, direction)?;

    // If there was some evaluation error (e.g., overflow) that hasn't been
    // caught yet, then do so now.
    if !dot.is_finite() {
        *dot = crate::def::INVALID;
    }

    Ok(())
}

/// `SCIPexprhdlrEvalFwDiffExpr`: calls the evaluation and forward-
/// differentiation callback of an expression handler.
///
/// The method evaluates an expression by taking the values of its children
/// into account and its forward derivative by taking the directional
/// derivatives of its children into account.
#[allow(clippy::too_many_arguments)]
pub fn exprhdlr_eval_fw_diff_expr(
    scip: &mut Scip,
    exprhdlr: ExprhdlrId,
    expr: ExprId,
    val: &mut f64,
    dot: &mut f64,
    childrenvals: Option<&[f64]>,
    sol: SolId,
    childrendirs: Option<&[f64]>,
    direction: SolId,
) -> ScipResult {
    let nchildren = scip.exprs[expr].children.len();
    let mut origvals: Option<Vec<f64>> = None;
    let mut origdots: Option<Vec<f64>> = None;

    // Temporarily overwrite the evalvalue in all children with values from
    // childrenvals.
    if let Some(cv) = childrenvals
        && nchildren > 0
    {
        let mut o = Vec::with_capacity(nchildren);
        for c in 0..nchildren {
            let child = scip.exprs[expr].children[c];
            o.push(scip.exprs[child].evalvalue);
            scip.exprs[child].evalvalue = cv[c];
        }
        origvals = Some(o);
    }

    // Temporarily overwrite the dot in all children with values from
    // childrendirs.
    if let Some(cd) = childrendirs
        && nchildren > 0
    {
        let mut o = Vec::with_capacity(nchildren);
        for c in 0..nchildren {
            let child = scip.exprs[expr].children[c];
            o.push(scip.exprs[child].dot);
            scip.exprs[child].dot = cd[c];
        }
        origdots = Some(o);
    }

    // Remember original value.
    let origval = scip.exprs[expr].evalvalue;

    // Call expression eval callback.
    (scip.exprhdlrs[exprhdlr].eval)(scip, expr, val, sol)?;

    // If there was some evaluation error (e.g., overflow) that hasn't been
    // caught yet, then do so now.
    if !val.is_finite() {
        *val = crate::def::INVALID;
    }

    // Temporarily overwrite evalvalue of expr, since some exprhdlr (e.g.,
    // product) access this value in fwdiff.
    scip.exprs[expr].evalvalue = *val;

    // Call forward-differentiation callback (if available).
    exprhdlr_fw_diff_expr(scip, exprhdlr, expr, dot, direction)?;

    // Restore original value.
    scip.exprs[expr].evalvalue = origval;

    // Restore original dots in children.
    if let Some(o) = origdots {
        for (c, v) in o.into_iter().enumerate() {
            let child = scip.exprs[expr].children[c];
            scip.exprs[child].dot = v;
        }
    }

    // Restore original evalvalues in children.
    if let Some(o) = origvals {
        for (c, v) in o.into_iter().enumerate() {
            let child = scip.exprs[expr].children[c];
            scip.exprs[child].evalvalue = v;
        }
    }

    Ok(())
}

/// `SCIPexprhdlrBwFwDiffExpr`: calls the evaluation callback for Hessian
/// directions (backward over forward) of an expression handler.
pub fn exprhdlr_bw_fw_diff_expr(scip: &mut Scip, exprhdlr: ExprhdlrId, expr: ExprId, childidx: i32, bardot: &mut f64, direction: SolId) -> ScipResult {
    if scip.exprhdlrs[exprhdlr].bwfwdiff.is_none() {
        *bardot = crate::def::INVALID;
        return Ok(());
    }

    // C calls the callback of expr's own handler here.
    let own = scip.exprhdlrs[scip.exprs[expr].exprhdlr].bwfwdiff.unwrap();
    own(scip, expr, childidx, bardot, direction)?;

    // If there was some evaluation error (e.g., overflow) that hasn't been
    // caught yet, then do so now.
    if !bardot.is_finite() {
        *bardot = crate::def::INVALID;
    }

    Ok(())
}

/// `SCIPexprhdlrIntEvalExpr`: calls the interval evaluation callback of an
/// expression handler.
pub fn exprhdlr_int_eval_expr(
    scip: &mut Scip,
    exprhdlr: ExprhdlrId,
    expr: ExprId,
    interval: &mut Interval,
    intevalvar: Option<ExprIntEvalVar>,
    intevalvardata: usize,
) -> ScipResult {
    if let Some(inteval) = scip.exprhdlrs[exprhdlr].inteval {
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[exprhdlr].intevaltime.start(en);
        }
        inteval(scip, expr, interval, intevalvar, intevalvardata)?;
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[exprhdlr].intevaltime.stop(en);
        }

        scip.exprhdlrs[exprhdlr].nintevalcalls += 1;
    }

    Ok(())
}

/// `SCIPexprhdlrEstimateExpr`: calls the estimator callback of an
/// expression handler.
#[allow(clippy::too_many_arguments)]
pub fn exprhdlr_estimate_expr(
    scip: &mut Scip,
    exprhdlr: ExprhdlrId,
    expr: ExprId,
    localbounds: &[Interval],
    globalbounds: &[Interval],
    refpoint: &[f64],
    overestimate: bool,
    targetvalue: f64,
    coefs: &mut [f64],
    constant: &mut f64,
    islocal: &mut bool,
    success: &mut bool,
    branchcand: &mut [bool],
) -> ScipResult {
    *success = false;

    if let Some(estimate) = scip.exprhdlrs[exprhdlr].estimate {
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[exprhdlr].estimatetime.start(en);
        }
        estimate(
            scip,
            expr,
            localbounds,
            globalbounds,
            refpoint,
            overestimate,
            targetvalue,
            coefs,
            constant,
            islocal,
            success,
            branchcand,
        )?;
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[exprhdlr].estimatetime.stop(en);
        }

        // Update statistics.
        scip.exprhdlrs[exprhdlr].nestimatecalls += 1;
    }

    Ok(())
}

/// `SCIPexprhdlrInitEstimatesExpr`: calls the intitial estimators callback
/// of an expression handler.
#[allow(clippy::too_many_arguments)]
pub fn exprhdlr_init_estimates_expr(
    scip: &mut Scip,
    exprhdlr: ExprhdlrId,
    expr: ExprId,
    bounds: &[Interval],
    overestimate: bool,
    coefs: &mut [Vec<f64>],
    constant: &mut [f64],
    nreturned: &mut i32,
) -> ScipResult {
    *nreturned = 0;

    if let Some(initestimates) = scip.exprhdlrs[exprhdlr].initestimates {
        let h = scip.exprs[expr].exprhdlr;
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[h].estimatetime.start(en);
        }
        initestimates(scip, expr, bounds, overestimate, coefs, constant, nreturned)?;
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[h].estimatetime.stop(en);
        }

        scip.exprhdlrs[exprhdlr].nestimatecalls += 1;
    }

    Ok(())
}

/// `SCIPexprhdlrSimplifyExpr`: calls the simplification callback of an
/// expression handler.
pub fn exprhdlr_simplify_expr(
    scip: &mut Scip,
    exprhdlr: ExprhdlrId,
    expr: ExprId,
    simplifiedexpr: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    if let Some(simplify) = scip.exprhdlrs[exprhdlr].simplify {
        let h = scip.exprs[expr].exprhdlr;
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[h].simplifytime.start(en);
        }
        simplify(scip, expr, simplifiedexpr, ownercreate, ownercreatedata)?;
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[h].simplifytime.stop(en);
        }

        // Update statistics.
        scip.exprhdlrs[exprhdlr].nsimplifycalls += 1;
        if expr != *simplifiedexpr {
            scip.exprhdlrs[exprhdlr].nsimplified += 1;
        }
    } else {
        *simplifiedexpr = expr;

        // If an expression handler doesn't implement simplify, we assume that it
        // is already simplified; we have to capture it, since it must simulate a
        // "normal" simplified call in which a new expression is created.
        expr_capture(scip, expr);
    }

    Ok(())
}

/// `SCIPexprhdlrReversePropExpr`: calls the reverse propagation callback of
/// an expression handler.
///
/// The method propagates given bounds over the children of an expression.
pub fn exprhdlr_reverse_prop_expr(scip: &mut Scip, exprhdlr: ExprhdlrId, expr: ExprId, bounds: Interval, childrenbounds: &mut [Interval], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    if let Some(reverseprop) = scip.exprhdlrs[exprhdlr].reverseprop {
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[exprhdlr].proptime.start(en);
        }
        reverseprop(scip, expr, bounds, childrenbounds, infeasible)?;
        {
            let en = scip.set.time_enabled;
            scip.exprhdlrs[exprhdlr].proptime.stop(en);
        }

        // Update statistics.
        let h = scip.exprs[expr].exprhdlr;
        if *infeasible {
            scip.exprhdlrs[h].ncutoffs += 1;
        }
        scip.exprhdlrs[h].npropcalls += 1;
    }

    Ok(())
}

/* ---------------- expressions ---------------- */

/// `SCIPexprCreate`: creates and captures an expression with given
/// expression data and children.
pub fn expr_create(
    scip: &mut Scip,
    exprhdlr: ExprhdlrId,
    exprdata: Option<Box<dyn Any>>,
    children: &[ExprId],
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult<ExprId> {
    let mut activity = Interval::default();
    // Initialize activity to entire interval.
    activity.set_entire(INTERVAL_INFINITY);

    let expr = scip.exprs.push(Expr {
        exprhdlr,
        exprdata,
        children: children.to_vec(),
        nuses: 0,
        iterdata: [ExprIterData::default(); EXPRITER_MAXNACTIVE],
        ownerdata: None,
        ownerfree: None,
        ownerevalactivity: None,
        evalvalue: 0.0,
        derivative: 0.0,
        dot: 0.0,
        bardot: 0.0,
        evaltag: 0,
        difftag: 0,
        activity,
        // To be less than initial domchgcount.
        activitytag: -1,
        curvature: ExprCurv::Unknown,
        integrality: ImplintType::None,
        quaddata: None,
        quadchecked: false,
    });

    for &c in children {
        expr_capture(scip, c);
    }

    expr_capture(scip, expr);

    scip.exprhdlrs[exprhdlr].ncreated += 1;

    // Initializes the ownerdata.
    if let Some(ownercreate) = ownercreate {
        ownercreate(scip, expr, ownercreatedata)?;
    }

    Ok(expr)
}

/// `SCIPexprAppendChild`: appends child to the children list of expr.
pub fn expr_append_child(scip: &mut Scip, expr: ExprId, child: ExprId) {
    scip.exprs[expr].children.push(child);

    // Capture child.
    expr_capture(scip, child);
}

/// `SCIPexprReplaceChild`: overwrites/replaces a child of an expressions.
///
/// @note the old child is released and the newchild is captured, unless
/// they are the same (=same pointer).
pub fn expr_replace_child(scip: &mut Scip, expr: ExprId, childidx: usize, newchild: ExprId) -> ScipResult {
    // Do nothing if child is not changing.
    if newchild == scip.exprs[expr].children[childidx] {
        return Ok(());
    }

    // Capture new child (do this before releasing the old child in case there
    // are equal).
    expr_capture(scip, newchild);

    let mut old = scip.exprs[expr].children[childidx];
    expr_release(scip, &mut old)?;
    scip.exprs[expr].children[childidx] = newchild;

    Ok(())
}

/// `SCIPexprRemoveChildren`: remove all children of expr.
pub fn expr_remove_children(scip: &mut Scip, expr: ExprId) -> ScipResult {
    for c in 0..scip.exprs[expr].children.len() {
        let mut child = scip.exprs[expr].children[c];
        expr_release(scip, &mut child)?;
    }
    scip.exprs[expr].children.clear();

    Ok(())
}

/// `SCIPexprDuplicateShallow`: duplicates the given expression without its
/// children.
pub fn expr_duplicate_shallow(scip: &mut Scip, expr: ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<ExprId> {
    let h = scip.exprs[expr].exprhdlr;

    // Copy expression data.
    let exprdatacopy = if scip.exprs[expr].exprdata.is_some() {
        Some((scip.exprhdlrs[h].copydata.unwrap())(scip, expr)?)
    } else {
        None
    };

    // Create expression with same handler and copied data, but without
    // children.
    expr_create(scip, h, exprdatacopy, &[], ownercreate, ownercreatedata)
}

/// `SCIPexprCopy` within one SCIP (`SCIPduplicateExpr`): copies an
/// expression including children. A `mapexpr` that returns an expression
/// (other than `NONE`) supplies the copy of that subexpression.
pub fn expr_copy(
    scip: &mut Scip,
    sourceexpr: ExprId,
    mapexpr: Option<ExprMapExpr>,
    mapexprdata: usize,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult<ExprId> {
    let mut it = ExprIter::create();
    it.init(scip, sourceexpr, ExprIterType::Dfs, true)?; // TODO use FALSE, i.e., don't duplicate common subexpr?
    it.set_stages_dfs(scip, EXPRITER_ENTEREXPR | EXPRITER_VISITEDCHILD);

    let mut expr = sourceexpr;
    while !it.is_end() {
        match it.get_stage_dfs() {
            EXPRITER_ENTEREXPR => {
                // Create expr that will hold the copy.
                if let Some(mapexpr) = mapexpr {
                    let exprcopy = mapexpr(scip, expr, ownercreate, ownercreatedata, mapexprdata)?;
                    if exprcopy != NONE {
                        // Map callback gave us an expression to use for the copy; store
                        // targetexpr.
                        it.set_current_user_data(scip, ExprIterUserData::from_ptr(exprcopy));

                        // Skip subexpression (assume that exprcopy is a complete copy)
                        // and continue.
                        expr = it.skip_dfs(scip);
                        continue;
                    }
                }

                let targetexprhdlr = scip.exprs[expr].exprhdlr;

                // Copy expression data.
                let targetexprdata = if scip.exprs[expr].exprdata.is_some() {
                    Some((scip.exprhdlrs[targetexprhdlr].copydata.unwrap())(scip, expr)?)
                } else {
                    None
                };

                // Create in targetexpr an expression of the same type as expr, but
                // without children for now.
                let exprcopy = expr_create(scip, targetexprhdlr, targetexprdata, &[], ownercreate, ownercreatedata)?;

                // Store targetexpr.
                it.set_current_user_data(scip, ExprIterUserData::from_ptr(exprcopy));
            }
            EXPRITER_VISITEDCHILD => {
                // Just visited child so a copy of himself should be available;
                // append it.
                let mut exprcopy = it.get_current_user_data(scip).ptrval();

                // Get copy of child.
                let mut childcopy = it.get_child_user_data_dfs(scip).ptrval();
                if childcopy == NONE {
                    // Abort: release exprcopy (should free also the already copied
                    // children).
                    expr_release(scip, &mut exprcopy)?;

                    it.set_current_user_data(scip, ExprIterUserData::from_ptr(NONE));

                    expr = it.skip_dfs(scip);
                    continue;
                }

                // Append child to exprcopy.
                expr_append_child(scip, exprcopy, childcopy);

                // Release childcopy (still captured by exprcopy).
                expr_release(scip, &mut childcopy)?;
            }
            _ => unreachable!("we should never be called in this stage"),
        }

        expr = it.get_next(scip);
    }

    // The target expression should be stored in the userdata of the
    // sourceexpr (can be NULL if aborted).
    let targetexpr = it.get_expr_user_data(scip, sourceexpr).ptrval();

    it.free(scip);

    Ok(targetexpr)
}

/// `SCIPexprCapture`: captures an expression (increments usage count).
#[inline]
pub fn expr_capture(scip: &mut Scip, expr: ExprId) {
    scip.exprs[expr].nuses += 1;
}

/// `freeExpr`: frees an expression.
fn free_expr(scip: &mut Scip, expr: &mut ExprId) {
    scip.exprs.remove(*expr);
    *expr = NONE;
}

/// The owner, quadratic and expression data of an expression being freed.
fn free_expr_data(scip: &mut Scip, expr: ExprId) -> ScipResult {
    // Call ownerfree callback, if given; we intentially call this also if
    // ownerdata is NULL, so owner can be notified without storing data.
    if let Some(ownerfree) = scip.exprs[expr].ownerfree {
        ownerfree(scip, expr)?;
    }

    // Free quadratic info.
    expr_free_quadratic(scip, expr);

    // Free expression data.
    if scip.exprs[expr].exprdata.is_some() {
        let h = scip.exprs[expr].exprhdlr;
        (scip.exprhdlrs[h].freedata.unwrap())(scip, expr)?;
    }

    Ok(())
}

/// `SCIPexprRelease`: releases an expression (decrements usage count and
/// possibly frees expression).
pub fn expr_release(scip: &mut Scip, expr: &mut ExprId) -> ScipResult {
    if scip.exprs[*expr].nuses > 1 {
        scip.exprs[*expr].nuses -= 1;
        *expr = NONE;
        return Ok(());
    }

    // Handle the root expr separately: free ownerdata, quaddata, and exprdata
    // first.
    free_expr_data(scip, *expr)?;

    // Now release and free children, where no longer in use.
    let mut it = ExprIter::create();
    it.init(scip, *expr, ExprIterType::Dfs, true)?;
    it.set_stages_dfs(scip, EXPRITER_VISITINGCHILD | EXPRITER_VISITEDCHILD);
    let mut expr2 = it.get_current();
    while !it.is_end() {
        // Expression should be used by its parent and maybe by the iterator
        // (only the root!); in VISITEDCHILD we assert that expression is only
        // used by its parent.
        match it.get_stage_dfs() {
            EXPRITER_VISITINGCHILD => {
                // Check whether a child needs to be visited (nuses == 1); if not,
                // then we still have to release it.
                let mut child = it.get_child_expr_dfs(scip);
                if scip.exprs[child].nuses > 1 {
                    // Child is not going to be freed: just release it.
                    expr_release(scip, &mut child)?;
                    expr2 = it.skip_dfs(scip);
                    continue;
                }

                // Free child's quaddata, ownerdata, and exprdata when entering
                // child.
                free_expr_data(scip, child)?;
            }
            EXPRITER_VISITEDCHILD => {
                // Free child after visiting it.
                let mut child = it.get_child_expr_dfs(scip);
                let idx = it.get_child_idx_dfs(scip) as usize;

                // Free child expression.
                free_expr(scip, &mut child);
                scip.exprs[expr2].children[idx] = NONE;
            }
            _ => unreachable!("we should never be called in this stage"),
        }
        expr2 = it.get_next(scip);
    }
    it.free(scip);

    // Handle the root expr separately: free its children and itself here.
    free_expr(scip, expr);

    Ok(())
}

/// `SCIPexprIsVar`: returns whether an expression is a variable
/// expression.
#[inline]
pub fn expr_is_var(scip: &Scip, expr: ExprId) -> bool {
    Some(scip.exprs[expr].exprhdlr) == scip.set.exprhdlrvar
}

/// `SCIPexprIsValue`: returns whether an expression is a value expression.
#[inline]
pub fn expr_is_value(scip: &Scip, expr: ExprId) -> bool {
    Some(scip.exprs[expr].exprhdlr) == scip.set.exprhdlrval
}

/// `SCIPexprIsSum`: returns whether an expression is a sum expression.
#[inline]
pub fn expr_is_sum(scip: &Scip, expr: ExprId) -> bool {
    Some(scip.exprs[expr].exprhdlr) == scip.set.exprhdlrsum
}

/// `SCIPexprIsProduct`: returns whether an expression is a product
/// expression.
#[inline]
pub fn expr_is_product(scip: &Scip, expr: ExprId) -> bool {
    Some(scip.exprs[expr].exprhdlr) == scip.set.exprhdlrproduct
}

/// `SCIPexprIsPower`: returns whether an expression is a power expression.
#[inline]
pub fn expr_is_power(scip: &Scip, expr: ExprId) -> bool {
    Some(scip.exprs[expr].exprhdlr) == scip.set.exprhdlrpow
}

/// `evalAndDiff`: evaluate and forward-differentiate expression; also
/// initializes derivative and bardot to 0.
fn eval_and_diff(scip: &mut Scip, mut expr: ExprId, sol: SolId, soltag: i64, direction: SolId) -> ScipResult {
    // Assume we'll get a domain error, so we don't have to get this expr back
    // if we abort the iteration; if there is no domain error, then we will
    // overwrite the evalvalue in the last leaveexpr stage.
    let e = &mut scip.exprs[expr];
    e.evalvalue = crate::def::INVALID;
    e.evaltag = soltag;
    e.dot = crate::def::INVALID;

    // Start a new difftag.
    scip.stat.exprlastdifftag += 1;

    let mut it = ExprIter::create();
    it.init(scip, expr, ExprIterType::Dfs, true)?;
    it.set_stages_dfs(scip, EXPRITER_LEAVEEXPR);

    expr = it.get_current();
    while !it.is_end() {
        // Evaluate expression only if necessary.
        if soltag == 0 || scip.exprs[expr].evaltag != soltag {
            let h = scip.exprs[expr].exprhdlr;
            let mut v = 0.0;
            exprhdlr_eval_expr(scip, h, expr, &mut v, None, sol)?;
            scip.exprs[expr].evalvalue = v;
            scip.exprs[expr].evaltag = soltag;
        }

        if scip.exprs[expr].evalvalue == crate::def::INVALID {
            break;
        }

        if scip.exprs[expr].difftag != scip.stat.exprlastdifftag {
            // Compute forward diff.
            let h = scip.exprs[expr].exprhdlr;
            let mut d = 0.0;
            exprhdlr_fw_diff_expr(scip, h, expr, &mut d, direction)?;
            scip.exprs[expr].dot = d;

            if d == crate::def::INVALID {
                break;
            }

            let e = &mut scip.exprs[expr];
            e.derivative = 0.0;
            e.bardot = 0.0;
            e.difftag = scip.stat.exprlastdifftag;
        }
        expr = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `SCIPexprEval`: evaluate an expression in a point.
///
/// Iterates over expressions to also evaluate children, if necessary. Value
/// can be received via `SCIPexprGetEvalValue()`. If an evaluation error
/// (division by zero, ...) occurs, this value will be set to
/// `SCIP_INVALID`.
///
/// If a nonzero soltag is passed, then only (sub)expressions are reevaluated
/// that have a different solution tag. If a soltag of 0 is passed, then
/// subexpressions are always reevaluated. The tag is stored together with
/// the value and can be received via `SCIPexprGetEvalTag()`.
pub fn expr_eval(scip: &mut Scip, mut expr: ExprId, sol: SolId, soltag: i64) -> ScipResult {
    // If value is up-to-date, then nothing to do.
    if soltag != 0 && scip.exprs[expr].evaltag == soltag {
        return Ok(());
    }

    // Assume we'll get a domain error, so we don't have to get this expr back
    // if we abort the iteration; if there is no domain error, then we will
    // overwrite the evalvalue in the last leaveexpr stage.
    scip.exprs[expr].evalvalue = crate::def::INVALID;
    scip.exprs[expr].evaltag = soltag;

    let mut it = ExprIter::create();
    it.init(scip, expr, ExprIterType::Dfs, true)?;
    it.set_stages_dfs(scip, EXPRITER_VISITINGCHILD | EXPRITER_LEAVEEXPR);

    'terminate: while !it.is_end() {
        match it.get_stage_dfs() {
            EXPRITER_VISITINGCHILD => {
                if soltag != 0 {
                    // Check whether child has been evaluated for that solution
                    // already.
                    let child = it.get_child_expr_dfs(scip);
                    if soltag == scip.exprs[child].evaltag {
                        if scip.exprs[child].evalvalue == crate::def::INVALID {
                            break 'terminate;
                        }

                        // Skip this child; this already returns the next one, so
                        // continue with loop.
                        expr = it.skip_dfs(scip);
                        continue;
                    }
                }
            }
            EXPRITER_LEAVEEXPR => {
                let h = scip.exprs[expr].exprhdlr;
                let mut v = 0.0;
                exprhdlr_eval_expr(scip, h, expr, &mut v, None, sol)?;
                scip.exprs[expr].evalvalue = v;
                scip.exprs[expr].evaltag = soltag;

                if v == crate::def::INVALID {
                    break 'terminate;
                }
            }
            _ => unreachable!("we should never be here"),
        }
        expr = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `SCIPexprEvalGradient`: evaluates gradient of an expression for a given
/// point.
///
/// Initiates an expression walk to also evaluate children, if necessary.
/// Value can be received via `SCIPgetExprPartialDiffNonlinear()`. If an
/// error (division by zero, ...) occurs, this value will be set to
/// `SCIP_INVALID`.
pub fn expr_eval_gradient(scip: &mut Scip, rootexpr: ExprId, sol: SolId, soltag: i64) -> ScipResult {
    // Ensure expression is evaluated.
    expr_eval(scip, rootexpr, sol, soltag)?;

    // Check if expression could not be evaluated.
    if scip.exprs[rootexpr].evalvalue == crate::def::INVALID {
        scip.exprs[rootexpr].derivative = crate::def::INVALID;
        return Ok(());
    }

    if expr_is_value(scip, rootexpr) {
        scip.exprs[rootexpr].derivative = 0.0;
        return Ok(());
    }

    scip.stat.exprlastdifftag += 1;
    let difftag = scip.stat.exprlastdifftag;

    scip.exprs[rootexpr].derivative = 1.0;
    scip.exprs[rootexpr].difftag = difftag;

    let mut it = ExprIter::create();
    it.init(scip, rootexpr, ExprIterType::Dfs, true)?;
    it.set_stages_dfs(scip, EXPRITER_VISITINGCHILD);

    let mut expr = it.get_current();
    while !it.is_end() {
        let child = it.get_child_expr_dfs(scip);

        // Reset the value of the partial derivative w.r.t. a variable expression
        // if we see it for the first time.
        if scip.exprs[child].difftag != difftag && expr_is_var(scip, child) {
            scip.exprs[child].derivative = 0.0;
        }

        // Update differentiation tag of the child.
        scip.exprs[child].difftag = difftag;

        // Call backward differentiation callback.

        let derivative = if expr_is_value(scip, child) {
            0.0
        } else {
            let mut d = crate::def::INVALID;
            let h = scip.exprs[expr].exprhdlr;
            let idx = it.get_child_idx_dfs(scip);
            exprhdlr_bw_diff_expr(scip, h, expr, idx, &mut d, None, 0.0)?;

            if d == crate::def::INVALID {
                scip.exprs[rootexpr].derivative = crate::def::INVALID;
                break;
            }
            d
        };

        // Update partial derivative stored in the child expression: for a
        // variable, we have to sum up the partial derivatives of the root w.r.t.
        // this variable over all parents; for other intermediate expressions, we
        // only store the partial derivative of the root w.r.t. this expression.
        let pd = scip.exprs[expr].derivative;
        if !expr_is_var(scip, child) {
            scip.exprs[child].derivative = pd * derivative;
        } else {
            scip.exprs[child].derivative += pd * derivative;
        }

        // Even with expr->derivative and derivative being finite, the product
        // may under- or overflow.
        if !scip.exprs[child].derivative.is_finite() {
            scip.exprs[child].derivative = crate::def::INVALID;
            scip.exprs[rootexpr].derivative = crate::def::INVALID;
            break;
        }
        expr = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `SCIPexprEvalHessianDir`: evaluates Hessian-vector product of an
/// expression for a given point and direction.
///
/// Evaluates children, if necessary. Value can be received via
/// `SCIPgetExprPartialDiffGradientDirNonlinear()`. If an error (division by
/// zero, ...) occurs, this value will be set to `SCIP_INVALID`.
pub fn expr_eval_hessian_dir(scip: &mut Scip, rootexpr: ExprId, sol: SolId, soltag: i64, direction: SolId) -> ScipResult {
    if expr_is_value(scip, rootexpr) {
        scip.exprs[rootexpr].dot = 0.0;
        scip.exprs[rootexpr].bardot = 0.0;
        return Ok(());
    }

    // Evaluate expression and directional derivative.
    eval_and_diff(scip, rootexpr, sol, soltag, direction)?;

    if scip.exprs[rootexpr].evalvalue == crate::def::INVALID {
        scip.exprs[rootexpr].derivative = crate::def::INVALID;
        scip.exprs[rootexpr].bardot = crate::def::INVALID;
        return Ok(());
    }

    scip.exprs[rootexpr].derivative = 1.0;

    let mut it = ExprIter::create();
    it.init(scip, rootexpr, ExprIterType::Dfs, true)?;
    it.set_stages_dfs(scip, EXPRITER_VISITINGCHILD);

    // Compute reverse diff and bardots: i.e. hessian times direction.
    let mut expr = it.get_current();
    while !it.is_end() {
        let child = it.get_child_expr_dfs(scip);

        // Call backward and forward-backward differentiation callback.
        let (derivative, hessiandir);
        if expr_is_value(scip, child) {
            derivative = 0.0;
            hessiandir = 0.0;
        } else {
            let mut d = crate::def::INVALID;
            let mut hd = crate::def::INVALID;
            let h = scip.exprs[expr].exprhdlr;
            let idx = it.get_child_idx_dfs(scip);
            exprhdlr_bw_diff_expr(scip, h, expr, idx, &mut d, None, crate::def::INVALID)?;
            exprhdlr_bw_fw_diff_expr(scip, h, expr, idx, &mut hd, NONE)?;

            if d == crate::def::INVALID || hd == crate::def::INVALID {
                scip.exprs[rootexpr].derivative = crate::def::INVALID;
                scip.exprs[rootexpr].bardot = crate::def::INVALID;
                break;
            }
            derivative = d;
            hessiandir = hd;
        }

        // Update partial derivative and hessian stored in the child expression:
        // for a variable, we have to sum up the partial derivatives of the root
        // w.r.t. this variable over all parents; for other intermediate
        // expressions, we only store the partial derivative of the root w.r.t.
        // this expression.
        let (pd, pb) = (scip.exprs[expr].derivative, scip.exprs[expr].bardot);
        if !expr_is_var(scip, child) {
            scip.exprs[child].derivative = pd * derivative;
            scip.exprs[child].bardot = pb * derivative + pd * hessiandir;
        } else {
            scip.exprs[child].derivative += pd * derivative;
            scip.exprs[child].bardot += pb * derivative + pd * hessiandir;
        }
        expr = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `SCIPexprEvalActivity`: possibly reevaluates and then returns the
/// activity of the expression.
///
/// Reevaluate activity if currently stored is no longer uptodate. If the
/// expr owner provided a evalactivity-callback, then call this. Otherwise,
/// loop over descendants and compare activitytag with stat's domchgcount,
/// i.e., whether some bound was changed since last evaluation, to check
/// whether exprhdlrs INTEVAL should be called.
///
/// @note If expression is set to be integral, then activities are tightened
/// to integral values. Thus, ensure that the integrality information is
/// valid (if set to TRUE; the default (FALSE) is always ok).
pub fn expr_eval_activity(scip: &mut Scip, rootexpr: ExprId) -> ScipResult {
    if let Some(ownerevalactivity) = scip.exprs[rootexpr].ownerevalactivity {
        // Call owner callback for activity-eval.
        ownerevalactivity(scip, rootexpr)?;

        return Ok(());
    }

    // Fallback if no callback is given.

    // If value is up-to-date, then nothing to do.
    if scip.exprs[rootexpr].activitytag == scip.stat.domchgcount {
        return Ok(());
    }

    let mut it = ExprIter::create();
    it.init(scip, rootexpr, ExprIterType::Dfs, true)?;
    it.set_stages_dfs(scip, EXPRITER_VISITINGCHILD | EXPRITER_LEAVEEXPR);

    let mut expr = it.get_current();
    while !it.is_end() {
        match it.get_stage_dfs() {
            EXPRITER_VISITINGCHILD => {
                // Skip child if it has been evaluated already.
                let child = it.get_child_expr_dfs(scip);
                if scip.exprs[child].activitytag == scip.stat.domchgcount {
                    expr = it.skip_dfs(scip);
                    continue;
                }
            }
            EXPRITER_LEAVEEXPR => {
                // We should not have entered this expression if its activity was
                // already uptodate.

                // Reset activity to entire if invalid, so we can use it as
                // starting point below.
                let mut activity = Interval::default();
                activity.set_entire(INTERVAL_INFINITY);
                scip.exprs[expr].activity = activity;

                // Call the inteval callback of the exprhdlr.
                let h = scip.exprs[expr].exprhdlr;
                exprhdlr_int_eval_expr(scip, h, expr, &mut activity, None, 0)?;

                // If expression is integral, then we try to tighten the interval
                // bounds a bit; this should undo the addition of some unnecessary
                // safety added by use of nextafter() in interval arithmetics, e.g.,
                // when doing pow() it would be ok to use ceil() and floor(), but for
                // safety we use SCIPceil and SCIPfloor for now the default
                // intevalVar does not relax variables, so can omit expressions
                // without children (constants should be ok, too).
                if scip.exprs[expr].integrality != ImplintType::None && !scip.exprs[expr].children.is_empty() {
                    if activity.inf > -INTERVAL_INFINITY {
                        activity.inf = scip.set.ceil(activity.inf);
                    }
                    if activity.sup < INTERVAL_INFINITY {
                        activity.sup = scip.set.floor(activity.sup);
                    }
                }

                // Mark activity as empty if either the lower/upper bound is
                // above/below +/- SCIPinfinity(). TODO this is a problem if
                // dual-presolve fixed a variable to +/- infinity.
                if scip.set.is_infinity(activity.inf) || scip.set.is_infinity(-activity.sup) {
                    activity.set_empty();
                }

                // Remember that activity is uptodate now.
                scip.exprs[expr].activity = activity;
                scip.exprs[expr].activitytag = scip.stat.domchgcount;
            }
            _ => unreachable!("you should never be here"),
        }
        expr = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `SCIPexprCompare`: compare expressions.
///
/// Returns -1, 0 or 1 if expr1 <, =, > expr2, respectively. See the
/// expression ordering rules OR1-OR12 of `expr.c`.
pub fn expr_compare(scip: &Scip, expr1: ExprId, expr2: ExprId) -> i32 {
    let exprhdlr1 = scip.exprs[expr1].exprhdlr;
    let exprhdlr2 = scip.exprs[expr2].exprhdlr;

    // Expressions are of the same kind/type; use compare callback or default
    // method.
    if exprhdlr1 == exprhdlr2 {
        return exprhdlr_compare_expr(scip, expr1, expr2);
    }

    // Expressions are of different kind/type.
    // Enforces OR6.
    if expr_is_value(scip, expr1) {
        return -1;
    }
    // Enforces OR12.
    if expr_is_value(scip, expr2) {
        return -expr_compare(scip, expr2, expr1);
    }

    // Enforces OR7.
    if expr_is_sum(scip, expr1) {
        let nchildren = scip.exprs[expr1].children.len();
        let compareresult = expr_compare(scip, scip.exprs[expr1].children[nchildren - 1], expr2);

        if compareresult != 0 {
            return compareresult;
        }

        // "base" of the largest expression of the sum is equal to expr2,
        // coefficient might tell us that expr2 is larger.
        if crate::expr_sum::get_coefs_expr_sum(scip, expr1)[nchildren - 1] < 1.0 {
            return -1;
        }

        // Largest expression of sum is larger or equal than expr2 => expr1 >
        // expr2.
        return 1;
    }
    // Enforces OR12.
    if expr_is_sum(scip, expr2) {
        return -expr_compare(scip, expr2, expr1);
    }

    // Enforces OR8.
    if expr_is_product(scip, expr1) {
        let nchildren = scip.exprs[expr1].children.len();
        let compareresult = expr_compare(scip, scip.exprs[expr1].children[nchildren - 1], expr2);

        if compareresult != 0 {
            return compareresult;
        }

        // Largest expression of product is larger or equal than expr2 => expr1 >
        // expr2.
        return 1;
    }
    // Enforces OR12.
    if expr_is_product(scip, expr2) {
        return -expr_compare(scip, expr2, expr1);
    }

    // Enforces OR9.
    if expr_is_power(scip, expr1) {
        let compareresult = expr_compare(scip, scip.exprs[expr1].children[0], expr2);

        if compareresult != 0 {
            return compareresult;
        }

        // Base equal to expr2, exponent might tell us that expr2 is larger.
        if crate::expr_pow::get_exponent_expr_pow(scip, expr1) < 1.0 {
            return -1;
        }

        // Power expression is larger => expr1 > expr2.
        return 1;
    }
    // Enforces OR12.
    if expr_is_power(scip, expr2) {
        return -expr_compare(scip, expr2, expr1);
    }

    // Enforces OR10.
    if expr_is_var(scip, expr1) {
        return -1;
    }
    // Enforces OR12.
    if expr_is_var(scip, expr2) {
        return -expr_compare(scip, expr2, expr1);
    }

    // Enforces OR11.
    let retval = strcmp(&scip.exprhdlrs[exprhdlr1].name, &scip.exprhdlrs[exprhdlr2].name);
    if retval == 0 {
        0
    } else if retval < 0 {
        -1
    } else {
        1
    }
}

/// `SCIPexprSimplify`: simplifies an expression.
///
/// See the SCIP_DECL_EXPRSIMPLIFY documentation; returns `(simplified,
/// changed, infeasible)`.
pub fn expr_simplify(scip: &mut Scip, rootexpr: ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<(ExprId, bool, bool)> {
    // Simplify bottom up: when leaving an expression it simplifies it and
    // stores the simplified expr in its iterators expression data; after the
    // child was visited, it is replaced with the simplified expr.
    let mut it = ExprIter::create();
    it.init(scip, rootexpr, ExprIterType::Dfs, true)?; // TODO can we set allowrevisited to FALSE?
    it.set_stages_dfs(scip, EXPRITER_VISITEDCHILD | EXPRITER_LEAVEEXPR);

    let mut changed = false;
    let infeasible = false;

    let mut expr = it.get_current();
    while !it.is_end() {
        match it.get_stage_dfs() {
            EXPRITER_VISITEDCHILD => {
                let mut newchild = it.get_child_user_data_dfs(scip).ptrval();
                let child = it.get_child_expr_dfs(scip);

                // If child got simplified, replace it with the new child.
                if newchild != child {
                    let idx = it.get_child_idx_dfs(scip) as usize;
                    expr_replace_child(scip, expr, idx, newchild)?;
                }

                // We do not need to hold newchild anymore.
                expr_release(scip, &mut newchild)?;
            }
            EXPRITER_LEAVEEXPR => {
                let mut refexpr = NONE;

                // TODO we should do constant folding (handle that all children are
                // value-expressions) here in a generic way instead of reimplementing
                // it in every handler.

                // Use simplification of expression handlers.
                let h = scip.exprs[expr].exprhdlr;
                exprhdlr_simplify_expr(scip, h, expr, &mut refexpr, ownercreate, ownercreatedata)?;
                if expr != refexpr {
                    changed = true;
                }

                it.set_current_user_data(scip, ExprIterUserData::from_ptr(refexpr));
            }
            _ => unreachable!("we should never be called in this stage"),
        }
        expr = it.get_next(scip);
    }

    let simplified = it.get_expr_user_data(scip, rootexpr).ptrval();

    it.free(scip);

    Ok((simplified, changed, infeasible))
}

/// `quadDetectProcessExpr`: updates the seen hashmap for a quadratic
/// expression found during detection.
fn quad_detect_process_expr(expr: ExprId, seenexpr: &mut HashMap<ExprId, i32>, nquadterms: &mut i32, nlinterms: &mut i32) {
    if let Some(&nseen) = seenexpr.get(&expr) {
        if nseen < 0 {
            // Only seen linearly before.
            *nlinterms -= 1;
            *nquadterms += 1;
            seenexpr.insert(expr, 2);
        } else {
            seenexpr.insert(expr, nseen + 1);
        }
    } else {
        *nquadterms += 1;
        seenexpr.insert(expr, 1);
    }
}

/// `quadDetectGetQuadexprterm`: returns a quadexprterm that contains the
/// expr; if no quadexprterm has been initialized for the expr yet, then it
/// initializes one.
fn quad_detect_get_quadexprterm(expr: ExprId, expr2idx: &mut HashMap<ExprId, i32>, seenexpr: &HashMap<ExprId, i32>, quadexpr: &mut QuadExpr) -> usize {
    if let Some(&idx) = expr2idx.get(&expr) {
        idx as usize
    } else {
        let idx = quadexpr.quadexprterms.len();
        expr2idx.insert(expr, idx as i32);
        let adjbilinsize = seenexpr.get(&expr).copied().unwrap_or(0);
        quadexpr.quadexprterms.push(QuadExprQuadTerm {
            expr,
            sqrcoef: 0.0,
            sqrexpr: NONE,
            lincoef: 0.0,
            nadjbilin: 0,
            adjbilinsize,
            adjbilin: Vec::with_capacity(adjbilinsize.max(0) as usize),
        });
        idx
    }
}

/// `SCIPexprCheckQuadratic`: checks whether an expression is quadratic.
///
/// An expression is quadratic if it is either a power expression with
/// exponent 2.0, a product of two expressions, or a sum of terms where at
/// least one is a square or a product of two.
///
/// Use `SCIPexprGetQuadraticData()` to get data about the representation as
/// quadratic.
pub fn expr_check_quadratic(scip: &mut Scip, expr: ExprId) -> ScipResult<bool> {
    if scip.exprs[expr].quadchecked {
        return Ok(scip.exprs[expr].quaddata.is_some());
    }

    scip.exprs[expr].quadchecked = true;

    // Check if expression is a quadratic expression.

    // Handle single square term.
    if expr_is_power(scip, expr) && crate::expr_pow::get_exponent_expr_pow(scip, expr) == 2.0 {
        let child = scip.exprs[expr].children[0];
        let quad = QuadExpr {
            quadexprterms: vec![QuadExprQuadTerm {
                expr: child,
                sqrcoef: 1.0,
                sqrexpr: NONE,
                ..Default::default()
            }],
            allexprsarevars: expr_is_var(scip, child),
            ..Default::default()
        };
        scip.exprs[expr].quaddata = Some(Box::new(quad));
        return Ok(true);
    }

    // Handle single bilinear term.
    if expr_is_product(scip, expr) && scip.exprs[expr].children.len() == 2 {
        let (c0, c1) = (scip.exprs[expr].children[0], scip.exprs[expr].children[1]);
        let quad = QuadExpr {
            quadexprterms: vec![
                QuadExprQuadTerm {
                    expr: c0,
                    nadjbilin: 1,
                    adjbilinsize: 0,
                    adjbilin: vec![0],
                    sqrexpr: NONE,
                    ..Default::default()
                },
                QuadExprQuadTerm {
                    expr: c1,
                    nadjbilin: 1,
                    adjbilinsize: 0,
                    adjbilin: vec![0],
                    sqrexpr: NONE,
                    ..Default::default()
                },
            ],
            bilinexprterms: vec![QuadExprBilinTerm {
                expr1: c1,
                expr2: c0,
                coef: 1.0,
                pos2: 0,
                prodexpr: NONE,
            }],
            allexprsarevars: expr_is_var(scip, c0) && expr_is_var(scip, c1),
            ..Default::default()
        };
        scip.exprs[expr].quaddata = Some(Box::new(quad));
        return Ok(true);
    }

    // Neither a sum, nor a square, nor a bilinear term.
    if !expr_is_sum(scip, expr) {
        return Ok(false);
    }

    let children = scip.exprs[expr].children.clone();
    let coefs = crate::expr_sum::get_coefs_expr_sum(scip, expr).to_vec();
    let mut seenexpr: HashMap<ExprId, i32> = HashMap::with_capacity(2 * children.len());
    let mut nquadterms = 0;
    let mut nlinterms = 0;
    let mut nbilinterms = 0;
    for c in 0..children.len() {
        let child = children[c];
        let coef = coefs[c];
        if coef == 0.0 {
            continue;
        }

        if expr_is_power(scip, child) && crate::expr_pow::get_exponent_expr_pow(scip, child) == 2.0 {
            // Quadratic term.
            quad_detect_process_expr(scip.exprs[child].children[0], &mut seenexpr, &mut nquadterms, &mut nlinterms);
        } else if expr_is_product(scip, child) && scip.exprs[child].children.len() == 2 {
            // Bilinear term.
            nbilinterms += 1;
            quad_detect_process_expr(scip.exprs[child].children[0], &mut seenexpr, &mut nquadterms, &mut nlinterms);
            quad_detect_process_expr(scip.exprs[child].children[1], &mut seenexpr, &mut nquadterms, &mut nlinterms);
        } else {
            // First time seen linearly --> assign -1; ++nlinterms; not first time
            // --> assign +=1.
            if let Some(&v) = seenexpr.get(&child) {
                seenexpr.insert(child, v + 1);
            } else {
                nlinterms += 1;
                seenexpr.insert(child, -1);
            }
        }
    }

    if nquadterms == 0 {
        // Only linear sum.
        return Ok(false);
    }

    // expr2idx maps expressions to indices; if index > 0, it is its index in the
    // linexprs array, otherwise -index-1 is its index in the quadexprterms
    // array.
    let mut expr2idx: HashMap<ExprId, i32> = HashMap::with_capacity((nquadterms + nlinterms) as usize);

    // Allocate memory, etc.
    let mut quad = QuadExpr {
        quadexprterms: Vec::with_capacity(nquadterms as usize),
        linexprs: Vec::with_capacity(nlinterms as usize),
        lincoefs: Vec::with_capacity(nlinterms as usize),
        bilinexprterms: Vec::with_capacity(nbilinterms as usize),
        constant: crate::expr_sum::get_constant_expr_sum(scip, expr),
        allexprsarevars: true,
        ..Default::default()
    };

    // For every term of the sum-expr.
    for c in 0..children.len() {
        let mut child = children[c];
        let coef = coefs[c];
        if coef == 0.0 {
            continue;
        }

        if expr_is_power(scip, child) && crate::expr_pow::get_exponent_expr_pow(scip, child) == 2.0 {
            // Quadratic term.
            child = scip.exprs[child].children[0];

            let t = quad_detect_get_quadexprterm(child, &mut expr2idx, &seenexpr, &mut quad);
            quad.quadexprterms[t].sqrcoef = coef;
            quad.quadexprterms[t].sqrexpr = children[c];

            if quad.allexprsarevars {
                quad.allexprsarevars = expr_is_var(scip, quad.quadexprterms[t].expr);
            }
        } else if expr_is_product(scip, child) && scip.exprs[child].children.len() == 2 {
            // Bilinear term.
            let expr1 = scip.exprs[child].children[0];
            let expr2 = scip.exprs[child].children[1];

            let mut bilin = QuadExprBilinTerm {
                coef,
                prodexpr: child,
                ..Default::default()
            };

            let s1 = seenexpr.get(&expr1).copied().unwrap_or(0);
            let s2 = seenexpr.get(&expr2).copied().unwrap_or(0);
            if s1 >= s2 {
                bilin.expr1 = expr1;
                bilin.expr2 = expr2;
            } else {
                bilin.expr1 = expr2;
                bilin.expr2 = expr1;
            }

            let nb = quad.bilinexprterms.len() as i32;
            let t = quad_detect_get_quadexprterm(expr1, &mut expr2idx, &seenexpr, &mut quad);
            quad.quadexprterms[t].adjbilin.push(nb);
            quad.quadexprterms[t].nadjbilin += 1;
            if quad.allexprsarevars {
                quad.allexprsarevars = expr_is_var(scip, quad.quadexprterms[t].expr);
            }

            let t = quad_detect_get_quadexprterm(expr2, &mut expr2idx, &seenexpr, &mut quad);
            quad.quadexprterms[t].adjbilin.push(nb);
            quad.quadexprterms[t].nadjbilin += 1;
            if quad.allexprsarevars {
                quad.allexprsarevars = expr_is_var(scip, quad.quadexprterms[t].expr);
            }

            // Store position of second factor in quadexprterms.
            bilin.pos2 = expr2idx.get(&bilin.expr2).copied().unwrap_or(0);
            quad.bilinexprterms.push(bilin);
        } else {
            // Linear term.
            if seenexpr.get(&child).copied().unwrap_or(0) < 0 {
                // Expression only appears linearly.
                quad.linexprs.push(child);
                quad.lincoefs.push(coef);

                if quad.allexprsarevars {
                    quad.allexprsarevars = expr_is_var(scip, child);
                }
            } else {
                // Expression appears non-linearly: set lin coef.
                let t = quad_detect_get_quadexprterm(child, &mut expr2idx, &seenexpr, &mut quad);
                quad.quadexprterms[t].lincoef = coef;

                if quad.allexprsarevars {
                    quad.allexprsarevars = expr_is_var(scip, quad.quadexprterms[t].expr);
                }
            }
        }
    }

    scip.exprs[expr].quaddata = Some(Box::new(quad));

    Ok(true)
}

/// `SCIPexprFreeQuadratic`: frees information on quadratic representation
/// of an expression.
///
/// Reverts `SCIPexprCheckQuadratic()`. Before doing changes to an
/// expression, it can be useful to call this function.
pub fn expr_free_quadratic(scip: &mut Scip, expr: ExprId) {
    let e = &mut scip.exprs[expr];
    e.quadchecked = false;
    e.quaddata = None;
}

/// `SCIPexprComputeQuadraticCurvature`: checks the curvature of the
/// quadratic function stored for an expression.
///
/// The reference has no LAPACK (`SCIPlapackIsAvailable()` is false), so
/// beyond the cached result this leaves the curvature unknown, as the C
/// does. `assumevarfixed` is the set of variables to assume fixed.
pub fn expr_compute_quadratic_curvature(scip: &mut Scip, expr: ExprId, assumevarfixed: Option<&HashMap<VarId, ()>>, mut storeeigeninfo: bool) -> ScipResult<ExprCurv> {
    let quad = scip.exprs[expr].quaddata.as_ref().expect("quadratic data");

    // Do not store eigen information if we are not considering full matrix.
    if assumevarfixed.is_some() {
        storeeigeninfo = false;
    }

    if quad.eigeninfostored || (quad.curvaturechecked && !storeeigeninfo) {
        let curv = quad.curvature;
        // If we are convex or concave on the full set of variables, then we will
        // also be so on a subset.
        if assumevarfixed.is_none() || curv != ExprCurv::Unknown {
            return Ok(curv);
        }
    }

    let curv = ExprCurv::Unknown;
    let n = quad.quadexprterms.len();

    // Do not check curvature if nn will be too large.
    if n > 23000 {
        log::info!("number of quadratic variables is too large ({n}) to check the curvature");
        return Ok(curv);
    }

    // TODO do some simple tests first; like diagonal entries don't change sign,
    // etc.

    // SCIPlapackIsAvailable() is false in the reference.
    Ok(curv)
}

/// `SCIPexprGetQuadraticData`: the quadratic representation of an
/// expression (after `expr_check_quadratic` found it quadratic).
pub fn expr_get_quadratic_data(scip: &Scip, expr: ExprId) -> &QuadExpr {
    scip.exprs[expr].quaddata.as_ref().expect("quadratic data")
}

/// `SCIPexprAreQuadraticExprsVariables`: returns whether all expressions
/// that are used in a quadratic expression are variable expressions.
pub fn expr_are_quadratic_exprs_variables(scip: &Scip, expr: ExprId) -> bool {
    scip.exprs[expr].quaddata.as_ref().unwrap().allexprsarevars
}

/// `SCIPexprGetMonomialData`: returns a monomial representation of a
/// product expression: `(coef, exponents, factors)`.
///
/// The array to store all factor expressions needs to be of size the
/// number of children in the expression which is given by
/// `SCIPexprGetNChildren()`.
pub fn expr_get_monomial_data(scip: &Scip, expr: ExprId) -> (f64, Vec<f64>, Vec<ExprId>) {
    let coef = crate::expr_product::get_coef_expr_product(scip, expr);

    let nexprs = scip.exprs[expr].children.len();
    let mut exponents = Vec::with_capacity(nexprs);
    let mut factors = Vec::with_capacity(nexprs);
    for c in 0..nexprs {
        let child = scip.exprs[expr].children[c];
        if expr_is_power(scip, child) {
            exponents.push(crate::expr_pow::get_exponent_expr_pow(scip, child));
            factors.push(scip.exprs[child].children[0]);
        } else {
            exponents.push(1.0);
            factors.push(child);
        }
    }

    (coef, exponents, factors)
}

/* ---------------- expression iterator (expriter.c) ---------------- */

/// Minimal size of BFS queue.
const MINBFSSIZE: usize = 8;
/// Minimal size of DFS stack.
const MINDFSSIZE: usize = 16;

/// `SCIP_EXPRITER`: expression iterator.
///
/// It lives outside `Scip` (as C's is a separate allocation); every method
/// that touches expressions or the iterator count in `stat` takes the SCIP.
/// Call [`ExprIter::free`] when done.
#[derive(Default)]
pub struct ExprIter {
    /// Whether the iterator has been initialized, that is, is in use.
    pub initialized: bool,
    /// Type of expression iterator.
    pub itertype: ExprIterType,
    /// Current expression of the iterator.
    pub curr: ExprId,
    /// Index of iterator data in expressions, or -1 if not using iterator
    /// data in expressions.
    pub iterindex: i32,
    /// Tag to mark and recognize an expression as visited, or 0 if not
    /// avoiding multiple visits.
    pub visitedtag: i64,

    /* data for rtopological mode */
    /// DFS stack.
    dfsexprs: Vec<ExprId>,
    /// Number of visited children for each expression in the stack.
    dfsnvisited: Vec<i32>,

    /* data for BFS mode */
    /// BFS queue.
    queue: VecDeque<ExprId>,

    /* data for DFS mode */
    /// Current stage.
    pub dfsstage: ExprIterStage,
    /// Stages in which to interrupt iterator.
    pub stopstages: ExprIterStage,
}

impl ExprIter {
    /// `SCIPexpriterCreate`: creates an expression iterator.
    pub fn create() -> Self {
        Self {
            curr: NONE,
            iterindex: -1,
            ..Default::default()
        }
    }

    /// `deinit`: deinitializes an expression iterator.
    fn deinit(&mut self, scip: &mut Scip) {
        if !self.initialized {
            return;
        }

        if self.iterindex >= 0 {
            // The iterindex must be the one of the last initialized iterator.
            debug_assert_eq!(self.iterindex, scip.stat.nactiveexpriter - 1);

            // Tell core that this iterator is no longer active.
            scip.stat.nactiveexpriter -= 1;

            self.iterindex = -1;
        }

        match self.itertype {
            ExprIterType::Bfs => self.queue = VecDeque::new(),
            ExprIterType::Rtopologic => {
                self.dfsexprs = Vec::new();
                self.dfsnvisited = Vec::new();
            }
            ExprIterType::Dfs => {}
        }
        self.initialized = false;
    }

    /// `SCIPexpriterFree`: frees an expression iterator.
    pub fn free(mut self, scip: &mut Scip) {
        self.deinit(scip);
    }

    /// `SCIPexpriterIsInit`: returns whether expression iterator is current
    /// initialized.
    pub fn is_init(&self) -> bool {
        self.initialized
    }

    fn idx(&self) -> usize {
        self.iterindex as usize
    }

    /// `reverseTopologicalInsert`: adds an expression to the DFS stack.
    fn reverse_topological_insert(&mut self, expr: ExprId) {
        self.dfsexprs.push(expr);
        self.dfsnvisited.push(0);
    }

    /// `doReverseTopologicalNext`: moves to the next expression according to
    /// a reverse topological order.
    fn do_reverse_topological_next(&mut self, scip: &Scip) -> ExprId {
        // No expression left.
        if self.dfsexprs.is_empty() {
            return NONE;
        }

        // Get expression on the top of the stack.
        let top = self.dfsexprs.len() - 1;
        let expr = self.dfsexprs[top];
        let childidx = self.dfsnvisited[top];

        // Remove the expression if all children have been visited.
        if childidx as usize >= scip.exprs[expr].children.len() {
            self.dfsexprs.pop();
            self.dfsnvisited.pop();
            expr
        }
        // Go to the next child.
        else {
            let mut child = scip.exprs[expr].children[childidx as usize];

            // Mark that the child has been visited.
            self.dfsnvisited[top] += 1;

            // Do left-most step.
            while !scip.exprs[child].children.is_empty() {
                // Add child to the DFS stack.
                self.reverse_topological_insert(child);

                // Mark that the child has been visited; note that child is on top
                // of the DFS stack.
                let t = self.dfsnvisited.len() - 1;
                self.dfsnvisited[t] += 1;

                child = scip.exprs[child].children[0];
            }

            // Return last child; NOTE this child is not been added to the stack.
            child
        }
    }

    /// `doBfsNext`: moves to the next expression according to a BFS rule.
    fn do_bfs_next(&mut self, scip: &mut Scip) -> ExprId {
        // No expression left.
        let Some(expr) = self.queue.pop_front() else {
            return NONE;
        };

        // We should have set the visitedtag when adding the expression to the
        // queue.

        // Add all (possibly non-visited) children to the queue.
        for i in 0..scip.exprs[expr].children.len() {
            let child = scip.exprs[expr].children[i];

            if self.visitedtag != 0 {
                // Skip children that have already been visited or have already
                // been added to the queue.
                if scip.exprs[child].iterdata[self.idx()].visitedtag == self.visitedtag {
                    continue;
                }

                // Mark child as being in the queue (will be inserted next).
                scip.exprs[child].iterdata[self.idx()].visitedtag = self.visitedtag;
            }

            // Add child to the queue.
            self.queue.push_back(child);
        }

        expr
    }

    /// `doDfsNext`: moves to the next expression according to a DFS rule.
    fn do_dfs_next(&mut self, scip: &mut Scip) -> ExprId {
        if self.curr == NONE {
            return NONE;
        }
        let idx = self.idx();
        let curr = self.curr;

        match self.dfsstage {
            EXPRITER_VISITEDCHILD | EXPRITER_ENTEREXPR => {
                if self.dfsstage == EXPRITER_VISITEDCHILD {
                    // Consider next child.
                    scip.exprs[curr].iterdata[idx].currentchild += 1;
                }

                // If there is an unvisited child (left), then go into visitingchild
                // stage, otherwise go to leave stage; expect that we will leave
                // expr, and change mind to visitingchild below.
                self.dfsstage = EXPRITER_LEAVEEXPR;
                while (scip.exprs[curr].iterdata[idx].currentchild as usize) < scip.exprs[curr].children.len() {
                    let child = scip.exprs[curr].children[scip.exprs[curr].iterdata[idx].currentchild as usize];
                    if self.visitedtag == 0 || self.visitedtag != scip.exprs[child].iterdata[idx].visitedtag {
                        // If visitedtag is not used or child "currentchild" has not
                        // been visited yet, then go into visitingchild stage for this
                        // child.
                        self.dfsstage = EXPRITER_VISITINGCHILD;
                        break;
                    }
                    scip.exprs[curr].iterdata[idx].currentchild += 1;
                }

                curr
            }
            EXPRITER_VISITINGCHILD => {
                // Remember the parent and set the first child that should be visited
                // of the new root.
                let child = scip.exprs[curr].children[scip.exprs[curr].iterdata[idx].currentchild as usize];
                scip.exprs[child].iterdata[idx].parent = curr;
                scip.exprs[child].iterdata[idx].currentchild = 0;

                // Visit child.
                self.dfsstage = EXPRITER_ENTEREXPR;

                child
            }
            EXPRITER_LEAVEEXPR => {
                // Go back to parent expression.

                // Remember that this expression has been visited.
                scip.exprs[curr].iterdata[idx].visitedtag = self.visitedtag;

                // Be in visitedchild stage for the parent.
                self.dfsstage = EXPRITER_VISITEDCHILD;

                scip.exprs[curr].iterdata[idx].parent
            }
            _ => unreachable!("unknown stage"),
        }
    }

    /// `SCIPexpriterInit`: initializes an expression iterator.
    ///
    /// @note If no conshdlr has been given, then iterator will not be
    /// initialized (allowrevisit and type are ignored).
    ///
    /// If type is DFS, then stopstages will be set to ENTEREXPR. Use
    /// `SCIPexpriterSetStagesDFS` to change this.
    pub fn init(&mut self, scip: &mut Scip, expr: ExprId, itertype: ExprIterType, allowrevisit: bool) -> ScipResult {
        self.deinit(scip);

        // Store the new type of the iterator.
        self.itertype = itertype;

        // Get iterindex, if necessary.
        if !allowrevisit || itertype == ExprIterType::Dfs {
            if scip.stat.nactiveexpriter as usize + 1 >= EXPRITER_MAXNACTIVE {
                log::error!("Maximal number of active expression iterators reached at subscip-depth {}.", scip.stat.subscipdepth);
                return Err(Retcode::MaxDepthLevel);
            }

            self.iterindex = scip.stat.nactiveexpriter;
            scip.stat.nactiveexpriter += 1;
        } else {
            self.iterindex = -1;
        }

        // Get new tag to recognize visited expressions.
        if !allowrevisit {
            scip.stat.exprlastvisitedtag += 1;
            self.visitedtag = scip.stat.exprlastvisitedtag;
        } else {
            self.visitedtag = 0;
        }

        match self.itertype {
            ExprIterType::Bfs => {
                self.queue = VecDeque::with_capacity(MINBFSSIZE);

                if expr == NONE {
                    self.curr = NONE;
                } else {
                    self.queue.push_back(expr);

                    if self.visitedtag != 0 {
                        // Mark expression as being in the queue.
                        scip.exprs[expr].iterdata[self.idx()].visitedtag = self.visitedtag;
                    }

                    self.curr = self.get_next(scip);
                }
            }
            ExprIterType::Rtopologic => {
                self.dfsexprs = Vec::with_capacity(MINDFSSIZE);
                self.dfsnvisited = Vec::with_capacity(MINDFSSIZE);

                if expr != NONE {
                    self.reverse_topological_insert(expr);
                    self.curr = self.get_next(scip);
                } else {
                    self.curr = NONE;
                }
            }
            ExprIterType::Dfs => {
                self.stopstages = EXPRITER_ENTEREXPR;

                self.curr = expr;

                if expr != NONE {
                    let idx = self.idx();
                    scip.exprs[expr].iterdata[idx].currentchild = 0;
                    scip.exprs[expr].iterdata[idx].parent = NONE;
                    self.dfsstage = EXPRITER_ENTEREXPR;
                }
            }
        }

        self.initialized = true;

        Ok(())
    }

    /// `SCIPexpriterRestartDFS`: restarts an already initialized expression
    /// iterator in DFS mode.
    ///
    /// The expression iterator will continue from the given expression,
    /// preserving already set stop stages and the visitedtag.
    pub fn restart_dfs(&mut self, scip: &mut Scip, expr: ExprId) -> ExprId {
        let idx = self.idx();

        // If we forbid revisiting and root expr has already been visited, then
        // set curr to NULL, that is, be at end of iterator.
        if self.visitedtag > 0 && scip.exprs[expr].iterdata[idx].visitedtag == self.visitedtag {
            self.curr = NONE;
            return NONE;
        }

        // Set current to given expr, make it the root, and set stage to
        // enterexpr.
        self.curr = expr;
        scip.exprs[expr].iterdata[idx].currentchild = 0;
        scip.exprs[expr].iterdata[idx].parent = NONE;
        self.dfsstage = EXPRITER_ENTEREXPR;

        if (self.stopstages & EXPRITER_ENTEREXPR) == 0 {
            return self.get_next(scip);
        }

        self.curr
    }

    /// `SCIPexpriterSetStagesDFS`: specifies in which stages to stop a DFS
    /// iterator.
    ///
    /// If the current stage is not one of the stopstages, then the iterator
    /// will be moved on.
    pub fn set_stages_dfs(&mut self, scip: &mut Scip, stopstages: ExprIterStage) {
        if (self.dfsstage & stopstages) == 0 {
            self.stopstages = stopstages;
            let _ = self.get_next(scip);
        } else {
            self.stopstages = stopstages;
        }
    }

    /// `SCIPexpriterGetCurrent`: gets the current expression that the
    /// expression iterator points to.
    #[inline]
    pub fn get_current(&self) -> ExprId {
        self.curr
    }

    /// `SCIPexpriterGetStageDFS`: gets the current stage that the
    /// expression iterator is in when using DFS.
    #[inline]
    pub fn get_stage_dfs(&self) -> ExprIterStage {
        self.dfsstage
    }

    /// `SCIPexpriterGetChildIdxDFS`: gets the index of the child that the
    /// expression iterator considers when in DFS mode and stage
    /// VISITINGCHILD or VISITEDCHILD.
    #[inline]
    pub fn get_child_idx_dfs(&self, scip: &Scip) -> i32 {
        scip.exprs[self.curr].iterdata[self.idx()].currentchild
    }

    /// `SCIPexpriterGetChildExprDFS`: gets the child expression that the
    /// expression iterator considers when in DFS mode and stage
    /// VISITINGCHILD or VISITEDCHILD.
    #[inline]
    pub fn get_child_expr_dfs(&self, scip: &Scip) -> ExprId {
        let e = &scip.exprs[self.curr];
        e.children[e.iterdata[self.idx()].currentchild as usize]
    }

    /// `SCIPexpriterGetParentDFS`: gives the parent of the current
    /// expression of an expression iteration if in DFS mode.
    #[inline]
    pub fn get_parent_dfs(&self, scip: &Scip) -> ExprId {
        scip.exprs[self.curr].iterdata[self.idx()].parent
    }

    /// `SCIPexpriterGetCurrentUserData`: gives the iterator specific user
    /// data of the current expression.
    #[inline]
    pub fn get_current_user_data(&self, scip: &Scip) -> ExprIterUserData {
        scip.exprs[self.curr].iterdata[self.idx()].userdata
    }

    /// `SCIPexpriterGetChildUserDataDFS`: gives the iterator specific user
    /// data of the current expressions current child.
    #[inline]
    pub fn get_child_user_data_dfs(&self, scip: &Scip) -> ExprIterUserData {
        let child = self.get_child_expr_dfs(scip);
        scip.exprs[child].iterdata[self.idx()].userdata
    }

    /// `SCIPexpriterGetExprUserData`: gives the iterator specific user data
    /// of a given expression.
    #[inline]
    pub fn get_expr_user_data(&self, scip: &Scip, expr: ExprId) -> ExprIterUserData {
        scip.exprs[expr].iterdata[self.idx()].userdata
    }

    /// `SCIPexpriterSetCurrentUserData`: sets the iterator specific user data
    /// of the current expression for an expression iteration if in DFS mode.
    #[inline]
    pub fn set_current_user_data(&self, scip: &mut Scip, userdata: ExprIterUserData) {
        scip.exprs[self.curr].iterdata[self.idx()].userdata = userdata;
    }

    /// `SCIPexpriterSetExprUserData`: sets the iterator specific user data of
    /// a given expression.
    #[inline]
    pub fn set_expr_user_data(&self, scip: &mut Scip, expr: ExprId, userdata: ExprIterUserData) {
        scip.exprs[expr].iterdata[self.idx()].userdata = userdata;
    }

    /// `SCIPexpriterSetChildUserData`: sets the iterator specific user data
    /// of the current expressions current child.
    #[inline]
    pub fn set_child_user_data(&self, scip: &mut Scip, userdata: ExprIterUserData) {
        let child = self.get_child_expr_dfs(scip);
        scip.exprs[child].iterdata[self.idx()].userdata = userdata;
    }

    /// `SCIPexpriterGetNext`: moves the iterator to the next expression.
    pub fn get_next(&mut self, scip: &mut Scip) -> ExprId {
        // Move to the next expression according to iterator type.
        match self.itertype {
            ExprIterType::Bfs => {
                self.curr = self.do_bfs_next(scip);
            }
            ExprIterType::Rtopologic => {
                self.curr = self.do_reverse_topological_next(scip);
                if self.visitedtag != 0 {
                    let idx = self.idx();

                    // Skip already visited expressions.
                    while self.curr != NONE {
                        if scip.exprs[self.curr].iterdata[idx].visitedtag == self.visitedtag {
                            // If curr has already been visited, get next one.
                            self.curr = self.do_reverse_topological_next(scip);
                        } else {
                            // curr has not been visited yet, so mark it as visited and
                            // interrupt loop.
                            scip.exprs[self.curr].iterdata[idx].visitedtag = self.visitedtag;
                            break;
                        }
                    }
                }
            }
            ExprIterType::Dfs => {
                // Get next until we are in a stopstage again; this might give
                // expr=NULL, if dfs has finished.
                loop {
                    self.curr = self.do_dfs_next(scip);
                    if self.curr == NONE || (self.dfsstage & self.stopstages) != 0 {
                        break;
                    }
                }
            }
        }

        self.curr
    }

    /// `SCIPexpriterSkipDFS`: moves a DFS iterator to one of the next
    /// expressions.
    ///
    /// If in ENTEREXPR stage, then all children of that expression will be
    /// skipped. If LEAVEEXPR is one of the stopstages, then it will be the
    /// next stage. If in VISITINGCHILD stage, then the child that was going
    /// to be entered next will be skipped and the iterator will be moved on
    /// to the next child (if any). If in VISITEDCHILD stage, then all
    /// remaining children will be skipped and we move on to the LEAVEEXPR
    /// stage (if a stop stage). If in LEAVEEXPR stage, then next will be
    /// called.
    pub fn skip_dfs(&mut self, scip: &mut Scip) -> ExprId {
        match self.dfsstage {
            EXPRITER_ENTEREXPR | EXPRITER_VISITEDCHILD => {
                // Move directly to leaveexpr.
                self.dfsstage = EXPRITER_LEAVEEXPR;
                // If leaveexpr is not a stopstage, then move on.
                while self.curr != NONE && (self.dfsstage & self.stopstages) == 0 {
                    self.curr = self.do_dfs_next(scip);
                }
                self.curr
            }
            EXPRITER_VISITINGCHILD => {
                // Skip the child to be visited; pretend we just visited this child
                // and get next.
                self.dfsstage = EXPRITER_VISITEDCHILD;
                self.get_next(scip)
            }
            _ => {
                log::error!("SCIPexpriterSkipDFS called in invalid stage {}", self.dfsstage);
                panic!("SCIPexpriterSkipDFS called in invalid stage");
            }
        }
    }

    /// `SCIPexpriterIsEnd`: returns whether the iterator visited all
    /// expressions already.
    #[inline]
    pub fn is_end(&self) -> bool {
        self.curr == NONE
    }
}

/// `SCIPgetExprNewSoltag`: returns a previously unused solution tag for
/// expression evaluation.
pub fn get_expr_new_soltag(scip: &mut Scip) -> i64 {
    scip.stat.exprlastsoltag += 1;
    scip.stat.exprlastsoltag
}
