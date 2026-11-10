//! SCIP's shared enumerations (`type_*.h`) and the ids that stand for its
//! pointers. An id indexes its owner's arena; where SCIP numbers the
//! object itself (`var->index`, `row->index`, `node->number`), the id is
//! that number.

pub type VarId = usize;
pub type ColId = usize;
pub type RowId = usize;
pub type ConsId = usize;
pub type NodeId = usize;
pub type SolId = usize;
pub type HeurId = usize;
pub type SepaId = usize;
pub type PropId = usize;
pub type PresolId = usize;
pub type ConshdlrId = usize;
pub type BranchruleId = usize;
pub type NodeselId = usize;
pub type EventhdlrId = usize;
pub type CutselId = usize;
pub type ConflicthdlrId = usize;
pub type ExprhdlrId = usize;
pub type ExprId = usize;
pub type NlhdlrId = usize;
pub type RelaxId = usize;

macro_rules! c_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident = $value:expr),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(i32)]
        pub enum $name {
            $($variant = $value),+
        }

        /// The first variant, as C's zeroed memory mostly has it.
        impl Default for $name {
            fn default() -> Self {
                [$(Self::$variant),+][0]
            }
        }
    };
}

c_enum!(
    /// `SCIP_STATUS`.
    Status {
        Unknown = 0, Optimal = 1, Infeasible = 2, Unbounded = 3, InfOrUnbd = 4, UserInterrupt = 10, Terminate = 11,
        NodeLimit = 20, TotalNodeLimit = 21, StallNodeLimit = 22, TimeLimit = 23, MemLimit = 24, GapLimit = 25,
        PrimalLimit = 26, DualLimit = 27, SolLimit = 28, BestSolLimit = 29, RestartLimit = 30,
    }
);

c_enum!(
    /// `SCIP_BRANCHDIR`.
    BranchDir { Downwards = 0, Upwards = 1, Fixed = 2, Auto = 3 }
);

c_enum!(
    /// `SCIP_LPSOLSTAT`.
    LpSolStat { NotSolved = 0, Optimal = 1, Infeasible = 2, UnboundedRay = 3, ObjLimit = 4, IterLimit = 5, TimeLimit = 6, Error = 7 }
);

c_enum!(
    /// `SCIP_BOUNDTYPE`.
    BoundType { Lower = 0, Upper = 1 }
);

impl BoundType {
    /// `SCIPboundtypeOpposite`.
    #[inline]
    pub fn opposite(self) -> BoundType {
        if self == BoundType::Lower { BoundType::Upper } else { BoundType::Lower }
    }
}

c_enum!(
    /// `SCIP_SIDETYPE`.
    SideType { Left = 0, Right = 1 }
);

c_enum!(
    /// `SCIP_VARSTATUS`.
    VarStatus { Original = 0, Loose = 1, Column = 2, Fixed = 3, Aggregated = 4, MultAggr = 5, Negated = 6 }
);

c_enum!(
    /// `SCIP_VARTYPE`.
    VarType { Binary = 0, Integer = 1, ImplInt = 2, Continuous = 3 }
);

c_enum!(
    /// `SCIP_IMPLINTTYPE`.
    ImplintType { None = 0, Weak = 1, Strong = 2 }
);

c_enum!(
    /// `SCIP_RESULT`.
    ScipResultCode {
        DidNotRun = 1, Delayed = 2, DidNotFind = 3, Feasible = 4, Infeasible = 5, Unbounded = 6, Cutoff = 7, Separated = 8,
        NewRound = 9, ReducedDom = 10, ConsAdded = 11, ConsChanged = 12, Branched = 13, SolveLp = 14, FoundSol = 15,
        Suspended = 16, Success = 17, DelayNode = 18,
    }
);

c_enum!(
    /// `SCIP_OBJSENSE`.
    ObjSense { Maximize = -1, Minimize = 1 }
);

c_enum!(
    /// `SCIP_BOUNDCHGTYPE`.
    BoundchgType { Branching = 0, ConsInfer = 1, PropInfer = 2 }
);

c_enum!(
    /// `SCIP_LOCKTYPE`.
    LockType { Model = 0, Conflict = 1 }
);

pub const NLOCKTYPES: usize = 2;

c_enum!(
    /// `SCIP_DOMCHGTYPE`.
    DomchgType { Dynamic = 0, Both = 1, Bound = 2 }
);

c_enum!(
    /// `SCIP_LPALGO`.
    LpAlgo { PrimalSimplex = 0, DualSimplex = 1, Barrier = 2, BarrierCrossover = 3 }
);

c_enum!(
    /// `SCIP_ROWORIGINTYPE`.
    RowOriginType { Unspec = 0, Conshdlr = 1, Cons = 2, Sepa = 3, Reopt = 4 }
);

c_enum!(
    /// `SCIP_NODETYPE`.
    NodeType {
        FocusNode = 0, ProbingNode = 1, Sibling = 2, Child = 3, Leaf = 4, DeadEnd = 5, Junction = 6, PseudoFork = 7,
        Fork = 8, Subroot = 9, RefocusNode = 10,
    }
);

c_enum!(
    /// `SCIP_SOLORIGIN`.
    SolOrigin { Original = 0, Zero = 1, LpSol = 2, NlpSol = 3, RelaxSol = 4, PseudoSol = 5, Partial = 6, Unknown = 7 }
);

c_enum!(
    /// `SCIP_SOLTYPE`.
    SolType { Unknown = 0, Heur = 1, Relax = 2, LpRelax = 3, StrongBranch = 4, Pseudo = 5 }
);

c_enum!(
    /// `SCIP_EFFICIACYCHOICE`.
    EfficiacyChoice { Lp = 0, Relax = 1, Nlp = 2 }
);

c_enum!(
    /// `SCIP_CONFTYPE`.
    ConfType { Unknown = 0, Propagation = 1, InfeasLp = 2, BndExceeding = 3, AltInfProof = 4, AltBndProof = 5 }
);

c_enum!(
    /// `SCIP_CONFIDENCELEVEL`.
    ConfidenceLevel { Min = 0, Low = 1, Medium = 2, High = 3, Max = 4 }
);
