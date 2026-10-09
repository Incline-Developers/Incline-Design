//! One schedule run, from a captured model to a replayed schedule.
//!
//! [`execute_schedule`] validates the input, runs the hourly dispatch (see
//! [`greedy`]) and replays its schedule against the whole horizon, polling
//! cancellation throughout. That first schedule is the run's answer unless
//! the run asked to improve it, which only a build with SCIP can do (see
//! [`super::scip_blend`]). Run Period and Run All Periods reach it through
//! [`crate::app::schedule_run`], which owns the job, currentness and
//! publication.

use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use web_time::Instant;

use super::{jobs::CancelFlag, schedule_pipeline::ScheduleRunInputs};
use crate::model::schedule::{
    optimisation::{
        Activity, DestinationKind, SourceId, TaskKind,
        blended::{
            formulation::BlendSizes,
            greedy,
            input::BlendInput,
            lp,
            replay::{BlendSolution, ExtractionAdjustments, ReplayReport},
        },
    },
    result::{BoundSource, DayByDayRole, DayByDaySummary, StartMethod},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScheduleRunIdentity {
    pub(crate) run_id: u64,
    pub(crate) inputs: ScheduleRunInputs,
    pub(crate) plan_revision: u64,
    pub(crate) document_revision: u64,
    /// The complete experimental-run identity: every authored input that can
    /// change feasibility, the objective or the encoded model, plus the run
    /// options, and no presentation name. See [`App::schedule_semantic_key`](crate::app::App::schedule_semantic_key).
    ///
    /// Stage 5A carried only the dig-only Setup inputs and the bar revision,
    /// which say nothing about stockpile representation, grade units, truck
    /// calendars or cashflow coefficients - so an edit to any of those could
    /// not retire a run that had read them.
    pub(crate) semantic: u64,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScheduleSolveOptions {
    pub(crate) time_limit: Option<Duration>,
    pub(crate) relative_gap: Option<f64>,
    pub(crate) diagnostic_logging: bool,
    /// Stop at the hourly dispatch schedule: no relaxation bound and no
    /// whole-horizon solve. What a recalculation after an edit asks for; an
    /// Improve run leaves it off. Only a dispatcher that fails falls through
    /// to the solver path, since that is the only way to get a schedule.
    #[serde(default)]
    pub(crate) first_schedule_only: bool,
}

impl Default for ScheduleSolveOptions {
    fn default() -> Self {
        Self {
            time_limit: Some(Duration::from_secs(60)),
            relative_gap: Some(1e-4),
            diagnostic_logging: false,
            first_schedule_only: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SolveTermination {
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
pub(crate) struct PhaseTimings {
    pub(crate) input_validation: Duration,
    pub(crate) formulation: Duration,
    pub(crate) solver: Duration,
    pub(crate) extraction: Duration,
    pub(crate) replay: Duration,
}

pub(crate) struct ScheduleCompletion {
    pub(crate) identity: ScheduleRunIdentity,
    /// The options this run was solved under, held with the result so a
    /// currentness check can ask the same question the request did.
    pub(crate) options: ScheduleSolveOptions,
    pub(crate) input: Arc<BlendInput>,
    pub(crate) termination: SolveTermination,
    pub(crate) backend_status: Option<BackendStatus>,
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
    pub(crate) timings: PhaseTimings,
    pub(crate) diagnostics: crate::model::schedule::result::SolveDiagnostics,
    #[cfg(feature = "scip")]
    pub(crate) solver_limit_overshoot: Option<Duration>,
    pub(crate) backend_version: String,
    pub(crate) wrapper_version: &'static str,
    #[cfg(feature = "scip")]
    pub(crate) event_callbacks: u64,
    #[cfg(feature = "scip")]
    pub(crate) interrupt_calls: u64,
    /// Present when the horizon was first solved day by day.
    pub(crate) day_by_day: Option<DayByDaySummary>,
}

impl ScheduleCompletion {
    pub(crate) fn new(identity: ScheduleRunIdentity, options: ScheduleSolveOptions, input: Arc<BlendInput>) -> Self {
        Self {
            identity,
            options,
            input,
            termination: SolveTermination::BackendFailure,
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
            timings: PhaseTimings::default(),
            diagnostics: Default::default(),
            #[cfg(feature = "scip")]
            solver_limit_overshoot: None,
            backend_version: lp::backend_name().to_owned(),
            wrapper_version: "",
            #[cfg(feature = "scip")]
            event_callbacks: 0,
            #[cfg(feature = "scip")]
            interrupt_calls: 0,
            day_by_day: None,
        }
    }

    pub(crate) fn stop(&mut self, reason: SolveTermination, diagnostic: impl Into<String>) {
        self.termination = reason;
        self.diagnostic = Some(diagnostic.into());
        self.solution = None;
        self.published_objective = None;
        log_outcome(self);
    }

    pub(crate) fn usable(&self) -> bool {
        matches!(self.termination, SolveTermination::Optimal | SolveTermination::FeasibleLimit)
            && self.solution.is_some()
            && self.replay.as_ref().is_some_and(|report| report.is_valid())
    }
}

/// What the backend said of its own solve, in SCIP's terms: only an Improve
/// run's whole-horizon solve reports one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum BackendStatus {
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
pub(crate) struct ScheduleActivity {
    phase: AtomicU8,
    #[cfg(feature = "scip")]
    pub(crate) formulation_checks: std::sync::atomic::AtomicU64,
    #[cfg(feature = "scip")]
    pub(crate) interrupt: Arc<crate::model::schedule::optimisation::scip::adapter::InterruptAudit>,
}

impl ScheduleActivity {
    #[allow(dead_code, reason = "read by the developer lifecycle checks")]
    pub(crate) fn phase(&self) -> u8 {
        self.phase.load(Ordering::Acquire)
    }
    pub(crate) fn set(&self, phase: u8) {
        self.phase.store(phase, Ordering::Release);
    }
}

/// The stitched day-by-day schedule, replayed against the whole horizon.
#[derive(Clone)]
pub(crate) struct Seed {
    pub(crate) solution: BlendSolution,
    pub(crate) replay: ReplayReport,
    pub(crate) summary: DayByDaySummary,
}

pub(crate) enum DayByDay {
    Seed(Box<Seed>),
    /// No usable start; the whole-horizon solve runs unseeded.
    Failed(String),
    /// Cancelled, or a backend failure the whole run cannot continue past.
    Stop(SolveTermination, String),
}

/// The hourly dispatch schedule (see [`greedy`]), replayed against the
/// whole horizon.
pub(crate) fn hourly_dispatch(out: &mut ScheduleCompletion, cancel: &CancelFlag) -> DayByDay {
    let started = Instant::now();
    // The dispatcher replays what it returns, to choose between its
    // schedules, so that time is counted as the solver's.
    let (solution, checked) = match greedy::dispatch_cancellable(&out.input, &cancel.signal()) {
        Ok(Some(found)) => (found.solution, found.replay),
        Ok(None) => return DayByDay::Stop(SolveTermination::Cancelled, "cancelled during dispatch".into()),
        Err(reason) => return DayByDay::Failed(reason),
    };
    out.timings.solver += started.elapsed();
    if !checked.is_valid() {
        for issue in checked.issues.iter().chain(&checked.grade_issues).take(5) {
            log::warn!("schedule run {}: hourly dispatch schedule: {issue}", out.identity.run_id);
        }
        let issue = checked.issues.iter().chain(&checked.grade_issues).next().cloned().unwrap_or_default();
        return DayByDay::Failed(format!("the replay rejected it: {issue}"));
    }
    let summary = DayByDaySummary {
        method: StartMethod::Hourly,
        windows: out.input.intervals.len(),
        seconds: started.elapsed().as_secs_f64(),
        value: Some(checked.replayed_objective),
        role: DayByDayRole::Improved,
        failure: None,
    };
    log::info!(
        "schedule run {}: hourly dispatch schedule of {} intervals worth {:.2} in {:.2?}",
        out.identity.run_id,
        summary.windows,
        checked.replayed_objective,
        started.elapsed()
    );
    // Drill and blast: every later solve keeps the release times the
    // dispatch found (`blended::drill_blast`).
    if let (Some(_), Some(timeline)) = (out.input.drill_blast.as_ref(), solution.drill_blast.as_ref()) {
        let fixed = timeline.clone();
        let mut input = (*out.input).clone();
        if let Some(chain) = input.drill_blast.as_mut() {
            chain.fixed = Some(fixed);
        }
        out.input = Arc::new(input);
    }
    DayByDay::Seed(Box::new(Seed {
        solution,
        replay: checked,
        summary,
    }))
}

/// Publish the day-by-day schedule. `bound` is a whole-horizon SCIP dual
/// bound that still stands, and `proved` the relaxation's, if there are any;
/// the tighter is published.
pub(crate) fn adopt_seed(out: &mut ScheduleCompletion, found: Seed, bound: Option<f64>, proved: Option<f64>, role: DayByDayRole) {
    let published = found.replay.replayed_objective;
    let raw = found.solution.reported_objective;
    let checked = published - found.replay.boundary_value_slack;
    if let Some(bound) = bound
        && let Some(problem) = exceeds_bound(checked, bound)
    {
        out.stop(SolveTermination::ValidationFailure, problem);
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
        SolveTermination::Optimal
    } else {
        SolveTermination::FeasibleLimit
    };
    // Shown early, it is not the run's outcome yet.
    if role != DayByDayRole::Early {
        log_outcome(out);
    }
}

/// SCIP's own definition, so the figure reads the same as a solver gap.
/// A schedule equal to its bound has no gap, a zero-valued one included;
/// any other gap over zero has no relative size.
pub(crate) fn relative_gap(raw: f64, bound: f64) -> Option<f64> {
    if raw == bound && raw.is_finite() {
        return Some(0.0);
    }
    let smaller = raw.abs().min(bound.abs());
    (smaller > 0.0 && raw.signum() == bound.signum()).then(|| (bound - raw).abs() / smaller)
}

/// The relaxation bound, unless a replayed schedule is worth more than it.
///
/// That cannot happen with a correct relaxation, so it is reported; but the
/// schedule was replayed on its own and the relaxation is only a second
/// opinion on its quality, so the bound is dropped rather than the schedule.
pub(crate) fn consistent_relaxation(proved: Option<f64>, published: f64, run_id: u64) -> Option<f64> {
    let proved = proved?;
    match exceeds_bound(published, proved) {
        None => Some(proved),
        Some(problem) => {
            log::warn!("schedule run {run_id}: relaxation bound not used: {problem}");
            None
        }
    }
}

pub(crate) fn exceeds_bound(published: f64, bound: f64) -> Option<String> {
    let tolerance = 1e-4_f64.max(bound.abs() * 1e-8);
    (published > bound + tolerance).then(|| format!("published objective {published} exceeds SCIP bound {bound} beyond {tolerance}"))
}

pub(crate) fn log_outcome(out: &ScheduleCompletion) {
    // No SCIP solve ran: the hourly dispatch has no solver limit, gap target
    // or SCIP diagnostics to report, only what it published.
    if out.backend_status.is_none() {
        let outcome = match out.termination {
            SolveTermination::Optimal => "first schedule published, proven within the gap target".to_owned(),
            SolveTermination::FeasibleLimit => "first schedule published".to_owned(),
            other => format!("no schedule published ({other:?})"),
        };
        log::info!(
            "schedule run {}: {outcome}; objective {:?}, bound {:?}; solve {:?}, replay {:?}; reason {:?}",
            out.identity.run_id,
            out.published_objective,
            out.primary_bound,
            out.timings.solver,
            out.timings.replay,
            out.diagnostic
        );
        return;
    }
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

/// `horizon` is the whole horizon a day-by-day window was cut from, or `None`
/// when `input` is the captured horizon itself. A window may still name a
/// block its horizon holds and an earlier window finished: the window leaves
/// that block out, and the formulation gives it no columns. A block neither
/// holds is an error either way.
pub(crate) fn validate_input(input: &BlendInput, horizon: Option<&BlendInput>, options: ScheduleSolveOptions, cancel: &CancelFlag) -> Result<(), String> {
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
    if let Some(chain) = &input.drill_blast
        && (!chain.window_end_h.is_finite()
            || chain.window_end_h <= 0.0
            || chain.window_end_h > 24.0
            || chain.blasts.iter().any(|blast| blast.quantity.iter().any(|amount| !amount.is_finite() || *amount < 0.0))
            || chain
                .agents
                .iter()
                .any(|agent| agent.rates.len() != input.intervals.len() || agent.rates.iter().any(|rate| !rate.is_finite() || *rate < 0.0))
            || chain.tasks.iter().any(|task| {
                task.agent >= chain.agents.len()
                    || !task.start_h.is_finite()
                    || !task.end_h.is_finite()
                    || task.start_h < 0.0
                    || task.end_h <= task.start_h
                    || task.sequence.iter().any(|blast| *blast >= chain.blasts.len())
            })
            || chain.fixed.as_ref().is_some_and(|timeline| timeline.blasts.len() != chain.blasts.len()))
    {
        return Err("invalid drill and blast input".to_owned());
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
        if !pile.opening_t.is_finite()
            || pile.opening_t < 0.0
            || pile.opening_q.len() != grades
            || pile.opening_q.iter().any(|q| !q.is_finite() || *q < 0.0 || (pile.opening_t == 0.0 && *q != 0.0))
        {
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
                || contained.iter().any(|q| !q.is_finite() || *q < 0.0 || (*tonnes == 0.0 && *q != 0.0))
            {
                return Err(format!("invalid opening on pile {} chunk {position}", pile.id.0));
            }
        }
        let (opening_t, opening_q) = pile.total_opening(grades);
        if !opening_t.is_finite()
            || opening_t < 0.0
            || opening_t > pile.capacity_t
            || opening_q.len() != grades
            || opening_q.iter().any(|q| !q.is_finite() || *q < 0.0 || (opening_t == 0.0 && *q != 0.0))
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
    let referable: BTreeSet<_> = ground_ids
        .iter()
        .copied()
        .chain(horizon.into_iter().flat_map(|full| full.ground.iter().map(|source| source.id)))
        .collect();
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
    let mut target_ids = BTreeSet::new();
    let mut marginal_cost = 0.0;
    let ceilings = crate::model::schedule::optimisation::blended::input::grade_ceilings(input);
    for (index, target) in input.grade_targets.iter().enumerate() {
        if cancel.is_cancelled() {
            return Ok(());
        }
        let specification = &target.specification;
        if target.grade >= grades
            || !destinations.contains(&target.destination)
            || !target_ids.insert((target.destination, target.grade, target.day))
            || specification.validate().is_err()
        {
            return Err(format!("invalid grade target {index}"));
        }
        marginal_cost += specification
            .hinges()
            .iter()
            .map(|&(boundary, _, slope)| slope * boundary.abs().max((ceilings[target.grade] - boundary).abs()))
            .sum::<f64>();
        use crate::model::schedule::grade_targets::{TARGET_PERIOD_H, target_day};
        if input
            .intervals
            .iter()
            .any(|interval| interval.end_h > (f64::from(target_day(interval.start_h)) + 1.0) * TARGET_PERIOD_H + 1e-9)
        {
            return Err(format!("an interval crosses grade target {index}'s period boundary"));
        }
    }
    if !marginal_cost.is_finite() {
        return Err("grade target marginal costs cannot be represented".into());
    }
    let mut opening_ids = BTreeSet::new();
    for &(target, period, tonnes, contained) in &input.target_opening {
        if target >= input.grade_targets.len()
            || !opening_ids.insert((target, period))
            || !tonnes.is_finite()
            || tonnes < 0.0
            || !contained.is_finite()
            || contained < 0.0
            || (tonnes == 0.0 && contained != 0.0)
        {
            return Err("invalid carried grade target receipts".into());
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
            TaskKind::Dig { sequence } if sequence.is_empty() || sequence.iter().any(|id| !referable.contains(id)) => {
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
            SourceId::Ground(id) => referable.contains(&id),
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
        if !destinations.contains(&limit.destination) || limit.grade >= grades || !limit.minimum.is_finite() || limit.minimum < 0.0 {
            return Err("invalid grade limit".into());
        }
    }
    let valid_bounds = |bounds: &[crate::model::schedule::optimisation::blended::input::GradeBound]| {
        bounds.iter().all(|bound| {
            bound.grade < grades
                && (bound.lower.is_some() || bound.upper.is_some())
                && [bound.lower, bound.upper].into_iter().flatten().all(|end| end.value.is_finite() && end.value >= 0.0)
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

/// Runs on one thread. With SCIP this is the solver process's main one, and
/// no SCIP model, pointer or solution wrapper leaves it; only owned, replayed
/// rows do.
///
/// `early` is handed the first schedule, completed as a publishable answer,
/// before an Improve run starts its whole-horizon solve, so a caller can show
/// it while the rest of the run looks for a better one. It is called at most
/// once, and never for a run that stops at the first schedule.
pub(crate) fn execute_schedule(
    input: Arc<BlendInput>,
    identity: ScheduleRunIdentity,
    options: ScheduleSolveOptions,
    cancel: &CancelFlag,
    activity: &ScheduleActivity,
    early: &dyn Fn(&ScheduleCompletion),
) -> ScheduleCompletion {
    let mut out = ScheduleCompletion::new(identity, options, input);
    if cancel.is_cancelled() {
        out.stop(SolveTermination::Cancelled, "cancelled before validation");
        return out;
    }

    activity.set(1);
    let started = Instant::now();
    let validation = validate_input(&out.input, None, options, cancel);
    out.timings.input_validation = started.elapsed();
    if cancel.is_cancelled() {
        out.stop(SolveTermination::Cancelled, "cancelled during input validation");
        return out;
    }
    if let Err(problem) = validation {
        out.stop(SolveTermination::InvalidInput, problem);
        return out;
    }

    // The whole run shares one solve budget. The hourly dispatch schedule
    // comes first (see `greedy`): it takes milliseconds, is shown at once and
    // seeds an Improve run's whole-horizon solve.

    let budget_started = Instant::now();
    activity.set(3);
    let attempt = hourly_dispatch(&mut out, cancel);
    #[cfg(feature = "scip")]
    if !options.first_schedule_only {
        return super::scip_blend::improve(out, attempt, budget_started, cancel, activity, early);
    }
    let _ = (budget_started, early);
    match attempt {
        DayByDay::Seed(found) => adopt_seed(&mut out, *found, None, None, DayByDayRole::Only),
        DayByDay::Failed(reason) => {
            log::info!("schedule run {}: no hourly dispatch schedule: {reason}", out.identity.run_id);
            out.stop(SolveTermination::BackendFailure, crate::i18n::tr!("schedule-first-schedule-failed", reason = reason));
        }
        DayByDay::Stop(reason, diagnostic) => out.stop(reason, diagnostic),
    }
    out
}
