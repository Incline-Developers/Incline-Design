//! HiGHS's `HEkkPrimal`: the primal simplex method with Devex pricing, a
//! composite phase 1, and bound shifting for primal feasibility.

use super::{Algorithm, ModelStatus, ekk::*};
use crate::{hvector::HVector, matrix::HYPER_PRICE_DENSITY, simplex::nla::sparse_loop_style, util::HSet};

const BAD_DEVEX_WEIGHT_FACTOR: f64 = 3.0;
const ALLOWED_NUM_BAD_DEVEX_WEIGHT: usize = 3;

pub(crate) fn solve(ekk: &mut Ekk, force_phase2: bool) {
    let mut primal = Primal::new(ekk);
    primal.solve(ekk, force_phase2);
}

struct Primal {
    num_col: usize,
    num_row: usize,
    num_tot: usize,
    solve_phase: i32,
    primal_feasibility_tolerance: f64,
    dual_feasibility_tolerance: f64,
    rebuild_reason: i32,
    variable_in: Option<usize>,
    move_in: i32,
    row_out: isize,
    variable_out: usize,
    move_out: i32,
    theta_dual: f64,
    theta_primal: f64,
    value_in: f64,
    alpha_col: f64,
    alpha_row: f64,
    num_flip_since_rebuild: usize,
    ph1_sorter_r: Vec<(f64, isize)>,
    ph1_sorter_t: Vec<(f64, isize)>,
    edge_weight: Vec<f64>,
    num_bad_devex_weight: usize,
    devex_index: Vec<i32>,
    num_free_col: usize,
    nonbasic_free_col_set: HSet,
    row_ep: HVector,
    row_ap: HVector,
    col_aq: HVector,
    col_basic_feasibility_change: HVector,
    row_basic_feasibility_change: HVector,
    col_steepest_edge: HVector,
}

impl Primal {
    fn new(ekk: &Ekk) -> Self {
        let num_col = ekk.lp.num_col;
        let num_row = ekk.lp.num_row;
        let num_tot = num_col + num_row;
        let num_free_col = (0..num_tot)
            .filter(|&var| ekk.info.work_lower[var] == f64::NEG_INFINITY && ekk.info.work_upper[var] == f64::INFINITY)
            .count();
        let mut nonbasic_free_col_set = HSet::default();
        if num_free_col > 0 {
            nonbasic_free_col_set.setup(num_free_col, num_tot);
        }
        Self {
            num_col,
            num_row,
            num_tot,
            solve_phase: SOLVE_PHASE_UNKNOWN,
            primal_feasibility_tolerance: 0.0,
            dual_feasibility_tolerance: 0.0,
            rebuild_reason: REBUILD_REASON_NO,
            variable_in: None,
            move_in: 0,
            row_out: NO_ROW_CHOSEN,
            variable_out: 0,
            move_out: 0,
            theta_dual: 0.0,
            theta_primal: 0.0,
            value_in: 0.0,
            alpha_col: 0.0,
            alpha_row: 0.0,
            num_flip_since_rebuild: 0,
            ph1_sorter_r: Vec::with_capacity(num_row),
            ph1_sorter_t: Vec::with_capacity(num_row),
            edge_weight: Vec::new(),
            num_bad_devex_weight: 0,
            devex_index: Vec::new(),
            num_free_col,
            nonbasic_free_col_set,
            row_ep: HVector::new(num_row),
            row_ap: HVector::new(num_col),
            col_aq: HVector::new(num_row),
            col_basic_feasibility_change: HVector::new(num_row),
            row_basic_feasibility_change: HVector::new(num_col),
            col_steepest_edge: HVector::new(num_row),
        }
    }

    fn solve(&mut self, ekk: &mut Ekk, pass_force_phase2: bool) {
        self.initialise_solve(ekk);
        if ekk.lp.num_row == 0 || !ekk.status.has_invert {
            ekk.model_status = ModelStatus::SolveError;
            return ekk.return_from_solve();
        }
        self.get_nonbasic_free_column_set(ekk);
        let primal_feasible_with_unperturbed_bounds = ekk.info.num_primal_infeasibilities == 0;
        let force_phase2 = pass_force_phase2 || ekk.info.max_primal_infeasibility * ekk.info.max_primal_infeasibility < ekk.options.primal_feasibility_tolerance;
        let no_simplex_primal_infeasibilities = primal_feasible_with_unperturbed_bounds || force_phase2;
        let near_optimal = ekk.info.num_dual_infeasibilities < 1000 && ekk.info.max_dual_infeasibility < 1e-3 && no_simplex_primal_infeasibilities;
        let perturb_bounds = !near_optimal;
        if perturb_bounds && ekk.info.primal_simplex_bound_perturbation_multiplier != 0.0 {
            ekk.initialise_bound(Algorithm::Primal, SOLVE_PHASE_UNKNOWN, perturb_bounds);
            ekk.initialise_nonbasic_value_and_move();
            ekk.compute_primal();
            ekk.compute_simplex_primal_infeasible();
        }
        if ekk.bailout() {
            return ekk.return_from_solve();
        }
        self.solve_phase = if ekk.info.num_primal_infeasibilities > 0 { SOLVE_PHASE_1 } else { SOLVE_PHASE_2 };
        if force_phase2 {
            self.solve_phase = SOLVE_PHASE_2;
        }
        ekk.info.backtracking_basis_edge_weight.resize(self.num_tot, 0.0);
        ekk.scattered_dual_edge_weight.resize(self.num_tot, 0.0);

        while self.solve_phase != 0 {
            ekk.status.has_primal_objective_value = false;
            if self.solve_phase == SOLVE_PHASE_UNKNOWN {
                ekk.compute_simplex_primal_infeasible();
                self.solve_phase = if ekk.info.num_primal_infeasibilities > 0 { SOLVE_PHASE_1 } else { SOLVE_PHASE_2 };
                if ekk.info.backtracking {
                    ekk.initialise_cost(Algorithm::Primal, self.solve_phase, false);
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
            if self.solve_phase == SOLVE_PHASE_EXIT || self.solve_phase == SOLVE_PHASE_OPTIMAL_CLEANUP {
                break;
            }
        }
        if self.solve_phase == SOLVE_PHASE_OPTIMAL {
            ekk.model_status = ModelStatus::Optimal;
        }
        if self.solve_phase == SOLVE_PHASE_OPTIMAL_CLEANUP {
            // Clean up the remaining primal infeasibilities with the dual
            // simplex method, unperturbed.
            ekk.compute_primal_objective_value();
            let saved = ekk.info.dual_simplex_cost_perturbation_multiplier;
            ekk.info.dual_simplex_cost_perturbation_multiplier = 0.0;
            super::dual::solve(ekk, true);
            ekk.info.dual_simplex_cost_perturbation_multiplier = saved;
            ekk.called_return_from_solve = false;
            if matches!(ekk.model_status, ModelStatus::SolveError | ModelStatus::NotSet) {
                return ekk.return_from_solve();
            }
        }
        ekk.return_from_solve();
    }

    fn initialise_solve(&mut self, ekk: &mut Ekk) {
        self.primal_feasibility_tolerance = ekk.options.primal_feasibility_tolerance;
        self.dual_feasibility_tolerance = ekk.options.dual_feasibility_tolerance;
        ekk.status.has_primal_objective_value = false;
        ekk.status.has_dual_objective_value = false;
        ekk.model_status = ModelStatus::NotSet;
        ekk.solve_bailout = false;
        ekk.called_return_from_solve = false;
        ekk.exit_algorithm = Algorithm::Primal;
        self.rebuild_reason = REBUILD_REASON_NO;
        if !ekk.status.has_dual_steepest_edge_weights {
            ekk.dual_edge_weight = vec![1.0; self.num_row];
            ekk.scattered_dual_edge_weight.resize(self.num_tot, 0.0);
        }
        self.initialise_devex_framework(ekk);
    }

    fn solve_phase1(&mut self, ekk: &mut Ekk) {
        ekk.status.has_primal_objective_value = false;
        ekk.status.has_dual_objective_value = false;
        if ekk.bailout() {
            return;
        }
        if !ekk.info.valid_backtracking_basis {
            ekk.put_backtracking_basis();
        }
        loop {
            self.rebuild(ekk);
            if self.solve_phase == SOLVE_PHASE_ERROR || self.solve_phase == SOLVE_PHASE_UNKNOWN {
                return;
            }
            if ekk.bailout() {
                return;
            }
            if self.solve_phase == SOLVE_PHASE_2 {
                break;
            }
            loop {
                self.iterate(ekk);
                if ekk.bailout() || self.solve_phase == SOLVE_PHASE_ERROR {
                    return;
                }
                if self.rebuild_reason != 0 {
                    break;
                }
            }
            let finished = ekk.status.has_fresh_rebuild && self.num_flip_since_rebuild == 0 && !ekk.rebuild_refactor(self.rebuild_reason);
            if finished && ekk.taboo_bad_basis_change() {
                self.solve_phase = SOLVE_PHASE_TABOO_BASIS;
                return;
            }
            if finished {
                break;
            }
        }
        if self.solve_phase == SOLVE_PHASE_1 && self.variable_in.is_none() {
            if ekk.info.bounds_shifted || ekk.info.bounds_perturbed {
                self.cleanup(ekk);
            } else {
                ekk.model_status = ModelStatus::Infeasible;
                self.solve_phase = SOLVE_PHASE_EXIT;
            }
        }
    }

    fn solve_phase2(&mut self, ekk: &mut Ekk) {
        ekk.status.has_primal_objective_value = false;
        ekk.status.has_dual_objective_value = false;
        if ekk.bailout() {
            return;
        }
        if !ekk.info.valid_backtracking_basis {
            ekk.put_backtracking_basis();
        }
        loop {
            self.rebuild(ekk);
            if self.solve_phase == SOLVE_PHASE_ERROR || self.solve_phase == SOLVE_PHASE_UNKNOWN {
                return;
            }
            if ekk.bailout() {
                return;
            }
            if self.solve_phase == SOLVE_PHASE_1 {
                break;
            }
            loop {
                self.iterate(ekk);
                if ekk.bailout() || self.solve_phase == SOLVE_PHASE_ERROR {
                    return;
                }
                if self.rebuild_reason != 0 {
                    break;
                }
            }
            let finished = ekk.status.has_fresh_rebuild && self.num_flip_since_rebuild == 0 && !ekk.rebuild_refactor(self.rebuild_reason);
            if finished && ekk.taboo_bad_basis_change() {
                self.solve_phase = SOLVE_PHASE_TABOO_BASIS;
                return;
            }
            if finished {
                break;
            }
        }
        if self.solve_phase == SOLVE_PHASE_1 {
        } else if self.variable_in.is_none() {
            self.cleanup(ekk);
            if ekk.info.num_primal_infeasibilities > 0 {
                self.solve_phase = SOLVE_PHASE_OPTIMAL_CLEANUP;
            } else {
                self.solve_phase = SOLVE_PHASE_OPTIMAL;
                ekk.model_status = ModelStatus::Optimal;
                ekk.compute_dual_objective_value(SOLVE_PHASE_2);
            }
        } else if ekk.info.bounds_shifted || ekk.info.bounds_perturbed {
            self.cleanup(ekk);
            if ekk.info.num_primal_infeasibilities > 0 {
                self.solve_phase = SOLVE_PHASE_1;
            }
        } else {
            self.solve_phase = SOLVE_PHASE_EXIT;
            ekk.primal_ray_record = Some((self.variable_in.unwrap(), -self.move_in));
            ekk.model_status = ModelStatus::Unbounded;
        }
    }

    fn cleanup(&mut self, ekk: &mut Ekk) {
        if !ekk.info.bounds_shifted && !ekk.info.bounds_perturbed {
            return;
        }
        ekk.initialise_bound(Algorithm::Primal, self.solve_phase, false);
        ekk.initialise_nonbasic_value_and_move();
        ekk.info.allow_bound_perturbation = false;
        ekk.compute_primal();
        ekk.compute_simplex_primal_infeasible();
        ekk.compute_primal_objective_value();
        ekk.info.updated_primal_objective_value = ekk.info.primal_objective_value;
        ekk.compute_simplex_dual_infeasible();
    }

    fn rebuild(&mut self, ekk: &mut Ekk) {
        ekk.clear_bad_basis_change_taboo_flag();
        let check_updated_objective_value = ekk.status.has_primal_objective_value;
        let previous_primal_objective_value = if check_updated_objective_value {
            ekk.info.updated_primal_objective_value
        } else {
            f64::NEG_INFINITY
        };
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
        if ekk.info.backtracking {
            self.solve_phase = SOLVE_PHASE_UNKNOWN;
            return;
        }
        ekk.compute_primal();
        if self.solve_phase == SOLVE_PHASE_2 {
            self.correct_primal(ekk);
        }
        self.get_basic_primal_infeasibility(ekk);
        if ekk.info.num_primal_infeasibilities > 0 {
            if self.solve_phase == SOLVE_PHASE_2 {
                self.solve_phase = SOLVE_PHASE_1;
            }
            self.phase1_compute_dual(ekk);
        } else {
            if self.solve_phase == SOLVE_PHASE_1 {
                ekk.initialise_cost(Algorithm::Primal, self.solve_phase, false);
                self.solve_phase = SOLVE_PHASE_2;
            }
            ekk.compute_dual();
        }
        ekk.compute_simplex_dual_infeasible();
        ekk.compute_primal_objective_value();
        if check_updated_objective_value {
            let correction = ekk.info.primal_objective_value - previous_primal_objective_value;
            ekk.info.updated_primal_objective_value += correction;
        }
        ekk.info.updated_primal_objective_value = ekk.info.primal_objective_value;
        ekk.reset_synthetic_clock();
        self.num_flip_since_rebuild = 0;
        ekk.status.has_fresh_rebuild = true;
    }

    fn iterate(&mut self, ekk: &mut Ekk) {
        self.row_out = NO_ROW_SOUGHT;
        self.chuzc(ekk);
        if self.variable_in.is_none() {
            self.rebuild_reason = REBUILD_REASON_POSSIBLY_OPTIMAL;
            return;
        }
        if !self.use_variable_in(ekk) {
            return;
        }
        if self.solve_phase == SOLVE_PHASE_1 {
            self.phase1_choose_row(ekk);
            if self.row_out == NO_ROW_CHOSEN {
                self.solve_phase = SOLVE_PHASE_ERROR;
                return;
            }
        } else {
            self.choose_row(ekk);
        }
        self.consider_bound_swap(ekk);
        if self.rebuild_reason == REBUILD_REASON_POSSIBLY_PRIMAL_UNBOUNDED {
            return;
        }
        if self.row_out >= 0 {
            self.assess_pivot(ekk);
            if self.rebuild_reason != 0 {
                return;
            }
        }
        if ekk.is_bad_basis_change(self.variable_in, self.row_out, self.rebuild_reason) {
            return;
        }
        self.update(ekk);
        if ekk.info.num_primal_infeasibilities == 0 && self.solve_phase == SOLVE_PHASE_1 {
            self.rebuild_reason = REBUILD_REASON_POSSIBLY_PHASE1_FEASIBLE;
        }
    }

    fn chuzc(&mut self, ekk: &mut Ekk) {
        Ekk::apply_taboo_variable_in(&mut ekk.bad_basis_change, &mut ekk.info.work_dual, 0.0);
        self.choose_column(ekk);
        Ekk::unapply_taboo_variable_in(&ekk.bad_basis_change, &mut ekk.info.work_dual);
    }

    fn choose_column(&mut self, ekk: &Ekk) {
        let nonbasic_move = &ekk.basis.nonbasic_move;
        let work_dual = &ekk.info.work_dual;
        let mut best_measure = 0.0;
        self.variable_in = None;
        for &col in self.nonbasic_free_col_set.entries() {
            let dual_infeasibility = work_dual[col].abs();
            if dual_infeasibility > self.dual_feasibility_tolerance && dual_infeasibility * dual_infeasibility > best_measure * self.edge_weight[col] {
                self.variable_in = Some(col);
                best_measure = dual_infeasibility * dual_infeasibility / self.edge_weight[col];
            }
        }
        for col in 0..self.num_tot {
            let dual_infeasibility = -(nonbasic_move[col] as f64) * work_dual[col];
            if dual_infeasibility > self.dual_feasibility_tolerance && dual_infeasibility * dual_infeasibility > best_measure * self.edge_weight[col] {
                self.variable_in = Some(col);
                best_measure = dual_infeasibility * dual_infeasibility / self.edge_weight[col];
            }
        }
    }

    fn use_variable_in(&mut self, ekk: &mut Ekk) -> bool {
        let variable_in = self.variable_in.unwrap();
        let updated_theta_dual = ekk.info.work_dual[variable_in];
        self.move_in = if updated_theta_dual > 0.0 { -1 } else { 1 };
        ekk.pivot_column_ftran(variable_in, &mut self.col_aq);
        let computed_theta_dual = ekk.compute_dual_for_tableau_column(variable_in, &self.col_aq);
        ekk.info.work_dual[variable_in] = computed_theta_dual;
        self.theta_dual = computed_theta_dual;
        let theta_dual_small = self.theta_dual.abs() <= self.dual_feasibility_tolerance;
        let theta_dual_sign_error = updated_theta_dual * computed_theta_dual <= 0.0;
        if theta_dual_small {
            ekk.info.num_dual_infeasibilities -= 1;
        }
        if theta_dual_small || theta_dual_sign_error {
            if !theta_dual_small && ekk.info.update_count > 0 {
                self.rebuild_reason = REBUILD_REASON_POSSIBLY_SINGULAR_BASIS;
            }
            return false;
        }
        true
    }

    fn phase1_choose_row(&mut self, ekk: &Ekk) {
        let info = &ekk.info;
        let tolerance = self.primal_feasibility_tolerance;
        let num_row = self.num_row as isize;
        let pivot_tolerance = if info.update_count < 10 {
            1e-9
        } else if info.update_count < 20 {
            1e-8
        } else {
            1e-7
        };
        self.ph1_sorter_r.clear();
        self.ph1_sorter_t.clear();
        let move_in = self.move_in as f64;
        for &row in self.col_aq.indices() {
            let alpha = self.col_aq.array[row] * move_in;
            let (value, lower, upper) = (info.base_value[row], info.base_lower[row], info.base_upper[row]);
            let irow = row as isize;
            if alpha > pivot_tolerance {
                if value > upper + tolerance {
                    let feas_theta = (value - upper - tolerance) / alpha;
                    self.ph1_sorter_r.push((feas_theta, irow));
                    self.ph1_sorter_t.push((feas_theta, irow));
                }
                if value > lower - tolerance && lower > f64::NEG_INFINITY {
                    let relax_theta = (value - lower + tolerance) / alpha;
                    let tight_theta = (value - lower) / alpha;
                    self.ph1_sorter_r.push((relax_theta, irow - num_row));
                    self.ph1_sorter_t.push((tight_theta, irow - num_row));
                }
            }
            if alpha < -pivot_tolerance {
                if value < lower - tolerance {
                    let feas_theta = (value - lower + tolerance) / alpha;
                    self.ph1_sorter_r.push((feas_theta, irow - num_row));
                    self.ph1_sorter_t.push((feas_theta, irow - num_row));
                }
                if value < upper + tolerance && upper < f64::INFINITY {
                    let relax_theta = (value - upper - tolerance) / alpha;
                    let tight_theta = (value - upper) / alpha;
                    self.ph1_sorter_r.push((relax_theta, irow));
                    self.ph1_sorter_t.push((tight_theta, irow));
                }
            }
        }
        if self.ph1_sorter_r.is_empty() {
            self.row_out = NO_ROW_CHOSEN;
            return;
        }
        let row_of = |index: isize| if index >= 0 { index as usize } else { (index + num_row) as usize };
        self.ph1_sorter_r.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut max_theta = self.ph1_sorter_r[0].0;
        let mut gradient = self.theta_dual.abs();
        for &(theta, index) in &self.ph1_sorter_r {
            gradient -= self.col_aq.array[row_of(index)].abs();
            if gradient <= 0.0 {
                break;
            }
            max_theta = theta;
        }
        self.ph1_sorter_t.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut max_alpha: f64 = 0.0;
        let mut last = self.ph1_sorter_t.len();
        for (i, &(theta, index)) in self.ph1_sorter_t.iter().enumerate() {
            let abs_alpha = self.col_aq.array[row_of(index)].abs();
            if theta > max_theta {
                last = i;
                break;
            }
            max_alpha = max_alpha.max(abs_alpha);
        }
        self.row_out = NO_ROW_CHOSEN;
        self.move_out = 0;
        for i in (1..=last).rev() {
            let index = self.ph1_sorter_t[i - 1].1;
            let row = row_of(index);
            if self.col_aq.array[row].abs() > max_alpha * 0.1 {
                self.row_out = row as isize;
                self.move_out = if index >= 0 { 1 } else { -1 };
                break;
            }
        }
    }

    fn choose_row(&mut self, ekk: &Ekk) {
        let info = &ekk.info;
        let tolerance = self.primal_feasibility_tolerance;
        self.row_out = NO_ROW_CHOSEN;
        let alpha_tolerance = if info.update_count < 10 {
            1e-9
        } else if info.update_count < 20 {
            1e-8
        } else {
            1e-7
        };
        let move_in = self.move_in as f64;
        let mut relax_theta = 1e100;
        for &row in self.col_aq.indices() {
            let alpha = self.col_aq.array[row] * move_in;
            if alpha > alpha_tolerance {
                let relax_space = info.base_value[row] - info.base_lower[row] + tolerance;
                if relax_space < relax_theta * alpha {
                    relax_theta = relax_space / alpha;
                }
            } else if alpha < -alpha_tolerance {
                let relax_space = info.base_value[row] - info.base_upper[row] - tolerance;
                if relax_space > relax_theta * alpha {
                    relax_theta = relax_space / alpha;
                }
            }
        }
        let mut best_alpha = 0.0;
        for &row in self.col_aq.indices() {
            let alpha = self.col_aq.array[row] * move_in;
            if alpha > alpha_tolerance {
                let tight_space = info.base_value[row] - info.base_lower[row];
                if tight_space < relax_theta * alpha && best_alpha < alpha {
                    best_alpha = alpha;
                    self.row_out = row as isize;
                }
            } else if alpha < -alpha_tolerance {
                let tight_space = info.base_value[row] - info.base_upper[row];
                if tight_space > relax_theta * alpha && best_alpha < -alpha {
                    best_alpha = -alpha;
                    self.row_out = row as isize;
                }
            }
        }
    }

    fn consider_bound_swap(&mut self, ekk: &Ekk) {
        let info = &ekk.info;
        let variable_in = self.variable_in.unwrap();
        if self.row_out == NO_ROW_CHOSEN {
            self.theta_primal = self.move_in as f64 * f64::INFINITY;
            self.move_out = 0;
        } else {
            let row_out = self.row_out as usize;
            self.alpha_col = self.col_aq.array[row_out];
            if self.solve_phase == SOLVE_PHASE_2 {
                self.move_out = if self.alpha_col * self.move_in as f64 > 0.0 { -1 } else { 1 };
            }
            self.theta_primal = if self.move_out == 1 {
                (info.base_value[row_out] - info.base_upper[row_out]) / self.alpha_col
            } else {
                (info.base_value[row_out] - info.base_lower[row_out]) / self.alpha_col
            };
        }
        let lower_in = info.work_lower[variable_in];
        let upper_in = info.work_upper[variable_in];
        self.value_in = info.work_value[variable_in] + self.theta_primal;
        let mut flipped = false;
        if self.move_in > 0 {
            if self.value_in > upper_in + self.primal_feasibility_tolerance {
                flipped = true;
                self.row_out = NO_ROW_CHOSEN;
                self.value_in = upper_in;
                self.theta_primal = upper_in - lower_in;
            }
        } else if self.value_in < lower_in - self.primal_feasibility_tolerance {
            flipped = true;
            self.row_out = NO_ROW_CHOSEN;
            self.value_in = lower_in;
            self.theta_primal = lower_in - upper_in;
        }
        let pivot_or_flipped = self.row_out >= 0 || flipped;
        if self.solve_phase == SOLVE_PHASE_2 && !pivot_or_flipped {
            self.rebuild_reason = REBUILD_REASON_POSSIBLY_PRIMAL_UNBOUNDED;
        }
    }

    fn assess_pivot(&mut self, ekk: &mut Ekk) {
        let row_out = self.row_out as usize;
        self.alpha_col = self.col_aq.array[row_out];
        self.variable_out = ekk.basis.basic_index[row_out];
        ekk.unit_btran(row_out, &mut self.row_ep);
        ekk.tableau_row_price(false, &self.row_ep, &mut self.row_ap);
        self.update_verify(ekk);
    }

    fn update_verify(&mut self, ekk: &Ekk) {
        let variable_in = self.variable_in.unwrap();
        let abs_alpha_from_col = self.alpha_col.abs();
        self.alpha_row = if variable_in < self.num_col {
            self.row_ap.array[variable_in]
        } else {
            self.row_ep.array[variable_in - self.num_col]
        };
        let abs_alpha_from_row = self.alpha_row.abs();
        let numerical_trouble = (abs_alpha_from_col - abs_alpha_from_row).abs() / abs_alpha_from_col.min(abs_alpha_from_row);
        if numerical_trouble > 1e-7 && ekk.info.update_count > 0 {
            self.rebuild_reason = REBUILD_REASON_POSSIBLY_SINGULAR_BASIS;
        }
    }

    fn update(&mut self, ekk: &mut Ekk) {
        let variable_in = self.variable_in.unwrap();
        let flipped = self.row_out < 0;
        if flipped {
            self.variable_out = variable_in;
            self.alpha_col = 0.0;
            ekk.info.work_value[variable_in] = self.value_in;
            ekk.basis.nonbasic_move[variable_in] = -self.move_in as i8;
        } else {
            self.adjust_perturbed_equation_out(ekk);
        }
        if self.solve_phase == SOLVE_PHASE_1 {
            self.phase1_update_primal(ekk);
            self.basic_feasibility_change_update_dual(ekk);
        } else {
            self.phase2_update_primal(ekk);
        }
        if flipped {
            ekk.invalidate_dual_infeasibility_record();
            self.num_flip_since_rebuild += 1;
            ekk.total_synthetic_tick += self.col_aq.synthetic_tick;
            return;
        }
        let row_out = self.row_out as usize;
        ekk.info.base_value[row_out] = self.value_in;
        self.consider_infeasible_value_in(ekk);
        self.theta_dual = ekk.info.work_dual[variable_in];
        self.update_dual(ekk);
        self.update_devex(ekk);
        self.remove_nonbasic_free_column(ekk);
        if ekk.status.has_dual_steepest_edge_weights {
            self.update_dual_steepest_edge_weights(ekk);
        }
        ekk.transform_for_update(&mut self.col_aq, &mut self.row_ep, variable_in, row_out);
        ekk.update_pivots(variable_in, row_out, self.move_out);
        let mut hint = self.rebuild_reason;
        ekk.update_factor(&self.col_aq, &self.row_ep, row_out, &mut hint);
        self.rebuild_reason = hint;
        ekk.update_matrix(variable_in, self.variable_out);
        if ekk.info.update_count >= ekk.info.update_limit {
            self.rebuild_reason = REBUILD_REASON_UPDATE_LIMIT_REACHED;
        }
        ekk.iteration_count += 1;
        if self.num_bad_devex_weight > ALLOWED_NUM_BAD_DEVEX_WEIGHT {
            self.initialise_devex_framework(ekk);
        }
        ekk.total_synthetic_tick += self.col_aq.synthetic_tick;
        ekk.total_synthetic_tick += self.row_ep.synthetic_tick;
    }

    fn update_dual(&mut self, ekk: &mut Ekk) {
        let variable_in = self.variable_in.unwrap();
        let work_dual = &mut ekk.info.work_dual;
        self.theta_dual = work_dual[variable_in] / self.alpha_col;
        for &col in self.row_ap.indices() {
            work_dual[col] -= self.theta_dual * self.row_ap.array[col];
        }
        for &row in self.row_ep.indices() {
            work_dual[row + self.num_col] -= self.theta_dual * self.row_ep.array[row];
        }
        work_dual[variable_in] = 0.0;
        work_dual[self.variable_out] = -self.theta_dual;
        ekk.invalidate_dual_infeasibility_record();
        ekk.status.has_dual_objective_value = false;
    }

    /// Phase 1 costs: -1 below a lower bound, 1 above an upper.
    fn phase1_compute_dual(&mut self, ekk: &mut Ekk) {
        let info = &mut ekk.info;
        let mut buffer = HVector::new(self.num_row);
        info.work_cost.iter_mut().for_each(|cost| *cost = 0.0);
        info.work_dual.iter_mut().for_each(|dual| *dual = 0.0);
        let base = info.primal_simplex_phase1_cost_perturbation_multiplier * 5e-7;
        let tolerance = self.primal_feasibility_tolerance;
        for row in 0..self.num_row {
            let value = info.base_value[row];
            let bound_violated = if value < info.base_lower[row] - tolerance {
                -1.0
            } else if value > info.base_upper[row] + tolerance {
                1.0
            } else {
                continue;
            };
            let mut cost = bound_violated;
            if base != 0.0 {
                cost *= 1.0 + base * info.num_tot_random_value[row];
            }
            buffer.array[row] = cost;
            buffer.index[buffer.count as usize] = row;
            buffer.count += 1;
        }
        if buffer.count <= 0 {
            return;
        }
        for row in 0..self.num_row {
            info.work_cost[ekk.basis.basic_index[row]] = buffer.array[row];
        }
        ekk.full_btran(&mut buffer);
        let mut buffer_long = HVector::new(self.num_col);
        ekk.full_price(&buffer, &mut buffer_long);
        for col in 0..self.num_col {
            ekk.info.work_dual[col] = -(ekk.basis.nonbasic_flag[col] as f64) * buffer_long.array[col];
        }
        for row in 0..self.num_row {
            let col = self.num_col + row;
            ekk.info.work_dual[col] = -(ekk.basis.nonbasic_flag[col] as f64) * buffer.array[row];
        }
    }

    fn phase1_update_primal(&mut self, ekk: &mut Ekk) {
        let info = &mut ekk.info;
        self.col_basic_feasibility_change.clear();
        let base = info.primal_simplex_phase1_cost_perturbation_multiplier * 5e-7;
        let tolerance = self.primal_feasibility_tolerance;
        for k in 0..self.col_aq.count as usize {
            let row = self.col_aq.index[k];
            info.base_value[row] -= self.theta_primal * self.col_aq.array[row];
            let col = ekk.basis.basic_index[row];
            let was_cost = info.work_cost[col];
            let value = info.base_value[row];
            let bound_violated = if value < info.base_lower[row] - tolerance {
                -1.0
            } else if value > info.base_upper[row] + tolerance {
                1.0
            } else {
                0.0
            };
            let mut cost = bound_violated;
            if base != 0.0 {
                cost *= 1.0 + base * info.num_tot_random_value[row];
            }
            info.work_cost[col] = cost;
            if was_cost != 0.0 {
                if cost == 0.0 {
                    info.num_primal_infeasibilities -= 1;
                }
            } else if cost != 0.0 {
                info.num_primal_infeasibilities += 1;
            }
            let delta_cost = cost - was_cost;
            if delta_cost != 0.0 {
                let change = &mut self.col_basic_feasibility_change;
                change.array[row] = delta_cost;
                change.index[change.count as usize] = row;
                change.count += 1;
                if col >= self.num_col {
                    info.work_dual[col] += delta_cost;
                }
            }
        }
    }

    fn consider_infeasible_value_in(&mut self, ekk: &mut Ekk) {
        let variable_in = self.variable_in.unwrap();
        let row_out = self.row_out as usize;
        let tolerance = self.primal_feasibility_tolerance;
        let info = &mut ekk.info;
        let base = info.primal_simplex_phase1_cost_perturbation_multiplier * 5e-7;
        let lower = info.work_lower[variable_in];
        let upper = info.work_upper[variable_in];
        let bound_violated = if self.value_in < lower - tolerance {
            -1
        } else if self.value_in > upper + tolerance {
            1
        } else {
            return;
        };
        if self.solve_phase == SOLVE_PHASE_1 {
            info.num_primal_infeasibilities += 1;
            let mut cost = bound_violated as f64;
            if base != 0.0 {
                cost *= 1.0 + base * info.num_tot_random_value[row_out];
            }
            info.work_cost[variable_in] = cost;
            info.work_dual[variable_in] += cost;
        } else {
            let random_value = info.num_tot_random_value[variable_in];
            if bound_violated > 0 {
                let shift = shift_bound(false, self.value_in, random_value, &mut info.work_upper[variable_in], tolerance);
                info.work_upper_shift[variable_in] += shift;
            } else {
                let shift = shift_bound(true, self.value_in, random_value, &mut info.work_lower[variable_in], tolerance);
                info.work_lower_shift[variable_in] += shift;
            }
            info.bounds_perturbed = true;
        }
    }

    /// Update the basic values, shifting any bound they now violate.
    fn phase2_update_primal(&mut self, ekk: &mut Ekk) {
        let variable_in = self.variable_in.unwrap();
        let tolerance = self.primal_feasibility_tolerance;
        let (use_col_indices, to_entry) = sparse_loop_style(self.col_aq.count, self.num_row);
        let info = &mut ekk.info;
        for entry in 0..to_entry {
            let row = if use_col_indices { self.col_aq.index[entry] } else { entry };
            info.base_value[row] -= self.theta_primal * self.col_aq.array[row];
            let value = info.base_value[row];
            let bound_violated = if value < info.base_lower[row] - tolerance {
                -1
            } else if value > info.base_upper[row] + tolerance {
                1
            } else {
                continue;
            };
            let col = ekk.basis.basic_index[row];
            let random_value = info.num_tot_random_value[col];
            if bound_violated > 0 {
                let shift = shift_bound(false, value, random_value, &mut info.work_upper[col], tolerance);
                info.base_upper[row] = info.work_upper[col];
                info.work_upper_shift[col] += shift;
            } else {
                let shift = shift_bound(true, value, random_value, &mut info.work_lower[col], tolerance);
                info.base_lower[row] = info.work_lower[col];
                info.work_lower_shift[col] += shift;
            }
            info.bounds_shifted = true;
        }
        info.updated_primal_objective_value += info.work_dual[variable_in] * self.theta_primal;
    }

    fn correct_primal(&mut self, ekk: &mut Ekk) -> bool {
        let tolerance = self.primal_feasibility_tolerance;
        let info = &mut ekk.info;
        let mut skipped = 0;
        for row in 0..self.num_row {
            let value = info.base_value[row];
            let bound_violated = if value < info.base_lower[row] - tolerance {
                -1
            } else if value > info.base_upper[row] + tolerance {
                1
            } else {
                continue;
            };
            if !info.allow_bound_perturbation {
                skipped += 1;
                continue;
            }
            let col = ekk.basis.basic_index[row];
            let random_value = info.num_tot_random_value[col];
            if bound_violated > 0 {
                let shift = shift_bound(false, value, random_value, &mut info.work_upper[col], tolerance);
                info.base_upper[row] = info.work_upper[col];
                info.work_upper_shift[col] += shift;
            } else {
                let shift = shift_bound(true, value, random_value, &mut info.work_lower[col], tolerance);
                info.base_lower[row] = info.work_lower[col];
                info.work_lower_shift[col] += shift;
            }
            info.bounds_perturbed = true;
        }
        skipped == 0
    }

    fn basic_feasibility_change_update_dual(&mut self, ekk: &mut Ekk) {
        ekk.nla.btran(
            &mut self.col_basic_feasibility_change,
            ekk.info.col_basic_feasibility_change_density,
            &ekk.basis.basic_index,
        );
        let local_density = self.col_basic_feasibility_change.count as f64 / self.num_row as f64;
        update_operation_result_density(local_density, &mut ekk.info.col_basic_feasibility_change_density);
        self.basic_feasibility_change_price(ekk);
        let (use_row_indices, to_entry) = sparse_loop_style(self.row_basic_feasibility_change.count, self.num_col);
        for entry in 0..to_entry {
            let col = if use_row_indices { self.row_basic_feasibility_change.index[entry] } else { entry };
            ekk.info.work_dual[col] -= self.row_basic_feasibility_change.array[col];
        }
        let (use_col_indices, to_entry) = sparse_loop_style(self.col_basic_feasibility_change.count, self.num_row);
        for entry in 0..to_entry {
            let row = if use_col_indices { self.col_basic_feasibility_change.index[entry] } else { entry };
            ekk.info.work_dual[self.num_col + row] -= self.col_basic_feasibility_change.array[row];
        }
        ekk.invalidate_dual_infeasibility_record();
    }

    fn basic_feasibility_change_price(&mut self, ekk: &mut Ekk) {
        let local_density = self.col_basic_feasibility_change.count as f64 / self.num_row as f64;
        let (use_col_price, use_row_price_w_switch) = Ekk::choose_price_technique(ekk.info.price_strategy, local_density);
        self.row_basic_feasibility_change.clear();
        if use_col_price {
            ekk.lp.a.price_by_column(false, &mut self.row_basic_feasibility_change, &self.col_basic_feasibility_change);
            for col in 0..self.num_col {
                self.row_basic_feasibility_change.array[col] *= ekk.basis.nonbasic_flag[col] as f64;
            }
        } else if use_row_price_w_switch {
            ekk.ar_matrix.price_by_row_with_switch(
                false,
                &mut self.row_basic_feasibility_change,
                &self.col_basic_feasibility_change,
                ekk.info.row_basic_feasibility_change_density,
                0,
                HYPER_PRICE_DENSITY,
            );
        } else {
            ekk.ar_matrix
                .price_by_row(false, &mut self.row_basic_feasibility_change, &self.col_basic_feasibility_change);
        }
        let local_density = self.row_basic_feasibility_change.count as f64 / self.num_col as f64;
        update_operation_result_density(local_density, &mut ekk.info.row_basic_feasibility_change_density);
    }

    fn initialise_devex_framework(&mut self, ekk: &Ekk) {
        self.edge_weight = vec![1.0; self.num_tot];
        self.devex_index = (0..self.num_tot)
            .map(|var| {
                let flag = ekk.basis.nonbasic_flag[var] as i32;
                flag * flag
            })
            .collect();
        self.num_bad_devex_weight = 0;
    }

    fn update_devex(&mut self, ekk: &Ekk) {
        let variable_in = self.variable_in.unwrap();
        let row_out = self.row_out as usize;
        let mut pivot_weight = 0.0;
        let (use_col_indices, to_entry) = sparse_loop_style(self.col_aq.count, self.num_row);
        for entry in 0..to_entry {
            let row = if use_col_indices { self.col_aq.index[entry] } else { entry };
            let col = ekk.basis.basic_index[row];
            let alpha = self.devex_index[col] as f64 * self.col_aq.array[row];
            pivot_weight += alpha * alpha;
        }
        pivot_weight += self.devex_index[variable_in] as f64;
        if self.edge_weight[variable_in] > BAD_DEVEX_WEIGHT_FACTOR * pivot_weight {
            self.num_bad_devex_weight += 1;
        }
        let pivot = self.col_aq.array[row_out];
        pivot_weight /= pivot * pivot;
        for &col in self.row_ap.indices() {
            let alpha = self.row_ap.array[col];
            let devex = pivot_weight * alpha * alpha + self.devex_index[col] as f64;
            if self.edge_weight[col] < devex {
                self.edge_weight[col] = devex;
            }
        }
        for &row in self.row_ep.indices() {
            let col = row + self.num_col;
            let alpha = self.row_ep.array[row];
            let devex = pivot_weight * alpha * alpha + self.devex_index[col] as f64;
            if self.edge_weight[col] < devex {
                self.edge_weight[col] = devex;
            }
        }
        self.edge_weight[self.variable_out] = pivot_weight.max(1.0);
        self.edge_weight[variable_in] = 1.0;
    }

    fn update_dual_steepest_edge_weights(&mut self, ekk: &mut Ekk) {
        let variable_in = self.variable_in.unwrap();
        let row_out = self.row_out as usize;
        self.col_steepest_edge.copy_from(&self.row_ep);
        ekk.nla.unapply_basis_matrix_row_scale(&mut self.col_steepest_edge);
        ekk.nla.ftran_in_scaled_space(&mut self.col_steepest_edge, ekk.info.row_dse_density);
        let local_density = self.col_steepest_edge.count as f64 / self.num_row as f64;
        update_operation_result_density(local_density, &mut ekk.info.row_dse_density);
        ekk.dual_edge_weight[row_out] = if ekk.simplex_in_scaled_space {
            self.row_ep.norm2()
        } else {
            ekk.nla.row_ep_2norm_in_scaled_space(row_out, &self.row_ep, &ekk.basis.basic_index)
        };
        let pivot = ekk.nla.pivot_in_scaled_space(&self.col_aq, variable_in, row_out, &ekk.basis.basic_index);
        let new_pivotal_edge_weight = ekk.dual_edge_weight[row_out] / (pivot * pivot);
        let kai = -2.0 / pivot;
        ekk.update_dual_steepest_edge_weights(row_out, variable_in, &self.col_aq, new_pivotal_edge_weight, kai, &self.col_steepest_edge.array);
        ekk.dual_edge_weight[row_out] = new_pivotal_edge_weight;
    }

    fn get_nonbasic_free_column_set(&mut self, ekk: &Ekk) {
        if self.num_free_col == 0 {
            return;
        }
        self.nonbasic_free_col_set.clear();
        for col in 0..self.num_tot {
            if ekk.basis.nonbasic_flag[col] == 1 && ekk.info.work_lower[col] == f64::NEG_INFINITY && ekk.info.work_upper[col] == f64::INFINITY {
                self.nonbasic_free_col_set.add(col);
            }
        }
    }

    fn remove_nonbasic_free_column(&mut self, ekk: &Ekk) {
        let variable_in = self.variable_in.unwrap();
        if ekk.basis.nonbasic_move[variable_in] == 0 {
            self.nonbasic_free_col_set.remove(variable_in);
        }
    }

    /// When the leaving variable is an equation whose bounds were
    /// perturbed, step to its true value.
    fn adjust_perturbed_equation_out(&mut self, ekk: &mut Ekk) {
        if !ekk.info.bounds_perturbed {
            return;
        }
        let variable_in = self.variable_in.unwrap();
        let (lp_lower, lp_upper) = if self.variable_out < self.num_col {
            (ekk.lp.col_lower[self.variable_out], ekk.lp.col_upper[self.variable_out])
        } else {
            let row = self.variable_out - self.num_col;
            (-ekk.lp.row_upper[row], -ekk.lp.row_lower[row])
        };
        if lp_lower < lp_upper {
            return;
        }
        let true_fixed_value = lp_lower;
        let row_out = self.row_out as usize;
        self.theta_primal = (ekk.info.base_value[row_out] - true_fixed_value) / self.alpha_col;
        ekk.info.work_lower[self.variable_out] = true_fixed_value;
        ekk.info.work_upper[self.variable_out] = true_fixed_value;
        ekk.info.work_range[self.variable_out] = 0.0;
        self.value_in = ekk.info.work_value[variable_in] + self.theta_primal;
    }

    fn get_basic_primal_infeasibility(&mut self, ekk: &mut Ekk) {
        let tolerance = self.primal_feasibility_tolerance;
        let info = &mut ekk.info;
        let mut num = 0;
        let mut max: f64 = 0.0;
        let mut sum = 0.0;
        for row in 0..self.num_row {
            let value = info.base_value[row];
            let infeasibility = if value < info.base_lower[row] - tolerance {
                info.base_lower[row] - value
            } else if value > info.base_upper[row] + tolerance {
                value - info.base_upper[row]
            } else {
                0.0
            };
            if infeasibility > 0.0 {
                if infeasibility > tolerance {
                    num += 1;
                }
                max = max.max(infeasibility);
                sum += infeasibility;
            }
        }
        info.num_primal_infeasibilities = num;
        info.max_primal_infeasibility = max;
        info.sum_primal_infeasibilities = sum;
    }
}

/// Move `bound` past `value` by a little more than the feasibility
/// tolerance, returning how far.
fn shift_bound(lower: bool, value: f64, random_value: f64, bound: &mut f64, tolerance: f64) -> f64 {
    let feasibility = (1.0 + random_value) * tolerance;
    if lower {
        let shift = (*bound - value) + feasibility;
        *bound -= shift;
        shift
    } else {
        let shift = (value - *bound) + feasibility;
        *bound += shift;
        shift
    }
}
