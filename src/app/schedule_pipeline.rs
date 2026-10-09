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
//! | Stockpiles | The stockpile solids and standalone stockpiles, and their capacities | Destinations that can receive |
//! | Dumps | The same, for dumps | Destinations that can receive |
//! | Crushers | Each crusher's daily budget and its overrides | Budgets the evaluator can spend |
//! | Destinations | The ordered routing rules | Rules whose destinations and fields resolve |
//! | Solids | The Solids run, which it starts when it is not current, and the tonnage field | Dig blocks that are all measured, or counted as 0 t |
//! | Scheduling Readiness | Everything above | [`ScheduleRunInputs`] |
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
    app::{
        planning_pipeline::{StageDiagnostic, StageNotReady, StageOutcome, StageState},
        step_pipeline::{Obsolete, PipelineStep, RunTurn, StepPipeline},
    },
    i18n::tr,
    model::{ReserveAggregation, ReserveFieldId},
    ui::state::{ScheduleRepairTarget, ScheduleStep},
};

/// Exactly what one Schedule Setup run validated, captured when it started.
///
/// Held with the result rather than re-read from the project: a result that
/// cannot say what it was calculated from cannot be told apart from one that
/// is still true.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// The Schedule Setup pipeline's state for one open project: the shared step
/// runner, and the result of the last run that reached the end.
#[derive(Debug)]
pub(crate) struct SchedulePipeline {
    steps: StepPipeline<ScheduleStep>,
    /// What the last completed run captured. Kept across edits and across
    /// cancellations - it is marked stale by [`Self::readiness`], never
    /// deleted, so the page can say what was true when it was last checked.
    result: Option<ScheduleRunResult>,
}

impl std::ops::Deref for SchedulePipeline {
    type Target = StepPipeline<ScheduleStep>;

    fn deref(&self) -> &Self::Target {
        &self.steps
    }
}

impl std::ops::DerefMut for SchedulePipeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.steps
    }
}

impl SchedulePipeline {
    pub(crate) fn new(runtime: u32) -> Self {
        Self {
            steps: StepPipeline::new(runtime),
            result: None,
        }
    }

    /// Publish the captured inputs of a run that reached the end.
    ///
    /// Called only by the readiness step, and only from the run that computed
    /// them: a result carrying another run's number would describe inputs it
    /// never saw.
    pub(crate) fn publish(&mut self, result: ScheduleRunResult) -> Result<(), Obsolete> {
        if result.run != self.generation() {
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
}

fn hash_of(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

impl PipelineStep for ScheduleStep {
    const ALL: &'static [Self] = &ScheduleStep::ALL;

    fn index(self) -> usize {
        ScheduleStep::index(self)
    }

    fn label(self) -> String {
        ScheduleStep::label(self)
    }
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
            // Another project's calculation says nothing about this one.
            self.schedule_calculation = None;
        }
        let fingerprints = self.schedule_fingerprints();
        let pipeline = self.schedule_pipeline.as_mut().expect("just ensured");
        let earliest_change = pipeline.take_fingerprints(&fingerprints);
        // A change that reaches only Solids is the Solids run moving on. A
        // running Solids step waits on that run and re-reads it every frame,
        // and settles against the inputs current then, so it is not stopped:
        // stopping it reported an edit that nobody made each time a project
        // opened.
        let earliest_change = earliest_change.filter(|&step| !(step == ScheduleStep::Solids && pipeline.running() == Some(ScheduleStep::Solids)));
        if let Some(step) = earliest_change
            && pipeline.invalidate_from(step)
        {
            crate::userspace_warn!("{}", tr!("schedule-run-stopped-by-edit", step = step.label()));
        }
        self.schedule_pipeline.as_mut().expect("just ensured").mark_stale();
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

        let classes: Vec<_> = plan.classes().iter().map(|class| (class.id.0, class.default_dig_rate_tph.to_bits(), class.kind)).collect();
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
        // Delays are checked as each is edited, so the step has nothing to
        // re-validate and a delay edit must not send Setup back to be run:
        // planners edit delays all the time. A calculated schedule still
        // retires on one, through `App::schedule_semantic_key`.
        let delays_step = hash_of((agents_step, "delays"));

        // Transport inputs hang off the fleet and go no further in the Setup
        // chain. A calculated schedule's own currentness reads them through
        // `App::schedule_semantic_key`, which is where a truck edit retires
        // it.
        let trucks = plan.trucks();
        let truck_classes: Vec<_> = trucks
            .classes
            .iter()
            .map(|class| {
                let periods: Vec<_> = class
                    .calendar
                    .periods
                    .iter()
                    .map(|(period, value)| (period.0, value.units, value.availability.map(f64::to_bits), value.utilisation.map(f64::to_bits)))
                    .collect();
                (
                    class.id.0,
                    class.name.clone(),
                    class.payload_t.to_bits(),
                    class.loaded_speed_kph.to_bits(),
                    class.unloaded_speed_kph.to_bits(),
                    class.calendar.default_units,
                    class.calendar.default_availability.to_bits(),
                    class.calendar.default_utilisation.to_bits(),
                    periods,
                )
            })
            .collect();
        let mut haul_hash = std::collections::hash_map::DefaultHasher::new();
        trucks.hash_content(&mut haul_hash);
        let truck_classes_step = hash_of((agents_step, truck_classes, std::hash::Hasher::finish(&haul_hash)));

        // Destinations are fingerprinted in the three groups the pages edit,
        // each chained onto the last, so editing a crusher's budget does not
        // retire the stockpile step - and the solids the stockpile and dump
        // pages *expose* are part of their inputs, because a solid renamed or
        // retyped in Solids changes what those pages list.
        let routing = plan.routing();
        let solid_destinations: Vec<_> = document
            .solids()
            .iter()
            .filter_map(|solid| {
                crate::model::schedule::DestinationKind::of_solid(solid.kind).map(|kind| {
                    (
                        solid.id.0,
                        solid.name.clone(),
                        kind,
                        routing.capacity_t(crate::model::schedule::DestinationId::Solid(solid.id)).map(f64::to_bits),
                    )
                })
            })
            .collect();
        let standalone = |kind: crate::model::schedule::DestinationKind| {
            routing
                .standalone
                .iter()
                .filter(move |entry| entry.kind == kind)
                .map(|entry| (entry.id.0, entry.name.clone(), entry.capacity_t.map(f64::to_bits)))
                .collect::<Vec<_>>()
        };
        let by_kind = |kind: crate::model::schedule::DestinationKind| solid_destinations.iter().filter(|(_, _, solid_kind, _)| *solid_kind == kind).cloned().collect::<Vec<_>>();
        // Which piles are used, and how each is represented: a used pile has
        // to have a representation. Only the reclaim bars' sources, so moving
        // a bar does not send Setup back to be run.
        let pile_use: Vec<_> = crate::model::schedule::destinations::available(document.solids(), routing)
            .iter()
            .filter(|entry| entry.kind == crate::model::schedule::DestinationKind::Stockpile)
            .map(|entry| {
                (
                    format!("{:?}", entry.id),
                    format!("{:?}", plan.experiment().representation(entry.id)),
                    routing.rules.iter().any(|rule| rule.enabled && rule.destinations.contains(&entry.id))
                        || plan.bars().iter().any(|bar| bar.reclaim().is_some_and(|reclaim| reclaim.sources.contains(&entry.id))),
                )
            })
            .collect();
        let stockpiles_step = hash_of((
            agents_step,
            by_kind(crate::model::schedule::DestinationKind::Stockpile),
            standalone(crate::model::schedule::DestinationKind::Stockpile),
            pile_use,
        ));
        let dumps_step = hash_of((
            stockpiles_step,
            by_kind(crate::model::schedule::DestinationKind::Dump),
            standalone(crate::model::schedule::DestinationKind::Dump),
        ));
        let crushers: Vec<_> = routing
            .standalone
            .iter()
            .filter(|entry| entry.kind == crate::model::schedule::DestinationKind::Crusher)
            .map(|entry| {
                let periods: Vec<_> = entry.crusher.periods.iter().map(|(period, value)| (period.0, value.tonnes().map(f64::to_bits))).collect();
                (entry.id.0, entry.name.clone(), entry.crusher.default_tpd.map(f64::to_bits), periods)
            })
            .collect();
        let crushers_step = hash_of((dumps_step, crushers));
        let rules = {
            let mut hasher = DefaultHasher::new();
            crushers_step.hash(&mut hasher);
            // The rules' own content hash, which already covers every field a
            // match depends on, including the fields the conditions name.
            for rule in &routing.rules {
                let mut rule_hasher = DefaultHasher::new();
                rule.hash_content_public(&mut rule_hasher);
                rule_hasher.finish().hash(&mut hasher);
            }
            // A condition reads a field through its definition, so
            // re-aggregating or deleting one is an input change.
            for field in document.reserve_fields() {
                field.id.0.hash(&mut hasher);
                field.name.hash(&mut hasher);
                format!("{:?}", field.aggregation).hash(&mut hasher);
            }
            hasher.finish()
        };
        let destinations_step = rules;

        let trucking_rules = {
            let mut hasher = DefaultHasher::new();
            truck_classes_step.hash(&mut hasher);
            for rule in &trucks.rules {
                let mut rule_hasher = DefaultHasher::new();
                rule.hash_content_public(&mut rule_hasher);
                rule_hasher.finish().hash(&mut hasher);
            }
            // What the rule's selectors can resolve against, so a destination
            // or loader that disappears is an input change here too.
            for entry in crate::model::schedule::destinations::available(document.solids(), plan.routing()) {
                format!("{:?}", entry.id).hash(&mut hasher);
            }
            for agent in plan.agents() {
                agent.id.0.hash(&mut hasher);
            }
            hasher.finish()
        };
        let trucking_rules_step = trucking_rules;

        // Cashflow hangs off the transport chain and, like it, reaches no
        // further: editing what a movement is worth must not retire a dig-only
        // calculation that never consulted a value. Only the coefficients are
        // folded in - a rule renamed is unsaved work, not a different number.
        let cashflow_step = {
            let mut hasher = DefaultHasher::new();
            trucking_rules_step.hash(&mut hasher);
            plan.cashflow().hash_content(&mut hasher);
            for field in document.reserve_fields() {
                field.id.0.hash(&mut hasher);
                format!("{:?}", field.aggregation).hash(&mut hasher);
            }
            // Include reclaim rates and opening inventory in readiness identity;
            // normal optimisation consumes these inputs as well as digging.
            for class in plan.classes() {
                class.id.0.hash(&mut hasher);
                class.default_reclaim_rate_tph.to_bits().hash(&mut hasher);
            }
            for agent in plan.agents() {
                agent.id.0.hash(&mut hasher);
                for (period, value) in &agent.calendar.periods {
                    period.0.hash(&mut hasher);
                    value.reclaim_rate_tph.map(f64::to_bits).hash(&mut hasher);
                }
            }
            for entry in crate::model::schedule::destinations::available(document.solids(), plan.routing()) {
                format!("{:?}", entry.id).hash(&mut hasher);
                if let Some(inventory) = plan.routing().inventory(entry.id) {
                    inventory.hash_content(&mut hasher);
                }
            }
            hasher.finish()
        };

        // Where the Solids run stands, not what it produced: a status is
        // cheap, and it moves exactly when a schedule's ground does.
        let solids = match self.planning_snapshot_status() {
            Ok(generation) => (0_u8, generation, String::new()),
            Err(reason) => (1, 0, reason.describe()),
        };
        // Exclusions decide which blocks have to be measured at all.
        let exclusions = {
            let mut hasher = DefaultHasher::new();
            for solid in document.solids() {
                solid.id.0.hash(&mut hasher);
                solid.exclusions.hash_content(&mut hasher);
            }
            hasher.finish()
        };
        let solids_step = hash_of((cashflow_step, solids, plan.unmeasured_as_zero(), exclusions));
        // Haulage checks the destinations and the Solids run's dig blocks
        // against the roads, so all three are its inputs.
        let haulage_step = {
            let mut hasher = DefaultHasher::new();
            (truck_classes_step, crushers_step, solids_step).hash(&mut hasher);
            document.haulage().hash_content(&mut hasher);
            hasher.finish()
        };
        let readiness_step = hash_of((destinations_step, solids_step, haulage_step));
        let drill_blast_step = {
            let mut hasher = DefaultHasher::new();
            delays_step.hash(&mut hasher);
            plan.drill_blast().hash_content(&mut hasher);
            hasher.finish()
        };

        // Dates and colours name the days; they change nothing a run reads.
        // The horizon they show is the experiment's, which a calculated
        // schedule's currentness already reads.
        let periods_step = hash_of((configuration, "periods"));

        [
            configuration,
            periods_step,
            classes_step,
            agents_step,
            delays_step,
            drill_blast_step,
            stockpiles_step,
            dumps_step,
            crushers_step,
            destinations_step,
            trucking_rules_step,
            cashflow_step,
            solids_step,
            haulage_step,
            readiness_step,
        ]
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
        let reason = self.schedule_run_blocker()?;
        let schedule_step = match reason {
            ScheduleNotReady::NoProject => return None,
            ScheduleNotReady::NotRun(step) | ScheduleNotReady::Stale(step) | ScheduleNotReady::Running(step) | ScheduleNotReady::Failed { step, .. } => step,
        };
        if matches!(schedule_step, ScheduleStep::Solids | ScheduleStep::Readiness)
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

    /// What actually stops a schedule run starting now, if anything.
    ///
    /// A Setup step that has not been run, or has gone stale, is no blocker:
    /// a run validates the Setup steps itself before it captures. What a run
    /// cannot fix is a Setup step that fails, and a Solids run that is
    /// missing - which the Solids step reports as its own, so that is looked for
    /// first and named as such.
    pub(crate) fn schedule_run_blocker(&self) -> Option<ScheduleNotReady> {
        let reason = self.schedule_run_inputs().err()?;
        match reason {
            ScheduleNotReady::NotRun(_) | ScheduleNotReady::Stale(_) => {
                let project = self.workspace.active_project()?;
                let solids = self.planning_pipeline.as_ref().filter(|pipeline| pipeline.runtime == project.runtime_id)?;
                solids
                    .readiness(&self.planning_fingerprints())
                    .err()
                    .map(|_| ScheduleNotReady::NotRun(ScheduleStep::Solids))
            }
            reason => Some(reason),
        }
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
        // As on Solids: a step run on its own that succeeds moves the list on.
        if let Some(step) = self.schedule_advance_after
            && !matches!(views.get(step.index()).map(|view| view.state), Some(StageState::Queued | StageState::Running))
        {
            self.schedule_advance_after = None;
            let next = ScheduleStep::ALL.into_iter().nth(step.index() + 1);
            if let (Some(StageState::Complete), Some(next)) = (views.get(step.index()).map(|view| view.state), next)
                && self.editor.schedule_setup_step == step
            {
                self.editor.schedule_setup_step = next;
                self.redraw_requested = true;
            }
        }
        if self.editor.schedule_stages != views || self.editor.schedule_run_active != running || self.editor.schedule_calculation_status != calculation {
            self.editor.schedule_stages = views;
            self.editor.schedule_run_active = running;
            self.editor.schedule_calculation_status = calculation;
            self.redraw_requested = true;
        }
    }

    /// Reset the Schedule Setup pipeline and run from its first step through
    /// the selected one.
    /// Whether the run started.
    pub(crate) fn run_schedule_step(&mut self, step: ScheduleStep) -> bool {
        self.run_schedule_step_as(step, true)
    }

    /// As [`Self::run_schedule_step`], saying whether someone asked for the
    /// run: only then may its Solids step start the Solids run.
    fn run_schedule_step_as(&mut self, step: ScheduleStep, requested: bool) -> bool {
        self.sync_schedule_pipeline();
        self.schedule_setup_runs_solids = requested;
        let Some(pipeline) = self.schedule_pipeline.as_mut() else {
            return false;
        };
        if !pipeline.restart_through(step) {
            return false;
        }
        self.advance_schedule_run();
        self.mirror_schedule_stages();
        true
    }

    pub(crate) fn run_all_schedule_steps(&mut self) {
        self.run_schedule_step(ScheduleStep::Readiness);
    }

    /// Validate every Setup step for a recalculation nobody pressed a button
    /// for, leaving the Solids run as it stands.
    pub(crate) fn check_all_schedule_steps(&mut self) {
        self.run_schedule_step_as(ScheduleStep::Readiness, false);
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
                RunTurn::Idle => return,
                RunTurn::Skip => continue,
                RunTurn::Start(step) => pipeline.start(step),
                RunTurn::Evaluate(step) => {
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
            ScheduleStep::Periods => self.evaluate_periods(),
            ScheduleStep::LoaderClasses => self.evaluate_loader_classes(),
            ScheduleStep::LoaderAgents => self.evaluate_loader_agents(),
            ScheduleStep::Delays => self.evaluate_delays(),
            ScheduleStep::DrillBlast => self.evaluate_drill_blast(),
            ScheduleStep::Stockpiles => self.evaluate_destination_kind(crate::model::schedule::DestinationKind::Stockpile),
            ScheduleStep::Dumps => self.evaluate_destination_kind(crate::model::schedule::DestinationKind::Dump),
            ScheduleStep::Crushers => self.evaluate_crushers(),
            ScheduleStep::Destinations => self.evaluate_destination_rules(),
            ScheduleStep::Haulage => self.evaluate_schedule_haulage(),
            ScheduleStep::TruckingRules => self.evaluate_trucking_rules(),
            ScheduleStep::Cashflow => self.evaluate_cashflow(),
            ScheduleStep::Solids => self.evaluate_schedule_solids(),
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

    /// Delays are optional, and every edit is checked as it is made, so the
    /// step has nothing to block on: it counts what there is.
    /// Periods has nothing to refuse: without a start date the schedule
    /// reads in day numbers, as it always has.
    fn evaluate_periods(&self) -> StageOutcome {
        let entities = self
            .workspace
            .active_document()
            .map_or(0, |document| document.schedule().experiment().planning_end_day as usize);
        StageOutcome::Settled {
            diagnostics: Vec::new(),
            entities,
        }
    }

    fn evaluate_delays(&self) -> StageOutcome {
        let entities = self.workspace.active_document().map_or(0, |document| {
            let delays = document.schedule().delays();
            delays.types.len() + delays.lists.len() + delays.rosters.len()
        });
        StageOutcome::Settled {
            diagnostics: Vec::new(),
            entities,
        }
    }

    /// Drill and blast is optional: off, or on with the blasts it sequences.
    /// A blast bar on a machine whose class is not a dozer, drill or MPU is
    /// refused when it is made, so nothing here can block a run.
    fn evaluate_drill_blast(&self) -> StageOutcome {
        let mut diagnostics = Vec::new();
        let entities = self.workspace.active_document().map_or(0, |document| {
            let config = document.schedule().drill_blast();
            if !config.enabled {
                return 0;
            }
            // Blasts that start Fired need no window, so this holds nothing
            // back; the rest wait all horizon.
            if config.effective_windows().is_empty() {
                diagnostics.push(StageDiagnostic {
                    entity: None,
                    message: tr!("drill-blast-no-windows"),
                    blocking: false,
                });
            }
            document.schedule().bars().iter().filter(|bar| bar.blast_order().is_some()).count()
        });
        StageOutcome::Settled { diagnostics, entities }
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

    /// The stockpiles or dumps this project can deliver to: the solids of that
    /// kind, plus the standalone destinations of it.
    ///
    /// Capacities are validated on the way in, so what is left to say here is
    /// about references rather than numbers: a capacity stored against a solid
    /// that is no longer a stockpile is *kept* - the solid change is undoable -
    /// and reported as a note, because a setting that is being preserved and
    /// one that is being applied should not look alike.
    fn evaluate_destination_kind(&self, kind: crate::model::schedule::DestinationKind) -> StageOutcome {
        use crate::model::schedule::destinations;

        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let plan = document.schedule();
        let routing = plan.routing();
        let available = destinations::available(document.solids(), routing);
        let mut diagnostics = Vec::new();
        let mut entities = 0;
        for entry in available.iter().filter(|entry| entry.kind == kind) {
            entities += 1;
            match entry.capacity_t {
                None => {}
                Some(capacity) if capacity.is_finite() && capacity >= 0.0 => {}
                Some(_) => diagnostics.push(StageDiagnostic {
                    entity: Some(entry.name.clone()),
                    message: crate::model::schedule::ScheduleError::InvalidCapacity.message(),
                    blocking: true,
                }),
            }
        }
        // Retained settings whose solid is gone or has been retyped. Not
        // blocking: nothing routes to them, and the rules that named them are
        // where the blocking problem is reported. Counted on the first of the
        // two pages only, so one retained setting is not reported twice.
        let orphans = if kind == crate::model::schedule::DestinationKind::Stockpile {
            routing
                .solids
                .iter()
                .filter(|entry| {
                    document
                        .solid(entry.solid)
                        .and_then(|solid| crate::model::schedule::DestinationKind::of_solid(solid.kind))
                        .is_none()
                })
                .count()
        } else {
            0
        };
        if orphans > 0 {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("destination-stage-retained", count = orphans.to_string()),
                blocking: false,
            });
        }
        // Opening stock against capacity. Refused when it is typed and when a
        // file is read, so reaching this means a capacity and a pile parted
        // company some other way; said out loud rather than assumed impossible,
        // and not blocking, because nothing in a dig-only run reads either.
        if kind == crate::model::schedule::DestinationKind::Stockpile {
            for entry in available.iter().filter(|entry| entry.kind == kind) {
                // What a calculation would refuse, said here first. Blocking
                // only on a pile something delivers to or reclaims from: one
                // nothing uses stops no run.
                let used = routing.rules.iter().any(|rule| rule.enabled && rule.destinations.contains(&entry.id))
                    || plan.bars().iter().any(|bar| bar.reclaim().is_some_and(|reclaim| reclaim.sources.contains(&entry.id)));
                for message in stockpile_problems(plan, document, entry) {
                    diagnostics.push(StageDiagnostic {
                        entity: Some(entry.name.clone()),
                        message,
                        blocking: used,
                    });
                }
                if entry.capacity_t.is_some_and(|capacity| entry.opening_t > capacity) {
                    diagnostics.push(StageDiagnostic {
                        entity: Some(entry.name.clone()),
                        message: tr!("inventory-stage-over-capacity", stockpile = entry.name.clone()),
                        blocking: false,
                    });
                }
                let invalid = routing
                    .inventory(entry.id)
                    .map_or(0, |inventory| inventory.invalid_field_references(document.reserve_fields()));
                if invalid > 0 {
                    diagnostics.push(StageDiagnostic {
                        entity: Some(entry.name.clone()),
                        message: tr!("inventory-stage-invalid-fields", count = invalid.to_string()),
                        // Capture refuses invalid fields on stockpiles actually
                        // used by the requested horizon, not on unused stockpiles.
                        blocking: false,
                    });
                }
            }
        }
        StageOutcome::Settled { diagnostics, entities }
    }

    /// Each crusher's daily budget: the default and every override it carries.
    fn evaluate_crushers(&self) -> StageOutcome {
        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let routing = document.schedule().routing();
        let crushers: Vec<_> = routing
            .standalone
            .iter()
            .filter(|entry| entry.kind == crate::model::schedule::DestinationKind::Crusher)
            .collect();
        let mut diagnostics = Vec::new();
        for crusher in &crushers {
            if let Err(error) = crusher.crusher.validate() {
                diagnostics.push(StageDiagnostic {
                    entity: Some(crusher.name.clone()),
                    message: error.message(),
                    blocking: true,
                });
            }
        }
        StageOutcome::Settled {
            diagnostics,
            entities: crushers.len(),
        }
    }

    /// The trucking rules: does each enabled one still resolve what it names.
    ///
    /// Also never blocking, for the same reason. A broken reference is reported
    /// against the rule that holds it and is never repaired: deleting a
    /// destination must not silently widen a rule that named it.
    fn evaluate_trucking_rules(&self) -> StageOutcome {
        use crate::model::schedule::{DestinationSelection, LoaderSelection, MovementSourceScope, MovementSourceSelection, destinations};
        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let plan = document.schedule();
        let trucks = plan.trucks();
        let routing = plan.routing();
        let mut diagnostics = Vec::new();
        if trucks.rules.is_empty() && !trucks.classes.is_empty() {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("truck-stage-no-rules"),
                blocking: false,
            });
        }
        let resolves = |id| destinations::resolve(id, document.solids(), routing).is_ok();
        for rule in trucks.rules.iter().filter(|rule| rule.enabled) {
            let mut report = |message: String| {
                diagnostics.push(StageDiagnostic {
                    entity: Some(rule.name.clone()),
                    message,
                    blocking: false,
                });
            };
            if let LoaderSelection::Only(agents) = &rule.loaders
                && agents.iter().any(|agent| plan.agent(*agent).is_none())
            {
                report(tr!("destination-stage-rule-loader-missing"));
            }
            if let MovementSourceSelection::Only(scopes) = &rule.sources
                && scopes.iter().any(|scope| matches!(scope, MovementSourceScope::Stockpile(id) if !resolves(*id)))
            {
                report(tr!("truck-rule-destination-missing"));
            }
            if let DestinationSelection::Only(ids) = &rule.destinations
                && ids.iter().any(|id| !resolves(*id))
            {
                report(tr!("truck-rule-destination-missing"));
            }
            if rule.classes.iter().any(|class| trucks.class(*class).is_none()) {
                report(tr!("truck-rule-class-missing"));
            }
        }
        StageOutcome::Settled {
            diagnostics,
            entities: trucks.rules.len(),
        }
    }

    /// The cashflow rules: does each enabled one still resolve what it names.
    ///
    /// Empty cashflow is valid. Capture refuses unresolved enabled rules when
    /// constructing the run. Nothing is silently repaired or widened here.
    fn evaluate_cashflow(&self) -> StageOutcome {
        use crate::model::schedule::{DestinationSelection, LoaderSelection, MovementSourceScope, MovementSourceSelection, destinations};
        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let plan = document.schedule();
        let cashflow = plan.cashflow();
        let routing = plan.routing();
        let mut diagnostics = Vec::new();
        if cashflow.rules.is_empty() {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("cashflow-stage-no-rules"),
                blocking: false,
            });
        }
        let resolves = |id| destinations::resolve(id, document.solids(), routing).is_ok();
        for rule in cashflow.rules.iter().filter(|rule| rule.enabled) {
            let mut report = |message: String| {
                diagnostics.push(StageDiagnostic {
                    entity: Some(rule.name.clone()),
                    message,
                    blocking: false,
                });
            };
            if let LoaderSelection::Only(agents) = &rule.loaders
                && agents.iter().any(|agent| plan.agent(*agent).is_none())
            {
                report(tr!("destination-stage-rule-loader-missing"));
            }
            if let MovementSourceSelection::Only(scopes) = &rule.sources
                && scopes.iter().any(|scope| matches!(scope, MovementSourceScope::Stockpile(id) if !resolves(*id)))
            {
                report(tr!("truck-rule-destination-missing"));
            }
            if let DestinationSelection::Only(ids) = &rule.destinations
                && ids.iter().any(|id| !resolves(*id))
            {
                report(tr!("truck-rule-destination-missing"));
            }
            // A condition reads its field through the definition, so a field
            // that is gone or has been re-aggregated invalidates the rule
            // rather than being read some other way.
            for condition in &rule.conditions {
                let field = document.reserve_fields().iter().find(|field| field.id == condition.field);
                match field {
                    None => report(tr!("destination-stage-field-missing")),
                    Some(field) => {
                        let categorical = field.aggregation == crate::model::ReserveAggregation::Category;
                        let wants_category = matches!(condition.test, crate::model::schedule::ConditionTest::Category { .. });
                        if categorical != wants_category {
                            report(tr!("destination-stage-field-kind"));
                        }
                    }
                }
            }
            // Said out loud rather than refused: a rule that describes
            // movements and pays nothing is legitimate, and is usually a
            // figure somebody has not filled in yet.
            if rule.value_per_tonne == 0.0 {
                report(tr!("cashflow-stage-zero-value", rule = rule.name.clone()));
            }
        }
        StageOutcome::Settled {
            diagnostics,
            entities: cashflow.rules.len(),
        }
    }

    /// The routing rules: does each one name a destination that still exists,
    /// and fields that can carry the conditions written against them.
    ///
    /// Source scopes are deliberately *not* checked here. A scope names ground,
    /// and which ground exists is the Solids run's answer - so an unplaceable
    /// scope is a readiness problem, reported against the run that could not
    /// place it.
    ///
    /// A disabled rule is skipped entirely: it is switched off, not broken.
    fn evaluate_destination_rules(&self) -> StageOutcome {
        use crate::model::{
            ReserveAggregation as Aggregation,
            schedule::{ConditionTest, destinations},
        };

        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let plan = document.schedule();
        let routing = plan.routing();
        let mut diagnostics = Vec::new();
        if routing.rules.iter().all(|rule| !rule.enabled) {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("destination-stage-no-rules"),
                blocking: true,
            });
        }
        for rule in routing.rules.iter().filter(|rule| rule.enabled) {
            for destination in &rule.destinations {
                if let Err(problem) = destinations::resolve(*destination, document.solids(), routing) {
                    diagnostics.push(StageDiagnostic {
                        entity: Some(rule.name.clone()),
                        message: problem.message(&rule.name),
                        blocking: true,
                    });
                }
            }
            if let crate::model::schedule::LoaderSelection::Only(agents) = &rule.loaders
                && agents.iter().any(|agent| plan.agent(*agent).is_none())
            {
                diagnostics.push(StageDiagnostic {
                    entity: Some(rule.name.clone()),
                    message: tr!("destination-stage-rule-loader-missing"),
                    blocking: true,
                });
            }
            for condition in &rule.conditions {
                let Some(field) = document.reserve_fields().iter().find(|field| field.id == condition.field) else {
                    diagnostics.push(StageDiagnostic {
                        entity: Some(rule.name.clone()),
                        message: tr!("destination-stage-field-missing"),
                        blocking: true,
                    });
                    continue;
                };
                // A condition is written against a field's kind. Re-aggregating
                // a category field into a weighted average does not make its
                // value set a range, so the mismatch is a configuration error
                // rather than a condition that quietly stops matching.
                let categorical = field.aggregation == Aggregation::Category;
                let wants_category = matches!(condition.test, ConditionTest::Category { .. });
                if categorical != wants_category {
                    diagnostics.push(StageDiagnostic {
                        entity: Some(rule.name.clone()),
                        message: tr!("destination-stage-field-kind", field = field.name.clone()),
                        blocking: true,
                    });
                }
            }
        }
        StageOutcome::Settled {
            diagnostics,
            entities: routing.rules.len(),
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
        if self.workspace.active_document().and_then(|document| document.schedule().tonnage_field()).is_none() {
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
        }

        let mut diagnostics = Vec::new();
        // One line for every unconnected block, not one per block: Haulage →
        // Layout tints them red, which says which far better than a list.
        if let Some(document) = self.workspace.active_document() {
            let network = document.haulage();
            if !network.roads.is_empty() {
                let index = crate::model::haulage::network::RoadIndex::new(network);
                let grade = document.schedule().trucks().classes.iter().map(|c| c.maximum_grade).reduce(f64::min).unwrap_or(0.1);
                let reach = network.settings.auto_join_m;
                let unconnected: Vec<f64> = snapshot
                    .blocks
                    .iter()
                    // A block held to a node is connected by choice.
                    .filter(|block| {
                        !network
                            .block_link(block.solid, block.flitch.base, &block.ground)
                            .iter()
                            .any(|id| index.node_join(*id).is_some())
                    })
                    .filter_map(|block| index.access_m(glam::DVec3::new(block.anchor[0], block.anchor[1], block.flitch.base), grade))
                    .filter(|access| *access > reach)
                    .collect();
                if !unconnected.is_empty() {
                    diagnostics.push(StageDiagnostic {
                        entity: None,
                        message: tr!(
                            "haul-unconnected-note",
                            areas = unconnected.len(),
                            longest = format!("{:.0}", unconnected.iter().copied().fold(0.0, f64::max))
                        ),
                        blocking: false,
                    });
                }
            }
        }
        StageOutcome::Settled {
            diagnostics,
            entities: snapshot.blocks.len(),
        }
    }

    /// The Solids run the schedule digs, run here when it is not current, and
    /// the tonnes each block it makes reads on the chosen field.
    ///
    /// Every block that is still mined has to be measured, unless the planner
    /// chose to count the unmeasured ones as 0 t. Excluded ground is not in
    /// the run's blocks, so it is never read.
    fn evaluate_schedule_solids(&mut self) -> StageOutcome {
        use crate::app::commands::solids_view::PlanningNotReady;

        let blocked = |message: String| StageOutcome::Settled {
            diagnostics: vec![StageDiagnostic {
                entity: None,
                message,
                blocking: true,
            }],
            entities: 0,
        };
        match self.planning_snapshot_status() {
            Ok(_) => {}
            // A Solids run in flight is not a failure: this step waits for it,
            // the way the block-model step waits for its scans.
            Err(reason @ PlanningNotReady::Running { .. }) => {
                return StageOutcome::Working {
                    message: Some(tr!("schedule-stage-waiting-solids", reason = reason.describe())),
                };
            }
            // As Haulage does: the Schedule's setup runs what it depends on.
            // Only a run someone asked for: an automatic recalculation never
            // starts Reserving behind the user's back.
            Err(reason @ (PlanningNotReady::NotRun { .. } | PlanningNotReady::Stale { .. })) => {
                return if self.schedule_setup_runs_solids && self.resume_planning_stages(crate::ui::state::SolidsStep::LAST) {
                    StageOutcome::Working {
                        message: Some(tr!("schedule-stage-waiting-solids", reason = reason.describe())),
                    }
                } else {
                    blocked(reason.describe())
                };
            }
            Err(reason) => return blocked(reason.describe()),
        }
        let snapshot = match self.planning_snapshot() {
            Ok(snapshot) => snapshot,
            Err(reason) => return blocked(reason.describe()),
        };
        let Some(document) = self.workspace.active_document() else {
            return blocked(tr!("planning-snapshot-no-project"));
        };
        let Some(field) = document.schedule().tonnage_field() else {
            // Configuration blocks before this step is reached; a run that got
            // here without one has had the choice removed underneath it.
            return blocked(tr!("schedule-stage-no-tonnage-field"));
        };
        let zero = document.schedule().unmeasured_as_zero();

        let mut diagnostics = Vec::new();
        // Two pits cut from the same design, topography and model reserve the
        // same ground, and each would schedule it. Refused rather than one
        // quietly subtracted from the other.
        let pits: Vec<_> = document.solids().iter().filter(|solid| solid.kind.is_blasted() && solid.surface.is_some()).collect();
        for (index, solid) in pits.iter().enumerate() {
            if let Some(first) = pits[..index]
                .iter()
                .find(|other| (other.surface, other.topography, other.block_model) == (solid.surface, solid.topography, solid.block_model))
            {
                diagnostics.push(StageDiagnostic {
                    entity: Some(solid.name.clone()),
                    message: tr!("schedule-stage-duplicate-solid", solid = solid.name.clone(), other = first.name.clone()),
                    blocking: true,
                });
            }
        }
        let mut unmeasured = 0;
        let mut counted_zero = 0;
        let mut negative = 0;
        let grades: Vec<ReserveFieldId> = document.schedule().experiment().grades.iter().map(|(grade, _)| *grade).collect();
        let mut negative_grades = vec![0usize; grades.len()];
        for block in &snapshot.blocks {
            if let super::commands::solids_view::MaterialState::Measured(totals) = &block.material {
                let has_negatives = |id: &ReserveFieldId| totals.all.numeric.get(id).is_some_and(|total| total.negatives > 0);
                if has_negatives(&field) {
                    negative += 1;
                }
                for (count, grade) in negative_grades.iter_mut().zip(&grades) {
                    if has_negatives(grade) {
                        *count += 1;
                    }
                }
            }
            match super::commands::schedule_readiness::block_tonnes_for_stage(block, field, false) {
                Ok(_) => {}
                // A figure that is there but cannot be tonnes is a broken
                // project whatever the planner chose.
                Err(reason) if reason.invalid => diagnostics.push(StageDiagnostic {
                    entity: Some(block.name.clone()),
                    message: reason.message,
                    blocking: true,
                }),
                Err(_) if zero && super::commands::schedule_readiness::block_tonnes_for_stage(block, field, true).is_ok() => counted_zero += 1,
                Err(_) => unmeasured += 1,
            }
        }
        if unmeasured > 0 {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("schedule-stage-blocks-unmeasured", count = unmeasured.to_string()),
                blocking: true,
            });
        }
        // Read as 0 t rather than refused: models write -99 and the like for
        // a blank, and filtering those out is the model's business.
        if negative > 0 {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("schedule-stage-blocks-negative-tonnes", count = negative.to_string()),
                blocking: false,
            });
        }
        for (count, grade) in negative_grades.iter().zip(&grades) {
            if *count > 0 {
                let name = document
                    .reserve_fields()
                    .iter()
                    .find(|known| known.id == *grade)
                    .map_or_else(String::new, |known| known.name.clone());
                diagnostics.push(StageDiagnostic {
                    entity: Some(name.clone()),
                    message: tr!("schedule-stage-blocks-negative-grade", count = count.to_string(), grade = name),
                    blocking: false,
                });
            }
        }
        // Shown on the step's page rather than as a warning: the planner
        // chose it.
        if self.editor.schedule_zero_blocks != counted_zero {
            self.editor.schedule_zero_blocks = counted_zero;
            self.redraw_requested = true;
        }
        StageOutcome::Settled {
            diagnostics,
            entities: snapshot.blocks.len(),
        }
    }
}

/// What would stop a calculation using this stockpile: a chunked pile
/// without the chunk size it is divided by, and an opening
/// chunk missing a grade the schedule tracks. The same checks capture makes,
/// worded for the Stockpiles page.
pub(crate) fn stockpile_problems(
    plan: &crate::model::schedule::SchedulePlan,
    document: &crate::model::Document,
    entry: &crate::model::schedule::destinations::DestinationView,
) -> Vec<String> {
    use crate::model::schedule::experiment::StockpileRepresentation;

    let mut problems = Vec::new();
    let experiment = plan.experiment();
    if experiment.representation(entry.id) == StockpileRepresentation::Chunks {
        match experiment.chunk_t(entry.id) {
            None => problems.push(tr!("pile-chunk-size-missing")),
            Some(chunk_t) if entry.capacity_t.is_some_and(|capacity| chunk_t > capacity) => problems.push(tr!("pile-chunk-over-capacity")),
            Some(_) => {}
        }
    }
    for chunk in plan.routing().inventory(entry.id).map(|inventory| inventory.lots.as_slice()).unwrap_or_default() {
        for (field, _) in &experiment.grades {
            let Some(name) = document.reserve_fields().iter().find(|known| known.id == *field).map(|known| known.name.clone()) else {
                continue;
            };
            let missing = chunk
                .portions
                .iter()
                .any(|portion| !matches!(portion.value(*field), Some(crate::model::schedule::OpeningValue::Number(value)) if value.is_finite()));
            let negative = chunk
                .portions
                .iter()
                .any(|portion| matches!(portion.value(*field), Some(crate::model::schedule::OpeningValue::Number(value)) if *value < 0.0));
            if missing {
                problems.push(tr!("pile-chunk-grade-missing", chunk = chunk.name.clone(), grade = name));
            } else if negative {
                problems.push(tr!("pile-chunk-grade-negative", chunk = chunk.name.clone(), grade = name));
            }
        }
    }
    problems
}
