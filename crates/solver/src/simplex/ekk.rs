//! HiGHS's `HEkk`: the simplex LP, its basis and factor, the working
//! arrays the dual and primal solvers share, and the operations they both
//! call.

use std::collections::HashSet;

use web_time::Instant;

use super::{
    Algorithm, ModelStatus,
    nla::{Nla, Scale, sparse_loop_style},
    options::{EdgeWeightStrategy, Options, PriceStrategy, SimplexStrategy},
};
use crate::{
    factor::{DEFAULT_PIVOT_THRESHOLD, MAX_PIVOT_THRESHOLD, PIVOT_THRESHOLD_CHANGE_FACTOR},
    hvector::{HVector, TINY},
    matrix::{ColWise, HYPER_PRICE_DENSITY, RowPartitioned},
    util::{CDouble, Random, hash, nearest_power_of_two_scale},
};

pub(crate) const SOLVE_PHASE_ERROR: i32 = -3;
pub(crate) const SOLVE_PHASE_EXIT: i32 = -2;
pub(crate) const SOLVE_PHASE_UNKNOWN: i32 = -1;
pub(crate) const SOLVE_PHASE_OPTIMAL: i32 = 0;
pub(crate) const SOLVE_PHASE_1: i32 = 1;
pub(crate) const SOLVE_PHASE_2: i32 = 2;
pub(crate) const SOLVE_PHASE_PRIMAL_INFEASIBLE_CLEANUP: i32 = 3;
pub(crate) const SOLVE_PHASE_OPTIMAL_CLEANUP: i32 = 4;
pub(crate) const SOLVE_PHASE_TABOO_BASIS: i32 = 5;

pub(crate) const REBUILD_REASON_NO: i32 = 0;
pub(crate) const REBUILD_REASON_UPDATE_LIMIT_REACHED: i32 = 1;
pub(crate) const REBUILD_REASON_SYNTHETIC_CLOCK_SAYS_INVERT: i32 = 2;
pub(crate) const REBUILD_REASON_POSSIBLY_OPTIMAL: i32 = 3;
pub(crate) const REBUILD_REASON_POSSIBLY_PHASE1_FEASIBLE: i32 = 4;
pub(crate) const REBUILD_REASON_POSSIBLY_PRIMAL_UNBOUNDED: i32 = 5;
pub(crate) const REBUILD_REASON_POSSIBLY_DUAL_UNBOUNDED: i32 = 6;
pub(crate) const REBUILD_REASON_POSSIBLY_SINGULAR_BASIS: i32 = 7;
pub(crate) const REBUILD_REASON_PRIMAL_INFEASIBLE_IN_PRIMAL_SIMPLEX: i32 = 8;
pub(crate) const REBUILD_REASON_CHOOSE_COLUMN_FAIL: i32 = 9;
pub(crate) const REBUILD_REASON_EXCESSIVE_PRIMAL_VALUE: i32 = 11;

pub(crate) const RUNNING_AVERAGE_MULTIPLIER: f64 = 0.05;
pub(crate) const NUMERICAL_TROUBLE_TOLERANCE: f64 = 1e-7;
const SYNTHETIC_TICK_REINVERSION_MIN_UPDATE_COUNT: usize = 50;
pub(crate) const MIN_DUAL_STEEPEST_EDGE_WEIGHT: f64 = 1e-4;
pub(crate) const ACCEPT_DSE_WEIGHT_THRESHOLD: f64 = 0.25;
pub(crate) const EXCESSIVE_PRIMAL_VALUE: f64 = 1e25;

pub(crate) const NO_ROW_SOUGHT: isize = -2;
pub(crate) const NO_ROW_CHOSEN: isize = -1;

/// The LP the simplex solver works on: costs, bounds and the matrix,
/// minimised when `sense` is 1 and maximised when it is -1.
#[derive(Clone, Debug, Default)]
pub(crate) struct SimplexLp {
    pub num_col: usize,
    pub num_row: usize,
    pub sense: f64,
    pub offset: f64,
    pub col_cost: Vec<f64>,
    pub col_lower: Vec<f64>,
    pub col_upper: Vec<f64>,
    pub row_lower: Vec<f64>,
    pub row_upper: Vec<f64>,
    pub a: ColWise,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Basis {
    pub basic_index: Vec<usize>,
    /// 1 for nonbasic, 0 for basic.
    pub nonbasic_flag: Vec<i8>,
    /// Which way a nonbasic variable may move: 1 up (at its lower bound),
    /// -1 down (at its upper), 0 fixed or free.
    pub nonbasic_move: Vec<i8>,
    pub hash: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Info {
    pub work_cost: Vec<f64>,
    pub work_dual: Vec<f64>,
    pub work_shift: Vec<f64>,
    pub work_lower: Vec<f64>,
    pub work_upper: Vec<f64>,
    pub work_range: Vec<f64>,
    pub work_value: Vec<f64>,
    pub work_lower_shift: Vec<f64>,
    pub work_upper_shift: Vec<f64>,
    pub base_lower: Vec<f64>,
    pub base_upper: Vec<f64>,
    pub base_value: Vec<f64>,
    pub num_tot_random_value: Vec<f64>,
    pub num_tot_permutation: Vec<usize>,
    pub num_col_permutation: Vec<usize>,
    pub devex_index: Vec<i32>,

    pub backtracking: bool,
    pub valid_backtracking_basis: bool,
    pub backtracking_basis: Basis,
    pub backtracking_basis_costs_shifted: bool,
    pub backtracking_basis_costs_perturbed: bool,
    pub backtracking_basis_bounds_shifted: bool,
    pub backtracking_basis_bounds_perturbed: bool,
    pub backtracking_basis_work_shift: Vec<f64>,
    pub backtracking_basis_edge_weight: Vec<f64>,

    pub simplex_strategy_primal: bool,
    pub dual_edge_weight_strategy: EdgeWeightStrategy,
    pub price_strategy: PriceStrategy,
    pub dual_simplex_cost_perturbation_multiplier: f64,
    pub primal_simplex_phase1_cost_perturbation_multiplier: f64,
    pub primal_simplex_bound_perturbation_multiplier: f64,
    pub factor_pivot_threshold: f64,
    pub update_limit: usize,
    pub control_iteration_count0: usize,
    pub col_aq_density: f64,
    pub row_ep_density: f64,
    pub row_ap_density: f64,
    pub row_dse_density: f64,
    pub col_steepest_edge_density: f64,
    pub col_basic_feasibility_change_density: f64,
    pub row_basic_feasibility_change_density: f64,
    pub col_bfrt_density: f64,
    pub primal_col_density: f64,
    pub dual_col_density: f64,
    pub allow_dual_steepest_edge_to_devex_switch: bool,
    pub dual_steepest_edge_weight_log_error_threshold: f64,
    pub costly_dse_frequency: f64,
    pub num_costly_dse_iteration: usize,
    pub costly_dse_measure: f64,
    pub average_log_low_dse_weight_error: f64,
    pub average_log_high_dse_weight_error: f64,
    pub store_squared_primal_infeasibility: bool,
    pub allow_cost_shifting: bool,
    pub allow_cost_perturbation: bool,
    pub allow_bound_perturbation: bool,
    pub costs_shifted: bool,
    pub costs_perturbed: bool,
    pub bounds_shifted: bool,
    pub bounds_perturbed: bool,

    /// -1 when not known.
    pub num_primal_infeasibilities: isize,
    pub max_primal_infeasibility: f64,
    pub sum_primal_infeasibilities: f64,
    pub num_dual_infeasibilities: isize,
    pub max_dual_infeasibility: f64,
    pub sum_dual_infeasibilities: f64,

    pub update_count: usize,
    pub dual_objective_value: f64,
    pub primal_objective_value: f64,
    pub updated_dual_objective_value: f64,
    pub updated_primal_objective_value: f64,
    pub num_basic_logicals: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Status {
    pub has_basis: bool,
    pub has_ar_matrix: bool,
    pub has_nla: bool,
    pub has_dual_steepest_edge_weights: bool,
    pub has_invert: bool,
    pub has_fresh_invert: bool,
    pub has_fresh_rebuild: bool,
    pub has_dual_objective_value: bool,
    pub has_primal_objective_value: bool,
    pub initialised_for_new_lp: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BadBasisChangeReason {
    Singular,
    Cycling,
    FailedInfeasibilityProof,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BadBasisChange {
    pub taboo: bool,
    pub row_out: usize,
    pub variable_out: usize,
    pub variable_in: Option<usize>,
    pub reason: BadBasisChangeReason,
    pub save_value: f64,
}

/// Something that may ask the solve to stop.
pub type Interrupt = std::sync::Arc<dyn Fn() -> bool + Send + Sync>;

pub(crate) struct Ekk {
    pub options: Options,
    pub lp: SimplexLp,
    /// Whether `lp` is the scaled LP, so the simplex solver works in the
    /// scaled space.
    pub simplex_in_scaled_space: bool,
    /// The matrix the factor uses when `lp` is unscaled but has scaling.
    pub scaled_a: Option<ColWise>,
    pub nla: Nla,
    pub ar_matrix: RowPartitioned,
    pub basis: Basis,
    pub info: Info,
    pub status: Status,
    pub model_status: ModelStatus,
    pub random: Random,
    pub dual_edge_weight: Vec<f64>,
    pub scattered_dual_edge_weight: Vec<f64>,
    pub iteration_count: usize,
    pub build_synthetic_tick: f64,
    pub total_synthetic_tick: f64,
    pub bad_basis_change: Vec<BadBasisChange>,
    pub visited_basis: HashSet<u64>,
    pub previous_iteration_cycling_detected: i64,
    pub solve_bailout: bool,
    pub called_return_from_solve: bool,
    pub exit_algorithm: Algorithm,
    pub dual_simplex_cleanup_level: usize,
    pub dual_simplex_phase1_cleanup_level: usize,
    pub cost_perturbation_base: f64,
    pub cost_perturbation_max_abs_cost: f64,
    pub dual_ray_record: Option<(usize, i32)>,
    pub primal_ray_record: Option<(usize, i32)>,
    pub edge_weight_error: f64,
    pub num_dual_phase_1_lp_dual_infeasibility: isize,
    pub cost_scale: f64,
    pub started: Instant,
    pub interrupt: Option<Interrupt>,
}

impl Ekk {
    pub fn new(options: Options, lp: SimplexLp, simplex_in_scaled_space: bool) -> Self {
        let mut ekk = Self {
            options,
            lp,
            simplex_in_scaled_space,
            scaled_a: None,
            nla: Nla::default(),
            ar_matrix: RowPartitioned::default(),
            basis: Basis::default(),
            info: Info {
                primal_simplex_phase1_cost_perturbation_multiplier: 1.0,
                allow_cost_shifting: true,
                allow_cost_perturbation: true,
                allow_bound_perturbation: true,
                num_primal_infeasibilities: -1,
                num_dual_infeasibilities: -1,
                ..Info::default()
            },
            status: Status::default(),
            model_status: ModelStatus::NotSet,
            random: Random::new(0),
            dual_edge_weight: Vec::new(),
            scattered_dual_edge_weight: Vec::new(),
            iteration_count: 0,
            build_synthetic_tick: 0.0,
            total_synthetic_tick: 0.0,
            bad_basis_change: Vec::new(),
            visited_basis: HashSet::new(),
            previous_iteration_cycling_detected: i64::MIN / 2,
            solve_bailout: false,
            called_return_from_solve: false,
            exit_algorithm: Algorithm::None,
            dual_simplex_cleanup_level: 0,
            dual_simplex_phase1_cleanup_level: 0,
            cost_perturbation_base: 0.0,
            cost_perturbation_max_abs_cost: 0.0,
            dual_ray_record: None,
            primal_ray_record: None,
            edge_weight_error: 0.0,
            num_dual_phase_1_lp_dual_infeasibility: 0,
            cost_scale: 1.0,
            started: Instant::now(),
            interrupt: None,
        };
        ekk.initialise_ekk();
        ekk
    }

    /// What a new `Highs::run` resets in an `HEkk` it keeps: the iteration
    /// count, the model status and the run's clock.
    pub fn reset_for_run(&mut self) {
        self.iteration_count = 0;
        self.model_status = ModelStatus::NotSet;
        self.started = Instant::now();
        self.initialise_ekk();
    }

    fn num_tot(&self) -> usize {
        self.lp.num_col + self.lp.num_row
    }

    fn initialise_ekk(&mut self) {
        if self.status.initialised_for_new_lp {
            return;
        }
        self.set_simplex_options();
        self.initialise_control();
        self.initialise_simplex_lp_random_vectors();
        self.bad_basis_change.clear();
        self.status.initialised_for_new_lp = true;
    }

    fn set_simplex_options(&mut self) {
        self.info.dual_edge_weight_strategy = self.options.dual_edge_weight_strategy;
        self.info.price_strategy = self.options.price_strategy;
        self.info.dual_simplex_cost_perturbation_multiplier = self.options.dual_simplex_cost_perturbation_multiplier;
        self.info.primal_simplex_bound_perturbation_multiplier = self.options.primal_simplex_bound_perturbation_multiplier;
        self.info.factor_pivot_threshold = self.options.factor_pivot_threshold;
        self.info.update_limit = self.options.simplex_update_limit;
        self.random.initialise(self.options.random_seed);
        self.info.store_squared_primal_infeasibility = true;
    }

    pub fn initialise_control(&mut self) {
        let info = &mut self.info;
        info.allow_dual_steepest_edge_to_devex_switch = self.options.dual_edge_weight_strategy == EdgeWeightStrategy::Choose;
        info.dual_steepest_edge_weight_log_error_threshold = self.options.dual_steepest_edge_weight_log_error_threshold;
        info.control_iteration_count0 = self.iteration_count;
        info.col_aq_density = 0.0;
        info.row_ep_density = 0.0;
        info.row_ap_density = 0.0;
        info.row_dse_density = 0.0;
        info.col_steepest_edge_density = 0.0;
        info.col_basic_feasibility_change_density = 0.0;
        info.row_basic_feasibility_change_density = 0.0;
        info.col_bfrt_density = 0.0;
        info.primal_col_density = 0.0;
        info.dual_col_density = 1.0;
        info.costly_dse_frequency = 0.0;
        info.num_costly_dse_iteration = 0;
        info.costly_dse_measure = 0.0;
        info.average_log_low_dse_weight_error = 0.0;
        info.average_log_high_dse_weight_error = 0.0;
    }

    fn initialise_simplex_lp_random_vectors(&mut self) {
        let num_col = self.lp.num_col;
        let num_tot = self.num_tot();
        if num_tot == 0 {
            return;
        }
        if num_col > 0 {
            let mut permutation: Vec<usize> = (0..num_col).collect();
            self.random.shuffle(&mut permutation);
            self.info.num_col_permutation = permutation;
        }
        let mut permutation: Vec<usize> = (0..num_tot).collect();
        self.random.shuffle(&mut permutation);
        self.info.num_tot_permutation = permutation;
        self.info.num_tot_random_value = (0..num_tot).map(|_| self.random.fraction()).collect();
    }

    /// A logical basis.
    pub fn set_logical_basis(&mut self) {
        let num_col = self.lp.num_col;
        let num_row = self.lp.num_row;
        let num_tot = num_col + num_row;
        self.basis.basic_index = vec![0; num_row];
        self.basis.nonbasic_flag = vec![0; num_tot];
        self.basis.nonbasic_move = vec![0; num_tot];
        self.basis.hash = 0;
        for col in 0..num_col {
            self.basis.nonbasic_flag[col] = 1;
            let lower = self.lp.col_lower[col];
            let upper = self.lp.col_upper[col];
            self.basis.nonbasic_move[col] = initial_move(lower, upper);
        }
        for row in 0..num_row {
            let var = num_col + row;
            self.basis.nonbasic_flag[var] = 0;
            hash::sparse_combine(&mut self.basis.hash, var);
            self.basis.basic_index[row] = var;
        }
        self.info.num_basic_logicals = num_row;
        self.status.has_basis = true;
    }

    /// HiGHS's `HEkk::setBasis(HighsBasis)`: the basis from statuses.
    pub fn set_basis(&mut self, basis: &crate::Basis) {
        let num_col = self.lp.num_col;
        let num_row = self.lp.num_row;
        let num_tot = num_col + num_row;
        self.basis.basic_index = vec![0; num_row];
        self.basis.nonbasic_flag = vec![1; num_tot];
        self.basis.nonbasic_move = vec![0; num_tot];
        self.basis.hash = 0;
        let mut num_basic = 0;
        let mut num_basic_logicals = 0;
        for var in 0..num_tot {
            let (status, lower, upper, is_row) = if var < num_col {
                (basis.col_status[var], self.lp.col_lower[var], self.lp.col_upper[var], false)
            } else {
                let row = var - num_col;
                (basis.row_status[row], self.lp.row_lower[row], self.lp.row_upper[row], true)
            };
            if status == crate::BasisStatus::Basic {
                self.basis.nonbasic_flag[var] = 0;
                if num_basic < num_row {
                    self.basis.basic_index[num_basic] = var;
                }
                num_basic += 1;
                hash::sparse_combine(&mut self.basis.hash, var);
                if is_row {
                    num_basic_logicals += 1;
                }
                continue;
            }
            // A logical moves the opposite way to its row.
            self.basis.nonbasic_move[var] = match (lower == upper, status, is_row) {
                (true, _, _) => 0,
                (false, crate::BasisStatus::Lower, false) | (false, crate::BasisStatus::Upper, true) => 1,
                (false, crate::BasisStatus::Upper, false) | (false, crate::BasisStatus::Lower, true) => -1,
                _ => 0,
            };
        }
        self.info.num_basic_logicals = num_basic_logicals;
        self.status.has_basis = true;
    }

    /// HiGHS's `getHighsBasis`, after `get_solution`.
    pub fn get_basis(&self) -> crate::Basis {
        use crate::BasisStatus;
        let num_col = self.lp.num_col;
        let status = |var: usize, lower: f64, upper: f64, is_row: bool| {
            if self.basis.nonbasic_flag[var] == 0 {
                return BasisStatus::Basic;
            }
            match (self.basis.nonbasic_move[var], is_row) {
                (1, false) | (-1, true) => BasisStatus::Lower,
                (-1, false) | (1, true) => BasisStatus::Upper,
                _ if lower == upper => {
                    let dual = self.lp.sense * self.info.work_dual[var];
                    if dual >= 0.0 { BasisStatus::Lower } else { BasisStatus::Upper }
                }
                _ => BasisStatus::Zero,
            }
        };
        crate::Basis {
            col_status: (0..num_col).map(|col| status(col, self.lp.col_lower[col], self.lp.col_upper[col], false)).collect(),
            row_status: (0..self.lp.num_row)
                .map(|row| status(num_col + row, self.lp.row_lower[row], self.lp.row_upper[row], true))
                .collect(),
        }
    }

    fn initialise_simplex_lp_basis_and_factor(&mut self) {
        if !self.status.has_basis {
            self.set_logical_basis();
        }
        if !self.status.has_nla {
            self.nla
                .factor
                .setup(self.lp.num_col, self.lp.num_row, self.info.factor_pivot_threshold, self.options.factor_pivot_tolerance);
            self.nla.num_col = self.lp.num_col;
            self.nla.num_row = self.lp.num_row;
            self.status.has_nla = true;
        }
        if !self.status.has_invert {
            let rank_deficiency = self.compute_factor();
            if rank_deficiency > 0 {
                self.handle_rank_deficiency();
                // A new basis: the edge weights and row-wise matrix no
                // longer correspond.
                self.status.has_dual_steepest_edge_weights = false;
                self.status.has_fresh_rebuild = false;
                self.status.has_dual_objective_value = false;
                self.status.has_primal_objective_value = false;
                self.set_nonbasic_move();
                self.status.has_basis = true;
                self.status.has_invert = true;
                self.status.has_fresh_invert = true;
            }
            self.reset_synthetic_clock();
        }
    }

    fn handle_rank_deficiency(&mut self) {
        let factor = &self.nla.factor;
        let records: Vec<(usize, usize)> = (0..factor.rank_deficiency).map(|k| (factor.row_with_no_pivot[k], factor.var_with_no_pivot[k])).collect();
        for (row_in, variable_out) in records {
            let variable_in = self.lp.num_col + row_in;
            self.basis.nonbasic_flag[variable_in] = 0;
            self.basis.nonbasic_flag[variable_out] = 1;
            let row_out = row_in;
            self.add_bad_basis_change(row_out, variable_in, Some(variable_out), BadBasisChangeReason::Singular, true);
        }
        self.status.has_ar_matrix = false;
    }

    pub fn initialise_for_solve(&mut self) {
        self.initialise_simplex_lp_basis_and_factor();
        self.update_simplex_options();
        self.initialise_simplex_lp_random_vectors();
        self.initialise_partitioned_rowwise_matrix();
        self.allocate_work_and_base_arrays();
        self.initialise_cost(Algorithm::Primal, SOLVE_PHASE_UNKNOWN, false);
        self.initialise_bound(Algorithm::Primal, SOLVE_PHASE_UNKNOWN, false);
        self.initialise_nonbasic_value_and_move();
        self.compute_primal();
        self.compute_dual();
        self.compute_simplex_infeasible();
        self.compute_dual_objective_value(SOLVE_PHASE_2);
        self.compute_primal_objective_value();
        let primal_feasible = self.info.num_primal_infeasibilities == 0;
        let dual_feasible = self.info.num_dual_infeasibilities == 0;
        self.visited_basis.clear();
        self.visited_basis.insert(self.basis.hash);
        self.model_status = ModelStatus::NotSet;
        if primal_feasible && dual_feasible {
            self.model_status = ModelStatus::Optimal;
        }
    }

    fn update_simplex_options(&mut self) {
        self.info.dual_simplex_cost_perturbation_multiplier = self.options.dual_simplex_cost_perturbation_multiplier;
        self.info.primal_simplex_bound_perturbation_multiplier = self.options.primal_simplex_bound_perturbation_multiplier;
    }

    /// Solve with the simplex method `strategy` picks.
    pub fn solve(&mut self, force_phase2: bool, strategy: SimplexStrategy) {
        self.initialise_control();
        self.dual_simplex_cleanup_level = 0;
        self.dual_simplex_phase1_cleanup_level = 0;
        self.previous_iteration_cycling_detected = i64::MIN / 2;
        self.initialise_for_solve();
        if self.model_status == ModelStatus::Optimal {
            return;
        }
        self.dual_ray_record = None;
        self.primal_ray_record = None;
        self.info.allow_cost_shifting = true;
        self.info.allow_cost_perturbation = true;
        self.info.allow_bound_perturbation = true;
        let primal = match strategy {
            SimplexStrategy::Primal => true,
            SimplexStrategy::Dual => false,
            SimplexStrategy::Choose => self.info.num_primal_infeasibilities == 0,
        };
        self.info.simplex_strategy_primal = primal;
        if primal {
            super::primal::solve(self, force_phase2);
        } else {
            super::dual::solve(self, force_phase2);
            // The dual simplex method cannot tell unbounded from
            // infeasible; the primal can.
            if self.model_status == ModelStatus::UnboundedOrInfeasible {
                super::primal::solve(self, false);
            }
        }
    }

    pub fn compute_factor(&mut self) -> usize {
        if self.status.has_fresh_invert {
            return 0;
        }
        self.bad_basis_change.clear();
        let a = self.scaled_a.as_ref().unwrap_or(&self.lp.a);
        let factor_a = a.as_factor_matrix();
        let rank_deficiency = self.nla.factor.build(factor_a, &mut self.basis.basic_index);
        self.nla.build_synthetic_tick = self.nla.factor.build_synthetic_tick;
        if rank_deficiency > 0 {
            self.status.has_invert = false;
            self.status.has_fresh_invert = false;
        } else {
            self.status.has_invert = true;
            self.status.has_fresh_invert = true;
        }
        self.info.update_count = 0;
        rank_deficiency
    }

    pub fn get_nonsingular_inverse(&mut self) -> bool {
        let basic_index_before = self.basis.basic_index.clone();
        let simplex_update_count = self.info.update_count;
        for row in 0..self.lp.num_row {
            self.scattered_dual_edge_weight[self.basis.basic_index[row]] = self.dual_edge_weight[row];
        }
        let rank_deficiency = self.compute_factor();
        if rank_deficiency > 0 {
            let deficient_hash = self.basis.hash;
            if !self.get_backtracking_basis() {
                return false;
            }
            self.info.backtracking = true;
            self.visited_basis.clear();
            self.visited_basis.insert(self.basis.hash);
            self.visited_basis.insert(deficient_hash);
            self.status.has_ar_matrix = false;
            self.status.has_fresh_rebuild = false;
            self.status.has_dual_objective_value = false;
            self.status.has_primal_objective_value = false;
            let backtrack_rank_deficiency = self.compute_factor();
            if backtrack_rank_deficiency > 0 {
                return false;
            }
            if simplex_update_count <= 1 {
                return false;
            }
            self.info.update_limit = simplex_update_count / 2;
        } else {
            self.put_backtracking_basis_from(basic_index_before);
            self.info.backtracking = false;
            self.info.update_limit = self.options.simplex_update_limit;
        }
        for row in 0..self.lp.num_row {
            self.dual_edge_weight[row] = self.scattered_dual_edge_weight[self.basis.basic_index[row]];
        }
        true
    }

    fn get_backtracking_basis(&mut self) -> bool {
        if !self.info.valid_backtracking_basis {
            return false;
        }
        self.basis = self.info.backtracking_basis.clone();
        self.info.costs_shifted = self.info.backtracking_basis_costs_shifted;
        self.info.costs_perturbed = self.info.backtracking_basis_costs_perturbed;
        self.info.bounds_shifted = self.info.backtracking_basis_bounds_shifted;
        self.info.bounds_perturbed = self.info.backtracking_basis_bounds_perturbed;
        self.info.work_shift.clone_from(&self.info.backtracking_basis_work_shift);
        let num_tot = self.num_tot();
        self.scattered_dual_edge_weight[..num_tot].copy_from_slice(&self.info.backtracking_basis_edge_weight[..num_tot]);
        true
    }

    pub fn put_backtracking_basis(&mut self) {
        for row in 0..self.lp.num_row {
            self.scattered_dual_edge_weight[self.basis.basic_index[row]] = self.dual_edge_weight[row];
        }
        let basic_index = self.basis.basic_index.clone();
        self.put_backtracking_basis_from(basic_index);
    }

    fn put_backtracking_basis_from(&mut self, basic_index_before: Vec<usize>) {
        self.info.valid_backtracking_basis = true;
        self.info.backtracking_basis = self.basis.clone();
        self.info.backtracking_basis.basic_index = basic_index_before;
        self.info.backtracking_basis_costs_shifted = self.info.costs_shifted;
        self.info.backtracking_basis_costs_perturbed = self.info.costs_perturbed;
        self.info.backtracking_basis_bounds_shifted = self.info.bounds_shifted;
        self.info.backtracking_basis_bounds_perturbed = self.info.bounds_perturbed;
        self.info.backtracking_basis_work_shift.clone_from(&self.info.work_shift);
        let num_tot = self.num_tot();
        self.info.backtracking_basis_edge_weight.resize(num_tot, 0.0);
        self.info.backtracking_basis_edge_weight[..num_tot].copy_from_slice(&self.scattered_dual_edge_weight[..num_tot]);
    }

    pub fn compute_primal_objective_value(&mut self) {
        let mut value = 0.0;
        for row in 0..self.lp.num_row {
            let var = self.basis.basic_index[row];
            if var < self.lp.num_col {
                value += self.info.base_value[row] * self.lp.col_cost[var];
            }
        }
        for col in 0..self.lp.num_col {
            if self.basis.nonbasic_flag[col] != 0 {
                value += self.info.work_value[col] * self.lp.col_cost[col];
            }
        }
        value *= self.cost_scale;
        value += self.lp.offset;
        self.info.primal_objective_value = value;
        self.status.has_primal_objective_value = true;
    }

    pub fn compute_dual_objective_value(&mut self, phase: i32) {
        let mut value = 0.0;
        for var in 0..self.num_tot() {
            if self.basis.nonbasic_flag[var] != 0 {
                value += self.info.work_value[var] * self.info.work_dual[var];
            }
        }
        value *= self.cost_scale;
        if phase != SOLVE_PHASE_1 {
            value += self.lp.sense * self.lp.offset;
        }
        self.info.dual_objective_value = value;
        self.status.has_dual_objective_value = true;
    }

    pub fn rebuild_refactor(&mut self, rebuild_reason: i32) -> bool {
        if self.info.update_count == 0 {
            return false;
        }
        let mut refactor = true;
        if self.options.no_unnecessary_rebuild_refactor
            && matches!(
                rebuild_reason,
                REBUILD_REASON_NO
                    | REBUILD_REASON_POSSIBLY_OPTIMAL
                    | REBUILD_REASON_POSSIBLY_PHASE1_FEASIBLE
                    | REBUILD_REASON_POSSIBLY_PRIMAL_UNBOUNDED
                    | REBUILD_REASON_POSSIBLY_DUAL_UNBOUNDED
                    | REBUILD_REASON_PRIMAL_INFEASIBLE_IN_PRIMAL_SIMPLEX
            )
        {
            refactor = false;
            let error_tolerance = self.options.rebuild_refactor_solution_error_tolerance;
            if error_tolerance > 0.0 {
                let solution_error = self.factor_solve_error();
                refactor = solution_error > error_tolerance;
            }
        }
        refactor
    }

    /// A cheap test of the factor's accuracy: solve for a random solution
    /// with up to 50 nonzeros and see how far off it comes back.
    fn factor_solve_error(&mut self) -> f64 {
        let num_col = self.lp.num_col;
        let num_row = self.lp.num_row;
        let mut btran_rhs = HVector::new(num_row);
        let mut ftran_rhs = HVector::new(num_row);
        let mut random = Random::new(1);
        let solution_num_nz = 50.min(num_row.div_ceil(2));
        let mut solution_value = Vec::new();
        let mut solution_index = Vec::new();
        let mut solution_nonzero = vec![false; num_row];
        loop {
            let row = random.integer(num_row);
            if solution_nonzero[row] {
                continue;
            }
            let value = random.fraction();
            solution_value.push(value);
            solution_index.push(row);
            solution_nonzero[row] = true;
            let col = self.basis.basic_index[row];
            self.lp.a.collect_aj(&mut ftran_rhs, col, value);
            if solution_value.len() == solution_num_nz {
                break;
            }
        }
        let mut btran_scattered_rhs = vec![0.0; num_col + num_row];
        for (k, &row) in solution_index.iter().enumerate() {
            for el in self.ar_matrix.p_end[row]..self.ar_matrix.start[row + 1] {
                let col = self.ar_matrix.index[el];
                btran_scattered_rhs[col] += self.ar_matrix.value[el] * solution_value[k];
            }
            let col = num_col + row;
            if self.basis.nonbasic_flag[col] == 0 {
                btran_scattered_rhs[col] = solution_value[k];
            }
        }
        for row in 0..num_row {
            let col = self.basis.basic_index[row];
            if btran_scattered_rhs[col] == 0.0 {
                continue;
            }
            btran_rhs.array[row] = btran_scattered_rhs[col];
            btran_rhs.index[btran_rhs.count as usize] = row;
            btran_rhs.count += 1;
        }
        let expected_density = solution_num_nz as f64 * self.info.col_aq_density;
        self.nla.ftran(&mut ftran_rhs, expected_density, &self.basis.basic_index);
        self.nla.btran(&mut btran_rhs, expected_density, &self.basis.basic_index);
        let mut error: f64 = 0.0;
        for (k, &row) in solution_index.iter().enumerate() {
            error = error.max((ftran_rhs.array[row] - solution_value[k]).abs());
            error = error.max((btran_rhs.array[row] - solution_value[k]).abs());
        }
        error
    }

    pub fn compute_dual_steepest_edge_weights(&mut self) {
        let num_row = self.lp.num_row;
        let mut row_ep = HVector::new(num_row);
        for row in 0..num_row {
            self.dual_edge_weight[row] = self.compute_dual_steepest_edge_weight(row, &mut row_ep);
        }
    }

    fn compute_dual_steepest_edge_weight(&mut self, row: usize, row_ep: &mut HVector) -> f64 {
        row_ep.clear();
        row_ep.count = 1;
        row_ep.index[0] = row;
        row_ep.array[row] = 1.0;
        row_ep.pack_flag = false;
        self.nla.btran_in_scaled_space(row_ep, self.info.row_ep_density);
        let local_density = row_ep.count as f64 / self.lp.num_row as f64;
        update_operation_result_density(local_density, &mut self.info.row_ep_density);
        row_ep.norm2()
    }

    pub fn update_dual_steepest_edge_weights(
        &mut self,
        row_out: usize,
        variable_in: usize,
        column: &HVector,
        new_pivotal_edge_weight: f64,
        kai: f64,
        dual_steepest_edge_array: &[f64],
    ) {
        let num_row = self.lp.num_row;
        let col_aq_scale = self.nla.variable_scale_factor(variable_in);
        let col_ap_scale = self.nla.basic_col_scale_factor(row_out, &self.basis.basic_index);
        let inv_col_ap_scale = 1.0 / col_ap_scale;
        let (use_row_indices, to_entry) = sparse_loop_style(column.count, num_row);
        let convert_to_scaled_space = !self.simplex_in_scaled_space;
        for entry in 0..to_entry {
            let row = if use_row_indices { column.index[entry] } else { entry };
            let mut aa = column.array[row];
            if aa == 0.0 {
                continue;
            }
            let mut dse_value = dual_steepest_edge_array[row];
            if convert_to_scaled_space {
                let basic_col_scale = self.nla.basic_col_scale_factor(row, &self.basis.basic_index);
                aa /= basic_col_scale;
                aa *= col_aq_scale;
                dse_value *= inv_col_ap_scale;
            }
            let weight = &mut self.dual_edge_weight[row];
            *weight += aa * (new_pivotal_edge_weight * aa + kai * dse_value);
            *weight = weight.max(MIN_DUAL_STEEPEST_EDGE_WEIGHT);
        }
    }

    pub fn update_dual_devex_weights(&mut self, column: &HVector, new_pivotal_edge_weight: f64) {
        let (use_row_indices, to_entry) = sparse_loop_style(column.count, self.lp.num_row);
        for entry in 0..to_entry {
            let row = if use_row_indices { column.index[entry] } else { entry };
            let aa = column.array[row];
            self.dual_edge_weight[row] = self.dual_edge_weight[row].max(new_pivotal_edge_weight * aa * aa);
        }
    }

    pub fn reset_synthetic_clock(&mut self) {
        self.build_synthetic_tick = self.nla.build_synthetic_tick;
        self.total_synthetic_tick = 0.0;
    }

    pub fn initialise_partitioned_rowwise_matrix(&mut self) {
        if self.status.has_ar_matrix {
            return;
        }
        self.ar_matrix = RowPartitioned::create(&self.lp.a, &self.basis.nonbasic_flag);
        self.status.has_ar_matrix = true;
    }

    pub fn zero_basic_duals(&mut self) {
        for row in 0..self.lp.num_row {
            self.info.work_dual[self.basis.basic_index[row]] = 0.0;
        }
    }

    fn set_nonbasic_move(&mut self) {
        let num_col = self.lp.num_col;
        for var in 0..self.num_tot() {
            if self.basis.nonbasic_flag[var] == 0 {
                self.basis.nonbasic_move[var] = 0;
                continue;
            }
            let (lower, upper) = if var < num_col {
                (self.lp.col_lower[var], self.lp.col_upper[var])
            } else {
                let row = var - num_col;
                (-self.lp.row_upper[row], -self.lp.row_lower[row])
            };
            self.basis.nonbasic_move[var] = initial_move(lower, upper);
        }
    }

    fn allocate_work_and_base_arrays(&mut self) {
        let num_tot = self.num_tot();
        let num_row = self.lp.num_row;
        let info = &mut self.info;
        for array in [
            &mut info.work_cost,
            &mut info.work_dual,
            &mut info.work_shift,
            &mut info.work_lower,
            &mut info.work_upper,
            &mut info.work_range,
            &mut info.work_value,
            &mut info.work_lower_shift,
            &mut info.work_upper_shift,
        ] {
            array.resize(num_tot, 0.0);
        }
        info.devex_index.resize(num_tot, 0);
        info.base_lower.resize(num_row, 0.0);
        info.base_upper.resize(num_row, 0.0);
        info.base_value.resize(num_row, 0.0);
    }

    fn initialise_lp_col_bound(&mut self) {
        for col in 0..self.lp.num_col {
            self.info.work_lower[col] = self.lp.col_lower[col];
            self.info.work_upper[col] = self.lp.col_upper[col];
            self.info.work_range[col] = self.info.work_upper[col] - self.info.work_lower[col];
            self.info.work_lower_shift[col] = 0.0;
            self.info.work_upper_shift[col] = 0.0;
        }
    }

    fn initialise_lp_row_bound(&mut self) {
        for row in 0..self.lp.num_row {
            let var = self.lp.num_col + row;
            self.info.work_lower[var] = -self.lp.row_upper[row];
            self.info.work_upper[var] = -self.lp.row_lower[row];
            self.info.work_range[var] = self.info.work_upper[var] - self.info.work_lower[var];
            self.info.work_lower_shift[var] = 0.0;
            self.info.work_upper_shift[var] = 0.0;
        }
    }

    pub fn initialise_cost(&mut self, algorithm: Algorithm, _solve_phase: i32, perturb: bool) {
        for col in 0..self.lp.num_col {
            self.info.work_cost[col] = self.lp.sense * self.lp.col_cost[col];
            self.info.work_shift[col] = 0.0;
        }
        for var in self.lp.num_col..self.num_tot() {
            self.info.work_cost[var] = 0.0;
            self.info.work_shift[var] = 0.0;
        }
        self.info.costs_shifted = false;
        self.info.costs_perturbed = false;
        if algorithm == Algorithm::Primal {
            return;
        }
        if !perturb || self.info.dual_simplex_cost_perturbation_multiplier == 0.0 {
            return;
        }
        let num_col = self.lp.num_col;
        let mut max_abs_cost: f64 = 0.0;
        for col in 0..num_col {
            max_abs_cost = max_abs_cost.max(self.info.work_cost[col].abs());
        }
        if max_abs_cost > 100.0 {
            max_abs_cost = max_abs_cost.sqrt().sqrt();
        }
        let num_tot = self.num_tot();
        let mut boxed_rate = 0.0;
        for var in 0..num_tot {
            if self.info.work_range[var] < 1e30 {
                boxed_rate += 1.0;
            }
        }
        boxed_rate /= num_tot as f64;
        if boxed_rate < 0.01 {
            max_abs_cost = max_abs_cost.min(1.0);
        }
        self.cost_perturbation_max_abs_cost = max_abs_cost;
        self.cost_perturbation_base = self.info.dual_simplex_cost_perturbation_multiplier * 5e-7 * max_abs_cost;
        for col in 0..num_col {
            let lower = self.lp.col_lower[col];
            let upper = self.lp.col_upper[col];
            let xpert = (1.0 + self.info.num_tot_random_value[col]) * (self.info.work_cost[col].abs() + 1.0) * self.cost_perturbation_base;
            if lower == f64::NEG_INFINITY && upper == f64::INFINITY {
                // Free: not perturbed.
            } else if upper == f64::INFINITY {
                self.info.work_cost[col] += xpert;
            } else if lower == f64::NEG_INFINITY {
                self.info.work_cost[col] -= xpert;
            } else if lower != upper {
                self.info.work_cost[col] += if self.info.work_cost[col] >= 0.0 { xpert } else { -xpert };
            }
        }
        let row_base = self.info.dual_simplex_cost_perturbation_multiplier * 1e-12;
        for var in num_col..num_tot {
            self.info.work_cost[var] += (0.5 - self.info.num_tot_random_value[var]) * row_base;
        }
        self.info.costs_perturbed = true;
    }

    pub fn initialise_bound(&mut self, algorithm: Algorithm, solve_phase: i32, perturb: bool) {
        self.initialise_lp_col_bound();
        self.initialise_lp_row_bound();
        self.info.bounds_shifted = false;
        self.info.bounds_perturbed = false;
        let num_tot = self.num_tot();
        if algorithm == Algorithm::Primal {
            if !perturb || self.info.primal_simplex_bound_perturbation_multiplier == 0.0 {
                return;
            }
            let base = self.info.primal_simplex_bound_perturbation_multiplier * 5e-7;
            for var in 0..num_tot {
                let mut lower = self.info.work_lower[var];
                let mut upper = self.info.work_upper[var];
                let fixed = lower == upper;
                if self.basis.nonbasic_flag[var] == 1 && fixed {
                    continue;
                }
                let random_value = self.info.num_tot_random_value[var];
                if lower > f64::NEG_INFINITY {
                    if lower < -1.0 {
                        lower -= random_value * base * (-lower);
                    } else if lower < 1.0 {
                        lower -= random_value * base;
                    } else {
                        lower -= random_value * base * lower;
                    }
                    self.info.work_lower[var] = lower;
                }
                if upper < f64::INFINITY {
                    if upper < -1.0 {
                        upper += random_value * base * (-upper);
                    } else if upper < 1.0 {
                        upper += random_value * base;
                    } else {
                        upper += random_value * base * upper;
                    }
                    self.info.work_upper[var] = upper;
                }
                self.info.work_range[var] = self.info.work_upper[var] - self.info.work_lower[var];
                if self.basis.nonbasic_flag[var] == 0 {
                    continue;
                }
                if self.basis.nonbasic_move[var] > 0 {
                    self.info.work_value[var] = lower;
                } else if self.basis.nonbasic_move[var] < 0 {
                    self.info.work_value[var] = upper;
                }
            }
            for row in 0..self.lp.num_row {
                let var = self.basis.basic_index[row];
                self.info.base_lower[row] = self.info.work_lower[var];
                self.info.base_upper[row] = self.info.work_upper[var];
            }
            self.info.bounds_perturbed = true;
            return;
        }
        if solve_phase == SOLVE_PHASE_2 {
            return;
        }
        // Dual phase 1 bounds: zero where the dual is feasible, else a
        // unit step its way; [-1000, 1000] for free variables.
        for var in 0..num_tot {
            let (lower, upper) = (self.info.work_lower[var], self.info.work_upper[var]);
            let (new_lower, new_upper) = if lower == f64::NEG_INFINITY && upper == f64::INFINITY {
                (-1000.0, 1000.0)
            } else if lower == f64::NEG_INFINITY {
                (-1.0, 0.0)
            } else if upper == f64::INFINITY {
                (0.0, 1.0)
            } else {
                (0.0, 0.0)
            };
            self.info.work_lower[var] = new_lower;
            self.info.work_upper[var] = new_upper;
            self.info.work_range[var] = new_upper - new_lower;
        }
    }

    pub fn initialise_nonbasic_value_and_move(&mut self) {
        for var in 0..self.num_tot() {
            if self.basis.nonbasic_flag[var] == 0 {
                self.basis.nonbasic_move[var] = 0;
                continue;
            }
            let lower = self.info.work_lower[var];
            let upper = self.info.work_upper[var];
            let original_move = self.basis.nonbasic_move[var];
            let (value, mv) = if lower == upper {
                (lower, 0)
            } else if lower != f64::NEG_INFINITY {
                if upper != f64::INFINITY {
                    if original_move == -1 { (upper, -1) } else { (lower, 1) }
                } else {
                    (lower, 1)
                }
            } else if upper != f64::INFINITY {
                (upper, -1)
            } else {
                (0.0, 0)
            };
            self.basis.nonbasic_move[var] = mv;
            self.info.work_value[var] = value;
        }
    }

    pub fn pivot_column_ftran(&mut self, col: usize, col_aq: &mut HVector) {
        col_aq.clear();
        col_aq.pack_flag = true;
        self.lp.a.collect_aj(col_aq, col, 1.0);
        self.nla.ftran(col_aq, self.info.col_aq_density, &self.basis.basic_index);
        let local_density = col_aq.count as f64 / self.lp.num_row as f64;
        update_operation_result_density(local_density, &mut self.info.col_aq_density);
    }

    pub fn unit_btran(&mut self, row: usize, row_ep: &mut HVector) {
        row_ep.clear();
        row_ep.count = 1;
        row_ep.index[0] = row;
        row_ep.array[row] = 1.0;
        row_ep.pack_flag = true;
        self.nla.btran(row_ep, self.info.row_ep_density, &self.basis.basic_index);
        let local_density = row_ep.count as f64 / self.lp.num_row as f64;
        update_operation_result_density(local_density, &mut self.info.row_ep_density);
    }

    pub fn full_btran(&mut self, buffer: &mut HVector) {
        self.nla.btran(buffer, self.info.dual_col_density, &self.basis.basic_index);
        let local_density = buffer.count as f64 / self.lp.num_row as f64;
        update_operation_result_density(local_density, &mut self.info.dual_col_density);
    }

    pub fn choose_price_technique(price_strategy: PriceStrategy, row_ep_density: f64) -> (bool, bool) {
        let density_for_column_price_switch = 0.75;
        let use_col_price = price_strategy == PriceStrategy::Col || (price_strategy == PriceStrategy::RowSwitchColSwitch && row_ep_density > density_for_column_price_switch);
        let use_row_price_w_switch = matches!(price_strategy, PriceStrategy::RowSwitch | PriceStrategy::RowSwitchColSwitch);
        (use_col_price, use_row_price_w_switch)
    }

    pub fn tableau_row_price(&mut self, quad_precision: bool, row_ep: &HVector, row_ap: &mut HVector) {
        let local_density = row_ep.count as f64 / self.lp.num_row as f64;
        let (use_col_price, use_row_price_w_switch) = Self::choose_price_technique(self.info.price_strategy, local_density);
        row_ap.clear();
        if use_col_price {
            self.lp.a.price_by_column(quad_precision, row_ap, row_ep);
        } else if use_row_price_w_switch {
            self.ar_matrix
                .price_by_row_with_switch(quad_precision, row_ap, row_ep, self.info.row_ap_density, 0, HYPER_PRICE_DENSITY);
        } else {
            self.ar_matrix.price_by_row(quad_precision, row_ap, row_ep);
        }
        if use_col_price {
            for col in 0..self.lp.num_col {
                row_ap.array[col] *= self.basis.nonbasic_flag[col] as f64;
            }
        }
        let local_density = row_ap.count as f64 / self.lp.num_col as f64;
        update_operation_result_density(local_density, &mut self.info.row_ap_density);
    }

    pub fn full_price(&self, full_col: &HVector, full_row: &mut HVector) {
        full_row.clear();
        self.lp.a.price_by_column(false, full_row, full_col);
    }

    pub fn compute_primal(&mut self) {
        let num_row = self.lp.num_row;
        let num_col = self.lp.num_col;
        let mut primal_col = HVector::new(num_row);
        for var in 0..num_col + num_row {
            if self.basis.nonbasic_flag[var] != 0 && self.info.work_value[var] != 0.0 {
                self.lp.a.collect_aj(&mut primal_col, var, self.info.work_value[var]);
            }
        }
        if primal_col.count > 0 {
            self.nla.ftran(&mut primal_col, self.info.primal_col_density, &self.basis.basic_index);
            let local_density = primal_col.count as f64 / num_row as f64;
            update_operation_result_density(local_density, &mut self.info.primal_col_density);
        }
        for row in 0..num_row {
            let var = self.basis.basic_index[row];
            self.info.base_value[row] = -primal_col.array[row];
            self.info.base_lower[row] = self.info.work_lower[var];
            self.info.base_upper[row] = self.info.work_upper[var];
        }
        self.info.num_primal_infeasibilities = -1;
    }

    pub fn compute_dual(&mut self) {
        let num_row = self.lp.num_row;
        let num_col = self.lp.num_col;
        let num_tot = num_col + num_row;
        let mut dual_col = HVector::new(num_row);
        for row in 0..num_row {
            let var = self.basis.basic_index[row];
            let value = self.info.work_cost[var] + self.info.work_shift[var];
            if value != 0.0 {
                dual_col.index[dual_col.count as usize] = row;
                dual_col.count += 1;
                dual_col.array[row] = value;
            }
        }
        for var in 0..num_tot {
            self.info.work_dual[var] = self.info.work_cost[var] + self.info.work_shift[var];
        }
        if dual_col.count > 0 {
            self.full_btran(&mut dual_col);
            let mut dual_row = HVector::new(num_col);
            self.full_price(&dual_col, &mut dual_row);
            for col in 0..num_col {
                self.info.work_dual[col] -= dual_row.array[col];
            }
            for var in num_col..num_tot {
                self.info.work_dual[var] -= dual_col.array[var - num_col];
            }
        }
        self.info.num_dual_infeasibilities = -1;
    }

    pub fn compute_dual_for_tableau_column(&self, var: usize, tableau_column: &HVector) -> f64 {
        let mut dual = self.info.work_cost[var];
        for &row in tableau_column.indices() {
            dual -= tableau_column.array[row] * self.info.work_cost[self.basis.basic_index[row]];
        }
        dual
    }

    pub fn reinvert_on_numerical_trouble(&mut self, alpha_from_col: f64, alpha_from_row: f64, numerical_trouble_tolerance: f64) -> (bool, f64) {
        let abs_col = alpha_from_col.abs();
        let abs_row = alpha_from_row.abs();
        let min_abs_alpha = abs_col.min(abs_row);
        let measure = (abs_col - abs_row).abs() / min_abs_alpha;
        let update_count = self.info.update_count;
        let reinvert = measure > numerical_trouble_tolerance && update_count > 0;
        if reinvert {
            let current = self.info.factor_pivot_threshold;
            let mut new_threshold = 0.0;
            if current < DEFAULT_PIVOT_THRESHOLD {
                new_threshold = (current * PIVOT_THRESHOLD_CHANGE_FACTOR).min(DEFAULT_PIVOT_THRESHOLD);
            } else if current < MAX_PIVOT_THRESHOLD && update_count < 10 {
                new_threshold = (current * PIVOT_THRESHOLD_CHANGE_FACTOR).min(MAX_PIVOT_THRESHOLD);
            }
            if new_threshold != 0.0 {
                self.info.factor_pivot_threshold = new_threshold;
                self.nla.factor.set_pivot_threshold(new_threshold);
            }
        }
        (reinvert, measure)
    }

    pub fn transform_for_update(&self, column: &mut HVector, row_ep: &mut HVector, variable_in: usize, row_out: usize) {
        self.nla.transform_for_update(column, row_ep, variable_in, row_out, &self.basis.basic_index);
    }

    pub fn flip_bound(&mut self, col: usize) {
        let mv = -self.basis.nonbasic_move[col];
        self.basis.nonbasic_move[col] = mv;
        self.info.work_value[col] = if mv == 1 { self.info.work_lower[col] } else { self.info.work_upper[col] };
    }

    /// Update the factor; returns a rebuild reason if one is now due.
    pub fn update_factor(&mut self, column: &HVector, row_ep: &HVector, row_out: usize, hint: &mut i32) {
        self.nla.factor.update(column, row_ep, row_out);
        self.status.has_invert = true;
        if self.info.update_count >= self.info.update_limit {
            *hint = REBUILD_REASON_UPDATE_LIMIT_REACHED;
        }
        let reinvert_synthetic_clock = self.total_synthetic_tick >= self.build_synthetic_tick;
        let performed_min_updates = self.info.update_count >= SYNTHETIC_TICK_REINVERSION_MIN_UPDATE_COUNT;
        if reinvert_synthetic_clock && performed_min_updates {
            *hint = REBUILD_REASON_SYNTHETIC_CLOCK_SAYS_INVERT;
        }
    }

    pub fn update_pivots(&mut self, variable_in: usize, row_out: usize, move_out: i32) {
        let variable_out = self.basis.basic_index[row_out];
        hash::sparse_inverse_combine(&mut self.basis.hash, variable_out);
        hash::sparse_combine(&mut self.basis.hash, variable_in);
        self.visited_basis.insert(self.basis.hash);

        self.basis.basic_index[row_out] = variable_in;
        self.basis.nonbasic_flag[variable_in] = 0;
        self.basis.nonbasic_move[variable_in] = 0;
        self.info.base_lower[row_out] = self.info.work_lower[variable_in];
        self.info.base_upper[row_out] = self.info.work_upper[variable_in];

        self.basis.nonbasic_flag[variable_out] = 1;
        if self.info.work_lower[variable_out] == self.info.work_upper[variable_out] {
            self.info.work_value[variable_out] = self.info.work_lower[variable_out];
            self.basis.nonbasic_move[variable_out] = 0;
        } else if move_out == -1 {
            self.info.work_value[variable_out] = self.info.work_lower[variable_out];
            self.basis.nonbasic_move[variable_out] = 1;
        } else {
            self.info.work_value[variable_out] = self.info.work_upper[variable_out];
            self.basis.nonbasic_move[variable_out] = -1;
        }
        let new_value = self.info.work_value[variable_out];
        let dual = self.info.work_dual[variable_out];
        self.info.updated_dual_objective_value += new_value * dual;
        self.info.update_count += 1;
        if variable_out < self.lp.num_col {
            self.info.num_basic_logicals += 1;
        }
        if variable_in < self.lp.num_col {
            self.info.num_basic_logicals -= 1;
        }
        self.status.has_invert = false;
        self.status.has_fresh_invert = false;
        self.status.has_fresh_rebuild = false;
    }

    pub fn is_bad_basis_change(&mut self, variable_in: Option<usize>, row_out: isize, rebuild_reason: i32) -> bool {
        if rebuild_reason != 0 {
            return false;
        }
        let (Some(variable_in), true) = (variable_in, row_out >= 0) else {
            return false;
        };
        let row_out = row_out as usize;
        let mut currhash = self.basis.hash;
        let variable_out = self.basis.basic_index[row_out];
        hash::sparse_inverse_combine(&mut currhash, variable_out);
        hash::sparse_combine(&mut currhash, variable_in);
        let mut cycling_detected = false;
        if self.visited_basis.contains(&currhash) {
            if self.iteration_count as i64 == self.previous_iteration_cycling_detected + 1 {
                cycling_detected = true;
            } else {
                self.previous_iteration_cycling_detected = self.iteration_count as i64;
            }
        }
        if cycling_detected {
            self.add_bad_basis_change(row_out, variable_out, Some(variable_in), BadBasisChangeReason::Cycling, true);
            return true;
        }
        for change in &mut self.bad_basis_change {
            if change.variable_out == variable_out && change.variable_in == Some(variable_in) && change.row_out == row_out {
                change.taboo = true;
                return true;
            }
        }
        false
    }

    pub fn update_matrix(&mut self, variable_in: usize, variable_out: usize) {
        self.ar_matrix.update(variable_in, variable_out, &self.lp.a);
    }

    pub fn compute_simplex_infeasible(&mut self) {
        self.compute_simplex_primal_infeasible();
        self.compute_simplex_dual_infeasible();
    }

    pub fn compute_simplex_primal_infeasible(&mut self) {
        let tolerance = self.options.primal_feasibility_tolerance;
        let mut num = 0;
        let mut max: f64 = 0.0;
        let mut sum = 0.0;
        let mut assess = |value: f64, lower: f64, upper: f64| {
            let infeasibility = if value < lower - tolerance {
                lower - value
            } else if value > upper + tolerance {
                value - upper
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
        };
        for var in 0..self.lp.num_col + self.lp.num_row {
            if self.basis.nonbasic_flag[var] != 0 {
                assess(self.info.work_value[var], self.info.work_lower[var], self.info.work_upper[var]);
            }
        }
        for row in 0..self.lp.num_row {
            assess(self.info.base_value[row], self.info.base_lower[row], self.info.base_upper[row]);
        }
        self.info.num_primal_infeasibilities = num;
        self.info.max_primal_infeasibility = max;
        self.info.sum_primal_infeasibilities = sum;
    }

    pub fn compute_simplex_dual_infeasible(&mut self) {
        let tolerance = self.options.dual_feasibility_tolerance;
        let mut num = 0;
        let mut max: f64 = 0.0;
        let mut sum = 0.0;
        for var in 0..self.num_tot() {
            if self.basis.nonbasic_flag[var] == 0 {
                continue;
            }
            let dual = self.info.work_dual[var];
            let lower = self.info.work_lower[var];
            let upper = self.info.work_upper[var];
            let infeasibility = if lower == f64::NEG_INFINITY && upper == f64::INFINITY {
                dual.abs()
            } else {
                -(self.basis.nonbasic_move[var] as f64) * dual
            };
            if infeasibility > 0.0 {
                if infeasibility >= tolerance {
                    num += 1;
                }
                max = max.max(infeasibility);
                sum += infeasibility;
            }
        }
        self.info.num_dual_infeasibilities = num;
        self.info.max_dual_infeasibility = max;
        self.info.sum_dual_infeasibilities = sum;
    }

    /// Dual infeasibilities by the LP's own bounds, for dual phase 1.
    pub fn compute_simplex_lp_dual_infeasible(&mut self) {
        let tolerance = self.options.dual_feasibility_tolerance;
        let mut num = 0;
        let num_col = self.lp.num_col;
        for var in 0..self.num_tot() {
            if self.basis.nonbasic_flag[var] == 0 {
                continue;
            }
            let (dual, lower, upper) = if var < num_col {
                (self.info.work_dual[var], self.lp.col_lower[var], self.lp.col_upper[var])
            } else {
                let row = var - num_col;
                (-self.info.work_dual[var], self.lp.row_lower[row], self.lp.row_upper[row])
            };
            let infeasibility = if upper == f64::INFINITY {
                if lower == f64::NEG_INFINITY { dual.abs() } else { -dual }
            } else if lower == f64::NEG_INFINITY {
                dual
            } else {
                0.0
            };
            if infeasibility > 0.0 && infeasibility >= tolerance {
                num += 1;
            }
        }
        self.num_dual_phase_1_lp_dual_infeasibility = num;
    }

    pub fn invalidate_primal_infeasibility_record(&mut self) {
        self.info.num_primal_infeasibilities = -1;
    }

    pub fn invalidate_dual_infeasibility_record(&mut self) {
        self.info.num_dual_infeasibilities = -1;
    }

    pub fn bailout(&mut self) -> bool {
        if self.solve_bailout {
            return true;
        }
        if self.options.time_limit < f64::INFINITY && self.started.elapsed().as_secs_f64() > self.options.time_limit {
            self.solve_bailout = true;
            self.model_status = ModelStatus::TimeLimit;
        } else if self.iteration_count >= self.options.simplex_iteration_limit {
            self.solve_bailout = true;
            self.model_status = ModelStatus::IterationLimit;
        } else if self.interrupt.as_ref().is_some_and(|interrupt| interrupt()) {
            self.solve_bailout = true;
            self.model_status = ModelStatus::Interrupt;
        }
        self.solve_bailout
    }

    /// Called once as each solve returns: take the perturbations and
    /// shifts back out, recomputing what they changed.
    pub fn return_from_solve(&mut self) {
        self.called_return_from_solve = true;
        self.info.valid_backtracking_basis = false;
        if self.model_status != ModelStatus::Optimal {
            self.invalidate_primal_infeasibility_record();
            self.invalidate_dual_infeasibility_record();
        }
        match self.model_status {
            ModelStatus::Optimal => {}
            ModelStatus::Infeasible => {
                if self.exit_algorithm == Algorithm::Primal {
                    self.initialise_cost(Algorithm::Dual, SOLVE_PHASE_2, false);
                    self.compute_dual();
                }
                self.compute_simplex_infeasible();
            }
            ModelStatus::UnboundedOrInfeasible => {
                self.initialise_bound(Algorithm::Dual, SOLVE_PHASE_2, false);
                self.compute_primal();
                self.compute_simplex_infeasible();
            }
            ModelStatus::Unbounded => {
                self.compute_simplex_infeasible();
            }
            ModelStatus::TimeLimit | ModelStatus::IterationLimit | ModelStatus::Interrupt | ModelStatus::Unknown | ModelStatus::ObjectiveBound => {
                self.initialise_bound(Algorithm::Dual, SOLVE_PHASE_2, false);
                self.initialise_nonbasic_value_and_move();
                self.compute_primal();
                self.initialise_cost(Algorithm::Dual, SOLVE_PHASE_2, false);
                self.compute_dual();
                self.compute_simplex_infeasible();
            }
            ModelStatus::NotSet | ModelStatus::SolveError => return,
        }
        self.zero_basic_duals();
        self.compute_primal_objective_value();
    }

    /// Column values, column duals, row values and row duals.
    pub fn get_solution(&mut self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let num_col = self.lp.num_col;
        let num_row = self.lp.num_row;
        for row in 0..num_row {
            let var = self.basis.basic_index[row];
            self.info.work_value[var] = self.info.base_value[row];
            self.info.work_dual[var] = 0.0;
        }
        let col_value = self.info.work_value[..num_col].to_vec();
        let col_dual = self.info.work_dual[..num_col].iter().map(|dual| self.lp.sense * dual).collect();
        let row_value = (0..num_row).map(|row| -self.info.work_value[num_col + row]).collect();
        let row_dual = (0..num_row).map(|row| -self.lp.sense * self.info.work_dual[num_col + row]).collect();
        (col_value, col_dual, row_value, row_dual)
    }

    pub fn clear_bad_basis_change_taboo_flag(&mut self) {
        for change in &mut self.bad_basis_change {
            change.taboo = false;
        }
    }

    pub fn taboo_bad_basis_change(&self) -> bool {
        self.bad_basis_change.iter().any(|change| change.taboo)
    }

    pub fn add_bad_basis_change(&mut self, row_out: usize, variable_out: usize, variable_in: Option<usize>, reason: BadBasisChangeReason, taboo: bool) {
        for record in &mut self.bad_basis_change {
            if record.row_out == row_out && record.variable_out == variable_out && record.variable_in == variable_in && record.reason == reason {
                record.taboo = taboo;
                return;
            }
        }
        self.bad_basis_change.push(BadBasisChange {
            taboo,
            row_out,
            variable_out,
            variable_in,
            reason,
            save_value: 0.0,
        });
    }

    /// Forget the bad basis changes whose pivotal row the step moves far.
    pub fn update_bad_basis_change(&mut self, col_aq: &HVector, theta_primal: f64) {
        let tolerance = self.options.primal_feasibility_tolerance;
        self.bad_basis_change.retain(|record| (col_aq.array[record.row_out] * theta_primal).abs() < tolerance);
    }

    pub fn apply_taboo_row_out(bad_basis_change: &mut [BadBasisChange], values: &mut [f64], overwrite_with: f64) {
        for change in bad_basis_change.iter_mut() {
            if change.taboo {
                change.save_value = values[change.row_out];
                values[change.row_out] = overwrite_with;
            }
        }
    }

    pub fn unapply_taboo_row_out(bad_basis_change: &[BadBasisChange], values: &mut [f64]) {
        for change in bad_basis_change.iter().rev() {
            if change.taboo {
                values[change.row_out] = change.save_value;
            }
        }
    }

    pub fn apply_taboo_variable_in(bad_basis_change: &mut [BadBasisChange], values: &mut [f64], overwrite_with: f64) {
        for change in bad_basis_change.iter_mut() {
            if change.taboo
                && let Some(col) = change.variable_in
            {
                change.save_value = values[col];
                values[col] = overwrite_with;
            }
        }
    }

    pub fn unapply_taboo_variable_in(bad_basis_change: &[BadBasisChange], values: &mut [f64]) {
        for change in bad_basis_change.iter().rev() {
            if change.taboo
                && let Some(col) = change.variable_in
            {
                values[col] = change.save_value;
            }
        }
    }

    pub fn logical_basis(&self) -> bool {
        self.basis.basic_index.iter().all(|&var| var >= self.lp.num_col)
    }

    /// Whether row `row_out` of `B⁻¹`, signed by `move_out`, proves the LP
    /// primal infeasible: the implied bound on its combination of rows
    /// cannot be met by any column values.
    pub fn proof_of_primal_infeasibility(&mut self, row_ep: &mut HVector, move_out: i32) -> bool {
        if !self.status.has_ar_matrix {
            self.initialise_partitioned_rowwise_matrix();
        }
        let mut proof_lower = CDouble::new(0.0);
        for k in 0..row_ep.count as usize {
            let row = row_ep.index[k];
            let row_ep_value = row_ep.array[row];
            if (row_ep_value * self.ar_matrix.max_abs_row_value(row)).abs() <= self.options.small_matrix_value {
                row_ep.array[row] = 0.0;
                continue;
            }
            row_ep.array[row] *= move_out as f64;
            let row_bound = if row_ep.array[row] > 0.0 {
                let bound = self.lp.row_lower[row];
                if bound == f64::NEG_INFINITY {
                    row_ep.array[row] = 0.0;
                    continue;
                }
                bound
            } else {
                let bound = self.lp.row_upper[row];
                if bound == f64::INFINITY {
                    row_ep.array[row] = 0.0;
                    continue;
                }
                bound
            };
            proof_lower += row_ep.array[row] * row_bound;
        }
        let (proof_value, proof_index) = self.ar_matrix.product_transpose_quad(row_ep);
        let mut implied_upper = CDouble::new(0.0);
        let mut sum_inf = CDouble::new(0.0);
        for (i, &col) in proof_index.iter().enumerate() {
            let value = proof_value[i];
            if value > 0.0 {
                if self.lp.col_upper[col] == f64::INFINITY {
                    sum_inf += value;
                    if sum_inf.value() > self.options.small_matrix_value {
                        break;
                    }
                    continue;
                }
                implied_upper += value * self.lp.col_upper[col];
            } else {
                if self.lp.col_lower[col] == f64::NEG_INFINITY {
                    sum_inf += -value;
                    if sum_inf.value() > self.options.small_matrix_value {
                        break;
                    }
                    continue;
                }
                implied_upper += value * self.lp.col_lower[col];
            }
        }
        let infinite_implied_upper = sum_inf.value() > self.options.small_matrix_value;
        let gap = (proof_lower - implied_upper).value();
        !infinite_implied_upper && gap > self.options.primal_feasibility_tolerance
    }

    pub fn proof_of_primal_infeasibility_from_ray(&mut self) -> bool {
        let Some((row_out, move_out)) = self.dual_ray_record else {
            return false;
        };
        let mut row_ep = HVector::new(self.lp.num_row);
        self.unit_btran(row_out, &mut row_ep);
        self.proof_of_primal_infeasibility(&mut row_ep, move_out)
    }

    /// The power-of-two scale that brings the largest of `value` near one.
    pub fn get_value_scale(count: usize, value: &[f64]) -> f64 {
        if count == 0 {
            return 1.0;
        }
        let max_abs_value = value[..count].iter().fold(0.0f64, |max, value| max.max(value.abs()));
        nearest_power_of_two_scale(max_abs_value)
    }

    /// Improve `row_ep = B⁻ᵀ e_{row_out}` by one step of iterative
    /// refinement, its residual computed in compensated arithmetic.
    pub fn unit_btran_iterative_refinement(&mut self, row_out: usize, row_ep: &mut HVector) {
        let num_row = self.lp.num_row;
        let num_col = self.lp.num_col;
        let mut residual = HVector::new(num_row);
        let mut quad_residual = vec![CDouble::new(0.0); num_row];
        quad_residual[row_out] = CDouble::new(-1.0);
        for row in 0..num_row {
            let var = self.basis.basic_index[row];
            let mut value = quad_residual[row];
            if var < num_col {
                for el in self.lp.a.start[var]..self.lp.a.start[var + 1] {
                    value += self.lp.a.value[el] * row_ep.array[self.lp.a.index[el]];
                }
            } else {
                value += row_ep.array[var - num_col];
            }
            quad_residual[row] = value;
        }
        let mut residual_norm: f64 = 0.0;
        for row in 0..num_row {
            let value = quad_residual[row].value();
            if value != 0.0 {
                residual.array[row] = value;
                residual.index[residual.count as usize] = row;
                residual.count += 1;
            }
            residual_norm = residual_norm.max(residual.array[row].abs());
        }
        if residual_norm == 0.0 {
            return;
        }
        let residual_scale = nearest_power_of_two_scale(residual_norm);
        for k in 0..residual.count as usize {
            residual.array[residual.index[k]] *= residual_scale;
        }
        self.nla.btran(&mut residual, 1.0, &self.basis.basic_index);
        row_ep.count = 0;
        for row in 0..num_row {
            if residual.array[row] != 0.0 {
                row_ep.array[row] -= residual.array[row] / residual_scale;
            }
            if row_ep.array[row].abs() < TINY {
                row_ep.array[row] = 0.0;
            } else {
                row_ep.index[row_ep.count as usize] = row;
                row_ep.count += 1;
            }
        }
    }

    pub fn assess_dse_weight_error(&mut self, computed_edge_weight: f64, updated_edge_weight: f64) {
        self.edge_weight_error = (updated_edge_weight - computed_edge_weight).abs() / computed_edge_weight.max(1.0);
        if updated_edge_weight < computed_edge_weight {
            let deviation = computed_edge_weight / updated_edge_weight;
            self.info.average_log_low_dse_weight_error = 0.99 * self.info.average_log_low_dse_weight_error + 0.01 * deviation.ln();
        } else {
            let deviation = updated_edge_weight / computed_edge_weight;
            self.info.average_log_high_dse_weight_error = 0.99 * self.info.average_log_high_dse_weight_error + 0.01 * deviation.ln();
        }
    }

    pub fn switch_to_devex(&mut self) -> bool {
        const COSTLY_DSE_MEASURE_LIMIT: f64 = 1000.0;
        const COSTLY_DSE_MINIMUM_DENSITY: f64 = 0.01;
        const COSTLY_DSE_FRACTION_NUM_TOTAL_ITERATION_BEFORE_SWITCH: f64 = 0.1;
        const COSTLY_DSE_FRACTION_NUM_COSTLY_DSE_ITERATION_BEFORE_SWITCH: f64 = 0.05;
        let info = &mut self.info;
        let mut switch_to_devex = false;
        let denominator = info.row_ep_density.max(info.col_aq_density).max(info.row_ap_density);
        if denominator > 0.0 {
            info.costly_dse_measure = info.row_dse_density / denominator;
            info.costly_dse_measure *= info.costly_dse_measure;
        } else {
            info.costly_dse_measure = 0.0;
        }
        let costly_dse_iteration = info.costly_dse_measure > COSTLY_DSE_MEASURE_LIMIT && info.row_dse_density > COSTLY_DSE_MINIMUM_DENSITY;
        info.costly_dse_frequency *= 1.0 - RUNNING_AVERAGE_MULTIPLIER;
        if costly_dse_iteration {
            info.num_costly_dse_iteration += 1;
            info.costly_dse_frequency += RUNNING_AVERAGE_MULTIPLIER;
            let local_iteration_count = (self.iteration_count - info.control_iteration_count0) as f64;
            let local_num_tot = (self.lp.num_col + self.lp.num_row) as f64;
            switch_to_devex = info.allow_dual_steepest_edge_to_devex_switch
                && (info.num_costly_dse_iteration as f64) > local_iteration_count * COSTLY_DSE_FRACTION_NUM_COSTLY_DSE_ITERATION_BEFORE_SWITCH
                && local_iteration_count > COSTLY_DSE_FRACTION_NUM_TOTAL_ITERATION_BEFORE_SWITCH * local_num_tot;
        }
        if !switch_to_devex {
            let local_measure = info.average_log_low_dse_weight_error + info.average_log_high_dse_weight_error;
            switch_to_devex = info.allow_dual_steepest_edge_to_devex_switch && local_measure > info.dual_steepest_edge_weight_log_error_threshold;
        }
        switch_to_devex
    }

    /// The scale factors of the LP when it is unscaled, for the NLA.
    pub fn set_unscaled_with(&mut self, scale: Scale) {
        let mut scaled_a = self.lp.a.clone();
        scaled_a.apply_scale(&scale.col, &scale.row);
        self.scaled_a = Some(scaled_a);
        self.nla.scale = Some(scale);
        self.simplex_in_scaled_space = false;
        self.status.has_ar_matrix = false;
    }
}

/// The side a nonbasic variable starts at: its finite bound nearer zero.
fn initial_move(lower: f64, upper: f64) -> i8 {
    if lower == upper {
        0
    } else if lower != f64::NEG_INFINITY {
        if upper != f64::INFINITY { if lower.abs() < upper.abs() { 1 } else { -1 } } else { 1 }
    } else if upper != f64::INFINITY {
        -1
    } else {
        0
    }
}

pub(crate) fn update_operation_result_density(local_density: f64, density: &mut f64) {
    *density = (1.0 - RUNNING_AVERAGE_MULTIPLIER) * *density + RUNNING_AVERAGE_MULTIPLIER * local_density;
}
