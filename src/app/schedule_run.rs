//! The Gantt's explicit Run Schedule: what it captures, what it publishes, and
//! when what it published stops being true.
//!
//! Nothing here recalculates on its own. Dragging a bar, resizing a window,
//! rerunning Solids or editing the fleet all *mark* the held result stale;
//! only Run Period and Run Schedule produce a new one. That separation is the
//! reason the page can be trusted: a figure on screen either belongs to a run
//! the user asked for, or is labelled as belonging to an earlier one.
//!
//! Three rules, the same three the Schedule Setup pipeline keeps:
//!
//! - **A run calculates from inputs captured when it started**, and they
//!   travel with the result.
//! - **An obsolete result is rejected, not published.** A run whose inputs
//!   moved while it was working is dropped with a stated reason.
//! - **Cancel publishes nothing.** The previously held result stays exactly as
//!   it was, and a cancelled run leaves no partial schedule behind.
//!
//! The evaluator runs on the bounded worker pool and polls cancellation
//! between event chunks. Preparation and publication stay on the UI thread;
//! publication repeats the input checks before accepting the result.

use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::{
    app::{
        commands::schedule_readiness::{ScheduleRunProblem, dispatch_input, dispatch_problem},
        schedule_pipeline::ScheduleRunInputs,
    },
    i18n::tr,
    model::schedule::{DispatchOutcome, DispatchSchedule, dispatch::DispatchRun},
};

/// How much of the schedule one Run Period covers.
///
/// A day, for now. What a period represents is expected to change - a week for
/// a long-term schedule - which is why it is one constant read in one place
/// rather than a `24.0` written wherever hours are counted.
pub(crate) const PERIOD_H: f64 = 24.0;

/// Events between cancellation polls on the background worker.
const EVENT_BUDGET: usize = 64;

/// What a run was asked to cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScheduleRunMode {
    /// One more period than the held result reached, from the origin.
    Period,
    /// To completion: until no loader has available work and no window opens.
    Whole,
}

/// One finished Run Schedule.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScheduleCalculation {
    /// What Schedule Setup had validated when this run started.
    pub(crate) inputs: ScheduleRunInputs,
    /// The authored bars it was calculated over, hashed. This is the half the
    /// Setup run deliberately excludes: moving a bar does not retire a Setup
    /// run, and does retire this one.
    pub(crate) plan_revision: u64,
    /// Which run it was, so a held result can be named.
    pub(crate) run: u64,
    pub(crate) horizon_limit_h: Option<f64>,
    pub(crate) schedule: Arc<DispatchSchedule>,
}

/// A Run Schedule in flight.
pub(crate) struct PendingScheduleRun {
    pub(crate) inputs: ScheduleRunInputs,
    plan_revision: u64,
    pub(crate) serial: u64,
    horizon_limit_h: Option<f64>,
}

/// Diagnostics from one refused run attempt, owned by the exact inputs that
/// produced them. They remain useful while those inputs stand and disappear
/// as soon as the project or plan changes.
pub(crate) struct ScheduleRunDiagnostics {
    pub(crate) inputs: ScheduleRunInputs,
    pub(crate) plan_revision: u64,
    pub(crate) problems: Vec<ScheduleRunProblem>,
}

impl std::fmt::Debug for PendingScheduleRun {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PendingScheduleRun")
            .field("serial", &self.serial)
            .field("horizon_limit_h", &self.horizon_limit_h)
            .finish_non_exhaustive()
    }
}

impl crate::app::App<'_> {
    /// The authored bars, hashed: every field a calculated schedule depends
    /// on, and nothing else.
    ///
    /// Presentation names are deliberately absent: the drawn result resolves
    /// current labels by stable ids. Members are present because they are the
    /// ground. Plan-wide fleet and
    /// configuration are deliberately absent - they are already captured by
    /// the Setup run's own inputs, and hashing them twice would only make the
    /// two disagree.
    pub(crate) fn schedule_plan_revision(&self) -> u64 {
        let Some(project) = self.workspace.active_project() else {
            return 0;
        };
        let document = &project.project.document;
        let document_revision = document.revision();
        if let Some((runtime, revision, key)) = self.schedule_plan_revision_cache.get()
            && runtime == project.runtime_id
            && revision == document_revision
        {
            return key;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for bar in document.schedule().bars() {
            bar.id.0.hash(&mut hasher);
            bar.agent.map(|agent| agent.0).hash(&mut hasher);
            bar.priority.hash(&mut hasher);
            bar.window.start_h.to_bits().hash(&mut hasher);
            bar.window.end_h.map(f64::to_bits).hash(&mut hasher);
            for member in bar.members() {
                member.solid.hash(&mut hasher);
                member.source.hash(&mut hasher);
                member.flitch_base.to_bits().hash(&mut hasher);
                member.flitch_top.to_bits().hash(&mut hasher);
                member.anchor.map(f64::to_bits).hash(&mut hasher);
                member.plan_area.to_bits().hash(&mut hasher);
                member.footprint.hash(&mut hasher);
                member.volume.map(f64::to_bits).hash(&mut hasher);
            }
        }
        let key = hasher.finish();
        self.schedule_plan_revision_cache.set(Some((project.runtime_id, document_revision, key)));
        key
    }

    /// Whether the held result still describes the project as it stands.
    ///
    /// Everything a run read is in one of two values, so this is the whole
    /// question: the Setup run's captured inputs cover the fleet, the tonnage
    /// field and the Solids run, and the plan revision covers the bars.
    pub(crate) fn schedule_calculation_is_current(&self) -> bool {
        let Some(calculation) = self.schedule_calculation.as_ref() else {
            return false;
        };
        self.schedule_run_inputs().is_ok_and(|inputs| inputs == calculation.inputs) && self.schedule_plan_revision() == calculation.plan_revision
    }

    /// Where a Run Period would stop: one period past whatever the held
    /// result reached, or the first period when there is nothing to carry on
    /// from.
    fn next_period_horizon(&self) -> f64 {
        match self.schedule_calculation.as_ref().filter(|_| self.schedule_calculation_is_current()) {
            Some(calculation) => calculation.horizon_limit_h.map_or(PERIOD_H, |reached| reached + PERIOD_H),
            None => PERIOD_H,
        }
    }

    /// Start a run, or say why one cannot start.
    ///
    /// Refusing is the common case while a project is being built up, so it
    /// is refused in the console with the reason rather than by a dead
    /// button: the button is what the user pressed, and it has to answer.
    pub(crate) fn start_schedule_run(&mut self, mode: ScheduleRunMode) {
        let preparation_started = web_time::Instant::now();
        // Validate the lightweight Schedule Setup stages as part of the
        // normal Run action. Solids remains an explicit prerequisite.
        self.run_all_schedule_steps();
        let inputs = match self.schedule_run_inputs() {
            Ok(inputs) => inputs,
            Err(reason) => {
                crate::userspace_warn!("{}", tr!("schedule-run-blocked", reason = reason.describe()));
                return;
            }
        };
        let horizon_limit_h = match mode {
            ScheduleRunMode::Period => Some(self.next_period_horizon()),
            ScheduleRunMode::Whole => None,
        };
        let plan_revision = self.schedule_plan_revision();
        let reports = self.schedule_reports();
        let built = {
            let Some(document) = self.workspace.active_document() else {
                return;
            };
            dispatch_input(document.schedule(), &reports, inputs.generation, horizon_limit_h)
        };
        let input = match built {
            Ok(input) => input,
            Err(problems) => return self.refuse_schedule_run(problems, inputs, plan_revision),
        };
        self.schedule_run_serial += 1;
        let serial = self.schedule_run_serial;
        self.schedule_run_diagnostics = None;
        self.pending_schedule_run = Some(PendingScheduleRun {
            inputs,
            plan_revision,
            serial,
            horizon_limit_h,
        });
        log::debug!("schedule preparation completed in {:?}", preparation_started.elapsed());
        self.spawn_job_quietly(
            tr!(literal = "Calculating schedule"),
            vec![crate::app::jobs::JobKey::ScheduleRun { runtime: inputs.runtime, serial }],
            move |cancel| {
                let seed_started = web_time::Instant::now();
                let mut run = match DispatchRun::start(input) {
                    Ok(run) => run,
                    Err(errors) => return Ok(Err(errors)),
                };
                let seeded = seed_started.elapsed();
                let advance_started = web_time::Instant::now();
                loop {
                    anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
                    match run.advance_events(EVENT_BUDGET) {
                        Ok(true) => break,
                        Ok(false) => {}
                        Err(errors) => return Ok(Err(errors)),
                    }
                }
                let advanced = advance_started.elapsed();
                let finish_started = web_time::Instant::now();
                let schedule = run.complete();
                let finished = finish_started.elapsed();
                log::debug!("schedule worker: seed {seeded:?}, advance {advanced:?}, finish {finished:?}");
                Ok(Ok(schedule))
            },
            move |app, result| {
                let same_request = app
                    .pending_schedule_run
                    .as_ref()
                    .is_some_and(|pending| pending.serial == serial && pending.inputs == inputs);
                if !same_request {
                    return;
                }
                if app.schedule_run_inputs().ok() != Some(inputs) || app.schedule_plan_revision() != plan_revision {
                    app.pending_schedule_run = None;
                    crate::userspace_warn!("{}", tr!("schedule-run-superseded"));
                    app.redraw_requested = true;
                    return;
                }
                app.pending_schedule_run = None;
                match result {
                    Ok(Ok(schedule)) => {
                        crate::userspace_log!("{}", tr!("schedule-run-finished", run = serial.to_string(), horizon = format!("{:.2}", schedule.horizon_h)));
                        app.schedule_run_diagnostics = None;
                        app.schedule_calculation = Some(ScheduleCalculation {
                            inputs,
                            plan_revision,
                            run: serial,
                            horizon_limit_h,
                            schedule: Arc::new(schedule),
                        });
                    }
                    Ok(Err(errors)) => {
                        let plan = app.workspace.active_document().map(|document| document.schedule().clone());
                        let problems = match plan {
                            Some(plan) => errors.into_iter().map(|error| dispatch_problem(&plan, error)).collect(),
                            None => Vec::new(),
                        };
                        app.refuse_schedule_run(problems, inputs, plan_revision);
                    }
                    Err(error) => crate::userspace_error!("{error:#}"),
                }
                app.redraw_requested = true;
            },
        );
        self.redraw_requested = true;
    }

    /// Publish nothing, keep whatever was held, and say why.
    fn refuse_schedule_run(&mut self, problems: Vec<ScheduleRunProblem>, inputs: ScheduleRunInputs, plan_revision: u64) {
        for problem in &problems {
            crate::userspace_warn!("{}", problem.message);
        }
        self.schedule_run_diagnostics = Some(ScheduleRunDiagnostics { inputs, plan_revision, problems });
        self.pending_schedule_run = None;
        self.redraw_requested = true;
    }

    /// Stop a run in flight. The held result is untouched.
    pub(crate) fn cancel_schedule_run_calculation(&mut self) {
        if let Some(pending) = self.pending_schedule_run.take() {
            self.cancel_jobs(|key| matches!(key, crate::app::jobs::JobKey::ScheduleRun { serial, .. } if *serial == pending.serial));
            crate::userspace_warn!("{}", tr!("schedule-run-cancelled"));
            self.redraw_requested = true;
        }
    }

    /// Retire a background run as soon as its captured inputs move.
    pub(crate) fn advance_schedule_calculation(&mut self) {
        let Some(pending) = self.pending_schedule_run.as_ref() else {
            return;
        };
        let expected = (pending.inputs, pending.plan_revision);
        if self.schedule_run_inputs().ok() != Some(expected.0) || self.schedule_plan_revision() != expected.1 {
            let serial = pending.serial;
            self.pending_schedule_run = None;
            self.cancel_jobs(|key| matches!(key, crate::app::jobs::JobKey::ScheduleRun { serial: pending, .. } if *pending == serial));
            crate::userspace_warn!("{}", tr!("schedule-run-superseded"));
            self.redraw_requested = true;
        }
    }

    /// Copy the held result into the editor state the Gantt draws from - but
    /// only while it is current.
    ///
    /// A stale result is kept and named, and its calculated bands are taken
    /// off the page. Drawing them over bars that have since moved would put a
    /// figure next to ground it was not calculated from, which is worse than
    /// showing nothing: the page says what is stale and what to press.
    pub(crate) fn mirror_schedule_calculation(&mut self) {
        let current = self.schedule_calculation_is_current();
        let running = self.pending_schedule_run.is_some();
        let dispatch = current.then(|| self.schedule_calculation.as_ref().expect("current calculation exists").schedule.clone());
        let status = match (running, self.schedule_calculation.as_ref()) {
            (true, _) => tr!("schedule-run-working"),
            (false, None) => match self.schedule_run_inputs() {
                Ok(_) => tr!("schedule-run-never"),
                Err(reason) => tr!("schedule-run-blocked", reason = reason.describe()),
            },
            (false, Some(calculation)) if !current => tr!("schedule-run-stale", run = calculation.run.to_string()),
            (false, Some(calculation)) => {
                let run = calculation.run.to_string();
                match calculation.schedule.outcome {
                    DispatchOutcome::Exhausted => tr!("schedule-run-complete", run = run, horizon = format!("{:.2}", calculation.schedule.horizon_h)),
                    DispatchOutcome::Limited => tr!("schedule-run-truncated", run = run, horizon = format!("{:.2}", calculation.schedule.horizon_h)),
                    DispatchOutcome::Stranded => tr!("schedule-run-stranded", run = run, horizon = format!("{:.2}", calculation.schedule.horizon_h)),
                }
            }
        };
        let stale = !current && self.schedule_calculation.is_some();
        let repair = (!running).then(|| self.schedule_repair_target()).flatten();
        let same_dispatch = match (&self.editor.schedule_dispatch, &dispatch) {
            (Some(held), Some(current)) => Arc::ptr_eq(held, current),
            (None, None) => true,
            _ => false,
        };
        if !same_dispatch
            || self.editor.schedule_run_status != status
            || self.editor.schedule_run_stale != stale
            || self.editor.schedule_run_working != running
            || self.editor.schedule_run_repair != repair
        {
            self.editor.schedule_dispatch = dispatch;
            self.editor.schedule_run_status = status;
            self.editor.schedule_run_stale = stale;
            self.editor.schedule_run_working = running;
            self.editor.schedule_run_repair = repair;
            self.redraw_requested = true;
        }
    }
}
