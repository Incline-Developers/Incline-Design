//! Developer-only, in-process SCIP job. Project capture and the Run Schedule
//! command remain separate work; this accepts an owned synthetic BlendInput.

use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

use russcip::Status;

use super::{
    App,
    jobs::{CancelFlag, JobKey},
    schedule_pipeline::ScheduleRunInputs,
};
use crate::model::schedule::optimisation::{
    Activity, DestinationKind, SourceId, TaskKind,
    blended::{
        formulation::BlendSizes,
        input::BlendInput,
        replay::{BlendSolution, ExtractionAdjustments, ReplayReport, replay_cancellable},
    },
    scip::{
        adapter::{self, InterruptAudit, SolveReport},
        blend::formulate_scip_with_cancel,
        experiments::extract_solution,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScipTermination {
    Optimal,
    FeasibleLimit,
    LimitNoIncumbent,
    Infeasible,
    Unbounded,
    Cancelled,
    /// The project could not be resolved into a model. The diagnostics name
    /// what to fix; nothing was solved and the previous result is untouched.
    CaptureFailure,
    InvalidInput,
    ValidationFailure,
    BackendFailure,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ScipPhaseTimings {
    pub(crate) input_validation: Duration,
    pub(crate) formulation: Duration,
    pub(crate) solver: Duration,
    pub(crate) extraction: Duration,
    pub(crate) replay: Duration,
}

/// Captured model approximations, never a claim of continuous mixing.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScipModelSettings {
    pub(crate) intervals: usize,
    pub(crate) segments_per_interval: usize,
    pub(crate) chunk_slots: usize,
    pub(crate) receipt_release_at_boundary: bool,
    pub(crate) chunk_slot_reuse: bool,
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
    pub(crate) adjustments: Option<ExtractionAdjustments>,
    pub(crate) sizes: BlendSizes,
    pub(crate) timings: ScipPhaseTimings,
    pub(crate) solver_limit_overshoot: Option<Duration>,
    pub(crate) backend_version: String,
    pub(crate) wrapper_version: &'static str,
    pub(crate) settings: ScipModelSettings,
    pub(crate) event_callbacks: u64,
    pub(crate) interrupt_calls: u64,
    /// What real-project capture produced, when this run came from a project
    /// rather than from a developer fixture.
    pub(crate) capture: Option<CaptureSummary>,
    /// Why capture refused, when it did. Never mixed with a solved result.
    pub(crate) capture_diagnostics: Vec<String>,
}

/// What capture resolved, kept with the result so the summary can be read in
/// the project's own terms without re-deriving anything.
#[derive(Clone, Debug)]
pub(crate) struct CaptureSummary {
    pub(crate) duration: Duration,
    pub(crate) fingerprint: u64,
    pub(crate) candidates: usize,
    pub(crate) ground_sources: usize,
    pub(crate) mixed_blocks: usize,
    pub(crate) event_budget_restricted: bool,
    pub(crate) estimated_columns: usize,
    /// Stated approximations this capture applied, verbatim.
    pub(crate) notes: Vec<String>,
    /// Dense stockpile id to the name the project gives it.
    pub(crate) pile_names: Vec<(u32, String)>,
}

impl ScipCompletion {
    fn new(identity: ScipRunIdentity, options: ScipSolveOptions, input: Arc<BlendInput>) -> Self {
        let settings = ScipModelSettings {
            intervals: input.intervals.len(),
            segments_per_interval: input.segments_per_interval,
            chunk_slots: input.piles.iter().map(|pile| pile.chunks.len()).sum(),
            receipt_release_at_boundary: true,
            chunk_slot_reuse: false,
        };
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
            adjustments: None,
            sizes: BlendSizes::default(),
            timings: ScipPhaseTimings::default(),
            solver_limit_overshoot: None,
            backend_version: adapter::version(),
            wrapper_version: "russcip 0.10.0",
            settings,
            event_callbacks: 0,
            interrupt_calls: 0,
            capture: None,
            capture_diagnostics: Vec::new(),
        }
    }

    fn stop(&mut self, reason: ScipTermination, diagnostic: impl Into<String>) {
        self.termination = reason;
        self.diagnostic = Some(diagnostic.into());
        self.solution = None;
        self.published_objective = None;
    }

    pub(crate) fn usable(&self) -> bool {
        matches!(self.termination, ScipTermination::Optimal | ScipTermination::FeasibleLimit)
            && self.solution.is_some()
            && self.replay.as_ref().is_some_and(|report| report.is_valid())
    }
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

/// Runs on one existing compute-pool worker. No SCIP model, pointer or
/// solution wrapper leaves this function; only owned, replayed rows do.
pub(crate) fn execute_scip_blend(input: Arc<BlendInput>, identity: ScipRunIdentity, options: ScipSolveOptions, cancel: &CancelFlag, activity: &ScipActivity) -> ScipCompletion {
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

    activity.set(2);
    let started = Instant::now();
    let built = formulate_scip_with_cancel(&out.input, Some(&cancel.signal()), Some(&activity.formulation_checks));
    out.timings.formulation = started.elapsed();
    if cancel.is_cancelled() || built.is_err() {
        out.stop(ScipTermination::Cancelled, "cancelled during formulation");
        return out;
    }
    let built = built.expect("checked formulation result");
    out.sizes = built.sizes;
    let columns = built.columns;

    let mut model = if options.diagnostic_logging {
        built.model.show_output()
    } else {
        built.model.hide_output()
    };
    if let Some(limit) = options.time_limit {
        model = match model.set_real_param("limits/time", limit.as_secs_f64()) {
            Ok(model) => model,
            Err(error) => {
                out.stop(ScipTermination::BackendFailure, format!("setting SCIP time limit: {error:?}"));
                return out;
            }
        };
    }
    if let Some(gap) = options.relative_gap {
        model = match model.set_real_param("limits/gap", gap) {
            Ok(model) => model,
            Err(error) => {
                out.stop(ScipTermination::BackendFailure, format!("setting SCIP gap: {error:?}"));
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
    out.timings.solver = started.elapsed();
    if let Some(limit) = options.time_limit {
        out.solver_limit_overshoot = out.timings.solver.checked_sub(limit);
    }
    let report = SolveReport::read(&solved);
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
    out.timings.extraction = started.elapsed();
    if cancel.is_cancelled() || extracted.is_err() {
        out.stop(ScipTermination::Cancelled, "cancelled during extraction");
        return out;
    }
    let Some(solution) = extracted.expect("checked extraction result") else {
        out.termination = classify_status(report.status, false);
        return out;
    };
    out.adjustments = Some(solution.adjustments);

    activity.set(5);
    let started = Instant::now();
    let checked = replay_cancellable(&out.input, &solution, &cancel.signal());
    out.timings.replay = started.elapsed();
    if cancel.is_cancelled() || checked.is_none() {
        out.stop(ScipTermination::Cancelled, "cancelled during replay");
        return out;
    }
    let checked = checked.expect("checked replay result");
    out.published_objective = Some(checked.replayed_objective);
    let valid = checked.is_valid();
    out.replay = Some(Arc::new(checked));
    if !valid {
        out.stop(ScipTermination::ValidationFailure, "independent blended replay rejected the incumbent");
        return out;
    }
    if let Some(bound) = out.primary_bound {
        let published = out.published_objective.expect("replayed objective");
        let tolerance = 1e-4_f64.max(bound.abs() * 1e-8);
        if published > bound + tolerance {
            out.stop(
                ScipTermination::ValidationFailure,
                format!("published objective {published} exceeds SCIP bound {bound} beyond {tolerance}"),
            );
            return out;
        }
    }
    out.termination = classify_status(report.status, true);
    if matches!(out.termination, ScipTermination::Optimal | ScipTermination::FeasibleLimit) {
        out.solution = Some(Arc::new(solution));
    }
    activity.set(6);
    out
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

impl App<'_> {
    /// Developer entry: submit an already captured synthetic blended input.
    ///
    /// Superseded for real work by [`Self::start_experimental_project_blend`],
    /// and kept because it is the one path that can submit a *fixture* - a
    /// scenario built by hand rather than resolved from a project - through
    /// the same job, cancellation and currentness machinery.
    #[allow(dead_code, reason = "the fixture submission path; the project path is what the UI action calls")]
    pub(crate) fn start_experimental_scip_blend(&mut self, input: BlendInput, inputs: ScheduleRunInputs, plan_revision: u64, options: ScipSolveOptions) -> Result<u64, String> {
        let Some(project) = self.workspace.active_project() else {
            return Err("no active project".into());
        };
        let document_revision = project.project.document.revision();
        if project.runtime_id != inputs.runtime || self.schedule_run_inputs().ok() != Some(inputs) || self.schedule_plan_revision() != plan_revision {
            return Err("captured schedule inputs are stale".into());
        }
        self.cancel_experimental_scip_blend();
        self.experimental_scip_serial += 1;
        let identity = ScipRunIdentity {
            run_id: self.experimental_scip_serial,
            inputs,
            plan_revision,
            document_revision,
            semantic: self.experimental_blend_key(options),
        };
        self.pending_experimental_scip = Some(identity);
        let input = Arc::new(input);
        let worker_input = Arc::clone(&input);
        let activity = Arc::new(ScipActivity::default());
        self.spawn_job_quietly(
            "Experimental SCIP blended schedule",
            vec![
                JobKey::ExperimentalScip {
                    runtime: inputs.runtime,
                    serial: identity.run_id,
                },
                JobKey::Project {
                    runtime_id: inputs.runtime,
                    document_revision,
                },
            ],
            move |cancel| Ok(execute_scip_blend(worker_input, identity, options, cancel, &activity)),
            move |app, result| {
                if app.pending_experimental_scip != Some(identity) {
                    return;
                }
                let current = request_current(
                    app.pending_experimental_scip,
                    identity,
                    app.schedule_run_inputs().ok(),
                    app.schedule_plan_revision(),
                    app.experimental_blend_key(options),
                );
                app.pending_experimental_scip = None;
                if !current {
                    return;
                }
                let completion = match result {
                    Ok(result) => result,
                    Err(error) => {
                        let mut failed = ScipCompletion::new(identity, options, input);
                        failed.stop(ScipTermination::BackendFailure, format!("worker failed: {error:#}"));
                        failed
                    }
                };
                log::info!(
                    "experimental SCIP run {}: {:?}, backend {:?}, {:?}",
                    identity.run_id,
                    completion.termination,
                    completion.backend_status,
                    completion.diagnostic
                );
                let completion = Arc::new(completion);
                if completion.usable() {
                    app.experimental_scip_result = Some(Arc::clone(&completion));
                }
                app.experimental_scip_diagnostics = Some(completion);
            },
        );
        Ok(identity.run_id)
    }

    /// Run the experimental optimiser on the project as it stands.
    ///
    /// The whole Stage 5B path in one action: capture the owned configuration
    /// here, resolve it into a model and solve it on a worker, replay the
    /// answer independently, and retain it only if it is still current.
    ///
    /// Deliberately separate from `Run Schedule`: the ordinary dispatcher,
    /// its Gantt, its calendar rows and its animation are untouched by this,
    /// and nothing here publishes into them.
    pub(crate) fn start_experimental_project_blend(&mut self) -> Result<u64, Vec<String>> {
        let Some(project) = self.workspace.active_project() else {
            return Err(vec![crate::i18n::tr!("planning-snapshot-no-project")]);
        };
        let runtime = project.runtime_id;
        let document_revision = project.project.document.revision();
        let options = {
            let experiment = project.project.document.schedule().experiment();
            ScipSolveOptions {
                time_limit: Some(Duration::from_secs_f64(experiment.solve_seconds)),
                relative_gap: Some(experiment.relative_gap),
                diagnostic_logging: false,
            }
        };
        // Bounded: a plan clone, a field list and two `Arc`s. Candidate
        // expansion is the worker's job.
        let captured = self.capture_experimental_snapshot();
        let snapshot = match captured {
            Ok(snapshot) => snapshot,
            Err(problems) => {
                let messages: Vec<String> = problems.iter().map(super::commands::schedule_capture::CaptureDiagnostic::describe).collect();
                return Err(messages);
            }
        };
        let inputs = snapshot.inputs();
        let plan_revision = snapshot.plan_revision;
        self.cancel_experimental_scip_blend();
        self.experimental_scip_serial += 1;
        let identity = ScipRunIdentity {
            run_id: self.experimental_scip_serial,
            inputs,
            plan_revision,
            document_revision,
            semantic: self.experimental_blend_key(options),
        };
        self.pending_experimental_scip = Some(identity);
        let activity = Arc::new(ScipActivity::default());
        self.spawn_job_quietly(
            "Experimental blended optimisation",
            vec![
                JobKey::ExperimentalScip { runtime, serial: identity.run_id },
                JobKey::Project {
                    runtime_id: runtime,
                    document_revision,
                },
            ],
            move |cancel| {
                let built = super::commands::schedule_capture::build(&snapshot, cancel);
                match built {
                    Ok(capture) => {
                        let input = Arc::new(capture.input);
                        let mut completion = execute_scip_blend(Arc::clone(&input), identity, options, cancel, &activity);
                        completion.capture = Some(CaptureSummary {
                            duration: capture.stats.duration,
                            fingerprint: capture.fingerprint,
                            candidates: capture.stats.candidates,
                            ground_sources: capture.stats.ground_sources,
                            mixed_blocks: capture.stats.mixed_blocks,
                            event_budget_restricted: capture.stats.event_budget_restricted,
                            estimated_columns: capture.stats.estimated_columns,
                            notes: capture.notes,
                            pile_names: capture.identities.piles.iter().map(|(id, _, name)| (id.0, name.clone())).collect(),
                        });
                        Ok(completion)
                    }
                    Err(problems) => {
                        let mut refused = ScipCompletion::new(identity, options, Arc::new(empty_input()));
                        if cancel.is_cancelled() {
                            refused.stop(ScipTermination::Cancelled, "cancelled during project capture");
                        } else {
                            refused.stop(ScipTermination::CaptureFailure, "the project could not be resolved into a blended model");
                            refused.capture_diagnostics = problems.iter().map(super::commands::schedule_capture::CaptureDiagnostic::describe).collect();
                        }
                        Ok(refused)
                    }
                }
            },
            move |app, result| {
                if app.pending_experimental_scip != Some(identity) {
                    return;
                }
                let current = request_current(
                    app.pending_experimental_scip,
                    identity,
                    app.schedule_run_inputs().ok(),
                    app.schedule_plan_revision(),
                    app.experimental_blend_key(options),
                );
                app.pending_experimental_scip = None;
                if !current {
                    // An edit landed while this was solving. The previous
                    // result stays exactly as it was, and this one is not
                    // published under it.
                    crate::userspace_warn!("{}", tr_experimental_superseded());
                    app.redraw_requested = true;
                    return;
                }
                let completion = match result {
                    Ok(completion) => completion,
                    Err(error) => {
                        let mut failed = ScipCompletion::new(identity, options, Arc::new(empty_input()));
                        failed.stop(ScipTermination::BackendFailure, format!("worker failed: {error:#}"));
                        failed
                    }
                };
                for diagnostic in &completion.capture_diagnostics {
                    crate::userspace_warn!("{diagnostic}");
                }
                let completion = Arc::new(completion);
                if completion.usable() {
                    app.experimental_scip_result = Some(Arc::clone(&completion));
                }
                app.experimental_scip_diagnostics = Some(completion);
                app.redraw_requested = true;
            },
        );
        self.redraw_requested = true;
        Ok(identity.run_id)
    }

    /// Cancellation is idempotent. The previous usable result stays held.
    pub(crate) fn cancel_experimental_scip_blend(&mut self) {
        if let Some(pending) = self.pending_experimental_scip.take() {
            self.cancel_jobs(|key| matches!(key, JobKey::ExperimentalScip { runtime, serial } if *runtime == pending.inputs.runtime && *serial == pending.run_id));
        }
    }

    pub(crate) fn advance_experimental_scip_blend(&mut self) {
        let Some(pending) = self.pending_experimental_scip else { return };
        if self.schedule_run_inputs().ok() != Some(pending.inputs) || self.schedule_plan_revision() != pending.plan_revision {
            self.cancel_experimental_scip_blend();
        }
    }

    /// Whether the retained experimental result still describes the project.
    ///
    /// `None` when nothing is retained. Presentation-only edits - a renamed
    /// stockpile, rule or machine, or a changed currency label - leave this
    /// `true`, because none of them changes a number the model encoded.
    pub(crate) fn experimental_scip_is_current(&self) -> bool {
        let Some(result) = self.experimental_scip_result.as_ref() else {
            return false;
        };
        let identity = result.identity;
        self.schedule_run_inputs().ok() == Some(identity.inputs)
            && self.schedule_plan_revision() == identity.plan_revision
            && self.experimental_blend_key(result.options) == identity.semantic
    }

    /// The complete experimental-run identity, cheaply.
    ///
    /// Every authored input the blended model encodes, hashed by stable id in
    /// a fixed order, plus the run options - and no presentation name, so a
    /// rename does not retire a result. This is deliberately built from the
    /// *project*, not by re-running capture: capture expands candidates, and
    /// a currentness check has to be affordable.
    ///
    /// It does not reuse the Setup pipeline's dig-only chain wholesale,
    /// because that chain is explicitly *not* complete for an optimised run:
    /// reclaim rates, opening stock, truck calendars and cashflow
    /// coefficients all ride separate chains there, on purpose.
    pub(crate) fn experimental_blend_key(&self, options: ScipSolveOptions) -> u64 {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // The run options: a result found under a 10-second limit is not the
        // same answer as one found under 600.
        options.time_limit.map(|limit| limit.as_nanos()).hash(&mut hasher);
        options.relative_gap.map(f64::to_bits).hash(&mut hasher);
        // The Setup gate and the authored bars, which between them cover the
        // project, the Solids run, the tonnage field, the fleet and every
        // bar's window, priority, ground and reclaim cap.
        match self.schedule_run_inputs() {
            Ok(inputs) => (0u8, inputs.runtime, inputs.generation, inputs.fleet_revision, inputs.tonnage_field.0).hash(&mut hasher),
            Err(_) => 1u8.hash(&mut hasher),
        }
        self.schedule_plan_revision().hash(&mut hasher);
        let Some(document) = self.workspace.active_document() else {
            return hasher.finish();
        };
        let plan = document.schedule();
        // Reserve field *definitions*: a condition or a grade reads a field
        // through one, so re-aggregating or deleting one is an input change.
        // Names are excluded.
        for field in document.reserve_fields() {
            field.id.0.hash(&mut hasher);
            format!("{:?}", field.aggregation).hash(&mut hasher);
        }
        // The reclaim half of the fleet, which the dig-only chain omits.
        for class in plan.classes() {
            (class.id.0, class.default_dig_rate_tph.to_bits(), class.default_reclaim_rate_tph.to_bits()).hash(&mut hasher);
        }
        for agent in plan.agents() {
            (agent.id.0, agent.class_id.0).hash(&mut hasher);
            agent.calendar.default_availability.to_bits().hash(&mut hasher);
            agent.calendar.default_utilisation.to_bits().hash(&mut hasher);
            for (period, value) in &agent.calendar.periods {
                period.0.hash(&mut hasher);
                value.availability.map(f64::to_bits).hash(&mut hasher);
                value.utilisation.map(f64::to_bits).hash(&mut hasher);
                value.rate_tph.map(f64::to_bits).hash(&mut hasher);
                value.reclaim_rate_tph.map(f64::to_bits).hash(&mut hasher);
            }
        }
        // Destinations: identity, kind, capacity, haul distance, crusher
        // budget and opening inventory - by id, never by name. The solids'
        // own kinds are read because that is what makes a solid a stockpile.
        let routing = plan.routing();
        routing.enabled.hash(&mut hasher);
        for solid in document.solids() {
            if let Some(kind) = crate::model::schedule::DestinationKind::of_solid(solid.kind) {
                let id = crate::model::schedule::DestinationId::Solid(solid.id);
                id.hash(&mut hasher);
                kind.hash(&mut hasher);
                routing.capacity_t(id).map(f64::to_bits).hash(&mut hasher);
                routing.distance_km(id).to_bits().hash(&mut hasher);
                if let Some(inventory) = routing.inventory(id) {
                    inventory.hash_content(&mut hasher);
                }
            }
        }
        for entry in &routing.standalone {
            entry.id.hash(&mut hasher);
            entry.kind.hash(&mut hasher);
            entry.capacity_t.map(f64::to_bits).hash(&mut hasher);
            entry.distance_km.to_bits().hash(&mut hasher);
            entry.inventory.hash_content(&mut hasher);
            entry.crusher.default_tpd.map(f64::to_bits).hash(&mut hasher);
            for (period, value) in &entry.crusher.periods {
                period.0.hash(&mut hasher);
                value.tonnes().map(f64::to_bits).hash(&mut hasher);
            }
        }
        // Rules and coefficients. Each of these hashes its own content and
        // deliberately leaves its name out.
        for rule in &routing.rules {
            rule.hash_content_public(&mut hasher);
        }
        plan.trucks().hash_content(&mut hasher);
        plan.cashflow().hash_content(&mut hasher);
        // Horizon, interval resolution, grade units and stockpile
        // representation - everything the experiment itself is told.
        plan.experiment().hash_content(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
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
        let result = execute_scip_blend(input, identity(1), ScipSolveOptions::default(), &CancelFlag::default(), &ScipActivity::default());
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
            tx.send(execute_scip_blend(input, identity(30), ScipSolveOptions::default(), &worker_cancel, &worker_activity))
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

/// A placeholder model for a run that never built one. Never solved: every
/// path that installs it has already stopped the completion with a reason.
fn empty_input() -> BlendInput {
    use crate::model::schedule::optimisation::blended::grade::GradeTable;

    BlendInput {
        intervals: Vec::new(),
        segments_per_interval: 0,
        grades: GradeTable::build(Vec::new(), &std::collections::BTreeMap::new()).expect("an empty grade table is always buildable"),
        piles: Vec::new(),
        loaders: Vec::new(),
        tasks: Vec::new(),
        ground: Vec::new(),
        destinations: Vec::new(),
        trucks: Vec::new(),
        movements: Vec::new(),
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    }
}

fn tr_experimental_superseded() -> String {
    crate::i18n::tr!("experiment-run-superseded")
}

/// Totals a reader asks for first, recomputed from the *replayed* rows rather
/// than from the solver's own report.
pub(crate) struct BlendTotals {
    pub(crate) mined_t: f64,
    pub(crate) reclaimed_t: f64,
    pub(crate) processed_t: f64,
}

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

impl App<'_> {
    /// Mirror the retained experimental answer into the editor state the
    /// Optimisation section reads.
    ///
    /// Formatting happens here, once, and only while that section is on
    /// screen: the UI draws rows and never computes a total, and a frame that
    /// is not showing the section costs nothing.
    pub(crate) fn mirror_experimental_blend(&mut self, showing: bool) {
        if !showing {
            if self.editor.experimental_blend.have_result || self.editor.experimental_blend.running {
                self.editor.experimental_blend = crate::ui::state::ExperimentalBlendView::default();
            }
            return;
        }
        let running = self.pending_experimental_scip.is_some();
        let current = self.experimental_scip_is_current();
        let mut view = crate::ui::state::ExperimentalBlendView {
            running,
            current,
            ..Default::default()
        };
        // The last attempt's diagnostics, whether or not it produced a result:
        // a failed or refused run must say why without disturbing what is
        // held.
        if let Some(latest) = self.experimental_scip_diagnostics.as_ref() {
            view.diagnostics.extend(latest.capture_diagnostics.iter().cloned());
            if !latest.usable()
                && let Some(reason) = latest.diagnostic.as_ref()
            {
                view.diagnostics.push(reason.clone());
            }
            if let Some(replay) = latest.replay.as_ref().filter(|_| !latest.usable()) {
                view.diagnostics.extend(replay.issues.iter().cloned());
                view.diagnostics.extend(replay.grade_issues.iter().cloned());
            }
        }
        if let Some(result) = self.experimental_scip_result.as_ref() {
            view.have_result = true;
            let input = &result.input;
            let horizon = input.intervals.last().map(|interval| interval.end_h).unwrap_or(0.0);
            let row = |label: &str, value: String| (label.to_owned(), value);
            view.rows.push(row("Horizon", format!("{horizon:.2} h over {} intervals", input.intervals.len())));
            view.rows.push(row("Termination", format!("{:?}", result.termination)));
            view.rows.push(row(
                "Validated objective",
                result.published_objective.map_or_else(|| "—".to_owned(), |value| format!("{value:.2}")),
            ));
            // The bound and gap are the backend's, about the model it solved,
            // and are reported as such. A valid incumbent is not optimality.
            view.rows
                .push(row("Bound", result.primary_bound.map_or_else(|| "—".to_owned(), |value| format!("{value:.2}"))));
            view.rows.push(row(
                "Relative gap",
                result.primary_gap.map_or_else(|| "not reported".to_owned(), |value| format!("{value:.4}")),
            ));
            if let Some(solution) = result.solution.as_ref() {
                let totals = totals(input, solution);
                view.rows.push(row("Mined", format!("{:.1} t", totals.mined_t)));
                view.rows.push(row("Reclaimed", format!("{:.1} t", totals.reclaimed_t)));
                view.rows.push(row("Processed", format!("{:.1} t", totals.processed_t)));
                let names = result.capture.as_ref().map(|capture| capture.pile_names.clone()).unwrap_or_default();
                for pile in &input.piles {
                    let supplied: f64 = solution
                        .movements
                        .iter()
                        .filter_map(|movement| {
                            input
                                .movements
                                .get(movement.candidate)
                                .filter(|candidate| candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id))
                                .map(|_| movement.tonnes_t)
                        })
                        .sum();
                    let name = names
                        .iter()
                        .find(|(id, _)| *id == pile.id.0)
                        .map(|(_, name)| name.clone())
                        .unwrap_or_else(|| format!("stockpile {}", pile.id.0));
                    view.rows.push(row(&format!("Supplied · {name}"), format!("{supplied:.1} t")));
                }
            }
            if let Some(replay) = result.replay.as_ref() {
                let names = result.capture.as_ref().map(|capture| capture.pile_names.clone()).unwrap_or_default();
                for closing in &replay.closing {
                    let name = names
                        .iter()
                        .find(|(id, _)| *id == closing.pile.0)
                        .map(|(_, name)| name.clone())
                        .unwrap_or_else(|| format!("stockpile {}", closing.pile.0));
                    // Grades, not contained quantities: an empty pile has no
                    // grade at all, and a dash says so rather than zero.
                    let grades: Vec<String> = closing
                        .contained
                        .iter()
                        .map(|contained| {
                            if closing.tonnes_t > 1e-6 {
                                format!("{:.4}", contained / closing.tonnes_t)
                            } else {
                                "—".to_owned()
                            }
                        })
                        .collect();
                    view.rows
                        .push(row(&format!("Closing · {name}"), format!("{:.1} t · {}", closing.tonnes_t, grades.join(" / "))));
                }
            }
            view.rows.push(row(
                "Solve",
                format!(
                    "{:.2} s (formulate {:.2} s, replay {:.2} s)",
                    result.timings.solver.as_secs_f64(),
                    result.timings.formulation.as_secs_f64(),
                    result.timings.replay.as_secs_f64()
                ),
            ));
            view.rows.push(row("Backend", format!("{} · {}", result.backend_version, result.wrapper_version)));
            if let Some(capture) = result.capture.as_ref() {
                view.rows.push(row(
                    "Capture",
                    format!(
                        "{:.2} s · {} candidates · {} ground sources",
                        capture.duration.as_secs_f64(),
                        capture.candidates,
                        capture.ground_sources
                    ),
                ));
                view.rows.push(row("Model identity", format!("{:016x}", capture.fingerprint)));
                view.rows.push(row(
                    "Model size",
                    format!(
                        "about {} movement columns over {} intervals x {} event positions",
                        capture.estimated_columns, result.settings.intervals, result.settings.segments_per_interval
                    ),
                ));
                if capture.mixed_blocks > 0 {
                    view.notes.push(format!(
                        "{} dig block(s) held more than one captured material; each was modelled as one block dug in its measured proportions",
                        capture.mixed_blocks
                    ));
                }
                view.notes.extend(capture.notes.iter().cloned());
                if capture.event_budget_restricted {
                    view.notes.push(format!(
                        "the derived execution-event budget was capped at {} positions per interval, so some source transitions may be unavailable",
                        result.settings.segments_per_interval
                    ));
                }
            }
            // Product approximations, stated every time: these are modelling
            // decisions, not solver limitations.
            if result.settings.receipt_release_at_boundary {
                view.notes
                    .push("receipts occupy pile capacity on arrival and join the reclaimable blend at the next interval boundary".to_owned());
            }
            if !result.settings.chunk_slot_reuse && result.settings.chunk_slots > 0 {
                view.notes.push("an emptied chunk slot is not reused within this horizon".to_owned());
            }
            if result.settings.chunk_slots > 0 {
                view.notes.push(format!(
                    "{} chunk slot(s) across all piles, which can limit total horizon receipts",
                    result.settings.chunk_slots
                ));
            }
            if let Some(replay) = result.replay.as_ref()
                && replay.chunk_dust_events > 0
            {
                view.notes.push(format!(
                    "{} chunk-lifecycle events totalling {:.6} t were below the {:.6} t indicator-tolerance threshold and were counted rather than discarded",
                    replay.chunk_dust_events, replay.chunk_dust_tonnes_t, replay.chunk_dust_threshold_t
                ));
            }
        }
        if self.editor.experimental_blend.rows != view.rows
            || self.editor.experimental_blend.notes != view.notes
            || self.editor.experimental_blend.diagnostics != view.diagnostics
            || self.editor.experimental_blend.running != view.running
            || self.editor.experimental_blend.current != view.current
            || self.editor.experimental_blend.have_result != view.have_result
        {
            self.editor.experimental_blend = view;
            self.redraw_requested = true;
        }
    }
}
