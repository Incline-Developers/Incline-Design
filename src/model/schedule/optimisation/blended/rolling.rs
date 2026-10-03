//! Day-by-day solving of a long horizon: the pieces that do not need a
//! solver.
//!
//! A week of hourly intervals is one large model, because every block stays
//! in it from the hour it becomes reachable to the end of the horizon. Solved
//! a day at a time the model stays near one day's size: each window holds the
//! day being decided, optionally plus a look-ahead, only the first day is
//! kept, and the state that day leaves behind becomes the next window's
//! opening state.
//!
//! Days are first solved alone ([`plan`] with no look-ahead), which is
//! fastest and on DreamLand's week also the best; only if that fails is the
//! run planned again with [`LOOKAHEAD_H`]. A day solved alone cannot see a
//! dead end it walks into: the chunked LIFO fixture closed every chunk on its
//! second day, because an open chunk was worth nothing that day, and left the
//! third day's diggers nowhere to put material.
//!
//! # What crosses a window boundary
//!
//! Everything a later day reads that an earlier day changed, and nothing
//! else:
//!
//! - **Ground.** Each block's remaining tonnes; a finished block leaves the
//!   input, which is what retires its columns. Composition does not change,
//!   because extraction is proportional.
//! - **Blended piles.** Closing tonnes and contained quantity, taken from the
//!   independent replay of the window. Receipts in the last committed
//!   interval join the released blend at that boundary, exactly as they
//!   would inside one model.
//! - **Chunks.** Each chunk's closing tonnes and contained quantity, again
//!   from the replay, and whether it was closed in the last kept interval. A
//!   closed chunk opens the next window closed, so an emptied slot is never
//!   reused; an open one may be closed at the boundary or keep filling, as it
//!   could inside one model.
//! - **Horizon-wide allowances.** What is left of each reclaim bar's cap and
//!   each dump's capacity, and what each crusher day has already taken.
//!
//! Loader, truck and bar windows are time-indexed and need no carry.
//!
//! # What this does not claim
//!
//! Each window is optimal, at best, given the days already kept, so the
//! stitched schedule has no horizon-wide bound of its own. It is replayed
//! against the whole horizon before anything uses it, and it serves as the
//! starting point for a whole-horizon solve that can improve on it and bound
//! it.
//!
//! Every lifecycle rule a boundary could break - a chunk reopening, a fill
//! out of sequence - is checked again when the stitched schedule is replayed
//! against the whole horizon.

use std::collections::BTreeMap;

use super::{
    input::{BlendInput, BlendPile},
    replay::{BlendSolution, ChunkRow, ExtractionAdjustments, MovementRow, ReplayReport},
};
use crate::model::schedule::optimisation::{Activity, DestinationId, DestinationKind, GroundId, Interval, IntervalRate, SourceId, StockpileId, TaskKind};

/// The part of each window that is kept.
pub(crate) const COMMIT_H: f64 = 24.0;
/// The part of a fallback window that is solved and then discarded, so a
/// kept day is not decided as though the horizon ended with it.
pub(crate) const LOOKAHEAD_H: f64 = 48.0;

/// Remaining tonnes at or below which a block counts as finished. This is the
/// replay's own readiness tolerance: a block the replay no longer counts as
/// holding work leaves the next window rather than being carried as dust.
const FINISHED_T: f64 = 1e-5;

/// One window, as indices into the whole horizon's intervals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Window {
    pub(crate) first: usize,
    /// How many intervals from `first` are kept.
    pub(crate) committed: usize,
    /// One past the last interval solved.
    pub(crate) end: usize,
}

/// The windows a horizon is solved in, each looking `lookahead_h` past its
/// kept day, or `None` when one window would cover it all.
///
/// Days are counted from the horizon's first interval. The last window keeps
/// everything it solves, because nothing lies beyond its look-ahead. No
/// window looks past the horizon.
pub(crate) fn plan(input: &BlendInput, lookahead_h: f64) -> Option<Vec<Window>> {
    let origin = input.intervals.first()?.start_h;
    let count = input.intervals.len();
    let until = |first: usize, hours: f64| first + input.intervals[first..].iter().take_while(|interval| interval.start_h < hours - 1e-9).count();
    let mut windows = Vec::new();
    let mut first = 0;
    while first < count {
        let day = ((input.intervals[first].start_h - origin) / COMMIT_H).floor();
        let commit_until = origin + (day + 1.0) * COMMIT_H;
        let committed_end = until(first, commit_until).max(first + 1);
        let end = until(first, commit_until + lookahead_h).max(committed_end);
        if end >= count {
            windows.push(Window {
                first,
                committed: count - first,
                end: count,
            });
            break;
        }
        windows.push(Window {
            first,
            committed: committed_end - first,
            end,
        });
        first = committed_end;
    }
    (windows.len() > 1).then_some(windows)
}

/// The state one window hands the next.
#[derive(Clone, Debug)]
pub(crate) struct Carry {
    ground: BTreeMap<GroundId, f64>,
    target_totals: BTreeMap<(usize, u32), (f64, f64)>,
    /// Released opening tonnes and contained quantity per grade.
    piles: BTreeMap<StockpileId, (f64, Vec<f64>)>,
    /// A chunked pile's chunks: tonnes, contained quantity per grade, and
    /// whether the chunk is closed.
    chunks: BTreeMap<StockpileId, Vec<(f64, Vec<f64>, bool)>>,
    /// End of the last interval each pile received in, for its rest.
    last_receipt_h: BTreeMap<StockpileId, f64>,
    /// When each chunk closed, for its rest; `None` for long enough ago.
    chunk_closed_h: BTreeMap<StockpileId, Vec<Option<f64>>>,
    /// Keyed by task index; only bars with an authored cap.
    reclaim_left: BTreeMap<usize, f64>,
    dump_left: BTreeMap<DestinationId, f64>,
    /// Keyed (crusher, absolute day as [`Interval::day`] counts it).
    crusher_used: BTreeMap<(DestinationId, usize), f64>,
}

impl Carry {
    pub(crate) fn opening(input: &BlendInput) -> Self {
        let grades = input.grades.count();
        Self {
            target_totals: input.target_opening.iter().map(|&(i, p, t, q)| ((i, p), (t, q))).collect(),
            ground: input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect(),
            piles: input.piles.iter().map(|pile| (pile.id, pile.total_opening(grades))).collect(),
            chunks: input
                .piles
                .iter()
                .filter(|pile| !pile.chunks.is_empty())
                .map(|pile| {
                    let chunks = (0..pile.chunks.len())
                        .map(|c| {
                            let (tonnes, mut contained) = match pile.chunk_opening.get(c) {
                                Some(opening) => opening.clone(),
                                None if pile.chunk_opening.is_empty() && c == 0 => (pile.opening_t, pile.opening_q.clone()),
                                None => (0.0, Vec::new()),
                            };
                            contained.resize(grades, 0.0);
                            (tonnes, contained, pile.chunk_starts_closed(c))
                        })
                        .collect();
                    (pile.id, chunks)
                })
                .collect(),
            last_receipt_h: input.piles.iter().filter_map(|pile| Some((pile.id, pile.last_receipt_h?))).collect(),
            chunk_closed_h: input
                .piles
                .iter()
                .filter(|pile| !pile.chunks.is_empty())
                .map(|pile| (pile.id, (0..pile.chunks.len()).map(|c| pile.chunk_closed_h.get(c).copied().flatten()).collect()))
                .collect(),
            reclaim_left: input
                .tasks
                .iter()
                .enumerate()
                .filter_map(|(index, task)| match task.kind {
                    TaskKind::Reclaim { maximum_t: Some(maximum), .. } => Some((index, maximum)),
                    _ => None,
                })
                .collect(),
            dump_left: input
                .destinations
                .iter()
                .filter_map(|destination| match (destination.kind, destination.capacity_t) {
                    (DestinationKind::Dump, Some(capacity)) => Some((destination.id, capacity)),
                    _ => None,
                })
                .collect(),
            crusher_used: BTreeMap::new(),
        }
    }

    /// The scenario of one window: the whole horizon's contract restricted
    /// to the window's intervals, opening in the carried state.
    ///
    /// Movement candidates keep their positions, so a window's rows name the
    /// same candidates as the whole horizon's, and the conditional values
    /// that point at them stay valid.
    pub(crate) fn window_input(&self, full: &BlendInput, window: Window) -> BlendInput {
        let grades = full.grades.count();
        let range = window.first..window.end;
        let intervals = full.intervals[range.clone()]
            .iter()
            .map(|interval| Interval {
                index: interval.index - window.first,
                ..*interval
            })
            .collect();
        let loaders = full
            .loaders
            .iter()
            .map(|loader| {
                let mut loader = loader.clone();
                loader.rates = loader
                    .rates
                    .iter()
                    .filter(|rate| range.contains(&rate.interval))
                    .map(|rate| IntervalRate {
                        interval: rate.interval - window.first,
                        ..*rate
                    })
                    .collect();
                loader
            })
            .collect();
        let trucks = full
            .trucks
            .iter()
            .map(|truck| {
                let mut truck = truck.clone();
                truck.hours = truck
                    .hours
                    .get(window.first.min(truck.hours.len())..window.end.min(truck.hours.len()))
                    .unwrap_or_default()
                    .to_vec();
                truck
            })
            .collect();
        let tasks = full
            .tasks
            .iter()
            .enumerate()
            .map(|(index, task)| {
                let mut task = task.clone();
                if let TaskKind::Reclaim { maximum_t: Some(maximum), .. } = &mut task.kind {
                    *maximum = self.reclaim_left.get(&index).copied().unwrap_or(*maximum).max(0.0);
                }
                task
            })
            .collect();
        let ground = full
            .ground
            .iter()
            .filter_map(|source| {
                let remaining = self.ground.get(&source.id).copied().unwrap_or(source.tonnes_t);
                (remaining > FINISHED_T).then(|| {
                    let mut source = source.clone();
                    source.tonnes_t = remaining;
                    source
                })
            })
            .collect();
        let piles = full
            .piles
            .iter()
            .map(|pile| {
                if let Some(chunks) = self.chunks.get(&pile.id) {
                    let mut opening_t = 0.0;
                    let mut opening_q = vec![0.0; grades];
                    for (tonnes, contained, _) in chunks {
                        opening_t += tonnes;
                        for (slot, value) in opening_q.iter_mut().zip(contained) {
                            *slot += value;
                        }
                    }
                    return BlendPile {
                        id: pile.id,
                        capacity_t: pile.capacity_t,
                        opening_t,
                        opening_q,
                        chunks: pile.chunks.clone(),
                        order: pile.order,
                        chunk_opening: chunks.iter().map(|(tonnes, contained, _)| (*tonnes, contained.clone())).collect(),
                        chunk_closed: chunks.iter().map(|(_, _, closed)| *closed).collect(),
                        modes: pile.modes.clone(),
                        exclusive: pile.exclusive,
                        rest_h: pile.rest_h,
                        last_receipt_h: self.last_receipt_h.get(&pile.id).copied().or(pile.last_receipt_h),
                        chunk_closed_h: self.chunk_closed_h.get(&pile.id).cloned().unwrap_or_else(|| pile.chunk_closed_h.clone()),
                    };
                }
                let (tonnes, contained) = self.piles.get(&pile.id).cloned().unwrap_or_else(|| pile.total_opening(grades));
                BlendPile {
                    id: pile.id,
                    capacity_t: pile.capacity_t,
                    opening_t: tonnes,
                    opening_q: contained,
                    chunks: Vec::new(),
                    order: pile.order,
                    chunk_opening: Vec::new(),
                    chunk_closed: Vec::new(),
                    modes: pile.modes.clone(),
                    exclusive: pile.exclusive,
                    rest_h: pile.rest_h,
                    last_receipt_h: self.last_receipt_h.get(&pile.id).copied().or(pile.last_receipt_h),
                    chunk_closed_h: Vec::new(),
                }
            })
            .collect();
        let destinations = full
            .destinations
            .iter()
            .map(|destination| {
                let mut destination = destination.clone();
                if let Some(left) = self.dump_left.get(&destination.id) {
                    destination.capacity_t = Some(left.max(0.0));
                }
                if destination.kind == DestinationKind::Crusher {
                    for (day, budget) in destination.crusher_daily_t.iter_mut().enumerate() {
                        if let Some(budget) = budget {
                            *budget = (*budget - self.crusher_used.get(&(destination.id, day)).copied().unwrap_or(0.0)).max(0.0);
                        }
                    }
                }
                destination
            })
            .collect();
        BlendInput {
            intervals,
            segments_per_interval: full.segments_per_interval,
            grades: full.grades.clone(),
            piles,
            loaders,
            tasks,
            ground,
            destinations,
            trucks,
            movements: full.movements.clone(),
            qualifications: full.qualifications.clone(),
            conditional_values: full.conditional_values.clone(),
            grade_limits: full.grade_limits.clone(),
            grade_targets: full.grade_targets.clone(),
            target_opening: self.target_totals.iter().map(|(&(i, p), &(t, q))| (i, p, t, q)).collect(),
            drill_blast: full.drill_blast.clone().map(|mut chain| {
                for agent in &mut chain.agents {
                    agent.rates = agent.rates[range.clone()].to_vec();
                }
                chain
            }),
        }
    }

    /// Advance past a window's kept intervals. `solution` and `replay` are
    /// the window's own, in its local interval numbering.
    pub(crate) fn advance(&mut self, full: &BlendInput, window: Window, solution: &BlendSolution, replay: &ReplayReport) {
        let mut received: BTreeMap<(StockpileId, usize), f64> = BTreeMap::new();
        for row in solution.movements.iter().filter(|row| row.interval < window.committed) {
            let Some(candidate) = full.movements.get(row.candidate) else { continue };
            if let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source)
                && let Some(left) = self.ground.get_mut(&ground)
            {
                *left -= row.tonnes_t;
            }
            if let (Activity::Reclaim, SourceId::Stockpile(pile)) = (candidate.activity, candidate.source) {
                // Mirrors the `reclmax` row: the bar's loader, from any pile
                // the bar approves.
                for (index, task) in full.tasks.iter().enumerate() {
                    if let TaskKind::Reclaim { approved_sources, .. } = &task.kind
                        && task.loader == candidate.loader
                        && approved_sources.contains(&pile)
                        && let Some(left) = self.reclaim_left.get_mut(&index)
                    {
                        *left -= row.tonnes_t;
                    }
                }
            }
            if let Some(left) = self.dump_left.get_mut(&candidate.destination) {
                *left -= row.tonnes_t;
            }
            if let Some(DestinationKind::Stockpile(pile)) = full.destinations.iter().find(|destination| destination.id == candidate.destination).map(|d| d.kind) {
                *received.entry((pile, row.interval)).or_default() += row.tonnes_t;
            }
            if full
                .destinations
                .iter()
                .any(|destination| destination.id == candidate.destination && destination.kind == DestinationKind::Crusher)
                && let Some(interval) = full.intervals.get(window.first + row.interval)
            {
                *self.crusher_used.entry((candidate.destination, interval.day() as usize)).or_default() += row.tonnes_t;
            }
        }
        for ((pile, interval), tonnes) in received {
            if tonnes > super::input::REST_RECEIPT_T
                && let Some(interval) = full.intervals.get(window.first + interval)
            {
                let last = self.last_receipt_h.entry(pile).or_insert(interval.end_h);
                *last = last.max(interval.end_h);
            }
        }
        super::replay::accumulate_target_receipts(
            full,
            solution.movements.iter().filter(|row| row.interval < window.committed),
            &replay.movement_contained,
            window.first,
            &mut self.target_totals,
        );
        let last = window.committed - 1;
        for pile in &full.piles {
            if let Some(state) = replay.pile_intervals.get(&(pile.id, last)) {
                // Clamped as a chunk is below: an emptied pile can close
                // with 3e-14 contained against 0 t, which the next window's
                // input check refuses.
                let tonnes = state.closing_t.clamp(0.0, pile.capacity_t);
                let contained = state.closing_q.iter().map(|quantity| quantity.clamp(0.0, tonnes)).collect();
                self.piles.insert(pile.id, (tonnes, contained));
            }
            let Some(chunks) = self.chunks.get_mut(&pile.id) else { continue };
            let closed_h = self.chunk_closed_h.entry(pile.id).or_insert_with(|| vec![None; pile.chunks.len()]);
            for (c, chunk) in chunks.iter_mut().enumerate() {
                // Closed in this window: when, for its rest.
                if !chunk.2
                    && let Some(first) = solution
                        .chunks
                        .iter()
                        .filter(|row| row.pile == pile.id && row.chunk == c && row.closed && row.interval < window.committed)
                        .map(|row| row.interval)
                        .min()
                    && let Some(interval) = full.intervals.get(window.first + first)
                    && let Some(slot) = closed_h.get_mut(c)
                {
                    *slot = Some(interval.start_h);
                }
                if let Some((tonnes, contained)) = replay.chunk_intervals.get(&(pile.id, c, last)) {
                    // Never above the chunk's capacity: the model's own
                    // opening row would otherwise have no solution for a
                    // tolerance's worth of overfill.
                    let tonnes = tonnes.clamp(0.0, pile.chunks[c]);
                    chunk.0 = tonnes;
                    chunk.1 = contained.iter().map(|quantity| quantity.clamp(0.0, tonnes)).collect();
                }
                if let Some(row) = solution.chunks.iter().find(|row| row.pile == pile.id && row.chunk == c && row.interval == last) {
                    chunk.2 |= row.closed;
                }
            }
        }
    }
}

/// The kept days of every window, renumbered onto the whole horizon.
pub(crate) struct Stitched {
    solution: BlendSolution,
}

impl Stitched {
    pub(crate) fn new() -> Self {
        Self {
            solution: BlendSolution {
                movements: Vec::new(),
                durations: BTreeMap::new(),
                chunks: Vec::new(),
                reported_objective: 0.0,
                adjustments: ExtractionAdjustments::default(),
                drill_blast: None,
            },
        }
    }

    /// Keep a window's committed intervals. `paid` is what the window's model
    /// credited conditional values over those intervals, so the stitched
    /// objective is the model's own figure, reconcilable by the replay in
    /// the usual way.
    pub(crate) fn keep(&mut self, full: &BlendInput, window: Window, solution: &BlendSolution, paid: f64) {
        for row in solution.movements.iter().filter(|row| row.interval < window.committed) {
            let value = full.movements.get(row.candidate).and_then(|candidate| candidate.value_per_tonne().ok()).unwrap_or(0.0);
            self.solution.reported_objective += row.tonnes_t * value;
            self.solution.movements.push(MovementRow {
                interval: row.interval + window.first,
                ..*row
            });
        }
        for (&(interval, segment), &duration) in &solution.durations {
            if interval < window.committed {
                self.solution.durations.insert((interval + window.first, segment), duration);
            }
        }
        for row in solution.chunks.iter().filter(|row| row.interval < window.committed) {
            self.solution.chunks.push(ChunkRow {
                interval: row.interval + window.first,
                ..row.clone()
            });
        }
        self.solution.reported_objective += paid;
        // Omitted dust is not attributed to intervals when extracted, so the
        // whole window's is counted: the reconciliation tolerance it widens
        // stays an over-estimate rather than an under-estimate.
        let adjustments = &mut self.solution.adjustments;
        adjustments.movement_count += solution.adjustments.movement_count;
        adjustments.movement_total_t += solution.adjustments.movement_total_t;
        adjustments.movement_max_t = adjustments.movement_max_t.max(solution.adjustments.movement_max_t);
    }

    pub(crate) fn finish(self) -> BlendSolution {
        self.solution
    }
}
