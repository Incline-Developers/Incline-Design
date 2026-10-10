//! The Haulage workspace's Setup pipeline: the road network and the truck
//! classes that drive it, checked in order.
//!
//! | Step | Reads | Produces |
//! | --- | --- | --- |
//! | Road Network | The network, its settings, the destinations and the Solids run's dig blocks | Roads checked for gaps, dead ends and steep grades |
//! | Truck Classes | Each class's payload, grade speeds and fleet calendar | Classes the cycle model can use |
//!
//! Built on [`crate::app::step_pipeline`] like the Schedule pipeline, which
//! depends on this one through its Haulage step: that step runs this pipeline
//! when it is not current and waits for it.

use std::hash::{DefaultHasher, Hash, Hasher};

use glam::DVec3;

use crate::{
    app::{
        planning_pipeline::{StageDiagnostic, StageOutcome},
        step_pipeline::{PipelineStep, RunTurn, StepPipeline},
    },
    i18n::tr,
    model::haulage::network::HaulNetwork,
    ui::state::{HaulageStageView, HaulageStep},
};

/// How long Auto waits after the last edit before rerunning the stale steps.
const HAULAGE_AUTO_SETTLE: std::time::Duration = std::time::Duration::from_millis(500);

impl PipelineStep for HaulageStep {
    const ALL: &'static [Self] = &HaulageStep::ALL;

    fn index(self) -> usize {
        HaulageStep::index(self)
    }

    fn label(self) -> String {
        HaulageStep::label(self)
    }
}

pub(crate) type HaulagePipeline = StepPipeline<HaulageStep>;

impl crate::app::App<'_> {
    /// Refresh the Haulage pipeline against the project as it stands, without
    /// running anything. Called before the Schedule pipeline's refresh, whose
    /// Haulage step reads where this one stands.
    pub(crate) fn sync_haulage_pipeline(&mut self) {
        let Some(runtime) = self.workspace.active_project().map(|project| project.runtime_id) else {
            self.haulage_pipeline = None;
            self.mirror_haulage_stages();
            return;
        };
        if self.haulage_pipeline.as_ref().is_none_or(|pipeline| pipeline.runtime != runtime) {
            self.haulage_pipeline = Some(HaulagePipeline::new(runtime));
            // What Auto tried belongs to the pipeline it tried it on.
            self.haulage_auto_attempted = None;
            self.haulage_auto_settle = None;
        }
        let fingerprints = self.haulage_fingerprints();
        let pipeline = self.haulage_pipeline.as_mut().expect("just ensured");
        if let Some(step) = pipeline.take_fingerprints(&fingerprints) {
            pipeline.invalidate_from(step);
        }
        pipeline.mark_stale();
        self.advance_haulage_run();
        self.mirror_haulage_stages();
    }

    /// Input fingerprints for both steps, the second chained onto the first.
    ///
    /// Road Network reads the classes' steepest allowed grade - a road is too
    /// steep for the trucks that drive it - so a class edit retires it too.
    fn haulage_fingerprints(&self) -> [u64; HaulageStep::ALL.len()] {
        let Some(document) = self.workspace.active_document() else {
            return [0; HaulageStep::ALL.len()];
        };
        let trucks = document.schedule().trucks();
        let mut hasher = DefaultHasher::new();
        document.haulage().hash_content(&mut hasher);
        format!("{:?}", super::commands::haulage::haul_destinations(document)).hash(&mut hasher);
        super::commands::haulage::max_grade(trucks).to_bits().hash(&mut hasher);
        // Where the Solids run stands, not what it produced: dead ends are
        // judged against its dig blocks, and a status moves exactly when they do.
        self.planning_snapshot_status().ok().hash(&mut hasher);
        let network = hasher.finish();

        let mut hasher = DefaultHasher::new();
        network.hash(&mut hasher);
        trucks.hash_classes(&mut hasher);
        [network, hasher.finish()]
    }

    /// Reset the pipeline and run from its first step through the selected
    /// one. Whether the run started.
    pub(crate) fn run_haulage_step(&mut self, step: HaulageStep) -> bool {
        self.sync_haulage_pipeline();
        let Some(pipeline) = self.haulage_pipeline.as_mut() else {
            return false;
        };
        if !pipeline.restart_through(step) {
            return false;
        }
        self.advance_haulage_run();
        self.mirror_haulage_stages();
        true
    }

    pub(crate) fn run_all_haulage_steps(&mut self) {
        self.run_haulage_step(HaulageStep::TruckClasses);
    }

    pub(crate) fn cancel_haulage_run(&mut self) {
        if let Some(pipeline) = self.haulage_pipeline.as_mut() {
            pipeline.cancel();
        }
        self.mirror_haulage_stages();
    }

    /// Rerun the stale steps on their own once edits have settled, while the
    /// Auto switch is on. At most once for one set of inputs, as on Solids.
    pub(crate) fn auto_run_haulage(&mut self) {
        self.haulage_auto_deadline = None;
        let Some(pipeline) = self.haulage_pipeline.as_ref() else {
            self.haulage_auto_settle = None;
            return;
        };
        if !self.editor.haulage_auto_run || pipeline.is_running() {
            self.haulage_auto_settle = None;
            if !self.editor.haulage_auto_run {
                self.haulage_auto_attempted = None;
            }
            return;
        }
        let key = self.haulage_fingerprints();
        if self.haulage_auto_attempted == Some(key) {
            return;
        }
        let now = web_time::Instant::now();
        let since = match self.haulage_auto_settle {
            Some((seen, since)) if seen == key => since,
            _ => {
                self.haulage_auto_settle = Some((key, now));
                now
            }
        };
        if now < since + HAULAGE_AUTO_SETTLE && self.editing_near(since, HAULAGE_AUTO_SETTLE) {
            self.haulage_auto_deadline = Some(since + HAULAGE_AUTO_SETTLE);
            return;
        }
        self.haulage_auto_settle = None;
        self.haulage_auto_attempted = Some(key);
        if self.haulage_pipeline.as_mut().is_some_and(HaulagePipeline::resume_all) {
            self.advance_haulage_run();
            self.mirror_haulage_stages();
        }
    }

    /// Start the next queued step and settle the running one. Every step is
    /// a check over the project as it stands, so a run finishes in one call.
    fn advance_haulage_run(&mut self) {
        for _ in 0..HaulageStep::ALL.len() * 2 + 1 {
            let Some(pipeline) = self.haulage_pipeline.as_mut() else { return };
            match pipeline.next_step() {
                RunTurn::Idle => return,
                RunTurn::Skip => continue,
                RunTurn::Start(step) => pipeline.start(step),
                RunTurn::Evaluate(step) => {
                    let generation = pipeline.generation();
                    let outcome = match step {
                        HaulageStep::Network => self.evaluate_road_network(),
                        HaulageStep::TruckClasses => self.evaluate_truck_classes(),
                    };
                    let Some(pipeline) = self.haulage_pipeline.as_mut() else { return };
                    if !matches!(pipeline.settle(step, outcome, generation), Ok(false)) {
                        return;
                    }
                }
            }
        }
    }

    /// Copy the pipeline's status into the editor state the panels read.
    fn mirror_haulage_stages(&mut self) {
        let (views, running) = match self.haulage_pipeline.as_ref() {
            None => (Default::default(), false),
            Some(pipeline) => (
                HaulageStep::ALL.map(|step| {
                    let status = pipeline.status(step);
                    HaulageStageView {
                        state: status.state,
                        message: status.message.clone(),
                        diagnostics: status.diagnostics.clone(),
                        last_success: status.last_success.clone(),
                        blocked_by: pipeline.blocked_by(step),
                    }
                }),
                pipeline.is_running(),
            ),
        };
        if self.editor.haulage_stages != views || self.editor.haulage_run_active != running {
            self.editor.haulage_stages = views;
            self.editor.haulage_run_active = running;
            self.redraw_requested = true;
        }
    }

    /// The road network: its settings, and what the Layout's issue list
    /// reports. Issues are warnings - a network with a gap still hauls on the
    /// part that joins - and an empty network is fine: every destination then
    /// hauls its fixed distance.
    fn evaluate_road_network(&self) -> StageOutcome {
        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let network = document.haulage();
        let settings = &network.settings;
        let mut diagnostics = Vec::new();
        if [settings.join_tolerance_m, settings.auto_join_m, settings.bench_speed_kph, settings.acceleration_kph_s]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("haul-stage-settings"),
                blocking: true,
            });
        }
        let blocks: Vec<DVec3> = self
            .planning_snapshot()
            .ok()
            .iter()
            .flat_map(|snapshot| &snapshot.blocks)
            .map(|block| DVec3::new(block.anchor[0], block.anchor[1], block.flitch.base))
            .collect();
        let destinations = super::commands::haulage::haul_destinations(document);
        for issue in network.issues(&destinations, super::commands::haulage::max_grade(document.schedule().trucks()), &blocks) {
            let subject = issue
                .road
                .and_then(|id| network.road(id))
                .map(|road| road.name.clone())
                .or_else(|| issue.node.map(|id| node_name(network, id)));
            diagnostics.push(StageDiagnostic {
                entity: subject,
                message: issue.kind.label(),
                blocking: false,
            });
        }
        StageOutcome::Settled {
            diagnostics,
            entities: network.roads.len(),
        }
    }

    /// The truck classes and their fleet calendars. How many trucks a class
    /// has is the Calendar's to say, day by day, so a class with none is not
    /// a finding here.
    ///
    /// Setup reports configuration; horizon-specific capture validates matching
    /// truck rules and coefficients. Empty configuration is not unrestricted haul.
    fn evaluate_truck_classes(&self) -> StageOutcome {
        let Some(document) = self.workspace.active_document() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        let trucks = document.schedule().trucks();
        let mut diagnostics = Vec::new();
        if trucks.classes.is_empty() {
            diagnostics.push(StageDiagnostic {
                entity: None,
                message: tr!("truck-stage-no-classes"),
                blocking: false,
            });
        }
        for class in &trucks.classes {
            if let Err(error) = class.validate_haulage() {
                diagnostics.push(StageDiagnostic {
                    entity: Some(class.name.clone()),
                    message: error.message(),
                    blocking: true,
                });
            }
            if let Err(error) = class.calendar.validate() {
                diagnostics.push(StageDiagnostic {
                    entity: Some(class.name.clone()),
                    message: error.message(),
                    blocking: false,
                });
            }
        }
        StageOutcome::Settled {
            diagnostics,
            entities: trucks.classes.len(),
        }
    }

    /// The Schedule pipeline's Haulage step: complete when this pipeline is,
    /// failed when it failed, and otherwise it runs this pipeline and waits.
    /// Haulage's warnings are carried across so the step says it has them.
    pub(crate) fn evaluate_schedule_haulage(&mut self) -> StageOutcome {
        use crate::app::planning_pipeline::StageState;

        if self.haulage_pipeline.as_ref().is_some_and(|pipeline| !pipeline.is_running() && !pipeline.is_current()) {
            self.run_all_haulage_steps();
        }
        let Some(pipeline) = self.haulage_pipeline.as_ref() else {
            return StageOutcome::Settled {
                diagnostics: Vec::new(),
                entities: 0,
            };
        };
        if pipeline.is_running() {
            return StageOutcome::Working {
                message: Some(tr!("schedule-haulage-waiting")),
            };
        }
        let diagnostics = match HaulageStep::ALL.into_iter().find(|&step| pipeline.status(step).state != StageState::Complete) {
            Some(step) => vec![StageDiagnostic {
                entity: Some(tr!("planning-page-haulage")),
                message: pipeline.status(step).message.clone().unwrap_or_else(|| tr!("stage-blocked-by", stage = step.label())),
                blocking: true,
            }],
            None => HaulageStep::ALL.into_iter().flat_map(|step| pipeline.status(step).diagnostics.clone()).collect(),
        };
        let mut diagnostics = diagnostics;
        let connections = self.haul_connections();
        diagnostics.extend(connection_diagnostics(&connections));
        self.editor.schedule_haul_connections = Some(connections);
        StageOutcome::Settled {
            diagnostics,
            entities: HaulageStep::ALL.len(),
        }
    }
}

/// Each destination side and pit that trucks cannot reach, as a warning:
/// the schedule still hauls from and to everything that can.
fn connection_diagnostics(connections: &crate::app::commands::haulage::HaulConnections) -> Vec<StageDiagnostic> {
    use crate::app::commands::haulage::HaulLink;

    let mut diagnostics = Vec::new();
    if connections.destinations.iter().any(|entry| entry.dump == HaulLink::NoRoads) {
        diagnostics.push(StageDiagnostic {
            entity: None,
            message: tr!("haul-link-no-roads-note"),
            blocking: false,
        });
        return diagnostics;
    }
    for entry in &connections.destinations {
        if entry.dump.is_problem() {
            diagnostics.push(StageDiagnostic {
                entity: Some(entry.name.clone()),
                message: tr!("haul-link-dump-problem", status = entry.dump.label()),
                blocking: false,
            });
        }
        if let Some(reclaim) = entry.reclaim.filter(|link| link.is_problem()) {
            diagnostics.push(StageDiagnostic {
                entity: Some(entry.name.clone()),
                message: tr!("haul-link-reclaim-problem", status = reclaim.label()),
                blocking: false,
            });
        }
    }
    for pit in connections.pits.iter().filter(|pit| pit.reached < pit.total) {
        diagnostics.push(StageDiagnostic {
            entity: Some(pit.name.clone()),
            message: tr!("haul-link-pit-problem", missed = (pit.total - pit.reached).to_string(), total = pit.total.to_string()),
            blocking: false,
        });
    }
    diagnostics
}

fn node_name(network: &HaulNetwork, id: crate::model::haulage::network::NodeId) -> String {
    network
        .roads
        .iter()
        .find(|road| road.from == id || road.to == id)
        .map_or_else(|| tr!("haul-node"), |road| tr!("haul-node-on", road = road.name.clone()))
}
