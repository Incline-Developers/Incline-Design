//! The SCIP main data structure (`struct_scip.h`) and its stages.
//!
//! Everything C reaches by pointer is here: singletons as fields, objects
//! of which there are many (or two, like the original and transformed
//! problem) in arenas indexed by id.

use crate::{
    arena::Arena,
    branch::{BranchCand, Branchrule},
    clock::Clock,
    cons::{Cons, Conshdlr},
    event::{Event, EventFilter, EventQueue, Eventhdlr},
    implics::{Clique, CliqueTable},
    lp::{Col, Lp, Row},
    nodesel::Nodesel,
    primal::Primal,
    prob::Prob,
    relax::{Relax, Relaxation},
    retcode::ScipResult,
    set::Set,
    sol::Sol,
    stat::Stat,
    tree::{Node, Tree},
    var::Var,
};

/// `SCIP_STAGE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    #[default]
    Init = 0,
    Problem = 1,
    Transforming = 2,
    Transformed = 3,
    InitPresolve = 4,
    Presolving = 5,
    ExitPresolve = 6,
    Presolved = 7,
    InitSolve = 8,
    Solving = 9,
    Solved = 10,
    ExitSolve = 11,
    FreeTrans = 12,
    Free = 13,
}

/// The id of the original problem's primal data in `Scip::primals`.
pub const ORIGPRIMAL: usize = 0;
/// The id of the transformed problem's primal data.
pub const TRANSPRIMAL: usize = 1;
/// The id of the global event filter in `Scip::eventfilters`.
pub const GLOBALEVENTFILTER: usize = 0;

/// `SCIP`: everything one solver instance owns.
#[derive(Default)]
pub struct Scip {
    pub set: Set,
    pub totaltime: Clock,
    pub stat: Stat,
    pub probs: Arena<Prob>,
    pub primals: Arena<Primal>,
    pub eventqueue: EventQueue,
    pub eventfilters: Arena<EventFilter>,
    pub branchcand: BranchCand,
    pub lp: Option<Box<Lp>>,
    pub tree: Option<Box<Tree>>,
    pub cliquetable: Option<Box<CliqueTable>>,
    pub relaxation: Relaxation,
    pub pricestore: Option<Box<crate::pricestore::PriceStore>>,
    pub conflictstore: Option<Box<crate::conflictstore::ConflictStore>>,
    pub conflict: Option<Box<crate::conflict::Conflict>>,
    /// `scip->sepastore` and `scip->sepastoreprobing`.
    pub sepastores: Vec<crate::sepastore::SepaStore>,
    /// Set by `SCIPinterruptSolve`, read wherever SCIP checks for a stop.
    pub interrupted: bool,

    pub vars: Arena<Var>,
    pub cols: Arena<Col>,
    pub rows: Arena<Row>,
    pub conss: Arena<Cons>,
    pub nodes: Arena<Node>,
    pub sols: Arena<Sol>,
    pub cliques: Arena<Clique>,
    pub events: Arena<Event>,

    pub conshdlrs: Arena<Conshdlr>,
    pub eventhdlrs: Arena<Eventhdlr>,
    pub branchrules: Arena<Branchrule>,
    pub nodesels: Arena<Nodesel>,
    pub relaxs: Arena<Relax>,
    pub sepas: Arena<crate::sepa::Sepa>,
    pub props: Arena<crate::prop::Prop>,
    pub presols: Arena<crate::presol::Presol>,
    pub heurs: Arena<crate::heur::Heur>,
    pub divesets: Arena<crate::heur::Diveset>,
    pub cutsels: Arena<crate::cutsel::Cutsel>,
    pub conflicthdlrs: Arena<crate::conflict::Conflicthdlr>,
    pub exprhdlrs: Arena<crate::expr::Exprhdlr>,
    pub exprs: Arena<crate::expr::Expr>,
    pub nlhdlrs: Arena<crate::nlhdlr::Nlhdlr>,
    /// `scip->nlp`: the NLP relaxation, once constructed.
    pub nlp: Option<Box<crate::nlp::Nlp>>,
    /// `SCIP_NLROW`s, by id.
    pub nlrows: Arena<crate::nlp::NlRow>,
    /// `SCIP_NLHDLREXPRDATA`s, which enforcements refer to by id.
    pub nlhdlrexprdatas: Arena<Box<dyn std::any::Any>>,
    pub cuts: Arena<crate::cutpool::Cut>,
    /// `scip->cutpool` and `scip->delayedcutpool`.
    pub cutpools: Vec<crate::cutpool::Cutpool>,
}

impl Scip {
    /// `SCIPcreate`.
    pub fn create() -> ScipResult<Box<Self>> {
        let mut scip = Box::<Scip>::default();
        crate::set::set_create(&mut scip)?;
        crate::nlp::nlp_include(&mut scip)?;
        Ok(scip)
    }
}
