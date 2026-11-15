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
//! blocks there are. The closures found so far split the nodes into groups
//! that every one of them agrees on, and a small linear program gives each
//! group a share, within the capacities and the precedence between groups,
//! and sets the next prices (Bienstock and Zuckerberg). The loop ends when
//! no closure beats that plan at its own prices, or splits a group.
//!
//! The plan is fractional, so it is rounded into an order rather than read
//! as a schedule: each period filled with the blocks the plan digs in it,
//! never before what they wait on. It is then dealt to the bars, and the
//! hourly dispatch works out what actually happens - this plan decides only
//! the sequence the dispatch is handed.

use std::collections::{BTreeMap, BTreeSet};

use super::{DestinationId, DestinationKind, GroundId, LoaderId, SourceId, TaskId, TaskKind, blended::input::BlendInput};

/// The most periods the plan works in. A longer horizon's periods are merged
/// evenly: the plan sets an order, and an order does not need finer steps.
const MAX_PERIODS: usize = 48;
/// The most closures priced before the order is taken from what was found.
const MAX_ROUNDS: usize = 200;
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
    /// Tonnes of it that can go only to stockpiles and dumps of limited
    /// room, by the one it goes to, its position in the input's
    /// destinations: a block with such material is not dug once that room
    /// is gone.
    piled: Vec<(usize, f64)>,
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
    /// When a dig bar naming it should have dug it, where that is inside
    /// the bar's window: ground the automatic bars only get if it is left.
    /// The plan takes it as gone then, at no cost to the automatic loaders
    /// and worth nothing to them, its feed and hauls taken off the
    /// crushers' and trucks' capacity in that period.
    authored_h: Option<f64>,
}

/// Amounts by crusher or truck class.
type Shares = [(usize, f64)];

impl Block {
    /// What it takes of the crushers and the trucks in the period it is dug:
    /// nothing for a dig bar's ground, which is off the limits already.
    fn on_limits(&self) -> (&Shares, &Shares) {
        if self.authored_h.is_some() { (&[], &[]) } else { (&self.crushed, &self.hauled) }
    }
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
    // Material some route takes to a destination of unlimited room.
    let limited = |destination: super::DestinationId| {
        input
            .destinations
            .iter()
            .find(|entry| entry.id == destination)
            .is_some_and(|entry| matches!(entry.kind, DestinationKind::Stockpile(_) | DestinationKind::Dump) && entry.capacity_t.is_some_and(f64::is_finite))
    };
    let free: BTreeSet<(GroundId, super::MaterialId)> = input
        .movements
        .iter()
        .filter(|movement| loaders.contains(&movement.loader) && !limited(movement.destination))
        .filter_map(|movement| match movement.source {
            SourceId::Ground(ground) => Some((ground, movement.material)),
            SourceId::Stockpile(_) => None,
        })
        .collect();
    let sources: BTreeMap<GroundId, &super::GroundSource> = input.ground.iter().map(|source| (source.id, source)).collect();
    // Ground no machine will ever release - a step of its blast that no
    // bar works - is never there to dig, and nor is anything under it.
    let lead: BTreeMap<GroundId, f64> = input
        .drill_blast
        .as_ref()
        .map_or_else(BTreeMap::new, |chain| chain.earliest_releases(&input.intervals).into_iter().collect());
    let never_h = super::blended::drill_blast::NEVER;
    // When the dig bars should dig the ground they name: each bar's blocks
    // one after another from when it opens, at its loader's usual rate,
    // none before its blast could be ready, while its window is open.
    let mut authored: BTreeMap<GroundId, f64> = BTreeMap::new();
    for task in input.tasks.iter().filter(|task| !request.tasks.contains(&task.id)) {
        let TaskKind::Dig { sequence } = &task.kind else { continue };
        let rates: Vec<f64> = input
            .loaders
            .iter()
            .find(|entry| entry.id == task.loader)
            .map(|entry| entry.rates.iter().map(|rate| rate.dig_tph).filter(|rate| *rate > 0.0).collect())
            .unwrap_or_default();
        if rates.is_empty() {
            continue;
        }
        let tph = rates.iter().sum::<f64>() / rates.len() as f64;
        let mut at = task.window_start_h;
        for ground in sequence {
            let ready = lead.get(ground).copied().unwrap_or(0.0);
            at = at.max(ready) + sources.get(ground).map_or(0.0, |source| source.tonnes_t) / tph;
            if at > task.window_end_h || ready >= never_h {
                break;
            }
            if position.contains_key(ground) && request.not_before_h.contains_key(ground) {
                authored.entry(*ground).and_modify(|held| *held = held.min(at)).or_insert(at);
            }
        }
    }
    let mut out: Vec<Block> = grounds
        .iter()
        .map(|ground| {
            let source = sources.get(ground);
            let tonnes = source.map_or(0.0, |source| source.tonnes_t);
            let mut value = 0.0;
            let mut crushed: Vec<(usize, f64)> = Vec::new();
            let mut piled: Vec<(usize, f64)> = Vec::new();
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
                if !free.contains(&(*ground, share.material))
                    && let Some(at) = input.destinations.iter().position(|entry| entry.id == *destination)
                {
                    match piled.iter_mut().find(|(held, _)| *held == at) {
                        Some((_, held)) => *held += share_t,
                        None => piled.push((at, share_t)),
                    }
                }
                if let Some(crusher) = crushers.iter().position(|id| id == destination) {
                    match crushed.iter_mut().find(|(held, _)| *held == crusher) {
                        Some((_, held)) => *held += share_t,
                        None => crushed.push((crusher, share_t)),
                    }
                }
            }
            let authored_h = authored.get(ground).copied();
            Block {
                ground: Some(*ground),
                tonnes: if authored_h.is_some() { 0.0 } else { tonnes },
                value: if authored_h.is_some() { 0.0 } else { value },
                crushed,
                piled,
                hauled,
                worked: [0.0; 3],
                above: Vec::new(),
                lead_h: lead.get(ground).copied().filter(|at| *at < never_h).unwrap_or(0.0),
                earliest_h: if lead.get(ground).is_some_and(|at| *at >= never_h) {
                    f64::INFINITY
                } else {
                    authored_h.or_else(|| request.not_before_h.get(ground).copied()).unwrap_or(0.0)
                },
                authored_h,
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
                piled: Vec::new(),
                hauled: Vec::new(),
                worked: [0.0; 3],
                above: over,
                lead_h: 0.0,
                earliest_h: 0.0,
                authored_h: None,
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
            piled: Vec::new(),
            hauled: Vec::new(),
            worked: [0.0; 3],
            above: over,
            lead_h: 0.0,
            earliest_h: 0.0,
            authored_h: None,
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
    let each = loader_capacity_each(input, request, periods);
    (0..periods.len()).map(|period| each.iter().map(|capacity| capacity[period]).sum()).collect()
}

/// [`loader_capacity`] loader by loader.
fn loader_capacity_each(input: &BlendInput, request: &PlanRequest<'_>, periods: &[(f64, f64)]) -> Vec<Vec<f64>> {
    let tonnes: BTreeMap<GroundId, f64> = input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect();
    let mut out = Vec::new();
    let loaders: BTreeSet<LoaderId> = input.tasks.iter().filter(|task| request.tasks.contains(&task.id)).map(|task| task.loader).collect();
    for loader in loaders {
        let mut capacity = vec![0.0; periods.len()];
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
        out.push(capacity);
    }
    out
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
    // Each loader limit's area, where it holds fewer loaders than the
    // automatic bars have: no more out of it in a period than its most
    // productive loaders that many could dig.
    let each = loader_capacity_each(input, request, &periods);
    let position: BTreeMap<GroundId, usize> = blocks.iter().enumerate().filter_map(|(index, block)| Some((block.ground?, index))).collect();
    let mut block_areas: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut area_row: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (area, entry) in input.loader_areas.iter().enumerate() {
        let allowed = entry.loaders as usize;
        if allowed >= each.len() {
            continue;
        }
        let members: Vec<usize> = entry.ground.iter().filter_map(|ground| position.get(ground).copied()).collect();
        if members.is_empty() {
            continue;
        }
        for member in members {
            block_areas[member].push(area);
        }
        for period in 0..spans {
            let mut capacities: Vec<f64> = each.iter().map(|capacity| capacity[period]).collect();
            capacities.sort_by(|a, b| b.total_cmp(a));
            area_row.insert((area, period), limits.len());
            limits.push(capacities[..allowed].iter().sum());
        }
    }
    // Each stockpile and dump of limited room that material with nowhere
    // else to go is sent to: by the end of each period no more can have
    // gone there than the room it opened with. What reclaim bars take out
    // of a pile is not counted on.
    let mut pile_row: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let piles_used: BTreeSet<usize> = blocks.iter().flat_map(|block| block.piled.iter().map(|(at, _)| *at)).collect();
    for at in piles_used {
        let entry = &input.destinations[at];
        let Some(capacity) = entry.capacity_t else { continue };
        let opening = match entry.kind {
            DestinationKind::Stockpile(pile) => input.piles.iter().find(|held| held.id == pile).map_or(0.0, |held| held.opening_t),
            _ => 0.0,
        };
        for period in 0..spans {
            pile_row.insert((at, period), limits.len());
            limits.push((capacity - opening).max(0.0));
        }
    }
    // What the dig bars' own ground takes of the crushers and trucks, in
    // the period they should dig it.
    for block in blocks {
        let Some(at) = block.authored_h else { continue };
        let Some(period) = periods.iter().position(|(start, end)| *start <= at && at < *end) else {
            continue;
        };
        for (crusher, tonnes) in &block.crushed {
            if let Some(row) = crush_row.get(&(*crusher, period)) {
                limits[*row] = (limits[*row] - tonnes).max(0.0);
            }
        }
        for (truck, hours) in &block.hauled {
            if let Some(row) = haul_row.get(&(*truck, period)) {
                limits[*row] = (limits[*row] - hours).max(0.0);
            }
        }
        for (at, tonnes) in &block.piled {
            for later in period..spans {
                if let Some(row) = pile_row.get(&(*at, later)) {
                    limits[*row] = (limits[*row] - tonnes).max(0.0);
                }
            }
        }
    }
    let usage = |column: &Column| {
        let mut used = vec![0.0; limits.len()];
        for (index, (block, period)) in blocks.iter().zip(column).enumerate() {
            let period = usize::from(*period);
            if period >= spans {
                continue;
            }
            used[period] += block.tonnes;
            for area in &block_areas[index] {
                used[area_row[&(*area, period)]] += block.tonnes;
            }
            if block.authored_h.is_none() {
                for (at, tonnes) in &block.piled {
                    for later in period..spans {
                        if let Some(row) = pile_row.get(&(*at, later)) {
                            used[*row] += tonnes;
                        }
                    }
                }
            }
            // A dig bar's ground: its feed and hauls are off the limits.
            let (crushed, hauled) = block.on_limits();
            for (crusher, tonnes) in crushed {
                if let Some(row) = crush_row.get(&(*crusher, period)) {
                    used[*row] += tonnes;
                }
            }
            for (truck, hours) in hauled {
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
    // Values go to the closure solver as integers: scaled so the largest is
    // well inside what it sums exactly.
    let largest = blocks.iter().map(|block| block.value.abs()).fold(0.0, f64::max).max(1.0);
    let scale = 1e12 / largest;
    let mut weights = vec![0_i64; count * spans];
    let mut solver = mineflow::Solver::new(&precedence, &weights).ok()?;

    let never = spans as u16;
    let nodes = count * spans;
    let forbidden = |node: usize| node % spans < opens[node / spans];
    // What each block dug in each period adds to each side row, and what it
    // is worth then.
    let uses_in = |index: usize, period: usize| -> Vec<(usize, f64)> {
        let block = &blocks[index];
        let mut out = vec![(period, block.tonnes)];
        for area in &block_areas[index] {
            out.push((area_row[&(*area, period)], block.tonnes));
        }
        if block.authored_h.is_none() {
            for (at, tonnes) in &block.piled {
                out.extend((period..spans).filter_map(|later| pile_row.get(&(*at, later)).map(|row| (*row, *tonnes))));
            }
        }
        let (crushed, hauled) = block.on_limits();
        for (crusher, tonnes) in crushed {
            if let Some(row) = crush_row.get(&(*crusher, period)) {
                out.push((*row, *tonnes));
            }
        }
        for (truck, hours) in hauled {
            if let Some(row) = haul_row.get(&(*truck, period)) {
                out.push((*row, *hours));
            }
        }
        for (step, amount) in block.worked.iter().enumerate() {
            if let Some(row) = machine_row.get(&(step, period)).filter(|_| *amount > 0.0) {
                out.push((*row, *amount));
            }
        }
        out
    };
    // The plan is a partition of the "dug by" nodes into groups, each node
    // dug by its period to its group's share (Bienstock and Zuckerberg): the
    // nodes every closure found so far agrees on are one group. Each closure
    // splits the groups it cuts across, and the master gives every group a
    // share of its own, within the capacities and with a group needing
    // whatever group its nodes need - far more plans than a mix of whole
    // closures, so far fewer rounds to the best.
    let mut group: Vec<u32> = vec![0; nodes];
    let mut groups = 1_usize;
    let mut share: Vec<f64> = vec![0.0];
    let mut prices = vec![0.0; limits.len()];
    let started = std::time::Instant::now();
    let (mut value, mut bound, mut rounds) = (0.0, f64::INFINITY, 0);
    while rounds < MAX_ROUNDS && started.elapsed() < request.budget {
        if cancelled() {
            return None;
        }
        let at = &prices;
        // The best closure at these prices. A block dug in a period earns
        // its discounted value less the capacity it pays for; as "dug by"
        // nodes that is the difference between one period and the next.
        let gain = |index: usize, period: usize| blocks[index].value * discount[period] - uses_in(index, period).iter().map(|(row, used)| at[*row] * used).sum::<f64>();
        for (index, block) in blocks.iter().enumerate() {
            for period in 0..spans {
                let here = gain(index, period);
                let next = if period + 1 < spans { gain(index, period + 1) } else { 0.0 };
                // Before its blast could be ready the block is not to be had
                // at any price.
                // A dig bar's ground is gone from when it should be dug: no
                // closure is left free to take it or not at no cost, which
                // would split the groups for nothing.
                weights[index * spans + period] = if period < opens[index] {
                    FORBIDDEN
                } else if block.authored_h.is_some() {
                    1
                } else {
                    ((here - next) * scale).round() as i64
                };
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
        // What this closure is worth at the prices, against what the plan
        // is: no closure beating it means the prices are right.
        let used = usage(&column);
        let priced = worth(&column) - used.iter().zip(at).map(|(used, price)| used * price).sum::<f64>();
        let allowance: f64 = limits.iter().zip(at).map(|(limit, price)| limit * price).sum();
        bound = bound.min(priced + allowance);
        if bound - value <= IMPROVEMENT * value.abs().max(1.0) {
            break;
        }
        // Split each group by the closure.
        let mut relabel: Vec<u32> = vec![u32::MAX; groups * 2];
        let mut next = 0_u32;
        for (node, held) in group.iter_mut().enumerate() {
            let key = *held as usize * 2 + usize::from(chosen[node]);
            if relabel[key] == u32::MAX {
                relabel[key] = next;
                next += 1;
            }
            *held = relabel[key];
        }
        if next as usize == groups {
            // The closure is a plan the master could already make: at its
            // own prices nothing better exists.
            break;
        }
        groups = next as usize;
        // The master: each group's worth and what it takes of each side row,
        // summed over its nodes, and what it needs of other groups.
        let mut objective = vec![0.0; groups];
        let mut coefficients: Vec<Vec<f64>> = vec![vec![0.0; groups]; limits.len()];
        let mut closed = vec![false; groups];
        for (index, block) in blocks.iter().enumerate() {
            for period in 0..spans {
                let node = index * spans + period;
                let held = group[node];
                if forbidden(node) {
                    closed[held as usize] = true;
                }
                let later = if period + 1 < spans { block.value * discount[period + 1] } else { 0.0 };
                objective[held as usize] += block.value * discount[period] - later;
                for (row, used) in uses_in(index, period) {
                    coefficients[row][held as usize] += used;
                }
                if period + 1 < spans {
                    for (row, used) in uses_in(index, period + 1) {
                        coefficients[row][held as usize] -= used;
                    }
                }
            }
        }
        let mut needs: Vec<(u32, u32)> = edges
            .iter()
            .map(|(from, to)| (group[*from as usize], group[*to as usize]))
            .filter(|(from, to)| from != to)
            .collect();
        needs.sort_unstable();
        needs.dedup();
        let mut master = solver::Problem::new(solver::Sense::Maximise);
        let cols: Vec<usize> = (0..groups).map(|held| master.add_col(objective[held], 0.0, if closed[held] { 0.0 } else { 1.0 })).collect();
        for (row, limit) in limits.iter().enumerate() {
            master.add_row(
                f64::NEG_INFINITY,
                *limit,
                coefficients[row]
                    .iter()
                    .enumerate()
                    .filter(|(_, used)| used.abs() > 1e-12)
                    .map(|(held, used)| (cols[held], *used)),
            );
        }
        for (from, to) in &needs {
            master.add_row(f64::NEG_INFINITY, 0.0, [(cols[*from as usize], 1.0), (cols[*to as usize], -1.0)]);
        }
        // Never past the budget, however hard the master gets.
        let options = solver::Options {
            time_limit: request.budget.saturating_sub(started.elapsed()).as_secs_f64().max(0.1),
            ..solver::Options::default()
        };
        let solution = solver::solve(&master, &options);
        if solution.status != solver::Status::Optimal {
            break;
        }
        value = solution.objective;
        share = solution.col_value;
        // A limit's price is what one more tonne of it would add.
        for (price, dual) in prices.iter_mut().zip(&solution.row_dual) {
            *price = dual.abs();
        }
    }
    // How much of each block the plan has dug by the end of each period.
    let by: Vec<Vec<f64>> = (0..count)
        .map(|index| {
            (0..spans)
                .map(|period| share.get(group[index * spans + period] as usize).copied().unwrap_or(0.0).clamp(0.0, 1.0))
                .collect()
        })
        .collect();
    // The plan is a mix of closures, each block dug in some of them, so no
    // one closure is the order. Each period is filled instead, to what the
    // loaders and crushers can take in it: first with the crusher feed the
    // plan has most of dug by then and whatever is still over it, then with
    // the rest of the ground it has most of dug, up to what the plan digs
    // besides feed in that period. A period carries the ore the plan meant
    // for it as well as the ground over it, where an order by when blocks
    // are dug on average would put all the stripping first.
    let waiting: Vec<usize> = blocks.iter().map(|block| block.above.len()).collect();
    let mut below: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (index, block) in blocks.iter().enumerate() {
        for over in &block.above {
            below[*over].push(index);
        }
    }
    // What the plan crushes and digs besides in each period: a period's
    // ground is split between the two as the plan splits it, or the ground
    // the plan is surest of - the stripping every closure digs early - takes
    // all of a period and leaves the crusher idle.
    let crushed_of = |block: &Block| block.crushed.iter().map(|(_, tonnes)| tonnes).sum::<f64>();
    let mut other_room = vec![0.0; spans];
    for (index, block) in blocks.iter().enumerate() {
        let other = (block.tonnes - crushed_of(block)).max(0.0);
        for (period, room) in other_room.iter_mut().enumerate() {
            let earlier = if period > 0 { by[index][period - 1] } else { 0.0 };
            *room += other * (by[index][period] - earlier).max(0.0);
        }
    }
    let key = |index: usize, period: usize| (by[index][period].to_bits(), std::cmp::Reverse(rank[index]), index);
    let mut fill = Fill {
        placed: vec![false; count],
        order: Vec::with_capacity(count),
        freed: (0..count).filter(|index| waiting[*index] == 0).collect(),
        waiting,
    };
    // Blocks that are mostly crusher feed.
    let ore: Vec<usize> = (0..count)
        .filter(|index| blocks[*index].tonnes > 0.0 && crushed_of(&blocks[*index]) >= 0.5 * blocks[*index].tonnes)
        .collect();
    let mut stamp = vec![usize::MAX; count];
    for period in 0..spans {
        let first = fill.order.len();
        let mut room = dig[period];
        let mut other_left = other_room[period];
        let mut crushing: BTreeMap<usize, f64> = crush_row.iter().filter(|((_, at), _)| *at == period).map(|(_, row)| (*row, limits[*row])).collect();
        let rows_of = |block: &Block| -> Vec<usize> { block.crushed.iter().filter_map(|(crusher, _)| crush_row.get(&(*crusher, period)).copied()).collect() };
        let take = |block: &Block, room: &mut f64, other_left: &mut f64, crushing: &mut BTreeMap<usize, f64>| {
            *room -= block.tonnes;
            *other_left -= (block.tonnes - crushed_of(block)).max(0.0);
            for (crusher, tonnes) in &block.crushed {
                if let Some(left) = crush_row.get(&(*crusher, period)).and_then(|row| crushing.get_mut(row)) {
                    *left -= tonnes;
                }
            }
        };
        // The crusher first: the ore the plan is surest of by now, with
        // whatever is still over it, while the loaders have room for both.
        let mut candidates: Vec<usize> = ore.iter().copied().filter(|index| !fill.placed[*index] && by[*index][period] >= 1e-6).collect();
        candidates.sort_by(|a, b| by[*b][period].total_cmp(&by[*a][period]).then(rank[*a].cmp(&rank[*b])));
        for index in candidates {
            if room <= 0.0 {
                break;
            }
            let crusher_full = |crushing: &BTreeMap<usize, f64>| rows_of(&blocks[index]).iter().any(|row| crushing.get(row).is_some_and(|left| *left <= 0.0));
            if fill.placed[index] || crusher_full(&crushing) {
                continue;
            }
            // Everything still over it, while it fits.
            let mut needed = vec![index];
            let mut stack = vec![index];
            stamp[index] = index;
            let mut tonnes = blocks[index].tonnes;
            let limit = room + blocks[index].tonnes;
            while let Some(at) = stack.pop() {
                for over in &blocks[at].above {
                    if !fill.placed[*over] && stamp[*over] != index {
                        stamp[*over] = index;
                        tonnes += blocks[*over].tonnes;
                        needed.push(*over);
                        stack.push(*over);
                    }
                }
                if tonnes > limit {
                    break;
                }
            }
            if tonnes > limit {
                continue;
            }
            needed.sort_by_key(|at| rank[*at]);
            for at in needed {
                take(&blocks[at], &mut room, &mut other_left, &mut crushing);
                fill.place(at, &below);
            }
        }
        // Then the rest of the period, by how sure the plan is of each.
        let mut heap: std::collections::BinaryHeap<(u64, std::cmp::Reverse<usize>, usize)> =
            fill.freed.drain(..).filter(|index| !fill.placed[*index]).map(|index| key(index, period)).collect();
        let mut held: Vec<usize> = Vec::new();
        while let Some((_, _, index)) = heap.pop() {
            if fill.placed[index] {
                continue;
            }
            let block = &blocks[index];
            let other = block.tonnes - crushed_of(block);
            let full = (block.tonnes > 0.0 && room <= 0.0)
                || (other > 0.5 * block.tonnes && other_left <= 0.0)
                || rows_of(block).iter().any(|row| crushing.get(row).is_some_and(|left| *left <= 0.0));
            if by[index][period] < 1e-6 || full {
                held.push(index);
                continue;
            }
            take(block, &mut room, &mut other_left, &mut crushing);
            fill.place(index, &below);
            heap.extend(fill.freed.drain(..).map(|index| key(index, period)));
        }
        // Loaders the budgets left idle take what was held for them, the
        // ground the plan digs soonest first: a loader standing still
        // while the plan's ground waits under what a full crusher held
        // back helps nothing, and what the crusher cannot take the
        // dispatch sends elsewhere.
        if room > 0.0 {
            let mut heap: std::collections::BinaryHeap<(u64, std::cmp::Reverse<usize>, usize)> =
                held.drain(..).filter(|index| !fill.placed[*index]).map(|index| key(index, period)).collect();
            while let Some((_, _, index)) = heap.pop() {
                if fill.placed[index] {
                    continue;
                }
                if room <= 0.0 || by[index][period] < 1e-6 {
                    held.push(index);
                    continue;
                }
                take(&blocks[index], &mut room, &mut other_left, &mut crushing);
                fill.place(index, &below);
                heap.extend(fill.freed.drain(..).map(|index| key(index, period)));
            }
        }
        fill.freed = held;
        interleave(&mut fill.order[first..], blocks, &rank, &crushed_of);
    }
    let Fill { placed, mut order, .. } = fill;
    // What no period took follows, by when the plan digs it on average and
    // never before anything it waits on.
    let dug: Vec<f64> = (0..count).map(|index| by[index][spans - 1]).collect();
    let when: Vec<f64> = (0..count)
        .map(|index| {
            let total: f64 = (0..spans)
                .map(|period| period as f64 * (by[index][period] - if period > 0 { by[index][period - 1] } else { 0.0 }))
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
    // Ground dig bars name, and whatever has to come off before it.
    let mut named: Vec<bool> = blocks
        .iter()
        .map(|block| block.ground.is_some_and(|ground| request.not_before_h.contains_key(&ground)))
        .collect();
    let mut stack: Vec<usize> = (0..count).filter(|index| named[*index]).collect();
    while let Some(index) = stack.pop() {
        for over in &blocks[index].above {
            if !std::mem::replace(&mut named[*over], true) {
                stack.push(*over);
            }
        }
    }
    let order = started_first(&order, blocks, &rank, &named);
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

/// The order with the ground dig bars name and leave first, from the top
/// down, as soon as nothing it waits on is still to come; the plan orders
/// the rest. That ground is started faces, mostly blasted already, with ore
/// open to the crusher at once, and a plan that leaves it for fresh ground
/// starves the crusher while the fresh ground is stripped and blasted.
fn started_first(order: &[usize], blocks: &[Block], rank: &[usize], started: &[bool]) -> Vec<usize> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    let mut place = vec![usize::MAX; blocks.len()];
    for (at, index) in order.iter().enumerate() {
        place[*index] = at;
    }
    let key = |index: usize| Reverse((usize::from(!started[index]), if started[index] { rank[index] } else { place[index] }, index));
    let mut waiting: Vec<usize> = blocks.iter().map(|block| block.above.len()).collect();
    let mut below: Vec<Vec<usize>> = vec![Vec::new(); blocks.len()];
    for (index, block) in blocks.iter().enumerate() {
        for over in &block.above {
            below[*over].push(index);
        }
    }
    let mut ready: BinaryHeap<Reverse<(usize, usize, usize)>> = order.iter().copied().filter(|index| waiting[*index] == 0).map(key).collect();
    let mut done = vec![false; blocks.len()];
    let mut out = Vec::with_capacity(order.len());
    while let Some(Reverse((_, _, index))) = ready.pop() {
        if std::mem::replace(&mut done[index], true) {
            continue;
        }
        out.push(index);
        for under in &below[index] {
            waiting[*under] = waiting[*under].saturating_sub(1);
            if waiting[*under] == 0 {
                ready.push(key(*under));
            }
        }
    }
    // Ground in a ring of waits keeps its place after the rest.
    out.extend(order.iter().copied().filter(|index| !done[*index]));
    out
}

/// A period's blocks put in the order they are dug in: crusher feed and the
/// rest in step, each as far through its own tonnes as the other, so the
/// crusher is fed through the period rather than all at its start - it
/// takes only so much a day - and nothing before what it waits on.
fn interleave(slice: &mut [usize], blocks: &[Block], rank: &[usize], crushed_of: &dyn Fn(&Block) -> f64) {
    use std::{cmp::Reverse, collections::BinaryHeap};
    if slice.len() < 2 {
        return;
    }
    let local: BTreeMap<usize, usize> = slice.iter().enumerate().map(|(at, index)| (*index, at)).collect();
    let mut waiting: Vec<usize> = slice
        .iter()
        .map(|index| blocks[*index].above.iter().filter(|over| local.contains_key(over)).count())
        .collect();
    let mut below: Vec<Vec<usize>> = vec![Vec::new(); slice.len()];
    for (at, index) in slice.iter().enumerate() {
        for over in &blocks[*index].above {
            if let Some(from) = local.get(over) {
                below[*from].push(at);
            }
        }
    }
    let feed = |at: usize| {
        let block = &blocks[slice[at]];
        block.tonnes > 0.0 && crushed_of(block) >= 0.5 * block.tonnes
    };
    let total = |want: bool| {
        slice
            .iter()
            .enumerate()
            .filter(|(at, _)| feed(*at) == want)
            .map(|(_, index)| blocks[*index].tonnes)
            .sum::<f64>()
            .max(1e-9)
    };
    let (feed_total, other_total) = (total(true), total(false));
    let mut queues: [BinaryHeap<Reverse<(usize, usize)>>; 2] = [BinaryHeap::new(), BinaryHeap::new()];
    for (at, count) in waiting.iter().enumerate() {
        if *count == 0 {
            queues[usize::from(feed(at))].push(Reverse((rank[slice[at]], at)));
        }
    }
    let (mut fed, mut other) = (0.0, 0.0);
    let mut out = Vec::with_capacity(slice.len());
    while out.len() < slice.len() {
        let want_feed = fed / feed_total <= other / other_total;
        let pick = if queues[usize::from(want_feed)].is_empty() {
            usize::from(!want_feed)
        } else {
            usize::from(want_feed)
        };
        let Some(Reverse((_, at))) = queues[pick].pop() else { break };
        if pick == 1 {
            fed += blocks[slice[at]].tonnes;
        } else {
            other += blocks[slice[at]].tonnes;
        }
        out.push(slice[at]);
        for under in &below[at] {
            waiting[*under] -= 1;
            if waiting[*under] == 0 {
                queues[usize::from(feed(*under))].push(Reverse((rank[slice[*under]], *under)));
            }
        }
    }
    if out.len() == slice.len() {
        slice.copy_from_slice(&out);
    }
}

/// The order a plan's periods are filled in, as it grows.
struct Fill {
    placed: Vec<bool>,
    order: Vec<usize>,
    /// What each block still waits on.
    waiting: Vec<usize>,
    /// Blocks free to be dug that have not been looked at yet.
    freed: Vec<usize>,
}

impl Fill {
    /// Put a block next in the order, freeing what waited only on it.
    fn place(&mut self, index: usize, below: &[Vec<usize>]) {
        self.placed[index] = true;
        self.order.push(index);
        for under in &below[index] {
            self.waiting[*under] = self.waiting[*under].saturating_sub(1);
            if self.waiting[*under] == 0 {
                self.freed.push(*under);
            }
        }
    }
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
    // Ground a loader's own dig bar names stays its to dig whoever is dealt
    // it: what the dig bar leaves is the automatic bars', but not before.
    let named: BTreeSet<(LoaderId, GroundId)> = input
        .tasks
        .iter()
        .filter(|task| !request.tasks.contains(&task.id))
        .filter_map(|task| match &task.kind {
            TaskKind::Dig { sequence } => Some(sequence.iter().map(move |ground| (task.loader, *ground))),
            _ => None,
        })
        .flatten()
        .collect();
    let keep: Vec<bool> = input
        .movements
        .iter()
        .map(|movement| match movement.source {
            SourceId::Ground(ground) if automatic.contains(&movement.loader) => {
                named.contains(&(movement.loader, ground)) || diggers.get(&ground).is_none_or(|loaders| loaders.contains(&movement.loader))
            }
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

/// A dispatched schedule's value discounted at `rate` a year, as the plan
/// values ground: each movement's value and each day's grade penalty at
/// the discount of its time, and whatever else the replay counted - pile
/// values that hang on a blend - at the movements' average discount.
pub(crate) fn discounted_value(input: &BlendInput, solution: &super::blended::replay::BlendSolution, replay: &super::blended::replay::ReplayReport, rate: f64) -> f64 {
    let factor = |hour: f64| (1.0 + rate.max(0.0)).powf(-hour / YEAR_H);
    let (mut plain, mut discounted) = (0.0, 0.0);
    for row in &solution.movements {
        let (Some(movement), Some(interval)) = (input.movements.get(row.candidate), input.intervals.get(row.interval)) else {
            continue;
        };
        let value = row.tonnes_t * movement.value_per_tonne().unwrap_or(0.0);
        plain += value;
        discounted += value * factor(0.5 * (interval.start_h + interval.end_h));
    }
    let (mut penalty, mut penalty_discounted) = (0.0, 0.0);
    for (&(index, day), &(tonnes, contained)) in &replay.target_totals {
        let Some(target) = input.grade_targets.get(index) else { continue };
        let cost = target.specification.penalty(tonnes, contained);
        penalty += cost;
        penalty_discounted += cost * factor((f64::from(day) + 0.5) * 24.0);
    }
    let average = if plain.abs() > 1e-9 { discounted / plain } else { 1.0 };
    let rest = replay.replayed_objective - plain + penalty;
    discounted - penalty_discounted + rest * average
}
