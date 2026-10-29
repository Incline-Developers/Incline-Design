//! IPX's `Iterate` (`iterate.cc`): the primal-dual point of the interior
//! point method, the state of each variable, and its residuals,
//! objectives and complementarity, computed when first asked for.

use std::cell::{Ref, RefCell};

use super::{
    Info,
    model::Model,
    sparse::{Int, dot, dot_column, infnorm, multiply_add, u},
};

const BARRIER_MIN: f64 = 1e-30;
const INF: f64 = f64::INFINITY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Fixed,
    Free,
    Barrier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StateDetail {
    BarrierLb,
    BarrierUb,
    BarrierBoxed,
    Free,
    Fixed,
    ImpliedLb,
    ImpliedUb,
    ImpliedEq,
}

#[derive(Clone, Debug, Default)]
struct Evaluated {
    rb: Vec<f64>,
    rl: Vec<f64>,
    ru: Vec<f64>,
    rc: Vec<f64>,
    pobjective: f64,
    dobjective: f64,
    presidual: f64,
    dresidual: f64,
    offset: f64,
    complementarity: f64,
    mu: f64,
    mu_min: f64,
    mu_max: f64,
    evaluated: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Iterate<'a> {
    model: &'a Model,
    pub x: Vec<f64>,
    pub xl: Vec<f64>,
    pub xu: Vec<f64>,
    pub y: Vec<f64>,
    pub zl: Vec<f64>,
    pub zu: Vec<f64>,
    variable_state: Vec<StateDetail>,
    cache: RefCell<Evaluated>,
    postprocessed: bool,
    feasibility_tol: f64,
    optimality_tol: f64,
    start_crossover_tol: f64,
    pub bounds_measure: f64,
    pub costs_measure: f64,
}

impl<'a> Iterate<'a> {
    pub fn new(model: &'a Model) -> Self {
        let m = u(model.rows());
        let n = u(model.cols());
        let (lb, ub) = (model.lb(), model.ub());
        let mut it = Iterate {
            model,
            x: vec![0.0; n + m],
            xl: vec![0.0; n + m],
            xu: vec![0.0; n + m],
            y: vec![0.0; m],
            zl: vec![0.0; n + m],
            zu: vec![0.0; n + m],
            variable_state: vec![StateDetail::Free; n + m],
            cache: RefCell::new(Evaluated {
                rb: vec![0.0; m],
                rl: vec![0.0; n + m],
                ru: vec![0.0; n + m],
                rc: vec![0.0; n + m],
                ..Default::default()
            }),
            postprocessed: false,
            feasibility_tol: 1e-6,
            optimality_tol: 1e-8,
            start_crossover_tol: -1.0,
            bounds_measure: 1.0 + model.norm_bounds(),
            costs_measure: 1.0 + model.norm_c(),
        };
        for j in 0..n + m {
            let (state, xl, xu, zl, zu) = if lb[j].is_finite() && ub[j].is_finite() {
                (StateDetail::BarrierBoxed, 1.0, 1.0, 1.0, 1.0)
            } else if lb[j].is_finite() {
                (StateDetail::BarrierLb, 1.0, INF, 1.0, 0.0)
            } else if ub[j].is_finite() {
                (StateDetail::BarrierUb, INF, 1.0, 0.0, 1.0)
            } else {
                (StateDetail::Free, INF, INF, 0.0, 0.0)
            };
            it.variable_state[j] = state;
            it.xl[j] = xl;
            it.xu[j] = xu;
            it.zl[j] = zl;
            it.zu[j] = zu;
        }
        it
    }

    /// Start from the given point, every variable with a barrier for each
    /// finite bound (a fixed one has both).
    pub fn initialize(&mut self, x: Vec<f64>, xl: Vec<f64>, xu: Vec<f64>, y: Vec<f64>, zl: Vec<f64>, zu: Vec<f64>) {
        let (lb, ub) = (self.model.lb(), self.model.ub());
        self.x = x;
        self.xl = xl;
        self.xu = xu;
        self.y = y;
        self.zl = zl;
        self.zu = zu;
        for j in 0..self.x.len() {
            self.variable_state[j] = if lb[j] == ub[j] || (lb[j].is_finite() && ub[j].is_finite()) {
                StateDetail::BarrierBoxed
            } else if lb[j].is_finite() {
                StateDetail::BarrierLb
            } else if ub[j].is_finite() {
                StateDetail::BarrierUb
            } else {
                StateDetail::Free
            };
        }
        self.invalidate();
        self.postprocessed = false;
    }

    pub fn model(&self) -> &'a Model {
        self.model
    }

    fn invalidate(&mut self) {
        self.cache.get_mut().evaluated = false;
    }

    /// Step `sp` in the primal and `sd` in the dual direction, keeping
    /// barrier terms positive.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, sp: f64, dx: Option<&[f64]>, dxl: Option<&[f64]>, dxu: Option<&[f64]>, sd: f64, dy: Option<&[f64]>, dzl: Option<&[f64]>, dzu: Option<&[f64]>) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        if let Some(dx) = dx {
            for j in 0..n + m {
                if self.state_of(j) != State::Fixed {
                    self.x[j] += sp * dx[j];
                }
            }
        }
        if let Some(dxl) = dxl {
            for j in 0..n + m {
                if self.has_barrier_lb(j) {
                    self.xl[j] += sp * dxl[j];
                    self.xl[j] = std_max(self.xl[j], BARRIER_MIN);
                }
            }
        }
        if let Some(dxu) = dxu {
            for j in 0..n + m {
                if self.has_barrier_ub(j) {
                    self.xu[j] += sp * dxu[j];
                    self.xu[j] = std_max(self.xu[j], BARRIER_MIN);
                }
            }
        }
        if let Some(dy) = dy {
            for i in 0..m {
                self.y[i] += sd * dy[i];
            }
        }
        if let Some(dzl) = dzl {
            for j in 0..n + m {
                if self.has_barrier_lb(j) {
                    self.zl[j] += sd * dzl[j];
                    self.zl[j] = std_max(self.zl[j], BARRIER_MIN);
                }
            }
        }
        if let Some(dzu) = dzu {
            for j in 0..n + m {
                if self.has_barrier_ub(j) {
                    self.zu[j] += sd * dzu[j];
                    self.zu[j] = std_max(self.zu[j], BARRIER_MIN);
                }
            }
        }
        self.invalidate();
    }

    pub fn state_of(&self, j: usize) -> State {
        match self.variable_state[j] {
            StateDetail::Fixed => State::Fixed,
            StateDetail::Free | StateDetail::ImpliedLb | StateDetail::ImpliedUb | StateDetail::ImpliedEq => State::Free,
            _ => State::Barrier,
        }
    }

    pub fn has_barrier_lb(&self, j: usize) -> bool {
        matches!(self.variable_state[j], StateDetail::BarrierLb | StateDetail::BarrierBoxed)
    }

    pub fn has_barrier_ub(&self, j: usize) -> bool {
        matches!(self.variable_state[j], StateDetail::BarrierUb | StateDetail::BarrierBoxed)
    }

    pub fn is_implied(&self, j: usize) -> bool {
        matches!(self.variable_state[j], StateDetail::ImpliedLb | StateDetail::ImpliedUb | StateDetail::ImpliedEq)
    }

    pub fn make_fixed(&mut self, j: usize) {
        self.xl[j] = 0.0;
        self.xu[j] = 0.0;
        self.zl[j] = 0.0;
        self.zu[j] = 0.0;
        self.variable_state[j] = StateDetail::Fixed;
        self.invalidate();
    }

    pub fn make_fixed_at(&mut self, j: usize, value: f64) {
        self.x[j] = value;
        self.make_fixed(j);
    }

    pub fn make_implied_lb(&mut self, j: usize) {
        self.xl[j] = INF;
        self.xu[j] = INF;
        self.variable_state[j] = StateDetail::ImpliedLb;
        self.invalidate();
    }

    pub fn make_implied_ub(&mut self, j: usize) {
        self.xl[j] = INF;
        self.xu[j] = INF;
        self.variable_state[j] = StateDetail::ImpliedUb;
        self.invalidate();
    }

    pub fn make_implied_eq(&mut self, j: usize) {
        self.xl[j] = INF;
        self.xu[j] = INF;
        self.zl[j] = 0.0;
        self.zu[j] = 0.0;
        self.variable_state[j] = StateDetail::ImpliedEq;
        self.invalidate();
    }

    /// The column's scaling factor in the normal equations: 0 if fixed,
    /// infinite if free, otherwise `1/sqrt(zl/xl + zu/xu)`.
    pub fn scaling_factor(&self, j: usize) -> f64 {
        match self.state_of(j) {
            State::Fixed => 0.0,
            State::Free => INF,
            State::Barrier => {
                let g = self.zl[j] / self.xl[j] + self.zu[j] / self.xu[j];
                1.0 / g.sqrt()
            }
        }
    }

    fn evaluated(&self) -> Ref<'_, Evaluated> {
        if !self.cache.borrow().evaluated {
            let mut cache = self.cache.borrow_mut();
            self.compute_residuals(&mut cache);
            self.compute_objectives(&mut cache);
            self.compute_complementarity(&mut cache);
            cache.evaluated = true;
        }
        self.cache.borrow()
    }

    pub fn rb(&self) -> Ref<'_, Vec<f64>> {
        Ref::map(self.evaluated(), |e| &e.rb)
    }
    pub fn rl(&self) -> Ref<'_, Vec<f64>> {
        Ref::map(self.evaluated(), |e| &e.rl)
    }
    pub fn ru(&self) -> Ref<'_, Vec<f64>> {
        Ref::map(self.evaluated(), |e| &e.ru)
    }
    pub fn rc(&self) -> Ref<'_, Vec<f64>> {
        Ref::map(self.evaluated(), |e| &e.rc)
    }
    pub fn pobjective(&self) -> f64 {
        self.evaluated().pobjective
    }
    pub fn dobjective(&self) -> f64 {
        self.evaluated().dobjective
    }
    pub fn pobjective_after_postproc(&self) -> f64 {
        let e = self.evaluated();
        e.pobjective + e.offset
    }
    pub fn dobjective_after_postproc(&self) -> f64 {
        let e = self.evaluated();
        e.dobjective + e.offset
    }
    pub fn presidual(&self) -> f64 {
        self.evaluated().presidual
    }
    pub fn dresidual(&self) -> f64 {
        self.evaluated().dresidual
    }
    pub fn complementarity(&self) -> f64 {
        self.evaluated().complementarity
    }
    pub fn mu(&self) -> f64 {
        self.evaluated().mu
    }
    pub fn mu_min(&self) -> f64 {
        self.evaluated().mu_min
    }
    pub fn mu_max(&self) -> f64 {
        self.evaluated().mu_max
    }

    pub fn feasible(&self) -> bool {
        let e = self.evaluated();
        e.presidual <= self.feasibility_tol * self.bounds_measure && e.dresidual <= self.feasibility_tol * self.costs_measure
    }

    pub fn optimal(&self) -> bool {
        let pobj = self.pobjective_after_postproc();
        let dobj = self.dobjective_after_postproc();
        let ave_obj = 0.5 * (pobj + dobj);
        let gap = pobj - dobj;
        gap.abs() <= self.optimality_tol * (1.0 + ave_obj.abs())
    }

    pub fn term_crit_reached(&self) -> bool {
        if self.feasible() && self.optimal() {
            if self.start_crossover_tol <= 0.0 {
                return true;
            }
            let (pres, dres) = self.residuals_from_dropping();
            if pres <= self.start_crossover_tol * (1.0 + self.model.norm_bounds()) && dres <= self.start_crossover_tol * (1.0 + self.model.norm_c()) {
                return true;
            }
        }
        false
    }

    pub fn feasibility_tol(&self) -> f64 {
        self.feasibility_tol
    }
    pub fn optimality_tol(&self) -> f64 {
        self.optimality_tol
    }
    pub fn set_feasibility_tol(&mut self, tol: f64) {
        self.feasibility_tol = tol;
    }
    pub fn set_optimality_tol(&mut self, tol: f64) {
        self.optimality_tol = tol;
    }
    pub fn set_start_crossover_tol(&mut self, tol: f64) {
        self.start_crossover_tol = tol;
    }

    /// Put fixed and implied variables back at their bounds with duals
    /// that make them complementary.
    pub fn postprocess(&mut self) {
        let model = self.model;
        let m = u(model.rows());
        let n = u(model.cols());
        let (c, lb, ub, ai) = (model.c(), model.lb(), model.ub(), model.ai());
        for j in 0..n + m {
            if self.state_of(j) == State::Fixed {
                self.xl[j] = self.x[j] - lb[j];
                self.xu[j] = ub[j] - self.x[j];
                if lb[j] == ub[j] {
                    let z = c[j] - dot_column(ai, j as Int, &self.y);
                    if z >= 0.0 {
                        self.zl[j] = z;
                    } else {
                        self.zu[j] = -z;
                    }
                }
            }
        }
        for j in 0..n + m {
            if self.is_implied(j) {
                let z = c[j] - dot_column(ai, j as Int, &self.y);
                match self.variable_state[j] {
                    StateDetail::ImpliedEq => {
                        if z >= 0.0 {
                            self.zl[j] = z;
                            self.zu[j] = 0.0;
                        } else {
                            self.zl[j] = 0.0;
                            self.zu[j] = -z;
                        }
                        self.x[j] = lb[j];
                    }
                    StateDetail::ImpliedLb => {
                        self.zl[j] = z;
                        self.zu[j] = 0.0;
                        self.x[j] = lb[j];
                    }
                    _ => {
                        self.zl[j] = 0.0;
                        self.zu[j] = -z;
                        self.x[j] = ub[j];
                    }
                }
                self.xl[j] = self.x[j] - lb[j];
                self.xu[j] = ub[j] - self.x[j];
            }
        }
        self.postprocessed = true;
        self.invalidate();
    }

    pub fn evaluate_postsolved(&self, info: &mut Info) {
        self.model.evaluate_interior_solution(&self.x, &self.xl, &self.xu, &self.y, &self.zl, &self.zu, info);
    }

    /// A complementary primal-dual point near the final iterate, for
    /// crossover to start from.
    pub fn drop_to_complementarity(&self, x: &mut [f64], y: &mut [f64], z: &mut [f64]) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let (lb, ub) = (self.model.lb(), self.model.ub());
        y.copy_from_slice(&self.y);
        for j in 0..n + m {
            let (xlj, xuj, zlj, zuj) = (self.xl[j], self.xu[j], self.zl[j], self.zu[j]);
            let mut xj = self.x[j];
            xj = std_max(xj, lb[j]);
            xj = std_min(xj, ub[j]);
            if lb[j] == ub[j] {
                x[j] = lb[j];
                z[j] = zlj - zuj;
            } else if lb[j].is_finite() && ub[j].is_finite() {
                if zlj * xuj >= zuj * xlj {
                    if zlj >= xlj {
                        x[j] = lb[j];
                        z[j] = std_max(0.0, zlj - zuj);
                    } else {
                        x[j] = xj;
                        z[j] = 0.0;
                    }
                } else if zuj >= xuj {
                    x[j] = ub[j];
                    z[j] = std_min(0.0, zlj - zuj);
                } else {
                    x[j] = xj;
                    z[j] = 0.0;
                }
            } else if lb[j].is_finite() {
                if zlj >= xlj {
                    x[j] = lb[j];
                    z[j] = std_max(0.0, zlj - zuj);
                } else {
                    x[j] = xj;
                    z[j] = 0.0;
                }
            } else if ub[j].is_finite() {
                if zuj >= xuj {
                    x[j] = ub[j];
                    z[j] = std_min(0.0, zlj - zuj);
                } else {
                    x[j] = xj;
                    z[j] = 0.0;
                }
            } else {
                x[j] = xj;
                z[j] = 0.0;
            }
        }
    }

    /// The largest primal and dual residuals that dropping to
    /// complementarity would cause.
    fn residuals_from_dropping(&self) -> (f64, f64) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let (ai, lb, ub) = (self.model.ai(), self.model.lb(), self.model.ub());
        let mut presmax = 0.0;
        let mut dresmax = 0.0;
        for j in 0..n + m {
            let mut xdrop = 0.0;
            let mut zdrop = 0.0;
            match self.variable_state[j] {
                StateDetail::BarrierLb => {
                    if self.zl[j] >= self.xl[j] {
                        xdrop = self.x[j] - lb[j];
                    } else {
                        zdrop = self.zl[j] - self.zu[j];
                    }
                }
                StateDetail::BarrierUb => {
                    if self.zu[j] >= self.xu[j] {
                        xdrop = self.x[j] - ub[j];
                    } else {
                        zdrop = self.zl[j] - self.zu[j];
                    }
                }
                StateDetail::BarrierBoxed => {
                    if self.zl[j] / self.xl[j] >= self.zu[j] / self.xu[j] {
                        if self.zl[j] >= self.xl[j] {
                            xdrop = self.x[j] - lb[j];
                        } else {
                            zdrop = self.zl[j] - self.zu[j];
                        }
                    } else if self.zu[j] >= self.xu[j] {
                        xdrop = self.x[j] - ub[j];
                    } else {
                        zdrop = self.zl[j] - self.zu[j];
                    }
                }
                _ => {}
            }
            let mut amax = 0.0;
            for p in ai.begin(j as Int)..ai.begin(j as Int + 1) {
                amax = std_max(amax, ai.value(p).abs());
            }
            presmax = std_max(presmax, f64::abs(xdrop) * amax);
            dresmax = std_max(dresmax, f64::abs(zdrop));
        }
        (presmax, dresmax)
    }

    fn compute_residuals(&self, e: &mut Evaluated) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let (lb, ub, ai) = (self.model.lb(), self.model.ub(), self.model.ai());
        e.rb.copy_from_slice(self.model.b());
        multiply_add(ai, &self.x, -1.0, &mut e.rb, 'N');
        let c = self.model.c();
        for j in 0..n + m {
            e.rc[j] = c[j] - self.zl[j] + self.zu[j];
        }
        multiply_add(ai, &self.y, -1.0, &mut e.rc, 'T');
        if !self.postprocessed {
            for (j, rc_j) in e.rc[..n + m].iter_mut().enumerate() {
                if self.state_of(j) == State::Fixed {
                    *rc_j = 0.0;
                }
            }
        }
        for j in 0..n + m {
            e.rl[j] = if self.has_barrier_lb(j) { lb[j] - self.x[j] + self.xl[j] } else { 0.0 };
            e.ru[j] = if self.has_barrier_ub(j) { ub[j] - self.x[j] - self.xu[j] } else { 0.0 };
        }
        e.presidual = infnorm(&e.rb);
        e.dresidual = infnorm(&e.rc);
        e.presidual = std_max(e.presidual, infnorm(&e.rl));
        e.presidual = std_max(e.presidual, infnorm(&e.ru));
    }

    fn compute_objectives(&self, e: &mut Evaluated) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        let (b, c, lb, ub, ai) = (self.model.b(), self.model.c(), self.model.lb(), self.model.ub(), self.model.ai());
        if self.postprocessed {
            e.offset = 0.0;
            e.pobjective = self.model.offset() + dot(c, &self.x);
            e.dobjective = self.model.offset() + dot(b, &self.y);
            for j in 0..n + m {
                if lb[j].is_finite() {
                    e.dobjective += lb[j] * self.zl[j];
                }
                if ub[j].is_finite() {
                    e.dobjective -= ub[j] * self.zu[j];
                }
            }
        } else {
            e.offset = 0.0;
            e.pobjective = self.model.offset();
            for j in 0..n + m {
                if self.state_of(j) != State::Fixed {
                    e.pobjective += c[j] * self.x[j];
                } else {
                    e.offset += c[j] * self.x[j];
                }
                if self.is_implied(j) {
                    e.pobjective -= (self.zl[j] - self.zu[j]) * self.x[j];
                    e.offset += (self.zl[j] - self.zu[j]) * self.x[j];
                }
            }
            e.dobjective = self.model.offset() + dot(b, &self.y);
            for j in 0..n + m {
                if self.has_barrier_lb(j) {
                    e.dobjective += lb[j] * self.zl[j];
                }
                if self.has_barrier_ub(j) {
                    e.dobjective -= ub[j] * self.zu[j];
                }
                if self.state_of(j) == State::Fixed {
                    e.dobjective -= self.x[j] * dot_column(ai, j as Int, &self.y);
                }
            }
        }
    }

    fn compute_complementarity(&self, e: &mut Evaluated) {
        let m = u(self.model.rows());
        let n = u(self.model.cols());
        e.complementarity = 0.0;
        e.mu_min = INF;
        e.mu_max = 0.0;
        let mut num_finite = 0;
        for j in 0..n + m {
            if self.has_barrier_lb(j) {
                let product = self.xl[j] * self.zl[j];
                e.complementarity += product;
                e.mu_min = std_min(e.mu_min, product);
                e.mu_max = std_max(e.mu_max, product);
                num_finite += 1;
            }
        }
        for j in 0..n + m {
            if self.has_barrier_ub(j) {
                let product = self.xu[j] * self.zu[j];
                e.complementarity += product;
                e.mu_min = std_min(e.mu_min, product);
                e.mu_max = std_max(e.mu_max, product);
                num_finite += 1;
            }
        }
        if num_finite > 0 {
            e.mu = e.complementarity / num_finite as f64;
        } else {
            e.mu = 0.0;
            e.mu_min = 0.0;
        }
    }
}

/// `std::max(a, b)`.
pub(crate) fn std_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`.
pub(crate) fn std_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}
