//! The narrow SCIP boundary: the few operations russcip's safe API does not
//! cover, plus the solver settings this investigation found to be
//! load-bearing.
//!
//! Deliberately small. russcip 0.10 covers variables, linear constraints,
//! quadratic (bilinear) constraints, solutions, bounds, status and time
//! limits; only the three items below need raw calls, and each is wrapped
//! once here rather than scattered through the formulation.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use russcip::{Event, EventMask, Eventhdlr, Model, ProblemCreated, SCIPEventhdlr, Solved, Solving, ffi, prelude::*};

/// SCIP build identification, recorded with every benchmark row.
pub(crate) fn version() -> String {
    // SCIPversion/SCIPtechVersion avoid parsing the banner.
    unsafe {
        let major = ffi::SCIPmajorVersion();
        let minor = ffi::SCIPminorVersion();
        let tech = ffi::SCIPtechVersion();
        format!("SCIP {major}.{minor}.{tech}")
    }
}

/// Observable proof that cancellation reached SCIP, not just the Rust worker.
#[derive(Default)]
pub(crate) struct InterruptAudit {
    pub(crate) callbacks: AtomicU64,
    pub(crate) calls: AtomicU64,
    pub(crate) failed: AtomicBool,
}

struct CancelOnEvent {
    signal: Arc<AtomicBool>,
    audit: Arc<InterruptAudit>,
}

impl Eventhdlr for CancelOnEvent {
    fn get_type(&self) -> EventMask {
        EventMask::PRESOLVE_ROUND | EventMask::NODE_FOCUSED | EventMask::LP_SOLVED | EventMask::BEST_SOL_FOUND
    }

    fn execute(&mut self, model: Model<Solving>, _handler: SCIPEventhdlr, _event: Event) {
        self.audit.callbacks.fetch_add(1, Ordering::Relaxed);
        if !self.signal.load(Ordering::Acquire) {
            return;
        }
        // SAFETY: russcip invokes this callback synchronously on the thread
        // executing this model's solve. Its borrowed Model<Solving> keeps the
        // SCIP pointer valid for this callback; no pointer or model crosses
        // threads or survives model teardown. SCIP 10.1.0 permits this call
        // during presolving and solving. The UI only writes the atomic flag.
        let code = unsafe { ffi::SCIPinterruptSolve(model.scip_ptr()) };
        if code == ffi::SCIP_Retcode_SCIP_OKAY {
            self.audit.calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.audit.failed.store(true, Ordering::Release);
        }
    }
}

pub(crate) fn install_cancellation(model: &mut Model<ProblemCreated>, signal: Arc<AtomicBool>, audit: Arc<InterruptAudit>) {
    model.include_eventhdlr(
        "incline_cancel",
        "interrupt from the worker thread after a cancellation request",
        Box::new(CancelOnEvent { signal, audit }),
    );
}

/// Relative MIP gap of a finished solve. Not exposed by russcip.
///
/// SCIP returns `+inf` when there is no incumbent; that is reported as
/// `None` so "no incumbent" stays distinct from "gap zero".
pub(crate) fn gap(solved: &Model<Solved>) -> Option<f64> {
    if solved.n_sols() == 0 {
        return None;
    }
    let raw = unsafe { ffi::SCIPgetGap(solved.scip_ptr()) };
    raw.is_finite().then_some(raw)
}

/// Solver settings this investigation established, rather than guessed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SolveTuning {
    /// Time limit handed to SCIP as `limits/time`; preparation and replay
    /// are outside this limit, and solve return can overshoot it.
    pub(crate) time: Option<Duration>,
    /// Whether a feasible starting solution will be supplied. This is not
    /// cosmetic - see the note below.
    pub(crate) seeded: bool,
}

impl SolveTuning {
    pub(crate) fn new(time: Option<Duration>) -> Self {
        Self { time, seeded: false }
    }

    pub(crate) fn seeded(mut self, seeded: bool) -> Self {
        self.seeded = seeded;
        self
    }

    /// Apply the settings.
    ///
    /// # The `allowweakdualreds` setting is a correctness fix, not a tuning knob
    ///
    /// Supplying a feasible starting solution to a model containing nonlinear
    /// constraints makes SCIP 10.0.2 terminate at the *seeded* objective and
    /// report it as `Optimal`, with a dual bound equal to that same value and
    /// zero branch-and-bound nodes. The better solution is simply never found.
    ///
    /// The reproducer is three variables and two constraints - maximise `c`
    /// subject to `c * t = m`, `t = 10 + 10 b`, `m = 6`, `b` binary. The true
    /// optimum is `c = 0.6` at `b = 0`, and an unseeded solve finds it. Seed
    /// the feasible `b = 1` point (`c = 0.3`) and the solve returns 0.3 as
    /// proven optimal.
    ///
    /// Measured behaviour of the reproducer:
    ///
    /// | configuration | result |
    /// |---|---|
    /// | no seed | 0.6, correct |
    /// | seeded, defaults | **0.3 reported optimal, bound 0.3, 0 nodes** |
    /// | seeded, `presolving/maxrounds = 0` | 0.6, correct |
    /// | seeded, `misc/allowweakdualreds = false` | 0.6, correct |
    /// | seeded, `misc/allowstrongdualreds = false` | still 0.3 |
    ///
    /// It is sign-independent (identical minimising a negated objective) and
    /// it does **not** occur on a purely linear MILP seeded the same way, so
    /// the interaction belongs to the nonlinear presolve path - precisely the
    /// path a blended formulation depends on.
    ///
    /// Because the corrupted run also reports a zero gap, no status check can
    /// detect it. Disabling weak dual reductions whenever a seed is supplied
    /// is therefore the conservative choice. Independent replay checks
    /// feasibility; it cannot detect a false optimality certificate.
    pub(crate) fn apply(&self, model: Model<ProblemCreated>) -> Model<ProblemCreated> {
        let mut model = model;
        if let Some(time) = self.time {
            model = model.set_real_param("limits/time", time.as_secs_f64()).expect("limits/time is a real parameter");
        }
        if self.seeded {
            model = model.set_bool_param("misc/allowweakdualreds", false).expect("misc/allowweakdualreds is a bool parameter");
        }
        model
    }
}

/// Everything the benchmark rows record about one finished solve, read once
/// so the `Model<Solved>` need not be threaded through reporting code.
#[derive(Clone, Debug)]
pub(crate) struct SolveReport {
    pub(crate) status: Status,
    pub(crate) objective: Option<f64>,
    pub(crate) bound: f64,
    pub(crate) gap: Option<f64>,
    pub(crate) nodes: usize,
    pub(crate) solve_time: f64,
}

impl SolveReport {
    pub(crate) fn read(solved: &Model<Solved>) -> Self {
        let has_incumbent = solved.n_sols() > 0;
        Self {
            status: solved.status(),
            objective: has_incumbent.then(|| solved.obj_val()),
            bound: solved.best_bound(),
            gap: gap(solved),
            nodes: solved.n_nodes(),
            solve_time: solved.solving_time(),
        }
    }
}
