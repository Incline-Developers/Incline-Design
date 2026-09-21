//! Independent physical replay of a blended schedule.
//!
//! This reconstructs the timeline from the published movement rows *alone*
//! and recomputes every quantity from first principles. It never reads a
//! solver variable other than the movement tonnes, and it never consults
//! SCIP's constraint-status report.
//!
//! That is not defensive ceremony. On the exact constraint class this module
//! investigates, SCIP 10.0.2 was observed reporting a suboptimal answer as
//! `Optimal` with a matching dual bound and a zero gap (see
//! [`super::super::scip::adapter::SolveTuning::apply`]). A backend's own verdict is
//! evidence, not proof.
//!
//! The blended grade is recomputed here by *division* - `Q / T` on the
//! replayed opening state - which is exactly the operation the formulation
//! had to avoid. That is the point: the model's cleared bilinear form and the
//! replay's direct ratio are independent routes to the same number, so a
//! disagreement between them is detectable.

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

use super::input::{BlendInput, BlendPile, GradeLimit, WINDOW_TOLERANCE_H, task_authorises};
use crate::model::schedule::optimisation::{Activity, Destination, DestinationId, DestinationKind, SourceId, StockpileId, TaskKind};

/// One published movement: how much material a candidate moved in a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MovementRow {
    pub(crate) candidate: usize,
    pub(crate) interval: usize,
    pub(crate) segment: usize,
    pub(crate) tonnes_t: f64,
}

/// One chunk's published state in one interval.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChunkRow {
    pub(crate) pile: StockpileId,
    pub(crate) chunk: usize,
    pub(crate) interval: usize,
    pub(crate) open_t: f64,
    pub(crate) reclaimed_t: f64,
    /// Closed to further receipts, and therefore reclaimable.
    pub(crate) closed: bool,
    pub(crate) received_t: f64,
    /// What each movement candidate delivered into this chunk in this
    /// interval. Published per candidate, not just as a total, so the replay
    /// can recompute the chunk's composition from its *own* receipts instead
    /// of accepting a grade the solver asserted.
    pub(crate) receipts: Vec<(usize, f64)>,
}

/// The extracted schedule, as published for replay.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlendSolution {
    pub(crate) movements: Vec<MovementRow>,
    pub(crate) durations: BTreeMap<(usize, usize), f64>,
    /// Empty for unchunked piles.
    pub(crate) chunks: Vec<ChunkRow>,
    /// The backend's raw objective, for comparison against the published rows.
    pub(crate) reported_objective: f64,
    /// Tiny solver columns omitted when extracting the published schedule.
    /// These are reported in aggregate so numerical changes remain visible.
    pub(crate) adjustments: ExtractionAdjustments,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ExtractionAdjustments {
    pub(crate) movement_count: usize,
    pub(crate) movement_total_t: f64,
    pub(crate) movement_max_t: f64,
    pub(crate) chunk_receipt_count: usize,
    pub(crate) chunk_receipt_total_t: f64,
    pub(crate) chunk_receipt_max_t: f64,
}

impl ExtractionAdjustments {
    pub(crate) fn movement(&mut self, tonnes: f64) {
        self.movement_count += 1;
        self.movement_total_t += tonnes.abs();
        self.movement_max_t = self.movement_max_t.max(tonnes.abs());
    }

    pub(crate) fn chunk_receipt(&mut self, tonnes: f64) {
        self.chunk_receipt_count += 1;
        self.chunk_receipt_total_t += tonnes.abs();
        self.chunk_receipt_max_t = self.chunk_receipt_max_t.max(tonnes.abs());
    }
}

/// The closing state of one pile, recomputed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClosingState {
    pub(crate) pile: StockpileId,
    pub(crate) tonnes_t: f64,
    pub(crate) contained: Vec<f64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ReplayReport {
    /// Every **physical** rule the replayed timeline breaks, in plain words:
    /// conservation, capacity, release timing, resource limits, authored
    /// order, chunk lifecycle. A candidate with any of these is not a
    /// schedule at all.
    pub(crate) issues: Vec<String>,
    /// Grade-dependent conditions the replayed timeline fails: a delivery
    /// that did not reach a minimum grade it was routed against.
    ///
    /// Kept apart from [`Self::issues`] on purpose. For the nonlinear method
    /// these are equally fatal, but the iterative fixed-grade method needs to
    /// distinguish "this timeline is impossible" from "this timeline is
    /// possible but my grade estimate was wrong", because only the second is
    /// something the next iteration can learn from.
    pub(crate) grade_issues: Vec<String>,
    /// The blend actually held, by `(pile, mixing key, grade)` - interval for
    /// an unchunked pile, [`super::formulation::chunk_key`] for a chunked
    /// one. Always populated, including for candidates that failed a grade
    /// condition, because that is exactly the case the iterative method has
    /// to re-estimate from.
    pub(crate) blends: BTreeMap<(StockpileId, usize, usize), f64>,
    /// Largest absolute residual on any conservation identity, in tonnes.
    pub(crate) max_absolute_residual_t: f64,
    /// The same residual relative to the quantity it was checked against.
    pub(crate) max_relative_residual: f64,
    /// Objective recomputed from the published rows.
    pub(crate) replayed_objective: f64,
    /// Replayed minus reported.
    pub(crate) objective_difference: f64,
    /// Material delivered below a grade boundary by an amount small enough to
    /// be explained by the formulation's own indicator tolerance (see
    /// [`INDICATOR_LEAK`]). Published rather than discarded: it is a
    /// quantified modelling artefact, not a clean result.
    pub(crate) negligible_grade_deliveries: usize,
    pub(crate) negligible_grade_tonnes_t: f64,
    /// Chunk-lifecycle events small enough to be the formulation's own
    /// indicator tolerance rather than a real movement, and the threshold
    /// they were measured against.
    ///
    /// Published, not discarded. A chunk "receiving" 0.2 kg after it closed
    /// is an artefact of `x <= M * indicator` with a binary allowed to sit a
    /// tolerance above zero, not a schedule that reopens chunks - but the
    /// reader is entitled to see how much of it there was.
    pub(crate) chunk_dust_events: usize,
    pub(crate) chunk_dust_tonnes_t: f64,
    pub(crate) chunk_dust_threshold_t: f64,
    pub(crate) closing: Vec<ClosingState>,
}

impl ReplayReport {
    /// Publishable: every physical *and* grade check passed.
    pub(crate) fn is_valid(&self) -> bool {
        self.issues.is_empty() && self.grade_issues.is_empty()
    }

    /// The timeline is physically replayable. It may still have been routed
    /// against a grade it did not reach.
    pub(crate) fn is_physically_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Tolerance for a physical check. SCIP satisfies constraints to
/// `numerics/feastol`; the replay allows a little more than that so it
/// reports genuine violations rather than arithmetic noise, and records the
/// residual it actually saw either way.
const REPLAY_TOLERANCE_T: f64 = 1e-5;

/// SCIP's default `numerics/feastol`, which it also applies as the
/// integrality tolerance: a binary may sit this far from 0 or 1.
const INTEGRALITY_TOLERANCE: f64 = 1e-6;

/// How much material a grade-limit indicator can pass while reading as
/// "unused", given a big-M of `capacity`.
///
/// This is derived, not chosen. The formulation gates a limited route with
/// `flow <= M x used`; a `used` sitting one integrality tolerance above zero
/// admits `M x tolerance` tonnes. The formulation already uses the tightest
/// physical M available (the routed loaders' rate x duration rather than the
/// pile's capacity), which is what keeps this figure small - but it cannot
/// reach zero, so the replay reports what leaks instead of ignoring it.
fn indicator_leak(capacity_t: f64) -> f64 {
    capacity_t * INTEGRALITY_TOLERANCE
}

/// Safety factor on a derived indicator bound, mirroring the accepted
/// backend's `INTEGRALITY_MARGIN`: the leak is `M x tolerance` in theory, and
/// presolve can rescale a row, so the threshold allows an order of magnitude.
const INTEGRALITY_MARGIN: f64 = 10.0;

/// The negligible-material scale for one chunked pile's lifecycle rows,
/// derived rather than chosen.
///
/// Every lifecycle rule is an `x <= M * indicator` row, so an indicator
/// sitting one integrality tolerance above zero admits `M x tolerance` of
/// material through a gate that reads as shut. The formulation already uses
/// the tightest `M` it physically can (what can move in one interval, not the
/// chunk's whole capacity); this is what that still leaves, and material
/// below it is counted and published rather than either failing the replay or
/// vanishing from it.
///
/// This is **not** a widened feasibility tolerance. Physical conservation,
/// capacity and grade checks keep [`REPLAY_TOLERANCE_T`]; only the lifecycle
/// *predicates* - "did this chunk receive anything", "does it still hold
/// material" - use this scale, because those are questions about whether an
/// indicator was on, and the answer is meaningless below its own resolution.
fn chunk_dust(pile: &BlendPile) -> f64 {
    let largest = pile.chunks.iter().copied().fold(0.0_f64, f64::max);
    (largest * INTEGRALITY_TOLERANCE * INTEGRALITY_MARGIN).max(REPLAY_TOLERANCE_T)
}

struct Checker<'a> {
    input: &'a BlendInput,
    cancel: Option<&'a AtomicBool>,
    report: ReplayReport,
}

impl<'a> Checker<'a> {
    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|flag| flag.load(Ordering::Acquire))
    }
    /// Record a conservation residual and complain when it exceeds tolerance.
    fn residual(&mut self, label: &str, actual: f64, expected: f64) {
        let absolute = (actual - expected).abs();
        let scale = expected.abs().max(1.0);
        self.report.max_absolute_residual_t = self.report.max_absolute_residual_t.max(absolute);
        self.report.max_relative_residual = self.report.max_relative_residual.max(absolute / scale);
        if absolute > REPLAY_TOLERANCE_T {
            self.report.issues.push(format!("{label}: replayed {actual:.6} against {expected:.6}"));
        }
    }

    fn breach(&mut self, label: &str, used: f64, limit: f64) {
        if used > limit + REPLAY_TOLERANCE_T {
            self.report.issues.push(format!("{label}: {used:.6} exceeds {limit:.6}"));
        }
    }
}

/// Replay `solution` against `input`.
pub(crate) fn replay(input: &BlendInput, solution: &BlendSolution) -> ReplayReport {
    replay_inner(input, solution, None).expect("replay without cancellation")
}

pub(crate) fn replay_cancellable(input: &BlendInput, solution: &BlendSolution, cancel: &AtomicBool) -> Option<ReplayReport> {
    replay_inner(input, solution, Some(cancel))
}

fn replay_inner<'a>(input: &'a BlendInput, solution: &BlendSolution, cancel: Option<&'a AtomicBool>) -> Option<ReplayReport> {
    let grades = input.grades.count();
    let segments = input.segments_per_interval.max(1);
    let mut checker = Checker {
        input,
        cancel,
        report: ReplayReport::default(),
    };

    let destinations: BTreeMap<DestinationId, &Destination> = input.destinations.iter().map(|entry| (entry.id, entry)).collect();

    // Index the published rows by cell.
    let mut by_cell: BTreeMap<(usize, usize), Vec<MovementRow>> = BTreeMap::new();
    for row in &solution.movements {
        if checker.cancelled() {
            return None;
        }
        if row.tonnes_t < -REPLAY_TOLERANCE_T {
            checker.report.issues.push(format!("movement {} moved negative tonnes {}", row.candidate, row.tonnes_t));
        }
        by_cell.entry((row.interval, row.segment)).or_default().push(*row);
    }

    // ---- objective, recomputed from the rows --------------------------------
    let mut replayed = 0.0;
    for row in &solution.movements {
        if checker.cancelled() {
            return None;
        }
        let Some(candidate) = input.movements.get(row.candidate) else {
            checker.report.issues.push(format!("movement row names unknown candidate {}", row.candidate));
            continue;
        };
        replayed += row.tonnes_t * candidate.value_per_tonne().unwrap_or(0.0);
    }
    checker.report.replayed_objective = replayed;

    // ---- ground depletion ---------------------------------------------------
    for source in &input.ground {
        if checker.cancelled() {
            return None;
        }
        let mut dug = 0.0;
        for row in &solution.movements {
            let Some(candidate) = input.movements.get(row.candidate) else { continue };
            if candidate.activity == Activity::Dig && candidate.source == SourceId::Ground(source.id) {
                dug += row.tonnes_t;
            }
        }
        checker.breach(&format!("ground {} depleted twice", source.id.0), dug, source.tonnes_t);
    }

    // ---- proportional extraction of a mixed block ---------------------------
    // Checked against the *measured* block composition carried on the ground
    // source, cell by cell, so a schedule that took the profitable portion of
    // a block first is caught here even if the horizon totals happen to
    // balance. This is deliberately not a check of the formulation's own
    // proportion rows: it recomputes the same physical fact from the
    // published movement tonnes alone.
    for source in &input.ground {
        if checker.cancelled() {
            return None;
        }
        if source.material.len() < 2 {
            continue;
        }
        let mut per_cell: BTreeMap<(usize, usize), (f64, BTreeMap<u32, f64>)> = BTreeMap::new();
        for row in &solution.movements {
            let Some(candidate) = input.movements.get(row.candidate) else { continue };
            if candidate.activity != Activity::Dig || candidate.source != SourceId::Ground(source.id) {
                continue;
            }
            let entry = per_cell.entry((row.interval, row.segment)).or_default();
            entry.0 += row.tonnes_t;
            *entry.1.entry(candidate.material.0).or_default() += row.tonnes_t;
        }
        for ((interval, segment), (extracted, by_material)) in per_cell {
            for share in &source.material {
                let moved = by_material.get(&share.material.0).copied().unwrap_or(0.0);
                checker.residual(
                    &format!(
                        "ground {} material {} was not dug in proportion in interval {interval} segment {segment}",
                        source.id.0, share.material.0
                    ),
                    moved,
                    share.fraction * extracted,
                );
            }
        }
    }

    // ---- loader and truck capacity per segment ------------------------------
    for interval in &input.intervals {
        if checker.cancelled() {
            return None;
        }
        for segment in 0..segments {
            let duration = solution.durations.get(&(interval.index, segment)).copied().unwrap_or(0.0);
            let rows = by_cell.get(&(interval.index, segment)).cloned().unwrap_or_default();

            for loader in &input.loaders {
                let Some(rate) = loader.rates.iter().find(|entry| entry.interval == interval.index) else {
                    continue;
                };
                for activity in [Activity::Dig, Activity::Reclaim] {
                    let tph = match activity {
                        Activity::Dig => rate.dig_tph,
                        Activity::Reclaim => rate.reclaim_tph,
                    };
                    let moved: f64 = rows
                        .iter()
                        .filter(|row| {
                            input
                                .movements
                                .get(row.candidate)
                                .is_some_and(|candidate| candidate.loader == loader.id && candidate.activity == activity)
                        })
                        .map(|row| row.tonnes_t)
                        .sum();
                    checker.breach(
                        &format!("loader {} {activity:?} rate in interval {} segment {segment}", loader.id.0, interval.index),
                        moved,
                        tph * duration,
                    );
                }
            }

            for truck in &input.trucks {
                let Some(hours) = truck.hours.get(interval.index).copied() else { continue };
                if interval.duration_h() <= 0.0 {
                    continue;
                }
                let used: f64 = rows
                    .iter()
                    .filter_map(|row| {
                        input
                            .movements
                            .get(row.candidate)
                            .filter(|candidate| candidate.truck == truck.id)
                            .map(|candidate| row.tonnes_t * candidate.truck_hours_per_tonne)
                    })
                    .sum();
                let available = hours / interval.duration_h() * duration;
                checker.breach(
                    &format!("truck class {} hours in interval {} segment {segment}", truck.id.0, interval.index),
                    used,
                    available,
                );
            }
        }

        // The event clock must fill its interval exactly.
        let clock: f64 = (0..segments)
            .map(|segment| solution.durations.get(&(interval.index, segment)).copied().unwrap_or(0.0))
            .sum();
        checker.residual(&format!("event clock for interval {}", interval.index), clock, interval.duration_h());
    }

    // ---- destination and crusher limits -------------------------------------
    for destination in &input.destinations {
        if checker.cancelled() {
            return None;
        }
        let delivered = |filter: &dyn Fn(usize) -> bool| -> f64 {
            solution
                .movements
                .iter()
                .filter(|row| input.movements.get(row.candidate).is_some_and(|candidate| candidate.destination == destination.id) && filter(row.interval))
                .map(|row| row.tonnes_t)
                .sum()
        };
        match destination.kind {
            DestinationKind::Crusher => {
                for (day, budget) in destination.crusher_daily_t.iter().enumerate() {
                    let Some(budget) = budget else { continue };
                    // Direct mining and reclaim share this budget.
                    let used = delivered(&|interval| input.intervals.get(interval).is_some_and(|entry| entry.day() as usize == day));
                    checker.breach(&format!("crusher {} budget on day {day}", destination.id.0), used, *budget);
                }
            }
            DestinationKind::Dump | DestinationKind::Stockpile(_) => {
                let Some(capacity) = destination.capacity_t else { continue };
                let used = delivered(&|_| true);
                checker.breach(&format!("destination {} capacity", destination.id.0), used, capacity);
            }
        }
    }

    // ---- reclaim caps -------------------------------------------------------
    for task in &input.tasks {
        if checker.cancelled() {
            return None;
        }
        let TaskKind::Reclaim {
            approved_sources,
            maximum_t: Some(maximum),
        } = &task.kind
        else {
            continue;
        };
        let reclaimed: f64 = solution
            .movements
            .iter()
            .filter(|row| {
                input.movements.get(row.candidate).is_some_and(|candidate| {
                    candidate.loader == task.loader
                        && candidate.activity == Activity::Reclaim
                        && matches!(candidate.source, SourceId::Stockpile(pile) if approved_sources.contains(&pile))
                })
            })
            .map(|row| row.tonnes_t)
            .sum();
        checker.breach(&format!("reclaim cap on task {}", task.id.0), reclaimed, *maximum);
    }

    // ---- exact authored bar windows -----------------------------------------
    // A movement may only occur inside the window a planner authored, and the
    // convention is *containment*, matching the accepted backend: a bar that
    // covers only part of a calendar interval is not worked in that interval.
    for row in &solution.movements {
        if checker.cancelled() {
            return None;
        }
        if row.tonnes_t <= REPLAY_TOLERANCE_T {
            continue;
        }
        let Some(candidate) = input.movements.get(row.candidate) else { continue };
        let Some(interval) = input.intervals.get(row.interval) else { continue };
        let authorised = input.tasks.iter().any(|task| covers(task, *interval) && task_authorises(task, candidate));
        if !authorised {
            checker
                .report
                .issues
                .push(format!("movement {} in interval {} is outside every authored bar window", row.candidate, row.interval));
        }
    }

    // ---- chunk eligibility, recomputed --------------------------------------
    // This also returns what the chunks actually *delivered* per pile and
    // interval, which for a chunked pile is not the pile-wide average.
    let drawn = check_chunks(&mut checker, solution);
    if checker.cancelled() {
        return None;
    }

    // ---- blended inventory, recomputed --------------------------------------
    let mut openings: BTreeMap<(StockpileId, usize), f64> = BTreeMap::new();
    for pile in &input.piles {
        if checker.cancelled() {
            return None;
        }
        let state = replay_pile(&mut checker, pile, &solution.movements, &destinations, grades, segments, &mut openings, &drawn);
        checker.report.closing.push(state);
    }

    // ---- mandatory authored bar priority ------------------------------------
    check_bar_priority(&mut checker, solution, &openings, segments);
    if checker.cancelled() {
        return None;
    }

    checker.report.objective_difference = checker.report.replayed_objective - solution.reported_objective;
    let largest_unit_value = input
        .movements
        .iter()
        .filter_map(|movement| movement.value_per_tonne().ok())
        .map(f64::abs)
        .chain(input.conditional_values.iter().map(|entry| entry.value_per_tonne.abs()))
        .fold(0.0_f64, f64::max);
    let extraction_effect = solution.adjustments.movement_total_t * largest_unit_value;
    let arithmetic_effect = 1e-10 * solution.reported_objective.abs().max(1.0);
    if !solution.reported_objective.is_finite() || checker.report.objective_difference.abs() > extraction_effect + arithmetic_effect {
        checker.report.issues.push(format!(
            "published cashflow {:.9} does not reconcile with raw solver objective {:.9}; extraction permits {:.3e}",
            checker.report.replayed_objective,
            solution.reported_objective,
            extraction_effect + arithmetic_effect
        ));
    }

    Some(checker.report)
}

/// The accepted backend's window convention: a bar is worked in an interval
/// only when its authored window covers the whole interval.
fn covers(task: &crate::model::schedule::optimisation::Task, interval: crate::model::schedule::optimisation::Interval) -> bool {
    task.window_start_h <= interval.start_h + WINDOW_TOLERANCE_H && task.window_end_h >= interval.end_h - WINDOW_TOLERANCE_H
}

/// Recompute authored bar priority from the replayed physical state (§4).
///
/// The model's `ready` columns are not consulted. Readiness is recomputed
/// here from replayed ground remaining, replayed released pile inventory and
/// replayed cumulative reclaim, so a model that got its own readiness wrong
/// is caught rather than agreed with.
///
/// Two things are checked per loader and execution cell:
///
/// - **No economic preemption.** If a higher-priority bar had work available,
///   no lower-priority bar on that loader may have moved material.
/// - **No manufactured blocking.** A loader may move nothing at all - that is
///   ordinary idling - but when it does move material it must be under its
///   highest-priority ready bar, so standing down cannot be used to make a
///   lower-priority bar look like the only option.
fn check_bar_priority(checker: &mut Checker<'_>, solution: &BlendSolution, openings: &BTreeMap<(StockpileId, usize), f64>, segments: usize) {
    let input = checker.input;
    let loaders: Vec<_> = input.loaders.iter().map(|entry| entry.id).collect();

    for loader in loaders {
        if checker.cancelled() {
            return;
        }
        // Authored order: priority, then window start, then id.
        let mut bars: Vec<usize> = input.tasks.iter().enumerate().filter(|(_, task)| task.loader == loader).map(|(index, _)| index).collect();
        bars.sort_by(|&left, &right| {
            let left = &input.tasks[left];
            let right = &input.tasks[right];
            (left.priority, left.window_start_h, left.id)
                .partial_cmp(&(right.priority, right.window_start_h, right.id))
                .expect("authored windows are finite")
        });
        if bars.len() < 2 {
            continue;
        }

        // Physical state walked forward, cell by cell.
        let mut remaining: BTreeMap<_, f64> = input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect();
        let mut spent: BTreeMap<usize, f64> = bars.iter().map(|bar| (*bar, 0.0)).collect();

        for interval in &input.intervals {
            if checker.cancelled() {
                return;
            }
            for segment in 0..segments {
                let rate = input
                    .loaders
                    .iter()
                    .find(|entry| entry.id == loader)
                    .and_then(|entry| entry.rates.iter().find(|entry| entry.interval == interval.index));

                let ready = |bar: usize, remaining: &BTreeMap<_, f64>, spent: &BTreeMap<usize, f64>| -> bool {
                    let task = &input.tasks[bar];
                    if !covers(task, *interval) {
                        return false;
                    }
                    let Some(rate) = rate else { return false };
                    match &task.kind {
                        TaskKind::Dig { sequence } => rate.dig_tph > 0.0 && sequence.iter().any(|source| remaining.get(source).copied().unwrap_or(0.0) > REPLAY_TOLERANCE_T),
                        TaskKind::Reclaim { approved_sources, maximum_t } => {
                            if rate.reclaim_tph <= 0.0 {
                                return false;
                            }
                            if let Some(maximum) = maximum_t
                                && spent.get(&bar).copied().unwrap_or(0.0) >= maximum - REPLAY_TOLERANCE_T
                            {
                                return false;
                            }
                            approved_sources
                                .iter()
                                .any(|pile| openings.get(&(*pile, interval.index)).copied().unwrap_or(0.0) > REPLAY_TOLERANCE_T)
                        }
                    }
                };

                let highest = bars.iter().copied().find(|bar| ready(*bar, &remaining, &spent));

                // What this loader actually moved in this cell, by bar.
                for row in solution.movements.iter().filter(|row| row.interval == interval.index && row.segment == segment) {
                    if row.tonnes_t <= REPLAY_TOLERANCE_T {
                        continue;
                    }
                    let Some(candidate) = input.movements.get(row.candidate) else { continue };
                    if candidate.loader != loader {
                        continue;
                    }
                    let worked: Vec<usize> = bars.iter().copied().filter(|bar| task_authorises(&input.tasks[*bar], candidate)).collect();
                    if worked.is_empty() {
                        continue;
                    }
                    match highest {
                        None => checker.report.issues.push(format!(
                            "loader {} moved {:.6} t in interval {} segment {segment} with no bar holding available work",
                            loader.0, row.tonnes_t, interval.index
                        )),
                        Some(expected) if !worked.contains(&expected) => checker.report.issues.push(format!(
                            "authored priority broken: loader {} worked bar {} in interval {} segment {segment} while higher-priority bar {} had work available",
                            loader.0, input.tasks[worked[0]].id.0, interval.index, input.tasks[expected].id.0
                        )),
                        Some(_) => {}
                    }
                }

                // Advance the physical state past this cell.
                for row in solution.movements.iter().filter(|row| row.interval == interval.index && row.segment == segment) {
                    let Some(candidate) = input.movements.get(row.candidate) else { continue };
                    if let SourceId::Ground(ground) = candidate.source
                        && candidate.activity == Activity::Dig
                        && let Some(slot) = remaining.get_mut(&ground)
                    {
                        *slot -= row.tonnes_t;
                    }
                    if candidate.loader == loader && candidate.activity == Activity::Reclaim {
                        for bar in &bars {
                            if task_authorises(&input.tasks[*bar], candidate)
                                && let Some(slot) = spent.get_mut(bar)
                            {
                                *slot += row.tonnes_t;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Walk one pile forward through the horizon, recomputing its blend.
#[allow(clippy::needless_range_loop)]
fn replay_pile(
    checker: &mut Checker<'_>,
    pile: &BlendPile,
    movements: &[MovementRow],
    destinations: &BTreeMap<DestinationId, &Destination>,
    grades: usize,
    segments: usize,
    openings: &mut BTreeMap<(StockpileId, usize), f64>,
    drawn: &BTreeMap<(StockpileId, usize), (f64, Vec<f64>)>,
) -> ClosingState {
    let input = checker.input;
    let (mut open_t, mut open_q) = pile.total_opening(grades);

    for interval in &input.intervals {
        if checker.cancelled() {
            break;
        }
        let k = interval.index;
        openings.insert((pile.id, k), open_t);

        // Receipts and reclaims inside this interval, from the rows alone.
        let mut received_t = 0.0;
        let mut received_q = vec![0.0; grades];
        let mut reclaimed_t = 0.0;
        let mut per_segment_receipts = vec![0.0; segments];
        let mut per_segment_reclaim = vec![0.0; segments];

        for row in movements.iter().filter(|row| row.interval == k) {
            let Some(candidate) = input.movements.get(row.candidate) else { continue };
            let to_pile = destinations
                .get(&candidate.destination)
                .is_some_and(|destination| matches!(destination.kind, DestinationKind::Stockpile(target) if target == pile.id));
            if to_pile {
                received_t += row.tonnes_t;
                if let Some(slot) = per_segment_receipts.get_mut(row.segment) {
                    *slot += row.tonnes_t;
                }
                for g in 0..grades {
                    // A receipt carries identified material, so its contained
                    // quantity is known exactly.
                    match input.grades.fraction(candidate.material, g) {
                        Some(fraction) => received_q[g] += row.tonnes_t * fraction,
                        None => checker
                            .report
                            .issues
                            .push(format!("material {} delivered to pile {} has no grade {g}", candidate.material.0, pile.id.0)),
                    }
                }
            }
            if candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id) {
                reclaimed_t += row.tonnes_t;
                if let Some(slot) = per_segment_reclaim.get_mut(row.segment) {
                    *slot += row.tonnes_t;
                }
            }
        }

        // Inventory release timing: a reclaim in interval k may only draw on
        // the released opening blend, never on material received during k.
        checker.breach(&format!("pile {} reclaim against released opening in interval {k}", pile.id.0), reclaimed_t, open_t);

        // Physical occupancy throughout the interval (§3), recomputed from
        // the rows: receipts occupy on arrival, reclaim frees space as it
        // happens. Released inventory is a separate quantity and was checked
        // just above; conflating the two is what previously made a full pile
        // unable to receive while it was being drawn down.
        //
        // Rates are constant inside an execution segment, so occupancy is
        // linear across a segment's interior and the endpoint values bound
        // it throughout.
        let mut occupied = open_t;
        for segment in 0..segments {
            occupied += per_segment_receipts[segment] - per_segment_reclaim[segment];
            checker.breach(&format!("pile {} capacity in interval {k} segment {segment}", pile.id.0), occupied, pile.capacity_t);
            if occupied < -REPLAY_TOLERANCE_T {
                checker
                    .report
                    .issues
                    .push(format!("pile {} holds {occupied:.6} t in interval {k} segment {segment}", pile.id.0));
            }
        }

        // The blended grade, by division - the operation the model avoids.
        // An empty pile has no grade and must supply nothing.
        let blend: Vec<f64> = if open_t > REPLAY_TOLERANCE_T {
            let computed: Vec<f64> = (0..grades).map(|g| open_q[g] / open_t).collect();
            for (g, value) in computed.iter().enumerate() {
                checker.report.blends.insert((pile.id, k, g), *value);
            }
            computed
        } else {
            if reclaimed_t > REPLAY_TOLERANCE_T {
                checker
                    .report
                    .issues
                    .push(format!("pile {} supplied {reclaimed_t:.6} t from an empty pile in interval {k}", pile.id.0));
            }
            vec![0.0; grades]
        };
        // What a reclaim actually carried.
        //
        // For an unchunked pile that is the pile's own released blend. For a
        // **chunked** pile it is not: the draw comes from particular chunks,
        // and their composition can be far from the pile-wide average. An
        // earlier revision checked grade limits against the pile average on
        // chunked piles and so reported a breach on a schedule that was in
        // fact correct - the model constrains the blend of what was drawn,
        // which is the physically right quantity.
        let (reclaimed_q, delivered_blend): (Vec<f64>, Vec<f64>) = match drawn.get(&(pile.id, k)) {
            Some((chunk_t, chunk_q)) if !pile.chunks.is_empty() => {
                let fractions: Vec<f64> = (0..grades)
                    .map(|g| {
                        if *chunk_t > REPLAY_TOLERANCE_T {
                            chunk_q.get(g).copied().unwrap_or(0.0) / chunk_t
                        } else {
                            0.0
                        }
                    })
                    .collect();
                checker.residual(
                    &format!("pile {} chunk draws against its reclaim movements in interval {k}", pile.id.0),
                    *chunk_t,
                    reclaimed_t,
                );
                (fractions.iter().map(|fraction| fraction * reclaimed_t).collect(), fractions)
            }
            _ => (blend.iter().map(|fraction| fraction * reclaimed_t).collect(), blend.clone()),
        };

        // Grade eligibility on what was actually delivered (§6).
        check_grade_limits(checker, pile, k, reclaimed_t, &reclaimed_q, &delivered_blend, destinations, movements);
        for row in movements.iter().filter(|row| row.interval == k && row.tonnes_t > REPLAY_TOLERANCE_T) {
            let Some(candidate) = checker.input.movements.get(row.candidate) else { continue };
            if candidate.activity != Activity::Reclaim || candidate.source != SourceId::Stockpile(pile.id) {
                continue;
            }
            if let Some(qualification) = checker
                .input
                .qualifications
                .iter()
                .find(|entry| entry.loader == candidate.loader && entry.pile == pile.id && entry.destination == candidate.destination)
            {
                if !qualification.holds(&delivered_blend) {
                    checker.report.grade_issues.push(format!(
                        "pile {} delivered {:.6} t to destination {} in interval {k} outside every permitted grade range",
                        pile.id.0, row.tonnes_t, candidate.destination.0
                    ));
                }
            } else if !checker.input.qualifications.is_empty() {
                checker.report.grade_issues.push(format!(
                    "pile {} delivered {:.6} t to destination {} in interval {k} without a matching route qualification",
                    pile.id.0, row.tonnes_t, candidate.destination.0
                ));
            }
            for payment in checker.input.conditional_values.iter().filter(|entry| entry.candidate == row.candidate) {
                if payment.holds(&delivered_blend) {
                    checker.report.replayed_objective += row.tonnes_t * payment.value_per_tonne;
                }
            }
        }

        // Conservation across the interval boundary.
        let closing_t = open_t + received_t - reclaimed_t;
        if closing_t < -REPLAY_TOLERANCE_T {
            checker.report.issues.push(format!("pile {} closes interval {k} at {closing_t:.6} t", pile.id.0));
        }
        for g in 0..grades {
            let closing_q = open_q[g] + received_q[g] - reclaimed_q[g];
            if closing_q < -REPLAY_TOLERANCE_T {
                checker.report.issues.push(format!(
                    "pile {} closes interval {k} with negative contained quantity {closing_q:.6} in grade {g}",
                    pile.id.0
                ));
            }
            // Contained quantity can never exceed its own tonnage.
            if closing_q > closing_t.max(0.0) + REPLAY_TOLERANCE_T {
                checker.report.issues.push(format!(
                    "pile {} holds {closing_q:.6} t of grade {g} in {closing_t:.6} t of material after interval {k}",
                    pile.id.0
                ));
            }
            open_q[g] = closing_q;
        }
        open_t = closing_t;
    }

    ClosingState {
        pile: pile.id,
        tonnes_t: open_t,
        contained: open_q,
    }
}

/// Check every minimum-grade condition against the grade actually delivered.
///
/// A payment or eligibility must never be awarded on a grade the material did
/// not reach. The comparison uses the same documented margin the formulation
/// tightened by, so the replay agrees with the model about where the boundary
/// is rather than inventing a second convention.
#[allow(clippy::too_many_arguments)]
fn check_grade_limits(
    checker: &mut Checker<'_>,
    pile: &BlendPile,
    interval: usize,
    reclaimed_t: f64,
    _reclaimed_q: &[f64],
    blend: &[f64],
    destinations: &BTreeMap<DestinationId, &Destination>,
    movements: &[MovementRow],
) {
    if reclaimed_t <= REPLAY_TOLERANCE_T {
        return;
    }
    let limits: Vec<GradeLimit> = checker.input.grade_limits.clone();
    for limit in limits {
        if checker.cancelled() {
            return;
        }
        // Did any of this interval's reclaim from this pile actually reach the
        // limited destination?
        let delivered: f64 = movements
            .iter()
            .filter(|row| row.interval == interval)
            .filter(|row| {
                checker.input.movements.get(row.candidate).is_some_and(|candidate| {
                    candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id) && candidate.destination == limit.destination
                })
            })
            .map(|row| row.tonnes_t)
            .sum();
        if delivered <= 0.0 {
            continue;
        }
        let _ = destinations;
        // The same bound the formulation used as its big-M.
        let routed_capacity: f64 = checker
            .input
            .movements
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id) && candidate.destination == limit.destination)
            .map(|(_, candidate)| {
                let rate = checker
                    .input
                    .loaders
                    .iter()
                    .find(|loader| loader.id == candidate.loader)
                    .and_then(|loader| loader.rates.iter().find(|entry| entry.interval == interval))
                    .map(|entry| entry.reclaim_tph)
                    .unwrap_or(0.0);
                let duration = checker.input.intervals.get(interval).map(|entry| entry.duration_h()).unwrap_or(0.0);
                rate * duration * checker.input.segments_per_interval.max(1) as f64
            })
            .sum();
        let actual = blend.get(limit.grade).copied().unwrap_or(0.0);
        // The authored boundary itself, with only arithmetic slack. Using the
        // formulation's safety margin here would let the replay agree with the
        // model by construction and conceal exactly the failure it exists to
        // catch.
        const ARITHMETIC_SLACK: f64 = 1e-9;
        let qualifies = if limit.inclusive {
            actual >= limit.minimum - ARITHMETIC_SLACK
        } else {
            actual > limit.minimum + ARITHMETIC_SLACK
        };
        if !qualifies {
            // A shortfall is a shortfall whatever its size - but a delivery
            // small enough to be the indicator's own tolerance is a different
            // claim from a schedule that really ships off-spec material, and
            // conflating them would either hide a real breach or drown a
            // clean result in noise. Both are reported; only one fails.
            if delivered <= indicator_leak(routed_capacity) {
                checker.report.negligible_grade_deliveries += 1;
                checker.report.negligible_grade_tonnes_t += delivered;
            } else {
                checker.report.grade_issues.push(format!(
                    "pile {} delivered {delivered:.6} t to destination {} in interval {interval} at grade {actual:.6}, below the required {:.6}",
                    pile.id.0, limit.destination.0, limit.minimum
                ));
            }
        }
    }
}

/// Independently check the §8 chunk lifecycle against the published rows.
///
/// Every rule here is checked from the published chunk state, not from the
/// constraints that were supposed to enforce it.
fn check_chunks(checker: &mut Checker<'_>, solution: &BlendSolution) -> BTreeMap<(StockpileId, usize), (f64, Vec<f64>)> {
    let mut drawn: BTreeMap<(StockpileId, usize), (f64, Vec<f64>)> = BTreeMap::new();
    if solution.chunks.is_empty() {
        return drawn;
    }
    let piles: Vec<BlendPile> = checker.input.piles.clone();
    let grades = checker.input.grades.count();
    let horizon = checker.input.intervals.len();
    for pile in piles.iter().filter(|entry| !entry.chunks.is_empty()) {
        if checker.cancelled() {
            return drawn;
        }
        let count = pile.chunks.len();
        // The scale below which a lifecycle indicator's own tolerance, not a
        // decision, explains what the published rows say. Derived from the
        // chunk capacities; see [`chunk_dust`].
        let dust = chunk_dust(pile);
        checker.report.chunk_dust_threshold_t = checker.report.chunk_dust_threshold_t.max(dust);

        // Recompute each chunk's composition from its own published receipts.
        // A chunk's grade is never taken on trust: it is walked forward from
        // the authored opening through the receipts the solution says it took,
        // and the recomputed tonnage is checked against the published one, so
        // a chunk whose stated stock does not follow from its own history is
        // caught.
        let mut held_t: Vec<f64> = (0..count).map(|chunk| pile.chunk_opening.get(chunk).map(|(tonnes, _)| *tonnes).unwrap_or(0.0)).collect();
        let mut held_q: Vec<Vec<f64>> = (0..count)
            .map(|chunk| {
                let mut row = pile.chunk_opening.get(chunk).map(|(_, contained)| contained.clone()).unwrap_or_default();
                row.resize(grades, 0.0);
                row
            })
            .collect();
        for interval in 0..horizon {
            if checker.cancelled() {
                return drawn;
            }
            for chunk in 0..count {
                let key = super::formulation::chunk_key(chunk, interval, horizon);
                if held_t[chunk] > REPLAY_TOLERANCE_T {
                    for g in 0..grades {
                        checker.report.blends.insert((pile.id, key, g), held_q[chunk][g] / held_t[chunk]);
                    }
                }
                let Some(state) = solution
                    .chunks
                    .iter()
                    .find(|entry| entry.pile == pile.id && entry.chunk == chunk && entry.interval == interval)
                else {
                    continue;
                };
                checker.residual(
                    &format!("pile {} chunk {chunk} opening tonnes in interval {interval}", pile.id.0),
                    state.open_t,
                    held_t[chunk],
                );

                // A reclaim draws the chunk's own released blend.
                let fraction: Vec<f64> = (0..grades)
                    .map(|g| if held_t[chunk] > REPLAY_TOLERANCE_T { held_q[chunk][g] / held_t[chunk] } else { 0.0 })
                    .collect();
                let entry = drawn.entry((pile.id, interval)).or_insert_with(|| (0.0, vec![0.0; grades]));
                entry.0 += state.reclaimed_t;
                for g in 0..grades {
                    entry.1[g] += fraction[g] * state.reclaimed_t;
                    held_q[chunk][g] -= fraction[g] * state.reclaimed_t;
                }
                held_t[chunk] -= state.reclaimed_t;
                for (candidate, tonnes) in &state.receipts {
                    let Some(movement) = checker.input.movements.get(*candidate) else { continue };
                    held_t[chunk] += tonnes;
                    for g in 0..grades {
                        match checker.input.grades.fraction(movement.material, g) {
                            Some(value) => held_q[chunk][g] += tonnes * value,
                            None => checker
                                .report
                                .issues
                                .push(format!("material {} delivered to pile {} chunk {chunk} has no grade {g}", movement.material.0, pile.id.0)),
                        }
                    }
                }
                if held_t[chunk] < -REPLAY_TOLERANCE_T {
                    checker
                        .report
                        .issues
                        .push(format!("pile {} chunk {chunk} closes interval {interval} at {:.6} t", pile.id.0, held_t[chunk]));
                }
            }
        }

        for interval in 0..checker.input.intervals.len() {
            if checker.cancelled() {
                return drawn;
            }
            let row = |chunk: usize| -> Option<ChunkRow> {
                solution
                    .chunks
                    .iter()
                    .find(|entry| entry.pile == pile.id && entry.chunk == chunk && entry.interval == interval)
                    .cloned()
            };
            for chunk in 0..count {
                let Some(state) = row(chunk) else { continue };

                // Capacity is authored per chunk.
                checker.breach(
                    &format!("pile {} chunk {chunk} capacity in interval {interval}", pile.id.0),
                    state.open_t,
                    pile.chunks[chunk],
                );

                // A chunk may not receive while closed, nor be reclaimed while open.
                if state.closed && state.received_t > REPLAY_TOLERANCE_T {
                    if state.received_t <= dust {
                        checker.report.chunk_dust_events += 1;
                        checker.report.chunk_dust_tonnes_t += state.received_t;
                    } else {
                        checker.report.issues.push(format!(
                            "pile {} chunk {chunk} received {:.6} t in interval {interval} after being closed",
                            pile.id.0, state.received_t
                        ));
                    }
                }
                if !state.closed && state.reclaimed_t > REPLAY_TOLERANCE_T {
                    if state.reclaimed_t <= dust {
                        checker.report.chunk_dust_events += 1;
                        checker.report.chunk_dust_tonnes_t += state.reclaimed_t;
                    } else {
                        checker
                            .report
                            .issues
                            .push(format!("pile {} chunk {chunk} was reclaimed in interval {interval} before being closed", pile.id.0));
                    }
                }

                // Sequential fill: receiving into a chunk requires every
                // earlier chunk to be closed.
                if state.received_t > dust {
                    for earlier in 0..chunk {
                        if row(earlier).is_some_and(|entry| !entry.closed) {
                            checker.report.issues.push(format!(
                                "pile {} chunk {chunk} received in interval {interval} while chunk {earlier} was still open",
                                pile.id.0
                            ));
                        }
                    }
                }

                // Authored order among released, non-empty chunks.
                if state.reclaimed_t > dust {
                    match pile.order {
                        crate::model::schedule::optimisation::ReclaimOrder::Fifo => {
                            for earlier in 0..chunk {
                                if row(earlier).is_some_and(|entry| entry.open_t > dust) {
                                    checker.report.issues.push(format!(
                                        "FIFO violated: pile {} drew chunk {chunk} in interval {interval} while chunk {earlier} still held material",
                                        pile.id.0
                                    ));
                                }
                            }
                        }
                        crate::model::schedule::optimisation::ReclaimOrder::Lifo => {
                            for later in (chunk + 1)..count {
                                if row(later).is_some_and(|entry| entry.closed && entry.open_t > dust) {
                                    checker.report.issues.push(format!(
                                        "LIFO violated: pile {} drew chunk {chunk} in interval {interval} while released chunk {later} still held material",
                                        pile.id.0
                                    ));
                                }
                            }
                        }
                    }
                }

                // No slot reuse: a chunk closed in one interval stays closed.
                if interval + 1 < checker.input.intervals.len() {
                    let next = solution
                        .chunks
                        .iter()
                        .find(|entry| entry.pile == pile.id && entry.chunk == chunk && entry.interval == interval + 1);
                    if state.closed && next.is_some_and(|entry| !entry.closed) {
                        checker.report.issues.push(format!("pile {} chunk {chunk} reopened after interval {interval}", pile.id.0));
                    }
                }
            }
        }
    }
    drawn
}
