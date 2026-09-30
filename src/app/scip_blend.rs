//! The SCIP solve of one owned blended model, run inside the solver process
//! (see [`super::solver_process`]).
//!
//! [`execute_scip_blend`] validates the input, builds the model, solves it,
//! extracts owned rows and runs the independent replay, polling cancellation
//! throughout. A horizon longer than one day-by-day window is first solved a
//! day at a time; the stitched schedule is replayed against the whole
//! horizon and seeds the whole-horizon solve, and whichever replayed schedule
//! is worth more is the one kept. Beside such a run, HiGHS solves the
//! model's linear relaxation for a bound SCIP may not reach in time. No SCIP
//! model, pointer or solution wrapper leaves it; only owned, replayed rows
//! do. Run Period and Run All Periods reach it through
//! [`crate::app::schedule_run`], which owns the job, currentness and
//! publication, and [`super::solver_process`], which runs it in a child
//! process and has the app replay what comes back.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::Write,
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use russcip::{Model, ProblemCreated, Status, Variable, ffi, prelude::*};

use super::{jobs::CancelFlag, schedule_pipeline::ScheduleRunInputs};
use crate::model::schedule::{
    optimisation::{
        Activity, DestinationKind, SourceId, TaskKind,
        blended::{
            formulation::BlendSizes,
            input::BlendInput,
            relaxation::{RelaxationBound, relaxation_bound},
            replay::{BlendSolution, ExtractionAdjustments, ReplayReport, replay_cancellable},
            rolling::{self, Carry, Stitched, Window},
        },
        scip::{
            adapter::{self, InterruptAudit, SolveReport},
            blend::formulate_scip_with_cancel,
            experiments::extract_solution,
        },
    },
    result::{BoundSource, DayByDayRole, DayByDaySummary},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScipRunIdentity {
    pub(crate) run_id: u64,
    pub(crate) inputs: ScheduleRunInputs,
    pub(crate) plan_revision: u64,
    pub(crate) document_revision: u64,
    /// The complete experimental-run identity: every authored input that can
    /// change feasibility, the objective or the encoded model, plus the run
    /// options, and no presentation name. See [`App::experimental_blend_key`].
    ///
    /// Stage 5A carried only the dig-only Setup inputs and the bar revision,
    /// which say nothing about stockpile representation, grade units, truck
    /// calendars or cashflow coefficients - so an edit to any of those could
    /// not retire a run that had read them.
    pub(crate) semantic: u64,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScipSolveOptions {
    pub(crate) time_limit: Option<Duration>,
    pub(crate) relative_gap: Option<f64>,
    pub(crate) diagnostic_logging: bool,
}

impl Default for ScipSolveOptions {
    fn default() -> Self {
        Self {
            time_limit: Some(Duration::from_secs(60)),
            relative_gap: Some(1e-4),
            diagnostic_logging: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ScipTermination {
    Optimal,
    FeasibleLimit,
    LimitNoIncumbent,
    Infeasible,
    Unbounded,
    Cancelled,
    InvalidInput,
    ValidationFailure,
    BackendFailure,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScipPhaseTimings {
    pub(crate) input_validation: Duration,
    pub(crate) formulation: Duration,
    pub(crate) solver: Duration,
    pub(crate) extraction: Duration,
    pub(crate) replay: Duration,
}

pub(crate) struct ScipCompletion {
    pub(crate) identity: ScipRunIdentity,
    /// The options this run was solved under, held with the result so a
    /// currentness check can ask the same question the request did.
    pub(crate) options: ScipSolveOptions,
    pub(crate) input: Arc<BlendInput>,
    pub(crate) termination: ScipTermination,
    pub(crate) backend_status: Option<Status>,
    pub(crate) diagnostic: Option<String>,
    pub(crate) solution: Option<Arc<BlendSolution>>,
    pub(crate) replay: Option<Arc<ReplayReport>>,
    pub(crate) published_objective: Option<f64>,
    pub(crate) raw_objective: Option<f64>,
    pub(crate) primary_bound: Option<f64>,
    pub(crate) primary_gap: Option<f64>,
    /// Which solve proved `primary_bound`.
    pub(crate) bound_source: BoundSource,
    pub(crate) adjustments: Option<ExtractionAdjustments>,
    pub(crate) sizes: BlendSizes,
    pub(crate) timings: ScipPhaseTimings,
    pub(crate) diagnostics: crate::model::schedule::result::SolveDiagnostics,
    pub(crate) solver_limit_overshoot: Option<Duration>,
    pub(crate) backend_version: String,
    pub(crate) wrapper_version: &'static str,
    pub(crate) event_callbacks: u64,
    pub(crate) interrupt_calls: u64,
    /// Present when the horizon was first solved day by day.
    pub(crate) day_by_day: Option<DayByDaySummary>,
}

impl ScipCompletion {
    pub(crate) fn new(identity: ScipRunIdentity, options: ScipSolveOptions, input: Arc<BlendInput>) -> Self {
        Self {
            identity,
            options,
            input,
            termination: ScipTermination::BackendFailure,
            backend_status: None,
            diagnostic: None,
            solution: None,
            replay: None,
            published_objective: None,
            raw_objective: None,
            primary_bound: None,
            primary_gap: None,
            bound_source: BoundSource::Scip,
            adjustments: None,
            sizes: BlendSizes::default(),
            timings: ScipPhaseTimings::default(),
            diagnostics: Default::default(),
            solver_limit_overshoot: None,
            backend_version: adapter::version(),
            wrapper_version: "russcip 0.10.0",
            event_callbacks: 0,
            interrupt_calls: 0,
            day_by_day: None,
        }
    }

    pub(crate) fn stop(&mut self, reason: ScipTermination, diagnostic: impl Into<String>) {
        self.termination = reason;
        self.diagnostic = Some(diagnostic.into());
        self.solution = None;
        self.published_objective = None;
        log_outcome(self);
    }

    pub(crate) fn usable(&self) -> bool {
        matches!(self.termination, ScipTermination::Optimal | ScipTermination::FeasibleLimit)
            && self.solution.is_some()
            && self.replay.as_ref().is_some_and(|report| report.is_valid())
    }

    /// What the solver process sends back for this completion.
    pub(crate) fn report(&self) -> ReportedCompletion {
        let issues = |pick: fn(&ReplayReport) -> &Vec<String>| self.replay.as_deref().map(pick).cloned().unwrap_or_default();
        ReportedCompletion {
            termination: self.termination,
            backend_status: self.backend_status.map(StatusName),
            diagnostic: self.diagnostic.clone(),
            solution: self.solution.as_deref().cloned(),
            replay_issues: issues(|report| &report.issues),
            replay_grade_issues: issues(|report| &report.grade_issues),
            published_objective: self.published_objective,
            raw_objective: self.raw_objective,
            primary_bound: self.primary_bound,
            primary_gap: self.primary_gap,
            bound_source: self.bound_source,
            adjustments: self.adjustments,
            sizes: self.sizes,
            timings: self.timings,
            diagnostics: self.diagnostics.clone(),
            solver_limit_overshoot: self.solver_limit_overshoot,
            backend_version: self.backend_version.clone(),
            event_callbacks: self.event_callbacks,
            interrupt_calls: self.interrupt_calls,
            day_by_day: self.day_by_day.clone(),
        }
    }

    /// Rebuild a completion the solver process reported, replaying its
    /// schedule again here rather than taking the process's word for it: the
    /// process is the part of the app a native fault can corrupt, so what it
    /// says is checked like any other solver claim.
    pub(crate) fn from_report(identity: ScipRunIdentity, options: ScipSolveOptions, input: Arc<BlendInput>, report: ReportedCompletion, cancel: &CancelFlag) -> Self {
        let mut out = Self::new(identity, options, input);
        out.termination = report.termination;
        out.backend_status = report.backend_status.map(|status| status.0);
        out.diagnostic = report.diagnostic;
        out.raw_objective = report.raw_objective;
        out.primary_bound = report.primary_bound;
        out.primary_gap = report.primary_gap;
        out.bound_source = report.bound_source;
        out.adjustments = report.adjustments;
        out.sizes = report.sizes;
        out.timings = report.timings;
        out.diagnostics = report.diagnostics;
        out.solver_limit_overshoot = report.solver_limit_overshoot;
        out.backend_version = report.backend_version;
        out.event_callbacks = report.event_callbacks;
        out.interrupt_calls = report.interrupt_calls;
        out.day_by_day = report.day_by_day;
        let Some(solution) = report.solution else {
            // Nothing to publish; the process's replay findings are kept only
            // to explain why.
            if !report.replay_issues.is_empty() || !report.replay_grade_issues.is_empty() {
                out.replay = Some(Arc::new(ReplayReport {
                    issues: report.replay_issues,
                    grade_issues: report.replay_grade_issues,
                    ..ReplayReport::default()
                }));
            }
            return out;
        };
        let started = Instant::now();
        let checked = replay_cancellable(&out.input, &solution, &cancel.signal());
        out.timings.replay += started.elapsed();
        let Some(checked) = checked else {
            out.stop(ScipTermination::Cancelled, "cancelled during replay");
            return out;
        };
        let replayed = checked.replayed_objective;
        let slack = checked.boundary_value_slack;
        let valid = checked.is_valid();
        out.replay = Some(Arc::new(checked));
        if !valid {
            out.stop(ScipTermination::ValidationFailure, "independent blended replay rejected the solver process's schedule");
            return out;
        }
        let claimed = report.published_objective.unwrap_or(f64::NAN);
        let agrees = (claimed - replayed).abs() <= REPORT_AGREEMENT * replayed.abs().max(1.0);
        if !agrees {
            out.stop(
                ScipTermination::ValidationFailure,
                format!("the solver process reported a schedule worth {claimed}, which replays to {replayed}"),
            );
            return out;
        }
        if let Some(bound) = out.primary_bound
            && let Some(problem) = exceeds_bound(replayed - slack, bound)
        {
            out.stop(ScipTermination::ValidationFailure, problem);
            return out;
        }
        out.published_objective = Some(replayed);
        out.solution = Some(Arc::new(solution));
        out
    }
}

/// How closely the app's replay must agree with the value the solver
/// process's replay found. Both run the same code on the same numbers - JSON
/// carries every `f64` exactly - so any real difference means the process
/// sent something other than what it replayed.
const REPORT_AGREEMENT: f64 = 1e-9;

/// A [`ScipCompletion`] as the solver process reports it (see
/// [`super::solver_process`]): the input stays with the app, which sent it,
/// and the replay is repeated by the app, so only its findings travel, to
/// explain a schedule that was not published.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ReportedCompletion {
    termination: ScipTermination,
    backend_status: Option<StatusName>,
    diagnostic: Option<String>,
    solution: Option<BlendSolution>,
    replay_issues: Vec<String>,
    replay_grade_issues: Vec<String>,
    published_objective: Option<f64>,
    raw_objective: Option<f64>,
    primary_bound: Option<f64>,
    primary_gap: Option<f64>,
    bound_source: BoundSource,
    adjustments: Option<ExtractionAdjustments>,
    sizes: BlendSizes,
    timings: ScipPhaseTimings,
    diagnostics: crate::model::schedule::result::SolveDiagnostics,
    solver_limit_overshoot: Option<Duration>,
    backend_version: String,
    event_callbacks: u64,
    interrupt_calls: u64,
    day_by_day: Option<DayByDaySummary>,
}

/// russcip's status, which has no serde support of its own.
#[derive(serde::Serialize, serde::Deserialize)]
struct StatusName(#[serde(with = "StatusDef")] Status);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Status")]
enum StatusDef {
    Unknown,
    UserInterrupt,
    NodeLimit,
    TotalNodeLimit,
    StallNodeLimit,
    TimeLimit,
    MemoryLimit,
    GapLimit,
    PrimalLimit,
    DualLimit,
    SolutionLimit,
    BestSolutionLimit,
    RestartLimit,
    Optimal,
    Infeasible,
    Unbounded,
    Inforunbd,
    Terminate,
}

/// Observable worker phase for developer lifecycle checks. SCIP objects are
/// never stored here; only atomics cross between the worker and poller.
#[derive(Default)]
pub(crate) struct ScipActivity {
    phase: AtomicU8,
    pub(crate) formulation_checks: std::sync::atomic::AtomicU64,
    pub(crate) interrupt: Arc<InterruptAudit>,
}

impl ScipActivity {
    #[allow(dead_code, reason = "read by the developer lifecycle checks")]
    pub(crate) fn phase(&self) -> u8 {
        self.phase.load(Ordering::Acquire)
    }
    fn set(&self, phase: u8) {
        self.phase.store(phase, Ordering::Release);
    }
}

/// Runs on one thread, the solver process's main one. No SCIP model, pointer or
/// solution wrapper leaves this function; only owned, replayed rows do.
///
/// `early` is handed the day-by-day schedule, completed as a publishable
/// answer, as soon as it has passed the whole-horizon replay and before the
/// whole-horizon solve starts, so a caller can show it while the rest of the
/// run looks for a better one. It is called at most once, and never for a
/// run that is not solved day by day.
pub(crate) fn execute_scip_blend(
    input: Arc<BlendInput>,
    identity: ScipRunIdentity,
    options: ScipSolveOptions,
    cancel: &CancelFlag,
    activity: &ScipActivity,
    early: &dyn Fn(&ScipCompletion),
) -> ScipCompletion {
    let mut out = ScipCompletion::new(identity, options, input);
    if cancel.is_cancelled() {
        out.stop(ScipTermination::Cancelled, "cancelled before validation");
        return out;
    }

    activity.set(1);
    let started = Instant::now();
    let validation = validate_input(&out.input, options, cancel);
    out.timings.input_validation = started.elapsed();
    if cancel.is_cancelled() {
        out.stop(ScipTermination::Cancelled, "cancelled during input validation");
        return out;
    }
    if let Err(problem) = validation {
        out.stop(ScipTermination::InvalidInput, problem);
        return out;
    }

    // The whole run shares one solve budget: day-by-day windows first, when
    // the horizon is long enough to need them, then the whole horizon with
    // whatever is left.
    let budget_started = Instant::now();
    let mut seed = None;
    let mut relaxation = None;
    if let Some(windows) = rolling::plan(&out.input) {
        relaxation = RelaxationJob::start(&out.input, options.time_limit, cancel, out.identity.run_id);
        match solve_day_by_day(&mut out, &windows, options, cancel, activity) {
            DayByDay::Seed(found) => {
                let proved = relaxation.as_mut().and_then(RelaxationJob::ready);
                let mut shown = ScipCompletion::new(out.identity, options, Arc::clone(&out.input));
                shown.sizes = out.sizes;
                shown.timings = out.timings;
                adopt_seed(&mut shown, (*found).clone(), None, proved, DayByDayRole::Early);
                if shown.primary_gap.is_some_and(|gap| options.relative_gap.is_some_and(|target| gap <= target)) {
                    log::info!(
                        "schedule run {}: the day-by-day schedule is within the gap target of the relaxation bound; no whole-horizon solve",
                        out.identity.run_id
                    );
                    adopt_seed(&mut out, *found, None, proved, DayByDayRole::Proven);
                    return out;
                }
                early(&shown);
                seed = Some(*found);
            }
            DayByDay::Failed(reason) => {
                log::warn!("schedule run {}: day-by-day start abandoned: {reason}", out.identity.run_id);
                out.day_by_day = Some(DayByDaySummary {
                    windows: windows.len(),
                    seconds: budget_started.elapsed().as_secs_f64(),
                    value: None,
                    role: DayByDayRole::Improved,
                    failure: Some(reason),
                });
            }
            DayByDay::Stop(reason, diagnostic) => {
                out.stop(reason, diagnostic);
                return out;
            }
        }
    }
    let remaining = options.time_limit.map(|limit| limit.saturating_sub(budget_started.elapsed()));
    if let Some(found) = seed.take_if(|_| remaining.is_some_and(|left| left < WHOLE_HORIZON_MINIMUM)) {
        log::info!(
            "schedule run {}: no time left for a whole-horizon solve; keeping the day-by-day schedule",
            out.identity.run_id
        );
        let proved = relaxation.as_mut().and_then(RelaxationJob::ready);
        adopt_seed(&mut out, found, None, proved, DayByDayRole::Kept);
        return out;
    }

    let mut completed = None;
    if let Some(found) = seed.as_ref() {
        activity.set(3);
        let started = Instant::now();
        let limit = remaining.map(|left| left.mul_f64(SEED_COMPLETION_SHARE));
        match complete_seed(&out.input, &found.solution, limit, cancel) {
            Ok(values) => {
                log::info!("schedule run {}: day-by-day seed completed in {:.2?}", out.identity.run_id, started.elapsed());
                completed = Some(values);
            }
            Err(_) if cancel.is_cancelled() => {
                out.stop(ScipTermination::Cancelled, "cancelled while completing the day-by-day seed");
                return out;
            }
            Err(problem) => log::warn!("schedule run {}: day-by-day seed not offered to SCIP: {problem}", out.identity.run_id),
        }
        out.timings.solver += started.elapsed();
    }
    let remaining = options.time_limit.map(|limit| limit.saturating_sub(budget_started.elapsed()));
    if let Some(found) = seed.take_if(|_| remaining.is_some_and(|left| left < WHOLE_HORIZON_MINIMUM)) {
        let proved = relaxation.as_mut().and_then(RelaxationJob::ready);
        adopt_seed(&mut out, found, None, proved, DayByDayRole::Kept);
        return out;
    }

    activity.set(2);
    let started = Instant::now();
    let built = formulate_scip_with_cancel(&out.input, Some(&cancel.signal()), Some(&activity.formulation_checks));
    out.timings.formulation += started.elapsed();
    if cancel.is_cancelled() || built.is_err() {
        out.stop(ScipTermination::Cancelled, "cancelled during formulation");
        return out;
    }
    let built = built.expect("checked formulation result");
    out.sizes = built.sizes;
    log::info!(
        "schedule run {}: model of {} variables ({} binary), {} linear and {} nonlinear constraints over {} intervals x {} event positions, built in {:?}",
        out.identity.run_id,
        out.sizes.variables,
        out.sizes.binaries,
        out.sizes.linear_constraints,
        out.sizes.nonlinear_constraints,
        out.input.intervals.len(),
        out.input.segments_per_interval,
        started.elapsed()
    );
    let columns = built.columns;
    if options.diagnostic_logging {
        log_model_structure(out.identity.run_id, &out.input);
    }

    let model = if options.diagnostic_logging {
        built.model.show_output()
    } else {
        built.model.hide_output()
    };
    let mut model = match configure(model, remaining.or(options.time_limit), options.relative_gap) {
        Ok(model) => model,
        Err(problem) => {
            out.stop(ScipTermination::BackendFailure, problem);
            return out;
        }
    };
    if let Some(values) = completed.as_ref() {
        model = match offer_seed(model, values) {
            Ok((model, stored)) => {
                if !stored {
                    log::warn!("schedule run {}: SCIP rejected the completed day-by-day seed", out.identity.run_id);
                }
                model
            }
            Err(problem) => {
                out.stop(ScipTermination::BackendFailure, problem);
                return out;
            }
        };
    }
    adapter::install_cancellation(&mut model, cancel.signal(), Arc::clone(&activity.interrupt));
    if cancel.is_cancelled() {
        out.stop(ScipTermination::Cancelled, "cancelled before solve");
        return out;
    }

    activity.set(3);
    let started = Instant::now();
    let solved = model.solve();
    let solve_time = started.elapsed();
    out.timings.solver += solve_time;
    if let Some(limit) = remaining.or(options.time_limit) {
        out.solver_limit_overshoot = solve_time.checked_sub(limit);
    }
    let report = SolveReport::read(&solved);
    out.diagnostics = adapter::diagnostics(&solved, &activity.interrupt);
    out.backend_status = Some(report.status);
    out.raw_objective = report.objective;
    out.primary_bound = report.bound.is_finite().then_some(report.bound);
    out.primary_gap = report.gap;
    out.event_callbacks = activity.interrupt.callbacks.load(Ordering::Acquire);
    out.interrupt_calls = activity.interrupt.calls.load(Ordering::Acquire);
    if activity.interrupt.failed.load(Ordering::Acquire) {
        out.stop(ScipTermination::BackendFailure, "SCIPinterruptSolve rejected a solver-thread callback");
        return out;
    }
    if cancel.is_cancelled() {
        out.stop(ScipTermination::Cancelled, format!("cancelled during solve; backend status {:?}", report.status));
        return out;
    }

    activity.set(4);
    let started = Instant::now();
    let extracted = extract_solution(&solved, &columns, || cancel.is_cancelled());
    out.timings.extraction += started.elapsed();
    if cancel.is_cancelled() || extracted.is_err() {
        out.stop(ScipTermination::Cancelled, "cancelled during extraction");
        return out;
    }
    let Some(solution) = extracted.expect("checked extraction result") else {
        if let Some(found) = seed {
            // A dual bound needs no incumbent, so it still bounds the seed.
            let (bound, proved) = (out.primary_bound, relaxation.as_mut().and_then(RelaxationJob::ready));
            adopt_seed(&mut out, found, bound, proved, DayByDayRole::Kept);
            return out;
        }
        out.termination = classify_status(report.status, false);
        log_outcome(&out);
        return out;
    };
    drop(solved);
    out.adjustments = Some(solution.adjustments);

    activity.set(5);
    let started = Instant::now();
    let checked = replay_cancellable(&out.input, &solution, &cancel.signal());
    out.timings.replay += started.elapsed();
    if cancel.is_cancelled() || checked.is_none() {
        out.stop(ScipTermination::Cancelled, "cancelled during replay");
        return out;
    }
    let checked = checked.expect("checked replay result");
    let valid = checked.is_valid();
    if !valid && let Some(found) = seed {
        // SCIP's answer failed the replay, so neither it nor the bound that
        // came with it is trusted; the seed was replayed on its own.
        for issue in checked.issues.iter().chain(&checked.grade_issues).take(5) {
            log::warn!("schedule run {}: whole-horizon incumbent rejected by replay: {issue}", out.identity.run_id);
        }
        let proved = relaxation.as_mut().and_then(RelaxationJob::ready);
        adopt_seed(&mut out, found, None, proved, DayByDayRole::Kept);
        return out;
    }
    out.published_objective = Some(checked.replayed_objective);
    out.replay = Some(Arc::new(checked));
    if !valid {
        out.stop(ScipTermination::ValidationFailure, "independent blended replay rejected the incumbent");
        return out;
    }
    if let Some(bound) = out.primary_bound {
        // The bound is on the model's objective, which values deliveries near
        // a conditional grade boundary conservatively; the published figure
        // uses the authored boundary and may sit above it by that much.
        let slack = out.replay.as_ref().map_or(0.0, |report| report.boundary_value_slack);
        let published = out.published_objective.expect("replayed objective") - slack;
        if let Some(problem) = exceeds_bound(published, bound) {
            out.stop(ScipTermination::ValidationFailure, problem);
            return out;
        }
    }
    let proved = relaxation.as_mut().and_then(RelaxationJob::ready);
    let mut termination = classify_status(report.status, true);
    {
        let slack = out.replay.as_ref().map_or(0.0, |report| report.boundary_value_slack);
        let published = out.published_objective.expect("replayed objective") - slack;
        if let Some(bound) = consistent_relaxation(proved, published, out.identity.run_id).filter(|bound| out.primary_bound.is_none_or(|scip| *bound < scip)) {
            out.primary_bound = Some(bound);
            out.primary_gap = out.raw_objective.and_then(|raw| relative_gap(raw, bound));
            out.bound_source = BoundSource::Relaxation;
            if termination == ScipTermination::FeasibleLimit && out.primary_gap.is_some_and(|gap| options.relative_gap.is_some_and(|target| gap <= target)) {
                termination = ScipTermination::Optimal;
            }
        }
    }
    if let Some(found) = seed {
        let improved = out.published_objective.expect("replayed objective") >= found.replay.replayed_objective;
        if !improved && termination != ScipTermination::Optimal {
            let bound = out.primary_bound.filter(|_| out.bound_source == BoundSource::Scip);
            adopt_seed(&mut out, found, bound, proved, DayByDayRole::Kept);
            return out;
        }
        out.day_by_day = Some(found.summary);
    }
    out.termination = termination;
    if matches!(out.termination, ScipTermination::Optimal | ScipTermination::FeasibleLimit) {
        out.solution = Some(Arc::new(solution));
    }
    activity.set(6);
    log_outcome(&out);
    out
}

/// Share of the solve budget the day-by-day windows may take between them.
/// The rest is the whole-horizon solve's, which is the only one that can
/// bound the result.
const DAY_BY_DAY_SHARE: f64 = 0.5;

/// Most of what is left after the windows that completing the seed may use.
/// Completion is propagation over fixed movements and normally takes
/// seconds; the cap only stops a pathological case eating the solve.
const SEED_COMPLETION_SHARE: f64 = 0.25;

/// Below this, a whole-horizon solve cannot build and presolve its model,
/// let alone improve on the seed, so the day-by-day schedule is kept as is.
const WHOLE_HORIZON_MINIMUM: Duration = Duration::from_secs(2);

/// How far the relaxation's optimum is loosened before it is used as a bound:
/// HiGHS solves the LP to feasibility and optimality tolerances of 1e-7, so
/// its reported optimum can sit a little below the exact one.
const RELAXATION_MARGIN_ABSOLUTE: f64 = 1e-4;
const RELAXATION_MARGIN_RELATIVE: f64 = 1e-6;

/// The HiGHS relaxation bound (see
/// [`crate::model::schedule::optimisation::blended::relaxation`]), solved on
/// its own thread beside a day-by-day run.
///
/// It starts with the run, needs nothing from the windows, and is only ever
/// polled: the run never waits for it. Dropping the job stops the solve at
/// HiGHS's next interior-point iteration, as does cancelling the run.
struct RelaxationJob {
    run_id: u64,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<RelaxationBound, String>>>,
    bound: Option<f64>,
}

impl RelaxationJob {
    fn start(input: &Arc<BlendInput>, limit: Option<Duration>, cancel: &CancelFlag, run_id: u64) -> Option<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let (input, cancel, stopped) = (Arc::clone(input), cancel.signal(), Arc::clone(&stop));
        let spawned = std::thread::Builder::new()
            .name("schedule-relaxation".into())
            .spawn(move || relaxation_bound(&input, limit, &|| stopped.load(Ordering::Acquire) || cancel.load(Ordering::Acquire)));
        match spawned {
            Ok(handle) => Some(Self {
                run_id,
                stop,
                handle: Some(handle),
                bound: None,
            }),
            Err(error) => {
                log::warn!("schedule run {run_id}: relaxation bound not started: {error}");
                None
            }
        }
    }

    /// The bound, once the solve has finished with one.
    fn ready(&mut self) -> Option<f64> {
        if let Some(handle) = self.handle.take_if(|handle| handle.is_finished()) {
            match handle.join() {
                Ok(Ok(found)) => {
                    log::info!(
                        "schedule run {}: relaxation bound {:.2} in {:.2?} ({} mixing equalities left out)",
                        self.run_id,
                        found.value,
                        found.elapsed,
                        found.dropped_mixing
                    );
                    self.bound = Some(found.value + RELAXATION_MARGIN_ABSOLUTE.max(found.value.abs() * RELAXATION_MARGIN_RELATIVE));
                }
                Ok(Err(problem)) => log::warn!("schedule run {}: no relaxation bound: {problem}", self.run_id),
                Err(_) => log::warn!("schedule run {}: the relaxation bound panicked", self.run_id),
            }
        }
        self.bound
    }
}

impl Drop for RelaxationJob {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// The stitched day-by-day schedule, replayed against the whole horizon.
#[derive(Clone)]
struct Seed {
    solution: BlendSolution,
    replay: ReplayReport,
    summary: DayByDaySummary,
}

enum DayByDay {
    Seed(Box<Seed>),
    /// No usable start; the whole-horizon solve runs unseeded.
    Failed(String),
    /// Cancelled, or a backend failure the whole run cannot continue past.
    Stop(ScipTermination, String),
}

/// Solve the horizon a day at a time (see [`rolling`]) and stitch the kept
/// days into one schedule for the whole horizon.
fn solve_day_by_day(out: &mut ScipCompletion, windows: &[Window], options: ScipSolveOptions, cancel: &CancelFlag, activity: &ScipActivity) -> DayByDay {
    let started = Instant::now();
    let full = Arc::clone(&out.input);
    let budget = options.time_limit.map(|limit| limit.mul_f64(DAY_BY_DAY_SHARE));
    let mut carry = Carry::opening(&full);
    let mut stitched = Stitched::new();
    for (position, &window) in windows.iter().enumerate() {
        if cancel.is_cancelled() {
            return DayByDay::Stop(ScipTermination::Cancelled, "cancelled during a day-by-day window".into());
        }
        let label = format!("window {}/{}", position + 1, windows.len());
        let limit = budget.map(|budget| budget.saturating_sub(started.elapsed()) / (windows.len() - position) as u32);
        if limit.is_some_and(|limit| limit.is_zero()) {
            return DayByDay::Failed(format!("{label}: no time left"));
        }
        let input = carry.window_input(&full, window);
        // A window's input is derived, not captured, so it is held to the
        // same checks before SCIP sees it.
        match validate_input(&input, options, cancel) {
            Ok(()) if cancel.is_cancelled() => return DayByDay::Stop(ScipTermination::Cancelled, "cancelled during a day-by-day window".into()),
            Ok(()) => {}
            Err(problem) => return DayByDay::Failed(format!("{label}: invalid window input: {problem}")),
        }

        activity.set(2);
        let phase = Instant::now();
        let built = formulate_scip_with_cancel(&input, Some(&cancel.signal()), Some(&activity.formulation_checks));
        out.timings.formulation += phase.elapsed();
        let Ok(built) = built else {
            return DayByDay::Stop(ScipTermination::Cancelled, "cancelled during formulation".into());
        };
        if built.sizes.variables > out.sizes.variables {
            out.sizes = built.sizes;
        }
        let model = if options.diagnostic_logging {
            built.model.show_output()
        } else {
            built.model.hide_output()
        };
        let mut model = match configure(model, limit, options.relative_gap) {
            Ok(model) => model,
            Err(problem) => return DayByDay::Stop(ScipTermination::BackendFailure, problem),
        };
        // The window's own progress record: the published diagnostics
        // describe the whole-horizon solve alone.
        let audit = Arc::new(adapter::InterruptAudit::default());
        adapter::install_cancellation(&mut model, cancel.signal(), Arc::clone(&audit));

        activity.set(3);
        let phase = Instant::now();
        let solved = model.solve();
        let solve_time = phase.elapsed();
        out.timings.solver += solve_time;
        if audit.failed.load(Ordering::Acquire) {
            return DayByDay::Stop(ScipTermination::BackendFailure, "SCIPinterruptSolve rejected a solver-thread callback".into());
        }
        if cancel.is_cancelled() {
            return DayByDay::Stop(ScipTermination::Cancelled, "cancelled during a day-by-day window".into());
        }
        let status = solved.status();
        let gap = adapter::gap(&solved);

        activity.set(4);
        let phase = Instant::now();
        let extracted = extract_solution(&solved, &built.columns, || cancel.is_cancelled());
        let paid = solved.best_sol().map_or(0.0, |best| {
            built
                .columns
                .paid
                .iter()
                .filter(|&(&(_, interval, _), _)| interval < window.committed)
                .map(|(&(conditional, _, _), column)| best.val(column) * input.conditional_values[conditional].value_per_tonne)
                .sum()
        });
        out.timings.extraction += phase.elapsed();
        drop(solved);
        let solution = match extracted {
            Err(()) => return DayByDay::Stop(ScipTermination::Cancelled, "cancelled during extraction".into()),
            Ok(None) => return DayByDay::Failed(format!("{label}: no schedule within {limit:?} ({status:?})")),
            Ok(Some(solution)) => solution,
        };

        activity.set(5);
        let phase = Instant::now();
        let checked = replay_cancellable(&input, &solution, &cancel.signal());
        out.timings.replay += phase.elapsed();
        let Some(checked) = checked else {
            return DayByDay::Stop(ScipTermination::Cancelled, "cancelled during replay".into());
        };
        if !checked.is_valid() {
            let issue = checked.issues.iter().chain(&checked.grade_issues).next().cloned().unwrap_or_default();
            return DayByDay::Failed(format!("{label}: replay rejected the window: {issue}"));
        }
        log::info!(
            "schedule run {}: day-by-day {label}, intervals {}..{} keeping {}: {} variables, {:?} objective {:.2} gap {:?} in {:.2?}",
            out.identity.run_id,
            window.first,
            window.end,
            window.committed,
            built.sizes.variables,
            status,
            solution.reported_objective,
            gap,
            solve_time
        );
        carry.advance(&full, window, &solution, &checked);
        stitched.keep(&full, window, &solution, paid);
    }

    let solution = stitched.finish();
    let phase = Instant::now();
    let checked = replay_cancellable(&full, &solution, &cancel.signal());
    out.timings.replay += phase.elapsed();
    let Some(checked) = checked else {
        return DayByDay::Stop(ScipTermination::Cancelled, "cancelled during replay".into());
    };
    if !checked.is_valid() {
        for issue in checked.issues.iter().chain(&checked.grade_issues).take(5) {
            log::warn!("schedule run {}: stitched day-by-day schedule: {issue}", out.identity.run_id);
        }
        let issue = checked.issues.iter().chain(&checked.grade_issues).next().cloned().unwrap_or_default();
        return DayByDay::Failed(format!("the stitched schedule failed the whole-horizon replay: {issue}"));
    }
    let summary = DayByDaySummary {
        windows: windows.len(),
        seconds: started.elapsed().as_secs_f64(),
        value: Some(checked.replayed_objective),
        role: DayByDayRole::Improved,
        failure: None,
    };
    log::info!(
        "schedule run {}: day-by-day schedule of {} windows worth {:.2} in {:.2}s",
        out.identity.run_id,
        summary.windows,
        checked.replayed_objective,
        summary.seconds
    );
    DayByDay::Seed(Box::new(Seed {
        solution,
        replay: checked,
        summary,
    }))
}

/// Publish the day-by-day schedule. `bound` is a whole-horizon SCIP dual
/// bound that still stands, and `proved` the relaxation's, if there are any;
/// the tighter is published.
fn adopt_seed(out: &mut ScipCompletion, found: Seed, bound: Option<f64>, proved: Option<f64>, role: DayByDayRole) {
    let published = found.replay.replayed_objective;
    let raw = found.solution.reported_objective;
    let checked = published - found.replay.boundary_value_slack;
    if let Some(bound) = bound
        && let Some(problem) = exceeds_bound(checked, bound)
    {
        out.stop(ScipTermination::ValidationFailure, problem);
        return;
    }
    let proved = consistent_relaxation(proved, checked, out.identity.run_id);
    let (bound, source) = match (bound, proved) {
        (scip, Some(proved)) if scip.is_none_or(|scip| proved < scip) => (Some(proved), BoundSource::Relaxation),
        (scip, _) => (scip, BoundSource::Scip),
    };
    out.raw_objective = Some(raw);
    out.primary_bound = bound;
    out.bound_source = source;
    out.primary_gap = bound.and_then(|bound| relative_gap(raw, bound));
    out.published_objective = Some(published);
    out.adjustments = Some(found.solution.adjustments);
    out.replay = Some(Arc::new(found.replay));
    out.solution = Some(Arc::new(found.solution));
    out.day_by_day = Some(DayByDaySummary { role, ..found.summary });
    out.termination = if role == DayByDayRole::Proven {
        ScipTermination::Optimal
    } else {
        ScipTermination::FeasibleLimit
    };
    // Shown early, it is not the run's outcome yet.
    if role != DayByDayRole::Early {
        log_outcome(out);
    }
}

/// SCIP's own definition, so the figure reads the same as a solver gap.
fn relative_gap(raw: f64, bound: f64) -> Option<f64> {
    let smaller = raw.abs().min(bound.abs());
    (smaller > 0.0 && raw.signum() == bound.signum()).then(|| (bound - raw).abs() / smaller)
}

/// The relaxation bound, unless a replayed schedule is worth more than it.
///
/// That cannot happen with a correct relaxation, so it is reported; but the
/// schedule was replayed on its own and the relaxation is only a second
/// opinion on its quality, so the bound is dropped rather than the schedule.
fn consistent_relaxation(proved: Option<f64>, published: f64, run_id: u64) -> Option<f64> {
    let proved = proved?;
    match exceeds_bound(published, proved) {
        None => Some(proved),
        Some(problem) => {
            log::warn!("schedule run {run_id}: relaxation bound not used: {problem}");
            None
        }
    }
}

fn exceeds_bound(published: f64, bound: f64) -> Option<String> {
    let tolerance = 1e-4_f64.max(bound.abs() * 1e-8);
    (published > bound + tolerance).then(|| format!("published objective {published} exceeds SCIP bound {bound} beyond {tolerance}"))
}

/// The run's limits and LP settings; everything else is SCIP's default.
///
/// # Primal simplex with devex pricing
///
/// SCIP starts the root LP with dual simplex and steepest-edge pricing. On
/// the blended model primal simplex with devex pricing is faster wherever it
/// was measured, and it is what lets a week's root LP finish inside a run:
///
/// | model | default | primal + devex |
/// |---|---|---|
/// | real project, week, first LP alone | 226.5 s | 103.0 s |
/// | real project, week, seeded run | root LP unfinished in 208 s | root LP at 112 s, 0.11 % gap |
/// | real project, one day | 0.45 s, optimal | 0.48 s, optimal |
/// | known-answer fixture, 24 h / 72 h | 0.35 s / 2.04 s | 0.05 s / 0.19 s |
/// | competition fixture, 24 / 48 / 72 h, 60 s | same bounds | same bounds, one better incumbent |
///
/// Only the LP algorithm changes, so the model and its optimum do not.
fn configure(model: Model<ProblemCreated>, time_limit: Option<Duration>, relative_gap: Option<f64>) -> Result<Model<ProblemCreated>, String> {
    let mut model = model;
    for (name, value) in [(c"lp/initalgorithm", b'p'), (c"lp/pricing", b'd')] {
        // SAFETY: a problem-stage parameter write on the thread that owns
        // the model. russcip 0.10 has no char-parameter setter.
        let code = unsafe { ffi::SCIPsetCharParam(model.scip_ptr(), name.as_ptr(), value as std::ffi::c_char) };
        if code != ffi::SCIP_Retcode_SCIP_OKAY {
            return Err(format!("setting SCIP {name:?}: retcode {code}"));
        }
    }
    if let Some(file) = ipopt_options_file() {
        model = model
            .set_str_param("nlpi/ipopt/optfile", file)
            .map_err(|error| format!("setting SCIP's Ipopt options file: {error:?}"))?;
    }
    if let Some(limit) = time_limit {
        model = model
            .set_real_param("limits/time", limit.as_secs_f64())
            .map_err(|error| format!("setting SCIP time limit: {error:?}"))?;
    }
    if let Some(gap) = relative_gap {
        model = model.set_real_param("limits/gap", gap).map_err(|error| format!("setting SCIP gap: {error:?}"))?;
    }
    Ok(model)
}

/// Ipopt's options for every SCIP solve: MUMPS orders its factorisations
/// with AMD rather than METIS.
///
/// The bundled build's METIS path is broken. Called by MUMPS through
/// `mumps_metis_nodend_mixedto32`, it corrupts the heap: glibc then either
/// aborts the process (`free(): invalid size` in `gk_malloc_cleanup`) or
/// leaves the solver thread hanging in `malloc`. Any Ipopt solve can reach
/// it - SCIP's sub-NLP heuristic did, in an unseeded solve of the
/// competition fixture over a week, as did MPEC in a seeded one. With this
/// option the same fixture completed every time; set to METIS explicitly it
/// hung. AMD changes only the fill-in of Ipopt's factorisations, not what
/// any solve means.
const IPOPT_OPTIONS: &str = "mumps_pivot_order 0\n";

/// SCIP takes Ipopt options only from a file. It is written once per
/// process, atomically, so concurrent runs and app instances never see it
/// half written. Without it, solves still run, on Ipopt's defaults.
fn ipopt_options_file() -> Option<&'static str> {
    static FILE: OnceLock<Option<String>> = OnceLock::new();
    FILE.get_or_init(|| {
        let path: PathBuf = std::env::temp_dir().join("incline-ipopt.opt");
        let written = crate::model::atomic_file::write_atomic(&path, |file| Ok(file.write_all(IPOPT_OPTIONS.as_bytes())?));
        match (written, path.to_str()) {
            (Ok(()), Some(path)) => Some(path.to_owned()),
            (Err(error), _) => {
                log::warn!("SCIP's Ipopt options were not written to {}: {error:#}", path.display());
                None
            }
            (Ok(()), None) => {
                log::warn!("SCIP's Ipopt options path {} is not valid UTF-8", path.display());
                None
            }
        }
    })
    .as_deref()
}

/// How far completion may move a seed value, in tonnes or hours: an absolute
/// part for values near zero and a relative part for large ones.
///
/// SCIP checks a window's answer in its presolved problem, and mapped back
/// onto the original columns it can miss an original row by more than
/// SCIP's feasibility tolerance. The independent replay accepts such a
/// schedule, but pinned exactly it can leave the whole-horizon model with no
/// feasible completion. On a real week this happened two ways:
///
/// - a loader dug its full rate from one block plus a 0.0002 t tail from the
///   block before it, over its rate row by that tail;
/// - each cell's dig from a mixed block was split between its materials in
///   proportion only to within that tolerance, so no one extraction total
///   satisfied every material's `portion` row at once.
///
/// A relative band of 1e-5 still left that week infeasible and 1e-4 did not;
/// the value used is ten times that. It moves only the seed SCIP starts from:
/// what is published is always a replayed schedule.
const SEED_BAND_ABSOLUTE: f64 = 1e-3;
const SEED_BAND_RELATIVE: f64 = 1e-3;

/// Complete the stitched schedule into a full solution of the whole-horizon
/// model, as values by variable name.
///
/// A second copy of the model is built with every movement and segment
/// duration held to the seed's value, give or take [`SEED_BAND_ABSOLUTE`]
/// and [`SEED_BAND_RELATIVE`], and a movement the seed does not make held at
/// zero, so what is left for SCIP is to fill in the state and indicator
/// columns those imply. The completed solution is SCIP's own, so it
/// satisfies the model as SCIP checks it. SCIP's own completion
/// heuristic was tried first and is not used: given the same values as a
/// partial solution it searched a neighbourhood of them instead, and on a
/// real week it spent the whole budget returning a schedule worth a
/// seventieth of the seed.
fn complete_seed(input: &BlendInput, seed: &BlendSolution, limit: Option<Duration>, cancel: &CancelFlag) -> Result<HashMap<String, f64>, String> {
    let built = formulate_scip_with_cancel(input, Some(&cancel.signal()), None).map_err(|_| "cancelled".to_owned())?;
    let tonnes: BTreeMap<(usize, usize, usize), f64> = seed.movements.iter().map(|row| ((row.candidate, row.interval, row.segment), row.tonnes_t)).collect();
    let missing = tonnes.keys().filter(|key| !built.columns.movement.contains_key(key)).count();
    if missing > 0 {
        return Err(format!("the seed names {missing} movement cells the whole-horizon model does not have"));
    }
    let scip = built.model.scip_ptr();
    let fix = |column: &Variable, value: f64| {
        let band = if value == 0.0 { 0.0 } else { SEED_BAND_ABSOLUTE.max(value.abs() * SEED_BAND_RELATIVE) };
        let (lb, ub) = (column.lb(), column.ub());
        let lower = (value - band).clamp(lb, ub);
        let upper = (value + band).clamp(lb, ub);
        // SAFETY: problem-stage bound changes on this model's own original
        // variables, on the thread that owns the model. Both lie inside the
        // original bounds, so they never cross.
        unsafe {
            ffi::SCIPchgVarLb(scip, column.inner(), lower);
            ffi::SCIPchgVarUb(scip, column.inner(), upper);
        }
    };
    for (key, column) in &built.columns.movement {
        fix(column, tonnes.get(key).copied().unwrap_or(0.0));
    }
    for (key, column) in &built.columns.duration {
        if let Some(duration) = seed.durations.get(key) {
            fix(column, *duration);
        }
    }
    let mut model = configure(built.model.hide_output(), limit, None)?;
    model = without_mpec(model)?;
    adapter::install_cancellation(&mut model, cancel.signal(), Arc::new(adapter::InterruptAudit::default()));
    let solved = model.solve();
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    let Some(best) = solved.best_sol() else {
        return Err(format!("SCIP found no completion ({:?})", solved.status()));
    };
    Ok(solved.orig_vars().iter().map(|variable| (variable.name(), best.val(variable))).collect())
}

/// Hand SCIP the completed seed. SCIP checks it against the original model
/// before storing it, so a seed it would not accept is reported rather than
/// half-used.
///
/// A seed makes weak dual reductions unsafe on this nonlinear model; see
/// [`adapter::SolveTuning::apply`] for the reproducer.
fn offer_seed(model: Model<ProblemCreated>, values: &HashMap<String, f64>) -> Result<(Model<ProblemCreated>, bool), String> {
    let solution = model.create_orig_sol();
    for variable in model.orig_vars() {
        if let Some(value) = values.get(&variable.name()) {
            solution.set_val(&variable, *value);
        }
    }
    let stored = model.add_sol(solution).is_ok();
    let model = model
        .set_bool_param("misc/allowweakdualreds", false)
        .map_err(|error| format!("configuring the seeded solve: {error:?}"))?;
    let model = without_mpec(model)?;
    Ok((model, stored))
}

/// Switch off SCIP's MPEC heuristic, in the seeded solve and the seed's
/// completion only.
///
/// On the chunked fixtures solved day by day, the seeded whole-horizon solve
/// hung in four runs out of four: MPEC handed Ipopt an NLP whose MUMPS
/// ordering (METIS) corrupted the heap, glibc aborted inside `malloc`, and
/// the abort left the solver thread waiting forever. With MPEC off the same
/// runs completed three times out of three, and the seed took the dynamic
/// FIFO fixture to its proven optimum. Unseeded solves keep SCIP's default.
fn without_mpec(model: Model<ProblemCreated>) -> Result<Model<ProblemCreated>, String> {
    model
        .set_int_param("heuristics/mpec/freq", -1)
        .map_err(|error| format!("switching off SCIP's MPEC heuristic: {error:?}"))
}

/// One line per finished solve, whatever became of it.
/// Diagnostic-only: what the captured project looks like and which
/// formulation families dominate the model. Built by a counting pass, so it
/// costs a second formulation walk but no solver memory.
fn log_model_structure(run_id: u64, input: &BlendInput) {
    let mut tonnes: Vec<f64> = input.ground.iter().map(|source| source.tonnes_t).collect();
    tonnes.sort_by(f64::total_cmp);
    let quantile = |q: f64| tonnes.get(((tonnes.len().saturating_sub(1)) as f64 * q).round() as usize).copied().unwrap_or(0.0);
    log::info!(
        "schedule run {run_id}: {} ground sources (tonnes min {:.0} / median {:.0} / p90 {:.0} / max {:.0}), {} multi-material, {} piles, {} loaders, {} tasks, {} movement candidates",
        input.ground.len(),
        quantile(0.0),
        quantile(0.5),
        quantile(0.9),
        quantile(1.0),
        input.ground.iter().filter(|source| source.material.len() > 1).count(),
        input.piles.len(),
        input.loaders.len(),
        input.tasks.len(),
        input.movements.len()
    );
    for task in &input.tasks {
        let rate = input
            .loaders
            .iter()
            .find(|loader| loader.id == task.loader)
            .map(|loader| loader.rates.iter().map(|rate| rate.dig_tph.max(rate.reclaim_tph)).fold(0.0_f64, f64::max))
            .unwrap_or(0.0);
        match &task.kind {
            TaskKind::Dig { sequence } => {
                let total: f64 = sequence
                    .iter()
                    .filter_map(|id| input.ground.iter().find(|source| source.id == *id))
                    .map(|source| source.tonnes_t)
                    .sum();
                log::info!(
                    "schedule run {run_id}: task {:?} loader {:?} priority {} window {:.1}-{:.1} h, dig sequence of {} blocks, {:.0} t, peak rate {:.0} t/h",
                    task.id,
                    task.loader,
                    task.priority,
                    task.window_start_h,
                    task.window_end_h,
                    sequence.len(),
                    total,
                    rate
                );
            }
            TaskKind::Reclaim { approved_sources, maximum_t } => log::info!(
                "schedule run {run_id}: task {:?} loader {:?} priority {} window {:.1}-{:.1} h, reclaim from {} piles, cap {:?}, peak rate {:.0} t/h",
                task.id,
                task.loader,
                task.priority,
                task.window_start_h,
                task.window_end_h,
                approved_sources.len(),
                maximum_t,
                rate
            ),
        }
    }
    for (family, size) in crate::model::schedule::optimisation::blended::formulation::family_sizes(input) {
        log::info!(
            "schedule run {run_id}: family {family:<12} {:>9} columns ({:>8} binary) {:>9} rows {:>10} entries",
            size.columns,
            size.binaries,
            size.rows,
            size.entries
        );
    }
}

fn log_outcome(out: &ScipCompletion) {
    log::info!(
        "schedule run {} diagnostics: {:?}; posted linear coefficient entries {}; objective {:?}, bound {:?}, gap {:?}",
        out.identity.run_id,
        out.diagnostics,
        out.sizes.linear_coefficient_entries,
        out.raw_objective,
        out.primary_bound,
        out.primary_gap
    );
    log::info!(
        "schedule run {}: {:?} (backend {:?}) under a {:?} limit and {:?} gap target; solve {:?}, replay {:?}; reason {:?}",
        out.identity.run_id,
        out.termination,
        out.backend_status,
        out.options.time_limit,
        out.options.relative_gap,
        out.timings.solver,
        out.timings.replay,
        out.diagnostic
    );
}

fn classify_status(status: Status, usable: bool) -> ScipTermination {
    match status {
        Status::Optimal | Status::GapLimit if usable => ScipTermination::Optimal,
        Status::Infeasible => ScipTermination::Infeasible,
        Status::Unbounded => ScipTermination::Unbounded,
        Status::UserInterrupt => ScipTermination::Cancelled,
        Status::TimeLimit
        | Status::NodeLimit
        | Status::TotalNodeLimit
        | Status::StallNodeLimit
        | Status::MemoryLimit
        | Status::SolutionLimit
        | Status::BestSolutionLimit
        | Status::RestartLimit
        | Status::PrimalLimit
        | Status::DualLimit => {
            if usable {
                ScipTermination::FeasibleLimit
            } else {
                ScipTermination::LimitNoIncumbent
            }
        }
        Status::Optimal | Status::GapLimit => ScipTermination::LimitNoIncumbent,
        Status::Unknown | Status::Inforunbd | Status::Terminate => ScipTermination::BackendFailure,
    }
}

/// Whether the answer that just came back still describes the project.
///
/// Three questions, not one: the request must still be the pending one, the
/// Setup gate must still name the same run, and every authored input the
/// model encoded must still hash the same. A rename changes none of them,
/// which is the point - and an edit to a chunk capacity, a truck roster or a
/// cashflow coefficient changes the last one, which is also the point.
#[cfg(all(test, feature = "blend-experiment"))]
fn request_current(pending: Option<ScipRunIdentity>, completed: ScipRunIdentity, current_inputs: Option<ScheduleRunInputs>, current_plan: u64, current_semantic: u64) -> bool {
    pending == Some(completed) && current_inputs == Some(completed.inputs) && current_plan == completed.plan_revision && current_semantic == completed.semantic
}

fn validate_input(input: &BlendInput, options: ScipSolveOptions, cancel: &CancelFlag) -> Result<(), String> {
    if options.time_limit.is_some_and(|limit| limit.is_zero() || !limit.as_secs_f64().is_finite()) {
        return Err("SCIP time limit must be positive and finite".into());
    }
    if options.relative_gap.is_some_and(|gap| !gap.is_finite() || !(0.0..=1.0).contains(&gap)) {
        return Err("SCIP relative gap must lie in 0..=1".into());
    }
    if input.segments_per_interval == 0 || input.intervals.is_empty() {
        return Err("the blended event grid is empty".into());
    }
    for (position, interval) in input.intervals.iter().enumerate() {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if interval.index != position
            || !interval.start_h.is_finite()
            || !interval.end_h.is_finite()
            || interval.start_h < 0.0
            || interval.end_h <= interval.start_h
            || (position > 0 && (interval.start_h - input.intervals[position - 1].end_h).abs() > 1e-9)
        {
            return Err(format!("invalid interval at position {position}"));
        }
    }
    let grades = input.grades.count();
    let mut pile_ids = BTreeSet::new();
    for pile in &input.piles {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if !pile_ids.insert(pile.id) || !pile.capacity_t.is_finite() || pile.capacity_t <= 0.0 {
            return Err(format!("invalid or duplicate pile {}", pile.id.0));
        }
        if !pile.opening_t.is_finite() || pile.opening_t < 0.0 || pile.opening_q.len() != grades || pile.opening_q.iter().any(|q| !q.is_finite() || *q < 0.0) {
            return Err(format!("invalid opening data on pile {}", pile.id.0));
        }
        if pile.chunks.is_empty() && !pile.chunk_opening.is_empty() {
            return Err(format!("chunk opening without chunks on pile {}", pile.id.0));
        }
        if !pile.chunk_closed.is_empty() && pile.chunk_closed.len() != pile.chunks.len() {
            return Err(format!("chunk closure flags do not match the chunks on pile {}", pile.id.0));
        }
        if pile.chunk_opening.len() > pile.chunks.len() || pile.chunks.iter().any(|cap| !cap.is_finite() || *cap <= 0.0) {
            return Err(format!("invalid chunks on pile {}", pile.id.0));
        }
        for (position, (tonnes, contained)) in pile.chunk_opening.iter().enumerate() {
            if contained.len() != grades
                || !tonnes.is_finite()
                || *tonnes < 0.0
                || *tonnes > pile.chunks[position]
                || contained.iter().any(|q| !q.is_finite() || *q < 0.0 || *q > *tonnes)
            {
                return Err(format!("invalid opening on pile {} chunk {position}", pile.id.0));
            }
        }
        let (opening_t, opening_q) = pile.total_opening(grades);
        if !opening_t.is_finite()
            || opening_t < 0.0
            || opening_t > pile.capacity_t
            || opening_q.len() != grades
            || opening_q.iter().any(|q| !q.is_finite() || *q < 0.0 || *q > opening_t)
        {
            return Err(format!("invalid opening on pile {}", pile.id.0));
        }
    }
    let mut ground_ids = BTreeSet::new();
    for source in &input.ground {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if !ground_ids.insert(source.id) || !source.tonnes_t.is_finite() || source.tonnes_t < 0.0 {
            return Err(format!("invalid or duplicate ground source {}", source.id.0));
        }
        if source.material.is_empty()
            || source.material.iter().any(|share| !share.fraction.is_finite() || !(0.0..=1.0).contains(&share.fraction))
            || (source.material.iter().map(|share| share.fraction).sum::<f64>() - 1.0).abs() > 1e-9
        {
            return Err(format!("invalid material shares on ground source {}", source.id.0));
        }
    }
    let loaders: BTreeSet<_> = input.loaders.iter().map(|loader| loader.id).collect();
    if loaders.len() != input.loaders.len() {
        return Err("duplicate loader".into());
    }
    for loader in &input.loaders {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if loader
            .rates
            .iter()
            .any(|rate| rate.interval >= input.intervals.len() || !rate.dig_tph.is_finite() || rate.dig_tph < 0.0 || !rate.reclaim_tph.is_finite() || rate.reclaim_tph < 0.0)
        {
            return Err(format!("invalid rates for loader {}", loader.id.0));
        }
    }
    let destinations: BTreeSet<_> = input.destinations.iter().map(|destination| destination.id).collect();
    if destinations.len() != input.destinations.len() {
        return Err("duplicate destination".into());
    }
    for destination in &input.destinations {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if destination.capacity_t.is_some_and(|cap| !cap.is_finite() || cap < 0.0)
            || destination.crusher_daily_t.iter().flatten().any(|cap| !cap.is_finite() || *cap < 0.0)
            || matches!(destination.kind, DestinationKind::Stockpile(pile) if !pile_ids.contains(&pile))
        {
            return Err(format!("invalid destination {}", destination.id.0));
        }
    }
    let trucks: BTreeSet<_> = input.trucks.iter().map(|truck| truck.id).collect();
    if trucks.len() != input.trucks.len() {
        return Err("duplicate truck class".into());
    }
    for truck in &input.trucks {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if truck.hours.len() < input.intervals.len() || truck.hours.iter().any(|hours| !hours.is_finite() || *hours < 0.0) {
            return Err(format!("invalid truck class {}", truck.id.0));
        }
    }
    let mut task_ids = BTreeSet::new();
    for task in &input.tasks {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if !task_ids.insert(task.id)
            || !loaders.contains(&task.loader)
            || !task.window_start_h.is_finite()
            || !task.window_end_h.is_finite()
            || task.window_end_h <= task.window_start_h
        {
            return Err(format!("invalid task {}", task.id.0));
        }
        match &task.kind {
            TaskKind::Dig { sequence } if sequence.is_empty() || sequence.iter().any(|id| !ground_ids.contains(id)) => {
                return Err(format!("invalid dig sequence on task {}", task.id.0));
            }
            TaskKind::Reclaim { approved_sources, maximum_t }
                if approved_sources.is_empty() || approved_sources.iter().any(|id| !pile_ids.contains(id)) || maximum_t.is_some_and(|value| !value.is_finite() || value < 0.0) =>
            {
                return Err(format!("invalid reclaim sources or cap on task {}", task.id.0));
            }
            _ => {}
        }
    }
    for (index, movement) in input.movements.iter().enumerate() {
        if cancel.is_cancelled() {
            return Ok(());
        }
        let source_exists = match movement.source {
            SourceId::Ground(id) => ground_ids.contains(&id),
            SourceId::Stockpile(id) => pile_ids.contains(&id),
        };
        if !source_exists
            || !matches!(
                (movement.activity, movement.source),
                (Activity::Dig, SourceId::Ground(_)) | (Activity::Reclaim, SourceId::Stockpile(_))
            )
            || !loaders.contains(&movement.loader)
            || !destinations.contains(&movement.destination)
            || !trucks.contains(&movement.truck)
            || !movement.truck_hours_per_tonne.is_finite()
            || movement.truck_hours_per_tonne < 0.0
            || movement.value_per_tonne().is_err()
            || (movement.activity == Activity::Dig && (0..grades).any(|g| input.grades.fraction(movement.material, g).is_none()))
        {
            return Err(format!("invalid movement candidate {index}"));
        }
    }
    for limit in &input.grade_limits {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if !destinations.contains(&limit.destination) || limit.grade >= grades || !limit.minimum.is_finite() || !(0.0..=1.0).contains(&limit.minimum) {
            return Err("invalid grade limit".into());
        }
    }
    let valid_bounds = |bounds: &[crate::model::schedule::optimisation::blended::input::GradeBound]| {
        bounds.iter().all(|bound| {
            bound.grade < grades
                && (bound.lower.is_some() || bound.upper.is_some())
                && [bound.lower, bound.upper]
                    .into_iter()
                    .flatten()
                    .all(|end| end.value.is_finite() && (0.0..=1.0).contains(&end.value))
                && match (bound.lower, bound.upper) {
                    (Some(lower), Some(upper)) => lower.value <= upper.value,
                    _ => true,
                }
        })
    };
    for qualification in &input.qualifications {
        if !loaders.contains(&qualification.loader)
            || !pile_ids.contains(&qualification.pile)
            || !destinations.contains(&qualification.destination)
            || qualification.alternatives.is_empty()
            || qualification.alternatives.iter().any(|alternative| !valid_bounds(&alternative.bounds))
        {
            return Err("invalid reclaim grade qualification".into());
        }
    }
    for payment in &input.conditional_values {
        if !input.movements.get(payment.candidate).is_some_and(|candidate| candidate.activity == Activity::Reclaim)
            || !payment.value_per_tonne.is_finite()
            || payment.bounds.is_empty()
            || !valid_bounds(&payment.bounds)
        {
            return Err("invalid conditional reclaim cashflow".into());
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "blend-experiment"))]
mod developer_checks {
    use std::{sync::mpsc, time::Duration};

    use super::*;
    use crate::model::{
        ReserveFieldId,
        schedule::optimisation::scip::scenarios::{competition_world, graded_blend_world},
    };

    fn identity(run_id: u64) -> ScipRunIdentity {
        ScipRunIdentity {
            run_id,
            inputs: ScheduleRunInputs {
                runtime: 1,
                generation: 1,
                fleet_revision: 1,
                tonnage_field: ReserveFieldId(1),
            },
            plan_revision: 1,
            document_revision: 1,
            semantic: 7,
        }
    }

    #[test]
    fn developer_scip_worker_validates_and_reports_limits() {
        let input = Arc::new(graded_blend_world(3));
        let result = execute_scip_blend(input, identity(1), ScipSolveOptions::default(), &CancelFlag::default(), &ScipActivity::default(), &|_| {});
        assert!(result.usable(), "{:?}: {:?}", result.termination, result.diagnostic);
        assert_eq!(result.backend_status, Some(Status::Optimal));
        assert!(result.replay.as_ref().is_some_and(|report| report.is_valid()));
        assert!(result.timings.solver > Duration::ZERO);
        assert!(result.timings.formulation > Duration::ZERO);

        let mut invalid = graded_blend_world(3);
        invalid.segments_per_interval = 0;
        let rejected = execute_scip_blend(
            Arc::new(invalid),
            identity(2),
            ScipSolveOptions::default(),
            &CancelFlag::default(),
            &ScipActivity::default(),
            &|_| {},
        );
        assert_eq!(rejected.termination, ScipTermination::InvalidInput);
        assert_eq!(rejected.timings.solver, Duration::ZERO);

        let cancelled = CancelFlag::default();
        cancelled.cancel();
        cancelled.cancel();
        let before = execute_scip_blend(
            Arc::new(graded_blend_world(3)),
            identity(3),
            ScipSolveOptions::default(),
            &cancelled,
            &ScipActivity::default(),
            &|_| {},
        );
        assert_eq!(before.termination, ScipTermination::Cancelled);
        assert_eq!(before.sizes.variables, 0);
        assert_eq!(before.timings.solver, Duration::ZERO);
        assert!(!before.usable());

        assert_eq!(classify_status(Status::TimeLimit, true), ScipTermination::FeasibleLimit);
        assert_eq!(classify_status(Status::TimeLimit, false), ScipTermination::LimitNoIncumbent);
        assert_eq!(classify_status(Status::Infeasible, false), ScipTermination::Infeasible);
        assert_eq!(classify_status(Status::Unbounded, false), ScipTermination::Unbounded);
    }

    #[test]
    fn developer_scip_real_time_limits_report_incumbent_separately() {
        let input = Arc::new(competition_world(24));
        for (run_id, limit) in [(40, Duration::from_millis(1)), (41, Duration::from_millis(250))] {
            let result = execute_scip_blend(
                Arc::clone(&input),
                identity(run_id),
                ScipSolveOptions {
                    time_limit: Some(limit),
                    ..Default::default()
                },
                &CancelFlag::default(),
                &ScipActivity::default(),
                &|_| {},
            );
            println!(
                "SCIP limit={limit:?} status={:?} termination={:?} usable={} objective={:?} solver={:?} overshoot={:?}",
                result.backend_status,
                result.termination,
                result.usable(),
                result.published_objective,
                result.timings.solver,
                result.solver_limit_overshoot
            );
            assert_eq!(result.backend_status, Some(Status::TimeLimit));
            assert_eq!(
                result.termination,
                if result.usable() {
                    ScipTermination::FeasibleLimit
                } else {
                    ScipTermination::LimitNoIncumbent
                }
            );
        }
    }

    #[test]
    fn developer_scip_currentness_and_invalid_replay_are_rejected() {
        let id = identity(10);
        assert!(request_current(Some(id), id, Some(id.inputs), id.plan_revision, id.semantic));
        assert!(!request_current(None, id, Some(id.inputs), id.plan_revision, id.semantic));
        assert!(!request_current(Some(identity(11)), id, Some(id.inputs), id.plan_revision, id.semantic));
        assert!(!request_current(Some(id), id, Some(id.inputs), id.plan_revision + 1, id.semantic));
        // An edit that changes no bar and no Setup input but does change a
        // coefficient the model encoded still retires the request.
        assert!(!request_current(Some(id), id, Some(id.inputs), id.plan_revision, id.semantic + 1));
        let result = execute_scip_blend(
            Arc::new(graded_blend_world(3)),
            id,
            ScipSolveOptions::default(),
            &CancelFlag::default(),
            &ScipActivity::default(),
            &|_| {},
        );
        assert!(result.usable());
        let mut replay = (**result.replay.as_ref().expect("replay")).clone();
        replay.issues.push("injected physical breach".into());
        let mut rejected = result;
        rejected.replay = Some(Arc::new(replay));
        assert!(!rejected.usable());
    }

    /// A reproducible, bounded developer check of cancellation during the
    /// actual solve. The phase and event counters prevent a pre-solve false
    /// positive; `try_recv` exercises nonblocking application-side polling.
    #[test]
    fn developer_scip_interrupts_an_active_solve() {
        let input = Arc::new(competition_world(48));
        let cancel = CancelFlag::default();
        let worker_cancel = cancel.clone();
        let activity = Arc::new(ScipActivity::default());
        let worker_activity = Arc::clone(&activity);
        let (tx, rx) = mpsc::channel();
        super::super::jobs::spawn_pool_task(move || {
            let result = execute_scip_blend(
                input,
                identity(20),
                ScipSolveOptions {
                    time_limit: Some(Duration::from_secs(12)),
                    ..Default::default()
                },
                &worker_cancel,
                &worker_activity,
                &|_| {},
            );
            tx.send(result).expect("developer receiver");
        });
        let waiting = Instant::now();
        while activity.phase() < 3 || activity.interrupt.callbacks.load(Ordering::Acquire) < 2 {
            assert!(waiting.elapsed() < Duration::from_secs(5), "SCIP never entered an observable solve");
            assert!(rx.try_recv().is_err(), "worker completed before cancellation request");
            std::thread::yield_now();
        }
        assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        let requested = Instant::now();
        cancel.cancel();
        let result = rx.recv_timeout(Duration::from_secs(10)).expect("SCIP did not return after cancellation");
        let latency = requested.elapsed();
        println!(
            "SCIP active-solve cancellation latency={latency:?} status={:?} callbacks={} interrupts={} solver={:?}",
            result.backend_status, result.event_callbacks, result.interrupt_calls, result.timings.solver
        );
        assert_eq!(result.termination, ScipTermination::Cancelled);
        assert!(!result.usable());
        assert!(result.interrupt_calls > 0, "cancellation did not reach SCIPinterruptSolve");
    }

    #[test]
    fn developer_scip_stops_during_formulation() {
        let input = Arc::new(competition_world(1000));
        let cancel = CancelFlag::default();
        let worker_cancel = cancel.clone();
        let activity = Arc::new(ScipActivity::default());
        let worker_activity = Arc::clone(&activity);
        let (tx, rx) = mpsc::channel();
        super::super::jobs::spawn_pool_task(move || {
            tx.send(execute_scip_blend(
                input,
                identity(30),
                ScipSolveOptions::default(),
                &worker_cancel,
                &worker_activity,
                &|_| {},
            ))
            .expect("developer receiver");
        });
        let waiting = Instant::now();
        while activity.formulation_checks.load(Ordering::Acquire) < 4 {
            assert!(waiting.elapsed() < Duration::from_secs(5), "formulation did not begin");
            std::thread::yield_now();
        }
        assert_eq!(activity.phase(), 2, "formulation completed before cancellation");
        let requested = Instant::now();
        cancel.cancel();
        let result = rx.recv_timeout(Duration::from_secs(5)).expect("formulation ignored cancellation");
        assert_eq!(result.termination, ScipTermination::Cancelled);
        assert_eq!(result.timings.solver, Duration::ZERO);
        println!(
            "SCIP formulation cancellation latency={:?} after {} checkpoints",
            requested.elapsed(),
            activity.formulation_checks.load(Ordering::Acquire)
        );
    }
}

/// Totals a reader asks for first, recomputed from the *replayed* rows rather
/// than from the solver's own report.
#[cfg(test)]
pub(crate) struct BlendTotals {
    pub(crate) mined_t: f64,
    pub(crate) reclaimed_t: f64,
    pub(crate) processed_t: f64,
}

#[cfg(test)]
pub(crate) fn totals(input: &BlendInput, solution: &BlendSolution) -> BlendTotals {
    let mut out = BlendTotals {
        mined_t: 0.0,
        reclaimed_t: 0.0,
        processed_t: 0.0,
    };
    for row in &solution.movements {
        let Some(candidate) = input.movements.get(row.candidate) else { continue };
        match candidate.activity {
            Activity::Dig => out.mined_t += row.tonnes_t,
            Activity::Reclaim => out.reclaimed_t += row.tonnes_t,
        }
        if input
            .destinations
            .iter()
            .any(|destination| destination.id == candidate.destination && destination.kind == DestinationKind::Crusher)
        {
            out.processed_t += row.tonnes_t;
        }
    }
    out
}
