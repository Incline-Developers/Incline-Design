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
//! readiness reports were measured against, through the same identity checks
//! - nothing here runs Solids, walks a scene composite or recomputes a
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
//! rows' own values are known at capture, exactly as
//! [`super::schedule_routing`] reads them, so every rule form is supported
//! there.

use std::{
    collections::BTreeMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
    time::{Duration, Instant},
};

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
}

impl CaptureDiagnostic {
    fn new(subject: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            subject: Some(subject.into()),
            message: message.into(),
            occurrences: 1,
        }
    }

    fn global(message: impl Into<String>) -> Self {
        Self {
            subject: None,
            message: message.into(),
            occurrences: 1,
        }
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
    pub(crate) grades: Vec<(ReserveFieldId, String, GradeUnit)>,
}

/// Measured facts about the capture itself, for the completion report.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CaptureStats {
    pub(crate) duration: Duration,
    pub(crate) candidates: usize,
    pub(crate) ground_sources: usize,
    /// Dig blocks holding more than one captured material, each captured as
    /// one block dug in its measured proportions.
    pub(crate) mixed_blocks: usize,
    /// Whether the derived event budget hit [`SEGMENT_CEILING`].
    pub(crate) event_budget_restricted: bool,
    /// Estimated model size, so an unbounded capture is refused rather than
    /// allocated.
    pub(crate) estimated_columns: usize,
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
    pub(crate) generation: u64,
    pub(crate) plan_revision: u64,
    pub(crate) fleet_revision: u64,
    plan: SchedulePlan,
    fields: Vec<ReserveField>,
    tonnage_field: ReserveFieldId,
    destinations: Vec<DestinationView>,
    snapshot: Arc<PlanningSnapshot>,
    reports: Arc<Vec<BarReport>>,
}

impl CaptureSnapshot {
    /// The Setup run this capture was taken against, so the caller can gate
    /// publication on exactly the inputs the model was built from.
    pub(crate) fn inputs(&self) -> crate::app::schedule_pipeline::ScheduleRunInputs {
        crate::app::schedule_pipeline::ScheduleRunInputs {
            runtime: self.runtime,
            generation: self.generation,
            fleet_revision: self.fleet_revision,
            tonnage_field: self.tonnage_field,
        }
    }
}

/// Cap on the estimated column count, so a horizon somebody typed three extra
/// zeroes into is refused with a figure rather than allocated.
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
    Dig { members: Vec<(usize, f64)> },
    Reclaim { sources: Vec<ProjectDestinationId>, maximum_t: Option<f64> },
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
}

impl GroundContext {
    /// One field's value on this material, read exactly as
    /// [`super::schedule_routing`] reads it.
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
    /// Take the owned configuration snapshot one experimental run reads.
    ///
    /// Bounded: it clones the plan and the field list, borrows the cached
    /// planning snapshot and reports through `Arc`, and resolves no candidate.
    pub(crate) fn capture_experimental_snapshot(&mut self) -> Result<CaptureSnapshot, Vec<CaptureDiagnostic>> {
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
            generation: inputs.generation,
            fleet_revision: inputs.fleet_revision,
            plan_revision,
            plan,
            fields: document.reserve_fields().to_vec(),
            tonnage_field,
            destinations,
            snapshot,
            reports,
        })
    }
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
    let horizon_h = f64::from(experiment.planning_end_day) * SCHEDULE_PERIOD_H;
    if experiment.planning_end_day == 0 || !horizon_h.is_finite() || horizon_h <= 0.0 {
        return Err(vec![CaptureDiagnostic::global(
            "the experimental planning horizon is not a finite, positive number of days",
        )]);
    }
    if !experiment.interval_h.is_finite() || experiment.interval_h <= 0.0 {
        return Err(vec![CaptureDiagnostic::global("the experimental calendar interval is not finite and positive")]);
    }

    // ---- bars in scope -----------------------------------------------------
    let mut scoped: Vec<ScopedBar> = Vec::new();
    for (bar, report) in plan.bars().iter().zip(source.reports.iter()) {
        if cancel.is_cancelled() {
            return Err(Vec::new());
        }
        let Some(agent) = bar.agent else {
            if bar.is_reclaim() || !bar.members().is_empty() {
                problems.push(CaptureDiagnostic::new(bar.name().to_owned(), "this bar is not assigned to a loader"));
            }
            continue;
        };
        // Open-ended bars are truncated by the explicit horizon; finite ones
        // are intersected with it. A bar entirely beyond the horizon is not a
        // fault - it simply has no work in this run.
        let start_h = bar.window.start_h;
        let end_h = bar.window.end_h.unwrap_or(horizon_h).min(horizon_h);
        if !(start_h.is_finite() && end_h.is_finite()) || end_h <= start_h {
            continue;
        }
        let work = if let Some(reclaim) = bar.reclaim() {
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
                members.push((position, tonnes));
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
    if scoped.is_empty() && problems.is_empty() {
        problems.push(CaptureDiagnostic::global("no assigned bar has work inside the experimental horizon"));
    }

    // ---- grades ------------------------------------------------------------
    // Only the fields the blend actually needs: an unmapped field nothing
    // reads must not block a run.
    let mut grade_fields = Vec::new();
    for (field, unit) in &experiment.grades {
        let basis = match unit {
            GradeUnit::Fraction => GradeBasis::Fraction,
            GradeUnit::Percent => GradeBasis::Percent,
        };
        match GradeField::accept(*field, basis, &source.fields, Some(source.tonnage_field)) {
            Ok(accepted) => grade_fields.push(accepted),
            Err(rejection) => problems.push(CaptureDiagnostic::new(field_name(&source.fields, *field), rejection.message())),
        }
    }

    // ---- destinations ------------------------------------------------------
    let mut destination_ids: BTreeMap<ProjectDestinationId, DestinationId> = BTreeMap::new();
    let mut destinations: Vec<Destination> = Vec::new();
    let mut pile_ids: BTreeMap<ProjectDestinationId, StockpileId> = BTreeMap::new();
    let mut identities = CaptureIdentities::default();
    let days = experiment.planning_end_day as usize;
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
            problems.push(CaptureDiagnostic::new(view.name.clone(), "choose a stockpile representation"));
            continue;
        }
        let Some(capacity_t) = view.capacity_t else {
            problems.push(CaptureDiagnostic::new(
                view.name.clone(),
                "the blended model needs a finite pile capacity, because occupancy is a constraint in it",
            ));
            continue;
        };
        // Opening lots, read exactly as authored. A missing grade is missing,
        // never zero.
        let inventory = routing.inventory(*project_id);
        let mut lots: Vec<(f64, Vec<f64>)> = Vec::new();
        for lot in inventory.map(|inventory| inventory.lots.as_slice()).unwrap_or_default() {
            let mut tonnes = 0.0;
            let mut contained = vec![0.0; grades];
            let mut sound = true;
            for portion in &lot.portions {
                for (position, grade) in grade_fields.iter().enumerate() {
                    let value = match portion.value(grade.field).map(|value| value.portion_value()) {
                        Some(PortionValue::Number(value)) => value,
                        _ => {
                            problems.push(CaptureDiagnostic::new(
                                format!("{} · {}", view.name.clone(), lot.name.clone()),
                                format!("'{}' is missing on an opening portion; a missing grade cannot be read as zero", grade.name),
                            ));
                            sound = false;
                            continue;
                        }
                    };
                    let fraction = grade.basis.to_fraction(value);
                    if !(0.0..=1.0).contains(&fraction) {
                        problems.push(CaptureDiagnostic::new(
                            format!("{} · {}", view.name.clone(), lot.name.clone()),
                            format!("'{}' is {value}, which is outside the range its declared unit allows", grade.name),
                        ));
                        sound = false;
                        continue;
                    }
                    contained[position] += portion.tonnes_t * fraction;
                }
                tonnes += portion.tonnes_t;
            }
            if sound {
                lots.push((tonnes, contained));
            }
        }
        let total_opening: f64 = lots.iter().map(|(tonnes, _)| tonnes).sum();
        if total_opening > capacity_t + 1e-6 {
            problems.push(CaptureDiagnostic::new(
                view.name.clone(),
                format!("opening stock of {total_opening:.2} t exceeds the pile capacity of {capacity_t:.2} t"),
            ));
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
    for bar in &scoped {
        if let ScopedWork::Dig { members } = &bar.work {
            for (position, _) in members {
                if !wanted_blocks.contains(position) {
                    wanted_blocks.push(*position);
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
                // table applies the declared basis once; converting here as
                // well would divide a percentage by a hundred twice.
                if !(0.0..=1.0).contains(&grade.basis.to_fraction(raw)) {
                    problems.push(CaptureDiagnostic::new(
                        block.name.clone(),
                        format!("'{}' is {raw}, which is outside the range its declared unit allows", grade.name),
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
        let unit = experiment.grade_unit(grade.field).unwrap_or(GradeUnit::Fraction);
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
            problems.push(CaptureDiagnostic::new(bar.name.clone(), "this bar's loader is no longer in the project"));
            continue;
        };
        let Some(class) = plan.class(agent.class_id) else {
            problems.push(CaptureDiagnostic::new(agent.name.clone(), "this loader's class is no longer in the project"));
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
        regular_step_h: experiment.interval_h,
        // Not read by the blended model, which has no parcels; a positive
        // value is required only so the shared interval builder validates.
        parcel_target_t: 1.0,
        event_segments: None,
        tolerances: NumericalTolerances::default(),
    };
    let intervals = match build_intervals(spec, &windows, &rate_changes) {
        Ok(intervals) => intervals,
        Err(error) => {
            problems.push(CaptureDiagnostic::global(format!("the experimental calendar could not be built: {error:?}")));
            Vec::new()
        }
    };

    // ---- per-interval rates and truck hours --------------------------------
    let loaders: Vec<Loader> = compiled
        .iter()
        .map(|(id, _, dig, reclaim)| Loader {
            id: *id,
            rates: intervals
                .iter()
                .map(|interval| IntervalRate {
                    interval: interval.index,
                    // The rate at the interval's own start: a calendar change
                    // inside an interval cannot happen, because every change
                    // is an interval boundary above.
                    dig_tph: dig.rate_at(interval.start_h),
                    reclaim_tph: reclaim.rate_at(interval.start_h),
                })
                .collect(),
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
                    problems.push(CaptureDiagnostic::new(class.name.clone(), error.message()));
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
                TaskKind::Reclaim {
                    approved_sources,
                    maximum_t: *maximum_t,
                }
            }
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
    let mut qualifications: Vec<GradeQualification> = Vec::new();
    let mut conditional_values: Vec<ConditionalValue> = Vec::new();
    let cashflow = plan.cashflow();
    let fleet = plan.trucks();
    let fields = &source.fields;
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
                            problems.push(CaptureDiagnostic::new(
                                format!("{} · {}", loader_name.clone(), portion_name(&identities, source, held.portion)),
                                "no enabled destination rule accepts this part of the block, and the rest of the block cannot be dug without it",
                            ));
                            continue;
                        }
                        for (destination, rule, preference) in offered {
                            expand_candidate(
                                &mut movements,
                                &mut conditional_values,
                                &mut problems,
                                ExpandArgs {
                                    activity: Activity::Dig,
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
                                routing,
                                fleet,
                                &truck_ids,
                                cashflow,
                                |field| held.value(field),
                                &grade_fields,
                                fields,
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
                            let dense = destination_ids[destination];
                            let bounds = match reclaim_conditions(&rule.conditions, &grade_fields, fields) {
                                Ok(bounds) => bounds,
                                Err(reason) => {
                                    problems.push(CaptureDiagnostic::new(rule.name.clone(), reason));
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
                        problems.push(CaptureDiagnostic::new(
                            format!("{} · {}", loader_name.clone(), pile_name.clone()),
                            "no enabled destination rule accepts material reclaimed from this stockpile",
                        ));
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
                            routing,
                            fleet,
                            &truck_ids,
                            cashflow,
                            |_| None,
                            &grade_fields,
                            fields,
                        );
                    }
                }
            }
        }
    }

    // ---- event budget and size guard ---------------------------------------
    let derived = derived_segments(&intervals, &loaders, &tasks, &movements);
    let event_budget_restricted = derived > SEGMENT_CEILING;
    let segments_per_interval = derived.clamp(1, SEGMENT_CEILING);
    let estimated_columns = movements.len().saturating_mul(intervals.len()).saturating_mul(segments_per_interval);
    if estimated_columns > COLUMN_CEILING {
        problems.push(CaptureDiagnostic::global(format!(
            "this horizon and resolution would build about {estimated_columns} movement columns, beyond the {COLUMN_CEILING} this experiment allows; shorten the horizon or widen the interval"
        )));
    }
    if movements.is_empty() && problems.is_empty() {
        problems.push(CaptureDiagnostic::global("no movement is permitted by the current rules, so there is nothing to optimise"));
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
    };
    let stats = CaptureStats {
        duration: started.elapsed(),
        candidates: input.movements.len(),
        ground_sources: input.ground.len(),
        mixed_blocks,
        event_budget_restricted,
        estimated_columns,
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

/// Everything one candidate needs, so the expansion below takes one argument
/// rather than eleven.
struct ExpandArgs<'a> {
    activity: Activity,
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
fn expand_candidate(
    movements: &mut Vec<MovementCandidate>,
    conditional_values: &mut Vec<ConditionalValue>,
    problems: &mut Diagnostics,
    args: ExpandArgs<'_>,
    routing: &crate::model::schedule::destinations::RoutingConfig,
    fleet: &trucking::TruckFleetConfig,
    truck_ids: &BTreeMap<trucking::TruckClassId, TruckClassId>,
    cashflow: &crate::model::schedule::cashflow::CashflowConfig,
    value: impl Fn(ReserveFieldId) -> Option<PortionValue> + Copy,
    grades: &[GradeField],
    fields: &[ReserveField],
) {
    let context = RouteContext {
        loader: args.agent,
        source: args.route,
        destination: args.destination,
    };
    let classes = fleet.allowed_classes(context);
    if classes.is_empty() {
        problems.push(CaptureDiagnostic::new(
            format!("{} · {}", args.loader_name, args.subject),
            "no compatible truck class serves this route, so nothing can be hauled on it",
        ));
        return;
    }
    let distance_km = routing.distance_km(args.destination);
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
                    Err(reason) => problems.push(CaptureDiagnostic::new(rule.name.clone(), reason)),
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
                problems.push(CaptureDiagnostic::new(args.subject.clone(), error.message()));
                return;
            }
        }
    };
    for class in classes {
        let Some(definition) = fleet.class(class) else { continue };
        let coefficients = match trucking::coefficients(definition, context, distance_km) {
            Ok(coefficients) => coefficients,
            Err(error) => {
                problems.push(CaptureDiagnostic::new(definition.name.clone(), error.message()));
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
                    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                        return Err(format!("'{name}' bound {} is outside the range its declared unit allows", bound.value));
                    }
                    Ok(GradeEndpoint {
                        value,
                        inclusive: bound.inclusive,
                    })
                };
                bounds.push(GradeBound {
                    grade: position,
                    lower: lower.as_ref().map(&endpoint).transpose()?,
                    upper: upper.as_ref().map(&endpoint).transpose()?,
                });
            }
        }
    }
    Ok(bounds)
}

/// The per-interval execution-segment budget, by the same union-bound rule
/// the accepted backend derives its own from: one, plus each loader's own
/// internal source transitions, because the event clock is shared and
/// distinct loaders' boundaries need not coincide.
///
/// Returns the derived figure *before* the shared [`SEGMENT_CEILING`] guard
/// is applied, so the caller can report that the budget was restricted.
fn derived_segments(intervals: &[Interval], loaders: &[Loader], tasks: &[Task], movements: &[MovementCandidate]) -> usize {
    let mut highest = 1;
    for interval in intervals {
        let mut budget = 1;
        for loader in loaders {
            let rate = loader.rates.get(interval.index);
            let can_dig = rate.is_some_and(|rate| rate.dig_tph > 0.0);
            let can_reclaim = rate.is_some_and(|rate| rate.reclaim_tph > 0.0);
            let mut sources: Vec<SourceId> = Vec::new();
            for task in tasks.iter().filter(|task| task.loader == loader.id) {
                if task.window_start_h > interval.start_h + 1e-9 || task.window_end_h < interval.end_h - 1e-9 {
                    continue;
                }
                match &task.kind {
                    TaskKind::Dig { sequence } if can_dig => {
                        for ground in sequence {
                            let source = SourceId::Ground(*ground);
                            if !sources.contains(&source) && movements.iter().any(|candidate| candidate.loader == loader.id && candidate.source == source) {
                                sources.push(source);
                            }
                        }
                    }
                    TaskKind::Reclaim { approved_sources, .. } if can_reclaim => {
                        for pile in approved_sources {
                            let source = SourceId::Stockpile(*pile);
                            if !sources.contains(&source) && movements.iter().any(|candidate| candidate.loader == loader.id && candidate.source == source) {
                                sources.push(source);
                            }
                        }
                    }
                    _ => {}
                }
            }
            budget += sources.len().saturating_sub(1);
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
    // The model itself. `BlendInput` derives `PartialEq` over exactly the
    // fields the solvers read, and every one of them is walked here.
    input.segments_per_interval.hash(&mut hasher);
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
    hasher.finish()
}

fn field_name(fields: &[ReserveField], id: ReserveFieldId) -> String {
    fields
        .iter()
        .find(|field| field.id == id)
        .map(|field| field.name.clone())
        .unwrap_or_else(|| format!("field {}", id.0))
}

#[cfg(test)]
impl CaptureSnapshot {
    /// Assemble a capture snapshot directly, for the end-to-end developer
    /// checks below.
    ///
    /// Exactly what [`crate::app::App::capture_experimental_snapshot`]
    /// produces, with the same types and the same invariants - the App read
    /// is a handful of field copies and has nothing of its own to get wrong.
    pub(crate) fn for_test(
        plan: SchedulePlan,
        fields: Vec<ReserveField>,
        tonnage_field: ReserveFieldId,
        destinations: Vec<DestinationView>,
        blocks: Vec<crate::app::commands::solids_view::DigBlockRecord>,
        reports: Vec<BarReport>,
    ) -> Self {
        Self {
            runtime: 1,
            generation: 9,
            plan_revision: 5,
            fleet_revision: 3,
            plan,
            fields,
            tonnage_field,
            destinations,
            snapshot: Arc::new(PlanningSnapshot {
                runtime: 1,
                generation: 9,
                blocks,
            }),
            reports: Arc::new(reports),
        }
    }
}

/// End-to-end checks of the real capture path against a small reproducible
/// project fixture.
///
/// The fixture is the one the brief asks for: an ordered dig sequence, a
/// reclaim bar, trucks and compatibility rules, a stockpile and a crusher,
/// opening inventory, and grade-dependent routing and cashflow.
#[cfg(test)]
mod project_capture_checks {
    use std::sync::Arc;

    use super::*;
    use crate::{
        app::commands::{
            schedule_readiness::{BarReport, MemberReport, ReclaimReport},
            solids_view::{BlockPortions, DigBlockRecord, MaterialState},
        },
        model::{
            DigBlockId, ReserveAggregation, SolidId,
            schedule::{
                Bound, ConditionTest, DestinationKind as Kind, DestinationSelection, FieldCondition, LoaderSelection, ReclaimOrder as ProjectOrder, WorkWindow,
                experiment::{GradeUnit, StockpileRepresentation},
            },
            solid_reserves::{CapturedField, MaterialCapture, MaterialPortion},
        },
        ui::state::BenchSelection,
    };

    const TONNES: ReserveFieldId = ReserveFieldId(1);
    const FE: ReserveFieldId = ReserveFieldId(2);
    const ROCK: ReserveFieldId = ReserveFieldId(3);

    fn fields() -> Vec<ReserveField> {
        vec![
            ReserveField {
                id: TONNES,
                name: "Tonnes".into(),
                aggregation: ReserveAggregation::Sum,
            },
            ReserveField {
                id: FE,
                name: "Fe".into(),
                aggregation: ReserveAggregation::WeightedAverage { weight_field: TONNES },
            },
            ReserveField {
                id: ROCK,
                name: "Rock".into(),
                aggregation: ReserveAggregation::Category,
            },
        ]
    }

    /// One dig block. `fe` lists the Fe percentage of each distinct material
    /// in it, and the block's tonnes are split evenly between them - which is
    /// what makes the multi-material split visible in the capture.
    fn block(id: u64, name: &str, tonnes: f64, fe: &[f64]) -> DigBlockRecord {
        let capture = MaterialCapture {
            fields: vec![
                CapturedField {
                    field: TONNES,
                    categorical: false,
                },
                CapturedField { field: FE, categorical: false },
                CapturedField { field: ROCK, categorical: true },
            ],
            labels: vec!["Ore".to_owned()],
            portions: vec![
                fe.iter()
                    .map(|value| MaterialPortion {
                        values: vec![tonnes / fe.len() as f64, *value, 0.0],
                        fraction: 1.0,
                    })
                    .collect(),
            ],
        };
        DigBlockRecord {
            id: DigBlockId(id),
            solid: SolidId(10),
            solid_name: "Pit".into(),
            bench: BenchSelection {
                base: 100.0,
                top: 110.0,
                is_flitch: false,
            },
            flitch: BenchSelection {
                base: 100.0,
                top: 110.0,
                is_flitch: true,
            },
            blast: None,
            name: name.into(),
            plan_area: 100.0,
            source: Default::default(),
            ground: Arc::new(Vec::new()),
            anchor: [0.0, 0.0],
            volume: Some(1000.0),
            replaces: Vec::new(),
            material: MaterialState::CapacityOnly,
            portions: Some(BlockPortions {
                capture: Arc::new(capture),
                slot: 0,
            }),
        }
    }

    fn dig_report(bar: crate::model::schedule::BarId, blocks: &[(DigBlockId, f64)]) -> BarReport {
        BarReport {
            bar,
            members: blocks
                .iter()
                .enumerate()
                .map(|(position, (id, tonnes))| MemberReport {
                    position: position + 1,
                    name: Some(format!("block {}", id.0)),
                    solid_name: Some("Pit".into()),
                    solid_type: Some("Pit".into()),
                    bench: None,
                    blast: None,
                    flitch: None,
                    area_name: None,
                    unresolved: None,
                    tonnes: Some(*tonnes),
                    resolved: Some(*id),
                })
                .collect(),
            reclaim: None,
            tonnes: Some(blocks.iter().map(|(_, tonnes)| tonnes).sum()),
            problems: Vec::new(),
            generation: Some(9),
        }
    }

    fn reclaim_report(bar: crate::model::schedule::BarId) -> BarReport {
        BarReport {
            bar,
            members: Vec::new(),
            reclaim: Some(ReclaimReport {
                source_names: vec![Some("SP01".into())],
            }),
            tonnes: None,
            problems: Vec::new(),
            generation: Some(9),
        }
    }

    /// The reproducible fixture: one pit bar digging two blocks in order, one
    /// reclaim bar on SP01, a crusher with a daily budget, a dump, one truck
    /// class permitted on everything, and grade-dependent routing and
    /// cashflow.
    struct Fixture {
        plan: SchedulePlan,
        destinations: Vec<DestinationView>,
        blocks: Vec<DigBlockRecord>,
        reports: Vec<BarReport>,
        crusher: crate::model::schedule::DestinationId,
        pile: crate::model::schedule::DestinationId,
        dump: crate::model::schedule::DestinationId,
    }

    fn project() -> Fixture {
        let mut plan = SchedulePlan::default();
        plan.set_tonnage_field(Some(TONNES));
        let class = plan.add_class("Excavator", 500.0).expect("class");
        plan.set_class_reclaim_rate(class, 400.0).expect("reclaim rate");
        let agent = plan.add_agent("EX01", class).expect("agent");
        let routing = plan.routing_mut();
        routing.set_enabled(true);
        let pile = crate::model::schedule::DestinationId::Standalone(routing.add_standalone("SP01", Kind::Stockpile).expect("pile"));
        let crusher = crate::model::schedule::DestinationId::Standalone(routing.add_standalone("CR01", Kind::Crusher).expect("crusher"));
        let dump = crate::model::schedule::DestinationId::Standalone(routing.add_standalone("WD01", Kind::Dump).expect("dump"));
        routing.set_standalone_capacity(as_standalone(pile), Some(5_000.0)).expect("pile capacity");
        routing.set_standalone_capacity(as_standalone(dump), Some(50_000.0)).expect("dump capacity");
        routing.set_crusher_default(as_standalone(crusher), Some(6_000.0)).expect("crusher budget");
        routing.set_reclaim_order(pile, ProjectOrder::Fifo).expect("order");
        // Opening stock: 1,000 t at 58% Fe, authored as one lot.
        let lot = routing.add_opening_lot(pile, "Initial ore", 1_000.0).expect("lot");
        let portion = routing
            .inventory(pile)
            .and_then(|inventory| inventory.lot(lot))
            .and_then(|lot| lot.portions.first())
            .map(|portion| portion.id)
            .expect("authored portion");
        routing
            .set_opening_portion_value(pile, lot, portion, FE, Some(crate::model::schedule::OpeningValue::Number(58.0)))
            .expect("opening grade");
        // Ex-pit material may go to the stockpile or the dump; reclaimed
        // material may go to the crusher only when it makes 60% Fe.
        let to_pile = routing.add_rule("Ore to stockpile", vec![pile, dump]).expect("rule");
        // Ex-pit only: a rule left on "any source" would also permit
        // rehandling the pile into itself, which is legitimate and is not
        // what this fixture is about.
        routing
            .set_rule_sources(
                to_pile,
                crate::model::schedule::MovementSourceSelection::Only(vec![crate::model::schedule::MovementSourceScope::Ground(crate::model::schedule::SourceScope::Pit(
                    SolidId(10),
                ))]),
            )
            .expect("sources");
        let to_crusher = routing.add_rule("High-grade reclaim", vec![crusher]).expect("rule");
        routing
            .set_rule_sources(
                to_crusher,
                crate::model::schedule::MovementSourceSelection::Only(vec![crate::model::schedule::MovementSourceScope::Stockpile(pile)]),
            )
            .expect("sources");
        routing
            .set_rule_conditions(
                to_crusher,
                vec![FieldCondition {
                    field: FE,
                    test: ConditionTest::Range {
                        lower: Some(Bound { value: 60.0, inclusive: true }),
                        upper: None,
                    },
                }],
            )
            .expect("conditions");
        let trucks = plan.trucks_mut();
        let truck = trucks.add_class("CAT 793").expect("truck class");
        trucks.set_class_payload(truck, 220.0).expect("payload");
        trucks
            .set_truck_cells(&[crate::model::schedule::TruckCellEdit {
                class: truck,
                cell: crate::model::schedule::CalendarCell::Default,
                field: crate::model::schedule::TruckField::Units,
                value: Some(8.0),
            }])
            .expect("fleet");
        let truck_rule = trucks.add_rule("All routes", vec![truck]).expect("trucking rule");
        trucks.set_rule_destinations(truck_rule, DestinationSelection::All).expect("destinations");
        trucks.set_rule_loaders(truck_rule, LoaderSelection::All).expect("loaders");
        let cashflow = plan.cashflow_mut();
        let crusher_value = cashflow.add_rule("Crusher feed").expect("cashflow rule");
        cashflow.set_rule_value(crusher_value, 60.0).expect("value");
        cashflow
            .set_rule_destinations(crusher_value, DestinationSelection::Only(vec![crusher]))
            .expect("cashflow destinations");
        let haul_cost = cashflow.add_rule("Haulage").expect("cashflow rule");
        cashflow.set_rule_value(haul_cost, -2.0).expect("value");
        // Bars: the dig sequence first, then the reclaim bar in a later lane.
        let dig = plan.add_bar("Pit 100", Some(agent), 0, WorkWindow { start_h: 0.0, end_h: None }).expect("dig bar");
        // The authored dig order: two blocks, in the order they are worked.
        plan.set_bar_members(dig, vec![member(1), member(2)]).expect("dig order");
        let reclaim = plan
            .add_reclaim_bar("Reclaim SP01", Some(agent), 1, WorkWindow { start_h: 0.0, end_h: None }, vec![pile], Some(800.0))
            .expect("reclaim bar");
        let experiment = plan.experiment_mut();
        experiment.set_planning_end_day(1).expect("horizon");
        experiment.set_interval_h(4.0).expect("interval");
        experiment.set_grade_unit(FE, Some(GradeUnit::Percent)).expect("grade unit");
        experiment.set_representation(pile, StockpileRepresentation::Blended).expect("representation");

        let blocks = vec![block(1, "B1", 2_000.0, &[62.0]), block(2, "B2", 1_500.0, &[55.0])];
        let reports = vec![dig_report(dig, &[(DigBlockId(1), 2_000.0), (DigBlockId(2), 1_500.0)]), reclaim_report(reclaim)];
        let destinations = views(&plan);
        Fixture {
            plan,
            destinations,
            blocks,
            reports,
            crusher,
            pile,
            dump,
        }
    }

    /// One authored reference to the fixture's ground. Distinct per block, so
    /// the plan's duplicate-ground check passes.
    fn member(block: u64) -> crate::model::schedule::DigBlockRef {
        crate::model::schedule::DigBlockRef {
            solid: SolidId(10),
            source: Default::default(),
            flitch_base: 100.0 + block as f64,
            flitch_top: 110.0 + block as f64,
            anchor: [block as f64, 0.0],
            plan_area: 100.0,
            footprint: crate::model::schedule::sequence::Footprint([block, block]),
            volume: Some(1000.0),
        }
    }

    fn as_standalone(id: crate::model::schedule::DestinationId) -> crate::model::schedule::StandaloneDestinationId {
        match id {
            crate::model::schedule::DestinationId::Standalone(id) => id,
            crate::model::schedule::DestinationId::Solid(_) => unreachable!("the fixture uses standalone destinations"),
        }
    }

    fn views(plan: &SchedulePlan) -> Vec<DestinationView> {
        crate::model::schedule::destinations::available(&[], plan.routing())
    }

    fn capture(fixture: &Fixture) -> Result<BlendCapture, Vec<CaptureDiagnostic>> {
        let snapshot = CaptureSnapshot::for_test(
            fixture.plan.clone(),
            fields(),
            TONNES,
            fixture.destinations.clone(),
            fixture.blocks.iter().map(clone_block).collect(),
            rebuild_reports(fixture),
        );
        build(&snapshot, &CancelFlag::default())
    }

    fn clone_block(block: &DigBlockRecord) -> DigBlockRecord {
        DigBlockRecord {
            id: block.id,
            solid: block.solid,
            solid_name: block.solid_name.clone(),
            bench: block.bench,
            flitch: block.flitch,
            blast: block.blast,
            name: block.name.clone(),
            plan_area: block.plan_area,
            source: block.source,
            ground: Arc::clone(&block.ground),
            anchor: block.anchor,
            volume: block.volume,
            replaces: block.replaces.clone(),
            material: MaterialState::CapacityOnly,
            portions: block.portions.clone(),
        }
    }

    fn rebuild_reports(fixture: &Fixture) -> Vec<BarReport> {
        let bars = fixture.plan.bars();
        vec![dig_report(bars[0].id, &[(DigBlockId(1), 2_000.0), (DigBlockId(2), 1_500.0)]), reclaim_report(bars[1].id)]
    }

    /// Item 1, 3 and 4: the capture produces the material, resource and rule
    /// inputs the project describes, reconciles the opening inventory and the
    /// captured grade units, and keeps the dig order and bar priority.
    #[test]
    fn project_capture_produces_the_expected_model() {
        let fixture = project();
        let capture = capture(&fixture).unwrap_or_else(|problems| panic!("capture refused: {:?}", problems.iter().map(CaptureDiagnostic::describe).collect::<Vec<_>>()));
        let input = &capture.input;

        // Horizon and calendar: one day at four-hour resolution, plus the
        // day boundary, is six intervals.
        assert_eq!(input.intervals.len(), 6, "intervals: {:?}", input.intervals);
        assert!((input.intervals.last().expect("an interval").end_h - 24.0).abs() < 1e-9);

        // Ground: the measured tonnes, unchanged, and the grades converted
        // from percent to a mass fraction.
        assert_eq!(input.ground.len(), 2);
        assert!((input.ground.iter().map(|source| source.tonnes_t).sum::<f64>() - 3_500.0).abs() < 1e-6);
        let first = input.ground[0].material[0].material;
        assert!((input.grades.fraction(first, 0).expect("Fe") - 0.62).abs() < 1e-12, "Fe was not converted from percent");

        // Opening inventory: 1,000 t at 58%, reconciled against the lot.
        let pile = input.piles.first().expect("a captured pile");
        assert!((pile.opening_t - 1_000.0).abs() < 1e-9);
        assert!((pile.opening_q[0] - 580.0).abs() < 1e-9, "opening contained Fe {}", pile.opening_q[0]);
        assert!(pile.chunks.is_empty(), "a blended pile carries no chunks");

        // Dig order and bar priority are mandatory and survive capture.
        let dig = input.tasks.iter().find(|task| matches!(task.kind, TaskKind::Dig { .. })).expect("a dig task");
        let reclaim = input.tasks.iter().find(|task| matches!(task.kind, TaskKind::Reclaim { .. })).expect("a reclaim task");
        assert_eq!(dig.priority, 0);
        assert_eq!(reclaim.priority, 1);
        let TaskKind::Dig { sequence } = &dig.kind else { unreachable!() };
        assert_eq!(sequence, &vec![GroundId(0), GroundId(1)], "the authored dig order was not preserved");
        let TaskKind::Reclaim { maximum_t, .. } = &reclaim.kind else { unreachable!() };
        assert_eq!(*maximum_t, Some(800.0), "the reclaim cap was not captured");

        // Rules: ex-pit material reaches the pile and the dump but not the
        // crusher; reclaimed material reaches the crusher under a minimum
        // grade the model carries as a row.
        let dense = |project: crate::model::schedule::DestinationId| {
            capture
                .identities
                .destinations
                .iter()
                .find(|(_, id, _)| *id == project)
                .map(|(dense, _, _)| *dense)
                .expect("a captured destination")
        };
        let dig_targets: Vec<_> = input
            .movements
            .iter()
            .filter(|candidate| candidate.activity == Activity::Dig)
            .map(|candidate| candidate.destination)
            .collect();
        assert!(dig_targets.contains(&dense(fixture.pile)) && dig_targets.contains(&dense(fixture.dump)));
        assert!(!dig_targets.contains(&dense(fixture.crusher)), "ex-pit material reached a crusher no rule permits");
        let reclaim_targets: Vec<_> = input
            .movements
            .iter()
            .filter(|candidate| candidate.activity == Activity::Reclaim)
            .map(|candidate| candidate.destination)
            .collect();
        assert_eq!(reclaim_targets, vec![dense(fixture.crusher)]);
        assert_eq!(input.qualifications.len(), 1);
        assert_eq!(input.qualifications[0].destination, dense(fixture.crusher));
        assert_eq!(input.qualifications[0].alternatives[0].bounds[0].lower.unwrap().value, 0.60);

        // Cashflow adds: crusher feed pays 60 and haulage costs 2 everywhere.
        let to_crusher = input
            .movements
            .iter()
            .find(|candidate| candidate.destination == dense(fixture.crusher))
            .expect("a crusher candidate");
        assert!((to_crusher.value_per_tonne().expect("a finite value") - 58.0).abs() < 1e-9);
        let to_dump = input
            .movements
            .iter()
            .find(|candidate| candidate.destination == dense(fixture.dump))
            .expect("a dump candidate");
        assert!((to_dump.value_per_tonne().expect("a finite value") + 2.0).abs() < 1e-9);

        // Trucks: one class, its hours integrated over the interval, and a
        // haulage coefficient derived from the authored payload and speeds.
        assert_eq!(input.trucks.len(), 1);
        assert!((input.trucks[0].hours[0] - 8.0 * 4.0).abs() < 1e-9, "truck hours {:?}", input.trucks[0].hours);
        assert!(to_crusher.truck_hours_per_tonne > 0.0);

        // Crusher budget, per day of the horizon.
        let crusher = input.destinations.iter().find(|entry| entry.id == dense(fixture.crusher)).expect("the crusher");
        assert_eq!(crusher.crusher_daily_t, vec![Some(6_000.0)]);
    }

    /// Item 8: unsupported predicates and missing required grades are
    /// diagnosed against the item to fix, and nothing is silently widened.
    #[test]
    fn unsupported_configuration_is_refused_with_the_item_named() {
        // A category condition applied to reclaim.
        let mut fixture = project();
        let rule = fixture.plan.routing().rules[1].id;
        fixture
            .plan
            .routing_mut()
            .set_rule_conditions(
                rule,
                vec![FieldCondition {
                    field: ROCK,
                    test: ConditionTest::Category { values: vec!["Ore".into()] },
                }],
            )
            .expect_err("a category condition on reclaim must be refused when authored");

        // An upper bound on a reclaim grade.
        let mut fixture = project();
        let rule = fixture.plan.routing().rules[1].id;
        fixture
            .plan
            .routing_mut()
            .set_rule_conditions(
                rule,
                vec![FieldCondition {
                    field: FE,
                    test: ConditionTest::Range {
                        lower: Some(Bound { value: 60.0, inclusive: true }),
                        upper: Some(Bound { value: 70.0, inclusive: true }),
                    },
                }],
            )
            .expect("conditions");
        let captured = capture(&fixture).expect("an upper bound on a tracked grade is supported");
        assert_eq!(captured.input.qualifications[0].alternatives[0].bounds[0].upper.unwrap().value, 0.70);

        // A stockpile with no representation chosen.
        let mut fixture = project();
        fixture
            .plan
            .experiment_mut()
            .set_representation(fixture.pile, StockpileRepresentation::NotConfigured)
            .expect("representation");
        let described: Vec<String> = capture(&fixture)
            .err()
            .expect("an unconfigured stockpile must be refused")
            .iter()
            .map(CaptureDiagnostic::describe)
            .collect();
        assert!(described.iter().any(|entry| entry.starts_with("SP01: choose a stockpile representation")), "{described:?}");

        // A grade with no unit mapping: the opening lot's Fe is then not a
        // tracked grade at all, and the crusher rule can no longer be
        // expressed - which is a refusal, not a widened rule.
        let mut fixture = project();
        fixture.plan.experiment_mut().set_grade_unit(FE, None).expect("grade unit");
        let described: Vec<String> = capture(&fixture)
            .err()
            .expect("an unmapped grade a rule depends on must be refused")
            .iter()
            .map(CaptureDiagnostic::describe)
            .collect();
        assert!(described.iter().any(|entry| entry.contains("not one of this run's blended grades")), "{described:?}");

        // A missing grade on an opening portion is missing, never zero.
        let mut fixture = project();
        let lot = fixture.plan.routing().inventory(fixture.pile).expect("inventory").lots[0].id;
        let portion = fixture.plan.routing().inventory(fixture.pile).expect("inventory").lots[0].portions[0].id;
        fixture
            .plan
            .routing_mut()
            .set_opening_portion_value(fixture.pile, lot, portion, FE, None)
            .expect("clear the grade");
        let described: Vec<String> = capture(&fixture)
            .err()
            .expect("a missing opening grade must be refused")
            .iter()
            .map(CaptureDiagnostic::describe)
            .collect();
        assert!(described.iter().any(|entry| entry.contains("Initial ore") && entry.contains("missing")), "{described:?}");
    }

    /// Item 6: a presentation-only rename does not change what the model is,
    /// and a coefficient edit does.
    #[test]
    fn renames_do_not_change_the_model_identity_and_coefficients_do() {
        let fixture = project();
        let before = capture(&fixture).expect("capture").fingerprint;

        let mut renamed = project();
        renamed
            .plan
            .routing_mut()
            .rename_standalone(as_standalone(renamed.pile), "Run-of-mine stockpile")
            .expect("rename");
        renamed.plan.rename_agent(renamed.plan.agents()[0].id, "EX02").expect("rename");
        let bar = renamed.plan.bars()[0].id;
        renamed.plan.rename_bar(bar, "North pit").expect("rename");
        renamed.plan.set_currency("AUD").expect("currency");
        renamed.destinations = views(&renamed.plan);
        assert_eq!(capture(&renamed).expect("capture").fingerprint, before, "a rename changed the model identity");

        let mut edited = project();
        let rule = edited.plan.cashflow().rules[0].id;
        edited.plan.cashflow_mut().set_rule_value(rule, 61.0).expect("value");
        assert_ne!(
            capture(&edited).expect("capture").fingerprint,
            before,
            "a cashflow coefficient did not change the model identity"
        );

        let mut retimed = project();
        retimed.plan.experiment_mut().set_interval_h(2.0).expect("interval");
        assert_ne!(
            capture(&retimed).expect("capture").fingerprint,
            before,
            "the calendar resolution did not change the model identity"
        );
    }

    /// The chunked representation maps authored lots to closed opening chunks
    /// and the configured capacities to receiving chunks behind them, and a
    /// block holding several materials stays one block made of proportions.
    #[test]
    fn chunked_piles_and_mixed_blocks_are_captured_and_reported() {
        let mut fixture = project();
        fixture
            .plan
            .experiment_mut()
            .set_representation(fixture.pile, StockpileRepresentation::Chunks)
            .expect("representation");
        fixture.plan.experiment_mut().set_receiving_chunks(fixture.pile, vec![800.0, 800.0]).expect("chunks");
        // One block now holds two materials of different grade.
        fixture.blocks = vec![block(1, "B1", 2_000.0, &[62.0, 50.0]), block(2, "B2", 1_500.0, &[55.0])];
        let capture = capture(&fixture).unwrap_or_else(|problems| panic!("capture refused: {:?}", problems.iter().map(CaptureDiagnostic::describe).collect::<Vec<_>>()));

        let pile = capture.input.piles.first().expect("a captured pile");
        assert_eq!(pile.chunks.len(), 3, "one opening lot and two receiving chunks");
        assert!((pile.chunk_opening[0].0 - 1_000.0).abs() < 1e-9, "the opening lot did not become a closed chunk");
        assert_eq!(pile.chunk_opening[1], (0.0, vec![0.0]));
        assert!((pile.chunk_opening[0].1[0] - 580.0).abs() < 1e-9);

        assert_eq!(capture.stats.mixed_blocks, 1);
        assert_eq!(capture.input.ground.len(), 2, "a mixed block must stay one physical dig block");
        let mixed = &capture.input.ground[0];
        assert_eq!(mixed.material.len(), 2, "both captured materials must be carried as shares of the one block");
        assert!((mixed.tonnes_t - 2_000.0).abs() < 1e-9, "the block lost its measured tonnes: {}", mixed.tonnes_t);
        for share in &mixed.material {
            assert!((share.fraction - 0.5).abs() < 1e-12, "share {} is not the measured proportion", share.fraction);
        }
        assert!((mixed.material.iter().map(|share| share.fraction).sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(
            capture.notes.iter().any(|note| note.contains("measured proportions")),
            "the mixed block was not reported: {:?}",
            capture.notes
        );
        let dig = capture.input.tasks.iter().find(|task| matches!(task.kind, TaskKind::Dig { .. })).expect("a dig task");
        let TaskKind::Dig { sequence } = &dig.kind else { unreachable!() };
        assert_eq!(sequence, &vec![GroundId(0), GroundId(1)], "the authored block order was not preserved");
    }

    /// The mixed-block acceptance case.
    ///
    /// One block holds an economically attractive portion (62% Fe, which a
    /// rule sends straight to the crusher at +60/t) and an unattractive one
    /// (50% Fe, which only the dump accepts at -20/t). The loader's rate is
    /// cut so the horizon stops it partway through that block. The solver
    /// must not take only the attractive half, and what it leaves behind must
    /// still be the block's measured composition.
    #[cfg(feature = "scip-code")]
    #[test]
    fn a_mixed_block_is_mined_in_proportion_even_when_only_part_of_it_pays() {
        use std::time::Duration;

        use crate::app::scip_blend::{ScipActivity, ScipRunIdentity, ScipSolveOptions, execute_scip_blend};

        let mut fixture = project();
        // Partway: 50 t/h over a 24 h horizon cannot finish a 2,000 t block.
        let class = fixture.plan.classes().first().expect("the fixture class").id;
        fixture.plan.set_class_rate(class, 50.0).expect("dig rate");
        // The attractive half goes straight to the crusher; the other half
        // has nowhere to go but the dump, and costs money to put there.
        let routing = fixture.plan.routing_mut();
        let direct = routing.add_rule("Ore direct to crusher", vec![fixture.crusher]).expect("rule");
        routing
            .set_rule_sources(
                direct,
                crate::model::schedule::MovementSourceSelection::Only(vec![crate::model::schedule::MovementSourceScope::Ground(crate::model::schedule::SourceScope::Pit(
                    SolidId(10),
                ))]),
            )
            .expect("sources");
        routing
            .set_rule_conditions(
                direct,
                vec![FieldCondition {
                    field: FE,
                    test: ConditionTest::Range {
                        lower: Some(Bound { value: 60.0, inclusive: true }),
                        upper: None,
                    },
                }],
            )
            .expect("conditions");
        let cashflow = fixture.plan.cashflow_mut();
        let waste = cashflow.add_rule("Waste handling").expect("cashflow rule");
        cashflow.set_rule_value(waste, -20.0).expect("value");
        cashflow
            .set_rule_destinations(waste, DestinationSelection::Only(vec![fixture.dump]))
            .expect("cashflow destinations");
        fixture.destinations = views(&fixture.plan);
        fixture.blocks = vec![block(1, "B1", 2_000.0, &[62.0, 50.0]), block(2, "B2", 1_500.0, &[55.0])];

        let captured = capture(&fixture).unwrap_or_else(|problems| panic!("capture refused: {:?}", problems.iter().map(CaptureDiagnostic::describe).collect::<Vec<_>>()));
        let attractive = captured.input.ground[0].material[0].material;
        let unattractive = captured.input.ground[0].material[1].material;
        let input = Arc::new(captured.input);
        let identity = ScipRunIdentity {
            run_id: 1,
            inputs: crate::app::schedule_pipeline::ScheduleRunInputs {
                runtime: 1,
                generation: 9,
                fleet_revision: 3,
                tonnage_field: TONNES,
            },
            plan_revision: 5,
            document_revision: 1,
            semantic: captured.fingerprint,
        };
        let options = ScipSolveOptions {
            time_limit: Some(Duration::from_secs(60)),
            ..Default::default()
        };
        let result = execute_scip_blend(Arc::clone(&input), identity, options, &CancelFlag::default(), &ScipActivity::default());
        assert!(result.usable(), "the mixed block did not solve: {:?} / {:?}", result.termination, result.diagnostic);
        let replay = result.replay.as_ref().expect("a replay");
        assert!(replay.is_valid(), "replay issues: {:?} / {:?}", replay.issues, replay.grade_issues);

        let solution = result.solution.as_ref().expect("an incumbent");
        let mut dug = [0.0_f64; 2];
        for row in &solution.movements {
            let candidate = &input.movements[row.candidate];
            if candidate.activity != Activity::Dig || candidate.source != SourceId::Ground(GroundId(0)) {
                continue;
            }
            if candidate.material == attractive {
                dug[0] += row.tonnes_t;
            } else if candidate.material == unattractive {
                dug[1] += row.tonnes_t;
            }
        }
        let total = dug[0] + dug[1];
        assert!(total > 1.0, "nothing was mined, so the case proves nothing");
        assert!(total < 2_000.0 - 1.0, "the block finished, so nothing was stopped partway: {total}");
        assert!(
            (dug[0] - dug[1]).abs() < 1e-4,
            "the solver took {:.3} t of the attractive material against {:.3} t of the rest",
            dug[0],
            dug[1]
        );
        // The remaining block is still half and half, which is the same fact
        // stated about what was left rather than what was taken.
        let left = [2_000.0 * 0.5 - dug[0], 2_000.0 * 0.5 - dug[1]];
        assert!((left[0] - left[1]).abs() < 1e-4, "the remaining block changed composition: {left:?}");
        println!("mixed block: {total:.1} t mined of 2,000 t, {:.1} t attractive / {:.1} t not", dug[0], dug[1]);
    }

    /// Item 2: the worker resolves the captured model, solves it and returns
    /// a replay-valid result - the whole path, minus only the job queue.
    ///
    /// Also item 3 in its strongest form: the tonnes and grades the answer
    /// moves are reconciled against the project's own figures.
    #[cfg(feature = "scip-code")]
    #[test]
    fn the_captured_model_solves_and_replays_against_the_project_figures() {
        use std::time::Duration;

        use crate::app::scip_blend::{ScipActivity, ScipRunIdentity, ScipSolveOptions, ScipTermination, execute_scip_blend, totals};

        let fixture = project();
        let captured = capture(&fixture).unwrap_or_else(|problems| panic!("capture refused: {:?}", problems.iter().map(CaptureDiagnostic::describe).collect::<Vec<_>>()));
        let input = Arc::new(captured.input);
        let identity = ScipRunIdentity {
            run_id: 1,
            inputs: crate::app::schedule_pipeline::ScheduleRunInputs {
                runtime: 1,
                generation: 9,
                fleet_revision: 3,
                tonnage_field: TONNES,
            },
            plan_revision: 5,
            document_revision: 1,
            semantic: captured.fingerprint,
        };
        let options = ScipSolveOptions {
            time_limit: Some(Duration::from_secs(30)),
            ..Default::default()
        };
        let result = execute_scip_blend(Arc::clone(&input), identity, options, &CancelFlag::default(), &ScipActivity::default());
        assert!(result.usable(), "the captured project did not solve: {:?} / {:?}", result.termination, result.diagnostic);
        assert_eq!(result.termination, ScipTermination::Optimal);
        let replay = result.replay.as_ref().expect("a replay");
        assert!(replay.is_valid(), "replay issues: {:?} / {:?}", replay.issues, replay.grade_issues);

        let solution = result.solution.as_ref().expect("an incumbent");
        let moved = totals(&input, solution);
        // Nothing may be mined that the ground did not hold, and nothing may
        // be reclaimed beyond the bar's authored cap.
        assert!(moved.mined_t <= 3_500.0 + 1e-6, "mined {}", moved.mined_t);
        assert!(moved.reclaimed_t <= 800.0 + 1e-6, "reclaimed {} beyond the 800 t cap", moved.reclaimed_t);
        // Everything that reached the crusher came out of the pile, and the
        // pile only qualifies once 62% ore has lifted the opening 58% blend
        // past the rule's 60% minimum.
        assert!((moved.processed_t - moved.reclaimed_t).abs() < 1e-6);
        let closing = replay.closing.first().expect("a closing pile");
        assert!(closing.tonnes_t <= 5_000.0 + 1e-6, "the pile closed above its capacity at {}", closing.tonnes_t);
        if moved.processed_t > 1e-6 {
            let delivered_grade = input.qualifications[0].alternatives[0].bounds[0].lower.unwrap().value;
            assert!((delivered_grade - 0.60).abs() < 1e-12);
        }
        println!(
            "captured project: mined {:.1} t, reclaimed {:.1} t, processed {:.1} t, objective {:?}, closing {:.1} t",
            moved.mined_t, moved.reclaimed_t, moved.processed_t, result.published_objective, closing.tonnes_t
        );
    }
}
