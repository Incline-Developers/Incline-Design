//! IPX's `IPM` (`ipm.cc`): Mehrotra's predictor-corrector interior point
//! method on the iterate, with its Newton systems solved by a KKT
//! solver, with the analytic centring steps HiGHS asks for when it wants
//! the analytic centre.

use super::{
    IPX_ERROR_TIME_INTERRUPT, IPX_ERROR_USER_INTERRUPT, IPX_STATUS_DUAL_INFEAS, IPX_STATUS_FAILED, IPX_STATUS_ITER_LIMIT, IPX_STATUS_NO_PROGRESS, IPX_STATUS_NOT_RUN,
    IPX_STATUS_OPTIMAL, IPX_STATUS_PRIMAL_INFEAS, IPX_STATUS_TIME_LIMIT, IPX_STATUS_USER_INTERRUPT, Info,
    control::Control,
    iterate::{Iterate, State, std_max, std_min},
    kkt::KktSolver,
    sparse::{Int, dot_column, infnorm, multiply_add, scatter_column, twonorm, u},
};

const DIVERGE_TOL: f64 = 1e6;

pub(crate) struct Step {
    pub x: Vec<f64>,
    pub xl: Vec<f64>,
    pub xu: Vec<f64>,
    pub y: Vec<f64>,
    pub zl: Vec<f64>,
    pub zu: Vec<f64>,
}

impl Step {
    fn new(m: usize, n: usize) -> Self {
        Self {
            x: vec![0.0; n + m],
            xl: vec![0.0; n + m],
            xu: vec![0.0; n + m],
            y: vec![0.0; m],
            zl: vec![0.0; n + m],
            zu: vec![0.0; n + m],
        }
    }
}

pub(crate) struct Ipm<'c> {
    control: &'c Control,
    step_primal: f64,
    step_dual: f64,
    num_bad_iter: Int,
    best_complementarity: f64,
    pub maxiter: Int,
    centring_ratio: f64,
    bad_products: Int,
}

impl<'c> Ipm<'c> {
    pub fn new(control: &'c Control) -> Self {
        Self {
            control,
            step_primal: 0.0,
            step_dual: 0.0,
            num_bad_iter: 0,
            best_complementarity: 0.0,
            maxiter: -1,
            centring_ratio: 0.0,
            bad_products: 0,
        }
    }

    pub fn starting_point(&mut self, kkt: &mut dyn KktSolver, iterate: &mut Iterate<'_>, info: &mut Info) {
        self.compute_starting_point(kkt, iterate, info);
        if info.errflag == IPX_ERROR_USER_INTERRUPT {
            info.errflag = 0;
            info.status_ipm = IPX_STATUS_USER_INTERRUPT;
        } else if info.errflag == IPX_ERROR_TIME_INTERRUPT {
            info.errflag = 0;
            info.status_ipm = IPX_STATUS_TIME_LIMIT;
        } else if info.errflag != 0 {
            info.status_ipm = IPX_STATUS_FAILED;
        } else {
            info.status_ipm = IPX_STATUS_NOT_RUN;
        }
    }

    pub fn driver(&mut self, kkt: &mut dyn KktSolver, iterate: &mut Iterate<'_>, info: &mut Info) {
        let model = iterate.model();
        let m = u(model.rows());
        let n = u(model.cols());
        let mut step = Step::new(m, n);
        self.num_bad_iter = 0;
        loop {
            if iterate.term_crit_reached() {
                info.status_ipm = IPX_STATUS_OPTIMAL;
                break;
            }
            if self.num_bad_iter >= 5 || (self.best_complementarity > 0.0 && iterate.complementarity() > DIVERGE_TOL * self.best_complementarity) {
                let dualized = model.dualized();
                let pobjective = iterate.pobjective_after_postproc();
                let dobjective = iterate.dobjective_after_postproc();
                info.status_ipm = if dobjective > std_max(10.0 * pobjective.abs(), 1.0) {
                    if dualized { IPX_STATUS_DUAL_INFEAS } else { IPX_STATUS_PRIMAL_INFEAS }
                } else if pobjective < -std_max(10.0 * dobjective.abs(), 1.0) {
                    if dualized { IPX_STATUS_PRIMAL_INFEAS } else { IPX_STATUS_DUAL_INFEAS }
                } else {
                    IPX_STATUS_NO_PROGRESS
                };
                break;
            }
            if info.iter >= self.maxiter {
                info.status_ipm = IPX_STATUS_ITER_LIMIT;
                break;
            }
            info.errflag = self.control.interrupt_check();
            if info.errflag != 0 {
                break;
            }
            kkt.factorize(Some(iterate), info);
            if info.errflag != 0 {
                break;
            }
            self.predictor(kkt, iterate, info, &mut step);
            if info.errflag != 0 {
                break;
            }
            self.add_corrector(kkt, iterate, info, &mut step);
            if info.errflag != 0 {
                break;
            }
            self.make_step(iterate, &step);
            info.iter += 1;
        }
        if info.errflag != 0 {
            if info.errflag == IPX_ERROR_USER_INTERRUPT {
                info.errflag = 0;
                info.status_ipm = IPX_STATUS_USER_INTERRUPT;
            } else if info.errflag == IPX_ERROR_TIME_INTERRUPT {
                info.errflag = 0;
                info.status_ipm = IPX_STATUS_TIME_LIMIT;
            } else {
                info.status_ipm = IPX_STATUS_FAILED;
            }
        }

        if self.control.params.run_centring != 0 && info.status_ipm == IPX_STATUS_OPTIMAL && !info.centring_tried {
            // Centrality is the spread of the products xj * zj around mu:
            // the point is centred once max/min is below the tolerance. A
            // step is taken while it reduces the spread enough or the
            // number of products outside [0.1 mu, 10 mu].
            let mu_frozen = iterate.mu();
            self.assess_centrality(iterate, &iterate.xl, &iterate.xu, &iterate.zl, &iterate.zu, iterate.mu());
            let mut prev_ratio = self.centring_ratio;
            let mut prev_bad_products = self.bad_products;
            info.centring_success = false;
            if prev_ratio < self.control.params.centring_ratio_tolerance {
                info.centring_success = true;
            } else {
                for _ in 0..self.control.params.max_centring_steps {
                    self.centring(kkt, iterate, info, &mut step, mu_frozen);
                    if !self.evaluate_centring_step(iterate, &step, prev_ratio, prev_bad_products) {
                        break;
                    }
                    self.make_step_centring(iterate, &step, true);
                    info.iter += 1;
                    self.assess_centrality(iterate, &iterate.xl, &iterate.xu, &iterate.zl, &iterate.zu, iterate.mu());
                    prev_ratio = self.centring_ratio;
                    prev_bad_products = self.bad_products;
                    if prev_ratio < self.control.params.centring_ratio_tolerance {
                        info.centring_success = true;
                        break;
                    }
                }
            }
            info.centring_tried = true;
        }
    }

    /// IPX's `Centring`: the Newton step towards the central path point
    /// for `mu` (sigma = 1).
    fn centring(&mut self, kkt: &mut dyn KktSolver, iterate: &Iterate<'_>, info: &mut Info, step: &mut Step, mu: f64) {
        let nm = iterate.x.len();
        let sigma = 1.0;
        let mut sl = vec![0.0; nm];
        let mut su = vec![0.0; nm];
        for j in 0..nm {
            if iterate.has_barrier_lb(j) {
                sl[j] = -iterate.xl[j] * iterate.zl[j] + sigma * mu;
            }
        }
        for j in 0..nm {
            if iterate.has_barrier_ub(j) {
                su[j] = -iterate.xu[j] * iterate.zu[j] + sigma * mu;
            }
        }
        self.solve_newton_system(kkt, iterate, info, &sl, &su, step);
    }

    /// IPX's `AssessCentrality`: the spread max/min of the products xj * zj
    /// (with mu inside), and how many lie outside [0.1 mu, 10 mu].
    fn assess_centrality(&mut self, iterate: &Iterate<'_>, xl: &[f64], xu: &[f64], zl: &[f64], zu: &[f64], mu: f64) {
        let nm = iterate.x.len();
        let mut minxz = f64::INFINITY;
        let mut maxxz = 0.0;
        let gamma = 0.1;
        self.bad_products = 0;
        for j in 0..nm {
            if iterate.has_barrier_lb(j) {
                let product = xl[j] * zl[j];
                if product < gamma * mu || product > mu / gamma {
                    self.bad_products += 1;
                }
                minxz = std_min(minxz, product);
                maxxz = std_max(maxxz, product);
            }
        }
        for j in 0..nm {
            if iterate.has_barrier_ub(j) {
                let product = xu[j] * zu[j];
                if product < gamma * mu || product > mu / gamma {
                    self.bad_products += 1;
                }
                minxz = std_min(minxz, product);
                maxxz = std_max(maxxz, product);
            }
        }
        maxxz = std_max(maxxz, mu);
        minxz = std_min(minxz, mu);
        self.centring_ratio = maxxz / minxz;
    }

    /// IPX's `EvaluateCentringStep`: whether the step reduces the spread
    /// enough, or the number of products off the central path.
    fn evaluate_centring_step(&mut self, iterate: &Iterate<'_>, step: &Step, prev_ratio: f64, prev_bad: Int) -> bool {
        self.step_sizes_centring(iterate, step, true);
        let nm = iterate.x.len();
        let mut xl_temp = iterate.xl.clone();
        let mut xu_temp = iterate.xu.clone();
        let mut zl_temp = iterate.zl.clone();
        let mut zu_temp = iterate.zu.clone();
        for j in 0..nm {
            if iterate.has_barrier_lb(j) {
                xl_temp[j] += self.step_primal * step.xl[j];
            }
            if iterate.has_barrier_ub(j) {
                xu_temp[j] += self.step_primal * step.xu[j];
            }
            if iterate.has_barrier_lb(j) {
                zl_temp[j] += self.step_dual * step.zl[j];
            }
            if iterate.has_barrier_ub(j) {
                zu_temp[j] += self.step_dual * step.zu[j];
            }
        }
        let mut mu_temp = 0.0;
        let mut num_finite = 0;
        for j in 0..nm {
            if iterate.has_barrier_lb(j) {
                mu_temp += xl_temp[j] * zl_temp[j];
                num_finite += 1;
            }
            if iterate.has_barrier_ub(j) {
                mu_temp += xu_temp[j] * zu_temp[j];
                num_finite += 1;
            }
        }
        mu_temp /= num_finite as f64;
        self.assess_centrality(iterate, &xl_temp, &xu_temp, &zl_temp, &zu_temp, mu_temp);
        self.centring_ratio < self.control.params.centring_ratio_reduction * prev_ratio || self.bad_products < prev_bad
    }

    fn compute_starting_point(&mut self, kkt: &mut dyn KktSolver, iterate: &mut Iterate<'_>, info: &mut Info) {
        let model = iterate.model();
        let m = u(model.rows());
        let n = u(model.cols());
        let (ai, b, c, lb, ub) = (model.ai(), model.b(), model.c(), model.lb(), model.ub());
        let mut x = vec![0.0; n + m];
        let mut xl = vec![0.0; n + m];
        let mut xu = vec![0.0; n + m];
        let mut y = vec![0.0; m];
        let mut zl = vec![0.0; n + m];
        let mut zu = vec![0.0; n + m];
        kkt.factorize(None, info);
        if info.errflag != 0 {
            return;
        }
        let mut rb = b.to_vec();
        for j in 0..n + m {
            let mut xj = 0.0;
            if xj < lb[j] {
                xj = lb[j];
            }
            if xj > ub[j] {
                xj = ub[j];
            }
            x[j] = xj;
            if xj != 0.0 {
                scatter_column(ai, j as Int, -xj, &mut rb);
            }
        }
        let tol = 0.1 * infnorm(&rb);
        zl.fill(0.0);
        kkt.solve(&zl.clone(), &rb, tol, &mut xl, &mut y, info);
        if info.errflag != 0 {
            return;
        }
        for j in 0..n + m {
            x[j] += xl[j];
        }
        let mut xinfeas = 0.0;
        for j in 0..n + m {
            xl[j] = x[j] - lb[j];
            xinfeas = std_max(xinfeas, -xl[j]);
            xu[j] = ub[j] - x[j];
            xinfeas = std_max(xinfeas, -xu[j]);
        }
        let xshift1 = 1.0 + 1.5 * xinfeas;
        for j in 0..n + m {
            xl[j] += xshift1;
            xu[j] += xshift1;
        }
        let cnorm = twonorm(c);
        if cnorm == 0.0 {
            for j in 0..n + m {
                zl[j] = if lb[j].is_finite() { 1.0 } else { 0.0 };
                zu[j] = if ub[j].is_finite() { 1.0 } else { 0.0 };
            }
        } else {
            rb.fill(0.0);
            let tol = 0.1 * infnorm(c);
            kkt.solve(c, &rb, tol, &mut zl, &mut y, info);
            if info.errflag != 0 {
                return;
            }
            zl.copy_from_slice(c);
            multiply_add(ai, &y, -1.0, &mut zl, 'T');
            let znorm = twonorm(&zl);
            let rho = 0.05;
            if znorm < rho * cnorm {
                for j in 0..n + m {
                    zl[j] += rho * c[j];
                }
                for yi in y.iter_mut() {
                    *yi *= 1.0 - rho;
                }
            }
            let mut zinfeas = 0.0;
            for j in 0..n + m {
                let zval = zl[j];
                zl[j] = 0.0;
                zu[j] = 0.0;
                if lb[j].is_finite() && ub[j].is_finite() {
                    zl[j] = 0.5 * zval;
                    zu[j] = -0.5 * zval;
                } else if lb[j].is_finite() {
                    zl[j] = zval;
                } else if ub[j].is_finite() {
                    zu[j] = -zval;
                }
                zinfeas = std_max(zinfeas, -zl[j]);
                zinfeas = std_max(zinfeas, -zu[j]);
            }
            let zshift1 = 1.0 + 1.5 * zinfeas;
            for j in 0..n + m {
                if lb[j].is_finite() {
                    zl[j] += zshift1;
                }
                if ub[j].is_finite() {
                    zu[j] += zshift1;
                }
            }
        }
        let mut xsum = 1.0;
        let mut zsum = 1.0;
        let mut mu = 1.0;
        for j in 0..n + m {
            if lb[j].is_finite() {
                xsum += xl[j];
                zsum += zl[j];
                mu += xl[j] * zl[j];
            }
            if ub[j].is_finite() {
                xsum += xu[j];
                zsum += zu[j];
                mu += xu[j] * zu[j];
            }
        }
        let xshift2 = 0.5 * mu / zsum;
        let zshift2 = 0.5 * mu / xsum;
        for j in 0..n + m {
            xl[j] += xshift2;
            xu[j] += xshift2;
        }
        for j in 0..n + m {
            if lb[j].is_finite() {
                zl[j] += zshift2;
            }
            if ub[j].is_finite() {
                zu[j] += zshift2;
            }
        }
        iterate.initialize(x, xl, xu, y, zl, zu);
        self.best_complementarity = iterate.complementarity();
    }

    fn predictor(&mut self, kkt: &mut dyn KktSolver, iterate: &Iterate<'_>, info: &mut Info, step: &mut Step) {
        let nm = iterate.x.len();
        let mut sl = vec![0.0; nm];
        let mut su = vec![0.0; nm];
        for j in 0..nm {
            sl[j] = if iterate.has_barrier_lb(j) { -iterate.xl[j] * iterate.zl[j] } else { 0.0 };
        }
        for j in 0..nm {
            su[j] = if iterate.has_barrier_ub(j) { -iterate.xu[j] * iterate.zu[j] } else { 0.0 };
        }
        self.solve_newton_system(kkt, iterate, info, &sl, &su, step);
    }

    fn add_corrector(&mut self, kkt: &mut dyn KktSolver, iterate: &Iterate<'_>, info: &mut Info, step: &mut Step) {
        let nm = iterate.x.len();
        let (xl, xu, zl, zu) = (&iterate.xl, &iterate.xu, &iterate.zl, &iterate.zu);
        let mu = iterate.mu();
        let step_xl = step_to_boundary(xl, &step.xl, None, 1.0);
        let step_xu = step_to_boundary(xu, &step.xu, None, 1.0);
        let step_zl = step_to_boundary(zl, &step.zl, None, 1.0);
        let step_zu = step_to_boundary(zu, &step.zu, None, 1.0);
        let maxp = std_min(step_xl, step_xu);
        let maxd = std_min(step_zl, step_zu);
        let mut muaff = 0.0;
        let mut num_finite = 0;
        for j in 0..nm {
            if iterate.has_barrier_lb(j) {
                muaff += (xl[j] + maxp * step.xl[j]) * (zl[j] + maxd * step.zl[j]);
                num_finite += 1;
            }
            if iterate.has_barrier_ub(j) {
                muaff += (xu[j] + maxp * step.xu[j]) * (zu[j] + maxd * step.zu[j]);
                num_finite += 1;
            }
        }
        muaff /= num_finite as f64;
        let ratio = muaff / mu;
        let sigma = ratio * ratio * ratio;
        let mut sl = vec![0.0; nm];
        let mut su = vec![0.0; nm];
        for j in 0..nm {
            sl[j] = if iterate.has_barrier_lb(j) {
                -xl[j] * zl[j] + sigma * mu - step.xl[j] * step.zl[j]
            } else {
                0.0
            };
        }
        for j in 0..nm {
            su[j] = if iterate.has_barrier_ub(j) {
                -xu[j] * zu[j] + sigma * mu - step.xu[j] * step.zu[j]
            } else {
                0.0
            };
        }
        self.solve_newton_system(kkt, iterate, info, &sl, &su, step);
    }

    fn step_sizes(&mut self, iterate: &Iterate<'_>, step: &Step) {
        self.step_sizes_centring(iterate, step, false);
    }

    fn step_sizes_centring(&mut self, iterate: &Iterate<'_>, step: &Step, is_centring: bool) {
        let nm = iterate.x.len();
        let (xl, xu, zl, zu) = (&iterate.xl, &iterate.xu, &iterate.zl, &iterate.zu);
        let (dxl, dxu, dzl, dzu) = (&step.xl, &step.xu, &step.zl, &step.zu);
        let gammaf = 0.9;
        let gammaa = 1.0 / (1.0 - gammaf);
        let mut block_xl = -1;
        let mut block_xu = -1;
        let mut block_zl = -1;
        let mut block_zu = -1;
        let step_xl = step_to_boundary(xl, dxl, Some(&mut block_xl), 1.0);
        let step_xu = step_to_boundary(xu, dxu, Some(&mut block_xu), 1.0);
        let step_zl = step_to_boundary(zl, dzl, Some(&mut block_zl), 1.0);
        let step_zu = step_to_boundary(zu, dzu, Some(&mut block_zu), 1.0);
        let maxp = step_xl.min(step_xu);
        let maxd = step_zl.min(step_zu);
        let mut mufull = 0.0;
        let mut num_finite = 0;
        for j in 0..nm {
            if iterate.has_barrier_lb(j) {
                mufull += (xl[j] + maxp * dxl[j]) * (zl[j] + maxd * dzl[j]);
                num_finite += 1;
            }
            if iterate.has_barrier_ub(j) {
                mufull += (xu[j] + maxp * dxu[j]) * (zu[j] + maxd * dzu[j]);
                num_finite += 1;
            }
        }
        mufull /= num_finite as f64;
        mufull /= gammaa;
        let mut alphap = 1.0;
        let mut alphad = 1.0;
        if maxp < 1.0 {
            if step_xl <= step_xu {
                let blockp = u(block_xl);
                let buffer = mufull / (zl[blockp] + maxd * dzl[blockp]);
                alphap = (xl[blockp] - buffer) / (-dxl[blockp]);
            } else {
                let blockp = u(block_xu);
                let buffer = mufull / (zu[blockp] + maxd * dzu[blockp]);
                alphap = (xu[blockp] - buffer) / (-dxu[blockp]);
            }
            alphap = std_max(alphap, gammaf * maxp);
            alphap = std_min(alphap, 1.0);
        }
        if maxd < 1.0 {
            if step_zl <= step_zu {
                let blockd = u(block_zl);
                let buffer = mufull / (xl[blockd] + maxp * dxl[blockd]);
                alphad = (zl[blockd] - buffer) / (-dzl[blockd]);
            } else {
                let blockd = u(block_zu);
                let buffer = mufull / (xu[blockd] + maxp * dxu[blockd]);
                alphad = (zu[blockd] - buffer) / (-dzu[blockd]);
            }
            alphad = std_max(alphad, gammaf * maxd);
            alphad = std_min(alphad, 1.0);
        }
        self.step_primal = std_min(alphap, 1.0 - 1e-6);
        self.step_dual = std_min(alphad, 1.0 - 1e-6);
        if is_centring {
            // A centring step stays well away from the boundary.
            self.step_primal = alphap * self.control.params.centring_alpha_scaling;
            self.step_dual = alphad * self.control.params.centring_alpha_scaling;
        }
    }

    fn make_step(&mut self, iterate: &mut Iterate<'_>, step: &Step) {
        self.make_step_centring(iterate, step, false);
    }

    fn make_step_centring(&mut self, iterate: &mut Iterate<'_>, step: &Step, is_centring: bool) {
        self.step_sizes_centring(iterate, step, is_centring);
        iterate.update(
            self.step_primal,
            Some(&step.x),
            Some(&step.xl),
            Some(&step.xu),
            self.step_dual,
            Some(&step.y),
            Some(&step.zl),
            Some(&step.zu),
        );
        if !is_centring {
            if std_min(self.step_primal, self.step_dual) < 0.05 {
                self.num_bad_iter += 1;
            } else {
                self.num_bad_iter = 0;
            }
            self.best_complementarity = std_min(self.best_complementarity, iterate.complementarity());
        }
    }

    fn solve_newton_system(&mut self, kkt: &mut dyn KktSolver, iterate: &Iterate<'_>, info: &mut Info, sl: &[f64], su: &[f64], step: &mut Step) {
        let model = iterate.model();
        let m = u(model.rows());
        let n = u(model.cols());
        let ai = model.ai();
        let (xl, xu, zl, zu) = (&iterate.xl, &iterate.xu, &iterate.zl, &iterate.zu);
        let rb = iterate.rb().clone();
        let rc = iterate.rc().clone();
        let rl = iterate.rl().clone();
        let ru = iterate.ru().clone();
        let mut rhs1 = vec![0.0; n + m];
        for j in 0..n + m {
            rhs1[j] = -rc[j];
        }
        for j in 0..n + m {
            if iterate.has_barrier_lb(j) {
                rhs1[j] += (sl[j] + zl[j] * rl[j]) / xl[j];
            }
            if iterate.has_barrier_ub(j) {
                rhs1[j] -= (su[j] - zu[j] * ru[j]) / xu[j];
            }
            if iterate.state_of(j) == State::Fixed {
                rhs1[j] = 0.0;
            }
        }
        let rhs2 = rb;
        let tol = self.control.params.kkt_tol * iterate.mu().sqrt();
        kkt.solve(&rhs1, &rhs2, tol, &mut step.x, &mut step.y, info);
        if info.errflag != 0 {
            return;
        }
        for dy in step.y.iter_mut() {
            *dy *= -1.0;
        }
        for j in 0..n + m {
            match iterate.state_of(j) {
                State::Fixed | State::Free => {
                    step.xl[j] = 0.0;
                    step.zl[j] = 0.0;
                }
                State::Barrier => {
                    step.xl[j] = step.x[j] - rl[j];
                    step.zl[j] = (sl[j] - zl[j] * step.xl[j]) / xl[j];
                }
            }
        }
        for j in 0..n + m {
            match iterate.state_of(j) {
                State::Fixed | State::Free => {
                    step.xu[j] = 0.0;
                    step.zu[j] = 0.0;
                }
                State::Barrier => {
                    step.xu[j] = ru[j] - step.x[j];
                    step.zu[j] = (su[j] - zu[j] * step.xu[j]) / xu[j];
                }
            }
        }
        for j in 0..n + m {
            if iterate.state_of(j) == State::Barrier {
                let atdy = dot_column(ai, j as Int, &step.y);
                let rcj = rc[j];
                if xl[j].is_finite() && xu[j].is_finite() {
                    if zl[j] * xu[j] >= zu[j] * xl[j] {
                        step.zl[j] = rcj + step.zu[j] - atdy;
                    } else {
                        step.zu[j] = -rcj + step.zl[j] + atdy;
                    }
                } else if xl[j].is_finite() {
                    step.zl[j] = rcj + step.zu[j] - atdy;
                } else {
                    step.zu[j] = -rcj + step.zl[j] + atdy;
                }
            }
        }
    }
}

/// The largest step in `[0, alpha]` keeping `x + step*dx` nonnegative,
/// and the index that blocks it (-1 if none).
fn step_to_boundary(x: &[f64], dx: &[f64], blocking_index: Option<&mut Int>, mut alpha: f64) -> f64 {
    let damp = 1.0 - f64::EPSILON;
    let mut iblock = -1;
    for i in 0..x.len() {
        if x[i] + alpha * dx[i] < 0.0 {
            alpha = -(x[i] * damp) / dx[i];
            iblock = i as Int;
        }
    }
    if let Some(blocking_index) = blocking_index {
        *blocking_index = iblock;
    }
    alpha
}
