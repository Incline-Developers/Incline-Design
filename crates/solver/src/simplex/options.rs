//! The options the simplex solver reads, at HiGHS's defaults.

/// HiGHS's `simplex_dual_edge_weight_strategy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EdgeWeightStrategy {
    /// Dual steepest edge, switching to Devex when it proves costly.
    #[default]
    Choose,
    Dantzig,
    Devex,
    SteepestEdge,
}

/// HiGHS's `simplex_price_strategy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PriceStrategy {
    Col,
    Row,
    RowSwitch,
    #[default]
    RowSwitchColSwitch,
}

/// Which simplex method to run (HiGHS's `simplex_strategy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SimplexStrategy {
    /// The primal method when the starting basis is primal feasible,
    /// otherwise the dual.
    Choose,
    #[default]
    Dual,
    Primal,
}

/// Which LP solver runs (HiGHS's `solver`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Solver {
    /// The simplex method (what HiGHS's `choose` picks for an LP).
    #[default]
    Simplex,
    /// IPX's interior point method, then crossover to a basis and the
    /// simplex method to finish.
    Ipm,
}

#[derive(Clone, Debug)]
pub struct Options {
    /// Presolve the LP first (HiGHS's `presolve`), unless a solve starts
    /// from a basis.
    pub presolve: bool,
    pub solver: Solver,
    pub simplex_strategy: SimplexStrategy,
    pub primal_feasibility_tolerance: f64,
    pub dual_feasibility_tolerance: f64,
    pub time_limit: f64,
    pub simplex_iteration_limit: usize,
    pub simplex_update_limit: usize,
    pub dual_edge_weight_strategy: EdgeWeightStrategy,
    pub price_strategy: PriceStrategy,
    pub dual_simplex_cost_perturbation_multiplier: f64,
    pub primal_simplex_bound_perturbation_multiplier: f64,
    pub dual_simplex_pivot_growth_tolerance: f64,
    pub factor_pivot_threshold: f64,
    pub factor_pivot_tolerance: f64,
    pub max_dual_simplex_cleanup_level: usize,
    pub max_dual_simplex_phase1_cleanup_level: usize,
    pub small_matrix_value: f64,
    pub large_matrix_value: f64,
    pub infinite_bound: f64,
    pub infinite_cost: f64,
    pub no_unnecessary_rebuild_refactor: bool,
    pub rebuild_refactor_solution_error_tolerance: f64,
    pub dual_steepest_edge_weight_log_error_threshold: f64,
    pub less_infeasible_dse_check: bool,
    pub less_infeasible_dse_choose_row: bool,
    pub allowed_matrix_scale_factor: i32,
    pub scale: bool,
    pub random_seed: u32,
    pub objective_bound: f64,
    pub ipm_optimality_tolerance: f64,
    pub start_crossover_tolerance: f64,
    pub ipm_iteration_limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            presolve: true,
            solver: Solver::Simplex,
            simplex_strategy: SimplexStrategy::Dual,
            primal_feasibility_tolerance: 1e-7,
            dual_feasibility_tolerance: 1e-7,
            time_limit: f64::INFINITY,
            simplex_iteration_limit: usize::MAX,
            simplex_update_limit: 5000,
            dual_edge_weight_strategy: EdgeWeightStrategy::Choose,
            price_strategy: PriceStrategy::RowSwitchColSwitch,
            dual_simplex_cost_perturbation_multiplier: 1.0,
            primal_simplex_bound_perturbation_multiplier: 1.0,
            dual_simplex_pivot_growth_tolerance: 1e-9,
            factor_pivot_threshold: crate::factor::DEFAULT_PIVOT_THRESHOLD,
            factor_pivot_tolerance: crate::factor::DEFAULT_PIVOT_TOLERANCE,
            max_dual_simplex_cleanup_level: 1,
            max_dual_simplex_phase1_cleanup_level: 2,
            small_matrix_value: 1e-9,
            large_matrix_value: 1e15,
            infinite_bound: 1e20,
            infinite_cost: 1e20,
            no_unnecessary_rebuild_refactor: true,
            rebuild_refactor_solution_error_tolerance: 1e-8,
            dual_steepest_edge_weight_log_error_threshold: 1e1,
            less_infeasible_dse_check: true,
            less_infeasible_dse_choose_row: true,
            allowed_matrix_scale_factor: 20,
            scale: true,
            random_seed: 0,
            objective_bound: f64::INFINITY,
            ipm_optimality_tolerance: 1e-8,
            start_crossover_tolerance: 1e-8,
            ipm_iteration_limit: i32::MAX as usize,
        }
    }
}
