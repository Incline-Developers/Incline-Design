//! The Schedule workspace's Setup pipeline: what a schedule needs before it
//! can be calculated, and whether what was checked is still true.
//!
//! Built on the same rule as the Solids pipeline in
//! [`crate::app::planning_pipeline`], and for the same reason: opening a page,
//! selecting a loader or dragging a bar never validates anything. Run to Step
//! and Run All do. What a page does is show where each step stands and what
//! the last completed run found.
//!
//! Steps run in a strict order, and each one's input fingerprint includes the
//! fingerprint of the step before it, so one edit marks exactly the suffix of
//! steps it can reach:
//!
//! | Step | Reads | Produces |
//! | --- | --- | --- |
//! | Configuration | The schedule's name and the field read as tonnes | A field that can be read as tonnes |
//! | Loader Classes | Each class's dig rate | Rates the evaluator can use |
//! | Loader Agents | Each machine's class | A fleet with an effective rate each |
//! | Scheduling Readiness | Everything above, and the Solids run | [`ScheduleRunInputs`] |
//!
//! Three rules separate what is authored from what is derived, and they are
//! the point of the whole module:
//!
//! - **A run calculates from inputs captured when it started.** They travel
//!   with the result, so what a held result describes can be compared against
//!   what the project holds now.
//! - **An obsolete result is rejected, not published.** If the inputs moved
//!   while a step was working, its outcome is discarded with a stated reason.
//! - **Staleness is marked, never destroyed, and cancelling publishes
//!   nothing.** An edit leaves the last result on screen, labelled; a
//!   cancelled run leaves it exactly as it was.

use std::hash::{DefaultHasher, Hash, Hasher};

use crate::{
    app::planning_pipeline::{StageDiagnostic, StageNotReady, StageOutcome, StageState, StageStatus, StageSummary},
    i18n::tr,
    model::{ReserveAggregation, ReserveFieldId},
    ui::state::{ScheduleRepairTarget, ScheduleStep},
};

/// Exactly what one Schedule Setup run validated, captured when it started.
///
/// Held with the result rather than re-read from the project: a result that
/// cannot say what it was calculated from cannot be told apart from one that
/// is still true.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScheduleRunInputs {
    /// The project. A different one does not inherit this answer.
    pub(crate) runtime: u32,
    /// The Solids run its ground and tonnes were validated against.
    pub(crate) generation: u64,
    /// The configuration and fleet it validated, hashed. Checkpoint C's Run
    /// Schedule adds the authored bars to this; they are no input to Setup,
    /// which is why moving a bar does not retire a Setup run.
    pub(crate) fleet_revision: u64,
    pub(crate) tonnage_field: ReserveFieldId,
}

/// One completed Schedule Setup run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScheduleRunResult {
    pub(crate) inputs: ScheduleRunInputs,
    /// Which run produced it, so it can be named rather than merely dated.
    pub(crate) run: u64,
    /// How many dig blocks the Solids run offered it.
    pub(crate) blocks: usize,
}

/// Why the schedule cannot be calculated from what the project holds now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ScheduleNotReady {
    NoProject,
    NotRun(ScheduleStep),
    /// A run produced it, and an edit since has retired it. The result itself
    /// is kept and labelled; only its currency is gone.
    Stale(ScheduleStep),
    Running(ScheduleStep),
    Failed {
        step: ScheduleStep,
        message: String,
    },
}

impl ScheduleNotReady {
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::NoProject => tr!("planning-snapshot-no-project"),
            Self::NotRun(step) => tr!("schedule-not-ready-not-run", step = step.label()),
            Self::Stale(step) => tr!("schedule-not-ready-stale", step = step.label()),
            Self::Running(step) => tr!("schedule-not-ready-running", step = step.label()),
            Self::Failed { step, message } => tr!("schedule-not-ready-failed", step = step.label(), message = message.clone()),
        }
    }
}

/// One turn of the runner, kept free of jobs and of the app so the scheduling
/// half can be driven straight through in a test.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScheduleRunStep {
    Idle,
    Start(ScheduleStep),
    /// Already current from an earlier run; take the next one.
    Skip,
    Evaluate(ScheduleStep),
}

/// Why a settled outcome was thrown away instead of published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Obsolete {
    /// It belongs to a run that has since been superseded or cancelled.
    Superseded,
    /// It belongs to a step that is no longer the running one.
    NotRunning,
}

/// The Schedule Setup pipeline's state for one open project.
#[derive(Debug)]
pub(crate) struct SchedulePipeline {
    /// Which project this belongs to; a different one starts over.
    pub(crate) runtime: u32,
    steps: [StageStatus; ScheduleStep::ALL.len()],
    fingerprints: [u64; ScheduleStep::ALL.len()],
    /// Increments on every Run to Step or Run All.
    generation: u64,
    queue: Vec<ScheduleStep>,
    running: Option<ScheduleStep>,
    /// What the last completed run captured. Kept across edits and across
    /// cancellations - it is marked stale by [`Self::readiness`], never
    /// deleted, so the page can say what was true when it was last checked.
    result: Option<ScheduleRunResult>,
}

impl SchedulePipeline {
    pub(crate) fn new(runtime: u32) -> Self {
        Self {
            runtime,
            steps: Default::default(),
            fingerprints: [0; ScheduleStep::ALL.len()],
            generation: 0,
            queue: Vec::new(),
            running: None,
            result: None,
        }
    }

    pub(crate) fn status(&self, step: ScheduleStep) -> &StageStatus {
        &self.steps[step.index()]
    }

    fn status_mut(&mut self, step: ScheduleStep) -> &mut StageStatus {
        &mut self.steps[step.index()]
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.is_some() || !self.queue.is_empty()
    }

    pub(crate) fn step_inputs_match(&self, step: ScheduleStep) -> bool {
        self.steps[step.index()].completed_inputs == Some(self.fingerprints[step.index()])
    }

    /// The step that has to run before `step` can, if any.
    pub(crate) fn blocked_by(&self, step: ScheduleStep) -> Option<ScheduleStep> {
        ScheduleStep::ALL
            .into_iter()
            .take(step.index())
            .find(|earlier| !self.steps[earlier.index()].state.is_current())
    }

    /// Begin a fresh run through the selected step, retiring all previous
    /// statuses. The held result is left alone: it is retired by the run that
    /// replaces it, not by the one that starts.
    fn restart_through(&mut self, step: ScheduleStep) -> bool {
        if self.is_running() {
            return false;
        }
        self.steps = Default::default();
        self.generation += 1;
        self.queue = ScheduleStep::ALL.into_iter().take(step.index() + 1).collect();
        for queued in self.queue.clone() {
            self.status_mut(queued).state = StageState::Queued;
        }
        true
    }

    pub(crate) fn next_step(&mut self) -> ScheduleRunStep {
        if let Some(step) = self.running {
            return ScheduleRunStep::Evaluate(step);
        }
        let Some(next) = (!self.queue.is_empty()).then(|| self.queue.remove(0)) else {
            return ScheduleRunStep::Idle;
        };
        if self.status(next).state.is_current() && self.step_inputs_match(next) {
            return ScheduleRunStep::Skip;
        }
        ScheduleRunStep::Start(next)
    }

    pub(crate) fn start(&mut self, step: ScheduleStep) {
        self.running = Some(step);
        self.status_mut(step).state = StageState::Running;
    }

    /// The run a step that is starting now belongs to, to be handed back to
    /// [`Self::settle`] with that step's outcome.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Apply one step's outcome, and say whether the run should stop here.
    ///
    /// `generation` is the run the work was started under. A step that
    /// finishes after its run was superseded or cancelled has computed against
    /// inputs the project has moved on from, so its outcome is discarded with
    /// the reason rather than published over a newer answer.
    pub(crate) fn settle(&mut self, step: ScheduleStep, outcome: StageOutcome, generation: u64) -> Result<bool, Obsolete> {
        if generation != self.generation {
            return Err(Obsolete::Superseded);
        }
        if self.running != Some(step) {
            return Err(Obsolete::NotRunning);
        }
        let fingerprint = self.fingerprints[step.index()];
        match outcome {
            StageOutcome::Working { message } => {
                let status = self.status_mut(step);
                status.state = StageState::Running;
                status.message = message;
                Ok(true)
            }
            StageOutcome::Settled { diagnostics, entities } => {
                let blocking = diagnostics.iter().filter(|entry| entry.blocking).count();
                let status = self.status_mut(step);
                status.diagnostics = diagnostics;
                status.run_generation = generation;
                status.message = None;
                if blocking > 0 {
                    status.state = StageState::Failed;
                    status.message = Some(tr!("stage-failed-count", count = blocking.to_string()));
                    self.running = None;
                    for later in std::mem::take(&mut self.queue) {
                        let status = self.status_mut(later);
                        status.state = StageState::Blocked;
                        status.message = Some(tr!("stage-blocked-by", stage = step.label()));
                    }
                    return Ok(true);
                }
                status.state = StageState::Complete;
                status.completed_inputs = Some(fingerprint);
                status.last_success = Some(StageSummary { entities, generation });
                self.running = None;
                Ok(false)
            }
        }
    }

    /// Publish the captured inputs of a run that reached the end.
    ///
    /// Called only by the readiness step, and only from the run that computed
    /// them: a result carrying another run's number would describe inputs it
    /// never saw.
    pub(crate) fn publish(&mut self, result: ScheduleRunResult) -> Result<(), Obsolete> {
        if result.run != self.generation {
            return Err(Obsolete::Superseded);
        }
        self.result = Some(result);
        Ok(())
    }

    /// Whether a completed run stands for these exact inputs.
    ///
    /// Every step, not only the last: the readiness step cannot be current
    /// over configuration an earlier step has since been marked stale for.
    pub(crate) fn readiness(&self, fingerprints: &[u64; ScheduleStep::ALL.len()]) -> Result<&ScheduleRunResult, ScheduleNotReady> {
        for step in ScheduleStep::ALL {
            let status = self.status(step);
            match status.state {
                StageState::Complete if status.completed_inputs == Some(fingerprints[step.index()]) => {}
                StageState::Complete | StageState::Stale => return Err(ScheduleNotReady::Stale(step)),
                StageState::Queued | StageState::Running => return Err(ScheduleNotReady::Running(step)),
                StageState::Failed | StageState::Blocked => {
                    return Err(ScheduleNotReady::Failed {
                        step,
                        message: status.message.clone().unwrap_or_else(|| status.state.label()),
                    });
                }
                StageState::NotRun | StageState::Cancelled => return Err(ScheduleNotReady::NotRun(step)),
            }
        }
        // Complete steps with no published result would be a run that settled
        // without capturing what it settled over; say so rather than letting
        // the Gantt calculate from nothing.
        self.result
            .as_ref()
            .filter(|result| result.inputs.fleet_revision == fingerprints[ScheduleStep::Readiness.index()])
            .ok_or(ScheduleNotReady::NotRun(ScheduleStep::Readiness))
    }

    /// Mark this step and every step after it as no longer current.
    ///
    /// Returns whether this stopped a run that was in flight.
    fn invalidate_from(&mut self, step: ScheduleStep) -> bool {
        // A queued step has not read anything yet: it will see these inputs
        // when its turn comes. Only a published result, or one being computed
        // right now, can end up describing inputs it never saw.
        let stopped = self.running.is_some_and(|running| running.index() >= step.index());
        for later in ScheduleStep::ALL.into_iter().skip(step.index()) {
            let status = self.status_mut(later);
            match status.state {
                StageState::Complete => status.state = StageState::Stale,
                StageState::Running => status.state = StageState::Cancelled,
                StageState::Queued if stopped => status.state = StageState::Cancelled,
                _ => {}
            }
        }
        if stopped {
            self.queue.clear();
            self.running = None;
        }
        stopped
    }

    /// Stop whatever is queued or running. The held result is untouched: a
    /// cancelled run publishes nothing, and takes nothing away either.
    fn cancel(&mut self) {
        let affected: Vec<_> = std::mem::take(&mut self.queue).into_iter().chain(self.running.take()).collect();
        for step in affected {
            let status = self.status_mut(step);
            if status.state.is_busy() {
                status.state = StageState::Cancelled;
                status.message = Some(tr!("stage-state-cancelled"));
            }
        }
        // A run stopped part way through leaves its earlier steps complete but
        // the run itself unfinished, so the next result cannot be allowed to
        // arrive under this number.
        self.generation += 1;
    }
}

fn hash_of(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

impl crate::app::App<'_> {
    /// Refresh the Schedule Setup pipeline against the project as it stands,
    /// without running anything.
    ///
    /// Called from [`crate::app::App::sync_planning_pipeline`] *after* it, not
    /// beside it: the readiness step's inputs include where the Solids run
    /// stands, so refreshing the two in the other order would fingerprint this
    /// pipeline against last frame's Solids state.
    ///
    /// Cheap enough to call every frame - it hashes configuration and the
    /// fleet, and asks the Solids pipeline for its status rather than for its
    /// artifacts.
    pub(crate) fn sync_schedule_pipeline(&mut self) {
        let Some(runtime) = self.workspace.active_project().map(|project| project.runtime_id) else {
            self.cancel_jobs(|key| matches!(key, crate::app::jobs::JobKey::ScheduleRun { .. }));
            self.schedule_pipeline = None;
            // A calculated schedule describes one project's ground; it does
            // not outlive the project it was calculated for.
            self.schedule_calculation = None;
            self.pending_schedule_run = None;
            self.schedule_run_diagnostics = None;
            self.mirror_schedule_stages();
            return;
        };
        if self.schedule_pipeline.as_ref().is_none_or(|pipeline| pipeline.runtime != runtime) {
            self.cancel_jobs(|key| matches!(key, crate::app::jobs::JobKey::ScheduleRun { .. }));
            self.pending_schedule_run = None;
            self.schedule_pipeline = Some(SchedulePipeline::new(runtime));
            self.schedule_run_diagnostics = None;
        }
        let fingerprints = self.schedule_fingerprints();
        let mut earliest_change = None;
        {
            let pipeline = self.schedule_pipeline.as_mut().expect("just ensured");
            for step in ScheduleStep::ALL {
                if pipeline.fingerprints[step.index()] != fingerprints[step.index()] {
                    earliest_change = Some(earliest_change.map_or(step, |first: ScheduleStep| if step.index() < first.index() { step } else { first }));
                }
            }
            pipeline.fingerprints = fingerprints;
        }
        if let Some(step) = earliest_change {
            let stopped = self.schedule_pipeline.as_mut().expect("just ensured").invalidate_from(step);
            if stopped {
                crate::userspace_warn!("{}", tr!("schedule-run-stopped-by-edit", step = step.label()));
            }
        }
        {
            let pipeline = self.schedule_pipeline.as_mut().expect("just ensured");
            for step in ScheduleStep::ALL {
                let current = pipeline.fingerprints[step.index()];
                let status = pipeline.status_mut(step);
                if status.state == StageState::Complete && status.completed_inputs != Some(current) {
                    status.state = StageState::Stale;
                }
            }
        }
        self.advance_schedule_run();
        // After the pipeline, never before it: a run in flight is checked
        // against the gate as it stands now, and this is where "now" is
        // established.
        self.advance_schedule_calculation();
        self.mirror_schedule_stages();
    }

    /// Input fingerprints for all four steps, each chained onto the one before
    /// it so an edit invalidates exactly the suffix it can reach.
    ///
    /// The authored bars are deliberately absent: they are the Gantt's to
    /// author and checkpoint C's Run Schedule to consume, and a Setup run that
    /// went stale every time a bar moved would say nothing about the setup.
    pub(crate) fn schedule_fingerprints(&self) -> [u64; ScheduleStep::ALL.len()] {
        let Some(document) = self.workspace.active_document() else {
            return [0; ScheduleStep::ALL.len()];
        };
        let plan = document.schedule();

        // The chosen field *and* how it aggregates: re-aggregating a field
        // away from Sum is exactly the edit that makes a completed run wrong
        // without changing the choice itself.
        let field = plan.tonnage_field().and_then(|id| {
            document
                .reserve_fields()
                .iter()
                .find(|field| field.id == id)
                .map(|field| (id.0, format!("{:?}", field.aggregation)))
        });
        let configuration = hash_of((plan.tonnage_field().map(|id| id.0), field));

        let classes: Vec<_> = plan.classes().iter().map(|class| (class.id.0, class.default_dig_rate_tph.to_bits())).collect();
        let classes_step = hash_of((configuration, classes));

        let agents: Vec<_> = plan
            .agents()
            .iter()
            .map(|agent| {
                let periods: Vec<_> = agent
                    .calendar
                    .periods
                    .iter()
                    .map(|(period, value)| {
                        (
                            period.0,
                            value.availability.map(f64::to_bits),
                            value.utilisation.map(f64::to_bits),
                            value.rate_tph.map(f64::to_bits),
                        )
                    })
                    .collect();
                (
                    agent.id.0,
                    agent.class_id.0,
                    agent.calendar.default_availability.to_bits(),
                    agent.calendar.default_utilisation.to_bits(),
                    periods,
                )
            })
            .collect();
        let agents_step = hash_of((classes_step, agents));

        // Where the Solids run stands, not what it produced: a status is
        // cheap, and it moves exactly when a schedule's ground does.
        let solids = match self.planning_snapshot_status() {
            Ok(generation) => (0_u8, generation, String::new()),
            Err(reason) => (1, 0, reason.describe()),
        };
        let readiness_step = hash_of((agents_step, solids));

        [configuration, classes_step, agents_step, readiness_step]
    }

    /// Where the Solids pipeline stands, without collecting its dig blocks.
    ///
    /// [`crate::app::App::planning_snapshot`] walks every solid's cache to
    /// build the block list; this asks the same gate the same question and
    /// stops at the answer, which is what makes it safe to call each frame.
    pub(crate) fn planning_snapshot_status(&self) -> Result<u64, crate::app::commands::solids_view::PlanningNotReady> {
        use crate::app::{commands::solids_view::PlanningNotReady, planning_pipeline::StageNotReady};

        let Some(project) = self.workspace.active_project() else {
            return Err(PlanningNotReady::NoProject);
        };
        let Some(pipeline) = self.planning_pipeline.as_ref().filter(|pipeline| pipeline.runtime == project.runtime_id) else {
            return Err(PlanningNotReady::NoProject);
        };
        pipeline.readiness(&self.planning_fingerprints()).map_err(|reason| match reason {
            StageNotReady::NotRun(stage) => PlanningNotReady::NotRun { stage: stage.label() },
            StageNotReady::Stale(stage) => PlanningNotReady::Stale { stage: stage.label() },
            StageNotReady::Running(stage) => PlanningNotReady::Running { stage: stage.label() },
            StageNotReady::Failed { stage, message } => PlanningNotReady::Failed { solid: stage.label(), message },
        })
    }

    /// What the Gantt is allowed to calculate from right now.
    ///
    /// The whole gate in one call: a current Schedule Setup run, or the one
    /// prerequisite that is not met. Inspecting the Gantt never asks this -
    /// only calculating does.
    pub(crate) fn schedule_run_inputs(&self) -> Result<ScheduleRunInputs, ScheduleNotReady> {
        let Some(project) = self.workspace.active_project() else {
            return Err(ScheduleNotReady::NoProject);
        };
        let Some(pipeline) = self.schedule_pipeline.as_ref().filter(|pipeline| pipeline.runtime == project.runtime_id) else {
            return Err(ScheduleNotReady::NoProject);
        };
        // Taken here rather than read from the frame-synchronised copy: a
        // caller can edit configuration and ask before the next frame runs.
        pipeline.readiness(&self.schedule_fingerprints()).map(|result| result.inputs)
    }

    /// The exact setup step that can repair the first prerequisite blocking a
    /// Gantt run. Scheduling Readiness delegates to Solids when that upstream
    /// pipeline is the actual blocker.
    pub(crate) fn schedule_repair_target(&self) -> Option<ScheduleRepairTarget> {
        let reason = self.schedule_run_inputs().err()?;
        let schedule_step = match reason {
            ScheduleNotReady::NoProject => return None,
            ScheduleNotReady::NotRun(step) | ScheduleNotReady::Stale(step) | ScheduleNotReady::Running(step) | ScheduleNotReady::Failed { step, .. } => step,
        };
        if schedule_step == ScheduleStep::Readiness
            && let Some(project) = self.workspace.active_project()
            && let Some(pipeline) = self.planning_pipeline.as_ref().filter(|pipeline| pipeline.runtime == project.runtime_id)
            && let Err(reason) = pipeline.readiness(&self.planning_fingerprints())
        {
            let step = match reason {
                StageNotReady::NotRun(step) | StageNotReady::Stale(step) | StageNotReady::Running(step) | StageNotReady::Failed { stage: step, .. } => step,
            };
            return Some(ScheduleRepairTarget::Solids(step));
        }
        Some(ScheduleRepairTarget::Schedule(schedule_step))
    }

    /// Copy the pipeline's status into the editor state the panels read.
    fn mirror_schedule_stages(&mut self) {
        use crate::ui::state::ScheduleStageView;

        let (views, running) = match self.schedule_pipeline.as_ref() {
            None => (Default::default(), false),
            Some(pipeline) => {
                let views = ScheduleStep::ALL.map(|step| {
                    let status = pipeline.status(step);
                    ScheduleStageView {
                        state: status.state,
                        message: status.message.clone(),
                        diagnostics: status.diagnostics.clone(),
                        last_success: status.last_success.clone(),
                        blocked_by: pipeline.blocked_by(step),
                    }
                });
                (views, pipeline.is_running())
            }
        };
        // What the Gantt would be told if it asked to calculate right now.
        // Held where the run controls are as well as on the Gantt, so an unmet
        // prerequisite is visible from the page that can fix it.
        let calculation = match self.schedule_run_inputs() {
            Ok(_) => tr!("schedule-calculation-ready"),
            Err(reason) => tr!("schedule-calculation-blocked", reason = reason.describe()),
        };
        if self.editor.schedule_stages != views || self.editor.schedule_run_active != running || self.editor.schedule_calculation_status != calculation {
            self.editor.schedule_stages = views;
            self.editor.schedule_run_active = running;
            self.editor.schedule_calculation_status = calculation;
            self.redraw_requested = true;
        }
    }

    /// Reset the Schedule Setup pipeline and run from its first step through
    /// the selected one.
    pub(crate) fn run_schedule_step(&mut self, step: ScheduleStep) {
        self.sync_schedule_pipeline();
        let Some(pipeline) = self.schedule_pipeline.as_mut() else {
            return;
        };
        if !pipeline.restart_through(step) {
            return;
        }
        self.advance_schedule_run();
        self.mirror_schedule_stages();
    }

    pub(crate) fn run_all_schedule_steps(&mut self) {
        self.run_schedule_step(ScheduleStep::Readiness);
    }

    pub(crate) fn cancel_schedule_run(&mut self) {
        if let Some(pipeline) = self.schedule_pipeline.as_mut() {
            pipeline.cancel();
        }
        self.mirror_schedule_stages();
    }

    /// Start the next queued step and settle the running one.
    fn advance_schedule_run(&mut self) {
        // Each step takes at most two passes - one to start it, one to settle
        // it - plus a pass to find the queue empty.
        for _ in 0..ScheduleStep::ALL.len() * 2 + 1 {
            let Some(pipeline) = self.schedule_pipeline.as_mut() else { return };
            match pipeline.next_step() {
                ScheduleRunStep::Idle => return,
                ScheduleRunStep::Skip => continue,
                ScheduleRunStep::Start(step) => pipeline.start(step),
                ScheduleRunStep::Evaluate(step) => {
                    let generation = pipeline.generation();
                    let outcome = self.evaluate_schedule_step(step);
                    let Some(pipeline) = self.schedule_pipeline.as_mut() else { return };
                    match pipeline.settle(step, outcome, generation) {
                        // Said out loud: a run that quietly discards its own
                        // answer looks like a button that did nothing.
                        Err(_) => {
                            crate::userspace_warn!("{}", tr!("schedule-result-superseded"));
                            return;
                        }
                        Ok(true) => return,
                        Ok(false) => {
                            if step == ScheduleStep::Readiness {
                                self.publish_schedule_result(generation);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Capture what the finished run validated, and hold it as its result.
    fn publish_schedule_result(&mut self, generation: u64) {
        let Ok((solids_generation, blocks)) = self.schedule_ground() else { return };
        let Some(runtime) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        let Some(tonnage_field) = self.workspace.active_document().and_then(|document| document.schedule().tonnage_field()) else {
            return;
        };
        let fleet_revision = self.schedule_fingerprints()[ScheduleStep::Readiness.index()];
        let Some(pipeline) = self.schedule_pipeline.as_mut() else { return };
        let published = pipeline.publish(ScheduleRunResult {
            inputs: ScheduleRunInputs {
                runtime,
                generation: solids_generation,
                fleet_revision,
                tonnage_field,
            },
            run: generation,
            blocks,
        });
        if published.is_err() {
            crate::userspace_warn!("{}", tr!("schedule-result-superseded"));
        }
    }

    /// The Solids run this schedule would be calculated over: its number and
    /// how many dig blocks it holds.
    fn schedule_ground(&self) -> Result<(u64, usize), crate::app::commands::solids_view::PlanningNotReady> {
        let snapshot = self.planning_snapshot()?;
        Ok((snapshot.generation, snapshot.blocks.len()))
    }

    /// What one step has to say about the project as it stands.
    ///
    /// Reads only: no step here edits the plan, starts a job or moves the
    /// Solids pipeline's markers.
    fn evaluate_schedule_step(&mut self, step: ScheduleStep) -> StageOutcome {
        match step {
            ScheduleStep::Configuration => self.evaluate_schedule_configuration(),
            ScheduleStep::LoaderClasses => self.evaluate_loader_classes(),
            ScheduleStep::LoaderAgents => self.evaluate_loader_agents(),
            ScheduleStep::Readiness => self.evaluate_schedule_readiness(),
        }
    }

    fn evaluate_schedule_configuration(&self) -> StageOutcome {
        let mut diagnostics = Vec::new();
        let field = self.workspace.active_document().and_then(|document| {
            let chosen = document.schedule().tonnage_field()?;
            Some((chosen, document.reserve_fields().iter().find(|field| field.id == chosen).cloned()))
        });
        match field {
            // Never guessed for the user: an unchosen field is the one thing
            // the whole schedule's tonnes rest on.
            None => diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("schedule-stage-no-tonnage-field"),
                blocking: true,
            }),
            Some((_, None)) => diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("sequence-tonnage-field-missing"),
                blocking: true,
            }),
            Some((_, Some(field))) if field.aggregation != ReserveAggregation::Sum => diagnostics.push(StageDiagnostic {
                entity: Some(field.name.clone()),
                message: tr!("schedule-stage-tonnage-field-not-summed"),
                blocking: true,
            }),
            Some((_, Some(_))) => {}
        }
        StageOutcome::Settled { diagnostics, entities: 1 }
    }

    fn evaluate_loader_classes(&self) -> StageOutcome {
        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let classes = document.schedule().classes();
        let mut diagnostics = Vec::new();
        if classes.is_empty() {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("schedule-stage-no-classes"),
                blocking: true,
            });
        }
        for class in classes {
            if !class.default_dig_rate_tph.is_finite() || class.default_dig_rate_tph <= 0.0 {
                diagnostics.push(StageDiagnostic {
                    entity: Some(class.name.clone()),
                    message: tr!("schedule-error-invalid-rate"),
                    blocking: true,
                });
            }
        }
        StageOutcome::Settled {
            diagnostics,
            entities: classes.len(),
        }
    }

    fn evaluate_loader_agents(&self) -> StageOutcome {
        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let plan = document.schedule();
        let mut diagnostics = Vec::new();
        if plan.agents().is_empty() {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("schedule-stage-no-agents"),
                blocking: true,
            });
        }
        for agent in plan.agents() {
            if let Err(error) = agent.calendar.validate() {
                diagnostics.push(StageDiagnostic {
                    entity: Some(agent.name.clone()),
                    message: error.message(),
                    blocking: true,
                });
                continue;
            }
            match plan.class(agent.class_id) {
                Some(class) => {
                    if let Err(error) = agent.calendar.compile(class.default_dig_rate_tph) {
                        diagnostics.push(StageDiagnostic {
                            entity: Some(agent.name.clone()),
                            message: error.message(),
                            blocking: true,
                        });
                    }
                }
                None => diagnostics.push(StageDiagnostic {
                    entity: Some(agent.name.clone()),
                    message: tr!("schedule-stage-agent-no-class"),
                    blocking: true,
                }),
            }
        }
        StageOutcome::Settled {
            diagnostics,
            entities: plan.agents().len(),
        }
    }

    /// The gate over the Solids run: has it been run to completion, and does
    /// it carry the figures this schedule would be calculated from.
    fn evaluate_schedule_readiness(&mut self) -> StageOutcome {
        use crate::app::commands::solids_view::PlanningNotReady;

        let snapshot = match self.planning_snapshot() {
            Ok(snapshot) => snapshot,
            // A Solids run in flight is not a failure: this step waits for it,
            // the way the block-model step waits for its scans.
            Err(reason @ PlanningNotReady::Running { .. }) => {
                return StageOutcome::Working {
                    message: Some(tr!("schedule-stage-waiting-solids", reason = reason.describe())),
                };
            }
            Err(reason) => {
                return StageOutcome::Settled {
                    diagnostics: vec![StageDiagnostic {
                        entity: None,
                        message: reason.describe(),
                        blocking: true,
                    }],
                    entities: 0,
                };
            }
        };
        let Some(field) = self.workspace.active_document().and_then(|document| document.schedule().tonnage_field()) else {
            // Configuration blocks before this step is reached; a run that got
            // here without one has had the choice removed underneath it.
            return StageOutcome::Settled {
                diagnostics: vec![StageDiagnostic {
                    entity: None,
                    message: tr!("schedule-stage-no-tonnage-field"),
                    blocking: true,
                }],
                entities: 0,
            };
        };

        let mut diagnostics = Vec::new();
        let mut unmeasured = 0;
        for block in &snapshot.blocks {
            match super::commands::schedule_readiness::block_tonnes_for_stage(block, field) {
                Ok(_) => {}
                // A figure that is there but cannot be tonnes is a broken
                // project, and blocks. A figure that is absent stops only the
                // bars that reach that ground, which each say so themselves -
                // blocking the whole schedule over ground nothing is
                // scheduled on would be a gate nobody could pass.
                Err(reason) if reason.invalid => diagnostics.push(StageDiagnostic {
                    entity: Some(block.name.clone()),
                    message: reason.message,
                    blocking: true,
                }),
                Err(_) => unmeasured += 1,
            }
        }
        if unmeasured > 0 {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("schedule-stage-blocks-unmeasured", count = unmeasured.to_string()),
                blocking: false,
            });
        }
        StageOutcome::Settled {
            diagnostics,
            entities: snapshot.blocks.len(),
        }
    }
}
