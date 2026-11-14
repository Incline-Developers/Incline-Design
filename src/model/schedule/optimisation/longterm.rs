//! The order automatic bars dig their ground in, and which bar digs what.
//!
//! The ground the schedule sequences for itself is planned over the whole
//! horizon at once, in periods: which period each block is best dug in, given
//! that a block cannot go before the ground above it, that the loaders move
//! only so many tonnes a period and that a crusher takes only so many. What
//! is wanted is the discounted value of the material moved.
//!
//! The capacities are not rows a solver has to satisfy block by block. Each
//! is given a price, and a block dug in a period pays for the capacity it
//! uses there. With prices fixed, the best plan is a maximum closure of the
//! graph whose nodes are "block dug by the end of period" - a block by a
//! period needs it by every later period and needs the ground above it by
//! the same period - which `mineflow` solves in one pass however many
//! blocks there are. A small linear program over the closures found so far
//! (a convex combination of them, within the capacities) sets the next
//! prices, and the loop ends when no closure beats the combination at its
//! own prices: Lagrangian relaxation, with the closures as its columns.
//!
//! The combination is fractional, so it is read as an order rather than a
//! schedule: blocks by the period they are expected in. That order respects
//! every dependency, since each closure does. It is then dealt to the bars,
//! and the hourly dispatch works out what actually happens - this plan
//! decides only the sequence the dispatch is handed.

use std::collections::{BTreeMap, BTreeSet};

use super::{DestinationId, DestinationKind, GroundId, LoaderId, SourceId, TaskId, TaskKind, blended::input::BlendInput};

/// The most periods the plan works in. A longer horizon's periods are merged
/// evenly: the plan sets an order, and an order does not need finer steps.
const MAX_PERIODS: usize = 48;
/// The most closures priced before the order is taken from what was found.
const MAX_ROUNDS: usize = 200;
/// How far each round's prices lean towards the best found so far.
const SMOOTHING: f64 = 0.8;
/// A closure must beat the combination by this share of its value to be
/// worth another round.
const IMPROVEMENT: f64 = 1e-6;
/// The weight of a node no closure may hold: far past anything the scaled
/// values sum to, and far inside what the closure solver adds up exactly.
const FORBIDDEN: i64 = -(1 << 56);
/// Hours in a year, for discounting.
const YEAR_H: f64 = 8760.0;

/// A material's best movement from a block: its value a tonne, where it
/// goes, and the truck class and hours a tonne its haul takes.
type Best = (f64, DestinationId, Option<usize>, f64);

/// What a plan is asked for.
pub(crate) struct PlanRequest<'a> {
    /// The automatic bars' tasks. Each lists every block any of them may
    /// dig, in an order that respects the dependencies.
    pub(crate) tasks: &'a [TaskId],
    /// When each period ends, in hours from the start, ascending.
    pub(crate) period_ends_h: &'a [f64],
    /// The yearly rate value is discounted at, as a fraction.
    pub(crate) discount_rate: f64,
    /// Ground that goes to one bar together: a blast's blocks on one level.
    pub(crate) groups: &'a BTreeMap<GroundId, u32>,
    /// The hour before which a block is not the automatic bars' to dig:
    /// ground a dig bar names, until that bar's window closes.
    pub(crate) not_before_h: &'a BTreeMap<GroundId, f64>,
    /// Ground to keep ahead of a block in the order though nothing in the
    /// chain holds the block to it.
    pub(crate) also_above: &'a BTreeMap<GroundId, Vec<GroundId>>,
    /// Authored dependencies: ground that waits, and the ground it waits
    /// for. Not yet in the chain, which would hold them block by block.
    pub(crate) dependencies: &'a [(Vec<GroundId>, Vec<GroundId>)],
    /// The chain's tasks automatic bars on machines work, by position: one
    /// blast each, in no order yet - the plan's is what will order them.
    pub(crate) machine_tasks: &'a [usize],
    /// How long the pricing may run.
    pub(crate) budget: std::time::Duration,
}

/// What the plan did, for the run's report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PlanSummary {
    pub(crate) blocks: usize,
    pub(crate) periods: usize,
    /// Closures priced; zero when there was nothing to choose between.
    pub(crate) rounds: usize,
    pub(crate) seconds: f64,
    /// The plan's discounted value, and the most any plan could reach at
    /// the last prices.
    pub(crate) value: f64,
    pub(crate) bound: f64,
}

/// One block as the plan sees it, or a blast's clearance: a step with no
/// ground of its own that the blast's ground waits on, and that waits on
/// the ground above the blast and on the blasts ahead of it in a machine
/// bar.
struct Block {
    /// `None` for a clearance.
    ground: Option<GroundId>,
    tonnes: f64,
    /// Undiscounted value of digging it, each material to its best
    /// destination.
    value: f64,
    /// Tonnes of it whose best destination is each crusher.
    crushed: Vec<(usize, f64)>,
    /// Truck hours its haul to those destinations takes, by truck class.
    hauled: Vec<(usize, f64)>,
    /// Its share of its blast's prep, drilling and charging, where machines
    /// working on automatic bars do them: square metres, metres, tonnes.
    worked: [f64; 3],
    /// Blocks that must be dug before it, by position.
    above: Vec<usize>,
    /// Hours its blast takes to prep, drill, charge and fire once it is
    /// clear, with every machine that could work it on it at once: the
    /// least time between the ground above going and this being ready.
    lead_h: f64,
    /// The hour before which its blast cannot have been finished, however
    /// soon it clears: a machine bar works its blasts one after another,
    /// so each waits its turn behind the work on those ahead of it.
    earliest_h: f64,
}

/// Give every automatic bar its blocks and their order: the plan's when
/// there is value to plan for, the order they came in otherwise. Candidates
/// for a block on a loader that was not given it are dropped.
pub(crate) fn assign(input: &mut BlendInput, request: &PlanRequest<'_>, planned: bool, cancelled: &dyn Fn() -> bool) -> PlanSummary {
    let Some(first) = input.tasks.iter().find(|task| request.tasks.contains(&task.id)) else {
        return PlanSummary::default();
    };
    let TaskKind::Dig { sequence } = &first.kind else { return PlanSummary::default() };
    let grounds: Vec<GroundId> = sequence.clone();
    let blocks = blocks(input, request, &grounds);
    let (order, summary) = match if planned { planned_order(input, request, &blocks, cancelled) } else { None } {
        Some((order, summary)) => (order, summary),
        None => (
            (0..blocks.len()).collect(),
            PlanSummary {
                blocks: blocks.len(),
                ..PlanSummary::default()
            },
        ),
    };
    deal(input, request, &blocks, &order);
    summary
}

/// The blocks, in the order the first automatic bar lists them, with what
/// each is worth and what must go before it.
fn blocks(input: &BlendInput, request: &PlanRequest<'_>, grounds: &[GroundId]) -> Vec<Block> {
    let position: BTreeMap<GroundId, usize> = grounds.iter().enumerate().map(|(index, ground)| (*ground, index)).collect();
    let loaders: BTreeSet<LoaderId> = input.tasks.iter().filter(|task| request.tasks.contains(&task.id)).map(|task| task.loader).collect();
    let crushers: Vec<DestinationId> = input
        .destinations
        .iter()
        .filter(|destination| destination.kind == DestinationKind::Crusher)
        .map(|destination| destination.id)
        .collect();
    // Each block's materials at their best: the highest value a tonne of it
    // can fetch from any automatic loader, where that sends it, and the
    // truck class and hours a tonne of it takes to get there - of two
    // equally good, the one the trucks spend less on.
    let mut best: BTreeMap<(GroundId, super::MaterialId), Best> = BTreeMap::new();
    for movement in &input.movements {
        let SourceId::Ground(ground) = movement.source else { continue };
        if !position.contains_key(&ground) || !loaders.contains(&movement.loader) {
            continue;
        }
        let value = movement.value_per_tonne().unwrap_or(0.0);
        let truck = input.trucks.iter().position(|class| class.id == movement.truck);
        let hours = if movement.truck_hours_per_tonne.is_finite() {
            movement.truck_hours_per_tonne.max(0.0)
        } else {
            0.0
        };
        let candidate = (value, movement.destination, truck, hours);
        best.entry((ground, movement.material))
            .and_modify(|held| {
                if value > held.0 + 1e-9 || (value >= held.0 - 1e-9 && hours < held.3) {
                    *held = candidate;
                }
            })
            .or_insert(candidate);
    }
    let sources: BTreeMap<GroundId, &super::GroundSource> = input.ground.iter().map(|source| (source.id, source)).collect();
    // Ground no machine will ever release - a step of its blast that no
    // bar works - is never there to dig, and nor is anything under it.
    let lead: BTreeMap<GroundId, f64> = input
        .drill_blast
        .as_ref()
        .map_or_else(BTreeMap::new, |chain| chain.earliest_releases(&input.intervals).into_iter().collect());
    let never_h = super::blended::drill_blast::NEVER;
    let mut out: Vec<Block> = grounds
        .iter()
        .map(|ground| {
            let source = sources.get(ground);
            let tonnes = source.map_or(0.0, |source| source.tonnes_t);
            let mut value = 0.0;
            let mut crushed: Vec<(usize, f64)> = Vec::new();
            let mut hauled: Vec<(usize, f64)> = Vec::new();
            for share in source.into_iter().flat_map(|source| source.material.iter()) {
                let Some((per_tonne, destination, truck, hours)) = best.get(&(*ground, share.material)) else {
                    continue;
                };
                let share_t = share.fraction * tonnes;
                value += per_tonne * share_t;
                if let Some(truck) = truck {
                    match hauled.iter_mut().find(|(held, _)| held == truck) {
                        Some((_, held)) => *held += hours * share_t,
                        None => hauled.push((*truck, hours * share_t)),
                    }
                }
                if let Some(crusher) = crushers.iter().position(|id| id == destination) {
                    match crushed.iter_mut().find(|(held, _)| *held == crusher) {
                        Some((_, held)) => *held += share_t,
                        None => crushed.push((crusher, share_t)),
                    }
                }
            }
            Block {
                ground: Some(*ground),
                tonnes,
                value,
                crushed,
                hauled,
                worked: [0.0; 3],
                above: Vec::new(),
                lead_h: lead.get(ground).copied().filter(|at| *at < never_h).unwrap_or(0.0),
                earliest_h: if lead.get(ground).is_some_and(|at| *at >= never_h) {
                    f64::INFINITY
                } else {
                    request.not_before_h.get(ground).copied().unwrap_or(0.0)
                },
            }
        })
        .collect();
    for (index, ground) in grounds.iter().enumerate() {
        if let Some(over) = request.also_above.get(ground) {
            out[index].above.extend(over.iter().filter_map(|ground| position.get(ground).copied()));
        }
    }
    // What holds each block. A dependency capture put in the chain holds
    // its block until the ground above it is dug. A blast's ground waits on
    // the blast's clearance, which waits on the ground above the blast -
    // and, because a machine bar works its blasts strictly in order, on the
    // clearance of every blast ahead of it in a bar: a plan that ignored
    // that would send a loader to ground no drill can reach yet.
    // The clearance each block's blast waits on, and that blast.
    let mut cleared_by: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    if let Some(chain) = &input.drill_blast {
        let mut clearance: BTreeMap<usize, usize> = BTreeMap::new();
        for (index, job) in chain.blasts.iter().enumerate() {
            let over: Vec<usize> = job.above.iter().filter_map(|ground| position.get(ground).copied()).collect();
            let held: Vec<usize> = job.releases.iter().filter_map(|ground| position.get(ground).copied()).collect();
            if job.gate {
                for block in held {
                    out[block].above.extend(over.iter().copied());
                }
                continue;
            }
            if job.stage > crate::model::schedule::BlastStage::NotStarted {
                continue;
            }
            let step = out.len();
            clearance.insert(index, step);
            for block in held {
                out[block].above.push(step);
                cleared_by.insert(block, (step, index));
            }
            out.push(Block {
                ground: None,
                tonnes: 0.0,
                value: 0.0,
                crushed: Vec::new(),
                hauled: Vec::new(),
                worked: [0.0; 3],
                above: over,
                lead_h: 0.0,
                earliest_h: 0.0,
            });
        }
        for (_, task) in chain
            .tasks
            .iter()
            .enumerate()
            .filter(|(index, task)| !task.delay && task.follow.is_none() && !request.machine_tasks.contains(index))
        {
            // Only where this is the one bar of its step holding the blast:
            // held by several, it is worked from whichever reaches it first.
            let step = chain.agents.get(task.agent).map(|agent| agent.activity);
            let only_here = |blast: usize| {
                chain
                    .tasks
                    .iter()
                    .filter(|other| !other.delay && other.follow.is_none() && chain.agents.get(other.agent).map(|agent| agent.activity) == step)
                    .filter(|other| other.sequence.contains(&blast))
                    .count()
                    == 1
            };
            for pair in task.sequence.windows(2).filter(|pair| only_here(pair[1])) {
                if let (Some(ahead), Some(behind)) = (clearance.get(&pair[0]), clearance.get(&pair[1])) {
                    out[*behind].above.push(*ahead);
                }
            }
            // The bar's own step on each blast in turn, from when the bar
            // opens, at the rate of every machine that could be on it.
            let Some(activity) = chain.agents.get(task.agent).map(|agent| agent.activity) else {
                continue;
            };
            let mut at = task.start_h;
            for blast in &task.sequence {
                let Some(job) = chain.blasts.get(*blast) else { continue };
                if !job.stage.has_done(activity) {
                    at = chain.earliest_done(*blast, activity, job.quantity[activity as usize], at, &input.intervals);
                }
                if at >= super::blended::drill_blast::NEVER {
                    break;
                }
                for ground in &job.releases {
                    if let Some(block) = position.get(ground) {
                        out[*block].earliest_h = out[*block].earliest_h.max(at);
                    }
                }
            }
        }
    }
    // Automatic machines' blasts: each step still to do on one, shared out
    // over its ground by tonnes. Their tasks hold one blast each, so they
    // add no order between blasts, and no machine pace: what limits them is
    // their machines' hours, priced like the loaders'.
    if let Some(chain) = &input.drill_blast {
        let mut counted: BTreeSet<(usize, usize)> = BTreeSet::new();
        for task in request.machine_tasks.iter().filter_map(|index| chain.tasks.get(*index)) {
            let (Some(blast), Some(agent)) = (task.sequence.first(), chain.agents.get(task.agent)) else {
                continue;
            };
            let step = agent.activity as usize;
            let Some(job) = chain.blasts.get(*blast) else { continue };
            if job.stage.has_done(agent.activity) || !counted.insert((step, *blast)) {
                continue;
            }
            let held: Vec<usize> = job.releases.iter().filter_map(|ground| position.get(ground).copied()).collect();
            let tonnes: f64 = held.iter().map(|block| out[*block].tonnes).sum();
            if tonnes <= 0.0 {
                continue;
            }
            for block in held {
                out[block].worked[step] += job.quantity[step] * out[block].tonnes / tonnes;
            }
        }
    }
    // Each dependency is one step that waits on the ground it names, which
    // the waiting ground waits on - through its blast's clearance where it
    // has one, as the chain will hold it - rather than an arc from every
    // block to every other.
    for (waiting, first) in request.dependencies {
        let over: Vec<usize> = first.iter().filter_map(|ground| position.get(ground).copied()).collect();
        if over.is_empty() {
            continue;
        }
        let step = out.len();
        out.push(Block {
            ground: None,
            tonnes: 0.0,
            value: 0.0,
            crushed: Vec::new(),
            hauled: Vec::new(),
            worked: [0.0; 3],
            above: over,
            lead_h: 0.0,
            earliest_h: 0.0,
        });
        let waited: BTreeSet<GroundId> = first.iter().copied().collect();
        let releases_waited = |job: usize| {
            input
                .drill_blast
                .as_ref()
                .and_then(|chain| chain.blasts.get(job))
                .is_some_and(|job| job.releases.iter().any(|ground| waited.contains(ground)))
        };
        let mut clearances: BTreeSet<usize> = BTreeSet::new();
        for block in waiting.iter().filter_map(|ground| position.get(ground).copied()) {
            match cleared_by.get(&block) {
                Some((clearance, job)) => {
                    if !releases_waited(*job) {
                        clearances.insert(*clearance);
                    }
                }
                None => out[block].above.push(step),
            }
        }
        for clearance in clearances {
            out[clearance].above.push(step);
        }
    }
    for block in &mut out {
        block.above.sort_unstable();
        block.above.dedup();
    }
    out
}

/// The plan's periods: the request's, merged evenly down to
/// [`MAX_PERIODS`], as `(start_h, end_h)`.
fn periods(request: &PlanRequest<'_>) -> Vec<(f64, f64)> {
    let ends: Vec<f64> = request.period_ends_h.iter().copied().filter(|end| end.is_finite() && *end > 0.0).collect();
    let step = ends.len().div_ceil(MAX_PERIODS).max(1);
    let mut out = Vec::new();
    let mut start = 0.0;
    for chunk in ends.chunks(step) {
        let end = chunk[chunk.len() - 1];
        if end > start {
            out.push((start, end));
            start = end;
        }
    }
    out
}

/// What machines on automatic bars can prep, drill and charge in each
/// period, by step then period: each machine's rate over the hours one of
/// its automatic bars is open. `None` for a step no such machine does.
fn machine_capacity(input: &BlendInput, request: &PlanRequest<'_>, periods: &[(f64, f64)]) -> [Vec<Option<f64>>; 3] {
    let mut out: [Vec<Option<f64>>; 3] = std::array::from_fn(|_| vec![None; periods.len()]);
    let Some(chain) = &input.drill_blast else { return out };
    let agents: BTreeSet<usize> = request.machine_tasks.iter().filter_map(|index| chain.tasks.get(*index)).map(|task| task.agent).collect();
    for agent in agents {
        let Some(machine) = chain.agents.get(agent) else { continue };
        let step = machine.activity as usize;
        let windows: Vec<(f64, f64)> = request
            .machine_tasks
            .iter()
            .filter_map(|index| chain.tasks.get(*index))
            .filter(|task| task.agent == agent)
            .map(|task| (task.start_h, task.end_h))
            .collect();
        for (interval, rate) in input.intervals.iter().zip(&machine.rates) {
            let middle = 0.5 * (interval.start_h + interval.end_h);
            if !windows.iter().any(|(start, end)| *start <= middle && middle < *end) {
                continue;
            }
            if let Some(period) = periods.iter().position(|(start, end)| *start <= middle && middle < *end) {
                *out[step][period].get_or_insert(0.0) += rate.max(0.0) * (interval.end_h - interval.start_h);
            }
        }
    }
    for by_period in &mut out {
        if by_period.iter().any(Option::is_some) {
            for limit in by_period.iter_mut() {
                limit.get_or_insert(0.0);
            }
        }
    }
    out
}

/// Hours each truck class has in each period, by class then period.
/// Hours the authored bars' hauls take are not taken off: those bars mostly
/// run early, before the automatic ones open.
fn truck_capacity(input: &BlendInput, periods: &[(f64, f64)]) -> Vec<Vec<f64>> {
    input
        .trucks
        .iter()
        .map(|class| {
            let mut hours = vec![0.0; periods.len()];
            for (interval, available) in input.intervals.iter().zip(&class.hours) {
                let middle = 0.5 * (interval.start_h + interval.end_h);
                if let Some(period) = periods.iter().position(|(start, end)| *start <= middle && middle < *end) {
                    hours[period] += available;
                }
            }
            hours
        })
        .collect()
}

/// Tonnes the automatic bars' loaders can dig in each period: each loader's
/// rate over the hours one of its automatic bars is open, less the hours
/// its dig bars need for the ground they name and the hours it is delayed.
fn loader_capacity(input: &BlendInput, request: &PlanRequest<'_>, periods: &[(f64, f64)]) -> Vec<f64> {
    let tonnes: BTreeMap<GroundId, f64> = input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect();
    let mut capacity = vec![0.0; periods.len()];
    let loaders: BTreeSet<LoaderId> = input.tasks.iter().filter(|task| request.tasks.contains(&task.id)).map(|task| task.loader).collect();
    for loader in loaders {
        let Some(machine) = input.loaders.iter().find(|entry| entry.id == loader) else {
            continue;
        };
        let rates: BTreeMap<usize, f64> = machine.rates.iter().map(|rate| (rate.interval, rate.dig_tph)).collect();
        // What the loader can dig in each interval, in tonnes.
        let mut free: Vec<f64> = input
            .intervals
            .iter()
            .map(|interval| rates.get(&interval.index).copied().unwrap_or(0.0).max(0.0) * (interval.end_h - interval.start_h))
            .collect();
        let open = |task: &super::Task, index: usize| {
            let interval = input.intervals[index];
            task.window_start_h < interval.end_h - 1e-9 && task.window_end_h > interval.start_h + 1e-9
        };
        for task in input.tasks.iter().filter(|task| task.loader == loader && !request.tasks.contains(&task.id)) {
            match &task.kind {
                // A dig bar takes the loader from its start until its ground
                // is gone.
                TaskKind::Dig { sequence } => {
                    let mut owed: f64 = sequence.iter().filter_map(|ground| tonnes.get(ground)).sum();
                    for (index, left) in free.iter_mut().enumerate() {
                        if owed <= 0.0 {
                            break;
                        }
                        if open(task, index) {
                            let taken = left.min(owed);
                            *left -= taken;
                            owed -= taken;
                        }
                    }
                }
                TaskKind::Delay => {
                    for (index, left) in free.iter_mut().enumerate() {
                        if open(task, index) {
                            *left = 0.0;
                        }
                    }
                }
                TaskKind::Reclaim { .. } => {}
            }
        }
        let automatic: Vec<&super::Task> = input.tasks.iter().filter(|task| task.loader == loader && request.tasks.contains(&task.id)).collect();
        for (index, interval) in input.intervals.iter().enumerate() {
            if !automatic.iter().any(|task| open(task, index)) {
                continue;
            }
            let middle = 0.5 * (interval.start_h + interval.end_h);
            if let Some(period) = periods.iter().position(|(start, end)| *start <= middle && middle < *end) {
                capacity[period] += free[index];
            }
        }
    }
    capacity
}

/// Tonnes each crusher takes in each period, `None` where a day of it has
/// no limit.
fn crusher_capacity(input: &BlendInput, periods: &[(f64, f64)]) -> Vec<Vec<Option<f64>>> {
    input
        .destinations
        .iter()
        .filter(|destination| destination.kind == DestinationKind::Crusher)
        .map(|destination| {
            periods
                .iter()
                .map(|(start, end)| {
                    let first = (start / 24.0).floor().max(0.0) as usize;
                    let last = (end / 24.0).ceil().max(0.0) as usize;
                    (first..last).map(|day| destination.crusher_daily_t.get(day).copied().flatten()).sum::<Option<f64>>()
                })
                .collect()
        })
        .collect()
}

/// A closure: the period each block is dug in, or the period count for never.
type Column = Vec<u16>;

/// The blocks in the order the plan would dig them, or `None` when there is
/// nothing to plan: no value on any block, a single period, or a solver
/// that could not be run.
fn planned_order(input: &BlendInput, request: &PlanRequest<'_>, blocks: &[Block], cancelled: &dyn Fn() -> bool) -> Option<(Vec<usize>, PlanSummary)> {
    // A place for each block after everything it waits on.
    let rank = topological_rank(blocks);
    let periods = periods(request);
    let (count, spans) = (blocks.len(), periods.len());
    if count < 2 || spans < 2 || !blocks.iter().any(|block| block.value.abs() > 1e-9) {
        return None;
    }
    let discount: Vec<f64> = periods
        .iter()
        .map(|(start, end)| (1.0 + request.discount_rate.max(0.0)).powf(-0.5 * (start + end) / YEAR_H))
        .collect();
    // The side rows: loader tonnes per period, then each limited crusher
    // period. `uses(block, period)` is what a block dug then takes of each.
    let dig = loader_capacity(input, request, &periods);
    let crush = crusher_capacity(input, &periods);
    let mut limits: Vec<f64> = dig.clone();
    let mut crush_row: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (crusher, by_period) in crush.iter().enumerate() {
        for (period, limit) in by_period.iter().enumerate() {
            if let Some(limit) = limit {
                crush_row.insert((crusher, period), limits.len());
                limits.push(*limit);
            }
        }
    }
    // What the automatic bars' machines can prep, drill and charge in each
    // period.
    let mut machine_row: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (step, by_period) in machine_capacity(input, request, &periods).into_iter().enumerate() {
        for (period, limit) in by_period.into_iter().enumerate() {
            if let Some(limit) = limit {
                machine_row.insert((step, period), limits.len());
                limits.push(limit);
            }
        }
    }
    // Each truck class's hours in each period, for the classes with a fleet.
    let mut haul_row: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (class, hours) in truck_capacity(input, &periods).into_iter().enumerate() {
        for (period, limit) in hours.into_iter().enumerate() {
            if limit.is_finite() && limit >= 0.0 {
                haul_row.insert((class, period), limits.len());
                limits.push(limit);
            }
        }
    }
    let usage = |column: &Column| {
        let mut used = vec![0.0; limits.len()];
        for (block, period) in blocks.iter().zip(column) {
            let period = usize::from(*period);
            if period >= spans {
                continue;
            }
            used[period] += block.tonnes;
            for (crusher, tonnes) in &block.crushed {
                if let Some(row) = crush_row.get(&(*crusher, period)) {
                    used[*row] += tonnes;
                }
            }
            for (truck, hours) in &block.hauled {
                if let Some(row) = haul_row.get(&(*truck, period)) {
                    used[*row] += hours;
                }
            }
            for (step, amount) in block.worked.iter().enumerate() {
                if let Some(row) = machine_row.get(&(step, period)).filter(|_| *amount > 0.0) {
                    used[*row] += amount;
                }
            }
        }
        used
    };
    let worth = |column: &Column| -> f64 {
        blocks
            .iter()
            .zip(column)
            .filter(|(_, period)| usize::from(**period) < spans)
            .map(|(block, period)| block.value * discount[usize::from(*period)])
            .sum()
    };

    // The closure graph: node `block * spans + period` is "dug by the end of
    // the period". It needs the same block by the next period, and the
    // ground above it by this one.
    //
    // A block whose blast needs time once it is clear needs the ground
    // above that many periods sooner, and cannot be dug before that many
    // have passed: whole periods only, so a lead shorter than a period
    // costs the plan nothing.
    let node = |block: usize, period: usize| (block * spans + period) as i64;
    let span_h = periods.last().map_or(1.0, |(_, end)| end / spans as f64).max(1e-9);
    let whole = |hours: f64| ((hours / span_h).floor().max(0.0) as usize).min(spans - 1);
    let lag: Vec<usize> = blocks.iter().map(|block| whole(block.lead_h)).collect();
    // `spans` for a block that is never there: forbidden in every period.
    let opens: Vec<usize> = blocks
        .iter()
        .zip(&lag)
        .map(|(block, lag)| if block.earliest_h.is_finite() { whole(block.earliest_h).max(*lag) } else { spans })
        .collect();
    let mut edges: Vec<(i64, i64)> = Vec::with_capacity(count * spans * 2);
    for (index, block) in blocks.iter().enumerate() {
        for period in 0..spans {
            if period + 1 < spans {
                edges.push((node(index, period), node(index, period + 1)));
            }
            if period >= lag[index] {
                for over in &block.above {
                    edges.push((node(index, period), node(*over, period - lag[index])));
                }
            }
        }
    }
    let precedence = mineflow::Precedence::explicit((count * spans) as i64, &edges).ok()?;
    drop(edges);
    // Values go to the closure solver as integers: scaled so the largest is
    // well inside what it sums exactly.
    let largest = blocks.iter().map(|block| block.value.abs()).fold(0.0, f64::max).max(1.0);
    let scale = 1e12 / largest;
    let mut weights = vec![0_i64; count * spans];
    let mut solver = mineflow::Solver::new(&precedence, &weights).ok()?;

    let never = spans as u16;
    let mut columns: Vec<Column> = vec![vec![never; count]];
    let mut theta = vec![1.0];
    let mut prices = vec![0.0; limits.len()];
    // Prices are smoothed towards the best seen so far (Wentges): the
    // master's own prices swing from one round to the next, and closures
    // priced at them chase the swings rather than closing the gap.
    let mut centre: Option<(Vec<f64>, f64)> = None;
    let mut smoothing = SMOOTHING;
    let started = std::time::Instant::now();
    let (mut value, mut bound, mut rounds) = (0.0, f64::INFINITY, 0);
    while rounds < MAX_ROUNDS && started.elapsed() < request.budget {
        if cancelled() {
            return None;
        }
        let at: Vec<f64> = match &centre {
            Some((best, _)) => best.iter().zip(&prices).map(|(best, now)| smoothing * best + (1.0 - smoothing) * now).collect(),
            None => prices.clone(),
        };
        // The best closure at these prices. A block dug in a period earns
        // its discounted value less the capacity it pays for; as "dug by"
        // nodes that is the difference between one period and the next.
        let gain = |block: &Block, period: usize| {
            let mut gain = block.value * discount[period] - at[period] * block.tonnes;
            for (crusher, tonnes) in &block.crushed {
                if let Some(row) = crush_row.get(&(*crusher, period)) {
                    gain -= at[*row] * tonnes;
                }
            }
            for (truck, hours) in &block.hauled {
                if let Some(row) = haul_row.get(&(*truck, period)) {
                    gain -= at[*row] * hours;
                }
            }
            for (step, amount) in block.worked.iter().enumerate() {
                if let Some(row) = machine_row.get(&(step, period)).filter(|_| *amount > 0.0) {
                    gain -= at[*row] * amount;
                }
            }
            gain
        };
        for (index, block) in blocks.iter().enumerate() {
            for period in 0..spans {
                let here = gain(block, period);
                let next = if period + 1 < spans { gain(block, period + 1) } else { 0.0 };
                // Before its blast could be ready the block is not to be had
                // at any price.
                weights[index * spans + period] = if period < opens[index] { FORBIDDEN } else { ((here - next) * scale).round() as i64 };
            }
        }
        solver.update_values(&weights).ok()?;
        solver.solve_with_cancel(cancelled).ok()?;
        let pit = solver.pit().ok()?;
        let chosen = pit.as_slice();
        let column: Column = (0..count)
            .map(|index| (0..spans).find(|period| chosen[index * spans + period]).map_or(never, |period| period as u16))
            .collect();
        rounds += 1;
        // What this closure is worth at the prices, against what the
        // combination is: no closure beating it means the prices are right.
        let used = usage(&column);
        let priced = worth(&column) - used.iter().zip(&at).map(|(used, price)| used * price).sum::<f64>();
        let allowance: f64 = limits.iter().zip(&at).map(|(limit, price)| limit * price).sum();
        let lagrangian = priced + allowance;
        bound = bound.min(lagrangian);
        if centre.as_ref().is_none_or(|(_, best)| lagrangian < *best) {
            centre = Some((at.clone(), lagrangian));
        }
        if bound - value <= IMPROVEMENT * value.abs().max(1.0) {
            break;
        }
        if columns.contains(&column) {
            // Nothing new at the master's own prices: the combination is
            // the best there is. At smoothed ones it only means they were
            // smoothed too far, so the next round leans on the master more.
            if smoothing <= 0.0 {
                break;
            }
            smoothing = if smoothing < 0.1 { 0.0 } else { smoothing * 0.5 };
            continue;
        }
        columns.push(column);
        // The best combination of the closures so far, within the limits.
        let mut master = solver::Problem::new(solver::Sense::Maximise);
        let cols: Vec<usize> = columns.iter().map(|column| master.add_col(worth(column), 0.0, f64::INFINITY)).collect();
        let uses: Vec<Vec<f64>> = columns.iter().map(&usage).collect();
        for (row, limit) in limits.iter().enumerate() {
            master.add_row(
                f64::NEG_INFINITY,
                *limit,
                cols.iter().zip(&uses).filter(|(_, used)| used[row] != 0.0).map(|(col, used)| (*col, used[row])),
            );
        }
        master.add_row(1.0, 1.0, cols.iter().map(|col| (*col, 1.0)));
        let solution = solver::solve(&master, &solver::Options::default());
        if solution.status != solver::Status::Optimal {
            // The combination before this closure still stands: the order
            // is taken from it.
            columns.pop();
            break;
        }
        value = solution.objective;
        theta = solution.col_value;
        // A limit's price is what one more tonne of it would add.
        for (price, dual) in prices.iter_mut().zip(&solution.row_dual) {
            *price = dual.abs();
        }
    }
    // How much of each block the plan has dug by the end of each period.
    let mut by: Vec<Vec<f64>> = vec![vec![0.0; spans]; count];
    for (column, weight) in columns.iter().zip(&theta) {
        for (index, period) in column.iter().enumerate() {
            if *period < never {
                by[index][usize::from(*period)] += weight;
            }
        }
    }
    for row in &mut by {
        for period in 1..spans {
            row[period] += row[period - 1];
        }
    }
    // The plan is a mix of closures, each block dug in some of them, so no
    // one closure is the order. Each period is filled instead, to what the
    // loaders and crushers can take in it, with the blocks free to be dug
    // that the plan has most of dug by then: a period carries the ore the
    // plan meant for it as well as the ground over it, where an order by
    // when blocks are dug on average would put all the stripping first.
    let mut waiting: Vec<usize> = blocks.iter().map(|block| block.above.len()).collect();
    let mut below: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (index, block) in blocks.iter().enumerate() {
        for over in &block.above {
            below[*over].push(index);
        }
    }
    let key = |index: usize, period: usize| (by[index][period].to_bits(), std::cmp::Reverse(rank[index]), index);
    let mut placed = vec![false; count];
    let mut order: Vec<usize> = Vec::with_capacity(count);
    let mut ready: Vec<usize> = (0..count).filter(|index| waiting[*index] == 0).collect();
    for period in 0..spans {
        let mut room = dig[period];
        let mut crushing: BTreeMap<usize, f64> = crush_row.iter().filter(|((_, at), _)| *at == period).map(|(_, row)| (*row, limits[*row])).collect();
        let mut heap: std::collections::BinaryHeap<(u64, std::cmp::Reverse<usize>, usize)> =
            ready.drain(..).filter(|index| !placed[*index]).map(|index| key(index, period)).collect();
        let mut held: Vec<usize> = Vec::new();
        while let Some((_, _, index)) = heap.pop() {
            if placed[index] {
                continue;
            }
            let block = &blocks[index];
            let rows: Vec<usize> = block.crushed.iter().filter_map(|(crusher, _)| crush_row.get(&(*crusher, period)).copied()).collect();
            let full = (block.tonnes > 0.0 && room <= 0.0) || rows.iter().any(|row| crushing.get(row).is_some_and(|left| *left <= 0.0));
            if by[index][period] < 1e-6 || full {
                held.push(index);
                continue;
            }
            placed[index] = true;
            order.push(index);
            room -= block.tonnes;
            for (crusher, tonnes) in &block.crushed {
                if let Some(left) = crush_row.get(&(*crusher, period)).and_then(|row| crushing.get_mut(row)) {
                    *left -= tonnes;
                }
            }
            for under in &below[index] {
                waiting[*under] = waiting[*under].saturating_sub(1);
                if waiting[*under] == 0 {
                    heap.push(key(*under, period));
                }
            }
        }
        ready = held;
    }
    // What no period took follows, by when the plan digs it on average and
    // never before anything it waits on.
    let dug: Vec<f64> = (0..count).map(|index| by[index][spans - 1]).collect();
    let when: Vec<f64> = (0..count)
        .map(|index| {
            let total: f64 = columns
                .iter()
                .zip(&theta)
                .filter(|(column, _)| column[index] < never)
                .map(|(column, weight)| weight * f64::from(column[index]))
                .sum();
            if dug[index] > 1e-12 { total / dug[index] } else { f64::from(never) }
        })
        .collect();
    let mut by_rank: Vec<usize> = (0..count).collect();
    by_rank.sort_by_key(|index| rank[*index]);
    let mut expected = when;
    for index in by_rank {
        let floor = blocks[index].above.iter().map(|over| expected[*over]).fold(f64::NEG_INFINITY, f64::max);
        expected[index] = expected[index].max(floor);
    }
    let mut rest: Vec<usize> = (0..count).filter(|index| !placed[*index]).collect();
    rest.sort_by(|a, b| expected[*a].total_cmp(&expected[*b]).then(rank[*a].cmp(&rank[*b])));
    order.extend(rest);
    Some((
        order,
        PlanSummary {
            blocks: count,
            periods: spans,
            rounds,
            seconds: started.elapsed().as_secs_f64(),
            value,
            bound,
        },
    ))
}

/// Each block's place in an order that puts it after everything it waits
/// on, earlier-listed blocks first where they are free to be.
///
/// Blasts can be authored to wait on one another in a ring - a machine bar
/// listing a lower blast ahead of the one over it - and ground caught in one
/// is never reached. It is placed where the ring is met, earliest-listed
/// first, so everything outside the ring still follows what it waits on.
fn topological_rank(blocks: &[Block]) -> Vec<usize> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    let mut waiting: Vec<usize> = blocks.iter().map(|block| block.above.len()).collect();
    let mut below: Vec<Vec<usize>> = vec![Vec::new(); blocks.len()];
    for (index, block) in blocks.iter().enumerate() {
        for over in &block.above {
            below[*over].push(index);
        }
    }
    let mut ready: BinaryHeap<Reverse<usize>> = waiting.iter().enumerate().filter(|(_, count)| **count == 0).map(|(index, _)| Reverse(index)).collect();
    let mut rank = vec![usize::MAX; blocks.len()];
    let mut placed = 0;
    let mut unplaced = 0;
    while placed < blocks.len() {
        let index = match ready.pop() {
            Some(Reverse(index)) => index,
            None => {
                // A ring: take its earliest-listed member as though free.
                while rank[unplaced] != usize::MAX {
                    unplaced += 1;
                }
                unplaced
            }
        };
        if rank[index] != usize::MAX {
            continue;
        }
        rank[index] = placed;
        placed += 1;
        for under in &below[index] {
            waiting[*under] = waiting[*under].saturating_sub(1);
            if waiting[*under] == 0 && rank[*under] == usize::MAX {
                ready.push(Reverse(*under));
            }
        }
    }
    rank
}

/// Hand the blocks, in `order`, to the automatic bars: each to the bar that
/// could start it soonest, counting when that bar is free and when the
/// ground the block waits on would be finished. A bar that dug the last of
/// the block's group keeps it unless that costs more than the block's own
/// digging time, so a loader stays on a face rather than trading blocks
/// with another. Each bar then lists only its own blocks, and candidates
/// for a block on another bar's loader are dropped.
fn deal(input: &mut BlendInput, request: &PlanRequest<'_>, blocks: &[Block], order: &[usize]) {
    struct Bar {
        task: TaskId,
        loader: LoaderId,
        /// When it would be free, at its rate with nothing else to wait for.
        clock_h: f64,
        tph: f64,
        sequence: Vec<GroundId>,
    }
    let mut bars: Vec<Bar> = input
        .tasks
        .iter()
        .filter(|task| request.tasks.contains(&task.id))
        .map(|task| {
            let rates: Vec<f64> = input
                .loaders
                .iter()
                .find(|entry| entry.id == task.loader)
                .map(|entry| entry.rates.iter().map(|rate| rate.dig_tph).filter(|rate| *rate > 0.0).collect())
                .unwrap_or_default();
            let tph = if rates.is_empty() { 1.0 } else { rates.iter().sum::<f64>() / rates.len() as f64 };
            Bar {
                task: task.id,
                loader: task.loader,
                clock_h: task.window_start_h,
                tph,
                sequence: Vec::new(),
            }
        })
        .collect();
    if bars.is_empty() {
        return;
    }
    let mut holder: BTreeMap<u32, usize> = BTreeMap::new();
    // When each block would be finished, by position.
    let mut finished = vec![0.0_f64; blocks.len()];
    for index in order {
        let block = &blocks[*index];
        let ready = block.above.iter().map(|over| finished[*over]).fold(0.0, f64::max) + block.lead_h;
        let ready = ready.max(block.earliest_h);
        // Ground never there to dig, or under such ground, goes to no bar.
        if !ready.is_finite() {
            finished[*index] = f64::INFINITY;
            continue;
        }
        let Some(ground) = block.ground else {
            finished[*index] = ready;
            continue;
        };
        let start = |bar: usize| bars[bar].clock_h.max(ready);
        let soonest = (0..bars.len()).min_by(|a, b| start(*a).total_cmp(&start(*b))).unwrap_or(0);
        let group = request.groups.get(&ground).copied();
        let bar = group
            .and_then(|group| holder.get(&group).copied())
            .filter(|held| start(*held) <= start(soonest) + block.tonnes / bars[*held].tph)
            .unwrap_or(soonest);
        if let Some(group) = group {
            holder.insert(group, bar);
        }
        finished[*index] = start(bar) + block.tonnes / bars[bar].tph;
        bars[bar].clock_h = finished[*index];
        bars[bar].sequence.push(ground);
    }
    // Which loaders may dig each block: a loader with two automatic bars
    // keeps the candidates for both.
    let mut diggers: BTreeMap<GroundId, BTreeSet<LoaderId>> = BTreeMap::new();
    for bar in &bars {
        for ground in &bar.sequence {
            diggers.entry(*ground).or_default().insert(bar.loader);
        }
    }
    let automatic: BTreeSet<LoaderId> = bars.iter().map(|bar| bar.loader).collect();
    let keep: Vec<bool> = input
        .movements
        .iter()
        .map(|movement| match movement.source {
            SourceId::Ground(ground) if automatic.contains(&movement.loader) => diggers.get(&ground).is_none_or(|loaders| loaders.contains(&movement.loader)),
            _ => true,
        })
        .collect();
    // Conditional values name their candidate by position, which moves.
    let mut moved = Vec::with_capacity(keep.len());
    let mut next = 0;
    for kept in &keep {
        moved.push(kept.then_some(next));
        next += usize::from(*kept);
    }
    input.conditional_values.retain_mut(|value| match moved.get(value.candidate).copied().flatten() {
        Some(position) => {
            value.candidate = position;
            true
        }
        None => false,
    });
    let mut kept = keep.iter();
    input.movements.retain(|_| kept.next().copied().unwrap_or(true));
    for bar in bars {
        if let Some(task) = input.tasks.iter_mut().find(|task| task.id == bar.task) {
            task.kind = TaskKind::Dig { sequence: bar.sequence };
        }
    }
}
