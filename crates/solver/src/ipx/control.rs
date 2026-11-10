//! IPX's `Control` and `Parameters`: its options, as HiGHS sets them, and
//! the interrupt check its loops make.

use web_time::Instant;

use super::{IPX_ERROR_TIME_INTERRUPT, IPX_ERROR_USER_INTERRUPT, sparse::Int};
use crate::simplex::ekk::Interrupt;

/// IPX's parameters, with HiGHS's values.
#[derive(Clone, Debug)]
pub(crate) struct Parameters {
    pub time_limit: f64,
    pub dualize: Int,
    pub scale: Int,
    pub ipm_maxiter: Int,
    pub ipm_feasibility_tol: f64,
    pub ipm_optimality_tol: f64,
    pub ipm_drop_primal: f64,
    pub ipm_drop_dual: f64,
    pub kkt_tol: f64,
    pub crash_basis: Int,
    pub dependency_tol: f64,
    pub volume_tol: f64,
    pub rows_per_slice: Int,
    pub maxskip_updates: Int,
    pub lu_pivottol: f64,
    pub run_crossover: Int,
    pub start_crossover_tol: f64,
    /// Crossover after the interior point method stops making progress.
    pub crossover_on_no_progress: bool,
    pub pfeasibility_tol: f64,
    pub dfeasibility_tol: f64,
    pub switchiter: Int,
    pub stop_at_switch: Int,
    pub update_heuristic: Int,
    pub maxpasses: Int,
    pub run_centring: Int,
    pub max_centring_steps: Int,
    pub centring_ratio_tolerance: f64,
    pub centring_ratio_reduction: f64,
    pub centring_alpha_scaling: f64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            time_limit: -1.0,
            dualize: -1,
            scale: 1,
            ipm_maxiter: 300,
            ipm_feasibility_tol: 1e-6,
            ipm_optimality_tol: 1e-8,
            ipm_drop_primal: 1e-9,
            ipm_drop_dual: 1e-9,
            kkt_tol: 0.3,
            crash_basis: 1,
            dependency_tol: 1e-6,
            volume_tol: 2.0,
            rows_per_slice: 10000,
            maxskip_updates: 10,
            lu_pivottol: 0.0625,
            run_crossover: 1,
            start_crossover_tol: 1e-8,
            crossover_on_no_progress: false,
            pfeasibility_tol: 1e-7,
            dfeasibility_tol: 1e-7,
            switchiter: -1,
            stop_at_switch: 0,
            update_heuristic: 1,
            maxpasses: -1,
            run_centring: 0,
            max_centring_steps: 5,
            centring_ratio_tolerance: 100.0,
            centring_ratio_reduction: 1.5,
            centring_alpha_scaling: 0.5,
        }
    }
}

pub(crate) struct Control {
    pub params: Parameters,
    started: Instant,
    /// Time already spent before IPX started (HiGHS's timer offset).
    offset: f64,
    interrupt: Option<Interrupt>,
}

impl Control {
    pub fn new(params: Parameters, interrupt: Option<Interrupt>) -> Self {
        Self {
            params,
            started: Instant::now(),
            offset: 0.0,
            interrupt,
        }
    }

    pub fn set_timer_offset(&mut self, offset: f64) {
        self.offset = offset;
    }

    pub fn reset_timer(&mut self) {
        self.started = Instant::now();
    }

    pub fn elapsed(&self) -> f64 {
        self.offset + self.started.elapsed().as_secs_f64()
    }

    /// IPX's `InterruptCheck`: a time limit or an interrupt asked for.
    pub fn interrupt_check(&self) -> Int {
        if self.params.time_limit >= 0.0 && self.params.time_limit < self.elapsed() {
            return IPX_ERROR_TIME_INTERRUPT;
        }
        if self.interrupt.as_ref().is_some_and(|interrupt| interrupt()) {
            return IPX_ERROR_USER_INTERRUPT;
        }
        0
    }
}
