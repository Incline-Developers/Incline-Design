//! The narrow SCIP boundary: progress and cancellation from inside the
//! solve, the dump for the reference SCIP, and the bound and gap read so
//! that "none" stays distinct from SCIP's infinity.
//!
//! Deliberately small. [`minlp::model`] covers variables, linear,
//! indicator and quadratic (bilinear) constraints, solutions, bounds,
//! status and parameters; the few things below are wrapped once here rather
//! than scattered through the formulation.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use minlp::{
    event::{
        EVENTTYPE_BESTSOLFOUND, EVENTTYPE_FIRSTLPSOLVED, EVENTTYPE_LPEVENT, EVENTTYPE_LPSOLVED, EVENTTYPE_NODEFOCUSED, EVENTTYPE_NODESOLVED, EVENTTYPE_PRESOLVEROUND,
        EVENTTYPE_SOLFOUND, EventType,
    },
    model::{Model, Solving},
    scip::Stage,
    types::{LpSolStat, Status},
};

use crate::model::schedule::result::SolveDiagnostics;

/// Solver identification, recorded with every benchmark row.
pub(crate) fn version() -> String {
    "SCIP 10.0.2 (minlp)".to_owned()
}

/// Observable proof that cancellation reached SCIP, not just the Rust worker.
#[derive(Default)]
pub(crate) struct InterruptAudit {
    pub(crate) callbacks: AtomicU64,
    pub(crate) calls: AtomicU64,
    pub(crate) failed: AtomicBool,
    progress: Mutex<SolveDiagnostics>,
}

/// The events progress is read on, and cancellation polled at.
const EVENTS: EventType = EVENTTYPE_PRESOLVEROUND | EVENTTYPE_NODEFOCUSED | EVENTTYPE_NODESOLVED | EVENTTYPE_FIRSTLPSOLVED | EVENTTYPE_LPSOLVED | EVENTTYPE_SOLFOUND;

/// Reads SCIP only on its solving thread, never from the UI. Observation must
/// not construct an LP, change parameters, or otherwise alter the solve.
fn record_progress(solving: &Solving, event: EventType, audit: &InterruptAudit) {
    let mut progress = audit.progress.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let matches = |mask: EventType| event & mask != 0;
    // Timing is valid during presolve and solve; depth and LP access are
    // restricted to solving-stage events.
    let elapsed = solving.solving_time();
    if matches(EVENTTYPE_SOLFOUND) {
        progress.first_incumbent_s.get_or_insert(elapsed);
        if matches(EVENTTYPE_BESTSOLFOUND) {
            progress.incumbent_improvements += 1;
        }
    }
    if matches(EVENTTYPE_PRESOLVEROUND) {
        progress.presolve_rounds += 1;
    }
    if matches(EVENTTYPE_NODEFOCUSED) {
        if solving.depth() == 0 && progress.presolved_variables.is_none() {
            progress.presolved_variables = Some(solving.n_vars());
            progress.presolved_constraints = Some(solving.n_conss());
        } else if solving.depth() > 0 {
            progress.root_node_finished = true;
        }
    }
    if matches(EVENTTYPE_NODESOLVED) && solving.depth() == 0 {
        progress.root_node_finished = true;
    }
    if matches(EVENTTYPE_LPEVENT) && solving.depth() == 0 {
        let completed = matches!(
            solving.lp_solstat(),
            LpSolStat::Optimal | LpSolStat::Infeasible | LpSolStat::UnboundedRay | LpSolStat::ObjLimit
        );
        if completed {
            if matches(EVENTTYPE_FIRSTLPSOLVED) {
                progress.root_initial_lp_s.get_or_insert(elapsed);
            }
            if matches(EVENTTYPE_LPSOLVED) {
                progress.root_cut_price_s.get_or_insert(elapsed);
            }
            if progress.root_lp_rows.is_none() {
                progress.root_lp_rows = Some(solving.n_lp_rows());
                progress.root_lp_columns = Some(solving.n_lp_cols());
                // Count once, O(rows), without reading every coefficient.
                progress.root_lp_nonzeros = Some(solving.lp_nonzeros());
            }
        }
    }
}

pub(crate) fn diagnostics(solved: &Model, audit: &InterruptAudit) -> SolveDiagnostics {
    let mut progress = audit.progress.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    // A limit can stop SCIP while it is still in presolve. Guard
    // stage-restricted statistics accordingly. LP matrix getters are
    // solving-only, so their snapshots are captured in the callback above.
    let stage = solved.stage();
    if (Stage::InitPresolve..=Stage::Solved).contains(&stage) {
        progress.presolve_s = solved.presolving_time();
    }
    if matches!(stage, Stage::Presolving | Stage::Presolved | Stage::Solving | Stage::Solved) {
        progress.lp_iterations = solved.n_lp_iterations().max(0) as u64;
    }
    if matches!(stage, Stage::Presolved | Stage::Solving | Stage::Solved) {
        progress.root_lp_iterations = solved.n_root_lp_iterations().max(0) as u64;
    }
    progress.final_variables = solved.n_vars();
    progress.final_constraints = solved.n_conss();
    progress.final_nonzeros = solved.n_nonzeros();
    progress.nodes = solved.n_nodes().max(0) as usize;
    progress
}

/// Whether models are dumped (`INCLINE_SCIP_DUMP`, see [`dump`]); a model
/// built while it is set records its calls from the start.
pub(crate) fn dumping() -> bool {
    std::env::var_os("INCLINE_SCIP_DUMP").is_some()
}

/// With `INCLINE_SCIP_DUMP` set to a directory, write the model about to be
/// solved there as `scip-<pid>-<n>.cip`, with its changed parameters beside it
/// (`.set`) and the seed offered to it (`.sol`, a name and value per line), so
/// the same solve can be run again by SCIP itself. `.spec` holds every call
/// that built the model, one per line (see `minlp::model`).
pub(crate) fn dump(model: &Model, seed: Option<&std::collections::HashMap<String, f64>>) {
    use std::fmt::Write as _;
    let Some(dir) = std::env::var_os("INCLINE_SCIP_DUMP") else { return };
    static DUMPED: AtomicU64 = AtomicU64::new(0);
    let base = std::path::Path::new(&dir).join(format!("scip-{}-{:03}", std::process::id(), DUMPED.fetch_add(1, Ordering::Relaxed)));
    let write = |extension: &str, text: &str| {
        if let Err(error) = std::fs::write(base.with_extension(extension), text) {
            log::warn!("could not write {}: {error}", base.with_extension(extension).display());
        }
    };
    match model.write_orig_problem() {
        Ok(text) => write("cip", &text),
        Err(error) => log::warn!("could not write {}: {error:?}", base.with_extension("cip").display()),
    }
    write("set", &model.write_changed_params());
    if let Some(calls) = model.recorded_calls() {
        write("spec", calls);
    }
    if let Some(seed) = seed {
        let mut values: Vec<_> = seed.iter().collect();
        values.sort_by(|a, b| a.0.cmp(b.0));
        let mut text = String::new();
        for (name, value) in values {
            let _ = writeln!(text, "{name} {value:e}");
        }
        write("sol", &text);
    }
}

/// Interrupt the solve at its next event once any of `signals` is raised.
pub(crate) fn install_cancellation(model: &mut Model, signals: Vec<Arc<AtomicBool>>, audit: Arc<InterruptAudit>) {
    let installed = model.include_event_callback(
        "incline_cancel",
        "interrupt from the worker thread after a cancellation request",
        EVENTS,
        move |solving, event| {
            audit.callbacks.fetch_add(1, Ordering::Relaxed);
            record_progress(solving, event, &audit);
            if !signals.iter().any(|signal| signal.load(Ordering::Acquire)) {
                return;
            }
            // The callback runs synchronously on the thread executing this
            // model's solve; the UI only writes the atomic flag.
            solving.interrupt();
            audit.calls.fetch_add(1, Ordering::Relaxed);
        },
    );
    if installed.is_err() {
        log::warn!("SCIP's cancellation handler was not installed");
    }
}

/// Relative MIP gap of a finished solve.
///
/// SCIP returns `+inf` when there is no incumbent; that is reported as
/// `None` so "no incumbent" stays distinct from "gap zero".
pub(crate) fn gap(solved: &Model) -> Option<f64> {
    if solved.n_sols() == 0 || bound(solved).is_none() {
        return None;
    }
    let raw = solved.gap();
    (raw.is_finite() && !solved.is_infinity(raw.abs())).then_some(raw)
}

/// The dual bound, or `None` while SCIP has none. SCIP reports "no bound" as
/// its own infinity (1e20 by default), which is a finite `f64` - so a solve
/// stopped before its first root LP used to publish 1e20 as a bound.
pub(crate) fn bound(solved: &Model) -> Option<f64> {
    let raw = solved.dual_bound();
    (raw.is_finite() && !solved.is_infinity(raw.abs())).then_some(raw)
}

/// Everything the benchmark rows record about one finished solve, read once
/// so the solved model need not be threaded through reporting code.
#[derive(Clone, Debug)]
pub(crate) struct SolveReport {
    pub(crate) status: Status,
    pub(crate) objective: Option<f64>,
    /// Infinite while SCIP has no dual bound.
    pub(crate) bound: f64,
    pub(crate) gap: Option<f64>,
}

impl SolveReport {
    pub(crate) fn read(solved: &Model) -> Self {
        let has_incumbent = solved.n_sols() > 0;
        Self {
            status: solved.status(),
            objective: has_incumbent.then(|| solved.primal_bound()),
            bound: bound(solved).unwrap_or(f64::INFINITY),
            gap: gap(solved),
        }
    }
}
