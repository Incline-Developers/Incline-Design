//! Whole-horizon scheduling inputs and results.
//!
//! This module is deliberately calculation-owned. Stage 5 will resolve the
//! mutable project into these stable ids, fixed material records and feasible
//! route candidates before starting a job. The solver never consults the
//! project again, and its output contains every choice needed for publication.
//!
//! # Calendar intervals and execution segments
//!
//! Calendar intervals ([`Interval`]) are where input settings are constant:
//! rates, fleet availability, bar-window eligibility and daily budget
//! boundaries. They carry no execution semantics of their own. Execution is
//! modelled inside each interval as an ordered sequence of shared *segments* -
//! event positions all loaders agree on - whose durations are optimiser
//! decisions. A loader works one source per segment and may finish a block,
//! lot or parcel and continue to the next within the same interval. Unused
//! segments take zero duration and publish nothing.

#![allow(dead_code, reason = "Stage 5 captures and publishes these Stage 4 contracts")]

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
mod highs;

/// The experimental blended-stockpile model: scenario contract, independent
/// replay, and the iterative fixed-grade HiGHS solver for it. Off by default
/// and never reachable from Run Schedule; see the module docs for what it
/// does and does not claim.
#[cfg(all(not(target_arch = "wasm32"), feature = "blend-experiment"))]
pub(crate) mod blended;

/// The nonlinear SCIP solver for that same blended model. Off by default, and
/// separate from [`blended`] because it is the only part that needs SCIP.
#[cfg(all(not(target_arch = "wasm32"), feature = "scip-code"))]
pub(crate) mod scip;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) struct $name(pub(crate) u32);
    };
}

id_type!(LoaderId);
id_type!(TaskId);
id_type!(GroundId);
id_type!(StockpileId);
id_type!(LotId);
id_type!(MaterialId);
id_type!(DestinationId);
id_type!(TruckClassId);
id_type!(RoutingRuleId);
id_type!(CashflowRuleId);

/// Numerical rules are part of the calculation metadata, rather than hidden
/// solver constants. They scale with the authored model where appropriate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NumericalTolerances {
    pub(crate) tonnes_t: f64,
    pub(crate) objective: f64,
    pub(crate) integrality: f64,
    pub(crate) parcel_occupancy_t: f64,
}

impl Default for NumericalTolerances {
    fn default() -> Self {
        Self {
            tonnes_t: 1e-6,
            objective: 1e-6,
            integrality: 1e-6,
            parcel_occupancy_t: 1e-5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HorizonSpec {
    pub(crate) end_h: f64,
    pub(crate) regular_step_h: f64,
    pub(crate) parcel_target_t: f64,
    /// Overrides the data-derived per-interval execution-segment budget. The
    /// derived budget is normally sufficient by construction; an override
    /// exists so a proof can repeat a fixture with more event positions and
    /// confirm the budget was not restricting transitions.
    pub(crate) event_segments: Option<u32>,
    pub(crate) tolerances: NumericalTolerances,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Interval {
    pub(crate) index: usize,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
}

impl Interval {
    pub(crate) fn duration_h(self) -> f64 {
        self.end_h - self.start_h
    }
    pub(crate) fn day(self) -> u32 {
        (self.start_h / 24.0).floor() as u32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InputError {
    NonFinite(&'static str),
    NonPositive(&'static str),
    DuplicateId(&'static str),
    MissingReference(&'static str),
    InvalidWindow,
    InvalidComposition,
    InvalidIntervals,
    NoRoute { task: TaskId, material: MaterialId },
    UnboundedInput(&'static str),
}

/// Split a finite horizon at its regular grid, every midnight, every authored
/// window edge and every supplied calendar change. Near-identical boundaries
/// collapse using the configured tonnes-independent time tolerance.
pub(crate) fn build_intervals(spec: HorizonSpec, windows: &[(f64, f64)], calendar_changes_h: &[f64]) -> Result<Vec<Interval>, InputError> {
    if !spec.end_h.is_finite() || spec.end_h <= 0.0 {
        return Err(InputError::NonPositive("horizon"));
    }
    if !spec.regular_step_h.is_finite() || spec.regular_step_h <= 0.0 {
        return Err(InputError::NonPositive("interval step"));
    }
    if !spec.parcel_target_t.is_finite() || spec.parcel_target_t <= 0.0 {
        return Err(InputError::NonPositive("parcel target"));
    }
    let mut points = vec![0.0, spec.end_h];
    let add_regular = |step: f64, points: &mut Vec<f64>| {
        let mut at = step;
        while at < spec.end_h {
            points.push(at);
            at += step;
        }
    };
    add_regular(spec.regular_step_h, &mut points);
    add_regular(24.0, &mut points);
    for &(start, end) in windows {
        if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
            return Err(InputError::InvalidWindow);
        }
        if start < spec.end_h {
            points.push(start);
        }
        if end < spec.end_h {
            points.push(end);
        }
    }
    for &at in calendar_changes_h {
        if !at.is_finite() || at < 0.0 {
            return Err(InputError::InvalidIntervals);
        }
        if at > 0.0 && at < spec.end_h {
            points.push(at);
        }
    }
    points.sort_by(f64::total_cmp);
    points.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
    Ok(points
        .windows(2)
        .enumerate()
        .map(|(index, edge)| Interval {
            index,
            start_h: edge[0],
            end_h: edge[1],
        })
        .collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Activity {
    Dig,
    Reclaim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReclaimOrder {
    Fifo,
    Lifo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SourceId {
    Ground(GroundId),
    Stockpile(StockpileId),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Material {
    pub(crate) id: MaterialId,
    /// Stable field ids and their already-mapped values are retained by Stage 3.
    /// Route and cashflow matching is completed during capture, so the core
    /// needs identity rather than a second condition engine.
    pub(crate) label: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MaterialShare {
    pub(crate) material: MaterialId,
    pub(crate) fraction: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GroundSource {
    pub(crate) id: GroundId,
    pub(crate) tonnes_t: f64,
    pub(crate) material: Vec<MaterialShare>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OpeningLot {
    pub(crate) id: LotId,
    pub(crate) tonnes_t: f64,
    pub(crate) material: Vec<MaterialShare>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stockpile {
    pub(crate) id: StockpileId,
    pub(crate) capacity_t: f64,
    /// Oldest to newest. Opening lots precede every generated receipt parcel.
    pub(crate) opening: Vec<OpeningLot>,
    pub(crate) order: ReclaimOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DestinationKind {
    Crusher,
    Dump,
    Stockpile(StockpileId),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Destination {
    pub(crate) id: DestinationId,
    pub(crate) kind: DestinationKind,
    /// Finite for dumps and stockpiles. Crushers use the daily budget instead.
    pub(crate) capacity_t: Option<f64>,
    /// One entry per day touched by the horizon. `None` is unlimited.
    pub(crate) crusher_daily_t: Vec<Option<f64>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IntervalRate {
    pub(crate) interval: usize,
    pub(crate) dig_tph: f64,
    pub(crate) reclaim_tph: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Loader {
    pub(crate) id: LoaderId,
    pub(crate) rates: Vec<IntervalRate>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TaskKind {
    Dig {
        sequence: Vec<GroundId>,
    },
    /// A vector now, although Stage 3 captures exactly one approved pile.
    Reclaim {
        approved_sources: Vec<StockpileId>,
        maximum_t: Option<f64>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Task {
    pub(crate) id: TaskId,
    pub(crate) loader: LoaderId,
    pub(crate) priority: u32,
    pub(crate) window_start_h: f64,
    pub(crate) window_end_h: f64,
    pub(crate) kind: TaskKind,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TruckClass {
    pub(crate) id: TruckClassId,
    /// Available shared truck-hours by interval. Segments receive their
    /// duration-proportional share of this budget: the effective fleet is
    /// `hours / interval duration` trucks at every instant of the interval.
    pub(crate) hours: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CashflowContribution {
    pub(crate) rule: CashflowRuleId,
    pub(crate) value_per_tonne: f64,
}

/// One feasible destination/truck combination. Capture creates these only when
/// the routing, trucking and cashflow selectors all resolve. Different truck
/// classes are separate candidates sharing the same physical movement.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MovementCandidate {
    pub(crate) loader: LoaderId,
    pub(crate) activity: Activity,
    pub(crate) source: SourceId,
    pub(crate) material: MaterialId,
    pub(crate) destination: DestinationId,
    pub(crate) truck: TruckClassId,
    pub(crate) truck_hours_per_tonne: f64,
    pub(crate) routing_rule: RoutingRuleId,
    pub(crate) routing_preference: u32,
    pub(crate) cashflow: Vec<CashflowContribution>,
}

impl MovementCandidate {
    pub(crate) fn value_per_tonne(&self) -> Result<f64, InputError> {
        let value = self
            .cashflow
            .iter()
            .try_fold(0.0_f64, |sum, item| {
                let next = sum + item.value_per_tonne;
                next.is_finite().then_some(next)
            })
            .ok_or(InputError::NonFinite("cashflow sum"))?;
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OptimisationInput {
    pub(crate) horizon: HorizonSpec,
    pub(crate) intervals: Vec<Interval>,
    pub(crate) materials: Vec<Material>,
    pub(crate) loaders: Vec<Loader>,
    pub(crate) tasks: Vec<Task>,
    pub(crate) ground: Vec<GroundSource>,
    pub(crate) stockpiles: Vec<Stockpile>,
    pub(crate) destinations: Vec<Destination>,
    pub(crate) trucks: Vec<TruckClass>,
    pub(crate) movements: Vec<MovementCandidate>,
}

fn unique<T: Ord + Copy>(mut values: impl Iterator<Item = T>) -> bool {
    let mut held = BTreeSet::new();
    values.all(|value| held.insert(value))
}

fn validate_shares(shares: &[MaterialShare], materials: &BTreeSet<MaterialId>, tolerance: f64) -> Result<(), InputError> {
    if shares.is_empty() || !unique(shares.iter().map(|share| share.material)) {
        return Err(InputError::InvalidComposition);
    }
    let mut total = 0.0;
    for share in shares {
        if !materials.contains(&share.material) {
            return Err(InputError::MissingReference("material"));
        }
        if !share.fraction.is_finite() || share.fraction <= 0.0 {
            return Err(InputError::InvalidComposition);
        }
        total += share.fraction;
    }
    if !total.is_finite() || (total - 1.0).abs() > tolerance {
        return Err(InputError::InvalidComposition);
    }
    Ok(())
}

impl OptimisationInput {
    pub(crate) fn validate(&self) -> Result<(), InputError> {
        if self.intervals.is_empty() {
            return Err(InputError::InvalidIntervals);
        }
        let mut expected = 0.0;
        for (index, interval) in self.intervals.iter().enumerate() {
            if interval.index != index || (interval.start_h - expected).abs() > 1e-9 || interval.end_h <= interval.start_h || !interval.end_h.is_finite() {
                return Err(InputError::InvalidIntervals);
            }
            expected = interval.end_h;
        }
        if (expected - self.horizon.end_h).abs() > 1e-9 {
            return Err(InputError::InvalidIntervals);
        }
        if !unique(self.materials.iter().map(|entry| entry.id)) {
            return Err(InputError::DuplicateId("material"));
        }
        if !unique(self.loaders.iter().map(|entry| entry.id)) {
            return Err(InputError::DuplicateId("loader"));
        }
        if !unique(self.tasks.iter().map(|entry| entry.id)) {
            return Err(InputError::DuplicateId("task"));
        }
        if !unique(self.ground.iter().map(|entry| entry.id)) {
            return Err(InputError::DuplicateId("ground"));
        }
        if !unique(self.stockpiles.iter().map(|entry| entry.id)) {
            return Err(InputError::DuplicateId("stockpile"));
        }
        if !unique(self.destinations.iter().map(|entry| entry.id)) {
            return Err(InputError::DuplicateId("destination"));
        }
        if !unique(self.trucks.iter().map(|entry| entry.id)) {
            return Err(InputError::DuplicateId("truck"));
        }
        let materials: BTreeSet<_> = self.materials.iter().map(|entry| entry.id).collect();
        let loaders: BTreeSet<_> = self.loaders.iter().map(|entry| entry.id).collect();
        let ground: BTreeSet<_> = self.ground.iter().map(|entry| entry.id).collect();
        let piles: BTreeSet<_> = self.stockpiles.iter().map(|entry| entry.id).collect();
        let destinations: BTreeSet<_> = self.destinations.iter().map(|entry| entry.id).collect();
        let trucks: BTreeSet<_> = self.trucks.iter().map(|entry| entry.id).collect();
        for source in &self.ground {
            if !source.tonnes_t.is_finite() || source.tonnes_t <= 0.0 {
                return Err(InputError::NonPositive("ground tonnes"));
            }
            validate_shares(&source.material, &materials, self.horizon.tolerances.tonnes_t)?;
        }
        for pile in &self.stockpiles {
            if !pile.capacity_t.is_finite() || pile.capacity_t < 0.0 {
                return Err(InputError::NonFinite("stockpile capacity"));
            }
            if !unique(pile.opening.iter().map(|lot| lot.id)) {
                return Err(InputError::DuplicateId("opening lot"));
            }
            let mut total = 0.0;
            for lot in &pile.opening {
                if !lot.tonnes_t.is_finite() || lot.tonnes_t <= 0.0 {
                    return Err(InputError::NonPositive("opening lot"));
                }
                validate_shares(&lot.material, &materials, self.horizon.tolerances.tonnes_t)?;
                total += lot.tonnes_t;
            }
            if total > pile.capacity_t + self.horizon.tolerances.tonnes_t {
                return Err(InputError::UnboundedInput("opening inventory exceeds capacity"));
            }
        }
        for loader in &self.loaders {
            if loader.rates.len() != self.intervals.len() {
                return Err(InputError::InvalidIntervals);
            }
            for (index, rate) in loader.rates.iter().enumerate() {
                if rate.interval != index || !rate.dig_tph.is_finite() || rate.dig_tph < 0.0 || !rate.reclaim_tph.is_finite() || rate.reclaim_tph < 0.0 {
                    return Err(InputError::NonFinite("loader rate"));
                }
            }
        }
        for task in &self.tasks {
            if !loaders.contains(&task.loader) {
                return Err(InputError::MissingReference("task loader"));
            }
            if !task.window_start_h.is_finite() || !task.window_end_h.is_finite() || task.window_start_h < 0.0 || task.window_end_h <= task.window_start_h {
                return Err(InputError::InvalidWindow);
            }
            match &task.kind {
                TaskKind::Dig { sequence } if sequence.is_empty() || sequence.iter().any(|id| !ground.contains(id)) => return Err(InputError::MissingReference("dig source")),
                TaskKind::Reclaim { approved_sources, maximum_t } => {
                    if approved_sources.is_empty() || approved_sources.iter().any(|id| !piles.contains(id)) {
                        return Err(InputError::MissingReference("reclaim source"));
                    }
                    if maximum_t.is_some_and(|value| !value.is_finite() || value <= 0.0) {
                        return Err(InputError::NonPositive("reclaim maximum"));
                    }
                }
                TaskKind::Dig { .. } => {}
            }
        }
        for destination in &self.destinations {
            if destination.capacity_t.is_some_and(|value| !value.is_finite() || value < 0.0) {
                return Err(InputError::NonFinite("destination capacity"));
            }
            if let DestinationKind::Stockpile(id) = destination.kind
                && (!piles.contains(&id) || destination.capacity_t.is_none())
            {
                return Err(InputError::MissingReference("destination stockpile"));
            }
            if destination.crusher_daily_t.iter().flatten().any(|value| !value.is_finite() || *value < 0.0) {
                return Err(InputError::NonFinite("crusher limit"));
            }
        }
        for truck in &self.trucks {
            if truck.hours.len() != self.intervals.len() || truck.hours.iter().any(|value| !value.is_finite() || *value < 0.0) {
                return Err(InputError::InvalidIntervals);
            }
        }
        for movement in &self.movements {
            if !loaders.contains(&movement.loader) || !materials.contains(&movement.material) || !destinations.contains(&movement.destination) || !trucks.contains(&movement.truck)
            {
                return Err(InputError::MissingReference("movement"));
            }
            match movement.source {
                SourceId::Ground(id) if !ground.contains(&id) => return Err(InputError::MissingReference("movement source")),
                SourceId::Stockpile(id) if !piles.contains(&id) => return Err(InputError::MissingReference("movement source")),
                _ => {}
            }
            if !movement.truck_hours_per_tonne.is_finite() || movement.truck_hours_per_tonne <= 0.0 {
                return Err(InputError::NonPositive("truck coefficient"));
            }
            movement.value_per_tonne()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SolveLimits {
    pub(crate) time: Option<Duration>,
    pub(crate) relative_gap: Option<f64>,
    pub(crate) threads: Option<u32>,
    pub(crate) nodes: Option<u64>,
    pub(crate) solutions: Option<u64>,
}

impl Default for SolveLimits {
    fn default() -> Self {
        Self {
            time: None,
            relative_gap: Some(1e-6),
            threads: None,
            nodes: None,
            solutions: None,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SolveStatus {
    Optimal,
    FeasibleLimit,
    Infeasible,
    LimitNoIncumbent,
    Unbounded,
    Cancelled,
    BackendFailure,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SolveStatistics {
    pub(crate) variables: usize,
    pub(crate) binaries: usize,
    pub(crate) constraints: usize,
    /// Time spent building the formulation, separate from the backend solve.
    pub(crate) formulation_time: Duration,
    pub(crate) solve_time: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SolveSummary {
    pub(crate) status: SolveStatus,
    /// The published objective: the value of the rows the result actually
    /// carries, after negligible-tonnage cleanup.
    pub(crate) objective: Option<f64>,
    /// The objective HiGHS reported for its own (uncleaned) solution, kept so
    /// postprocessing changes can never silently pass as solver truth.
    pub(crate) raw_solver_objective: Option<f64>,
    pub(crate) best_bound: Option<f64>,
    pub(crate) relative_gap: Option<f64>,
    pub(crate) backend: &'static str,
    pub(crate) message: Option<String>,
    /// Whether the tie-preference stage ran to a solution. Preferences are a
    /// second lexicographic stage held above the primary objective, so a
    /// false flag means the published rows are primary-optimal but the
    /// preference ordering among equally valuable alternatives was not
    /// applied - a time-budget outcome, never a correctness one.
    pub(crate) tie_preferences_applied: bool,
    pub(crate) statistics: SolveStatistics,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ActivityRecord {
    pub(crate) interval: usize,
    pub(crate) segment: usize,
    pub(crate) loader: LoaderId,
    pub(crate) task: TaskId,
    pub(crate) activity: Activity,
    pub(crate) source: SourceId,
    pub(crate) tonnes_t: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MovementRecord {
    pub(crate) interval: usize,
    pub(crate) segment: usize,
    pub(crate) loader: LoaderId,
    pub(crate) task: TaskId,
    pub(crate) source: SourceId,
    pub(crate) material: MaterialId,
    pub(crate) destination: DestinationId,
    pub(crate) truck: TruckClassId,
    pub(crate) tonnes_t: f64,
    pub(crate) truck_hours: f64,
    pub(crate) routing_rule: RoutingRuleId,
    pub(crate) cashflow: Vec<CashflowContribution>,
    pub(crate) value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReceiptParcel {
    pub(crate) stockpile: StockpileId,
    pub(crate) receipt_interval: usize,
    /// The execution segment of the receipt interval whose supply created the
    /// parcel. Receipt order is (interval, segment, position).
    pub(crate) segment: usize,
    pub(crate) release_interval: usize,
    pub(crate) position: usize,
    pub(crate) source: SourceId,
    pub(crate) material: MaterialId,
    pub(crate) tonnes_t: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum InventoryUnit {
    Opening(LotId),
    Receipt { interval: usize, segment: usize, position: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReclaimRecord {
    pub(crate) interval: usize,
    pub(crate) segment: usize,
    pub(crate) loader: LoaderId,
    pub(crate) task: TaskId,
    pub(crate) stockpile: StockpileId,
    pub(crate) unit: InventoryUnit,
    pub(crate) tonnes_t: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BalanceRecord {
    pub(crate) interval: usize,
    pub(crate) ground_remaining_t: BTreeMap<GroundId, f64>,
    pub(crate) stockpile_closing_t: BTreeMap<StockpileId, f64>,
    pub(crate) destination_cumulative_t: BTreeMap<DestinationId, f64>,
    pub(crate) crusher_daily_used_t: BTreeMap<(DestinationId, u32), f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolutionMetadata {
    pub(crate) horizon_h: f64,
    pub(crate) regular_step_h: f64,
    pub(crate) parcel_target_t: f64,
    /// The execution-segment budget granted to each calendar interval.
    pub(crate) segment_counts: Vec<usize>,
    /// The ceiling that clamps the segment budget: the derived guard or the
    /// clamped explicit override. A budget equal to the ceiling may be
    /// restricting transitions; re-solve with a larger override to check.
    pub(crate) segment_ceiling: usize,
    pub(crate) interval_edges_h: Vec<f64>,
    /// The derived negligible-inventory scale: below this an inventory unit
    /// cannot be distinguished from empty by the formulation, because a
    /// nearly-integral emptiness binary can hide that much against the row's
    /// big-M. The model, the publication snapping and the validator all use
    /// it, so a published unit above it is live everywhere.
    pub(crate) inventory_dust_t: f64,
    pub(crate) tolerances: NumericalTolerances,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OptimisationResult {
    pub(crate) summary: SolveSummary,
    pub(crate) resolution: ResolutionMetadata,
    /// Decided duration in hours of every execution segment, per interval.
    pub(crate) segment_durations_h: Vec<Vec<f64>>,
    /// Segments per interval with positive duration.
    pub(crate) used_segments: Vec<usize>,
    /// Heuristic flag: some loader was still working its authored source in an
    /// interval's final segment with that source unfinished, so a larger event
    /// budget might have found more within-interval transitions.
    ///
    /// Three things stay distinct and must not be conflated when reporting a
    /// run: [`SolveSummary::status`], which is the solver's verdict on the
    /// model as encoded; this flag, which says the encoding's event budget
    /// may have restricted it; and the discretisation the encoding rests on
    /// at all - calendar interval length and parcel target - which nothing
    /// here measures. A clear flag is not proof the budget did not bind;
    /// [`solve_within_event_budget`] spends a bounded amount of a caller's
    /// wall time trying to disprove a set one.
    pub(crate) event_budget_restricted: bool,
    /// Quantified normalisation record for this publication.
    pub(crate) publication_audit: PublicationAudit,
    pub(crate) activities: Vec<ActivityRecord>,
    pub(crate) movements: Vec<MovementRecord>,
    pub(crate) parcels: Vec<ReceiptParcel>,
    pub(crate) reclaims: Vec<ReclaimRecord>,
    pub(crate) balances: Vec<BalanceRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ValidationIssue {
    pub(crate) message: String,
    pub(crate) magnitude: f64,
}

/// Quantified record of the publication normalisation passes: what the
/// dig/reclaim/parcel snapping changed, the largest excess of the published
/// timeline against any original hard capacity afterwards, and how far the
/// published objective moved from the solver's own value. Self-consistency
/// of the published rows is necessary but not sufficient; this audit bounds
/// the aggregate distance between what was solved and what is published.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PublicationAudit {
    pub(crate) dig_rows_snapped: usize,
    pub(crate) dig_max_adjustment_t: f64,
    pub(crate) dig_total_adjustment_t: f64,
    pub(crate) reclaim_rows_snapped: usize,
    pub(crate) reclaim_max_adjustment_t: f64,
    pub(crate) reclaim_total_adjustment_t: f64,
    /// Parcel publication adjustments: snapping full parcels to the target,
    /// merging dust into the stream's last parcel, and dropping negligible
    /// parcels entirely.
    pub(crate) parcel_max_adjustment_t: f64,
    pub(crate) parcel_total_adjustment_t: f64,
    /// Largest positive excess of the published timeline against any original
    /// hard bound: loader rate × segment duration, per-segment fleet share,
    /// crusher daily budget, dump capacity, stockpile capacity at segment
    /// ends, and reclaim bar maxima.
    pub(crate) max_capacity_excess_t: f64,
    /// The solver's objective for its raw solution minus the published
    /// objective; the reported best bound and gap remain solver-side and
    /// therefore describe the raw solution, not this adjusted figure.
    pub(crate) objective_adjustment: f64,
}

/// Stage 4's public native entry point. The browser retains the contracts and
/// can load projects, but intentionally has no solver backend.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn solve(input: &OptimisationInput, limits: SolveLimits, cancellation: &CancellationToken) -> OptimisationResult {
    highs::solve(input, limits, cancellation)
}

/// Growth factor applied to the event-segment budget on each retry.
#[cfg(not(target_arch = "wasm32"))]
/// Ceiling applied to the *derived* per-interval segment budget. The
/// derivation governs normally; this only stops pathological inputs (for
/// example a fleet of tiny opening lots) from allocating thousands of event
/// positions. Published in the result metadata; a run it flags as restricted
/// can be re-solved with an explicit override up to
/// [`SEGMENT_OVERRIDE_CEILING`].
///
/// Solver-independent: the blended experiment's capture derives its own
/// budget by the same union-bound rule and is held to the same guard, so
/// there is one event-budget policy rather than two.
pub(crate) const SEGMENT_CEILING: usize = 24;

/// Ceiling applied to an explicit event-segment override, so a flagged
/// restricted run can be re-solved with more event positions without the
/// guard being unbounded.
pub(crate) const SEGMENT_OVERRIDE_CEILING: usize = 64;

const EVENT_RETRY_GROWTH: usize = 2;

/// Re-solve with more event positions while the result says its event budget
/// may have restricted within-interval transitions.
///
/// The restriction flag is a heuristic: it reports that a loader worked an
/// interval's last event position with authored work still outstanding, which
/// *may* mean another transition was wanted. A clear flag on a larger budget
/// is therefore evidence, not proof, and this function does not turn it into
/// one. What it gives is a bounded, reusable way to spend a little more of a
/// wall-time budget on disproving the suspicion, and a result whose metadata
/// still states the budget it was solved at.
///
/// Attempts stop at the first unflagged result, when the budget stops
/// growing (the override ceiling), when the caller's wall time is spent, when
/// the attempt allowance runs out, or as soon as a solve returns no incumbent
/// (a larger model will not find one where a smaller one could not). The
/// answer returned is the last one solved, never a mixture.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn solve_within_event_budget(input: &OptimisationInput, limits: SolveLimits, attempts: usize, cancellation: &CancellationToken) -> OptimisationResult {
    let started = std::time::Instant::now();
    let mut attempt = input.clone();
    let mut result = solve(&attempt, limits, cancellation);
    for _ in 1..attempts.max(1) {
        if !result.event_budget_restricted || cancellation.is_cancelled() {
            break;
        }
        if !matches!(result.summary.status, SolveStatus::Optimal | SolveStatus::FeasibleLimit) {
            break;
        }
        let ceiling = result.resolution.segment_ceiling;
        let granted = result.resolution.segment_counts.iter().copied().max().unwrap_or(1);
        let next = (granted * EVENT_RETRY_GROWTH).min(ceiling.max(granted));
        if next <= granted {
            break;
        }
        let remaining = match limits.time {
            Some(limit) => match limit.checked_sub(started.elapsed()) {
                Some(left) if left > Duration::from_millis(500) => Some(left),
                _ => break,
            },
            None => None,
        };
        attempt.horizon.event_segments = Some(next as u32);
        result = solve(&attempt, SolveLimits { time: remaining, ..limits }, cancellation);
    }
    result
}

fn destination_receiving(input: &OptimisationInput, candidate: &MovementCandidate, interval: usize) -> bool {
    let Some(destination) = input.destinations.iter().find(|entry| entry.id == candidate.destination) else {
        return false;
    };
    match destination.kind {
        DestinationKind::Crusher => destination
            .crusher_daily_t
            .get(input.intervals[interval].day() as usize)
            .copied()
            .flatten()
            .is_none_or(|limit| limit > 0.0),
        DestinationKind::Dump | DestinationKind::Stockpile(_) => destination.capacity_t.is_none_or(|capacity| capacity > 0.0),
    }
}

/// Independently reconcile the published timeline against the original inputs.
/// This is intentionally separate from model construction: a formulation bug
/// cannot validate itself merely because its own constraints were satisfied,
/// and the published rows have passed through negligible-tonnage cleanup that
/// must not have created a material violation of what was solved.
///
/// The check replays the whole horizon segment by segment: per-segment loader
/// and fleet capacity, ground conservation and precedence, authored task
/// order, destination budgets, stockpile release timing and FIFO/LIFO order,
/// receipt parcel structure and interleaving, reclaim caps, and objective
/// reconciliation of both the published rows and the raw solver value.
pub(crate) fn validate_solution(input: &OptimisationInput, result: &OptimisationResult) -> Vec<ValidationIssue> {
    let tolerance = input.horizon.tolerances.tonnes_t;
    // Published rows carry the parcel snap tolerance's worth of quantisation
    // against the solver-enforced bounds; anything larger is a real violation.
    let dust = tolerance.max(input.horizon.parcel_target_t * 1e-6);
    // Inventory units are compared at the model's own published resolution:
    // below it the formulation cannot distinguish a unit from empty, so the
    // replay must not demand a finer distinction than was solvable.
    let unit_dust = dust.max(result.resolution.inventory_dust_t);
    let mut issues = Vec::new();

    let finite = result.movements.iter().all(|movement| movement.tonnes_t.is_finite() && movement.value.is_finite())
        && result.parcels.iter().all(|parcel| parcel.tonnes_t.is_finite())
        && result.reclaims.iter().all(|reclaim| reclaim.tonnes_t.is_finite())
        && result
            .segment_durations_h
            .iter()
            .flat_map(|durations| durations.iter().copied())
            .all(|duration| duration.is_finite());
    if !finite {
        issues.push(ValidationIssue {
            message: "nonfinite solution value".into(),
            magnitude: f64::INFINITY,
        });
    }

    // Segment durations: nonnegative, inside the interval, summing to it, and
    // agreeing with the budget the resolution metadata records.
    for interval in &input.intervals {
        let budget = result.resolution.segment_counts.get(interval.index).copied().unwrap_or(0);
        let Some(durations) = result.segment_durations_h.get(interval.index) else {
            issues.push(ValidationIssue {
                message: format!("missing segment durations for interval {}", interval.index),
                magnitude: 1.0,
            });
            continue;
        };
        if durations.len() != budget {
            issues.push(ValidationIssue {
                message: format!("segment duration count {} does not match the budget {budget}", durations.len()),
                magnitude: (durations.len() as f64) - budget as f64,
            });
        }
        let total: f64 = durations.iter().sum();
        if durations.iter().any(|duration| *duration < -tolerance || *duration > interval.duration_h() + tolerance) || (total - interval.duration_h()).abs() > 1e-6 {
            issues.push(ValidationIssue {
                message: format!("segment durations do not add up to interval {}", interval.index),
                magnitude: (total - interval.duration_h()).abs(),
            });
        }
    }

    // Per-segment loader capacity: at most one (task, source) and tonnage
    // within the rate multiplied by that segment's decided duration.
    for loader in &input.loaders {
        for interval in &input.intervals {
            for segment in 0..result.resolution.segment_counts.get(interval.index).copied().unwrap_or(0) {
                let rows: Vec<_> = result
                    .activities
                    .iter()
                    .filter(|row| row.loader == loader.id && row.interval == interval.index && row.segment == segment)
                    .collect();
                let assignments: BTreeSet<_> = rows.iter().map(|row| (row.task, row.source)).collect();
                if assignments.len() > 1 {
                    issues.push(ValidationIssue {
                        message: format!("loader {:?} works several sources in interval {} segment {segment}", loader.id, interval.index),
                        magnitude: assignments.len() as f64,
                    });
                }
                let duration = result.segment_durations_h[interval.index].get(segment).copied().unwrap_or(0.0);
                let dig: f64 = rows.iter().filter(|row| row.activity == Activity::Dig).map(|row| row.tonnes_t).sum();
                let reclaim: f64 = rows.iter().filter(|row| row.activity == Activity::Reclaim).map(|row| row.tonnes_t).sum();
                // Publication may snap a dig or reclaim row up to its
                // source's remaining balance within a three-dust window, so
                // bound checks carry that much per published row.
                let allowance = tolerance + 3.0 * dust * rows.len() as f64;
                let rate = &loader.rates[interval.index];
                let dig_over = dig - rate.dig_tph * duration;
                let reclaim_over = reclaim - rate.reclaim_tph * duration;
                if dig_over > allowance {
                    issues.push(ValidationIssue {
                        message: format!("dig rate exceeded in interval {} segment {segment}", interval.index),
                        magnitude: dig_over,
                    });
                }
                if reclaim_over > allowance {
                    issues.push(ValidationIssue {
                        message: format!("reclaim rate exceeded in interval {} segment {segment}", interval.index),
                        magnitude: reclaim_over,
                    });
                }
            }
        }
    }

    // Shared fleet capacity per segment: movement truck-hours within the
    // duration-proportional share of every class's interval budget.
    for truck in &input.trucks {
        for interval in &input.intervals {
            let units = truck.hours[interval.index] / interval.duration_h();
            for segment in 0..result.resolution.segment_counts.get(interval.index).copied().unwrap_or(0) {
                let duration = result.segment_durations_h[interval.index].get(segment).copied().unwrap_or(0.0);
                let used: f64 = result
                    .movements
                    .iter()
                    .filter(|movement| movement.interval == interval.index && movement.segment == segment && movement.truck == truck.id)
                    .map(|movement| movement.truck_hours)
                    .sum();
                let over = used - units * duration;
                if over > tolerance.max(dust) {
                    issues.push(ValidationIssue {
                        message: format!("truck {:?} over its segment share in interval {} segment {segment}", truck.id, interval.index),
                        magnitude: over,
                    });
                }
            }
        }
    }

    // Crusher daily budgets and dump capacities over the whole horizon.
    for destination in input.destinations.iter().filter(|destination| matches!(destination.kind, DestinationKind::Crusher)) {
        let days = ((input.horizon.end_h / 24.0).ceil() as usize).max(1);
        for day in 0..days {
            if let Some(limit) = destination.crusher_daily_t.get(day).copied().flatten() {
                let used: f64 = result
                    .movements
                    .iter()
                    .filter(|movement| movement.destination == destination.id && input.intervals[movement.interval].day() as usize == day)
                    .map(|movement| movement.tonnes_t)
                    .sum();
                if used - limit > tolerance {
                    issues.push(ValidationIssue {
                        message: format!("crusher {:?} day {day} over budget", destination.id),
                        magnitude: used - limit,
                    });
                }
            }
        }
    }
    for destination in input.destinations.iter().filter(|destination| matches!(destination.kind, DestinationKind::Dump)) {
        if let Some(capacity) = destination.capacity_t {
            let received: f64 = result.movements.iter().filter(|row| row.destination == destination.id).map(|row| row.tonnes_t).sum();
            if received - capacity > tolerance {
                issues.push(ValidationIssue {
                    message: "dump capacity exceeded".into(),
                    magnitude: received - capacity,
                });
            }
        }
    }

    // Cashflow attached to every published movement still reconciles.
    for movement in &result.movements {
        let expected = movement.tonnes_t * movement.cashflow.iter().map(|entry| entry.value_per_tonne).sum::<f64>();
        if (movement.value - expected).abs() > input.horizon.tolerances.objective.max(tolerance * expected.abs()) {
            issues.push(ValidationIssue {
                message: "movement objective does not reconcile".into(),
                magnitude: (movement.value - expected).abs(),
            });
        }
    }

    // Ground conservation: never overdrawn, composition always proportional.
    for ground in &input.ground {
        let extracted: f64 = result
            .activities
            .iter()
            .filter(|row| row.activity == Activity::Dig && row.source == SourceId::Ground(ground.id))
            .map(|row| row.tonnes_t)
            .sum();
        if extracted - ground.tonnes_t > tolerance {
            issues.push(ValidationIssue {
                message: format!("ground {:?} overdrawn", ground.id),
                magnitude: extracted - ground.tonnes_t,
            });
        }
        // Movements publish raw solver values while dig rows may have been
        // snapped to their ground balance within the dust window, so the
        // composition budget carries that much per published dig row.
        let dig_rows = result
            .activities
            .iter()
            .filter(|row| row.activity == Activity::Dig && row.source == SourceId::Ground(ground.id))
            .count();
        for share in &ground.material {
            let moved: f64 = result
                .movements
                .iter()
                .filter(|row| row.source == SourceId::Ground(ground.id) && row.material == share.material)
                .map(|row| row.tonnes_t)
                .sum();
            let expected = extracted * share.fraction;
            if (moved - expected).abs() > tolerance.max(expected.abs() * tolerance).max(3.0 * dust * dig_rows as f64) {
                issues.push(ValidationIssue {
                    message: "ground composition was not preserved".into(),
                    magnitude: (moved - expected).abs(),
                });
            }
        }
    }

    // Reclaim bars respect their cumulative maximum.
    for (task_index, task) in input.tasks.iter().enumerate() {
        let TaskKind::Reclaim { maximum_t: Some(maximum), .. } = &task.kind else {
            continue;
        };
        let rows: Vec<&ReclaimRecord> = result
            .reclaims
            .iter()
            .filter(|row| row.task == task.id && input.tasks.iter().position(|other| other.id == row.task) == Some(task_index))
            .collect();
        let reclaimed: f64 = rows.iter().map(|row| row.tonnes_t).sum();
        if reclaimed - *maximum > tolerance + 3.0 * dust * rows.len() as f64 {
            issues.push(ValidationIssue {
                message: "reclaim bar maximum exceeded".into(),
                magnitude: reclaimed - *maximum,
            });
        }
    }

    let pile_destination: BTreeMap<StockpileId, DestinationId> = input
        .destinations
        .iter()
        .filter_map(|destination| match destination.kind {
            DestinationKind::Stockpile(pile) => Some((pile, destination.id)),
            _ => None,
        })
        .collect();
    // Replay the horizon segment by segment with the published rows: ledgers,
    // destination availability, authored order, and FIFO/LIFO all advance at
    // execution-event boundaries exactly as the model claims.
    let mut ground_extracted: BTreeMap<GroundId, f64> = input.ground.iter().map(|ground| (ground.id, 0.0)).collect();
    let mut crusher_used: BTreeMap<(DestinationId, u32), f64> = BTreeMap::new();
    let mut dump_used: BTreeMap<DestinationId, f64> = BTreeMap::new();
    let mut pile_inventory: BTreeMap<StockpileId, f64> = input
        .stockpiles
        .iter()
        .map(|pile| (pile.id, pile.opening.iter().map(|lot| lot.tonnes_t).sum::<f64>()))
        .collect();
    // Released inventory: opening lots first, then each interval's parcels at
    // the start of the following interval, ordered by (segment, position).
    let mut released_units: BTreeMap<StockpileId, Vec<InventoryUnit>> = input
        .stockpiles
        .iter()
        .map(|pile| (pile.id, pile.opening.iter().map(|lot| InventoryUnit::Opening(lot.id)).collect()))
        .collect();
    let mut unit_balance: BTreeMap<InventoryUnit, f64> = input
        .stockpiles
        .iter()
        .flat_map(|pile| pile.opening.iter().map(|lot| (InventoryUnit::Opening(lot.id), lot.tonnes_t)))
        .collect();

    for interval in &input.intervals {
        // Parcels received during the previous interval become reclaimable
        // units now, in receipt order.
        for pile in &input.stockpiles {
            let mut arrivals: Vec<&ReceiptParcel> = result
                .parcels
                .iter()
                .filter(|parcel| parcel.stockpile == pile.id && parcel.release_interval == interval.index)
                .collect();
            arrivals.sort_by_key(|parcel| (parcel.segment, parcel.position));
            for parcel in arrivals {
                let unit = InventoryUnit::Receipt {
                    interval: parcel.receipt_interval,
                    segment: parcel.segment,
                    position: parcel.position,
                };
                // One position is one inventory unit; the streams that
                // supplied it share its balance and its place in the order.
                let units = released_units.get_mut(&pile.id).expect("released list exists");
                if !units.contains(&unit) {
                    units.push(unit.clone());
                }
                *unit_balance.entry(unit).or_insert(0.0) += parcel.tonnes_t;
            }
        }
        for segment in 0..result.resolution.segment_counts.get(interval.index).copied().unwrap_or(0) {
            // Destination capacity remaining at this segment's start.
            let destination_remaining = |destination: &Destination| -> f64 {
                match destination.kind {
                    DestinationKind::Crusher => {
                        destination.crusher_daily_t.get(interval.day() as usize).copied().flatten().unwrap_or(f64::INFINITY)
                            - crusher_used.get(&(destination.id, interval.day())).copied().unwrap_or(0.0)
                    }
                    DestinationKind::Dump => destination.capacity_t.unwrap_or(f64::INFINITY) - dump_used.get(&destination.id).copied().unwrap_or(0.0),
                    DestinationKind::Stockpile(pile) => destination.capacity_t.unwrap_or(f64::INFINITY) - pile_inventory.get(&pile).copied().unwrap_or(0.0),
                }
            };
            let remaining_of = |id: DestinationId| -> f64 {
                input
                    .destinations
                    .iter()
                    .find(|destination| destination.id == id)
                    .map(&destination_remaining)
                    .unwrap_or(0.0)
            };
            let pile_releasable = |pile: &Stockpile| -> f64 {
                released_units
                    .get(&pile.id)
                    .map(|units| units.iter().filter_map(|unit| unit_balance.get(unit).copied()).sum::<f64>())
                    .unwrap_or(0.0)
            };
            let route_available = |loader: LoaderId, activity: Activity, source: SourceId, material: MaterialId| -> bool {
                input.movements.iter().any(|candidate| {
                    candidate.loader == loader
                        && candidate.activity == activity
                        && candidate.source == source
                        && candidate.material == material
                        && destination_receiving(input, candidate, interval.index)
                        && input.trucks.iter().any(|truck| truck.id == candidate.truck && truck.hours[interval.index] > 0.0)
                        && remaining_of(candidate.destination) > tolerance
                })
            };
            // Authored order: the first task of each loader whose current work
            // is operationally eligible is the only task it may work now.
            for loader in &input.loaders {
                let mut sorted: Vec<&Task> = input.tasks.iter().filter(|task| task.loader == loader.id).collect();
                sorted.sort_by(|left, right| {
                    (left.priority, left.window_start_h, left.id)
                        .partial_cmp(&(right.priority, right.window_start_h, right.id))
                        .expect("finite windows")
                });
                let depleted = |ground: GroundId| -> bool {
                    let tonnes = input.ground.iter().find(|source| source.id == ground).map(|source| source.tonnes_t).unwrap_or(0.0);
                    ground_extracted.get(&ground).copied().unwrap_or(0.0) >= tonnes - dust.min(tonnes)
                };
                let window_active = |task: &Task| task.window_start_h <= interval.start_h + 1e-9 && task.window_end_h >= interval.end_h - 1e-9;
                let mut expected_task: Option<&Task> = None;
                let mut expected_source: Option<GroundId> = None;
                for task in &sorted {
                    match &task.kind {
                        TaskKind::Dig { sequence } => {
                            if !window_active(task) || loader.rates[interval.index].dig_tph <= 0.0 {
                                continue;
                            }
                            // The first unfinished stage blocks the task until
                            // it is routable; later stages never leapfrog it.
                            let Some(stage) = sequence.iter().position(|ground| !depleted(*ground)) else {
                                continue;
                            };
                            let source = sequence[stage];
                            let routable = input.ground.iter().find(|ground| ground.id == source).is_some_and(|data| {
                                data.material
                                    .iter()
                                    .all(|share| route_available(loader.id, Activity::Dig, SourceId::Ground(source), share.material))
                            });
                            if routable {
                                expected_task = Some(task);
                                expected_source = Some(source);
                                break;
                            }
                            // Operationally blocked: the next authored task
                            // becomes eligible.
                        }
                        TaskKind::Reclaim { approved_sources, .. } => {
                            if !window_active(task) || loader.rates[interval.index].reclaim_tph <= 0.0 {
                                continue;
                            }
                            let eligible = approved_sources.iter().any(|pile_id| {
                                input
                                    .stockpiles
                                    .iter()
                                    .find(|pile| pile.id == *pile_id)
                                    .is_some_and(|pile| pile_releasable(pile) > unit_dust)
                            });
                            if eligible {
                                expected_task = Some(task);
                                break;
                            }
                            // No released stock: blocked, next authored task.
                        }
                    }
                }
                let rows: Vec<&ActivityRecord> = result
                    .activities
                    .iter()
                    .filter(|row| row.loader == loader.id && row.interval == interval.index && row.segment == segment)
                    .collect();
                if !rows.is_empty() {
                    if let Some(task) = expected_task {
                        let wrong_task = rows.iter().any(|row| row.task != task.id);
                        let wrong_source = expected_source.is_some_and(|source| rows.iter().any(|row| row.source != SourceId::Ground(source)));
                        if wrong_task || wrong_source {
                            issues.push(ValidationIssue {
                                message: format!(
                                    "authored order bypassed: loader {:?} interval {} segment {segment} worked {:?} while task {:?} was eligible",
                                    loader.id,
                                    interval.index,
                                    rows.first().map(|row| row.task),
                                    task.id
                                ),
                                magnitude: 1.0,
                            });
                        }
                    } else {
                        issues.push(ValidationIssue {
                            message: format!("loader {:?} worked in interval {} segment {segment} with no eligible task", loader.id, interval.index),
                            magnitude: 1.0,
                        });
                    }
                }
            }

            // FIFO/LIFO: all reclaim in this segment draws from the single
            // eligible unit of its pile.
            for pile in &input.stockpiles {
                let rows: Vec<&ReclaimRecord> = result
                    .reclaims
                    .iter()
                    .filter(|row| row.stockpile == pile.id && row.interval == interval.index && row.segment == segment)
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                let units = released_units.get(&pile.id).expect("released list exists");
                let eligible = match pile.order {
                    ReclaimOrder::Fifo => units.iter().find(|unit| unit_balance.get(*unit).copied().unwrap_or(0.0) > unit_dust),
                    ReclaimOrder::Lifo => units.iter().rev().find(|unit| unit_balance.get(*unit).copied().unwrap_or(0.0) > unit_dust),
                };
                if rows.iter().any(|row| Some(&row.unit) != eligible) {
                    issues.push(ValidationIssue {
                        message: format!("reclaim did not use the eligible FIFO/LIFO unit in interval {} segment {segment}", interval.index),
                        magnitude: 1.0,
                    });
                }
                for row in rows {
                    if let Some(balance) = unit_balance.get_mut(&row.unit) {
                        *balance -= row.tonnes_t;
                        if *balance < -tolerance {
                            issues.push(ValidationIssue {
                                message: "inventory unit overdrawn".into(),
                                magnitude: -*balance,
                            });
                        }
                    } else {
                        issues.push(ValidationIssue {
                            message: "reclaim used an unreleased unit".into(),
                            magnitude: row.tonnes_t,
                        });
                    }
                }
            }

            // Advance every ledger through this segment.
            for row in result
                .activities
                .iter()
                .filter(|row| row.interval == interval.index && row.segment == segment && row.activity == Activity::Dig)
            {
                if let SourceId::Ground(id) = row.source
                    && let Some(held) = ground_extracted.get_mut(&id)
                {
                    *held += row.tonnes_t;
                }
            }
            for row in result.movements.iter().filter(|row| row.interval == interval.index && row.segment == segment) {
                if let Some(destination) = input.destinations.iter().find(|destination| destination.id == row.destination) {
                    match destination.kind {
                        DestinationKind::Crusher => *crusher_used.entry((destination.id, interval.day())).or_default() += row.tonnes_t,
                        DestinationKind::Dump => *dump_used.entry(destination.id).or_default() += row.tonnes_t,
                        DestinationKind::Stockpile(pile) => *pile_inventory.entry(pile).or_default() += row.tonnes_t,
                    }
                }
            }
            for row in result.reclaims.iter().filter(|row| row.interval == interval.index && row.segment == segment) {
                *pile_inventory.entry(row.stockpile).or_default() -= row.tonnes_t;
            }
            for (pile, inventory) in &pile_inventory {
                if *inventory < -(tolerance + dust)
                    || *inventory
                        > input
                            .stockpiles
                            .iter()
                            .find(|entry| entry.id == *pile)
                            .map(|entry| entry.capacity_t)
                            .unwrap_or(f64::INFINITY)
                            + tolerance
                            + dust
                {
                    issues.push(ValidationIssue {
                        message: format!("stockpile {pile:?} balance outside capacity in interval {} segment {segment}", interval.index),
                        magnitude: inventory.abs(),
                    });
                }
            }
        }

        // End-of-interval published balances reconcile with the replay.
        if let Some(balance) = result.balances.get(interval.index) {
            for (pile, inventory) in &pile_inventory {
                if let Some(published) = balance.stockpile_closing_t.get(pile)
                    && (*published - *inventory).abs() > tolerance.max(inventory.abs() * tolerance)
                {
                    issues.push(ValidationIssue {
                        message: "stockpile closing balance does not reconcile".into(),
                        magnitude: (*published - *inventory).abs(),
                    });
                }
            }
            for (ground, extracted) in &ground_extracted {
                if let Some(published) = balance.ground_remaining_t.get(ground)
                    && (*published - (input.ground.iter().find(|source| source.id == *ground).map(|source| source.tonnes_t).unwrap_or(0.0) - extracted)).abs() > tolerance
                {
                    issues.push(ValidationIssue {
                        message: "ground remaining balance does not reconcile".into(),
                        magnitude: (*published - extracted).abs(),
                    });
                }
            }
        }
    }

    // Receipt parcels: structure, conservation with the movements of their own
    // segment, and balanced interleaving among overlapping supplies.
    for parcel in &result.parcels {
        if parcel.tonnes_t < input.horizon.tolerances.parcel_occupancy_t * 0.5 {
            issues.push(ValidationIssue {
                message: "invalid receipt parcel size".into(),
                magnitude: parcel.tonnes_t,
            });
        }
        if parcel.release_interval != parcel.receipt_interval + 1 {
            issues.push(ValidationIssue {
                message: "receipt release is not next interval".into(),
                magnitude: 1.0,
            });
        }
    }
    let parcel_groups: BTreeSet<_> = result.parcels.iter().map(|parcel| (parcel.stockpile, parcel.receipt_interval, parcel.segment)).collect();
    for (pile, interval, segment) in parcel_groups {
        let group: Vec<_> = result
            .parcels
            .iter()
            .filter(|parcel| parcel.stockpile == pile && parcel.receipt_interval == interval && parcel.segment == segment)
            .collect();
        if group.iter().enumerate().any(|(position, parcel)| parcel.position != position) {
            issues.push(ValidationIssue {
                message: "receipt positions are not contiguous".into(),
                magnitude: 1.0,
            });
        }
        // A published parcel is a full parcel (the target size) or a stream's
        // single remainder; merged publication dust may lift the stream's last
        // parcel by at most the snap tolerance per parcel in the group.
        let ceiling = input.horizon.parcel_target_t + tolerance + group.len() as f64 * dust;
        if let Some(over) = group.iter().find(|parcel| parcel.tonnes_t > ceiling) {
            issues.push(ValidationIssue {
                message: "invalid receipt parcel size".into(),
                magnitude: over.tonnes_t,
            });
        }
        let Some(destination) = pile_destination.get(&pile) else {
            continue;
        };
        let receipts: f64 = result
            .movements
            .iter()
            .filter(|row| row.interval == interval && row.segment == segment && row.destination == *destination)
            .map(|row| row.tonnes_t)
            .sum();
        let parcelled: f64 = group.iter().map(|parcel| parcel.tonnes_t).sum();
        let snap = tolerance.max(input.horizon.parcel_target_t * 1e-6);
        if (receipts - parcelled).abs() > tolerance.max(receipts.abs() * tolerance).max(group.len() as f64 * snap) {
            issues.push(ValidationIssue {
                message: "receipt parcels do not reconcile with received tonnes".into(),
                magnitude: (receipts - parcelled).abs(),
            });
        }
        let streams: BTreeSet<_> = group.iter().map(|parcel| (parcel.source, parcel.material)).collect();
        for stream in streams {
            // At most one parcel per stream may sit below the target size - the
            // remainder - and no full parcel may follow it. The tolerance
            // absorbs solver row-scaling slack around the exact target size.
            let mut ordered: Vec<_> = group.iter().filter(|parcel| (parcel.source, parcel.material) == stream).collect();
            ordered.sort_by_key(|parcel| parcel.position);
            let sub_target_tolerance = tolerance.max(input.horizon.parcel_target_t * 1e-6);
            let sub_target: Vec<_> = ordered
                .iter()
                .filter(|parcel| parcel.tonnes_t < input.horizon.parcel_target_t - sub_target_tolerance)
                .collect();
            if sub_target.len() > 1 {
                issues.push(ValidationIssue {
                    message: "stream has several remainder parcels".into(),
                    magnitude: sub_target.len() as f64,
                });
            }
            if let Some(last) = sub_target.last()
                && ordered.iter().any(|parcel| parcel.position > last.position)
            {
                issues.push(ValidationIssue {
                    message: "remainder parcel does not follow the stream's full parcels".into(),
                    magnitude: 1.0,
                });
            }
            let total: f64 = group.iter().filter(|parcel| (parcel.source, parcel.material) == stream).map(|parcel| parcel.tonnes_t).sum();
            for prefix in 1..=group.len() {
                let supplied: f64 = group[..prefix]
                    .iter()
                    .filter(|parcel| (parcel.source, parcel.material) == stream)
                    .map(|parcel| parcel.tonnes_t)
                    .sum();
                let expected = prefix as f64 / group.len() as f64 * total;
                if (supplied - expected).abs() > input.horizon.parcel_target_t + tolerance {
                    issues.push(ValidationIssue {
                        message: "receipt interleaving discrepancy exceeded".into(),
                        magnitude: (supplied - expected).abs() - input.horizon.parcel_target_t,
                    });
                }
            }
        }
    }

    // Objective reconciliation: the published rows must reproduce the published
    // objective, and the cleanup must not have moved it materially from what
    // the solver actually achieved.
    let objective: f64 = result.movements.iter().map(|movement| movement.value).sum();
    if let Some(reported) = result.summary.objective
        && (objective - reported).abs() > input.horizon.tolerances.objective.max(tolerance * reported.abs())
    {
        issues.push(ValidationIssue {
            message: "objective does not reconcile".into(),
            magnitude: (objective - reported).abs(),
        });
    }
    if let Some(raw) = result.summary.raw_solver_objective
        && let Some(published) = result.summary.objective
        && (raw - published).abs() > input.horizon.tolerances.objective.max(tolerance * raw.abs().max(published.abs())) * result.movements.len().max(1) as f64
    {
        issues.push(ValidationIssue {
            message: "published objective materially differs from the solver objective".into(),
            magnitude: (raw - published).abs(),
        });
    }
    issues
}
