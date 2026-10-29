//! Port of `cons_nonlinear.c`: constraint handler for nonlinear constraints
//! specified by algebraic expressions.

use std::{any::Any, collections::HashMap};

use crate::{
    def::{INVALID, eps_is_int},
    event::{self, Event},
    expr::{self, ExprCurv, ExprId, ExprIntEvalVar, INTERVAL_INFINITY, Monotone},
    interval::{self, Interval},
    misc::RandNumGen,
    nlhdlr::{self, NlhdlrId, NlhdlrMethod},
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    types::{ConsId, ConshdlrId, EventhdlrId, HeurId, LockType, SolId, VarId},
    var::NONE,
};

/* fundamental constraint handler properties */
const CONSHDLR_NAME: &str = "nonlinear";
const CONSHDLR_DESC: &str = "handler for nonlinear constraints specified by algebraic expressions";
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = 50;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -4000010;
/// Frequency for using all instead of only the useful constraints in
/// separation, propagation and enforcement, -1 for no eager evaluations, 0
/// for first only.
const CONSHDLR_EAGERFREQ: i32 = 100;
/// Should the constraint handler be skipped, if no constraints are
/// available?
const CONSHDLR_NEEDSCONS: bool = true;

/* optional constraint handler properties */
/// Priority of the constraint handler for separation.
const CONSHDLR_SEPAPRIORITY: i32 = 10;
/// Frequency for separating cuts; zero means to separate only in the root
/// node.
const CONSHDLR_SEPAFREQ: i32 = 1;
/// Should separation method be delayed, if other separators found cuts?
const CONSHDLR_DELAYSEPA: bool = false;

/// Frequency for propagating domains; zero means only preprocessing
/// propagation.
const CONSHDLR_PROPFREQ: i32 = 1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const CONSHDLR_DELAYPROP: bool = false;
/// Propagation timing mask of the constraint handler.
const CONSHDLR_PROP_TIMING: crate::cons::PropTiming = crate::cons::PROPTIMING_BEFORELP;

/// Presolving timing of the constraint handler (fast, medium, or
/// exhaustive).
const CONSHDLR_PRESOLTIMING: crate::cons::PresolTiming = crate::cons::PRESOLTIMING_ALWAYS;
/// Maximal number of presolving rounds the constraint handler participates
/// in (-1: no limit).
const CONSHDLR_MAXPREROUNDS: i32 = -1;

/// Maximum perturbation.
const VERTEXPOLY_MAXPERTURBATION: f64 = 1e-3;
/// Use dual or primal simplex algorithm?
const VERTEXPOLY_USEDUALSIMPLEX: bool = true;
/// Seed for random number generator, which is used to move points away from
/// the boundary.
const VERTEXPOLY_RANDNUMINITSEED: u32 = 20181029;
/// Adjust resulting facets in checkRikun() up to a violation of this value
/// times lpfeastol.
const VERTEXPOLY_ADJUSTFACETFACTOR: f64 = 1e1;

/// Seed for random number generator, which is used to select from several
/// similar good branching candidates.
const BRANCH_RANDNUMINITSEED: u32 = 20191229;

/// Maximal number of auxiliary expressions per bilinear term.
const BILIN_MAXNAUXEXPRS: i32 = 10;

/// `SCIP_MAXVERTEXPOLYDIM`: maximal dimension of vertex-polyhedral function
/// to compute facet for.
pub const MAXVERTEXPOLYDIM: usize = 14;

/// `infty2infty`: translate from one value of infinity to another; if val
/// is >= infty1, then give infty2, else give val.
#[inline]
fn infty2infty(infty1: f64, infty2: f64, val: f64) -> f64 {
    if val >= infty1 { infty2 } else { val }
}

/// `POWEROFTWO`: translates x to 2^x for non-negative integer x.
#[inline]
fn poweroftwo(x: u32) -> u32 {
    0x1u32 << x
}

/// C's `MAX(a, b)`.
#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// C's `MIN(a, b)`.
#[inline]
fn cmin(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}

/* data structures */

/// `EXPRENFO`: enforcement data of an expression.
#[derive(Clone, Copy, Debug)]
pub struct ExprEnfo {
    /// Nonlinear handler.
    pub nlhdlr: NlhdlrId,
    /// Data of nonlinear handler (an id in `scip.nlhdlrexprdatas`, `NONE`
    /// for NULL).
    pub nlhdlrexprdata: usize,
    /// Methods where nonlinear handler participates.
    pub nlhdlrparticipation: NlhdlrMethod,
    /// Was the initsepa callback of nlhdlr called.
    pub issepainit: bool,
    /// Auxiliary value of expression w.r.t. currently enforced solution.
    pub auxvalue: f64,
    /// Whether sepabelow uses activity of some expression.
    pub sepabelowusesactivity: bool,
    /// Whether sepaabove uses activity of some expression.
    pub sepaaboveusesactivity: bool,
}

/// `SCIP_EXPR_OWNERDATA`: data stored by constraint handler in an
/// expression that belongs to a nonlinear constraint.
#[derive(Clone, Debug)]
pub struct OwnerData {
    /// Nonlinear constraint handler.
    pub conshdlr: ConshdlrId,

    /* locks and monotonicity */
    /// Positive locks counter.
    pub nlockspos: i32,
    /// Negative locks counter.
    pub nlocksneg: i32,
    /// Array containing monotonicity of expression w.r.t. each child (`None`
    /// for NULL).
    pub monotonicity: Option<Vec<Monotone>>,

    /* propagation (in addition to activity that is stored in expr) */
    /// Bounds to propagate in reverse propagation.
    pub propbounds: Interval,
    /// Tag to indicate whether propbounds are valid for the current
    /// propagation rounds.
    pub propboundstag: u32,
    /// Whether expression is queued for propagation.
    pub inpropqueue: bool,

    /* enforcement of expr == auxvar (or expr <= auxvar, or expr >= auxvar) */
    /// Enforcements.
    pub enfos: Vec<ExprEnfo>,
    /// Number of enforcements, or -1 if not initialized.
    pub nenfos: i32,
    /// Last enforcement round where expression was enforced successfully.
    pub lastenforced: u32,
    /// Number of nonlinear handlers whose activity computation (or domain
    /// propagation) depends on the activity of the expression.
    pub nactivityusesprop: u32,
    /// Number of nonlinear handlers whose separation (estimate or enfo)
    /// depends on the activity of the expression.
    pub nactivityusessepa: u32,
    /// Number of nonlinear handlers whose separation uses an auxvar in the
    /// expression.
    pub nauxvaruses: u32,
    /// Auxiliary variable used for outer approximation cuts.
    pub auxvar: VarId,

    /* branching */
    /// Sum of violation scores for branching stored for this expression.
    pub violscoresum: f64,
    /// Max of violation scores for branching stored for this expression.
    pub violscoremax: f64,
    /// Number of violation scores stored for this expression.
    pub nviolscores: i32,
    /// Tag to decide whether a violation score of an expression needs to be
    /// initialized.
    pub violscoretag: u32,

    /* additional data for variable expressions */
    /// Constraints in which this variable appears.
    pub conss: Vec<ConsId>,
    /// Is the array of constraints sorted.
    pub consssorted: bool,

    /// Position of eventdata in SCIP's event filter, -1 if not catching
    /// events.
    pub filterpos: i32,
}

/// `SCIP_CONSDATA`: constraint data for nonlinear constraints.
#[derive(Clone, Debug)]
pub struct ConsData {
    /* data that defines the constraint: expression and sides */
    /// Expression that represents this constraint.
    pub expr: ExprId,
    /// Left-hand side.
    pub lhs: f64,
    /// Right-hand side.
    pub rhs: f64,

    /* variables */
    /// Array containing all variable expressions (`None` if not stored).
    pub varexprs: Option<Vec<ExprId>>,
    /// Do we catch events on variables?
    pub catchedevents: bool,

    /* constraint violation */
    /// Violation of left-hand side by current solution.
    pub lhsviol: f64,
    /// Violation of right-hand side by current solution.
    pub rhsviol: f64,
    /// Norm of gradient of constraint function in current solution (if
    /// evaluated).
    pub gradnorm: f64,
    /// Tag of solution used that gradnorm corresponds to.
    pub gradnormsoltag: i64,

    /* status flags */
    /// Did we propagate the current bounds already?
    pub ispropagated: bool,
    /// Did we simplify the expression tree already?
    pub issimplified: bool,

    /* locks */
    /// Number of positive locks.
    pub nlockspos: i32,
    /// Number of negative locks.
    pub nlocksneg: i32,

    /* repair infeasible solutions */
    /// Variable that may be decreased without making any other constraint
    /// infeasible, or NULL if none.
    pub linvardecr: VarId,
    /// Variable that may be increased without making any other constraint
    /// infeasible, or NULL if none.
    pub linvarincr: VarId,
    /// Linear coefficient of linvardecr.
    pub linvardecrcoef: f64,
    /// Linear coefficient of linvarincr.
    pub linvarincrcoef: f64,

    /* miscellaneous */
    /// Curvature of the root expression w.r.t. the original variables.
    pub curv: ExprCurv,
    /// A nonlinear row representation of this constraint.
    pub nlrow: usize,
    /// An index of the constraint that is unique among all expr-constraints in
    /// this SCIP instance and is constant.
    pub consindex: i32,
}

impl ConsData {
    /// The number of stored variable expressions (`nvarexprs`).
    #[inline]
    pub fn nvarexprs(&self) -> usize {
        self.varexprs.as_ref().map_or(0, Vec::len)
    }
}

/// `SCIP_DECL_NONLINCONSUPGD`: (cons, nvarexprs, nupgdconss, upgdconss,
/// upgdconsssize); upgrades a nonlinear constraint.
pub type NonlinConsUpgd = fn(&mut Scip, ConsId, i32, &mut i32, &mut Vec<ConsId>, i32) -> ScipResult;

/// `CONSUPGRADE`: constraint upgrade method.
#[derive(Clone, Debug)]
pub struct ConsUpgrade {
    /// Method to call for upgrading nonlinear constraint.
    pub consupgd: NonlinConsUpgd,
    /// Priority of upgrading method.
    pub priority: i32,
    /// Is upgrading enabled.
    pub active: bool,
    /// The name of the upgrading constraint handler (finds the upgrade for
    /// its parameter).
    pub conshdlrname: String,
}

/// `SCIP_CONSNONLINEAR_AUXEXPR`: auxiliary expression of a bilinear term.
#[derive(Clone, Copy, Debug)]
pub struct AuxExpr {
    /// Coefficients in the auxiliary expression.
    pub coefs: [f64; 3],
    /// Constant.
    pub cst: f64,
    /// Auxiliary variable w in xy {<=,>=,=} auxexpr(w, x, y).
    pub auxvar: VarId,
    /// Whether the auxexpr underestimates the product.
    pub underestimate: bool,
    /// Whether the auxexpr overestimates the product.
    pub overestimate: bool,
}

/// `SCIP_CONSNONLINEAR_BILINTERM`: bilinear term structure.
///
/// The union `aux` is `auxexprs` (case `nauxexprs` > 0) or `auxvar` (case
/// `nauxexprs` = 0).
#[derive(Clone, Debug)]
pub struct BilinTerm {
    /// First variable.
    pub x: VarId,
    /// Second variable.
    pub y: VarId,
    /// Auxiliary expressions for the implicit product of x and y.
    pub auxexprs: Vec<AuxExpr>,
    /// Auxiliary variable for the explicit product of x and y.
    pub auxvar: VarId,
    /// Number of aux.exprs (0 for products without implicit relations).
    pub nauxexprs: i32,
    /// Number of positive expression locks.
    pub nlockspos: i32,
    /// Number of negative expression locks.
    pub nlocksneg: i32,
    /// Does the product exist explicitly in the problem?
    pub existing: bool,
}

/// `SCIP_CONSHDLRDATA`: constraint handler data.
pub struct ConshdlrData {
    /* nonlinear handler */
    /// Nonlinear handlers.
    pub nlhdlrs: Vec<NlhdlrId>,
    /// Whether we are currently in detectNlhdlr.
    pub indetect: bool,
    /// A flag that is used only during detectNlhdlr().
    pub registerusesactivitysepabelow: bool,
    /// A flag that is used only during detectNlhdlr().
    pub registerusesactivitysepaabove: bool,

    /* constraint upgrades */
    /// Constraint upgrade methods for specializing nonlinear constraints.
    pub consupgrades: Vec<ConsUpgrade>,

    /* other plugins */
    /// Handler for variable bound change events.
    pub eventhdlr: EventhdlrId,
    /// A pointer to the subnlp heuristic, if available.
    pub subnlpheur: HeurId,
    /// A pointer to the trysol heuristic, if available.
    pub trysolheur: HeurId,

    /* tags and counters */
    /// Unique id for the next auxiliary variable.
    pub auxvarid: i32,
    /// Tag indicating current variable bounds.
    pub curboundstag: i64,
    /// Tag when bounds where most recently relaxed.
    pub lastboundrelax: i64,
    /// Tag when method used to evaluate activity of variables changed last.
    pub lastvaractivitymethodchange: i64,
    /// Total number of enforcement calls, including current one.
    pub enforound: u32,
    /// Last used consindex, plus one.
    pub lastconsindex: i32,

    /* activity intervals and domain propagation */
    /// Method currently used for activity calculation of variable
    /// expressions.
    pub intevalvar: IntEvalVar,
    /// Whether global variable bounds should be used for activity
    /// calculation.
    pub globalbounds: bool,
    /// Expression queue to be used in reverse propagation, filled by
    /// SCIPtightenExprIntervalNonlinear.
    pub reversepropqueue: std::collections::VecDeque<ExprId>,
    /// Whether bound change passed to SCIPtightenExprIntervalNonlinear should
    /// be forced.
    pub forceboundtightening: bool,
    /// Tag indicating current propagation rounds, to match with
    /// expr->propboundstag.
    pub curpropboundstag: u32,

    /* parameters */
    pub maxproprounds: i32,
    pub propauxvars: bool,
    pub varboundrelax: u8,
    pub varboundrelaxamount: f64,
    pub conssiderelaxamount: f64,
    pub vp_maxperturb: f64,
    pub vp_adjfacetthreshold: f64,
    pub vp_dualsimplex: bool,
    pub reformbinprods: bool,
    pub reformbinprodsand: bool,
    pub reformbinprodsfac: i32,
    pub forbidmultaggrnlvar: bool,
    pub tightenlpfeastol: bool,
    pub propinenforce: bool,
    pub weakcutthreshold: f64,
    pub strongcutmaxcoef: f64,
    pub strongcutefficacy: bool,
    pub forcestrongcut: bool,
    pub enfoauxviolfactor: f64,
    pub weakcutminviolfactor: f64,
    pub rownotremovable: u8,
    pub violscale: u8,
    pub checkvarlocks: u8,
    pub branchauxmindepth: i32,
    pub branchexternal: bool,
    pub branchhighviolfactor: f64,
    pub branchhighscorefactor: f64,
    pub branchviolweight: f64,
    pub branchfracweight: f64,
    pub branchdualweight: f64,
    pub branchpscostweight: f64,
    pub branchdomainweight: f64,
    pub branchvartypeweight: f64,
    pub branchscoreagg: u8,
    pub branchviolsplit: u8,
    pub branchpscostreliable: f64,
    pub branchmixfractional: f64,
    pub linearizeheursol: u8,
    pub assumeconvex: bool,

    /* statistics */
    pub nweaksepa: i64,
    pub ntightenlp: i64,
    pub ndesperatetightenlp: i64,
    pub ndesperatebranch: i64,
    pub ndesperatecutoff: i64,
    pub nforcelp: i64,
    pub canonicalizetime: crate::clock::Clock,
    pub ncanonicalizecalls: i64,

    /* facets of envelops of vertex-polyhedral functions */
    /// Random number generator used to perturb reference point.
    pub vp_randnumgen: Option<RandNumGen>,
    /// LPs used to compute facets for functions of different dimension.
    pub vp_lp: Vec<Option<Box<crate::lpi::Lpi>>>,

    /* hashing of bilinear terms */
    /// Hash table for bilinear terms (`None` if not created): the index of
    /// the term of (x, y) with x.index <= y.index (only ever looked up).
    pub bilinhashtable: Option<HashMap<(VarId, VarId), usize>>,
    /// Bilinear terms (`None` while the array is not allocated).
    pub bilinterms: Option<Vec<BilinTerm>>,
    /// Maximal number of auxiliary expressions per bilinear term.
    pub bilinmaxnauxexprs: i32,

    /* branching */
    /// Random number generated used in branching variable selection.
    pub branchrandnumgen: Option<RandNumGen>,
    /// Value of parameter branching/lpgainnormalize.
    pub branchpscostupdatestrategy: u8,

    /* misc */
    /// Whether variables contained in a single constraint have been already
    /// considered.
    pub checkedvarlocks: bool,
    /// Hashmap to map SCIP variables to variable-expressions.
    pub var2expr: HashMap<VarId, ExprId>,
    /// Filter position of new solution event handler, if caught.
    pub newsoleventfilterpos: i32,
}

/// The `SCIP_DECL_EXPR_INTEVALVAR` methods the constraint handler uses for
/// activity calculation of variable expressions (comparable, unlike fn
/// pointers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntEvalVar {
    /// `intEvalVarBoundTightening`.
    BoundTightening,
    /// `intEvalVarRedundancyCheck`.
    RedundancyCheck,
}

impl IntEvalVar {
    /// The method.
    pub fn func(self) -> ExprIntEvalVar {
        match self {
            IntEvalVar::BoundTightening => int_eval_var_bound_tightening,
            IntEvalVar::RedundancyCheck => int_eval_var_redundancy_check,
        }
    }
}

/// `BRANCHCAND`: branching candidate with various scores.
#[derive(Clone, Copy, Debug, Default)]
struct BranchCand {
    /// Expression that holds branching candidate, NULL if candidate is due to
    /// fractionality of integer variable.
    expr: ExprId,
    /// Variable that is branching candidate.
    var: VarId,
    /// Aux-violation score of candidate.
    auxviol: f64,
    /// Domain score of candidate.
    domain: f64,
    /// Dual score of candidate.
    dual: f64,
    /// Pseudo-cost score of candidate.
    pscost: f64,
    /// Variable type score of candidate.
    vartype: f64,
    /// Fractionality score of candidate.
    fractionality: f64,
    /// Weighted sum of other scores, see scoreBranchingCandidates().
    weighted: f64,
}

/* data access */

/// The constraint data of a nonlinear constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of a nonlinear constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

/// The constraint handler data of the nonlinear constraint handler.
#[inline]
pub fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/// The constraint handler data of the nonlinear constraint handler,
/// mutably.
#[inline]
pub fn hdm(scip: &mut Scip, h: ConshdlrId) -> &mut ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_mut().unwrap().downcast_mut::<ConshdlrData>().unwrap()
}

/// `SCIPexprGetOwnerData` of an expression owned by the nonlinear
/// constraint handler.
#[inline]
pub fn od(scip: &Scip, expr: ExprId) -> &OwnerData {
    scip.exprs[expr].ownerdata.as_ref().unwrap().downcast_ref::<OwnerData>().unwrap()
}

/// `SCIPexprGetOwnerData`, mutably.
#[inline]
pub fn odm(scip: &mut Scip, expr: ExprId) -> &mut OwnerData {
    scip.exprs[expr].ownerdata.as_mut().unwrap().downcast_mut::<OwnerData>().unwrap()
}

/// The constraint handler of a constraint.
#[inline]
fn cons_hdlr(scip: &Scip, cons: ConsId) -> ConshdlrId {
    scip.conss[cons].conshdlr
}

/* local methods */

/// `freeAuxVar`: frees auxiliary variables of expression, if any.
fn free_aux_var(scip: &mut Scip, expr: ExprId) -> ScipResult {
    let auxvar = od(scip, expr).auxvar;
    if auxvar == NONE {
        return Ok(());
    }

    // Remove variable locks; as this is a relaxation-only variable, no other
    // plugin should use it for deducing any type of reductions or cutting
    // planes.
    crate::api::add_var_locks_type(scip, auxvar, LockType::Model, -1, -1)?;

    // Release auxiliary variable.
    let mut v = auxvar;
    crate::api::release_var(scip, &mut v)?;
    odm(scip, expr).auxvar = NONE;

    Ok(())
}

/// `freeEnfoData`: frees data used for enforcement of expression, that is,
/// nonlinear handlers.
///
/// Can also clear indicators whether expr needs enforcement methods, that
/// is, free an associated auxiliary variable and reset the nactivityuses
/// counts.
fn free_enfo_data(scip: &mut Scip, expr: ExprId, freeauxvar: bool) -> ScipResult {
    if freeauxvar {
        // Free auxiliary variable.
        free_aux_var(scip, expr)?;

        // Reset count on activity and auxvar usage.
        let o = odm(scip, expr);
        o.nactivityusesprop = 0;
        o.nactivityusessepa = 0;
        o.nauxvaruses = 0;
    }

    // Free data stored by nonlinear handlers.
    let nenfos = od(scip, expr).nenfos;
    for e in 0..nenfos.max(0) as usize {
        let enfo = od(scip, expr).enfos[e];
        let nlhdlr = enfo.nlhdlr;

        if enfo.issepainit {
            // Call the separation deinitialization callback of the nonlinear
            // handler.
            nlhdlr::nlhdlr_exitsepa(scip, nlhdlr, expr, enfo.nlhdlrexprdata)?;
            odm(scip, expr).enfos[e].issepainit = false;
        }

        // Free nlhdlr exprdata, if there is any and there is a method to free
        // this data.
        if enfo.nlhdlrexprdata != NONE {
            let mut d = enfo.nlhdlrexprdata;
            nlhdlr::nlhdlr_freeexprdata(scip, nlhdlr, expr, &mut d)?;
            odm(scip, expr).enfos[e].nlhdlrexprdata = d;
        }
    }

    // Free array with enfo data.
    let o = odm(scip, expr);
    o.enfos = Vec::new();

    // We need to look at this expression in detect again.
    o.nenfos = -1;

    Ok(())
}

/// `exprownerFree`: callback that frees data that this conshdlr stored in
/// an expression.
fn exprowner_free(scip: &mut Scip, expr: ExprId) -> ScipResult {
    // Expression should not be locked anymore.

    free_enfo_data(scip, expr, true)?;

    // Expression should not be enforced anymore.

    if expr::expr_is_var(scip, expr) {
        // There should be no constraints left that still use this variable; thus,
        // there should also be no variable event catched (via this exprhdlr).

        // Update var2expr hashmap in conshdlrdata.
        let conshdlr = od(scip, expr).conshdlr;
        let var = crate::expr_var::get_var_expr_var(scip, expr);

        // Remove var -> expr map from hashmap if present (if no
        // variable-expression stored for var hashmap, then the var hasn't been
        // used in any constraint, so do nothing; if variable-expression stored
        // for var is different, then also do nothing).
        let h = hdm(scip, conshdlr);
        if h.var2expr.get(&var) == Some(&expr) {
            h.var2expr.remove(&var);
        }
    }

    scip.exprs[expr].ownerdata = None;

    Ok(())
}

/// `exprownerEvalactivity`: possibly reevaluates and then returns the
/// activity of the expression.
///
/// Reevaluate activity if currently stored is not up to date (some bound was
/// changed since last evaluation).
fn exprowner_evalactivity(scip: &mut Scip, expr: ExprId) -> ScipResult {
    let conshdlr = od(scip, expr).conshdlr;

    if scip.exprs[expr].activitytag < hd(scip, conshdlr).curboundstag {
        // Update activity of expression.
        forward_prop_expr(scip, conshdlr, expr, false, None, None)?;
    }

    Ok(())
}

/// `exprownerCreate`: callback that creates data that this conshdlr wants
/// to store in an expression; `ownercreatedata` is the constraint handler.
pub fn exprowner_create(scip: &mut Scip, expr: ExprId, ownercreatedata: usize) -> ScipResult {
    let conshdlr = ownercreatedata;
    let mut ownerdata = OwnerData {
        conshdlr,
        nlockspos: 0,
        nlocksneg: 0,
        monotonicity: None,
        propbounds: Interval::default(),
        propboundstag: 0,
        inpropqueue: false,
        enfos: Vec::new(),
        nenfos: -1,
        lastenforced: 0,
        nactivityusesprop: 0,
        nactivityusessepa: 0,
        nauxvaruses: 0,
        auxvar: NONE,
        violscoresum: 0.0,
        violscoremax: 0.0,
        nviolscores: 0,
        violscoretag: 0,
        conss: Vec::new(),
        consssorted: false,
        filterpos: 0,
    };

    if expr::expr_is_var(scip, expr) {
        ownerdata.filterpos = -1;

        // Add to var2expr hashmap if not having expr for var yet.
        let var = crate::expr_var::get_var_expr_var(scip, expr);
        let h = hdm(scip, conshdlr);
        // (If expr was just created, then it shouldn't already be stored as
        // image of var.)
        h.var2expr.entry(var).or_insert(expr);
    } else {
        // Just so that we can use filterpos to recognize whether an expr is a
        // varexpr if not having a SCIP pointer around.
        ownerdata.filterpos = -2;
    }

    let e = &mut scip.exprs[expr];
    e.ownerdata = Some(Box::new(ownerdata));
    e.ownerfree = Some(exprowner_free);
    e.ownerevalactivity = Some(exprowner_evalactivity);

    Ok(())
}

/// `createExprVar`: creates a variable expression or retrieves from hashmap
/// in conshdlr data.
fn create_expr_var(scip: &mut Scip, conshdlr: ConshdlrId, var: VarId) -> ScipResult<ExprId> {
    // Get variable expression representing the given variable if there is one
    // already.
    match hd(scip, conshdlr).var2expr.get(&var).copied() {
        None => {
            // Create a new variable expression; this also captures the expression.
            // (exprownerCreate should have added var->expr to var2expr.)
            crate::expr_var::create_expr_var(scip, var, Some(exprowner_create), conshdlr)
        }
        Some(expr) => {
            // Only capture already existing expr to get a consistent uses-count.
            expr::expr_capture(scip, expr);
            Ok(expr)
        }
    }
}

/// `mapexprvar`: map var exprs to var-expr from var2expr hashmap;
/// `mapexprdata` is the constraint handler.
fn mapexprvar(scip: &mut Scip, sourceexpr: ExprId, _ownercreate: Option<expr::ExprOwnerCreate>, _ownercreatedata: usize, mapexprdata: usize) -> ScipResult<ExprId> {
    let conshdlr = mapexprdata;

    // Do not provide map if not variable.
    if !expr::expr_is_var(scip, sourceexpr) {
        return Ok(NONE);
    }

    let var = crate::expr_var::get_var_expr_var(scip, sourceexpr);
    create_expr_var(scip, conshdlr, var)
}

/// `mapexprtransvar`: map var exprs to var-expr from var2expr hashmap
/// corresponding to transformed var; `mapexprdata` is the constraint
/// handler.
fn mapexprtransvar(scip: &mut Scip, sourceexpr: ExprId, _ownercreate: Option<expr::ExprOwnerCreate>, _ownercreatedata: usize, mapexprdata: usize) -> ScipResult<ExprId> {
    let conshdlr = mapexprdata;

    // Do not provide map if not variable.
    if !expr::expr_is_var(scip, sourceexpr) {
        return Ok(NONE);
    }

    let mut var = crate::expr_var::get_var_expr_var(scip, sourceexpr);

    // Transform variable.
    var = crate::api::get_transformed_var(scip, var)?;

    create_expr_var(scip, conshdlr, var)
}

/// `storeVarExprs`: stores all variable expressions into a given
/// constraint.
fn store_var_exprs(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    // Skip if we have stored the variable expressions already.
    if cd(scip, cons).varexprs.is_some() {
        return Ok(());
    }

    // (The C first gets an upper bound on number of variable expressions to
    // size the array.)
    let expr = cd(scip, cons).expr;
    let varexprs = crate::scip_expr::get_expr_var_exprs(scip, expr)?;

    // Ensure that for every variable an entry exists in the var2expr hashmap;
    // when removing duplicate subexpressions it can happen that a var->varexpr
    // map was removed from the hashmap.
    for &ve in &varexprs {
        let var = crate::expr_var::get_var_expr_var(scip, ve);
        hdm(scip, conshdlr).var2expr.entry(var).or_insert(ve);
    }

    cdm(scip, cons).varexprs = Some(varexprs);

    Ok(())
}

/// `freeVarExprs`: frees all variable expression stored in storeVarExprs().
fn free_var_exprs(scip: &mut Scip, cons: ConsId) -> ScipResult {
    // Skip if we have stored the variable expressions already.
    let Some(varexprs) = cdm(scip, cons).varexprs.take() else {
        return Ok(());
    };

    // Release variable expressions.
    for mut e in varexprs {
        expr::expr_release(scip, &mut e)?;
    }

    Ok(())
}

/// `intEvalVarBoundTightening`: interval evaluation of variables as used in
/// bound tightening; `intevalvardata` is the constraint handler.
///
/// Returns slightly relaxed local variable bounds of a variable as interval.
/// Does not relax beyond integer values, thus does not relax bounds on
/// integer variables at all.
fn int_eval_var_bound_tightening(scip: &mut Scip, var: VarId, intevalvardata: usize) -> Interval {
    let h = hd(scip, intevalvardata);
    let v = &scip.vars[var];

    let (mut lb, mut ub) = if h.globalbounds { (v.glbdom.lb, v.glbdom.ub) } else { (v.locdom.lb, v.locdom.ub) };

    // Implicit integer variables may have non-integer bounds, apparently (run
    // space25a).
    if crate::var::var_is_implied_integral(scip, var) {
        lb = crate::def::eps_round(lb, 0.0);
        ub = crate::def::eps_round(ub, 0.0);
    }

    // Integer variables should always have integral bounds in SCIP.

    match h.varboundrelax {
        b'n' => {
            // No relaxation.
        }
        b'a' => {
            // Relax by absolute value; do not look at integer variables, they
            // already have integral bounds, so wouldn't be relaxed.
            if !crate::var::var_is_integral(scip, var) {
                if !scip.set.is_infinity(-lb) {
                    // Reduce lb by epsilon, or to the next integer value, which ever is
                    // larger.
                    let bnd = lb.floor();
                    lb = cmax(bnd, lb - h.varboundrelaxamount);
                }

                if !scip.set.is_infinity(ub) {
                    // Increase ub by epsilon, or to the next integer value, which ever is
                    // smaller.
                    let bnd = ub.ceil();
                    ub = cmin(bnd, ub + h.varboundrelaxamount);
                }
            }
        }
        b'b' => {
            // Relax always by absolute value; do not look at integer variables,
            // they already have integral bounds, so wouldn't be relaxed.
            if !crate::var::var_is_integral(scip, var) {
                if !scip.set.is_infinity(-lb) {
                    lb -= h.varboundrelaxamount;
                }

                if !scip.set.is_infinity(ub) {
                    ub += h.varboundrelaxamount;
                }
            }
        }
        b'r' => {
            // Relax by relative value; do not look at integer variables, they
            // already have integral bounds, so wouldn't be relaxed.
            if !crate::var::var_is_integral(scip, var) {
                // Relax bounds by epsilon*max(1,|bnd|), instead of just epsilon as in
                // case 'a', thus we trust the first log(epsilon) digits; however, when
                // domains get small, relaxing can excessively weaken bound tightening,
                // thus do only fraction of |ub-lb| if that is smaller; further, do not
                // relax beyond next integer value.
                if !scip.set.is_infinity(-lb) {
                    let bnd = lb.floor();
                    lb = cmax(bnd, lb - cmin(h.varboundrelaxamount * cmax(1.0, lb.abs()), 0.001 * (ub - lb).abs()));
                }

                if !scip.set.is_infinity(ub) {
                    let bnd = ub.ceil();
                    ub = cmin(bnd, ub + cmin(h.varboundrelaxamount * cmax(1.0, ub.abs()), 0.001 * (ub - lb).abs()));
                }
            }
        }
        c => {
            log::error!("Unsupported value '{}' for varboundrelax option.", c as char);
            panic!("SCIPABORT");
        }
    }

    // Convert SCIPinfinity() to SCIP_INTERVAL_INFINITY.
    let inf = scip.set.infinity();
    lb = -infty2infty(inf, INTERVAL_INFINITY, -lb);
    ub = infty2infty(inf, INTERVAL_INFINITY, ub);

    let mut interval = Interval::default();
    interval.set_bounds(lb, ub);

    interval
}

/// `compIndexConsNonlinear`: compares two nonlinear constraints by its
/// index.
pub fn comp_index_cons_nonlinear(scip: &Scip, elem1: ConsId, elem2: ConsId) -> i32 {
    cd(scip, elem1).consindex.wrapping_sub(cd(scip, elem2).consindex)
}

/// `processVarEvent`: processes variable fixing or bound change event; the
/// event data is the variable expression.
fn process_var_event(scip: &mut Scip, _eventhdlr: EventhdlrId, ev: &Event, eventdata: usize) -> ScipResult {
    let eventtype = ev.eventtype;
    let expr = eventdata;
    let mut boundtightened = false;

    // We only catch varevents for variables in constraints, so there should be
    // constraints.

    if eventtype & event::EVENTTYPE_BOUNDTIGHTENED != 0 {
        boundtightened = true;
    }

    // Usually, if fixing a variable results in a boundchange, we should have
    // seen a boundtightened-event as well; however, if the boundchange is
    // smaller than epsilon, such an event will be omitted, but we still want to
    // make sure the activity of the var-expr is reevaluated (mainly to avoid a
    // failing assert) in this case; since we cannot easily see whether a
    // variable bound was actually changed in a varfixed event, we treat any
    // varfixed event as a boundtightening (and usually it is, I would think).
    if eventtype & event::EVENTTYPE_VARFIXED != 0 {
        boundtightened = true;
    }

    // If a variable is changed to implicit-integer and has a fractional bound,
    // then the behavior of intEvalVarBoundTightening is changing, because we
    // will round the bounds and no longer consider relaxing them; we will mark
    // corresponding constraints as not-propagated in this case to get the
    // tightened bounds on the var-expr (mainly to avoid a failing assert, see
    // github issue #70); usually, a change to implicit-integer would result in a
    // boundchange on the variable as well, but not if the bound was already
    // almost integral.
    if eventtype & event::EVENTTYPE_IMPLTYPECHANGED != 0 && ev.data.newimpltype != crate::types::ImplintType::None {
        let v = &scip.vars[ev.data.var];
        if !eps_is_int(v.glbdom.lb, 0.0) || !eps_is_int(v.glbdom.ub, 0.0) {
            boundtightened = true;
        }
    }

    // Notify constraints that use this variable expression (expr) to
    // repropagate and possibly resimplify:
    // - propagation can only find something new if a bound was tightened
    // - simplify can only find something new if a var is fixed (or maybe a
    //   bound is tightened) and we look at global changes (that is, we are not
    //   looking at boundchanges in probing)
    if boundtightened {
        let conss = od(scip, expr).conss.clone();
        let presolving = scip.set.stage == Stage::Presolving && !crate::api::in_probing(scip);
        for c in conss {
            let consdata = cdm(scip, c);

            // If bound tightening, then mark constraints to be propagated again.
            // TODO we could try be more selective here and only trigger a propagation
            // if a relevant bound has changed, that is, we don't need to repropagate
            // x + ... <= rhs if only the upper bound of x has been tightened; the
            // locks don't help since they are not available separately for each
            // constraint.
            consdata.ispropagated = false;

            // If still in presolve (but not probing), then mark constraints to be
            // unsimplified.
            if presolving {
                consdata.issimplified = false;
            }
        }
    }

    // Update curboundstag, lastboundrelax, and expr activity.
    if (eventtype & event::EVENTTYPE_BOUNDCHANGED != 0) || boundtightened {
        let conshdlr = od(scip, expr).conshdlr;
        let h = hdm(scip, conshdlr);

        // Increase tag on bounds.
        h.curboundstag += 1;

        // Remember also if we relaxed bounds now.
        if eventtype & event::EVENTTYPE_BOUNDRELAXED != 0 {
            h.lastboundrelax = h.curboundstag;
        }

        // Update the activity of the var-expr here immediately (we could call
        // expr->activity = intevalvar(var, consdhlr) directly, but then the
        // exprhdlr statistics are not updated).
        let intevalvar = h.intevalvar.func();
        let mut activity = Interval::default();
        let eh = scip.exprs[expr].exprhdlr;
        expr::exprhdlr_int_eval_expr(scip, eh, expr, &mut activity, Some(intevalvar), conshdlr)?;

        let tag = hd(scip, conshdlr).curboundstag;
        let e = &mut scip.exprs[expr];
        e.activity = activity;
        e.activitytag = tag;
    }

    Ok(())
}

/// The variable events the constraint handler catches on variables of its
/// constraints.
const VAREVENTTYPE: event::EventType = event::EVENTTYPE_BOUNDCHANGED | event::EVENTTYPE_VARFIXED | event::EVENTTYPE_TYPECHANGED | event::EVENTTYPE_IMPLTYPECHANGED;

/// `catchVarEvent`: registers event handler to catch variable events on
/// variable.
///
/// Additionally, the given constraint is stored in the ownerdata of the
/// variable-expression. When an event occurs, all stored constraints are
/// notified.
fn catch_var_event(scip: &mut Scip, eventhdlr: EventhdlrId, expr: ExprId, cons: ConsId) -> ScipResult {
    // Append cons to ownerdata->conss; we're not capturing the constraint here
    // to avoid circular references.
    odm(scip, expr).conss.push(cons);

    // Updated sorted flag.
    let n = od(scip, expr).conss.len();
    if n <= 1 {
        odm(scip, expr).consssorted = true;
    } else if od(scip, expr).consssorted {
        let (a, b) = (od(scip, expr).conss[n - 2], od(scip, expr).conss[n - 1]);
        odm(scip, expr).consssorted = comp_index_cons_nonlinear(scip, a, b) < 0;
    }

    // Catch variable events, if not done so yet (first constraint).
    if od(scip, expr).filterpos < 0 {
        let var = crate::expr_var::get_var_expr_var(scip, expr);
        let mut filterpos = -1;
        crate::api::catch_var_event(scip, var, VAREVENTTYPE, eventhdlr, expr, Some(&mut filterpos))?;
        odm(scip, expr).filterpos = filterpos;
    }

    Ok(())
}

/// `catchVarEvents`: catch variable events.
fn catch_var_events(scip: &mut Scip, eventhdlr: EventhdlrId, cons: ConsId) -> ScipResult {
    // Check if we have catched variable events already.
    if cd(scip, cons).catchedevents {
        return Ok(());
    }

    let conshdlr = cons_hdlr(scip, cons);

    let varexprs = cd(scip, cons).varexprs.clone().unwrap_or_default();
    for expr in varexprs {
        catch_var_event(scip, eventhdlr, expr, cons)?;

        // From now on, activity of var-expr will usually be updated in
        // processVarEvent if variable bound is changing; since we just registered
        // this eventhdlr, we should make sure that the activity is also up to date
        // now.
        if scip.exprs[expr].activitytag < hd(scip, conshdlr).curboundstag {
            let mut activity = Interval::default();
            let eh = scip.exprs[expr].exprhdlr;
            expr::exprhdlr_int_eval_expr(scip, eh, expr, &mut activity, Some(int_eval_var_bound_tightening), conshdlr)?;
            let tag = hd(scip, conshdlr).curboundstag;
            let e = &mut scip.exprs[expr];
            e.activity = activity;
            e.activitytag = tag;
        }
    }

    cdm(scip, cons).catchedevents = true;

    Ok(())
}

/// `dropVarEvent`: unregisters event handler to catch variable events on
/// variable.
///
/// The given constraint is removed from the constraints array in the
/// ownerdata of the variable-expression. If this was the last constraint,
/// then the event handler is unregistered for this variable.
fn drop_var_event(scip: &mut Scip, eventhdlr: EventhdlrId, expr: ExprId, cons: ConsId) -> ScipResult {
    let n = od(scip, expr).conss.len();
    let pos;
    if od(scip, expr).conss[n - 1] == cons {
        pos = n - 1;
    } else {
        if !od(scip, expr).consssorted {
            let mut conss = std::mem::take(&mut odm(scip, expr).conss);
            crate::sort::sort(&mut conss, (), |&a, &b| comp_index_cons_nonlinear(scip, a, b));
            let o = odm(scip, expr);
            o.conss = conss;
            o.consssorted = true;
        }

        match crate::sort::sortedvec_find(&od(scip, expr).conss, |&a, &b| comp_index_cons_nonlinear(scip, a, b), cons) {
            Ok(p) => pos = p,
            Err(_) => {
                log::error!(
                    "Constraint <{}> not in constraint array of expression for variable <{}>",
                    scip.conss[cons].name,
                    scip.vars[crate::expr_var::get_var_expr_var(scip, expr)].name
                );
                return Err(Retcode::Error);
            }
        }
    }

    // Move last constraint into position of removed constraint.
    let o = odm(scip, expr);
    if pos < n - 1 {
        o.conss[pos] = o.conss[n - 1];
        o.consssorted = false;
    }
    o.conss.pop();

    // Drop variable events if that was the last constraint.
    if o.conss.is_empty() {
        let filterpos = o.filterpos;
        let var = crate::expr_var::get_var_expr_var(scip, expr);
        crate::api::drop_var_event(scip, var, VAREVENTTYPE, eventhdlr, expr, filterpos)?;
        odm(scip, expr).filterpos = -1;
    }

    Ok(())
}

/// `dropVarEvents`: drop variable events.
fn drop_var_events(scip: &mut Scip, eventhdlr: EventhdlrId, cons: ConsId) -> ScipResult {
    // Check if we have catched variable events already.
    if !cd(scip, cons).catchedevents {
        return Ok(());
    }

    let varexprs = cd(scip, cons).varexprs.clone().unwrap_or_default();
    for &e in varexprs.iter().rev() {
        drop_var_event(scip, eventhdlr, e, cons)?;
    }

    cdm(scip, cons).catchedevents = false;

    Ok(())
}

/// `createCons`: creates and captures a nonlinear constraint.
///
/// @attention Use copyexpr=FALSE only if expr is already "owned" by
/// conshdlr, that is, if expressions were created with exprownerCreate() and
/// ownerdata passed in the last two arguments.
fn create_cons(scip: &mut Scip, conshdlr: ConshdlrId, name: &str, expr: ExprId, lhs: f64, rhs: f64, copyexpr: bool, flags: &crate::cons::ConsFlags) -> ScipResult<ConsId> {
    if flags.local && scip.set.stage == Stage::Solving && crate::api::get_depth(scip) != 0 {
        log::error!("Locally valid nonlinear constraints are not supported, yet.");
        return Err(Retcode::InvalidCall);
    }

    // TODO we should allow for non-initial nonlinear constraints.
    if !flags.initial {
        log::error!("Non-initial nonlinear constraints are not supported, yet.");
        return Err(Retcode::InvalidCall);
    }

    if lhs.is_nan() || rhs.is_nan() {
        log::error!("{} hand side of nonlinear constraint <{name}> is nan", if lhs.is_nan() { "left" } else { "right" });
        return Err(Retcode::InvalidData);
    }

    // Create constraint data.
    let consexpr = if copyexpr {
        // Copy expression, thereby map variables expressions to already existing
        // variables expressions in var2expr map, or augment var2expr map.
        crate::scip_expr::duplicate_expr(scip, expr, Some(mapexprvar), conshdlr, Some(exprowner_create), conshdlr)?
    } else {
        expr::expr_capture(scip, expr);
        expr
    };

    let h = hdm(scip, conshdlr);
    let consindex = h.lastconsindex;
    h.lastconsindex += 1;

    let consdata = ConsData {
        expr: consexpr,
        lhs,
        rhs,
        varexprs: None,
        catchedevents: false,
        lhsviol: 0.0,
        rhsviol: 0.0,
        gradnorm: 0.0,
        gradnormsoltag: 0,
        ispropagated: false,
        issimplified: false,
        nlockspos: 0,
        nlocksneg: 0,
        linvardecr: NONE,
        linvarincr: NONE,
        linvardecrcoef: 0.0,
        linvarincrcoef: 0.0,
        curv: ExprCurv::Unknown,
        nlrow: NONE,
        consindex,
    };

    // Create constraint.
    let mut f = *flags;
    f.stickingatnode = false;
    crate::scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), &f)
}

/// `getExprAbsOrigViolation`: returns absolute violation for auxvar
/// relation in an expression w.r.t. original variables.
///
/// Assume the expression is f(x), where x are original (i.e., not
/// auxiliary) variables. Assume that f(x) is associated with auxiliary
/// variable z.
///
/// If there are negative locks, then return the violation of z <= f(x) and
/// sets `violover` to TRUE. If there are positive locks, then return the
/// violation of z >= f(x) and sets `violunder` to TRUE. Of course, if there
/// both negative and positive locks, then return the violation of z = f(x).
/// If f could not be evaluated, then return SCIPinfinity() and set both
/// `violover` and `violunder` to TRUE.
///
/// @note This does not reevaluate the violation, but assumes that the
/// expression has been evaluated.
fn get_expr_abs_orig_violation(scip: &Scip, expr: ExprId, sol: SolId, violunder: Option<&mut bool>, violover: Option<&mut bool>) -> f64 {
    let evalvalue = scip.exprs[expr].evalvalue;
    get_expr_abs_aux_violation(scip, expr, evalvalue, sol, violunder, violover)
}

/// `getExprAbsAuxViolation`: returns absolute violation for auxvar relation
/// in an expression w.r.t. auxiliary variables.
///
/// Assume the expression is f(w), where w are auxiliary variables that were
/// introduced by some nlhdlr. Assume that f(w) is associated with auxiliary
/// variable z.
///
/// If there are negative locks, then return the violation of z <= f(w) and
/// sets `violover` to TRUE. If there are positive locks, then return the
/// violation of z >= f(w) and sets `violunder` to TRUE. Of course, if there
/// both negative and positive locks, then return the violation of z = f(w).
/// If f could not be evaluated, then return SCIPinfinity() and set both
/// `violover` and `violunder` to TRUE.
///
/// @note This does not reevaluate the violation, but assumes that f(w) is
/// passed in with auxvalue.
fn get_expr_abs_aux_violation(scip: &Scip, expr: ExprId, auxvalue: f64, sol: SolId, violunder: Option<&mut bool>, violover: Option<&mut bool>) -> f64 {
    let ownerdata = od(scip, expr);

    let set = |under: bool, over: bool, violunder: Option<&mut bool>, violover: Option<&mut bool>| {
        if let Some(v) = violunder {
            *v = under;
        }
        if let Some(v) = violover {
            *v = over;
        }
    };

    if auxvalue == INVALID {
        set(true, true, violunder, violover);
        return scip.set.infinity();
    }

    let auxvarvalue = crate::api::get_sol_val(scip, sol, ownerdata.auxvar);

    if ownerdata.nlocksneg > 0 && auxvarvalue > auxvalue {
        set(false, true, violunder, violover);
        return auxvarvalue - auxvalue;
    }

    if ownerdata.nlockspos > 0 && auxvalue > auxvarvalue {
        set(true, false, violunder, violover);
        return auxvalue - auxvarvalue;
    }

    set(false, false, violunder, violover);
    0.0
}

/// `computeViolation`: computes violation of a constraint.
fn compute_violation(scip: &mut Scip, cons: ConsId, sol: SolId, soltag: i64) -> ScipResult {
    let expr = cd(scip, cons).expr;

    expr::expr_eval(scip, expr, sol, soltag)?;
    let activity = scip.exprs[expr].evalvalue;

    let inf = scip.set.infinity();

    // Consider constraint as violated if it is undefined in the current point.
    if activity == INVALID {
        let consdata = cdm(scip, cons);
        consdata.lhsviol = inf;
        consdata.rhsviol = inf;
        return Ok(());
    }

    // Compute violations.
    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
    let lhsviol = if scip.set.is_infinity(-lhs) { -inf } else { lhs - activity };
    let rhsviol = if scip.set.is_infinity(rhs) { -inf } else { activity - rhs };
    let consdata = cdm(scip, cons);
    consdata.lhsviol = lhsviol;
    consdata.rhsviol = rhsviol;

    Ok(())
}

/// C's `MAX3(x, y, z)`.
#[inline]
fn cmax3(x: f64, y: f64, z: f64) -> f64 {
    if x >= y { cmax(x, z) } else { cmax(y, z) }
}

/// `getConsAbsViolation`: returns absolute violation of a constraint.
///
/// @note This does not reevaluate the violation, but assumes that
/// computeViolation() has been called before.
fn get_cons_abs_violation(scip: &Scip, cons: ConsId) -> f64 {
    let consdata = cd(scip, cons);
    cmax3(0.0, consdata.lhsviol, consdata.rhsviol)
}

/// `getConsRelViolation`: computes relative violation of a constraint.
///
/// @note This does not reevaluate the violation, but assumes that
/// computeViolation() has been called before.
fn get_cons_rel_violation(scip: &mut Scip, cons: ConsId, sol: SolId, soltag: i64) -> ScipResult<f64> {
    let conshdlr = cons_hdlr(scip, cons);
    let violscale = hd(scip, conshdlr).violscale;

    let mut viol = get_cons_abs_violation(scip, cons);

    if violscale == b'n' {
        return Ok(viol);
    }

    if scip.set.is_infinity(viol) {
        return Ok(viol);
    }

    if violscale == b'a' {
        let consdata = cd(scip, cons);
        let mut scale = cmax(1.0, scip.exprs[consdata.expr].evalvalue.abs());

        // Consider value of side that is violated for scaling, too.
        if consdata.lhsviol > 0.0 && consdata.lhs.abs() > scale {
            scale = consdata.lhs.abs();
        } else if consdata.rhsviol > 0.0 && consdata.rhs.abs() > scale {
            scale = consdata.rhs.abs();
        }

        viol /= scale;
        return Ok(viol);
    }

    // If not 'n' or 'a', then it has to be 'g' at the moment.
    if soltag == 0 || cd(scip, cons).gradnormsoltag != soltag {
        // We need the varexprs to conveniently access the gradient.
        store_var_exprs(scip, conshdlr, cons)?;

        // Update cached value of norm of gradient.
        cdm(scip, cons).gradnorm = 0.0;

        // Compute gradient.
        let expr = cd(scip, cons).expr;
        expr::expr_eval_gradient(scip, expr, sol, soltag)?;

        // Gradient evaluation error -> no scaling.
        let mut gradnorm = 0.0;
        if scip.exprs[expr].derivative != INVALID {
            for &ve in cd(scip, cons).varexprs.as_ref().unwrap() {
                let deriv = scip.exprs[ve].derivative;
                if deriv == INVALID {
                    gradnorm = 0.0;
                    break;
                }

                gradnorm += deriv * deriv;
            }
        }
        let consdata = cdm(scip, cons);
        consdata.gradnorm = gradnorm.sqrt();
        consdata.gradnormsoltag = soltag;
    }

    viol /= cmax(1.0, cd(scip, cons).gradnorm);

    Ok(viol)
}

/// `isConsViolated`: returns whether constraint is currently violated.
///
/// @note This does not reevaluate the violation, but assumes that
/// computeViolation() has been called before.
fn is_cons_violated(scip: &Scip, cons: ConsId) -> bool {
    get_cons_abs_violation(scip, cons) > scip.set.feastol()
}

/// `findUnlockedLinearVar`: checks for a linear variable that can be
/// increased or decreased without harming feasibility.
fn find_unlocked_linear_var(scip: &mut Scip, cons: ConsId) {
    {
        let consdata = cdm(scip, cons);
        consdata.linvarincr = NONE;
        consdata.linvardecr = NONE;
        consdata.linvarincrcoef = 0.0;
        consdata.linvardecrcoef = 0.0;
    }

    let expr = cd(scip, cons).expr;

    // Root expression is not a sum -> no unlocked linear variable available.
    if !expr::expr_is_sum(scip, expr) {
        return;
    }

    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);

    for i in 0..scip.exprs[expr].children.len() {
        let child = scip.exprs[expr].children[i];

        // Check whether the child is a variable expression.
        if expr::expr_is_var(scip, child) {
            let var = crate::expr_var::get_var_expr_var(scip, child);
            let coef = crate::expr_sum::get_coefs_expr_sum(scip, expr)[i];

            let (poslock, neglock) = if coef > 0.0 {
                ((!scip.set.is_infinity(rhs)) as i32, (!scip.set.is_infinity(-lhs)) as i32)
            } else {
                ((!scip.set.is_infinity(-lhs)) as i32, (!scip.set.is_infinity(rhs)) as i32)
            };

            let obj = scip.vars[var].obj;

            if crate::var::var_get_n_locks_down_type(scip, var, LockType::Model) - neglock == 0 {
                // For a*x + f(y) \in [lhs, rhs], we can decrease x without harming other
                // constraints; if we have already one candidate, then take the one
                // where the loss in the objective function is less.
                let consdata = cd(scip, cons);
                if consdata.linvardecr == NONE || scip.vars[consdata.linvardecr].obj / consdata.linvardecrcoef > obj / coef {
                    let consdata = cdm(scip, cons);
                    consdata.linvardecr = var;
                    consdata.linvardecrcoef = coef;
                }
            }

            if crate::var::var_get_n_locks_up_type(scip, var, LockType::Model) - poslock == 0 {
                // For a*x + f(y) \in [lhs, rhs], we can increase x without harm; if we
                // have already one candidate, then take the one where the loss in the
                // objective function is less.
                let consdata = cd(scip, cons);
                if consdata.linvarincr == NONE || scip.vars[consdata.linvarincr].obj / consdata.linvarincrcoef > obj / coef {
                    let consdata = cdm(scip, cons);
                    consdata.linvarincr = var;
                    consdata.linvarincrcoef = coef;
                }
            }
        }
    }
}

/// `proposeFeasibleSolution`: given a solution where every nonlinear
/// constraint is either feasible or can be made feasible by moving a linear
/// variable, construct the corresponding feasible solution and pass it to
/// the trysol heuristic.
///
/// The method assumes that this is always possible and that not all
/// constraints are feasible already.
fn propose_feasible_solution(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], sol: SolId) -> ScipResult<bool> {
    let mut success = false;

    // Don't propose new solutions if not in presolve or solving.
    if scip.set.stage < Stage::InitPresolve || scip.set.stage >= Stage::Solved {
        return Ok(false);
    }

    let newsol = if sol != NONE {
        crate::api::create_sol_copy(scip, sol)?
    } else {
        create_lp_sol(scip, NONE)?
    };
    crate::api::unlink_sol(scip, newsol)?;

    let feastol = scip.set.feastol();
    let mut c = 0;
    while c < conss.len() {
        let cons = conss[c];
        let consdata = cd(scip, cons).clone();
        let mut viol;

        // Get absolute violation and sign.
        if consdata.lhsviol > feastol {
            viol = consdata.lhsviol; // lhs - activity
        } else if consdata.rhsviol > feastol {
            viol = -consdata.rhsviol; // rhs - activity
        } else {
            c += 1;
            continue; // Constraint is satisfied.
        }

        if consdata.linvarincr != NONE && ((viol > 0.0 && consdata.linvarincrcoef > 0.0) || (viol < 0.0 && consdata.linvarincrcoef < 0.0)) {
            let var = consdata.linvarincr;

            // Compute how much we would like to increase var.
            let mut delta = viol / consdata.linvarincrcoef;

            // If var has an upper bound, may need to reduce delta.
            let ub = scip.vars[var].glbdom.ub;
            if !scip.set.is_infinity(ub) {
                let gap = ub - crate::api::get_sol_val(scip, newsol, var);
                delta = cmin(cmax(0.0, gap), delta);
            }
            if scip.set.is_positive(delta) {
                // If variable is integral, round delta up so that it will still have an
                // integer value.
                if crate::var::var_is_integral(scip, var) {
                    delta = scip.set.ceil(delta);
                }

                crate::sol::sol_inc_val(scip, newsol, var, delta)?;

                // Adjust constraint violation, if satisfied go on to next constraint.
                viol -= consdata.linvarincrcoef * delta;
                if scip.set.is_zero(viol) {
                    c += 1;
                    continue;
                }
            }
        }

        if consdata.linvardecr != NONE && ((viol > 0.0 && consdata.linvardecrcoef < 0.0) || (viol < 0.0 && consdata.linvardecrcoef > 0.0)) {
            let var = consdata.linvardecr;

            // Compute how much we would like to decrease var.
            let mut delta = viol / consdata.linvardecrcoef;

            // If var has a lower bound, may need to reduce delta.
            let lb = scip.vars[var].glbdom.lb;
            if !scip.set.is_infinity(-lb) {
                let gap = crate::api::get_sol_val(scip, newsol, var) - lb;
                delta = cmax(cmin(0.0, gap), delta);
            }
            if scip.set.is_negative(delta) {
                // If variable is integral, round delta down so that it will still have
                // an integer value.
                if crate::var::var_is_integral(scip, var) {
                    delta = scip.set.floor(delta);
                }

                crate::sol::sol_inc_val(scip, newsol, consdata.linvardecr, delta)?;

                // Adjust constraint violation, if satisfied go on to next constraint.
                viol -= consdata.linvardecrcoef * delta;
                if scip.set.is_zero(viol) {
                    c += 1;
                    continue;
                }
            }
        }

        // Still here... so probably we could not make constraint feasible due to
        // variable bounds, thus give up.
        break;
    }

    // If we have a solution that should satisfy all quadratic constraints and
    // has a better objective than the current upper bound, then pass it to the
    // trysol heuristic.
    if c == conss.len() {
        let upperbound = crate::api::get_upperbound(scip);
        if scip.set.is_infinity(upperbound) || {
            let obj = crate::api::get_sol_trans_obj(scip, newsol);
            scip.set.is_sum_lt(obj, upperbound)
        } {
            let trysolheur = hd(scip, conshdlr).trysolheur;
            crate::heur_trysol::heur_pass_sol_try_sol(scip, trysolheur, newsol)?;
            success = true;
        }
    }

    crate::api::free_sol(scip, newsol)?;

    Ok(success)
}

/// `SCIPcreateLPSol`: creates a primal solution, initialized to the current
/// LP solution.
fn create_lp_sol(scip: &mut Scip, heur: HeurId) -> ScipResult<SolId> {
    if !crate::tree::tree_has_current_node_lp(scip) {
        log::error!("LP solution does not exist");
        return Err(Retcode::InvalidCall);
    }

    crate::sol::sol_create_lp_sol(scip, crate::prob::TRANSPROB, crate::scip::TRANSPRIMAL, heur)
}

/// `notifyNlhdlrNewsol`: notify nonlinear handlers to add linearization in
/// new solution that has been found.
///
/// The idea is that nonlinear handlers add globally valid tight estimators
/// in a given solution as cuts to the cutpool.
///
/// Essentially we want to ensure that the LP relaxation is tight in the new
/// solution, if possible. As the nonlinear handlers define the extended
/// formulation, they should know whether it is possible to generate a cut
/// that is valid and supporting in the given solution. For example, for
/// convex constraints, we achieve this by linearizing. For SOC, we also
/// linearize, but on a a convex reformulation.
///
/// Since linearization may happen in auxiliary variables, we ensure that
/// auxiliary variables are set to the eval-value of its expression, i.e., we
/// change sol so it is also feasible in the extended formulation.
fn notify_nlhdlr_newsol(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], sol: SolId, solisbest: bool) -> ScipResult {
    // TODO probably we just evaluated all expressions when checking the sol
    // before it was added; would be nice to recognize this and skip
    // reevaluating.
    let soltag = expr::get_expr_new_soltag(scip);

    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_LEAVEEXPR);

    for &cons in conss {
        // Skip constraints that are not enabled or deleted or have separation
        // disabled.
        let c = &scip.conss[cons];
        if !c.enabled || c.deleted || !c.sepaenabled {
            continue;
        }

        let consexpr = cd(scip, cons).expr;
        expr::expr_eval(scip, consexpr, sol, soltag)?;

        let mut e = it.restart_dfs(scip, consexpr);
        while !it.is_end() {
            // Set value for auxvar in sol to value of expr, in case it is used to
            // compute estimators higher up of this expression.
            let auxvar = od(scip, e).auxvar;
            if auxvar != NONE {
                let v = scip.exprs[e].evalvalue;
                crate::api::set_sol_val(scip, sol, auxvar, v)?;
            }

            // Let nonlinear handler generate cuts by calling the sollinearize
            // callback.
            for k in 0..od(scip, e).nenfos.max(0) as usize {
                let enfo = od(scip, e).enfos[k];
                // Call sollinearize callback, if implemented by nlhdlr.
                nlhdlr::nlhdlr_sollinearize(
                    scip,
                    conshdlr,
                    cons,
                    enfo.nlhdlr,
                    e,
                    enfo.nlhdlrexprdata,
                    sol,
                    solisbest,
                    enfo.nlhdlrparticipation & nlhdlr::NLHDLR_METHOD_SEPAABOVE != 0,
                    enfo.nlhdlrparticipation & nlhdlr::NLHDLR_METHOD_SEPABELOW != 0,
                )?;
            }

            e = it.get_next(scip);
        }
    }

    it.free(scip);

    Ok(())
}

/// `processNewSolutionEvent`: processes the event that a new primal solution
/// has been found; the event data is the constraint handler.
fn process_new_solution_event(scip: &mut Scip, _eventhdlr: EventhdlrId, ev: &Event, eventdata: usize) -> ScipResult {
    let conshdlr = eventdata;

    if scip.conshdlrs[conshdlr].nconss == 0 {
        return Ok(());
    }

    let sol = ev.data.sol;

    // We are only interested in solution coming from some heuristic other than
    // trysol, but not from the tree; the reason for ignoring trysol solutions is
    // that they may come from the tree, but postprocessed via
    // proposeFeasibleSolution.
    let solheur = crate::sol::sol_get_heur(scip, sol);
    if solheur == NONE || solheur == hd(scip, conshdlr).trysolheur {
        return Ok(());
    }

    let conss = crate::cons::conshdlr_get_conss(scip, conshdlr);
    notify_nlhdlr_newsol(scip, conshdlr, &conss, sol, ev.eventtype & event::EVENTTYPE_BESTSOLFOUND != 0)
}

/// `tightenAuxVarBounds`: tightens the bounds of the auxiliary variable
/// associated with an expression (or original variable if being a
/// variable-expression) according to given bounds.
///
/// The given bounds may very well be the exprs activity (when called from
/// forwardPropExpr()), but can also be some tighter bounds (when called from
/// SCIPtightenExprIntervalNonlinear()).
///
/// Nothing will happen if SCIP is not in presolve or solve.
fn tighten_aux_var_bounds(scip: &mut Scip, conshdlr: ConshdlrId, expr: ExprId, bounds: Interval, cutoff: &mut bool, mut ntightenings: Option<&mut i32>) -> ScipResult {
    // The given bounds must not be empty (we could cope, but we shouldn't be
    // called in this situation).

    *cutoff = false;

    let var = get_expr_aux_var_nonlinear(scip, expr);
    if var == NONE {
        return Ok(());
    }

    // Force tightening if conshdlrdata says so or it would mean fixing the
    // variable.
    let force = hd(scip, conshdlr).forceboundtightening || scip.set.is_eq(bounds.inf, bounds.sup);

    // Try to tighten lower bound of (auxiliary) variable.
    let (infeasible, tightenedlb) = crate::scip_var::tighten_var_lb(scip, var, bounds.inf, force)?;
    *cutoff = infeasible;
    if tightenedlb && let Some(n) = ntightenings.as_deref_mut() {
        *n += 1;
    }
    if *cutoff {
        return Ok(());
    }

    // Try to tighten upper bound of (auxiliary) variable.
    let (infeasible, tightenedub) = crate::scip_var::tighten_var_ub(scip, var, bounds.sup, force)?;
    *cutoff = infeasible;
    if tightenedub && let Some(n) = ntightenings {
        *n += 1;
    }

    // TODO expr->activity should have been reevaluated now due to
    // boundchange-events, but it used to relax bounds; that seems unnecessary
    // and we could easily undo this here, e.g., if( tightenedlb )
    // expr->activity.inf = bounds.inf.

    Ok(())
}

/// `forwardPropExpr`: propagate bounds of the expressions in a given
/// expression tree (that is, updates activity intervals) and tries to
/// tighten the bounds of the auxiliary variables accordingly.
fn forward_prop_expr(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    rootexpr: ExprId,
    tightenauxvars: bool,
    mut infeasible: Option<&mut bool>,
    mut ntightenings: Option<&mut i32>,
) -> ScipResult {
    if let Some(i) = infeasible.as_deref_mut() {
        *i = false;
    }
    if let Some(n) = ntightenings.as_deref_mut() {
        *n = 0;
    }

    let (lastboundrelax, curboundstag) = {
        let h = hd(scip, conshdlr);
        (h.lastboundrelax, h.curboundstag)
    };

    // If value is valid and empty, then we cannot improve, so do nothing.
    if scip.exprs[rootexpr].activitytag >= lastboundrelax && interval::is_empty(INTERVAL_INFINITY, scip.exprs[rootexpr].activity) {
        if let Some(i) = infeasible.as_deref_mut() {
            *i = true;
        }

        // Just update tag to curboundstag.
        scip.exprs[rootexpr].activitytag = curboundstag;

        return Ok(());
    }

    // If value is up-to-date, then nothing to do.
    if scip.exprs[rootexpr].activitytag == curboundstag {
        return Ok(());
    }

    // If activity of rootexpr is not used, but expr participated in detect
    // (nenfos >= 0), then we do nothing; it seems wrong to be called for such an
    // expression (unless we are in detect at the moment), so I add a
    // SCIPABORT(); during detect, we are in some in-between state where we may
    // want to eval activity on exprs that we did not notify about their
    // activity usage.
    {
        let o = od(scip, rootexpr);
        if o.nenfos >= 0 && o.nactivityusesprop == 0 && o.nactivityusessepa == 0 && !hd(scip, conshdlr).indetect {
            panic!("SCIPABORT: root expr activity is not used but enfo initialized");
        }
    }

    let mut it = expr::ExprIter::create();
    it.init(scip, rootexpr, expr::ExprIterType::Dfs, true)?;
    it.set_stages_dfs(scip, expr::EXPRITER_VISITINGCHILD | expr::EXPRITER_LEAVEEXPR);

    let mut e = it.get_current();
    while !it.is_end() {
        match it.get_stage_dfs() {
            expr::EXPRITER_VISITINGCHILD => {
                // Skip child if it has been evaluated already.
                let child = it.get_child_expr_dfs(scip);
                if hd(scip, conshdlr).curboundstag == scip.exprs[child].activitytag {
                    if interval::is_empty(INTERVAL_INFINITY, scip.exprs[child].activity)
                        && let Some(i) = infeasible.as_deref_mut()
                    {
                        *i = true;
                    }

                    e = it.skip_dfs(scip);
                    continue;
                }
            }
            expr::EXPRITER_LEAVEEXPR => 'leave: {
                let h = hd(scip, conshdlr);
                let (curboundstag, lastboundrelax, lastvaractivitymethodchange, globalbounds, indetect, intevalvar) =
                    (h.curboundstag, h.lastboundrelax, h.lastvaractivitymethodchange, h.globalbounds, h.indetect, h.intevalvar);

                // We should not have entered this expression if its activity was
                // already up to date.

                // For var exprs where varevents are catched, activity is updated
                // immediately when the varbound has been changed so we can assume that
                // the activity is up to date for all these variables UNLESS we changed
                // the method used to evaluate activity of variable expressions or we
                // currently use global bounds (varevents are catched for local bound
                // changes only).
                if expr::expr_is_var(scip, e) && od(scip, e).filterpos >= 0 && scip.exprs[e].activitytag >= lastvaractivitymethodchange && !globalbounds {
                    // (The reference is built without NDEBUG: it evaluates the activity
                    // once more to assert that it is up to date.)
                    let mut exprhdlrinterval = Interval::default();
                    let eh = scip.exprs[e].exprhdlr;
                    expr::exprhdlr_int_eval_expr(scip, eh, e, &mut exprhdlrinterval, Some(intevalvar.func()), conshdlr)?;

                    scip.exprs[e].activitytag = curboundstag;
                    break 'leave;
                }

                let mut activity = Interval::default();
                if scip.exprs[e].activitytag < lastboundrelax {
                    // Start with entire activity if current one is invalid.
                    activity.set_entire(INTERVAL_INFINITY);
                } else if interval::is_empty(INTERVAL_INFINITY, scip.exprs[e].activity) {
                    // If already empty, then don't try to compute even better activity. If
                    // cons_nonlinear were alone, then we should have noted that we are
                    // infeasible so an assert(infeasible == NULL || *infeasible) should work
                    // here. However, after reporting a cutoff due to expr->activity being
                    // empty, SCIP may wander to a different node and call propagation
                    // again. If no bounds in a nonlinear constraint have been relaxed when
                    // switching nodes (so expr->activitytag >= conshdlrdata->lastboundrelax),
                    // then we will still have expr->activity being empty, but will have
                    // forgotten that we found infeasibility here before
                    // (!2221#note_134120). Therefore we just set *infeasibility=TRUE here
                    // and stop.
                    if let Some(i) = infeasible.as_deref_mut() {
                        *i = true;
                    }
                    break 'leave;
                } else {
                    // Start with current activity, since it is valid.
                    activity = scip.exprs[e].activity;
                }

                // If activity of expr is not used, but expr participated in detect
                // (nenfos >= 0), then do nothing.
                {
                    let o = od(scip, e);
                    if o.nenfos >= 0 && o.nactivityusesprop == 0 && o.nactivityusessepa == 0 && !indetect {
                        break 'leave;
                    }
                }

                // Run interval eval of nonlinear handlers or expression handler.
                let nenfos = od(scip, e).nenfos;
                if nenfos > 0 {
                    // For expressions with enforcement, nlhdlrs take care of interval
                    // evaluation.
                    let mut k = 0;
                    while k < nenfos as usize && !interval::is_empty(INTERVAL_INFINITY, activity) {
                        let enfo = od(scip, e).enfos[k];
                        k += 1;

                        // Skip nlhdlr if it does not want to participate in activity
                        // computation.
                        if enfo.nlhdlrparticipation & nlhdlr::NLHDLR_METHOD_ACTIVITY == 0 {
                            continue;
                        }

                        let nh = enfo.nlhdlr;

                        // Skip nlhdlr if it does not provide interval evaluation (so it may
                        // only provide reverse propagation).
                        if scip.nlhdlrs[nh].inteval.is_none() {
                            continue;
                        }

                        // Let nlhdlr evaluate current expression.
                        let mut nlhdlrinterval = activity;
                        let intevalvar = hd(scip, conshdlr).intevalvar.func();
                        nlhdlr::nlhdlr_inteval(scip, nh, e, enfo.nlhdlrexprdata, &mut nlhdlrinterval, Some(intevalvar), conshdlr)?;

                        // Update activity by intersecting with computed activity.
                        let cur = activity;
                        interval::intersect_eps(&mut activity, scip.set.epsilon(), cur, nlhdlrinterval);
                    }
                } else {
                    // For node without enforcement (before or during detect), call the
                    // callback of the exprhdlr directly.
                    let mut exprhdlrinterval = activity;
                    let eh = scip.exprs[e].exprhdlr;
                    expr::exprhdlr_int_eval_expr(scip, eh, e, &mut exprhdlrinterval, Some(intevalvar.func()), conshdlr)?;

                    // Update expr->activity by intersecting with computed activity.
                    let cur = activity;
                    interval::intersect_eps(&mut activity, scip.set.epsilon(), cur, exprhdlrinterval);
                }

                // If expression is integral, then we try to tighten the interval bounds a
                // bit; this should undo the addition of some unnecessary safety added by
                // use of nextafter() in interval arithmetics, e.g., when doing pow(); it
                // would be ok to use ceil() and floor(), but for safety we use SCIPceil and
                // SCIPfloor for now; do this only if using boundtightening-inteval and not
                // in redundancy check (there we really want to relax all variables);
                // boundtightening-inteval does not relax integer variables, so can omit
                // expressions without children (constants should be ok, too).
                if scip.exprs[e].integrality != crate::types::ImplintType::None
                    && hd(scip, conshdlr).intevalvar == IntEvalVar::BoundTightening
                    && !scip.exprs[e].children.is_empty()
                {
                    if activity.inf > -INTERVAL_INFINITY {
                        activity.inf = scip.set.ceil(activity.inf);
                    }
                    if activity.sup < INTERVAL_INFINITY {
                        activity.sup = scip.set.floor(activity.sup);
                    }
                }

                // Mark the current node to be infeasible if either the lower/upper bound
                // is above/below +/- SCIPinfinity().
                // TODO this is a problem if dual-presolve fixed a variable to +/-
                // infinity.
                if scip.set.is_infinity(activity.inf) || scip.set.is_infinity(-activity.sup) {
                    activity.set_empty();
                }

                // Now finally store activity in expr.
                let curboundstag = hd(scip, conshdlr).curboundstag;
                scip.exprs[e].activity = activity;
                scip.exprs[e].activitytag = curboundstag;

                if interval::is_empty(INTERVAL_INFINITY, activity) {
                    if let Some(i) = infeasible.as_deref_mut() {
                        *i = true;
                    }
                } else if tightenauxvars && od(scip, e).auxvar != NONE {
                    let mut tighteninfeasible = false;

                    tighten_aux_var_bounds(scip, conshdlr, e, activity, &mut tighteninfeasible, ntightenings.as_deref_mut())?;
                    if tighteninfeasible {
                        if let Some(i) = infeasible.as_deref_mut() {
                            *i = true;
                        }
                        activity.set_empty();
                        let curboundstag = hd(scip, conshdlr).curboundstag;
                        scip.exprs[e].activity = activity;
                        scip.exprs[e].activitytag = curboundstag;
                    }
                }
            }
            _ => {
                // You should never be here.
                log::error!("unexpected iterator stage");
                panic!("SCIPABORT");
            }
        }

        e = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `isIntervalBetter`: returns whether intersecting `oldinterval` with
/// `newinterval` would provide a properly smaller interval.
///
/// If `subsetsufficient` is TRUE, then the intersection being smaller than
/// oldinterval is sufficient.
///
/// If `subsetsufficient` is FALSE, then we require
/// - a change from an unbounded interval to a bounded one, or
/// - or a change from an unfixed (width > epsilon) to a fixed interval, or
/// - a minimal tightening of one of the interval bounds as defined by
///   SCIPis{Lb,Ub}Better().
fn is_interval_better(scip: &Scip, subsetsufficient: bool, newinterval: Interval, oldinterval: Interval) -> bool {
    if subsetsufficient {
        // oldinterval \cap newinterval < oldinterval iff not oldinterval is subset
        // of newinterval.
        return !interval::is_subset_eq(INTERVAL_INFINITY, oldinterval, newinterval);
    }

    // Check whether lower bound of interval becomes finite.
    if oldinterval.inf <= -INTERVAL_INFINITY && newinterval.inf > -INTERVAL_INFINITY {
        return true;
    }

    // Check whether upper bound of interval becomes finite.
    if oldinterval.sup >= INTERVAL_INFINITY && newinterval.sup > INTERVAL_INFINITY {
        return true;
    }

    // Check whether intersection will have width <= epsilon, if oldinterval
    // doesn't have yet.
    if !scip.set.is_eq(oldinterval.inf, oldinterval.sup) && scip.set.is_eq(cmax(oldinterval.inf, newinterval.inf), cmin(oldinterval.sup, newinterval.sup)) {
        return true;
    }

    // Check whether lower bound on interval will be better by SCIP's quality
    // measures for boundchanges.
    if scip.set.is_lb_better(newinterval.inf, oldinterval.inf, oldinterval.sup) {
        return true;
    }

    // Check whether upper bound on interval will be better by SCIP's quality
    // measures for boundchanges.
    if scip.set.is_ub_better(newinterval.sup, oldinterval.inf, oldinterval.sup) {
        return true;
    }

    false
}

/// `reversePropQueue`: propagates bounds for each sub-expression in the
/// `reversepropqueue` by starting from the root expressions.
///
/// The expression will be traversed in breadth first search by using this
/// queue.
///
/// @note Calling this function requires feasible intervals for each
/// sub-expression; this is guaranteed by calling forwardPropExpr() before
/// calling this function.
///
/// @note Calling this function with `*infeasible` = TRUE will only empty
/// the queue.
fn reverse_prop_queue(scip: &mut Scip, conshdlr: ConshdlrId, infeasible: &mut bool, ntightenings: &mut i32) -> ScipResult {
    *ntightenings = 0;

    // Main loop that calls reverse propagation for expressions on the queue;
    // when reverseprop finds a tightening for an expression, then that
    // expression is added to the queue (within the reverseprop call).
    while !hd(scip, conshdlr).reversepropqueue.is_empty() && !*infeasible {
        let e = hdm(scip, conshdlr).reversepropqueue.pop_front().unwrap();

        // Mark that the expression is not in the queue anymore.
        odm(scip, e).inpropqueue = false;

        // Since the expr was in the propagation queue, the propbounds should
        // belong to current propagation and should not be empty.

        // (Intersecting propbounds with activity and auxvar bounds is disabled.)
        let propbounds = od(scip, e).propbounds;

        let nenfos = od(scip, e).nenfos;
        if nenfos > 0 {
            // For nodes with enforcement, call reverse propagation callbacks of
            // nlhdlrs.
            let mut k = 0;
            while k < nenfos as usize && !*infeasible {
                let enfo = od(scip, e).enfos[k];
                k += 1;

                // Skip nlhdlr if it does not want to participate in activity
                // computation.
                if enfo.nlhdlrparticipation & nlhdlr::NLHDLR_METHOD_ACTIVITY == 0 {
                    continue;
                }

                // Call the reverseprop of the nlhdlr.
                let mut nreds = 0;
                nlhdlr::nlhdlr_reverseprop(scip, conshdlr, enfo.nlhdlr, e, enfo.nlhdlrexprdata, propbounds, infeasible, &mut nreds)?;
                *ntightenings += nreds;
            }
        } else if scip.exprhdlrs[scip.exprs[e].exprhdlr].reverseprop.is_some() {
            // If expr without enforcement (before detect), call reverse propagation
            // callback of exprhdlr directly.

            // If someone added an expr without nlhdlr into the reversepropqueue, then
            // this must be because its enfo hasn't been initialized in detectNlhdlr
            // yet (nenfos < 0).
            let children = scip.exprs[e].children.clone();
            let mut childrenbounds: Vec<Interval> = children.iter().map(|&c| get_expr_bounds_nonlinear(scip, c)).collect();

            // Call the reverseprop of the exprhdlr.
            let eh = scip.exprs[e].exprhdlr;
            expr::exprhdlr_reverse_prop_expr(scip, eh, e, propbounds, &mut childrenbounds, infeasible)?;

            if !*infeasible {
                for (c, &child) in children.iter().enumerate() {
                    tighten_expr_interval_nonlinear(scip, child, childrenbounds[c], infeasible, Some(ntightenings))?;
                }
            }
        }
    }

    // Reset inpropqueue for all remaining expr's in queue (can happen in case of
    // early stop due to infeasibility).
    while let Some(e) = hdm(scip, conshdlr).reversepropqueue.pop_front() {
        // Mark that the expression is not in the queue anymore.
        odm(scip, e).inpropqueue = false;
    }

    Ok(())
}

/// `propConss`: calls domain propagation for a given set of constraints.
///
/// The algorithm alternates calls of forward and reverse propagation.
/// Forward propagation ensures that activity of expressions is up to date.
/// Reverse propagation tries to derive tighter variable bounds by reversing
/// the activity computation, using the constraints [lhs,rhs] interval as
/// starting point.
///
/// The propagation algorithm works as follows:
///  1. apply forward propagation (update activities) for all constraints not
///     marked as propagated
///  2. if presolve or propauxvars is disabled: collect expressions for which
///     the constraint sides provide tighter bounds; if solve and propauxvars
///     is enabled: collect expressions for which auxvars (including those in
///     root exprs) provide tighter bounds
///  3. apply reverse propagation to all collected expressions; don't explore
///     sub-expressions which have not changed since the beginning of the
///     propagation loop
///  4. if we have found enough tightenings go to 1, otherwise leave
///     propagation loop
///
/// @note After calling forward propagation for a constraint, we mark this
/// constraint as propagated. This flag might be reset during the reverse
/// propagation when we find a bound tightening of a variable expression
/// contained in the constraint. Resetting this flag is done in the EVENTEXEC
/// callback of the event handler.
///
/// TODO should we distinguish between expressions where activity information
/// is used for separation and those where not, e.g., try less to propagate on
/// convex constraints?
fn prop_conss(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], force: bool, result: &mut crate::types::ScipResultCode, nchgbds: &mut i32) -> ScipResult {
    use crate::types::ScipResultCode;

    let mut cutoff = false;
    let mut ntightenings;

    // No constraints to propagate.
    if conss.is_empty() {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;
    let mut roundnr = 0;

    // tightenAuxVarBounds() needs to know whether boundtightenings are to be
    // forced.
    hdm(scip, conshdlr).forceboundtightening = force;

    // Invalidate all propbounds (probably not needed).
    hdm(scip, conshdlr).curpropboundstag += 1;

    // Create iterator that we will use if we need to look at all auxvars.
    let propauxvars = hd(scip, conshdlr).propauxvars;
    let mut revpropcollectit = if propauxvars { Some(expr::ExprIter::create()) } else { None };

    // Main propagation loop.
    loop {
        // Apply forward propagation (update expression activities) and add
        // promising root expressions into queue for reversepropagation.
        for &cons in conss {
            // Skip deleted, non-active, or propagation-disabled constraints.
            let c = &scip.conss[cons];
            if c.deleted || !c.active || !c.propenabled {
                continue;
            }

            // Skip already propagated constraints, i.e., constraints where no
            // (original) variable has changed and thus activity didn't change.
            if cd(scip, cons).ispropagated {
                continue;
            }

            // Update activities in expression.
            ntightenings = 0;
            let consexpr = cd(scip, cons).expr;
            forward_prop_expr(scip, conshdlr, consexpr, true, Some(&mut cutoff), Some(&mut ntightenings))?;
            if cutoff {
                *result = ScipResultCode::Cutoff;
                break;
            }

            let auxvar = od(scip, consexpr).auxvar;

            // TODO for a constraint that only has an auxvar for consdata->expr (e.g.,
            // convex quadratic), we could also just do the if(TRUE)-branch.
            if !propauxvars || auxvar == NONE {
                // Check whether constraint sides (relaxed by epsilon) or auxvar bounds
                // provide a tightening (if we have auxvar (not in presolve), then bounds
                // of the auxvar are initially set to constraint sides, so taking auxvar
                // bounds is enough).
                let conssides = if auxvar == NONE {
                    // Relax sides by SCIPepsilon() and handle infinite sides.
                    let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
                    let relax = hd(scip, conshdlr).conssiderelaxamount;
                    let l = if scip.set.is_infinity(-lhs) { -INTERVAL_INFINITY } else { lhs - relax };
                    let r = if scip.set.is_infinity(rhs) { INTERVAL_INFINITY } else { rhs + relax };
                    let mut cs = Interval::default();
                    cs.set_bounds(l, r);
                    cs
                } else {
                    int_eval_var_bound_tightening(scip, auxvar, conshdlr)
                };
                tighten_expr_interval_nonlinear(scip, consexpr, conssides, &mut cutoff, Some(&mut ntightenings))?;
            } else {
                // Check whether bounds of any auxvar used in constraint provides a
                // tightening (for the root expression, bounds of auxvar are initially set
                // to constraint sides), but skip exprs that have an auxvar, but do not
                // participate in propagation.
                let it = revpropcollectit.as_mut().unwrap();
                it.init(scip, consexpr, expr::ExprIterType::Bfs, false)?;
                let mut e = it.get_current();
                while !it.is_end() && !cutoff {
                    let o = od(scip, e);
                    if o.auxvar == NONE || (o.nactivityusesprop == 0 && o.nactivityusessepa == 0) {
                        e = it.get_next(scip);
                        continue;
                    }

                    let conssides = int_eval_var_bound_tightening(scip, o.auxvar, conshdlr);
                    tighten_expr_interval_nonlinear(scip, e, conssides, &mut cutoff, Some(&mut ntightenings))?;
                    e = it.get_next(scip);
                }
            }

            if cutoff {
                *result = ScipResultCode::Cutoff;
                break;
            }

            if ntightenings > 0 {
                *nchgbds += ntightenings;
                *result = ScipResultCode::ReducedDom;
            }

            // Mark constraint as propagated; this will be reset via the event system
            // when we find a variable tightening.
            cdm(scip, cons).ispropagated = true;
        }

        // Apply backward propagation (if cutoff is TRUE, then this call empties
        // the queue).
        ntightenings = 0;
        reverse_prop_queue(scip, conshdlr, &mut cutoff, &mut ntightenings)?;
        if cutoff {
            *result = ScipResultCode::Cutoff;
            break;
        }

        if ntightenings > 0 {
            *nchgbds += ntightenings;
            *result = ScipResultCode::ReducedDom;
        }

        roundnr += 1;
        if !(ntightenings > 0 && roundnr < hd(scip, conshdlr).maxproprounds) {
            break;
        }
    }

    if let Some(it) = revpropcollectit {
        it.free(scip);
    }

    hdm(scip, conshdlr).forceboundtightening = false;

    // Invalidate propbounds in all exprs, so noone accidentally uses them
    // outside propagation.
    hdm(scip, conshdlr).curpropboundstag += 1;

    Ok(())
}

/* public expression functions (cons_nonlinear.h) used throughout */

/// `SCIPgetExprAuxVarNonlinear`: returns the variable used for linearizing
/// a given expression (return value might be NULL).
///
/// @note for variable expression it returns the corresponding variable
pub fn get_expr_aux_var_nonlinear(scip: &Scip, expr: ExprId) -> VarId {
    let ownerdata = od(scip, expr);

    if ownerdata.filterpos >= -1 {
        crate::expr_var::get_var_expr_var(scip, expr)
    } else {
        ownerdata.auxvar
    }
}

/// `SCIPincrementCurBoundsTagNonlinear`: increments `curboundstag` and
/// resets `lastboundrelax` in constraint handler data.
pub fn increment_cur_bounds_tag_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, boundrelax: bool) {
    let h = hdm(scip, conshdlr);
    h.curboundstag += 1;
    if boundrelax {
        h.lastboundrelax = h.curboundstag;
    }
}

/// `SCIPgetExprBoundsNonlinear`: returns bounds on the expression.
///
/// This gives an intersection of bounds from
/// - activity calculation (SCIPexprGetActivity()), if valid,
/// - auxiliary variable, if present,
/// - stored by SCIPtightenExprIntervalNonlinear() during domain propagation
///
/// @note The returned interval can be empty!
pub fn get_expr_bounds_nonlinear(scip: &mut Scip, expr: ExprId) -> Interval {
    let ownerdata = od(scip, expr).clone();
    let conshdlr = ownerdata.conshdlr;
    let (curpropboundstag, lastboundrelax, intevalvar) = {
        let h = hd(scip, conshdlr);
        (h.curpropboundstag, h.lastboundrelax, h.intevalvar)
    };

    let mut bounds = Interval::default();

    // Start with propbounds if they belong to current propagation.
    if ownerdata.propboundstag == curpropboundstag {
        bounds = ownerdata.propbounds;
    } else {
        bounds.set_entire(INTERVAL_INFINITY);
    }

    if scip.exprs[expr].activitytag >= lastboundrelax {
        // Apply propbounds to expr activity, but ensure it's not-empty if very close
        // disjoint intervals.
        let cur = bounds;
        interval::intersect_eps(&mut bounds, scip.set.epsilon(), scip.exprs[expr].activity, cur);
    }

    if ownerdata.auxvar != NONE {
        // Apply auxiliary variable bounds to bounds.
        let auxvarbounds = (intevalvar.func())(scip, ownerdata.auxvar, conshdlr);
        let cur = bounds;
        interval::intersect_eps(&mut bounds, scip.set.epsilon(), cur, auxvarbounds);
    }

    bounds
}

/// `SCIPtightenExprIntervalNonlinear`: informs the expression about new
/// bounds that can be used for reverse-propagation and to tighten bounds of
/// corresponding (auxiliary) variable (if any).
///
/// @attention this function should only be called during domain propagation
/// in cons_nonlinear
pub fn tighten_expr_interval_nonlinear(scip: &mut Scip, expr: ExprId, mut newbounds: Interval, cutoff: &mut bool, ntightenings: Option<&mut i32>) -> ScipResult {
    let conshdlr = od(scip, expr).conshdlr;

    // The code below assumes that current activity is valid; if it turns out
    // that we cannot ensure that, then we should change code.

    *cutoff = false;

    if scip.exprs[expr].integrality != crate::types::ImplintType::None {
        // Apply integrality to new bounds; it should be ok to use normal ceil()
        // and floor(), but for safety, we use SCIPceil and SCIPfloor for now.
        if newbounds.inf > -INTERVAL_INFINITY {
            newbounds.inf = scip.set.ceil(newbounds.inf);
        }
        if newbounds.sup < INTERVAL_INFINITY {
            newbounds.sup = scip.set.floor(newbounds.sup);
        }
    }

    if interval::is_empty(INTERVAL_INFINITY, newbounds) {
        *cutoff = true;
        return Ok(());
    }

    // Treat the new bounds as empty if either the lower/upper bound is
    // above/below +/- SCIPinfinity().
    if scip.set.is_infinity(newbounds.inf) || scip.set.is_infinity(-newbounds.sup) {
        *cutoff = true;
        return Ok(());
    }

    // Tighten newbounds w.r.t. existing expr->propbounds or activity.
    let curpropboundstag = hd(scip, conshdlr).curpropboundstag;
    let cur = newbounds;
    if od(scip, expr).propboundstag == curpropboundstag {
        // If already having propbounds in expr, then tighten newbounds by
        // propbounds.
        interval::intersect_eps(&mut newbounds, scip.set.epsilon(), od(scip, expr).propbounds, cur);
    } else {
        // First time we have propbounds for expr in this propagation rounds:
        // intersect with activity (though don't let it become empty if very close
        // intervals).
        interval::intersect_eps(&mut newbounds, scip.set.epsilon(), scip.exprs[expr].activity, cur);
    }

    // Check if the new bounds lead to an empty interval.
    if interval::is_empty(INTERVAL_INFINITY, newbounds) {
        *cutoff = true;
        return Ok(());
    }

    // If expr is not constant or variable, then store newbounds in
    // expr->propbounds
    // - for constant, the intersection with activity should have been
    //   sufficient to determine infeasibilty
    // - for variable, the tightenAuxVarBounds call below should be suffient to
    //   have to new bounds acknowledged
    let haschildren = !scip.exprs[expr].children.is_empty();
    if haschildren {
        let o = odm(scip, expr);
        o.propbounds = newbounds;
        o.propboundstag = curpropboundstag;
    }

    // If updated propbounds do not allow a sufficient tightening, then do not
    // consider adding to queue for reverse propagation or update of auxvar
    // bounds.
    let force = hd(scip, conshdlr).forceboundtightening;
    if !is_interval_better(scip, force, newbounds, scip.exprs[expr].activity) {
        return Ok(());
    }

    let o = od(scip, expr);
    if haschildren && !o.inpropqueue && (o.nactivityusesprop > 0 || o.nactivityusessepa > 0 || o.nenfos < 0) {
        // Add expression to propagation queue if not there yet and not var or
        // constant and if it should have a nlhdlr with a reverseprop callback or
        // nlhdlrs are not initialized yet (nenfos < 0).
        hdm(scip, conshdlr).reversepropqueue.push_back(expr);
        odm(scip, expr).inpropqueue = true;
    }

    // Update bounds on variable or auxiliary variable.
    tighten_aux_var_bounds(scip, conshdlr, expr, newbounds, cutoff, ntightenings)
}

/// `SCIPmarkExprPropagateNonlinear`: mark constraints that include this
/// expression to be propagated again.
///
/// This can be used by, e.g., nlhdlrs, to trigger a new propagation of
/// constraints without a change of variable bounds, e.g., because new
/// information on the expression is available that could potentially lead to
/// tighter expression activity values.
///
/// Note, that this call marks also constraints for propagation which only
/// share some variable with this expression.
pub fn mark_expr_propagate_nonlinear(scip: &mut Scip, expr: ExprId) -> ScipResult {
    let conshdlr = od(scip, expr).conshdlr;
    increment_cur_bounds_tag_nonlinear(scip, conshdlr, false);

    let mut it = expr::ExprIter::create();
    it.init(scip, expr, expr::ExprIterType::Dfs, false)?;

    let mut e = expr;
    while !it.is_end() {
        if expr::expr_is_var(scip, e) {
            for c in od(scip, e).conss.clone() {
                cdm(scip, c).ispropagated = false;
            }
        }
        e = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `SCIPregisterExprUsageNonlinear`: method to be called by a nlhdlr during
/// NLHDLRDETECT to notify an expression that it will be used.
///
/// - if `useauxvar` is enabled, then ensures that an auxiliary variable will
///   be created in INITLP
/// - if `useactivityforprop` or `useactivityforsepa{below,above}` is enabled,
///   then ensured that activity will be updated for `expr`
/// - if `useactivityforprop` is enabled, then increments the count returned
///   by SCIPgetExprNPropUsesActivityNonlinear()
/// - if `useactivityforsepa{below,above}` is enabled, then increments the
///   count returned by SCIPgetExprNSepaUsesActivityNonlinear() and also
///   increments this count for all variables in the expression.
///
/// The distinction into `useactivityforprop` and
/// `useactivityforsepa{below,above}` is to recognize variables which domain
/// influences under/overestimators. Domain propagation routines (like OBBT)
/// may invest more work for these variables. The distinction into
/// `useactivityforsepabelow` and `useactivityforsepaabove` is to recognize
/// whether a nlhdlr that called this method will use activity of `expr` in
/// enfomethod SCIP_NLHDLR_METHOD_SEPABELOW or SCIP_NLHDLR_METHOD_SEPAABOVE.
pub fn register_expr_usage_nonlinear(
    scip: &mut Scip,
    expr: ExprId,
    mut useauxvar: bool,
    useactivityforprop: bool,
    useactivityforsepabelow: bool,
    useactivityforsepaabove: bool,
) -> ScipResult {
    // Do not store auxvar request for variable expressions.
    if useauxvar && expr::expr_is_var(scip, expr) {
        useauxvar = false;
    }

    let o = od(scip, expr);
    if o.nenfos >= 0
        && ((o.nactivityusesprop == 0 && o.nactivityusessepa == 0 && (useactivityforprop || useactivityforsepabelow || useactivityforsepaabove))
            || (o.nauxvaruses == 0 && useauxvar))
    {
        // If we already have ran detect of nlhdlrs on expr (nenfos >= 0), then we
        // need to rerun detection if we require additional enforcement methods,
        // that is,
        // - activity of expr was not used before but will be used now, or
        // - auxiliary variable of expr was not required before but will be used
        //   now
        free_enfo_data(scip, expr, false)?;
    }

    let o = odm(scip, expr);
    if useauxvar {
        o.nauxvaruses += 1;
    }

    if useactivityforprop {
        o.nactivityusesprop += 1;
    }

    if useactivityforsepabelow || useactivityforsepaabove {
        o.nactivityusessepa += 1;
    }

    // Remember that SCIPregisterExprUsageNonlinear() has been called with
    // useactivityforsepa{below,above}=TRUE; this information is used in
    // detectNlhdlr().
    let conshdlr = o.conshdlr;
    if useactivityforsepabelow {
        hdm(scip, conshdlr).registerusesactivitysepabelow = true;
    }
    if useactivityforsepaabove {
        hdm(scip, conshdlr).registerusesactivitysepaabove = true;
    }

    if useactivityforprop {
        // If activity will be used for propagation, then make sure there is a
        // valid activity; this way, we can do a reversepropcall after
        // detectNlhdlr.
        expr::expr_eval_activity(scip, expr)?;
    }

    // Increase the nactivityusedsepa counter for all variables used in the
    // given expression.
    if (useactivityforsepabelow || useactivityforsepaabove) && !scip.exprs[expr].children.is_empty() {
        // Create and initialize iterator.
        let mut it = expr::ExprIter::create();
        it.init(scip, expr, expr::ExprIterType::Dfs, false)?;

        let mut e = expr;
        while !it.is_end() {
            if expr::expr_is_var(scip, e) {
                odm(scip, e).nactivityusessepa += 1;
            }
            e = it.get_next(scip);
        }

        // Free iterator.
        it.free(scip);
    }

    Ok(())
}

/// `propExprDomains`: calls the reverseprop callbacks of all nlhdlrs in all
/// expressions in all constraints using activity as bounds.
///
/// This is meant to propagate any domain restrictions on functions onto
/// variable bounds, if possible.
///
/// Assumes that activities are still valid and curpropboundstag does not
/// need to be increased. Therefore, a good place to call this function is
/// immediately after propConss() or after forwardPropExpr() if outside
/// propagation.
fn prop_expr_domains(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], result: &mut crate::types::ScipResultCode, nchgbds: &mut i32) -> ScipResult {
    use crate::types::ScipResultCode;

    let mut cutoff = false;
    let mut ntightenings = 0;

    *result = ScipResultCode::DidNotFind;

    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;

    let mut c = 0;
    while c < conss.len() && !cutoff {
        let cons = conss[c];
        c += 1;

        // Skip deleted, non-active, or propagation-disabled constraints.
        let co = &scip.conss[cons];
        if co.deleted || !co.active || !co.propenabled {
            continue;
        }

        let consexpr = cd(scip, cons).expr;
        let mut e = it.restart_dfs(scip, consexpr);
        while !it.is_end() && !cutoff {
            // Call reverseprop for those nlhdlr that participate in this expr's
            // activity computation; this will propagate the current activity.
            for k in 0..od(scip, e).nenfos.max(0) as usize {
                let enfo = od(scip, e).enfos[k];
                if enfo.nlhdlrparticipation & nlhdlr::NLHDLR_METHOD_ACTIVITY == 0 {
                    continue;
                }

                ntightenings = 0;
                let activity = scip.exprs[e].activity;
                nlhdlr::nlhdlr_reverseprop(scip, conshdlr, enfo.nlhdlr, e, enfo.nlhdlrexprdata, activity, &mut cutoff, &mut ntightenings)?;

                if cutoff {
                    // Stop everything if we detected infeasibility.
                    *result = ScipResultCode::Cutoff;
                    break;
                }

                if ntightenings > 0 {
                    *nchgbds += ntightenings;
                    *result = ScipResultCode::ReducedDom;
                }
            }
            e = it.get_next(scip);
        }
    }

    // Apply backward propagation (if cutoff is TRUE, then this call empties
    // the queue).
    reverse_prop_queue(scip, conshdlr, &mut cutoff, &mut ntightenings)?;
    if cutoff {
        *result = ScipResultCode::Cutoff;
    } else if ntightenings > 0 {
        *nchgbds += ntightenings;
        *result = ScipResultCode::ReducedDom;
    }

    it.free(scip);

    // Invalidate propbounds in all exprs, so noone accidentally uses them
    // outside propagation.
    hdm(scip, conshdlr).curpropboundstag += 1;

    Ok(())
}

/// `propagateLocks`: propagates variable locks through expression and adds
/// locks to variables.
fn propagate_locks(scip: &mut Scip, expr: ExprId, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    // If no locks, then nothing to propagate.
    if nlockspos == 0 && nlocksneg == 0 {
        return Ok(());
    }

    let mut it = expr::ExprIter::create();
    it.init(scip, expr, expr::ExprIterType::Dfs, true)?;
    it.set_stages_dfs(scip, expr::EXPRITER_ENTEREXPR | expr::EXPRITER_VISITINGCHILD | expr::EXPRITER_LEAVEEXPR);

    // Store locks in root node.
    it.set_current_user_data(scip, expr::ExprIterUserData::from_intvals([nlockspos, nlocksneg]));

    let mut e = expr;
    while !it.is_end() {
        // Collect locks.
        let ituserdata = it.get_current_user_data(scip);
        let [nlockspos, nlocksneg] = ituserdata.intvals();

        match it.get_stage_dfs() {
            expr::EXPRITER_ENTEREXPR => {
                if expr::expr_is_var(scip, e) {
                    // If a variable, then also add nlocksneg/nlockspos via
                    // SCIPaddVarLocks().
                    let var = crate::expr_var::get_var_expr_var(scip, e);
                    crate::api::add_var_locks_type(scip, var, LockType::Model, nlocksneg, nlockspos)?;
                }

                // Add locks to expression.
                let o = odm(scip, e);
                o.nlockspos += nlockspos;
                o.nlocksneg += nlocksneg;

                // Add monotonicity information if expression has been locked for the
                // first time.
                let first = o.nlockspos == nlockspos && o.nlocksneg == nlocksneg;
                let nchildren = scip.exprs[e].children.len();
                let h = scip.exprs[e].exprhdlr;
                if first && nchildren > 0 && scip.exprhdlrs[h].monotonicity.is_some() {
                    // Store the monotonicity for each child.
                    let mut monotonicity = vec![Monotone::Unknown; nchildren];
                    for (i, m) in monotonicity.iter_mut().enumerate() {
                        expr::exprhdlr_monotonicity_expr(scip, h, e, i as i32, m)?;
                    }
                    odm(scip, e).monotonicity = Some(monotonicity);
                }
            }
            expr::EXPRITER_LEAVEEXPR => {
                // Remove monotonicity information if expression has been unlocked.
                let o = odm(scip, e);
                if o.nlockspos == 0 && o.nlocksneg == 0 && o.monotonicity.is_some() {
                    // Keep this assert for checking whether someone changed an
                    // expression without updating locks properly.
                    o.monotonicity = None;
                }
            }
            expr::EXPRITER_VISITINGCHILD => {
                // Get monotonicity of child.
                // NOTE: the monotonicity stored in an expression might be different
                // from the result obtained by SCIPcallExprMonotonicity.
                let idx = it.get_child_idx_dfs(scip) as usize;
                let monotonicity = od(scip, e).monotonicity.as_ref().map_or(Monotone::Unknown, |m| m[idx]);

                // Compute resulting locks of the child expression.
                let childlocks = match monotonicity {
                    Monotone::Inc => [nlockspos, nlocksneg],
                    Monotone::Dec => [nlocksneg, nlockspos],
                    Monotone::Unknown => [nlockspos + nlocksneg, nlockspos + nlocksneg],
                    Monotone::Const => [0, 0],
                };

                // Set locks in child expression.
                it.set_child_user_data(scip, expr::ExprIterUserData::from_intvals(childlocks));
            }
            _ => {
                // You should never be here.
                panic!("SCIPABORT");
            }
        }

        e = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `addLocks`: main function for adding locks to expressions and variables.
///
/// Locks for a nonlinear constraint are used to update locks for all
/// sub-expressions and variables. Locks of expressions depend on the
/// monotonicity of expressions w.r.t. their children, e.g., consider the
/// constraint x^2 <= 1 with x in [-2,-1] implies an up-lock for the root
/// expression (pow) and a down-lock for its child x because x^2 is
/// decreasing on [-2,-1]. Since the monotonicity (and thus the locks) might
/// also depend on variable bounds, the function remembers the computed
/// monotonicity information of each expression until all locks of an
/// expression have been removed, which implies that updating the
/// monotonicity information during the next locking of this expression does
/// not break existing locks.
///
/// @note When modifying the structure of an expression, e.g., during
/// simplification, it is necessary to remove all locks from an expression and
/// repropagating them after the structural changes have been applied. Because
/// of existing common sub-expressions, it might be necessary to remove the
/// locks of all constraints to ensure that an expression is unlocked (see
/// canonicalizeConstraints() for an example).
fn add_locks(scip: &mut Scip, cons: ConsId, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    if nlockspos == 0 && nlocksneg == 0 {
        return Ok(());
    }

    let (lhs, rhs, expr) = {
        let c = cd(scip, cons);
        (c.lhs, c.rhs, c.expr)
    };

    // No constraint sides -> nothing to lock.
    if scip.set.is_infinity(rhs) && scip.set.is_infinity(-lhs) {
        return Ok(());
    }

    // Remember locks.
    let consdata = cdm(scip, cons);
    consdata.nlockspos += nlockspos;
    consdata.nlocksneg += nlocksneg;

    // Compute locks for lock propagation.
    if !scip.set.is_infinity(rhs) && !scip.set.is_infinity(-lhs) {
        propagate_locks(scip, expr, nlockspos + nlocksneg, nlockspos + nlocksneg)?;
    } else if !scip.set.is_infinity(rhs) {
        propagate_locks(scip, expr, nlockspos, nlocksneg)?;
    } else {
        propagate_locks(scip, expr, nlocksneg, nlockspos)?;
    }

    Ok(())
}

/// `createNlRow`: create a nonlinear row representation of a nonlinear
/// constraint and stores them in consdata.
fn create_nl_row(scip: &mut Scip, cons: ConsId) -> ScipResult {
    if cd(scip, cons).nlrow != NONE {
        let mut nlrow = cd(scip, cons).nlrow;
        crate::nlp::release_nl_row(scip, &mut nlrow)?;
        cdm(scip, cons).nlrow = NONE;
    }

    // Better curvature info will be set in initSolve() just before nlrow is
    // added to NLP.
    let (lhs, rhs, expr) = {
        let c = cd(scip, cons);
        (c.lhs, c.rhs, c.expr)
    };
    let name = scip.conss[cons].name.clone();
    let nlrow = crate::nlp::create_nl_row(scip, &name, 0.0, &[], &[], NONE, lhs, rhs, ExprCurv::Unknown)?;
    cdm(scip, cons).nlrow = nlrow;

    if expr::expr_is_sum(scip, expr) {
        // If root is a sum, then split into linear and nonlinear terms.
        let coefs = crate::expr_sum::get_coefs_expr_sum(scip, expr).to_vec();

        // Constant term of sum.
        let constant = crate::expr_sum::get_constant_expr_sum(scip, expr);
        crate::nlp::chg_nl_row_constant(scip, nlrow, constant)?;

        // A sum-expression that will hold the nonlinear terms and be passed to the
        // nlrow eventually.
        let conshdlr = cons_hdlr(scip, cons);
        let mut nonlinpart = crate::expr_sum::create_expr_sum(scip, &[], None, 0.0, Some(exprowner_create), conshdlr)?;

        let children = scip.exprs[expr].children.clone();
        for (i, &child) in children.iter().enumerate() {
            if expr::expr_is_var(scip, child) {
                // Linear term.
                let var = crate::expr_var::get_var_expr_var(scip, child);
                crate::nlp::add_linear_coef_to_nl_row(scip, nlrow, var, coefs[i])?;
            } else {
                // Nonlinear term.
                crate::expr_sum::append_expr_sum_expr(scip, nonlinpart, child, coefs[i]);
            }
        }

        if !scip.exprs[nonlinpart].children.is_empty() {
            // Add expression to nlrow (this will make a copy).
            crate::nlp::set_nl_row_expr(scip, nlrow, nonlinpart)?;
        }
        expr::expr_release(scip, &mut nonlinpart)?;
    } else {
        crate::nlp::set_nl_row_expr(scip, nlrow, expr)?;
    }

    Ok(())
}

/// `enfodataCmp`: compares enfodata by enforcement priority of nonlinear
/// handler.
///
/// If handlers have same enforcement priority, then compare by detection
/// priority, then by name.
fn enfodata_cmp(scip: &Scip, elem1: &ExprEnfo, elem2: &ExprEnfo) -> i32 {
    let h1 = &scip.nlhdlrs[elem1.nlhdlr];
    let h2 = &scip.nlhdlrs[elem2.nlhdlr];

    if h1.enfopriority != h2.enfopriority {
        return h1.enfopriority - h2.enfopriority;
    }

    if h1.detectpriority != h2.detectpriority {
        return h1.detectpriority - h2.detectpriority;
    }

    h1.name.as_bytes().cmp(h2.name.as_bytes()) as i32
}

/// `detectNlhdlr`: install nlhdlrs in one expression; `cons` is the
/// constraint for which expr == consdata->expr, otherwise NULL.
fn detect_nlhdlr(scip: &mut Scip, expr: ExprId, cons: ConsId) -> ScipResult {
    let conshdlr = od(scip, expr).conshdlr;

    // There should be no enforcer yet and detection should not even have
    // considered expr yet.

    // Check which enforcement methods are required by setting flags in
    // enforcemethods for those that are NOT required
    // - if no auxiliary variable is used, then do not need sepabelow or
    //   sepaabove
    // - if auxiliary variable is used, but nobody positively (up) locks expr ->
    //   only need to enforce expr >= auxvar -> no need for underestimation
    // - if auxiliary variable is used, but nobody negatively (down) locks expr
    //   -> only need to enforce expr <= auxvar -> no need for overestimation
    // - if no one uses activity, then do not need activity methods
    let mut enforcemethods = nlhdlr::NLHDLR_METHOD_NONE;
    {
        let o = od(scip, expr);
        if o.nauxvaruses == 0 {
            enforcemethods |= nlhdlr::NLHDLR_METHOD_SEPABOTH;
        } else {
            if o.nlockspos == 0 {
                // No need for underestimation.
                enforcemethods |= nlhdlr::NLHDLR_METHOD_SEPABELOW;
            }
            if o.nlocksneg == 0 {
                // No need for overestimation.
                enforcemethods |= nlhdlr::NLHDLR_METHOD_SEPAABOVE;
            }
        }
        if o.nactivityusesprop == 0 && o.nactivityusessepa == 0 {
            enforcemethods |= nlhdlr::NLHDLR_METHOD_ACTIVITY;
        }
    }

    // It doesn't make sense to have been called on detectNlhdlr, if the expr
    // isn't used for anything.

    // All methods that have not been flagged above are the ones that we want to
    // be handled by nlhdlrs.
    let enforcemethodsallowed = !enforcemethods & nlhdlr::NLHDLR_METHOD_ALL;

    {
        let o = odm(scip, expr);
        o.nenfos = 0;
        o.enfos = Vec::with_capacity(2);
    }
    hdm(scip, conshdlr).indetect = true;

    let nlhdlrs = hd(scip, conshdlr).nlhdlrs.clone();
    for nh in nlhdlrs {
        // Skip disabled nlhdlrs.
        if !scip.nlhdlrs[nh].enabled {
            continue;
        }

        // Call detect routine of nlhdlr.
        let mut nlhdlrexprdata = NONE;
        let mut enforcemethodsnew = enforcemethods;
        let mut nlhdlrparticipating = nlhdlr::NLHDLR_METHOD_NONE;
        {
            let h = hdm(scip, conshdlr);
            // SCIPregisterExprUsageNonlinear() as called by detect may set these to
            // TRUE.
            h.registerusesactivitysepabelow = false;
            h.registerusesactivitysepaabove = false;
        }
        nlhdlr::nlhdlr_detect(scip, conshdlr, nh, expr, cons, &mut enforcemethodsnew, &mut nlhdlrparticipating, &mut nlhdlrexprdata)?;

        // Nlhdlr might have claimed more than needed: clean up sepa flags.
        nlhdlrparticipating &= enforcemethodsallowed;

        // Detection is only allowed to augment to nlhdlrenforcemethods, so
        // previous enforcemethods must still be set.

        // Because of the previous assert, nlhdlrenforcenew ^ enforcemethods are
        // the methods enforced by this nlhdlr. They are also cleaned up here to
        // ensure that only the needed methods are claimed.
        let _nlhdlrenforcemethods = (enforcemethodsnew ^ enforcemethods) & enforcemethodsallowed;

        // Nlhdlr needs to participate for the methods it is enforcing.
        if nlhdlrparticipating == nlhdlr::NLHDLR_METHOD_NONE {
            // Nlhdlr might not have detected anything, or all set flags might have
            // been removed by clean up; in the latter case, we may need to free
            // nlhdlrexprdata.

            // Free nlhdlr exprdata, if there is any and there is a method to free
            // this data.
            if nlhdlrexprdata != NONE {
                nlhdlr::nlhdlr_freeexprdata(scip, nh, expr, &mut nlhdlrexprdata)?;
            }
            // Nlhdlr cannot have added an enforcement method if it doesn't
            // participate (actually redundant due to previous asserts).

            continue;
        }

        // Store nlhdlr and its data.
        let (sepabelow, sepaabove) = {
            let h = hd(scip, conshdlr);
            (h.registerusesactivitysepabelow, h.registerusesactivitysepaabove)
        };
        let o = odm(scip, expr);
        o.enfos.push(ExprEnfo {
            nlhdlr: nh,
            nlhdlrexprdata,
            nlhdlrparticipation: nlhdlrparticipating,
            issepainit: false,
            auxvalue: 0.0,
            sepabelowusesactivity: sepabelow,
            sepaaboveusesactivity: sepaabove,
        });
        o.nenfos += 1;

        // Update enforcement flags.
        enforcemethods = enforcemethodsnew;
    }

    hdm(scip, conshdlr).indetect = false;

    // Stop if an enforcement method is missing but we are already in solving
    // stage (as long as the expression provides its callbacks, the default
    // nlhdlr should have provided all enforcement methods).
    if enforcemethods != nlhdlr::NLHDLR_METHOD_ALL && scip.set.stage == Stage::Solving {
        log::error!("no nonlinear handler provided some of the required enforcement methods");
        return Err(Retcode::Error);
    }

    // Sort nonlinear handlers by enforcement priority, in decreasing order.
    if od(scip, expr).nenfos > 1 {
        let mut enfos = std::mem::take(&mut odm(scip, expr).enfos);
        crate::sort::sort_down(&mut enfos, (), |a, b| enfodata_cmp(scip, a, b));
        odm(scip, expr).enfos = enfos;
    }

    Ok(())
}

/// `detectNlhdlrs`: detect nlhdlrs that can handle the expressions.
fn detect_nlhdlrs(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, true)?;

    let localsolve = scip.set.stage == Stage::Solving && crate::api::get_depth(scip) != 0;
    if localsolve {
        // Ensure that activities are recomputed w.r.t. the global variable bounds
        // if CONSACTIVE is called in a local node; for example, this happens if
        // globally valid nonlinear constraints are added during the tree search.
        increment_cur_bounds_tag_nonlinear(scip, conshdlr, true);
        let h = hdm(scip, conshdlr);
        h.globalbounds = true;
        h.lastvaractivitymethodchange = h.curboundstag;
    }

    for &cons in conss {
        // If a constraint is separated, we currently need it to be initial, too;
        // this is because INITLP will create the auxiliary variables that are used
        // for any separation.
        // TODO we may relax this with a little more programming effort when
        // required, see also TODO in INITLP.

        let consexpr = cd(scip, cons).expr;

        // Because of common sub-expressions it might happen that we already
        // detected a nonlinear handler and added it to the expr; then we would
        // normally skip to run DETECT again; HOWEVER: most likely we have been
        // running DETECT with cons == NULL, which may interest less nlhdlrs; thus,
        // if expr is the root expression, we rerun DETECT.
        if od(scip, consexpr).nenfos > 0 {
            free_enfo_data(scip, consexpr, false)?;
        }

        // If constraint will be enforced, and we are in solve, then ensure
        // auxiliary variable for root expression; this way we can treat the root
        // expression like any other expression when enforcing via separation; if
        // constraint will be propagated, then register activity usage of root
        // expression; this can trigger a call to forwardPropExpr, for which we
        // better have the indetect flag set.
        hdm(scip, conshdlr).indetect = true;
        let (separated, enforced, propagated) = {
            let c = &scip.conss[cons];
            (c.separate, c.enforce, c.propagate)
        };
        let useauxvar = scip.set.stage >= Stage::InitSolve && (separated || enforced);
        register_expr_usage_nonlinear(scip, consexpr, useauxvar, propagated, false, false)?;
        hdm(scip, conshdlr).indetect = false;

        // presolveSingleLockedVars() may need the activity of product expressions
        // in a sum expr that is in the root expr of a nonlinear constraint with
        // only one finite side; if this presolver may be run in the current
        // presolve round (presoltiming=exhaustive could be added as additional
        // criterion), then we ensure that a routine will be present to compute this
        // activity (SCIPregisterExprUsageNonlinear actually updates activity
        // already).
        let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
        let h = hd(scip, conshdlr);
        if scip.set.stage == Stage::Presolving
            && !h.checkedvarlocks
            && h.checkvarlocks != b'd'
            && (scip.set.is_infinity(-lhs) || scip.set.is_infinity(rhs))
            && expr::expr_is_sum(scip, consexpr)
        {
            for c in 0..scip.exprs[consexpr].children.len() {
                let e = scip.exprs[consexpr].children[c];

                if expr::expr_is_product(scip, e) {
                    register_expr_usage_nonlinear(scip, e, false, true, false, false)?;
                }
            }
        }

        // Compute integrality information for all subexpressions.
        crate::scip_expr::compute_expr_integrality(scip, consexpr)?;

        // Run detectNlhdlr on all expr where required.
        let mut e = it.restart_dfs(scip, consexpr);
        while !it.is_end() {
            let o = od(scip, e);

            // Skip exprs that we already looked at.
            if o.nenfos >= 0 {
                e = it.get_next(scip);
                continue;
            }

            // If there is use of the auxvar, then someone requires that auxvar ==
            // expr (or auxvar >= expr or auxvar <= expr) or we are at the root
            // expression (expr==consdata->expr); thus, we need to find nlhdlrs that
            // separate or estimate; if there is use of the activity, then there is
            // someone requiring that activity of this expression is updated; this
            // someone would also benefit from better bounds on the activity of this
            // expression; thus, we need to find nlhdlrs that do interval-evaluation.
            if o.nauxvaruses > 0 || o.nactivityusesprop > 0 || o.nactivityusessepa > 0 {
                detect_nlhdlr(scip, e, if e == consexpr { cons } else { NONE })?;
            } else {
                // Remember that we looked at this expression during detectNlhdlrs even
                // though we have not actually run detectNlhdlr, because no nlhdlr
                // showed interest in this expr, in some situations (forwardPropExpr, to
                // be specific) we will have to distinguish between exprs for which we
                // have not initialized enforcement yet (nenfos < 0) and expressions
                // which are just not used in enforcement (nenfos == 0).
                odm(scip, e).nenfos = 0;
            }

            e = it.get_next(scip);
        }

        // Include this constraint into the next propagation round because the
        // added nlhdlr may do find tighter bounds now.
        if propagated {
            cdm(scip, cons).ispropagated = false;
        }
    }

    if scip.set.stage == Stage::Solving && crate::api::get_depth(scip) != 0 {
        // Ensure that the local bounds are used again when reevaluating the
        // expressions later; this is only needed if CONSACTIVE is called in a local
        // node (see begin of this function).
        increment_cur_bounds_tag_nonlinear(scip, conshdlr, false);
        let h = hdm(scip, conshdlr);
        h.globalbounds = false;
        h.lastvaractivitymethodchange = h.curboundstag;
    } else {
        // Ensure that all activities (except for var-exprs) are reevaluated since
        // better methods may be available now.
        increment_cur_bounds_tag_nonlinear(scip, conshdlr, false);
    }

    it.free(scip);

    Ok(())
}

/// `initSolve`: initializes (pre)solving data of constraints.
///
/// This initializes data in a constraint that is used for separation,
/// propagation, etc, and assumes that expressions will not be modified. In
/// particular, this function
/// - runs the detection method of nlhldrs
/// - looks for unlocked linear variables
/// - checks curvature (if not in presolve)
/// - creates and add row to NLP (if not in presolve)
///
/// This function can be called in presolve and solve and can be called
/// several times with different sets of constraints, e.g., it should be
/// called in INITSOL and for constraints that are added during solve.
fn init_solve(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    for &cons in conss {
        // Check for a linear variable that can be increase or decreased without
        // harming feasibility.
        find_unlocked_linear_var(scip, cons);

        if scip.set.stage == Stage::InitSolve || scip.set.stage == Stage::Solving {
            let mut success = false;
            let (lhs, rhs, consexpr) = {
                let c = cd(scip, cons);
                (c.lhs, c.rhs, c.expr)
            };

            if !hd(scip, conshdlr).assumeconvex {
                // Call the curvature detection algorithm of the convex nonlinear handler.
                // Check only for those curvature that may result in a convex inequality,
                // i.e., whether f(x) is concave when f(x) >= lhs and/or f(x) is convex
                // when f(x) <= rhs. Also we can assume that we are nonlinear, so do not
                // check for convex if already concave.
                if !scip.set.is_infinity(-lhs) {
                    success = crate::nlhdlr_convex::has_expr_curvature(scip, consexpr, ExprCurv::Concave, None)?;
                    if success {
                        cdm(scip, cons).curv = ExprCurv::Concave;
                    }
                }
                if !success && !scip.set.is_infinity(rhs) {
                    success = crate::nlhdlr_convex::has_expr_curvature(scip, consexpr, ExprCurv::Convex, None)?;
                    if success {
                        cdm(scip, cons).curv = ExprCurv::Convex;
                    }
                }
            } else if !scip.set.is_infinity(-lhs) && !scip.set.is_infinity(rhs) {
                log::warn!(
                    "Nonlinear constraint <{}> has finite left- and right-hand side, but constraints/nonlinear/assumeconvex is enabled.",
                    scip.conss[cons].name
                );
                cdm(scip, cons).curv = ExprCurv::Linear;
            } else {
                cdm(scip, cons).curv = if !scip.set.is_infinity(rhs) { ExprCurv::Convex } else { ExprCurv::Concave };
            }

            // Add nlrow representation to NLP, if NLP had been constructed.
            if crate::nlp::is_nlp_constructed(scip) && scip.conss[cons].active {
                if cd(scip, cons).nlrow == NONE {
                    create_nl_row(scip, cons)?;
                }
                let (nlrow, curv) = (cd(scip, cons).nlrow, cd(scip, cons).curv);
                crate::nlp::set_nl_row_curvature(scip, nlrow, curv);
                crate::nlp::add_nl_row(scip, nlrow)?;
            }
        }
    }

    // Register non linear handlers.
    detect_nlhdlrs(scip, conshdlr, conss)
}

/// `deinitSolve`: deinitializes (pre)solving data of constraints.
///
/// This removes the initialization data created in initSolve().
///
/// This function can be called in presolve and solve.
///
/// TODO At the moment, it should not be called for a constraint if there are
/// other constraints that use the same expressions but still require their
/// nlhdlr. We should probably only decrement the auxvar and activity usage
/// for the root expr and then proceed as in detectNlhdlrs(), i.e., free enfo
/// data only where none is used.
fn deinit_solve(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_LEAVEEXPR);

    // Call deinitialization callbacks of expression and nonlinear handlers;
    // free nonlinear handlers information from expressions; remove auxiliary
    // variables and nactivityuses counts from expressions.
    for &cons in conss {
        let consexpr = cd(scip, cons).expr;

        // Check and remember whether activity in root is valid.
        let rootactivityvalid = scip.exprs[consexpr].activitytag >= hd(scip, conshdlr).lastboundrelax;

        let mut e = it.restart_dfs(scip, consexpr);
        while !it.is_end() {
            // Remove nonlinear handlers in expression and their data and auxiliary
            // variables; reset activityusage count.
            free_enfo_data(scip, e, true)?;

            // Remove quadratic info.
            expr::expr_free_quadratic(scip, e);

            if rootactivityvalid {
                // Ensure activity is valid if consdata->expr activity is valid; this is
                // mainly to ensure that we do not leave invalid activities in parts of
                // the expression tree where activity was not used, e.g., an expr's
                // activity was kept up to date by a nlhdlr, but without using some childs
                // activity, so this childs activity would be invalid, which can generate
                // confusion.
                expr::expr_eval_activity(scip, e)?;
            }

            e = it.get_next(scip);
        }

        if cd(scip, cons).nlrow != NONE {
            // Remove row from NLP, if still in solving; if we are in exitsolve, the
            // whole NLP will be freed anyway.
            let mut nlrow = cd(scip, cons).nlrow;
            if scip.set.stage == Stage::Solving {
                crate::nlp::del_nl_row(scip, nlrow)?;
            }

            crate::nlp::release_nl_row(scip, &mut nlrow)?;
            cdm(scip, cons).nlrow = NONE;
        }

        // Forget about linear variables that can be increased or decreased without
        // harming feasibility.
        let c = cdm(scip, cons);
        c.linvardecr = NONE;
        c.linvarincr = NONE;

        // Forget about curvature.
        c.curv = ExprCurv::Unknown;
    }

    it.free(scip);

    Ok(())
}

/// `isBinaryProduct`: helper method to decide whether a given expression is
/// product of at least two binary variables.
fn is_binary_product(scip: &Scip, expr: ExprId) -> bool {
    // Check whether the expression is a product.
    if !expr::expr_is_product(scip, expr) {
        return false;
    }

    // Don't consider products with a coefficient != 1 and products with a
    // single child; simplification will take care of this expression later.
    if scip.exprs[expr].children.len() <= 1 || crate::expr_product::get_coef_expr_product(scip, expr) != 1.0 {
        return false;
    }

    for &child in &scip.exprs[expr].children {
        if !expr::expr_is_var(scip, child) {
            return false;
        }

        let var = crate::expr_var::get_var_expr_var(scip, child);

        // Check whether variable is binary, in any feasible solution; we need to
        // forbid weakly implied binary variables because cons_and wouldn't ensure
        // that the product condition holds in any feasible solution (i.e., when
        // vars may not be at bounds) in this case, which would lead to accepting
        // solutions that are not feasible in the original (non-reformulated) cons.
        if !crate::var::var_is_binary(scip, var) || scip.vars[var].varimpltype == crate::types::ImplintType::Weak {
            return false;
        }
    }

    true
}

/// `getBilinearBinaryTerms`: helper method to collect all bilinear binary
/// product terms; returns (xs, ys, childidxs).
fn get_bilinear_binary_terms(scip: &Scip, sumexpr: ExprId) -> (Vec<VarId>, Vec<VarId>, Vec<usize>) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut childidxs = Vec::new();

    for (i, &child) in scip.exprs[sumexpr].children.iter().enumerate() {
        if scip.exprs[child].children.len() == 2 && is_binary_product(scip, child) {
            let x = crate::expr_var::get_var_expr_var(scip, scip.exprs[child].children[0]);
            let y = crate::expr_var::get_var_expr_var(scip, scip.exprs[child].children[1]);

            if x != y {
                xs.push(x);
                ys.push(y);
                childidxs.push(i);
            }
        }
    }

    (xs, ys, childidxs)
}

/// `reformulateFactorizedBinaryQuadratic`: helper method to reformulate x_i
/// sum_j c_ij x_j.
fn reformulate_factorized_binary_quadratic(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    facvar: VarId,
    vars: &[VarId],
    coefs: &[f64],
    mut naddconss: Option<&mut i32>,
) -> ScipResult<ExprId> {
    let mut minact = 0.0;
    let mut maxact = 0.0;
    let mut impltype = crate::types::ImplintType::Strong;

    // Compute minimum and maximum activity of sum_j c_ij x_j.
    // TODO could compute minact and maxact for facvar=0 and facvar=1
    // separately, taking implied bounds into account, allowing for possibly
    // tighter big-M's below.
    for &c in coefs {
        minact += cmin(c, 0.0);
        maxact += cmax(c, 0.0);

        if impltype != crate::types::ImplintType::None && !scip.set.is_integral(c) {
            impltype = crate::types::ImplintType::None;
        }
    }

    let consname = scip.conss[cons].name.clone();
    let facname = scip.vars[facvar].name.clone();
    let inf = scip.set.infinity();

    // Create and add auxiliary variable.
    let name = format!("binreform_{consname}_{facname}");
    let mut auxvar = crate::scip_var::create_var_impl(scip, &name, minact, maxact, 0.0, crate::types::VarType::Continuous, impltype, true, false)?;
    crate::scip_var::add_var(scip, auxvar)?;

    // Create and add z - maxact x <= 0.
    if !scip.set.is_zero(maxact) {
        let name = format!("binreform_{consname}_{facname}_1");
        let mut newcons = crate::cons_varbound::create_cons_basic_varbound(scip, &name, auxvar, facvar, -maxact, -inf, 0.0)?;
        crate::scip_cons::add_cons(scip, newcons)?;
        crate::scip_cons::release_cons(scip, &mut newcons)?;
        if let Some(n) = naddconss.as_deref_mut() {
            *n += 1;
        }
    }

    // Create and add 0 <= z - minact x.
    if !scip.set.is_zero(minact) {
        let name = format!("binreform_{consname}_{facname}_2");
        let mut newcons = crate::cons_varbound::create_cons_basic_varbound(scip, &name, auxvar, facvar, -minact, 0.0, inf)?;
        crate::scip_cons::add_cons(scip, newcons)?;
        crate::scip_cons::release_cons(scip, &mut newcons)?;
        if let Some(n) = naddconss.as_deref_mut() {
            *n += 1;
        }
    }

    // Create and add minact <= sum_j c_j x_j - z + minact x_i.
    let name = format!("binreform_{consname}_{facname}_3");
    let mut newcons = crate::cons_linear::create_cons_basic_linear(scip, &name, vars, coefs, minact, inf)?;
    crate::cons_linear::add_coef_linear(scip, newcons, auxvar, -1.0)?;
    if !scip.set.is_zero(minact) {
        crate::cons_linear::add_coef_linear(scip, newcons, facvar, minact)?;
    }
    crate::scip_cons::add_cons(scip, newcons)?;
    crate::scip_cons::release_cons(scip, &mut newcons)?;
    if let Some(n) = naddconss.as_deref_mut() {
        *n += 1;
    }

    // Create and add sum_j c_j x_j - z + maxact x_i <= maxact.
    let name = format!("binreform_{consname}_{facname}_4");
    let mut newcons = crate::cons_linear::create_cons_basic_linear(scip, &name, vars, coefs, -inf, maxact)?;
    crate::cons_linear::add_coef_linear(scip, newcons, auxvar, -1.0)?;
    if !scip.set.is_zero(maxact) {
        crate::cons_linear::add_coef_linear(scip, newcons, facvar, maxact)?;
    }
    crate::scip_cons::add_cons(scip, newcons)?;
    crate::scip_cons::release_cons(scip, &mut newcons)?;
    if let Some(n) = naddconss {
        *n += 1;
    }

    // Create variable expression.
    let newexpr = create_expr_var(scip, conshdlr, auxvar)?;

    // Release auxvar.
    crate::api::release_var(scip, &mut auxvar)?;

    Ok(newexpr)
}

/// `getFactorizedBinaryQuadraticExpr`: helper method to generate an
/// expression for a sum of products of binary variables; note that the
/// method captures the generated expression (`NONE` if none).
fn get_factorized_binary_quadratic_expr(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    sumexpr: ExprId,
    minterms: i32,
    mut naddconss: Option<&mut i32>,
) -> ScipResult<ExprId> {
    // Check whether sumexpr is indeed a sum.
    if !expr::expr_is_sum(scip, sumexpr) {
        return Ok(NONE);
    }

    let nchildren = scip.exprs[sumexpr].children.len();
    let sumcoefs = crate::expr_sum::get_coefs_expr_sum(scip, sumexpr).to_vec();
    let nvars = crate::api::get_n_vars(scip) as usize;
    let ntotalvars = scip.stat.nvaridx as usize;

    // Check whether there are enough terms available.
    if (nchildren as i32) < minterms {
        return Ok(NONE);
    }

    // Collect all bilinear binary product terms.
    let (xs, ys, childidxs) = get_bilinear_binary_terms(scip, sumexpr);
    let nterms = xs.len();

    // Check whether there are enough terms available.
    if (nterms as i32) < minterms {
        return Ok(NONE);
    }

    // Store how often each variable appears in a bilinear binary product.
    let mut vars = crate::api::get_vars(scip).to_vec();
    let mut count = vec![0i32; ntotalvars];
    let mut isused = vec![false; nchildren];
    let mut exprs: Vec<ExprId> = Vec::with_capacity(nchildren);
    let mut exprcoefs: Vec<f64> = Vec::with_capacity(nchildren);

    for i in 0..nterms {
        let xidx = scip.vars[xs[i]].index as usize;
        let yidx = scip.vars[ys[i]].index as usize;
        count[xidx] += 1;
        count[yidx] += 1;
    }

    // Sort variables; don't change order of count array because it depends on
    // problem indices.
    {
        let mut tmpcount = count[..nvars].to_vec();
        crate::sort::sort_down(&mut tmpcount, (&mut vars[..],), crate::sort::cmp_int);
    }

    for i in 0..nvars {
        let facvar = vars[i];
        let mut tmpvars = Vec::new();
        let mut tmpcoefs = Vec::new();

        // Skip candidate if there are not enough terms left.
        if count[scip.vars[vars[i]].index as usize] < minterms {
            continue;
        }

        // Collect variables for x_i * sum_j c_ij x_j.
        for j in 0..nterms {
            let childidx = childidxs[j];

            if !isused[childidx] && (xs[j] == facvar || ys[j] == facvar) {
                let coef = sumcoefs[childidx];

                // Collect corresponding variable.
                tmpvars.push(if xs[j] == facvar { ys[j] } else { xs[j] });
                tmpcoefs.push(coef);

                // Update counters.
                let xidx = scip.vars[xs[j]].index as usize;
                let yidx = scip.vars[ys[j]].index as usize;
                count[xidx] -= 1;
                count[yidx] -= 1;

                // Mark term to be used.
                isused[childidx] = true;
            }
        }

        // Create required constraints and store the generated expression.
        let e = reformulate_factorized_binary_quadratic(scip, conshdlr, cons, facvar, &tmpvars, &tmpcoefs, naddconss.as_deref_mut())?;
        exprs.push(e);
        exprcoefs.push(1.0);
    }

    // Factorization was only successful if at least one expression has been
    // generated.
    let mut newexpr = NONE;
    if !exprs.is_empty() {
        let nexprsold = exprs.len();

        // Add all children of the sum that have not been used.
        for i in 0..nchildren {
            if !isused[i] {
                exprs.push(scip.exprs[sumexpr].children[i]);
                exprcoefs.push(sumcoefs[i]);
            }
        }

        // Create a new sum expression.
        let constant = crate::expr_sum::get_constant_expr_sum(scip, sumexpr);
        newexpr = crate::expr_sum::create_expr_sum(scip, &exprs, Some(&exprcoefs), constant, Some(exprowner_create), conshdlr)?;

        // Release all expressions that have been generated by
        // reformulateFactorizedBinaryQuadratic().
        for e in exprs.iter_mut().take(nexprsold) {
            expr::expr_release(scip, e)?;
        }
    }

    Ok(newexpr)
}

/// `getBinaryProductExprDo`: helper method to create an AND constraint or
/// varbound constraints for a given binary product expression.
fn get_binary_product_expr_do(scip: &mut Scip, conshdlr: ConshdlrId, prodexpr: ExprId, naddconss: Option<&mut i32>, empathy4and: bool) -> ScipResult<ExprId> {
    let nchildren = scip.exprs[prodexpr].children.len();

    // Memory to store the variables of the variable expressions (+1 for w) and
    // their name.
    let mut vars = Vec::with_capacity(nchildren + 1);
    let mut coefs = Vec::with_capacity(nchildren + 1);

    // Prepare the names of the variable and the constraints.
    let mut name = String::from("binreform");
    for i in 0..nchildren {
        let v = crate::expr_var::get_var_expr_var(scip, scip.exprs[prodexpr].children[i]);
        vars.push(v);
        coefs.push(1.0);
        name.push('_');
        name.push_str(&scip.vars[v].name);
    }

    // Create and add variable.
    let mut w = crate::scip_var::create_var_impl(
        scip,
        &name,
        0.0,
        1.0,
        0.0,
        crate::types::VarType::Continuous,
        crate::types::ImplintType::Strong,
        true,
        false,
    )?;
    crate::scip_var::add_var(scip, w)?;

    // Use variable bound constraints if it is a bilinear product and there is
    // no empathy for an AND constraint.
    if nchildren == 2 && !empathy4and {
        let x = vars[0];
        let y = vars[1];
        let (xname, yname) = (scip.vars[x].name.clone(), scip.vars[y].name.clone());
        let inf = scip.set.infinity();

        // Create and add x - w >= 0.
        let mut cons = crate::cons_varbound::create_cons_basic_varbound(scip, &format!("binreform_{xname}_{yname}_1"), x, w, -1.0, 0.0, inf)?;
        crate::scip_cons::add_cons(scip, cons)?;
        crate::scip_cons::release_cons(scip, &mut cons)?;

        // Create and add y - w >= 0.
        let mut cons = crate::cons_varbound::create_cons_basic_varbound(scip, &format!("binreform_{xname}_{yname}_2"), y, w, -1.0, 0.0, inf)?;
        crate::scip_cons::add_cons(scip, cons)?;
        crate::scip_cons::release_cons(scip, &mut cons)?;

        // Create and add x + y - w <= 1.
        vars.push(w);
        coefs.push(-1.0);
        let mut cons = crate::cons_linear::create_cons_basic_linear(scip, &format!("binreform_{xname}_{yname}_3"), &vars[..3], &coefs[..3], -inf, 1.0)?;
        crate::scip_cons::add_cons(scip, cons)?;
        crate::scip_cons::release_cons(scip, &mut cons)?;

        // Update number of added constraints.
        if let Some(n) = naddconss {
            *n += 3;
        }
    } else {
        // Create, add, and release AND constraint.
        let mut cons = crate::cons_and::create_cons_basic_and(scip, &name, w, &vars[..nchildren])?;
        crate::scip_cons::add_cons(scip, cons)?;
        crate::scip_cons::release_cons(scip, &mut cons)?;

        // Update number of added constraints.
        if let Some(n) = naddconss {
            *n += 1;
        }
    }

    // Create variable expression.
    let newexpr = create_expr_var(scip, conshdlr, w)?;

    // Release created variable.
    crate::api::release_var(scip, &mut w)?;

    Ok(newexpr)
}

/// `getBinaryProductExpr`: helper method to generate an expression for the
/// product of binary variables; note that the method captures the generated
/// expression (`NONE` if none).
fn get_binary_product_expr(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    exprmap: &mut HashMap<ExprId, ExprId>,
    prodexpr: ExprId,
    naddconss: Option<&mut i32>,
    mut nchgcoefs: Option<&mut i32>,
) -> ScipResult<ExprId> {
    // Only consider products of binary variables.
    if !is_binary_product(scip, prodexpr) {
        return Ok(NONE);
    }

    let nchildren = scip.exprs[prodexpr].children.len();
    let reformbinprodsand = hd(scip, conshdlr).reformbinprodsand;

    // Check whether there is already an expression that represents the
    // product.
    if let Some(&e) = exprmap.get(&prodexpr) {
        // Capture expression.
        expr::expr_capture(scip, e);
        return Ok(e);
    }

    let newexpr = if nchildren == 2 {
        let mut found_clique = false;
        let mut found = NONE;

        // Get variables from the product expression.
        let x = crate::expr_var::get_var_expr_var(scip, scip.exprs[prodexpr].children[0]);
        let y = crate::expr_var::get_var_expr_var(scip, scip.exprs[prodexpr].children[1]);

        let xcliques = |scip: &Scip, value: bool| -> Vec<usize> {
            scip.vars[x]
                .cliquelist
                .as_ref()
                .map_or(Vec::new(), |l| l.cliques[value as usize].0[..l.ncliques[value as usize] as usize].to_vec())
        };

        // First try to find a clique containing both variables; look in cliques
        // containing x.
        for c in xcliques(scip, true) {
            if crate::implics::clique_has_var(&scip.cliques[c], y, true) {
                // x + y <= 1 => x*y = 0: create zero value expression.
                found = crate::expr_value::create_expr_value(scip, 0.0, Some(exprowner_create), conshdlr)?;
                if let Some(n) = nchgcoefs.as_deref_mut() {
                    *n += 1;
                }
                found_clique = true;
                break;
            }

            if crate::implics::clique_has_var(&scip.cliques[c], y, false) {
                // x + (1-y) <= 1 => x*y = x: create variable expression for x.
                found = create_expr_var(scip, conshdlr, x)?;
                if let Some(n) = nchgcoefs.as_deref_mut() {
                    *n += 2;
                }
                found_clique = true;
                break;
            }
        }

        if !found_clique {
            // Look in cliques containing complement of x.
            for c in xcliques(scip, false) {
                if crate::implics::clique_has_var(&scip.cliques[c], y, true) {
                    // (1-x) + y <= 1 => x*y = y: create variable expression for y.
                    found = create_expr_var(scip, conshdlr, y)?;
                    if let Some(n) = nchgcoefs.as_deref_mut() {
                        *n += 1;
                    }
                    found_clique = true;
                    break;
                }

                if crate::implics::clique_has_var(&scip.cliques[c], y, false) {
                    // (1-x) + (1-y) <= 1 => x*y = x + y - 1: create sum expression.
                    let mut c0 = create_expr_var(scip, conshdlr, x)?;
                    let mut c1 = create_expr_var(scip, conshdlr, y)?;
                    found = crate::expr_sum::create_expr_sum(scip, &[c0, c1], Some(&[1.0, 1.0]), -1.0, Some(exprowner_create), conshdlr)?;
                    expr::expr_release(scip, &mut c0)?;
                    expr::expr_release(scip, &mut c1)?;

                    if let Some(n) = nchgcoefs.as_deref_mut() {
                        *n += 3;
                    }
                    found_clique = true;
                    break;
                }
            }
        }

        // If the variables are not in a clique, do standard linearization.
        if !found_clique {
            get_binary_product_expr_do(scip, conshdlr, prodexpr, naddconss, reformbinprodsand)?
        } else {
            found
        }
    } else {
        // Linearize binary product using an AND constraint because nchildren > 2.
        get_binary_product_expr_do(scip, conshdlr, prodexpr, naddconss, reformbinprodsand)?
    };

    // Hash variable expression.
    exprmap.insert(prodexpr, newexpr);

    Ok(newexpr)
}

/// `replaceBinaryProducts`: helper function to replace binary products in a
/// given constraint.
fn replace_binary_products(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    exprmap: &mut HashMap<ExprId, ExprId>,
    it: &mut expr::ExprIter,
    mut naddconss: Option<&mut i32>,
    mut nchgcoefs: Option<&mut i32>,
) -> ScipResult {
    let reformbinprodsfac = hd(scip, conshdlr).reformbinprodsfac;

    let consexpr = cd(scip, cons).expr;
    let mut e = it.restart_dfs(scip, consexpr);
    while !it.is_end() {
        let mut newexpr = NONE;

        let childexpridx = it.get_child_idx_dfs(scip) as usize;
        let childexpr = it.get_child_expr_dfs(scip);

        // Try to factorize variables in a sum expression that contains several
        // products of binary variables.
        if reformbinprodsfac > 1 {
            newexpr = get_factorized_binary_quadratic_expr(scip, conshdlr, cons, childexpr, reformbinprodsfac, naddconss.as_deref_mut())?;
        }

        // Try to create an expression that represents a product of binary
        // variables.
        if newexpr == NONE {
            newexpr = get_binary_product_expr(scip, conshdlr, exprmap, childexpr, naddconss.as_deref_mut(), nchgcoefs.as_deref_mut())?;
        }

        if newexpr != NONE {
            // Replace product expression.
            expr::expr_replace_child(scip, e, childexpridx, newexpr)?;

            // Note that the expression has been captured by getBinaryProductExpr and
            // SCIPreplaceExprChild.
            expr::expr_release(scip, &mut newexpr)?;

            // Mark the constraint to not be simplified anymore.
            cdm(scip, cons).issimplified = false;
        }

        e = it.get_next(scip);
    }

    Ok(())
}

/// `presolveBinaryProducts`: reformulates products of binary variables
/// during presolving.
///
/// Let sum_{i,j} Q_{ij} x_i x_j be a subexpression that only contains
/// binary variables. Each term x_i x_j is reformulated with the help of an
/// extra (implicit integer) variable z_{ij} in {0,1}: z_{ij} <= x_i, z_{ij}
/// <= x_j, x_i + x_j - z_{ij} <= 1.
///
/// Before reformulating x_i x_j in this way, it is checked whether there is
/// a clique that contains x_i and x_j. These cliques allow for a better
/// reformulation. There are four cases:
///  1. x_i + x_j <= 1 implies that x_i x_j = 0
///  2. x_i + (1 - x_j) <= 1 implies x_i x_j = x_i
///  3. (1 - x_i) + x_j <= 1 implies x_i x_j = x_j
///  4. (1 - x_i) + (1 - x_j) <= 1 implies x_i x_j = x_i + x_j - 1
///
/// The reformulation using z_{ij} or the cliques is implemented in
/// getBinaryProductExpr().
///
/// Introducing too many extra variables and constraints can have a negative
/// impact on the performance (e.g., due to slow probing). For this reason, it
/// is checked in getFactorizedBinaryQuadraticExpr() whether sum_{i,j} Q_{ij}
/// x_i x_j contains large (>= `reformbinprodsfac` parameter) lower sums of the
/// form x_i sum_j Q_{ij} x_j. Such a lower sum is reformulated with only one
/// extra variable w_i (see the C). We mark w_i to be implicit integer if all
/// Q_{ij} are integer. After each replacement of a lower sum, it is checked
/// whether there are enough terms left to factorize other binary variables.
/// Lower sums with a larger number of terms are prioritized.
fn presolve_binary_products(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], mut naddconss: Option<&mut i32>, mut nchgcoefs: Option<&mut i32>) -> ScipResult {
    // No nonlinear constraints or binary variables -> skip.
    if conss.is_empty() || crate::api::get_n_bin_vars(scip) + crate::api::get_n_impl_vars(scip) == 0 {
        return Ok(());
    }

    let reformbinprodsfac = hd(scip, conshdlr).reformbinprodsfac;

    // Create expression hash map.
    let mut exprmap = HashMap::new();

    // Create expression iterator.
    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_VISITINGCHILD);

    for &cons in conss {
        let mut newexpr = NONE;

        // Try to reformulate the root expression.
        if reformbinprodsfac > 1 {
            let consexpr = cd(scip, cons).expr;
            newexpr = get_factorized_binary_quadratic_expr(scip, conshdlr, cons, consexpr, reformbinprodsfac, naddconss.as_deref_mut())?;
        }

        // Release the root node if another expression has been found.
        if newexpr != NONE {
            let mut old = cd(scip, cons).expr;
            expr::expr_release(scip, &mut old)?;
            let c = cdm(scip, cons);
            c.expr = newexpr;

            // Mark constraint to be not simplified anymore.
            c.issimplified = false;
        }

        // Replace each product of binary variables separately.
        replace_binary_products(scip, conshdlr, cons, &mut exprmap, &mut it, naddconss.as_deref_mut(), nchgcoefs.as_deref_mut())?;
    }

    // Free memory.
    it.free(scip);

    Ok(())
}

/// `scaleConsSides`: scales the sides of the constraint l <= sum_i c_i
/// f_i(x) <= r; returns whether the expression of cons changed.
///
/// Let n_+ the number of positive coefficients c_i and n_- be the number of
/// negative coefficients. Then scale by -1 if
/// - n_+ < n_-, or
/// - n_+ = n_- and r = infinity.
fn scale_cons_sides(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, changed: &mut bool) -> ScipResult {
    let consexpr = cd(scip, cons).expr;

    if expr::expr_is_sum(scip, consexpr) {
        let coefs = crate::expr_sum::get_coefs_expr_sum(scip, consexpr).to_vec();
        let constant = crate::expr_sum::get_constant_expr_sum(scip, consexpr);
        let children = scip.exprs[consexpr].children.clone();
        let nchildren = children.len();

        // Handle special case when constraint is l <= -f(x) <= r and f(x) not a
        // sum: simplfy ensures f is not a sum.
        if nchildren == 1 && constant == 0.0 && coefs[0] == -1.0 {
            let mut e = consexpr;
            let consdata = cdm(scip, cons);
            consdata.expr = children[0];
            expr::expr_capture(scip, children[0]);
            let consdata = cdm(scip, cons);
            std::mem::swap(&mut consdata.lhs, &mut consdata.rhs);
            consdata.lhs = -consdata.lhs;
            consdata.rhs = -consdata.rhs;
            expr::expr_release(scip, &mut e)?;
            *changed = true;
            return Ok(());
        }

        // Compute n_+ - n_i.
        let mut counter = 0;
        for &c in &coefs[..nchildren] {
            counter += if c > 0.0 { 1 } else { -1 };
        }

        if counter < 0 || (counter == 0 && scip.set.is_infinity(cd(scip, cons).rhs)) {
            let newcoefs: Vec<f64> = coefs[..nchildren].iter().map(|c| -c).collect();

            // Create a new sum expression.
            let e = crate::expr_sum::create_expr_sum(scip, &children, Some(&newcoefs), -constant, Some(exprowner_create), conshdlr)?;

            // Replace expression in constraint data and scale sides.
            let mut old = consexpr;
            expr::expr_release(scip, &mut old)?;
            let consdata = cdm(scip, cons);
            consdata.expr = e;
            std::mem::swap(&mut consdata.lhs, &mut consdata.rhs);
            consdata.lhs = -consdata.lhs;
            consdata.rhs = -consdata.rhs;

            *changed = true;
        }
    }

    Ok(())
}

/// `forbidNonlinearVariablesMultiaggration`: forbid multiaggrations of
/// variables that appear nonlinear in constraints.
fn forbid_nonlinear_variables_multiaggration(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    if !hd(scip, conshdlr).forbidmultaggrnlvar {
        return Ok(());
    }

    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;

    let mark_vars = |scip: &mut Scip, it: &mut expr::ExprIter, root: ExprId| -> ScipResult {
        let mut e = it.restart_dfs(scip, root);
        while !it.is_end() {
            if expr::expr_is_var(scip, e) {
                let var = crate::expr_var::get_var_expr_var(scip, e);
                crate::scip_var::mark_do_not_multaggr_var(scip, var)?;
            }
            e = it.get_next(scip);
        }
        Ok(())
    };

    for &cons in conss {
        let consexpr = cd(scip, cons).expr;

        // If root expression is sum, then forbid multiaggregation only for
        // variables that are not in linear terms of sum, i.e., skip children of
        // sum that are variables.
        if expr::expr_is_sum(scip, consexpr) {
            let children = scip.exprs[consexpr].children.clone();
            for child in children {
                // Skip variable expression, as they correspond to a linear term.
                if expr::expr_is_var(scip, child) {
                    continue;
                }

                mark_vars(scip, &mut it, child)?;
            }
        } else {
            mark_vars(scip, &mut it, consexpr)?;
        }
    }

    it.free(scip);

    Ok(())
}

/// `canonicalizeConstraints`: simplifies expressions and replaces common
/// subexpressions for a set of constraints.
///
/// @todo put the constant to the constraint sides
#[allow(clippy::too_many_arguments)]
fn canonicalize_constraints(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    presoltiming: crate::cons::PresolTiming,
    infeasible: &mut bool,
    mut ndelconss: Option<&mut i32>,
    naddconss: Option<&mut i32>,
    nchgcoefs: Option<&mut i32>,
) -> ScipResult {
    // Update number of canonicalize calls.
    hdm(scip, conshdlr).ncanonicalizecalls += 1;

    let te = scip.set.time_enabled;
    hdm(scip, conshdlr).canonicalizetime.start(te);

    *infeasible = false;

    // Set havechange to TRUE in the first call of canonicalize; otherwise we
    // might not replace common subexpressions.
    let mut havechange = hd(scip, conshdlr).ncanonicalizecalls == 1;

    // Free nonlinear handlers information from expressions.
    // TODO can skip this in first presolve round.
    deinit_solve(scip, conshdlr, conss)?;

    // Allocate memory for storing locks of each constraint.
    let mut nlockspos = vec![0; conss.len()];
    let mut nlocksneg = vec![0; conss.len()];

    // Unlock all constraints.
    for (i, &cons) in conss.iter().enumerate() {
        // Remember locks.
        nlockspos[i] = cd(scip, cons).nlockspos;
        nlocksneg[i] = cd(scip, cons).nlocksneg;

        // Remove locks.
        add_locks(scip, cons, -nlockspos[i], -nlocksneg[i])?;
    }

    // (The reference is built without NDEBUG and checks here that all locks of
    // each expression have been removed.)

    // Reformulate products of binary variables.
    if hd(scip, conshdlr).reformbinprods && scip.set.stage == Stage::Presolving && (presoltiming & crate::cons::PRESOLTIMING_EXHAUSTIVE) != 0 {
        let mut tmpnaddconss = 0;
        let mut tmpnchgcoefs = 0;

        // Call this function before simplification because expressions might not
        // be simplified after reformulating binary products; the detection of
        // some nonlinear handlers might assume that expressions are simplified.
        presolve_binary_products(scip, conshdlr, conss, Some(&mut tmpnaddconss), Some(&mut tmpnchgcoefs))?;

        // Update counters.
        if let Some(n) = naddconss {
            *n += tmpnaddconss;
        }
        if let Some(n) = nchgcoefs {
            *n += tmpnchgcoefs;
        }

        // Check whether at least one expression has changed.
        if tmpnaddconss + tmpnchgcoefs > 0 {
            havechange = true;
        }
    }

    for (i, &cons) in conss.iter().enumerate() {
        // Call simplify for each expression.
        if !cd(scip, cons).issimplified && cd(scip, cons).expr != NONE {
            let consexpr = cd(scip, cons).expr;
            let (mut simplified, changed, inf) = crate::scip_expr::simplify_expr(scip, consexpr, Some(exprowner_create), conshdlr)?;
            *infeasible = inf;
            cdm(scip, cons).issimplified = true;

            if changed {
                havechange = true;
            }

            // If root expression changed, then we need to take care updating the
            // locks as well (the consdata is the one holding consdata->expr "as a
            // child"). If root expression did not change, some subexpression may
            // still have changed, but the locks were taking care of in the
            // corresponding SCIPreplaceExprChild() call.
            if simplified != consexpr {
                // Release old expression.
                let mut old = consexpr;
                expr::expr_release(scip, &mut old)?;

                // Store simplified expression.
                cdm(scip, cons).expr = simplified;
            } else {
                // The simplify captures simplified in any case, also if nothing has
                // changed. Therefore, we have to release it here.
                expr::expr_release(scip, &mut simplified)?;
            }

            if *infeasible {
                break;
            }

            // Scale constraint sides.
            let mut changed = false;
            scale_cons_sides(scip, conshdlr, cons, &mut changed)?;
            if changed {
                havechange = true;
            }

            // Handle constant root expression; either the problem is infeasible or
            // the constraint is redundant.
            let consexpr = cd(scip, cons).expr;
            if expr::expr_is_value(scip, consexpr) {
                let value = crate::expr_value::get_value_expr_value(scip, consexpr);
                let (lhs, rhs) = (cd(scip, cons).lhs, cd(scip, cons).rhs);
                if (!scip.set.is_infinity(-lhs) && scip.set.is_feas_negative(value - lhs)) || (!scip.set.is_infinity(rhs) && scip.set.is_feas_positive(value - rhs)) {
                    *infeasible = true;
                    break;
                } else {
                    add_locks(scip, cons, nlockspos[i], nlocksneg[i])?;
                    crate::scip_cons::del_cons(scip, cons)?;
                    if let Some(n) = ndelconss.as_deref_mut() {
                        *n += 1;
                    }
                    havechange = true;
                }
            }
        }
    }

    // Replace common subexpressions.
    if havechange && !*infeasible {
        let mut rootexprs: Vec<ExprId> = conss.iter().map(|&c| cd(scip, c).expr).collect();
        let replacedroot = crate::scip_expr::replace_common_subexpressions(scip, &mut rootexprs)?;

        // Update pointer to root expr in constraints, if any has changed;
        // SCIPreplaceCommonSubexpressions will have released the old expr and
        // captures the new one.
        if replacedroot {
            for (i, &cons) in conss.iter().enumerate() {
                cdm(scip, cons).expr = rootexprs[i];
            }
        }

        // TODO this is a possibly expensive way to update the variable expressions
        // stored inside an expression which might have been changed after
        // simplification; now we completely recollect all variable expression and
        // variable events.

        // Each variable stores the constraints for which it catched varbound events
        // sorted by the constraint index. Thus, for performance reasons, it is
        // better to call dropVarEvents in descending order of constraint index.
        let mut consssorted = conss.to_vec();
        crate::sort::sort(&mut consssorted, (), |&a, &b| comp_index_cons_nonlinear(scip, a, b));

        let eventhdlr = hd(scip, conshdlr).eventhdlr;
        for &cons in consssorted.iter().rev() {
            if scip.conss[cons].deleted {
                continue;
            }

            drop_var_events(scip, eventhdlr, cons)?;
            free_var_exprs(scip, cons)?;
        }
        for &cons in &consssorted {
            if scip.conss[cons].deleted {
                continue;
            }

            store_var_exprs(scip, conshdlr, cons)?;
            catch_var_events(scip, eventhdlr, cons)?;
        }

        // Forbid multiaggregation for nonlinear variables again (in case new
        // variables appeared now); a multiaggregation of a nonlinear variable can
        // yield to a large increase in expressions due to expanding terms in
        // simplify, e.g. ,(sum_i x_i)^2, so we just forbid these.
        forbid_nonlinear_variables_multiaggration(scip, conshdlr, conss)?;
    }

    // Restore locks.
    for (i, &cons) in conss.iter().enumerate() {
        if scip.conss[cons].deleted {
            continue;
        }

        add_locks(scip, cons, nlockspos[i], nlocksneg[i])?;
    }

    // Run nlhdlr detect if in presolving stage (that is, not in exitpre).
    // TODO can we skip this in presoltiming fast?
    if scip.set.stage == Stage::Presolving && !*infeasible {
        // Reset one of the number of detections counter to count only current
        // presolving round.
        for nh in hd(scip, conshdlr).nlhdlrs.clone() {
            nlhdlr::nlhdlr_reset_n_detectionslast(scip, nh);
        }

        init_solve(scip, conshdlr, conss)?;
    }

    hdm(scip, conshdlr).canonicalizetime.stop(te);

    Ok(())
}

/// `presolveMergeConss`: merges constraints that have the same root
/// expression; returns whether at least one constraint could be deleted.
fn presolve_merge_conss(scip: &mut Scip, conss: &[ConsId]) -> ScipResult<bool> {
    let mut success = false;
    let nconss = conss.len();

    // Not enough constraints available.
    if nconss <= 1 {
        return Ok(false);
    }

    let mut expr2cons: HashMap<ExprId, usize> = HashMap::new();
    let mut updatelocks = vec![false; nconss];
    let mut nlockspos = vec![0; nconss];
    let mut nlocksneg = vec![0; nconss];

    for c in 0..nconss {
        // Ignore deleted constraints.
        if scip.conss[conss[c]].deleted {
            continue;
        }

        let consexpr = cd(scip, conss[c]).expr;

        // Add expression to the hash map if not seen so far.
        match expr2cons.get(&consexpr).copied() {
            None => {
                expr2cons.insert(consexpr, c);
            }
            Some(idx) => {
                let (lhs, rhs) = (cd(scip, conss[c]).lhs, cd(scip, conss[c]).rhs);
                let img = cd(scip, conss[idx]).clone();

                // Check whether locks need to be updated.
                if !updatelocks[idx] && ((scip.set.is_infinity(-img.lhs) && !scip.set.is_infinity(-lhs)) || (scip.set.is_infinity(img.rhs) && !scip.set.is_infinity(rhs))) {
                    nlockspos[idx] = img.nlockspos;
                    nlocksneg[idx] = img.nlocksneg;
                    add_locks(scip, conss[idx], -img.nlockspos, -img.nlocksneg)?;
                    updatelocks[idx] = true;
                }

                // Update constraint sides.
                let imgconsdata = cdm(scip, conss[idx]);
                imgconsdata.lhs = cmax(imgconsdata.lhs, lhs);
                imgconsdata.rhs = cmin(imgconsdata.rhs, rhs);

                // Delete constraint.
                crate::scip_cons::del_cons(scip, conss[c])?;
                success = true;
            }
        }
    }

    // Restore locks of updated constraints.
    if success {
        for c in 0..nconss {
            if updatelocks[c] {
                add_locks(scip, conss[c], nlockspos[c], nlocksneg[c])?;
            }
        }
    }

    Ok(success)
}

/// `intEvalVarRedundancyCheck`: interval evaluation of variables as used in
/// redundancy check; `intevalvardata` is the constraint handler.
///
/// Returns local variable bounds of a variable, relaxed by feastol, as
/// interval.
fn int_eval_var_redundancy_check(scip: &mut Scip, var: VarId, intevalvardata: usize) -> Interval {
    let h = hd(scip, intevalvardata);
    let v = &scip.vars[var];

    let (mut lb, mut ub) = if h.globalbounds { (v.glbdom.lb, v.glbdom.ub) } else { (v.locdom.lb, v.locdom.ub) };

    // Relax variable bounds, if there are bounds and variable is not fixed
    // (actually some assert complains if trying SCIPisRelEQ if both bounds are
    // at different infinity).
    if !(scip.set.is_infinity(-lb) && scip.set.is_infinity(ub)) && !scip.set.is_rel_eq(lb, ub) {
        if !scip.set.is_infinity(-lb) {
            lb -= scip.set.feastol();
        }
        if !scip.set.is_infinity(ub) {
            ub += scip.set.feastol();
        }
    }

    // Convert SCIPinfinity() to SCIP_INTERVAL_INFINITY.
    let inf = scip.set.infinity();
    lb = -infty2infty(inf, INTERVAL_INFINITY, -lb);
    ub = infty2infty(inf, INTERVAL_INFINITY, ub);

    let mut interval = Interval::default();
    interval.set_bounds(lb, ub);

    interval
}

/// `presolveRedundantConss`: removes constraints that are always feasible
/// or very simple.
///
/// Checks whether the activity of constraint functions is a subset of the
/// constraint sides (relaxed by feastol). To compute the activity, we use
/// forwardPropExpr(), but relax variable bounds by feastol, because
/// solutions to be checked might violate variable bounds by up to feastol,
/// too. This is the main reason why the redundancy check is not done in
/// propConss(), which relaxes variable bounds by epsilon only.
///
/// Also removes constraints of the form lhs <= variable <= rhs.
///
/// @todo it would be sufficient to check constraints for which we know that
/// they are not currently violated by a valid solution
///
/// @note This could should not run during solving, because the forwardProp
/// takes the bounds of auxiliary variables into account. For the root
/// expression, these bounds are already set to the constraint sides, so that
/// the activity of every expression would appear as if the constraint is
/// redundant.
fn presolve_redundant_conss(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], cutoff: &mut bool, ndelconss: &mut i32, nchgbds: &mut i32) -> ScipResult {
    // No constraints to check.
    if conss.is_empty() {
        return Ok(());
    }

    // Increase curboundstag and set lastvaractivitymethodchange; we do this here
    // to trigger a reevaluation of all variable bounds, since we will relax
    // variable bounds for the redundancy check differently than for domain
    // propagation; we also update lastboundrelax to ensure activites of all
    // expressions are indeed reevaluated.
    {
        let h = hdm(scip, conshdlr);
        h.curboundstag += 1;
        h.lastvaractivitymethodchange = h.curboundstag;
        h.lastboundrelax = h.curboundstag;
        h.intevalvar = IntEvalVar::RedundancyCheck;
    }

    *cutoff = false;
    let feastol = scip.set.feastol();

    'terminate: for &cons in conss {
        if !scip.conss[cons].active || scip.conss[cons].deleted {
            continue;
        }

        let (consexpr, lhs, rhs) = {
            let c = cd(scip, cons);
            (c.expr, c.lhs, c.rhs)
        };

        // Handle constant expressions separately: either the problem is
        // infeasible or the constraint is redundant.
        if expr::expr_is_value(scip, consexpr) {
            let value = crate::expr_value::get_value_expr_value(scip, consexpr);
            if (!scip.set.is_infinity(-lhs) && value < lhs - feastol) || (!scip.set.is_infinity(rhs) && value > rhs + feastol) {
                *cutoff = true;
                break 'terminate;
            }
            crate::scip_cons::del_cons_local(scip, cons)?;
            *ndelconss += 1;
            continue;
        }

        // Handle variable expressions separately: tighten variable bounds to
        // constraint sides, then remove constraint (now redundant).
        if expr::expr_is_var(scip, consexpr) {
            let var = crate::expr_var::get_var_expr_var(scip, consexpr);

            // Ensure that variable bounds are within constraint sides.
            if !scip.set.is_infinity(-lhs) {
                let (infeasible, tightened) = crate::scip_var::tighten_var_lb(scip, var, lhs, true)?;
                *cutoff = infeasible;
                if tightened {
                    *nchgbds += 1;
                }
                if *cutoff {
                    break 'terminate;
                }
            }

            if !scip.set.is_infinity(rhs) {
                let (infeasible, tightened) = crate::scip_var::tighten_var_ub(scip, var, rhs, true)?;
                *cutoff = infeasible;
                if tightened {
                    *nchgbds += 1;
                }
                if *cutoff {
                    break 'terminate;
                }
            }

            // Delete the (now) redundant constraint locally.
            crate::scip_cons::del_cons_local(scip, cons)?;
            *ndelconss += 1;
            continue;
        }

        // Reevaluate expression activity, now using intEvalVarRedundancyCheck; we
        // relax variable bounds by feastol here, as solutions that are checked
        // later can also violate variable bounds by up to feastol (relaxing fixed
        // variables seems to be too much, but they would be removed by presolve
        // soon anyway).
        forward_prop_expr(scip, conshdlr, consexpr, false, Some(cutoff), None)?;

        // It is unlikely that we detect infeasibility by doing forward
        // propagation.
        if *cutoff {
            break 'terminate;
        }

        let activity = scip.exprs[consexpr].activity;

        // Relax sides by feastol; we could accept every solution that violates
        // constraints up to feastol as redundant, so this is the most permissive
        // we can be.
        let mut sides = Interval::default();
        sides.set_bounds(
            if scip.set.is_infinity(-lhs) { -INTERVAL_INFINITY } else { lhs - feastol },
            if scip.set.is_infinity(rhs) { INTERVAL_INFINITY } else { rhs + feastol },
        );

        if interval::is_subset_eq(INTERVAL_INFINITY, activity, sides) {
            crate::scip_cons::del_cons_local(scip, cons)?;
            *ndelconss += 1;
            continue;
        }
    }

    // Make sure all activities are reevaluated again, since we relaxed bounds in
    // a different way.
    let h = hdm(scip, conshdlr);
    h.curboundstag += 1;
    h.lastvaractivitymethodchange = h.curboundstag;
    h.lastboundrelax = h.curboundstag;
    h.intevalvar = IntEvalVar::BoundTightening;

    Ok(())
}

/// `presolveUpgrade`: tries to automatically convert a nonlinear constraint
/// into a more specific and more specialized constraint; returns whether the
/// constraint was upgraded.
fn presolve_upgrade(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, nupgdconss: &mut i32, naddconss: &mut i32) -> ScipResult<bool> {
    let mut upgraded = false;
    let mut nupgdconss_ = 0;

    // If there are no upgrade methods, we can stop.
    let consupgrades = hd(scip, conshdlr).consupgrades.clone();
    if consupgrades.is_empty() {
        return Ok(false);
    }

    let mut upgdconsssize = 2;
    let mut upgdconss: Vec<ConsId> = Vec::with_capacity(upgdconsssize as usize);

    // Call the upgrading methods.
    let nvarexprs = cd(scip, cons).nvarexprs() as i32;

    // Try all upgrading methods in priority order in case the upgrading step is
    // enabled.
    for up in consupgrades {
        if !up.active {
            continue;
        }

        upgdconss.clear();
        (up.consupgd)(scip, cons, nvarexprs, &mut nupgdconss_, &mut upgdconss, upgdconsssize)?;

        while nupgdconss_ < 0 {
            // Upgrade function requires more memory: resize upgdconss and call
            // again.
            upgdconsssize = -nupgdconss_;
            upgdconss.clear();
            (up.consupgd)(scip, cons, nvarexprs, &mut nupgdconss_, &mut upgdconss, upgdconsssize)?;
        }

        if nupgdconss_ > 0 {
            // Got upgrade: add the upgraded constraints to the problem and forget
            // them.
            for c in upgdconss.iter_mut().take(nupgdconss_ as usize) {
                crate::scip_cons::add_cons(scip, *c)?;
                crate::scip_cons::release_cons(scip, c)?;
            }

            // Count the first upgrade constraint as constraint upgrade and the
            // remaining ones as added constraints.
            *nupgdconss += 1;
            *naddconss += nupgdconss_ - 1;
            upgraded = true;

            // Delete upgraded constraint.
            crate::scip_cons::del_cons(scip, cons)?;

            break;
        }
    }

    Ok(upgraded)
}

/// `isSingleLockedCand`: returns whether the variable of a given variable
/// expression is a candidate for presolveSingleLockedVars(), i.e., the
/// variable is only contained in a single nonlinear constraint, has no
/// objective coefficient, has finite variable bounds, and is not binary.
fn is_single_locked_cand(scip: &Scip, expr: ExprId) -> bool {
    let var = crate::expr_var::get_var_expr_var(scip, expr);
    let ownerdata = od(scip, expr);
    let v = &scip.vars[var];

    crate::var::var_get_n_locks_down_type(scip, var, LockType::Model) == ownerdata.nlocksneg
        && crate::var::var_get_n_locks_up_type(scip, var, LockType::Model) == ownerdata.nlockspos
        && ownerdata.conss.len() == 1
        && scip.set.is_zero(v.obj)
        && !scip.set.is_infinity(-v.glbdom.lb)
        && !scip.set.is_infinity(v.glbdom.ub)
        && (v.vartype != crate::types::VarType::Binary || crate::var::var_is_implied_integral(scip, var))
        && !scip.set.is_eq(v.glbdom.lb, v.glbdom.ub)
}

/// `removeSingleLockedVars`: removes all variable expressions that are
/// contained in a given expression from a hash map.
fn remove_single_locked_vars(scip: &mut Scip, expr: ExprId, it: &mut expr::ExprIter, exprcands: &mut std::collections::HashSet<ExprId>) {
    let mut e = it.restart_dfs(scip, expr);
    while !it.is_end() {
        if expr::expr_is_var(scip, e) {
            exprcands.remove(&e);
        }
        e = it.get_next(scip);
    }
}

/// `presolveSingleLockedVars`: presolving method to fix a variable x_i to
/// one of its bounds if the variable is only contained in a single nonlinear
/// constraint g(x) <= rhs (>= lhs) if g() is concave (convex) in x_i.
///
/// If a continuous variable has bounds [0,1], then the variable type is
/// changed to be binary. Otherwise, a bound disjunction constraint is added.
///
/// @todo the same reduction can be applied if g(x) is not concave, but
/// monotone in x_i for g(x) <= rhs (done in prop_dualfix?)
fn presolve_single_locked_vars(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, nchgvartypes: &mut i32, naddconss: &mut i32, infeasible: &mut bool) -> ScipResult {
    *nchgvartypes = 0;
    *naddconss = 0;
    *infeasible = false;

    let (lhs, rhs, consexpr) = {
        let c = cd(scip, cons);
        (c.lhs, c.rhs, c.expr)
    };

    // Only consider constraints with one finite side.
    if !scip.set.is_infinity(-lhs) && !scip.set.is_infinity(rhs) {
        return Ok(());
    }

    // Only consider sum expressions.
    if !expr::expr_is_sum(scip, consexpr) {
        return Ok(());
    }

    // Remember which side is finite.
    let haslhs = !scip.set.is_infinity(-lhs);
    let hasrhs = !scip.set.is_infinity(rhs);

    // Allocate memory.
    let mut exprcands = std::collections::HashSet::new();
    let mut singlelocked = Vec::new();

    // Check all variable expressions for single locked variables.
    for &ve in cd(scip, cons).varexprs.as_ref().unwrap() {
        if is_single_locked_cand(scip, ve) {
            exprcands.insert(ve);
            singlelocked.push(ve);
        }
    }

    if !singlelocked.is_empty() {
        let children = scip.exprs[consexpr].children.clone();
        let coefs = crate::expr_sum::get_coefs_expr_sum(scip, consexpr).to_vec();

        // Create iterator.
        let mut it = expr::ExprIter::create();
        it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
        it.set_stages_dfs(scip, expr::EXPRITER_ENTEREXPR);

        for (i, &child) in children.iter().enumerate() {
            let coef = coefs[i];

            // Ignore linear terms.
            if expr::expr_is_var(scip, child) {
                continue;
            }

            // Consider products coef * prod_j f_j(x)
            // - if f_j(x) is a single variable, ignore it
            // - if f_j(x) = x^(2k), then keep it if product is concave when fixing all
            //   other factor; since x^(2k) >= 0, it suffices to check that the
            //   activity of the whole product is non-negative if haslhs or
            //   non-positive if hasrhs
            // - remove all other variable expressions from exprcand
            if expr::expr_is_product(scip, child) {
                // Activity has been ensured to be uptodate (or at least still valid) by
                // call to SCIPregisterExprUsageNonlinear() in detectNlhdlrs() in
                // canonicalize.
                let productactivity = scip.exprs[child].activity;

                // Check whether variables in even-powered factor terms can be
                // restricted to bounds (as in SCIPisExprPower() below).
                let keepevenpower = (haslhs && productactivity.inf >= 0.0) || (hasrhs && productactivity.sup <= 0.0);

                for j in 0..scip.exprs[child].children.len() {
                    let grandchild = scip.exprs[child].children[j];

                    // If grandchild is x^(2k), then do not remove x from exprcands.
                    if keepevenpower && expr::expr_is_power(scip, grandchild) && expr::expr_is_var(scip, scip.exprs[grandchild].children[0]) {
                        let exponent = crate::expr_pow::get_exponent_expr_pow(scip, grandchild);
                        if exponent > 1.0 && exponent % 2.0 == 0.0 {
                            continue;
                        }
                    }

                    if !expr::expr_is_var(scip, grandchild) {
                        // Mark all variable expressions that are contained in the
                        // expression.
                        remove_single_locked_vars(scip, grandchild, &mut it, &mut exprcands);
                    }
                }
            }
            // Fixing a variable x to one of its bounds is only valid for ... +x^p >=
            // lhs or ... -x^p <= rhs if p = 2k for an integer k >= 1.
            else if expr::expr_is_power(scip, child) {
                let grandchild = scip.exprs[child].children[0];
                let exponent = crate::expr_pow::get_exponent_expr_pow(scip, child);

                // Check for even integral exponent.
                let valid = exponent > 1.0 && exponent % 2.0 == 0.0;

                if !valid || !expr::expr_is_var(scip, grandchild) || (hasrhs && coef > 0.0) || (haslhs && coef < 0.0) {
                    // Mark all variable expressions that are contained in the expression.
                    remove_single_locked_vars(scip, grandchild, &mut it, &mut exprcands);
                }
            }
            // All other cases cannot be handled.
            else {
                // Mark all variable expressions that are contained in the expression.
                remove_single_locked_vars(scip, child, &mut it, &mut exprcands);
            }
        }

        // Free expression iterator.
        it.free(scip);
    }

    // Check whether the bound disjunction constraint handler is available.
    let hasbounddisj = crate::set::set_find_conshdlr(scip, "bounddisjunction") != NONE;
    let checkvarlocks = hd(scip, conshdlr).checkvarlocks;

    // Fix variable to one of its bounds by either changing its variable type or
    // adding a disjunction constraint.
    for &sl in &singlelocked {
        // Only consider expressions that are still contained in the exprcands map.
        if exprcands.contains(&sl) {
            let var = crate::expr_var::get_var_expr_var(scip, sl);
            let (glb, gub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);

            // Try to change the variable type to binary.
            if checkvarlocks == b't' && scip.set.is_eq(glb, 0.0) && scip.set.is_eq(gub, 1.0) {
                *infeasible = crate::scip_var::chg_var_type(scip, var, crate::types::VarType::Binary)?;
                *nchgvartypes += 1;

                if *infeasible {
                    break;
                }
            }
            // Add bound disjunction constraint if bounds of the variable are finite.
            else if hasbounddisj && !scip.set.is_infinity(-glb) && !scip.set.is_infinity(gub) {
                let vars = [var, var];
                let boundtypes = [crate::types::BoundType::Lower, crate::types::BoundType::Upper];
                let bounds = [gub, glb];

                // Create, add, and release bound disjunction constraint.
                let name = format!("quadvarbnddisj_{}", scip.vars[var].name);
                let flags = crate::cons::ConsFlags {
                    initial: true,
                    separate: true,
                    enforce: true,
                    check: false,
                    propagate: true,
                    local: false,
                    modifiable: false,
                    dynamic: false,
                    removable: false,
                    stickingatnode: false,
                };
                let mut newcons = crate::cons_bounddisjunction::create_cons_bounddisjunction(scip, &name, &vars, &boundtypes, &bounds, &flags)?;
                crate::scip_cons::add_cons(scip, newcons)?;
                crate::scip_cons::release_cons(scip, &mut newcons)?;
                *naddconss += 1;
            }
        }
    }

    Ok(())
}

/// `presolveImplint`: presolving method to check if there is a single
/// linear continuous variable that can be made implicit integer.
fn presolve_implint(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId], nchgvartypes: &mut i32, infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    // Nothing can be done on purley continuous problem.
    if crate::api::get_n_vars(scip) == crate::api::get_n_cont_vars(scip) {
        return Ok(());
    }

    // No continuous var can be made implicit-integer if there are no
    // continuous variables.
    if crate::api::get_n_cont_vars(scip) == 0 {
        return Ok(());
    }

    for &cons in conss {
        let (lhs, rhs, consexpr) = {
            let c = cd(scip, cons);
            (c.lhs, c.rhs, c.expr)
        };

        // The constraint must be an equality constraint.
        if !scip.set.is_eq(lhs, rhs) {
            continue;
        }

        // The root expression needs to be a sum expression.
        if !expr::expr_is_sum(scip, consexpr) {
            continue;
        }

        let children = scip.exprs[consexpr].children.clone();
        let nchildren = children.len();

        // The sum expression must have at least two children (with one child, we
        // would look for a coef*x = constant, which is presolved away anyway).
        if nchildren <= 1 {
            continue;
        }

        let coefs = crate::expr_sum::get_coefs_expr_sum(scip, consexpr).to_vec();
        let constant = crate::expr_sum::get_constant_expr_sum(scip, consexpr);
        let mut cand = NONE;
        let mut candcoef = 0.0;

        // Find first continuous variable and get value of its coefficient.
        for i in 0..nchildren {
            if !expr::expr_is_var(scip, children[i]) || crate::var::var_is_integral(scip, crate::expr_var::get_var_expr_var(scip, children[i])) {
                continue;
            }

            candcoef = coefs[i];

            // lhs/rhs - constant divided by candcoef must be integral; if not, break
            // with cand == NULL, so give up.
            if scip.set.is_integral((lhs - constant) / candcoef) {
                cand = children[i];
            }

            break;
        }

        // No suitable continuous variable found.
        if cand == NONE {
            continue;
        }

        let mut impltype = crate::types::ImplintType::Strong;

        // Check whether all other coefficients are integral when diving by
        // candcoef and all other children are integral.
        for i in 0..nchildren {
            if children[i] == cand {
                continue;
            }

            let ci = scip.exprs[children[i]].integrality;
            if (ci as i32) < (impltype as i32) {
                impltype = ci;
            }

            // Child i must be integral.
            if impltype == crate::types::ImplintType::None {
                cand = NONE;
                break;
            }

            // Coefficient of child i must be integral if diving by candcoef.
            if !scip.set.is_integral(coefs[i] / candcoef) {
                cand = NONE;
                break;
            }
        }

        if cand == NONE {
            continue;
        }

        // Change variable type.
        let var = crate::expr_var::get_var_expr_var(scip, cand);
        *infeasible = crate::scip_var::chg_var_impl_type(scip, var, impltype)?;
        *nchgvartypes += 1;

        if *infeasible {
            return Ok(());
        }

        // Mark expression as being integral (as would be done by expr_var.c in the
        // next round of updating integrality info).
        scip.exprs[cand].integrality = impltype;
    }

    Ok(())
}

/// `createAuxVar`: creates auxiliary variable for a given expression.
///
/// @note for a variable expression it does nothing
/// @note this function can only be called in stage SCIP_STAGE_SOLVING
fn create_aux_var(scip: &mut Scip, expr: ExprId) -> ScipResult {
    // If we already have auxvar, then do nothing.
    if od(scip, expr).auxvar != NONE {
        return Ok(());
    }

    // If expression is a variable-expression, then do nothing.
    if expr::expr_is_var(scip, expr) {
        return Ok(());
    }

    if scip.set.stage != Stage::Solving {
        log::error!("it is not possible to create auxiliary variables during stage={}", scip.set.stage as i32);
        return Err(Retcode::InvalidCall);
    }

    let conshdlr = od(scip, expr).conshdlr;

    // It doesn't harm much to have an auxvar for a constant, as this can be
    // handled well by the default hdlr, but it usually indicates a missing
    // simplify; if we find situations where we need to have an auxvar for a
    // constant, then remove this assert.

    // Create and capture auxiliary variable.
    let hname = scip.exprhdlrs[scip.exprs[expr].exprhdlr].name.clone();
    let name = format!("auxvar_{hname}_{}", hd(scip, conshdlr).auxvarid);
    hdm(scip, conshdlr).auxvarid += 1;

    // Type of auxiliary variable depends on integrality information of the
    // expression.
    let impltype = scip.exprs[expr].integrality;

    // Get activity of expression to initialize variable bounds, if something
    // valid is available (evalActivity was called in initSepa).
    let mut activity = Interval::default();
    if scip.exprs[expr].activitytag >= hd(scip, conshdlr).lastboundrelax {
        activity = scip.exprs[expr].activity;

        // We cannot handle a domain error here at the moment, but it seems
        // unlikely that it could occur; if it appear, then we could change code to
        // handle this properly, but for now we just ensure that we continue
        // correctly and abort in debug mode only.
        if interval::is_empty(INTERVAL_INFINITY, activity) {
            panic!("SCIPABORT: empty activity for auxiliary variable");
        }
    } else {
        activity.set_entire(INTERVAL_INFINITY);
    }

    // If root node, then activity is globally valid, so use it to initialize
    // the global bounds of the auxvar; otherwise, we create var without bounds
    // here and use activity to set local bounds below (needs to be after adding
    // var).
    let inf = scip.set.infinity();
    let depth = crate::api::get_depth(scip);
    let (lb, ub) = if depth == 0 { (cmax(-inf, activity.inf), cmin(inf, activity.sup)) } else { (-inf, inf) };
    let auxvar = crate::scip_var::create_var_impl(scip, &name, lb, ub, 0.0, crate::types::VarType::Continuous, impltype, true, false)?;
    odm(scip, expr).auxvar = auxvar;

    // Mark the auxiliary variable to be added for the relaxation only; this
    // prevents SCIP to create linear constraints from cuts or conflicts that
    // contain auxiliary variables, or to copy the variable to a subscip.
    crate::var::var_mark_relaxation_only(scip, auxvar);

    crate::scip_var::add_var(scip, auxvar)?;

    // Add variable locks in both directions.
    // TODO should be sufficient to lock only according to expr->nlockspos/neg,
    // but then we need to also update the auxvars locks when the expr locks
    // change.
    crate::api::add_var_locks_type(scip, auxvar, LockType::Model, 1, 1)?;

    if depth > 0 {
        // Initialize local bounds to (locally valid) activity.
        let mut cutoff = false;
        tighten_aux_var_bounds(scip, conshdlr, expr, activity, &mut cutoff, None)?;
    }

    Ok(())
}

/// `initSepa`: initializes separation for constraint.
///
/// - ensures that activities are up to date in all expressions
/// - creates auxiliary variables where required
/// - calls propExprDomains() to possibly tighten auxvar bounds
/// - calls separation initialization callback of nlhdlrs
fn init_sepa(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    let mut nreductions = 0;

    // Start with new propbounds (just to be sure, should not be needed).
    hdm(scip, conshdlr).curpropboundstag += 1;

    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;

    // First ensure activities are up to date and create auxvars.
    *infeasible = false;
    for &cons in conss {
        let (consexpr, lhs, rhs) = {
            let c = cd(scip, cons);
            (c.expr, c.lhs, c.rhs)
        };

        // Ensure we have a valid activity for auxvars and propExprDomains() call
        // below.
        expr::expr_eval_activity(scip, consexpr)?;

        let mut e = it.restart_dfs(scip, consexpr);
        while !it.is_end() {
            if od(scip, e).nauxvaruses > 0 {
                create_aux_var(scip, e)?;
            }
            e = it.get_next(scip);
        }

        let auxvar = od(scip, consexpr).auxvar;
        if auxvar != NONE {
            // Change the bounds of the auxiliary variable of the root node to
            // [lhs,rhs].
            let (inf, _) = crate::scip_var::tighten_var_lb(scip, auxvar, lhs, true)?;
            *infeasible = inf;
            if *infeasible {
                break;
            }

            let (inf, _) = crate::scip_var::tighten_var_ub(scip, auxvar, rhs, true)?;
            *infeasible = inf;
            if *infeasible {
                break;
            }
        }
    }

    // Now run a special version of reverseprop to ensure that important bound
    // information (like function domains) is stored in bounds of auxvars, since
    // sometimes they cannot be recovered from activity evaluation even after
    // some rounds of domain propagation (e.g., log(x*y), which becomes log(w),
    // w=x*y; log(w) implies w >= 0, but we may not be able to derive bounds on
    // x and y such that w >= 0 is ensured).
    let mut result = crate::types::ScipResultCode::DidNotRun;
    prop_expr_domains(scip, conshdlr, conss, &mut result, &mut nreductions)?;
    if result == crate::types::ScipResultCode::Cutoff {
        *infeasible = true;
    }

    // Now call initsepa of nlhdlrs.
    // TODO skip if !SCIPconsIsInitial(conss[c]) ? but at the moment, initSepa()
    // is called from INITLP anyway, so we have SCIPconsIsInitial(conss[c])
    // anyway.
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
    let mut c = 0;
    while c < conss.len() && !*infeasible {
        let cons = conss[c];
        c += 1;

        let consexpr = cd(scip, cons).expr;
        let mut e = it.restart_dfs(scip, consexpr);
        while !it.is_end() && !*infeasible {
            if od(scip, e).nauxvaruses == 0 {
                e = it.get_next(scip);
                continue;
            }

            for k in 0..od(scip, e).nenfos.max(0) as usize {
                let o = od(scip, e);
                let enfo = o.enfos[k];

                // Skip if initsepa was already called, e.g., because this expression is
                // also part of a constraint which participated in a previous initSepa()
                // call.
                if enfo.issepainit {
                    continue;
                }

                // Only call initsepa if it will actually separate.
                if enfo.nlhdlrparticipation & nlhdlr::NLHDLR_METHOD_SEPABOTH == 0 {
                    continue;
                }

                // Only init sepa if there is an initsepa callback.
                if scip.nlhdlrs[enfo.nlhdlr].initsepa.is_none() {
                    continue;
                }

                // Check whether expression needs to be under- or overestimated.
                let overestimate = o.nlocksneg > 0;
                let underestimate = o.nlockspos > 0;

                // Call the separation initialization callback of the nonlinear handler.
                nlhdlr::nlhdlr_initsepa(scip, conshdlr, cons, enfo.nlhdlr, e, enfo.nlhdlrexprdata, overestimate, underestimate, infeasible)?;
                odm(scip, e).enfos[k].issepainit = true;

                if *infeasible {
                    // Stop everything if we detected infeasibility.
                    break;
                }
            }

            e = it.get_next(scip);
        }
    }

    it.free(scip);

    Ok(())
}

/// `branchAuxNonlinear`: returns whether we are ok to branch on auxiliary
/// variables.
///
/// Currently returns whether depth of node in B&B tree is at least value of
/// constraints/nonlinear/branching/aux parameter.
fn branch_aux_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> bool {
    hd(scip, conshdlr).branchauxmindepth <= crate::api::get_depth(scip)
}

/// `getViolSplitWeight`: gets weight of variable when splitting violation
/// score onto several variables in an expression.
fn get_viol_split_weight(scip: &Scip, conshdlr: ConshdlrId, var: VarId, sol: SolId) -> f64 {
    let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

    match hd(scip, conshdlr).branchviolsplit {
        // Uniform: everyone gets the same score.
        b'u' => 1.0,
        // Midness of solution: 0.5 if in middle of domain, 0.05 if close to lower
        // or upper bound.
        b'm' => {
            let solval = crate::api::get_sol_val(scip, sol, var);
            let weight = cmin(solval - lb, ub - solval) / (ub - lb);
            cmax(0.05, weight)
        }
        // Domain width.
        b'd' => ub - lb,
        // Logarithmic domain width: log-scale if width is below 0.1 or above 10,
        // otherwise actual width.
        b'l' => {
            let width = ub - lb;
            if width > 10.0 {
                return 10.0 * width.log10();
            }
            if width < 0.1 {
                return 0.1 / (-width.log10());
            }
            width
        }
        _ => {
            log::error!("invalid value for parameter constraints/expr/branching/violsplit");
            panic!("SCIPABORT");
        }
    }
}

/// `addExprsViolScore`: adds violation-branching score to a set of
/// expressions, thereby distributing the score.
///
/// Each expression must either be a variable expression or have an
/// aux-variable.
///
/// If unbounded variables are present, each unbounded var gets an even
/// score. If no unbounded variables, then parameter
/// constraints/nonlinear/branching/violsplit decides weight for each var.
fn add_exprs_viol_score(scip: &mut Scip, exprs: &[ExprId], violscore: f64, sol: SolId) -> bool {
    let mut weightsum = 0.0; // Sum of weights over all candidates with bounded domain.
    let mut nunbounded = 0; // Number of candidates with unbounded domain.

    if exprs.len() == 1 {
        add_expr_viol_score_nonlinear(scip, exprs[0], violscore);
        return true;
    }

    if exprs.is_empty() {
        return false;
    }

    let conshdlr = od(scip, exprs[0]).conshdlr;

    for &e in exprs {
        let var = get_expr_aux_var_nonlinear(scip, e);
        let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

        if scip.set.is_infinity(-lb) || scip.set.is_infinity(ub) {
            nunbounded += 1;
        } else if !scip.set.is_eq(lb, ub) {
            weightsum += get_viol_split_weight(scip, conshdlr, var, sol);
        }
    }

    let mut success = false;
    for &e in exprs {
        let var = get_expr_aux_var_nonlinear(scip, e);
        let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

        if nunbounded > 0 {
            if scip.set.is_infinity(-lb) || scip.set.is_infinity(ub) {
                add_expr_viol_score_nonlinear(scip, e, violscore / nunbounded as f64);
                success = true;
            }
        } else if !scip.set.is_eq(lb, ub) {
            let weight = get_viol_split_weight(scip, conshdlr, var, sol);
            add_expr_viol_score_nonlinear(scip, e, violscore * weight / weightsum);
            success = true;
        }
        // Else: skip score for fixed variable.
    }

    success
}

/// `addExprViolScoresAuxVars`: adds violation-branching score to children
/// of expression for given auxiliary variables.
///
/// Iterates over the successors of `expr` to find expressions that are
/// associated with one of the given auxiliary variables. Adds
/// violation-branching scores to all found exprs by means of
/// SCIPaddExprsViolScoreNonlinear().
///
/// @note This method may modify the given auxvars array by means of sorting.
fn add_expr_viol_scores_aux_vars(scip: &mut Scip, expr: ExprId, violscore: f64, auxvars: &mut [VarId], sol: SolId) -> ScipResult<bool> {
    // Sort variables to make lookup below faster.
    crate::sort::sort(auxvars, (), |&a, &b| crate::var::var_comp(scip, a, b));

    let mut it = expr::ExprIter::create();
    it.init(scip, expr, expr::ExprIterType::Bfs, false)?;

    let mut exprs = Vec::with_capacity(auxvars.len());
    let mut e = it.get_next(scip);
    while !it.is_end() {
        let auxvar = get_expr_aux_var_nonlinear(scip, e);
        if auxvar != NONE {
            // If auxvar of expr is contained in auxvars array, add branching score to
            // expr.
            if crate::sort::sortedvec_find(auxvars, |&a, &b| crate::var::var_comp(scip, a, b), auxvar).is_ok() {
                exprs.push(e);

                if exprs.len() == auxvars.len() {
                    break;
                }
            }
        }
        e = it.get_next(scip);
    }

    it.free(scip);

    add_exprs_viol_score_nonlinear(scip, &exprs, violscore, sol)
}

/// `registerBranchingCandidatesAllUnfixed`: registers all unfixed variables
/// in violated constraints as branching candidates; returns the number of
/// notifications performed.
fn register_branching_candidates_all_unfixed(scip: &mut Scip, _conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult<i32> {
    let mut nnotify = 0;

    for &cons in conss {
        // Consider only violated constraints.
        if !is_cons_violated(scip, cons) {
            continue;
        }

        // Register all variables that have not been fixed yet.
        let varexprs = cd(scip, cons).varexprs.clone().unwrap_or_default();
        for ve in varexprs {
            let var = crate::expr_var::get_var_expr_var(scip, ve);
            if !scip.set.is_eq(scip.vars[var].locdom.lb, scip.vars[var].locdom.ub) {
                let viol = get_cons_abs_violation(scip, cons);
                crate::api::add_extern_branch_cand(scip, var, viol, INVALID);
                nnotify += 1;
            }
        }
    }

    Ok(nnotify)
}

/// `SCIPaddExprViolScoreNonlinear`: adds violation-branching score to an
/// expression.
///
/// Adds a score to the expression-specific violation-branching score,
/// thereby marking it as branching candidate. The expression must either be
/// a variable expression or have an aux-variable. In the latter case,
/// branching on auxiliary variables must have been enabled. In case of doubt,
/// use SCIPaddExprsViolScoreNonlinear(). Roughly, the difference between these
/// functions is that the current function adds `violscore` to the expression
/// directly, while SCIPaddExprsViolScoreNonlinear() will split the violation
/// score among all the given expressions according to parameter
/// constraints/nonlinear/branching/violsplit.
pub fn add_expr_viol_score_nonlinear(scip: &mut Scip, expr: ExprId, violscore: f64) {
    let conshdlr = od(scip, expr).conshdlr;
    let enforound = hd(scip, conshdlr).enforound;
    let o = odm(scip, expr);

    // If not allowing to branch on auxvars, then expr must be a var-expr; if
    // allowing to branch on auxvars, then expr must be a var-expr or have an
    // auxvar.

    // Reset branching score if we are in a different enfo round.
    if o.violscoretag != enforound {
        o.violscoresum = violscore;
        o.violscoremax = violscore;
        o.nviolscores = 1;
        o.violscoretag = enforound;
        return;
    }

    o.violscoresum += violscore;
    if violscore > o.violscoremax {
        o.violscoremax = violscore;
    }
    o.nviolscores += 1;
}

/// `SCIPaddExprsViolScoreNonlinear`: adds violation-branching score to a set
/// of expressions, distributing the score among all the expressions; returns
/// whether at least one violscore was added.
///
/// Each expression must either be a variable expression or have an
/// aux-variable. If branching on aux-variables is disabled, then the
/// violation branching score will be distributed among all variables present
/// in `exprs`.
pub fn add_exprs_viol_score_nonlinear(scip: &mut Scip, exprs: &[ExprId], violscore: f64, sol: SolId) -> ScipResult<bool> {
    if exprs.is_empty() {
        return Ok(false);
    }

    // If allowing to branch on auxiliary variables, then call internal
    // addConsExprExprsViolScore immediately.
    if branch_aux_nonlinear(scip, od(scip, exprs[0]).conshdlr) {
        return Ok(add_exprs_viol_score(scip, exprs, violscore, sol));
    }

    // If not allowing to branch on aux vars, then create new array containing
    // var expressions that exprs depend on.
    let mut varexprs = Vec::with_capacity(5);

    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;

    for &root in exprs {
        let mut e = it.restart_dfs(scip, root);
        while !it.is_end() {
            if expr::expr_is_var(scip, e) {
                // Add variable expression to vars array.
                varexprs.push(e);
            }
            e = it.get_next(scip);
        }
    }

    it.free(scip);

    Ok(add_exprs_viol_score(scip, &varexprs, violscore, sol))
}

/// `SCIPgetExprViolScoreNonlinear`: gives violation-branching score stored
/// in expression, or 0.0 if no valid score has been stored.
pub fn get_expr_viol_score_nonlinear(scip: &Scip, expr: ExprId) -> f64 {
    let o = od(scip, expr);
    let h = hd(scip, o.conshdlr);

    if h.enforound != o.violscoretag {
        return 0.0;
    }

    if o.nviolscores == 0 {
        return 0.0;
    }

    match h.branchscoreagg {
        // Average.
        b'a' => o.violscoresum / o.nviolscores as f64,
        // Maximum.
        b'm' => o.violscoremax,
        // Sum.
        b's' => o.violscoresum,
        c => {
            log::error!("Invalid value {} for branchscoreagg parameter", c as char);
            panic!("SCIPABORT");
        }
    }
}

/// `SCIPgetExprPartialDiffNonlinear`: returns the partial derivative of an
/// expression w.r.t. a variable (or SCIP_INVALID if there was an evaluation
/// error).
pub fn get_expr_partial_diff_nonlinear(scip: &Scip, expr: ExprId, var: VarId) -> f64 {
    // Return 0.0 for value expression.
    if expr::expr_is_value(scip, expr) {
        return 0.0;
    }

    // Check if an error occurred during the last SCIPevalExprGradient() call.
    if scip.exprs[expr].derivative == INVALID {
        return INVALID;
    }

    let conshdlr = od(scip, expr).conshdlr;

    // Use variable to expressions mapping which is stored in the constraint
    // handler data.
    let varexpr = hd(scip, conshdlr).var2expr[&var];

    // Use difftag to decide whether the variable belongs to the expression.
    if scip.exprs[expr].difftag != scip.exprs[varexpr].difftag {
        0.0
    } else {
        scip.exprs[varexpr].derivative
    }
}

/// `SCIPgetExprPartialDiffGradientDirNonlinear`: returns the var's
/// coordinate of Hu partial derivative of an expression w.r.t. a variable
/// (or SCIP_INVALID if there was an evaluation error).
pub fn get_expr_partial_diff_gradient_dir_nonlinear(scip: &Scip, expr: ExprId, var: VarId) -> f64 {
    // Return 0.0 for value expression.
    if expr::expr_is_value(scip, expr) {
        return 0.0;
    }

    // Check if an error occurred during the last SCIPevalExprHessianDir() call.
    if scip.exprs[expr].bardot == INVALID {
        return INVALID;
    }

    let conshdlr = od(scip, expr).conshdlr;

    // Use variable to expressions mapping which is stored in the constraint
    // handler data; if this fails it means that we are asking for the var's
    // component of H*u for a var that doesn't appear in any nonlinear
    // constraint, so maybe we can also just return 0.0.
    let varexpr = hd(scip, conshdlr).var2expr[&var];

    // Use difftag to decide whether the variable belongs to the expression.
    if scip.exprs[expr].difftag != scip.exprs[varexpr].difftag {
        0.0
    } else {
        scip.exprs[varexpr].bardot
    }
}

/// `SCIPevalExprQuadraticAuxNonlinear`: evaluates quadratic term in a
/// solution w.r.t. auxiliary variables.
///
/// \note This requires that for every expr used in the quadratic data, a
/// variable or auxiliary variable is available.
pub fn eval_expr_quadratic_aux_nonlinear(scip: &Scip, expr: ExprId, sol: SolId) -> f64 {
    let q = expr::expr_get_quadratic_data(scip, expr);
    let val = |e: ExprId| crate::api::get_sol_val(scip, sol, get_expr_aux_var_nonlinear(scip, e));

    let mut auxvalue = q.constant;

    // Linear terms.
    for i in 0..q.linexprs.len() {
        auxvalue += q.lincoefs[i] * val(q.linexprs[i]);
    }

    // Quadratic terms.
    for t in &q.quadexprterms {
        let solval = val(t.expr);
        auxvalue += (t.lincoef + t.sqrcoef * solval) * solval;
    }

    // Bilinear terms.
    for t in &q.bilinexprterms {
        auxvalue += t.coef * val(t.expr1) * val(t.expr2);
    }

    auxvalue
}

/// `registerBranchingCandidates`: registers all variables in violated
/// constraints with branching scores as external branching candidates;
/// returns whether at least one branching candidate was added.
fn register_branching_candidates(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult<bool> {
    let mut success = false;
    let branchaux = branch_aux_nonlinear(scip, conshdlr);

    let mut it = None;
    if branchaux {
        let mut i = expr::ExprIter::create();
        i.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
        it = Some(i);
    }

    // Register external branching candidates.
    for &cons in conss {
        // Consider only violated constraints.
        if !is_cons_violated(scip, cons) {
            continue;
        }

        if !branchaux {
            // If not branching on auxvars, then violation-branching scores will have
            // been added to original variables only, so we can loop over variable
            // expressions.
            let varexprs = cd(scip, cons).varexprs.clone().unwrap_or_default();
            for ve in varexprs {
                let violscore = get_expr_viol_score_nonlinear(scip, ve);

                // Skip variable expressions that do not have a violation score.
                if violscore == 0.0 {
                    continue;
                }

                let var = crate::expr_var::get_var_expr_var(scip, ve);
                let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

                // Consider variable for branching if it has not been fixed yet.
                if !scip.set.is_eq(lb, ub) {
                    crate::api::add_extern_branch_cand(scip, var, violscore, INVALID);
                    success = true;
                }

                // Invalidate violscore-tag, so that we do not register variables that
                // appear in multiple constraints several times as external branching
                // candidate, see SCIPgetExprViolScoreNonlinear().
                odm(scip, ve).violscoretag = 0;
            }
        } else {
            let it = it.as_mut().unwrap();
            let consexpr = cd(scip, cons).expr;
            let mut e = it.restart_dfs(scip, consexpr);
            while !it.is_end() {
                let violscore = get_expr_viol_score_nonlinear(scip, e);
                if violscore != 0.0 {
                    // If some nlhdlr added a branching score for this expression, then it
                    // considered this expression as a variable, so this expression should
                    // either be an original variable or have an auxiliary variable.
                    let var = get_expr_aux_var_nonlinear(scip, e);
                    let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

                    // Consider variable for branching if it has not been fixed yet.
                    if !scip.set.is_eq(lb, ub) {
                        crate::api::add_extern_branch_cand(scip, var, violscore, INVALID);
                        success = true;
                    }
                }
                e = it.get_next(scip);
            }
        }
    }

    if let Some(it) = it {
        it.free(scip);
    }

    Ok(success)
}

/// `collectBranchingCandidates`: collect branching candidates from
/// violated constraints.
///
/// Fills array with expressions that serve as branching candidates. Collects
/// those expressions that have a branching score assigned and stores the
/// score in the auxviol field of the branching candidate.
///
/// If branching on aux-variables is allowed, then iterate through
/// expressions of violated constraints, otherwise iterate through
/// variable-expressions only.
fn collect_branching_candidates(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], maxrelconsviol: f64, sol: SolId, soltag: i64, cands: &mut Vec<BranchCand>) -> ScipResult {
    let branchaux = branch_aux_nonlinear(scip, conshdlr);

    let mut it = None;
    if branchaux {
        let mut i = expr::ExprIter::create();
        i.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
        it = Some(i);
    }

    cands.clear();

    for attempt in 0..2 {
        // Collect branching candidates from violated constraints; in the first
        // attempt, consider only constraints with large violation; in the second
        // attempt, consider all remaining violated constraints.
        for &cons in conss {
            // Consider only violated constraints.
            if !is_cons_violated(scip, cons) {
                continue;
            }

            let consviol = get_cons_rel_violation(scip, cons, sol, soltag)?;

            let high = hd(scip, conshdlr).branchhighviolfactor * maxrelconsviol;
            if (attempt == 0 && consviol < high) || (attempt == 1 && consviol >= high) {
                continue;
            }

            let enforound = hd(scip, conshdlr).enforound;

            if !branchaux {
                // If not branching on auxvars, then violation-branching scores will be
                // available for original variables only, so we can loop over variable
                // expressions; unfortunately, we don't know anymore which constraint
                // contributed the violation-branching score to the variable, therefore
                // we invalidate the score of a variable after processing it.
                let varexprs = cd(scip, cons).varexprs.clone().unwrap_or_default();
                for ve in varexprs {
                    // Skip variable expressions that do not have a valid violation score.
                    if enforound != od(scip, ve).violscoretag {
                        continue;
                    }

                    let var = crate::expr_var::get_var_expr_var(scip, ve);
                    let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

                    // Skip already fixed variable.
                    if scip.set.is_eq(lb, ub) {
                        continue;
                    }

                    let auxviol = get_expr_viol_score_nonlinear(scip, ve);
                    cands.push(BranchCand {
                        expr: ve,
                        var,
                        auxviol,
                        fractionality: 0.0,
                        ..Default::default()
                    });

                    // Invalidate violscore-tag, so that we do not register variables that
                    // appear in multiple constraints several times as external branching
                    // candidate.
                    odm(scip, ve).violscoretag = 0;
                }
            } else {
                let it = it.as_mut().unwrap();
                let consexpr = cd(scip, cons).expr;
                let mut e = it.restart_dfs(scip, consexpr);
                while !it.is_end() {
                    if od(scip, e).violscoretag == enforound {
                        // If some nlhdlr added a branching score for this expression, then it
                        // considered this expression as variables, so this expression should
                        // either be an original variable or have an auxiliary variable.
                        let var = get_expr_aux_var_nonlinear(scip, e);
                        let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

                        // Skip already fixed variable.
                        if !scip.set.is_eq(lb, ub) {
                            let auxviol = get_expr_viol_score_nonlinear(scip, e);
                            cands.push(BranchCand {
                                expr: e,
                                var,
                                auxviol,
                                fractionality: 0.0,
                                ..Default::default()
                            });
                        }
                    }
                    e = it.get_next(scip);
                }
            }
        }

        // If we have branching candidates, then we don't need another attempt.
        if !cands.is_empty() {
            break;
        }
    }

    if let Some(it) = it {
        it.free(scip);
    }

    Ok(())
}

/// `getDualBranchscore`: computes a branching score for a variable that
/// reflects how important branching on this variable would be for improving
/// the dual bound from the LP relaxation.
///
/// See cons_nonlinear.c for the derivation: we sum the values lambda_i *
/// estimategap_i for all rows from this constraint handler in which the
/// variable appears and divide by the LP objective value (if >1).
fn get_dual_branchscore(scip: &mut Scip, conshdlr: ConshdlrId, var: VarId) -> f64 {
    // If LP not solved, then the dual branching score is not available.
    if crate::api::get_lp_solstat(scip) != crate::types::LpSolStat::Optimal {
        return 0.0;
    }

    // If var is not in the LP, then the dual branching score is not available.
    if scip.vars[var].varstatus != crate::types::VarStatus::Column {
        return 0.0;
    }

    let col = scip.vars[var].data.col;
    if scip.cols[col].lppos < 0 {
        return 0.0;
    }

    // TODO there is a big warning on when not to use this method; is the check
    // for SCIPcolIsInLP sufficient?
    let nrows = scip.cols[col].nlprows as usize;
    let rows = scip.cols[col].rows.0[..nrows].to_vec();

    // Aggregate duals from all rows from consexpr with non-zero dual.
    // TODO: this is a quick-and-dirty implementation, and not used by default;
    // in the long run, this should be either removed or replaced by a proper
    // implementation.
    let mut dualscore = 0.0;
    for r in rows {
        let row = &scip.rows[r];

        // Rows from cuts that may be replaced by tighter ones after branching are
        // the interesting ones; these would typically be local, unless they are
        // created at the root node, so not check for local now, but trust that
        // estimators that do not improve after branching will have an estimategap
        // of 0.
        if row.origintype != crate::types::RowOriginType::Cons || row.origin != conshdlr {
            continue;
        }

        if scip.set.is_zero(row.dualsol) {
            continue;
        }

        // Gap not stored, maybe because it was 0.
        let Some(p) = row.name.find("_estimategap=") else {
            continue;
        };

        let bytes = row.name.as_bytes();
        let mut estimategap = crate::misc::strtod(bytes, p + 13).map_or(0.0, |(v, _)| v);
        if !estimategap.is_finite() || scip.set.is_huge_value(estimategap) {
            estimategap = scip.set.get_huge_value();
        }

        dualscore += estimategap * row.dualsol.abs();
    }

    // Divide by optimal value of LP for scaling.
    let objval = crate::api::get_lp_objval(scip);
    dualscore /= cmax(1.0, objval.abs());

    dualscore
}

/// The pseudo-cost score of a branching candidate, or SCIP_INVALID (part of
/// `scoreBranchingCandidates`).
fn pscost_score(scip: &Scip, conshdlr: ConshdlrId, cand: &BranchCand, sol: SolId) -> f64 {
    use crate::types::BranchDir;

    let reliable = hd(scip, conshdlr).branchpscostreliable;
    let var = cand.var;
    let pscost = |delta: f64| crate::var::var_get_pseudocost(scip, var, delta);
    let count = |dir: BranchDir| crate::var::var_get_pseudocost_count_current_run(scip, var, dir);

    let (pscostdown, pscostup);
    if cand.expr != NONE {
        let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);
        if scip.set.is_infinity(-lb) || scip.set.is_infinity(ub) {
            return INVALID;
        }

        // Decide how to compute pseudo-cost scores; this should be consistent with
        // the way how pseudo-costs are updated in the core, which is decided by
        // branching/lpgainnormalize for continuous variables and move in LP-value
        // for non-continuous variables.
        let strategy = if !crate::var::var_is_integral(scip, var) {
            hd(scip, conshdlr).branchpscostupdatestrategy
        } else {
            b'l'
        };

        let brpoint = crate::api::get_branching_point(scip, var, INVALID);
        let solval = crate::api::get_sol_val(scip, sol, var);

        // branch_relpscost deems pscosts as reliable, if the pseudo-count is at
        // least something between 1 and 4 or it uses some statistical tests
        // involving SCIPisVarPscostRelerrorReliable. For here, I use a simple
        // #counts >= branchpscostreliable.
        // TODO use SCIPgetVarPseudocostCount() instead?
        pscostdown = if count(BranchDir::Downwards) >= reliable {
            match strategy {
                b's' => pscost(-(ub - crate::scip_var::adjusted_var_lb(scip, var, brpoint))),
                b'd' => pscost(-(crate::scip_var::adjusted_var_ub(scip, var, brpoint) - lb)),
                b'l' => {
                    if scip.set.is_infinity(solval) {
                        INVALID
                    } else if solval <= crate::scip_var::adjusted_var_ub(scip, var, brpoint) {
                        pscost(0.0)
                    } else {
                        pscost(-(solval - crate::scip_var::adjusted_var_ub(scip, var, brpoint)))
                    }
                }
                c => {
                    log::error!("pscost update strategy {} unknown", c as char);
                    INVALID
                }
            }
        } else {
            INVALID
        };

        pscostup = if count(BranchDir::Upwards) >= reliable {
            match strategy {
                b's' => pscost(crate::scip_var::adjusted_var_ub(scip, var, brpoint) - lb),
                b'd' => pscost(ub - crate::scip_var::adjusted_var_lb(scip, var, brpoint)),
                b'l' => {
                    if scip.set.is_infinity(-solval) {
                        INVALID
                    } else if solval >= crate::scip_var::adjusted_var_lb(scip, var, brpoint) {
                        pscost(0.0)
                    } else {
                        pscost(crate::scip_var::adjusted_var_lb(scip, var, brpoint) - solval)
                    }
                }
                c => {
                    log::error!("pscost update strategy {} unknown", c as char);
                    INVALID
                }
            }
        } else {
            INVALID
        };
    } else {
        let solval = crate::api::get_sol_val(scip, sol, var);

        // The calculation for pscostdown/up follows SCIPgetVarPseudocostScore(),
        // i.e., set solvaldelta to the (negated) difference between variable value
        // and rounded down value for pscostdown and different between variable
        // value and rounded up value for pscostup.
        pscostdown = if count(BranchDir::Downwards) >= reliable {
            pscost(scip.set.feas_ceil(solval - 1.0) - solval)
        } else {
            INVALID
        };
        pscostup = if count(BranchDir::Upwards) >= reliable {
            pscost(scip.set.feas_floor(solval + 1.0) - solval)
        } else {
            INVALID
        };
    }

    // TODO if both are valid, we get pscostdown*pscostup, but does this compare
    // well with vars were only pscostdown or pscostup is used? maybe we should
    // use (pscostdown+pscostup)/2 or sqrt(pscostdown*pscostup) ?
    if pscostdown == INVALID && pscostup == INVALID {
        INVALID
    } else if pscostdown == INVALID {
        pscostup
    } else if pscostup == INVALID {
        pscostdown
    } else {
        // Pass NULL for var to avoid multiplication with branch-factor.
        crate::branch::branch_get_score(scip, NONE, pscostdown, pscostup)
    }
}

/// `scoreBranchingCandidates`: computes branching scores (including
/// weighted score) for a set of candidates.
///
/// For each candidate in the array, compute and store the various branching
/// scores (violation, pseudo-costs, vartype, domainwidth). For pseudo-costs,
/// it's possible that the score is not available, in which case
/// cands[c].pscost will be set to SCIP_INVALID.
///
/// For each score, compute the maximum over all candidates.
///
/// Then compute for each candidate a "weighted" score using the weights as
/// specified by parameters and the scores as previously computed, but scale
/// each score to be in [0,1], i.e., divide each score by the maximum score of
/// all candidates. Further divide by the sum of all weights where a score was
/// available (even if the score was 0).
fn score_branching_candidates(scip: &mut Scip, conshdlr: ConshdlrId, cands: &mut [BranchCand], considerfracnl: bool, sol: SolId) {
    let mut maxscore = BranchCand::default();

    let (violw, fracw, domw, dualw, pscw, vtw) = {
        let h = hd(scip, conshdlr);
        (
            h.branchviolweight,
            h.branchfracweight,
            h.branchdomainweight,
            h.branchdualweight,
            h.branchpscostweight,
            h.branchvartypeweight,
        )
    };

    for cand in cands.iter_mut() {
        if violw > 0.0 {
            // cands[c].auxviol was set in collectBranchingCandidates, so only update
            // maxscore here.
            maxscore.auxviol = cmax(maxscore.auxviol, cand.auxviol);
        }

        if fracw > 0.0 && crate::var::var_is_nonimplied_integral(scip, cand.var) {
            // When collecting for branching on fractionality (cands[c].expr == NULL),
            // only fractional integer variables should appear as candidates here and
            // their fractionality should have been recorded in
            // branchingIntegralOrNonlinear.
            if considerfracnl && cand.fractionality == 0.0 {
                // For an integer variable that is subject to spatial branching, we also
                // record the fractionality (but separately from auxviol) if
                // considerfracnl is TRUE; this way, we can give preference to fractional
                // integer nonlinear variables.
                let solval = crate::api::get_sol_val(scip, sol, cand.var);
                let rounded = scip.set.round(solval);
                cand.fractionality = (solval - rounded).abs();
            }

            maxscore.fractionality = cmax(cand.fractionality, maxscore.fractionality);
        } else {
            cand.fractionality = 0.0;
        }

        if domw > 0.0 && cand.expr != NONE {
            let var = get_expr_aux_var_nonlinear(scip, cand.expr);

            // Get domain width, taking infinity at 1e20 on purpose.
            let domainwidth = scip.vars[var].locdom.ub - scip.vars[var].locdom.lb;

            // Domain-score is going to be log(2*infinity / domainwidth) if domain
            // width >= 1 and log(2 * infinity * MAX(epsilon, domainwidth)) for domain
            // width < 1; the idea is to penalize very large and very small domains.
            cand.domain = if domainwidth >= 1.0 {
                (2.0 * scip.set.infinity() / domainwidth).log10()
            } else {
                (2.0 * scip.set.infinity() * cmax(scip.set.epsilon(), domainwidth)).log10()
            };

            maxscore.domain = cmax(cand.domain, maxscore.domain);
        } else {
            cand.domain = 0.0;
        }

        if dualw > 0.0 && cand.expr != NONE {
            let var = get_expr_aux_var_nonlinear(scip, cand.expr);
            cand.dual = get_dual_branchscore(scip, conshdlr, var);
            maxscore.dual = cmax(cand.dual, maxscore.dual);
        } else {
            cand.dual = 0.0;
        }

        if pscw > 0.0 && crate::api::get_n_obj_vars(scip) > 0 {
            cand.pscost = pscost_score(scip, conshdlr, cand, sol);

            if cand.pscost != INVALID {
                maxscore.pscost = cmax(cand.pscost, maxscore.pscost);
            }
        } else {
            cand.pscost = INVALID;
        }

        if vtw > 0.0 {
            cand.vartype = match scip.vars[cand.var].vartype {
                crate::types::VarType::Binary => 1.0,
                crate::types::VarType::Integer => 0.1,
                crate::types::VarType::Continuous => {
                    if crate::var::var_is_implied_integral(scip, cand.var) {
                        0.01
                    } else {
                        0.0
                    }
                }
                #[allow(unreachable_patterns)]
                _ => {
                    log::error!("invalid variable type");
                    panic!("SCIPABORT");
                }
            };
            maxscore.vartype = cmax(cand.vartype, maxscore.vartype);
        }
    }

    // Now compute a weighted score for each candidate from the single scores;
    // the single scores are scaled to be in [0,1] for this.
    for cand in cands.iter_mut() {
        cand.weighted = 0.0;
        let mut weightsum = 0.0;

        if maxscore.auxviol > 0.0 {
            cand.weighted += violw * cand.auxviol / maxscore.auxviol;
            weightsum += violw;
        }

        if maxscore.fractionality > 0.0 {
            cand.weighted += fracw * cand.fractionality / maxscore.fractionality;
            weightsum += fracw;
        }

        if maxscore.domain > 0.0 {
            cand.weighted += domw * cand.domain / maxscore.domain;
            weightsum += domw;
        }

        if maxscore.dual > 0.0 {
            cand.weighted += dualw * cand.dual / maxscore.dual;
            weightsum += dualw;
        }

        if maxscore.pscost > 0.0 {
            // Use pseudo-costs only if available; do not add pscostscore, if not
            // available, also do not add into weightsum.
            if cand.pscost != INVALID {
                cand.weighted += pscw * cand.pscost / maxscore.pscost;
                weightsum += pscw;
            }
        }

        if maxscore.vartype > 0.0 {
            cand.weighted += vtw * cand.vartype / maxscore.vartype;
            weightsum += vtw;
        }

        cand.weighted /= weightsum;
    }
}

/// `branchcandCompare`: compare two branching candidates by their weighted
/// score.
///
/// If weighted score is equal, use variable index of (aux)var; if variables
/// are the same, then use whether variable was added due to nonlinearity or
/// fractionality.
fn branchcand_compare(scip: &Scip, cands: &[BranchCand], ind1: usize, ind2: usize) -> i32 {
    let (a, b) = (&cands[ind1], &cands[ind2]);

    if a.weighted != b.weighted {
        return if a.weighted < b.weighted { -1 } else { 1 };
    }

    if a.var != b.var {
        return scip.vars[a.var].index.wrapping_sub(scip.vars[b.var].index);
    }

    if a.expr != NONE { 1 } else { -1 }
}

/// `selectBranchingCandidate`: picks a candidate from array of branching
/// candidates; returns its index.
fn select_branching_candidate(scip: &mut Scip, conshdlr: ConshdlrId, cands: &mut [BranchCand], considerfracnl: bool, sol: SolId) -> ScipResult<usize> {
    let mut ncands = cands.len();

    if ncands == 1 {
        return Ok(0);
    }

    // If there are more than one candidate, then compute scores and select.

    // Compute additional scores on branching candidates and weighted score.
    score_branching_candidates(scip, conshdlr, cands, considerfracnl, sol);

    // Sort candidates by weighted score.
    let mut perm: Vec<usize> = (0..ncands).collect();
    {
        let s: &Scip = scip;
        crate::sort::sort_down(&mut perm, (), |&a, &b| branchcand_compare(s, cands, a, b));
    }

    // Binary search to find first low-scored (score below branchhighscorefactor
    // * maximal-score) candidate.
    let mut left = 0usize;
    let mut right = ncands - 1;
    let threshold = hd(scip, conshdlr).branchhighscorefactor * cands[perm[0]].weighted;
    while left < right {
        let mid = (left + right) / 2;
        if cands[perm[mid]].weighted >= threshold {
            left = mid + 1;
        } else {
            right = mid;
        }
    }

    if left < ncands {
        if cands[perm[left]].weighted >= threshold {
            ncands = left + 1;
        } else {
            ncands = left;
        }
    }

    if ncands > 1 {
        // Choose at random from candidates 0..ncands-1.
        if hd(scip, conshdlr).branchrandnumgen.is_none() {
            let seed = scip.set.initialize_random_seed(BRANCH_RANDNUMINITSEED);
            hdm(scip, conshdlr).branchrandnumgen = Some(RandNumGen::new(seed));
        }
        let c = hdm(scip, conshdlr).branchrandnumgen.as_mut().unwrap().get_int(0, ncands as i32 - 1);
        Ok(perm[c as usize])
    } else {
        Ok(perm[0])
    }
}

/// `branching`: do spatial branching or register branching candidates.
fn branching(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], maxrelconsviol: f64, sol: SolId, soltag: i64, result: &mut crate::types::ScipResultCode) -> ScipResult {
    use crate::types::ScipResultCode;

    *result = ScipResultCode::DidNotFind;

    if hd(scip, conshdlr).branchexternal {
        // Just register branching candidates as external.
        if register_branching_candidates(scip, conshdlr, conss)? {
            *result = ScipResultCode::Infeasible;
        }
        return Ok(());
    }

    // Collect branching candidates and their auxviol-score.
    let mut cands = Vec::new();
    collect_branching_candidates(scip, conshdlr, conss, maxrelconsviol, sol, soltag, &mut cands)?;

    // If no unfixed branching candidate in all violated constraint, then it's
    // probably numerics that prevented us to separate or decide a cutoff; we
    // will return here and let the fallbacks in consEnfo() decide how to
    // proceed.
    if cands.is_empty() {
        return Ok(());
    }

    // Here we include fractionality of integer variables into the branching
    // score, but if we know there will be no fractional integer variables, then
    // we can shortcut and turn this off.
    let considerfracnl = sol == NONE && crate::api::get_n_lp_branch_cands(scip) > 0;
    let selected = select_branching_candidate(scip, conshdlr, &mut cands, considerfracnl, sol)?;

    let var = cands[selected].var;
    let point = crate::api::get_branching_point(scip, var, INVALID);
    let children = crate::api::branch_var_val(scip, var, point)?;

    *result = if children.downchild != NONE || children.eqchild != NONE || children.upchild != NONE {
        ScipResultCode::Branched
    } else {
        // If there are no children, then variable should have been fixed by
        // SCIPbranchVarVal.
        ScipResultCode::ReducedDom
    };

    Ok(())
}

/// `SCIPgetExprAbsOrigViolationNonlinear`: computes absolute violation for
/// auxvar relation in an expression w.r.t. original variables, evaluating
/// the expression if necessary.
pub fn get_expr_abs_orig_violation_nonlinear(scip: &mut Scip, expr: ExprId, sol: SolId, soltag: i64, violunder: Option<&mut bool>, violover: Option<&mut bool>) -> ScipResult<f64> {
    // Make sure expression has been evaluated.
    expr::expr_eval(scip, expr, sol, soltag)?;

    // Get violation from internal method.
    Ok(get_expr_abs_orig_violation(scip, expr, sol, violunder, violover))
}

/// `SCIPgetExprAbsAuxViolationNonlinear`: computes absolute violation for
/// auxvar relation in an expression w.r.t. auxiliary variables.
pub fn get_expr_abs_aux_violation_nonlinear(scip: &Scip, expr: ExprId, auxvalue: f64, sol: SolId, violunder: Option<&mut bool>, violover: Option<&mut bool>) -> f64 {
    get_expr_abs_aux_violation(scip, expr, auxvalue, sol, violunder, violover)
}

/// `SCIPgetExprRelAuxViolationNonlinear`: computes relative violation for
/// auxvar relation in an expression w.r.t. auxiliary variables, that is, the
/// absolute violation divided by max(1,|f(w)|).
pub fn get_expr_rel_aux_violation_nonlinear(scip: &Scip, expr: ExprId, auxvalue: f64, sol: SolId, violunder: Option<&mut bool>, violover: Option<&mut bool>) -> f64 {
    // Get violation from internal method.
    let mut viol = get_expr_abs_aux_violation(scip, expr, auxvalue, sol, violunder, violover);

    if !scip.set.is_infinity(viol) {
        // TODO maybe we should rather use max(eps,|auxvalue|)?
        viol /= cmax(1.0, auxvalue.abs());
    }

    viol
}

/// `SCIPprocessRowprepNonlinear`: processes a rowprep for cut addition and
/// maybe report branchscores.
#[allow(clippy::too_many_arguments)]
pub fn process_rowprep_nonlinear(
    scip: &mut Scip,
    nlhdlr: NlhdlrId,
    cons: ConsId,
    expr: ExprId,
    rowprep: &mut crate::rowprep::RowPrep,
    overestimate: bool,
    auxvar: VarId,
    auxvalue: f64,
    allowweakcuts: bool,
    mut branchscoresuccess: bool,
    inenforcement: bool,
    sol: SolId,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    use crate::types::ScipResultCode;

    let mut auxvarvalue = INVALID;
    let mut estimateval = INVALID;

    // Decide on minimal violation of cut.
    let mincutviolation = if sol == NONE {
        // We enforce an LP solution.
        crate::lp::lp_get_feastol(scip)
    } else {
        scip.set.feastol()
    };

    let conshdlr = cons_hdlr(scip, cons);
    let (weakcutthreshold, strongcutmaxcoef, strongcutefficacy, forcestrongcut, rownotremovable, branchdualweight) = {
        let h = hd(scip, conshdlr);
        (
            h.weakcutthreshold,
            h.strongcutmaxcoef,
            h.strongcutefficacy,
            h.forcestrongcut,
            h.rownotremovable,
            h.branchdualweight,
        )
    };

    let mut sepasuccess = true;

    let mut cutviol = rowprep.get_violation(scip, sol, None);
    if cutviol > 0.0 {
        auxvarvalue = crate::api::get_sol_val(scip, sol, auxvar);

        // Check whether cut is weak (if f(x) not defined, then it's never weak).
        if !allowweakcuts && auxvalue != INVALID {
            // Let the estimator be c'x-b, the auxvar is z (=auxvarvalue), and the
            // expression is f(x) (=auxvalue); see cons_nonlinear.c for when this is
            // a weak cut; when linearizing convex expressions, then we should have
            // c'x-b = f(x), so they would never be weak.
            if (!overestimate && (cutviol <= weakcutthreshold * (auxvalue - auxvarvalue))) || (overestimate && (-cutviol >= weakcutthreshold * (auxvalue - auxvarvalue))) {
                sepasuccess = false;
            }
        }

        // Save estimator value for later, see long comment above why this gives
        // the value for c'x-b.
        estimateval = auxvarvalue + if !overestimate { cutviol } else { -cutviol };
    } else {
        sepasuccess = false;
    }

    // Clean up estimator.
    if sepasuccess {
        // If not allowweakcuts, then do not attempt to get cuts more violated by
        // scaling them up, instead, may even scale them down, that is, scale so
        // that max coef is close to 1.
        if !allowweakcuts {
            rowprep.cleanup2(scip, sol, strongcutmaxcoef, Some(&mut sepasuccess));

            if sepasuccess {
                cutviol = rowprep.get_violation(scip, sol, Some(&mut sepasuccess));

                if sepasuccess {
                    sepasuccess = cutviol > mincutviolation;
                }
            }

            if sepasuccess && auxvalue != INVALID {
                // Check whether cut is weak now; auxvar z may now have a coefficient due
                // to scaling (down) in cleanup - take this into account when
                // reconstructing estimateval from cutviol (TODO improve or remove?).
                let mut auxvarcoef = 0.0;

                // Get absolute value of coef of auxvar in row - this makes the whole
                // check here more expensive than it should be...
                for i in 0..rowprep.get_n_vars() {
                    if rowprep.vars[i] == auxvar {
                        auxvarcoef = rowprep.coefs[i].abs();
                        break;
                    }
                }

                if auxvarcoef == 0.0
                    || (!overestimate && (cutviol / auxvarcoef <= weakcutthreshold * (auxvalue - auxvarvalue)))
                    || (overestimate && (-cutviol / auxvarcoef >= weakcutthreshold * (auxvalue - auxvarvalue)))
                {
                    sepasuccess = false;
                }
            }
        } else {
            // TODO if violations are really tiny, then maybe handle special (decrease
            // LP feastol, for example).

            // If estimate didn't report branchscores explicitly, then consider
            // branching on those children for which the following cleanup changes
            // coefficients (we had/have this in expr_sum this way).
            if !branchscoresuccess {
                rowprep.record_modifications();
            }

            rowprep.cleanup(scip, sol, mincutviolation, Some(&mut cutviol), Some(&mut sepasuccess));

            // If cleanup left us with a useless cut, then consider branching on
            // variables for which coef were changed.
            if !sepasuccess && !branchscoresuccess && !rowprep.modifiedvars.is_empty() {
                let violscore = get_expr_rel_aux_violation_nonlinear(scip, expr, auxvalue, sol, None, None);

                let mut modified = rowprep.modifiedvars.clone();
                branchscoresuccess = add_expr_viol_scores_aux_vars(scip, expr, violscore, &mut modified, sol)?;
                rowprep.modifiedvars = modified;

                // addConsExprExprBranchScoresAuxVars can fail if the only vars for which
                // the coef was changed were fixed, are this expr's auxvar, or if a
                // variable in the rowprep is not in expr (can happen with indicator
                // added by perspective).
            }
        }
    }

    // If cut looks good (numerics ok and cutting off solution), then turn into
    // row and add to sepastore.
    if sepasuccess {
        if branchdualweight > 0.0 {
            // Store remaining gap |f(x)-estimateval| in row name, which could be used
            // in getDualBranchscore; skip if gap is zero.
            if auxvalue == INVALID {
                rowprep.name.push_str("_estimategap=inf");
            } else if !scip.set.is_eq(auxvalue, estimateval) {
                rowprep.name.push_str(&format!("_estimategap={}", crate::misc::fmt_g((auxvalue - estimateval).abs())));
            }
        }

        let mut row = rowprep.get_row_cons(scip, cons)?;

        if !allowweakcuts && strongcutefficacy && !crate::scip_lp::is_cut_efficacious(scip, sol, row) {
            // Cut is not efficacious.
        } else if !crate::sepastore::sepastore_is_cut_applicable(scip, row) {
            // Cut is not applicable.
        } else {
            // I take !allowweakcuts as equivalent for having a strong cut (we usually
            // have allowweakcuts=TRUE only if we haven't found strong cuts before).
            let infeasible = crate::scip_lp::add_row(scip, row, forcestrongcut && !allowweakcuts && inenforcement)?;

            // Mark row as not removable from LP for current node (this can prevent
            // some cycling).
            if rownotremovable == b'a' || (rownotremovable == b'e' && inenforcement) {
                crate::lp::row_mark_not_removable_local(scip, row);
            }

            if infeasible {
                *result = ScipResultCode::Cutoff;
                nlhdlr::nlhdlr_increment_n_cutoffs(scip, nlhdlr);
            } else {
                *result = ScipResultCode::Separated;
                nlhdlr::nlhdlr_increment_n_separated(scip, nlhdlr);
            }
        }

        crate::scip_lp::release_row(scip, &mut row)?;
    } else if branchscoresuccess {
        // Well, not branched, but addConsExprExprViolScoresAuxVars() added scores
        // to (aux)variables and that makes the expressions eligible for branching
        // candidate, see enforceConstraints() and branching().
        *result = ScipResultCode::Branched;
    }
    // Else: no separation, no branching candidates.

    Ok(())
}

/// `enforceExprNlhdlr`: call enforcement or estimate callback of nonlinear
/// handler.
///
/// Calls the enforcement callback, if available. Otherwise, calls the
/// estimate callback, if available, and constructs a cut from the estimator.
///
/// If cut is weak, but estimator is not tight, tries to add branching
/// candidates.
#[allow(clippy::too_many_arguments)]
fn enforce_expr_nlhdlr(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    nh: NlhdlrId,
    expr: ExprId,
    nlhdlrexprdata: usize,
    sol: SolId,
    auxvalue: f64,
    overestimate: bool,
    separated: bool,
    allowweakcuts: bool,
    inenforcement: bool,
    branchcandonly: bool,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    use crate::types::ScipResultCode;

    // Call enforcement callback of the nlhdlr.
    nlhdlr::nlhdlr_enfo(
        scip,
        conshdlr,
        cons,
        nh,
        expr,
        nlhdlrexprdata,
        sol,
        auxvalue,
        overestimate,
        allowweakcuts,
        separated,
        inenforcement,
        branchcandonly,
        result,
    )?;

    // If it was not running (e.g., because it was not available) or did not
    // find anything, then try with estimator callback.
    if *result != ScipResultCode::DidNotRun && *result != ScipResultCode::DidNotFind {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Now call the estimator callback of the nlhdlr.
    if scip.nlhdlrs[nh].estimate.is_some() {
        let mut sepasuccess = false;
        let mut branchscoresuccess = false;
        let mut rowpreps = Vec::new();

        let auxvar = get_expr_aux_var_nonlinear(scip, expr);

        let auxvarval = crate::api::get_sol_val(scip, sol, auxvar);
        nlhdlr::nlhdlr_estimate(
            scip,
            conshdlr,
            nh,
            expr,
            nlhdlrexprdata,
            sol,
            auxvalue,
            overestimate,
            auxvarval,
            inenforcement,
            &mut rowpreps,
            &mut sepasuccess,
            &mut branchscoresuccess,
        )?;

        for mut rowprep in rowpreps {
            if !branchcandonly {
                // Complete estimator to cut.
                rowprep.add_term(auxvar, -1.0);

                // Add the cut and/or branching scores (branching scores that could be
                // added here are to deal with bad numerics of cuts; we skip these if
                // branchcandonly).
                process_rowprep_nonlinear(
                    scip,
                    nh,
                    cons,
                    expr,
                    &mut rowprep,
                    overestimate,
                    auxvar,
                    auxvalue,
                    allowweakcuts,
                    branchscoresuccess,
                    inenforcement,
                    sol,
                    result,
                )?;
            }
        }

        if branchcandonly && branchscoresuccess {
            *result = ScipResultCode::Branched;
        }
    }

    Ok(())
}

/// `enforceExpr`: tries to enforce violation in an expression by
/// separation, bound tightening, or finding a branching candidate.
///
/// If not inenforcement, then we should be called by consSepa(), and thus
/// only try separation.
#[allow(clippy::too_many_arguments)]
fn enforce_expr(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    expr: ExprId,
    sol: SolId,
    soltag: i64,
    allowweakcuts: bool,
    inenforcement: bool,
    branchcandonly: bool,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    use crate::types::ScipResultCode;

    *result = ScipResultCode::DidNotFind;

    // Make sure that this expression has been evaluated.
    expr::expr_eval(scip, expr, sol, soltag)?;

    // Decide whether under- or overestimate is required and get amount of
    // violation.
    let mut underestimate = false;
    let mut overestimate = false;
    let origviol = get_expr_abs_orig_violation(scip, expr, sol, Some(&mut underestimate), Some(&mut overestimate));

    let (enfoauxviolfactor, enforound) = {
        let h = hd(scip, conshdlr);
        (h.enfoauxviolfactor, h.enforound)
    };

    // No sufficient violation w.r.t. the original variables -> skip expression.
    if !overestimate && !underestimate {
        return Ok(());
    }

    // Check aux-violation w.r.t. each nonlinear handlers and try to enforce when
    // there is a decent violation.
    for e in 0..od(scip, expr).nenfos.max(0) as usize {
        let enfo = od(scip, expr).enfos[e];

        // Skip nlhdlr that do not want to participate in any separation.
        if enfo.nlhdlrparticipation & nlhdlr::NLHDLR_METHOD_SEPABOTH == 0 {
            continue;
        }

        // If looking for branching candidates only, then skip nlhdlr that wouldn't
        // created branching candidates.
        if branchcandonly && !enfo.sepaaboveusesactivity && !enfo.sepabelowusesactivity {
            continue;
        }

        let nh = enfo.nlhdlr;

        // Evaluate the expression w.r.t. the nlhdlrs auxiliary variables.
        let mut auxvalue = 0.0;
        nlhdlr::nlhdlr_evalaux(scip, nh, expr, enfo.nlhdlrexprdata, &mut auxvalue, sol)?;
        odm(scip, expr).enfos[e].auxvalue = auxvalue;

        // TODO if expr is root of constraint (consdata->expr == expr), then compare
        // auxvalue with constraint sides instead of auxvarvalue, as the former is
        // what actually matters (see cons_nonlinear.c); if changing this here, we
        // must also adapt analyzeViolation().
        let mut auxunderestimate = false;
        let mut auxoverestimate = false;
        let auxviol = get_expr_abs_aux_violation(scip, expr, auxvalue, sol, Some(&mut auxunderestimate), Some(&mut auxoverestimate));

        // If aux-violation is much smaller than orig-violation, then better enforce
        // further down in the expression first.
        if !scip.set.is_infinity(auxviol) && auxviol < enfoauxviolfactor * origviol {
            // TODO should we do expr->lastenforced = conshdlrdata->enforound even
            // though we haven't enforced, but only decided not to enforce?
            continue;
        }

        // If aux-violation is small (below feastol) and we look only for strong
        // cuts, then it's unlikely to give a strong cut, so skip it.
        if !allowweakcuts && auxviol < scip.set.feastol() {
            // TODO should we do expr->lastenforced = conshdlrdata->enforound even
            // though we haven't enforced, but only decided not to enforce?
            continue;
        }

        // Call the separation or estimation callback of the nonlinear handler for
        // over- (pass 0) and underestimation (pass 1) if we want to over/
        // underestimate and violation w.r.t. auxiliary variables is also present on
        // this side and nlhdlr wants to be called for separation on this side.
        let mut stop = false;
        for pass in 0..2 {
            let (want, auxwant, method, usesactivity, over) = if pass == 0 {
                (overestimate, auxoverestimate, nlhdlr::NLHDLR_METHOD_SEPAABOVE, enfo.sepaaboveusesactivity, true)
            } else {
                (underestimate, auxunderestimate, nlhdlr::NLHDLR_METHOD_SEPABELOW, enfo.sepabelowusesactivity, false)
            };

            if want && auxwant && (enfo.nlhdlrparticipation & method) != 0 && (!branchcandonly || usesactivity) {
                let mut hdlrresult = ScipResultCode::DidNotFind;
                let separated = *result == ScipResultCode::Separated;
                enforce_expr_nlhdlr(
                    scip,
                    conshdlr,
                    cons,
                    nh,
                    expr,
                    enfo.nlhdlrexprdata,
                    sol,
                    auxvalue,
                    over,
                    separated,
                    allowweakcuts,
                    inenforcement,
                    branchcandonly,
                    &mut hdlrresult,
                )?;

                if hdlrresult == ScipResultCode::Cutoff {
                    *result = ScipResultCode::Cutoff;
                    odm(scip, expr).lastenforced = enforound;
                    stop = true;
                    break;
                }

                if hdlrresult == ScipResultCode::Separated {
                    *result = ScipResultCode::Separated;
                    odm(scip, expr).lastenforced = enforound;
                    // TODO or should we give other nlhdlr another chance? (also #3070)
                    stop = true;
                    break;
                }

                if hdlrresult == ScipResultCode::ReducedDom {
                    *result = ScipResultCode::ReducedDom;
                    odm(scip, expr).lastenforced = enforound;
                    // TODO or should we always just stop here?
                }

                if hdlrresult == ScipResultCode::Branched {
                    // Separation and domain reduction takes precedence over branching.
                    if *result == ScipResultCode::DidNotFind {
                        *result = ScipResultCode::Branched;
                    }
                    odm(scip, expr).lastenforced = enforound;
                }
            }
        }
        if stop {
            break;
        }
    }

    Ok(())
}

/// `enforceConstraint`: helper function to enforce a single constraint;
/// returns whether some enforcement took place.
#[allow(clippy::too_many_arguments)]
fn enforce_constraint(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    cons: ConsId,
    sol: SolId,
    soltag: i64,
    it: &mut expr::ExprIter,
    allowweakcuts: bool,
    inenforcement: bool,
    branchcandonly: bool,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult<bool> {
    use crate::types::ScipResultCode;

    let mut success = false;

    if inenforcement && !branchcandonly && !cd(scip, cons).ispropagated {
        // If there are boundchanges that haven't been propagated to activities yet,
        // then do this now and update bounds of auxiliary variables, since some
        // nlhdlr/exprhdlr may look at auxvar bounds or activities (TODO: nlhdlr
        // tells us now whether they do and so we could skip). For now, update
        // bounds of auxiliary variables only if called from enforcement, since
        // updating auxvar bounds in separation doesn't seem to be right (see
        // cons_nonlinear.c).
        let mut infeasible = false;
        let mut ntightenings = 0;
        let consexpr = cd(scip, cons).expr;
        forward_prop_expr(scip, conshdlr, consexpr, inenforcement, Some(&mut infeasible), Some(&mut ntightenings))?;
        if infeasible {
            *result = ScipResultCode::Cutoff;
            return Ok(success);
        }

        // If we tightened an auxvar bound, we better communicate that.
        if ntightenings > 0 {
            *result = ScipResultCode::ReducedDom;
        }
    }

    let consexpr = cd(scip, cons).expr;
    let mut e = it.restart_dfs(scip, consexpr);
    while !it.is_end() {
        // We can only enforce if there is an auxvar to compare with.
        if od(scip, e).auxvar == NONE {
            e = it.get_next(scip);
            continue;
        }

        let enforound = hd(scip, conshdlr).enforound;
        if od(scip, e).lastenforced == enforound {
            success = true;
            e = it.get_next(scip);
            continue;
        }

        let mut resultexpr = ScipResultCode::DidNotRun;
        enforce_expr(scip, conshdlr, cons, e, sol, soltag, allowweakcuts, inenforcement, branchcandonly, &mut resultexpr)?;

        // If not enforced, then we must not have found a cutoff, cut, domain
        // reduction, or branchscore.
        if od(scip, e).lastenforced == enforound {
            success = true;
        }

        if resultexpr == ScipResultCode::Cutoff {
            *result = ScipResultCode::Cutoff;
            break;
        }

        if resultexpr == ScipResultCode::Separated {
            *result = ScipResultCode::Separated;
        }

        if resultexpr == ScipResultCode::ReducedDom && *result != ScipResultCode::Separated {
            *result = ScipResultCode::ReducedDom;
        }

        if resultexpr == ScipResultCode::Branched && *result != ScipResultCode::Separated && *result != ScipResultCode::ReducedDom {
            *result = ScipResultCode::Branched;
        }

        e = it.get_next(scip);
    }

    Ok(success)
}

/// `enforceConstraints`: try to separate violated constraints and, if in
/// enforcement, register branching scores.
///
/// If branchcandonly=TRUE, then do not separate or propagate, but register
/// branching scores only.
///
/// Sets result to
/// - SCIP_DIDNOTFIND, if nothing of the below has been done
/// - SCIP_CUTOFF, if node can be cutoff,
/// - SCIP_SEPARATED, if a cut has been added,
/// - SCIP_REDUCEDDOM, if a domain reduction has been found or a variable got
///   fixed (in an attempt to branch on it),
/// - SCIP_BRANCHED, if branching has been done (if branchcandonly=TRUE, then
///   collected branching candidates only),
/// - SCIP_INFEASIBLE, if external branching candidates were registered
#[allow(clippy::too_many_arguments)]
fn enforce_constraints(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    sol: SolId,
    soltag: i64,
    inenforcement: bool,
    branchcandonly: bool,
    maxrelconsviol: f64,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    use crate::types::ScipResultCode;

    // Increase tag to tell whether branching scores in expression belong to this
    // sweep and which expressions have already been enforced in this sweep (we
    // also want to distinguish sepa rounds, so this need to be here and not in
    // consEnfo).
    hdm(scip, conshdlr).enforound += 1;

    *result = ScipResultCode::DidNotFind;

    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, true)?;

    for &cons in conss {
        // Skip constraints that are not enabled or deleted.
        let c = &scip.conss[cons];
        if !c.enabled || c.deleted {
            continue;
        }

        // Skip constraints that have separation disabled if we are only in
        // separation.
        if !inenforcement && !c.sepaenabled {
            continue;
        }

        // Skip non-violated constraints.
        if !is_cons_violated(scip, cons) {
            continue;
        }

        let consenforced = enforce_constraint(scip, conshdlr, cons, sol, soltag, &mut it, false, inenforcement, branchcandonly, result)?;

        if *result == ScipResultCode::Cutoff {
            break;
        }

        if !consenforced && inenforcement && !branchcandonly {
            let viol = get_cons_rel_violation(scip, cons, sol, soltag)?;
            if viol > hd(scip, conshdlr).weakcutminviolfactor * maxrelconsviol {
                let consenforced = enforce_constraint(scip, conshdlr, cons, sol, soltag, &mut it, true, inenforcement, branchcandonly, result)?;
                if consenforced {
                    // TODO maybe this should not be counted per constraint, but per
                    // enforcement round?
                    hdm(scip, conshdlr).nweaksepa += 1;
                }
                if *result == ScipResultCode::Cutoff {
                    break;
                }
            }
        }
    }

    it.free(scip);

    if *result == ScipResultCode::Branched && !branchcandonly {
        // Having result set to branched here means only that we have branching
        // candidates, we still need to do the actual branching.
        branching(scip, conshdlr, conss, maxrelconsviol, sol, soltag, result)?;

        // Branching should either have branched: result == SCIP_BRANCHED, or fixed
        // a variable: result == SCIP_REDUCEDDOM, or have registered external
        // branching candidates: result == SCIP_INFEASIBLE, or have not done
        // anything: result == SCIP_DIDNOTFIND.
    }

    Ok(())
}

/// `SCIPgetAvgPseudocostCount`: gets the average number of pseudo cost
/// updates for the given direction over all variables.
fn get_avg_pseudocost_count(scip: &Scip, dir: crate::types::BranchDir) -> f64 {
    let p = &scip.probs[crate::prob::TRANSPROB];
    scip.stat.glbhistory.get_pseudocost_count(dir) / ((p.nbinvars + p.nintvars).max(1) as f64)
}

/// `branchingIntegralOrNonlinear`: decide whether to branch on fractional
/// integer or nonlinear variable; returns `(branchintegral, cutoff)`.
///
/// The routine collects spatial branching candidates by a call to
/// enforceConstraints(branchcandonly=TRUE) and collectBranchingCandidates().
/// Then it adds fractional integer variables to the candidate list. Variables
/// that are candidate for both spatial branching and fractionality are
/// considered as two separate candidates. selectBranchingCandidate() then
/// selects a variable for branching from the joined candidate list. If the
/// selected variable is a fractional integer one, then branchintegral=TRUE is
/// returned, otherwise FALSE. Some shortcuts exist for cases where there are
/// no candidates of the one kind or the other.
fn branching_integral_or_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], soltag: i64, maxrelconsviol: f64) -> ScipResult<(bool, bool)> {
    use crate::types::ScipResultCode;

    if crate::api::get_n_bin_vars(scip) + crate::api::get_n_int_vars(scip) == 0 || crate::api::get_n_lp_branch_cands(scip) == 0 {
        return Ok((false, false));
    }

    let mut result = ScipResultCode::DidNotRun;
    enforce_constraints(scip, conshdlr, conss, NONE, 0, true, true, maxrelconsviol, &mut result)?;
    match result {
        ScipResultCode::DidNotFind => {
            // No branching candidates found could mean that the LP solution is in a
            // convex region.
            return Ok((true, false));
        }
        ScipResultCode::Cutoff => {
            // Probably cannot happen, but easy to handle.
            return Ok((false, true));
        }
        ScipResultCode::Separated | ScipResultCode::ReducedDom => {
            // We asked enforceConstraints() to collect branching candidates only, it
            // shouldn't have separated or propagated.
            log::error!("Unexpected separation or propagation from enforceConstraints(branchcandonly = TRUE)");
            return Err(Retcode::Error);
        }
        ScipResultCode::Branched => {
            // Actually meaning that branching candidates were registered (the result
            // for which we have gone through all this effort).
        }
        _ => {
            log::error!("Unexpected return from enforceConstraints(branchcandonly = TRUE)");
            return Err(Retcode::Error);
        }
    }

    // Collect spatial branching candidates and their auxviol-score.
    let mut cands = Vec::new();
    collect_branching_candidates(scip, conshdlr, conss, maxrelconsviol, NONE, soltag, &mut cands)?;

    // Add fractional integer variables to branching candidates.
    let (lpcands, _lpcandssol, lpcandsfrac, nlpcands, _, _) = crate::api::get_lp_branch_cands(scip)?;
    for c in 0..nlpcands as usize {
        cands.push(BranchCand {
            expr: NONE,
            var: lpcands[c],
            auxviol: 0.0,
            fractionality: lpcandsfrac[c],
            ..Default::default()
        });
    }

    // Select a variable for branching; to keep things separate, do not include
    // fractionality of integer variables into scores of spatial branching
    // candidates; the same variables appear among the candidates for branching
    // on integrality, where its fractionality is considered.
    let selected = select_branching_candidate(scip, conshdlr, &mut cands, false, NONE)?;

    Ok((cands[selected].expr == NONE, false))
}

/// `branchingIntegralFirst`: decide whether to consider spatial branching
/// before integrality has been enforced.
///
/// This decides whether we are still at a phase where we always want to
/// branch on fractional integer variables if any (return TRUE), or whether
/// branchingIntegralOrNonlinear() should be used (return FALSE).
///
/// This essentially checks whether the average pseudo cost count exceeds the
/// value of parameter branchmixfractional.
fn branching_integral_first(scip: &mut Scip, conshdlr: ConshdlrId, sol: SolId) -> bool {
    // If LP still unbounded, then work on nonlinear constraints first.
    if sol == NONE && crate::api::get_lp_solstat(scip) == crate::types::LpSolStat::UnboundedRay {
        return false;
    }

    // No branching in cons_integral if no integer variables.
    let nbinvars = crate::api::get_n_bin_vars(scip);
    let nintvars = crate::api::get_n_int_vars(scip);
    if nbinvars + nintvars == 0 {
        return false;
    }

    // No branching in cons_integral if LP solution not fractional.
    if sol == NONE && crate::api::get_n_lp_branch_cands(scip) == 0 {
        return false;
    }

    // No branching in cons_integral if relax solution not fractional.
    if sol != NONE {
        let vars = crate::api::get_vars(scip).to_vec();
        let isfractional = vars[..(nbinvars + nintvars) as usize]
            .iter()
            .any(|&v| !scip.set.is_feas_integral(crate::api::get_sol_val(scip, sol, v)));
        if !isfractional {
            return false;
        }
    }

    let branchmixfractional = hd(scip, conshdlr).branchmixfractional;

    // branchmixfractional being infinity means that integral should always go
    // first.
    if scip.set.is_infinity(branchmixfractional) {
        return true;
    }

    // branchmixfractional being 0.0 means we do not wait for any pseudocosts to
    // be available.
    if branchmixfractional == 0.0 {
        return false;
    }

    // If not yet enough pseudocosts for down or up direction, then branch on
    // fractionality.
    // @todo this gives the total pseudocost count divided by the number of
    // discrete variables; if we updated pseudocost after branching on continuous
    // variables, wouldn't this be incorrect? (#3637)
    if get_avg_pseudocost_count(scip, crate::types::BranchDir::Downwards) < branchmixfractional {
        return true;
    }
    if get_avg_pseudocost_count(scip, crate::types::BranchDir::Upwards) < branchmixfractional {
        return true;
    }

    // We may have decent pseudocosts, so go for rule that chooses between
    // fractional and spatial branching based on candidates.
    false
}

/// Violation statistics from `analyzeViolation`.
#[derive(Clone, Copy, Debug)]
struct ViolationInfo {
    maxabsconsviol: f64,
    maxrelconsviol: f64,
    minauxviol: f64,
    maxauxviol: f64,
    maxvarboundviol: f64,
}

/// `analyzeViolation`: collect information on violation in expressions.
///
/// Assumes that constraint violations have been computed.
fn analyze_violation(scip: &mut Scip, conss: &[ConsId], sol: SolId, soltag: i64) -> ScipResult<ViolationInfo> {
    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;

    let mut info = ViolationInfo {
        maxabsconsviol: 0.0,
        maxrelconsviol: 0.0,
        minauxviol: scip.set.infinity(),
        maxauxviol: 0.0,
        maxvarboundviol: 0.0,
    };

    for &cons in conss {
        // Skip constraints that are not enabled, deleted, or have separation
        // disabled.
        let c = &scip.conss[cons];
        if !c.enabled || c.deleted || !c.sepaenabled {
            continue;
        }

        let v = get_cons_abs_violation(scip, cons);
        info.maxabsconsviol = cmax(info.maxabsconsviol, v);

        // Skip non-violated constraints.
        if !is_cons_violated(scip, cons) {
            continue;
        }

        let v = get_cons_rel_violation(scip, cons, sol, soltag)?;
        info.maxrelconsviol = cmax(info.maxrelconsviol, v);

        let consexpr = cd(scip, cons).expr;
        let mut e = it.restart_dfs(scip, consexpr);
        while !it.is_end() {
            let auxvar = od(scip, e).auxvar;
            if auxvar == NONE {
                // Check violation of variable bounds of original variable.
                if expr::expr_is_var(scip, e) {
                    let var = crate::expr_var::get_var_expr_var(scip, e);
                    let auxvarvalue = crate::api::get_sol_val(scip, sol, var);
                    let (auxvarlb, auxvarub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);

                    let mut origviol = 0.0;
                    if auxvarlb > auxvarvalue && !scip.set.is_infinity(-auxvarlb) {
                        origviol = auxvarlb - auxvarvalue;
                    } else if auxvarub < auxvarvalue && !scip.set.is_infinity(auxvarub) {
                        origviol = auxvarvalue - auxvarub;
                    }
                    if origviol > 0.0 {
                        info.maxvarboundviol = cmax(info.maxvarboundviol, origviol);
                    }
                }
                e = it.get_next(scip);
                continue;
            }

            let auxvarvalue = crate::api::get_sol_val(scip, sol, auxvar);
            let (auxvarlb, auxvarub) = (scip.vars[auxvar].locdom.lb, scip.vars[auxvar].locdom.ub);

            // Check violation of variable bounds of auxiliary variable.
            if auxvarlb - auxvarvalue > info.maxvarboundviol && !scip.set.is_infinity(-auxvarlb) {
                info.maxvarboundviol = auxvarlb - auxvarvalue;
            } else if auxvarvalue - auxvarub > info.maxvarboundviol && !scip.set.is_infinity(auxvarub) {
                info.maxvarboundviol = auxvarvalue - auxvarub;
            }

            let origviol = get_expr_abs_orig_violation(scip, e, sol, None, None);

            // No violation w.r.t. the original variables -> skip expression.
            if origviol == 0.0 {
                e = it.get_next(scip);
                continue;
            }

            // Compute aux-violation for each nonlinear handlers.
            for k in 0..od(scip, e).nenfos.max(0) as usize {
                let enfo = od(scip, e).enfos[k];

                // Eval in auxvars is only defined for nlhdrs that separate; there might
                // not even be auxvars otherwise.
                if enfo.nlhdlrparticipation & nlhdlr::NLHDLR_METHOD_SEPABOTH == 0 {
                    continue;
                }

                // Evaluate the expression w.r.t. the nlhdlrs auxiliary variables.
                let mut auxvalue = 0.0;
                nlhdlr::nlhdlr_evalaux(scip, enfo.nlhdlr, e, enfo.nlhdlrexprdata, &mut auxvalue, sol)?;
                odm(scip, e).enfos[k].auxvalue = auxvalue;

                let auxviol = get_expr_abs_aux_violation(scip, e, auxvalue, sol, None, None);

                if auxviol > 0.0 {
                    info.maxauxviol = cmax(info.maxauxviol, auxviol);
                    info.minauxviol = cmin(info.minauxviol, auxviol);
                }
            }

            e = it.get_next(scip);
        }
    }

    it.free(scip);

    Ok(info)
}

/// `consEnfo`: enforcement of constraints called by enfolp and enforelax.
fn cons_enfo(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], sol: SolId, result: &mut crate::types::ScipResultCode) -> ScipResult {
    use crate::types::ScipResultCode;

    if branching_integral_first(scip, conshdlr, sol) {
        // Let cons_integral handle enforcement.
        *result = ScipResultCode::Infeasible;
        return Ok(());
    }

    let soltag = expr::get_expr_new_soltag(scip);

    *result = ScipResultCode::Feasible;
    for &cons in conss {
        compute_violation(scip, cons, sol, soltag)?;

        if is_cons_violated(scip, cons) {
            *result = ScipResultCode::Infeasible;
        }
    }

    if *result == ScipResultCode::Feasible {
        return Ok(());
    }

    let info = analyze_violation(scip, conss, sol, soltag)?;
    let (maxrelconsviol, maxauxviol, maxvarboundviol) = (info.maxrelconsviol, info.maxauxviol, info.maxvarboundviol);

    // Look at fractional and nonlinear branching candidates and decide whether
    // to branch on fractional vars, first.
    if sol == NONE {
        let (branchintegral, cutoff) = branching_integral_or_nonlinear(scip, conshdlr, conss, soltag, maxrelconsviol)?;
        if cutoff {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }
        if branchintegral {
            // Let cons_integral handle enforcement.
            *result = ScipResultCode::Infeasible;
            return Ok(());
        }
    }

    let (propinenforce, tightenlpfeastol) = {
        let h = hd(scip, conshdlr);
        (h.propinenforce, h.tightenlpfeastol)
    };

    // Try to propagate.
    if propinenforce {
        let mut propresult = ScipResultCode::DidNotRun;
        let mut nchgbds = 0;

        prop_conss(scip, conshdlr, conss, true, &mut propresult, &mut nchgbds)?;

        if propresult == ScipResultCode::Cutoff || propresult == ScipResultCode::ReducedDom {
            *result = propresult;
            return Ok(());
        }
    }

    let lpfeastol = |scip: &Scip| crate::lp::lp_get_feastol(scip);

    // Tighten the LP tolerance if violation in variables bounds is larger than
    // aux-violation (max |expr - auxvar| over all violated expr/auxvar in
    // violated constraints).
    if tightenlpfeastol && maxvarboundviol > maxauxviol && scip.set.is_positive(lpfeastol(scip)) && sol == NONE {
        let f = cmax(scip.set.epsilon(), cmin(maxvarboundviol / 2.0, lpfeastol(scip) / 2.0));
        crate::lp::lp_set_feastol_extern(scip, f);
        hdm(scip, conshdlr).ntightenlp += 1;

        *result = ScipResultCode::SolveLp;

        return Ok(());
    }

    // Tighten the LP tolerance if violation in auxiliaries is below LP feastol,
    // as we could have problems to find a cut with violation above LP tolerance
    // (especially when auxviolation is below 10*eps = ROWPREP_SCALEUP_VIOLNONZERO
    // in misc_rowprep.c).
    if tightenlpfeastol && maxauxviol < lpfeastol(scip) && scip.set.is_positive(lpfeastol(scip)) && sol == NONE {
        let f = cmax(scip.set.epsilon(), maxauxviol / 2.0);
        crate::lp::lp_set_feastol_extern(scip, f);
        hdm(scip, conshdlr).ntightenlp += 1;

        *result = ScipResultCode::SolveLp;

        return Ok(());
    }

    enforce_constraints(scip, conshdlr, conss, sol, soltag, true, false, maxrelconsviol, result)?;

    if matches!(
        *result,
        ScipResultCode::Cutoff | ScipResultCode::Separated | ScipResultCode::ReducedDom | ScipResultCode::Branched | ScipResultCode::Infeasible
    ) {
        return Ok(());
    }

    if sol == NONE && crate::api::get_n_lp_branch_cands(scip) > 0 {
        // If there are still fractional integer variables, then let cons_integral
        // go first.
        *result = ScipResultCode::Infeasible;
        return Ok(());
    }

    if tightenlpfeastol && scip.set.is_positive(maxvarboundviol) && scip.set.is_positive(lpfeastol(scip)) && sol == NONE {
        let f = cmax(scip.set.epsilon(), cmin(maxvarboundviol / 2.0, lpfeastol(scip) / 2.0));
        crate::lp::lp_set_feastol_extern(scip, f);
        hdm(scip, conshdlr).ntightenlp += 1;

        *result = ScipResultCode::SolveLp;

        return Ok(());
    }

    if tightenlpfeastol && scip.set.is_positive(maxauxviol) && scip.set.is_positive(lpfeastol(scip)) && sol == NONE {
        // Try whether tighten the LP feasibility tolerance could help; maybe it is
        // just some cut that hasn't been taken into account sufficiently (see
        // cons_nonlinear.c).
        let f = cmax(scip.set.epsilon(), cmin(maxauxviol / 2.0, lpfeastol(scip) / 10.0));
        crate::lp::lp_set_feastol_extern(scip, f);
        hdm(scip, conshdlr).ndesperatetightenlp += 1;

        *result = ScipResultCode::SolveLp;

        return Ok(());
    }

    // Try to propagate, if not tried above TODO(?) allow to disable this as
    // well.
    if !propinenforce {
        let mut propresult = ScipResultCode::DidNotRun;
        let mut nchgbds = 0;

        prop_conss(scip, conshdlr, conss, true, &mut propresult, &mut nchgbds)?;

        if propresult == ScipResultCode::Cutoff || propresult == ScipResultCode::ReducedDom {
            *result = propresult;
            return Ok(());
        }
    }

    // Could not find branching candidates even when looking at minimal violated
    // (>eps) expressions; now look if we find any unfixed variable that we could
    // still branch on.
    let nnotify = register_branching_candidates_all_unfixed(scip, conshdlr, conss)?;

    if nnotify > 0 {
        hdm(scip, conshdlr).ndesperatebranch += 1;

        // enforceConstraints may have changed it to SCIP_DIDNOTFIND.
        *result = ScipResultCode::Infeasible;
        return Ok(());
    }

    // If everything is fixed in violated constraints, then let's cut off the
    // node (see cons_nonlinear.c for why this is not really desperate).
    *result = ScipResultCode::Cutoff;

    // It's only "desperate" if the LP solution does not coincide with variable
    // fixings (should we use something tighter than epsilon here?).
    if !scip.set.is_zero(maxvarboundviol) {
        hdm(scip, conshdlr).ndesperatecutoff += 1;
    }

    Ok(())
}

/// `consSepa`: separation for all violated constraints to be used by SEPA
/// callbacks.
fn cons_sepa(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], sol: SolId, result: &mut crate::types::ScipResultCode) -> ScipResult {
    let mut haveviol = false;

    *result = crate::types::ScipResultCode::DidNotFind;

    let soltag = expr::get_expr_new_soltag(scip);

    // Compute violations.
    for &cons in conss {
        // Skip constraints that are not enabled, deleted, or have separation
        // disabled.
        let c = &scip.conss[cons];
        if !c.enabled || c.deleted || !c.sepaenabled {
            continue;
        }

        compute_violation(scip, cons, sol, soltag)?;

        if is_cons_violated(scip, cons) {
            haveviol = true;
        }
    }

    // If none of our constraints are violated, don't attempt separation.
    if !haveviol {
        return Ok(());
    }

    // Call separation.
    enforce_constraints(scip, conshdlr, conss, sol, soltag, false, false, INVALID, result)
}

/// `auxexprComp`: compare two auxiliary expressions.
///
/// Compares auxiliary variables, followed by coefficients, and then
/// constants.
fn auxexpr_comp(scip: &Scip, auxexpr1: &AuxExpr, auxexpr2: &AuxExpr) -> i32 {
    // Compare the auxiliary variables.
    // TODO can one of these be NULL?
    let compvars = crate::var::var_compare(scip, auxexpr1.auxvar, auxexpr2.auxvar);
    if compvars != 0 {
        return compvars;
    }

    // Compare the coefficients and constants.
    for i in 0..3 {
        if auxexpr1.coefs[i] != auxexpr2.coefs[i] {
            return if auxexpr1.coefs[i] < auxexpr2.coefs[i] { -1 } else { 1 };
        }
    }

    if auxexpr1.cst < auxexpr2.cst {
        -1
    } else if auxexpr1.cst == auxexpr2.cst {
        0
    } else {
        1
    }
}

/// `bilinTermAddAuxExpr`: add an auxiliary expression to a bilinear term;
/// returns whether auxexpr has been added.
fn bilin_term_add_aux_expr(scip: &mut Scip, conshdlr: ConshdlrId, termidx: usize, auxexpr: AuxExpr) -> bool {
    let bilinmaxnauxexprs = hd(scip, conshdlr).bilinmaxnauxexprs;
    let term = &hd(scip, conshdlr).bilinterms.as_ref().unwrap()[termidx];

    // Check if auxexpr has already been added to term.
    let (found, pos) = if term.nauxexprs == 0 {
        (false, 0)
    } else {
        match crate::sort::sortedvec_find(&term.auxexprs, |a, b| auxexpr_comp(scip, a, b), auxexpr) {
            Ok(p) => (true, p),
            Err(p) => (false, p),
        }
    };

    let term = &mut hdm(scip, conshdlr).bilinterms.as_mut().unwrap()[termidx];
    if !found {
        if term.nauxexprs >= bilinmaxnauxexprs {
            return false;
        }

        // Insert expression at the correct position.
        term.auxexprs.insert(pos, auxexpr);
        term.nauxexprs += 1;
        true
    } else {
        term.auxexprs[pos].underestimate |= auxexpr.underestimate;
        term.auxexprs[pos].overestimate |= auxexpr.overestimate;
        false
    }
}

/// `bilinearTermsInsertAll`: iterates through all expressions of all
/// nonlinear constraints and adds the corresponding bilinear terms to the
/// hash table.
fn bilinear_terms_insert_all(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    if conss.is_empty() {
        return Ok(());
    }

    // Check whether the bilinear terms have been stored already.
    if hd(scip, conshdlr).bilinterms.is_some() {
        return Ok(());
    }

    // Create and initialize iterator.
    let mut it = expr::ExprIter::create();
    it.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_ENTEREXPR);

    // Iterate through all constraints.
    for &cons in conss {
        let consexpr = cd(scip, cons).expr;

        // Iterate through all expressions.
        let mut e = it.restart_dfs(scip, consexpr);
        while !it.is_end() {
            let children = &scip.exprs[e].children;
            let mut x = NONE;
            let mut y = NONE;

            // Check whether the expression is of the form f(..)^2.
            if expr::expr_is_power(scip, e) && crate::expr_pow::get_exponent_expr_pow(scip, e) == 2.0 {
                x = get_expr_aux_var_nonlinear(scip, children[0]);
                y = x;
            }
            // Check whether the expression is of the form f(..) * g(..).
            else if expr::expr_is_product(scip, e) && children.len() == 2 {
                x = get_expr_aux_var_nonlinear(scip, children[0]);
                y = get_expr_aux_var_nonlinear(scip, children[1]);
            }

            // Add variables to the hash table.
            if x != NONE && y != NONE {
                let auxvar = get_expr_aux_var_nonlinear(scip, e);
                let (lp, ln) = (od(scip, e).nlockspos, od(scip, e).nlocksneg);
                insert_bilinear_term_existing_nonlinear(scip, conshdlr, x, y, auxvar, lp, ln)?;
            }

            e = it.get_next(scip);
        }
    }

    // Release iterator.
    it.free(scip);

    Ok(())
}

/// `bilinearTermsInsertEntry`: store x, y and the locks in a new bilinear
/// term; returns the position of the term in bilinterms array.
fn bilinear_terms_insert_entry(scip: &mut Scip, conshdlr: ConshdlrId, mut x: VarId, mut y: VarId, nlockspos: i32, nlocksneg: i32, existing: bool) -> ScipResult<usize> {
    // Ensure that x.index <= y.index.
    if crate::var::var_compare(scip, x, y) == 1 {
        std::mem::swap(&mut x, &mut y);
    }

    let idx = get_bilin_term_idx_nonlinear(scip, conshdlr, x, y);

    // Update or create the term.
    if idx >= 0 {
        // The term has already been added: get term and add locks.
        let term = &mut hdm(scip, conshdlr).bilinterms.as_mut().unwrap()[idx as usize];
        term.nlockspos += nlockspos;
        term.nlocksneg += nlocksneg;
        return Ok(idx as usize);
    }

    // This is the first time we encounter this product.
    let h = hdm(scip, conshdlr);
    let terms = h.bilinterms.get_or_insert_with(Vec::new);
    let idx = terms.len();

    // Get term and set values in the created bilinear term.
    terms.push(BilinTerm {
        x,
        y,
        auxexprs: Vec::new(),
        auxvar: NONE,
        nauxexprs: 0,
        nlockspos,
        nlocksneg,
        existing,
    });

    // Save to the hashtable: insert the index of the bilinear term into the
    // hash table.
    h.bilinhashtable.get_or_insert_with(HashMap::new).insert((x, y), idx);

    // Capture product variables.
    crate::var::var_capture(scip, x);
    crate::var::var_capture(scip, y);

    Ok(idx)
}

/// `bilinearTermsFree`: frees array of bilinear terms and hash table.
fn bilinear_terms_free(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    // Check whether bilinear terms have been stored.
    let Some(terms) = hdm(scip, conshdlr).bilinterms.take() else {
        return Ok(());
    };

    // Release variables.
    for mut term in terms {
        crate::api::release_var(scip, &mut term.y)?;
        crate::api::release_var(scip, &mut term.x)?;

        for a in term.auxexprs.iter_mut() {
            if a.auxvar != NONE {
                crate::api::release_var(scip, &mut a.auxvar)?;
            }
        }

        if term.nauxexprs > 0 {
            continue;
        }

        // The rest is for simple terms with a single auxvar; it might be that
        // there is a bilinear term without a corresponding auxiliary variable.
        if term.auxvar != NONE {
            crate::api::release_var(scip, &mut term.auxvar)?;
        }
    }

    // Free hash table.
    hdm(scip, conshdlr).bilinhashtable = None;

    Ok(())
}

/// `SCIPassumeConvexNonlinear`: returns whether all nonlinear constraints
/// are assumed to be convex.
pub fn assume_convex_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> bool {
    hd(scip, conshdlr).assumeconvex
}

/// `SCIPcollectBilinTermsNonlinear`: collects all bilinear terms for a
/// given set of constraints.
pub fn collect_bilin_terms_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    bilinear_terms_insert_all(scip, conshdlr, conss)
}

/// `SCIPgetNBilinTermsNonlinear`: returns the total number of bilinear
/// terms that are contained in all nonlinear constraints.
pub fn get_n_bilin_terms_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> usize {
    hd(scip, conshdlr).bilinterms.as_ref().map_or(0, Vec::len)
}

/// `SCIPgetBilinTermsNonlinear`: returns all bilinear terms that are
/// contained in all nonlinear constraints.
pub fn get_bilin_terms_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> &[BilinTerm] {
    hd(scip, conshdlr).bilinterms.as_deref().unwrap_or(&[])
}

/// `SCIPgetBilinTermIdxNonlinear`: returns the index of the bilinear term
/// representing the product of the two given variables, or -1 if the
/// variables do not appear bilinearly.
pub fn get_bilin_term_idx_nonlinear(scip: &Scip, conshdlr: ConshdlrId, mut x: VarId, mut y: VarId) -> i32 {
    let Some(table) = hd(scip, conshdlr).bilinhashtable.as_ref() else {
        return -1;
    };

    // Ensure that x.index <= y.index.
    if crate::var::var_compare(scip, x, y) == 1 {
        std::mem::swap(&mut x, &mut y);
    }

    // Use a new entry to find the image in the bilinear hash table.
    table.get(&(x, y)).map_or(-1, |&i| i as i32)
}

/// `SCIPgetBilinTermNonlinear`: returns the bilinear term that represents
/// the product of two given variables (`None` if the variables do not
/// appear bilinearly).
pub fn get_bilin_term_nonlinear(scip: &Scip, conshdlr: ConshdlrId, x: VarId, y: VarId) -> Option<&BilinTerm> {
    let idx = get_bilin_term_idx_nonlinear(scip, conshdlr, x, y);
    if idx >= 0 {
        Some(&hd(scip, conshdlr).bilinterms.as_ref().unwrap()[idx as usize])
    } else {
        None
    }
}

/// `SCIPevalBilinAuxExprNonlinear`: evaluates an auxiliary expression for a
/// bilinear term.
pub fn eval_bilin_aux_expr_nonlinear(scip: &Scip, x: VarId, y: VarId, auxexpr: &AuxExpr, sol: SolId) -> f64 {
    auxexpr.cst
        + auxexpr.coefs[0] * crate::api::get_sol_val(scip, sol, auxexpr.auxvar)
        + auxexpr.coefs[1] * crate::api::get_sol_val(scip, sol, x)
        + auxexpr.coefs[2] * crate::api::get_sol_val(scip, sol, y)
}

/// `SCIPinsertBilinearTermExistingNonlinear`: stores the variables of a
/// bilinear term in the data of the constraint handler.
pub fn insert_bilinear_term_existing_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, x: VarId, y: VarId, auxvar: VarId, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    let idx = bilinear_terms_insert_entry(scip, conshdlr, x, y, nlockspos, nlocksneg, true)?;

    // Store and capture auxiliary variable.
    if auxvar != NONE {
        hdm(scip, conshdlr).bilinterms.as_mut().unwrap()[idx].auxvar = auxvar;
        crate::var::var_capture(scip, auxvar);
    }

    Ok(())
}

/// `SCIPinsertBilinearTermImplicitNonlinear`: stores the variables of a
/// bilinear term in the data of the constraint handler.
#[allow(clippy::too_many_arguments)]
pub fn insert_bilinear_term_implicit_nonlinear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    x: VarId,
    y: VarId,
    auxvar: VarId,
    coefx: f64,
    coefy: f64,
    coefaux: f64,
    cst: f64,
    overestimate: bool,
) -> ScipResult {
    let nlockspos = if overestimate { 1 } else { 0 };
    let nlocksneg = if overestimate { 0 } else { 1 };

    let idx = bilinear_terms_insert_entry(scip, conshdlr, x, y, nlockspos, nlocksneg, false)?;

    let term = hd(scip, conshdlr).bilinterms.as_ref().unwrap()[idx].clone();

    if term.existing && term.nauxexprs == 0 && term.auxvar != NONE {
        // This is the case where we are adding an implicitly defined relation for
        // a product that has already been explicitly defined; convert auxvar into
        // an auxexpr.

        // Nothing to do if we aren't allowed to add more than one auxexpr per
        // term.
        if hd(scip, conshdlr).bilinmaxnauxexprs <= 1 {
            return Ok(());
        }

        let auxvarexpr = AuxExpr {
            cst: 0.0,
            coefs: [1.0, 0.0, 0.0],
            auxvar: term.auxvar,
            underestimate: term.nlocksneg > 0,
            overestimate: term.nlockspos > 0,
        };

        // Before we were working with term->aux.var; now aux.var has been saved and
        // aux.exprs can be initialised to NULL.
        hdm(scip, conshdlr).bilinterms.as_mut().unwrap()[idx].auxvar = NONE;

        // Since there were no auxexprs before and we've already checked for
        // bilinmaxnauxexprs, auxvarexpr should always be added.
        bilin_term_add_aux_expr(scip, conshdlr, idx, auxvarexpr);
    }

    // Create and add auxexpr.
    let termx = hd(scip, conshdlr).bilinterms.as_ref().unwrap()[idx].x;
    let auxexpr = AuxExpr {
        underestimate: !overestimate,
        overestimate,
        auxvar,
        coefs: if termx == x { [coefaux, coefx, coefy] } else { [coefaux, coefy, coefx] },
        cst,
    };

    let added = bilin_term_add_aux_expr(scip, conshdlr, idx, auxexpr);

    if added && auxvar != NONE {
        // Capture auxiliary variable.
        crate::var::var_capture(scip, auxvar);
    }

    Ok(())
}

/* vertex polyhedral separation */

/// `buildVertexPolyhedralSeparationLP`: builds LP used to compute facets of
/// the convex envelope of vertex-polyhedral functions.
fn build_vertex_polyhedral_separation_lp(nvars: usize) -> ScipResult<crate::lpi::Lpi> {
    use crate::lpi;

    // Create lpi to store the LP.
    let mut lp = lpi::lpi_create("facet finding LP", lpi::OBJSEN_MINIMIZE)?;

    let nrows = nvars + 1;
    let ncols = poweroftwo(nvars as u32) as usize;
    let nnonz = (ncols * (nrows + 1)) / 2;

    // Allocate necessary memory; set obj, lb, and ub to zero.
    let obj = vec![0.0; ncols];
    let lb = vec![0.0; ncols];
    let mut ub = vec![0.0; ncols];
    let mut beg = vec![0i32; ncols];
    let mut val = Vec::with_capacity(nnonz);
    let mut ind = Vec::with_capacity(nnonz);

    // Calculate nonzero entries in the LP.
    for i in 0..ncols {
        // An upper bound of 1.0 is implied by the last row, but I presume that LP
        // solvers prefer unbounded variables.
        ub[i] = lpi::lpi_infinity(&lp);

        beg[i] = ind.len() as i32;

        // Iterate through the bit representation of i.
        let mut row = 0;
        let mut a = 1usize;
        while a <= i {
            if (a & i) != 0 {
                val.push(1.0);
                ind.push(row);
            }

            a <<= 1;
            row += 1;
        }

        // Put 1 as a coefficient for sum_{i} lambda_i = 1 row (last row).
        val.push(1.0);
        ind.push(nrows as i32 - 1);
    }

    // Load all data into LP interface; we can assume nrows (=nvars+1) <= ncols
    // (=2^nvars), so we can pass lb as dummy lhs and rhs.
    lpi::lpi_load_col_lp(&mut lp, lpi::OBJSEN_MINIMIZE, &obj, &lb, &ub, &lb[..nrows], &lb[..nrows], &beg, &ind, &val)?;

    // For the last row, we can set the rhs to 1.0 already.
    lpi::lpi_chg_sides(&mut lp, &[nrows as i32 - 1], &[1.0], &[1.0])?;

    Ok(lp)
}

/// `computeVertexPolyhedralMaxFacetError`: the given facet might not be a
/// valid under(over)estimator, because of numerics and bad fixings; we
/// compute max_{v in V} f(v) - (alpha v + beta) (max_{v in V} alpha v +
/// beta - f(v)) where V is the set of vertices of the domain.
#[allow(clippy::too_many_arguments)]
fn compute_vertex_polyhedral_max_facet_error(
    overestimate: bool,
    funvals: &[f64],
    boxv: &[f64],
    nallvars: usize,
    nvars: usize,
    nonfixedpos: &[usize],
    facetcoefs: &[f64],
    facetconstant: f64,
) -> f64 {
    let ncorners = poweroftwo(nvars as u32);
    let mut maxerror = 0.0;

    // Check the origin (all variables at lower bound).
    let mut facetval = facetconstant;
    for i in 0..nallvars {
        facetval += facetcoefs[i] * boxv[2 * i];
    }

    // Compute largest/smallest possible value of function, depending on whether
    // we are over/under-estimating.
    let funval = funvals[0];
    let error = if overestimate { funval - facetval } else { facetval - funval };

    // Update maximum error.
    maxerror = cmax(error, maxerror);

    let mut prev = 0u32;
    for i in 1..ncorners {
        let gray = i ^ (i >> 1);
        let mut diff = gray ^ prev;

        // Compute position of unique 1 of diff.
        let mut pos = 0;
        loop {
            diff >>= 1;
            if diff == 0 {
                break;
            }
            pos += 1;
        }

        let origpos = nonfixedpos[pos];

        if gray > prev {
            facetval += facetcoefs[origpos] * (boxv[2 * origpos + 1] - boxv[2 * origpos]);
        } else {
            facetval -= facetcoefs[origpos] * (boxv[2 * origpos + 1] - boxv[2 * origpos]);
        }

        // Compute largest/smallest possible value of function, depending on
        // whether we are over/under-estimating.
        let funval = funvals[gray as usize];
        let error = if overestimate { funval - facetval } else { facetval - funval };

        // Update maximum error.
        maxerror = cmax(error, maxerror);

        prev = gray;
    }

    maxerror
}

/// `computeVertexPolyhedralFacetLP`: computes a facet of the convex or
/// concave envelope of a vertex polyhedral function by solving an LP.
#[allow(clippy::too_many_arguments)]
fn compute_vertex_polyhedral_facet_lp(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    overestimate: bool,
    xstar: &[f64],
    boxv: &[f64],
    nonfixedpos: &[usize],
    funvals: &[f64],
    nvars: usize,
    targetvalue: f64,
    success: &mut bool,
    facetcoefs: &mut [f64],
    facetconstant: &mut f64,
) -> ScipResult {
    use crate::lpi::{self, LpParam};

    *success = false;

    let (vp_maxperturb, vp_dualsimplex) = {
        let h = hd(scip, conshdlr);
        (h.vp_maxperturb, h.vp_dualsimplex)
    };

    if hd(scip, conshdlr).vp_randnumgen.is_none() && vp_maxperturb > 0.0 {
        let seed = scip.set.initialize_random_seed(VERTEXPOLY_RANDNUMINITSEED);
        hdm(scip, conshdlr).vp_randnumgen = Some(RandNumGen::new(seed));
    }

    // Construct an LP for this size, if not having one already.
    if hd(scip, conshdlr).vp_lp[nvars].is_none() {
        let lp = build_vertex_polyhedral_separation_lp(nvars)?;
        hdm(scip, conshdlr).vp_lp[nvars] = Some(Box::new(lp));
    }

    let feastol = scip.set.feastol();
    let epsilon = scip.set.epsilon();
    let inf = scip.set.infinity();
    let mut lp = hdm(scip, conshdlr).vp_lp[nvars].take().unwrap();

    // Get number of cols and rows of separation lp; number of columns should
    // equal the number of corners = 2^nvars.
    let ncols = lpi::lpi_get_ncols(&lp) as usize;
    let nrows = lpi::lpi_get_nrows(&lp) as usize;

    // Allocate necessary memory (aux is used to transform x^* and then to store
    // LP solution).
    let mut aux = vec![0.0; nrows];
    let inds: Vec<i32> = (0..ncols as i32).collect();

    // Set up the described LP on the transformed space.

    // Compute T^-1(x^*), i.e. T^-1(x^*)_i = (x^*_i - lb_i)/(ub_i - lb_i).
    let mut mindomwidth = 2.0 * inf;
    for i in 0..nrows - 1 {
        let varpos = nonfixedpos[i];
        let lb = boxv[2 * varpos];
        let ub = boxv[2 * varpos + 1];
        let solval = xstar[varpos];

        if ub - lb < mindomwidth {
            mindomwidth = ub - lb;
        }

        // Explicitly handle solution which violate bounds of variables (this can
        // happen because of tolerances).
        aux[i] = if solval <= lb {
            0.0
        } else if solval >= ub {
            1.0
        } else {
            (solval - lb) / (ub - lb)
        };

        // Perturb point to hopefully obtain a facet of the convex envelope.
        if vp_maxperturb > 0.0 {
            let rng = hdm(scip, conshdlr).vp_randnumgen.as_mut().unwrap();
            if aux[i] == 1.0 {
                aux[i] -= rng.get_real(0.0, vp_maxperturb);
            } else if aux[i] == 0.0 {
                aux[i] += rng.get_real(0.0, vp_maxperturb);
            } else {
                let mut perturbation = cmin(aux[i], 1.0 - aux[i]) / 2.0;
                perturbation = cmin(perturbation, vp_maxperturb);
                aux[i] += rng.get_real(-perturbation, perturbation);
            }
        }
    }

    let result = (|| -> ScipResult {
        // Update LP.
        lpi::lpi_chg_obj(&mut lp, &inds, funvals)?;
        lpi::lpi_chg_sides(&mut lp, &inds[..nrows - 1], &aux[..nrows - 1], &aux[..nrows - 1])?;
        lpi::lpi_chg_objsen(&mut lp, if overestimate { lpi::OBJSEN_MAXIMIZE } else { lpi::OBJSEN_MINIMIZE })?;

        // We can stop the LP solve if will not meet the target value anyway, but
        // only if xstar hasn't been perturbed.
        if vp_maxperturb == 0.0 && !scip.set.is_infinity(targetvalue.abs()) {
            lpi::lpi_set_realpar(&mut lp, LpParam::ObjLim, targetvalue)?;
        }

        // Set an iteration limit so we do not run forever.
        lpi::lpi_set_intpar(&mut lp, LpParam::LpItLim, 100 * ncols as i32)?;

        // Since we work with the dual of the LP, primal feastol determines how much
        // we want the computed facet to be the best possible one.
        lpi::lpi_set_realpar(&mut lp, LpParam::Feastol, feastol)?;

        // Since we work with the dual of the LP, dual feastol determines validity
        // of the facet; if some ub-lb is small, we need higher accuracy, since below
        // we divide coefs by ub-lb (we moved and scaled the box); thus, we set the
        // dual feastol to be between SCIPepsilon and SCIPfeastol.
        lpi::lpi_set_realpar(&mut lp, LpParam::DualFeastol, cmin(feastol, cmax(epsilon, mindomwidth * feastol)))?;

        // Solve the LP and store the resulting facet for the transformed space.
        let lpsolveretcode = if vp_dualsimplex { lpi::lpi_solve_dual(&mut lp) } else { lpi::lpi_solve_primal(&mut lp) };
        if lpsolveretcode == Err(Retcode::LpError) {
            return Ok(());
        }
        lpsolveretcode?;

        // Any dual feasible solution should provide a valid estimator (and a dual
        // optimal one a facet).
        if !lpi::lpi_is_dual_feasible(&lp) {
            return Ok(());
        }

        // Get dual solution (facet of convex envelope); again, we have to be
        // careful since the LP can have more rows and columns than needed, in
        // particular, \bar \beta is the last dual multiplier.
        lpi::lpi_get_sol(&mut lp, None, None, Some(&mut aux), None, None)?;

        for i in 0..nvars {
            facetcoefs[nonfixedpos[i]] = aux[i];
        }
        // Last dual multiplier is the constant.
        *facetconstant = aux[nrows - 1];

        // Transform the facet to original space and compute value at x^*, i.e.,
        // alpha x + beta.
        let mut facetvalue = 0.0;
        for pos in &nonfixedpos[..nvars] {
            let varpos = *pos;
            let lb = boxv[2 * varpos];
            let ub = boxv[2 * varpos + 1];

            // alpha_i := alpha_bar_i / (ub_i - lb_i).
            facetcoefs[varpos] /= ub - lb;

            // beta = beta_bar - sum_i alpha_i * lb_i.
            *facetconstant -= facetcoefs[varpos] * lb;

            // Evaluate.
            facetvalue += facetcoefs[varpos] * xstar[varpos];
        }

        // Add beta to the facetvalue: at this point in the code, facetvalue =
        // g(x^*).
        facetvalue += *facetconstant;

        // If overestimate, then we want facetvalue < targetvalue; if underestimate,
        // then we want facetvalue > targetvalue; if none holds, give up; so maybe
        // here we should check against the minimal violation.
        if overestimate == (facetvalue > targetvalue) {
            return Ok(());
        }

        // If we made it until here, then we have a nice facet.
        *success = true;

        Ok(())
    })();

    hdm(scip, conshdlr).vp_lp[nvars] = Some(lp);

    result
}

/// `computeVertexPolyhedralFacetUnivariate`: computes a facet of the convex
/// or concave envelope of a univariate vertex polyhedral function.
///
/// In other words, compute the line that passes through two given points.
fn compute_vertex_polyhedral_facet_univariate(left: f64, right: f64, funleft: f64, funright: f64, success: &mut bool, facetcoef: &mut f64, facetconstant: &mut f64) {
    *facetcoef = (funright - funleft) / (right - left);
    *facetconstant = funleft - *facetcoef * left;
    *success = true;
}

/// `SCIPlapackIsAvailable`: the reference is built without LAPACK.
const LAPACK_IS_AVAILABLE: bool = false;

/// `SCIPcomputeFacetVertexPolyhedralNonlinear`: computes a facet of the
/// convex or concave envelope of a vertex polyhedral function.
///
/// If f(x) is vertex-polyhedral, then f is a convex or concave function on
/// the box (see cons_nonlinear.h for the full description). Returns whether
/// a facet could be computed successfully.
#[allow(clippy::too_many_arguments)]
pub fn compute_facet_vertex_polyhedral_nonlinear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    overestimate: bool,
    function: &mut dyn FnMut(&mut Scip, &[f64]) -> f64,
    xstar: &[f64],
    boxv: &[f64],
    nallvars: usize,
    targetvalue: f64,
    facetcoefs: &mut [f64],
    facetconstant: &mut f64,
) -> ScipResult<bool> {
    let mut success = false;

    // Identify fixed variables.
    let mut nonfixedpos = Vec::with_capacity(nallvars);
    for j in 0..nallvars {
        if scip.set.is_rel_eq(boxv[2 * j], boxv[2 * j + 1]) {
            continue;
        }
        nonfixedpos.push(j);
    }
    let nvars = nonfixedpos.len();

    // If all variables are fixed, then we could provide something trivial, but
    // that wouldn't be the job of separation; if too many variables are not
    // fixed, then we do nothing currently.
    if nvars == 0 || nvars > MAXVERTEXPOLYDIM {
        log::warn!("SCIPcomputeFacetVertexPolyhedralNonlinear() called with {nvars} nonfixed variables. Must be between [1,{MAXVERTEXPOLYDIM}].");
        return Ok(false);
    }

    // Compute f(v^i) for each corner v^i of [l,u].
    let ncorners = poweroftwo(nvars as u32) as usize;
    let mut funvals = vec![0.0; ncorners];
    let mut corner = vec![0.0; nallvars];
    for j in 0..nallvars {
        if scip.set.is_rel_eq(boxv[2 * j], boxv[2 * j + 1]) {
            corner[j] = (boxv[2 * j] + boxv[2 * j + 1]) / 2.0;
        }
    }
    for (i, funval) in funvals[..ncorners].iter_mut().enumerate() {
        for (j, &varpos) in nonfixedpos.iter().enumerate() {
            // If j'th bit of row index i is set, then take upper bound on var j,
            // otherwise lower bound var j.
            corner[varpos] = if (i >> j) & 0x1 != 0 { boxv[2 * varpos + 1] } else { boxv[2 * varpos] };
        }

        *funval = function(scip, &corner);

        if *funval == INVALID || scip.set.is_infinity(funval.abs()) {
            return Ok(false);
        }
    }

    // Clear coefs array; below we only fill in coefs for nonfixed variables.
    facetcoefs[..nallvars].fill(0.0);

    if nvars == 1 {
        let p = nonfixedpos[0];
        compute_vertex_polyhedral_facet_univariate(boxv[2 * p], boxv[2 * p + 1], funvals[0], funvals[1], &mut success, &mut facetcoefs[p], facetconstant);

        // Check whether target has been missed.
        if success && overestimate == (*facetconstant + facetcoefs[p] * xstar[p] > targetvalue) {
            success = false;
        }
    } else if nvars == 2 && LAPACK_IS_AVAILABLE {
        unreachable!("the reference is built without LAPACK");
    } else {
        compute_vertex_polyhedral_facet_lp(
            scip,
            conshdlr,
            overestimate,
            xstar,
            boxv,
            &nonfixedpos,
            &funvals,
            nvars,
            targetvalue,
            &mut success,
            facetcoefs,
            facetconstant,
        )?;
    }

    if !success {
        return Ok(false);
    }

    // Check and adjust facet with the algorithm of Rikun et al.
    let maxfaceterror = compute_vertex_polyhedral_max_facet_error(overestimate, &funvals, boxv, nallvars, nvars, &nonfixedpos, facetcoefs, *facetconstant);

    // Adjust constant part of the facet by maxerror to make it a valid
    // over/underestimator (not facet though).
    if maxfaceterror > 0.0 {
        let feastol = if scip.set.stage == Stage::Solving {
            crate::lp::lp_get_feastol(scip)
        } else {
            scip.set.feastol()
        };

        // Evaluate function in middle point to get some idea for a scaling.
        for &p in &nonfixedpos {
            corner[p] = (boxv[2 * p] + boxv[2 * p + 1]) / 2.0;
        }
        let mut midval = function(scip, &corner);
        if midval == INVALID {
            midval = 1.0;
        }

        // There seem to be numerical problems if the error is too large; in this
        // case we reject the facet.
        if maxfaceterror > hd(scip, conshdlr).vp_adjfacetthreshold * feastol * midval.abs() {
            return Ok(false);
        }

        if overestimate {
            *facetconstant += maxfaceterror;
        } else {
            *facetconstant -= maxfaceterror;
        }
    }

    // If we made it until here, then we have a nice facet.
    Ok(true)
}

/*
 * Callback methods of constraint handler
 */

/// `conshdlrCopyNonlinear`: copy method for constraint handler plugins
/// (called when SCIP copies plugins).
fn conshdlr_copy_nonlinear(scip: &mut Scip, sourcescip: &Scip, conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Create basic data of constraint handler and include it to scip.
    include_conshdlr_nonlinear(scip)?;

    let targetconshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);

    // Copy nonlinear handlers.
    for &nlhdlr in &hd(sourcescip, conshdlr).nlhdlrs {
        nlhdlr::nlhdlr_copyhdlr(scip, targetconshdlr, sourcescip, conshdlr, nlhdlr)?;
    }

    *valid = true;

    Ok(())
}

/// `consFreeNonlinear`: destructor of constraint handler to free constraint
/// handler data (called when SCIP is exiting).
fn cons_free_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    // Free nonlinear handlers.
    let nlhdlrs = std::mem::take(&mut hdm(scip, conshdlr).nlhdlrs);
    for nlhdlr in nlhdlrs {
        nlhdlr::nlhdlr_free(scip, nlhdlr)?;
    }

    // (Upgrade functions, the clock, the queue, the random number generator,
    // the vertex-polyhedral LPs and var2expr go with the data.)
    scip.conshdlrs[conshdlr].conshdlrdata = None;

    Ok(())
}

/// `consInitNonlinear`: initialization method of constraint handler (called
/// after problem was transformed).
fn cons_init_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    let h = hdm(scip, conshdlr);
    // Make sure current activity tags in expressions are invalid, because we
    // start catching variable events only now.
    h.curboundstag += 1;
    h.lastboundrelax = h.curboundstag;
    // Set to 1 so it is larger than initial value of lastenforound in exprs.
    h.enforound = 1;
    // Reset numbering for auxiliary variables.
    h.auxvarid = 0;

    let eventhdlr = h.eventhdlr;
    for &cons in conss {
        store_var_exprs(scip, conshdlr, cons)?;
        catch_var_events(scip, eventhdlr, cons)?;
    }

    // Sort nonlinear handlers by detection priority, in decreasing order.
    if hd(scip, conshdlr).nlhdlrs.len() > 1 {
        let mut nlhdlrs = std::mem::take(&mut hdm(scip, conshdlr).nlhdlrs);
        crate::sort::sort_down(&mut nlhdlrs, (), |&a, &b| nlhdlr::nlhdlr_comp(scip, a, b));
        hdm(scip, conshdlr).nlhdlrs = nlhdlrs;
    }

    // Get heuristics for later use.
    let subnlpheur = crate::set::set_find_heur(scip, "subnlp");
    let trysolheur = crate::set::set_find_heur(scip, "trysol");
    let h = hdm(scip, conshdlr);
    h.subnlpheur = subnlpheur;
    h.trysolheur = trysolheur;

    // Reset statistics in nonlinear handlers (TODO only if misc/resetstat ==
    // TRUE) and call nlhdlrInit.
    let nlhdlrs = hd(scip, conshdlr).nlhdlrs.clone();
    for nlhdlr in nlhdlrs {
        nlhdlr::nlhdlr_init(scip, nlhdlr)?;
    }

    // Reset statistics in constraint handler.
    let h = hdm(scip, conshdlr);
    h.nweaksepa = 0;
    h.ntightenlp = 0;
    h.ndesperatebranch = 0;
    h.ndesperatecutoff = 0;
    h.ndesperatetightenlp = 0;
    h.nforcelp = 0;
    h.canonicalizetime.reset();
    h.ncanonicalizecalls = 0;

    Ok(())
}

/// `consExitNonlinear`: deinitialization method of constraint handler
/// (called before transformed problem is freed).
fn cons_exit_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    if !conss.is_empty() {
        // For better performance of dropVarEvents, we sort by index,
        // descending.
        let mut consssorted = conss.to_vec();
        crate::sort::sort_down(&mut consssorted, (), |&a, &b| comp_index_cons_nonlinear(scip, a, b));

        let eventhdlr = hd(scip, conshdlr).eventhdlr;
        for &cons in &consssorted {
            drop_var_events(scip, eventhdlr, cons)?;
            free_var_exprs(scip, cons)?;
        }
    }

    let h = hdm(scip, conshdlr);
    h.subnlpheur = NONE;
    h.trysolheur = NONE;

    h.vp_randnumgen = None;

    // Free LPs used to construct facets of envelops of vertex-polyhedral
    // functions.
    for lp in h.vp_lp.iter_mut() {
        *lp = None;
    }

    h.branchrandnumgen = None;

    // Deinitialize nonlinear handlers.
    let nlhdlrs = hd(scip, conshdlr).nlhdlrs.clone();
    for nlhdlr in nlhdlrs {
        nlhdlr::nlhdlr_exit(scip, nlhdlr)?;
    }

    Ok(())
}

/// `consExitpreNonlinear`: presolving deinitialization method of constraint
/// handler (called after presolving has been finished).
fn cons_exitpre_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    if conss.is_empty() {
        return Ok(());
    }

    // Skip some extra work if already known to be infeasible.
    if crate::api::get_status(scip) == crate::types::Status::Infeasible {
        return Ok(());
    }

    // Simplify constraints and replace common subexpressions.
    let mut infeasible = false;
    canonicalize_constraints(scip, conshdlr, conss, crate::cons::PRESOLTIMING_ALWAYS, &mut infeasible, None, None, None)?;

    // Currently SCIP does not offer to communicate this, but at the moment
    // this can only become true if canonicalizeConstraints called
    // detectNlhdlrs (which it doesn't do in EXITPRESOLVE stage) or if a
    // constraint expression became constant.

    // Tell SCIP that we have something nonlinear.
    crate::nlp::enable_nlp(scip);

    Ok(())
}

/// `consInitsolNonlinear`: solving process initialization method of
/// constraint handler (called when branch and bound process is about to
/// begin).
fn cons_initsol_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    use crate::types::Status;

    // Skip remaining initializations if we have solved already. If
    // infeasibility was found by our boundtightening, then curvature check
    // may also fail as some exprhdlr (e.g., pow) assumes nonempty activities
    // in expressions.
    match crate::api::get_status(scip) {
        Status::Optimal | Status::Infeasible | Status::Unbounded | Status::InfOrUnbd => return Ok(()),
        _ => {}
    }

    // Reset one of the number of detections counter to count only current
    // round.
    let nlhdlrs = hd(scip, conshdlr).nlhdlrs.clone();
    for nlhdlr in nlhdlrs {
        nlhdlr::nlhdlr_reset_n_detectionslast(scip, nlhdlr);
    }

    init_solve(scip, conshdlr, conss)?;

    // Check that branching/lpgainnormalize is set to a known value if
    // pseudo-costs are used in branching.
    if hd(scip, conshdlr).branchpscostweight > 0.0 {
        let strategy = crate::paramset::paramset_get_char(scip, "branching/lpgainnormalize")?;
        hdm(scip, conshdlr).branchpscostupdatestrategy = strategy;
        if !b"lds".contains(&strategy) {
            log::error!("branching/lpgainnormalize strategy {} unknown", strategy as char);
            return Err(Retcode::InvalidData);
        }
    }

    Ok(())
}

/// `consExitsolNonlinear`: solving process deinitialization method of
/// constraint handler (called before branch and bound process data is
/// freed).
fn cons_exitsol_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], _restart: bool) -> ScipResult {
    deinit_solve(scip, conshdlr, conss)?;

    // Free hash table for bilinear terms.
    bilinear_terms_free(scip, conshdlr)?;

    // Reset flag to allow another call of presolSingleLockedVars() after a
    // restart.
    hdm(scip, conshdlr).checkedvarlocks = false;

    // Drop catching new solution event, if catched before.
    let filterpos = hd(scip, conshdlr).newsoleventfilterpos;
    if filterpos >= 0 {
        let eventhdlr = crate::set::set_find_eventhdlr(scip, &format!("{CONSHDLR_NAME}_newsolution"));
        let eventtype = if hd(scip, conshdlr).linearizeheursol == b'i' {
            event::EVENTTYPE_BESTSOLFOUND
        } else {
            event::EVENTTYPE_SOLFOUND
        };
        crate::api::drop_event(scip, eventtype, eventhdlr, conshdlr, filterpos)?;
        hdm(scip, conshdlr).newsoleventfilterpos = -1;
    }

    Ok(())
}

/// `consDeleteNonlinear`: frees specific constraint data.
fn cons_delete_nonlinear(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    // (The helpers reach the data through the constraint, so it goes back on
    // it while they run.)
    scip.conss[cons].consdata = consdata.take();

    // Free variable expressions.
    free_var_exprs(scip, cons)?;

    let mut e = cd(scip, cons).expr;
    expr::expr_release(scip, &mut e)?;

    // Free nonlinear row representation.
    let mut nlrow = cd(scip, cons).nlrow;
    if nlrow != NONE {
        crate::nlp::release_nl_row(scip, &mut nlrow)?;
    }

    scip.conss[cons].consdata = None;

    Ok(())
}

/// `consTransNonlinear`: transforms constraint data into data belonging to
/// the transformed problem.
fn cons_trans_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    let (sourceexpr, lhs, rhs) = {
        let c = cd(scip, sourcecons);
        (c.expr, c.lhs, c.rhs)
    };

    // Get a copy of sourceexpr with transformed vars.
    let mut targetexpr = crate::scip_expr::duplicate_expr(scip, sourceexpr, Some(mapexprtransvar), conshdlr, Some(exprowner_create), conshdlr)?;

    // Create transformed cons (only captures targetexpr, no need to copy
    // again).
    let name = scip.conss[sourcecons].name.clone();
    let flags = crate::cons::cons_flags(scip, sourcecons);
    *targetcons = create_cons(scip, conshdlr, &name, targetexpr, lhs, rhs, false, &flags)?;

    // Release target expr.
    expr::expr_release(scip, &mut targetexpr)?;

    Ok(())
}

/// `consInitlpNonlinear`: LP initialization method of constraint handler
/// (called before the initial LP relaxation at a node is solved).
fn cons_initlp_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    // Create auxiliary variables and call separation initialization
    // callbacks of the expression handlers.
    // TODO if we ever want to allow constraints that are separated but not
    // initial, then we need to call initSepa also during SEPALP, ENFOLP,
    // etc, whenever a constraint may be separated the first time; for now,
    // there is an assert in detectNlhdlrs to require initial if separated.
    init_sepa(scip, conshdlr, conss, infeasible)?;

    // Catch new solution event.
    if hd(scip, conshdlr).linearizeheursol != b'o' && hd(scip, conshdlr).newsoleventfilterpos == -1 {
        let eventhdlr = crate::set::set_find_eventhdlr(scip, &format!("{CONSHDLR_NAME}_newsolution"));
        let eventtype = if hd(scip, conshdlr).linearizeheursol == b'i' {
            event::EVENTTYPE_BESTSOLFOUND
        } else {
            event::EVENTTYPE_SOLFOUND
        };
        let mut filterpos = -1;
        crate::api::catch_event(scip, eventtype, eventhdlr, conshdlr, Some(&mut filterpos))?;
        hdm(scip, conshdlr).newsoleventfilterpos = filterpos;
    }

    // Collect all bilinear terms for which an auxvar is present.
    // TODO this will only do something for the first call of initlp after
    // initsol, because it cannot handle addition (and removal?) of
    // constraints during solve; this is typically the majority of
    // constraints, but the method should be made more flexible.
    bilinear_terms_insert_all(scip, conshdlr, conss)?;

    Ok(())
}

/// `consSepalpNonlinear`: separation method of constraint handler for LP
/// solutions.
fn cons_sepalp_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], _nusefulconss: i32, result: &mut crate::types::ScipResultCode) -> ScipResult {
    cons_sepa(scip, conshdlr, conss, NONE, result)
}

/// `consSepasolNonlinear`: separation method of constraint handler for
/// arbitrary primal solutions.
fn cons_sepasol_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], _nusefulconss: i32, sol: SolId, result: &mut crate::types::ScipResultCode) -> ScipResult {
    cons_sepa(scip, conshdlr, conss, sol, result)
}

/// `consEnfolpNonlinear`: constraint enforcing method of constraint handler
/// for LP solutions.
fn cons_enfolp_nonlinear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    cons_enfo(scip, conshdlr, conss, NONE, result)
}

/// `consEnforelaxNonlinear`: constraint enforcing method of constraint
/// handler for relaxation solutions.
fn cons_enforelax_nonlinear(
    scip: &mut Scip,
    sol: SolId,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    cons_enfo(scip, conshdlr, conss, sol, result)
}

/// `consEnfopsNonlinear`: constraint enforcing method of constraint handler
/// for pseudo solutions.
fn cons_enfops_nonlinear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _solinfeasible: bool,
    _objinfeasible: bool,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    use crate::types::ScipResultCode;

    let soltag = expr::get_expr_new_soltag(scip);

    *result = ScipResultCode::Feasible;
    for &cons in conss {
        compute_violation(scip, cons, NONE, soltag)?;

        if is_cons_violated(scip, cons) {
            *result = ScipResultCode::Infeasible;
        }
    }

    if *result == ScipResultCode::Feasible {
        return Ok(());
    }

    // Try to propagate.
    // TODO obey propinenfo parameter, but we need something to recognize
    // cutoff.
    let mut nchgbds = 0;
    let mut propresult = ScipResultCode::DidNotRun;
    prop_conss(scip, conshdlr, conss, true, &mut propresult, &mut nchgbds)?;

    if propresult == ScipResultCode::Cutoff || propresult == ScipResultCode::ReducedDom {
        *result = propresult;
        return Ok(());
    }

    // Register all unfixed variables in all violated constraints as branching
    // candidates.
    let nnotify = register_branching_candidates_all_unfixed(scip, conshdlr, conss)?;
    if nnotify > 0 {
        return Ok(());
    }

    // Need to evaluate LP.
    *result = ScipResultCode::SolveLp;
    hdm(scip, conshdlr).nforcelp += 1;

    Ok(())
}

/// `consCheckNonlinear`: feasibility check method of constraint handler for
/// integral solutions.
#[allow(clippy::too_many_arguments)]
fn cons_check_nonlinear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    sol: SolId,
    _checkintegrality: bool,
    _checklprows: bool,
    printreason: bool,
    completely: bool,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    use crate::types::{LpSolStat, ScipResultCode, SolOrigin};

    *result = ScipResultCode::Feasible;
    let soltag = expr::get_expr_new_soltag(scip);
    let mut maxviol: f64 = 0.0;

    let mut maypropfeasible = hd(scip, conshdlr).trysolheur != NONE && scip.set.stage >= Stage::Transformed && scip.set.stage <= Stage::Solving;
    if maypropfeasible && (sol == NONE || scip.sols[sol].solorigin == SolOrigin::LpSol) && crate::api::get_lp_solstat(scip) == LpSolStat::UnboundedRay {
        maypropfeasible = false;
    }

    // Check nonlinear constraints for feasibility.
    for &cons in conss {
        compute_violation(scip, cons, sol, soltag)?;

        let absviol = get_cons_abs_violation(scip, cons);
        let relviol = get_cons_rel_violation(scip, cons, sol, soltag)?;
        crate::api::update_sol_cons_violation(scip, sol, absviol, relviol);

        if absviol > scip.set.feastol() {
            *result = ScipResultCode::Infeasible;
            maxviol = cmax(maxviol, absviol);

            // Print reason for infeasibility.
            if printreason {
                // (The port prints no constraints.)
            } else if (hd(scip, conshdlr).subnlpheur == NONE || sol == NONE) && !maypropfeasible && !completely {
                // If we don't want to pass to subnlp heuristic and don't need to
                // print reasons, then can stop checking here.
                return Ok(());
            }

            // Do not try to shift linear variables if violation is at infinity
            // (leads to setting variable to infinity in solution, which is not
            // allowed).
            if maypropfeasible && scip.set.is_infinity(get_cons_abs_violation(scip, cons)) {
                maypropfeasible = false;
            }

            if maypropfeasible {
                let c = cd(scip, cons);
                if c.lhsviol > scip.set.feastol() {
                    // Check if there is a variable which may help to get the left
                    // hand side satisfied; if there is no such variable, then we
                    // cannot get feasible.
                    if !(c.linvarincr != NONE && c.linvarincrcoef > 0.0) && !(c.linvardecr != NONE && c.linvardecrcoef < 0.0) {
                        maypropfeasible = false;
                    }
                } else {
                    // Check if there is a variable which may help to get the right
                    // hand side satisfied; if there is no such variable, then we
                    // cannot get feasible.
                    if !(c.linvarincr != NONE && c.linvarincrcoef < 0.0) && !(c.linvardecr != NONE && c.linvardecrcoef > 0.0) {
                        maypropfeasible = false;
                    }
                }
            }
        }
    }

    if *result == ScipResultCode::Infeasible && maypropfeasible {
        let success = propose_feasible_solution(scip, conshdlr, conss, sol)?;

        // Do not pass solution to NLP heuristic if we made it feasible this way.
        if success {
            return Ok(());
        }
    }

    if *result == ScipResultCode::Infeasible && hd(scip, conshdlr).subnlpheur != NONE && sol != NONE && !scip.set.is_infinity(maxviol) {
        unimplemented!("SCIPupdateStartpointHeurSubNlp (heur_subnlp is not ported)");
    }

    Ok(())
}

/// `consPropNonlinear`: domain propagation method of constraint handler.
fn cons_prop_nonlinear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _nmarkedconss: i32,
    _proptiming: crate::cons::PropTiming,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    let mut nchgbds = 0;

    prop_conss(scip, conshdlr, conss, false, result, &mut nchgbds)?;

    // TODO would it make sense to check for redundant constraints?

    Ok(())
}

/// `consPresolNonlinear`: presolving method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_presol_nonlinear(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nrounds: i32,
    presoltiming: crate::cons::PresolTiming,
    _newcounts: &crate::cons::PresolCounts,
    counts: &mut crate::cons::PresolCounts,
    result: &mut crate::types::ScipResultCode,
) -> ScipResult {
    use crate::{
        cons::{PRESOLTIMING_EXHAUSTIVE, PRESOLTIMING_MEDIUM},
        types::ScipResultCode,
    };

    *result = ScipResultCode::DidNotFind;

    if conss.is_empty() {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    // Simplify constraints and replace common subexpressions, reinit nlhdlrs.
    let mut infeasible = false;
    canonicalize_constraints(
        scip,
        conshdlr,
        conss,
        presoltiming,
        &mut infeasible,
        Some(&mut counts.ndelconss),
        Some(&mut counts.naddconss),
        Some(&mut counts.nchgcoefs),
    )?;
    if infeasible {
        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    // Merge constraints with the same root expression.
    if presoltiming & PRESOLTIMING_EXHAUSTIVE != 0 {
        let success = presolve_merge_conss(scip, conss)?;
        if success {
            *result = ScipResultCode::Success;
        }
    }

    // Propagate constraints.
    prop_conss(scip, conshdlr, conss, false, result, &mut counts.nchgbds)?;
    if *result == ScipResultCode::Cutoff {
        return Ok(());
    }

    // Propagate function domains (TODO integrate with simplify?).
    if (presoltiming & PRESOLTIMING_EXHAUSTIVE != 0) || nrounds == 0 {
        let mut localresult = ScipResultCode::DidNotRun;
        prop_expr_domains(scip, conshdlr, conss, &mut localresult, &mut counts.nchgbds)?;
        if localresult == ScipResultCode::Cutoff {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }
        if localresult == ScipResultCode::ReducedDom {
            *result = ScipResultCode::ReducedDom;
        }
    }

    // Check for redundant constraints, remove constraints that are a value
    // expression.
    presolve_redundant_conss(scip, conshdlr, conss, &mut infeasible, &mut counts.ndelconss, &mut counts.nchgbds)?;
    if infeasible {
        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    // Try to upgrade constraints.
    for &cons in conss {
        // Skip inactive and deleted constraints.
        if scip.conss[cons].deleted || !scip.conss[cons].active {
            continue;
        }

        presolve_upgrade(scip, conshdlr, cons, &mut counts.nupgdconss, &mut counts.naddconss)?;
    }

    // Try to change continuous variables that appear linearly to be implicit
    // integer.
    if presoltiming & PRESOLTIMING_MEDIUM != 0 {
        presolve_implint(scip, conshdlr, conss, &mut counts.nchgvartypes, &mut infeasible)?;

        if infeasible {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }
    }

    // Fix variables that are contained in only one nonlinear constraint to
    // their upper or lower bounds, if possible.
    if (presoltiming & PRESOLTIMING_EXHAUSTIVE != 0)
        && crate::api::is_presolve_finished_with(scip, counts)
        && !hd(scip, conshdlr).checkedvarlocks
        && hd(scip, conshdlr).checkvarlocks != b'd'
    {
        // Run this presolving technique only once because we don't want to
        // generate identical bound disjunction constraints multiple times.
        hdm(scip, conshdlr).checkedvarlocks = true;

        for &cons in conss {
            let mut tmpnchgvartypes = 0;
            let mut tmpnaddconss = 0;

            presolve_single_locked_vars(scip, conshdlr, cons, &mut tmpnchgvartypes, &mut tmpnaddconss, &mut infeasible)?;

            if infeasible {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }

            counts.nchgvartypes += tmpnchgvartypes;
            counts.naddconss += tmpnaddconss;
        }
    }

    if counts.ndelconss > 0 || counts.nchgbds > 0 || counts.nupgdconss > 0 || counts.naddconss > 0 || counts.nchgvartypes > 0 {
        *result = ScipResultCode::Success;
    } else {
        *result = ScipResultCode::DidNotFind;
    }

    Ok(())
}

/// `consLockNonlinear`: variable rounding lock method of constraint handler.
fn cons_lock_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, _locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    let mut reinitsolve = false;

    // Check whether we need to initSolve again because
    // - we have enfo initialized (nenfos >= 0)
    // - and locks appeared (going from zero to nonzero) or disappeared (going
    //   from nonzero to zero) now.
    let (consexpr, cnlockspos, cnlocksneg) = {
        let c = cd(scip, cons);
        (c.expr, c.nlockspos, c.nlocksneg)
    };
    if od(scip, consexpr).nenfos >= 0 {
        if (cnlockspos == 0) != (nlockspos == 0) {
            reinitsolve = true;
        }
        if (cnlocksneg == 0) != (nlocksneg == 0) {
            reinitsolve = true;
        }
    }

    if reinitsolve {
        deinit_solve(scip, conshdlr, &[cons])?;
    }

    // Add locks.
    add_locks(scip, cons, nlockspos, nlocksneg)?;

    if reinitsolve {
        init_solve(scip, conshdlr, &[cons])?;
    }

    Ok(())
}

/// `consActiveNonlinear`: constraint activation notification method of
/// constraint handler.
fn cons_active_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    let mut infeasible = false;

    // Simplify root expression if the constraint has been added after
    // presolving.
    if scip.set.stage > Stage::ExitPresolve {
        if !cd(scip, cons).issimplified {
            // Simplify constraint.
            let consexpr = cd(scip, cons).expr;
            let (simplified, _changed, inf) = crate::scip_expr::simplify_expr(scip, consexpr, Some(exprowner_create), conshdlr)?;
            infeasible = inf;
            let mut e = consexpr;
            expr::expr_release(scip, &mut e)?;
            let c = cdm(scip, cons);
            c.expr = simplified;
            c.issimplified = true;
        }

        // Ensure each variable is represented by one variable expression only
        // (need this for storeVarExprs() with simplified=TRUE below).
        let mut exprs = [cd(scip, cons).expr];
        crate::scip_expr::replace_common_subexpressions(scip, &mut exprs)?;
        cdm(scip, cons).expr = exprs[0];

        // Ensure that varexprs in consdata->expr are the one from var2expr
        // hashmap.
        let mut it = expr::ExprIter::create();
        it.init(scip, exprs[0], expr::ExprIterType::Dfs, false)?;
        it.set_stages_dfs(scip, expr::EXPRITER_VISITINGCHILD);
        let mut e = it.get_current();
        while !it.is_end() {
            let child = it.get_child_expr_dfs(scip);
            if expr::expr_is_var(scip, child) {
                // Check which expression is stored in the hashmap for the var of
                // child.
                let var = crate::expr_var::get_var_expr_var(scip, child);
                let hashmapexpr = hd(scip, conshdlr).var2expr.get(&var).copied();
                // If a varexpr exists already in the hashmap, but it is child,
                // then replace child by the one in the hashmap.
                if let Some(hashmapexpr) = hashmapexpr
                    && hashmapexpr != child
                {
                    let idx = it.get_child_idx_dfs(scip) as usize;
                    expr::expr_replace_child(scip, e, idx, hashmapexpr)?;
                }
            }
            e = it.get_next(scip);
        }
        it.free(scip);
    }

    // Store variable expressions.
    if scip.set.stage > Stage::Transformed {
        store_var_exprs(scip, conshdlr, cons)?;
    }

    // Add manually locks to constraints that are not checked for feasibility.
    if !scip.conss[cons].check {
        add_locks(scip, cons, 1, 0)?;
    }

    if scip.set.stage > Stage::InitPresolve && !infeasible {
        init_solve(scip, conshdlr, &[cons])?;
    }

    // TODO deal with infeasibility.

    Ok(())
}

/// `consDeactiveNonlinear`: constraint deactivation notification method of
/// constraint handler.
fn cons_deactive_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    if scip.set.stage < Stage::ExitSolve {
        deinit_solve(scip, conshdlr, &[cons])?;
    }

    if scip.set.stage > Stage::Transformed {
        let eventhdlr = hd(scip, conshdlr).eventhdlr;
        drop_var_events(scip, eventhdlr, cons)?;
        free_var_exprs(scip, cons)?;
    }

    // Remove locks that have been added in consActiveExpr().
    if !scip.conss[cons].check {
        add_locks(scip, cons, -1, 0)?;
    }

    Ok(())
}

/// `consEnableNonlinear`: constraint enabling notification method of
/// constraint handler.
fn cons_enable_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    if scip.set.stage >= Stage::Transformed {
        let eventhdlr = hd(scip, conshdlr).eventhdlr;
        catch_var_events(scip, eventhdlr, cons)?;
    }

    Ok(())
}

/// `consDisableNonlinear`: constraint disabling notification method of
/// constraint handler.
fn cons_disable_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId) -> ScipResult {
    if scip.set.stage >= Stage::Transformed {
        let eventhdlr = hd(scip, conshdlr).eventhdlr;
        drop_var_events(scip, eventhdlr, cons)?;
    }

    Ok(())
}

/// `consCopyNonlinear`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_nonlinear(
    scip: &mut Scip,
    cons: &mut ConsId,
    name: Option<&str>,
    sourcescip: &mut Scip,
    _sourceconshdlr: ConshdlrId,
    sourcecons: ConsId,
    maps: &mut crate::copy::CopyMaps,
    flags: &crate::cons::ConsFlags,
    global: bool,
    valid: &mut bool,
) -> ScipResult {
    let (sourceexpr, lhs, rhs) = {
        let c = cd(sourcescip, sourcecons);
        (c.expr, c.lhs, c.rhs)
    };

    let targetconshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);

    let (mut targetexpr, copyvalid) = crate::scip_expr::copy_expr(sourcescip, scip, sourceexpr, Some(exprowner_create), targetconshdlr, Some(maps), global)?;
    *valid = copyvalid;

    if targetexpr == NONE {
        *valid = false;
    }

    *cons = NONE;
    if *valid {
        // Create copy (only capture targetexpr, no need to copy again).
        let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);
        *cons = create_cons(scip, targetconshdlr, &consname, targetexpr, lhs, rhs, false, flags)?;
    }

    if targetexpr != NONE {
        // Release target expr.
        expr::expr_release(scip, &mut targetexpr)?;
    }

    Ok(())
}

/// `consGetVarsNonlinear`: constraint method of constraint handler which
/// returns the variables (if possible).
fn cons_get_vars_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    // Store variable expressions if not done so far.
    store_var_exprs(scip, conshdlr, cons)?;

    // (The port's array grows to fit.)
    vars.clear();
    let varexprs = cd(scip, cons).varexprs.clone().unwrap_or_default();
    for ve in varexprs {
        vars.push(crate::expr_var::get_var_expr_var(scip, ve));
    }

    *success = true;

    Ok(())
}

/// `consGetNVarsNonlinear`: constraint method of constraint handler which
/// returns the number of variables (if possible).
fn cons_get_n_vars_nonlinear(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    // Store variable expressions if not done so far.
    store_var_exprs(scip, conshdlr, cons)?;

    *nvars = cd(scip, cons).nvarexprs() as i32;
    *success = true;

    Ok(())
}

/*
 * constraint handler specific interface methods
 */

/// `SCIPincludeConshdlrNonlinear`: creates the handler for nonlinear
/// constraints and includes it in SCIP.
pub fn include_conshdlr_nonlinear(scip: &mut Scip) -> ScipResult {
    // Create nonlinear constraint handler data (the parameters' storage is
    // filled when they are added below).
    let conshdlrdata = ConshdlrData {
        nlhdlrs: Vec::new(),
        indetect: false,
        registerusesactivitysepabelow: false,
        registerusesactivitysepaabove: false,
        consupgrades: Vec::new(),
        eventhdlr: NONE,
        subnlpheur: NONE,
        trysolheur: NONE,
        auxvarid: 0,
        curboundstag: 1,
        lastboundrelax: 1,
        lastvaractivitymethodchange: 0,
        enforound: 0,
        lastconsindex: 0,
        intevalvar: IntEvalVar::BoundTightening,
        globalbounds: false,
        reversepropqueue: std::collections::VecDeque::with_capacity(100),
        forceboundtightening: false,
        curpropboundstag: 1,
        maxproprounds: 0,
        propauxvars: false,
        varboundrelax: 0,
        varboundrelaxamount: 0.0,
        conssiderelaxamount: 0.0,
        vp_maxperturb: 0.0,
        vp_adjfacetthreshold: 0.0,
        vp_dualsimplex: false,
        reformbinprods: false,
        reformbinprodsand: false,
        reformbinprodsfac: 0,
        forbidmultaggrnlvar: false,
        tightenlpfeastol: false,
        propinenforce: false,
        weakcutthreshold: 0.0,
        strongcutmaxcoef: 0.0,
        strongcutefficacy: false,
        forcestrongcut: false,
        enfoauxviolfactor: 0.0,
        weakcutminviolfactor: 0.0,
        rownotremovable: 0,
        violscale: 0,
        checkvarlocks: 0,
        branchauxmindepth: 0,
        branchexternal: false,
        branchhighviolfactor: 0.0,
        branchhighscorefactor: 0.0,
        branchviolweight: 0.0,
        branchfracweight: 0.0,
        branchdualweight: 0.0,
        branchpscostweight: 0.0,
        branchdomainweight: 0.0,
        branchvartypeweight: 0.0,
        branchscoreagg: 0,
        branchviolsplit: 0,
        branchpscostreliable: 0.0,
        branchmixfractional: 0.0,
        linearizeheursol: 0,
        assumeconvex: false,
        nweaksepa: 0,
        ntightenlp: 0,
        ndesperatetightenlp: 0,
        ndesperatebranch: 0,
        ndesperatecutoff: 0,
        nforcelp: 0,
        canonicalizetime: crate::clock::Clock::new(),
        ncanonicalizecalls: 0,
        vp_randnumgen: None,
        vp_lp: (0..=MAXVERTEXPOLYDIM).map(|_| None).collect(),
        bilinhashtable: None,
        bilinterms: None,
        bilinmaxnauxexprs: 0,
        branchrandnumgen: None,
        branchpscostupdatestrategy: 0,
        checkedvarlocks: false,
        var2expr: HashMap::new(),
        newsoleventfilterpos: -1,
    };

    // Include constraint handler.
    if crate::set::set_find_conshdlr(scip, CONSHDLR_NAME) != NONE {
        log::error!("constraint handler <{CONSHDLR_NAME}> already included.");
        return Err(Retcode::InvalidData);
    }
    let methods = crate::cons::ConshdlrMethods {
        conshdlrcopy: Some(conshdlr_copy_nonlinear),
        consfree: Some(cons_free_nonlinear),
        consinit: Some(cons_init_nonlinear),
        consexit: Some(cons_exit_nonlinear),
        consinitpre: None,
        consexitpre: Some(cons_exitpre_nonlinear),
        consinitsol: Some(cons_initsol_nonlinear),
        consexitsol: Some(cons_exitsol_nonlinear),
        consdelete: Some(cons_delete_nonlinear),
        constrans: Some(cons_trans_nonlinear),
        consinitlp: Some(cons_initlp_nonlinear),
        conssepalp: Some(cons_sepalp_nonlinear),
        conssepasol: Some(cons_sepasol_nonlinear),
        consenfolp: Some(cons_enfolp_nonlinear),
        consenforelax: Some(cons_enforelax_nonlinear),
        consenfops: Some(cons_enfops_nonlinear),
        conscheck: Some(cons_check_nonlinear),
        consprop: Some(cons_prop_nonlinear),
        conspresol: Some(cons_presol_nonlinear),
        consresprop: None,
        conslock: Some(cons_lock_nonlinear),
        consactive: Some(cons_active_nonlinear),
        consdeactive: Some(cons_deactive_nonlinear),
        consenable: Some(cons_enable_nonlinear),
        consdisable: Some(cons_disable_nonlinear),
        consdelvars: None,
        conscopy: Some(cons_copy_nonlinear),
        consgetvars: Some(cons_get_vars_nonlinear),
        consgetnvars: Some(cons_get_n_vars_nonlinear),
        consgetdivebdchgs: None,
    };
    let conshdlr = crate::cons::conshdlr_create(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_SEPAPRIORITY,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_SEPAFREQ,
        CONSHDLR_PROPFREQ,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_MAXPREROUNDS,
        CONSHDLR_DELAYSEPA,
        CONSHDLR_DELAYPROP,
        CONSHDLR_NEEDSCONS,
        CONSHDLR_PROP_TIMING,
        CONSHDLR_PRESOLTIMING,
        methods,
        Some(Box::new(conshdlrdata)),
    )?;
    crate::set::set_include_conshdlr(scip, conshdlr);

    // Add nonlinear constraint handler parameters.
    // TODO organize into more subcategories.
    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, $f)
        };
    }
    use crate::paramset::{paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_real};
    let infinity = scip.set.infinity();
    let epsilon = scip.set.epsilon();
    paramset_add_int(
        scip,
        "constraints/nonlinear/maxproprounds",
        "limit on number of propagation rounds for a set of constraints within one round of SCIP propagation",
        t!(i32, maxproprounds),
        false,
        10,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/propauxvars",
        "whether to check bounds of all auxiliary variable to seed reverse propagation",
        t!(bool, propauxvars),
        true,
        true,
        None,
    )?;
    paramset_add_char(
        scip,
        "constraints/nonlinear/varboundrelax",
        "strategy on how to relax variable bounds during bound tightening: relax (n)ot, relax by (a)bsolute value, relax always by a(b)solute value, relax by (r)relative value",
        t!(u8, varboundrelax),
        true,
        b'r',
        Some("nabr"),
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/varboundrelaxamount",
        "by how much to relax variable bounds during bound tightening if strategy 'a', 'b', or 'r'",
        t!(f64, varboundrelaxamount),
        true,
        epsilon,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/conssiderelaxamount",
        "by how much to relax constraint sides during bound tightening",
        t!(f64, conssiderelaxamount),
        true,
        epsilon,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/vpmaxperturb",
        "maximal relative perturbation of reference point when computing facet of envelope of vertex-polyhedral function (dim>2)",
        t!(f64, vp_maxperturb),
        true,
        VERTEXPOLY_MAXPERTURBATION,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/vpadjfacetthresh",
        "adjust computed facet of envelope of vertex-polyhedral function up to a violation of this value times LP feasibility tolerance",
        t!(f64, vp_adjfacetthreshold),
        true,
        VERTEXPOLY_ADJUSTFACETFACTOR,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/vpdualsimplex",
        "whether to use dual simplex instead of primal simplex for LP that computes facet of vertex-polyhedral function",
        t!(bool, vp_dualsimplex),
        true,
        VERTEXPOLY_USEDUALSIMPLEX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/nonlinear/bilinmaxnauxexprs",
        "maximal number of auxiliary expressions per bilinear term",
        t!(i32, bilinmaxnauxexprs),
        false,
        BILIN_MAXNAUXEXPRS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/reformbinprods",
        "whether to reformulate products of binary variables during presolving",
        t!(bool, reformbinprods),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/reformbinprodsand",
        "whether to use the AND constraint handler for reformulating binary products",
        t!(bool, reformbinprodsand),
        false,
        true,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/nonlinear/reformbinprodsfac",
        "minimum number of terms to reformulate bilinear binary products by factorizing variables (<= 1: disabled)",
        t!(i32, reformbinprodsfac),
        false,
        50,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/forbidmultaggrnlvar",
        "whether to forbid multiaggregation of nonlinear variables",
        t!(bool, forbidmultaggrnlvar),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/tightenlpfeastol",
        "whether to tighten LP feasibility tolerance during enforcement, if it seems useful",
        t!(bool, tightenlpfeastol),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/propinenforce",
        "whether to (re)run propagation in enforcement",
        t!(bool, propinenforce),
        true,
        false,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/weakcutthreshold",
        "threshold for when to regard a cut from an estimator as weak (lower values allow more weak cuts)",
        t!(f64, weakcutthreshold),
        true,
        0.2,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/strongcutmaxcoef",
        "\"strong\" cuts will be scaled to have their maximal coef in [1/strongcutmaxcoef,strongcutmaxcoef]",
        t!(f64, strongcutmaxcoef),
        true,
        1000.0,
        1.0,
        infinity,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/strongcutefficacy",
        "consider efficacy requirement when deciding whether a cut is \"strong\"",
        t!(bool, strongcutefficacy),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/forcestrongcut",
        "whether to force \"strong\" cuts in enforcement",
        t!(bool, forcestrongcut),
        true,
        false,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/enfoauxviolfactor",
        "an expression will be enforced if the \"auxiliary\" violation is at least this factor times the \"original\" violation",
        t!(f64, enfoauxviolfactor),
        true,
        0.01,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/weakcutminviolfactor",
        "retry enfo of constraint with weak cuts if violation is least this factor of maximal violated constraints",
        t!(f64, weakcutminviolfactor),
        true,
        0.5,
        0.0,
        2.0,
        None,
    )?;
    paramset_add_char(
        scip,
        "constraints/nonlinear/rownotremovable",
        "whether to make rows to be non-removable in the node where they are added (can prevent some cycling): 'o'ff, in 'e'nforcement only, 'a'lways",
        t!(u8, rownotremovable),
        true,
        b'o',
        Some("oea"),
        None,
    )?;
    paramset_add_char(
        scip,
        "constraints/nonlinear/violscale",
        "method how to scale violations to make them comparable (not used for feasibility check): (n)one, (a)ctivity and side, norm of (g)radient",
        t!(u8, violscale),
        true,
        b'n',
        Some("nag"),
        None,
    )?;
    paramset_add_char(
        scip,
        "constraints/nonlinear/checkvarlocks",
        "whether variables contained in a single constraint should be forced to be at their lower or upper bounds ('d'isable, change 't'ype, add 'b'ound disjunction)",
        t!(u8, checkvarlocks),
        true,
        b't',
        Some("bdt"),
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/nonlinear/branching/aux",
        "from which depth on in the tree to allow branching on auxiliary variables (variables added for extended formulation)",
        t!(i32, branchauxmindepth),
        false,
        i32::MAX,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/branching/external",
        "whether to use external branching candidates and branching rules for branching",
        t!(bool, branchexternal),
        false,
        false,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/highviolfactor",
        "consider a constraint highly violated if its violation is >= this factor * maximal violation among all constraints",
        t!(f64, branchhighviolfactor),
        false,
        0.0,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/highscorefactor",
        "consider a variable branching score high if its branching score >= this factor * maximal branching score among all variables",
        t!(f64, branchhighscorefactor),
        false,
        0.9,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/violweight",
        "weight by how much to consider the violation assigned to a variable for its branching score",
        t!(f64, branchviolweight),
        false,
        1.0,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/fracweight",
        "weight by how much to consider fractionality of integer variables in branching score for spatial branching",
        t!(f64, branchfracweight),
        false,
        1.0,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/dualweight",
        "weight by how much to consider the dual values of rows that contain a variable for its branching score",
        t!(f64, branchdualweight),
        false,
        0.0,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/pscostweight",
        "weight by how much to consider the pseudo cost of a variable for its branching score",
        t!(f64, branchpscostweight),
        false,
        1.0,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/domainweight",
        "weight by how much to consider the domain width in branching score",
        t!(f64, branchdomainweight),
        false,
        0.0,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/vartypeweight",
        "weight by how much to consider variable type (continuous: 0, binary: 1, integer: 0.1, impl-integer: 0.01) in branching score",
        t!(f64, branchvartypeweight),
        false,
        0.5,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_char(
        scip,
        "constraints/nonlinear/branching/scoreagg",
        "how to aggregate several branching scores given for the same expression: 'a'verage, 'm'aximum, 's'um",
        t!(u8, branchscoreagg),
        false,
        b's',
        Some("ams"),
        None,
    )?;
    paramset_add_char(
        scip,
        "constraints/nonlinear/branching/violsplit",
        "method used to split violation in expression onto variables: 'u'niform, 'm'idness of solution, 'd'omain width, 'l'ogarithmic domain width",
        t!(u8, branchviolsplit),
        false,
        b'm',
        Some("umdl"),
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/pscostreliable",
        "minimum pseudo-cost update count required to consider pseudo-costs reliable",
        t!(f64, branchpscostreliable),
        false,
        2.0,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/nonlinear/branching/mixfractional",
        "minimal average pseudo cost count for discrete variables at which to start considering spatial branching before branching on fractional integer variables",
        t!(f64, branchmixfractional),
        false,
        infinity,
        0.0,
        infinity,
        None,
    )?;
    paramset_add_char(
        scip,
        "constraints/nonlinear/linearizeheursol",
        "whether tight linearizations of nonlinear constraints should be added to cutpool when some heuristics finds a new solution ('o'ff, on new 'i'ncumbents, on 'e'very solution)",
        t!(u8, linearizeheursol),
        false,
        b'o',
        Some("oie"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/nonlinear/assumeconvex",
        "whether to assume that any constraint in the presolved problem is convex",
        t!(bool, assumeconvex),
        true,
        false,
        None,
    )?;

    // Include handler for bound change events.
    let eventhdlr = crate::api::include_eventhdlr_basic(
        scip,
        &format!("{CONSHDLR_NAME}_boundchange"),
        "signals a bound change to a nonlinear constraint",
        process_var_event,
        None,
    )?;
    hdm(scip, conshdlr).eventhdlr = eventhdlr;

    // (The statistics tables and the display dialog are not ported.)

    crate::api::include_eventhdlr_basic(
        scip,
        &format!("{CONSHDLR_NAME}_newsolution"),
        "handles the event that a new primal solution has been found",
        process_new_solution_event,
        None,
    )?;

    Ok(())
}

/// `SCIPincludeConsUpgradeNonlinear`: includes a nonlinear constraint
/// upgrade method into the nonlinear constraint handler.
pub fn include_cons_upgrade_nonlinear(scip: &mut Scip, nlconsupgd: NonlinConsUpgd, priority: i32, active: bool, conshdlrname: &str) -> ScipResult {
    // Find the nonlinear constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        log::error!("nonlinear constraint handler not found");
        return Err(Retcode::PluginNotFound);
    }

    // Check whether upgrade method exists already.
    if hd(scip, conshdlr).consupgrades.iter().rev().any(|u| std::ptr::fn_addr_eq(u.consupgd, nlconsupgd)) {
        return Ok(());
    }

    // Create a nonlinear constraint upgrade data object and insert it into
    // constraint handler data.
    let consupgrade = ConsUpgrade {
        consupgd: nlconsupgd,
        priority,
        active,
        conshdlrname: conshdlrname.to_string(),
    };
    let ups = &mut hdm(scip, conshdlr).consupgrades;
    let mut i = ups.len();
    while i > 0 && ups[i - 1].priority < consupgrade.priority {
        i -= 1;
    }
    ups.insert(i, consupgrade);

    // Adds parameter to turn on and off the upgrading step (the upgrade is
    // found by its handler's name, as its position moves with later
    // insertions).
    let paramname = format!("constraints/{CONSHDLR_NAME}/upgrade/{conshdlrname}");
    let paramdesc = format!("enable nonlinear upgrading for constraint handler <{conshdlrname}>");
    let key = conshdlrname.to_string();
    let key2 = key.clone();
    let target = Some(std::rc::Rc::new(crate::paramset::Target::<bool> {
        get: Box::new(move |scip: &Scip| &hd(scip, conshdlr).consupgrades.iter().find(|u| u.conshdlrname == key).unwrap().active),
        get_mut: Box::new(move |scip: &mut Scip| &mut hdm(scip, conshdlr).consupgrades.iter_mut().find(|u| u.conshdlrname == key2).unwrap().active),
    }));
    crate::paramset::paramset_add_bool(scip, &paramname, &paramdesc, target, false, active, None)?;

    Ok(())
}

/// `SCIPcreateConsNonlinear`: creates and captures a nonlinear constraint
/// (the expression is copied).
pub fn create_cons_nonlinear(scip: &mut Scip, name: &str, expr: ExprId, lhs: f64, rhs: f64, flags: &crate::cons::ConsFlags) -> ScipResult<ConsId> {
    // Find the nonlinear constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        log::error!("nonlinear constraint handler not found");
        return Err(Retcode::PluginNotFound);
    }

    // Create constraint.
    create_cons(scip, conshdlr, name, expr, lhs, rhs, true, flags)
}

/// `SCIPcreateConsBasicNonlinear`: creates and captures a nonlinear
/// constraint with all its constraint flags set to their default values.
pub fn create_cons_basic_nonlinear(scip: &mut Scip, name: &str, expr: ExprId, lhs: f64, rhs: f64) -> ScipResult<ConsId> {
    create_cons_nonlinear(scip, name, expr, lhs, rhs, &crate::cons::ConsFlags::default())
}

/// `SCIPcreateConsQuadraticNonlinear`: creates and captures a quadratic
/// nonlinear constraint.
#[allow(clippy::too_many_arguments)]
pub fn create_cons_quadratic_nonlinear(
    scip: &mut Scip,
    name: &str,
    linvars: &[VarId],
    lincoefs: &[f64],
    quadvars1: &[VarId],
    quadvars2: &[VarId],
    quadcoefs: &[f64],
    lhs: f64,
    rhs: f64,
    flags: &crate::cons::ConsFlags,
) -> ScipResult<ConsId> {
    // Check data for infinity or nan values.
    for (i, &c) in lincoefs.iter().enumerate() {
        if !c.is_finite() || scip.set.is_infinity(c) {
            log::error!("Infinite or nan coefficient of variable {} in quadratic constraint {name}", scip.vars[linvars[i]].name);
            return Err(Retcode::InvalidData);
        }
    }

    for (i, &c) in quadcoefs.iter().enumerate() {
        if !c.is_finite() || scip.set.is_infinity(c) {
            log::error!(
                "Infinite or nan coefficient of term {}*{} in quadratic constraint {name}",
                scip.vars[quadvars1[i]].name,
                scip.vars[quadvars2[i]].name
            );
            return Err(Retcode::InvalidData);
        }
    }

    // lhs and rhs will be checked in createCons.

    // Get nonlinear constraint handler.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        log::error!("nonlinear constraint handler not found");
        return Err(Retcode::PluginNotFound);
    }

    // Create quadratic expression.
    let mut expr = crate::scip_expr::create_expr_quadratic(scip, linvars, lincoefs, quadvars1, quadvars2, quadcoefs, Some(exprowner_create), conshdlr)?;

    // Create nonlinear constraint.
    let cons = create_cons(scip, conshdlr, name, expr, lhs, rhs, false, flags)?;

    // Release quadratic expression (captured by constraint now).
    expr::expr_release(scip, &mut expr)?;

    Ok(cons)
}

/// `SCIPcreateConsBasicQuadraticNonlinear`: creates and captures a
/// quadratic nonlinear constraint with all its constraint flags set to their
/// default values.
#[allow(clippy::too_many_arguments)]
pub fn create_cons_basic_quadratic_nonlinear(
    scip: &mut Scip,
    name: &str,
    linvars: &[VarId],
    lincoefs: &[f64],
    quadvars1: &[VarId],
    quadvars2: &[VarId],
    quadcoefs: &[f64],
    lhs: f64,
    rhs: f64,
) -> ScipResult<ConsId> {
    create_cons_quadratic_nonlinear(scip, name, linvars, lincoefs, quadvars1, quadvars2, quadcoefs, lhs, rhs, &crate::cons::ConsFlags::default())
}

/// `SCIPcreateConsBasicSOCNonlinear`: creates and captures a nonlinear
/// constraint that is a second-order cone constraint with all its
/// constraint flags set to their default values:
/// sqrt(gamma + sum_i (alpha_i (x_i + beta_i))^2) <= alpha_{n+1} (x_{n+1} +
/// beta_{n+1}).
#[allow(clippy::too_many_arguments)]
pub fn create_cons_basic_soc_nonlinear(
    scip: &mut Scip,
    name: &str,
    vars: &[VarId],
    coefs: Option<&[f64]>,
    offsets: Option<&[f64]>,
    constant: f64,
    rhsvar: VarId,
    rhscoeff: f64,
    rhsoffset: f64,
) -> ScipResult<ConsId> {
    use crate::{expr_pow::create_expr_pow, expr_sum::create_expr_sum, expr_var::create_expr_var as create_var_expr};

    // Check values for infinity or nan.
    for (i, &var) in vars.iter().enumerate() {
        if let Some(c) = coefs
            && (!c[i].is_finite() || scip.set.is_infinity(c[i]))
        {
            log::error!(
                "Second-order cone term with infinite or nan coefficient of variable {} in nonlinear constraint {name}",
                scip.vars[var].name
            );
            return Err(Retcode::InvalidData);
        }
        if let Some(o) = offsets
            && (!o[i].is_finite() || scip.set.is_infinity(o[i]))
        {
            log::error!(
                "Second-order cone term with infinite or nan offset for variable {} in nonlinear constraint {name}",
                scip.vars[var].name
            );
            return Err(Retcode::InvalidData);
        }
    }
    if !constant.is_finite() || scip.set.is_infinity(constant) {
        log::error!("Second-order cone constant with infinite or nan value in nonlinear constraint {name}");
        return Err(Retcode::InvalidData);
    }
    if !rhscoeff.is_finite() || scip.set.is_infinity(rhscoeff) {
        log::error!("Infinite or nan coefficient of right hand side variable in second-order cone constraint {name}");
        return Err(Retcode::InvalidData);
    }
    if !rhsoffset.is_finite() || scip.set.is_infinity(rhsoffset) {
        log::error!("Infinite or nan right hand side offset in second-order cone constraint {name}");
        return Err(Retcode::InvalidData);
    }

    // lhs and rhs will be checked in createCons.

    // gamma
    let mut lhssum = create_expr_sum(scip, &[], None, constant, None, 0)?;
    for i in 0..vars.len() {
        // x_i
        let mut varexpr = create_var_expr(scip, vars[i], None, 0)?;
        let mut powexpr;
        if let Some(o) = offsets
            && o[i] != 0.0
        {
            // x_i + beta_i
            let mut sum = create_expr_sum(scip, &[varexpr], None, o[i], None, 0)?;
            // (x_i + beta_i)^2
            powexpr = create_expr_pow(scip, sum, 2.0, None, 0)?;
            expr::expr_release(scip, &mut sum)?;
        } else {
            // x_i^2
            powexpr = create_expr_pow(scip, varexpr, 2.0, None, 0)?;
        }

        // + alpha_i^2 (x_i + beta_i)^2
        crate::expr_sum::append_expr_sum_expr(scip, lhssum, powexpr, coefs.map_or(1.0, |c| c[i] * c[i]));
        expr::expr_release(scip, &mut varexpr)?;
        expr::expr_release(scip, &mut powexpr)?;
    }

    // sqrt(...)
    let mut term0 = create_expr_pow(scip, lhssum, 0.5, None, 0)?;
    expr::expr_release(scip, &mut lhssum)?;

    // x_{n+1}
    let mut term1 = create_var_expr(scip, rhsvar, None, 0)?;

    // sqrt(...) - alpha_{n+1}x_{n_1}
    let mut e = create_expr_sum(scip, &[term0, term1], Some(&[1.0, -rhscoeff]), 0.0, None, 0)?;

    expr::expr_release(scip, &mut term1)?;
    expr::expr_release(scip, &mut term0)?;

    let ninf = -scip.set.infinity();
    let cons = create_cons_basic_nonlinear(scip, name, e, ninf, rhscoeff * rhsoffset)?;

    expr::expr_release(scip, &mut e)?;

    Ok(cons)
}

/// `SCIPcreateConsBasicSignpowerNonlinear`: creates and captures a
/// signpower nonlinear constraint with all its constraint flags set to their
/// default values: lhs <= sign(x+a) |x+a|^n + c z <= rhs.
#[allow(clippy::too_many_arguments)]
pub fn create_cons_basic_signpower_nonlinear(scip: &mut Scip, name: &str, x: VarId, z: VarId, exponent: f64, xoffset: f64, zcoef: f64, lhs: f64, rhs: f64) -> ScipResult<ConsId> {
    use crate::{expr_pow::create_expr_signpower, expr_sum::create_expr_sum, expr_var::create_expr_var as create_var_expr};

    if !exponent.is_finite() {
        log::error!("exponent in nonlinear signpower constraint <{name}> is infinite or nan");
        return Err(Retcode::InvalidData);
    }
    if !xoffset.is_finite() {
        log::error!("argument offset in nonlinear signpower constraint <{name}> is infinite or nan");
        return Err(Retcode::InvalidData);
    }
    if !zcoef.is_finite() {
        log::error!("coefficient of linear variable in nonlinear signpower constraint <{name}> is infinite or nan");
        return Err(Retcode::InvalidData);
    }

    let mut xexpr = create_var_expr(scip, x, None, 0)?;
    let mut term0 = if xoffset != 0.0 {
        // x + xoffset
        let mut sumexpr = create_expr_sum(scip, &[xexpr], None, xoffset, None, 0)?;
        // signpow(x + xoffset, exponent)
        let t = create_expr_signpower(scip, sumexpr, exponent, None, 0)?;
        expr::expr_release(scip, &mut sumexpr)?;
        t
    } else {
        // signpow(x, exponent)
        create_expr_signpower(scip, xexpr, exponent, None, 0)?
    };

    let mut term1 = create_var_expr(scip, z, None, 0)?;

    // signpowexpr + zcoef * z
    let mut sumexpr = create_expr_sum(scip, &[term0, term1], Some(&[1.0, zcoef]), 0.0, None, 0)?;

    let cons = create_cons_basic_nonlinear(scip, name, sumexpr, lhs, rhs)?;

    expr::expr_release(scip, &mut sumexpr)?;
    expr::expr_release(scip, &mut term1)?;
    expr::expr_release(scip, &mut term0)?;
    expr::expr_release(scip, &mut xexpr)?;

    Ok(cons)
}

/// `SCIPgetCurBoundsTagNonlinear`: gets tag indicating current local
/// variable bounds.
pub fn get_cur_bounds_tag_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> i64 {
    hd(scip, conshdlr).curboundstag
}

/// `SCIPgetLastBoundRelaxTagNonlinear`: gets the `curboundstag` from the
/// last time where variable bounds were relaxed.
pub fn get_last_bound_relax_tag_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> i64 {
    hd(scip, conshdlr).lastboundrelax
}

/// `SCIPgetVarExprHashmapNonlinear`: returns the hashmap that is internally
/// used to map variables to their corresponding variable expressions.
pub fn get_var_expr_hashmap_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> &HashMap<VarId, ExprId> {
    &hd(scip, conshdlr).var2expr
}

/// `SCIPgetExprNonlinear`: returns the expression of the given nonlinear
/// constraint.
pub fn get_expr_nonlinear(scip: &Scip, cons: ConsId) -> ExprId {
    cd(scip, cons).expr
}

/// `SCIPgetLhsNonlinear`: gets the left hand side of a nonlinear
/// constraint.
pub fn get_lhs_nonlinear(scip: &Scip, cons: ConsId) -> f64 {
    cd(scip, cons).lhs
}

/// `SCIPgetRhsNonlinear`: gets the right hand side of a nonlinear
/// constraint.
pub fn get_rhs_nonlinear(scip: &Scip, cons: ConsId) -> f64 {
    cd(scip, cons).rhs
}

/// `SCIPgetNlRowNonlinear`: gets the nonlinear constraint as a nonlinear
/// row representation.
pub fn get_nl_row_nonlinear(scip: &mut Scip, cons: ConsId) -> ScipResult<usize> {
    if cd(scip, cons).nlrow == NONE {
        create_nl_row(scip, cons)?;
    }

    Ok(cd(scip, cons).nlrow)
}

/// `SCIPgetCurvatureNonlinear`: returns the curvature of the expression of
/// a given nonlinear constraint (computed during CONSINITSOL).
pub fn get_curvature_nonlinear(scip: &Scip, cons: ConsId) -> ExprCurv {
    cd(scip, cons).curv
}

/// `SCIPcheckQuadraticNonlinear`: checks whether expression of constraint
/// can be represented as quadratic form; only TRUE if the whole expression
/// is quadratic (in the non-extended formulation) and non-linear.
pub fn check_quadratic_nonlinear(scip: &mut Scip, cons: ConsId) -> ScipResult<bool> {
    let e = cd(scip, cons).expr;

    // Check whether constraint expression is quadratic in extended
    // formulation.
    let mut isquadratic = expr::expr_check_quadratic(scip, e)?;

    // If not quadratic in non-extended formulation, then do indicate
    // quadratic.
    if isquadratic {
        isquadratic = expr::expr_are_quadratic_exprs_variables(scip, e);
    }

    Ok(isquadratic)
}

/// `SCIPchgLhsNonlinear`: changes left-hand-side of a nonlinear constraint
/// (only in the problem stage).
pub fn chg_lhs_nonlinear(scip: &mut Scip, cons: ConsId, lhs: f64) -> ScipResult {
    if scip.set.stage != Stage::Problem {
        log::error!("SCIPchgLhsNonlinear can only be called in problem stage.");
        return Err(Retcode::InvalidCall);
    }

    // We should have an original constraint.
    let c = cdm(scip, cons);
    if c.lhs == lhs {
        return Ok(());
    }

    c.lhs = lhs;

    // Not sure we care about any of these flags for original constraints.
    c.ispropagated = false;

    Ok(())
}

/// `SCIPchgRhsNonlinear`: changes right-hand-side of a nonlinear constraint
/// (only in the problem stage).
pub fn chg_rhs_nonlinear(scip: &mut Scip, cons: ConsId, rhs: f64) -> ScipResult {
    if scip.set.stage != Stage::Problem {
        log::error!("SCIPchgLhsNonlinear can only be called in problem stage.");
        return Err(Retcode::InvalidCall);
    }

    // We should have an original constraint.
    let c = cdm(scip, cons);
    if c.rhs == rhs {
        return Ok(());
    }

    c.rhs = rhs;

    // Not sure we care about any of these flags for original constraints.
    c.ispropagated = false;

    Ok(())
}

/// `SCIPchgExprNonlinear`: changes expression of a nonlinear constraint
/// (only in the problem stage).
pub fn chg_expr_nonlinear(scip: &mut Scip, cons: ConsId, newexpr: ExprId) -> ScipResult {
    if scip.set.stage != Stage::Problem {
        log::error!("SCIPchgExprNonlinear can only be called in problem stage.");
        return Err(Retcode::InvalidCall);
    }

    // We should have an original constraint.
    let conshdlr = cons_hdlr(scip, cons);

    let mut old = cd(scip, cons).expr;
    expr::expr_release(scip, &mut old)?;

    // Copy expression, thereby map variables expressions to already existing
    // variables expressions in var2expr map, or augment var2expr map.
    let e = crate::scip_expr::duplicate_expr(scip, newexpr, Some(mapexprvar), conshdlr, Some(exprowner_create), conshdlr)?;

    // Not sure we care about any of these flags for original constraints.
    let c = cdm(scip, cons);
    c.expr = e;
    c.curv = ExprCurv::Unknown;
    c.issimplified = false;
    c.ispropagated = false;

    Ok(())
}

/// `SCIPaddLinearVarNonlinear`: adds coef * var to nonlinear constraint
/// (only in the problem stage).
pub fn add_linear_var_nonlinear(scip: &mut Scip, cons: ConsId, var: VarId, coef: f64) -> ScipResult {
    if scip.set.stage != Stage::Problem {
        log::error!("SCIPaddLinearVarNonlinear can only be called in problem stage.");
        return Err(Retcode::InvalidCall);
    }

    // We should have an original constraint.

    if coef == 0.0 {
        return Ok(());
    }

    let conshdlr = cons_hdlr(scip, cons);

    let mut varexpr = create_expr_var(scip, conshdlr, var)?;

    // Append to sum, if consdata->expr is sum and not used anywhere else.
    let e = cd(scip, cons).expr;
    if scip.exprs[e].nuses == 1 && expr::expr_is_sum(scip, e) {
        crate::expr_sum::append_expr_sum_expr(scip, e, varexpr, coef);
    } else {
        // Create new expression = 1 * consdata->expr + coef * var.
        let ne = crate::expr_sum::create_expr_sum(scip, &[e, varexpr], Some(&[1.0, coef]), 0.0, Some(exprowner_create), conshdlr)?;
        cdm(scip, cons).expr = ne;

        // Release old root expr.
        let mut old = e;
        expr::expr_release(scip, &mut old)?;
    }

    expr::expr_release(scip, &mut varexpr)?;

    // Not sure we care about any of these flags for original constraints.
    let c = cdm(scip, cons);
    c.issimplified = false;
    c.ispropagated = false;

    Ok(())
}

/// `SCIPaddExprNonlinear`: adds coef * expr to nonlinear constraint (only
/// in the problem stage).
pub fn add_expr_nonlinear(scip: &mut Scip, cons: ConsId, addexpr: ExprId, coef: f64) -> ScipResult {
    if scip.set.stage != Stage::Problem {
        log::error!("SCIPaddExprNonlinear can only be called in problem stage.");
        return Err(Retcode::InvalidCall);
    }

    // We should have an original constraint.

    if coef == 0.0 {
        return Ok(());
    }

    let conshdlr = cons_hdlr(scip, cons);

    // Free quadratic representation, if any is stored.
    let e = cd(scip, cons).expr;
    expr::expr_free_quadratic(scip, e);

    // Free varexprs in consdata, in case they have been stored (e.g., by a
    // call to consGet(N)VarsNonlinear).
    free_var_exprs(scip, cons)?;

    // Copy expression, thereby map variables expressions to already existing
    // variables expressions in var2expr map, or augment var2expr map.
    let mut exprowned = crate::scip_expr::duplicate_expr(scip, addexpr, Some(mapexprvar), conshdlr, Some(exprowner_create), conshdlr)?;

    // Append to sum, if consdata->expr is sum and not used anywhere else.
    if scip.exprs[e].nuses == 1 && expr::expr_is_sum(scip, e) {
        crate::expr_sum::append_expr_sum_expr(scip, e, exprowned, coef);
    } else {
        // Create new expression = 1 * consdata->expr + coef * var.
        let ne = crate::expr_sum::create_expr_sum(scip, &[e, exprowned], Some(&[1.0, coef]), 0.0, Some(exprowner_create), conshdlr)?;
        cdm(scip, cons).expr = ne;

        // Release old root expr.
        let mut old = e;
        expr::expr_release(scip, &mut old)?;
    }

    expr::expr_release(scip, &mut exprowned)?;

    // Not sure we care about any of these flags for original constraints.
    let c = cdm(scip, cons);
    c.issimplified = false;
    c.ispropagated = false;

    Ok(())
}

/// `SCIPgetExprActivityNonlinear`: computes value of constraint expression
/// in a given solution (`INVALID` in case of a domain error).
pub fn get_expr_activity_nonlinear(scip: &mut Scip, cons: ConsId, sol: SolId) -> ScipResult<f64> {
    let e = cd(scip, cons).expr;
    expr::expr_eval(scip, e, sol, 0)?;
    Ok(scip.exprs[e].evalvalue)
}

/// `SCIPgetAbsViolationNonlinear`: gets absolute violation of nonlinear
/// constraint (evaluates the constraint in the given solution).
pub fn get_abs_violation_nonlinear(scip: &mut Scip, cons: ConsId, sol: SolId) -> ScipResult<f64> {
    compute_violation(scip, cons, sol, 0)?;
    Ok(get_cons_abs_violation(scip, cons))
}

/// `SCIPgetRelViolationNonlinear`: gets scaled violation of nonlinear
/// constraint (evaluates the constraint in the given solution; the scaling
/// depends on constraints/nonlinear/violscale).
pub fn get_rel_violation_nonlinear(scip: &mut Scip, cons: ConsId, sol: SolId) -> ScipResult<f64> {
    compute_violation(scip, cons, sol, 0)?;
    get_cons_rel_violation(scip, cons, sol, 0)
}

/// `SCIPgetLinvarMayDecreaseNonlinear`: returns a variable that appears
/// linearly that may be decreased without making any other constraint
/// infeasible, and its coefficient.
pub fn get_linvar_may_decrease_nonlinear(scip: &mut Scip, cons: ConsId) -> (VarId, f64) {
    // Check for a linear variable that can be increased or decreased without
    // harming feasibility.
    find_unlocked_linear_var(scip, cons);

    let c = cd(scip, cons);
    (c.linvardecr, c.linvardecrcoef)
}

/// `SCIPgetLinvarMayIncreaseNonlinear`: returns a variable that appears
/// linearly that may be increased without making any other constraint
/// infeasible, and its coefficient.
pub fn get_linvar_may_increase_nonlinear(scip: &mut Scip, cons: ConsId) -> (VarId, f64) {
    // Check for a linear variable that can be increased or decreased without
    // harming feasibility.
    find_unlocked_linear_var(scip, cons);

    let c = cd(scip, cons);
    (c.linvarincr, c.linvarincrcoef)
}

/*
 * Methods for Expressions in Nonlinear Constraints
 */

/// `SCIPgetExprNLocksPosNonlinear`: returns the number of positive rounding
/// locks of an expression.
pub fn get_expr_n_locks_pos_nonlinear(scip: &Scip, expr: ExprId) -> i32 {
    od(scip, expr).nlockspos
}

/// `SCIPgetExprNLocksNegNonlinear`: returns the number of negative rounding
/// locks of an expression.
pub fn get_expr_n_locks_neg_nonlinear(scip: &Scip, expr: ExprId) -> i32 {
    od(scip, expr).nlocksneg
}

/// `SCIPgetExprNEnfosNonlinear`: returns the number of enforcements for an
/// expression.
pub fn get_expr_n_enfos_nonlinear(scip: &Scip, expr: ExprId) -> i32 {
    od(scip, expr).nenfos
}

/// `SCIPgetExprEnfoDataNonlinear`: returns the data for one of the
/// enforcements of an expression.
pub fn get_expr_enfo_data_nonlinear(scip: &Scip, expr: ExprId, idx: usize) -> &ExprEnfo {
    &od(scip, expr).enfos[idx]
}

/// `SCIPsetExprEnfoAuxValueNonlinear`: sets the auxiliary value of
/// expression for one of the enforcements of an expression.
pub fn set_expr_enfo_aux_value_nonlinear(scip: &mut Scip, expr: ExprId, idx: usize, auxvalue: f64) {
    odm(scip, expr).enfos[idx].auxvalue = auxvalue;
}

/// `SCIPgetExprNPropUsesActivityNonlinear`: number of nonlinear handlers
/// whose activity computation and propagation methods depend on the
/// activity of the expression.
pub fn get_expr_n_prop_uses_activity_nonlinear(scip: &Scip, expr: ExprId) -> u32 {
    od(scip, expr).nactivityusesprop
}

/// `SCIPgetExprNSepaUsesActivityNonlinear`: number of nonlinear handlers
/// whose separation methods (estimate or enforcement) depend on the activity
/// of the expression.
pub fn get_expr_n_sepa_uses_activity_nonlinear(scip: &Scip, expr: ExprId) -> u32 {
    od(scip, expr).nactivityusessepa
}

/// `SCIPgetExprNAuxvarUsesNonlinear`: number of nonlinear handlers whose
/// separation methods (estimate or enforcement) use auxiliary variable of
/// the expression.
pub fn get_expr_n_auxvar_uses_nonlinear(scip: &Scip, expr: ExprId) -> u32 {
    od(scip, expr).nauxvaruses
}

/// `SCIPincludeNlhdlrNonlinear`: creates a nonlinear handler and includes
/// it into the nonlinear constraint handler.
#[allow(clippy::too_many_arguments)]
pub fn include_nlhdlr_nonlinear(
    scip: &mut Scip,
    name: &str,
    desc: Option<&str>,
    detectpriority: i32,
    enfopriority: i32,
    detect: nlhdlr::NlhdlrDetect,
    evalaux: nlhdlr::NlhdlrEvalAux,
    nlhdlrdata: Option<Box<dyn Any>>,
) -> ScipResult<NlhdlrId> {
    // Find myself.
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    if conshdlr == NONE {
        log::error!("nonlinear constraint handler not found");
        return Err(Retcode::PluginNotFound);
    }

    // Create nlhdlr.
    let nlhdlr = nlhdlr::nlhdlr_create(scip, name, desc, detectpriority, enfopriority, detect, evalaux, nlhdlrdata)?;

    // Include into constraint handler.
    hdm(scip, conshdlr).nlhdlrs.push(nlhdlr);

    // Sort nonlinear handlers by detection priority, in decreasing order;
    // will happen in INIT, so only do when called late.
    if scip.set.stage > Stage::Init && hd(scip, conshdlr).nlhdlrs.len() > 1 {
        let mut nlhdlrs = std::mem::take(&mut hdm(scip, conshdlr).nlhdlrs);
        crate::sort::sort_down(&mut nlhdlrs, (), |&a, &b| nlhdlr::nlhdlr_comp(scip, a, b));
        hdm(scip, conshdlr).nlhdlrs = nlhdlrs;
    }

    Ok(nlhdlr)
}

/// `SCIPgetNNlhdlrsNonlinear`: get number of nonlinear handler.
pub fn get_n_nlhdlrs_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> usize {
    hd(scip, conshdlr).nlhdlrs.len()
}

/// `SCIPgetNlhdlrsNonlinear`: get nonlinear handlers.
pub fn get_nlhdlrs_nonlinear(scip: &Scip, conshdlr: ConshdlrId) -> &[NlhdlrId] {
    &hd(scip, conshdlr).nlhdlrs
}

/// `SCIPfindNlhdlrNonlinear`: returns a nonlinear handler of a given name
/// (or `NONE` if not found).
pub fn find_nlhdlr_nonlinear(scip: &Scip, conshdlr: ConshdlrId, name: &str) -> NlhdlrId {
    for &h in &hd(scip, conshdlr).nlhdlrs {
        if scip.nlhdlrs[h].name == name {
            return h;
        }
    }

    NONE
}

/// `SCIPgetNlhdlrExprDataNonlinear`: gives expression data that a given
/// nonlinear handler stored in an expression (`NONE` if expr has not been
/// detected by nlhdlr or nlhdlr did not store data).
pub fn get_nlhdlr_expr_data_nonlinear(scip: &Scip, nlhdlr: NlhdlrId, expr: ExprId) -> usize {
    let o = od(scip, expr);
    for enfo in &o.enfos[..o.nenfos.max(0) as usize] {
        if enfo.nlhdlr == nlhdlr {
            return enfo.nlhdlrexprdata;
        }
    }

    NONE
}

/// `consParseNonlinear`: constraint parsing method of constraint handler;
/// `s` is the text after the constraint's name and colon. Returns the
/// constraint, or `None` if parsing did not succeed.
pub fn parse_cons_nonlinear(scip: &mut Scip, name: &str, s: &[u8], flags: &crate::cons::ConsFlags) -> ScipResult<Option<ConsId>> {
    let conshdlr = crate::set::set_find_conshdlr(scip, CONSHDLR_NAME);
    let at = |pos: usize| -> u8 { s.get(pos).copied().unwrap_or(0) };

    // Return if string empty.
    if at(0) == 0 {
        return Ok(None);
    }

    let mut pos = 0;

    // Set left and right hand side to their default values.
    let infinity = scip.set.infinity();
    let mut lhs = -infinity;
    let mut rhs = infinity;

    // Parse constraint to get lhs, rhs, and expression in between (from
    // cons_linear.c::consparse, but parsing whole string first, then getting
    // expression).

    // Check for left hand side.
    if at(0).is_ascii_digit() || ((at(0) == b'-' || at(0) == b'+') && at(1).is_ascii_digit()) {
        // There is a number coming, maybe it is a left-hand-side.
        let Some((v, endptr)) = crate::api::parse_real(scip, s, 0) else {
            log::error!("error parsing number from <{}>", String::from_utf8_lossy(s));
            return Err(Retcode::ReadError);
        };
        lhs = v;

        // Ignore whitespace.
        let endptr = crate::misc::skip_space(s, endptr);

        if at(endptr) != b'<' || at(endptr + 1) != b'=' {
            // No '<=' coming, so it was the beginning of the expression and not a
            // left-hand-side.
            lhs = -infinity;
        } else {
            // It was indeed a left-hand-side, so continue parsing after it.
            pos = crate::misc::skip_space(s, endptr + 2);
        }
    }

    // Parse expression: so far we did not allocate memory, so can just return
    // in case of readerror.
    let (mut consexprtree, end) = crate::scip_expr::parse_expr(scip, s, pos, Some(exprowner_create), conshdlr)?;

    // Check for left or right hand side.
    let pos = crate::misc::skip_space(s, end);

    let success;

    // Check for free constraint.
    if s[pos.min(s.len())..].starts_with(b"[free]") {
        if !scip.set.is_infinity(-lhs) {
            log::error!("cannot have left hand side and [free] status ");
            expr::expr_release(scip, &mut consexprtree)?;
            return Ok(None);
        }
        success = true;
    } else {
        match at(pos) {
            b'<' => {
                success = at(pos + 1) == b'=' && crate::api::parse_real(scip, s, pos + 2).map(|(v, _)| rhs = v).is_some();
            }
            b'=' => {
                if !scip.set.is_infinity(-lhs) {
                    log::error!("cannot have == on rhs if there was a <= on lhs");
                    expr::expr_release(scip, &mut consexprtree)?;
                    return Ok(None);
                } else {
                    success = at(pos + 1) == b'=' && crate::api::parse_real(scip, s, pos + 2).map(|(v, _)| rhs = v).is_some();
                    lhs = rhs;
                }
            }
            b'>' => {
                if !scip.set.is_infinity(-lhs) {
                    log::error!("cannot have => on rhs if there was a <= on lhs");
                    expr::expr_release(scip, &mut consexprtree)?;
                    return Ok(None);
                } else {
                    success = at(pos + 1) == b'=' && crate::api::parse_real(scip, s, pos + 2).map(|(v, _)| lhs = v).is_some();
                }
            }
            0 => success = true,
            c => {
                log::error!("unexpected character {}", c as char);
                expr::expr_release(scip, &mut consexprtree)?;
                return Ok(None);
            }
        }
    }

    // Create constraint (C creates it even when the side failed to parse).
    let cons = create_cons(scip, conshdlr, name, consexprtree, lhs, rhs, false, flags)?;

    expr::expr_release(scip, &mut consexprtree)?;

    Ok(if success { Some(cons) } else { None })
}
