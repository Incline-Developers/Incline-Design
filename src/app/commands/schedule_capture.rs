//! Building a complete, immutable [`BlendInput`] out of an actual project.
//!
//! This is the join the blended experiment was missing: everything before it
//! was handed a synthetic scenario. Here the project's own validated planning
//! snapshot, authored bars, destinations, trucking and cashflow rules become
//! one owned model, or a list of diagnostics naming what stopped it.
//!
//! # What this reuses, and what it refuses to re-derive
//!
//! Ground and material come from the *same* cached planning snapshot the
//! readiness reports were measured against, through the same identity
//! checks: nothing here runs Solids, walks a scene composite or recomputes a
//! reserve. Routing, trucking and cashflow are resolved by their own authored
//! semantics, not reimplemented: rule order is preference, trucking rules
//! union their classes, and matching cashflow rules add.
//!
//! # Where real project semantics and the experimental model differ
//!
//! Each is a refusal naming what it refuses, never a silent adaptation.
//!
//! 1. **Category conditions on reclaim.** The blended pile keeps tonnes and
//!    contained quantity and discards categories, so a rule that requires a
//!    category *and applies to reclaim* cannot be evaluated. Refused, naming
//!    the rule.
//! 2. **Untracked numerical fields on reclaim.** Upper, lower and two-sided
//!    bounds are supported for tracked grades. A field with no blended grade
//!    has no quantity to test and is refused with its name.
//!
//! Several materials inside one dig block are **not** an approximation. A
//! block whose captured rows disagree stays one ground source with one
//! completion balance and one material share per distinct material. A
//! movement candidate still names one material, but the model constrains the
//! candidates of a block to move in its measured proportions in every
//! execution segment, so the block's remaining composition never changes and
//! a portion nobody can route blocks the extraction rather than being left
//! behind. An earlier revision captured such a block as ordered sub-blocks;
//! that let a loader take one material out first and is why the proportion
//! rows exist.
//!
//! Conditions on *dug* material are not approximated at all: the contributing
//! block-model rows' own values are known at capture - each portion's mapped
//! values before spatial proration, so a rule naming a category and a grade
//! asks whether they occur in the *same* material - and every rule form is
//! supported there.

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use web_time::Instant;

use super::{schedule_readiness::BarReport, solids_view::PlanningSnapshot};
use crate::{
    app::jobs::CancelFlag,
    model::{
        Document, ReserveField, ReserveFieldId,
        schedule::{
            BarId, DestinationId as ProjectDestinationId, DestinationKind as ProjectDestinationKind, LoaderAgentId, PortionValue, RouteContext, RouteSource, SCHEDULE_PERIOD_H,
            SchedulePlan,
            calendar::{CalendarPeriod, RateKind},
            cashflow::{Activity as CashflowActivity, MovementContext},
            destinations::{ConditionTest, DestinationView, FieldCondition},
            experiment::{GradeUnit, StockpileRepresentation},
            inventory::ReclaimOrder as ProjectReclaimOrder,
            optimisation::{
                Activity, CashflowContribution, CashflowRuleId, Destination, DestinationId, DestinationKind, GroundId, GroundSource, HorizonSpec, Interval, IntervalRate, Loader,
                LoaderId, MaterialId, MaterialShare, MovementCandidate, NumericalTolerances, ReclaimOrder, RoutingRuleId, SEGMENT_CEILING, SourceId, StockpileId, Task, TaskId,
                TaskKind, TruckClass, TruckClassId,
                blended::{
                    grade::{GradeBasis, GradeField, GradeTable},
                    input::{BlendInput, BlendPile, ConditionalValue, GradeBound, GradeEndpoint, GradePredicate, GradeQualification},
                },
                build_intervals,
            },
            trucking,
        },
    },
    ui::state::ScheduleStep,
};

/// One reason capture stopped, against the item a planner can act on.
///
/// Grouped: a project with four hundred blocks missing one grade produces one
/// diagnostic saying so, with a sample, rather than four hundred lines.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CaptureDiagnostic {
    /// The stockpile, field, rule, bar or block this is about.
    pub(crate) subject: Option<String>,
    pub(crate) message: String,
    /// How many items produced this same diagnostic. `1` for most.
    pub(crate) occurrences: usize,
    /// The Schedule Setup step that owns the repair, when there is one.
    pub(crate) repair: Option<ScheduleStep>,
}

impl CaptureDiagnostic {
    fn new(subject: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            subject: Some(subject.into()),
            message: message.into(),
            occurrences: 1,
            repair: None,
        }
    }

    pub(crate) fn global(message: impl Into<String>) -> Self {
        Self {
            subject: None,
            message: message.into(),
            occurrences: 1,
            repair: None,
        }
    }

    /// Name the setup step that repairs this.
    pub(crate) fn at(mut self, step: ScheduleStep) -> Self {
        self.repair = Some(step);
        self
    }

    pub(crate) fn describe(&self) -> String {
        let body = match &self.subject {
            Some(subject) => format!("{subject}: {}", self.message),
            None => self.message.clone(),
        };
        if self.occurrences > 1 { format!("{body} (x{})", self.occurrences) } else { body }
    }
}

/// Collects diagnostics, folding repeats of the same message together.
#[derive(Default)]
struct Diagnostics {
    entries: Vec<CaptureDiagnostic>,
}

impl Diagnostics {
    fn push(&mut self, diagnostic: CaptureDiagnostic) {
        // Same message, same kind of subject: one entry with a count and the
        // first subject retained, so the source can still be found.
        if let Some(existing) = self.entries.iter_mut().find(|entry| entry.message == diagnostic.message) {
            existing.occurrences += 1;
            return;
        }
        self.entries.push(diagnostic);
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// What the experimental model resolved each of its dense ids from, so a
/// result can be explained in the project's own terms.
#[derive(Clone, Debug, Default)]
pub(crate) struct CaptureIdentities {
    pub(crate) loaders: Vec<(LoaderId, LoaderAgentId, String)>,
    pub(crate) tasks: Vec<(TaskId, BarId, String)>,
    pub(crate) piles: Vec<(StockpileId, ProjectDestinationId, String)>,
    pub(crate) destinations: Vec<(DestinationId, ProjectDestinationId, String)>,
    pub(crate) trucks: Vec<(TruckClassId, trucking::TruckClassId, String)>,
    /// Each captured ground source: the dig block it came from and how many
    /// distinct captured materials that block holds.
    pub(crate) ground: Vec<(GroundId, String, usize)>,
    /// The Solids-run block each captured ground source is.
    pub(crate) ground_blocks: BTreeMap<GroundId, crate::model::DigBlockId>,
    pub(crate) grades: Vec<(ReserveFieldId, String, GradeUnit)>,
    /// Each scoped dig bar's resolved blocks, in authored order.
    pub(crate) bar_blocks: Vec<(BarId, Vec<crate::model::DigBlockId>)>,
    /// Each scoped reclaim bar's cap.
    pub(crate) reclaim_caps: Vec<(BarId, Option<f64>)>,
    /// Calendar delays (delay lists and rosters) per captured loader, merged:
    /// the hours it was given no rate for them.
    pub(crate) delays: Vec<(LoaderAgentId, f64, f64)>,
    /// Every stockpile's authored opening tonnes, whether or not the run uses
    /// it, so an untouched pile still reports the stock it holds.
    pub(crate) pile_openings: Vec<(ProjectDestinationId, f64)>,
    /// What each chunk of a chunked pile is, by position.
    pub(crate) chunk_labels: BTreeMap<StockpileId, Vec<String>>,
    /// How many of a chunked pile's chunks hold opening lots; the rest
    /// receive.
    pub(crate) opening_chunks: BTreeMap<StockpileId, usize>,
    /// Drill and blast, by position in the input's chain: each blast, and
    /// the machine each chain agent is.
    pub(crate) blasts: Vec<CapturedBlast>,
    pub(crate) blast_agents: Vec<LoaderAgentId>,
}

/// One blast the run sequences, as the results draw it.
#[derive(Clone, Debug)]
pub(crate) struct CapturedBlast {
    pub(crate) reference: crate::model::schedule::BlastRef,
    pub(crate) solid_name: String,
    pub(crate) name: String,
    pub(crate) bench: crate::ui::state::BenchSelection,
    pub(crate) face: Arc<crate::model::arrangement::Face>,
    /// Its holes' collars, in plan, in drilling order.
    pub(crate) collars: Vec<glam::DVec2>,
    /// Square metres to prep, metres to drill and tonnes to charge.
    pub(crate) quantity: [f64; 3],
}

/// Measured facts about the capture itself, for the completion report.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CaptureStats {
    pub(crate) duration: Duration,
    pub(crate) candidates: usize,
    pub(crate) ground_sources: usize,
    /// Whether the derived event budget hit [`SEGMENT_CEILING`].
    pub(crate) event_budget_restricted: bool,
    /// Blasts a dig bar needs that no machine bar will prep, drill or charge,
    /// so their loaders wait all horizon. Named in the notes.
    pub(crate) unworked_blasts: usize,
}

/// A finished capture: the model, what its ids mean, and one semantic key.
pub(crate) struct BlendCapture {
    pub(crate) input: BlendInput,
    pub(crate) identities: CaptureIdentities,
    /// Everything that can change feasibility, the objective or the encoded
    /// model, hashed. Presentation names are deliberately excluded.
    pub(crate) fingerprint: u64,
    pub(crate) stats: CaptureStats,
    /// Stated approximations this capture applied. Never a silent adaptation.
    pub(crate) notes: Vec<String>,
}

/// The owned, immutable configuration one capture reads.
///
/// Taken on the UI thread and then left alone: the expensive half - candidate
/// expansion, material interning, calendar integration - runs on a worker
/// against this and consults no live project state.
pub(crate) struct CaptureSnapshot {
    pub(crate) runtime: u32,
    /// The horizon this run was asked to cover, from project hour zero.
    pub(crate) horizon_h: f64,
    /// Whether the run solves the whole horizon at once (Improve), which the
    /// column ceiling guards; the hourly first schedule never builds that model.
    pub(crate) whole_horizon: bool,
    pub(crate) generation: u64,
    pub(crate) plan_revision: u64,
    plan: SchedulePlan,
    haulage: crate::model::haulage::HaulNetwork,
    haul_points: BTreeMap<ProjectDestinationId, glam::DVec3>,
    fields: Vec<ReserveField>,
    tonnage_field: ReserveFieldId,
    destinations: Vec<DestinationView>,
    snapshot: Arc<PlanningSnapshot>,
    reports: Arc<Vec<BarReport>>,
    /// Each solid's ground taken out of mining, read as the run started.
    exclusions: Vec<(crate::model::SolidId, crate::model::MiningExclusions)>,
}

/// Cap on the estimated column count of a whole-horizon solve (Improve), so a
/// horizon somebody typed three extra zeroes into is refused with a figure
/// rather than allocated.
const COLUMN_CEILING: usize = 4_000_000;

/// A bar in scope for the experimental run: assigned, with work, and with a
/// window that reaches the horizon.
struct ScopedBar {
    bar: BarId,
    name: String,
    agent: LoaderAgentId,
    priority: u32,
    start_h: f64,
    end_h: f64,
    work: ScopedWork,
}

enum ScopedWork {
    Dig {
        members: Vec<(usize, f64)>,
    },
    Reclaim {
        sources: Vec<ProjectDestinationId>,
        maximum_t: Option<f64>,
    },
    /// A delay bar: holds its loader while it has priority.
    Delay,
}

/// Where one captured ground source came from, and what it is made of.
///
/// The portion's *own* mapped values are kept, not the block's average: a
/// rule that names a category and a grade must be able to ask whether they
/// occur in the same material, which is the whole reason the scan retained
/// rows rather than totals.
struct GroundContext {
    material: MaterialId,
    solid: crate::model::SolidId,
    bench: (f64, f64),
    flitch: (f64, f64),
    capture: Arc<crate::model::solid_reserves::MaterialCapture>,
    values: Vec<f64>,
    /// Which of the block's captured portions this is, for diagnostics.
    portion: usize,
    position: glam::DVec3,
    /// The road nodes the block is held to, when it is not left to the nearest road.
    haul_link: Vec<crate::model::haulage::NodeId>,
}

impl GroundContext {
    /// One field's value on this material: the portion's own mapped value,
    /// never the block's average.
    fn value(&self, field: ReserveFieldId) -> Option<PortionValue> {
        let position = self.capture.position(field)?;
        let value = self.values[position];
        Some(if self.capture.is_categorical(position) {
            match self.capture.label(value) {
                Some(label) => PortionValue::Category(label.to_owned()),
                None => PortionValue::Missing,
            }
        } else if value.is_finite() {
            PortionValue::Number(value)
        } else {
            PortionValue::Missing
        })
    }
}

impl crate::app::App<'_> {
    /// Take the owned configuration snapshot one schedule run reads.
    ///
    /// Bounded: it clones the plan and the field list, borrows the cached
    /// planning snapshot and reports through `Arc`, and resolves no candidate.
    pub(crate) fn capture_schedule_snapshot(&mut self, horizon_h: f64, whole_horizon: bool) -> Result<CaptureSnapshot, Vec<CaptureDiagnostic>> {
        let inputs = match self.schedule_run_inputs() {
            Ok(inputs) => inputs,
            Err(reason) => return Err(vec![CaptureDiagnostic::global(reason.describe())]),
        };
        let plan_revision = self.schedule_plan_revision();
        let reports = self.schedule_reports();
        let Some(snapshot) = self.schedule_snapshot() else {
            return Err(vec![CaptureDiagnostic::global(crate::i18n::tr!("planning-snapshot-no-project"))]);
        };
        let Some(project) = self.workspace.active_project() else {
            return Err(vec![CaptureDiagnostic::global(crate::i18n::tr!("planning-snapshot-no-project"))]);
        };
        let document: &Document = &project.project.document;
        let plan = document.schedule().clone();
        let Some(tonnage_field) = plan.tonnage_field() else {
            return Err(vec![CaptureDiagnostic::global(crate::i18n::tr!("schedule-stage-no-tonnage-field"))]);
        };
        let destinations = crate::model::schedule::destinations::available(document.solids(), plan.routing());
        Ok(CaptureSnapshot {
            runtime: project.runtime_id,
            horizon_h,
            whole_horizon,
            generation: inputs.generation,
            plan_revision,
            plan,
            haulage: document.haulage().clone(),
            haul_points: self.haul_destination_points(document),
            fields: document.reserve_fields().to_vec(),
            tonnage_field,
            destinations,
            snapshot,
            reports,
            exclusions: document
                .solids()
                .iter()
                .filter(|solid| !solid.exclusions.is_empty())
                .map(|solid| (solid.id, solid.exclusions.clone()))
                .collect(),
        })
    }
}

/// The drill and blast chain: every blast of the run, how much of each step
/// it needs and what must be dug before it is clear, and the dozers, drills
/// and MPUs working the blast bars.
fn capture_drill_blast(
    source: &CaptureSnapshot,
    horizon_h: f64,
    intervals: &[Interval],
    block_ground: &BTreeMap<usize, GroundId>,
    identities: &mut CaptureIdentities,
    notes: &mut Vec<String>,
    problems: &mut Diagnostics,
) -> Option<crate::model::schedule::optimisation::blended::drill_blast::DrillBlastInput> {
    use crate::model::{
        drill_hole::{DrillPatternLayout, generate_pattern_collars},
        schedule::{
            MachineKind,
            optimisation::blended::drill_blast::{BlastAgent, BlastJob, BlastTask, DrillBlastInput},
        },
    };
    let plan = &source.plan;
    let config = plan.drill_blast();
    let snapshot = &source.snapshot;
    let mut blasts = Vec::with_capacity(snapshot.blasts.len());
    let mut unclearable: Vec<String> = Vec::new();
    for record in &snapshot.blasts {
        let reference = record.reference();
        let pattern = config
            .patterns
            .iter()
            .find(|entry| record.holds(&entry.blast))
            .map_or(config.pattern, |entry| entry.pattern);
        let depth = (record.bench.top - record.bench.base) + pattern.subdrill_m;
        let outer: Vec<glam::DVec3> = record
            .face
            .first()
            .map_or_else(Vec::new, |ring| ring.iter().map(|point| point.extend(record.bench.top)).collect());
        let layout = if pattern.staggered { DrillPatternLayout::Staggered } else { DrillPatternLayout::Square };
        let collars: Vec<glam::DVec2> = generate_pattern_collars(&outer, pattern.burden_m, pattern.spacing_m, 0.0, glam::DVec2::ZERO, layout)
            .map(|collars| {
                collars
                    .into_iter()
                    .map(|collar| collar.truncate())
                    .filter(|collar| crate::model::arrangement::point_in_face(&record.face, *collar))
                    .collect()
            })
            .unwrap_or_else(|error| {
                problems.push(CaptureDiagnostic::new(record.name.clone(), error).at(ScheduleStep::DrillBlast));
                Vec::new()
            });
        let holes = collars.len() as f64;
        let quantity = [record.area, holes * depth, holes * config.charge_per_hole_t(depth)];
        // Configured statuses name a blast by a point inside it.
        let stage = config
            .statuses
            .iter()
            .find(|entry| record.holds(&entry.blast))
            .map_or(crate::model::schedule::BlastStage::NotStarted, |entry| entry.stage);
        let mut releases = Vec::new();
        let mut above = Vec::new();
        let mut blocked = false;
        for (position, block) in snapshot.blocks.iter().enumerate() {
            let in_blast = block.solid == record.solid
                && block.blast.is_some_and(|blast| {
                    (blast.bench_base() - record.bench.base).abs() < 1e-6 && crate::model::arrangement::point_in_face(&record.face, glam::DVec2::from(blast.anchor()))
                });
            if in_blast {
                if let Some(ground) = block_ground.get(&position) {
                    releases.push(*ground);
                }
                continue;
            }
            // Ground in a higher bench, within the buffer in plan.
            if block.flitch.base < record.bench.top - 1e-6 {
                continue;
            }
            if plan_gap(&block.ground, &record.face) > config.buffer_m + 1e-9 {
                continue;
            }
            match block_ground.get(&position) {
                Some(ground) => above.push(*ground),
                // Standing ground no bar digs never goes.
                None => blocked = true,
            }
        }
        if blocked && stage == crate::model::schedule::BlastStage::NotStarted {
            unclearable.push(crate::ui::elements::solids_view::blast_path(&record.solid_name, record.bench.base, &record.name));
        }
        blasts.push(BlastJob {
            quantity,
            stage,
            releases,
            above,
            never_clear: blocked,
        });
        identities.blasts.push(CapturedBlast {
            reference,
            solid_name: record.solid_name.clone(),
            name: record.name.clone(),
            bench: record.bench,
            face: Arc::clone(&record.face),
            collars,
            quantity,
        });
    }
    if !unclearable.is_empty() {
        notes.push(crate::i18n::tr!("drill-blast-unclearable", blasts = unclearable.join(", ")));
    }

    // Machines and their bars.
    let mut agents: Vec<BlastAgent> = Vec::new();
    let mut tasks: Vec<BlastTask> = Vec::new();
    // Follow bars name their leader by id; its index is known only once
    // every bar has been read.
    let mut leaders: Vec<(usize, crate::model::schedule::LoaderAgentId)> = Vec::new();
    for bar in plan.bars() {
        if bar.blast_order().is_none() && bar.delay().is_none() && bar.follow().is_none() {
            continue;
        }
        let members = bar.blast_order().map_or(&[][..], |order| order.members.as_slice());
        let Some(agent_id) = bar.agent else { continue };
        let (Some(agent), Some(kind)) = (plan.agent(agent_id), plan.agent_kind(agent_id)) else {
            continue;
        };
        let Some(activity) = kind.activity() else { continue };
        let start_h = bar.window.start_h;
        let end_h = bar.window.end_h.unwrap_or(horizon_h).min(horizon_h);
        if end_h <= start_h {
            continue;
        }
        let agent_index = match identities.blast_agents.iter().position(|id| *id == agent_id) {
            Some(index) => index,
            None => {
                let Some(class) = plan.class(agent.class_id) else { continue };
                let calendar = match agent.calendar.compile_rate(RateKind::Dig, class.default_dig_rate_tph) {
                    Ok(calendar) => calendar,
                    Err(error) => {
                        problems.push(CaptureDiagnostic::new(agent.name.clone(), error.message()));
                        continue;
                    }
                };
                // Calendar delays and the machine's own delay bars stand it.
                let stood = plan.delays().merged_for(agent_id, horizon_h);
                let rates = intervals
                    .iter()
                    .map(|interval| {
                        if stood.iter().any(|(start, end)| *start <= interval.start_h + 1e-9 && interval.start_h < *end - 1e-9) {
                            0.0
                        } else {
                            MachineKind::hourly(kind, calendar.rate_at(interval.start_h))
                        }
                    })
                    .collect();
                identities.blast_agents.push(agent_id);
                agents.push(BlastAgent { activity, rates });
                agents.len() - 1
            }
        };
        let mut sequence = Vec::new();
        let mut missing = 0usize;
        for member in members {
            match snapshot.blasts.iter().position(|record| record.holds(member)) {
                Some(index) if !sequence.contains(&index) => sequence.push(index),
                Some(_) => {}
                None => missing += 1,
            }
        }
        if missing > 0 {
            notes.push(crate::i18n::tr!("drill-blast-missing", bar = bar.name().to_owned(), count = missing.to_string()));
        }
        if let Some(leader) = bar.follow().and_then(|work| work.leader) {
            leaders.push((tasks.len(), leader));
        }
        tasks.push(BlastTask {
            agent: agent_index,
            priority: bar.priority,
            start_h,
            end_h,
            sequence,
            delay: bar.delay().is_some(),
            follow: None,
        });
    }
    // A leader with no blast bars of its own has nothing to follow, and the
    // bar idles.
    for (task, leader) in leaders {
        tasks[task].follow = identities.blast_agents.iter().position(|id| *id == leader);
    }
    Some(DrillBlastInput {
        blasts,
        agents,
        tasks,
        window_end_h: config.window_end_h,
        windows: config.windows.clone(),
        fixed: None,
    })
}

/// The gap between two pieces of ground in plan: zero where they touch or
/// overlap, otherwise the shortest distance between their outlines.
fn plan_gap(a: &crate::model::arrangement::Face, b: &crate::model::arrangement::Face) -> f64 {
    use crate::model::arrangement::point_in_face;
    let (Some(outer_a), Some(outer_b)) = (a.first(), b.first()) else {
        return f64::INFINITY;
    };
    if outer_a.iter().any(|point| point_in_face(b, *point)) || outer_b.iter().any(|point| point_in_face(a, *point)) {
        return 0.0;
    }
    let edges = |ring: &Vec<glam::DVec2>| (0..ring.len()).map(|i| (ring[i], ring[(i + 1) % ring.len()])).collect::<Vec<_>>();
    let point_segment = |p: glam::DVec2, (s, e): (glam::DVec2, glam::DVec2)| {
        let d = e - s;
        let t = if d.length_squared() > 0.0 {
            ((p - s).dot(d) / d.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (s + d * t).distance(p)
    };
    let crosses = |(a0, a1): (glam::DVec2, glam::DVec2), (b0, b1): (glam::DVec2, glam::DVec2)| {
        let side = |p: glam::DVec2, q: glam::DVec2, r: glam::DVec2| (q - p).perp_dot(r - p);
        side(a0, a1, b0) * side(a0, a1, b1) < 0.0 && side(b0, b1, a0) * side(b0, b1, a1) < 0.0
    };
    let mut gap = f64::INFINITY;
    for edge_a in a.iter().flat_map(edges) {
        for edge_b in b.iter().flat_map(edges) {
            if crosses(edge_a, edge_b) {
                return 0.0;
            }
            gap = gap
                .min(point_segment(edge_a.0, edge_b))
                .min(point_segment(edge_a.1, edge_b))
                .min(point_segment(edge_b.0, edge_a))
                .min(point_segment(edge_b.1, edge_a));
        }
    }
    gap
}

/// Whether a dig block is ground the planner has taken out of mining.
fn excluded(block: &crate::app::commands::solids_view::DigBlockRecord, exclusions: &[(crate::model::SolidId, crate::model::MiningExclusions)]) -> bool {
    exclusions.iter().any(|(solid, exclusions)| {
        *solid == block.solid
            && exclusions.excludes(
                block.bench.base,
                block.blast.map(|blast| (blast.bench_base(), blast.anchor(), block.blast_area.unwrap_or(f64::INFINITY))),
                block.flitch.base,
                &block.ground,
            )
    })
}

/// Resolve one owned snapshot into the experimental model, or every reason it
/// cannot be.
///
/// Runs on a worker. Cancellation is polled in each substantial loop.
pub(crate) fn build(source: &CaptureSnapshot, cancel: &CancelFlag) -> Result<BlendCapture, Vec<CaptureDiagnostic>> {
    let started = Instant::now();
    let mut problems = Diagnostics::default();
    let mut notes: Vec<String> = Vec::new();
    let plan = &source.plan;
    let experiment = plan.experiment();
    let routing = plan.routing();

    // ---- horizon -----------------------------------------------------------
    // Requested by the run, never inferred from where the bars end: an
    // open-ended bar has no end, and the planning end day bounds it.
    let horizon_h = source.horizon_h;
    if !horizon_h.is_finite() || horizon_h <= 0.0 {
        return Err(vec![
            CaptureDiagnostic::global(crate::i18n::tr!("schedule-capture-horizon")).at(ScheduleStep::Configuration),
        ]);
    }
    if !experiment.interval_h.is_finite() || experiment.interval_h <= 0.0 {
        return Err(vec![
            CaptureDiagnostic::global(crate::i18n::tr!("schedule-capture-interval")).at(ScheduleStep::Configuration),
        ]);
    }
    // The optimiser accounts for every tonne by destination. An older project
    // that switched routing off is told so and pointed at the switch; it is
    // never switched on for it, and there is no dig-only fallback.
    if !routing.enabled {
        return Err(vec![
            CaptureDiagnostic::global(crate::i18n::tr!("schedule-capture-routing-off")).at(ScheduleStep::Configuration),
        ]);
    }
    // Every report must come from the one Solids run the Setup gate named: a
    // model assembled from two runs would describe ground that was never all
    // there at once.
    if source.reports.len() != plan.bars().len()
        || source
            .reports
            .iter()
            .any(|report| report.generation.is_some_and(|generation| generation != source.generation))
    {
        return Err(vec![
            CaptureDiagnostic::global(crate::i18n::tr!("schedule-dispatch-generation-changed")).at(ScheduleStep::Readiness),
        ]);
    }

    // ---- bars in scope -----------------------------------------------------
    let mut scoped: Vec<ScopedBar> = Vec::new();
    for (bar, report) in plan.bars().iter().zip(source.reports.iter()) {
        if cancel.is_cancelled() {
            return Err(Vec::new());
        }
        let Some(agent) = bar.agent else {
            // An unassigned delay holds no machine, so it is not a fault.
            if bar.is_reclaim() || !bar.members().is_empty() {
                problems.push(CaptureDiagnostic::new(bar.name().to_owned(), "this bar is not assigned to a loader"));
            }
            continue;
        };
        // Dozers, drills and MPUs are not loaders: their bars, delays
        // included, are drill and blast's (see `capture_drill_blast`).
        if plan.agent_kind(agent).is_some_and(crate::model::schedule::MachineKind::is_drill_blast) {
            continue;
        }
        // Open-ended bars are truncated by the explicit horizon; finite ones
        // are intersected with it. A bar entirely beyond the horizon is not a
        // fault - it simply has no work in this run.
        let start_h = bar.window.start_h;
        let end_h = bar.window.end_h.unwrap_or(horizon_h).min(horizon_h);
        if !(start_h.is_finite() && end_h.is_finite()) || end_h <= start_h {
            continue;
        }
        let work = if bar.delay().is_some() {
            ScopedWork::Delay
        } else if let Some(reclaim) = bar.reclaim() {
            ScopedWork::Reclaim {
                sources: reclaim.sources.clone(),
                maximum_t: reclaim.maximum_t,
            }
        } else {
            if bar.members().is_empty() {
                continue;
            }
            if !report.is_ready() {
                for problem in &report.problems {
                    problems.push(CaptureDiagnostic::new(bar.name().to_owned(), problem.message()));
                }
                continue;
            }
            let mut members = Vec::new();
            let mut skipped = 0usize;
            for member in &report.members {
                let (Some(resolved), Some(tonnes)) = (member.resolved, member.tonnes) else {
                    problems.push(CaptureDiagnostic::new(bar.name().to_owned(), "a dig block in this bar has no measured tonnage"));
                    continue;
                };
                let Some(position) = source.snapshot.blocks.iter().position(|block| block.id == resolved) else {
                    problems.push(CaptureDiagnostic::new(
                        bar.name().to_owned(),
                        "a dig block in this bar is not in the planning run this capture reads",
                    ));
                    continue;
                };
                // Excluded ground is never dug: it leaves the sequence, and
                // the bar works on to the next block.
                if excluded(&source.snapshot.blocks[position], &source.exclusions) {
                    skipped += 1;
                    continue;
                }
                members.push((position, tonnes));
            }
            if skipped > 0 {
                notes.push(crate::i18n::tr!("schedule-capture-excluded", bar = bar.name().to_owned(), count = skipped.to_string()));
            }
            ScopedWork::Dig { members }
        };
        scoped.push(ScopedBar {
            bar: bar.id,
            name: bar.name().to_owned(),
            agent,
            priority: bar.priority,
            start_h,
            end_h,
            work,
        });
    }
    let has_blast_work = plan.drill_blast().enabled
        && plan
            .bars()
            .iter()
            .any(|bar| bar.agent.is_some() && bar.blast_order().is_some_and(|order| !order.members.is_empty()));
    if scoped.iter().all(|bar| matches!(bar.work, ScopedWork::Delay)) && !has_blast_work && problems.is_empty() {
        problems.push(CaptureDiagnostic::global(crate::i18n::tr!("schedule-capture-no-work")));
    }

    // ---- grades ------------------------------------------------------------
    // Only the fields the blend actually needs: an unmapped field nothing
    // reads must not block a run.
    let mut grade_fields = Vec::new();
    for (field, _) in &experiment.grades {
        match GradeField::accept(*field, GradeBasis::Stored, &source.fields, Some(source.tonnage_field)) {
            Ok(accepted) => grade_fields.push(accepted),
            Err(rejection) => problems.push(CaptureDiagnostic::new(field_name(&source.fields, *field), rejection.message()).at(ScheduleStep::Configuration)),
        }
    }

    // ---- destinations ------------------------------------------------------
    let mut destination_ids: BTreeMap<ProjectDestinationId, DestinationId> = BTreeMap::new();
    let mut destinations: Vec<Destination> = Vec::new();
    let mut pile_ids: BTreeMap<ProjectDestinationId, StockpileId> = BTreeMap::new();
    let mut identities = CaptureIdentities::default();
    let days = (horizon_h / SCHEDULE_PERIOD_H).ceil().max(1.0) as usize;
    for view in &source.destinations {
        let dense = DestinationId(destinations.len() as u32);
        destination_ids.insert(view.id, dense);
        identities.destinations.push((dense, view.id, view.name.clone()));
        let kind = match view.kind {
            ProjectDestinationKind::Crusher => DestinationKind::Crusher,
            ProjectDestinationKind::Dump => DestinationKind::Dump,
            ProjectDestinationKind::Stockpile => {
                let pile = StockpileId(pile_ids.len() as u32);
                pile_ids.insert(view.id, pile);
                identities.pile_openings.push((view.id, view.opening_t));
                identities.piles.push((pile, view.id, view.name.clone()));
                DestinationKind::Stockpile(pile)
            }
        };
        let crusher_daily_t = match (view.kind, view.id) {
            (ProjectDestinationKind::Crusher, ProjectDestinationId::Standalone(id)) => {
                let calendar = routing.crusher(id);
                (0..days).map(|day| calendar.and_then(|calendar| calendar.limit_at(CalendarPeriod(day as u32)))).collect()
            }
            _ => Vec::new(),
        };
        destinations.push(Destination {
            id: dense,
            kind,
            capacity_t: view.capacity_t,
            crusher_daily_t,
        });
    }

    // A rule restricted to ground the Solids run cannot place no longer
    // restricts what it was written to restrict: a configuration error, not a
    // rule that quietly widened.
    for rule in routing.rules.iter().filter(|rule| rule.enabled) {
        if let crate::model::schedule::MovementSourceSelection::Only(scopes) = &rule.sources
            && scopes
                .iter()
                .filter_map(|scope| scope.ground())
                .any(|scope| !scope_is_placeable(scope, &source.snapshot.blocks))
        {
            problems.push(CaptureDiagnostic::global(crate::i18n::tr!("routing-problem-scope-unplaced", rule = rule.name.clone())).at(ScheduleStep::Destinations));
        }
    }

    // ---- which stockpiles this run actually uses ---------------------------
    // A pile nobody reclaims from and nothing can be delivered to does not
    // need a representation, and demanding one would block a run over a pile
    // the schedule never touches.
    let mut used_piles: Vec<ProjectDestinationId> = Vec::new();
    for bar in &scoped {
        if let ScopedWork::Reclaim { sources, .. } = &bar.work {
            for pile in sources {
                if !used_piles.contains(pile) {
                    used_piles.push(*pile);
                }
            }
        }
    }
    for rule in routing.rules.iter().filter(|rule| rule.enabled) {
        for destination in &rule.destinations {
            if pile_ids.contains_key(destination) && !used_piles.contains(destination) {
                used_piles.push(*destination);
            }
        }
    }

    // ---- piles -------------------------------------------------------------
    let grades = grade_fields.len();
    let mut piles: Vec<BlendPile> = Vec::new();
    for project_id in &used_piles {
        if cancel.is_cancelled() {
            return Err(Vec::new());
        }
        let Some(view) = source.destinations.iter().find(|view| view.id == *project_id) else {
            continue;
        };
        let Some(&pile) = pile_ids.get(project_id) else { continue };
        let representation = experiment.representation(*project_id);
        if representation == StockpileRepresentation::NotConfigured {
            problems.push(CaptureDiagnostic::new(view.name.clone(), crate::i18n::tr!("schedule-capture-representation")).at(ScheduleStep::Stockpiles));
            continue;
        }
        let Some(capacity_t) = view.capacity_t else {
            problems.push(
                CaptureDiagnostic::new(view.name.clone(), "the blended model needs a finite pile capacity, because occupancy is a constraint in it").at(ScheduleStep::Stockpiles),
            );
            continue;
        };
        // Opening lots, read exactly as authored. A missing grade is missing,
        // never zero.
        let inventory = routing.inventory(*project_id);
        let mut lots: Vec<(f64, Vec<f64>)> = Vec::new();
        let mut lot_names: Vec<String> = Vec::new();
        for lot in inventory.map(|inventory| inventory.lots.as_slice()).unwrap_or_default() {
            let mut tonnes = 0.0;
            let mut contained = vec![0.0; grades];
            let mut sound = true;
            for portion in &lot.portions {
                for (position, grade) in grade_fields.iter().enumerate() {
                    let value = match portion.value(grade.field).map(|value| value.portion_value()) {
                        Some(PortionValue::Number(value)) => value,
                        _ => {
                            problems.push(
                                CaptureDiagnostic::new(
                                    format!("{} · {}", view.name.clone(), lot.name.clone()),
                                    format!("'{}' is missing on an opening portion; a missing grade cannot be read as zero", grade.name),
                                )
                                .at(ScheduleStep::Stockpiles),
                            );
                            sound = false;
                            continue;
                        }
                    };
                    let fraction = grade.basis.to_fraction(value);
                    if !fraction.is_finite() || fraction < 0.0 {
                        problems.push(
                            CaptureDiagnostic::new(
                                format!("{} · {}", view.name.clone(), lot.name.clone()),
                                format!("'{}' is {value}, which must be a finite, non-negative numeric grade", grade.name),
                            )
                            .at(ScheduleStep::Stockpiles),
                        );
                        sound = false;
                        continue;
                    }
                    contained[position] += portion.tonnes_t * fraction;
                }
                tonnes += portion.tonnes_t;
            }
            if sound {
                lots.push((tonnes, contained));
                lot_names.push(lot.name.clone());
            }
        }
        let total_opening: f64 = lots.iter().map(|(tonnes, _)| tonnes).sum();
        if total_opening > capacity_t + 1e-6 {
            problems.push(
                CaptureDiagnostic::new(
                    view.name.clone(),
                    format!("opening stock of {total_opening:.2} t exceeds the pile capacity of {capacity_t:.2} t"),
                )
                .at(ScheduleStep::Stockpiles),
            );
            continue;
        }
        let mut blend = BlendPile {
            id: pile,
            capacity_t,
            opening_t: total_opening,
            opening_q: (0..grades).map(|position| lots.iter().map(|(_, contained)| contained[position]).sum()).collect(),
            chunks: Vec::new(),
            order: match view.reclaim_order {
                ProjectReclaimOrder::Fifo => ReclaimOrder::Fifo,
                ProjectReclaimOrder::Lifo => ReclaimOrder::Lifo,
            },
            chunk_opening: Vec::new(),
            chunk_closed: Vec::new(),
            modes: {
                let operation = plan.stockpile_operation(*project_id);
                let mut modes: Vec<_> = (0..days).map(|day| operation.mode_at(CalendarPeriod(day as u32))).collect();
                while modes.last().is_some_and(|mode| *mode == Default::default()) {
                    modes.pop();
                }
                modes
            },
            exclusive: !plan.stockpile_operation(*project_id).simultaneous,
            rest_h: plan.stockpile_operation(*project_id).rest_h,
            last_receipt_h: None,
            chunk_closed_h: Vec::new(),
        };
        match representation {
            StockpileRepresentation::NotConfigured => unreachable!("refused above"),
            StockpileRepresentation::Blended => {
                // One blend: the authored lots are combined, explicitly, and
                // FIFO/LIFO stops applying rather than being reinterpreted.
                if lots.len() > 1 {
                    notes.push(format!("{}: {} opening lots combined into one blend; reclaim order does not apply", view.name, lots.len()));
                }
            }
            StockpileRepresentation::Chunks => {
                let receiving = experiment.stockpile(*project_id).map(|entry| entry.receiving_chunks.clone()).unwrap_or_default();
                // Each authored opening lot becomes a closed, immediately
                // reclaimable chunk holding its own actual composition; the
                // configured receiving chunks fill in order behind them.
                let mut capacities: Vec<f64> = lots.iter().map(|(tonnes, _)| tonnes.max(f64::MIN_POSITIVE)).collect();
                let mut openings: Vec<(f64, Vec<f64>)> = lots.clone();
                capacities.extend(receiving.iter().copied());
                openings.extend(receiving.iter().map(|_| (0.0, vec![0.0; grades])));
                if capacities.is_empty() {
                    problems.push(CaptureDiagnostic::new(
                        view.name.clone(),
                        "ordered blended chunks need at least one opening lot or one receiving chunk",
                    ));
                    continue;
                }
                if receiving.is_empty() {
                    notes.push(format!("{}: no receiving chunks are configured, so this pile can only be drawn down", view.name));
                }
                let mut labels: Vec<String> = lot_names.iter().map(|name| crate::i18n::tr!("schedule-chunk-opening", lot = name.clone())).collect();
                labels.extend((1..=receiving.len()).map(|number| crate::i18n::tr!("schedule-chunk-receiving", number = number.to_string())));
                identities.chunk_labels.insert(pile, labels);
                identities.opening_chunks.insert(pile, lots.len());
                blend.chunks = capacities;
                blend.chunk_opening = openings;
            }
        }
        piles.push(blend);
    }

    // ---- materials and ground ---------------------------------------------
    // One ground source per *physical* dig block, carrying one material share
    // per distinct captured material. A block is one volume with one
    // completion balance; its portions are fixed proportions of it, not
    // separate blocks the loader may choose between.
    let mut ground: Vec<GroundSource> = Vec::new();
    let mut captured: BTreeMap<MaterialId, Vec<Option<f64>>> = BTreeMap::new();
    let mut material_rows: BTreeMap<MaterialId, Vec<f64>> = BTreeMap::new();
    let mut block_ground: BTreeMap<usize, GroundId> = BTreeMap::new();
    let mut context: BTreeMap<(GroundId, MaterialId), GroundContext> = BTreeMap::new();
    let mut mixed_blocks = 0;
    let mut wanted_blocks: Vec<usize> = Vec::new();
    let mut measured: BTreeMap<usize, f64> = BTreeMap::new();
    for bar in &scoped {
        if let ScopedWork::Dig { members } = &bar.work {
            for (position, tonnes) in members {
                if !wanted_blocks.contains(position) {
                    wanted_blocks.push(*position);
                    measured.insert(*position, *tonnes);
                }
            }
        }
    }
    for &position in &wanted_blocks {
        if cancel.is_cancelled() {
            return Err(Vec::new());
        }
        let block = &source.snapshot.blocks[position];
        let Some(held) = block.portions.as_ref() else {
            problems.push(CaptureDiagnostic::new(
                block.name.clone(),
                "nothing was captured about this block's material, so its blend cannot be computed",
            ));
            continue;
        };
        let capture = &held.capture;
        let Some(tonnage_position) = capture.position(source.tonnage_field) else {
            problems.push(CaptureDiagnostic::new(
                block.name.clone(),
                "the nominated tonnes field is not among this block's captured values",
            ));
            continue;
        };
        let id = GroundId(ground.len() as u32);
        let mut shares: Vec<MaterialShare> = Vec::new();
        let mut block_tonnes = 0.0;
        let mut sound = true;
        for (index, portion) in held.portions().iter().enumerate() {
            let value = portion.values[tonnage_position];
            if !value.is_finite() {
                continue;
            }
            let tonnes = value * portion.fraction;
            if !tonnes.is_finite() || tonnes <= 0.0 {
                continue;
            }
            let mut row = Vec::with_capacity(grades);
            for grade in &grade_fields {
                let Some(slot) = capture.position(grade.field) else {
                    problems.push(CaptureDiagnostic::new(
                        block.name.clone(),
                        format!("'{}' was not captured for this block, and a blend cannot be computed without it", grade.name),
                    ));
                    sound = false;
                    break;
                };
                let raw = portion.values[slot];
                if capture.is_categorical(slot) || !raw.is_finite() {
                    problems.push(CaptureDiagnostic::new(
                        block.name.clone(),
                        format!("'{}' is missing on some of this block's material; a missing grade cannot be read as zero", grade.name),
                    ));
                    sound = false;
                    break;
                }
                // The *captured* value, in the field's own unit. The grade
                // table retains the stored scale; no unit conversion is needed.
                if raw < 0.0 {
                    problems.push(CaptureDiagnostic::new(
                        block.name.clone(),
                        format!("'{}' is {raw}, which must be a finite, non-negative numeric grade", grade.name),
                    ));
                    sound = false;
                    break;
                }
                row.push(raw);
            }
            if !sound {
                break;
            }
            // Equivalent material records are interned, but never across
            // distinct source identities: two blocks of identical grade stay
            // two blocks, because their ground, their bars and their rules
            // differ.
            let material = MaterialId(material_rows.len() as u32 + 1);
            captured.insert(material, row.iter().map(|value| Some(*value)).collect());
            material_rows.insert(material, row);
            // `fraction` is filled in once the block total is known, because
            // a share is a proportion of the whole block.
            shares.push(MaterialShare { material, fraction: tonnes });
            block_tonnes += tonnes;
            context.insert(
                (id, material),
                GroundContext {
                    material,
                    solid: block.solid,
                    bench: (block.bench.base, block.bench.top),
                    flitch: (block.flitch.base, block.flitch.top),
                    capture: Arc::clone(capture),
                    values: portion.values.clone(),
                    portion: index,
                    position: glam::DVec3::new(block.anchor[0], block.anchor[1], block.flitch.base),
                    haul_link: source.haulage.block_link(block.solid, block.flitch.base, &block.ground).to_vec(),
                },
            );
        }
        if !sound {
            continue;
        }
        if shares.is_empty() || block_tonnes <= 0.0 {
            problems.push(CaptureDiagnostic::new(block.name.clone(), "this block measured no tonnes of the nominated field"));
            continue;
        }
        // The portions come from the same scan as the block's measured total,
        // so a disagreement beyond rounding means they are not describing the
        // same measurement - and the model would dig tonnes nobody measured.
        if let Some(&total) = measured.get(&position)
            && (block_tonnes - total).abs() > 1e-6 + 1e-6 * total.abs()
        {
            problems.push(
                CaptureDiagnostic::global(crate::i18n::tr!(
                    "routing-problem-unreconciled",
                    block = block.name.clone(),
                    portions = format!("{block_tonnes:.3}"),
                    total = format!("{total:.3}")
                ))
                .at(ScheduleStep::Readiness),
            );
            continue;
        }
        for share in &mut shares {
            share.fraction /= block_tonnes;
        }
        if shares.len() > 1 {
            mixed_blocks += 1;
        }
        identities.ground.push((id, block.name.clone(), shares.len()));
        ground.push(GroundSource {
            id,
            tonnes_t: block_tonnes,
            material: shares,
        });
        block_ground.insert(position, id);
        identities.ground_blocks.insert(id, block.id);
    }
    if mixed_blocks > 0 {
        notes.push(format!(
            "{mixed_blocks} dig block(s) hold more than one captured material; each is one block dug in its measured proportions, so every segment removes every portion together"
        ));
    }
    let grade_table = match GradeTable::build(grade_fields.clone(), &captured) {
        Ok(table) => table,
        Err(rejection) => {
            problems.push(CaptureDiagnostic::global(rejection.message()));
            GradeTable::build(Vec::new(), &BTreeMap::new()).expect("an empty grade table is always buildable")
        }
    };
    for grade in &grade_fields {
        let unit = GradeUnit::Stored;
        identities.grades.push((grade.field, grade.name.clone(), unit));
    }

    // ---- loaders and their calendars ---------------------------------------
    let mut loader_ids: BTreeMap<LoaderAgentId, LoaderId> = BTreeMap::new();
    let mut rate_changes: Vec<f64> = Vec::new();
    let mut compiled: Vec<(
        LoaderId,
        LoaderAgentId,
        crate::model::schedule::calendar::CompiledRateCalendar,
        crate::model::schedule::calendar::CompiledRateCalendar,
    )> = Vec::new();
    for bar in &scoped {
        if loader_ids.contains_key(&bar.agent) {
            continue;
        }
        let Some(agent) = plan.agent(bar.agent) else {
            problems.push(CaptureDiagnostic::new(bar.name.clone(), "this bar's loader is no longer in the project").at(ScheduleStep::LoaderAgents));
            continue;
        };
        let Some(class) = plan.class(agent.class_id) else {
            problems.push(CaptureDiagnostic::new(agent.name.clone(), "this loader's class is no longer in the project").at(ScheduleStep::LoaderAgents));
            continue;
        };
        let dig = match agent.calendar.compile_rate(RateKind::Dig, class.default_dig_rate_tph) {
            Ok(calendar) => calendar,
            Err(error) => {
                problems.push(CaptureDiagnostic::new(agent.name.clone(), error.message()));
                continue;
            }
        };
        let reclaim = match agent.calendar.compile_rate(RateKind::Reclaim, class.default_reclaim_rate_tph) {
            Ok(calendar) => calendar,
            Err(error) => {
                problems.push(CaptureDiagnostic::new(agent.name.clone(), error.message()));
                continue;
            }
        };
        let id = LoaderId(loader_ids.len() as u32);
        loader_ids.insert(bar.agent, id);
        identities.loaders.push((id, bar.agent, agent.name.clone()));
        // Delay lists and rosters: the machine has no rate for those hours.
        // Their edges are interval boundaries, so an interval is either
        // wholly delayed or not at all.
        let delayed = plan.delays().merged_for(bar.agent, horizon_h);
        for &(start, end) in &delayed {
            rate_changes.extend([start, end].into_iter().filter(|at| *at > 0.0 && *at < horizon_h));
            identities.delays.push((bar.agent, start, end));
        }
        for calendar in [&dig, &reclaim] {
            let mut at = 0.0_f64;
            while let Some(next) = calendar.next_change_after(at) {
                if next >= horizon_h {
                    break;
                }
                rate_changes.push(next);
                at = next;
            }
        }
        compiled.push((id, bar.agent, dig, reclaim));
    }

    // ---- calendar intervals ------------------------------------------------
    // Regular step, day boundaries, exact authored window edges and every
    // relevant input-setting change, which is what `build_intervals` does.
    let mut windows: Vec<(f64, f64)> = scoped.iter().map(|bar| (bar.start_h, bar.end_h)).collect();
    if plan.drill_blast().enabled {
        // Drill and blast shares this grid: preserve its authored windows,
        // machine calendar changes, delays and exact firing-window ends.
        for bar in plan.bars().iter().filter(|bar| {
            bar.agent
                .is_some_and(|agent| plan.agent_kind(agent).is_some_and(crate::model::schedule::MachineKind::is_drill_blast))
        }) {
            windows.push((bar.window.start_h, bar.window.end_h.unwrap_or(horizon_h).min(horizon_h)));
        }
        for agent in plan
            .agents()
            .iter()
            .filter(|agent| plan.agent_kind(agent.id).is_some_and(crate::model::schedule::MachineKind::is_drill_blast))
        {
            let Some(class) = plan.class(agent.class_id) else { continue };
            match agent.calendar.compile_rate(RateKind::Dig, class.default_dig_rate_tph) {
                Ok(calendar) => {
                    let mut at = 0.0;
                    while let Some(next) = calendar.next_change_after(at) {
                        if next >= horizon_h {
                            break;
                        }
                        rate_changes.push(next);
                        at = next;
                    }
                }
                Err(error) => problems.push(CaptureDiagnostic::new(agent.name.clone(), error.message())),
            }
            for (start, end) in plan.delays().merged_for(agent.id, horizon_h) {
                rate_changes.extend([start, end]);
            }
        }
        for (_, start, end) in plan.drill_blast().window_spans(0.0, horizon_h) {
            rate_changes.extend([start, end].into_iter().filter(|at| *at < horizon_h));
        }
    }
    windows.retain(|(start, end)| end > start);
    for class in &plan.trucks().classes {
        let mut period = CalendarPeriod(0);
        while let Some(next) = class.calendar.next_change_after(period) {
            let at = f64::from(next.0) * SCHEDULE_PERIOD_H;
            if at >= horizon_h {
                break;
            }
            rate_changes.push(at);
            period = next;
        }
    }
    for entry in &routing.standalone {
        let mut period = CalendarPeriod(0);
        while let Some(next) = entry.crusher.next_change_after(period) {
            let at = f64::from(next.0) * SCHEDULE_PERIOD_H;
            if at >= horizon_h {
                break;
            }
            rate_changes.push(at);
            period = next;
        }
    }
    let spec = HorizonSpec {
        end_h: horizon_h,
        regular_step_h: if plan.drill_blast().enabled {
            experiment.interval_h.min(1.0)
        } else {
            experiment.interval_h
        },
        // Not read by the blended model, which has no parcels; a positive
        // value is required only so the shared interval builder validates.
        parcel_target_t: 1.0,
        event_segments: None,
        tolerances: NumericalTolerances::default(),
    };
    let intervals = match build_intervals(spec, &windows, &rate_changes) {
        Ok(intervals) => intervals,
        Err(error) => {
            problems.push(CaptureDiagnostic::global(format!("the schedule calendar could not be built: {error:?}")).at(ScheduleStep::Configuration));
            Vec::new()
        }
    };

    // ---- per-interval rates and truck hours --------------------------------
    let loaders: Vec<Loader> = compiled
        .iter()
        .map(|(id, agent, dig, reclaim)| {
            let delayed: Vec<(f64, f64)> = identities
                .delays
                .iter()
                .filter(|(owner, _, _)| owner == agent)
                .map(|(_, start, end)| (*start, *end))
                .collect();
            Loader {
                id: *id,
                rates: intervals
                    .iter()
                    .map(|interval| {
                        // The rate at the interval's own start: a calendar
                        // change or delay edge inside an interval cannot
                        // happen, because every one is an interval boundary
                        // above.
                        let stood = delayed.iter().any(|(start, end)| *start <= interval.start_h + 1e-9 && interval.start_h < *end - 1e-9);
                        IntervalRate {
                            interval: interval.index,
                            dig_tph: if stood { 0.0 } else { dig.rate_at(interval.start_h) },
                            reclaim_tph: if stood { 0.0 } else { reclaim.rate_at(interval.start_h) },
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    let mut trucks: Vec<TruckClass> = Vec::new();
    let mut truck_ids: BTreeMap<trucking::TruckClassId, TruckClassId> = BTreeMap::new();
    for class in &plan.trucks().classes {
        let dense = TruckClassId(trucks.len() as u32);
        truck_ids.insert(class.id, dense);
        identities.trucks.push((dense, class.id, class.name.clone()));
        let mut hours = Vec::with_capacity(intervals.len());
        for interval in &intervals {
            match trucking::available_truck_hours(&class.calendar, interval.start_h, interval.end_h) {
                Ok(value) => hours.push(value),
                Err(error) => {
                    problems.push(CaptureDiagnostic::new(class.name.clone(), error.message()).at(ScheduleStep::Haulage));
                    hours.push(0.0);
                }
            }
        }
        trucks.push(TruckClass { id: dense, hours });
    }

    // ---- tasks -------------------------------------------------------------
    let mut tasks: Vec<Task> = Vec::new();
    for bar in &scoped {
        let Some(&loader) = loader_ids.get(&bar.agent) else { continue };
        let id = TaskId(tasks.len() as u32);
        let kind = match &bar.work {
            ScopedWork::Dig { members } => {
                // The authored dig order, one entry per physical block.
                let mut sequence = Vec::new();
                for (position, _) in members {
                    sequence.extend(block_ground.get(position).copied());
                }
                if sequence.is_empty() {
                    continue;
                }
                identities
                    .bar_blocks
                    .push((bar.bar, members.iter().map(|(position, _)| source.snapshot.blocks[*position].id).collect()));
                TaskKind::Dig { sequence }
            }
            ScopedWork::Reclaim { sources, maximum_t } => {
                let mut approved_sources = Vec::with_capacity(sources.len());
                for pile in sources {
                    let Some(&approved) = pile_ids.get(pile) else {
                        problems.push(CaptureDiagnostic::new(
                            bar.name.clone(),
                            "a permitted reclaim source is no longer a stockpile in this project",
                        ));
                        continue;
                    };
                    if piles.iter().any(|entry| entry.id == approved) {
                        approved_sources.push(approved);
                    }
                }
                if approved_sources.len() != sources.len() {
                    continue;
                }
                identities.reclaim_caps.push((bar.bar, *maximum_t));
                TaskKind::Reclaim {
                    approved_sources,
                    maximum_t: *maximum_t,
                }
            }
            ScopedWork::Delay => TaskKind::Delay,
        };
        identities.tasks.push((id, bar.bar, bar.name.clone()));
        tasks.push(Task {
            id,
            loader,
            priority: bar.priority,
            window_start_h: bar.start_h,
            window_end_h: bar.end_h,
            kind,
        });
    }

    // ---- movement candidates ------------------------------------------------
    // All permitted destinations are candidates: routing order is a
    // preference the optimiser is free to trade against capacity, not a
    // reason to leave a later permitted destination out.
    let mut movements: Vec<MovementCandidate> = Vec::new();
    let mut rehandle_rules: Vec<crate::model::schedule::RuleId> = Vec::new();
    let mut qualifications: Vec<GradeQualification> = Vec::new();
    let mut conditional_values: Vec<ConditionalValue> = Vec::new();
    let cashflow = plan.cashflow();
    let fleet = plan.trucks();
    let fields = &source.fields;
    let mut haul = HaulCapture::new(&source.haulage, &source.haul_points, plan);
    for task in &tasks {
        if cancel.is_cancelled() {
            return Err(Vec::new());
        }
        let Some((_, agent, loader_name)) = identities.loaders.iter().find(|(id, _, _)| *id == task.loader) else {
            continue;
        };
        let agent = *agent;
        match &task.kind {
            TaskKind::Dig { sequence } => {
                for &source in sequence {
                    // Every portion of the block, because they move together.
                    let portions: Vec<MaterialId> = context.keys().filter(|(ground, _)| *ground == source).map(|(_, material)| *material).collect();
                    for material in portions {
                        let Some(held) = context.get(&(source, material)) else { continue };
                        let route = RouteSource::Ground {
                            solid: held.solid,
                            bench: held.bench,
                            flitch: held.flitch,
                        };
                        // Ordered rules, resolved exactly as the dispatcher's
                        // routing does - the same `accepts`, the same per-row
                        // values, and the same first-mention-wins de-duplication.
                        let mut offered: Vec<(ProjectDestinationId, crate::model::schedule::RuleId, u32)> = Vec::new();
                        for rule in routing.rules.iter().filter(|rule| rule.enabled) {
                            if !rule.accepts(agent, route, |field| held.value(field)) {
                                continue;
                            }
                            for destination in &rule.destinations {
                                if !offered.iter().any(|(id, _, _)| id == destination) {
                                    let preference = offered.len() as u32;
                                    offered.push((*destination, rule.id, preference));
                                }
                            }
                        }
                        if offered.is_empty() {
                            // A block moves in proportion, so an unroutable
                            // portion stops the whole block rather than leaving
                            // its share behind.
                            problems.push(
                                CaptureDiagnostic::new(
                                    format!("{} · {}", loader_name.clone(), portion_name(&identities, source, held.portion)),
                                    "no enabled destination rule accepts this part of the block, and the rest of the block cannot be dug without it",
                                )
                                .at(ScheduleStep::Destinations),
                            );
                            continue;
                        }
                        for (destination, rule, preference) in offered {
                            expand_candidate(
                                &mut movements,
                                &mut conditional_values,
                                &mut problems,
                                ExpandArgs {
                                    activity: Activity::Dig,
                                    position: Some(held.position),
                                    haul_link: &held.haul_link,
                                    loader: task.loader,
                                    agent,
                                    loader_name,
                                    source: SourceId::Ground(source),
                                    material: held.material,
                                    route,
                                    destination,
                                    dense_destination: destination_ids[&destination],
                                    routing_rule: rule,
                                    routing_preference: preference,
                                    subject: portion_name(&identities, source, held.portion),
                                },
                                fleet,
                                &truck_ids,
                                cashflow,
                                |field| held.value(field),
                                &grade_fields,
                                fields,
                                &mut haul,
                            );
                        }
                    }
                }
            }
            TaskKind::Reclaim { approved_sources, .. } => {
                for &pile in approved_sources {
                    let Some((_, project_pile, pile_name)) = identities.piles.iter().find(|(id, _, _)| *id == pile) else {
                        continue;
                    };
                    let project_pile = *project_pile;
                    let route = RouteSource::Stockpile(project_pile);
                    // A blended pile's grade is a decision variable, so no
                    // condition can be evaluated here. Rules are therefore
                    // split into those that permit unconditionally and those
                    // whose conditions have to become model rows - or be
                    // refused.
                    let mut offered: Vec<(ProjectDestinationId, crate::model::schedule::RuleId, u32)> = Vec::new();
                    for rule in routing.rules.iter().filter(|rule| rule.enabled) {
                        // Identity only: a blended pile's grade is a decision
                        // variable, so the conditions cannot be evaluated here
                        // and are translated into model rows or refused.
                        if !rule.accepts_identity(agent, route) {
                            continue;
                        }
                        for destination in &rule.destinations {
                            // Rehandling from one pile into another (or into
                            // itself) is not modelled: a reclaimed blend has
                            // no captured material for the receiving pile to
                            // hold. Stated, never silently dropped.
                            if pile_ids.contains_key(destination) {
                                if !rehandle_rules.contains(&rule.id) {
                                    rehandle_rules.push(rule.id);
                                    notes.push(crate::i18n::tr!("schedule-capture-rehandle", rule = rule.name.clone()));
                                }
                                continue;
                            }
                            let dense = destination_ids[destination];
                            let bounds = match reclaim_conditions(&rule.conditions, &grade_fields, fields) {
                                Ok(bounds) => bounds,
                                Err(reason) => {
                                    problems.push(CaptureDiagnostic::new(rule.name.clone(), reason).at(ScheduleStep::Destinations));
                                    continue;
                                }
                            };
                            if let Some(qualification) = qualifications
                                .iter_mut()
                                .find(|entry| entry.loader == task.loader && entry.pile == pile && entry.destination == dense)
                            {
                                qualification.alternatives.push(GradePredicate {
                                    rule: RoutingRuleId(rule.id.0 as u32),
                                    bounds,
                                });
                            } else {
                                qualifications.push(GradeQualification {
                                    loader: task.loader,
                                    pile,
                                    destination: dense,
                                    alternatives: vec![GradePredicate {
                                        rule: RoutingRuleId(rule.id.0 as u32),
                                        bounds,
                                    }],
                                });
                            }
                            if !offered.iter().any(|(id, _, _)| id == destination) {
                                let preference = offered.len() as u32;
                                offered.push((*destination, rule.id, preference));
                            }
                        }
                    }
                    if offered.is_empty() {
                        problems.push(
                            CaptureDiagnostic::new(
                                format!("{} · {}", loader_name.clone(), pile_name.clone()),
                                "no enabled destination rule accepts material reclaimed from this stockpile",
                            )
                            .at(ScheduleStep::Destinations),
                        );
                        continue;
                    }
                    for (destination, rule, preference) in offered {
                        let dense = destination_ids[&destination];
                        expand_candidate(
                            &mut movements,
                            &mut conditional_values,
                            &mut problems,
                            ExpandArgs {
                                activity: Activity::Reclaim,
                                position: None,
                                haul_link: &[],
                                loader: task.loader,
                                agent,
                                loader_name,
                                source: SourceId::Stockpile(pile),
                                // A blended pile's composition is a decision,
                                // never a captured material.
                                material: MaterialId(0),
                                route,
                                destination,
                                dense_destination: dense,
                                routing_rule: rule,
                                routing_preference: preference,
                                subject: pile_name.clone(),
                            },
                            fleet,
                            &truck_ids,
                            cashflow,
                            |_| None,
                            &grade_fields,
                            fields,
                            &mut haul,
                        );
                    }
                }
            }
            // A delay moves nothing, so it has no candidates.
            TaskKind::Delay => {}
        }
    }

    let mut grade_targets = Vec::new();
    let mut authored = Vec::new();
    // A calendar for a grade no longer tracked is kept for when it is tracked
    // again, and prices nothing meanwhile: its rows are not shown either.
    for calendar in plan.crusher_grade_calendars().iter().filter(|c| grade_fields.iter().any(|field| field.field == c.field)) {
        for day in 0..days {
            if cancel.is_cancelled() {
                return Err(Vec::new());
            }
            if let Some(specification) = calendar.specification(crate::model::schedule::CalendarPeriod(day as u32)) {
                authored.push((specification, day as u32));
            }
        }
    }
    for (target, day) in &authored {
        let Some(&destination) = destination_ids.get(&target.destination) else {
            problems.push(CaptureDiagnostic::global(crate::i18n::tr!("grade-target-missing-destination")).at(ScheduleStep::Configuration));
            continue;
        };
        // Tracked, by the filter above; grades are blended in their stored
        // numbers, so the authored band needs no conversion.
        let Some(grade) = grade_fields.iter().position(|field| field.field == target.field) else {
            continue;
        };
        let specification = target.clone();
        if specification.validate().is_err() {
            problems.push(CaptureDiagnostic::new(field_name(fields, target.field), crate::i18n::tr!("grade-target-invalid")).at(ScheduleStep::Destinations));
            continue;
        }
        grade_targets.push(crate::model::schedule::optimisation::blended::input::BlendGradeTarget {
            destination,
            grade,
            specification,
            day: *day,
        });
    }

    // ---- event budget and size guard ---------------------------------------
    let derived = derived_segments(&intervals, &loaders, &tasks, &movements);
    let event_capacity = experiment.event_capacity.unwrap_or(SEGMENT_CEILING);
    if !(1..=SEGMENT_CEILING).contains(&event_capacity) {
        return Err(vec![
            CaptureDiagnostic::global(crate::i18n::tr!("schedule-capture-event-capacity")).at(ScheduleStep::Configuration),
        ]);
    }
    let event_budget_restricted = derived > event_capacity;
    let segments_per_interval = derived.clamp(1, event_capacity);
    let estimated_columns = movements.len().saturating_mul(intervals.len()).saturating_mul(segments_per_interval);
    if source.whole_horizon && estimated_columns > COLUMN_CEILING {
        problems.push(
            CaptureDiagnostic::global(crate::i18n::tr!(
                "schedule-capture-too-many-columns",
                columns = format!("{:.1}", estimated_columns as f64 / 1e6),
                ceiling = format!("{:.0}", COLUMN_CEILING as f64 / 1e6)
            ))
            .at(ScheduleStep::Configuration),
        );
    }
    // ---- drill and blast ----------------------------------------------------
    let drill_blast = if plan.drill_blast().enabled {
        capture_drill_blast(source, horizon_h, &intervals, &block_ground, &mut identities, &mut notes, &mut problems)
    } else {
        None
    };
    // A dig bar on ground whose blast no machine works waits for it all
    // horizon. The run is still a valid schedule, so it is a note and a
    // warning on the status rather than a refusal.
    let unworked = drill_blast.as_ref().map(|input| input.unworked()).unwrap_or_default();
    if !unworked.is_empty() {
        let blasts = unworked
            .iter()
            .filter_map(|(index, activity)| {
                identities.blasts.get(*index).map(|blast| {
                    let label = crate::ui::elements::solids_view::blast_path(&blast.solid_name, blast.bench.base, &blast.name);
                    crate::i18n::tr!("drill-blast-unworked-entry", blast = label, step = activity.label())
                })
            })
            .collect::<Vec<_>>()
            .join(", ");
        notes.push(crate::i18n::tr!("drill-blast-unworked", blasts = blasts));
    }

    haul.report(&identities, &mut notes, &mut problems);
    if movements.is_empty() && !has_blast_work && problems.is_empty() {
        problems.push(CaptureDiagnostic::global(crate::i18n::tr!("schedule-capture-no-movement")).at(ScheduleStep::Destinations));
    }
    if !problems.is_empty() {
        return Err(problems.entries);
    }

    let input = BlendInput {
        intervals,
        segments_per_interval,
        grades: grade_table,
        piles,
        loaders,
        tasks,
        ground,
        destinations,
        trucks,
        movements,
        qualifications,
        conditional_values,
        grade_limits: Vec::new(),
        grade_targets,
        target_opening: Vec::new(),
        drill_blast,
    };
    let stats = CaptureStats {
        duration: started.elapsed(),
        candidates: input.movements.len(),
        ground_sources: input.ground.len(),
        event_budget_restricted,
        unworked_blasts: unworked.len(),
    };
    let fingerprint = fingerprint(source, &input);
    Ok(BlendCapture {
        input,
        identities,
        fingerprint,
        stats,
        notes,
    })
}

/// Destination/class searches are shared by every block and material.
struct HaulCapture<'a> {
    network: &'a crate::model::haulage::HaulNetwork,
    points: &'a BTreeMap<ProjectDestinationId, glam::DVec3>,
    index: crate::model::haulage::network::RoadIndex,
    plan: &'a SchedulePlan,
    /// Unconnected blocks are reported once by the Readiness step, not here.
    searches: BTreeMap<(ProjectDestinationId, trucking::TruckClassId), Option<crate::model::haulage::routing::DestinationSearch<'a>>>,
    /// Sources each destination offered by routing could not be reached
    /// from by road, so no candidate was made for them.
    unroutable: BTreeMap<ProjectDestinationId, BTreeSet<SourceId>>,
    /// Sources offered a destination, and those that reached one by road.
    offered: BTreeSet<SourceId>,
    routed: BTreeSet<SourceId>,
}
impl<'a> HaulCapture<'a> {
    fn new(network: &'a crate::model::haulage::HaulNetwork, points: &'a BTreeMap<ProjectDestinationId, glam::DVec3>, plan: &'a SchedulePlan) -> Self {
        Self {
            network,
            points,
            index: crate::model::haulage::network::RoadIndex::new(network),
            plan,
            searches: BTreeMap::new(),
            unroutable: BTreeMap::new(),
            offered: BTreeSet::new(),
            routed: BTreeSet::new(),
        }
    }
    fn point(&self, id: ProjectDestinationId, reclaim: bool) -> Option<glam::DVec3> {
        self.network.destination_point(id, reclaim, self.points.get(&id).copied())
    }
    /// The haul's cycle along the roads, or `None` when no road route
    /// reaches the destination from this source: there is no other way to
    /// say how long the haul takes, so it is not offered at all.
    fn cycle(&mut self, args: &ExpandArgs<'_>, class: &'a trucking::TruckClass) -> Option<trucking::CycleBreakdown> {
        let loader = self.plan.agent(args.agent).and_then(|a| self.plan.class(a.class_id));
        let rate = loader.map_or(0.0, |c| {
            if args.activity == Activity::Dig {
                c.default_dig_rate_tph
            } else {
                c.default_reclaim_rate_tph
            }
        });
        let spot = loader.map_or(0.0, |c| c.spot_time_s);
        let dump = self.plan.routing().dump_time_s(args.destination);
        let source = match args.route {
            RouteSource::Ground { .. } => args.position,
            RouteSource::Stockpile(id) => self.point(id, true),
        };
        let target = self.point(args.destination, false);
        if let (Some(source), Some(target)) = (source, target) {
            let search = self
                .searches
                .entry((args.destination, class.id))
                .or_insert_with(|| crate::model::haulage::routing::DestinationSearch::new(self.network, &self.index, class, target));
            if let Some(route) = search
                .as_ref()
                .and_then(|s| s.route(&self.index, source, args.haul_link, args.activity == Activity::Dig, rate, spot, dump))
            {
                return Some(route.cycle);
            }
        }
        None
    }

    /// What the roads could not reach, for the Haulage step: one note per
    /// destination, and a problem for each source with nowhere to go.
    fn report(&self, identities: &CaptureIdentities, notes: &mut Vec<String>, problems: &mut Diagnostics) {
        for (destination, sources) in &self.unroutable {
            let name = identities
                .destinations
                .iter()
                .find(|(_, id, _)| id == destination)
                .map_or_else(String::new, |(_, _, name)| name.clone());
            notes.push(crate::i18n::tr!("schedule-capture-unroutable", destination = name, count = sources.len().to_string()));
        }
        let stranded = self.offered.difference(&self.routed).count();
        if stranded > 0 {
            problems.push(CaptureDiagnostic::global(crate::i18n::tr!("schedule-capture-stranded", count = stranded.to_string())).at(ScheduleStep::Haulage));
        }
    }
}

/// Everything one candidate needs, so the expansion below takes one argument
/// rather than eleven.
struct ExpandArgs<'a> {
    activity: Activity,
    position: Option<glam::DVec3>,
    haul_link: &'a [crate::model::haulage::NodeId],
    loader: LoaderId,
    agent: LoaderAgentId,
    loader_name: &'a str,
    source: SourceId,
    material: MaterialId,
    route: RouteSource,
    destination: ProjectDestinationId,
    dense_destination: DestinationId,
    routing_rule: crate::model::schedule::RuleId,
    routing_preference: u32,
    subject: String,
}

/// One permitted (loader, source, destination) movement becomes one candidate
/// per compatible truck class.
///
/// Trucking rules union their classes, and no matching class is *no candidate*
/// rather than unlimited trucks - which is the authored semantics and is why
/// this can produce nothing without that being an error.
#[allow(
    clippy::too_many_arguments,
    reason = "the project's four rule configurations and the two id maps are all genuinely needed here"
)]
fn expand_candidate<'a>(
    movements: &mut Vec<MovementCandidate>,
    conditional_values: &mut Vec<ConditionalValue>,
    problems: &mut Diagnostics,
    args: ExpandArgs<'_>,
    fleet: &'a trucking::TruckFleetConfig,
    truck_ids: &BTreeMap<trucking::TruckClassId, TruckClassId>,
    cashflow: &crate::model::schedule::cashflow::CashflowConfig,
    value: impl Fn(ReserveFieldId) -> Option<PortionValue> + Copy,
    grades: &[GradeField],
    fields: &[ReserveField],
    haul: &mut HaulCapture<'a>,
) {
    let context = RouteContext {
        loader: args.agent,
        source: args.route,
        destination: args.destination,
    };
    let classes = fleet.allowed_classes(context);
    if classes.is_empty() {
        problems.push(
            CaptureDiagnostic::new(
                format!("{} · {}", args.loader_name, args.subject),
                "no compatible truck class serves this route, so nothing can be hauled on it",
            )
            .at(ScheduleStep::TruckingRules),
        );
        return;
    }
    let movement = MovementContext {
        activity: match args.activity {
            Activity::Dig => CashflowActivity::Dig,
            Activity::Reclaim => CashflowActivity::Reclaim,
        },
        loader: args.agent,
        source: args.route,
        destination: args.destination,
    };
    let mut conditioned = Vec::new();
    let contributions: Vec<CashflowContribution> = if args.activity == Activity::Reclaim {
        let mut unconditional = Vec::new();
        for rule in cashflow.rules.iter().filter(|rule| rule.matches_identity(movement)) {
            if rule.conditions.is_empty() {
                unconditional.push(CashflowContribution {
                    rule: CashflowRuleId(rule.id.0 as u32),
                    value_per_tonne: rule.value_per_tonne,
                });
            } else {
                match reclaim_conditions(&rule.conditions, grades, fields) {
                    Ok(bounds) => conditioned.push((CashflowRuleId(rule.id.0 as u32), rule.value_per_tonne, bounds)),
                    Err(reason) => problems.push(CaptureDiagnostic::new(rule.name.clone(), reason).at(ScheduleStep::Cashflow)),
                }
            }
        }
        unconditional
    } else {
        match cashflow.valuation(movement, value) {
            Ok(valuation) => valuation
                .contributions
                .iter()
                .map(|(rule, value)| CashflowContribution {
                    rule: CashflowRuleId(rule.0 as u32),
                    value_per_tonne: *value,
                })
                .collect(),
            Err(error) => {
                problems.push(CaptureDiagnostic::new(args.subject.clone(), error.message()).at(ScheduleStep::Cashflow));
                return;
            }
        }
    };
    haul.offered.insert(args.source);
    for class in classes {
        let Some(definition) = fleet.class(class) else { continue };
        let Some(cycle) = haul.cycle(&args, definition) else {
            haul.unroutable.entry(args.destination).or_default().insert(args.source);
            continue;
        };
        haul.routed.insert(args.source);
        let coefficients = match trucking::coefficients_from_cycle(definition, cycle) {
            Ok(coefficients) => coefficients,
            Err(error) => {
                problems.push(CaptureDiagnostic::new(definition.name.clone(), error.message()).at(ScheduleStep::Haulage));
                continue;
            }
        };
        let Some(&dense) = truck_ids.get(&class) else { continue };
        let candidate = movements.len();
        movements.push(MovementCandidate {
            loader: args.loader,
            activity: args.activity,
            source: args.source,
            material: args.material,
            destination: args.dense_destination,
            truck: dense,
            truck_hours_per_tonne: coefficients.truck_hours_per_tonne,
            cycle,
            routing_rule: RoutingRuleId(args.routing_rule.0 as u32),
            routing_preference: args.routing_preference,
            cashflow: contributions.clone(),
        });
        for (rule, value_per_tonne, bounds) in &conditioned {
            conditional_values.push(ConditionalValue {
                candidate,
                rule: *rule,
                value_per_tonne: *value_per_tonne,
                bounds: bounds.clone(),
            });
        }
    }
}

/// Translate authored reclaim conditions into bounds on the blended grade.
fn reclaim_conditions(conditions: &[FieldCondition], grades: &[GradeField], fields: &[ReserveField]) -> Result<Vec<GradeBound>, String> {
    let mut bounds = Vec::new();
    for condition in conditions {
        let name = field_name(fields, condition.field);
        // The kind of condition first: a category is unsupported because the
        // blend discards categories, which is a better answer than "that
        // field is not a grade".
        if matches!(condition.test, ConditionTest::Category { .. }) {
            return Err(format!("category conditions on '{name}' are unsupported after blending"));
        }
        let Some(position) = grades.iter().position(|grade| grade.field == condition.field) else {
            return Err(format!("'{name}' is not one of this run's blended grades, so it cannot be tested on reclaimed material"));
        };
        match &condition.test {
            ConditionTest::Category { .. } => unreachable!("refused above"),
            ConditionTest::Range { lower, upper } => {
                if lower.is_none() && upper.is_none() {
                    return Err(format!("a condition on '{name}' with neither bound is not a condition"));
                }
                let endpoint = |bound: &crate::model::schedule::destinations::Bound| -> Result<GradeEndpoint, String> {
                    let value = grades[position].basis.to_fraction(bound.value);
                    if !value.is_finite() || value < 0.0 {
                        return Err(format!("'{name}' bound {} must be finite and non-negative", bound.value));
                    }
                    Ok(GradeEndpoint {
                        value,
                        inclusive: bound.inclusive,
                    })
                };
                bounds.push(GradeBound {
                    grade: position,
                    lower: lower.as_ref().map(endpoint).transpose()?,
                    upper: upper.as_ref().map(endpoint).transpose()?,
                });
            }
        }
    }
    Ok(bounds)
}

/// The per-interval execution-segment budget, by the same union-bound rule
/// the accepted backend derives its own from: one, plus each loader's own
/// internal transitions, because the event clock is shared and distinct
/// loaders' boundaries need not coincide.
///
/// A loader works the blocks of one bar's sequence back to back inside a
/// segment, so a dig bar needs a segment of its own, not one per block. The
/// exception is a block another loader can also dig: the next block waits
/// for a segment boundary after it, so that transition still counts. A
/// reclaim bar keeps one per approved pile.
///
/// Returns the derived figure *before* the shared [`SEGMENT_CEILING`] guard
/// is applied, so the caller can report that the budget was restricted.
fn derived_segments(intervals: &[Interval], loaders: &[Loader], tasks: &[Task], movements: &[MovementCandidate]) -> usize {
    let digs = |loader: LoaderId, ground: GroundId| movements.iter().any(|candidate| candidate.loader == loader && candidate.source == SourceId::Ground(ground));
    let mut highest = 1;
    for interval in intervals {
        let mut budget = 1;
        for loader in loaders {
            let rate = loader.rates.get(interval.index);
            let can_dig = rate.is_some_and(|rate| rate.dig_tph > 0.0);
            let can_reclaim = rate.is_some_and(|rate| rate.reclaim_tph > 0.0);
            let mut phases = 0usize;
            let mut piles: Vec<StockpileId> = Vec::new();
            for task in tasks.iter().filter(|task| task.loader == loader.id) {
                if task.window_start_h > interval.start_h + 1e-9 || task.window_end_h < interval.end_h - 1e-9 {
                    continue;
                }
                match &task.kind {
                    TaskKind::Dig { sequence } if can_dig => {
                        let worked: Vec<GroundId> = sequence.iter().copied().filter(|ground| digs(loader.id, *ground)).collect();
                        if worked.is_empty() {
                            continue;
                        }
                        let shared = worked
                            .windows(2)
                            .filter(|pair| loaders.iter().any(|other| other.id != loader.id && digs(other.id, pair[0])))
                            .count();
                        phases += 1 + shared;
                    }
                    TaskKind::Reclaim { approved_sources, .. } if can_reclaim => {
                        for pile in approved_sources {
                            let source = SourceId::Stockpile(*pile);
                            if !piles.contains(pile) && movements.iter().any(|candidate| candidate.loader == loader.id && candidate.source == source) {
                                piles.push(*pile);
                            }
                        }
                    }
                    _ => {}
                }
            }
            budget += (phases + piles.len()).saturating_sub(1);
        }
        highest = highest.max(budget);
    }
    highest
}

fn block_name(identities: &CaptureIdentities, ground: GroundId) -> String {
    identities
        .ground
        .iter()
        .find(|(id, _, _)| *id == ground)
        .map(|(_, name, _)| name.clone())
        .unwrap_or_else(|| format!("block {}", ground.0))
}

/// One portion of a block, named so a diagnostic can be located. A block with
/// a single captured material is named by itself.
fn portion_name(identities: &CaptureIdentities, ground: GroundId, portion: usize) -> String {
    let name = block_name(identities, ground);
    let portions = identities.ground.iter().find(|(id, _, _)| *id == ground).map(|(_, _, count)| *count).unwrap_or(1);
    if portions > 1 { format!("{name} (material {})", portion + 1) } else { name }
}

/// The complete experimental-run identity.
///
/// Everything that can change feasibility, the objective or the encoded
/// model, and nothing that cannot: names and the currency label are
/// deliberately absent, so a rename does not retire a result.
///
/// Hashed off the *built* model rather than off the project, which is what
/// makes it complete: every rate, window, capacity, coefficient, opening lot,
/// chunk capacity and grade conversion is already resolved into it, so there
/// is no input a separate list could forget. The project identities that
/// produced it are folded in beside, so two projects that happen to build the
/// same model are still two runs.
fn fingerprint(source: &CaptureSnapshot, input: &BlendInput) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.runtime.hash(&mut hasher);
    source.generation.hash(&mut hasher);
    source.plan_revision.hash(&mut hasher);
    source.tonnage_field.0.hash(&mut hasher);
    source.haulage.hash_content(&mut hasher);
    // The model itself. `BlendInput` derives `PartialEq` over exactly the
    // fields the solvers read, and every one of them is walked here.
    input.segments_per_interval.hash(&mut hasher);
    serde_json::to_string(&input.drill_blast).unwrap_or_default().hash(&mut hasher);
    for interval in &input.intervals {
        interval.index.hash(&mut hasher);
        interval.start_h.to_bits().hash(&mut hasher);
        interval.end_h.to_bits().hash(&mut hasher);
    }
    for grade in &input.grades.fields {
        grade.field.0.hash(&mut hasher);
        format!("{:?}", grade.basis).hash(&mut hasher);
    }
    for pile in &input.piles {
        pile.id.0.hash(&mut hasher);
        pile.capacity_t.to_bits().hash(&mut hasher);
        pile.opening_t.to_bits().hash(&mut hasher);
        for value in &pile.opening_q {
            value.to_bits().hash(&mut hasher);
        }
        for capacity in &pile.chunks {
            capacity.to_bits().hash(&mut hasher);
        }
        for (tonnes, contained) in &pile.chunk_opening {
            tonnes.to_bits().hash(&mut hasher);
            for value in contained {
                value.to_bits().hash(&mut hasher);
            }
        }
        format!("{:?}", pile.order).hash(&mut hasher);
    }
    for loader in &input.loaders {
        loader.id.0.hash(&mut hasher);
        for rate in &loader.rates {
            rate.interval.hash(&mut hasher);
            rate.dig_tph.to_bits().hash(&mut hasher);
            rate.reclaim_tph.to_bits().hash(&mut hasher);
        }
    }
    for task in &input.tasks {
        task.id.0.hash(&mut hasher);
        task.loader.0.hash(&mut hasher);
        task.priority.hash(&mut hasher);
        task.window_start_h.to_bits().hash(&mut hasher);
        task.window_end_h.to_bits().hash(&mut hasher);
        match &task.kind {
            TaskKind::Dig { sequence } => {
                0u8.hash(&mut hasher);
                for ground in sequence {
                    ground.0.hash(&mut hasher);
                }
            }
            TaskKind::Reclaim { approved_sources, maximum_t } => {
                1u8.hash(&mut hasher);
                for pile in approved_sources {
                    pile.0.hash(&mut hasher);
                }
                maximum_t.map(f64::to_bits).hash(&mut hasher);
            }
            TaskKind::Delay => 2u8.hash(&mut hasher),
        }
    }
    for entry in &input.ground {
        entry.id.0.hash(&mut hasher);
        entry.tonnes_t.to_bits().hash(&mut hasher);
        for share in &entry.material {
            share.material.0.hash(&mut hasher);
            share.fraction.to_bits().hash(&mut hasher);
            for grade in 0..input.grades.count() {
                input.grades.fraction(share.material, grade).map(f64::to_bits).hash(&mut hasher);
            }
        }
    }
    for destination in &input.destinations {
        destination.id.0.hash(&mut hasher);
        format!("{:?}", destination.kind).hash(&mut hasher);
        destination.capacity_t.map(f64::to_bits).hash(&mut hasher);
        for limit in &destination.crusher_daily_t {
            limit.map(f64::to_bits).hash(&mut hasher);
        }
    }
    for truck in &input.trucks {
        truck.id.0.hash(&mut hasher);
        for hours in &truck.hours {
            hours.to_bits().hash(&mut hasher);
        }
    }
    for candidate in &input.movements {
        candidate.loader.0.hash(&mut hasher);
        format!("{:?}", candidate.activity).hash(&mut hasher);
        format!("{:?}", candidate.source).hash(&mut hasher);
        candidate.material.0.hash(&mut hasher);
        candidate.destination.0.hash(&mut hasher);
        candidate.truck.0.hash(&mut hasher);
        candidate.truck_hours_per_tonne.to_bits().hash(&mut hasher);
        candidate.routing_rule.0.hash(&mut hasher);
        candidate.routing_preference.hash(&mut hasher);
        for contribution in &candidate.cashflow {
            contribution.rule.0.hash(&mut hasher);
            contribution.value_per_tonne.to_bits().hash(&mut hasher);
        }
    }
    for limit in &input.grade_limits {
        limit.destination.0.hash(&mut hasher);
        limit.grade.hash(&mut hasher);
        limit.minimum.to_bits().hash(&mut hasher);
        limit.inclusive.hash(&mut hasher);
    }
    for qualification in &input.qualifications {
        qualification.loader.0.hash(&mut hasher);
        qualification.pile.0.hash(&mut hasher);
        qualification.destination.0.hash(&mut hasher);
        for alternative in &qualification.alternatives {
            alternative.rule.0.hash(&mut hasher);
            for bound in &alternative.bounds {
                bound.grade.hash(&mut hasher);
                bound.lower.map(|end| (end.value.to_bits(), end.inclusive)).hash(&mut hasher);
                bound.upper.map(|end| (end.value.to_bits(), end.inclusive)).hash(&mut hasher);
            }
        }
    }
    for conditional in &input.conditional_values {
        conditional.candidate.hash(&mut hasher);
        conditional.rule.0.hash(&mut hasher);
        conditional.value_per_tonne.to_bits().hash(&mut hasher);
        for bound in &conditional.bounds {
            bound.grade.hash(&mut hasher);
            bound.lower.map(|end| (end.value.to_bits(), end.inclusive)).hash(&mut hasher);
            bound.upper.map(|end| (end.value.to_bits(), end.inclusive)).hash(&mut hasher);
        }
    }
    for target in &input.grade_targets {
        target.destination.0.hash(&mut hasher);
        target.grade.hash(&mut hasher);
        target.day.hash(&mut hasher);
        target.specification.lower.map(f64::to_bits).hash(&mut hasher);
        target.specification.target.to_bits().hash(&mut hasher);
        target.specification.upper.map(f64::to_bits).hash(&mut hasher);
        target.specification.penalty_per_tonne.to_bits().hash(&mut hasher);
        target.specification.outside_multiplier.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

/// Whether one scope is placeable against the blocks a run produced.
///
/// A scope that is not is reported against the rule that names it rather than
/// repaired: a scope whose bench has been re-cut is a rule that no longer
/// restricts what it was written to restrict, and silently widening it would
/// route material somewhere nobody chose.
fn scope_is_placeable(scope: crate::model::schedule::SourceScope, blocks: &[crate::app::commands::solids_view::DigBlockRecord]) -> bool {
    blocks
        .iter()
        .any(|block| scope.covers(block.solid, (block.bench.base, block.bench.top), (block.flitch.base, block.flitch.top)))
}

fn field_name(fields: &[ReserveField], id: ReserveFieldId) -> String {
    fields
        .iter()
        .find(|field| field.id == id)
        .map(|field| field.name.clone())
        .unwrap_or_else(|| format!("field {}", id.0))
}
