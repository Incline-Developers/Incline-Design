//! HiGHS's `HighsMipSolver` (`mip/HighsMipSolver.cpp`) with one worker,
//! and the parts of `HighsMipSolverData` that drive it: presolve and the
//! setup after it, the root node with its separation rounds and
//! heuristics, restarts, and branch and bound over the node queue.
//!
//! HiGHS computes the analytic centre in a task it spawns at the root and
//! syncs later; with one thread a spawned task runs when it is synced and
//! never when it is cancelled first, which is what happens here.

use std::rc::Rc;

use super::{
    MipModel, VarType,
    cut_pool::CutSet,
    data::{MipData, MipOptions, MipStatus, SolutionSource},
    domain::{Domain, Reason},
    heuristics::PrimalHeuristics,
    lp_relaxation::{LpDomain, LpRelaxation, LpStatus},
    pseudocost::{Pseudocost, PseudocostInitialization},
    search::{NodeResult, Search},
    separation::Separation,
};
use crate::{
    Basis, BasisStatus,
    integers::{fractionality, nearest_integer},
    presolve::mip_presolve::{MipPresolve, PresolveStatus},
    simplex::ModelStatus,
    util::CDouble,
};

const INF: f64 = f64::INFINITY;
/// HiGHS's `kHighsIInf`.
const IINF: i64 = i32::MAX as i64;

/// HiGHS's `RestartVote`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestartVote {
    NoCheck,
    NoHugeTree,
    HugeTree,
    WouldRestart,
}

/// What a MIP solve leaves, in the original model's terms.
#[derive(Clone, Debug)]
pub(crate) struct MipResult {
    pub status: MipStatus,
    /// The incumbent, if there is one, and its objective.
    pub solution: Vec<f64>,
    pub objective: f64,
    pub dual_bound: f64,
    pub primal_bound: f64,
    pub gap: f64,
    pub nodes: i64,
    pub lp_iterations: i64,
}

pub(crate) struct MipSolver {
    pub data: MipData,
    lp: LpRelaxation,
    heuristics: PrimalHeuristics,
    search: Search,
    /// The analytic centre task was spawned and not yet synced or
    /// cancelled.
    analytic_center_pending: bool,
    /// The symmetry detection task, spawned with its graph set up.
    symmetry_pending: Option<super::symmetry_detection::SymmetryDetection>,
}

impl MipSolver {
    /// HiGHS's `HighsMipSolver` on `model` (minimised; `orig_model` and
    /// `orig_sense` are the model as given), up to `HighsMipSolverData`'s
    /// constructor and `init()`.
    pub fn new(options: MipOptions, model: MipModel, orig_model: MipModel, orig_sense: f64) -> Self {
        let mut data = MipData::new_with_original(options, model, orig_model, orig_sense);
        data.detect_symmetries = data.options.mip_detect_symmetry;
        let lp = LpRelaxation::new(&data);
        let heuristics = PrimalHeuristics::new(&data);
        let search = Search::new(&mut data, false);
        Self {
            data,
            lp,
            heuristics,
            search,
            analytic_center_pending: false,
            symmetry_pending: None,
        }
    }

    /// HiGHS's `runMipPresolve`.
    fn run_mip_presolve(&mut self, presolve_reduction_limit: i64, first_presolve: bool) {
        let orig_num_row = self.data.orig_model.num_row;
        let status = {
            let mut presolve = MipPresolve::new(&mut self.data, orig_num_row, first_presolve, presolve_reduction_limit);
            presolve.run()
        };
        self.data.model_status = match status {
            PresolveStatus::Infeasible => MipStatus::Infeasible,
            PresolveStatus::UnboundedOrInfeasible => MipStatus::UnboundedOrInfeasible,
            PresolveStatus::ReducedToEmpty => MipStatus::Optimal,
            _ => MipStatus::NotSet,
        };
    }

    /// Fresh pseudocosts, from the ones a restart carried over if any
    /// (HiGHS's `HighsPseudocost(mipsolver)`).
    fn fresh_pseudocost(data: &MipData) -> Pseudocost {
        let postsolve = &data.postsolve;
        let orig_col_index = |col: usize| postsolve.orig_col_index(col);
        Pseudocost::new(
            data.model.num_col,
            data.options.mip_pscost_minreliable,
            data.pscostinit.as_ref().map(|init| (init, &orig_col_index as &dyn Fn(usize) -> usize)),
        )
    }

    /// HiGHS's `runSetup`: the presolved model's bounds, limits, incumbent,
    /// row data, column classes and the first propagation.
    fn run_setup(&mut self) {
        let data = &mut self.data;
        self.lp.set_solved_first_lp(false);
        let offset = data.model.offset;
        data.upper_limit -= offset;
        data.optimality_limit -= offset;
        data.lower_bound -= offset;
        data.upper_bound -= offset;

        if data.solution_objective != INF {
            data.incumbent = data.postsolve.reduced_primal_solution(&data.solution);
            let solobj = data.solution_objective * data.orig_sense - data.model.offset;
            let tol = data.options.mip_feasibility_tolerance;
            let feasible = data.bound_violation <= tol && data.integrality_violation <= tol && data.row_violation <= tol;
            if feasible && solobj < data.upper_bound {
                data.upper_bound = solobj;
                let new_upper_limit = data.compute_new_upper_limit(solobj, 0.0, 0.0);
                if new_upper_limit < data.upper_limit {
                    data.upper_limit = new_upper_limit;
                    data.optimality_limit = data.compute_new_upper_limit(solobj, data.options.mip_abs_gap, data.options.mip_rel_gap);
                    data.nodequeue.get_mut().set_optimality_limit(data.optimality_limit);
                }
            }
        }

        if data.model.num_col == 0 {
            data.add_incumbent(&[], 0.0, SolutionSource::EmptyMip, true, false);
        }

        data.redcostfixing = Default::default();
        data.pseudocost = Self::fresh_pseudocost(data);
        let num_col = data.model.num_col;
        data.nodequeue.get_mut().set_num_col(num_col);
        data.nodequeue.get_mut().set_optimality_limit(data.optimality_limit);

        data.continuous_cols.clear();
        data.integer_cols.clear();
        data.implint_cols.clear();
        data.integral_cols.clear();

        data.row_matrix_set = true;
        data.model.build_row_wise();
        let model = &data.model;
        data.uplocks = vec![0; num_col];
        data.downlocks = vec![0; num_col];
        for i in 0..num_col {
            for j in model.a_start[i]..model.a_start[i + 1] {
                let row = model.a_index[j];
                if model.row_lower[row] != -INF {
                    if model.a_value[j] < 0.0 {
                        data.uplocks[i] += 1;
                    } else {
                        data.downlocks[i] += 1;
                    }
                }
                if model.row_upper[row] != INF {
                    if model.a_value[j] < 0.0 {
                        data.downlocks[i] += 1;
                    } else {
                        data.uplocks[i] += 1;
                    }
                }
            }
        }

        let num_row = data.model.num_row;
        data.rowintegral = vec![false; num_row];
        data.max_abs_row_coef = vec![0.0; num_row];
        for i in 0..num_row {
            let model = &data.model;
            let mut maxabsval = 0.0;
            let mut integral = true;
            for j in model.ar_start[i]..model.ar_start[i + 1] {
                integral = integral && model.is_col_integral(model.ar_index[j]) && fractionality(model.ar_value[j]) <= data.epsilon;
                maxabsval = super::cmax(maxabsval, model.ar_value[j].abs());
            }
            if integral {
                let feastol = data.feastol;
                let model = &mut data.model;
                if model.row_lower[i] != -INF {
                    model.row_lower[i] = (model.row_lower[i] - feastol).ceil();
                }
                if model.row_upper[i] != INF {
                    model.row_upper[i] = (model.row_upper[i] + feastol).floor();
                }
            }
            data.rowintegral[i] = integral;
            data.max_abs_row_coef[i] = maxabsval;
        }

        // Row activities, and every row propagated once.
        {
            let (objective, cliquetable, model) = (&mut data.objective_function, &mut data.cliquetable, &data.model);
            objective.setup_clique_partition(model, |cost, clqvars, partition_start| {
                let mut vars: Vec<super::clique_table::CliqueVar> = clqvars.iter().map(|&(col, val)| super::clique_table::CliqueVar::new(col, usize::from(val))).collect();
                cliquetable.clique_partition_objective(cost, &mut vars, partition_start);
                for (pair, var) in clqvars.iter_mut().zip(&vars) {
                    *pair = (var.col(), var.val() != 0);
                }
            });
        }
        data.parts().on_domain(|dom, env| dom.setup_objective_propagation(env));
        let feastol = data.feastol;
        data.domain.compute_row_activities(&data.model, feastol);
        data.parts().on_domain(|dom, env| dom.propagate(env));
        if data.domain.infeasible() {
            data.model_status = MipStatus::Infeasible;
            data.update_lower_bound(INF);
            data.pruned_treeweight.set(CDouble::new(1.0));
            return;
        }
        if data.model.num_col == 0 {
            data.model_status = MipStatus::Optimal;
            return;
        }
        if data.check_limits(0) {
            return;
        }
        let changed: Vec<usize> = data.domain.changed_cols().to_vec();
        for col in changed {
            let mut parts = data.parts();
            parts.implications.cleanup_varbounds(col, parts.domain, parts.cliquetable, &mut parts.cx);
        }
        data.domain.clear_changed_cols();

        self.lp.lp_solver_mut().options.presolve = false;

        data.check_obj_integrality();
        data.rootlpsol.clear();
        data.firstlpsol.clear();
        let mut num_binary = 0;
        data.max_tree_size_log2 = 0;
        for i in 0..num_col {
            let fixed = data.domain.is_fixed(i);
            match data.model.integrality[i] {
                VarType::Continuous => {
                    if fixed {
                        continue;
                    }
                    data.continuous_cols.push(i);
                }
                VarType::ImplicitInteger => {
                    if fixed {
                        continue;
                    }
                    data.implint_cols.push(i);
                    data.integral_cols.push(i);
                }
                VarType::Integer => {
                    if fixed {
                        if fractionality(data.domain.col_lower[i]) > data.feastol {
                            // An integer fixed at a fractional value.
                            data.model_status = MipStatus::Infeasible;
                            data.update_lower_bound(INF);
                            data.pruned_treeweight.set(CDouble::new(1.0));
                            return;
                        }
                        continue;
                    }
                    data.integer_cols.push(i);
                    data.integral_cols.push(i);
                    let range = super::cmin(1024.0, 1.0 + data.model.col_upper[i] - data.model.col_lower[i]);
                    data.max_tree_size_log2 += range.log2().ceil() as i64;
                    num_binary += (data.model.col_lower[i] == 0.0 && data.model.col_upper[i] == 1.0) as usize;
                }
                VarType::SemiContinuous | VarType::SemiInteger => unreachable!("semi-continuous columns are reformulated before the MIP solver"),
            }
        }

        Self::basis_transfer(data);

        data.numintegercols = data.integer_cols.len();
        data.detect_symmetries = data.detect_symmetries && num_binary > 0;
        data.num_clique_entries_after_presolve = data.cliquetable.num_entries();
        if data.num_restarts == 0 {
            data.num_clique_entries_after_first_presolve = data.cliquetable.num_entries();
        }

        self.heuristics.setup_int_cols(data);

        if data.upper_limit == INF {
            data.analytic_center_computed = false;
        }
        data.analytic_center_status = ModelStatus::NotSet;
        data.analytic_center.clear();
        data.symmetries.clear();
    }

    /// HiGHS's `basisTransfer`: the root basis a restart kept, in the
    /// presolved model's terms.
    fn basis_transfer(data: &mut MipData) {
        if let Some(rootbasis) = &data.rootbasis {
            let num_row = data.model.num_row;
            let num_col = data.model.num_col;
            let postsolve = &data.postsolve;
            data.firstrootbasis = Some(Basis {
                col_status: (0..num_col).map(|i| rootbasis.col_status[postsolve.orig_col_index(i)]).collect(),
                row_status: (0..num_row).map(|i| rootbasis.row_status[postsolve.orig_row_index(i)]).collect(),
            });
            data.firstrootbasis_alien = true;
        }
    }

    /// HiGHS's `startAnalyticCenterComputation`: spawn the task.
    fn start_analytic_center_computation(&mut self) {
        self.analytic_center_pending = true;
    }

    /// The analytic centre task: IPX with centring on the model with zero
    /// costs, without presolve.
    fn compute_analytic_center(&mut self) {
        let model = &self.data.model;
        let lp = crate::simplex::ekk::SimplexLp {
            num_col: model.num_col,
            num_row: model.num_row,
            sense: 1.0,
            offset: model.offset,
            col_cost: vec![0.0; model.num_col],
            col_lower: model.col_lower.clone(),
            col_upper: model.col_upper.clone(),
            row_lower: model.row_lower.clone(),
            row_upper: model.row_upper.clone(),
            a: crate::matrix::ColWise {
                num_col: model.num_col,
                num_row: model.num_row,
                start: model.a_start.clone(),
                index: model.a_index.clone(),
                value: model.a_value.clone(),
            },
        };
        let options = crate::Options {
            presolve: false,
            solver: crate::Solver::Ipm,
            ..crate::Options::default()
        };
        let (status, col_value) = if lp.num_row == 0 || lp.a.value.is_empty() {
            let problem = crate::Problem {
                sense: crate::Sense::Minimise,
                offset: lp.offset,
                col_cost: lp.col_cost.clone(),
                col_lower: lp.col_lower.clone(),
                col_upper: lp.col_upper.clone(),
                integrality: Vec::new(),
                row_lower: lp.row_lower.clone(),
                row_upper: lp.row_upper.clone(),
                row_start: vec![0; lp.num_row + 1],
                row_index: Vec::new(),
                row_value: Vec::new(),
            };
            let solution = crate::solve_unconstrained(&problem, &options, 1.0, &lp.col_lower, &lp.col_upper, &lp.row_lower, &lp.row_upper);
            (solution.status, Some(solution.col_value))
        } else {
            crate::ipx::analytic_centre(&lp, &options, self.data.interrupt.clone())
        };
        let Some(col_value) = col_value.filter(|sol| sol.len() == self.data.model.num_col) else {
            return;
        };
        self.data.analytic_center_status = status;
        self.data.analytic_center = col_value;
    }

    /// HiGHS's `finishAnalyticCenterComputation`: sync the task, then fix
    /// the columns the centre puts at a bound.
    fn finish_analytic_center_computation(&mut self) {
        if self.analytic_center_pending {
            self.analytic_center_pending = false;
            self.compute_analytic_center();
        }
        let data = &mut self.data;
        data.analytic_center_computed = true;
        if data.analytic_center_status == ModelStatus::Optimal {
            for i in 0..data.model.num_col {
                let bound_range = data.domain.col_upper[i] - data.domain.col_lower[i];
                if bound_range == 0.0 {
                    continue;
                }
                let tolerance = data.feastol * super::cmin(bound_range, 1.0);
                if data.analytic_center[i] <= data.model.col_lower[i] + tolerance {
                    let bound = data.model.col_lower[i];
                    data.parts()
                        .on_domain(|dom, env| dom.change_bound(super::DomainChange::new(bound, i, super::BoundType::Upper), Reason::unspecified(), env));
                    if data.domain.infeasible() {
                        return;
                    }
                } else if data.analytic_center[i] >= data.model.col_upper[i] - tolerance {
                    let bound = data.model.col_upper[i];
                    data.parts()
                        .on_domain(|dom, env| dom.change_bound(super::DomainChange::new(bound, i, super::BoundType::Lower), Reason::unspecified(), env));
                    if data.domain.infeasible() {
                        return;
                    }
                }
            }
            data.parts().on_domain(|dom, env| dom.propagate(env));
        }
    }

    /// HiGHS's `TaskGroup::cancel` on the root node's tasks.
    fn cancel_tasks(&mut self) {
        self.analytic_center_pending = false;
        self.symmetry_pending = None;
    }

    /// HiGHS's `startSymmetryDetection`: the model's graph and its first
    /// refinement now, the search spawned.
    fn start_symmetry_detection(&mut self) {
        let mut detection = super::symmetry_detection::SymmetryDetection::load_model_as_graph(&self.data.model, self.data.options.small_matrix_value);
        self.data.detect_symmetries = detection.initialize_detection();
        self.symmetry_pending = self.data.detect_symmetries.then_some(detection);
    }

    /// HiGHS's `finishSymmetryDetection`: sync the search, then the
    /// orbitopes' types and the global stabilizer orbits.
    fn finish_symmetry_detection(&mut self) {
        let mut symmetries = super::symmetry::Symmetries::default();
        if let Some(mut detection) = self.symmetry_pending.take() {
            detection.run(&mut symmetries);
        }
        self.data.symmetries = symmetries;
        if crate::util::tracing() {
            eprintln!(
                "SYM gens {} perms {} orbitopes {}",
                self.data.symmetries.num_generators,
                self.data.symmetries.num_perms,
                self.data.symmetries.orbitopes.len()
            );
        }
        if self.data.symmetries.num_generators == 0 {
            self.data.detect_symmetries = false;
        }
        let data = &mut self.data;
        for orbitope in &mut data.symmetries.orbitopes {
            orbitope.determine_orbitope_type(&mut data.cliquetable);
        }
        if data.symmetries.num_perms != 0 {
            let mut workspace = super::symmetry::StabilizerOrbitWorkspace::default();
            data.global_orbits = Some(data.symmetries.compute_stabilizer_orbits(&data.domain, &data.model, &mut workspace));
        }
    }

    /// HiGHS's `evaluateRootLp`: propagate, resolve the LP while bounds
    /// change, and take what it shows (incumbent, bound, reduced cost
    /// fixing).
    fn evaluate_root_lp(&mut self) -> LpStatus {
        loop {
            let data = &mut self.data;
            data.parts().on_domain(|dom, env| dom.propagate(env));
            if let Some(orbits) = data.global_orbits.clone()
                && !data.domain.infeasible()
            {
                let mut parts = data.parts();
                let symmetries = parts.symmetries;
                parts.on_domain(|dom, env| orbits.orbital_fixing(symmetries, dom, env));
            }
            if data.domain.infeasible() {
                data.update_lower_bound(super::cmin(INF, data.upper_bound));
                data.pruned_treeweight.set(CDouble::new(1.0));
                data.num_nodes += 1;
                data.num_leaves += 1;
                return LpStatus::Infeasible;
            }
            let mut lp_bounds_changed = false;
            if !data.domain.changed_cols().is_empty() {
                lp_bounds_changed = true;
                data.remove_fixed_indices();
                self.lp.flush_global_domain(data);
            }
            let mut lp_was_solved = false;
            let status;
            if lp_bounds_changed || self.lp.lp_solver().status() == ModelStatus::NotSet {
                let mut lp_iters = -self.lp.num_lp_iterations();
                status = self.lp.resolve_lp(LpDomain::Global, data);
                lp_iters += self.lp.num_lp_iterations();
                data.total_lp_iterations += lp_iters;
                data.avgrootlpiters = self.lp.avg_solve_iters();
                lp_was_solved = true;
                if status == LpStatus::Unbounded {
                    data.model_status = if data.solution.is_empty() {
                        MipStatus::UnboundedOrInfeasible
                    } else {
                        MipStatus::Unbounded
                    };
                    data.pruned_treeweight.set(CDouble::new(1.0));
                    data.num_nodes += 1;
                    data.num_leaves += 1;
                    return status;
                }
                if status == LpStatus::Optimal && self.lp.fractional_integers().is_empty() {
                    let sol = self.lp.lp_solver().col_value().expect("an LP solution").to_vec();
                    if data.add_incumbent(&sol, self.lp.objective(), SolutionSource::EvaluateNode, true, false) {
                        data.model_status = MipStatus::Optimal;
                        data.update_lower_bound(data.upper_bound);
                        data.pruned_treeweight.set(CDouble::new(1.0));
                        data.num_nodes += 1;
                        data.num_leaves += 1;
                        return LpStatus::Infeasible;
                    }
                }
                if status == LpStatus::Optimal && data.options.mip_heuristic_run_zi_round {
                    let sol = self.lp.lp_solver().col_value().expect("an LP solution").to_vec();
                    self.heuristics.zi_round(data, &sol);
                }
            } else {
                status = self.lp.status();
            }
            if status == LpStatus::Infeasible {
                data.update_lower_bound(super::cmin(INF, data.upper_bound));
                data.pruned_treeweight.set(CDouble::new(1.0));
                data.num_nodes += 1;
                data.num_leaves += 1;
                return status;
            }
            if self.lp.status().unscaled_dual_feasible() {
                data.update_lower_bound(super::cmax(self.lp.objective(), data.lower_bound));
                if lp_was_solved {
                    data.add_root_redcost(&mut self.lp);
                    if data.upper_limit != INF {
                        data.propagate_root_redcost();
                    }
                }
            }
            if data.lower_bound > data.optimality_limit {
                data.pruned_treeweight.set(CDouble::new(1.0));
                data.num_nodes += 1;
                data.num_leaves += 1;
                return LpStatus::Infeasible;
            }
            if data.domain.changed_cols().is_empty() {
                return status;
            }
        }
    }

    /// HiGHS's `rootSeparationRound`: one round of cuts, then the LP again
    /// and the rounding heuristics while there is no incumbent. Returns
    /// whether the root is done (infeasible).
    fn root_separation_round(&mut self, sepa: &mut Separation, ncuts: &mut usize, status: &mut LpStatus) -> bool {
        let mut tmp_lp_iters = -self.lp.num_lp_iterations();
        *ncuts = sepa.separation_round(&mut self.lp, &mut LpDomain::Global, status, &mut self.data);
        tmp_lp_iters += self.lp.num_lp_iterations();
        self.data.avgrootlpiters = self.lp.avg_solve_iters();
        self.data.total_lp_iterations += tmp_lp_iters;
        self.data.sepa_lp_iterations += tmp_lp_iters;

        *status = self.evaluate_root_lp();
        if *status == LpStatus::Infeasible {
            return true;
        }
        if self.data.submip || self.data.incumbent.is_empty() {
            let solvals = self.lp_solution();
            self.heuristics.randomized_rounding(&mut self.data, &solvals);
            if self.data.options.mip_heuristic_run_shifting {
                self.heuristics.shifting(&self.lp, &mut self.data, &solvals);
            }
            self.heuristics.flush_statistics(&mut self.data);
            *status = self.evaluate_root_lp();
            if *status == LpStatus::Infeasible {
                return true;
            }
        }
        false
    }

    /// The LP's column values, empty when it stopped without any (cut off
    /// by the time limit or a stop inside presolve): the heuristics then
    /// pass over it, as HiGHS's do a solution of the wrong size.
    fn lp_solution(&self) -> Vec<f64> {
        self.lp.lp_solver().col_value().map(<[f64]>::to_vec).unwrap_or_default()
    }

    /// HiGHS's `evaluateRootNode`: the root LP, separation rounds while
    /// they help, the root heuristics, and the restart when presolve can
    /// remove enough; the root node goes to the queue.
    fn evaluate_root_node(&mut self) {
        let mut max_sepa_rounds: i64 = if self.data.submip { 5 } else { IINF };
        if self.data.num_restarts == 0 {
            max_sepa_rounds = max_sepa_rounds.min((2.0 * (self.data.max_tree_size_log2 as f64).sqrt()) as i64);
        }
        'restart: loop {
            if self.data.detect_symmetries {
                self.start_symmetry_detection();
            }
            if !self.data.analytic_center_computed {
                self.start_analytic_center_computation();
            }

            self.lp.set_iteration_limit(IINF as usize);
            self.lp.load_model(&self.data);
            self.data.domain.clear_changed_cols();
            self.lp.set_objective_limit(self.data.upper_limit, &self.data);
            let lb = super::cmax(self.data.lower_bound, self.data.domain.objective_lower_bound());
            self.data.update_lower_bound(lb);

            if let Some(basis) = self.data.firstrootbasis.clone() {
                if self.data.firstrootbasis_alien {
                    self.lp.lp_solver_mut().set_alien_basis(&basis);
                } else {
                    self.lp.lp_solver_mut().set_basis(&basis);
                }
            } else {
                self.lp.lp_solver_mut().options.presolve = !self.data.options.mip_root_presolve_only;
            }

            let mut status = self.evaluate_root_lp();
            if self.data.num_restarts == 0 {
                self.data.firstrootlpiters = self.data.total_lp_iterations;
            }
            self.lp.lp_solver_mut().options.presolve = false;
            if status == LpStatus::Infeasible || status == LpStatus::Unbounded {
                return;
            }

            self.data.firstlpsol = self.lp_solution();
            self.data.firstlpsolobj = self.lp.objective();
            self.data.rootlpsolobj = self.data.firstlpsolobj;

            if let Some(basis) = self.lp.lp_solver().basis().cloned()
                && self.lp.num_rows() == self.data.model.num_row
            {
                self.data.firstrootbasis = Some(basis);
            } else {
                // The root basis must fit the model without cuts: the slack
                // basis.
                self.data.firstrootbasis = Some(Basis {
                    col_status: vec![BasisStatus::Nonbasic; self.data.model.num_col],
                    row_status: vec![BasisStatus::Basic; self.data.model.num_row],
                });
            }
            self.data.firstrootbasis_alien = false;

            if self.data.cutpool.num_cuts() != 0 {
                let mut cutset = CutSet::default();
                self.data.cutpool.separate_lp_cuts_after_restart(&mut cutset);
                self.lp.add_cuts(&mut cutset);
                status = self.evaluate_root_lp();
                self.lp.remove_obsolete_rows(true, &mut self.data);
                if status == LpStatus::Infeasible {
                    return;
                }
            }

            self.lp.set_iteration_limit((10000.0f64).max(10.0 * self.data.avgrootlpiters) as usize);

            let firstlpsol = self.data.firstlpsol.clone();
            if self.data.options.mip_heuristic_run_zi_round {
                self.heuristics.zi_round(&mut self.data, &firstlpsol);
            }
            self.heuristics.randomized_rounding(&mut self.data, &firstlpsol);
            if self.data.options.mip_heuristic_run_shifting {
                self.heuristics.shifting(&self.lp, &mut self.data, &firstlpsol);
            }
            self.heuristics.flush_statistics(&mut self.data);

            status = self.evaluate_root_lp();
            if status == LpStatus::Infeasible {
                return;
            }

            self.data.rootlpsolobj = self.data.firstlpsolobj;
            self.data.remove_fixed_indices();
            if self.data.options.mip_allow_restart && self.data.options.presolve {
                let fixing_rate = self.data.percentage_inactive_integers();
                if fixing_rate >= 10.0 {
                    self.cancel_tasks();
                    self.perform_restart();
                    self.data.num_restarts_root += 1;
                    if self.data.model_status == MipStatus::NotSet {
                        continue 'restart;
                    }
                    return;
                }
            }

            // Separation.
            let num_col = self.data.model.num_col;
            let mut avgdirection = vec![0.0; num_col];
            let mut curdirection = vec![0.0; num_col];
            let mut stall: i64 = 0;
            let mut smoothprogress = 0.0;
            let mut nseparounds: i64 = 0;
            let mut sepa = Separation::new(&self.data);

            while status.scaled_optimal() && !self.lp.fractional_integers().is_empty() && stall < 3 {
                if self.data.check_limits(0) {
                    return;
                }
                if nseparounds == max_sepa_rounds {
                    break;
                }
                self.data.remove_fixed_indices();
                if !self.data.submip && self.data.options.presolve {
                    let fixing_rate = self.data.percentage_inactive_integers();
                    if fixing_rate >= 10.0 {
                        stall = -1;
                        break;
                    }
                }
                nseparounds += 1;
                let mut ncuts = 0;
                if self.root_separation_round(&mut sepa, &mut ncuts, &mut status) {
                    return;
                }
                if nseparounds >= 5 && !self.data.submip && !self.data.analytic_center_computed {
                    if self.data.check_limits(0) {
                        return;
                    }
                    self.finish_analytic_center_computation();
                    self.heuristics.central_rounding(&mut self.data);
                    self.heuristics.flush_statistics(&mut self.data);
                    if self.data.check_limits(0) {
                        return;
                    }
                    status = self.evaluate_root_lp();
                    if status == LpStatus::Infeasible {
                        return;
                    }
                }

                let solvals = self.lp_solution();
                // A first root LP stopped without values leaves the
                // direction to measure progress along from this one.
                if self.data.firstlpsol.len() != num_col {
                    self.data.firstlpsol = solvals.clone();
                }
                let firstlpsol = &self.data.firstlpsol;
                let mut sqrnorm = CDouble::new(0.0);
                for i in 0..num_col {
                    curdirection[i] = firstlpsol[i] - solvals[i];
                    sqrnorm += curdirection[i] * curdirection[i];
                }
                let scale = (1.0 / sqrnorm.sqrt()).value();
                sqrnorm = CDouble::new(0.0);
                let mut dotproduct = CDouble::new(0.0);
                for i in 0..num_col {
                    avgdirection[i] = (scale * curdirection[i] - avgdirection[i]) / nseparounds as f64;
                    sqrnorm += avgdirection[i] * avgdirection[i];
                    dotproduct += avgdirection[i] * curdirection[i];
                }
                let progress = (dotproduct / sqrnorm.sqrt()).value();
                if nseparounds == 1 {
                    smoothprogress = progress;
                } else {
                    let alpha = 1.0 / 3.0;
                    let nextprogress = (1.0 - alpha) * smoothprogress + alpha * progress;
                    if nextprogress < smoothprogress * 1.01 && (self.lp.objective() - self.data.firstlpsolobj) <= (self.data.rootlpsolobj - self.data.firstlpsolobj) * 1.001 {
                        stall += 1;
                    } else {
                        stall = 0;
                    }
                    smoothprogress = nextprogress;
                }
                self.data.rootlpsolobj = self.lp.objective();
                self.lp.set_iteration_limit((10000.0f64).max(10.0 * self.data.avgrootlpiters) as usize);
                if ncuts == 0 {
                    break;
                }
            }

            self.lp.set_iteration_limit(IINF as usize);
            status = self.evaluate_root_lp();
            if status == LpStatus::Infeasible {
                return;
            }
            self.data.rootlpsol = self.lp_solution();
            self.data.rootlpsolobj = self.lp.objective();
            self.lp.set_iteration_limit((10000.0f64).max(10.0 * self.data.avgrootlpiters) as usize);

            if self.data.options.mip_heuristic_run_zi_round {
                let firstlpsol = self.data.firstlpsol.clone();
                self.heuristics.zi_round(&mut self.data, &firstlpsol);
                self.heuristics.flush_statistics(&mut self.data);
            }
            if self.data.options.mip_heuristic_run_shifting {
                let rootlpsol = self.data.rootlpsol.clone();
                self.heuristics.shifting(&self.lp, &mut self.data, &rootlpsol);
                self.heuristics.flush_statistics(&mut self.data);
            }

            if !self.data.analytic_center_computed {
                if self.data.check_limits(0) {
                    return;
                }
                self.finish_analytic_center_computation();
                self.heuristics.central_rounding(&mut self.data);
                self.heuristics.flush_statistics(&mut self.data);
                if self.data.check_limits(0) {
                    return;
                }
                let separate = !self.data.domain.changed_cols().is_empty();
                status = self.evaluate_root_lp();
                if status == LpStatus::Infeasible {
                    return;
                }
                if separate && status.scaled_optimal() {
                    let mut ncuts = 0;
                    if self.root_separation_round(&mut sepa, &mut ncuts, &mut status) {
                        return;
                    }
                    nseparounds += 1;
                }
            }

            if self.data.check_limits(0) {
                return;
            }

            'heuristics: {
                if self.data.rootlpsol.is_empty() {
                    break 'heuristics;
                }
                if self.data.upper_limit != INF && !self.data.more_heuristics_allowed() {
                    break 'heuristics;
                }
                if self.data.options.mip_heuristic_run_root_reduced_cost {
                    self.heuristics.root_reduced_cost(&self.lp, &mut self.data);
                    self.heuristics.flush_statistics(&mut self.data);
                }
                if self.data.check_limits(0) {
                    return;
                }
                let separate = !self.data.domain.changed_cols().is_empty();
                status = self.evaluate_root_lp();
                if status == LpStatus::Infeasible {
                    return;
                }
                if separate && status.scaled_optimal() {
                    let mut ncuts = 0;
                    if self.root_separation_round(&mut sepa, &mut ncuts, &mut status) {
                        return;
                    }
                    nseparounds += 1;
                }
                if self.data.upper_limit != INF && !self.data.more_heuristics_allowed() {
                    break 'heuristics;
                }
                if self.data.check_limits(0) {
                    return;
                }
                if self.data.options.mip_heuristic_run_rens {
                    let rootlpsol = self.data.rootlpsol.clone();
                    self.heuristics.rens(&self.lp, &mut self.data, &rootlpsol);
                    self.heuristics.flush_statistics(&mut self.data);
                }
                if self.data.check_limits(0) {
                    return;
                }
                let separate = !self.data.domain.changed_cols().is_empty();
                status = self.evaluate_root_lp();
                if status == LpStatus::Infeasible {
                    return;
                }
                if separate && status.scaled_optimal() {
                    let mut ncuts = 0;
                    if self.root_separation_round(&mut sepa, &mut ncuts, &mut status) {
                        return;
                    }
                    nseparounds += 1;
                }
                if self.data.upper_limit != INF || self.data.submip {
                    break 'heuristics;
                }
                if self.data.check_limits(0) {
                    return;
                }
                self.heuristics.feasibility_pump(&self.lp, &mut self.data);
                self.heuristics.flush_statistics(&mut self.data);
                if self.data.check_limits(0) {
                    return;
                }
                status = self.evaluate_root_lp();
                if status == LpStatus::Infeasible {
                    return;
                }
            }

            if self.data.lower_bound > self.data.upper_limit {
                self.data.model_status = MipStatus::Optimal;
                self.data.pruned_treeweight.set(CDouble::new(1.0));
                self.data.num_nodes += 1;
                self.data.num_leaves += 1;
                return;
            }

            let separate = !self.data.domain.changed_cols().is_empty();
            status = self.evaluate_root_lp();
            if status == LpStatus::Infeasible {
                return;
            }
            if separate && status.scaled_optimal() {
                let mut ncuts = 0;
                if self.root_separation_round(&mut sepa, &mut ncuts, &mut status) {
                    return;
                }
                nseparounds += 1;
            }

            self.data.remove_fixed_indices();
            if self.lp.lp_solver().basis().is_some() {
                self.lp.remove_obsolete_rows(true, &mut self.data);
            }
            self.data.rootlpsolobj = self.lp.objective();

            if self.data.lower_bound <= self.data.upper_limit {
                if !self.data.submip && self.data.options.mip_allow_restart && self.data.options.presolve {
                    if !self.data.analytic_center_computed {
                        self.finish_analytic_center_computation();
                    }
                    let fixing_rate = self.data.percentage_inactive_integers();
                    if fixing_rate >= 2.5 + 7.5 * (self.data.submip as i32 as f64) || (!self.data.submip && fixing_rate > 0.0 && self.data.num_restarts == 0) {
                        self.cancel_tasks();
                        if stall != -1 {
                            max_sepa_rounds = max_sepa_rounds.min(nseparounds);
                        }
                        self.perform_restart();
                        if self.data.terminated {
                            return;
                        }
                        self.data.num_restarts_root += 1;
                        if self.data.model_status == MipStatus::NotSet {
                            continue 'restart;
                        }
                        return;
                    }
                }
                if self.data.detect_symmetries {
                    self.finish_symmetry_detection();
                    status = self.evaluate_root_lp();
                    if status == LpStatus::Infeasible {
                        return;
                    }
                }
                // The root node starts the search.
                let estimate = self.lp.compute_best_estimate(&self.data.pseudocost, &self.data);
                let lower_bound = self.data.lower_bound;
                self.data.nodequeue.get_mut().emplace_node(Vec::new(), Vec::new(), lower_bound, estimate, 1);
            }
            return;
        }
    }

    /// HiGHS's `performRestart`: presolve again from the LP with its cuts
    /// and the global domain, carrying pseudocosts and the root basis over.
    fn perform_restart(&mut self) {
        let data = &mut self.data;
        let orig_num_col = data.postsolve.orig_num_col();
        let pscostinit = {
            let postsolve = &data.postsolve;
            PseudocostInitialization::new_original(&data.pseudocost, data.options.mip_pscost_minreliable, orig_num_col, |col| postsolve.orig_col_index(col))
        };
        data.pscostinit = Some(pscostinit);
        data.num_restarts += 1;
        data.num_leaves_before_run = data.num_leaves;
        data.num_nodes_before_run = data.num_nodes;
        data.total_lp_iterations_before_run = data.total_lp_iterations;
        data.heuristic_lp_iterations_before_run = data.heuristic_lp_iterations;
        data.sepa_lp_iterations_before_run = data.sepa_lp_iterations;
        data.sb_lp_iterations_before_run = data.sb_lp_iterations;
        let num_lp_rows = self.lp.num_rows();
        let num_model_rows = data.model.num_row;
        let num_cuts = num_lp_rows - num_model_rows;
        if num_cuts > 0 {
            data.postsolve.append_cuts_to_model(num_cuts);
        }
        // The presolved model becomes the LP, cuts included.
        {
            let solver = self.lp.lp_solver();
            let (start, index, value) = solver.matrix();
            let model = &mut data.model;
            model.num_col = solver.num_cols();
            model.num_row = solver.num_rows();
            model.col_cost = solver.col_cost().to_vec();
            model.col_lower = solver.col_lower().to_vec();
            model.col_upper = solver.col_upper().to_vec();
            model.row_lower = solver.row_lower().to_vec();
            model.row_upper = solver.row_upper().to_vec();
            model.a_start = start.to_vec();
            model.a_index = index.to_vec();
            model.a_value = value.to_vec();
        }

        let mut root_basis_set = false;
        if let Some(basis) = &data.firstrootbasis {
            // The root basis in the original space, for a starting basis
            // after presolve.
            let postsolve = &data.postsolve;
            let mut root_basis = Basis {
                col_status: vec![BasisStatus::Lower; postsolve.orig_num_col()],
                row_status: vec![BasisStatus::Basic; postsolve.orig_num_row()],
            };
            for i in 0..data.model.num_col {
                root_basis.col_status[postsolve.orig_col_index(i)] = basis.col_status[i];
            }
            for (i, &status) in basis.row_status.iter().enumerate() {
                root_basis.row_status[postsolve.orig_row_index(i)] = status;
            }
            data.rootbasis = Some(root_basis);
            root_basis_set = true;
        }

        // Limits and bounds into the original space, as presolve changes
        // the offset.
        let offset = data.model.offset;
        data.upper_limit += offset;
        data.optimality_limit += offset;
        data.upper_bound += offset;
        data.lower_bound += offset;

        data.incumbent.clear();
        data.pruned_treeweight.set(CDouble::new(0.0));
        data.nodequeue.get_mut().clear();
        data.global_orbits = None;

        let num_reductions = data.postsolve.num_reductions() as i64;
        let restart_presolve_reduction_limit = data.options.restart_presolve_reduction_limit;
        let further_presolve_reduction_limit = if restart_presolve_reduction_limit >= 0 {
            num_reductions + restart_presolve_reduction_limit
        } else {
            -1
        };
        self.run_mip_presolve(further_presolve_reduction_limit, false);

        let data = &mut self.data;
        if data.model_status != MipStatus::NotSet {
            let offset = data.model.offset;
            data.upper_limit -= offset;
            data.optimality_limit -= offset;
            if data.model_status == MipStatus::Optimal {
                data.upper_bound = 0.0;
                data.transform_new_integer_feasible_solution(&[], true);
            } else {
                data.upper_bound -= offset;
            }
            data.lower_bound -= offset;
            let upper_bound = data.upper_bound;
            data.update_lower_bound(upper_bound);
            if data.solution_objective != INF && data.model_status == MipStatus::Infeasible {
                data.model_status = MipStatus::Optimal;
            }
            return;
        }
        self.run_setup();
        if self.data.terminated {
            return;
        }
        self.data.postsolve.remove_cuts_from_model(num_cuts);
        if root_basis_set {
            self.data.rootbasis = None;
        }
        self.data.pscostinit = None;
    }

    /// HiGHS's `checkRestart` for the worker: whether the tree looks huge
    /// enough to restart.
    #[allow(clippy::too_many_arguments)]
    fn check_restart(
        &self,
        mut num_worker_votes: i64,
        next_check: i64,
        num_nodes_last_check: i64,
        treeweight_last_check: f64,
        upper_lim_last_check: f64,
        lower_bound_last_check: f64,
        num_huge_tree_estim: i64,
    ) -> RestartVote {
        let data = &self.data;
        let n_nodes = self.search.nnodes + data.num_nodes;
        if !data.submip && n_nodes >= next_check && data.options.mip_allow_restart {
            let n_tree_restarts = data.num_restarts - data.num_restarts_root;
            let tree_weight = self.search.treeweight + data.pruned_treeweight.get();
            let curr_node_estim = (num_nodes_last_check - data.num_nodes_before_run) as f64
                + (n_nodes - num_nodes_last_check) as f64 * (1.0 - tree_weight).value() / super::cmax((tree_weight - treeweight_last_check).value(), data.epsilon);
            let mut active_integer_ratio = 1.0 - data.percentage_inactive_integers() / 100.0;
            active_integer_ratio *= active_integer_ratio;
            let mut huge_tree = false;
            let mut gap_reduction = 1.0;
            if data.upper_limit != INF {
                let old_gap = upper_lim_last_check - lower_bound_last_check;
                let new_gap = data.upper_limit - data.lower_bound;
                gap_reduction = old_gap / new_gap;
            }
            if gap_reduction < 1.0 + (0.05 / active_integer_ratio) && curr_node_estim >= active_integer_ratio * 20.0 * (n_nodes - data.num_nodes_before_run) as f64 {
                huge_tree = true;
            } else {
                num_worker_votes = -IINF;
            }
            let min_huge_tree_offset = (data.num_leaves + self.search.nleaves - data.num_leaves_before_run) / 1000;
            let min_huge_tree_estim = nearest_integer(active_integer_ratio * (10 + min_huge_tree_offset) as f64 * 1.5f64.powi(n_tree_restarts));
            let do_restart = num_huge_tree_estim + num_worker_votes >= min_huge_tree_estim;
            if do_restart {
                return RestartVote::WouldRestart;
            }
            if huge_tree {
                return RestartVote::HugeTree;
            }
            return RestartVote::NoHugeTree;
        }
        RestartVote::NoCheck
    }

    /// HiGHS's `isBasisConsistent` for the LP relaxation.
    fn basis_consistent(&self, basis: &Basis) -> bool {
        let solver = self.lp.lp_solver();
        basis.col_status.len() == solver.num_cols()
            && basis.row_status.len() == solver.num_rows()
            && basis.col_status.iter().chain(&basis.row_status).filter(|&&status| status == BasisStatus::Basic).count() == solver.num_rows()
    }

    /// HiGHS's `HighsMipSolver::run`.
    pub fn run(&mut self) -> MipResult {
        self.data.model_status = MipStatus::NotSet;
        self.data.max_submip_level = self.data.max_submip_level.max(self.data.submip_level);
        let limit = self.data.options.presolve_reduction_limit;
        self.run_mip_presolve(limit, true);
        if self.data.model_status == MipStatus::NotSet && self.data.elapsed() >= self.data.options.time_limit {
            self.data.model_status = MipStatus::TimeLimit;
        }
        if self.data.model_status != MipStatus::NotSet {
            if self.data.model_status == MipStatus::Optimal {
                self.data.lower_bound = 0.0;
                self.data.upper_bound = 0.0;
                self.data.transform_new_integer_feasible_solution(&[], true);
            }
            return self.cleanup_solve();
        }
        self.run_setup();
        if self.data.domain.infeasible() {
            return self.cleanup_solve();
        }

        'restart: loop {
            if self.data.model_status == MipStatus::NotSet {
                if self.data.check_limits(0) {
                    return self.cleanup_solve();
                }
                if self.data.trivial_heuristics() && self.data.model_status == MipStatus::NotSet {
                    self.data.model_status = MipStatus::Infeasible;
                    return self.cleanup_solve();
                }
                if self.data.options.mip_heuristic_run_feasibility_jump && self.feasibility_jump() && self.data.model_status == MipStatus::NotSet {
                    self.data.model_status = MipStatus::Infeasible;
                    return self.cleanup_solve();
                }
                self.evaluate_root_node();
                // The root's task group goes, cancelling what it did not
                // sync.
                self.cancel_tasks();
                if self.data.terminated {
                    return self.cleanup_solve();
                }
                // Age five times, to drop cuts the root stored and never
                // needed.
                for _ in 0..5 {
                    self.data.cutpool.perform_aging();
                }
            }
            if self.data.nodequeue.get_mut().is_empty() || self.data.check_limits(0) {
                return self.cleanup_solve();
            }
            let best = self.data.nodequeue.get_mut().best_lower_bound();
            self.data.update_lower_bound(best);

            let submip = self.data.submip;
            self.search = Search::new(&mut self.data, submip);
            let mut sepa = Separation::new(&self.data);

            let mut num_stall_nodes: i64 = 0;
            let mut last_lb_leave: i64 = 0;
            let mut num_queue_leaves: i64 = 0;
            let mut num_huge_tree_estim: i64 = 0;
            let mut num_nodes_last_check = self.data.num_nodes;
            let mut next_check = self.data.num_nodes;
            let mut treeweight_last_check = 0.0;
            let mut upper_lim_last_check = self.data.upper_limit;
            let mut lower_bound_last_check = self.data.lower_bound;

            let mut root_node = true;
            while !self.data.nodequeue.get_mut().is_empty() {
                let mut vote = RestartVote::NoCheck;
                let assigned = self.data.nodequeue.get_mut().num_active_nodes().min(1) == 1;
                if assigned {
                    // HiGHS's `installNodes` for the one worker.
                    if root_node {
                        let node = self.data.nodequeue.get_mut().pop_best_bound_node();
                        self.search.install_node(node, &mut self.data);
                    } else {
                        if num_queue_leaves - last_lb_leave >= 10 {
                            let node = self.data.nodequeue.get_mut().pop_best_bound_node();
                            self.search.install_node(node, &mut self.data);
                            last_lb_leave = num_queue_leaves;
                        } else {
                            let queue = self.data.nodequeue.get_mut();
                            let best_bound_node_stack_size = queue.best_bound_domchg_stack_size();
                            let best_bound_node_lb = queue.best_lower_bound();
                            let next_node = queue.pop_best_node();
                            if next_node.lower_bound == best_bound_node_lb && next_node.domchgstack.len() == best_bound_node_stack_size {
                                last_lb_leave = num_queue_leaves;
                            }
                            self.search.install_node(next_node, &mut self.data);
                        }
                        num_queue_leaves += 1;
                        if self.search.current_estimate() >= self.data.upper_limit {
                            num_stall_nodes += 1;
                            if self.data.options.mip_max_stall_nodes != IINF && num_stall_nodes >= self.data.options.mip_max_stall_nodes {
                                self.data.model_status = MipStatus::SolutionLimit;
                                break;
                            }
                        } else {
                            num_stall_nodes = 0;
                        }
                    }
                    if root_node {
                        num_queue_leaves += 0;
                    }
                    let avgiter = self.lp.avg_solve_iters();
                    self.process_node(&mut sepa, root_node, avgiter);
                    vote = self.check_restart(
                        1,
                        next_check,
                        num_nodes_last_check,
                        treeweight_last_check,
                        upper_lim_last_check,
                        lower_bound_last_check,
                        num_huge_tree_estim,
                    );
                }
                root_node = false;

                let mut infeasible = false;
                if assigned {
                    if self.data.domain.infeasible() {
                        infeasible = true;
                    }
                    self.search.open_nodes_to_queue(&mut self.lp, &mut self.data);
                    self.search.flush_statistics(&mut self.data);
                    self.heuristics.flush_statistics(&mut self.data);
                }
                if infeasible {
                    self.data.nodequeue.get_mut().clear();
                    self.data.pruned_treeweight.set(CDouble::new(1.0));
                    let upper_bound = self.data.upper_bound;
                    self.data.update_lower_bound(super::cmin(INF, upper_bound));
                    break;
                }
                let bound = super::cmin(self.data.upper_bound, self.data.nodequeue.get_mut().best_lower_bound());
                self.data.update_lower_bound(bound);
                if self.data.check_limits(0) {
                    break;
                }

                self.data.parts().on_domain(|dom, env| dom.propagate(env));
                {
                    let feastol = self.data.feastol;
                    let mut parts = self.data.parts();
                    let mut queue = parts.cx.nodequeue.expect("the node queue").borrow_mut();
                    let pruned = parts.on_domain(|dom, env| queue.prune_infeasible_nodes(dom, feastol, env));
                    drop(queue);
                    let treeweight = self.data.pruned_treeweight.get() + pruned;
                    self.data.pruned_treeweight.set(treeweight);
                }
                if self.data.domain.infeasible() {
                    self.data.nodequeue.get_mut().clear();
                    self.data.pruned_treeweight.set(CDouble::new(1.0));
                    let upper_bound = self.data.upper_bound;
                    self.data.update_lower_bound(super::cmin(INF, upper_bound));
                    break;
                }
                let bound = super::cmin(self.data.upper_bound, self.data.nodequeue.get_mut().best_lower_bound());
                self.data.update_lower_bound(bound);
                if self.data.nodequeue.get_mut().is_empty() {
                    break;
                }

                self.reset_global_domain();

                // HiGHS's `checkWorkerRestartVotes` with one voter.
                let (num_restart_votes, num_huge_tree_votes) = match vote {
                    RestartVote::WouldRestart => (1, 0),
                    RestartVote::HugeTree => (0, 1),
                    _ => (0, 0),
                };
                let joint = self.check_restart(
                    (num_restart_votes + num_huge_tree_votes + 2) / 2,
                    next_check,
                    num_nodes_last_check,
                    treeweight_last_check,
                    upper_lim_last_check,
                    lower_bound_last_check,
                    num_huge_tree_estim,
                );
                match joint {
                    RestartVote::WouldRestart => {
                        self.perform_restart();
                        continue 'restart;
                    }
                    RestartVote::HugeTree => {
                        next_check = self.data.num_nodes + 100;
                        num_huge_tree_estim += (num_restart_votes + num_huge_tree_votes + 2) / 2;
                    }
                    RestartVote::NoHugeTree => {
                        num_huge_tree_estim = 0;
                        treeweight_last_check = self.data.pruned_treeweight.get().value();
                        num_nodes_last_check = self.data.num_nodes;
                        upper_lim_last_check = self.data.upper_limit;
                        lower_bound_last_check = self.data.lower_bound;
                    }
                    RestartVote::NoCheck => {}
                }
            }
            return self.cleanup_solve();
        }
    }

    /// The main loop's `resetGlobalDomain` with one worker: the global
    /// domain's new bounds into the clique table and variable bounds, and
    /// a fresh local domain.
    fn reset_global_domain(&mut self) {
        if self.data.domain.changed_cols().is_empty() {
            return;
        }
        self.data.parts().on_cliques(|ct, dom, cc| ct.cleanup_fixed(dom, cc));
        let changed: Vec<usize> = self.data.domain.changed_cols().to_vec();
        for col in changed {
            let mut parts = self.data.parts();
            parts.implications.cleanup_varbounds(col, parts.domain, parts.cliquetable, &mut parts.cx);
        }
        self.data.parts().on_domain(|dom, env| dom.set_domain_change_stack(&[], env));
        self.search.reset_local_domain(&mut self.lp, &self.data);
        self.data.domain.clear_changed_cols();
        self.data.remove_fixed_indices();
    }

    /// The main loop's `processNode` for the worker: evaluate and separate
    /// the node (except at the root, done already), then heuristics, a
    /// dive, and plunging while it stays cheap.
    fn process_node(&mut self, sepa: &mut Separation, skip_separation: bool, avgiter: f64) {
        let mut nodes_explored: i64 = 0;
        if !skip_separation {
            // `evaluateNode`.
            if self.search.evaluate_node(&mut self.lp, &mut self.data) == NodeResult::SubOptimal {
                self.search.current_node_to_queue(&mut self.data);
            }
            // `pruneNode`.
            let mut pruned = false;
            if self.search.current_node_pruned() {
                self.search.backtrack(&mut self.lp, &mut self.data, true);
                self.data.parts().on_domain(|dom, env| dom.propagate(env));
                pruned = true;
                self.search.nnodes += 1;
                self.search.nleaves += 1;
            }
            if self.data.domain.infeasible() || pruned {
                return;
            }
            if self.data.check_limits(0) {
                return;
            }
            // `separateAndStoreBasis`.
            if self.data.options.mip_allow_cut_separation_at_nodes {
                let mut localdom = std::mem::replace(&mut self.search.localdom, Domain::new(&self.data.model));
                sepa.separate(&mut self.lp, &mut LpDomain::Local(&mut localdom), &mut self.data);
                self.search.localdom = localdom;
            } else {
                self.data.cutpool.perform_aging();
            }
            if self.data.domain.infeasible() {
                self.search.cutoff_node();
                self.search.open_nodes_to_queue(&mut self.lp, &mut self.data);
                return;
            }
            if self.lp.status() != LpStatus::Error && self.lp.status() != LpStatus::NotSet {
                self.lp.store_basis();
            }
            let consistent = self.lp.stored_basis().is_some_and(|basis| self.basis_consistent(&basis));
            if !consistent {
                let mut basis = self.data.firstrootbasis.clone().unwrap_or(Basis {
                    col_status: Vec::new(),
                    row_status: Vec::new(),
                });
                basis.row_status.resize(self.lp.num_rows(), BasisStatus::Basic);
                self.lp.set_stored_basis(Some(Rc::new(basis)));
            }
        }
        self.data.conflictpool.perform_aging();
        let mut iterlimit = (10.0 * super::cmax(avgiter, self.data.avgrootlpiters)) as i64;
        iterlimit = iterlimit.max(10000).max((3 * self.data.firstrootlpiters) / 2);
        self.lp.set_iteration_limit(iterlimit as usize);
        let mut consider_heuristics = true;
        loop {
            if consider_heuristics && self.data.more_heuristics_allowed() && self.run_heuristics() {
                break;
            }
            consider_heuristics = false;
            if self.data.domain.infeasible() {
                break;
            }
            // `dive` without a node limit.
            if !self.search.current_node_pruned() {
                if self.search.dive(&mut self.lp, &mut self.data, i64::MAX) == NodeResult::SubOptimal {
                    break;
                }
                self.search.nleaves += 1;
            }
            if self.data.check_limits(self.search.nnodes) {
                break;
            }
            if self.search.nnodes + nodes_explored >= 100 {
                break;
            }
            nodes_explored += self.search.nnodes;
            // `backtrackPlunge`.
            if !self.search.backtrack_plunge(&mut self.lp, &mut self.data) {
                break;
            }
            if self.data.conflictpool.num_conflicts() > self.data.options.mip_pool_soft_limit {
                self.data.conflictpool.perform_aging();
            }
            self.search.flush_statistics(&mut self.data);
        }
    }

    /// The main loop's `runHeuristics`: evaluate the node, then randomized
    /// rounding and RENS without an incumbent, RINS with one. Returns
    /// whether to stop with the node.
    fn run_heuristics(&mut self) -> bool {
        if self.search.evaluate_node(&mut self.lp, &mut self.data) == NodeResult::SubOptimal {
            return true;
        }
        if self.search.current_node_pruned() {
            self.search.nleaves += 1;
            return false;
        }
        if self.data.incumbent.is_empty() {
            let sol = self.lp_solution();
            self.heuristics.randomized_rounding(&mut self.data, &sol);
        }
        if self.data.incumbent.is_empty() {
            if self.data.options.mip_heuristic_run_rens {
                let sol = self.lp_solution();
                self.heuristics.rens(&self.lp, &mut self.data, &sol);
            }
        } else if self.data.options.mip_heuristic_run_rins {
            let sol = self.lp_solution();
            self.heuristics.rins(&self.lp, &mut self.data, &sol);
        }
        self.data.domain.infeasible()
    }

    /// HiGHS's `feasibilityJump` driver. Returns whether it found the
    /// model infeasible.
    fn feasibility_jump(&mut self) -> bool {
        super::feasibility_jump::run(&mut self.data)
    }

    /// HiGHS's `cleanupSolve`: the final bounds, gap and status.
    fn cleanup_solve(&mut self) -> MipResult {
        let data = &mut self.data;
        let tol = data.options.mip_feasibility_tolerance;
        let havesolution = data.solution_objective != INF;
        let feasible = havesolution && data.bound_violation <= tol && data.integrality_violation <= tol && data.row_violation <= tol;
        let mut dual_bound = data.lower_bound;
        if data.objective_function.is_integral() {
            let scale = data.objective_function.integral_scale();
            let rounded_lower_bound = (data.lower_bound * scale - data.feastol).ceil() / scale;
            dual_bound = super::cmax(dual_bound, rounded_lower_bound);
        }
        dual_bound += data.model.offset;
        let mut primal_bound = data.upper_bound + data.model.offset;
        dual_bound = super::cmin(dual_bound, primal_bound);
        if data.orig_sense < 0.0 {
            dual_bound = -dual_bound;
            primal_bound = -primal_bound;
        }
        if data.model_status == MipStatus::NotSet || data.model_status == MipStatus::Infeasible {
            data.model_status = if feasible && havesolution { MipStatus::Optimal } else { MipStatus::Infeasible };
        }
        let gap = if primal_bound == 0.0 {
            if dual_bound == 0.0 { 0.0 } else { INF }
        } else if primal_bound != INF {
            (primal_bound - dual_bound).abs() / primal_bound.abs()
        } else {
            INF
        };
        MipResult {
            status: data.model_status,
            solution: data.solution.clone(),
            objective: data.solution_objective,
            dual_bound,
            primal_bound,
            gap,
            nodes: data.num_nodes,
            lp_iterations: data.total_lp_iterations,
        }
    }
}

/// HiGHS's `solveSubMip` from the sub-MIP's options to its solve: a MIP
/// solver on `request`'s LP and bounds with the model's integrality, its
/// limits, the parent's clique table, implications and pseudocosts.
pub(crate) fn run_sub_mip(data: &mut MipData, request: super::heuristics::SubMipRequest) -> super::heuristics::SubMipOutcome {
    let mut options = data.options.clone();
    options.mip_max_leaves = request.max_leaves;
    options.mip_max_nodes = request.max_nodes;
    options.mip_max_stall_nodes = request.stall_nodes;
    options.mip_pscost_minreliable = 0;
    options.time_limit -= data.elapsed();
    options.objective_bound = data.upper_limit;
    if !data.submip {
        let mut curr_abs_gap = data.upper_limit - data.lower_bound;
        if curr_abs_gap == INF {
            curr_abs_gap = data.lower_bound.abs();
            if curr_abs_gap == INF {
                curr_abs_gap = 0.0;
            }
        }
        options.mip_rel_gap = 0.0;
        options.mip_abs_gap = data.feastol * super::cmax(curr_abs_gap, 1000.0);
    }
    options.presolve = !options.mip_root_presolve_only;
    options.mip_detect_symmetry = false;
    options.mip_heuristic_effort = 0.8;

    let lp = request.lp;
    let num_col = request.col_lower.len();
    let mut model = MipModel {
        num_col,
        num_row: lp.row_lower.len(),
        col_cost: lp.col_cost,
        col_lower: request.col_lower,
        col_upper: request.col_upper,
        row_lower: lp.row_lower,
        row_upper: lp.row_upper,
        integrality: data.model.integrality.clone(),
        offset: 0.0,
        a_start: lp.a_start,
        a_index: lp.a_index,
        a_value: lp.a_value,
        ..MipModel::default()
    };
    model.build_row_wise();

    let mut solver = MipSolver::new(options, model.clone(), model, 1.0);
    let sub = &mut solver.data;
    sub.submip = true;
    sub.submip_level = data.submip_level + 1;
    sub.interrupt = data.interrupt.clone();
    sub.rootbasis = request.basis.cloned();
    sub.pscostinit = Some(PseudocostInitialization::new(&data.pseudocost, 1));
    // `init()` with `clqtableinit` and `implicinit`.
    let (lower, upper) = (sub.orig_model.col_lower.clone(), sub.orig_model.col_upper.clone());
    sub.cliquetable.build_from(&lower, &upper, &data.cliquetable);
    let feastol = sub.feastol;
    let (implications, domain, model) = (&mut sub.implications, &sub.domain, &sub.model);
    implications.build_from(&data.implications, domain, model, feastol);
    // A sub-MIP's search is `HighsSearch` for a sub-MIP.
    solver.search = Search::new(&mut solver.data, true);

    let result = solver.run();
    let sub = &solver.data;
    super::heuristics::SubMipOutcome {
        infeasible: result.status == MipStatus::Infeasible,
        solution: sub.solution.clone(),
        node_count: result.nodes,
        total_lp_iterations: sub.total_lp_iterations,
        total_repair_lp: sub.total_repair_lp,
        total_repair_lp_feasible: sub.total_repair_lp_feasible,
        total_repair_lp_iterations: sub.total_repair_lp_iterations,
        max_submip_level: sub.max_submip_level,
        terminated: false,
    }
}
