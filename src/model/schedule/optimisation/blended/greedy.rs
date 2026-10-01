//! A dispatch schedule, built without a solver, for SCIP to start from.
//!
//! SCIP finds good blended schedules slowly. Its primal heuristics need the
//! root LP, and a three-day window's root LP can take longer than the run
//! gives the window. Until then the best schedule it holds is often a loader
//! working one hour in ten: on a real project's week at a 60 s limit, every
//! window ended at its limit worth a tenth of what the loaders could earn
//! digging flat out. Yet digging flat out is usually close to optimal, and it
//! is cheap to construct directly.
//!
//! [`dispatch`] does that, interval by interval, the way a dispatcher would:
//!
//! - Each loader works its highest-priority bar with work available, defined
//!   exactly as the replay's priority check defines it, from the state at
//!   the start of the interval.
//! - A dig bar works through its blocks in authored order at the loader's
//!   rate, several blocks back to back if they fit. A block another loader
//!   may also dig must be gone a whole interval before the next one starts,
//!   as the formulation's `order` rows require.
//! - A reclaim bar draws the released opening blend of its approved piles,
//!   within its authored cap.
//! - Each block's materials leave in proportion, and every portion goes to its
//!   best-paying destinations first, within crusher days, dump and pile
//!   capacity, and truck hours. A block whose material cannot all be placed
//!   is dug only as far as it can be.
//! - Loaders take that shared room in order of what their bar pays best, so
//!   a reclaim worth more at the crusher is not crowded out by a cheaper
//!   direct feed.
//!
//! It is a heuristic and claims nothing about optimality. It can be poor:
//! it digs whenever it can, so a project whose movements all lose money is
//! better left idle. Its answer is only used after the replay has accepted
//! it, and only as a starting point that SCIP must match or beat.
//!
//! Each interval is worked as one execution segment holding the whole
//! interval. A reclaim goes where a grade decides admission - a route
//! qualification or a minimum grade - only when the pile's released blend
//! clears the boundary by the formulation's own margin, and never where a
//! grade-conditional value applies, which is the model's to price. Chunked
//! piles are not dispatched: the input is refused rather than half-handled.

use std::collections::BTreeMap;

use super::{
    input::{BlendInput, GRADE_CUSHION_T, GRADE_MARGIN, GradeQualification, authored_tasks, interval_rate, task_active, task_authorises},
    replay::{BlendSolution, ExtractionAdjustments, MovementRow},
};
use crate::model::schedule::optimisation::{Activity, DestinationId, DestinationKind, GroundId, Interval, MovementCandidate, SourceId, StockpileId, TaskKind, TruckClassId};

/// Remaining tonnes at or below which a block counts as finished.
const FINISHED_T: f64 = 1e-9;

/// Below this a dig or a reclaim is not worth a movement row.
const NEGLIGIBLE_T: f64 = 1e-6;

/// How many times a block's extraction is scaled back to fit a material
/// portion that could not all be placed. Each pass places at least as much
/// as the one before, so a handful settles any block.
const FIT_PASSES: usize = 8;

/// A dispatch schedule for `input`, or `None` when the input holds a
/// chunked pile.
pub(crate) fn dispatch(input: &BlendInput) -> Option<BlendSolution> {
    if input.piles.iter().any(|pile| !pile.chunks.is_empty()) {
        return None;
    }
    let mut state = State::new(input);
    for interval in &input.intervals {
        state.work(*interval);
    }
    Some(state.finish())
}

/// The dispatcher's physical state, walked forward an interval at a time.
struct State<'a> {
    input: &'a BlendInput,
    ground: BTreeMap<GroundId, f64>,
    /// Released opening tonnes and contained quantity per grade.
    piles: BTreeMap<StockpileId, (f64, Vec<f64>)>,
    /// What each dump has taken, and each crusher day.
    dumped: BTreeMap<DestinationId, f64>,
    crushed: BTreeMap<(DestinationId, u32), f64>,
    /// Keyed by task index.
    reclaimed: BTreeMap<usize, f64>,
    /// Dig candidates by (loader, block, material), best-paying first, and
    /// reclaim candidates by (loader, pile).
    digs: BTreeMap<(usize, GroundId, u32), Vec<usize>>,
    reclaims: BTreeMap<(usize, StockpileId), Vec<usize>>,
    /// Blocks some other loader could dig, keyed (loader, block).
    shared: BTreeMap<(usize, GroundId), bool>,
    rows: BTreeMap<(usize, usize), f64>,
    durations: BTreeMap<(usize, usize), f64>,
}

/// What one interval has used so far of its shared allowances.
#[derive(Clone, Default)]
struct Used {
    trucks: BTreeMap<TruckClassId, f64>,
    dumped: BTreeMap<DestinationId, f64>,
    crushed: BTreeMap<DestinationId, f64>,
    /// Receipts per pile: tonnes and contained quantity per grade.
    received: BTreeMap<StockpileId, (f64, Vec<f64>)>,
    rows: Vec<(usize, f64)>,
}

impl<'a> State<'a> {
    fn new(input: &'a BlendInput) -> Self {
        let grades = input.grades.count();
        let loader_index = |id| input.loaders.iter().position(|loader| loader.id == id);
        let mut digs: BTreeMap<(usize, GroundId, u32), Vec<usize>> = BTreeMap::new();
        let mut reclaims: BTreeMap<(usize, StockpileId), Vec<usize>> = BTreeMap::new();
        for (index, candidate) in input.movements.iter().enumerate() {
            let Some(loader) = loader_index(candidate.loader) else { continue };
            if candidate.value_per_tonne().is_err() {
                continue;
            }
            match (candidate.activity, candidate.source) {
                (Activity::Dig, SourceId::Ground(ground)) => digs.entry((loader, ground, candidate.material.0)).or_default().push(index),
                (Activity::Reclaim, SourceId::Stockpile(pile)) => reclaims.entry((loader, pile)).or_default().push(index),
                _ => {}
            }
        }
        let value = |index: &usize| input.movements[*index].value_per_tonne().unwrap_or(0.0);
        for list in digs.values_mut().chain(reclaims.values_mut()) {
            list.sort_by(|left, right| value(right).total_cmp(&value(left)).then(left.cmp(right)));
        }
        let mut shared = BTreeMap::new();
        for &(loader, ground, _) in digs.keys() {
            let other = digs.keys().any(|&(other, block, _)| other != loader && block == ground);
            shared.insert((loader, ground), other);
        }
        Self {
            input,
            ground: input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect(),
            piles: input.piles.iter().map(|pile| (pile.id, pile.total_opening(grades))).collect(),
            dumped: BTreeMap::new(),
            crushed: BTreeMap::new(),
            reclaimed: BTreeMap::new(),
            digs,
            reclaims,
            shared,
            rows: BTreeMap::new(),
            durations: BTreeMap::new(),
        }
    }

    fn work(&mut self, interval: Interval) {
        let input = self.input;
        let k = interval.index;
        let duration = interval.duration_h();
        for segment in 0..input.segments_per_interval.max(1) {
            self.durations.insert((k, segment), if segment == 0 { duration } else { 0.0 });
        }
        // Readiness is judged on the state the interval opened with, as the
        // replay judges it.
        let opening_ground = self.ground.clone();
        let mut used = Used::default();
        let mut drawn: BTreeMap<StockpileId, f64> = BTreeMap::new();
        // Each loader's bar, then the loaders in order of what their bar pays
        // best, so shared crusher, truck and pile room goes to the most
        // valuable work first.
        let mut working: Vec<(usize, usize, f64)> = (0..input.loaders.len())
            .filter_map(|loader_index| {
                let rate = interval_rate(&input.loaders[loader_index], k)?;
                let bar = authored_tasks(input, loader_index)
                    .into_iter()
                    .find(|&bar| self.ready(bar, interval, rate.dig_tph, rate.reclaim_tph, &opening_ground))?;
                let best = input
                    .movements
                    .iter()
                    .filter(|candidate| task_authorises(&input.tasks[bar], candidate))
                    .filter_map(|candidate| candidate.value_per_tonne().ok())
                    .fold(f64::NEG_INFINITY, f64::max);
                Some((loader_index, bar, best))
            })
            .collect();
        working.sort_by(|left, right| right.2.total_cmp(&left.2).then(left.0.cmp(&right.0)));
        for (loader_index, bar, _) in working {
            let Some(rate) = interval_rate(&input.loaders[loader_index], k) else { continue };
            let task = &input.tasks[bar];
            match &task.kind {
                TaskKind::Dig { sequence } => self.dig(loader_index, bar, sequence, interval, rate.dig_tph * duration, &opening_ground, &mut used),
                TaskKind::Reclaim { approved_sources, maximum_t } => {
                    let mut capacity = rate.reclaim_tph * duration;
                    if let Some(maximum) = maximum_t {
                        capacity = capacity.min(maximum - self.reclaimed.get(&bar).copied().unwrap_or(0.0));
                    }
                    for pile in approved_sources {
                        if capacity <= NEGLIGIBLE_T {
                            break;
                        }
                        let Some((open_t, _)) = self.piles.get(pile) else { continue };
                        let available = open_t - drawn.get(pile).copied().unwrap_or(0.0);
                        let wanted = capacity.min(available);
                        if wanted <= NEGLIGIBLE_T {
                            continue;
                        }
                        let candidates = self.reclaim_candidates(loader_index, *pile, bar, interval);
                        let placed = self.place(&candidates, wanted, interval, &mut used);
                        let moved: f64 = placed.iter().map(|(_, tonnes)| tonnes).sum();
                        used.rows.extend(placed);
                        *drawn.entry(*pile).or_default() += moved;
                        *self.reclaimed.entry(bar).or_default() += moved;
                        capacity -= moved;
                    }
                }
            }
        }

        // Close the interval: rows, allowances and pile balances.
        for (candidate, tonnes) in used.rows {
            *self.rows.entry((candidate, k)).or_default() += tonnes;
        }
        for (destination, tonnes) in used.dumped {
            *self.dumped.entry(destination).or_default() += tonnes;
        }
        for (destination, tonnes) in used.crushed {
            *self.crushed.entry((destination, interval.day())).or_default() += tonnes;
        }
        for (pile, (open_t, open_q)) in &mut self.piles {
            let reclaimed = drawn.get(pile).copied().unwrap_or(0.0);
            if reclaimed > 0.0 && *open_t > 0.0 {
                let share = reclaimed / *open_t;
                for contained in open_q.iter_mut() {
                    *contained -= *contained * share;
                }
            }
            *open_t -= reclaimed;
            if let Some((received_t, received_q)) = used.received.get(pile) {
                *open_t += received_t;
                for (contained, received) in open_q.iter_mut().zip(received_q) {
                    *contained += received;
                }
            }
        }
    }

    /// The replay's readiness, from the state the interval opened with.
    fn ready(&self, bar: usize, interval: Interval, dig_tph: f64, reclaim_tph: f64, opening_ground: &BTreeMap<GroundId, f64>) -> bool {
        let task = &self.input.tasks[bar];
        if !task_active(task, interval) {
            return false;
        }
        match &task.kind {
            TaskKind::Dig { sequence } => dig_tph > 0.0 && sequence.iter().any(|block| opening_ground.get(block).is_some_and(|left| *left > FINISHED_T)),
            TaskKind::Reclaim { approved_sources, maximum_t } => {
                reclaim_tph > 0.0
                    && maximum_t.is_none_or(|maximum| self.reclaimed.get(&bar).copied().unwrap_or(0.0) < maximum - NEGLIGIBLE_T)
                    && approved_sources.iter().any(|pile| self.piles.get(pile).is_some_and(|(open_t, _)| *open_t > NEGLIGIBLE_T))
            }
        }
    }

    /// Work a dig bar's sequence for one interval.
    #[allow(clippy::too_many_arguments)]
    fn dig(&mut self, loader: usize, bar: usize, sequence: &[GroundId], interval: Interval, mut capacity: f64, opening_ground: &BTreeMap<GroundId, f64>, used: &mut Used) {
        let input = self.input;
        let mut previous: Option<GroundId> = None;
        for &block in sequence {
            if capacity <= NEGLIGIBLE_T {
                return;
            }
            // A block the input no longer holds was finished before it.
            let Some(&left) = self.ground.get(&block) else {
                previous = Some(block);
                continue;
            };
            if let Some(earlier) = previous {
                // The formulation's `order` row: the earlier block gone by
                // the end of this cell, or of the one before when another
                // loader could also have dug it.
                let shared = self.shared.get(&(loader, earlier)).copied().unwrap_or(false);
                let gone = if shared { opening_ground.get(&earlier) } else { self.ground.get(&earlier) };
                if gone.is_some_and(|left| *left > FINISHED_T) {
                    return;
                }
            }
            previous = Some(block);
            if left <= FINISHED_T {
                continue;
            }
            let Some(source) = input.ground.iter().find(|source| source.id == block) else { return };
            let portions: Vec<(f64, Vec<usize>)> = source
                .material
                .iter()
                .map(|share| {
                    let candidates = self
                        .digs
                        .get(&(loader, block, share.material.0))
                        .map(|list| list.iter().copied().filter(|&index| task_authorises(&input.tasks[bar], &input.movements[index])).collect())
                        .unwrap_or_default();
                    (share.fraction, candidates)
                })
                .collect();
            let mut extract = left.min(capacity);
            let mut fitted = None;
            for _ in 0..FIT_PASSES {
                let mut trial = used.clone();
                let mut rows = Vec::new();
                let mut worst = 1.0_f64;
                for (fraction, candidates) in &portions {
                    let wanted = fraction * extract;
                    if wanted <= 0.0 {
                        continue;
                    }
                    let placed = self.place(candidates, wanted, interval, &mut trial);
                    let moved: f64 = placed.iter().map(|(_, tonnes)| tonnes).sum();
                    worst = worst.min(moved / wanted);
                    rows.extend(placed);
                }
                if worst >= 1.0 - 1e-12 {
                    trial.rows.extend(rows);
                    fitted = Some(trial);
                    break;
                }
                // Scale back to what the tightest portion could place, and
                // place again from scratch so the portions stay in proportion.
                extract *= worst * (1.0 - 1e-9);
                if extract <= NEGLIGIBLE_T {
                    break;
                }
            }
            let Some(fitted) = fitted else { return };
            *used = fitted;
            capacity -= extract;
            let finished = extract >= left;
            self.ground.insert(block, if finished { 0.0 } else { left - extract });
            if !finished {
                return;
            }
        }
    }

    /// Reclaim candidates for one loader and pile under `bar`, admitted on
    /// the pile's released blend with the formulation's margin to spare.
    fn reclaim_candidates(&self, loader: usize, pile: StockpileId, bar: usize, interval: Interval) -> Vec<usize> {
        let input = self.input;
        let Some((open_t, open_q)) = self.piles.get(&pile) else { return Vec::new() };
        let blend: Vec<f64> = open_q.iter().map(|contained| contained / open_t).collect();
        let band = GRADE_MARGIN + GRADE_CUSHION_T / open_t.max(GRADE_CUSHION_T) + 1e-9;
        self.reclaims
            .get(&(loader, pile))
            .map(|list| {
                list.iter()
                    .copied()
                    .filter(|&index| {
                        let candidate = &input.movements[index];
                        if !task_authorises(&input.tasks[bar], candidate) || !task_active(&input.tasks[bar], interval) {
                            return false;
                        }
                        // The older fixtures' minimum grades, on the blend
                        // the reclaim carries.
                        let short = input
                            .grade_limits
                            .iter()
                            .any(|limit| limit.destination == candidate.destination && blend.get(limit.grade).is_none_or(|value| *value < limit.minimum + band));
                        if short {
                            return false;
                        }
                        // A conditional value is the model's to price.
                        if input.conditional_values.iter().any(|entry| entry.candidate == index) {
                            return false;
                        }
                        if input.qualifications.is_empty() {
                            return true;
                        }
                        input
                            .qualifications
                            .iter()
                            .find(|entry| entry.loader == candidate.loader && entry.pile == pile && entry.destination == candidate.destination)
                            .is_some_and(|entry| admits_clearly(entry, &blend, band))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Place up to `wanted` tonnes among `candidates`, best-paying first,
    /// recording each delivery's use of destinations and trucks in `used`.
    fn place(&self, candidates: &[usize], wanted: f64, interval: Interval, used: &mut Used) -> Vec<(usize, f64)> {
        let mut rows = Vec::new();
        let mut left = wanted;
        for &index in candidates {
            if left <= 0.0 {
                break;
            }
            let candidate = &self.input.movements[index];
            let room = self.destination_room(candidate, interval, used).min(self.truck_room(candidate, interval, used));
            let tonnes = left.min(room);
            if tonnes <= NEGLIGIBLE_T {
                continue;
            }
            self.record(candidate, tonnes, used);
            rows.push((index, tonnes));
            left -= tonnes;
        }
        rows
    }

    fn destination_room(&self, candidate: &MovementCandidate, interval: Interval, used: &Used) -> f64 {
        let input = self.input;
        let Some(destination) = input.destinations.iter().find(|entry| entry.id == candidate.destination) else {
            return 0.0;
        };
        match destination.kind {
            DestinationKind::Dump => destination.capacity_t.map_or(f64::INFINITY, |capacity| {
                capacity - self.dumped.get(&destination.id).copied().unwrap_or(0.0) - used.dumped.get(&destination.id).copied().unwrap_or(0.0)
            }),
            DestinationKind::Crusher => match destination.crusher_daily_t.get(interval.day() as usize).copied().flatten() {
                Some(budget) => budget - self.crushed.get(&(destination.id, interval.day())).copied().unwrap_or(0.0) - used.crushed.get(&destination.id).copied().unwrap_or(0.0),
                None => f64::INFINITY,
            },
            // Receipts must fit on top of the opening stock, with no credit
            // for what the interval reclaims: the replay's stricter check
            // when a delivering loader works blocks back to back.
            DestinationKind::Stockpile(pile) => {
                let Some(target) = input.piles.iter().find(|entry| entry.id == pile) else { return 0.0 };
                let open_t = self.piles.get(&pile).map_or(0.0, |(tonnes, _)| *tonnes);
                target.capacity_t - open_t - used.received.get(&pile).map_or(0.0, |(tonnes, _)| *tonnes)
            }
        }
        .max(0.0)
    }

    fn truck_room(&self, candidate: &MovementCandidate, interval: Interval, used: &Used) -> f64 {
        if candidate.truck_hours_per_tonne <= 0.0 {
            return f64::INFINITY;
        }
        let Some(truck) = self.input.trucks.iter().find(|truck| truck.id == candidate.truck) else {
            return f64::INFINITY;
        };
        let Some(hours) = truck.hours.get(interval.index) else { return f64::INFINITY };
        ((hours - used.trucks.get(&truck.id).copied().unwrap_or(0.0)) / candidate.truck_hours_per_tonne).max(0.0)
    }

    fn record(&self, candidate: &MovementCandidate, tonnes: f64, used: &mut Used) {
        let input = self.input;
        *used.trucks.entry(candidate.truck).or_default() += tonnes * candidate.truck_hours_per_tonne;
        let Some(destination) = input.destinations.iter().find(|entry| entry.id == candidate.destination) else {
            return;
        };
        match destination.kind {
            DestinationKind::Dump => *used.dumped.entry(destination.id).or_default() += tonnes,
            DestinationKind::Crusher => *used.crushed.entry(destination.id).or_default() += tonnes,
            DestinationKind::Stockpile(pile) => {
                let grades = input.grades.count();
                let entry = used.received.entry(pile).or_insert_with(|| (0.0, vec![0.0; grades]));
                entry.0 += tonnes;
                for (g, contained) in entry.1.iter_mut().enumerate() {
                    *contained += tonnes * input.grades.fraction(candidate.material, g).unwrap_or(0.0);
                }
            }
        }
    }

    fn finish(self) -> BlendSolution {
        let input = self.input;
        let movements: Vec<MovementRow> = self
            .rows
            .into_iter()
            .map(|((candidate, interval), tonnes_t)| MovementRow {
                candidate,
                interval,
                segment: 0,
                tonnes_t,
            })
            .collect();
        let reported_objective = movements
            .iter()
            .map(|row| row.tonnes_t * input.movements[row.candidate].value_per_tonne().unwrap_or(0.0))
            .sum();
        BlendSolution {
            movements,
            durations: self.durations,
            chunks: Vec::new(),
            reported_objective,
            adjustments: ExtractionAdjustments::default(),
        }
    }
}

/// Whether some alternative of `qualification` holds on `blend` with `band`
/// to spare on every boundary it tests, so the formulation's margin admits
/// it too.
fn admits_clearly(qualification: &GradeQualification, blend: &[f64], band: f64) -> bool {
    qualification.alternatives.iter().any(|alternative| {
        alternative.half_spaces().into_iter().all(|test| {
            let value = blend.get(test.grade).copied().unwrap_or(0.0);
            if test.above {
                value >= test.endpoint.value + band
            } else {
                value <= test.endpoint.value - band
            }
        })
    })
}
