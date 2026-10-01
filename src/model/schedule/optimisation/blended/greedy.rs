//! A dispatch schedule: the horizon walked forward an interval at a time,
//! each interval's movements chosen by one small linear program.
//!
//! The whole-horizon model is a mixed-integer program whose size grows with
//! the horizon and whose solve time grows faster still, and with a time limit
//! what it returns depends on where the limit fell. Measured on a real
//! project's 1, 3 and 4 day horizons and on the test worlds, a schedule built
//! interval by interval was already worth what the whole-horizon solve proved
//! optimal, in milliseconds; over a week it was 0.1% short. So this is the
//! schedule a run shows first, and the start the whole-horizon solve must
//! match or beat.
//!
//! Each interval is one linear program over what the interval allows:
//!
//! - Each loader works its highest-priority bar with work available, defined
//!   exactly as the replay's priority check defines it, from the state at
//!   the start of the interval.
//! - A dig bar works its current block, and the next ones in authored order
//!   once each is finished, up to the loader's rate. A block's materials
//!   leave in proportion. A block another loader may also dig must be gone a
//!   whole interval before the next one starts, as the formulation's `order`
//!   rows require.
//! - A reclaim bar draws the released opening blend of its approved piles,
//!   within its authored cap.
//! - Every tonne goes where the program puts it, within the interval's truck
//!   hours, crusher day, dump and pile room, shared by all loaders. Trucks are
//!   not allotted to loaders beforehand: the program shares them out.
//!
//! The objective is the interval's movement value plus, for each tonne moved,
//! a production credit that covers negative value and the maximum marginal
//! target penalty, keeping production preferable to standing.
//! An interval sees nothing after itself, so without the credit a block whose
//! material is worth nothing until it is out of the way would never be dug,
//! and the ore behind it never reached. With positive values and no soft
//! targets, the credit is a tie-break of 1e-7 of the largest
//! value per tonne, and so are authored routing preferences. Soft targets
//! subtract the running destination-period penalty from that objective.
//!
//! Nothing here looks past the interval, so it claims nothing about the
//! horizon's optimum: stockpiling for later, saving a crusher day for better
//! feed and the like are the whole-horizon solve's to find. Its answer is
//! only used after the replay has accepted it.
//!
//! Each interval is worked as one execution segment holding the whole
//! interval. A reclaim goes where a grade decides admission - a route
//! qualification or a minimum grade - only when the pile's released blend
//! clears the boundary by the formulation's own margin, and never where a
//! grade-conditional value applies, which is the model's to price.
//!
//! A chunked pile follows the formulation's chunk lifecycle with one rule of
//! its own: a chunk closes when it is full, and only then. Receipts go to the
//! first chunk still open, up to its room. A chunk is open or closed for a
//! whole interval, so one that fills during an interval closes at the start of
//! the next, and the next chunk starts receiving then. Reclaim draws one
//! chunk, the one the authored order releases - FIFO the oldest holding
//! material, if it is closed; LIFO the newest closed one holding material -
//! at that chunk's own blend. The formulation may also close a partly filled
//! chunk; the dispatcher never does, which is what kept it out of the dead end
//! where every chunk closed early and the diggers had nowhere to deliver.

use std::collections::BTreeMap;

use highs::{Col, HighsModelStatus, RowProblem, Sense};

use super::{
    input::{BlendInput, BlendPile, GRADE_CUSHION_T, GRADE_MARGIN, GradeQualification, authored_tasks, interval_rate, task_active, task_authorises},
    replay::{BlendSolution, ChunkRow, ExtractionAdjustments, MovementRow},
};
use crate::model::schedule::optimisation::{
    Activity, DestinationId, DestinationKind, GroundId, Interval, MovementCandidate, ReclaimOrder, SourceId, StockpileId, TaskKind, TruckClassId,
};

/// Remaining tonnes at or below which a block counts as finished.
const FINISHED_T: f64 = 1e-9;

/// Below this a dig or a reclaim is not worth a movement row, and loader
/// capacity left over does not open the next block.
const NEGLIGIBLE_T: f64 = 1e-6;

/// An extraction this close to a block's remainder finishes it: the linear
/// program's own tolerance would otherwise leave dust behind.
const SNAP_T: f64 = 1e-6;

/// The tie-break weight per tonne, relative to the largest movement value.
const TIE_WEIGHT: f64 = 1e-7;

/// A dispatch schedule for `input`, or why there is none.
pub(crate) fn dispatch(input: &BlendInput) -> Result<BlendSolution, String> {
    let mut state = State::new(input);
    for interval in &input.intervals {
        state.work(*interval)?;
    }
    Ok(state.finish())
}

/// The dispatcher's physical state, walked forward an interval at a time.
struct State<'a> {
    input: &'a BlendInput,
    target_totals: BTreeMap<(usize, u32), (f64, f64)>,
    ground: BTreeMap<GroundId, f64>,
    /// Released opening tonnes and contained quantity per grade; a chunked
    /// pile's is the sum of its chunks.
    piles: BTreeMap<StockpileId, (f64, Vec<f64>)>,
    /// Chunked piles only.
    chunks: BTreeMap<StockpileId, Vec<Chunk>>,
    chunk_rows: Vec<ChunkRow>,
    /// What each dump has taken, and each crusher day.
    dumped: BTreeMap<DestinationId, f64>,
    crushed: BTreeMap<(DestinationId, u32), f64>,
    /// Keyed by task index.
    reclaimed: BTreeMap<usize, f64>,
    /// Dig candidates by (loader, block, material) and reclaim candidates
    /// by (loader, pile).
    digs: BTreeMap<(usize, GroundId, u32), Vec<usize>>,
    reclaims: BTreeMap<(usize, StockpileId), Vec<usize>>,
    /// Blocks some other loader could dig, keyed (loader, block).
    shared: BTreeMap<(usize, GroundId), bool>,
    /// Objective per tonne of each candidate: its value, the production
    /// credit and the tie-breaks.
    weight: Vec<f64>,
    rows: BTreeMap<(usize, usize), f64>,
    durations: BTreeMap<(usize, usize), f64>,
}

/// One chunk of a chunked pile, as it opens an interval.
struct Chunk {
    capacity_t: f64,
    held_t: f64,
    held_q: Vec<f64>,
    closed: bool,
}

/// What a pile releases to reclaim in an interval.
struct Released {
    /// The chunk drawn, for a chunked pile.
    chunk: Option<usize>,
    tonnes: f64,
    blend: Vec<f64>,
}

/// One loader's work in an interval.
struct Bar {
    loader: usize,
    task: usize,
    /// Tonnes the loader can move in the interval.
    capacity: f64,
    /// Dig bars only: the blocks it works, in authored order. Every block
    /// before the last was finished by an earlier solve of the interval.
    blocks: Vec<GroundId>,
}

/// An interval's answer.
#[derive(Default)]
struct Plan {
    rows: Vec<(usize, f64)>,
    /// Keyed (loader, block).
    extracted: BTreeMap<(usize, GroundId), f64>,
}

impl<'a> State<'a> {
    fn new(input: &'a BlendInput) -> Self {
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
        let mut shared = BTreeMap::new();
        for &(loader, ground, _) in digs.keys() {
            let other = digs.keys().any(|&(other, block, _)| other != loader && block == ground);
            shared.insert((loader, ground), other);
        }
        let values: Vec<f64> = input.movements.iter().map(|candidate| candidate.value_per_tonne().unwrap_or(0.0)).collect();
        let tie = TIE_WEIGHT * values.iter().fold(1.0_f64, |largest, value| largest.max(value.abs()));
        // Bound marginal target cost over the grades carried by actual sources. This
        // keeps production preferable even when no movement values exist.
        let ceilings = super::input::grade_ceilings(input);
        let mut target_costs = BTreeMap::<(DestinationId, usize), f64>::new();
        for target in &input.grade_targets {
            let cost = target
                .specification
                .hinges()
                .iter()
                .map(|&(boundary, _, slope)| slope * boundary.abs().max((ceilings[target.grade] - boundary).abs()))
                .sum::<f64>();
            let entry = target_costs.entry((target.destination, target.grade)).or_default();
            *entry = entry.max(cost);
        }
        let target_credit: f64 = target_costs.values().sum();
        let credit = -values.iter().copied().fold(0.0_f64, f64::min) + target_credit + tie;
        let latest = input.movements.iter().map(|candidate| candidate.routing_preference).max().unwrap_or(0);
        let weight = input
            .movements
            .iter()
            .zip(&values)
            .map(|(candidate, value)| value + credit - tie * 0.5 * f64::from(candidate.routing_preference) / f64::from(latest + 1))
            .collect();
        Self {
            input,
            target_totals: input.target_opening.iter().map(|&(i, p, t, q)| ((i, p), (t, q))).collect(),
            ground: input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect(),
            piles: input.piles.iter().map(|pile| (pile.id, pile.total_opening(input.grades.count()))).collect(),
            chunks: input
                .piles
                .iter()
                .filter(|pile| !pile.chunks.is_empty())
                .map(|pile| (pile.id, opening_chunks(pile, input.grades.count())))
                .collect(),
            chunk_rows: Vec::new(),
            dumped: BTreeMap::new(),
            crushed: BTreeMap::new(),
            reclaimed: BTreeMap::new(),
            digs,
            reclaims,
            shared,
            weight,
            rows: BTreeMap::new(),
            durations: BTreeMap::new(),
        }
    }

    fn work(&mut self, interval: Interval) -> Result<(), String> {
        let input = self.input;
        let k = interval.index;
        let duration = interval.duration_h();
        for segment in 0..input.segments_per_interval.max(1) {
            self.durations.insert((k, segment), if segment == 0 { duration } else { 0.0 });
        }
        // Readiness is judged on the state the interval opened with, as the
        // replay judges it.
        let opening_ground = self.ground.clone();
        let mut bars: Vec<Bar> = (0..input.loaders.len())
            .filter_map(|loader| {
                let rate = interval_rate(&input.loaders[loader], k)?;
                let task = authored_tasks(input, loader)
                    .into_iter()
                    .find(|&task| self.ready(task, interval, rate.dig_tph, rate.reclaim_tph, &opening_ground))?;
                match &input.tasks[task].kind {
                    TaskKind::Dig { sequence } => Some(Bar {
                        loader,
                        task,
                        capacity: rate.dig_tph * duration,
                        blocks: self.next_block(loader, sequence, None, &opening_ground).into_iter().collect(),
                    }),
                    TaskKind::Reclaim { maximum_t, .. } => Some(Bar {
                        loader,
                        task,
                        capacity: maximum_t.map_or(rate.reclaim_tph * duration, |maximum| {
                            (rate.reclaim_tph * duration).min(maximum - self.reclaimed.get(&task).copied().unwrap_or(0.0))
                        }),
                        blocks: Vec::new(),
                    }),
                    // The loader's highest-priority bar is a delay: it stands.
                    TaskKind::Delay => None,
                }
            })
            .collect();

        // Solve, and while a loader finishes its last block with rate to
        // spare, open the next one and solve again with the finished block
        // held finished. The earlier answer stays feasible, so no re-solve is
        // worth less.
        let mut plan = self.solve(interval, &bars)?;
        loop {
            let mut grew = false;
            for bar in &mut bars {
                let TaskKind::Dig { sequence } = &input.tasks[bar.task].kind else { continue };
                let Some(&last) = bar.blocks.last() else { continue };
                let left = self.ground.get(&last).copied().unwrap_or(0.0);
                let used: f64 = bar.blocks.iter().map(|block| plan.extracted.get(&(bar.loader, *block)).copied().unwrap_or(0.0)).sum();
                let finished = plan.extracted.get(&(bar.loader, last)).copied().unwrap_or(0.0) >= left - SNAP_T;
                if !finished || bar.capacity - used <= NEGLIGIBLE_T || self.shared.get(&(bar.loader, last)).copied().unwrap_or(false) {
                    continue;
                }
                if let Some(next) = self.next_block(bar.loader, sequence, Some(last), &opening_ground) {
                    bar.blocks.push(next);
                    grew = true;
                }
            }
            if !grew {
                break;
            }
            plan = self.solve(interval, &bars)?;
        }
        self.apply(interval, plan);
        Ok(())
    }

    /// The first block of `sequence` after `after` with tonnes left that
    /// authored order lets `loader` start, `after` itself counting as
    /// finished in this interval.
    fn next_block(&self, loader: usize, sequence: &[GroundId], after: Option<GroundId>, opening_ground: &BTreeMap<GroundId, f64>) -> Option<GroundId> {
        let start = after.map_or(0, |after| sequence.iter().position(|block| *block == after).map_or(sequence.len(), |position| position + 1));
        let mut previous = after;
        for &block in &sequence[start..] {
            // A block the input no longer holds was finished before it.
            let Some(&left) = self.ground.get(&block) else {
                previous = Some(block);
                continue;
            };
            if let Some(earlier) = previous.filter(|earlier| Some(*earlier) != after) {
                // The formulation's `order` row: the earlier block gone by
                // the end of this cell, or of the one before when another
                // loader could also have dug it.
                let shared = self.shared.get(&(loader, earlier)).copied().unwrap_or(false);
                let gone = if shared { opening_ground.get(&earlier) } else { self.ground.get(&earlier) };
                if gone.is_some_and(|left| *left > FINISHED_T) {
                    return None;
                }
            }
            previous = Some(block);
            if left > FINISHED_T {
                return Some(block);
            }
        }
        None
    }

    /// The interval's linear program over `bars`, solved.
    fn solve(&self, interval: Interval, bars: &[Bar]) -> Result<Plan, String> {
        let input = self.input;
        let mut problem = RowProblem::default();
        let mut columns: Vec<(usize, Col)> = Vec::new();
        let mut extractions: Vec<((usize, GroundId), Col)> = Vec::new();
        let mut block_total: BTreeMap<GroundId, Vec<(Col, f64)>> = BTreeMap::new();
        let mut pile_draw: BTreeMap<StockpileId, Vec<(Col, f64)>> = BTreeMap::new();
        for bar in bars {
            let task = &input.tasks[bar.task];
            let mut loader_total = Vec::new();
            match &task.kind {
                TaskKind::Dig { .. } => {
                    for (position, &block) in bar.blocks.iter().enumerate() {
                        let left = self.ground.get(&block).copied().unwrap_or(0.0);
                        let Some(source) = input.ground.iter().find(|source| source.id == block) else {
                            continue;
                        };
                        let extract = if position + 1 < bar.blocks.len() {
                            problem.add_column(0.0, left..=left)
                        } else {
                            problem.add_column(0.0, 0.0..=left)
                        };
                        extractions.push(((bar.loader, block), extract));
                        loader_total.push((extract, 1.0));
                        block_total.entry(block).or_default().push((extract, 1.0));
                        for share in &source.material {
                            let mut portion = vec![(extract, -share.fraction)];
                            for &index in self.digs.get(&(bar.loader, block, share.material.0)).into_iter().flatten() {
                                if task_authorises(task, &input.movements[index]) {
                                    let col = problem.add_column(self.weight[index], 0.0..);
                                    columns.push((index, col));
                                    portion.push((col, 1.0));
                                }
                            }
                            problem.add_row(0.0..=0.0, portion);
                        }
                    }
                }
                TaskKind::Reclaim { approved_sources, .. } => {
                    for pile in approved_sources {
                        if self.released(*pile).is_none_or(|released| released.tonnes <= NEGLIGIBLE_T) {
                            continue;
                        }
                        for index in self.reclaim_candidates(bar.loader, *pile, bar.task, interval) {
                            let col = problem.add_column(self.weight[index], 0.0..);
                            columns.push((index, col));
                            loader_total.push((col, 1.0));
                            pile_draw.entry(*pile).or_default().push((col, 1.0));
                        }
                    }
                }
                TaskKind::Delay => {}
            }
            problem.add_row(..=bar.capacity.max(0.0), loader_total);
        }
        for (block, terms) in block_total {
            problem.add_row(..=self.ground.get(&block).copied().unwrap_or(0.0), terms);
        }
        for (pile, terms) in pile_draw {
            problem.add_row(..=self.released(pile).map_or(0.0, |released| released.tonnes), terms);
        }

        // Shared room: truck hours per class, and each destination's.
        let mut trucks: BTreeMap<TruckClassId, Vec<(Col, f64)>> = BTreeMap::new();
        let mut destinations: BTreeMap<DestinationId, Vec<(Col, f64)>> = BTreeMap::new();
        for &(index, col) in &columns {
            let candidate = &input.movements[index];
            if candidate.truck_hours_per_tonne > 0.0 {
                trucks.entry(candidate.truck).or_default().push((col, candidate.truck_hours_per_tonne));
            }
            destinations.entry(candidate.destination).or_default().push((col, 1.0));
        }
        for (truck, terms) in trucks {
            if let Some(hours) = input.trucks.iter().find(|entry| entry.id == truck).and_then(|entry| entry.hours.get(interval.index)) {
                problem.add_row(..=*hours, terms);
            }
        }
        for (destination, terms) in destinations {
            let room = self.destination_room(destination, interval);
            if room.is_finite() {
                problem.add_row(..=room, terms);
            }
        }

        if columns.is_empty() && extractions.is_empty() {
            return Ok(Plan::default());
        }
        for (index, target) in input.grade_targets.iter().enumerate().filter(|(_, t)| t.applies(interval.start_h)) {
            let period = crate::model::schedule::grade_targets::target_day(interval.start_h);
            let (opening_t, opening_q) = self.target_totals.get(&(index, period)).copied().unwrap_or_default();
            for (boundary, direction, slope) in target.specification.hinges() {
                if slope == 0.0 {
                    continue;
                }
                let slack = problem.add_column(-slope, 0.0..);
                let mut terms = vec![(slack, -1.0)];
                for &(candidate, column) in &columns {
                    let movement = &input.movements[candidate];
                    if movement.destination != target.destination {
                        continue;
                    }
                    let fraction = match movement.source {
                        SourceId::Stockpile(pile) if movement.activity == Activity::Reclaim => self.released(pile).and_then(|r| r.blend.get(target.grade).copied()),
                        _ => input.grades.fraction(movement.material, target.grade),
                    }
                    .unwrap_or(0.0);
                    terms.push((column, direction * (fraction - boundary)));
                }
                problem.add_row(..=-direction * (opening_q - boundary * opening_t), terms);
            }
        }
        let mut model = problem.optimise(Sense::Maximise);
        model.make_quiet();
        // One thread, so the same input always gives the same schedule.
        model.set_option("threads", 1);
        let solved = model.try_solve().map_err(|status| format!("HiGHS failed on interval {}: {status:?}", interval.index))?;
        if solved.status() != HighsModelStatus::Optimal {
            return Err(format!("interval {}: HiGHS ended {:?}", interval.index, solved.status()));
        }
        let solution = solved.get_solution();
        Ok(Plan {
            rows: columns.into_iter().map(|(index, col)| (index, solution[col].max(0.0))).collect(),
            extracted: extractions.into_iter().map(|(key, col)| (key, solution[col].max(0.0))).collect(),
        })
    }

    /// Record an interval's answer, made exact: an extraction within
    /// [`SNAP_T`] of finishing its block finishes it, and each material's
    /// rows are scaled to that material's share of the extraction.
    fn apply(&mut self, interval: Interval, plan: Plan) {
        let input = self.input;
        let k = interval.index;
        let mut extracted = plan.extracted;
        for ((_, block), tonnes) in &mut extracted {
            let left = self.ground.get(block).copied().unwrap_or(0.0);
            if *tonnes >= left - SNAP_T {
                *tonnes = left;
            } else if *tonnes < NEGLIGIBLE_T {
                *tonnes = 0.0;
            }
        }
        // Dig rows by (loader, block, material), for the rescale.
        let mut portions: BTreeMap<(usize, GroundId, u32), Vec<(usize, f64)>> = BTreeMap::new();
        let mut rows = Vec::new();
        for (index, tonnes) in plan.rows {
            let candidate = &input.movements[index];
            match candidate.source {
                SourceId::Ground(block) => {
                    let loader = input.loaders.iter().position(|loader| loader.id == candidate.loader).expect("a dig candidate's loader");
                    portions.entry((loader, block, candidate.material.0)).or_default().push((index, tonnes));
                }
                _ if tonnes > NEGLIGIBLE_T => rows.push((index, tonnes)),
                _ => {}
            }
        }
        for ((loader, block, material), entries) in portions {
            let fraction = input
                .ground
                .iter()
                .find(|source| source.id == block)
                .and_then(|source| source.material.iter().find(|share| share.material.0 == material))
                .map_or(0.0, |share| share.fraction);
            let target = fraction * extracted.get(&(loader, block)).copied().unwrap_or(0.0);
            let total: f64 = entries.iter().map(|(_, tonnes)| tonnes).sum();
            if target <= 0.0 || total <= 0.0 {
                continue;
            }
            rows.extend(
                entries
                    .into_iter()
                    .map(|(index, tonnes)| (index, tonnes * target / total))
                    .filter(|(_, tonnes)| *tonnes > FINISHED_T),
            );
        }
        for ((_, block), tonnes) in extracted {
            if let Some(left) = self.ground.get_mut(&block) {
                *left = (*left - tonnes).max(0.0);
            }
        }

        // A draw within [`SNAP_T`] of emptying what its pile released
        // empties it, so a drained chunk reads as empty to the order rules.
        let mut released: BTreeMap<StockpileId, Released> = BTreeMap::new();
        for pile in &input.piles {
            if let Some(found) = self.released(pile.id) {
                released.insert(pile.id, found);
            }
        }
        for (pile, found) in &released {
            let drawn: f64 = rows
                .iter()
                .filter(|(index, _)| input.movements[*index].source == SourceId::Stockpile(*pile))
                .map(|(_, tonnes)| tonnes)
                .sum();
            if drawn > 0.0 && drawn >= found.tonnes - SNAP_T && drawn != found.tonnes {
                let scale = found.tonnes / drawn;
                for (index, tonnes) in &mut rows {
                    if input.movements[*index].source == SourceId::Stockpile(*pile) {
                        *tonnes *= scale;
                    }
                }
            }
        }

        // Rows, allowances and pile balances.
        let grades = input.grades.count();
        let mut drawn: BTreeMap<StockpileId, f64> = BTreeMap::new();
        let mut received: BTreeMap<StockpileId, (f64, Vec<f64>)> = BTreeMap::new();
        let mut receipts: BTreeMap<StockpileId, Vec<(usize, f64)>> = BTreeMap::new();
        for (index, tonnes) in rows {
            let candidate = &input.movements[index];
            *self.rows.entry((index, k)).or_default() += tonnes;
            let blend = match candidate.source {
                SourceId::Stockpile(pile) => {
                    *drawn.entry(pile).or_default() += tonnes;
                    if let Some(task) = self.reclaim_task(candidate, interval) {
                        *self.reclaimed.entry(task).or_default() += tonnes;
                    }
                    released.get(&pile).map(|found| found.blend.clone()).unwrap_or_else(|| vec![0.0; grades])
                }
                _ => (0..grades).map(|grade| input.grades.fraction(candidate.material, grade).unwrap_or(0.0)).collect(),
            };
            for (i, target) in input
                .grade_targets
                .iter()
                .enumerate()
                .filter(|(_, t)| t.destination == candidate.destination && t.applies(interval.start_h))
            {
                let entry = self
                    .target_totals
                    .entry((i, crate::model::schedule::grade_targets::target_day(interval.start_h)))
                    .or_default();
                entry.0 += tonnes;
                entry.1 += tonnes * blend.get(target.grade).copied().unwrap_or(0.0);
            }
            let Some(destination) = input.destinations.iter().find(|entry| entry.id == candidate.destination) else {
                continue;
            };
            match destination.kind {
                DestinationKind::Dump => *self.dumped.entry(destination.id).or_default() += tonnes,
                DestinationKind::Crusher => *self.crushed.entry((destination.id, interval.day())).or_default() += tonnes,
                DestinationKind::Stockpile(pile) => {
                    receipts.entry(pile).or_default().push((index, tonnes));
                    let entry = received.entry(pile).or_insert_with(|| (0.0, vec![0.0; grades]));
                    entry.0 += tonnes;
                    for (contained, grade) in entry.1.iter_mut().zip(blend) {
                        *contained += tonnes * grade;
                    }
                }
            }
        }
        for (pile, (open_t, open_q)) in &mut self.piles {
            if let Some(chunks) = self.chunks.get_mut(pile) {
                let reclaimed = drawn.get(pile).copied().unwrap_or(0.0);
                let drawn_chunk = released.get(pile).and_then(|found| found.chunk);
                let receiving = chunks.iter().position(|chunk| !chunk.closed);
                let pile_receipts = receipts.remove(pile).unwrap_or_default();
                for (index, chunk) in chunks.iter_mut().enumerate() {
                    let draw = if Some(index) == drawn_chunk { reclaimed.min(chunk.held_t) } else { 0.0 };
                    let (taken_t, taken_q) = if Some(index) == receiving {
                        received.get(pile).cloned().unwrap_or((0.0, vec![0.0; grades]))
                    } else {
                        (0.0, vec![0.0; grades])
                    };
                    self.chunk_rows.push(ChunkRow {
                        pile: *pile,
                        chunk: index,
                        interval: k,
                        open_t: chunk.held_t,
                        reclaimed_t: draw,
                        closed: chunk.closed,
                        received_t: taken_t,
                        receipts: if Some(index) == receiving { pile_receipts.clone() } else { Vec::new() },
                    });
                    if draw > 0.0 && chunk.held_t > 0.0 {
                        let share = draw / chunk.held_t;
                        for contained in &mut chunk.held_q {
                            *contained -= *contained * share;
                        }
                    }
                    chunk.held_t -= draw;
                    if draw > 0.0 && chunk.held_t <= FINISHED_T {
                        chunk.held_t = 0.0;
                        chunk.held_q.iter_mut().for_each(|contained| *contained = 0.0);
                    }
                    chunk.held_t += taken_t;
                    for (contained, taken) in chunk.held_q.iter_mut().zip(&taken_q) {
                        *contained += taken;
                    }
                    // Full, it closes for the next interval.
                    if !chunk.closed && chunk.capacity_t - chunk.held_t <= SNAP_T {
                        chunk.closed = true;
                    }
                }
                *open_t = chunks.iter().map(|chunk| chunk.held_t).sum();
                *open_q = (0..grades).map(|grade| chunks.iter().map(|chunk| chunk.held_q[grade]).sum()).collect();
                continue;
            }
            let reclaimed = drawn.get(pile).copied().unwrap_or(0.0).min(*open_t);
            if reclaimed > 0.0 && *open_t > 0.0 {
                let share = reclaimed / *open_t;
                for contained in open_q.iter_mut() {
                    *contained -= *contained * share;
                }
            }
            *open_t -= reclaimed;
            if let Some((received_t, received_q)) = received.get(pile) {
                *open_t += received_t;
                for (contained, received) in open_q.iter_mut().zip(received_q) {
                    *contained += received;
                }
            }
        }
    }

    /// What `pile` releases to reclaim this interval: an unchunked pile its
    /// whole opening blend, a chunked one the chunk its order releases, if
    /// any.
    fn released(&self, pile: StockpileId) -> Option<Released> {
        let blend = |tonnes: f64, contained: &[f64]| contained.iter().map(|value| if tonnes > 0.0 { value / tonnes } else { 0.0 }).collect();
        let Some(chunks) = self.chunks.get(&pile) else {
            let (open_t, open_q) = self.piles.get(&pile)?;
            return Some(Released {
                chunk: None,
                tonnes: *open_t,
                blend: blend(*open_t, open_q),
            });
        };
        let order = self.input.piles.iter().find(|entry| entry.id == pile)?.order;
        let index = match order {
            ReclaimOrder::Fifo => chunks.iter().position(|chunk| chunk.held_t > FINISHED_T).filter(|&index| chunks[index].closed)?,
            ReclaimOrder::Lifo => chunks.iter().rposition(|chunk| chunk.closed && chunk.held_t > FINISHED_T)?,
        };
        let chunk = &chunks[index];
        Some(Released {
            chunk: Some(index),
            tonnes: chunk.held_t,
            blend: blend(chunk.held_t, &chunk.held_q),
        })
    }

    /// The reclaim bar a reclaim row was worked under: its loader's first
    /// active bar that authorises it.
    fn reclaim_task(&self, candidate: &MovementCandidate, interval: Interval) -> Option<usize> {
        let loader = self.input.loaders.iter().position(|loader| loader.id == candidate.loader)?;
        authored_tasks(self.input, loader)
            .into_iter()
            .find(|&task| task_active(&self.input.tasks[task], interval) && task_authorises(&self.input.tasks[task], candidate))
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
            TaskKind::Delay => true,
        }
    }

    /// Reclaim candidates for one loader and pile under `bar`, admitted on
    /// the blend the pile releases with the formulation's margin to spare.
    fn reclaim_candidates(&self, loader: usize, pile: StockpileId, bar: usize, interval: Interval) -> Vec<usize> {
        let input = self.input;
        let Some(Released { tonnes, blend, .. }) = self.released(pile) else { return Vec::new() };
        let band = GRADE_MARGIN + GRADE_CUSHION_T / tonnes.max(GRADE_CUSHION_T) + 1e-9;
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

    /// What `destination` can still take in `interval`.
    fn destination_room(&self, destination: DestinationId, interval: Interval) -> f64 {
        let input = self.input;
        let Some(destination) = input.destinations.iter().find(|entry| entry.id == destination) else {
            return 0.0;
        };
        match destination.kind {
            DestinationKind::Dump => destination
                .capacity_t
                .map_or(f64::INFINITY, |capacity| capacity - self.dumped.get(&destination.id).copied().unwrap_or(0.0)),
            DestinationKind::Crusher => match destination.crusher_daily_t.get(interval.day() as usize).copied().flatten() {
                Some(budget) => budget - self.crushed.get(&(destination.id, interval.day())).copied().unwrap_or(0.0),
                None => f64::INFINITY,
            },
            // Receipts must fit on top of the opening stock, with no credit
            // for what the interval reclaims: the replay's stricter check
            // when a delivering loader works blocks back to back.
            // A chunked pile also takes no more than its receiving chunk's room.
            DestinationKind::Stockpile(pile) => {
                let Some(target) = input.piles.iter().find(|entry| entry.id == pile) else { return 0.0 };
                let room = target.capacity_t - self.piles.get(&pile).map_or(0.0, |(tonnes, _)| *tonnes);
                match self.chunks.get(&pile) {
                    Some(chunks) => chunks.iter().find(|chunk| !chunk.closed).map_or(0.0, |chunk| room.min(chunk.capacity_t - chunk.held_t)),
                    None => room,
                }
            }
        }
        .max(0.0)
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
        let reported_objective: f64 = movements
            .iter()
            .map(|row| row.tonnes_t * input.movements[row.candidate].value_per_tonne().unwrap_or(0.0))
            .sum();
        BlendSolution {
            movements,
            durations: self.durations,
            chunks: self.chunk_rows,
            reported_objective: reported_objective - super::replay::target_penalty(input, &self.target_totals),
            adjustments: ExtractionAdjustments::default(),
        }
    }
}

/// A chunked pile's chunks as the horizon opens. Without authored per-chunk
/// opening the pile's opening goes into chunk 0, as the formulation puts it.
fn opening_chunks(pile: &BlendPile, grades: usize) -> Vec<Chunk> {
    pile.chunks
        .iter()
        .enumerate()
        .map(|(index, capacity_t)| {
            let (held_t, mut held_q) = match pile.chunk_opening.get(index) {
                Some((tonnes, contained)) => (*tonnes, contained.clone()),
                None if pile.chunk_opening.is_empty() && index == 0 => (pile.opening_t, pile.opening_q.clone()),
                None => (0.0, Vec::new()),
            };
            held_q.resize(grades, 0.0);
            Chunk {
                capacity_t: *capacity_t,
                held_t,
                held_q,
                closed: pile.chunk_starts_closed(index),
            }
        })
        .collect()
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
