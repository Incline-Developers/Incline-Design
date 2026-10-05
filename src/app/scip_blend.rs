//! Improve: the SCIP solve of one owned blended model, run inside the solver
//! process (see [`super::solver_process`]). Built only with the `scip`
//! feature.
//!
//! [`improve`] takes over from [`super::schedule_solve::execute_schedule`] once the first schedule
//! is in hand: it builds the whole-horizon model, solves it seeded with that
//! schedule, extracts owned rows and runs the independent replay, polling
//! cancellation throughout. When the hourly dispatch found no schedule, a
//! horizon longer than one day-by-day window is first solved a day at a
//! time; the stitched schedule is replayed against the whole horizon and
//! seeds the whole-horizon solve, and whichever replayed schedule is worth
//! more is the one kept. Beside such a run, HiGHS solves the model's linear
//! relaxation for a bound SCIP may not reach in time. No SCIP model, pointer
//! or solution wrapper leaves it; only owned, replayed rows do.

use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use russcip::{Model, ProblemCreated, Status, Variable, ffi, prelude::*};

use super::{
    jobs::CancelFlag,
    schedule_solve::{
        BackendStatus, DayByDay, PhaseTimings, ScheduleActivity, ScheduleCompletion, ScheduleRunIdentity, ScheduleSolveOptions, Seed, SolveTermination, adopt_seed,
        consistent_relaxation, exceeds_bound, log_outcome, relative_gap, validate_input,
    },
};
use crate::model::schedule::{
    optimisation::{
        Activity, DestinationKind, SourceId, TaskKind,
        blended::{
            formulation::{BlendColumns, BlendSizes},
            greedy,
            input::BlendInput,
            lp,
            relaxation::{RelaxationBound, relaxation_bound},
            replay::{BlendSolution, ExtractionAdjustments, ReplayReport, replay_cancellable},
            rolling::{self, Carry, Stitched, Window},
        },
        scip::{
            adapter::{self, SolveReport},
            blend::formulate_scip_with_cancel,
            experiments::extract_solution,
        },
    },
    result::{BoundSource, DayByDayRole, DayByDaySummary, StartMethod},
};

/// The binding Improve is built against, for the run's report.
const WRAPPER_VERSION: &str = "russcip 0.10.0";

impl ScheduleCompletion {
    /// What the solver process sends back for this completion.
    pub(crate) fn report(&self) -> ReportedCompletion {
        let issues = |pick: fn(&ReplayReport) -> &Vec<String>| self.replay.as_deref().map(pick).cloned().unwrap_or_default();
        ReportedCompletion {
            termination: self.termination,
            backend_status: self.backend_status,
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
    pub(crate) fn from_report(identity: ScheduleRunIdentity, options: ScheduleSolveOptions, input: Arc<BlendInput>, report: ReportedCompletion, cancel: &CancelFlag) -> Self {
        let mut out = Self::new(identity, options, input);
        out.termination = report.termination;
        out.backend_status = report.backend_status;
        if out.backend_version != lp::backend_name() {
            out.wrapper_version = WRAPPER_VERSION;
        }
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
            out.stop(SolveTermination::Cancelled, "cancelled during replay");
            return out;
        };
        let replayed = checked.replayed_objective;
        let slack = checked.boundary_value_slack;
        let valid = checked.is_valid();
        out.replay = Some(Arc::new(checked));
        if !valid {
            out.stop(SolveTermination::ValidationFailure, "independent blended replay rejected the solver process's schedule");
            return out;
        }
        let claimed = report.published_objective.unwrap_or(f64::NAN);
        let agrees = (claimed - replayed).abs() <= REPORT_AGREEMENT * replayed.abs().max(1.0);
        if !agrees {
            out.stop(
                SolveTermination::ValidationFailure,
                format!("the solver process reported a schedule worth {claimed}, which replays to {replayed}"),
            );
            return out;
        }
        if let Some(bound) = out.primary_bound
            && let Some(problem) = exceeds_bound(replayed - slack, bound)
        {
            out.stop(SolveTermination::ValidationFailure, problem);
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

/// A [`ScheduleCompletion`] as the solver process reports it (see
/// [`super::solver_process`]): the input stays with the app, which sent it,
/// and the replay is repeated by the app, so only its findings travel, to
/// explain a schedule that was not published.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ReportedCompletion {
    termination: SolveTermination,
    backend_status: Option<BackendStatus>,
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
    timings: PhaseTimings,
    diagnostics: crate::model::schedule::result::SolveDiagnostics,
    solver_limit_overshoot: Option<Duration>,
    backend_version: String,
    event_callbacks: u64,
    interrupt_calls: u64,
    day_by_day: Option<DayByDaySummary>,
}

impl From<Status> for BackendStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Unknown => Self::Unknown,
            Status::UserInterrupt => Self::UserInterrupt,
            Status::NodeLimit => Self::NodeLimit,
            Status::TotalNodeLimit => Self::TotalNodeLimit,
            Status::StallNodeLimit => Self::StallNodeLimit,
            Status::TimeLimit => Self::TimeLimit,
            Status::MemoryLimit => Self::MemoryLimit,
            Status::GapLimit => Self::GapLimit,
            Status::PrimalLimit => Self::PrimalLimit,
            Status::DualLimit => Self::DualLimit,
            Status::SolutionLimit => Self::SolutionLimit,
            Status::BestSolutionLimit => Self::BestSolutionLimit,
            Status::RestartLimit => Self::RestartLimit,
            Status::Optimal => Self::Optimal,
            Status::Infeasible => Self::Infeasible,
            Status::Unbounded => Self::Unbounded,
            Status::Inforunbd => Self::Inforunbd,
            Status::Terminate => Self::Terminate,
        }
    }
}

/// Improve the first schedule `attempt` holds, within what is left of the
/// run's budget since `budget_started`. Reached from
/// [`super::schedule_solve::execute_schedule`]
/// for a run that did not ask for the first schedule only.
///
/// `early` is handed the first schedule, completed as a publishable answer,
/// before the whole-horizon solve starts.
pub(crate) fn improve(
    mut out: ScheduleCompletion,
    mut attempt: DayByDay,
    budget_started: Instant,
    cancel: &CancelFlag,
    activity: &ScheduleActivity,
    early: &dyn Fn(&ScheduleCompletion),
) -> ScheduleCompletion {
    let options = out.options;
    out.backend_version = adapter::version();
    out.wrapper_version = WRAPPER_VERSION;
    let mut seed = None;
    let mut relaxation = None;
    let mut windows = None;
    if let DayByDay::Failed(reason) = &attempt {
        log::info!("schedule run {}: no hourly dispatch schedule: {reason}", out.identity.run_id);
        // A recalculation does not fall through to the solver: that takes
        // up to the whole solve budget, which an edit should never cost.
        if options.first_schedule_only {
            let reason = crate::i18n::tr!("schedule-first-schedule-failed", reason = reason.clone());
            out.stop(SolveTermination::BackendFailure, reason);
            return out;
        }
        if let Some(planned) = rolling::plan(&out.input, 0.0) {
            relaxation = RelaxationJob::start(&out.input, options.time_limit, cancel, out.identity.run_id);
            // Days alone first; with a look-ahead only when that fails (see
            // `rolling`), from the start and within the same share of the budget.
            let budget = options.time_limit.map(|limit| limit.mul_f64(DAY_BY_DAY_SHARE));
            let mut planned = planned;
            attempt = solve_day_by_day(&mut out, &planned, budget, options, cancel, activity);
            if let DayByDay::Failed(reason) = &attempt
                && let Some(ahead) = rolling::plan(&out.input, rolling::LOOKAHEAD_H)
            {
                log::warn!(
                    "schedule run {}: days solved alone failed ({reason}); solving them again with a {} h look-ahead",
                    out.identity.run_id,
                    rolling::LOOKAHEAD_H
                );
                planned = ahead;
                attempt = solve_day_by_day(
                    &mut out,
                    &planned,
                    budget.map(|budget| budget.saturating_sub(budget_started.elapsed())),
                    options,
                    cancel,
                    activity,
                );
            }
            windows = Some(planned.len());
        }
    } else if !options.first_schedule_only {
        relaxation = RelaxationJob::start(&out.input, options.time_limit, cancel, out.identity.run_id);
    }
    match attempt {
        DayByDay::Seed(found) if options.first_schedule_only => {
            adopt_seed(&mut out, *found, None, None, DayByDayRole::Only);
            return out;
        }
        DayByDay::Seed(found) => {
            let proved = relaxation.as_mut().and_then(RelaxationJob::ready);
            let mut shown = ScheduleCompletion::new(out.identity, options, Arc::clone(&out.input));
            shown.sizes = out.sizes;
            shown.timings = out.timings;
            adopt_seed(&mut shown, (*found).clone(), None, proved, DayByDayRole::Early);
            if let Some(bound) = shown.primary_bound
                && options.relative_gap.is_some_and(|target| gap_closed(found.solution.reported_objective, bound, target))
            {
                log::info!(
                    "schedule run {}: the first schedule is within the gap target of the relaxation bound; no whole-horizon solve",
                    out.identity.run_id
                );
                adopt_seed(&mut out, *found, None, proved, DayByDayRole::Proven);
                return out;
            }
            early(&shown);
            seed = Some(*found);
        }
        DayByDay::Failed(reason) => {
            if let Some(windows) = windows {
                log::warn!("schedule run {}: day-by-day start abandoned: {reason}", out.identity.run_id);
                out.day_by_day = Some(DayByDaySummary {
                    method: StartMethod::DayByDay,
                    windows,
                    seconds: budget_started.elapsed().as_secs_f64(),
                    value: None,
                    role: DayByDayRole::Improved,
                    failure: Some(reason),
                });
            }
        }
        DayByDay::Stop(reason, diagnostic) => {
            out.stop(reason, diagnostic);
            return out;
        }
    }
    let remaining = options.time_limit.map(|limit| limit.saturating_sub(budget_started.elapsed()));
    if let Some(found) = seed.take_if(|_| remaining.is_some_and(|left| left < WHOLE_HORIZON_MINIMUM)) {
        log::info!("schedule run {}: no time left for a whole-horizon solve; keeping the first schedule", out.identity.run_id);
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
                log::info!("schedule run {}: first schedule completed into a seed in {:.2?}", out.identity.run_id, started.elapsed());
                completed = Some(values);
            }
            Err(_) if cancel.is_cancelled() => {
                out.stop(SolveTermination::Cancelled, "cancelled while completing the first schedule into a seed");
                return out;
            }
            Err(problem) => log::warn!("schedule run {}: first schedule not offered to SCIP: {problem}", out.identity.run_id),
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
        out.stop(SolveTermination::Cancelled, "cancelled during formulation");
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
            out.stop(SolveTermination::BackendFailure, problem);
            return out;
        }
    };
    if let Some(values) = completed.as_ref() {
        model = match offer_seed(model, values) {
            Ok((model, stored)) => {
                if !stored {
                    log::warn!("schedule run {}: SCIP rejected the completed seed", out.identity.run_id);
                }
                model
            }
            Err(problem) => {
                out.stop(SolveTermination::BackendFailure, problem);
                return out;
            }
        };
    }
    // SCIP polls `interrupt`, which `watch_solve` raises for the run's own
    // cancellation or once the relaxation bound proves the seed.
    let interrupt = Arc::new(AtomicBool::new(false));
    adapter::install_cancellation(&mut model, Arc::clone(&interrupt), Arc::clone(&activity.interrupt));
    if cancel.is_cancelled() {
        out.stop(SolveTermination::Cancelled, "cancelled before solve");
        return out;
    }

    activity.set(3);
    let started = Instant::now();
    let (finished, proof) = (AtomicBool::new(false), AtomicBool::new(false));
    let proving = seed.as_ref().map(|found| found.solution.reported_objective).zip(options.relative_gap);
    let solved = std::thread::scope(|scope| {
        let (interrupt, finished_ref, proof, relaxation) = (&*interrupt, &finished, &proof, &mut relaxation);
        scope.spawn(move || watch_solve(cancel, interrupt, finished_ref, proof, proving, relaxation));
        let solved = model.solve();
        finished.store(true, Ordering::Release);
        solved
    });
    let solve_time = started.elapsed();
    out.timings.solver += solve_time;
    if proof.load(Ordering::Acquire)
        && !cancel.is_cancelled()
        && let Some(found) = seed.take()
    {
        log::info!(
            "schedule run {}: the relaxation bound proves the first schedule within the gap target; whole-horizon solve stopped after {solve_time:.2?}",
            out.identity.run_id
        );
        let proved = relaxation.as_mut().and_then(RelaxationJob::ready);
        adopt_seed(&mut out, found, None, proved, DayByDayRole::Proven);
        return out;
    }
    if let Some(limit) = remaining.or(options.time_limit) {
        out.solver_limit_overshoot = solve_time.checked_sub(limit);
    }
    let report = SolveReport::read(&solved);
    out.diagnostics = adapter::diagnostics(&solved, &activity.interrupt);
    out.backend_status = Some(report.status.into());
    out.raw_objective = report.objective;
    out.primary_bound = report.bound.is_finite().then_some(report.bound);
    out.primary_gap = report.gap;
    out.event_callbacks = activity.interrupt.callbacks.load(Ordering::Acquire);
    out.interrupt_calls = activity.interrupt.calls.load(Ordering::Acquire);
    if activity.interrupt.failed.load(Ordering::Acquire) {
        out.stop(SolveTermination::BackendFailure, "SCIPinterruptSolve rejected a solver-thread callback");
        return out;
    }
    if cancel.is_cancelled() {
        out.stop(SolveTermination::Cancelled, format!("cancelled during solve; backend status {:?}", report.status));
        return out;
    }

    activity.set(4);
    let started = Instant::now();
    let extracted = extract_solution(&solved, &columns, || cancel.is_cancelled());
    out.timings.extraction += started.elapsed();
    if cancel.is_cancelled() || extracted.is_err() {
        out.stop(SolveTermination::Cancelled, "cancelled during extraction");
        return out;
    }
    let Some(mut solution) = extracted.expect("checked extraction result") else {
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
    if let Some(chain) = out.input.drill_blast.as_ref() {
        solution.drill_blast.clone_from(&chain.fixed);
    }
    let checked = replay_cancellable(&out.input, &solution, &cancel.signal());
    out.timings.replay += started.elapsed();
    if cancel.is_cancelled() || checked.is_none() {
        out.stop(SolveTermination::Cancelled, "cancelled during replay");
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
        out.stop(SolveTermination::ValidationFailure, "independent blended replay rejected the incumbent");
        return out;
    }
    if let Some(bound) = out.primary_bound {
        // The bound is on the model's objective, which values deliveries near
        // a conditional grade boundary conservatively; the published figure
        // uses the authored boundary and may sit above it by that much.
        let slack = out.replay.as_ref().map_or(0.0, |report| report.boundary_value_slack);
        let published = out.published_objective.expect("replayed objective") - slack;
        if let Some(problem) = exceeds_bound(published, bound) {
            out.stop(SolveTermination::ValidationFailure, problem);
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
            if termination == SolveTermination::FeasibleLimit
                && let Some(raw) = out.raw_objective
                && options.relative_gap.is_some_and(|target| gap_closed(raw, bound, target))
            {
                termination = SolveTermination::Optimal;
            }
        }
    }
    if let Some(found) = seed {
        // Kept on its published value, whatever SCIP proved: an optimum of
        // the model's conservative valuation near a grade boundary can be
        // worth less as published than the seed.
        let improved = out.published_objective.expect("replayed objective") >= found.replay.replayed_objective;
        if !improved {
            let bound = out.primary_bound.filter(|_| out.bound_source == BoundSource::Scip);
            let proven = options
                .relative_gap
                .zip(out.primary_bound)
                .is_some_and(|(target, bound)| gap_closed(found.solution.reported_objective, bound, target));
            adopt_seed(&mut out, found, bound, proved, DayByDayRole::Kept);
            if proven && out.solution.is_some() {
                out.termination = SolveTermination::Optimal;
            }
            return out;
        }
        // Drill and blast happened as the dispatch simulated it.
        solution.drill_blast.clone_from(&found.solution.drill_blast);
        out.day_by_day = Some(found.summary);
    }
    out.termination = termination;
    if matches!(out.termination, SolveTermination::Optimal | SolveTermination::FeasibleLimit) {
        out.solution = Some(Arc::new(solution));
    }
    activity.set(6);
    log_outcome(&out);
    out
}

/// Pass the run's cancellation on to a whole-horizon solve until `finished`,
/// and stop the solve early once the relaxation bound proves the seed.
///
/// `proving` is the seed's model objective and the gap target. SCIP left to
/// itself would run to its time limit: a day-by-day schedule found in
/// seconds is usually proven by the relaxation well after the whole-horizon
/// solve has started, and SCIP's own bound on a long horizon stays loose.
/// Whatever SCIP has found by then is set aside; it cannot be worth more
/// than the gap target above the seed.
fn watch_solve(cancel: &CancelFlag, interrupt: &AtomicBool, finished: &AtomicBool, proof: &AtomicBool, proving: Option<(f64, f64)>, relaxation: &mut Option<RelaxationJob>) {
    let mut proving = proving.filter(|_| relaxation.is_some());
    while !finished.load(Ordering::Acquire) {
        if cancel.is_cancelled() {
            interrupt.store(true, Ordering::Release);
            return;
        }
        if let Some((raw, target)) = proving
            && let Some(bound) = relaxation.as_mut().and_then(RelaxationJob::ready)
        {
            proving = None;
            if gap_closed(raw, bound, target) {
                proof.store(true, Ordering::Release);
                interrupt.store(true, Ordering::Release);
                return;
            }
        }
        std::thread::sleep(WATCH_INTERVAL);
    }
}

/// Whether `bound` proves `raw` within the gap `target`: relatively, or, for
/// a schedule worth nothing or next to it, within the relaxation's own
/// tolerance, which no relative gap can express.
fn gap_closed(raw: f64, bound: f64, target: f64) -> bool {
    relative_gap(raw, bound).is_some_and(|gap| gap <= target) || (raw.is_finite() && bound.is_finite() && (bound - raw).abs() <= ABSOLUTE_GAP)
}

/// The value gap below which a schedule counts as proven whatever its size:
/// twice [`RELAXATION_MARGIN_ABSOLUTE`], so a schedule equal
/// to the relaxation's optimum closes it.
const ABSOLUTE_GAP: f64 = 2e-4;

/// How often [`watch_solve`] looks at the cancellation flag and the
/// relaxation bound.
const WATCH_INTERVAL: Duration = Duration::from_millis(50);

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

/// Solve the horizon a day at a time (see [`rolling`]) and stitch the kept
/// days into one schedule for the whole horizon, all within `budget`.
fn solve_day_by_day(
    out: &mut ScheduleCompletion,
    windows: &[Window],
    budget: Option<Duration>,
    options: ScheduleSolveOptions,
    cancel: &CancelFlag,
    activity: &ScheduleActivity,
) -> DayByDay {
    let started = Instant::now();
    let full = Arc::clone(&out.input);
    let mut carry = Carry::opening(&full);
    let mut stitched = Stitched::new();
    for (position, &window) in windows.iter().enumerate() {
        if cancel.is_cancelled() {
            return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during a day-by-day window".into());
        }
        let label = format!("window {}/{}", position + 1, windows.len());
        let limit = budget.map(|budget| budget.saturating_sub(started.elapsed()) / (windows.len() - position) as u32);
        if limit.is_some_and(|limit| limit.is_zero()) {
            return DayByDay::Failed(format!("{label}: no time left"));
        }
        let input = carry.window_input(&full, window);
        // A window's input is derived, not captured, so it is held to the
        // same checks before SCIP sees it.
        match validate_input(&input, Some(&full), options, cancel) {
            Ok(()) if cancel.is_cancelled() => return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during a day-by-day window".into()),
            Ok(()) => {}
            Err(problem) => return DayByDay::Failed(format!("{label}: invalid window input: {problem}")),
        }

        activity.set(2);
        let phase = Instant::now();
        let built = formulate_scip_with_cancel(&input, Some(&cancel.signal()), Some(&activity.formulation_checks));
        out.timings.formulation += phase.elapsed();
        let Ok(built) = built else {
            return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during formulation".into());
        };
        if built.sizes.variables > out.sizes.variables {
            out.sizes = built.sizes;
        }

        // A window starts from its dispatch schedule, so SCIP returns
        // nothing worse even when its first LP takes longer than the window
        // has; see `greedy`.
        let phase = Instant::now();
        let start = match dispatch_start(&input, limit.map(|limit| limit.mul_f64(SEED_COMPLETION_SHARE)), cancel) {
            Ok((values, value)) => {
                log::info!(
                    "schedule run {}: {label} starts from a dispatch schedule worth {value:.2}, completed in {:.2?}",
                    out.identity.run_id,
                    phase.elapsed()
                );
                Some(values)
            }
            Err(_) if cancel.is_cancelled() => return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during a day-by-day window".into()),
            Err(problem) => {
                log::warn!("schedule run {}: {label} has no dispatch start: {problem}", out.identity.run_id);
                None
            }
        };
        out.timings.solver += phase.elapsed();
        let limit = limit.map(|limit| limit.saturating_sub(phase.elapsed()));

        let model = if options.diagnostic_logging {
            built.model.show_output()
        } else {
            built.model.hide_output()
        };
        let mut model = match configure(model, limit, options.relative_gap) {
            Ok(model) => model,
            Err(problem) => return DayByDay::Stop(SolveTermination::BackendFailure, problem),
        };
        if let Some(values) = start.as_ref() {
            model = match offer_seed(model, values) {
                Ok((model, stored)) => {
                    if !stored {
                        log::warn!("schedule run {}: SCIP rejected {label}'s dispatch start", out.identity.run_id);
                    }
                    model
                }
                Err(problem) => return DayByDay::Stop(SolveTermination::BackendFailure, problem),
            };
        }
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
            return DayByDay::Stop(SolveTermination::BackendFailure, "SCIPinterruptSolve rejected a solver-thread callback".into());
        }
        if cancel.is_cancelled() {
            return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during a day-by-day window".into());
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
            Err(()) => return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during extraction".into()),
            Ok(None) => return DayByDay::Failed(format!("{label}: no schedule within {limit:?} ({status:?})")),
            Ok(Some(solution)) => solution,
        };

        activity.set(5);
        let phase = Instant::now();
        let checked = replay_cancellable(&input, &solution, &cancel.signal());
        out.timings.replay += phase.elapsed();
        let Some(checked) = checked else {
            return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during replay".into());
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

    let mut solution = stitched.finish();
    if !full.grade_targets.is_empty() {
        let Some(checked) = replay_cancellable(&full, &solution, &cancel.signal()) else {
            return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during target scoring".into());
        };
        solution.reported_objective -= checked.grade_target_penalty;
    }
    let phase = Instant::now();
    let checked = replay_cancellable(&full, &solution, &cancel.signal());
    out.timings.replay += phase.elapsed();
    let Some(checked) = checked else {
        return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during replay".into());
    };
    if !checked.is_valid() {
        for issue in checked.issues.iter().chain(&checked.grade_issues).take(5) {
            log::warn!("schedule run {}: stitched day-by-day schedule: {issue}", out.identity.run_id);
        }
        let issue = checked.issues.iter().chain(&checked.grade_issues).next().cloned().unwrap_or_default();
        return DayByDay::Failed(format!("the stitched schedule failed the whole-horizon replay: {issue}"));
    }
    let summary = DayByDaySummary {
        method: StartMethod::DayByDay,
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
    // A SCIP built from source here (`scip-source`) has no Ipopt, and so no
    // Ipopt parameters to set.
    // SAFETY: a parameter lookup on the thread that owns the model.
    let has_ipopt = unsafe { !ffi::SCIPgetParam(model.scip_ptr(), c"nlpi/ipopt/optfile".as_ptr()).is_null() };
    if has_ipopt && let Some(file) = ipopt_options_file() {
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
/// Only a SCIP with Ipopt has these: `scip-source` has none, but a
/// `scip-system` installation may. The prebuilt scipoptsuite-deploy
/// library's METIS path is broken. Called by MUMPS through
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

/// A dispatch schedule for `input` (see [`greedy`]), replayed and completed
/// into a start SCIP can be offered, or why there is none.
///
/// It is only ever a start: SCIP checks it against the model before storing
/// it, and whatever SCIP returns is replayed again on its own.
fn dispatch_start(input: &BlendInput, limit: Option<Duration>, cancel: &CancelFlag) -> Result<(HashMap<String, f64>, f64), String> {
    let mut solution = greedy::dispatch_cancellable(input, &cancel.signal())?.ok_or("cancelled")?;
    // The rows are the dispatcher's own, so the objective they report is
    // what the replay values them at.
    solution.reported_objective = replay_cancellable(input, &solution, &cancel.signal()).ok_or("cancelled")?.replayed_objective;
    let checked = replay_cancellable(input, &solution, &cancel.signal()).ok_or("cancelled")?;
    if !checked.is_valid() {
        let issue = checked.issues.iter().chain(&checked.grade_issues).next().cloned().unwrap_or_default();
        return Err(format!("the replay rejected the dispatch schedule: {issue}"));
    }
    Ok((complete_seed(input, &solution, limit, cancel)?, checked.replayed_objective))
}

/// Complete a schedule - the stitched day-by-day one, or a dispatch
/// schedule - into a full solution of its model, as values by variable name.
///
/// A second copy of the model is built with every movement and segment
/// duration held to the seed's value, give or take [`SEED_BAND_ABSOLUTE`]
/// and [`SEED_BAND_RELATIVE`], and a movement the seed does not make held at
/// zero, so what is left for SCIP is to fill in the state and indicator
/// columns those imply. An unchunked pile's state is held too, at exactly
/// what the seed's movements give it (see [`pin_pile_state`]). The completed
/// solution is SCIP's own, so it satisfies the model as SCIP checks it.
/// Should that hold leave no completion, the movements are tried alone.
///
/// The copy is solved without presolve. Presolved, SCIP checked the
/// completion in its reduced problem, and mapped back onto the original
/// columns it overran a loader's rate row by up to 0.007 t on a real week:
/// SCIP then refused it as a start. Nearly every column is already fixed,
/// so presolve has little to do. SCIP's own completion
/// heuristic was tried first and is not used: given the same values as a
/// partial solution it searched a neighbourhood of them instead, and on a
/// real week it spent the whole budget returning a schedule worth a
/// seventieth of the seed.
fn complete_seed(input: &BlendInput, seed: &BlendSolution, limit: Option<Duration>, cancel: &CancelFlag) -> Result<HashMap<String, f64>, String> {
    let started = Instant::now();
    match complete_seed_with(input, seed, true, limit, cancel) {
        // A stitched day-by-day seed may need its movements' band to meet
        // the model's rows, which exact pile state can rule out: try again
        // with the movements held alone, in what is left.
        Err(problem) if !cancel.is_cancelled() && limit.is_none_or(|limit| started.elapsed() < limit) => {
            let left = limit.map(|limit| limit.saturating_sub(started.elapsed()));
            complete_seed_with(input, seed, false, left, cancel).map_err(|again| format!("{problem}; with the movements alone, {again}"))
        }
        done => done,
    }
}

/// [`complete_seed`], with unchunked piles' state held exactly when
/// `pin_piles` is set.
fn complete_seed_with(input: &BlendInput, seed: &BlendSolution, pin_piles: bool, limit: Option<Duration>, cancel: &CancelFlag) -> Result<HashMap<String, f64>, String> {
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
    if pin_piles {
        pin_pile_state(input, seed, &built.columns, |column, value| {
            let value = value.clamp(column.lb(), column.ub());
            // SAFETY: as for `fix` above.
            unsafe {
                ffi::SCIPchgVarLb(scip, column.inner(), value);
                ffi::SCIPchgVarUb(scip, column.inner(), value);
            }
        });
    }
    let mut model = configure(built.model.hide_output(), limit, None)?;
    model = without_mpec(model)?;
    model = model
        .set_int_param("presolving/maxrounds", 0)
        .map_err(|error| format!("configuring the seed's completion: {error:?}"))?;
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

/// Hold each unchunked pile's state - opening and reclaimed tonnes, and the
/// contained quantity of each grade in both - at what the seed's movements
/// give it under perfect mixing, as the replay computes it.
///
/// With only the movements held, every mixing equality is left a bilinear
/// row for SCIP to search, and across a week of a reclaimed pile it ran out
/// of time without a completion. Held, the equalities are only checked.
/// Chunked piles mix per chunk and are left to SCIP.
fn pin_pile_state(input: &BlendInput, seed: &BlendSolution, columns: &BlendColumns<Variable>, mut pin: impl FnMut(&Variable, f64)) {
    let grades = input.grades.count();
    for pile in input.piles.iter().filter(|pile| pile.chunks.is_empty()) {
        let mut open_t = pile.opening_t;
        let mut open_q: Vec<f64> = (0..grades).map(|g| pile.opening_q.get(g).copied().unwrap_or(0.0)).collect();
        for interval in &input.intervals {
            let k = interval.index;
            let mut received_t = 0.0;
            let mut received_q = vec![0.0; grades];
            let mut reclaimed_t = 0.0;
            for row in seed.movements.iter().filter(|row| row.interval == k) {
                let candidate = &input.movements[row.candidate];
                let to_pile = input
                    .destinations
                    .iter()
                    .any(|destination| destination.id == candidate.destination && destination.kind == DestinationKind::Stockpile(pile.id));
                if to_pile {
                    received_t += row.tonnes_t;
                    for (g, quantity) in received_q.iter_mut().enumerate() {
                        *quantity += row.tonnes_t * input.grades.fraction(candidate.material, g).unwrap_or(0.0);
                    }
                }
                if candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id) {
                    reclaimed_t += row.tonnes_t;
                }
            }
            let blend: Vec<f64> = open_q.iter().map(|quantity| if open_t > 0.0 { quantity / open_t } else { 0.0 }).collect();
            if let Some(column) = columns.open_t.get(&(pile.id, k)) {
                pin(column, open_t);
            }
            if let Some(column) = columns.recl_t.get(&(pile.id, k)) {
                pin(column, reclaimed_t);
            }
            for g in 0..grades {
                if let Some(column) = columns.open_q.get(&(pile.id, k, g)) {
                    pin(column, open_q[g]);
                }
                if let Some(column) = columns.recl_q.get(&(pile.id, k, g)) {
                    pin(column, blend[g] * reclaimed_t);
                }
                open_q[g] += received_q[g] - blend[g] * reclaimed_t;
            }
            open_t += received_t - reclaimed_t;
        }
    }
}

/// Hand SCIP the completed seed. SCIP checks it against the original model
/// before storing it, so a seed it would not accept is reported rather than
/// half-used.
///
/// # Weak dual reductions are switched off: a correctness fix
///
/// Seeding a model with nonlinear constraints makes SCIP 10.0.2 stop at the
/// *seeded* objective and report it as `Optimal`, with a dual bound equal to
/// it, zero gap and zero nodes. The reproducer: maximise `c` subject to
/// `c * t = m`, `t = 10 + 10 b`, `m = 6`, `b` binary. The optimum is `c = 0.6`
/// at `b = 0`; seed `b = 1` (`c = 0.3`) and SCIP returns 0.3 as proven optimal.
///
/// | configuration | result |
/// |---|---|
/// | no seed | 0.6, correct |
/// | seeded, defaults | **0.3 reported optimal, bound 0.3, 0 nodes** |
/// | seeded, `presolving/maxrounds = 0` | 0.6, correct |
/// | seeded, `misc/allowweakdualreds = false` | 0.6, correct |
/// | seeded, `misc/allowstrongdualreds = false` | still 0.3 |
///
/// It is sign-independent and does not occur on a purely linear MILP seeded
/// the same way. Because the wrong run reports a zero gap, no status check
/// can catch it, and replay checks feasibility, not optimality certificates.
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
            TaskKind::Delay => log::info!(
                "schedule run {run_id}: task {:?} loader {:?} priority {} window {:.1}-{:.1} h, delay",
                task.id,
                task.loader,
                task.priority,
                task.window_start_h,
                task.window_end_h
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

fn classify_status(status: Status, usable: bool) -> SolveTermination {
    match status {
        Status::Optimal | Status::GapLimit if usable => SolveTermination::Optimal,
        Status::Infeasible => SolveTermination::Infeasible,
        Status::Unbounded => SolveTermination::Unbounded,
        Status::UserInterrupt => SolveTermination::Cancelled,
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
                SolveTermination::FeasibleLimit
            } else {
                SolveTermination::LimitNoIncumbent
            }
        }
        Status::Optimal | Status::GapLimit => SolveTermination::LimitNoIncumbent,
        Status::Unknown | Status::Inforunbd | Status::Terminate => SolveTermination::BackendFailure,
    }
}
