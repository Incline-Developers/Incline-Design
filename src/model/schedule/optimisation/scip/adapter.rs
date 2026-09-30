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
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use russcip::{Event, EventMask, Eventhdlr, Model, ProblemCreated, SCIPEventhdlr, Solved, Solving, ffi, prelude::*};

use crate::model::schedule::result::SolveDiagnostics;

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
    progress: Mutex<SolveDiagnostics>,
}

struct CancelOnEvent {
    signal: Arc<AtomicBool>,
    audit: Arc<InterruptAudit>,
}

impl Eventhdlr for CancelOnEvent {
    fn get_type(&self) -> EventMask {
        EventMask::PRESOLVE_ROUND | EventMask::NODE_FOCUSED | EventMask::NODE_SOLVED | EventMask::FIRST_LP_SOLVED | EventMask::LP_SOLVED | EventMask::SOL_FOUND
    }

    fn execute(&mut self, model: Model<Solving>, _handler: SCIPEventhdlr, event: Event) {
        self.audit.callbacks.fetch_add(1, Ordering::Relaxed);
        record_progress(&model, event.event_type(), &self.audit);
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

/// Reads SCIP only on its solving thread, never from the UI. Observation must
/// not construct an LP, change parameters, or otherwise alter the solve.
fn record_progress(model: &Model<Solving>, event: EventMask, audit: &InterruptAudit) {
    let mut progress = audit.progress.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let scip = model.scip_ptr();
    // SAFETY: synchronous solver callback. Timing is valid during presolve
    // and solve; depth and LP access are restricted to solving-stage events.
    unsafe {
        let elapsed = ffi::SCIPgetSolvingTime(scip);
        if event.matches(EventMask::SOL_FOUND) {
            progress.first_incumbent_s.get_or_insert(elapsed);
            if event.matches(EventMask::BEST_SOL_FOUND) {
                progress.incumbent_improvements += 1;
            }
        }
        if event.matches(EventMask::PRESOLVE_ROUND) {
            progress.presolve_rounds += 1;
        }
        if event.matches(EventMask::NODE_FOCUSED) {
            if ffi::SCIPgetDepth(scip) == 0 && progress.presolved_variables.is_none() {
                progress.presolved_variables = Some(ffi::SCIPgetNVars(scip).max(0) as usize);
                progress.presolved_constraints = Some(ffi::SCIPgetNConss(scip).max(0) as usize);
            } else if ffi::SCIPgetDepth(scip) > 0 {
                progress.root_node_finished = true;
            }
        }
        if event.matches(EventMask::NODE_SOLVED) && ffi::SCIPgetDepth(scip) == 0 {
            progress.root_node_finished = true;
        }
        if event.matches(EventMask::LP_EVENT) && ffi::SCIPgetDepth(scip) == 0 {
            let status = ffi::SCIPgetLPSolstat(scip);
            let completed = matches!(
                status,
                ffi::SCIP_LPSolStat_SCIP_LPSOLSTAT_OPTIMAL
                    | ffi::SCIP_LPSolStat_SCIP_LPSOLSTAT_INFEASIBLE
                    | ffi::SCIP_LPSolStat_SCIP_LPSOLSTAT_UNBOUNDEDRAY
                    | ffi::SCIP_LPSolStat_SCIP_LPSOLSTAT_OBJLIMIT
            );
            if completed {
                if event.matches(EventMask::FIRST_LP_SOLVED) {
                    progress.root_initial_lp_s.get_or_insert(elapsed);
                }
                if event.matches(EventMask::LP_SOLVED) {
                    progress.root_cut_price_s.get_or_insert(elapsed);
                }
                if progress.root_lp_rows.is_none() {
                    let count = ffi::SCIPgetNLPRows(scip).max(0) as usize;
                    progress.root_lp_rows = Some(count);
                    progress.root_lp_columns = Some(ffi::SCIPgetNLPCols(scip).max(0) as usize);
                    let rows = ffi::SCIPgetLPRows(scip);
                    // Count once, O(rows), without reading every coefficient.
                    // A zero-row LP may expose a null pointer; never slice it.
                    let mut nonzeros = 0;
                    for index in 0..count {
                        nonzeros += ffi::SCIProwGetNNonz(*rows.add(index)).max(0) as usize;
                    }
                    progress.root_lp_nonzeros = Some(nonzeros);
                }
            }
        }
    }
}

pub(crate) fn diagnostics(solved: &Model<Solved>, audit: &InterruptAudit) -> SolveDiagnostics {
    let mut progress = audit.progress.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    // SAFETY: a limit can return Model<Solved> while SCIP is still in presolve.
    // Guard stage-restricted statistics accordingly. LP matrix getters are
    // solving-only, so their snapshots are captured in the callback above.
    unsafe {
        let scip = solved.scip_ptr();
        let stage = ffi::SCIPgetStage(scip);
        if (ffi::SCIP_Stage_SCIP_STAGE_INITPRESOLVE..=ffi::SCIP_Stage_SCIP_STAGE_SOLVED).contains(&stage) {
            progress.presolve_s = ffi::SCIPgetPresolvingTime(scip);
        }
        if matches!(
            stage,
            ffi::SCIP_Stage_SCIP_STAGE_PRESOLVING | ffi::SCIP_Stage_SCIP_STAGE_PRESOLVED | ffi::SCIP_Stage_SCIP_STAGE_SOLVING | ffi::SCIP_Stage_SCIP_STAGE_SOLVED
        ) {
            progress.lp_iterations = ffi::SCIPgetNLPIterations(scip).max(0) as u64;
        }
        if matches!(
            stage,
            ffi::SCIP_Stage_SCIP_STAGE_PRESOLVED | ffi::SCIP_Stage_SCIP_STAGE_SOLVING | ffi::SCIP_Stage_SCIP_STAGE_SOLVED
        ) {
            progress.root_lp_iterations = ffi::SCIPgetNRootLPIterations(scip).max(0) as u64;
        }
        progress.final_variables = ffi::SCIPgetNVars(scip).max(0) as usize;
        progress.final_constraints = ffi::SCIPgetNConss(scip).max(0) as usize;
        progress.final_nonzeros = ffi::SCIPgetNNZs(scip).max(0) as u64;
    }
    progress.nodes = solved.n_nodes();
    progress
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
    if solved.n_sols() == 0 || bound(solved).is_none() {
        return None;
    }
    let raw = unsafe { ffi::SCIPgetGap(solved.scip_ptr()) };
    (raw.is_finite() && !is_infinity(solved, raw)).then_some(raw)
}

/// The dual bound, or `None` while SCIP has none. SCIP reports "no bound" as
/// its own infinity (1e20 by default), which is a finite `f64` - so a solve
/// stopped before its first root LP used to publish 1e20 as a bound.
pub(crate) fn bound(solved: &Model<Solved>) -> Option<f64> {
    let raw = solved.best_bound();
    (raw.is_finite() && !is_infinity(solved, raw)).then_some(raw)
}

fn is_infinity(solved: &Model<Solved>, value: f64) -> bool {
    // SAFETY: a read of the model's own numerics settings.
    unsafe { ffi::SCIPisInfinity(solved.scip_ptr(), value.abs()) != 0 }
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
    /// Infinite while SCIP has no dual bound.
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
            bound: bound(solved).unwrap_or(f64::INFINITY),
            gap: gap(solved),
            nodes: solved.n_nodes(),
            solve_time: solved.solving_time(),
        }
    }
}
