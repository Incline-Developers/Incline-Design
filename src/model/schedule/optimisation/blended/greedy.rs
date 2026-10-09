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
//!   the start of the interval. A dig bar whose current block has a material
//!   with nowhere to go - every destination full, or a pile not building - has
//!   no work, so the loader works its next bar and comes back the first
//!   interval there is room; see `OutletIndex::of`.
//! - A dig bar works its current block, and the next ones in authored order
//!   once each is finished, up to the loader's rate. A block's materials
//!   leave in proportion. A block another loader may also dig must be gone a
//!   whole interval before the next one starts, as the formulation's `order`
//!   rows require.
//! - A reclaim bar draws the released opening blend of one of its approved
//!   piles at a time, within its authored cap.
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
//! A loader with a utilisation incentive is paid, in the objective only, for
//! each dug tonne by how far its scheduled utilisation so far today - hours
//! worked at its effective rates over the day's productive hours to the end
//! of the interval - stands below its target, in bands of
//! [`super::utilisation::BAND`]. The lower bands pay more, so the program fills them
//! first: the loader furthest behind is trucked up first. It is never money:
//! segments are still kept only when the money is no less, and the reported
//! value is the movements' own.
//!
//! Nothing here looks past the interval, so it claims nothing about the
//! horizon's optimum: stockpiling for later, saving a crusher day for better
//! feed and the like are the whole-horizon solve's to find. Its answer is
//! only used after the replay has accepted it.
//!
//! An interval is worked in execution segments sharing one clock, their
//! lengths the program's to choose, up to the captured budget. It starts as
//! one. A loader whose bar has no work left before the interval ends - every
//! block of its sequence dug, or its cap spent - moves to its next bar in a
//! segment of its own, and a reclaim bar that emptied its pile to another
//! pile it approves; the whole interval is solved again, and the new segment
//! kept only when the program values it more and the interval's money is no
//! less: the production credit alone does not buy a segment. Inventory,
//! blends, chunks, rests and the drill and blast chain still move once per
//! interval: receipts are reclaimable only from the next one, and take room
//! on top of what earlier segments reclaimed.
//!
//! Moving to the next bar early can still take ground a later bar would
//! have been worth more from, which no interval sees. So whenever segments
//! were used, the schedule of one bar per loader per interval is worked too,
//! both are replayed, and the one worth more is kept.
//!
//! A reclaim goes where a grade decides admission - a route
//! qualification or a minimum grade - only when the pile's released blend
//! clears the boundary by the formulation's own margin. A grade-conditional
//! value is priced on that same released blend, as the formulation prices
//! it: a reward only clear inside its bounds, a cost unless clear outside.
//!
//! A stockpile's authored mode is a fact of the interval: a pile not
//! building has no room, so routing passes it over as it would a full one,
//! and a pile not reclaiming gives a reclaim bar no work, so its loader moves
//! on to its next bar as it would from an empty pile.
//!
//! A chunked pile follows the formulation's chunk lifecycle: chunks fill in
//! their order, and a chunk closes when it is full and at no other time.
//! Receipts go to the first chunk still open, up to its room, so one left
//! partly filled when its pile stops building is topped up when building
//! resumes. A chunk is open or closed for a whole interval, so one that fills
//! during an interval closes at the start of the next, and the next chunk
//! starts receiving then. A chunk holding material is reclaimable once its
//! last delivery has rested, open or closed. Reclaim draws one chunk, the one
//! next in the authored order - FIFO the first holding material, LIFO the
//! last - and only once it has rested, at that chunk's own blend: a chunk
//! still resting holds the whole pile, as a loader waits at the face rather
//! than digging in behind it.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
};

use super::{
    drill_blast::Chain,
    input::{
        BlendInput, BlendPile, DIG_ROOM_T, GRADE_CUSHION_T, GRADE_MARGIN, GradeQualification, OutletIndex, REST_RECEIPT_T, attribute_reclaim, authored_tasks, block_diggable,
        interval_rate, task_active, task_authorises,
    },
    lp::{Col, LinearProgram},
    plan::PlanTargets,
    replay::{BlendSolution, ChunkRow, ExtractionAdjustments, MovementRow, ReplayReport, replay_cancellable},
};
use crate::model::schedule::optimisation::{Activity, DestinationId, DestinationKind, GroundId, Interval, LoaderId, ReclaimOrder, SourceId, StockpileId, TaskKind, TruckClassId};

/// Remaining tonnes at or below which a block counts as finished.
const FINISHED_T: f64 = 1e-9;

/// Below this a dig or a reclaim is not worth a movement row, and loader
/// capacity left over does not open the next block.
const NEGLIGIBLE_T: f64 = 1e-6;

/// An extraction this close to a block's remainder finishes it: the linear
/// program's own tolerance would otherwise leave dust behind.
const SNAP_T: f64 = 1e-6;

/// Ground a block must hold for a bar to have work whatever the replay's
/// tolerance makes of it.
const SURELY_HELD_T: f64 = 1e-3;

/// How much more, relative to the interval's objective, a schedule with one
/// more segment must be worth to be kept: a segment the program leaves empty
/// would only use up the interval's budget.
const IMPROVEMENT: f64 = 1e-9;

/// The tie-break weight per tonne, relative to the largest movement value.
const TIE_WEIGHT: f64 = 1e-7;

/// How much of the interval's money, relative, one more segment may give up
/// to the program's own rounding and still be kept.
const MONEY_SLACK: f64 = 1e-7;

/// A dispatch schedule and the replay's report on it.
pub(crate) struct Dispatched {
    pub(crate) solution: BlendSolution,
    pub(crate) replay: ReplayReport,
}

/// A dispatch schedule for `input`, replayed, or why there is none, polling
/// `cancel` within each interval's work: `Ok(None)` once it is set.
///
/// With segments to use, the schedule of one bar per loader per interval is
/// worked beside the one that moves loaders on within an interval, and of the
/// two the replay accepts, the one it values more is kept. The replay is
/// returned with it, so the caller need not run it again.
pub(crate) fn dispatch_cancellable(input: &BlendInput, cancel: &AtomicBool) -> Result<Option<Dispatched>, String> {
    dispatch_following(input, None, cancel)
}

/// [`dispatch_cancellable`], steered towards a whole-horizon plan's targets
/// when there is one; see [`super::plan`].
pub(crate) fn dispatch_following(input: &BlendInput, plan: Option<&PlanTargets>, cancel: &AtomicBool) -> Result<Option<Dispatched>, String> {
    let checked = |found: Result<Option<BlendSolution>, String>| -> Result<Option<Dispatched>, String> {
        let Some(solution) = found? else { return Ok(None) };
        let Some(replay) = replay_cancellable(input, &solution, cancel) else { return Ok(None) };
        Ok(Some(Dispatched { solution, replay }))
    };
    if input.segments_per_interval <= 1 {
        return checked(dispatch(input, plan, cancel, false));
    }
    let (segmented, single) = rayon::join(|| checked(dispatch(input, plan, cancel, true)), || checked(dispatch(input, plan, cancel, false)));
    let (segmented, single) = match (segmented, single) {
        (Ok(Some(segmented)), Ok(Some(single))) => (segmented, single),
        (Ok(None), _) | (_, Ok(None)) => return Ok(None),
        (Ok(Some(found)), Err(_)) | (Err(_), Ok(Some(found))) => return Ok(Some(found)),
        (Err(reason), Err(_)) => return Err(reason),
    };
    if !segmented.replay.is_valid() {
        let issue = segmented.replay.issues.iter().chain(&segmented.replay.grade_issues).next().cloned().unwrap_or_default();
        log::warn!("hourly dispatch: bar changes within an interval were rejected by the replay ({issue}); one bar per interval instead");
        return Ok(Some(single));
    }
    let (worth, baseline) = (segmented.replay.ranked_objective(), single.replay.ranked_objective());
    if single.replay.is_valid() && baseline > worth + MONEY_SLACK * worth.abs().max(1.0) {
        log::info!("hourly dispatch: one bar per interval is worth {baseline:.2} against {worth:.2} with bar changes within an interval; keeping it");
        return Ok(Some(single));
    }
    Ok(Some(segmented))
}

/// The dispatch schedule, with loaders moving to their next bar inside an
/// interval when `segmented`.
fn dispatch(input: &BlendInput, plan: Option<&PlanTargets>, cancel: &AtomicBool, segmented: bool) -> Result<Option<BlendSolution>, String> {
    let mut state = State::new(input, cancel);
    state.segmented = segmented;
    state.plan = plan;
    for interval in &input.intervals {
        if state.cancelled() {
            return Ok(None);
        }
        match state.work(*interval) {
            // An interval cut short reports why it stopped as a failure.
            Err(_) if state.cancelled() => return Ok(None),
            found => found?,
        }
    }
    if state.cancelled() {
        return Ok(None);
    }
    Ok(Some(state.finish()))
}

/// The dispatcher's physical state, walked forward an interval at a time.
struct State<'a> {
    input: &'a BlendInput,
    target_totals: BTreeMap<(usize, u32), (f64, f64)>,
    /// Per (loader, day): hours worked at the loader's effective rates and
    /// productive hours it had, over the intervals of the day so far. Their
    /// ratio is the scheduled utilisation the incentive is priced against.
    utilisation: BTreeMap<(usize, u32), (f64, f64)>,
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
    /// End of the last interval each pile received in, for its rest.
    last_receipt_h: BTreeMap<StockpileId, f64>,
    /// Piles that may not build and reclaim at once, closed to deliveries
    /// this interval because a reclaim bar is drawing them.
    blocked: BTreeSet<StockpileId>,
    /// Such piles left to building this interval: closed to reclaim.
    building: BTreeSet<StockpileId>,
    /// Keyed by task index.
    reclaimed: BTreeMap<usize, f64>,
    /// Dig candidates by (loader, block, material) and reclaim candidates
    /// by (loader, pile).
    digs: BTreeMap<(usize, GroundId, u32), Vec<usize>>,
    reclaims: BTreeMap<(usize, StockpileId), Vec<usize>>,
    outlets: OutletIndex,
    /// Blocks some other loader could dig, keyed (loader, block).
    shared: BTreeMap<(usize, GroundId), bool>,
    /// Objective per tonne of each candidate: its value, the production
    /// credit and the tie-breaks. A reclaim's grade-conditional value is
    /// added per interval, on the blend it draws.
    weight: Vec<f64>,
    /// What of each candidate's weight is not money: the production credit
    /// and the tie-breaks.
    credit: Vec<f64>,
    /// Grade-conditional value earned by the rows so far.
    conditional: f64,
    /// Keyed (candidate, interval, segment).
    rows: BTreeMap<(usize, usize, usize), f64>,
    durations: BTreeMap<(usize, usize), f64>,
    /// The drill and blast chain, walked beside the loaders; see
    /// [`super::drill_blast`]. A block it has not released is not dug.
    chain: Option<Chain<'a>>,
    /// Start of the interval being worked, for the chain's releases.
    now_h: f64,
    /// Whether a loader may move to its next bar inside an interval.
    segmented: bool,
    /// Targets to follow, and the dug tonnes each (loader, destination, day)
    /// has delivered so far.
    plan: Option<&'a PlanTargets>,
    followed: BTreeMap<(usize, DestinationId, u32), f64>,
    cancel: &'a AtomicBool,
}

/// One chunk of a chunked pile, as it opens an interval.
struct Chunk {
    capacity_t: f64,
    held_t: f64,
    held_q: Vec<f64>,
    closed: bool,
    /// End of the last interval it received in, for its rest; `None` for
    /// none in this run.
    received_h: Option<f64>,
}

/// What a pile releases to reclaim in an interval.
struct Released {
    /// The chunk drawn, for a chunked pile.
    chunk: Option<usize>,
    tonnes: f64,
    blend: Vec<f64>,
}

/// One loader's work in one segment of an interval.
#[derive(Clone)]
struct Bar {
    loader: usize,
    task: usize,
    /// Dig bars only: the blocks it works in the segment, in authored order.
    /// Every block before the last is finished by the segment's end.
    blocks: Vec<GroundId>,
    /// Reclaim bars only: the one pile it draws in the segment, once the
    /// program has picked it from those the bar approves.
    pile: Option<StockpileId>,
    /// Reclaim bars only: piles the bar emptied in an earlier segment.
    emptied: Vec<StockpileId>,
}

/// What the loaders work in an interval, segment by segment.
///
/// Segments share one clock across loaders, and their lengths are the
/// program's to choose. A loader moves to its next bar only once its bar has
/// no work left - every block of its sequence dug, or its cap spent - and
/// that is written into the program, so the replay finds the same bar its
/// highest-priority ready one segment by segment.
#[derive(Clone)]
struct Topology {
    segments: Vec<Vec<Bar>>,
    /// (block, segment): the block's ground is gone by the end of the
    /// segment, whichever loaders dig it.
    pins: Vec<(GroundId, usize)>,
    /// (task, segment): the reclaim bar's cap is spent by the end of the
    /// segment.
    spent: Vec<(usize, usize)>,
}

impl Topology {
    /// Blocks gone before the last segment opens.
    fn done(&self) -> BTreeSet<GroundId> {
        self.pins.iter().map(|(block, _)| *block).collect()
    }
}

/// An interval's answer.
#[derive(Default)]
struct Plan {
    /// (candidate, segment, tonnes).
    rows: Vec<(usize, usize, f64)>,
    /// Keyed (loader, block, segment).
    extracted: BTreeMap<(usize, GroundId, usize), f64>,
    durations: Vec<f64>,
    objective: f64,
    /// The objective less the production credit and tie-breaks: what the
    /// interval's movements are worth.
    money: f64,
}

impl Plan {
    /// What `loader` dug of `block` across the interval.
    fn dug(&self, loader: usize, block: GroundId) -> f64 {
        self.extracted
            .iter()
            .filter(|((by, at, _), _)| *by == loader && *at == block)
            .map(|(_, tonnes)| tonnes)
            .sum()
    }

    /// What every loader dug of `block` across the interval.
    fn dug_by_all(&self, block: GroundId) -> f64 {
        self.extracted.iter().filter(|((_, at, _), _)| *at == block).map(|(_, tonnes)| tonnes).sum()
    }
}

impl<'a> State<'a> {
    fn new(input: &'a BlendInput, cancel: &'a AtomicBool) -> Self {
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
        // The most a reclaim's conditional costs could take off its value,
        // so the production credit covers them too.
        let mut worst = values.clone();
        for entry in input.conditional_values.iter().filter(|entry| entry.value_per_tonne < 0.0) {
            if let Some(value) = worst.get_mut(entry.candidate) {
                *value += entry.value_per_tonne;
            }
        }
        let largest = input
            .conditional_values
            .iter()
            .map(|entry| entry.value_per_tonne.abs())
            .chain(values.iter().map(|value| value.abs()))
            .fold(1.0_f64, f64::max);
        let tie = TIE_WEIGHT * largest;
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
        let credit = -worst.iter().copied().fold(0.0_f64, f64::min) + target_credit + tie;
        let latest = input.movements.iter().map(|candidate| candidate.routing_preference).max().unwrap_or(0);
        let credit: Vec<f64> = input
            .movements
            .iter()
            .map(|candidate| credit - tie * 0.5 * f64::from(candidate.routing_preference) / f64::from(latest + 1))
            .collect();
        let weight = values.iter().zip(&credit).map(|(value, credit)| value + credit).collect();
        Self {
            input,
            target_totals: input.target_opening.iter().map(|&(i, p, t, q)| ((i, p), (t, q))).collect(),
            utilisation: BTreeMap::new(),
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
            last_receipt_h: input.piles.iter().filter_map(|pile| Some((pile.id, pile.last_receipt_h?))).collect(),
            blocked: BTreeSet::new(),
            building: BTreeSet::new(),
            reclaimed: BTreeMap::new(),
            digs,
            reclaims,
            outlets: OutletIndex::new(input),
            shared,
            weight,
            credit,
            conditional: 0.0,
            rows: BTreeMap::new(),
            durations: BTreeMap::new(),
            chain: input.drill_blast.as_ref().map(Chain::new),
            now_h: 0.0,
            segmented: true,
            plan: None,
            followed: BTreeMap::new(),
            cancel,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn work(&mut self, interval: Interval) -> Result<(), String> {
        let input = self.input;
        let k = interval.index;
        // Readiness is judged on the state the interval opened with, as the
        // replay judges it.
        let opening_ground = self.ground.clone();
        // Drill and blast works the hour first, on the ground standing as it
        // opens; what fires is dug from the end of its window.
        self.now_h = interval.start_h;
        if let Some(chain) = self.chain.as_mut() {
            chain.advance(interval, |ground| opening_ground.get(&ground).is_some_and(|left| *left > FINISHED_T));
        }
        let first: Vec<Bar> = (0..input.loaders.len())
            .filter_map(|loader| {
                let rate = interval_rate(&input.loaders[loader], k)?;
                let task = authored_tasks(input, loader)
                    .into_iter()
                    .find(|&task| self.ready(task, interval, rate.dig_tph, rate.reclaim_tph, &opening_ground))?;
                self.open_bar(loader, task, &BTreeSet::new(), &opening_ground)
            })
            .collect();
        let mut topology = Topology {
            segments: vec![first],
            pins: Vec::new(),
            spent: Vec::new(),
        };

        // A pile that may not build and reclaim at once gives the hour to
        // the reclaim bar drawing it. One that the bar then leaves untouched
        // is left to building instead, so the hour is not lost to nothing.
        self.blocked = self.exclusive_draws(&topology, interval);
        self.building.clear();
        let plan = self.solve(interval, &topology)?;
        let plan = self.reopen_idle(interval, &topology, plan)?;
        let mut plan = self.settle(interval, &mut topology, plan, &opening_ground)?;

        // While a loader has finished its bar with time left in the
        // interval, give it its next bar in a segment of its own and solve
        // the whole interval again. The answer before stays feasible with
        // the new segment empty, so one is only kept when it is worth more,
        // to the program and in money.
        let budget = if self.segmented { input.segments_per_interval.max(1) } else { 1 };
        let mut tried: BTreeSet<(usize, usize, usize)> = BTreeSet::new();
        while topology.segments.len() < budget && !self.cancelled() {
            let Some(mut trial) = self.transition(interval, &topology, &plan, &opening_ground, &mut tried) else {
                break;
            };
            // What the interval already decided for an exclusive pile
            // stands. One a new bar approves and nothing decided yet goes to
            // building if the answer so far delivers there, else to the bar.
            let (blocked, building) = (self.blocked.clone(), self.building.clone());
            for pile in self.exclusive_draws(&trial, interval) {
                if blocked.contains(&pile) || building.contains(&pile) {
                    continue;
                }
                if self.delivered(&plan, pile) > NEGLIGIBLE_T {
                    self.building.insert(pile);
                } else {
                    self.blocked.insert(pile);
                }
            }
            let found = self
                .solve(interval, &trial)
                .and_then(|found| self.reopen_idle(interval, &trial, found))
                .and_then(|found| self.settle(interval, &mut trial, found, &opening_ground));
            match found {
                Ok(found)
                    if found.objective > plan.objective + IMPROVEMENT * plan.objective.abs().max(1.0) && found.money >= plan.money - MONEY_SLACK * plan.money.abs().max(1.0) =>
                {
                    topology = trial;
                    plan = found;
                }
                _ => (self.blocked, self.building) = (blocked, building),
            }
        }
        self.apply(interval, &topology, plan);
        Ok(())
    }

    /// Leave to building the blocked piles `plan` does not draw, and solve
    /// again if there were any.
    fn reopen_idle(&mut self, interval: Interval, topology: &Topology, plan: Plan) -> Result<Plan, String> {
        let input = self.input;
        let idle: Vec<StockpileId> = self
            .blocked
            .iter()
            .copied()
            .filter(|pile| {
                plan.rows
                    .iter()
                    .filter(|(index, _, _)| input.movements[*index].source == SourceId::Stockpile(*pile))
                    .map(|(_, _, tonnes)| tonnes)
                    .sum::<f64>()
                    <= NEGLIGIBLE_T
            })
            .collect();
        if idle.is_empty() {
            return Ok(plan);
        }
        self.blocked.retain(|pile| !idle.contains(pile));
        self.building.extend(idle);
        self.solve(interval, topology)
    }

    /// What `plan` delivers to `pile`.
    fn delivered(&self, plan: &Plan, pile: StockpileId) -> f64 {
        let input = self.input;
        plan.rows
            .iter()
            .filter(|(index, _, _)| {
                input
                    .destinations
                    .iter()
                    .any(|entry| entry.id == input.movements[*index].destination && entry.kind == DestinationKind::Stockpile(pile))
            })
            .map(|(_, _, tonnes)| tonnes)
            .sum()
    }

    /// A loader's bar as it opens in a segment, or `None` when it has no
    /// work there: a delay, or a dig bar held by a block not yet blasted.
    fn open_bar(&self, loader: usize, task: usize, done: &BTreeSet<GroundId>, opening_ground: &BTreeMap<GroundId, f64>) -> Option<Bar> {
        match &self.input.tasks[task].kind {
            TaskKind::Dig { sequence } => Some(Bar {
                loader,
                task,
                blocks: self.next_block(loader, sequence, None, done, opening_ground).into_iter().collect(),
                pile: None,
                emptied: Vec::new(),
            }),
            TaskKind::Reclaim { .. } => Some(Bar {
                loader,
                task,
                blocks: Vec::new(),
                pile: None,
                emptied: Vec::new(),
            }),
            // The loader's highest-priority bar is a delay: it stands.
            TaskKind::Delay => None,
        }
    }

    /// Exclusive piles the reclaim bars of `topology` draw this interval.
    fn exclusive_draws(&self, topology: &Topology, interval: Interval) -> BTreeSet<StockpileId> {
        let input = self.input;
        topology
            .segments
            .iter()
            .flatten()
            .filter_map(|bar| match &input.tasks[bar.task].kind {
                TaskKind::Reclaim { approved_sources, .. } => Some(approved_sources),
                _ => None,
            })
            .flatten()
            .copied()
            .filter(|pile| {
                input.piles.iter().any(|entry| entry.id == *pile && entry.exclusive)
                    && self.reclaims(*pile, interval)
                    && self.released(*pile, interval).is_some_and(|released| released.tonnes > NEGLIGIBLE_T)
            })
            .collect()
    }

    /// Settle a solved topology: a reclaim bar the program let draw on
    /// several piles in one segment keeps the one it drew most from, and
    /// while a loader finishes the last block of the last segment with rate
    /// to spare, open its next one and solve again with the finished block
    /// held finished. Each re-solve can move draws, so the two alternate
    /// until neither changes anything; a narrowed bar stays narrowed, so
    /// this ends. The answer before growing stays feasible, so no growth is
    /// worth less.
    fn settle(&self, interval: Interval, topology: &mut Topology, mut plan: Plan, opening_ground: &BTreeMap<GroundId, f64>) -> Result<Plan, String> {
        let input = self.input;
        let last = topology.segments.len() - 1;
        let done = topology.done();
        loop {
            if self.cancelled() {
                return Ok(plan);
            }
            if narrow(input, topology, &plan) {
                plan = self.solve(interval, topology)?;
                continue;
            }
            let mut grew = false;
            let duration = plan.durations.get(last).copied().unwrap_or(0.0);
            for bar in &mut topology.segments[last] {
                let TaskKind::Dig { sequence } = &input.tasks[bar.task].kind else { continue };
                let Some(&end) = bar.blocks.last() else { continue };
                let left = self.ground.get(&end).copied().unwrap_or(0.0);
                let rate = interval_rate(&input.loaders[bar.loader], interval.index).map_or(0.0, |rate| rate.dig_tph);
                let used: f64 = bar.blocks.iter().map(|block| plan.extracted.get(&(bar.loader, *block, last)).copied().unwrap_or(0.0)).sum();
                let finished = plan.dug(bar.loader, end) >= left - SNAP_T;
                if !finished || rate * duration - used <= NEGLIGIBLE_T || self.shared.get(&(bar.loader, end)).copied().unwrap_or(false) {
                    continue;
                }
                if let Some(next) = self.next_block(bar.loader, sequence, Some(end), &done, opening_ground) {
                    bar.blocks.push(next);
                    grew = true;
                }
            }
            if !grew {
                return Ok(plan);
            }
            plan = self.solve(interval, topology)?;
        }
    }

    /// The next topology to try: a loader whose bar in the last segment has
    /// no work left by the interval's end moves to its next bar in a new
    /// segment, or a reclaim bar that emptied its pile moves to another it
    /// approves. `None` when no loader has one not yet `tried`.
    ///
    /// The bar that follows must be the loader's highest-priority ready one
    /// at the new segment's start whatever the program then does, so the
    /// bars between are passed over only when they surely have no work: a
    /// dig bar whose remaining blocks the program must finish first, a
    /// reclaim bar whose cap or piles were empty as the interval opened.
    fn transition(
        &self,
        interval: Interval,
        topology: &Topology,
        plan: &Plan,
        opening_ground: &BTreeMap<GroundId, f64>,
        tried: &mut BTreeSet<(usize, usize, usize)>,
    ) -> Option<Topology> {
        let input = self.input;
        let last = topology.segments.len() - 1;
        let done = topology.done();
        // Blocks a loader works this interval: what they hold is the
        // program's to decide, so a bar holding one may still have work.
        let worked: BTreeSet<GroundId> = topology.segments.iter().flatten().flat_map(|bar| bar.blocks.iter().copied()).collect();
        for bar in &topology.segments[last] {
            if !tried.insert((bar.loader, bar.task, last)) {
                continue;
            }
            let Some(rate) = interval_rate(&input.loaders[bar.loader], interval.index) else {
                continue;
            };
            let mut next = topology.clone();
            next.segments.push(
                topology.segments[last]
                    .iter()
                    .filter(|other| other.loader != bar.loader)
                    .map(|other| Bar {
                        blocks: other.blocks.last().copied().into_iter().collect(),
                        ..other.clone()
                    })
                    .collect(),
            );
            match &input.tasks[bar.task].kind {
                TaskKind::Dig { sequence } => {
                    // Every block of the sequence gone, by any loader.
                    let remaining: Vec<GroundId> = sequence
                        .iter()
                        .copied()
                        .filter(|block| !done.contains(block) && self.ground.get(block).is_some_and(|left| *left > FINISHED_T))
                        .collect();
                    if remaining.iter().any(|block| plan.dug_by_all(*block) < self.ground[block] - SNAP_T) {
                        continue;
                    }
                    next.pins.extend(remaining.iter().map(|block| (*block, last)));
                }
                TaskKind::Reclaim { maximum_t, approved_sources } => {
                    let drawn: f64 = plan
                        .rows
                        .iter()
                        .filter(|(index, at, _)| *at <= last && self.under(bar, *index, topology, *at))
                        .map(|(_, _, tonnes)| tonnes)
                        .sum();
                    let left = maximum_t.map(|maximum| maximum - self.reclaimed.get(&bar.task).copied().unwrap_or(0.0));
                    if left.is_some_and(|left| drawn >= left - SNAP_T) {
                        next.spent.push((bar.task, last));
                    } else {
                        // Another pile of the same bar, once the one it drew is empty.
                        let Some(pile) = bar.pile.or_else(|| self.only_pile(bar, plan, last)) else { continue };
                        let pile_drawn: f64 = plan
                            .rows
                            .iter()
                            .filter(|(index, _, _)| input.movements[*index].source == SourceId::Stockpile(pile))
                            .map(|(_, _, tonnes)| tonnes)
                            .sum();
                        let released = self.released(pile, interval).map_or(0.0, |found| found.tonnes);
                        let mut emptied = bar.emptied.clone();
                        emptied.push(pile);
                        let others = approved_sources.iter().any(|other| {
                            !emptied.contains(other)
                                && !self.building.contains(other)
                                && self.reclaims(*other, interval)
                                && self.released(*other, interval).is_some_and(|found| found.tonnes > NEGLIGIBLE_T)
                        });
                        if pile_drawn < released - SNAP_T || !others {
                            continue;
                        }
                        next.segments[last + 1].push(Bar {
                            pile: None,
                            emptied,
                            ..bar.clone()
                        });
                        return Some(next);
                    }
                }
                TaskKind::Delay => continue,
            }
            // The loader's next bar, read as the new segment opens.
            let tasks = authored_tasks(input, bar.loader);
            let position = tasks.iter().position(|task| *task == bar.task)?;
            let gone = next.done();
            let mut following = None;
            for &task in &tasks[position + 1..] {
                match self.has_work(task, interval, rate.dig_tph, rate.reclaim_tph, &gone, &worked) {
                    Some(true) => {
                        following = Some(task);
                        break;
                    }
                    Some(false) => {}
                    // A bar that may or may not have work: no safe successor.
                    None => break,
                }
            }
            let Some(task) = following else { continue };
            let Some(opened) = self.open_bar(bar.loader, task, &gone, opening_ground) else {
                continue;
            };
            if matches!(input.tasks[task].kind, TaskKind::Dig { .. }) && opened.blocks.is_empty() {
                continue;
            }
            next.segments[last + 1].push(opened);
            return Some(next);
        }
        None
    }

    /// Whether `task` has work as a segment opens once the blocks in `gone`
    /// are dug: `Some` when that holds whatever the interval's program does
    /// with the blocks in `worked`, `None` when it depends on it.
    fn has_work(&self, task: usize, interval: Interval, dig_tph: f64, reclaim_tph: f64, gone: &BTreeSet<GroundId>, worked: &BTreeSet<GroundId>) -> Option<bool> {
        let entry = &self.input.tasks[task];
        if !task_active(entry, interval) {
            return Some(false);
        }
        match &entry.kind {
            TaskKind::Dig { sequence } => {
                if dig_tph <= 0.0 {
                    return Some(false);
                }
                let left: Vec<GroundId> = sequence
                    .iter()
                    .copied()
                    .filter(|block| !gone.contains(block) && self.ground.get(block).is_some_and(|left| *left > FINISHED_T))
                    .collect();
                if left.is_empty() || !self.diggable(entry, left[0], interval) {
                    Some(false)
                } else if left.iter().any(|block| !worked.contains(block) && self.ground[block] > SURELY_HELD_T) {
                    Some(true)
                } else {
                    None
                }
            }
            // Read on the interval's opening, and only a cap spent before it.
            TaskKind::Reclaim { .. } => Some(self.ready(task, interval, dig_tph, reclaim_tph, &self.ground)),
            TaskKind::Delay => Some(true),
        }
    }

    /// Whether a movement row in `segment` was worked under `bar`'s task.
    fn under(&self, bar: &Bar, index: usize, topology: &Topology, segment: usize) -> bool {
        let candidate = &self.input.movements[index];
        input_loader(self.input, candidate.loader) == Some(bar.loader) && topology.segments[segment].iter().any(|other| other.loader == bar.loader && other.task == bar.task)
    }

    /// The one pile `bar` drew in `segment`, if it drew exactly one.
    fn only_pile(&self, bar: &Bar, plan: &Plan, segment: usize) -> Option<StockpileId> {
        let piles: BTreeSet<StockpileId> = plan
            .rows
            .iter()
            .filter(|(index, at, tonnes)| *at == segment && *tonnes > NEGLIGIBLE_T && input_loader(self.input, self.input.movements[*index].loader) == Some(bar.loader))
            .filter_map(|(index, _, _)| match self.input.movements[*index].source {
                SourceId::Stockpile(pile) => Some(pile),
                SourceId::Ground(_) => None,
            })
            .collect();
        (piles.len() == 1).then(|| *piles.first().expect("one pile"))
    }

    /// The first block of `sequence` after `after` with tonnes left that
    /// authored order lets `loader` start, `after` itself counting as
    /// finished in this interval, and the blocks in `done` gone before the
    /// segment opens.
    fn next_block(&self, loader: usize, sequence: &[GroundId], after: Option<GroundId>, done: &BTreeSet<GroundId>, opening_ground: &BTreeMap<GroundId, f64>) -> Option<GroundId> {
        let start = after.map_or(0, |after| sequence.iter().position(|block| *block == after).map_or(sequence.len(), |position| position + 1));
        let mut previous = after;
        for &block in &sequence[start..] {
            // A block the input no longer holds was finished before it.
            let Some(&left) = self.ground.get(&block).filter(|_| !done.contains(&block)) else {
                previous = Some(block);
                continue;
            };
            if let Some(earlier) = previous.filter(|earlier| Some(*earlier) != after && !done.contains(earlier)) {
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
                // A block not yet blasted holds the bar: the loader waits for
                // it rather than skipping ahead.
                if self.chain.as_ref().is_some_and(|chain| !chain.available(block, self.now_h)) {
                    return None;
                }
                return Some(block);
            }
        }
        None
    }

    /// The interval's linear program over `topology`, solved.
    ///
    /// Each segment has its own length, the lengths filling the interval.
    /// Within a segment a loader moves at most its rate for that length, and
    /// each truck class works its share of the interval's hours. Ground,
    /// crushers, dumps and soft targets are shared by the whole interval; a
    /// pile's room by the end of each segment, net of what segments before it
    /// reclaimed.
    /// Solve one interval, its utilisation incentive priced. Should the
    /// incentive's band columns leave the program unsolvable, the interval is
    /// solved without them rather than failing the run: the incentive steers
    /// the schedule, and is never a reason not to have one.
    fn solve(&self, interval: Interval, topology: &Topology) -> Result<Plan, String> {
        match self.solve_priced(interval, topology, true) {
            Err(error) if !self.cancelled() && self.has_incentive(interval) => {
                let plan = self.solve_priced(interval, topology, false).map_err(|_| error.clone())?;
                log::warn!("interval {}: solved without the utilisation incentive, which left it unsolvable: {error}", interval.index);
                Ok(plan)
            }
            result => result,
        }
    }

    fn has_incentive(&self, interval: Interval) -> bool {
        self.plan.is_some()
            || self
                .input
                .loaders
                .iter()
                .any(|loader| interval_rate(loader, interval.index).is_some_and(|rate| rate.utilisation_incentive > 0.0 && rate.dig_tph > 0.0))
    }

    fn solve_priced(&self, interval: Interval, topology: &Topology, priced: bool) -> Result<Plan, String> {
        if self.cancelled() {
            return Err(format!("interval {}: cancelled", interval.index));
        }
        let input = self.input;
        let duration = interval.duration_h();
        let segments = topology.segments.len();
        let mut problem = LinearProgram::default();
        let lengths: Vec<Col> = (0..segments).map(|_| problem.add_column(0.0, 0.0..)).collect();
        problem.add_row(duration..=duration, lengths.iter().map(|col| (*col, 1.0)).collect());
        // (candidate, segment) and its column.
        let mut columns: Vec<(usize, usize, Col)> = Vec::new();
        let mut extractions: Vec<((usize, GroundId, usize), Col)> = Vec::new();
        let mut block_total: BTreeMap<GroundId, Vec<(Col, f64)>> = BTreeMap::new();
        let mut pile_draw: BTreeMap<StockpileId, Vec<(Col, f64)>> = BTreeMap::new();
        let mut caps: BTreeMap<usize, Vec<(usize, Col)>> = BTreeMap::new();
        let mut pins = topology.pins.clone();
        for (segment, bars) in topology.segments.iter().enumerate() {
            for bar in bars {
                let task = &input.tasks[bar.task];
                let Some(rate) = interval_rate(&input.loaders[bar.loader], interval.index) else {
                    continue;
                };
                let mut loader_total = Vec::new();
                let tph = match &task.kind {
                    TaskKind::Dig { .. } => {
                        for (position, &block) in bar.blocks.iter().enumerate() {
                            let left = self.ground.get(&block).copied().unwrap_or(0.0);
                            let Some(source) = input.ground.iter().find(|source| source.id == block) else {
                                continue;
                            };
                            if position + 1 < bar.blocks.len() {
                                pins.push((block, segment));
                            }
                            let extract = problem.add_column(0.0, 0.0..=left);
                            extractions.push(((bar.loader, block, segment), extract));
                            loader_total.push((extract, 1.0));
                            block_total.entry(block).or_default().push((extract, 1.0));
                            for share in &source.material {
                                let mut portion = vec![(extract, -share.fraction)];
                                for &index in self.digs.get(&(bar.loader, block, share.material.0)).into_iter().flatten() {
                                    if task_authorises(task, &input.movements[index]) {
                                        let col = problem.add_column(self.weight[index], 0.0..);
                                        columns.push((index, segment, col));
                                        portion.push((col, 1.0));
                                    }
                                }
                                problem.add_row(0.0..=0.0, portion);
                            }
                        }
                        rate.dig_tph
                    }
                    TaskKind::Reclaim { approved_sources, .. } => {
                        for pile in approved_sources
                            .iter()
                            .filter(|pile| bar.pile.is_none_or(|only| only == **pile) && !bar.emptied.contains(pile) && !self.building.contains(pile))
                        {
                            let Some(released) = self.released(*pile, interval).filter(|found| self.reclaims(*pile, interval) && found.tonnes > NEGLIGIBLE_T) else {
                                continue;
                            };
                            for index in self.reclaim_candidates(bar.loader, *pile, bar.task, interval) {
                                let value = conditional_value(input, index, &released.blend, released.tonnes);
                                let col = problem.add_column(self.weight[index] + value, 0.0..);
                                columns.push((index, segment, col));
                                loader_total.push((col, 1.0));
                                pile_draw.entry(*pile).or_default().push((col, 1.0));
                                caps.entry(bar.task).or_default().push((segment, col));
                            }
                        }
                        rate.reclaim_tph
                    }
                    TaskKind::Delay => 0.0,
                };
                loader_total.push((lengths[segment], -tph));
                problem.add_row(..=0.0, loader_total);
            }
        }
        for (block, terms) in block_total {
            problem.add_row(..=self.ground.get(&block).copied().unwrap_or(0.0), terms);
        }
        // A block pinned to a segment is gone by its end.
        for (block, through) in pins {
            let left = self.ground.get(&block).copied().unwrap_or(0.0);
            let terms: Vec<(Col, f64)> = extractions
                .iter()
                .filter(|((_, at, segment), _)| *at == block && *segment <= through)
                .map(|(_, col)| (*col, 1.0))
                .collect();
            if terms.is_empty() {
                return Err(format!("interval {}: block {} must be finished but no loader digs it", interval.index, block.0));
            }
            problem.add_row(left..=left, terms);
        }
        for (pile, terms) in pile_draw {
            problem.add_row(..=self.released(pile, interval).map_or(0.0, |released| released.tonnes), terms);
        }
        for (task, draws) in caps {
            let TaskKind::Reclaim { maximum_t: Some(maximum), .. } = input.tasks[task].kind else {
                continue;
            };
            let left = (maximum - self.reclaimed.get(&task).copied().unwrap_or(0.0)).max(0.0);
            problem.add_row(..=left, draws.iter().map(|(_, col)| (*col, 1.0)).collect());
            for &(_, through) in topology.spent.iter().filter(|(spent, _)| *spent == task) {
                problem.add_row(left..=left, draws.iter().filter(|(segment, _)| *segment <= through).map(|(_, col)| (*col, 1.0)).collect());
            }
        }

        // Shared room: truck hours per class and segment, each segment
        // getting its length's share, and each destination's over the
        // interval.
        let mut trucks: BTreeMap<(TruckClassId, usize), Vec<(Col, f64)>> = BTreeMap::new();
        let mut destinations: BTreeMap<DestinationId, Vec<(Col, f64)>> = BTreeMap::new();
        // Per pile, (segment, column, +1 a receipt or -1 a reclaim).
        let mut occupancy: BTreeMap<StockpileId, Vec<(usize, Col, f64)>> = BTreeMap::new();
        for &(index, segment, col) in &columns {
            let candidate = &input.movements[index];
            if candidate.truck_hours_per_tonne > 0.0 {
                trucks.entry((candidate.truck, segment)).or_default().push((col, candidate.truck_hours_per_tonne));
            }
            destinations.entry(candidate.destination).or_default().push((col, 1.0));
            if let Some(DestinationKind::Stockpile(pile)) = input.destinations.iter().find(|entry| entry.id == candidate.destination).map(|entry| entry.kind) {
                occupancy.entry(pile).or_default().push((segment, col, 1.0));
            }
            if let (Activity::Reclaim, SourceId::Stockpile(pile)) = (candidate.activity, candidate.source) {
                occupancy.entry(pile).or_default().push((segment, col, -1.0));
            }
        }
        for ((truck, segment), mut terms) in trucks {
            if let Some(hours) = input.trucks.iter().find(|entry| entry.id == truck).and_then(|entry| entry.hours.get(interval.index)) {
                if duration > 0.0 {
                    terms.push((lengths[segment], -hours / duration));
                    problem.add_row(..=0.0, terms);
                } else {
                    problem.add_row(..=*hours, terms);
                }
            }
        }
        for (destination, terms) in destinations {
            let room = self.destination_room(destination, interval);
            if room.is_finite() {
                problem.add_row(..=room, terms);
            }
        }
        // A pile's receipts up to the end of each segment fit on top of its
        // opening stock less what the segments before reclaimed. A segment's
        // own reclaim frees nothing for its receipts: a loader working blocks
        // back to back delivers unevenly, so the pile can peak inside it.
        for (pile, flows) in occupancy {
            let Some(entry) = input.piles.iter().find(|entry| entry.id == pile) else { continue };
            let room = entry.capacity_t - self.piles.get(&pile).map_or(0.0, |(tonnes, _)| *tonnes);
            for segment in (0..segments).filter(|segment| flows.iter().any(|(at, _, sign)| at == segment && *sign > 0.0)) {
                let terms: Vec<(Col, f64)> = flows
                    .iter()
                    .filter(|(at, _, sign)| *at < segment || (*at == segment && *sign > 0.0))
                    .map(|(_, col, sign)| (*col, *sign))
                    .collect();
                problem.add_row(..=room.max(0.0), terms);
            }
        }

        // The utilisation incentive: each loader's dug hours climb through
        // bands of its scheduled utilisation so far today, from where it
        // stands up to its target, each band paying the incentive for every
        // point it lies below the target. The rate falls band by band, so the
        // program fills the lower, better-paid bands first by itself - a
        // concave reward, exact as a linear program. Never money.
        let mut incentive: Vec<(Col, f64)> = Vec::new();
        for (loader, entry) in input.loaders.iter().enumerate().filter(|_| priced) {
            let Some(rate) = interval_rate(entry, interval.index).filter(|rate| rate.utilisation_incentive > 0.0 && rate.dig_tph > 0.0) else {
                continue;
            };
            let dug: Vec<(Col, f64)> = extractions
                .iter()
                .filter(|((by, _, _), _)| *by == loader)
                .map(|(_, col)| (*col, -1.0 / rate.dig_tph))
                .collect();
            if dug.is_empty() || duration <= 0.0 {
                continue;
            }
            let (worked, had) = self.utilisation.get(&(loader, interval.day())).copied().unwrap_or_default();
            let available = had + duration;
            let lower = worked / available;
            // Bands this interval can reach: it adds at most its own length.
            let reach = lower + duration / available + super::utilisation::BAND;
            let mut band_hours = Vec::new();
            for (hours, per_hour) in super::utilisation::bands(lower, reach, rate.utilisation_target, available, rate.utilisation_incentive, rate.dig_tph) {
                let col = problem.add_column(per_hour, 0.0..=hours);
                incentive.push((col, per_hour));
                band_hours.push((col, 1.0));
            }
            if band_hours.is_empty() {
                continue;
            }
            band_hours.extend(dug);
            problem.add_row(..=0.0, band_hours);
        }

        // A plan's targets: each loader's dug tonnes to each destination,
        // paced through the day. Never money.
        if let Some(plan) = self.plan.filter(|_| priced) {
            let day = interval.day();
            let paced = plan.paced(interval);
            let mut flows: BTreeMap<(usize, DestinationId), Vec<Col>> = BTreeMap::new();
            for &(index, _, col) in &columns {
                let candidate = &input.movements[index];
                if candidate.activity == Activity::Dig
                    && let Some(loader) = input_loader(input, candidate.loader)
                {
                    flows.entry((loader, candidate.destination)).or_default().push(col);
                }
            }
            for ((loader, destination), cols) in flows {
                let planned = plan.routes.get(&(loader, destination, day)).copied().unwrap_or(0.0);
                let due = (planned * paced - self.followed.get(&(loader, destination, day)).copied().unwrap_or(0.0)).max(0.0);
                let mut over: Vec<(Col, f64)> = cols.iter().map(|col| (*col, 1.0)).collect();
                if due > NEGLIGIBLE_T && plan.follow > 0.0 {
                    let kept = problem.add_column(plan.follow, 0.0..=due);
                    incentive.push((kept, plan.follow));
                    let mut row = vec![(kept, 1.0)];
                    row.extend(cols.iter().map(|col| (*col, -1.0)));
                    problem.add_row(..=0.0, row);
                    over.push((kept, -1.0));
                }
                if plan.overrun > 0.0 {
                    let past = problem.add_column(-plan.overrun, 0.0..);
                    incentive.push((past, -plan.overrun));
                    over.push((past, -1.0));
                    problem.add_row(..=0.0, over);
                }
            }
        }

        if columns.is_empty() && extractions.is_empty() {
            return Ok(Plan {
                durations: (0..segments).map(|segment| if segment == 0 { duration } else { 0.0 }).collect(),
                ..Plan::default()
            });
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
                for &(candidate, _, column) in &columns {
                    let movement = &input.movements[candidate];
                    if movement.destination != target.destination {
                        continue;
                    }
                    let fraction = match movement.source {
                        SourceId::Stockpile(pile) if movement.activity == Activity::Reclaim => self.released(pile, interval).and_then(|r| r.blend.get(target.grade).copied()),
                        _ => input.grades.fraction(movement.material, target.grade),
                    }
                    .unwrap_or(0.0);
                    terms.push((column, direction * (fraction - boundary)));
                }
                problem.add_row(..=-direction * (opening_q - boundary * opening_t), terms);
            }
        }
        let solution = problem.maximise().map_err(|reason| format!("interval {}: {reason}", interval.index))?;
        let rows: Vec<(usize, usize, f64)> = columns.into_iter().map(|(index, segment, col)| (index, segment, solution[col.index()].max(0.0))).collect();
        let objective = problem.value(&solution);
        let steered: f64 = incentive.iter().map(|(col, per_hour)| per_hour * solution[col.index()].max(0.0)).sum();
        Ok(Plan {
            money: objective - steered - rows.iter().map(|(index, _, tonnes)| self.credit[*index] * tonnes).sum::<f64>(),
            rows,
            extracted: extractions.into_iter().map(|(key, col)| (key, solution[col.index()].max(0.0))).collect(),
            durations: lengths.iter().map(|col| solution[col.index()].max(0.0)).collect(),
            objective,
        })
    }

    /// Record an interval's answer, made exact: segment lengths fill the
    /// interval, a block pinned to a segment is dug no later, an extraction
    /// within [`SNAP_T`] of finishing its block finishes it, and each
    /// material's rows are scaled to that material's share of the
    /// extraction.
    fn apply(&mut self, interval: Interval, topology: &Topology, plan: Plan) {
        let input = self.input;
        let k = interval.index;
        let duration = interval.duration_h();
        let filled: f64 = plan.durations.iter().sum();
        for segment in 0..input.segments_per_interval.max(1) {
            let length = match plan.durations.get(segment) {
                Some(length) if filled > 0.0 => length * duration / filled,
                _ => {
                    if segment == 0 {
                        duration
                    } else {
                        0.0
                    }
                }
            };
            self.durations.insert((k, segment), length);
        }
        let mut pins = topology.pins.clone();
        for (segment, bars) in topology.segments.iter().enumerate() {
            for bar in bars {
                pins.extend(bar.blocks.iter().rev().skip(1).map(|block| (*block, segment)));
            }
        }
        let mut extracted = plan.extracted;
        for ((_, block, segment), tonnes) in &mut extracted {
            if pins.iter().any(|(pinned, through)| pinned == block && segment > through) {
                *tonnes = 0.0;
            }
        }
        let mut totals: BTreeMap<(usize, GroundId), f64> = BTreeMap::new();
        for ((loader, block, _), tonnes) in &extracted {
            *totals.entry((*loader, *block)).or_default() += tonnes;
        }
        for ((loader, block, _), tonnes) in &mut extracted {
            let total = totals[&(*loader, *block)];
            let left = self.ground.get(block).copied().unwrap_or(0.0);
            let exact = if total >= left - SNAP_T {
                left
            } else if total < NEGLIGIBLE_T {
                0.0
            } else {
                total
            };
            *tonnes = if total > 0.0 { *tonnes * exact / total } else { 0.0 };
        }
        // Dig rows by (loader, block, material, segment), for the rescale.
        let mut portions: BTreeMap<PortionKey, Vec<(usize, f64)>> = BTreeMap::new();
        let mut rows = Vec::new();
        for (index, segment, tonnes) in plan.rows {
            let candidate = &input.movements[index];
            match candidate.source {
                SourceId::Ground(block) => {
                    let loader = input_loader(input, candidate.loader).expect("a dig candidate's loader");
                    portions.entry((loader, block, candidate.material.0, segment)).or_default().push((index, tonnes));
                }
                _ if tonnes > NEGLIGIBLE_T => rows.push((index, segment, tonnes)),
                _ => {}
            }
        }
        for ((loader, block, material, segment), entries) in portions {
            let fraction = input
                .ground
                .iter()
                .find(|source| source.id == block)
                .and_then(|source| source.material.iter().find(|share| share.material.0 == material))
                .map_or(0.0, |share| share.fraction);
            let target = fraction * extracted.get(&(loader, block, segment)).copied().unwrap_or(0.0);
            let total: f64 = entries.iter().map(|(_, tonnes)| tonnes).sum();
            if target <= 0.0 || total <= 0.0 {
                continue;
            }
            rows.extend(
                entries
                    .into_iter()
                    .map(|(index, tonnes)| (index, segment, tonnes * target / total))
                    .filter(|(_, _, tonnes)| *tonnes > FINISHED_T),
            );
        }
        for ((_, block, _), tonnes) in extracted {
            if let Some(left) = self.ground.get_mut(&block) {
                *left = (*left - tonnes).max(0.0);
            }
        }
        // Earlier segments first, so reclaim is charged to bars in the order
        // they worked.
        rows.sort_by_key(|(index, segment, _)| (*segment, *index));

        // A draw within [`SNAP_T`] of emptying what its pile released
        // empties it, so a drained chunk reads as empty to the order rules.
        let mut released: BTreeMap<StockpileId, Released> = BTreeMap::new();
        for pile in &input.piles {
            if let Some(found) = self.released(pile.id, interval) {
                released.insert(pile.id, found);
            }
        }
        for (pile, found) in &released {
            let drawn: f64 = rows
                .iter()
                .filter(|(index, _, _)| input.movements[*index].source == SourceId::Stockpile(*pile))
                .map(|(_, _, tonnes)| tonnes)
                .sum();
            if drawn > 0.0 && drawn >= found.tonnes - SNAP_T && drawn != found.tonnes {
                let scale = found.tonnes / drawn;
                for (index, _, tonnes) in &mut rows {
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
        // Conditional values are judged on what each pile gave up in all,
        // as the replay judges them.
        let mut pile_drawn: BTreeMap<StockpileId, f64> = BTreeMap::new();
        for (index, _, tonnes) in &rows {
            if let SourceId::Stockpile(pile) = input.movements[*index].source {
                *pile_drawn.entry(pile).or_default() += tonnes;
            }
        }
        // The day so far's utilisation, for the incentive: every loader with
        // productive time this interval had it, and worked the hours its
        // tonnes took at its effective rates - reclaiming included, as busy.
        for (loader, entry) in input.loaders.iter().enumerate() {
            let Some(rate) = interval_rate(entry, k).filter(|rate| rate.dig_tph > 0.0 || rate.reclaim_tph > 0.0) else {
                continue;
            };
            let worked: f64 = rows
                .iter()
                .filter(|(index, _, _)| input.movements[*index].loader == entry.id)
                .map(|(index, _, tonnes)| {
                    let per_hour = match input.movements[*index].activity {
                        Activity::Dig => rate.dig_tph,
                        Activity::Reclaim => rate.reclaim_tph,
                    };
                    if per_hour > 0.0 { tonnes / per_hour } else { 0.0 }
                })
                .sum();
            let day = self.utilisation.entry((loader, interval.day())).or_default();
            day.0 += worked;
            day.1 += duration;
        }
        if self.plan.is_some() {
            for (index, _, tonnes) in &rows {
                let candidate = &input.movements[*index];
                if candidate.activity == Activity::Dig
                    && let Some(loader) = input_loader(input, candidate.loader)
                {
                    *self.followed.entry((loader, candidate.destination, interval.day())).or_default() += tonnes;
                }
            }
        }
        for (index, segment, tonnes) in rows {
            let candidate = &input.movements[index];
            *self.rows.entry((index, k, segment)).or_default() += tonnes;
            let blend = match candidate.source {
                SourceId::Stockpile(pile) => {
                    *drawn.entry(pile).or_default() += tonnes;
                    for (task, share) in attribute_reclaim(input, candidate, interval, tonnes, &self.reclaimed) {
                        *self.reclaimed.entry(task).or_default() += share;
                    }
                    let blend = released.get(&pile).map(|found| found.blend.clone()).unwrap_or_else(|| vec![0.0; grades]);
                    self.conditional += tonnes * conditional_value(input, index, &blend, pile_drawn.get(&pile).copied().unwrap_or(tonnes));
                    blend
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
        for (pile, (received_t, _)) in &received {
            if *received_t > REST_RECEIPT_T {
                self.last_receipt_h.insert(*pile, interval.end_h);
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
                    // Only a live chunk is published: the one receiving, and
                    // any holding material. One not yet reached, or emptied
                    // and closed, has nothing to say, and a pile may have
                    // many of those.
                    if Some(index) == receiving || chunk.held_t > FINISHED_T || draw > 0.0 {
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
                    }
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
                    if taken_t > REST_RECEIPT_T {
                        chunk.received_h = Some(interval.end_h);
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
    fn released(&self, pile: StockpileId, interval: Interval) -> Option<Released> {
        let blend = |tonnes: f64, contained: &[f64]| contained.iter().map(|value| if tonnes > 0.0 { value / tonnes } else { 0.0 }).collect();
        let entry = self.input.piles.iter().find(|entry| entry.id == pile)?;
        let Some(chunks) = self.chunks.get(&pile) else {
            if !self.rested(pile, interval) {
                return None;
            }
            let (open_t, open_q) = self.piles.get(&pile)?;
            return Some(Released {
                chunk: None,
                tonnes: *open_t,
                blend: blend(*open_t, open_q),
            });
        };
        // The chunk next in order, once its last delivery has rested; while
        // it rests the pile releases nothing.
        let next = match entry.order {
            ReclaimOrder::Fifo => chunks.iter().position(|chunk| chunk.held_t > FINISHED_T)?,
            ReclaimOrder::Lifo => chunks.iter().rposition(|chunk| chunk.held_t > FINISHED_T)?,
        };
        let index = Some(next).filter(|&index| entry.chunk_rested(index, chunks[index].received_h, interval))?;
        let chunk = &chunks[index];
        Some(Released {
            chunk: Some(index),
            tonnes: chunk.held_t,
            blend: blend(chunk.held_t, &chunk.held_q),
        })
    }

    /// The replay's readiness, from the state the interval opened with.
    fn ready(&self, bar: usize, interval: Interval, dig_tph: f64, reclaim_tph: f64, opening_ground: &BTreeMap<GroundId, f64>) -> bool {
        let task = &self.input.tasks[bar];
        if !task_active(task, interval) {
            return false;
        }
        match &task.kind {
            // Its current block, and somewhere with room for all of it.
            TaskKind::Dig { sequence } => {
                dig_tph > 0.0
                    && sequence
                        .iter()
                        .find(|block| opening_ground.get(block).is_some_and(|left| *left > FINISHED_T))
                        .is_some_and(|current| self.diggable(task, *current, interval))
            }
            TaskKind::Reclaim { approved_sources, maximum_t } => {
                reclaim_tph > 0.0
                    && maximum_t.is_none_or(|maximum| self.reclaimed.get(&bar).copied().unwrap_or(0.0) < maximum - NEGLIGIBLE_T)
                    && approved_sources.iter().any(|pile| {
                        // A chunked pile has work only in a chunk it releases:
                        // what sits in the chunk still filling cannot be drawn.
                        self.reclaims(*pile, interval)
                            && if self.chunks.contains_key(pile) {
                                self.released(*pile, interval).is_some_and(|found| found.tonnes > NEGLIGIBLE_T)
                            } else {
                                self.piles.get(pile).is_some_and(|(open_t, _)| *open_t > NEGLIGIBLE_T)
                            }
                    })
            }
            TaskKind::Delay => true,
        }
    }

    /// Whether `task` could dig `block` as `interval` opens: every material
    /// of it has a destination with room; see [`OutletIndex::of`].
    fn diggable(&self, task: &super::super::Task, block: GroundId, interval: Interval) -> bool {
        block_diggable(&self.outlets.of(self.input, task, block), |destination| self.room_at_opening(destination, interval))
    }

    /// Whether `destination` has room as `interval` opens, for a dig bar's
    /// readiness: building, and more than [`DIG_ROOM_T`] short of what it can
    /// take.
    fn room_at_opening(&self, destination: DestinationId, interval: Interval) -> bool {
        let input = self.input;
        let Some(entry) = input.destinations.iter().find(|entry| entry.id == destination) else {
            return false;
        };
        match entry.kind {
            DestinationKind::Dump => entry
                .capacity_t
                .is_none_or(|capacity| capacity - self.dumped.get(&destination).copied().unwrap_or(0.0) > DIG_ROOM_T),
            DestinationKind::Crusher => match entry.crusher_daily_t.get(interval.day() as usize).copied().flatten() {
                Some(budget) => budget - self.crushed.get(&(destination, interval.day())).copied().unwrap_or(0.0) > DIG_ROOM_T,
                None => true,
            },
            DestinationKind::Stockpile(pile) => {
                input.piles.iter().any(|entry| entry.id == pile && entry.builds(interval))
                    && entry
                        .capacity_t
                        .is_none_or(|capacity| capacity - self.piles.get(&pile).map_or(0.0, |(open_t, _)| *open_t) > DIG_ROOM_T)
            }
        }
    }

    /// Whether `pile` may be reclaimed in `interval`: its authored mode
    /// allows it and, unchunked, its newest material has rested. A chunked
    /// pile's rest is its chunks' own, judged where a chunk is released.
    fn reclaims(&self, pile: StockpileId, interval: Interval) -> bool {
        self.input.piles.iter().find(|entry| entry.id == pile).is_some_and(|entry| entry.reclaims(interval)) && self.rested(pile, interval)
    }

    /// Whether an unchunked pile's newest material has rested by `interval`.
    fn rested(&self, pile: StockpileId, interval: Interval) -> bool {
        let Some(entry) = self.input.piles.iter().find(|entry| entry.id == pile) else {
            return false;
        };
        !entry.chunks.is_empty() || self.last_receipt_h.get(&pile).is_none_or(|received_h| entry.rested(*received_h, interval))
    }

    /// Reclaim candidates for one loader and pile under `bar`, admitted on
    /// the blend the pile releases with the formulation's margin to spare.
    fn reclaim_candidates(&self, loader: usize, pile: StockpileId, bar: usize, interval: Interval) -> Vec<usize> {
        let input = self.input;
        let Some(Released { tonnes, blend, .. }) = self.released(pile, interval) else {
            return Vec::new();
        };
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
            // The pile's own room is held segment by segment in `solve`; a
            // chunked pile takes no more than its receiving chunk's room as
            // the interval opened, which reclaim from other chunks does not
            // free.
            DestinationKind::Stockpile(pile) => {
                if !input.piles.iter().any(|entry| entry.id == pile && entry.builds(interval)) || self.blocked.contains(&pile) {
                    return 0.0;
                }
                match self.chunks.get(&pile) {
                    Some(chunks) => chunks.iter().find(|chunk| !chunk.closed).map_or(0.0, |chunk| chunk.capacity_t - chunk.held_t),
                    None => f64::INFINITY,
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
            .map(|((candidate, interval, segment), tonnes_t)| MovementRow {
                candidate,
                interval,
                segment,
                tonnes_t,
            })
            .collect();
        let reported_objective: f64 = movements
            .iter()
            .map(|row| row.tonnes_t * input.movements[row.candidate].value_per_tonne().unwrap_or(0.0))
            .sum();
        // The incentive measured as the replay measures it - over whole days -
        // so the report reconciles; the dispatch's own day-so-far pricing only
        // steered which rows these are.
        let incentive = super::utilisation::value(input, &movements);
        BlendSolution {
            movements,
            durations: self.durations,
            chunks: self.chunk_rows,
            reported_objective: reported_objective + self.conditional - super::replay::target_penalty(input, &self.target_totals) + incentive,
            adjustments: ExtractionAdjustments::default(),
            drill_blast: self.chain.map(Chain::finish),
        }
    }
}

/// Narrow every reclaim bar that drew on several piles in one segment to
/// the one it drew most from, since a loader reclaims one pile at a time.
/// Whether any was.
fn narrow(input: &BlendInput, topology: &mut Topology, plan: &Plan) -> bool {
    let mut narrowed = false;
    for (segment, bars) in topology.segments.iter_mut().enumerate() {
        for bar in bars.iter_mut().filter(|bar| matches!(input.tasks[bar.task].kind, TaskKind::Reclaim { .. })) {
            let mut drawn: BTreeMap<StockpileId, f64> = BTreeMap::new();
            for &(index, at, tonnes) in &plan.rows {
                let candidate = &input.movements[index];
                if at == segment
                    && candidate.activity == Activity::Reclaim
                    && input.loaders[bar.loader].id == candidate.loader
                    && let SourceId::Stockpile(pile) = candidate.source
                    && tonnes > NEGLIGIBLE_T
                {
                    *drawn.entry(pile).or_default() += tonnes;
                }
            }
            if drawn.len() > 1 {
                bar.pile = drawn.into_iter().max_by(|left, right| left.1.total_cmp(&right.1)).map(|(pile, _)| pile);
                narrowed = true;
            }
        }
    }
    narrowed
}

/// A dig row's (loader, block, material, segment).
type PortionKey = (usize, GroundId, u32, usize);

/// The position of loader `id` in the input.
fn input_loader(input: &BlendInput, id: LoaderId) -> Option<usize> {
    input.loaders.iter().position(|loader| loader.id == id)
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
                received_h: None,
            }
        })
        .collect()
}

/// The grade-conditional value per tonne a reclaim on candidate `index`
/// earns from `blend`, `tonnes` of it leaving the pile, priced as the
/// formulation prices it: a reward only clear inside its bounds, a cost
/// unless clear outside them. Never above the published value, so a
/// schedule's reported objective stays one the model would grant it.
fn conditional_value(input: &BlendInput, index: usize, blend: &[f64], tonnes: f64) -> f64 {
    input
        .conditional_values
        .iter()
        .filter(|entry| entry.candidate == index)
        .filter(|entry| {
            let (holds, near) = (entry.holds(blend), entry.near_boundary(blend, tonnes));
            if entry.value_per_tonne >= 0.0 { holds && !near } else { holds || near }
        })
        .map(|entry| entry.value_per_tonne)
        .sum()
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
