//! IPX's `LpSolver` (`lp_solver.cc`): the interior point method in two
//! stages (diagonal then basis preconditioning), then crossover to a
//! basic solution.

use super::{
    basis::{Basis, starting_basis},
    control::Control,
    crossover::Crossover,
    ipm::Ipm,
    iterate::Iterate,
    kkt::{KktSolverBasis, KktSolverDiag},
    model::{Model, dual_infeasibility, primal_infeasibility},
    sparse::{Int, u},
    *,
};

/// What `LpSolver::Solve` leaves: its info and, after a crossover that
/// succeeded, the basic solution in the solver's terms.
#[derive(Default)]
pub(crate) struct LpSolver {
    pub info: Info,
    x_crossover: Vec<f64>,
    y_crossover: Vec<f64>,
    z_crossover: Vec<f64>,
    crossover_weights: Vec<f64>,
    basic_statuses: Vec<Int>,
    /// The final IPM iterate `(x, xl, xu, y, zl, zu)` in the solver's
    /// terms (HiGHS's `iterate_`).
    interior: Option<[Vec<f64>; 6]>,
}

impl LpSolver {
    /// `LpSolver::Solve` for a loaded `model`. Returns IPX's status.
    pub fn solve(&mut self, control: &mut Control, model: &Model) -> Int {
        if model.empty() {
            self.info.status = IPX_STATUS_NO_MODEL;
            return self.info.status;
        }
        self.clear_solution(model);
        control.reset_timer();
        let control = &*control;
        let mut iterate = Iterate::new(model);
        let mut basis: Option<Basis<'_>> = None;
        self.interior_point_solve(control, model, &mut iterate, &mut basis);
        self.interior = Some([
            iterate.x.clone(),
            iterate.xl.clone(),
            iterate.xu.clone(),
            iterate.y.clone(),
            iterate.zl.clone(),
            iterate.zu.clone(),
        ]);
        let run_crossover_on = control.params.run_crossover == 1;
        let run_crossover_choose = control.params.run_crossover == -1;
        let run_crossover_not_off = run_crossover_choose || run_crossover_on;
        let run_crossover = (self.info.status_ipm == IPX_STATUS_OPTIMAL && run_crossover_on) || (self.info.status_ipm == IPX_STATUS_IMPRECISE && run_crossover_not_off);
        if run_crossover {
            self.build_crossover_starting_point(model, &iterate);
            self.run_crossover(control, model, basis.as_mut().expect("crossover runs after a starting basis"));
        }
        let status_of = |status| status == IPX_STATUS_PRIMAL_INFEAS || status == IPX_STATUS_DUAL_INFEAS;
        if status_of(self.info.status_ipm) || status_of(self.info.status_crossover) {
            self.info.status = IPX_STATUS_SOLVED;
        } else {
            let method_status = if run_crossover { self.info.status_crossover } else { self.info.status_ipm };
            self.info.status = if method_status == IPX_STATUS_OPTIMAL || method_status == IPX_STATUS_IMPRECISE {
                IPX_STATUS_SOLVED
            } else {
                IPX_STATUS_STOPPED
            };
        }
        self.info.status
    }

    /// `LpSolver::GetBasicSolution`: `(x, slack, y, z, cbasis, vbasis)` in
    /// the user's terms, if crossover found one.
    #[allow(clippy::type_complexity)]
    pub fn basic_solution(&self, model: &Model) -> Option<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<Int>, Vec<Int>)> {
        if self.basic_statuses.is_empty() {
            return None;
        }
        let (x, slack, y, z) = model.postsolve_basic_solution(&self.x_crossover, &self.y_crossover, &self.z_crossover, &self.basic_statuses);
        let (cbasis, vbasis) = model.postsolve_basis(&self.basic_statuses);
        Some((x, slack, y, z, cbasis, vbasis))
    }

    /// `LpSolver::GetInteriorSolution`: the final IPM iterate in the user's
    /// terms, `(x, xl, xu, slack, y, zl, zu)`.
    #[allow(clippy::type_complexity)]
    pub fn interior_solution(&self, model: &Model) -> Option<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)> {
        let [x, xl, xu, y, zl, zu] = self.interior.as_ref()?;
        Some(model.postsolve_interior_solution(x, xl, xu, y, zl, zu))
    }

    fn clear_solution(&mut self, model: &Model) {
        *self = Self::default();
        model.get_info(&mut self.info);
    }

    fn interior_point_solve<'a>(&mut self, control: &'a Control, model: &'a Model, iterate: &mut Iterate<'a>, basis: &mut Option<Basis<'a>>) {
        iterate.set_feasibility_tol(control.params.ipm_feasibility_tol);
        iterate.set_optimality_tol(control.params.ipm_optimality_tol);
        if control.params.run_crossover != 0 {
            iterate.set_start_crossover_tol(control.params.start_crossover_tol);
        }
        self.run_ipm(control, model, iterate, basis);
        iterate.postprocess();
        iterate.evaluate_postsolved(&mut self.info);
        if self.info.status_ipm == IPX_STATUS_OPTIMAL
            && (self.info.rel_objgap.abs() > control.params.ipm_optimality_tol
                || self.info.rel_presidual > control.params.ipm_feasibility_tol
                || self.info.rel_dresidual > control.params.ipm_feasibility_tol)
        {
            self.info.status_ipm = IPX_STATUS_IMPRECISE;
        }
        if self.info.centring_tried {
            self.info.status_ipm = if self.info.centring_success { IPX_STATUS_OPTIMAL } else { IPX_STATUS_IMPRECISE };
        }
    }

    fn run_ipm<'a>(&mut self, control: &'a Control, model: &'a Model, iterate: &mut Iterate<'a>, basis: &mut Option<Basis<'a>>) {
        let mut ipm = Ipm::new(control);
        self.info.centring_tried = false;
        self.info.centring_success = false;
        self.compute_starting_point(control, model, &mut ipm, iterate);
        if self.info.status_ipm != IPX_STATUS_NOT_RUN {
            return;
        }
        self.run_initial_ipm(control, model, &mut ipm, iterate);
        if self.info.status_ipm != IPX_STATUS_NOT_RUN {
            return;
        }
        self.build_starting_basis(control, model, iterate, basis);
        if self.info.status_ipm != IPX_STATUS_NOT_RUN || self.info.centring_tried {
            return;
        }
        self.run_main_ipm(control, &mut ipm, iterate, basis.as_mut().expect("the starting basis was built"));
    }

    fn compute_starting_point(&mut self, control: &Control, model: &Model, ipm: &mut Ipm<'_>, iterate: &mut Iterate<'_>) {
        let mut kkt = KktSolverDiag::new(control, model);
        ipm.starting_point(&mut kkt, iterate, &mut self.info);
    }

    fn run_initial_ipm(&mut self, control: &Control, model: &Model, ipm: &mut Ipm<'_>, iterate: &mut Iterate<'_>) {
        let mut kkt = KktSolverDiag::new(control, model);
        let switchiter = control.params.switchiter;
        if switchiter < 0 {
            let m = model.rows();
            kkt.maxiter = std::cmp::min(500, 10 + m / 20);
            ipm.maxiter = control.params.ipm_maxiter;
        } else {
            ipm.maxiter = std::cmp::min(switchiter, control.params.ipm_maxiter);
        }
        ipm.driver(&mut kkt, iterate, &mut self.info);
        match self.info.status_ipm {
            IPX_STATUS_OPTIMAL | IPX_STATUS_NO_PROGRESS => self.info.status_ipm = IPX_STATUS_NOT_RUN,
            IPX_STATUS_FAILED => {
                self.info.status_ipm = IPX_STATUS_NOT_RUN;
                self.info.errflag = 0;
            }
            IPX_STATUS_ITER_LIMIT if self.info.iter < control.params.ipm_maxiter => {
                self.info.status_ipm = IPX_STATUS_NOT_RUN;
            }
            _ => {}
        }
    }

    fn build_starting_basis<'a>(&mut self, control: &'a Control, model: &'a Model, iterate: &mut Iterate<'a>, basis: &mut Option<Basis<'a>>) {
        if control.params.stop_at_switch < 0 {
            self.info.status_ipm = IPX_STATUS_DEBUG;
            return;
        }
        let basis = basis.insert(Basis::new(control, model));
        starting_basis(iterate, basis, &mut self.info);
        if self.info.errflag == IPX_ERROR_USER_INTERRUPT {
            self.info.errflag = 0;
            self.info.status_ipm = IPX_STATUS_USER_INTERRUPT;
            return;
        } else if self.info.errflag == IPX_ERROR_TIME_INTERRUPT {
            self.info.errflag = 0;
            self.info.status_ipm = IPX_STATUS_TIME_LIMIT;
            return;
        } else if self.info.errflag != 0 {
            self.info.status_ipm = IPX_STATUS_FAILED;
            return;
        }
        if model.dualized() {
            std::mem::swap(&mut self.info.dependent_rows, &mut self.info.dependent_cols);
            std::mem::swap(&mut self.info.rows_inconsistent, &mut self.info.cols_inconsistent);
        }
        if control.params.stop_at_switch > 0 {
            self.info.status_ipm = IPX_STATUS_DEBUG;
            return;
        }
        if self.info.rows_inconsistent {
            self.info.status_ipm = IPX_STATUS_PRIMAL_INFEAS;
        } else if self.info.cols_inconsistent {
            self.info.status_ipm = IPX_STATUS_DUAL_INFEAS;
        }
    }

    fn run_main_ipm(&mut self, control: &Control, ipm: &mut Ipm<'_>, iterate: &mut Iterate<'_>, basis: &mut Basis<'_>) {
        let mut kkt = KktSolverBasis::new(control, basis);
        ipm.maxiter = control.params.ipm_maxiter;
        ipm.driver(&mut kkt, iterate, &mut self.info);
    }

    fn build_crossover_starting_point(&mut self, model: &Model, iterate: &Iterate<'_>) {
        let m = u(model.rows());
        let n = u(model.cols());
        self.x_crossover = vec![0.0; n + m];
        self.y_crossover = vec![0.0; m];
        self.z_crossover = vec![0.0; n + m];
        iterate.drop_to_complementarity(&mut self.x_crossover, &mut self.y_crossover, &mut self.z_crossover);
        self.crossover_weights = (0..n + m).map(|j| iterate.scaling_factor(j)).collect();
    }

    fn run_crossover(&mut self, control: &Control, model: &Model, basis: &mut Basis<'_>) {
        let m = u(model.rows());
        let n = u(model.cols());
        let (lb, ub) = (model.lb(), model.ub());
        self.basic_statuses.clear();
        let weights = (!self.crossover_weights.is_empty()).then_some(&self.crossover_weights[..]);
        let mut crossover = Crossover::new(control);
        crossover.push_all(basis, &mut self.x_crossover, &mut self.y_crossover, &mut self.z_crossover, weights, &mut self.info);
        self.info.updates_crossover = crossover.primal_pivots + crossover.dual_pivots;
        if self.info.status_crossover != IPX_STATUS_OPTIMAL {
            self.x_crossover.clear();
            self.y_crossover.clear();
            self.z_crossover.clear();
            return;
        }
        basis.compute_basic_solution(&mut self.x_crossover, &mut self.y_crossover, &mut self.z_crossover);
        self.basic_statuses = (0..n + m)
            .map(|j| {
                if basis.is_basic(j as Int) {
                    IPX_BASIC
                } else if lb[j] == ub[j] {
                    if self.z_crossover[j] >= 0.0 { IPX_NONBASIC_LB } else { IPX_NONBASIC_UB }
                } else if self.x_crossover[j] == lb[j] {
                    IPX_NONBASIC_LB
                } else if self.x_crossover[j] == ub[j] {
                    IPX_NONBASIC_UB
                } else {
                    IPX_SUPERBASIC
                }
            })
            .collect();
        // IPX's debug stream evaluates these even when it prints nothing;
        // the singular value estimate solves with the factors, which can
        // compact them, so it is kept for its effect.
        let _ = (
            primal_infeasibility(model, &self.x_crossover),
            dual_infeasibility(model, &self.x_crossover, &self.z_crossover),
        );
        let _ = basis.min_singular_value();
        model.evaluate_basic_solution(&self.x_crossover, &self.y_crossover, &self.z_crossover, &self.basic_statuses, &mut self.info);
        if self.info.primal_infeas > control.params.pfeasibility_tol || self.info.dual_infeas > control.params.dfeasibility_tol {
            self.info.status_crossover = IPX_STATUS_IMPRECISE;
        }
    }
}
