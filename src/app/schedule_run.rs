//! Run Period, Run All Periods, Improve and the automatic recalculation:
//! what a schedule run captures, what it publishes, and when what it
//! published stops being true.
//!
//! Dragging a bar, resizing a window, rerunning Solids or editing the fleet
//! all *mark* the held result stale; a figure on screen either belongs to the
//! inputs as they stand, or is labelled as belonging to an earlier run.
//!
//! # Recalculation
//!
//! The hourly schedule takes a fraction of a second, so with Auto on (the
//! default) an edit is followed, once edits have settled for
//! [`AUTO_SETTLE`], by a run of its own ([`ScheduleRunMode::Auto`]). It
//! covers the horizon the held result did, says nothing in the console, and
//! is tried once per set of inputs: one that is refused, fails or is stopped
//! by the user is not retried until something changes. Improve - the
//! whole-horizon optimiser - only ever runs when asked for.
//!
//! # One path
//!
//! Every run takes the same route: an owned capture snapshot on the UI
//! thread, then capture, the SCIP solve, extraction, the independent replay
//! and publication on one worker of the shared pool. Only a schedule the
//! replay accepted is ever published, and there is no other scheduling
//! algorithm behind this one - a run that fails publishes nothing and says
//! why.
//!
//! # Horizons
//!
//! Every run starts at project hour zero from the authored opening stock; none
//! continues from an earlier run's closing state. Run All Periods asks for
//! the whole horizon up to the configured planning end day. Run Period asks
//! for one day more than the held, current result covers - or for day one
//! when there is none - and reoptimises the whole of that longer horizon, so
//! days it calculated before may change. It never asks past the planning end.
//!
//! # Day-by-day schedules shown early
//!
//! A horizon long enough to be solved day by day first (see
//! [`crate::model::schedule::optimisation::blended::rolling`]) has a valid,
//! replayed schedule well before its whole-horizon solve ends. The worker
//! publishes it then, and it is shown - under the same currentness checks as
//! any other answer - while the whole-horizon solve looks for a better one.
//! Stopping the run at that point keeps it; the run's final answer replaces
//! it, and is never worth less, because the whole-horizon solve starts from
//! it and the better of the two is the one published.
//!
//! # Currentness
//!
//! A result is current while [`crate::app::App::schedule_semantic_key`] is
//! unchanged and its horizon still lies inside the planning end. The key is
//! built from every authored input the model encodes, by stable id and with
//! no presentation name, so a rename leaves a result current while a changed
//! coefficient retires it. A run in flight whose key moves is cancelled and
//! its answer, if it arrives anyway, is refused.

use std::{
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use crate::{
    app::schedule_pipeline::{ScheduleNotReady, ScheduleRunInputs},
    i18n::tr,
    model::schedule::{
        SCHEDULE_PERIOD_H,
        result::{BoundSource, CalculatedSchedule, DayByDayRole, SolveQuality, StartMethod},
    },
    ui::state::{ScheduleRepairTarget, ScheduleStep},
};

/// What a run was asked to cover, and how hard it looks.
///
/// Every mode but [`Self::Improve`] stops at the hourly dispatch schedule
/// (see [`crate::model::schedule::optimisation::blended::greedy`]), which
/// takes a fraction of a second. Improve starts from that schedule and spends
/// the configured solve time looking for a better one over the whole horizon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScheduleRunMode {
    /// One day more than the held current result, from hour zero.
    Period,
    /// From hour zero to the planning end day.
    All,
    /// The held result's horizon, or the planning end when there is none,
    /// solved with the whole-horizon optimiser.
    Improve,
    /// A recalculation after an edit, nobody pressed anything: the horizon
    /// the held result covered, even a stale one, so a schedule run to day 3
    /// stays a schedule to day 3. Quiet in the console.
    Auto,
}

/// How long the schedule has to stay unedited before it is recalculated on
/// its own, so a burst of edits costs one run rather than one each.
const AUTO_SETTLE: std::time::Duration = std::time::Duration::from_millis(350);

/// A run in flight.
#[derive(Debug)]
pub(crate) struct PendingScheduleRun {
    pub(crate) serial: u64,
    pub(crate) runtime: u32,
    inputs: ScheduleRunInputs,
    semantic: u64,
    pub(crate) requested_end_h: f64,
    /// Started by [`ScheduleRunMode::Auto`]: said in the log, not the console.
    auto: bool,
    /// Started by [`ScheduleRunMode::Improve`].
    pub(crate) improve: bool,
    /// Where the worker leaves the day-by-day schedule for the UI thread.
    early: Arc<Mutex<Option<Arc<CalculatedSchedule>>>>,
    /// Whether the held result is this run's day-by-day schedule.
    showing_early: bool,
}

/// Why the last attempt published nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code, reason = "produced by the native schedule run; the browser build does not calculate"))]
pub(crate) enum AttemptOutcome {
    /// The project could not be resolved into a model.
    Refused,
    /// A limit was reached before any schedule was found.
    NoSolution,
    /// No schedule satisfies the configured rules over the horizon.
    Infeasible,
    Cancelled,
    /// The backend failed, or its answer failed independent validation.
    Failed,
}

/// The last run that published nothing, owned by the inputs it was made
/// against. Its messages stand while those inputs do.
#[derive(Clone, Debug)]
pub(crate) struct ScheduleAttempt {
    pub(crate) serial: u64,
    pub(crate) semantic: u64,
    pub(crate) outcome: AttemptOutcome,
    pub(crate) messages: Vec<String>,
    pub(crate) repair: Option<ScheduleStep>,
}

/// What the worker hands back.
#[cfg(not(target_arch = "wasm32"))]
enum RunOutcome {
    Published(Arc<CalculatedSchedule>),
    NotPublished {
        outcome: AttemptOutcome,
        messages: Vec<String>,
        repair: Option<ScheduleStep>,
    },
}

/// The last day a horizon reaches, for labels.
fn day_of(end_h: f64) -> u32 {
    (end_h / SCHEDULE_PERIOD_H).ceil().max(0.0) as u32
}

impl crate::app::App<'_> {
    /// The authored bars, hashed: every field a calculated schedule depends
    /// on, and nothing else.
    ///
    /// Presentation names are deliberately absent: the drawn result resolves
    /// current labels by stable ids. Members are present because they are the
    /// ground.
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
            if let Some(work) = bar.reclaim() {
                1u8.hash(&mut hasher);
                work.sources.hash(&mut hasher);
                work.maximum_t.map(f64::to_bits).hash(&mut hasher);
            } else if bar.delay().is_some() {
                // Its type is presentation; that it is a delay is not.
                2u8.hash(&mut hasher);
            } else {
                0u8.hash(&mut hasher);
            }
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

    /// The complete semantic identity of a schedule calculation, cheaply.
    ///
    /// Every authored input the model encodes, hashed by stable id in a fixed
    /// order, and no presentation name - so a rename does not retire a
    /// result. Built from the *project* rather than by re-running capture,
    /// because currentness is asked every frame.
    ///
    /// How a run was solved is deliberately absent: the solve time limit and
    /// the gap target change how hard the optimiser looked, not what the
    /// schedule means. So is the planning end day, which bounds a run's
    /// horizon rather than entering its model; a result that reaches past a
    /// shortened planning end is retired by
    /// [`Self::schedule_calculation_is_current`] instead.
    pub(crate) fn schedule_semantic_key(&self) -> u64 {
        let inputs = self.schedule_run_inputs().ok();
        let plan_revision = self.schedule_plan_revision();
        let Some(project) = self.workspace.active_project() else {
            return 0;
        };
        let document = &project.project.document;
        let mut probe = std::collections::hash_map::DefaultHasher::new();
        inputs
            .map(|inputs| (inputs.runtime, inputs.generation, inputs.fleet_revision, inputs.tonnage_field.0))
            .hash(&mut probe);
        plan_revision.hash(&mut probe);
        let probe = probe.finish();
        if let Some((runtime, revision, held, key)) = self.schedule_semantic_cache.get()
            && runtime == project.runtime_id
            && revision == document.revision()
            && held == probe
        {
            return key;
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // The Setup gate and the authored bars, which between them cover the
        // project, the Solids run, the tonnage field, the fleet and every
        // bar's window, priority, ground and reclaim cap.
        probe.hash(&mut hasher);
        let plan = document.schedule();
        for calendar in plan.crusher_grade_calendars() {
            calendar.hash_content(&mut hasher);
        }
        for operation in plan.stockpile_operations() {
            operation.hash_content(&mut hasher);
        }
        // Reserve field *definitions*: a condition or a grade reads a field
        // through one, so re-aggregating or deleting one is an input change.
        for field in document.reserve_fields() {
            field.id.0.hash(&mut hasher);
            format!("{:?}", field.aggregation).hash(&mut hasher);
        }
        // The reclaim half of the fleet, which the Setup chain omits.
        for class in plan.classes() {
            (
                class.id.0,
                class.default_dig_rate_tph.to_bits(),
                class.default_reclaim_rate_tph.to_bits(),
                class.spot_time_s.to_bits(),
            )
                .hash(&mut hasher);
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
        // budget and opening inventory - by id, never by name.
        let routing = plan.routing();
        routing.enabled.hash(&mut hasher);
        document.haulage().hash_content(&mut hasher);
        plan.trucks().hash_content(&mut hasher);
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
            entry.dump_time_s.map(f64::to_bits).hash(&mut hasher);
            entry.inventory.hash_content(&mut hasher);
            entry.crusher.default_tpd.map(f64::to_bits).hash(&mut hasher);
            for (period, value) in &entry.crusher.periods {
                period.0.hash(&mut hasher);
                value.tonnes().map(f64::to_bits).hash(&mut hasher);
            }
        }
        // Rules and coefficients, each hashing its own content without its
        // name.
        for rule in &routing.rules {
            rule.hash_content_public(&mut hasher);
        }
        plan.trucks().hash_content(&mut hasher);
        plan.cashflow().hash_content(&mut hasher);
        // Ground taken out of mining.
        for solid in document.solids() {
            solid.id.hash(&mut hasher);
            solid.exclusions.hash_content(&mut hasher);
        }
        // The hours delay lists and rosters take each machine out.
        plan.delays().hash_calendar(&plan.agent_ids(), self.planning_end_h(), &mut hasher);
        // Resolution, tracked grades (legacy tags retained) and pile representation.
        plan.experiment().hash_semantics(&mut hasher);
        let key = hasher.finish();
        self.schedule_semantic_cache.set(Some((project.runtime_id, document.revision(), probe, key)));
        key
    }

    /// The configured planning end, in project hours.
    pub(crate) fn planning_end_h(&self) -> f64 {
        self.workspace.active_document().map_or(SCHEDULE_PERIOD_H, |document| {
            f64::from(document.schedule().experiment().planning_end_day) * SCHEDULE_PERIOD_H
        })
    }

    /// Whether the held result still describes the project as it stands.
    pub(crate) fn schedule_calculation_is_current(&self) -> bool {
        let Some(calculation) = self.schedule_calculation.as_ref() else {
            return false;
        };
        calculation.semantic == self.schedule_semantic_key() && calculation.requested_end_h <= self.planning_end_h() + 1e-9
    }

    /// Start a run, or say why one cannot start.
    ///
    /// Refusing is the common case while a project is being built up, so it
    /// is refused with the reason rather than by a dead button: the button is
    /// what the user pressed, and it has to answer.
    pub(crate) fn start_schedule_run(&mut self, mode: ScheduleRunMode) {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = mode;
            crate::userspace_warn!("{}", tr!("schedule-run-desktop-only"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.start_native_schedule_run(mode);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start_native_schedule_run(&mut self, mode: ScheduleRunMode) {
        use super::{
            commands::schedule_capture,
            jobs::JobKey,
            schedule_publish::{PublishMeta, publish},
            scip_blend::{ScipRunIdentity, ScipSolveOptions},
            solver_process,
        };

        let auto = mode == ScheduleRunMode::Auto;
        // Validate the lightweight Schedule Setup stages as part of the run.
        // Solids remains an explicit prerequisite.
        self.run_all_schedule_steps();
        let inputs = match self.schedule_run_inputs() {
            Ok(inputs) => inputs,
            Err(reason) => {
                // The status line already says why; a recalculation nobody
                // asked for does not repeat it in the console.
                if auto {
                    log::info!("schedule recalculation blocked: {}", reason.describe());
                } else {
                    crate::userspace_warn!("{}", tr!("schedule-run-blocked", reason = reason.describe()));
                }
                return;
            }
        };
        let end_h = self.planning_end_h();
        let held_end_h = self.schedule_calculation.as_ref().map(|held| held.requested_end_h.min(end_h));
        let requested_end_h = match mode {
            ScheduleRunMode::All => end_h,
            ScheduleRunMode::Auto => held_end_h.unwrap_or(end_h),
            ScheduleRunMode::Improve => held_end_h.filter(|_| self.schedule_calculation_is_current()).unwrap_or(end_h),
            ScheduleRunMode::Period => match self.schedule_calculation.as_ref().filter(|_| self.schedule_calculation_is_current()) {
                Some(held) if held.requested_end_h >= end_h - 1e-9 => {
                    crate::userspace_log!("{}", tr!("schedule-run-at-end", day = day_of(end_h).to_string()));
                    return;
                }
                Some(held) => (held.requested_end_h + SCHEDULE_PERIOD_H).min(end_h),
                None => SCHEDULE_PERIOD_H.min(end_h),
            },
        };
        let snapshot = match self.capture_schedule_snapshot(requested_end_h) {
            Ok(snapshot) => snapshot,
            Err(problems) => {
                let semantic = self.schedule_semantic_key();
                self.schedule_run_serial += 1;
                let serial = self.schedule_run_serial;
                self.record_attempt(ScheduleAttempt {
                    serial,
                    semantic,
                    outcome: AttemptOutcome::Refused,
                    repair: problems.iter().find_map(|problem| problem.repair),
                    messages: problems.iter().map(schedule_capture::CaptureDiagnostic::describe).collect(),
                });
                return;
            }
        };
        let Some(project) = self.workspace.active_project() else { return };
        let runtime = project.runtime_id;
        let document_revision = project.project.document.revision();
        let options = {
            let settings = project.project.document.schedule().experiment();
            ScipSolveOptions {
                time_limit: Some(std::time::Duration::from_secs_f64(settings.solve_seconds)),
                relative_gap: Some(settings.relative_gap),
                // Developer aid: SCIP's own progress log on stdout.
                diagnostic_logging: std::env::var_os("INCLINE_SCIP_LOG").is_some(),
                first_schedule_only: mode != ScheduleRunMode::Improve,
            }
        };
        self.cancel_schedule_run_calculation_quietly();
        let semantic = self.schedule_semantic_key();
        self.schedule_run_serial += 1;
        let serial = self.schedule_run_serial;
        let plan_revision = snapshot.plan_revision;
        let generation = snapshot.generation;
        let early_slot = Arc::new(Mutex::new(None));
        let worker_slot = Arc::clone(&early_slot);
        let window = self.window.clone();
        self.pending_schedule_run = Some(PendingScheduleRun {
            serial,
            runtime,
            inputs,
            semantic,
            requested_end_h,
            auto,
            improve: mode == ScheduleRunMode::Improve,
            early: early_slot,
            showing_early: false,
        });
        let identity = ScipRunIdentity {
            run_id: serial,
            inputs,
            plan_revision,
            document_revision,
            semantic,
        };
        self.spawn_job_quietly(
            tr!("schedule-run-job"),
            vec![JobKey::ScheduleRun { runtime, serial }],
            move |cancel| {
                let capture = match schedule_capture::build(&snapshot, cancel) {
                    Ok(capture) => capture,
                    Err(problems) => {
                        return Ok(if cancel.is_cancelled() {
                            RunOutcome::NotPublished {
                                outcome: AttemptOutcome::Cancelled,
                                messages: Vec::new(),
                                repair: None,
                            }
                        } else {
                            RunOutcome::NotPublished {
                                outcome: AttemptOutcome::Refused,
                                repair: problems.iter().find_map(|problem| problem.repair),
                                messages: problems.iter().map(schedule_capture::CaptureDiagnostic::describe).collect(),
                            }
                        });
                    }
                };
                let input = Arc::new(capture.input);
                let mut notes = capture.notes;
                if capture.stats.event_budget_restricted {
                    notes.push(tr!("schedule-note-event-budget", positions = input.segments_per_interval.to_string()));
                }
                let publish_completion = |completion: &super::scip_blend::ScipCompletion| {
                    let (Some(solution), Some(replay)) = (completion.solution.as_ref(), completion.replay.as_ref()) else {
                        return Err("the run holds no replayed schedule".to_owned());
                    };
                    let meta = PublishMeta {
                        run: serial,
                        semantic,
                        generation,
                        requested_end_h,
                        completion,
                        capture_s: capture.stats.duration.as_secs_f64(),
                        model_identity: capture.fingerprint,
                        candidates: capture.stats.candidates,
                        ground_sources: capture.stats.ground_sources,
                        event_budget_restricted: capture.stats.event_budget_restricted,
                        notes: notes.clone(),
                    };
                    publish(&input, solution, replay, &capture.identities, meta, cancel)
                };
                let early = |completion: &super::scip_blend::ScipCompletion| {
                    if !completion.usable() {
                        return;
                    }
                    match publish_completion(completion) {
                        Ok(schedule) => {
                            *worker_slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(schedule));
                            if let Some(window) = window.as_ref() {
                                window.request_redraw();
                            }
                        }
                        // The whole-horizon answer is still to come; a
                        // day-by-day schedule that cannot be published is
                        // simply not shown early.
                        Err(reason) => log::warn!("schedule run {serial}: day-by-day schedule not shown early: {reason}"),
                    }
                };
                let completion = solver_process::solve(Arc::clone(&input), identity, options, cancel, &early);
                if !completion.usable() {
                    return Ok(not_published(&completion));
                }
                Ok(match publish_completion(&completion) {
                    Ok(schedule) => RunOutcome::Published(Arc::new(schedule)),
                    Err(_) if cancel.is_cancelled() => RunOutcome::NotPublished {
                        outcome: AttemptOutcome::Cancelled,
                        messages: Vec::new(),
                        repair: None,
                    },
                    Err(reason) => RunOutcome::NotPublished {
                        outcome: AttemptOutcome::Failed,
                        messages: vec![tr!("schedule-run-publication-failed", reason = reason)],
                        repair: None,
                    },
                })
            },
            move |app, result| {
                let same_request = app
                    .pending_schedule_run
                    .as_ref()
                    .is_some_and(|pending| pending.serial == serial && pending.runtime == runtime);
                if !same_request {
                    return;
                }
                let showing_early = app.pending_schedule_run.take().is_some_and(|pending| pending.showing_early);
                // Checked again here, not only when the run started: an edit
                // that landed while it was solving retires it, and a late
                // answer is never published under inputs it did not read.
                if app.schedule_run_inputs().ok() != Some(inputs) || app.schedule_semantic_key() != semantic || app.planning_end_h() < requested_end_h - 1e-9 {
                    if !auto {
                        crate::userspace_warn!("{}", tr!("schedule-run-superseded"));
                    }
                    app.redraw_requested = true;
                    return;
                }
                match result {
                    Ok(RunOutcome::Published(schedule)) => {
                        let finished = tr!("schedule-run-finished", run = serial.to_string(), day = day_of(schedule.requested_end_h).to_string());
                        if auto {
                            log::info!("{finished}");
                        } else {
                            crate::userspace_log!("{finished}");
                        }
                        app.schedule_run_diagnostics = None;
                        app.schedule_calculation = Some(schedule);
                    }
                    Ok(RunOutcome::NotPublished { outcome, messages, repair }) => {
                        // The day-by-day schedule already on screen was
                        // replayed on its own; the whole-horizon solve
                        // failing does not retract it.
                        if showing_early {
                            app.settle_early_schedule(serial, DayByDayRole::Kept);
                        }
                        app.record_attempt(ScheduleAttempt {
                            serial,
                            semantic,
                            outcome,
                            messages,
                            repair,
                        })
                    }
                    Err(error) => {
                        if showing_early {
                            app.settle_early_schedule(serial, DayByDayRole::Kept);
                        }
                        app.record_attempt(ScheduleAttempt {
                            serial,
                            semantic,
                            outcome: AttemptOutcome::Failed,
                            messages: vec![format!("{error:#}")],
                            repair: None,
                        })
                    }
                }
                app.redraw_requested = true;
            },
        );
        self.redraw_requested = true;
    }

    /// Recalculate the schedule on its own once its inputs have settled
    /// after an edit: the first schedule only, over the horizon the held
    /// result covered.
    ///
    /// Runs only while switched on, with no run in flight and no current
    /// result, and at most once for one set of inputs - a recalculation
    /// that cannot succeed, or a run the user stopped, is not retried until
    /// something changes.
    pub(crate) fn auto_recalculate_schedule(&mut self) {
        self.schedule_auto_deadline = None;
        if cfg!(target_arch = "wasm32")
            || !self.editor.schedule_auto_recalculate
            || self.workspace.active_project().is_none()
            || self.pending_schedule_run.is_some()
            || self.schedule_calculation_is_current()
        {
            self.schedule_auto_settle = None;
            return;
        }
        let key = self.auto_key();
        if self.schedule_auto_attempted == Some(key) {
            return;
        }
        let now = web_time::Instant::now();
        let since = match self.schedule_auto_settle {
            Some((seen, since)) if seen == key => since,
            _ => {
                self.schedule_auto_settle = Some((key, now));
                // The pages were mirrored before this frame knew a
                // recalculation was coming; mirror them again now.
                self.redraw_requested = true;
                now
            }
        };
        if now < since + AUTO_SETTLE {
            self.schedule_auto_deadline = Some(since + AUTO_SETTLE);
            return;
        }
        self.schedule_auto_settle = None;
        self.start_schedule_run(ScheduleRunMode::Auto);
        // Starting validates the Setup steps, which moves the key; what is
        // remembered is what the run (or its refusal) was made against.
        self.schedule_auto_attempted = Some(self.auto_key());
    }

    /// Whether the held result is out of date only until a recalculation
    /// already under way, or about to start, replaces it.
    fn schedule_recalculating(&self) -> bool {
        if !self.editor.schedule_auto_recalculate || self.schedule_calculation_is_current() {
            return false;
        }
        match self.pending_schedule_run.as_ref() {
            Some(pending) => pending.auto,
            None => self.schedule_auto_settle.is_some() && self.schedule_auto_attempted != Some(self.auto_key()),
        }
    }

    /// What a recalculation depends on: the schedule's inputs and the
    /// planning end its horizon is clipped to.
    fn auto_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.schedule_semantic_key().hash(&mut hasher);
        self.planning_end_h().to_bits().hash(&mut hasher);
        // The semantic key holds the Setup gate only once it passes; these
        // move with the Solids run and the Setup inputs while it does not,
        // so a refused recalculation is tried again once they change.
        self.schedule_fingerprints().hash(&mut hasher);
        hasher.finish()
    }

    /// Publish nothing, keep whatever was held, and say why.
    fn record_attempt(&mut self, attempt: ScheduleAttempt) {
        self.schedule_auto_attempted = Some(self.auto_key());
        let headline = attempt_headline(&attempt);
        crate::userspace_warn!("{headline}");
        for message in &attempt.messages {
            crate::userspace_warn!("{message}");
        }
        self.schedule_run_diagnostics = Some(attempt);
        self.redraw_requested = true;
    }

    /// Stop a run in flight. The held result is untouched: when that is the
    /// run's own day-by-day schedule, stopping is how it is kept.
    pub(crate) fn cancel_schedule_run_calculation(&mut self) {
        if let Some(pending) = self.pending_schedule_run.as_ref() {
            let (serial, semantic, showing_early) = (pending.serial, pending.semantic, pending.showing_early);
            self.cancel_schedule_run_calculation_quietly();
            // Stopped on purpose: not to be restarted behind the user's back.
            self.schedule_auto_attempted = Some(self.auto_key());
            if showing_early {
                crate::userspace_log!("{}", tr!("schedule-run-stopped-early", run = serial.to_string()));
                return;
            }
            self.record_attempt(ScheduleAttempt {
                serial,
                semantic,
                outcome: AttemptOutcome::Cancelled,
                messages: Vec::new(),
                repair: None,
            });
        }
    }

    /// Stop a run in flight without reporting it: it is being replaced, or the
    /// project it belongs to is going away.
    pub(crate) fn cancel_schedule_run_calculation_quietly(&mut self) {
        if let Some(pending) = self.pending_schedule_run.take() {
            self.cancel_jobs(|key| matches!(key, crate::app::jobs::JobKey::ScheduleRun { serial, runtime } if *serial == pending.serial && *runtime == pending.runtime));
            if pending.showing_early {
                self.settle_early_schedule(pending.serial, DayByDayRole::Stopped);
            }
            self.redraw_requested = true;
        }
    }

    /// Say what became of a run's day-by-day schedule once no whole-horizon
    /// solve is looking for a better one.
    fn settle_early_schedule(&mut self, serial: u64, role: DayByDayRole) {
        let Some(held) = self.schedule_calculation.as_ref().filter(|held| held.run == serial) else {
            return;
        };
        let mut settled = CalculatedSchedule::clone(held);
        if let Some(summary) = settled.report.day_by_day.as_mut() {
            summary.role = role;
        }
        self.schedule_calculation = Some(Arc::new(settled));
    }

    /// Retire a background run as soon as its inputs move, and show its
    /// day-by-day schedule once the worker has one.
    pub(crate) fn advance_schedule_calculation(&mut self) {
        let Some(pending) = self.pending_schedule_run.as_ref() else {
            return;
        };
        let (inputs, semantic, requested_end_h, auto) = (pending.inputs, pending.semantic, pending.requested_end_h, pending.auto);
        if self.schedule_run_inputs().ok() != Some(inputs) || self.schedule_semantic_key() != semantic || self.planning_end_h() < requested_end_h - 1e-9 {
            self.cancel_schedule_run_calculation_quietly();
            // A recalculation overtaken by an edit is simply started again
            // once the edits settle; only a run someone asked for is reported.
            if !auto {
                crate::userspace_warn!("{}", tr!("schedule-run-superseded"));
            }
            return;
        }
        let early = pending.early.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        if let Some(schedule) = early {
            let serial = pending.serial;
            crate::userspace_log!(
                "{}",
                tr!("schedule-run-early", run = serial.to_string(), day = day_of(schedule.requested_end_h).to_string())
            );
            self.schedule_run_diagnostics = None;
            self.schedule_calculation = Some(schedule);
            if let Some(pending) = self.pending_schedule_run.as_mut() {
                pending.showing_early = true;
            }
            self.redraw_requested = true;
        }
    }

    /// Copy the held result into the editor state the pages draw from - but
    /// only while it is current.
    ///
    /// A stale result is kept and named, and its calculated figures are taken
    /// off the pages: drawing them beside bars that have since moved would put
    /// a figure next to ground it was not calculated from.
    pub(crate) fn mirror_schedule_calculation(&mut self) {
        let current = self.schedule_calculation_is_current();
        let running = self.pending_schedule_run.as_ref().map(|pending| pending.requested_end_h);
        let improving = self.pending_schedule_run.as_ref().is_some_and(|pending| pending.showing_early) && current;
        let improve = self.pending_schedule_run.as_ref().is_some_and(|pending| pending.improve);
        let result = self.schedule_calculation.clone().filter(|_| current);
        let semantic = self.schedule_semantic_key();
        let attempt = self.schedule_run_diagnostics.as_ref().filter(|attempt| attempt.semantic == semantic);
        let held_status = self.schedule_calculation.as_ref().map(|calculation| {
            if current {
                result_status(calculation)
            } else {
                tr!("schedule-run-stale", run = calculation.run.to_string())
            }
        });
        // A recalculation in hand says so quietly rather than warning that
        // the last result is out of date: it is about to be replaced.
        let recalculating = self.schedule_recalculating();
        let status = match (running, attempt, held_status) {
            _ if recalculating => tr!("schedule-run-updating"),
            (Some(end_h), _, _) if improving => tr!("schedule-run-improving", day = day_of(end_h).to_string()),
            (Some(end_h), _, _) => tr!("schedule-run-working", day = day_of(end_h).to_string()),
            // The newest attempt failed: say so first, then what is still
            // shown.
            (None, Some(attempt), Some(held)) if self.schedule_calculation.as_ref().is_none_or(|calculation| calculation.run <= attempt.serial) => {
                format!("{} {}", attempt_headline(attempt), held)
            }
            (None, Some(attempt), None) => attempt_headline(attempt),
            (None, _, Some(held)) => held,
            (None, _, None) => {
                if cfg!(target_arch = "wasm32") {
                    tr!("schedule-run-desktop-only")
                } else {
                    match self.schedule_run_blocker() {
                        None => tr!("schedule-run-never"),
                        Some(ScheduleNotReady::NotRun(ScheduleStep::Readiness)) => tr!("schedule-run-needs-solids"),
                        Some(reason) => tr!("schedule-run-blocked", reason = reason.describe()),
                    }
                }
            }
        };
        let mut details = Vec::new();
        if let Some(attempt) = attempt.filter(|attempt| self.schedule_calculation.as_ref().is_none_or(|calculation| calculation.run <= attempt.serial)) {
            details.extend(attempt.messages.iter().cloned());
        }
        if let Some(calculation) = self.schedule_calculation.as_ref().filter(|_| current) {
            let currency = self
                .workspace
                .active_document()
                .map(|document| document.schedule().currency().to_owned())
                .unwrap_or_default();
            details.extend(result_details(calculation, &currency));
        }
        let stale = !current && self.schedule_calculation.is_some() && !recalculating;
        let repair = if running.is_some() {
            None
        } else {
            attempt
                .and_then(|attempt| attempt.repair)
                .map(ScheduleRepairTarget::Schedule)
                .or_else(|| self.schedule_repair_target())
        };
        let same_result = match (&self.editor.schedule_result, &result) {
            (Some(held), Some(current)) => Arc::ptr_eq(held, current),
            (None, None) => true,
            _ => false,
        };
        if !same_result
            || self.editor.schedule_run_status != status
            || self.editor.schedule_run_details != details
            || self.editor.schedule_run_stale != stale
            || self.editor.schedule_run_working != running.is_some()
            || self.editor.schedule_run_improving != improving
            || self.editor.schedule_run_improve != improve
            || self.editor.schedule_run_repair != repair
        {
            self.editor.schedule_result = result;
            self.editor.schedule_run_status = status;
            self.editor.schedule_run_details = details;
            self.editor.schedule_run_stale = stale;
            self.editor.schedule_run_working = running.is_some();
            self.editor.schedule_run_improving = improving;
            self.editor.schedule_run_improve = improve;
            self.editor.schedule_run_repair = repair;
            self.redraw_requested = true;
        }
    }
}

/// Translate a finished solve that produced nothing publishable.
#[cfg(not(target_arch = "wasm32"))]
fn not_published(completion: &super::scip_blend::ScipCompletion) -> RunOutcome {
    use super::scip_blend::ScipTermination;

    let outcome = match completion.termination {
        ScipTermination::LimitNoIncumbent => AttemptOutcome::NoSolution,
        ScipTermination::Infeasible => AttemptOutcome::Infeasible,
        ScipTermination::Cancelled => AttemptOutcome::Cancelled,
        ScipTermination::InvalidInput => AttemptOutcome::Refused,
        ScipTermination::Optimal | ScipTermination::FeasibleLimit | ScipTermination::Unbounded | ScipTermination::ValidationFailure | ScipTermination::BackendFailure => {
            AttemptOutcome::Failed
        }
    };
    let mut messages = Vec::new();
    if outcome != AttemptOutcome::Cancelled {
        if let Some(reason) = completion.diagnostic.as_ref() {
            messages.push(reason.clone());
        }
        if completion.termination == ScipTermination::LimitNoIncumbent && completion.diagnostics.presolved_variables.is_some() && !completion.diagnostics.root_node_finished {
            messages.push(if completion.diagnostics.root_initial_lp_s.is_some() {
                tr!("schedule-detail-root-unfinished")
            } else {
                tr!("schedule-detail-root-lp-unobserved")
            });
        }
        // The first few independent-replay findings, which are what make a
        // solver's claim unpublishable. Every one is in the log.
        if let Some(replay) = completion.replay.as_ref() {
            for issue in replay.issues.iter().chain(&replay.grade_issues) {
                log::warn!("schedule replay: {issue}");
            }
            messages.extend(replay.issues.iter().chain(&replay.grade_issues).take(5).cloned());
        }
    }
    RunOutcome::NotPublished { outcome, messages, repair: None }
}

fn attempt_headline(attempt: &ScheduleAttempt) -> String {
    let run = attempt.serial.to_string();
    match attempt.outcome {
        AttemptOutcome::Refused => tr!("schedule-run-refused", run = run),
        AttemptOutcome::NoSolution => tr!("schedule-run-no-solution", run = run),
        AttemptOutcome::Infeasible => tr!("schedule-run-infeasible", run = run),
        AttemptOutcome::Cancelled => tr!("schedule-run-cancelled", run = run),
        AttemptOutcome::Failed => tr!("schedule-run-failed", run = run),
    }
}

/// The one line the run controls show for a held, current result: how far
/// it reaches, what it is worth, and how hard it was looked for.
fn result_status(calculation: &CalculatedSchedule) -> String {
    // Only what a planner acts on: how far the schedule reaches and whether
    // it can still be improved. The value, the run number and the solver's
    // working are in the details on hover.
    let day = day_of(calculation.requested_end_h).to_string();
    let report = &calculation.report;
    let first_only = report.day_by_day.as_ref().is_some_and(|start| start.role == DayByDayRole::Only);
    let mut status = match (report.quality, report.gap) {
        _ if first_only => tr!("schedule-run-first", day = day),
        (Some(SolveQuality::Optimal), _) => tr!("schedule-run-optimal", day = day),
        // The gap is a solver figure, and on a long horizon often measures
        // how loose the bound is rather than the schedule: it is on hover.
        _ => tr!("schedule-run-limited-no-gap", day = day),
    };
    // A restriction that can hold production back is said where the status
    // is read, not only in the details.
    if report.event_budget_restricted {
        status.push_str(&tr!("schedule-run-suffix-event-budget"));
    }
    if report.chunk_slots_full {
        status.push_str(&tr!("schedule-run-suffix-chunks-full"));
    }
    status
}

/// Everything the status tooltip says about a held result.
fn result_details(calculation: &CalculatedSchedule, currency: &str) -> Vec<String> {
    let report = &calculation.report;
    let mut lines = vec![tr!(
        "schedule-detail-value",
        value = crate::ui::elements::schedule_calendar::format_money(report.objective),
        currency = currency.to_owned()
    )];
    lines.push(match (report.bound, report.gap) {
        // A bound more than twice the schedule says nothing useful about it.
        (Some(_), Some(gap)) if gap > 1.0 => tr!("schedule-detail-bound-weak"),
        (Some(bound), Some(gap)) if report.bound_source == BoundSource::Relaxation => tr!(
            "schedule-detail-bound-relaxation",
            bound = crate::ui::elements::schedule_calendar::format_money(bound),
            gap = format!("{:.3}", gap * 100.0)
        ),
        (Some(bound), Some(gap)) => tr!(
            "schedule-detail-bound",
            bound = crate::ui::elements::schedule_calendar::format_money(bound),
            gap = format!("{:.3}", gap * 100.0)
        ),
        (Some(bound), None) => tr!("schedule-detail-bound-no-gap", bound = crate::ui::elements::schedule_calendar::format_money(bound)),
        _ => tr!("schedule-detail-no-bound"),
    });
    if let Some(start) = report.day_by_day.as_ref() {
        let seconds = format!("{:.2}", start.seconds);
        let method = match start.method {
            StartMethod::DayByDay => tr!("schedule-detail-start-day-by-day", windows = start.windows.to_string()),
            StartMethod::Hourly => tr!("schedule-detail-start-hourly", intervals = start.windows.to_string()),
        };
        lines.push(match (start.value, start.failure.as_ref()) {
            (Some(value), _) => {
                let value = crate::ui::elements::schedule_calendar::format_money(value);
                match start.role {
                    DayByDayRole::Improved => tr!("schedule-detail-start-improved", start = method, seconds = seconds, value = value),
                    DayByDayRole::Kept => tr!("schedule-detail-start-kept", start = method, seconds = seconds, value = value),
                    DayByDayRole::Early => tr!("schedule-detail-start-early", start = method, seconds = seconds, value = value),
                    DayByDayRole::Stopped => tr!("schedule-detail-start-stopped", start = method, seconds = seconds, value = value),
                    DayByDayRole::Proven => tr!("schedule-detail-start-proven", start = method, seconds = seconds, value = value),
                    DayByDayRole::Only => tr!("schedule-detail-start-only", start = method, seconds = seconds, value = value),
                }
            }
            (None, reason) => tr!("schedule-detail-day-by-day-failed", reason = reason.cloned().unwrap_or_default()),
        });
    }
    lines.push(tr!(
        "schedule-detail-timings",
        solve = format!("{:.2}", report.solve_s),
        capture = format!("{:.2}", report.capture_s),
        formulate = format!("{:.2}", report.formulation_s),
        replay = format!("{:.2}", report.replay_s + report.extraction_s + report.publication_s)
    ));
    lines.push(tr!(
        "schedule-detail-model",
        variables = report.variables.to_string(),
        binaries = report.binaries.to_string(),
        constraints = report.constraints.to_string(),
        intervals = report.intervals.to_string(),
        positions = report.segments_per_interval.to_string()
    ));
    lines.push(tr!(
        "schedule-detail-capture",
        candidates = report.candidates.to_string(),
        blocks = report.ground_sources.to_string(),
        identity = format!("{:016x}", report.model_identity)
    ));
    lines.push(tr!("schedule-detail-backend", backend = report.backend.clone()));
    lines.push(tr!(
        "schedule-detail-proof-progress",
        presolve = format!("{:.2}", report.diagnostics.presolve_s),
        nodes = report.diagnostics.nodes.to_string(),
        iterations = report.diagnostics.lp_iterations.to_string(),
        entries = report.linear_coefficient_entries.to_string()
    ));
    if let Some(first) = report.diagnostics.first_incumbent_s {
        lines.push(tr!("schedule-detail-first-incumbent", seconds = format!("{first:.2}")));
    }
    if report.quality == Some(SolveQuality::Limited) && report.diagnostics.presolved_variables.is_some() && !report.diagnostics.root_node_finished {
        lines.push(if report.diagnostics.root_initial_lp_s.is_some() {
            tr!("schedule-detail-root-unfinished")
        } else {
            tr!("schedule-detail-root-lp-unobserved")
        });
    }
    // What optimality does not remove, stated every time.
    lines.push(tr!("schedule-detail-optimal-scope"));
    lines.push(tr!("schedule-detail-release"));
    if report.chunk_slots > 0 {
        lines.push(tr!("schedule-detail-chunks", slots = report.chunk_slots.to_string()));
    }
    lines.push(tr!(
        "schedule-detail-grade-margin",
        fraction = format!("{:e}", report.grade_margin),
        percent = format!("{}", report.grade_margin * 100.0)
    ));
    if report.boundary_rows > 0 {
        lines.push(tr!(
            "schedule-detail-grade-band",
            deliveries = report.boundary_rows.to_string(),
            tonnes = format!("{:.3}", report.boundary_tonnes_t),
            value = crate::ui::elements::schedule_calendar::format_money(report.boundary_value_slack)
        ));
    }
    if report.indicator_leak_value > 0.005 {
        lines.push(tr!(
            "schedule-detail-indicator-leak",
            value = crate::ui::elements::schedule_calendar::format_money(report.indicator_leak_value)
        ));
    }
    if report.omitted_rows > 0 {
        lines.push(tr!(
            "schedule-detail-omitted",
            rows = report.omitted_rows.to_string(),
            tonnes = format!("{:.3e}", report.omitted_tonnes_t)
        ));
    }
    lines.extend(report.notes.iter().cloned());
    lines
}
