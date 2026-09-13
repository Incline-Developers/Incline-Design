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
//! The run is advanced in bounded steps from the frame loop rather than
//! computed in one call, which is what gives Cancel something to cancel and
//! stops a degenerate input from freezing the window.

use std::hash::{Hash, Hasher};

use crate::{
    app::{
        commands::schedule_readiness::{ScheduleRunProblem, dispatch_input, dispatch_problem},
        schedule_pipeline::ScheduleRunInputs,
    },
    i18n::tr,
    model::schedule::{DispatchSchedule, dispatch::DispatchRun},
};

/// How much of the schedule one Run Period covers.
///
/// A day, for now. What a period represents is expected to change - a week for
/// a long-term schedule - which is why it is one constant read in one place
/// rather than a `24.0` written wherever hours are counted.
pub(crate) const PERIOD_H: f64 = 24.0;

/// How many events one frame of a run may simulate.
///
/// Large enough that any schedule a person authors finishes in the frame they
/// pressed the button in, small enough that a degenerate one still gives the
/// window back.
const EVENT_BUDGET: usize = 4096;

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
    pub(crate) schedule: DispatchSchedule,
}

/// A Run Schedule in flight.
pub(crate) struct PendingScheduleRun {
    run: DispatchRun,
    inputs: ScheduleRunInputs,
    plan_revision: u64,
    serial: u64,
    horizon_limit_h: Option<f64>,
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
    /// A bar's name is in it because the drawn result names bars; its members
    /// are in it because they are the ground. Plan-wide fleet and
    /// configuration are deliberately absent - they are already captured by
    /// the Setup run's own inputs, and hashing them twice would only make the
    /// two disagree.
    pub(crate) fn schedule_plan_revision(&self) -> u64 {
        let Some(document) = self.workspace.active_document() else {
            return 0;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for bar in document.schedule().bars() {
            bar.id.0.hash(&mut hasher);
            bar.order.name.hash(&mut hasher);
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
        hasher.finish()
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
            Err(problems) => return self.refuse_schedule_run(problems),
        };
        let run = match DispatchRun::start(input) {
            Ok(run) => run,
            Err(errors) => {
                let Some(document) = self.workspace.active_document() else {
                    return;
                };
                let plan = document.schedule().clone();
                return self.refuse_schedule_run(errors.into_iter().map(|error| dispatch_problem(&plan, error)).collect());
            }
        };
        self.schedule_run_serial += 1;
        let serial = self.schedule_run_serial;
        self.schedule_run_problems.clear();
        self.pending_schedule_run = Some(PendingScheduleRun {
            run,
            inputs,
            plan_revision,
            serial,
            horizon_limit_h,
        });
        self.redraw_requested = true;
        // A schedule a person authored finishes here, in the frame the button
        // was pressed in. The frame loop is the fallback, not the path.
        self.advance_schedule_calculation();
    }

    /// Publish nothing, keep whatever was held, and say why.
    fn refuse_schedule_run(&mut self, problems: Vec<ScheduleRunProblem>) {
        for problem in &problems {
            crate::userspace_warn!("{}", problem.message);
        }
        self.schedule_run_problems = problems;
        self.pending_schedule_run = None;
        self.redraw_requested = true;
    }

    /// Stop a run in flight. The held result is untouched.
    pub(crate) fn cancel_schedule_run_calculation(&mut self) {
        if self.pending_schedule_run.take().is_some() {
            crate::userspace_warn!("{}", tr!("schedule-run-cancelled"));
            self.redraw_requested = true;
        }
    }

    /// Advance a run in flight by one frame's worth of events.
    ///
    /// The inputs are rechecked first: a run whose ground, fleet or bars moved
    /// while it was working is describing a project that no longer exists, and
    /// is dropped rather than finished.
    pub(crate) fn advance_schedule_calculation(&mut self) {
        let Some(pending) = self.pending_schedule_run.as_ref() else {
            return;
        };
        let expected = (pending.inputs, pending.plan_revision);
        if self.schedule_run_inputs().ok() != Some(expected.0) || self.schedule_plan_revision() != expected.1 {
            self.pending_schedule_run = None;
            crate::userspace_warn!("{}", tr!("schedule-run-superseded"));
            self.redraw_requested = true;
            return;
        }
        let pending = self.pending_schedule_run.as_mut().expect("checked above");
        let Some(outcome) = pending.run.advance(EVENT_BUDGET) else {
            // More to do: come back next frame rather than holding the UI.
            self.redraw_requested = true;
            return;
        };
        let pending = self.pending_schedule_run.take().expect("checked above");
        match outcome {
            Ok(schedule) => {
                crate::userspace_log!(
                    "{}",
                    tr!("schedule-run-finished", run = pending.serial.to_string(), horizon = format!("{:.2}", schedule.horizon_h))
                );
                self.schedule_run_problems.clear();
                self.schedule_calculation = Some(ScheduleCalculation {
                    inputs: pending.inputs,
                    plan_revision: pending.plan_revision,
                    run: pending.serial,
                    horizon_limit_h: pending.horizon_limit_h,
                    schedule,
                });
            }
            Err(errors) => {
                let plan = self.workspace.active_document().map(|document| document.schedule().clone());
                let problems = match plan {
                    Some(plan) => errors.into_iter().map(|error| dispatch_problem(&plan, error)).collect(),
                    None => Vec::new(),
                };
                self.refuse_schedule_run(problems);
            }
        }
        self.redraw_requested = true;
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
        let dispatch = if current {
            self.schedule_calculation.as_ref().map(|calculation| calculation.schedule.clone())
        } else {
            None
        };
        let status = match (running, self.schedule_calculation.as_ref()) {
            (true, _) => tr!("schedule-run-working"),
            (false, None) => match self.schedule_run_inputs() {
                Ok(_) => tr!("schedule-run-never"),
                Err(reason) => tr!("schedule-run-blocked", reason = reason.describe()),
            },
            (false, Some(calculation)) if !current => tr!("schedule-run-stale", run = calculation.run.to_string()),
            (false, Some(calculation)) => {
                let run = calculation.run.to_string();
                if calculation.schedule.truncated {
                    tr!("schedule-run-truncated", run = run, horizon = format!("{:.2}", calculation.schedule.horizon_h))
                } else {
                    tr!("schedule-run-complete", run = run, horizon = format!("{:.2}", calculation.schedule.horizon_h))
                }
            }
        };
        let stale = !current && self.schedule_calculation.is_some();
        if self.editor.schedule_dispatch != dispatch
            || self.editor.schedule_run_status != status
            || self.editor.schedule_run_stale != stale
            || self.editor.schedule_run_working != running
        {
            self.editor.schedule_dispatch = dispatch;
            self.editor.schedule_run_status = status;
            self.editor.schedule_run_stale = stale;
            self.editor.schedule_run_working = running;
            self.redraw_requested = true;
        }
    }
}
