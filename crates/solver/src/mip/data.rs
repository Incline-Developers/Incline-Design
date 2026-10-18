//! HiGHS's `HighsMipSolverData`, for one worker: everything the MIP
//! search shares (the model, the global domain, the pools, the clique
//! table, implications, pseudocosts, the node queue and the incumbent).
//! The LP relaxation, the search and the heuristics are kept apart from
//! it so that each can borrow it whole while it works.

use std::cell::{Cell, RefCell};

use super::{
    MipModel, VarType,
    clique_table::{CliqueCtx, CliqueTable},
    conflict_pool::ConflictPool,
    cut_pool::CutPool,
    domain::{Domain, DomainEnv, MipCtx},
    implications::Implications,
    node_queue::NodeQueue,
    objective_function::ObjectiveFunction,
    pseudocost::Pseudocost,
    symmetry::Symmetries,
};
use crate::{Basis, integers::fractionality, util::CDouble};

const INF: f64 = f64::INFINITY;

/// The HiGHS options the MIP solver reads, with HiGHS's defaults.
#[derive(Clone, Debug)]
pub(crate) struct MipOptions {
    pub mip_feasibility_tolerance: f64,
    pub small_matrix_value: f64,
    pub random_seed: u32,
    pub objective_bound: f64,
    pub objective_target: f64,
    pub time_limit: f64,
    pub mip_abs_gap: f64,
    pub mip_rel_gap: f64,
    pub mip_pool_age_limit: usize,
    pub mip_pool_soft_limit: usize,
    pub mip_lp_age_limit: usize,
    pub mip_pscost_minreliable: i64,
    pub mip_root_presolve_only: bool,
    pub mip_allow_restart: bool,
    pub mip_lifting_for_probing: i64,
    pub mip_heuristic_effort: f64,
    pub mip_heuristic_run_feasibility_jump: bool,
    pub mip_heuristic_run_rins: bool,
    pub mip_heuristic_run_rens: bool,
    pub mip_heuristic_run_root_reduced_cost: bool,
    pub mip_heuristic_run_zi_round: bool,
    pub mip_heuristic_run_shifting: bool,
    pub mip_detect_symmetry: bool,
    pub mip_allow_cut_separation_at_nodes: bool,
    pub mip_max_nodes: i64,
    pub mip_max_stall_nodes: i64,
    pub mip_max_leaves: i64,
    pub mip_max_improving_sols: i64,
    pub presolve: bool,
    pub presolve_reduction_limit: i64,
    pub restart_presolve_reduction_limit: i64,
}

impl Default for MipOptions {
    fn default() -> Self {
        Self {
            mip_feasibility_tolerance: 1e-6,
            small_matrix_value: 1e-9,
            random_seed: 0,
            objective_bound: INF,
            objective_target: -INF,
            time_limit: INF,
            mip_abs_gap: 1e-6,
            mip_rel_gap: 1e-4,
            mip_pool_age_limit: 30,
            mip_pool_soft_limit: 10000,
            mip_lp_age_limit: 10,
            mip_pscost_minreliable: 8,
            mip_root_presolve_only: false,
            mip_allow_restart: true,
            mip_lifting_for_probing: -1,
            mip_heuristic_effort: 0.05,
            mip_heuristic_run_feasibility_jump: true,
            mip_heuristic_run_rins: true,
            mip_heuristic_run_rens: true,
            mip_heuristic_run_root_reduced_cost: true,
            mip_heuristic_run_zi_round: false,
            mip_heuristic_run_shifting: false,
            mip_detect_symmetry: true,
            mip_allow_cut_separation_at_nodes: true,
            mip_max_nodes: i32::MAX as i64,
            mip_max_stall_nodes: i32::MAX as i64,
            mip_max_leaves: i32::MAX as i64,
            mip_max_improving_sols: i32::MAX as i64,
            presolve: true,
            presolve_reduction_limit: -1,
            restart_presolve_reduction_limit: -1,
        }
    }
}

/// HiGHS's `MipSolutionSource`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SolutionSource {
    None,
    Branching,
    CentralRounding,
    FeasibilityPump,
    Heuristic,
    Shifting,
    FeasibilityJump,
    SubMip,
    EmptyMip,
    RandomizedRounding,
    SolveLp,
    EvaluateNode,
    Unbounded,
    UserSolution,
    HighsSolution,
    ZiRound,
    TrivialL,
    TrivialP,
    TrivialU,
    TrivialZ,
    Cleanup,
}

/// HiGHS's `HighsModelStatus` as the MIP solver sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MipStatus {
    NotSet,
    Optimal,
    Infeasible,
    Unbounded,
    UnboundedOrInfeasible,
    Interrupt,
    ObjectiveTarget,
    SolutionLimit,
    TimeLimit,
}

/// The parts of [`MipData`] a domain operation borrows, split so that the
/// global domain, clique table and implications can change alongside.
pub(crate) struct Parts<'a> {
    pub cx: MipCtx<'a>,
    pub domain: &'a mut Domain,
    pub cliquetable: &'a mut CliqueTable,
    pub implications: &'a mut Implications,
    pub pseudocost: &'a mut Pseudocost,
    pub symmetries: &'a Symmetries,
}

impl Parts<'_> {
    /// Run `f` on the global domain with what its operations read.
    pub fn on_domain<R>(&mut self, f: impl FnOnce(&mut Domain, &mut DomainEnv) -> R) -> R {
        let mut env = DomainEnv {
            cx: &mut self.cx,
            cliquetable: self.cliquetable,
            implications: self.implications,
        };
        f(self.domain, &mut env)
    }

    /// Run `f` on the clique table with the global domain and what it
    /// changes.
    pub fn on_cliques<R>(&mut self, f: impl FnOnce(&mut CliqueTable, &mut Domain, &mut CliqueCtx) -> R) -> R {
        let mut cc = CliqueCtx {
            cx: &mut self.cx,
            implications: self.implications,
        };
        f(self.cliquetable, self.domain, &mut cc)
    }
}

pub(crate) struct MipData {
    pub options: MipOptions,
    /// The presolved model the search works on (HiGHS's `model_`).
    pub model: MipModel,
    pub domain: Domain,
    pub cutpool: CutPool,
    pub conflictpool: ConflictPool,
    pub pseudocost: Pseudocost,
    pub cliquetable: CliqueTable,
    pub implications: Implications,
    pub objective_function: ObjectiveFunction,
    pub symmetries: Symmetries,
    pub redcostfixing: super::redcost_fixing::RedcostFixing,
    pub nodequeue: RefCell<NodeQueue>,
    pub pruned_treeweight: Cell<CDouble>,

    pub feastol: f64,
    pub epsilon: f64,
    pub heuristic_effort: f64,
    pub max_abs_row_coef: Vec<f64>,
    pub rowintegral: Vec<bool>,
    pub uplocks: Vec<usize>,
    pub downlocks: Vec<usize>,
    pub integer_cols: Vec<usize>,
    pub implint_cols: Vec<usize>,
    pub integral_cols: Vec<usize>,
    pub continuous_cols: Vec<usize>,

    pub firstlpsol: Vec<f64>,
    pub rootlpsol: Vec<f64>,
    pub firstlpsolobj: f64,
    pub firstrootbasis: Option<Basis>,
    /// `firstrootbasis.alien`: transferred over a restart.
    pub firstrootbasis_alien: bool,
    pub rootlpsolobj: f64,
    pub num_clique_entries_after_presolve: usize,
    pub num_clique_entries_after_first_presolve: usize,

    pub num_nodes: i64,
    pub num_leaves: i64,
    pub total_lp_iterations: i64,
    pub heuristic_lp_iterations: i64,
    pub sepa_lp_iterations: i64,
    pub sb_lp_iterations: i64,

    pub lower_bound: f64,
    pub upper_bound: f64,
    pub upper_limit: f64,
    pub optimality_limit: f64,
    pub incumbent: Vec<f64>,

    pub num_improving_sols: i64,
    /// Solving a sub-MIP for a heuristic (HiGHS's `mipsolver.submip`).
    pub submip: bool,
    pub submip_level: i32,
    pub max_submip_level: i32,
    /// A sub-MIP's terminator stopped (HiGHS's `termination_status_`).
    pub terminated: bool,
    pub analytic_center: Vec<f64>,
    pub firstrootlpiters: i64,
    pub avgrootlpiters: f64,
    pub total_repair_lp: i64,
    pub total_repair_lp_feasible: i64,
    pub total_repair_lp_iterations: i64,
    /// HiGHS's `mipsolver.modelstatus_`.
    pub model_status: MipStatus,
    /// HiGHS's `mipsolver.solution_objective_`: the incumbent's objective
    /// in the original model's space and sense.
    pub solution_objective: f64,
    /// The original model's sense (1 minimise, -1 maximise).
    pub orig_sense: f64,
    pub global_orbits: Option<std::rc::Rc<super::symmetry::StabilizerOrbits>>,
    /// Stops the solve as HiGHS's user interrupt callback does.
    pub interrupt: Option<crate::simplex::ekk::Interrupt>,

    /// When the solve started, for the time limit.
    pub started: web_time::Instant,

    /// The reductions presolve made, to undo on solutions.
    pub postsolve: crate::presolve::mip_postsolve::MipPostsolveStack,
    /// The row-wise copy matches the model (HiGHS's `rowMatrixSet`).
    pub row_matrix_set: bool,
    pub cliques_extracted: bool,
    pub num_restarts: i32,
    /// Pseudocosts carried over a restart (HiGHS's `mipsolver.pscostinit`).
    pub pscostinit: Option<super::pseudocost::PseudocostInitialization>,

    /// The model as given (HiGHS's `orig_model_`), with its own costs;
    /// `orig_sense` says which way.
    pub orig_model: MipModel,
    /// The incumbent in the original model's space (HiGHS's
    /// `mipsolver.solution_`) and its violations.
    pub solution: Vec<f64>,
    pub bound_violation: f64,
    pub row_violation: f64,
    pub integrality_violation: f64,
    /// A root basis in the original space for after a restart (HiGHS's
    /// `mipsolver.rootbasis`).
    pub rootbasis: Option<Basis>,
    pub num_restarts_root: i32,
    pub numintegercols: usize,
    pub max_tree_size_log2: i64,
    pub analytic_center_computed: bool,
    pub analytic_center_status: crate::simplex::ModelStatus,
    pub detect_symmetries: bool,
    pub num_leaves_before_run: i64,
    pub num_nodes_before_run: i64,
    pub total_lp_iterations_before_run: i64,
    pub heuristic_lp_iterations_before_run: i64,
    pub sepa_lp_iterations_before_run: i64,
    pub sb_lp_iterations_before_run: i64,
}

impl MipData {
    /// HiGHS's `HighsMipSolverData(mipsolver)` and `init()` over `model`,
    /// the original model, minimised (`orig_sense` says whether the costs
    /// were negated for that).
    pub fn new(options: MipOptions, model: MipModel) -> Self {
        Self::new_with_original(options, model.clone(), model, 1.0)
    }

    /// As [`MipData::new`], with the original model and its sense kept
    /// apart from the minimised one presolve works on.
    pub fn new_with_original(options: MipOptions, model: MipModel, orig_model: MipModel, orig_sense: f64) -> Self {
        let num_col = model.num_col;
        let mut domain = Domain::new(&model);
        domain.set_global(true);
        let cutpool = CutPool::new(num_col, options.mip_pool_age_limit, options.mip_pool_soft_limit);
        let conflictpool = ConflictPool::new(5 * options.mip_pool_age_limit, options.mip_pool_soft_limit);
        domain.add_cutpool(&cutpool);
        domain.add_conflict_pool(&conflictpool, num_col);
        let mut postsolve = crate::presolve::mip_postsolve::MipPostsolveStack::default();
        postsolve.initialize_index_maps(model.num_row, num_col);
        let implications = Implications::new(num_col, model.num_nonzero());
        let objective_function = ObjectiveFunction::new(&model);
        Self {
            feastol: options.mip_feasibility_tolerance,
            epsilon: options.small_matrix_value,
            heuristic_effort: options.mip_heuristic_effort,
            upper_limit: options.objective_bound,
            optimality_limit: options.objective_bound,
            options,
            domain,
            cutpool,
            conflictpool,
            pseudocost: Pseudocost::default(),
            cliquetable: CliqueTable::new(num_col),
            implications,
            objective_function,
            symmetries: Symmetries::default(),
            redcostfixing: Default::default(),
            nodequeue: RefCell::new(NodeQueue::default()),
            pruned_treeweight: Cell::new(CDouble::new(0.0)),
            max_abs_row_coef: Vec::new(),
            rowintegral: Vec::new(),
            uplocks: Vec::new(),
            downlocks: Vec::new(),
            integer_cols: Vec::new(),
            implint_cols: Vec::new(),
            integral_cols: Vec::new(),
            continuous_cols: Vec::new(),
            firstlpsol: Vec::new(),
            rootlpsol: Vec::new(),
            firstlpsolobj: -INF,
            firstrootbasis: None,
            firstrootbasis_alien: false,
            rootlpsolobj: -INF,
            num_clique_entries_after_presolve: 0,
            num_clique_entries_after_first_presolve: 0,
            num_nodes: 0,
            num_leaves: 0,
            total_lp_iterations: 0,
            heuristic_lp_iterations: 0,
            sepa_lp_iterations: 0,
            sb_lp_iterations: 0,
            lower_bound: -INF,
            upper_bound: INF,
            incumbent: Vec::new(),
            num_improving_sols: 0,
            submip: false,
            submip_level: 0,
            max_submip_level: 0,
            terminated: false,
            analytic_center: Vec::new(),
            firstrootlpiters: 0,
            avgrootlpiters: 0.0,
            total_repair_lp: 0,
            total_repair_lp_feasible: 0,
            total_repair_lp_iterations: 0,
            model_status: MipStatus::NotSet,
            solution_objective: INF,
            orig_sense,
            global_orbits: None,
            interrupt: None,
            started: web_time::Instant::now(),
            postsolve,
            row_matrix_set: false,
            cliques_extracted: false,
            num_restarts: 0,
            pscostinit: None,
            model,
            orig_model,
            solution: Vec::new(),
            bound_violation: 0.0,
            row_violation: 0.0,
            integrality_violation: 0.0,
            rootbasis: None,
            num_restarts_root: 0,
            numintegercols: 0,
            max_tree_size_log2: 0,
            analytic_center_computed: false,
            analytic_center_status: crate::simplex::ModelStatus::NotSet,
            detect_symmetries: false,
            num_leaves_before_run: 0,
            num_nodes_before_run: 0,
            total_lp_iterations_before_run: 0,
            heuristic_lp_iterations_before_run: 0,
            sepa_lp_iterations_before_run: 0,
            sb_lp_iterations_before_run: 0,
        }
    }

    /// Split into the parts a domain operation borrows.
    pub fn parts(&mut self) -> Parts<'_> {
        Parts {
            cx: MipCtx {
                model: &self.model,
                feastol: self.feastol,
                epsilon: self.epsilon,
                upper_limit: self.upper_limit,
                objective: &self.objective_function,
                cutpool: &mut self.cutpool,
                conflictpool: &mut self.conflictpool,
                nodequeue: Some(&self.nodequeue),
                pruned_treeweight: Some(&self.pruned_treeweight),
                num_integral_cols: self.integral_cols.len(),
            },
            domain: &mut self.domain,
            cliquetable: &mut self.cliquetable,
            implications: &mut self.implications,
            pseudocost: &mut self.pseudocost,
            symmetries: &self.symmetries,
        }
    }

    /// HiGHS's `setupDomainPropagation`: the row-wise copy, fresh
    /// pseudocosts, the rows' largest coefficients and a global domain
    /// with its activities.
    pub fn setup_domain_propagation(&mut self) {
        self.model.build_row_wise();
        let postsolve = &self.postsolve;
        let orig_col_index = |col: usize| postsolve.orig_col_index(col);
        self.pseudocost = Pseudocost::new(
            self.model.num_col,
            self.options.mip_pscost_minreliable,
            self.pscostinit.as_ref().map(|init| (init, &orig_col_index as &dyn Fn(usize) -> usize)),
        );
        self.max_abs_row_coef = (0..self.model.num_row)
            .map(|i| {
                let mut maxabsval = 0.0;
                for &v in &self.model.ar_value[self.model.ar_start[i]..self.model.ar_start[i + 1]] {
                    maxabsval = super::cmax(maxabsval, v.abs());
                }
                maxabsval
            })
            .collect();
        self.domain = Domain::new(&self.model);
        self.domain.set_global(true);
        self.domain.compute_row_activities(&self.model, self.feastol);
    }

    pub fn elapsed(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// HiGHS's `solveSubMip` from the sub-MIP's setup to its solve.
    pub fn run_sub_mip(&mut self, request: super::heuristics::SubMipRequest) -> super::heuristics::SubMipOutcome {
        super::solver::run_sub_mip(self, request)
    }

    /// HiGHS's `updateLowerBound` (without the primal-dual integral, which
    /// only reports).
    pub fn update_lower_bound(&mut self, new_lower_bound: f64) {
        self.lower_bound = new_lower_bound;
    }

    /// HiGHS's `getInfeasibleRows`: `(row, sense, violation)` for each row
    /// `solution` violates.
    pub fn infeasible_rows(&self, solution: &[f64]) -> Vec<(usize, i32, f64)> {
        let mut rows = Vec::new();
        let model = &self.model;
        for i in 0..model.num_row {
            let mut activity = CDouble::new(0.0);
            for j in model.ar_start[i]..model.ar_start[i + 1] {
                activity += CDouble::new(solution[model.ar_index[j]]) * model.ar_value[j];
            }
            let activity = activity.value();
            if activity > model.row_upper[i] + self.feastol {
                rows.push((i, 1, (activity - model.row_upper[i]).abs()));
            }
            if activity < model.row_lower[i] - self.feastol {
                rows.push((i, -1, (model.row_lower[i] - activity).abs()));
            }
        }
        rows
    }

    /// HiGHS's `checkLimits`: whether to stop, setting the model status
    /// on the first limit reached.
    pub fn check_limits(&mut self, node_offset: i64) -> bool {
        let reached = |data: &mut Self, status: MipStatus| {
            if data.model_status == MipStatus::NotSet {
                data.model_status = status;
            }
            true
        };
        // HiGHS asks the user's callback only outside sub-MIPs; asking in
        // them too only stops sooner.
        if self.interrupt.as_ref().is_some_and(|interrupt| interrupt()) {
            return reached(self, MipStatus::Interrupt);
        }
        let options = &self.options;
        if !self.submip
            && self.solution_objective < INF
            && options.objective_target > -INF
            && self.orig_sense * self.solution_objective < self.orig_sense * options.objective_target
        {
            return reached(self, MipStatus::ObjectiveTarget);
        }
        let iinf = i32::MAX as i64;
        if options.mip_max_nodes != iinf && self.num_nodes + node_offset >= options.mip_max_nodes {
            return reached(self, MipStatus::SolutionLimit);
        }
        if options.mip_max_leaves != iinf && self.num_leaves >= options.mip_max_leaves {
            return reached(self, MipStatus::SolutionLimit);
        }
        if options.mip_max_improving_sols != iinf && self.num_improving_sols >= options.mip_max_improving_sols {
            return reached(self, MipStatus::SolutionLimit);
        }
        if options.time_limit < INF && self.elapsed() >= options.time_limit {
            return reached(self, MipStatus::TimeLimit);
        }
        false
    }

    /// HiGHS's `checkSolution`.
    pub fn check_solution(&self, solution: &[f64]) -> bool {
        let model = &self.model;
        for i in 0..model.num_col {
            if solution[i] < model.col_lower[i] - self.feastol || solution[i] > model.col_upper[i] + self.feastol {
                return false;
            }
            if model.is_col_integer(i) && fractionality(solution[i]) > self.feastol {
                return false;
            }
        }
        for i in 0..model.num_row {
            let mut rowactivity = 0.0;
            for j in model.ar_start[i]..model.ar_start[i + 1] {
                rowactivity += solution[model.ar_index[j]] * model.ar_value[j];
            }
            if rowactivity > model.row_upper[i] + self.feastol || rowactivity < model.row_lower[i] - self.feastol {
                return false;
            }
        }
        true
    }

    /// HiGHS's `trySolution`.
    pub fn try_solution(&mut self, solution: &[f64], source: SolutionSource) -> bool {
        let model = &self.model;
        if solution.len() != model.num_col {
            return false;
        }
        let mut obj = CDouble::new(0.0);
        for i in 0..model.num_col {
            if solution[i] < model.col_lower[i] - self.feastol || solution[i] > model.col_upper[i] + self.feastol {
                return false;
            }
            if model.is_col_integer(i) && fractionality(solution[i]) > self.feastol {
                return false;
            }
            obj += model.col_cost[i] * solution[i];
        }
        for i in 0..model.num_row {
            let mut rowactivity = 0.0;
            for j in model.ar_start[i]..model.ar_start[i + 1] {
                rowactivity += solution[model.ar_index[j]] * model.ar_value[j];
            }
            if rowactivity > model.row_upper[i] + self.feastol || rowactivity < model.row_lower[i] - self.feastol {
                return false;
            }
        }
        self.add_incumbent(solution, obj.value(), source, true, false)
    }

    /// HiGHS's `HighsCutPool::addCut` on the global pool: the cut
    /// `inds . x <= rhs` (sorted by column in place), then the cliques it
    /// implies when `extract_cliques` and it is short. `None` for a
    /// duplicate.
    #[allow(clippy::too_many_arguments)]
    pub fn add_cut(&mut self, inds: &mut [usize], vals: &mut [f64], rhs: f64, integral: bool, propagate: bool, extract_cliques: bool, is_conflict: bool) -> Option<usize> {
        let index = self
            .cutpool
            .add_cut(inds, vals, rhs, integral, propagate, is_conflict, self.model.num_nonzero(), self.model.num_row)?;
        if extract_cliques && inds.len() <= 100 {
            let num_entries_after_presolve = self.num_clique_entries_after_presolve;
            let mut parts = self.parts();
            parts.on_cliques(|cliquetable, domain, cc| cliquetable.extract_cliques_from_cut(cc.cx.model, domain, inds, vals, rhs, num_entries_after_presolve, cc));
        }
        Some(index)
    }

    /// HiGHS's `addIncumbent`: a new incumbent if `sol` (in the presolved
    /// space, objective `solobj`) improves, with the limits and the global
    /// domain updated.
    pub fn add_incumbent(&mut self, sol: &[f64], solobj: f64, source: SolutionSource, print_display_line: bool, is_user_solution: bool) -> bool {
        let _ = (source, print_display_line, is_user_solution);
        let possibly_store_as_new_incumbent = solobj < self.upper_bound;
        if possibly_store_as_new_incumbent {
            let solobj = self.transform_new_integer_feasible_solution(sol, true);
            if solobj >= self.upper_bound {
                return false;
            }
            self.upper_bound = solobj;
            self.incumbent = sol.to_vec();
            let new_upper_limit = self.compute_new_upper_limit(solobj, 0.0, 0.0);
            if new_upper_limit < self.upper_limit {
                self.num_improving_sols += 1;
                self.upper_limit = new_upper_limit;
                self.optimality_limit = self.compute_new_upper_limit(solobj, self.options.mip_abs_gap, self.options.mip_rel_gap);
                self.nodequeue.get_mut().set_optimality_limit(self.optimality_limit);
                self.parts().on_domain(|dom, env| dom.propagate(env));
                if !self.domain.infeasible() {
                    self.propagate_root_redcost();
                }
                if self.domain.infeasible() {
                    self.pruned_treeweight.set(CDouble::new(1.0));
                    self.nodequeue.get_mut().clear();
                    return true;
                }
                self.parts().on_cliques(|ct, dom, cc| ct.extract_obj_cliques(cc.cx.model, dom, cc));
                if self.domain.infeasible() {
                    self.pruned_treeweight.set(CDouble::new(1.0));
                    self.nodequeue.get_mut().clear();
                    return true;
                }
                let upper_limit = self.upper_limit;
                let pruned = self.nodequeue.get_mut().perform_bounding(upper_limit);
                self.pruned_treeweight.set(self.pruned_treeweight.get() + pruned);
            }
        } else if self.incumbent.is_empty() {
            self.incumbent = sol.to_vec();
        }
        true
    }

    /// HiGHS's `solutionFeasible` on the original model: the solution's
    /// violations and objective.
    fn solution_feasible(&self, col_value: &[f64], row_value: &[f64]) -> (bool, f64, f64, f64, CDouble) {
        let lp = &self.orig_model;
        let tol = self.options.mip_feasibility_tolerance;
        let mut bound_violation: f64 = 0.0;
        let mut row_violation: f64 = 0.0;
        let mut integrality_violation: f64 = 0.0;
        let mut obj = CDouble::new(lp.offset);
        for i in 0..lp.num_col {
            let value = col_value[i];
            obj += lp.col_cost[i] * value;
            if lp.integrality[i] == VarType::Integer {
                integrality_violation = super::cmax(fractionality(value), integrality_violation);
            }
            let (lower, upper) = (lp.col_lower[i], lp.col_upper[i]);
            let primal_infeasibility = if value < lower - tol {
                lower - value
            } else if value > upper + tol {
                value - upper
            } else {
                continue;
            };
            bound_violation = super::cmax(bound_violation, primal_infeasibility);
        }
        for i in 0..lp.num_row {
            let value = row_value[i];
            let (lower, upper) = (lp.row_lower[i], lp.row_upper[i]);
            let primal_infeasibility = if value < lower - tol {
                lower - value
            } else if value > upper + tol {
                value - upper
            } else {
                continue;
            };
            row_violation = super::cmax(row_violation, primal_infeasibility);
        }
        let feasible = bound_violation <= tol && integrality_violation <= tol && row_violation <= tol;
        (feasible, bound_violation, row_violation, integrality_violation, obj)
    }

    /// The user's solution, as `HighsMipSolver`'s constructor takes it:
    /// its violations and objective on the original model, feasible or
    /// not.
    pub fn set_start_solution(&mut self, col_value: Vec<f64>, row_value: &[f64]) {
        let (_, bound_violation, row_violation, integrality_violation, obj) = self.solution_feasible(&col_value, row_value);
        self.bound_violation = bound_violation;
        self.row_violation = row_violation;
        self.integrality_violation = integrality_violation;
        self.solution_objective = obj.value();
        self.solution = col_value;
    }

    /// HiGHS's `calculateRowValuesQuad` on the original model.
    pub(crate) fn orig_row_values(&self, col_value: &[f64]) -> Vec<f64> {
        let lp = &self.orig_model;
        let mut quad = vec![CDouble::new(0.0); lp.num_row];
        for col in 0..lp.num_col {
            for k in lp.a_start[col]..lp.a_start[col + 1] {
                quad[lp.a_index[k]] += CDouble::new(col_value[col]) * lp.a_value[k];
            }
        }
        quad.into_iter().map(CDouble::value).collect()
    }

    /// HiGHS's `transformNewIntegerFeasibleSolution`: the solution in the
    /// original space (repaired by an LP over the continuous columns if
    /// postsolve left it infeasible), stored as the incumbent there when
    /// asked, and its objective in the presolved space (infinite when it
    /// stays infeasible).
    pub fn transform_new_integer_feasible_solution(&mut self, sol: &[f64], possibly_store_as_new_incumbent: bool) -> f64 {
        let mut col_value = sol.to_vec();
        let mut row_value = Vec::new();
        let tol = crate::presolve::mip_postsolve::Tolerances {
            primal_feasibility_tolerance: 1e-7,
            mip_feasibility_tolerance: self.options.mip_feasibility_tolerance,
        };
        self.postsolve.undo_primal(tol, &mut col_value, &mut row_value);
        row_value = self.orig_row_values(&col_value);
        let mut allow_try_again = true;
        let (feasible, bound_violation, row_violation, integrality_violation, obj) = loop {
            let checked = self.solution_feasible(&col_value, &row_value);
            if !checked.0 && allow_try_again {
                // Fix the integers at their rounded values and solve an LP
                // for the rest.
                let orig = &self.orig_model;
                let mut col_lower = orig.col_lower.clone();
                let mut col_upper = orig.col_upper.clone();
                for i in 0..orig.num_col {
                    if orig.integrality[i] == VarType::Integer {
                        let solval = col_value[i].round();
                        col_lower[i] = super::cmax(col_lower[i], solval);
                        col_upper[i] = super::cmin(col_upper[i], solval);
                    }
                }
                self.total_repair_lp += 1;
                let time_available = super::cmax(self.options.time_limit - self.elapsed(), 0.1);
                let lp_options = crate::Options {
                    time_limit: time_available,
                    primal_feasibility_tolerance: self.options.mip_feasibility_tolerance,
                    presolve: !self.options.mip_root_presolve_only,
                    solver: crate::Solver::Simplex,
                    ..crate::Options::default()
                };
                let mut repair = crate::LpSolver::from_col_wise(
                    self.orig_sense,
                    orig.offset,
                    orig.col_cost.clone(),
                    col_lower,
                    col_upper,
                    orig.row_lower.clone(),
                    orig.row_upper.clone(),
                    orig.a_start.clone(),
                    orig.a_index.clone(),
                    orig.a_value.clone(),
                    lp_options,
                );
                repair.run();
                self.total_repair_lp_iterations += repair.iterations() as i64;
                let primal_feasible = repair.primal_feasible();
                if primal_feasible {
                    self.total_repair_lp_feasible += 1;
                    col_value = repair.col_value().unwrap().to_vec();
                    row_value = repair.row_value().unwrap().to_vec();
                    allow_try_again = false;
                    continue;
                }
            }
            break checked;
        };
        let objective_value = obj.value();
        let transformed_solobj = (self.orig_sense * obj - self.model.offset).value();
        if transformed_solobj >= self.upper_bound && !sol.is_empty() {
            return transformed_solobj;
        }
        if possibly_store_as_new_incumbent {
            if feasible {
                self.row_violation = row_violation;
                self.bound_violation = bound_violation;
                self.integrality_violation = integrality_violation;
                self.solution = col_value;
                self.solution_objective = objective_value;
            } else {
                let tol = self.options.mip_feasibility_tolerance;
                let current_feasible = self.solution_objective != INF && self.bound_violation <= tol && self.integrality_violation <= tol && self.row_violation <= tol;
                if !current_feasible {
                    self.row_violation = row_violation;
                    self.bound_violation = bound_violation;
                    self.integrality_violation = integrality_violation;
                    self.solution = col_value;
                    self.solution_objective = objective_value;
                }
                return INF;
            }
        }
        transformed_solobj
    }

    /// HiGHS's `computeNewUpperLimit`: the objective limit a solution of
    /// value `ub` leaves, within the gaps.
    pub fn compute_new_upper_limit(&self, ub: f64, mip_abs_gap: f64, mip_rel_gap: f64) -> f64 {
        let objective = &self.objective_function;
        let mut new_upper_limit;
        if objective.is_integral() {
            let scale = objective.integral_scale();
            new_upper_limit = (scale * ub - 0.5).floor() / scale;
            if mip_rel_gap != 0.0 {
                new_upper_limit = super::cmin(new_upper_limit, ub - (mip_rel_gap * (ub + self.model.offset).abs() * scale - self.epsilon).ceil() / scale);
            }
            if mip_abs_gap != 0.0 {
                new_upper_limit = super::cmin(new_upper_limit, ub - (mip_abs_gap * scale - self.epsilon).ceil() / scale);
            }
            // So the next better integer objective is surely searched.
            new_upper_limit += self.feastol;
        } else {
            new_upper_limit = super::cmin(ub - self.feastol, ub.next_down());
            if mip_rel_gap != 0.0 {
                new_upper_limit = super::cmin(new_upper_limit, ub - mip_rel_gap * (ub + self.model.offset).abs());
            }
            if mip_abs_gap != 0.0 {
                new_upper_limit = super::cmin(new_upper_limit, ub - mip_abs_gap);
            }
        }
        new_upper_limit
    }

    /// HiGHS's `moreHeuristicsAllowed`: whether the heuristics' share of
    /// LP iterations leaves room for more.
    pub fn more_heuristics_allowed(&self) -> bool {
        let pruned_treeweight = self.pruned_treeweight.get();
        if self.submip {
            return (self.heuristic_lp_iterations as f64) < self.total_lp_iterations as f64 * self.heuristic_effort;
        } else if pruned_treeweight < 1e-3 && self.num_leaves - self.num_leaves_before_run < 10 && self.num_nodes - self.num_nodes_before_run < 1000 {
            if (self.heuristic_lp_iterations as f64) < self.total_lp_iterations as f64 * self.heuristic_effort + 10000.0 {
                return true;
            }
        } else if self.heuristic_lp_iterations < 100000 + ((self.total_lp_iterations - self.heuristic_lp_iterations - self.sb_lp_iterations) >> 1) {
            let heur_iters_curr_run = self.heuristic_lp_iterations - self.heuristic_lp_iterations_before_run;
            let sb_iters_curr_run = self.sb_lp_iterations - self.sb_lp_iterations_before_run;
            let node_iters_curr_run = self.total_lp_iterations - self.total_lp_iterations_before_run - heur_iters_curr_run - sb_iters_curr_run;
            let total_heuristic_effort_estim = self.heuristic_lp_iterations as f64
                / ((self.total_lp_iterations - node_iters_curr_run) as f64 + node_iters_curr_run as f64 / super::cmax(0.01, pruned_treeweight.value()));
            if total_heuristic_effort_estim < super::cmax(0.3 / 0.8, super::cmin(pruned_treeweight.value(), 0.8) / 0.8) * self.heuristic_effort {
                return true;
            }
        }
        false
    }

    /// HiGHS's `removeFixedIndices`.
    pub fn remove_fixed_indices(&mut self) {
        let domain = &self.domain;
        self.integral_cols.retain(|&col| !domain.is_fixed(col));
        self.integer_cols.retain(|&col| !domain.is_fixed(col));
        self.implint_cols.retain(|&col| !domain.is_fixed(col));
        self.continuous_cols.retain(|&col| !domain.is_fixed(col));
    }

    /// HiGHS's `percentageInactiveIntegers`.
    pub fn percentage_inactive_integers(&self) -> f64 {
        100.0 * (1.0 - (self.integer_cols.len() as f64 - self.cliquetable.substitutions().len() as f64) / self.numintegercols as f64)
    }

    /// HiGHS's `checkObjIntegrality`.
    pub fn check_obj_integrality(&mut self) {
        self.objective_function.check_integrality(self.epsilon);
    }

    /// HiGHS's `limitsToGap`: the gap between the bounds, in the original
    /// objective's offset.
    pub fn limits_to_gap(&self, use_lower_bound: f64, use_upper_bound: f64) -> (f64, f64, f64) {
        let offset = self.model.offset;
        let mut lb = use_lower_bound + offset;
        if lb.abs() <= self.epsilon {
            lb = 0.0;
        }
        let mut ub = INF;
        let mut gap = INF;
        if use_upper_bound != INF {
            ub = use_upper_bound + offset;
            if ub.abs() <= self.epsilon {
                ub = 0.0;
            }
            lb = super::cmin(ub, lb);
            gap = if ub == 0.0 { if lb == 0.0 { 0.0 } else { INF } } else { (ub - lb) / ub.abs() };
        }
        (gap, lb, ub)
    }

    /// HiGHS's `solutionRowFeasible`.
    fn solution_row_feasible(&self, solution: &[f64]) -> bool {
        let model = &self.model;
        for i in 0..model.num_row {
            let mut activity = CDouble::new(0.0);
            for j in model.ar_start[i]..model.ar_start[i + 1] {
                activity += CDouble::new(solution[model.ar_index[j]]) * model.ar_value[j];
            }
            let rowactivity = activity.value();
            if rowactivity > model.row_upper[i] + self.feastol || rowactivity < model.row_lower[i] - self.feastol {
                return false;
            }
        }
        true
    }

    /// HiGHS's `trivialHeuristics`: all integers at zero, at their lower
    /// or upper bounds, or at the bound their locks favour, for a model
    /// without continuous columns. Returns whether the model is infeasible.
    pub fn trivial_heuristics(&mut self) -> bool {
        if !self.continuous_cols.is_empty() {
            return false;
        }
        let heuristic_source = [SolutionSource::TrivialZ, SolutionSource::TrivialL, SolutionSource::TrivialU, SolutionSource::TrivialP];
        let mut col_lower = self.model.col_lower.clone();
        let mut col_upper = self.model.col_upper.clone();
        let mut all_integer_lower_non_positive = true;
        let mut all_integer_lower_zero = true;
        let mut all_integer_lower_finite = true;
        let mut all_integer_upper_finite = true;
        for k in 0..self.numintegercols {
            let i_col = self.integer_cols[k];
            col_lower[i_col] = col_lower[i_col].ceil();
            col_upper[i_col] = col_upper[i_col].floor();
            let legal_bounds =
                col_lower[i_col] <= col_upper[i_col] && col_lower[i_col] < INF && col_upper[i_col] > -INF && !col_lower[i_col].is_nan() && !col_upper[i_col].is_nan();
            if !legal_bounds || col_lower[i_col] > col_upper[i_col] {
                return true;
            }
            if col_lower[i_col] > 0.0 {
                all_integer_lower_non_positive = false;
            }
            if col_lower[i_col] != 0.0 {
                all_integer_lower_zero = false;
            }
            if col_lower[i_col] <= -INF {
                all_integer_lower_finite = false;
            }
            if col_upper[i_col] >= INF {
                all_integer_upper_finite = false;
            }
            if !(all_integer_lower_non_positive || all_integer_lower_zero || all_integer_upper_finite) {
                break;
            }
        }
        let all_integer_boxed = all_integer_lower_finite && all_integer_upper_finite;
        let feasibility_tolerance = self.options.mip_feasibility_tolerance;
        let num_col = self.model.num_col;
        let mut solution = vec![0.0; num_col];
        for try_heuristic in 0..4 {
            match try_heuristic {
                0 => {
                    if !all_integer_lower_non_positive {
                        continue;
                    }
                    let model = &self.model;
                    if (0..model.num_row).any(|i| model.row_lower[i] > feasibility_tolerance || model.row_upper[i] < -feasibility_tolerance) {
                        continue;
                    }
                    solution = vec![0.0; num_col];
                }
                1 => {
                    if all_integer_lower_zero || !self.solution_row_feasible(&col_lower) {
                        continue;
                    }
                    solution = col_lower.clone();
                }
                2 => {
                    if !all_integer_upper_finite || !self.solution_row_feasible(&col_upper) {
                        continue;
                    }
                    solution = col_upper.clone();
                }
                _ => {
                    if !all_integer_boxed {
                        continue;
                    }
                    for k in 0..self.numintegercols {
                        let i_col = self.integer_cols[k];
                        let model = &self.model;
                        let mut num_positive_values = 0;
                        let mut num_negative_values = 0;
                        for el in model.a_start[i_col]..model.a_start[i_col + 1] {
                            if model.a_value[el] > 0.0 {
                                num_positive_values += 1;
                            } else {
                                num_negative_values += 1;
                            }
                        }
                        solution[i_col] = if num_positive_values > num_negative_values {
                            col_lower[i_col]
                        } else {
                            col_upper[i_col]
                        };
                    }
                    if !self.solution_row_feasible(&solution) {
                        continue;
                    }
                }
            }
            let mut cdouble_obj = CDouble::new(0.0);
            for i_col in 0..num_col {
                cdouble_obj += self.model.col_cost[i_col] * solution[i_col];
            }
            let sol = solution.clone();
            self.add_incumbent(&sol, cdouble_obj.value(), heuristic_source[try_heuristic], true, false);
        }
        false
    }
}
