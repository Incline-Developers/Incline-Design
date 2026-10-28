//! HiGHS's `HEkkDual`: the dual simplex method, serial ("plain") strategy,
//! with dual steepest edge or Devex pricing and the bound-flipping ratio
//! test.

use super::{Algorithm, ModelStatus, dual_rhs::DualRhs, dual_row::DualRow, ekk::*, options::EdgeWeightStrategy};
use crate::hvector::HVector;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeWeightMode {
    Dantzig,
    Devex,
    SteepestEdge,
}

pub(crate) fn solve(ekk: &mut Ekk, force_phase2: bool) {
    let mut dual = Dual::new(ekk);
    dual.solve(ekk, force_phase2);
}

struct Dual {
    num_row: usize,
    num_col: usize,
    num_tot: usize,
    edge_weight_mode: EdgeWeightMode,
    force_phase2: bool,
    solve_phase: i32,
    rebuild_reason: i32,
    row_ep: HVector,
    row_ap: HVector,
    col_aq: HVector,
    col_bfrt: HVector,
    dual_row: DualRow,
    dual_infeas_count: isize,
    dual_rhs: DualRhs,
    row_out: isize,
    variable_out: usize,
    move_out: i32,
    variable_in: Option<usize>,
    delta_primal: f64,
    theta_dual: f64,
    theta_primal: f64,
    alpha_col: f64,
    alpha_row: f64,
    computed_edge_weight: f64,
    num_devex_iterations: usize,
    new_devex_framework: bool,
}

impl Dual {
    fn new(ekk: &Ekk) -> Self {
        let num_row = ekk.lp.num_row;
        let num_col = ekk.lp.num_col;
        let num_tot = num_row + num_col;
        let mut dual_row = DualRow::default();
        dual_row.setup(num_tot);
        let mut dual_rhs = DualRhs::default();
        dual_rhs.setup(num_row);
        Self {
            num_row,
            num_col,
            num_tot,
            edge_weight_mode: EdgeWeightMode::SteepestEdge,
            force_phase2: false,
            solve_phase: SOLVE_PHASE_UNKNOWN,
            rebuild_reason: REBUILD_REASON_NO,
            row_ep: HVector::new(num_row),
            row_ap: HVector::new(num_col),
            col_aq: HVector::new(num_row),
            col_bfrt: HVector::new(num_row),
            dual_row,
            dual_infeas_count: 0,
            dual_rhs,
            row_out: NO_ROW_CHOSEN,
            variable_out: 0,
            move_out: 0,
            variable_in: None,
            delta_primal: 0.0,
            theta_dual: 0.0,
            theta_primal: 0.0,
            alpha_col: 0.0,
            alpha_row: 0.0,
            computed_edge_weight: 0.0,
            num_devex_iterations: 0,
            new_devex_framework: false,
        }
    }

    fn solve(&mut self, ekk: &mut Ekk, pass_force_phase2: bool) {
        self.initialise_solve(ekk);
        if ekk.lp.num_row == 0 {
            ekk.model_status = ModelStatus::SolveError;
            return ekk.return_from_solve();
        }
        self.possibly_use_li_dual_steepest_edge(ekk);

        ekk.initialise_cost(Algorithm::Dual, SOLVE_PHASE_UNKNOWN, false);
        ekk.compute_dual();
        ekk.compute_simplex_dual_infeasible();
        let dual_feasible_with_unperturbed_costs = ekk.info.num_dual_infeasibilities == 0;
        self.force_phase2 = pass_force_phase2 || ekk.info.max_dual_infeasibility * ekk.info.max_dual_infeasibility < ekk.options.dual_feasibility_tolerance;
        let no_simplex_dual_infeasibilities = dual_feasible_with_unperturbed_costs || self.force_phase2;
        let near_optimal = no_simplex_dual_infeasibilities && ekk.info.num_primal_infeasibilities < 1000 && ekk.info.max_primal_infeasibility < 1e-3;
        let perturb_costs = !near_optimal;
        ekk.initialise_cost(Algorithm::Dual, SOLVE_PHASE_UNKNOWN, perturb_costs);
        if ekk.bailout() {
            return ekk.return_from_solve();
        }

        if !ekk.status.has_dual_steepest_edge_weights {
            ekk.dual_edge_weight = vec![1.0; self.num_row];
            ekk.scattered_dual_edge_weight.resize(self.num_tot, 0.0);
            if self.edge_weight_mode == EdgeWeightMode::SteepestEdge {
                if ekk.logical_basis() {
                    ekk.status.has_dual_steepest_edge_weights = true;
                } else if near_optimal {
                    self.edge_weight_mode = EdgeWeightMode::Devex;
                } else {
                    ekk.compute_dual_steepest_edge_weights();
                    ekk.status.has_dual_steepest_edge_weights = true;
                }
            }
            if self.edge_weight_mode == EdgeWeightMode::Devex {
                self.initialise_devex_framework(ekk);
            }
        }
        ekk.info.backtracking_basis_edge_weight.resize(self.num_tot, 0.0);
        ekk.scattered_dual_edge_weight.resize(self.num_tot, 0.0);

        if perturb_costs {
            ekk.compute_dual();
            self.compute_dual_infeasibilities_with_fixed_variable_flips(ekk);
            self.dual_infeas_count = ekk.info.num_dual_infeasibilities;
        }

        self.solve_phase = if self.force_phase2 {
            SOLVE_PHASE_2
        } else if self.dual_infeas_count > 0 {
            SOLVE_PHASE_1
        } else {
            SOLVE_PHASE_2
        };
        while self.solve_phase != 0 {
            ekk.status.has_dual_objective_value = false;
            if self.solve_phase == SOLVE_PHASE_UNKNOWN {
                ekk.initialise_bound(Algorithm::Dual, SOLVE_PHASE_UNKNOWN, false);
                ekk.initialise_nonbasic_value_and_move();
                self.compute_dual_infeasibilities_with_fixed_variable_flips(ekk);
                self.dual_infeas_count = ekk.info.num_dual_infeasibilities;
                self.solve_phase = if self.dual_infeas_count > 0 { SOLVE_PHASE_1 } else { SOLVE_PHASE_2 };
                if ekk.info.backtracking {
                    ekk.initialise_bound(Algorithm::Dual, self.solve_phase, false);
                    ekk.initialise_nonbasic_value_and_move();
                    ekk.info.backtracking = false;
                }
            }
            if self.solve_phase == SOLVE_PHASE_1 {
                self.solve_phase1(ekk);
            } else if self.solve_phase == SOLVE_PHASE_2 {
                self.solve_phase2(ekk);
            } else {
                ekk.model_status = ModelStatus::SolveError;
                return ekk.return_from_solve();
            }
            if ekk.solve_bailout {
                return ekk.return_from_solve();
            }
            if self.solve_phase == SOLVE_PHASE_TABOO_BASIS {
                ekk.model_status = ModelStatus::Unknown;
                return ekk.return_from_solve();
            }
            if self.solve_phase == SOLVE_PHASE_ERROR {
                ekk.model_status = ModelStatus::SolveError;
                return ekk.return_from_solve();
            }
            if self.solve_phase == SOLVE_PHASE_EXIT {
                break;
            }
            if self.solve_phase == SOLVE_PHASE_OPTIMAL_CLEANUP || self.solve_phase == SOLVE_PHASE_PRIMAL_INFEASIBLE_CLEANUP {
                break;
            }
        }
        if self.solve_phase == SOLVE_PHASE_OPTIMAL_CLEANUP || self.solve_phase == SOLVE_PHASE_PRIMAL_INFEASIBLE_CLEANUP {
            ekk.dual_simplex_cleanup_level += 1;
            if self.solve_phase == SOLVE_PHASE_PRIMAL_INFEASIBLE_CLEANUP {
                ekk.compute_simplex_infeasible();
            }
            if ekk.dual_simplex_cleanup_level > ekk.options.max_dual_simplex_cleanup_level {
                ekk.model_status = if self.solve_phase == SOLVE_PHASE_OPTIMAL_CLEANUP {
                    ModelStatus::Optimal
                } else {
                    ModelStatus::Infeasible
                };
            } else {
                // Clean up the remaining dual infeasibilities with the
                // primal simplex method.
                let saved = ekk.info.primal_simplex_bound_perturbation_multiplier;
                ekk.info.primal_simplex_bound_perturbation_multiplier = 0.0;
                super::primal::solve(ekk, true);
                ekk.info.primal_simplex_bound_perturbation_multiplier = saved;
                ekk.called_return_from_solve = false;
                if matches!(ekk.model_status, ModelStatus::SolveError | ModelStatus::NotSet) {
                    return ekk.return_from_solve();
                }
            }
        }
        ekk.return_from_solve();
    }

    fn initialise_solve(&mut self, ekk: &mut Ekk) {
        self.interpret_dual_edge_weight_strategy(ekk.info.dual_edge_weight_strategy);
        ekk.model_status = ModelStatus::NotSet;
        ekk.solve_bailout = false;
        ekk.called_return_from_solve = false;
        ekk.exit_algorithm = Algorithm::Dual;
        self.rebuild_reason = REBUILD_REASON_NO;
    }

    fn interpret_dual_edge_weight_strategy(&mut self, strategy: EdgeWeightStrategy) {
        self.edge_weight_mode = match strategy {
            EdgeWeightStrategy::Choose | EdgeWeightStrategy::SteepestEdge => EdgeWeightMode::SteepestEdge,
            EdgeWeightStrategy::Dantzig => EdgeWeightMode::Dantzig,
            EdgeWeightStrategy::Devex => EdgeWeightMode::Devex,
        };
    }

    /// Use plain rather than squared infeasibilities in CHUZR for LPs whose
    /// entries are all ±1 in short columns.
    fn possibly_use_li_dual_steepest_edge(&self, ekk: &mut Ekk) {
        ekk.info.store_squared_primal_infeasibility = true;
        if ekk.options.less_infeasible_dse_check && is_less_infeasible_dse_candidate(ekk) && ekk.options.less_infeasible_dse_choose_row {
            ekk.info.store_squared_primal_infeasibility = false;
        }
    }

    fn solve_phase1(&mut self, ekk: &mut Ekk) {
        ekk.status.has_primal_objective_value = false;
        ekk.status.has_dual_objective_value = false;
        self.rebuild_reason = REBUILD_REASON_NO;
        if ekk.bailout() {
            return;
        }
        ekk.initialise_bound(Algorithm::Dual, self.solve_phase, false);
        ekk.initialise_nonbasic_value_and_move();
        // Phase 1 boxes the free variables, so an earlier phase 2's list of
        // them must not be used (HiGHS #3353).
        self.dual_row.free_list.clear();
        if !ekk.info.valid_backtracking_basis {
            ekk.put_backtracking_basis();
        }
        loop {
            self.rebuild(ekk);
            if self.solve_phase == SOLVE_PHASE_ERROR {
                ekk.model_status = ModelStatus::SolveError;
                return;
            }
            if self.solve_phase == SOLVE_PHASE_UNKNOWN {
                return;
            }
            if ekk.bailout() {
                break;
            }
            loop {
                self.iterate(ekk);
                if ekk.bailout() {
                    break;
                }
                if self.rebuild_reason != 0 {
                    break;
                }
            }
            if ekk.solve_bailout {
                break;
            }
            let finished = ekk.status.has_fresh_rebuild && !ekk.rebuild_refactor(self.rebuild_reason);
            if finished && ekk.taboo_bad_basis_change() {
                self.solve_phase = SOLVE_PHASE_TABOO_BASIS;
                return;
            }
            if finished {
                break;
            }
        }
        if ekk.solve_bailout {
            return;
        }
        if self.row_out == NO_ROW_CHOSEN {
            if ekk.info.dual_objective_value == 0.0 {
                self.solve_phase = SOLVE_PHASE_2;
            } else {
                self.assess_phase1_optimality(ekk);
            }
        } else if self.rebuild_reason == REBUILD_REASON_CHOOSE_COLUMN_FAIL || self.rebuild_reason == REBUILD_REASON_EXCESSIVE_PRIMAL_VALUE {
            self.solve_phase = SOLVE_PHASE_ERROR;
            ekk.model_status = ModelStatus::SolveError;
        } else if self.variable_in.is_none() {
            if ekk.info.costs_perturbed {
                self.cleanup(ekk);
                if self.dual_infeas_count == 0 {
                    self.solve_phase = SOLVE_PHASE_2;
                }
            } else {
                self.solve_phase = SOLVE_PHASE_ERROR;
                ekk.model_status = ModelStatus::SolveError;
            }
        }
        if self.solve_phase == SOLVE_PHASE_2 || self.solve_phase == SOLVE_PHASE_EXIT || self.solve_phase == SOLVE_PHASE_ERROR {
            ekk.initialise_bound(Algorithm::Dual, SOLVE_PHASE_2, false);
            ekk.initialise_nonbasic_value_and_move();
            if self.solve_phase == SOLVE_PHASE_2 && ekk.dual_simplex_phase1_cleanup_level < ekk.options.max_dual_simplex_phase1_cleanup_level {
                ekk.info.allow_cost_shifting = true;
                ekk.info.allow_cost_perturbation = true;
            }
        }
    }

    fn solve_phase2(&mut self, ekk: &mut Ekk) {
        ekk.status.has_primal_objective_value = false;
        ekk.status.has_dual_objective_value = false;
        self.rebuild_reason = REBUILD_REASON_NO;
        self.solve_phase = SOLVE_PHASE_2;
        ekk.solve_bailout = false;
        if ekk.bailout() {
            return;
        }
        self.dual_row.create_freelist(ekk);
        if !ekk.info.valid_backtracking_basis {
            ekk.put_backtracking_basis();
        }
        loop {
            self.rebuild(ekk);
            if self.solve_phase == SOLVE_PHASE_ERROR {
                ekk.model_status = ModelStatus::SolveError;
                return;
            }
            if self.solve_phase == SOLVE_PHASE_UNKNOWN {
                return;
            }
            if ekk.bailout() {
                break;
            }
            if self.bailout_on_dual_objective(ekk) {
                break;
            }
            if self.dual_infeas_count > 0 {
                break;
            }
            loop {
                self.iterate(ekk);
                if ekk.bailout() {
                    break;
                }
                if self.bailout_on_dual_objective(ekk) {
                    break;
                }
                if self.rebuild_reason == REBUILD_REASON_POSSIBLY_DUAL_UNBOUNDED {
                    self.assess_possibly_dual_unbounded(ekk);
                }
                if self.rebuild_reason != 0 {
                    break;
                }
            }
            if ekk.solve_bailout {
                break;
            }
            let finished = ekk.status.has_fresh_rebuild && !ekk.rebuild_refactor(self.rebuild_reason);
            if finished && ekk.taboo_bad_basis_change() {
                self.solve_phase = SOLVE_PHASE_TABOO_BASIS;
                return;
            }
            if finished {
                break;
            }
        }
        if ekk.solve_bailout {
            return;
        }
        if self.dual_infeas_count > 0 {
            self.solve_phase = SOLVE_PHASE_1;
        } else if self.row_out == NO_ROW_CHOSEN {
            self.cleanup(ekk);
            if self.dual_infeas_count > 0 {
                self.solve_phase = SOLVE_PHASE_OPTIMAL_CLEANUP;
            } else {
                self.solve_phase = SOLVE_PHASE_OPTIMAL;
                ekk.model_status = ModelStatus::Optimal;
            }
        } else if self.rebuild_reason == REBUILD_REASON_CHOOSE_COLUMN_FAIL || self.rebuild_reason == REBUILD_REASON_EXCESSIVE_PRIMAL_VALUE {
            self.solve_phase = SOLVE_PHASE_ERROR;
            ekk.model_status = ModelStatus::SolveError;
        }
    }

    fn rebuild(&mut self, ekk: &mut Ekk) {
        ekk.clear_bad_basis_change_taboo_flag();
        let refactor_basis_matrix = ekk.rebuild_refactor(self.rebuild_reason);
        self.rebuild_reason = REBUILD_REASON_NO;
        if refactor_basis_matrix {
            if !ekk.get_nonsingular_inverse() {
                self.solve_phase = SOLVE_PHASE_ERROR;
                return;
            }
            ekk.reset_synthetic_clock();
        }
        if !ekk.status.has_ar_matrix {
            ekk.initialise_partitioned_rowwise_matrix();
        }
        let check_updated_objective_value = ekk.status.has_dual_objective_value;
        let previous_dual_objective_value = if check_updated_objective_value {
            ekk.info.updated_dual_objective_value
        } else {
            f64::NEG_INFINITY
        };
        ekk.compute_dual();
        if ekk.info.backtracking {
            self.solve_phase = SOLVE_PHASE_UNKNOWN;
            return;
        }
        self.correct_dual_infeasibilities(ekk);
        ekk.compute_primal();
        self.dual_rhs.create_array_of_primal_infeasibilities(ekk);
        let density = ekk.info.col_aq_density;
        self.dual_rhs.create_infeas_list(ekk, density);
        ekk.compute_dual_objective_value(self.solve_phase);
        if check_updated_objective_value {
            let correction = ekk.info.dual_objective_value - previous_dual_objective_value;
            ekk.info.updated_dual_objective_value += correction;
        }
        ekk.info.updated_dual_objective_value = ekk.info.dual_objective_value;
        ekk.reset_synthetic_clock();
        ekk.invalidate_primal_infeasibility_record();
        ekk.invalidate_dual_infeasibility_record();
        ekk.status.has_fresh_rebuild = true;
    }

    /// Remove the cost perturbations and shifts, recomputing the duals.
    fn cleanup(&mut self, ekk: &mut Ekk) {
        if self.solve_phase == SOLVE_PHASE_1 {
            ekk.dual_simplex_phase1_cleanup_level += 1;
        }
        ekk.initialise_cost(Algorithm::Dual, SOLVE_PHASE_UNKNOWN, false);
        ekk.info.allow_cost_perturbation = false;
        ekk.initialise_bound(Algorithm::Dual, self.solve_phase, false);
        ekk.compute_dual();
        ekk.compute_simplex_dual_infeasible();
        self.dual_infeas_count = ekk.info.num_dual_infeasibilities;
        ekk.compute_dual_objective_value(self.solve_phase);
        ekk.info.updated_dual_objective_value = ekk.info.dual_objective_value;
    }

    fn iterate(&mut self, ekk: &mut Ekk) {
        self.choose_row(ekk);
        self.choose_column(ekk);
        if ekk.is_bad_basis_change(self.variable_in, self.row_out, self.rebuild_reason) {
            return;
        }
        self.update_ftran_bfrt(ekk);
        self.update_ftran(ekk);
        if self.edge_weight_mode == EdgeWeightMode::SteepestEdge {
            self.update_ftran_dse(ekk);
        }
        self.update_verify(ekk);
        self.update_dual(ekk);
        self.update_primal(ekk);
        ekk.status.has_primal_objective_value = false;
        self.update_pivots(ekk);
        if self.new_devex_framework {
            self.initialise_devex_framework(ekk);
        }
        self.iteration_analysis(ekk);
    }

    fn iteration_analysis(&mut self, ekk: &mut Ekk) {
        if self.edge_weight_mode == EdgeWeightMode::SteepestEdge && ekk.switch_to_devex() {
            self.edge_weight_mode = EdgeWeightMode::Devex;
            self.initialise_devex_framework(ekk);
        }
    }

    fn choose_row(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        Ekk::apply_taboo_row_out(&mut ekk.bad_basis_change, &mut self.dual_rhs.work_infeasibility, 0.0);
        loop {
            let chosen = self.dual_rhs.choose_normal(ekk);
            let Some(row_out) = chosen else {
                self.row_out = NO_ROW_CHOSEN;
                self.rebuild_reason = REBUILD_REASON_POSSIBLY_OPTIMAL;
                return;
            };
            self.row_out = row_out as isize;
            let row_ep = &mut self.row_ep;
            row_ep.clear();
            row_ep.count = 1;
            row_ep.index[0] = row_out;
            row_ep.array[row_out] = 1.0;
            row_ep.pack_flag = true;
            ekk.nla.btran(row_ep, ekk.info.row_ep_density, &ekk.basis.basic_index);
            if self.edge_weight_mode == EdgeWeightMode::SteepestEdge {
                let updated_edge_weight = ekk.dual_edge_weight[row_out];
                self.computed_edge_weight = if ekk.simplex_in_scaled_space {
                    row_ep.norm2()
                } else {
                    ekk.nla.row_ep_2norm_in_scaled_space(row_out, row_ep, &ekk.basis.basic_index)
                };
                ekk.dual_edge_weight[row_out] = self.computed_edge_weight;
                ekk.assess_dse_weight_error(self.computed_edge_weight, updated_edge_weight);
                if updated_edge_weight >= ACCEPT_DSE_WEIGHT_THRESHOLD * self.computed_edge_weight {
                    break;
                }
            } else {
                break;
            }
        }
        Ekk::unapply_taboo_row_out(&ekk.bad_basis_change, &mut self.dual_rhs.work_infeasibility);
        let row_out = self.row_out as usize;
        self.variable_out = ekk.basis.basic_index[row_out];
        let base_value = ekk.info.base_value[row_out];
        self.delta_primal = if base_value < ekk.info.base_lower[row_out] {
            base_value - ekk.info.base_lower[row_out]
        } else {
            base_value - ekk.info.base_upper[row_out]
        };
        self.move_out = if self.delta_primal < 0.0 { -1 } else { 1 };
        let local_density = self.row_ep.count as f64 / self.num_row as f64;
        update_operation_result_density(local_density, &mut ekk.info.row_ep_density);
    }

    fn choose_column(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        ekk.tableau_row_price(false, &self.row_ep, &mut self.row_ap);
        self.dual_row.clear();
        self.dual_row.work_delta = self.delta_primal;
        self.dual_row.create_freemove(ekk, &self.row_ep);
        self.dual_row.choose_makepack(&self.row_ap, 0);
        self.dual_row.choose_makepack(&self.row_ep, self.num_col);
        let row_ep_scale = Ekk::get_value_scale(self.dual_row.pack_count, &self.dual_row.pack_value);
        let mut chuzc_pass = 0;
        loop {
            self.dual_row.choose_possible(ekk);
            self.variable_in = None;
            if self.dual_row.work_theta <= 0.0 || self.dual_row.work_count == 0 {
                self.rebuild_reason = REBUILD_REASON_POSSIBLY_DUAL_UNBOUNDED;
                return;
            }
            if !self.dual_row.choose_final(ekk) {
                self.rebuild_reason = REBUILD_REASON_CHOOSE_COLUMN_FAIL;
                return;
            }
            let Some(pivot) = self.dual_row.work_pivot else {
                break;
            };
            let growth_tolerance = ekk.options.dual_simplex_pivot_growth_tolerance;
            let scaled_value = row_ep_scale * self.dual_row.work_alpha;
            if scaled_value.abs() <= growth_tolerance {
                if chuzc_pass == 0 {
                    self.improve_choose_column_row(ekk);
                } else {
                    let pack = &mut self.dual_row;
                    for i in 0..pack.pack_count {
                        if pack.pack_index[i] == pivot {
                            pack.pack_index[i] = pack.pack_index[pack.pack_count - 1];
                            pack.pack_value[i] = pack.pack_value[pack.pack_count - 1];
                            pack.pack_count -= 1;
                            break;
                        }
                    }
                }
                self.dual_row.work_pivot = None;
            }
            if self.dual_row.work_pivot.is_some() || self.dual_row.pack_count == 0 {
                break;
            }
            chuzc_pass += 1;
        }
        self.dual_row.delete_freemove(ekk);
        self.variable_in = self.dual_row.work_pivot;
        self.alpha_row = self.dual_row.work_alpha;
        self.theta_dual = self.dual_row.work_theta;
        if self.variable_in.is_none() {
            // Every candidate pivot was too small: treat as unbounded.
            self.rebuild_reason = REBUILD_REASON_POSSIBLY_DUAL_UNBOUNDED;
            return;
        }
        if self.edge_weight_mode == EdgeWeightMode::Devex && !self.new_devex_framework {
            self.dual_row.compute_devex_weight(ekk);
            self.computed_edge_weight = self.dual_row.computed_edge_weight.max(1.0);
        }
    }

    /// Refine the pivotal row and price it again in compensated arithmetic.
    fn improve_choose_column_row(&mut self, ekk: &mut Ekk) {
        self.dual_row.delete_freemove(ekk);
        ekk.unit_btran_iterative_refinement(self.row_out as usize, &mut self.row_ep);
        ekk.tableau_row_price(true, &self.row_ep, &mut self.row_ap);
        self.dual_row.clear();
        self.dual_row.work_delta = self.delta_primal;
        self.dual_row.create_freemove(ekk, &self.row_ep);
        self.dual_row.choose_makepack(&self.row_ap, 0);
        self.dual_row.choose_makepack(&self.row_ep, self.num_col);
    }

    fn update_ftran(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        let variable_in = self.variable_in.unwrap();
        self.col_aq.clear();
        self.col_aq.pack_flag = true;
        ekk.lp.a.collect_aj(&mut self.col_aq, variable_in, 1.0);
        ekk.nla.ftran(&mut self.col_aq, ekk.info.col_aq_density, &ekk.basis.basic_index);
        let local_density = self.col_aq.count as f64 / self.num_row as f64;
        update_operation_result_density(local_density, &mut ekk.info.col_aq_density);
        self.alpha_col = self.col_aq.array[self.row_out as usize];
    }

    fn update_ftran_bfrt(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        self.dual_row.update_flip(ekk, &mut self.col_bfrt);
        if self.col_bfrt.count > 0 {
            ekk.nla.ftran(&mut self.col_bfrt, ekk.info.col_bfrt_density, &ekk.basis.basic_index);
        }
        let local_density = self.col_bfrt.count as f64 / self.num_row as f64;
        update_operation_result_density(local_density, &mut ekk.info.col_bfrt_density);
    }

    /// Turn `row_ep` into the DSE column `B⁻¹ B⁻ᵀ e_p`.
    fn update_ftran_dse(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        ekk.nla.unapply_basis_matrix_row_scale(&mut self.row_ep);
        ekk.nla.ftran_in_scaled_space(&mut self.row_ep, ekk.info.row_dse_density);
        let local_density = self.row_ep.count as f64 / self.num_row as f64;
        update_operation_result_density(local_density, &mut ekk.info.row_dse_density);
    }

    fn update_verify(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        let (reinvert, _) = ekk.reinvert_on_numerical_trouble(self.alpha_col, self.alpha_row, NUMERICAL_TROUBLE_TOLERANCE);
        if reinvert {
            self.rebuild_reason = REBUILD_REASON_POSSIBLY_SINGULAR_BASIS;
        }
    }

    fn update_dual(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        let variable_in = self.variable_in.unwrap();
        if self.theta_dual == 0.0 {
            self.shift_cost(ekk, variable_in, -ekk.info.work_dual[variable_in]);
        } else {
            self.dual_row.update_dual(ekk, self.theta_dual);
        }
        let delta_dual = ekk.info.work_dual[variable_in];
        let value = ekk.info.work_value[variable_in];
        let change = ekk.basis.nonbasic_flag[variable_in] as f64 * (-value * delta_dual) * ekk.cost_scale;
        ekk.info.updated_dual_objective_value += change;
        ekk.info.work_dual[variable_in] = 0.0;
        ekk.info.work_dual[self.variable_out] = -self.theta_dual;
        self.shift_back(ekk, self.variable_out);
    }

    fn update_primal(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        let row_out = self.row_out as usize;
        let variable_in = self.variable_in.unwrap();
        if self.edge_weight_mode == EdgeWeightMode::Devex {
            let updated_edge_weight = ekk.dual_edge_weight[row_out];
            ekk.dual_edge_weight[row_out] = self.computed_edge_weight;
            self.new_devex_framework = self.new_devex_framework_test(updated_edge_weight);
        }
        self.dual_rhs.update_primal(ekk, &self.col_bfrt, 1.0);
        self.dual_rhs.update_infeas_list(ekk, &self.col_bfrt);
        let x_out = ekk.info.base_value[row_out];
        let l_out = ekk.info.base_lower[row_out];
        let u_out = ekk.info.base_upper[row_out];
        self.theta_primal = (x_out - if self.delta_primal < 0.0 { l_out } else { u_out }) / self.alpha_col;
        if !self.dual_rhs.update_primal(ekk, &self.col_aq, self.theta_primal) {
            self.rebuild_reason = REBUILD_REASON_EXCESSIVE_PRIMAL_VALUE;
            return;
        }
        ekk.update_bad_basis_change(&self.col_aq, self.theta_primal);
        if self.edge_weight_mode == EdgeWeightMode::SteepestEdge {
            let pivot = ekk.nla.pivot_in_scaled_space(&self.col_aq, variable_in, row_out, &ekk.basis.basic_index);
            let new_pivotal_edge_weight = ekk.dual_edge_weight[row_out] / (pivot * pivot);
            let kai = -2.0 / pivot;
            ekk.update_dual_steepest_edge_weights(row_out, variable_in, &self.col_aq, new_pivotal_edge_weight, kai, &self.row_ep.array);
            ekk.dual_edge_weight[row_out] = new_pivotal_edge_weight;
        } else if self.edge_weight_mode == EdgeWeightMode::Devex {
            let new_pivotal_edge_weight = (ekk.dual_edge_weight[row_out] / (self.alpha_col * self.alpha_col)).max(1.0);
            ekk.update_dual_devex_weights(&self.col_aq, new_pivotal_edge_weight);
            ekk.dual_edge_weight[row_out] = new_pivotal_edge_weight;
            self.num_devex_iterations += 1;
        }
        self.dual_rhs.update_infeas_list(ekk, &self.col_aq);
        ekk.total_synthetic_tick += self.col_aq.synthetic_tick;
        ekk.total_synthetic_tick += self.row_ep.synthetic_tick;
    }

    fn shift_cost(&mut self, ekk: &mut Ekk, col: usize, amount: f64) {
        ekk.info.costs_shifted = true;
        if amount == 0.0 {
            return;
        }
        ekk.info.work_shift[col] = amount;
    }

    fn shift_back(&mut self, ekk: &mut Ekk, col: usize) {
        if ekk.info.work_shift[col] == 0.0 {
            return;
        }
        ekk.info.work_dual[col] -= ekk.info.work_shift[col];
        ekk.info.work_shift[col] = 0.0;
    }

    fn update_pivots(&mut self, ekk: &mut Ekk) {
        if self.rebuild_reason != 0 {
            return;
        }
        let variable_in = self.variable_in.unwrap();
        let row_out = self.row_out as usize;
        ekk.transform_for_update(&mut self.col_aq, &mut self.row_ep, variable_in, row_out);
        ekk.update_pivots(variable_in, row_out, self.move_out);
        ekk.iteration_count += 1;
        let mut hint = self.rebuild_reason;
        ekk.update_factor(&self.col_aq, &self.row_ep, row_out, &mut hint);
        self.rebuild_reason = hint;
        ekk.update_matrix(variable_in, self.variable_out);
        self.dual_row.delete_freelist(variable_in);
        let value = ekk.info.work_value[variable_in] + self.theta_primal;
        self.dual_rhs.update_pivots(ekk, row_out, value);
    }

    fn initialise_devex_framework(&mut self, ekk: &mut Ekk) {
        ekk.info.devex_index.resize(self.num_tot, 0);
        for var in 0..self.num_tot {
            let flag = ekk.basis.nonbasic_flag[var] as i32;
            ekk.info.devex_index[var] = 1 - flag * flag;
        }
        ekk.dual_edge_weight = vec![1.0; self.num_row];
        self.num_devex_iterations = 0;
        self.new_devex_framework = false;
    }

    fn new_devex_framework_test(&self, updated_edge_weight: f64) -> bool {
        const MIN_ABS_NUMBER_DEVEX_ITERATIONS: usize = 25;
        const MIN_RLV_NUMBER_DEVEX_ITERATIONS: f64 = 1e-2;
        const MAX_ALLOWED_DEVEX_WEIGHT_RATIO: f64 = 3.0;
        let devex_ratio = (updated_edge_weight / self.computed_edge_weight).max(self.computed_edge_weight / updated_edge_weight);
        let i_te = ((self.num_row as f64 / MIN_RLV_NUMBER_DEVEX_ITERATIONS) as usize).max(MIN_ABS_NUMBER_DEVEX_ITERATIONS);
        let accept_ratio = devex_ratio <= MAX_ALLOWED_DEVEX_WEIGHT_RATIO * MAX_ALLOWED_DEVEX_WEIGHT_RATIO;
        let accept_it = self.num_devex_iterations <= i_te;
        !accept_ratio || !accept_it
    }

    fn compute_dual_infeasibilities_with_fixed_variable_flips(&mut self, ekk: &mut Ekk) {
        let tolerance = ekk.options.dual_feasibility_tolerance;
        let mut num = 0;
        let mut max: f64 = 0.0;
        let mut sum = 0.0;
        for var in 0..self.num_tot {
            if ekk.basis.nonbasic_flag[var] == 0 {
                continue;
            }
            let lower = ekk.info.work_lower[var];
            let upper = ekk.info.work_upper[var];
            let dual = ekk.info.work_dual[var];
            let infeasibility = if lower == f64::NEG_INFINITY && upper == f64::INFINITY {
                dual.abs()
            } else {
                -(ekk.basis.nonbasic_move[var] as f64) * dual
            };
            if infeasibility > 0.0 {
                if infeasibility >= tolerance {
                    num += 1;
                }
                max = max.max(infeasibility);
                sum += infeasibility;
            }
        }
        ekk.info.num_dual_infeasibilities = num;
        ekk.info.max_dual_infeasibility = max;
        ekk.info.sum_dual_infeasibilities = sum;
    }

    /// Flip boxed and fixed variables to their dual-feasible bound, and
    /// shift the costs of the others; count free infeasibilities.
    fn correct_dual_infeasibilities(&mut self, ekk: &mut Ekk) {
        let mut free_infeasibility_count = 0;
        let tolerance = ekk.options.dual_feasibility_tolerance;
        for var in 0..self.num_tot {
            if ekk.basis.nonbasic_flag[var] == 0 {
                continue;
            }
            let lower = ekk.info.work_lower[var];
            let upper = ekk.info.work_upper[var];
            let current_dual = ekk.info.work_dual[var];
            let mv = ekk.basis.nonbasic_move[var] as f64;
            let fixed = lower == upper;
            let boxed = lower > f64::NEG_INFINITY && upper < f64::INFINITY;
            let free = lower == f64::NEG_INFINITY && upper == f64::INFINITY;
            if free {
                if current_dual.abs() >= tolerance {
                    free_infeasibility_count += 1;
                }
                continue;
            }
            let dual_infeasibility = -mv * current_dual;
            if dual_infeasibility < tolerance {
                continue;
            }
            if fixed || (boxed && !self.force_phase2) {
                ekk.flip_bound(var);
                continue;
            }
            ekk.info.costs_shifted = true;
            let new_dual = if mv == 1.0 {
                (1.0 + ekk.random.fraction()) * tolerance
            } else {
                -(1.0 + ekk.random.fraction()) * tolerance
            };
            let shift = new_dual - current_dual;
            ekk.info.work_dual[var] = new_dual;
            ekk.info.work_cost[var] += shift;
        }
        self.dual_infeas_count = free_infeasibility_count;
        self.force_phase2 = false;
    }

    fn assess_phase1_optimality(&mut self, ekk: &mut Ekk) {
        if ekk.info.costs_perturbed {
            self.cleanup(ekk);
        }
        self.assess_phase1_optimality_unperturbed(ekk);
        if self.dual_infeas_count <= 0 && self.solve_phase == SOLVE_PHASE_2 {
            self.exit_phase1_reset_duals(ekk);
        }
    }

    fn assess_phase1_optimality_unperturbed(&mut self, ekk: &mut Ekk) {
        if self.dual_infeas_count != 0 {
            return;
        }
        if ekk.info.dual_objective_value == 0.0 {
            self.solve_phase = SOLVE_PHASE_2;
            return;
        }
        ekk.compute_simplex_lp_dual_infeasible();
        if ekk.num_dual_phase_1_lp_dual_infeasibility == 0 {
            self.solve_phase = SOLVE_PHASE_2;
        } else {
            ekk.model_status = ModelStatus::UnboundedOrInfeasible;
            self.solve_phase = SOLVE_PHASE_EXIT;
        }
    }

    fn exit_phase1_reset_duals(&mut self, ekk: &mut Ekk) {
        if !ekk.info.costs_perturbed {
            ekk.initialise_cost(Algorithm::Dual, SOLVE_PHASE_2, true);
            ekk.compute_dual();
        }
        let num_col = ekk.lp.num_col;
        let mut num_shift = 0;
        for var in 0..self.num_tot {
            if ekk.basis.nonbasic_flag[var] == 0 {
                continue;
            }
            let (lp_lower, lp_upper) = if var < num_col {
                (ekk.lp.col_lower[var], ekk.lp.col_upper[var])
            } else {
                let row = var - num_col;
                (ekk.lp.row_lower[row], ekk.lp.row_upper[row])
            };
            if lp_lower == f64::NEG_INFINITY && lp_upper == f64::INFINITY {
                let shift = -ekk.info.work_dual[var];
                ekk.info.work_dual[var] = 0.0;
                ekk.info.work_cost[var] += shift;
                num_shift += 1;
            }
        }
        if num_shift > 0 {
            ekk.info.costs_shifted = true;
        }
    }

    fn bailout_on_dual_objective(&mut self, ekk: &mut Ekk) -> bool {
        // An objective bound is only for minimisation in phase 2, and is
        // infinite unless set.
        if !ekk.solve_bailout && ekk.lp.sense == 1.0 && self.solve_phase == SOLVE_PHASE_2 && ekk.info.updated_dual_objective_value > ekk.options.objective_bound {
            ekk.solve_bailout = self.reached_exact_objective_bound(ekk);
        }
        ekk.solve_bailout
    }

    /// HiGHS's `reachedExactObjectiveBound`: the updated dual objective has
    /// passed the bound, so every so often check the dual objective of the
    /// unperturbed costs before stopping there.
    fn reached_exact_objective_bound(&mut self, ekk: &mut Ekk) -> bool {
        let use_row_ap_density = if ekk.info.row_ap_density.is_nan() {
            0.01
        } else {
            ekk.info.row_ap_density.clamp(0.01, 1.0)
        };
        let check_frequency = ((1.0 / use_row_ap_density) as usize).max(1);
        if !ekk.info.update_count.is_multiple_of(check_frequency) {
            return false;
        }
        let objective_bound = ekk.options.objective_bound;
        let mut dual_col = HVector::new(self.num_row);
        let mut dual_row = HVector::new(self.num_col);
        let exact_dual_objective_value = compute_exact_dual_objective_value(ekk, &mut dual_col, &mut dual_row);
        if exact_dual_objective_value <= objective_bound {
            return false;
        }
        if ekk.info.costs_perturbed || ekk.info.costs_shifted {
            ekk.initialise_cost(Algorithm::Dual, SOLVE_PHASE_2, false);
        }
        for col in 0..self.num_col {
            ekk.info.work_dual[col] = ekk.info.work_cost[col] - dual_row.array[col];
        }
        for var in self.num_col..self.num_tot {
            ekk.info.work_dual[var] = -dual_col.array[var - self.num_col];
        }
        self.force_phase2 = false;
        self.correct_dual_infeasibilities(ekk);
        ekk.model_status = ModelStatus::ObjectiveBound;
        true
    }

    fn assess_possibly_dual_unbounded(&mut self, ekk: &mut Ekk) {
        if self.solve_phase != SOLVE_PHASE_2 || !ekk.status.has_fresh_rebuild {
            return;
        }
        if ekk.proof_of_primal_infeasibility(&mut self.row_ep, self.move_out) {
            self.solve_phase = SOLVE_PHASE_EXIT;
            ekk.dual_ray_value.clear();
            ekk.dual_ray_record = Some((self.row_out as usize, self.move_out));
            ekk.model_status = ModelStatus::Infeasible;
        } else {
            let variable_in = self.variable_in;
            ekk.add_bad_basis_change(
                self.row_out as usize,
                self.variable_out,
                variable_in,
                super::ekk::BadBasisChangeReason::FailedInfeasibilityProof,
                true,
            );
            self.rebuild_reason = REBUILD_REASON_NO;
        }
    }
}

/// HiGHS's `computeExactDualObjectiveValue`: the dual objective of the
/// basis with the LP's own costs, or minus infinity when a nonbasic
/// variable's active bound is infinite. `dual_col` gets the row duals and
/// `dual_row` the columns' `yᵀA`.
fn compute_exact_dual_objective_value(ekk: &mut Ekk, dual_col: &mut HVector, dual_row: &mut HVector) -> f64 {
    let num_col = ekk.lp.num_col;
    let num_row = ekk.lp.num_row;
    dual_col.clear();
    for row in 0..num_row {
        let var = ekk.basis.basic_index[row];
        if var < num_col {
            let value = ekk.lp.col_cost[var];
            if value != 0.0 {
                dual_col.array[row] = value;
                dual_col.index[dual_col.count as usize] = row;
                dual_col.count += 1;
            }
        }
    }
    dual_row.clear();
    if dual_col.count > 0 {
        ekk.nla.btran(dual_col, 1.0, &ekk.basis.basic_index);
        ekk.lp.a.price_by_column(false, dual_row, dual_col);
    }
    ekk.compute_simplex_dual_infeasible();
    let small = ekk.options.small_matrix_value;
    let mut dual_objective = crate::util::CDouble::new(ekk.lp.offset);
    for col in 0..num_col {
        if ekk.basis.nonbasic_flag[col] == 0 {
            continue;
        }
        let exact_dual = ekk.lp.col_cost[col] - dual_row.array[col];
        let active_value = if exact_dual > small {
            ekk.lp.col_lower[col]
        } else if exact_dual < -small {
            ekk.lp.col_upper[col]
        } else {
            ekk.info.work_value[col]
        };
        if active_value.is_infinite() {
            return f64::NEG_INFINITY;
        }
        dual_objective += active_value * exact_dual;
    }
    for var in num_col..num_col + num_row {
        if ekk.basis.nonbasic_flag[var] == 0 {
            continue;
        }
        let row = var - num_col;
        let exact_dual = dual_col.array[row];
        let active_value = if exact_dual > small {
            ekk.lp.row_lower[row]
        } else if exact_dual < -small {
            ekk.lp.row_upper[row]
        } else {
            -ekk.info.work_value[var]
        };
        if active_value.is_infinite() {
            return f64::NEG_INFINITY;
        }
        dual_objective += active_value * exact_dual;
    }
    dual_objective.value()
}

/// HiGHS's `isLessInfeasibleDSECandidate`: all entries ±1, columns short.
fn is_less_infeasible_dse_candidate(ekk: &Ekk) -> bool {
    let a = &ekk.lp.a;
    let max_assess_col_num_en = 24;
    for col in 0..a.num_col {
        let col_num_en = a.start[col + 1] - a.start[col];
        if col_num_en > max_assess_col_num_en {
            return false;
        }
        for value_el in &a.value[a.start[col]..a.start[col + 1]] {
            if value_el.abs() != 1.0 {
                return false;
            }
        }
    }
    let average_col_num_en = a.start[a.num_col] as f64 / a.num_col as f64;
    average_col_num_en <= 6.0
}
