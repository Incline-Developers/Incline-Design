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
//! `offer_seed` in `app/scip_blend.rs`). A backend's own verdict is
//! evidence, not proof.
//!
//! The blended grade is recomputed here by *division* - `Q / T` on the
//! replayed opening state - which is exactly the operation the formulation
//! had to avoid. That is the point: the model's cleared bilinear form and the
//! replay's direct ratio are independent routes to the same number, so a
//! disagreement between them is detectable.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
};

use super::input::{BlendInput, BlendPile, GradeLimit, WINDOW_TOLERANCE_H, attribute_reclaim, authored_tasks, dig_authority, task_authorises};
use crate::model::schedule::optimisation::{Activity, Destination, DestinationId, DestinationKind, GroundId, LoaderId, SourceId, StockpileId, TaskKind};

/// One published movement: how much material a candidate moved in a cell.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct MovementRow {
    pub(crate) candidate: usize,
    pub(crate) interval: usize,
    pub(crate) segment: usize,
    pub(crate) tonnes_t: f64,
}

/// One chunk's published state in one interval.
///
/// Published for every live chunk: the one receiving, and any holding
/// material or drawn from. A chunk with no row in an interval holds nothing -
/// not yet reached, or emptied and closed - and keeps the state its last row
/// gave it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct BlendSolution {
    pub(crate) movements: Vec<MovementRow>,
    #[serde(with = "pairs")]
    pub(crate) durations: BTreeMap<(usize, usize), f64>,
    /// Empty for unchunked piles.
    pub(crate) chunks: Vec<ChunkRow>,
    /// The backend's raw objective, for comparison against the published rows.
    pub(crate) reported_objective: f64,
    /// Tiny solver columns omitted when extracting the published schedule.
    /// These are reported in aggregate so numerical changes remain visible.
    pub(crate) adjustments: ExtractionAdjustments,
    /// What drill and blast did, from the hourly dispatch that simulated it;
    /// a later solve keeps the dispatch's.
    #[serde(default)]
    pub(crate) drill_blast: Option<super::drill_blast::DrillBlastTimeline>,
}

/// A map as a list of pairs, for keys JSON cannot hold as object keys.
mod pairs {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<K: Serialize, V: Serialize, S: Serializer>(map: &BTreeMap<K, V>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(map)
    }

    pub(super) fn deserialize<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error> {
        Ok(Vec::<(K, V)>::deserialize(deserializer)?.into_iter().collect())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ExtractionAdjustments {
    pub(crate) movement_count: usize,
    pub(crate) movement_total_t: f64,
    pub(crate) movement_max_t: f64,
    pub(crate) chunk_receipt_count: usize,
    pub(crate) chunk_receipt_total_t: f64,
    pub(crate) chunk_receipt_max_t: f64,
}

impl ExtractionAdjustments {
    #[cfg_attr(not(feature = "scip"), allow(dead_code, reason = "used by Improve"))]
    pub(crate) fn movement(&mut self, tonnes: f64) {
        self.movement_count += 1;
        self.movement_total_t += tonnes.abs();
        self.movement_max_t = self.movement_max_t.max(tonnes.abs());
    }

    #[cfg_attr(not(feature = "scip"), allow(dead_code, reason = "used by Improve"))]
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
    pub(crate) movement_contained: BTreeMap<(usize, usize), Vec<f64>>,
    pub(crate) target_totals: BTreeMap<(usize, u32), (f64, f64)>,
    pub(crate) grade_target_penalty: f64,
    /// Money corresponding to the contained-tonne feasibility tolerance in
    /// target allocation and deviation rows. Never replaces the replayed cost.
    pub(crate) target_value_tolerance: f64,
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
    /// [`indicator_leak`]). Published rather than discarded: it is a
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
    /// Conditional-value deliveries whose blend lay within
    /// [`super::input::GRADE_MARGIN`] of one of the rule's boundaries, where
    /// the model values the rule conservatively (see `GRADE_MARGIN`). The
    /// published objective uses the authored boundary; `boundary_value_slack`
    /// bounds how far that can exceed the model's own objective.
    pub(crate) boundary_rows: usize,
    pub(crate) boundary_tonnes_t: f64,
    pub(crate) boundary_value_slack: f64,
    /// How far the model's own objective can overstate the published one
    /// through conditional-value indicators, derived rather than chosen.
    ///
    /// Each conditional payment is gated by a big-M row, `paid <= M x
    /// indicator` (or its mirror for a cost), and a binary may sit one
    /// integrality tolerance off its value; so a row can be credited up to
    /// `M x tolerance` tonnes the authored rule would not pay - never more
    /// than the row itself moved. This is that leak, summed, at the authored
    /// value per tonne. The published objective is still the authored one;
    /// this only bounds how far the solver's raw figure may sit above it.
    pub(crate) indicator_leak_value: f64,
    /// The authored bar each published movement row was worked under,
    /// aligned with [`BlendSolution::movements`] and recomputed from the
    /// replayed physical state: the highest-priority ready bar that
    /// authorises the row's source. `None` only for a row no bar authorises,
    /// which is itself reported as an issue.
    pub(crate) row_tasks: Vec<Option<crate::model::schedule::optimisation::TaskId>>,
    /// Every pile's replayed state across every interval, keyed
    /// `(pile, interval)`, so publication reads balances and blends off the
    /// replay instead of recomputing them a second way.
    pub(crate) pile_intervals: BTreeMap<(StockpileId, usize), PileInterval>,
    /// Every chunk's replayed closing tonnes and contained quantity per
    /// grade, keyed `(pile, chunk, interval)`: walked forward from the opening
    /// through the chunk's own published receipts and draws, so a day-by-day
    /// window opens from state the replay recomputed rather than the model's
    /// own figures.
    pub(crate) chunk_intervals: BTreeMap<(StockpileId, usize, usize), (f64, Vec<f64>)>,
}

/// One pile's replayed balance over one interval.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PileInterval {
    pub(crate) opening_t: f64,
    pub(crate) received_t: f64,
    pub(crate) reclaimed_t: f64,
    pub(crate) closing_t: f64,
    /// Contained quantity per grade at the interval's close.
    pub(crate) closing_q: Vec<f64>,
    /// The blend this interval's reclaim actually carried: the released
    /// opening blend for an unchunked pile, the drawn chunks' blend for a
    /// chunked one. Empty when nothing was reclaimed.
    pub(crate) delivered_blend: Vec<f64>,
}

impl ReplayReport {
    /// Publishable: every physical *and* grade check passed.
    pub(crate) fn is_valid(&self) -> bool {
        self.issues.is_empty() && self.grade_issues.is_empty()
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
        if !input.grade_targets.is_empty() && candidate.activity != Activity::Reclaim {
            let contained = checker.report.movement_contained.entry((row.candidate, row.interval)).or_insert_with(|| vec![0.0; grades]);
            for (g, q) in contained.iter_mut().enumerate() {
                if let Some(fraction) = input.grades.fraction(candidate.material, g) {
                    *q += row.tonnes_t * fraction;
                } else if input.grade_targets.iter().any(|t| t.grade == g && t.destination == candidate.destination) {
                    checker.report.issues.push(format!("grade target movement {} has no grade {g}", row.candidate));
                }
            }
        }
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

    // ---- authored dig-block order -------------------------------------------
    // Walked cell by cell from the rows. A loader may work several blocks of
    // its sequence back to back inside one segment, so the later block needs
    // the earlier one gone by the end of the same cell - unless another
    // loader also dug the earlier block in that cell, when nothing says who
    // finished it first and the earlier block must be gone a cell before.
    {
        let mut remaining: BTreeMap<_, f64> = input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect();
        let ordered: BTreeMap<_, Vec<usize>> = input.loaders.iter().enumerate().map(|(index, loader)| (loader.id, authored_tasks(input, index))).collect();
        let tolerance = |ground| {
            input
                .ground
                .iter()
                .find(|source| source.id == ground)
                .map_or(REPLAY_TOLERANCE_T, |source| indicator_leak(source.tonnes_t).max(REPLAY_TOLERANCE_T))
        };
        for interval in &input.intervals {
            if checker.cancelled() {
                return None;
            }
            for segment in 0..segments {
                let before = remaining.clone();
                // Tonnes each (loader, block) dug in this cell.
                let mut dug: BTreeMap<_, f64> = BTreeMap::new();
                for row in by_cell.get(&(interval.index, segment)).map(Vec::as_slice).unwrap_or_default() {
                    let Some(candidate) = input.movements.get(row.candidate) else { continue };
                    let SourceId::Ground(ground) = candidate.source else { continue };
                    if candidate.activity != Activity::Dig {
                        continue;
                    }
                    *dug.entry((candidate.loader, ground)).or_default() += row.tonnes_t;
                    if let Some(slot) = remaining.get_mut(&ground) {
                        *slot -= row.tonnes_t;
                    }
                }
                // Each dig is held only to the order of the bar it was
                // worked under; see `dig_authority`.
                for (&(loader, later), _) in dug.iter().filter(|(_, tonnes)| **tonnes > REPLAY_TOLERANCE_T) {
                    let Some(ordered) = ordered.get(&loader) else { continue };
                    let Some(task) = dig_authority(input, ordered, later, *interval).map(|index| &input.tasks[index]) else {
                        continue;
                    };
                    let TaskKind::Dig { sequence } = &task.kind else { continue };
                    for pair in sequence.windows(2).filter(|pair| pair[1] == later) {
                        let [earlier, later] = pair else { continue };
                        let Some(&left) = remaining.get(earlier) else { continue };
                        let shared = dug
                            .iter()
                            .any(|(&(other, ground), &tonnes)| other != task.loader && ground == *earlier && tonnes > REPLAY_TOLERANCE_T);
                        let left = if shared { before.get(earlier).copied().unwrap_or(0.0) } else { left };
                        if left > tolerance(*earlier) {
                            checker.report.issues.push(format!(
                                "authored order broken: loader {} dug block {} in interval {} segment {segment} while block {} still held {left:.6} t",
                                task.loader.0, later.0, interval.index, earlier.0
                            ));
                        }
                    }
                }
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
                // One pile at a time: a loader switches piles only at a
                // segment boundary, as the formulation's `onesrc` rows say.
                let mut drawn: BTreeSet<StockpileId> = BTreeSet::new();
                for row in rows.iter().filter(|row| row.tonnes_t > REPLAY_TOLERANCE_T) {
                    if let Some(candidate) = input.movements.get(row.candidate)
                        && candidate.loader == loader.id
                        && candidate.activity == Activity::Reclaim
                        && let SourceId::Stockpile(pile) = candidate.source
                    {
                        drawn.insert(pile);
                    }
                }
                if drawn.len() > 1 {
                    let piles: Vec<String> = drawn.iter().map(|pile| pile.0.to_string()).collect();
                    checker.report.issues.push(format!(
                        "loader {} reclaimed from piles {} at once in interval {} segment {segment}",
                        loader.id.0,
                        piles.join(", "),
                        interval.index
                    ));
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
            DestinationKind::Dump => {
                let Some(capacity) = destination.capacity_t else { continue };
                let used = delivered(&|_| true);
                checker.breach(&format!("destination {} capacity", destination.id.0), used, capacity);
            }
            // Independently reconstructed segment occupancy in replay_pile
            // checks storage capacity, including opening stock and reclaim.
            DestinationKind::Stockpile(_) => {}
        }
    }

    // ---- reclaim caps -------------------------------------------------------
    // Each movement counts against the bar it was worked under, in time
    // order, exactly as dispatch charged it.
    let mut reclaimed: BTreeMap<usize, f64> = BTreeMap::new();
    let mut rows: Vec<_> = solution.movements.iter().collect();
    rows.sort_by_key(|row| row.interval);
    for row in rows {
        let (Some(candidate), Some(interval)) = (input.movements.get(row.candidate), input.intervals.get(row.interval)) else {
            continue;
        };
        if candidate.activity != Activity::Reclaim {
            continue;
        }
        for (task, share) in attribute_reclaim(input, candidate, *interval, row.tonnes_t, &reclaimed) {
            *reclaimed.entry(task).or_default() += share;
        }
    }
    for (index, task) in input.tasks.iter().enumerate() {
        if checker.cancelled() {
            return None;
        }
        let TaskKind::Reclaim { maximum_t: Some(maximum), .. } = &task.kind else { continue };
        checker.breach(&format!("reclaim cap on task {}", task.id.0), reclaimed.get(&index).copied().unwrap_or(0.0), *maximum);
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

    // ---- drill and blast releases ------------------------------------------
    // Ground is dug only once its blast has released it: by the dispatch's
    // own timeline when the schedule carries one, otherwise by the release
    // times the input fixes.
    if let Some(chain) = input.drill_blast.as_ref() {
        let releases: BTreeMap<GroundId, f64> = match solution.drill_blast.as_ref() {
            Some(timeline) if timeline.blasts.len() == chain.blasts.len() => timeline.releases(chain).into_iter().collect(),
            _ => chain.releases(),
        };
        if let Some(timeline) = solution.drill_blast.as_ref().or(chain.fixed.as_ref()) {
            if timeline.blasts.len() != chain.blasts.len() {
                checker.report.issues.push("drill and blast timeline has the wrong number of blasts".to_owned());
            }
            for (ground, deadline) in chain.clearance_deadlines(timeline) {
                let Some(source) = input.ground.iter().find(|source| source.id == ground) else {
                    continue;
                };
                let dug: f64 = solution
                    .movements
                    .iter()
                    .filter(|row| {
                        input.intervals.get(row.interval).is_some_and(|interval| interval.end_h <= deadline + 1e-9)
                            && input
                                .movements
                                .get(row.candidate)
                                .is_some_and(|candidate| candidate.activity == Activity::Dig && candidate.source == SourceId::Ground(ground))
                    })
                    .map(|row| row.tonnes_t)
                    .sum();
                if source.tonnes_t - dug > indicator_leak(source.tonnes_t).max(REPLAY_TOLERANCE_T) {
                    checker.report.issues.push(format!("block {} still standing at blast clearance hour {deadline}", ground.0));
                }
            }
            // Check the machine work independently of the solver's release data.
            // Window solves carry full-horizon work; only audit machine rates in
            // the intervals this input holds.
            for (position, work) in timeline.work.iter().enumerate() {
                let (Some(agent), Some(blast), Some(events)) = (chain.agents.get(work.agent), chain.blasts.get(work.blast), timeline.blasts.get(work.blast)) else {
                    checker.report.issues.push("drill and blast work names an unknown machine or blast".to_owned());
                    continue;
                };
                let step = work.activity as usize;
                let before = if step == 0 { events.cleared_h } else { events.done_h[step - 1] };
                let duration = work.end_h - work.start_h;
                if !work.quantity.is_finite()
                    || work.quantity < 0.0
                    || !duration.is_finite()
                    || duration <= 0.0
                    || agent.activity != work.activity
                    || !before.is_some_and(|at| at <= work.start_h + 1e-9)
                    || blast.stage.has_done(work.activity)
                    || events.done_h[step].is_some_and(|done| work.end_h > done + 1e-9)
                {
                    checker.report.issues.push(format!("invalid drill and blast work span {position}"));
                    continue;
                }
                if timeline.work[..position]
                    .iter()
                    .any(|prior| prior.agent == work.agent && prior.start_h < work.end_h - 1e-9 && work.start_h < prior.end_h - 1e-9)
                {
                    checker.report.issues.push(format!("drill and blast machine {} works overlapping spans", work.agent));
                }
                for interval in &input.intervals {
                    if work.end_h <= interval.start_h || work.start_h >= interval.end_h {
                        continue;
                    }
                    if work.quantity / duration > agent.rates.get(interval.index).copied().unwrap_or(0.0) + 1e-6 {
                        checker
                            .report
                            .issues
                            .push(format!("drill and blast machine {} exceeds its rate in interval {}", work.agent, interval.index));
                    }
                }
            }
            for (blast_index, (blast, events)) in chain.blasts.iter().zip(&timeline.blasts).enumerate() {
                if blast.stage == crate::model::schedule::BlastStage::NotStarted && blast.never_clear && events.cleared_h.is_some() {
                    checker.report.issues.push(format!("blast {blast_index} clears unmineable ground"));
                }
                for activity in crate::model::schedule::BlastActivity::ALL {
                    let step = activity as usize;
                    let worked: f64 = timeline
                        .work
                        .iter()
                        .filter(|work| work.blast == blast_index && work.activity == activity)
                        .map(|work| work.quantity)
                        .sum();
                    if worked > blast.quantity[step] + 1e-6 || (events.done_h[step].is_some() && !blast.stage.has_done(activity) && (worked - blast.quantity[step]).abs() > 1e-6) {
                        checker.report.issues.push(format!("blast {blast_index} has inconsistent {activity:?} work"));
                    }
                }
                if blast.stage != crate::model::schedule::BlastStage::Fired
                    && events
                        .fired_h
                        .is_some_and(|fired| !events.done_h[2].is_some_and(|charged| (chain.fires_at(charged) - fired).abs() < 1e-9))
                {
                    checker.report.issues.push(format!("blast {blast_index} fires outside its window"));
                }
            }
        }
        for row in &solution.movements {
            if row.tonnes_t <= REPLAY_TOLERANCE_T {
                continue;
            }
            let Some(candidate) = input.movements.get(row.candidate) else { continue };
            let Some(interval) = input.intervals.get(row.interval) else { continue };
            if let SourceId::Ground(ground) = candidate.source
                && releases.get(&ground).is_some_and(|released| *released > interval.start_h + 1e-9)
            {
                checker
                    .report
                    .issues
                    .push(format!("block {} dug in interval {} before its blast released it", ground.0, row.interval));
            }
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

    checker.report.target_totals = target_totals(input, solution, &checker.report.movement_contained);
    checker.report.grade_target_penalty = target_penalty(input, &checker.report.target_totals);
    checker.report.replayed_objective -= checker.report.grade_target_penalty;
    // Each reclaim cell allocates contained metal independently; each hinge
    // introduces a linear feasibility residual. Price those physical residuals
    // at their authored slopes instead of relaxing reconciliation by a dollar
    // constant or a fraction of the entire objective.
    for &(index, period) in checker.report.target_totals.keys() {
        let target = &input.grade_targets[index];
        let cells: BTreeSet<_> = solution
            .movements
            .iter()
            .filter(|row| {
                input
                    .movements
                    .get(row.candidate)
                    .is_some_and(|movement| movement.destination == target.destination && movement.activity == Activity::Reclaim)
                    && input
                        .intervals
                        .get(row.interval)
                        .is_some_and(|interval| crate::model::schedule::grade_targets::target_day(interval.start_h) == period)
            })
            .map(|row| (row.candidate, row.interval, row.segment))
            .collect();
        let slopes: f64 = target.specification.hinges().iter().map(|&(_, _, slope)| slope).sum();
        checker.report.target_value_tolerance += REPLAY_TOLERANCE_T * slopes * (1.0 + cells.len() as f64);
    }
    checker.report.boundary_value_slack += checker.report.target_value_tolerance;
    checker.report.objective_difference = checker.report.replayed_objective - solution.reported_objective;
    let ceilings = super::input::grade_ceilings(input);
    let largest_unit_value = input
        .movements
        .iter()
        .filter_map(|movement| movement.value_per_tonne().ok())
        .map(f64::abs)
        .chain(input.conditional_values.iter().map(|entry| entry.value_per_tonne.abs()))
        .chain(input.grade_targets.iter().map(|target| {
            target
                .specification
                .hinges()
                .iter()
                .map(|&(boundary, _, slope)| slope * boundary.abs().max((ceilings[target.grade] - boundary).abs()))
                .sum::<f64>()
        }))
        .fold(0.0_f64, f64::max);
    let extraction_effect = solution.adjustments.movement_total_t * largest_unit_value;
    let arithmetic_effect = 1e-10 * solution.reported_objective.abs().max(1.0);
    // The model values a conditional rule conservatively within one margin of
    // its boundary, so the authored (published) value may exceed the raw
    // objective by at most what those deliveries could shift - never fall
    // below it, apart from the separately priced target feasibility residuals.
    let difference = checker.report.objective_difference;
    let below = -(extraction_effect + arithmetic_effect + checker.report.indicator_leak_value + checker.report.target_value_tolerance);
    let above = extraction_effect + arithmetic_effect + checker.report.boundary_value_slack;
    if !solution.reported_objective.is_finite() || difference < below || difference > above {
        checker.report.issues.push(format!(
            "published net value {:.9} does not reconcile with raw solver objective {:.9}; extraction permits {:.3e}, grade/target allowance {:.3e} and indicator tolerance {:.3e}",
            checker.report.replayed_objective,
            solution.reported_objective,
            extraction_effect + arithmetic_effect,
            checker.report.boundary_value_slack,
            checker.report.indicator_leak_value
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
    checker.report.row_tasks = vec![None; solution.movements.len()];
    // Rows by cell, once: walking every published row for every loader and
    // every cell is the horizon squared.
    let mut cell_rows: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (index, row) in solution.movements.iter().enumerate() {
        cell_rows.entry((row.interval, row.segment)).or_default().push(index);
    }
    let empty = Vec::new();

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
        if bars.is_empty() {
            continue;
        }
        // Priority can only be broken between two bars; a single bar still
        // needs its rows attributed.
        let check = bars.len() > 1;

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
                            approved_sources.iter().any(|pile| {
                                input.piles.iter().find(|entry| entry.id == *pile).is_some_and(|entry| entry.reclaims(*interval))
                                    && openings.get(&(*pile, interval.index)).copied().unwrap_or(0.0) > REPLAY_TOLERANCE_T
                            })
                        }
                        TaskKind::Delay => true,
                    }
                };

                let highest = bars.iter().copied().find(|bar| ready(*bar, &remaining, &spent));
                let rows = cell_rows.get(&(interval.index, segment)).unwrap_or(&empty);

                // What this loader actually moved in this cell, by bar.
                for &index in rows {
                    let row = solution.movements[index];
                    let Some(candidate) = input.movements.get(row.candidate) else { continue };
                    if candidate.loader != loader {
                        continue;
                    }
                    let worked: Vec<usize> = bars
                        .iter()
                        .copied()
                        .filter(|bar| covers(&input.tasks[*bar], *interval) && task_authorises(&input.tasks[*bar], candidate))
                        .collect();
                    // The bar the row was worked under: the highest-priority
                    // ready one when it authorises the row, and otherwise the
                    // first authorising bar - which only a row the checks
                    // below reject can need.
                    let attributed = highest.filter(|bar| worked.contains(bar)).or_else(|| worked.first().copied());
                    checker.report.row_tasks[index] = attributed.map(|bar| input.tasks[bar].id);
                    if row.tonnes_t <= REPLAY_TOLERANCE_T || worked.is_empty() || !check {
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

                // Advance the physical state past this cell. Ground is shared
                // by every loader; a reclaim cap is this loader's own.
                for &index in rows {
                    let row = solution.movements[index];
                    let Some(candidate) = input.movements.get(row.candidate) else { continue };
                    if let SourceId::Ground(ground) = candidate.source
                        && candidate.activity == Activity::Dig
                        && let Some(slot) = remaining.get_mut(&ground)
                    {
                        *slot -= row.tonnes_t;
                    }
                    if candidate.loader == loader && candidate.activity == Activity::Reclaim {
                        for (bar, share) in attribute_reclaim(input, candidate, *interval, row.tonnes_t, &spent) {
                            *spent.entry(bar).or_default() += share;
                        }
                    }
                }
            }
        }
    }
}

/// Walk one pile forward through the horizon, recomputing its blend.
#[allow(clippy::needless_range_loop, clippy::too_many_arguments)]
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
    let ceilings = super::input::grade_ceilings(input);
    let mut last_receipt_h = pile.last_receipt_h;

    for interval in &input.intervals {
        if checker.cancelled() {
            break;
        }
        let k = interval.index;
        let interval_duration = interval.duration_h();
        // An unchunked pile still resting has no work for a reclaim bar.
        let resting = pile.chunks.is_empty() && last_receipt_h.is_some_and(|received_h| !pile.rested(received_h, *interval));
        openings.insert((pile.id, k), if resting { 0.0 } else { open_t });

        // Receipts and reclaims inside this interval, from the rows alone.
        let mut received_t = 0.0;
        let mut received_q = vec![0.0; grades];
        let mut reclaimed_t = 0.0;
        let mut per_segment_receipts = vec![0.0; segments];
        let mut per_segment_reclaim = vec![0.0; segments];
        // Blocks each loader dug per segment, and which loaders delivered
        // here: a loader that worked two blocks back to back delivered at an
        // uneven rate, so occupancy can peak inside that segment.
        let mut blocks_dug: BTreeMap<(usize, LoaderId), BTreeSet<GroundId>> = BTreeMap::new();
        let mut delivering: BTreeSet<(usize, LoaderId)> = BTreeSet::new();

        for row in movements.iter().filter(|row| row.interval == k) {
            let Some(candidate) = input.movements.get(row.candidate) else { continue };
            let to_pile = destinations
                .get(&candidate.destination)
                .is_some_and(|destination| matches!(destination.kind, DestinationKind::Stockpile(target) if target == pile.id));
            if candidate.activity == Activity::Dig
                && row.tonnes_t > REPLAY_TOLERANCE_T
                && let SourceId::Ground(ground) = candidate.source
            {
                blocks_dug.entry((row.segment, candidate.loader)).or_default().insert(ground);
            }
            if to_pile && row.tonnes_t > REPLAY_TOLERANCE_T {
                delivering.insert((row.segment, candidate.loader));
            }
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

        if resting {
            checker.breach(&format!("pile {} reclaim before its rest, interval {k}", pile.id.0), reclaimed_t, 0.0);
        }
        if pile.exclusive && received_t > REPLAY_TOLERANCE_T && reclaimed_t > REPLAY_TOLERANCE_T {
            checker.report.issues.push(format!(
                "pile {} received {received_t:.6} t and was reclaimed {reclaimed_t:.6} t in interval {k}, but may not do both at once",
                pile.id.0
            ));
        }
        if received_t > super::input::REST_RECEIPT_T {
            last_receipt_h = Some(interval.end_h);
        }

        // The authored mode of the interval's day.
        if !pile.builds(*interval) {
            checker.breach(&format!("pile {} receipts on a day it is not building, interval {k}", pile.id.0), received_t, 0.0);
        }
        if !pile.reclaims(*interval) {
            checker.breach(&format!("pile {} reclaim on a day it is not reclaiming, interval {k}", pile.id.0), reclaimed_t, 0.0);
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
        // it throughout - except where a loader delivering here worked blocks
        // back to back. There the segment's receipts must fit on top of its
        // opening occupancy, with no credit for its own reclaim.
        let mut occupied = open_t;
        for segment in 0..segments {
            let uneven = delivering
                .iter()
                .any(|&(cell, loader)| cell == segment && blocks_dug.get(&(segment, loader)).is_some_and(|blocks| blocks.len() > 1));
            if uneven && per_segment_reclaim[segment] > REPLAY_TOLERANCE_T {
                checker.breach(
                    &format!("pile {} capacity while blocks were worked back to back in interval {k} segment {segment}", pile.id.0),
                    occupied + per_segment_receipts[segment],
                    pile.capacity_t,
                );
            }
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
        for row in movements.iter().filter(|row| row.interval == k) {
            if !checker.input.grade_targets.is_empty()
                && checker
                    .input
                    .movements
                    .get(row.candidate)
                    .is_some_and(|candidate| candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id))
            {
                let contained = checker.report.movement_contained.entry((row.candidate, k)).or_insert_with(|| vec![0.0; grades]);
                for (q, fraction) in contained.iter_mut().zip(&delivered_blend) {
                    *q += row.tonnes_t * fraction;
                }
            }
        }
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
                // The model's `M` for this payment's rows is the loader's rate
                // over the interval; see `formulate`.
                let big_m = super::input::loader_rate(checker.input, candidate, k).unwrap_or(0.0) * interval_duration;
                checker.report.indicator_leak_value += row.tonnes_t.min(big_m * INTEGRALITY_TOLERANCE * INTEGRALITY_MARGIN) * payment.value_per_tonne.abs();
                if payment.near_boundary(&delivered_blend, reclaimed_t) {
                    checker.report.boundary_rows += 1;
                    checker.report.boundary_tonnes_t += row.tonnes_t;
                    checker.report.boundary_value_slack += row.tonnes_t * payment.value_per_tonne.abs();
                }
            }
        }

        // Conservation across the interval boundary.
        let closing_t = open_t + received_t - reclaimed_t;
        let opening_t = open_t;
        let delivered = if reclaimed_t > REPLAY_TOLERANCE_T { delivered_blend.clone() } else { Vec::new() };
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
            // Weighted content cannot exceed tonnes × the richest source grade.
            if closing_q > closing_t.max(0.0) * ceilings[g] + REPLAY_TOLERANCE_T {
                checker.report.issues.push(format!(
                    "pile {} holds {closing_q:.6} t of grade {g} in {closing_t:.6} t of material after interval {k}",
                    pile.id.0
                ));
            }
            open_q[g] = closing_q;
        }
        open_t = closing_t;
        checker.report.pile_intervals.insert(
            (pile.id, k),
            PileInterval {
                opening_t,
                received_t,
                reclaimed_t,
                closing_t,
                closing_q: open_q.clone(),
                delivered_blend: delivered,
            },
        );
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
#[allow(clippy::needless_range_loop)]
fn check_chunks(checker: &mut Checker<'_>, solution: &BlendSolution) -> BTreeMap<(StockpileId, usize), (f64, Vec<f64>)> {
    let mut drawn: BTreeMap<(StockpileId, usize), (f64, Vec<f64>)> = BTreeMap::new();
    if solution.chunks.is_empty() {
        return drawn;
    }
    let piles: Vec<BlendPile> = checker.input.piles.clone();
    let grades = checker.input.grades.count();
    let horizon = checker.input.intervals.len();
    // Each published row by its key, indexed once; the first of any repeat,
    // as a scan would find it.
    let mut published: BTreeMap<(StockpileId, usize, usize), &ChunkRow> = BTreeMap::new();
    for row in &solution.chunks {
        published.entry((row.pile, row.chunk, row.interval)).or_insert(row);
    }
    // The chunks each pile publishes in each interval, in chunk order.
    let mut live: BTreeMap<(StockpileId, usize), Vec<usize>> = BTreeMap::new();
    for &(pile, chunk, interval) in published.keys() {
        live.entry((pile, interval)).or_default().push(chunk);
    }
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
                let Some(state) = published.get(&(pile.id, chunk, interval)) else {
                    // Only a chunk holding nothing may go unpublished.
                    if held_t[chunk] > REPLAY_TOLERANCE_T {
                        checker.report.issues.push(format!(
                            "pile {} chunk {chunk} holds {:.6} t in interval {interval} but has no published row",
                            pile.id.0, held_t[chunk]
                        ));
                        checker.report.chunk_intervals.insert((pile.id, chunk, interval), (held_t[chunk], held_q[chunk].clone()));
                    }
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
                checker.report.chunk_intervals.insert((pile.id, chunk, interval), (held_t[chunk], held_q[chunk].clone()));
            }
        }

        // Each chunk's state walked forward through its rows: closed or not,
        // and since when, for its rest. A chunk with no row keeps its state.
        let mut closed_state: Vec<bool> = (0..count).map(|chunk| pile.chunk_starts_closed(chunk)).collect();
        let mut closed_since: Vec<Option<f64>> = (0..count)
            .map(|chunk| {
                pile.chunk_starts_closed(chunk)
                    .then(|| pile.chunk_closed_h.get(chunk).copied().flatten().unwrap_or(f64::NEG_INFINITY))
            })
            .collect();
        for interval in 0..horizon {
            if checker.cancelled() {
                return drawn;
            }
            let at = checker.input.intervals[interval];
            let here: &[usize] = live.get(&(pile.id, interval)).map(Vec::as_slice).unwrap_or_default();

            // Transitions first, so the checks below read every chunk's
            // state in this interval.
            let mut closing: Vec<usize> = Vec::new();
            for &chunk in here {
                let state = published[&(pile.id, chunk, interval)];
                if closed_state[chunk] && !state.closed {
                    if interval == 0 && pile.chunk_starts_closed(chunk) {
                        checker
                            .report
                            .issues
                            .push(format!("pile {} chunk {chunk} opens the horizon open but must start closed", pile.id.0));
                    } else {
                        // A chunk is filled once: an emptied one is not refilled.
                        checker.report.issues.push(format!("pile {} chunk {chunk} reopened in interval {interval}", pile.id.0));
                    }
                }
                if !closed_state[chunk] && state.closed {
                    closing.push(chunk);
                    closed_since[chunk].get_or_insert(at.start_h);
                }
                closed_state[chunk] = state.closed;
            }
            // Closed, and closed for the pile's rest.
            let released = |chunk: usize, entry: &ChunkRow| entry.closed && closed_since[chunk].is_some_and(|closed_h| pile.rested(closed_h, at));

            for &chunk in here {
                let state = published[&(pile.id, chunk, interval)];

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
                if state.closed && !released(chunk, state) && state.reclaimed_t > dust {
                    checker
                        .report
                        .issues
                        .push(format!("pile {} chunk {chunk} was reclaimed in interval {interval} before its rest", pile.id.0));
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
                // earlier chunk to be closed, published this interval or not.
                if state.received_t > dust
                    && let Some(earlier) = (0..chunk).find(|&earlier| !closed_state[earlier])
                {
                    checker.report.issues.push(format!(
                        "pile {} chunk {chunk} received in interval {interval} while chunk {earlier} was still open",
                        pile.id.0
                    ));
                }

                // Authored order among released, non-empty chunks. A chunk
                // with no row holds nothing, so only published ones count.
                if state.reclaimed_t > dust {
                    match pile.order {
                        crate::model::schedule::optimisation::ReclaimOrder::Fifo => {
                            for &earlier in here.iter().filter(|&&other| other < chunk) {
                                if published[&(pile.id, earlier, interval)].open_t > dust {
                                    checker.report.issues.push(format!(
                                        "FIFO violated: pile {} drew chunk {chunk} in interval {interval} while chunk {earlier} still held material",
                                        pile.id.0
                                    ));
                                }
                            }
                        }
                        crate::model::schedule::optimisation::ReclaimOrder::Lifo => {
                            for &later in here.iter().filter(|&&other| other > chunk) {
                                let entry = published[&(pile.id, later, interval)];
                                if released(later, entry) && entry.open_t > dust {
                                    checker.report.issues.push(format!(
                                        "LIFO violated: pile {} drew chunk {chunk} in interval {interval} while released chunk {later} still held material",
                                        pile.id.0
                                    ));
                                }
                            }
                        }
                    }
                }

                // A chunk closes only when full or where its pile is not
                // building.
                if closing.contains(&chunk) && pile.builds(at) && state.open_t < pile.chunks[chunk] - super::input::CHUNK_FULL_T - dust.max(REPLAY_TOLERANCE_T) {
                    checker.report.issues.push(format!(
                        "pile {} chunk {chunk} closed in interval {interval} holding {:.6} t of {:.6} t while its pile was building",
                        pile.id.0, state.open_t, pile.chunks[chunk]
                    ));
                }
            }
        }
    }
    drawn
}

/// Aggregate actual contained quantities into absolute target periods.
pub(crate) fn target_totals(input: &BlendInput, solution: &BlendSolution, contained: &BTreeMap<(usize, usize), Vec<f64>>) -> BTreeMap<(usize, u32), (f64, f64)> {
    if input.grade_targets.is_empty() {
        return BTreeMap::new();
    }
    let mut totals: BTreeMap<_, _> = input.target_opening.iter().map(|&(target, period, t, q)| ((target, period), (t, q))).collect();
    accumulate_target_receipts(input, solution.movements.iter(), contained, 0, &mut totals);
    totals
}

/// Add retained receipts without copying the window input or its inventory.
pub(crate) fn accumulate_target_receipts<'a>(
    input: &BlendInput,
    movements: impl Iterator<Item = &'a MovementRow>,
    contained: &BTreeMap<(usize, usize), Vec<f64>>,
    interval_offset: usize,
    totals: &mut BTreeMap<(usize, u32), (f64, f64)>,
) {
    if input.grade_targets.is_empty() {
        return;
    }
    let mut tonnes: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for row in movements {
        *tonnes.entry((row.candidate, row.interval)).or_default() += row.tonnes_t;
    }
    for ((candidate, local_interval), t) in tonnes {
        let Some(movement) = input.movements.get(candidate) else { continue };
        let Some(interval) = input.intervals.get(interval_offset + local_interval) else {
            continue;
        };
        for (index, target) in input
            .grade_targets
            .iter()
            .enumerate()
            .filter(|(_, target)| target.destination == movement.destination && target.applies(interval.start_h))
        {
            let entry = totals.entry((index, crate::model::schedule::grade_targets::target_day(interval.start_h))).or_default();
            entry.0 += t;
            entry.1 += contained.get(&(candidate, local_interval)).and_then(|q| q.get(target.grade)).copied().unwrap_or(0.0);
        }
    }
}

/// Incremental cost: carried receipts have already been charged by an earlier window.
pub(crate) fn target_penalty(input: &BlendInput, totals: &BTreeMap<(usize, u32), (f64, f64)>) -> f64 {
    let closing: f64 = totals.iter().map(|(&(index, _), &(t, q))| input.grade_targets[index].specification.penalty(t, q)).sum();
    let opening: f64 = input
        .target_opening
        .iter()
        .map(|&(index, _, t, q)| input.grade_targets[index].specification.penalty(t, q))
        .sum();
    closing - opening
}
