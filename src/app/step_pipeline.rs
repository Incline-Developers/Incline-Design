//! The run and invalidation state of a Setup pipeline whose steps are cheap
//! checks run in order: the Schedule pipeline in
//! [`crate::app::schedule_pipeline`] and the Haulage one in
//! [`crate::app::haulage_pipeline`].
//!
//! Steps run in a strict order, and each step's input fingerprint includes the
//! fingerprint of the step before it, so one edit marks exactly the suffix of
//! steps it can reach. A step evaluated while a run is under way may answer
//! [`StageOutcome::Working`]; it is evaluated again on the next pass, which is
//! how a step waits on work that runs elsewhere.

use crate::{
    app::planning_pipeline::{StageOutcome, StageState, StageStatus, StageSummary},
    i18n::tr,
};

/// One step of a [`StepPipeline`]: its place in the order and its name.
pub(crate) trait PipelineStep: Copy + Eq + std::fmt::Debug + 'static {
    /// Every step, in run order.
    const ALL: &'static [Self];

    fn index(self) -> usize;

    fn label(self) -> String;
}

/// One turn of the runner, kept free of jobs and of the app so the scheduling
/// half can be driven straight through in a test.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RunTurn<S> {
    Idle,
    Start(S),
    /// Already current from an earlier run; take the next one.
    Skip,
    Evaluate(S),
}

/// Why a settled outcome was thrown away instead of published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Obsolete {
    /// It belongs to a run that has since been superseded or cancelled.
    Superseded,
    /// It belongs to a step that is no longer the running one.
    NotRunning,
}

/// One pipeline's step statuses and the run in flight, for one open project.
#[derive(Debug)]
pub(crate) struct StepPipeline<S: PipelineStep> {
    /// Which project this belongs to; a different one starts over.
    pub(crate) runtime: u32,
    steps: Vec<StageStatus>,
    fingerprints: Vec<u64>,
    /// Increments on every Run to Step, Run All and Auto run.
    generation: u64,
    queue: Vec<S>,
    running: Option<S>,
}

impl<S: PipelineStep> StepPipeline<S> {
    pub(crate) fn new(runtime: u32) -> Self {
        Self {
            runtime,
            steps: vec![StageStatus::default(); S::ALL.len()],
            fingerprints: vec![0; S::ALL.len()],
            generation: 0,
            queue: Vec::new(),
            running: None,
        }
    }

    pub(crate) fn status(&self, step: S) -> &StageStatus {
        &self.steps[step.index()]
    }

    fn status_mut(&mut self, step: S) -> &mut StageStatus {
        &mut self.steps[step.index()]
    }

    pub(crate) fn running(&self) -> Option<S> {
        self.running
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.is_some() || !self.queue.is_empty()
    }

    pub(crate) fn step_inputs_match(&self, step: S) -> bool {
        self.steps[step.index()].completed_inputs == Some(self.fingerprints[step.index()])
    }

    /// The step that has to run before `step` can, if any.
    pub(crate) fn blocked_by(&self, step: S) -> Option<S> {
        S::ALL.iter().copied().take(step.index()).find(|earlier| !self.steps[earlier.index()].state.is_current())
    }

    /// Whether every step is complete for exactly the inputs it has now.
    pub(crate) fn is_current(&self) -> bool {
        S::ALL.iter().all(|&step| self.status(step).state.is_current() && self.step_inputs_match(step))
    }

    /// Take the inputs as they stand now, and return the earliest step whose
    /// fingerprint moved.
    pub(crate) fn take_fingerprints(&mut self, fingerprints: &[u64]) -> Option<S> {
        let earliest = S::ALL.iter().copied().find(|step| self.fingerprints[step.index()] != fingerprints[step.index()]);
        self.fingerprints.copy_from_slice(fingerprints);
        earliest
    }

    /// Mark complete steps whose inputs no longer match what they ran on.
    pub(crate) fn mark_stale(&mut self) {
        for &step in S::ALL {
            let current = self.fingerprints[step.index()];
            let status = self.status_mut(step);
            if status.state == StageState::Complete && status.completed_inputs != Some(current) {
                status.state = StageState::Stale;
            }
        }
    }

    /// Begin a fresh run through the selected step, retiring all previous
    /// statuses.
    pub(crate) fn restart_through(&mut self, step: S) -> bool {
        if self.is_running() {
            return false;
        }
        self.steps = vec![StageStatus::default(); S::ALL.len()];
        self.generation += 1;
        self.queue = S::ALL.iter().copied().take(step.index() + 1).collect();
        for queued in self.queue.clone() {
            self.status_mut(queued).state = StageState::Queued;
        }
        true
    }

    /// Queue the steps from the first one that is not current, keeping the
    /// current ones before it. What Auto runs.
    ///
    /// A step marked stale whose inputs have come back to what it ran on - an
    /// undo, say - is current again, as long as everything before it is.
    pub(crate) fn resume_all(&mut self) -> bool {
        if self.is_running() {
            return false;
        }
        let mut first = None;
        for &step in S::ALL {
            let matches = self.step_inputs_match(step);
            let status = self.status_mut(step);
            if status.state == StageState::Stale && matches {
                status.state = StageState::Complete;
            }
            if !(status.state.is_current() && matches) {
                first = Some(step);
                break;
            }
        }
        let Some(first) = first else { return false };
        self.generation += 1;
        self.queue = S::ALL.iter().copied().skip(first.index()).collect();
        for queued in self.queue.clone() {
            self.status_mut(queued).state = StageState::Queued;
        }
        true
    }

    pub(crate) fn next_step(&mut self) -> RunTurn<S> {
        if let Some(step) = self.running {
            return RunTurn::Evaluate(step);
        }
        let Some(next) = (!self.queue.is_empty()).then(|| self.queue.remove(0)) else {
            return RunTurn::Idle;
        };
        if self.status(next).state.is_current() && self.step_inputs_match(next) {
            return RunTurn::Skip;
        }
        RunTurn::Start(next)
    }

    pub(crate) fn start(&mut self, step: S) {
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
    pub(crate) fn settle(&mut self, step: S, outcome: StageOutcome, generation: u64) -> Result<bool, Obsolete> {
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
                    status.message = Some(tr!("stage-failed-count", count = blocking));
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

    /// Mark this step and every step after it as no longer current.
    ///
    /// Returns whether this stopped a run that was in flight.
    pub(crate) fn invalidate_from(&mut self, step: S) -> bool {
        // A queued step has not read anything yet: it will see these inputs
        // when its turn comes. Only a published result, or one being computed
        // right now, can end up describing inputs it never saw.
        let stopped = self.running.is_some_and(|running| running.index() >= step.index());
        for &later in S::ALL.iter().skip(step.index()) {
            let status = self.status_mut(later);
            match status.state {
                // A failure is a finding about inputs that have now changed:
                // it may no longer hold, so it is out of date like a success
                // would be, and the next run says whether it still does.
                StageState::Complete | StageState::Failed | StageState::Blocked => status.state = StageState::Stale,
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

    /// Stop whatever is queued or running.
    pub(crate) fn cancel(&mut self) {
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
