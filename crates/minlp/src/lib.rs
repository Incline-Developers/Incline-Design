//! Minlp: a pure Rust port of SCIP 10.0.2, the mixed-integer nonlinear
//! programming solver.
//!
//! It ports the part of SCIP that Incline's Improve uses: linear,
//! indicator and quadratic (nonconvex bilinear) constraints over
//! continuous and binary columns, solved by SCIP's spatial branch and
//! bound with its default plugins, with crates/solver's HiGHS port as the
//! LP solver where SCIP would use SoPlex.
//!
//! The port follows SCIP bit for bit: run against SCIP 10.0.2 built with
//! HiGHS 1.15.0 as its LP solver, it takes the same nodes, LP iterations,
//! cuts and solutions on the same instance and settings.

#![allow(clippy::too_many_arguments, reason = "SCIP's signatures, kept for the port")]

/// The current LP, `scip.lp` (C's `SCIP_LP*`), as a place; borrows only
/// that field.
macro_rules! lp {
    ($scip:expr) => {
        (**$scip.lp.as_mut().unwrap())
    };
}

/// The branch-and-bound tree, `scip.tree`, as a place.
macro_rules! treer {
    ($scip:expr) => {
        (**$scip.tree.as_ref().unwrap())
    };
}

macro_rules! tree {
    ($scip:expr) => {
        (**$scip.tree.as_mut().unwrap())
    };
}

pub mod api;
pub mod arena;
pub mod bandit;
pub mod branch;
pub mod branch_distribution;
pub mod branch_inference;
pub mod branch_mostinf;
pub mod branch_pscost;
pub mod branch_relpscost;
pub mod clock;
pub mod conflict;
pub mod conflictstore;
pub mod cons;
pub mod cons_and;
pub mod cons_bounddisjunction;
pub mod cons_components;
pub mod cons_indicator;
pub mod cons_integral;
pub mod cons_knapsack;
pub mod cons_linear;
pub mod cons_logicor;
pub mod cons_nonlinear;
pub mod cons_setppc;
pub mod cons_varbound;
pub mod copy;
pub mod cutpool;
pub mod cuts;
pub mod cutsel;
pub mod cutsel_hybrid;
pub mod cvec;
pub mod def;
pub mod event;
pub mod event_estim;
pub mod expr;
pub mod expr_pow;
pub mod expr_product;
pub mod expr_sum;
pub mod expr_value;
pub mod expr_var;
pub mod exprcurv;
pub mod hash;
pub mod heur;
pub mod heur_actconsdiving;
pub mod heur_adaptivediving;
pub mod heur_alns;
pub mod heur_clique;
pub mod heur_coefdiving;
pub mod heur_conflictdiving;
pub mod heur_crossover;
pub mod heur_distributiondiving;
pub mod heur_farkasdiving;
pub mod heur_feaspump;
pub mod heur_fracdiving;
pub mod heur_gins;
pub mod heur_guideddiving;
pub mod heur_indicator;
pub mod heur_indicatordiving;
pub mod heur_intshifting;
pub mod heur_linesearchdiving;
pub mod heur_locks;
pub mod heur_lpface;
pub mod heur_objpscostdiving;
pub mod heur_oneopt;
pub mod heur_pscostdiving;
pub mod heur_randrounding;
pub mod heur_rens;
pub mod heur_rins;
pub mod heur_rootsoldiving;
pub mod heur_rounding;
pub mod heur_shiftandpropagate;
pub mod heur_shifting;
pub mod heur_simplerounding;
pub mod heur_trivial;
pub mod heur_trysol;
pub mod heur_undercover;
pub mod heur_vbounds;
pub mod heur_veclendiving;
pub mod heur_zirounding;
pub mod heuristics;
pub mod history;
pub mod hypergraph;
pub mod implics;
pub mod interval;
pub mod lp;
pub mod lpi;
pub mod matrix;
pub mod misc;
pub mod misc_linear;
pub mod model;
pub mod nlhdlr;
pub mod nlhdlr_bilinear;
pub mod nlhdlr_convex;
pub mod nlhdlr_default;
pub mod nlhdlr_perspective;
pub mod nlhdlr_quadratic;
pub mod nlhdlr_quotient;
pub mod nlhdlr_signomial;
pub mod nlhdlr_soc;
pub mod nlp;
pub mod nodesel;
pub mod nodesel_estimate;
pub mod nodesel_restartdfs;
pub mod paramset;
pub mod plugins;
pub mod pqueue;
pub mod presol;
pub mod presol_domcol;
pub mod presol_dualcomp;
pub mod presol_dualsparsify;
pub mod presol_gateextraction;
pub mod presol_implics;
pub mod presol_inttobinary;
pub mod presol_sparsify;
pub mod presol_trivial;
pub mod presolve;
pub mod pricestore;
pub mod primal;
pub mod prob;
pub mod prop;
pub mod prop_dualfix;
pub mod prop_genvbounds;
pub mod prop_obbt;
pub mod prop_probing;
pub mod prop_pseudoobj;
pub mod prop_redcost;
pub mod prop_rootredcost;
pub mod prop_vbounds;
pub mod quad;
pub mod reader_cip;
pub mod relax;
pub mod relax_benders;
pub mod retcode;
pub mod rounding;
pub mod rowprep;
pub mod scip;
pub mod scip_cons;
pub mod scip_expr;
pub mod scip_lp;
pub mod scip_probing;
pub mod scip_solve;
pub mod scip_strongbranch;
pub mod scip_var;
pub mod sepa;
pub mod sepa_aggregation;
pub mod sepa_clique;
pub mod sepa_disjunctive;
pub mod sepa_flower;
pub mod sepa_gomory;
pub mod sepa_impliedbounds;
pub mod sepa_mcf;
pub mod sepa_minor;
pub mod sepa_mixing;
pub mod sepa_rlt;
pub mod sepa_zerohalf;
pub mod sepastore;
pub mod set;
pub mod sol;
pub mod solve;
pub mod sort;
pub mod stat;
pub mod tclique;
pub mod tree;
pub mod treemodel;
pub mod types;
pub mod var;
