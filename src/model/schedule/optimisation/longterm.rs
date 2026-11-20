//! The order automatic bars dig their ground in, and which bar digs what.
//!
//! The ground the schedule sequences for itself is planned over the whole
//! horizon at once, in periods: which period each block is best dug in, given
//! that a block cannot go before the ground above it, that the loaders move
//! only so many tonnes a period and that a crusher takes only so many. What
//! is wanted is the discounted value of the material moved, less what each
//! period's crusher feed costs off its grade targets.
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
/// How near the best on its groups each master is solved, as a share of
/// the plan's gap.
const INNER: f64 = 0.1;
/// Rounds a plan stays in the master unused.
const IDLE_ROUNDS: u32 = 3;
/// How far each master's closures are priced toward the prices that gave
/// its lowest bound.
const SMOOTHING: f64 = 0.7;
/// The most closures the last plan is restarted from.
const PRIOR_LEVELS: usize = 200;
/// Rounds a plan restarted from the last one goes on without bettering it.
const RESTART_FLAT_ROUNDS: usize = 3;
/// How many of the crusher feed blocks the plan is surest of, in turn, a
/// period's next is chosen from for its grade.
const BLEND_CHOICES: usize = 50;
/// The weight of a node no closure may hold: far past anything the scaled
/// values sum to, and far inside what the closure solver adds up exactly.
const FORBIDDEN: i64 = -(1 << 56);
/// Hours in a day, over which an automatic bar's loader blends its ground.
const DAY_H: f64 = 24.0;
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
    /// When the pricing stops short of proving the plan the best.
    pub(crate) stop: crate::model::schedule::experiment::PlanStop,
    /// Each block's identity across edits, which its `GroundId` is not.
    pub(crate) stable: &'a BTreeMap<GroundId, u64>,
    /// What the last plan over this mine left: see [`PlanMemory`].
    pub(crate) memory: Option<&'a PlanMemory>,
    /// Earlier plans, any of which over the same problem answers it too.
    pub(crate) earlier: &'a [std::sync::Arc<PlanMemory>],
    /// The ground a bar limited to some pits, benches or flitches may take,
    /// by its task; a task not named takes any.
    pub(crate) allowed: &'a BTreeMap<TaskId, BTreeSet<GroundId>>,
}

/// What a plan leaves for the next one, so an edit is planned from where
/// the last plan ended rather than from nothing: the key of the problem it
/// solved, which a plan over the same problem answers with its order
/// without pricing anything, and how much of each block it had dug by each
/// period, which a plan over a changed one starts its groups from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PlanMemory {
    key: u64,
    order: Vec<usize>,
    summary: PlanSummary,
    /// By the block's identity across edits.
    dug_by: BTreeMap<u64, Vec<f32>>,
    /// What a tonne of the automatic loaders' digging was worth in each
    /// period at the last prices, in that period's money, by when the
    /// period ends: [`BlendInput::dig_prices`].
    pub(crate) dig_prices: Vec<(f64, f64)>,
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
    /// The order is the last plan's, over the same problem.
    pub(crate) reused: bool,
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
    /// What that crusher feed contains of each grade a crusher target
    /// prices: `(crusher, grade, contained)`.
    assay: Vec<(usize, usize, f64)>,
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

/// The precedence pairs `(from, to)` between `groups` nodes less those
/// that others imply: `from` reaching `to` some other way. `None` when the
/// pairs hold a cycle.
fn reduce_needs(groups: usize, needs: &[(u32, u32)]) -> Option<Vec<(u32, u32)>> {
    let mut out: Vec<Vec<u32>> = vec![Vec::new(); groups];
    let mut indegree = vec![0_usize; groups];
    for (from, to) in needs {
        out[*from as usize].push(*to);
        indegree[*to as usize] += 1;
    }
    // Kahn's order: every pair goes forward in it.
    let mut order: Vec<usize> = (0..groups).filter(|node| indegree[*node] == 0).collect();
    let mut at = 0;
    while at < order.len() {
        let node = order[at];
        at += 1;
        for next in &out[node] {
            indegree[*next as usize] -= 1;
            if indegree[*next as usize] == 0 {
                order.push(*next as usize);
            }
        }
    }
    if order.len() < groups {
        return None;
    }
    let mut position = vec![0_usize; groups];
    for (index, node) in order.iter().enumerate() {
        position[*node] = index;
    }
    // What each node reaches, a bit a node, filled from the last in the
    // order back: a pair is kept unless a nearer one already reaches its end.
    let words = groups.div_ceil(64);
    let mut reach = vec![0_u64; groups * words];
    let mut kept: Vec<(u32, u32)> = Vec::with_capacity(needs.len());
    let mut seen = vec![0_u64; words];
    for &node in order.iter().rev() {
        let mut next: Vec<u32> = out[node].clone();
        next.sort_by_key(|at| position[*at as usize]);
        next.dedup();
        seen.fill(0);
        for to in next {
            let to = to as usize;
            if seen[to / 64] >> (to % 64) & 1 == 1 {
                continue;
            }
            kept.push((node as u32, to as u32));
            seen[to / 64] |= 1 << (to % 64);
            for (word, reached) in seen.iter_mut().zip(&reach[to * words..(to + 1) * words]) {
                *word |= reached;
            }
        }
        reach[node * words..(node + 1) * words].copy_from_slice(&seen);
    }
    Some(kept)
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
pub(crate) fn assign(input: &mut BlendInput, request: &PlanRequest<'_>, planned: bool, cancelled: &dyn Fn() -> bool) -> (PlanSummary, Vec<u32>, Option<PlanMemory>) {
    let Some(first) = input.tasks.iter().find(|task| request.tasks.contains(&task.id)) else {
        return (PlanSummary::default(), Vec::new(), None);
    };
    let TaskKind::Dig { sequence } = &first.kind else {
        return (PlanSummary::default(), Vec::new(), None);
    };
    let grounds: Vec<GroundId> = sequence.clone();
    let blocks = blocks(input, request, &grounds);
    let (order, summary, memory) = match if planned { planned_order(input, request, &blocks, cancelled) } else { None } {
        Some((order, summary, memory)) => (order, summary, Some(memory)),
        None => (
            (0..blocks.len()).collect(),
            PlanSummary {
                blocks: blocks.len(),
                ..PlanSummary::default()
            },
            None,
        ),
    };
    deal(input, request, &blocks, &order);
    (summary, order.iter().map(|at| *at as u32).collect(), memory)
}

/// Deal the automatic bars' ground in `order`, the plan's blocks by
/// position as [`assign`] returned them for this same model, without
/// planning: false, and nothing dealt, when it does not fit the blocks.
pub(crate) fn assign_order(input: &mut BlendInput, request: &PlanRequest<'_>, order: &[u32]) -> bool {
    let Some(first) = input.tasks.iter().find(|task| request.tasks.contains(&task.id)) else {
        return false;
    };
    let TaskKind::Dig { sequence } = &first.kind else { return false };
    let grounds: Vec<GroundId> = sequence.clone();
    let blocks = blocks(input, request, &grounds);
    let mut seen = vec![false; blocks.len()];
    let order: Vec<usize> = order.iter().map(|at| *at as usize).collect();
    if order.len() != blocks.len() || order.iter().any(|at| *at >= blocks.len() || std::mem::replace(&mut seen[*at], true)) {
        return false;
    }
    deal(input, request, &blocks, &order);
    true
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
    // The grades each crusher's targets price, by crusher.
    let targeted: BTreeSet<(usize, usize)> = input
        .grade_targets
        .iter()
        .filter_map(|target| Some((crushers.iter().position(|id| *id == target.destination)?, target.grade)))
        .collect();
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
            let mut assay: Vec<(usize, usize, f64)> = Vec::new();
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
                    let fractions = input.grades.fractions(share.material).unwrap_or_default();
                    for grade in targeted.iter().filter(|(at, _)| *at == crusher).map(|(_, grade)| *grade) {
                        let contained = share_t * fractions.get(grade).copied().unwrap_or(0.0);
                        match assay.iter_mut().find(|(at, held, _)| *at == crusher && *held == grade) {
                            Some((_, _, held)) => *held += contained,
                            None => assay.push((crusher, grade, contained)),
                        }
                    }
                }
            }
            let authored_h = authored.get(ground).copied();
            Block {
                ground: Some(*ground),
                tonnes: if authored_h.is_some() { 0.0 } else { tonnes },
                value: if authored_h.is_some() { 0.0 } else { value },
                crushed,
                assay,
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
                assay: Vec::new(),
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
    // Automatic machines' blasts: each step still to do on one, all of it
    // on the blast's clearance, which its ground waits on - no block of a
    // blast is there until the whole blast is prepped, drilled and charged.
    // Their tasks hold one blast each, so they add no order between blasts,
    // and no machine pace: what limits them is their machines' hours,
    // priced like the loaders'. A blast already started has no clearance
    // step: its work is shared out over its ground by tonnes.
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
            if let Some((step_node, _)) = held.iter().find_map(|block| cleared_by.get(block).filter(|(_, at)| at == blast)) {
                out[*step_node].worked[step] += job.quantity[step];
                continue;
            }
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
            assay: Vec::new(),
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
fn planned_order(input: &BlendInput, request: &PlanRequest<'_>, blocks: &[Block], cancelled: &dyn Fn() -> bool) -> Option<(Vec<usize>, PlanSummary, PlanMemory)> {
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
    // Each crusher grade target, period by period: what the period's feed
    // contains against its tonnes, past each of the target's hinges, costs
    // what the daily target charges for it. A period that keeps its average
    // in the band can still have days outside it, so this is the least the
    // days will pay, but it is what a plan chasing the richest ore never
    // sees: its feed drifts out of the band for months.
    struct Hinge {
        row: usize,
        boundary: f64,
        direction: f64,
        cost: f64,
    }
    let crusher_ids: Vec<DestinationId> = input
        .destinations
        .iter()
        .filter(|entry| entry.kind == DestinationKind::Crusher)
        .map(|entry| entry.id)
        .collect();
    let mut hinges: Vec<Hinge> = Vec::new();
    let mut hinge_rows: BTreeMap<(usize, usize, usize), Vec<usize>> = BTreeMap::new();
    {
        let mut by_period: BTreeMap<(usize, usize, usize), Vec<&crate::model::schedule::grade_targets::GradeTarget>> = BTreeMap::new();
        for target in &input.grade_targets {
            let Some(crusher) = crusher_ids.iter().position(|id| *id == target.destination) else {
                continue;
            };
            let hour = (f64::from(target.day) + 0.5) * crate::model::schedule::grade_targets::TARGET_PERIOD_H;
            let Some(period) = periods.iter().position(|(start, end)| *start <= hour && hour < *end) else {
                continue;
            };
            by_period.entry((crusher, target.grade, period)).or_default().push(&target.specification);
        }
        for (key, specifications) in by_period {
            // The specification most of the period's days have.
            let Some(specification) = specifications.iter().max_by_key(|held| specifications.iter().filter(|other| other == held).count()) else {
                continue;
            };
            for (boundary, direction, slope) in specification.hinges() {
                if slope <= 0.0 {
                    continue;
                }
                hinge_rows.entry(key).or_default().push(hinges.len());
                hinges.push(Hinge {
                    row: limits.len(),
                    boundary,
                    direction,
                    cost: slope * discount[key.2],
                });
                limits.push(0.0);
            }
        }
    }
    let hinge_use = |block: &Block, period: usize, out: &mut Vec<(usize, f64)>| {
        for (crusher, grade, contained) in &block.assay {
            let Some(rows) = hinge_rows.get(&(*crusher, *grade, period)) else { continue };
            let tonnes: f64 = block.crushed.iter().filter(|(at, _)| at == crusher).map(|(_, tonnes)| tonnes).sum();
            for hinge in rows.iter().map(|at| &hinges[*at]) {
                out.push((hinge.row, hinge.direction * (contained - hinge.boundary * tonnes)));
            }
        }
    };
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
        // Its feed is in the crusher's blend all the same.
        let mut fed = Vec::new();
        hinge_use(block, period, &mut fed);
        for (row, used) in fed {
            limits[row] -= used;
        }
    }
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
    // Everything the plan and its rounding read, blocks by their identity
    // across edits: the same key is the same problem, and the last plan's
    // order its answer.
    let stable = |block: &Block| block.ground.and_then(|ground| request.stable.get(&ground).copied());
    let problem = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (index, block) in blocks.iter().enumerate() {
            stable(block).hash(&mut hasher);
            block.ground.is_some_and(|ground| request.not_before_h.contains_key(&ground)).hash(&mut hasher);
            format!(
                "{:?}",
                (
                    block.tonnes,
                    block.value,
                    &block.crushed,
                    &block.assay,
                    &block.piled,
                    &block.hauled,
                    block.worked,
                    &block.above,
                    block.lead_h,
                    block.earliest_h,
                    block.authored_h,
                    (&block_areas[index], rank[index]),
                )
            )
            .hash(&mut hasher);
        }
        format!("{periods:?}{discount:?}{limits:?}{opens:?}{lag:?}{:?}", input.grade_targets).hash(&mut hasher);
        for hinge in &hinges {
            format!("{:?}", (hinge.row, hinge.boundary, hinge.direction, hinge.cost)).hash(&mut hasher);
        }
        for entry in input.destinations.iter().filter(|entry| entry.kind == DestinationKind::Crusher) {
            format!("{:?}", entry.crusher_daily_t).hash(&mut hasher);
        }
        hasher.finish()
    };
    if let Some(memory) = request
        .memory
        .into_iter()
        .chain(request.earlier.iter().map(|memory| &**memory))
        .find(|memory| memory.key == problem && memory.order.len() == count)
    {
        return Some((
            memory.order.clone(),
            PlanSummary {
                reused: true,
                rounds: 0,
                seconds: 0.0,
                ..memory.summary
            },
            memory.clone(),
        ));
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
        if block.authored_h.is_none() {
            hinge_use(block, period, &mut out);
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
    // Every node's use of the side rows, worked out once: each round reads
    // all of them twice over.
    let (use_start, use_rows): (Vec<usize>, Vec<(usize, f64)>) = {
        let mut start = Vec::with_capacity(nodes + 1);
        let mut rows = Vec::new();
        start.push(0);
        for index in 0..count {
            for period in 0..spans {
                rows.extend(uses_in(index, period));
                start.push(rows.len());
            }
        }
        (start, rows)
    };
    let uses_of = |index: usize, period: usize| {
        let node = index * spans + period;
        &use_rows[use_start[node]..use_start[node + 1]]
    };
    // What a closure, as the period each block is dug in, takes of each row.
    let usage = |column: &Column| {
        let mut used = vec![0.0; limits.len()];
        for (index, period) in column.iter().enumerate() {
            let period = usize::from(*period);
            if period < spans {
                for &(row, amount) in uses_of(index, period) {
                    used[row] += amount;
                }
            }
        }
        used
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
    // From where the last plan ended: its nodes dug to the same share are
    // one group, and the first master is given the closures those shares
    // stack up from, so it starts with the last plan and the closures need
    // only find what the edit changed.
    let mut prior_share: Option<Vec<f64>> = None;
    if let Some(memory) = request.memory {
        let mut shares = vec![0.0; nodes];
        for (index, block) in blocks.iter().enumerate() {
            if let Some(prior) = stable(block).and_then(|at| memory.dug_by.get(&at)).filter(|shares| shares.len() == spans) {
                for period in 0..spans {
                    shares[index * spans + period] = f64::from(prior[period]).clamp(0.0, 1.0);
                }
            }
        }
        // A block the last plan cannot be matched to was dug at least as
        // far as anything waiting on it.
        {
            let known: Vec<bool> = blocks
                .iter()
                .map(|block| stable(block).and_then(|at| memory.dug_by.get(&at)).is_some_and(|shares| shares.len() == spans))
                .collect();
            let mut needed: Vec<Vec<usize>> = vec![Vec::new(); nodes];
            for (from, to) in &edges {
                needed[*from as usize].push(*to as usize);
            }
            let mut stack: Vec<usize> = (0..nodes).filter(|node| shares[*node] > 0.0).collect();
            while let Some(node) = stack.pop() {
                for to in &needed[node] {
                    if !known[*to / spans] && shares[*to] < shares[node] {
                        shares[*to] = shares[node];
                        stack.push(*to);
                    }
                }
            }
        }
        let mut labels: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
        for (node, share) in shares.iter().enumerate() {
            let next = labels.len() as u32;
            group[node] = *labels.entry(share.to_bits()).or_insert(next);
        }
        groups = labels.len().max(1);
        prior_share = Some(shares);
    }
    let mut share: Vec<f64> = vec![0.0];
    let mut prices = vec![0.0; limits.len()];
    let started = std::time::Instant::now();
    // The first split is a plan that can be dug: every block, in the order
    // the blocks came, in the first period what it waits on and the limits
    // leave room in - the grade targets are not limits, a period's feed may
    // be off them at their cost. With nothing to start from, the first
    // closures, priced at nothing, dig everything at once, and each round
    // after them settles little more than a period, so most of the budget
    // went on reaching a plan as good as the plainest one.
    let hard: Vec<bool> = {
        let mut hard = vec![true; limits.len()];
        for hinge in &hinges {
            hard[hinge.row] = false;
        }
        hard
    };
    let mut by_rank: Vec<usize> = (0..count).collect();
    by_rank.sort_by_key(|index| rank[*index]);
    let mut seeded = {
        let mut left = limits.clone();
        let mut column: Column = vec![never; count];
        for &index in &by_rank {
            let after = blocks[index].above.iter().map(|over| usize::from(column[*over]) + lag[index]).max().unwrap_or(0);
            let first = after.max(opens[index]);
            let fits = |period: usize, left: &[f64]| {
                uses_of(index, period)
                    .iter()
                    .all(|(row, amount)| !hard[*row] || *amount <= 0.0 || left[*row] >= *amount - 1e-6)
            };
            if let Some(period) = (first..spans).find(|period| fits(*period, &left)) {
                for &(row, amount) in uses_of(index, period) {
                    left[row] -= amount;
                }
                column[index] = period as u16;
            }
        }
        Some(column)
    };
    let (mut value, mut bound, mut rounds) = (0.0, f64::INFINITY, 0);
    use crate::model::schedule::experiment::PlanStop;
    let out_of_time = || matches!(request.stop, PlanStop::Seconds(seconds) if started.elapsed().as_secs_f64() >= seconds);
    let most_rounds = match request.stop {
        PlanStop::Rounds(most) => (most as usize).min(MAX_ROUNDS),
        _ => MAX_ROUNDS,
    };
    // The value at the end of the round before, for a plan that stops once a
    // round adds too little.
    let mut last_value: Option<f64> = None;
    let restarted = prior_share.is_some();
    let (mut best_value, mut flat) = (f64::NEG_INFINITY, 0);
    // The master's plans, as the groups each digs: what one digs, and so
    // what it is worth and takes of each row, stays the same as the groups
    // split.
    struct Plan {
        held: Vec<bool>,
        worth: f64,
        uses: Vec<f64>,
        /// Rounds since the mix last used it.
        idle: u32,
    }
    let mut plans: Vec<Plan> = Vec::new();
    while rounds < most_rounds && !out_of_time() {
        if cancelled() {
            return None;
        }
        let at = &prices;
        // Each node's side of the split: whether the closure holds it, or for
        // the first plan, the period it digs the block in and the node's -
        // a group for each period's blocks by each period, so the master
        // can move any period's ground sooner or later from the start.
        let (chosen, sides): (Vec<usize>, usize) = if let Some(column) = seeded.take() {
            (
                (0..nodes).map(|node| usize::from(column[node / spans]) * spans + node % spans).collect(),
                (spans + 1) * spans,
            )
        } else {
            // The best closure at these prices. A block dug in a period earns
            // its discounted value less the capacity it pays for; as "dug by"
            // nodes that is the difference between one period and the next.
            let mut gains = vec![0.0; spans + 1];
            for (index, block) in blocks.iter().enumerate() {
                for (period, gain) in gains.iter_mut().take(spans).enumerate() {
                    *gain = block.value * discount[period] - uses_of(index, period).iter().map(|(row, used)| at[*row] * used).sum::<f64>();
                }
                for period in 0..spans {
                    let (here, next) = (gains[period], gains[period + 1]);
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
            // Restarted from the last plan, which was the best there was
            // before the edit: rounds that add nothing to it only prove it.
            if restarted {
                if value > best_value + IMPROVEMENT * value.abs().max(1.0) {
                    (best_value, flat) = (value, 0);
                } else {
                    flat += 1;
                    if flat >= RESTART_FLAT_ROUNDS {
                        break;
                    }
                }
            }
            (chosen.iter().map(|held| usize::from(*held)).collect(), 2)
        };
        // Split each group by the closure. Each new group remembers the
        // group it came from and its side of the split.
        let mut relabel: Vec<u32> = vec![u32::MAX; groups * sides];
        let mut parent: Vec<u32> = Vec::new();
        let mut side: Vec<usize> = Vec::new();
        for (node, held) in group.iter_mut().enumerate() {
            let key = *held as usize * sides + chosen[node];
            if relabel[key] == u32::MAX {
                relabel[key] = parent.len() as u32;
                parent.push(*held);
                side.push(chosen[node]);
            }
            *held = relabel[key];
        }
        if parent.len() == groups {
            // The closure is a plan the master could already make: at its
            // own prices nothing better exists.
            break;
        }
        groups = parent.len();
        // The last shares, in the new groups, until the master gives new ones.
        share = parent.iter().map(|old| share.get(*old as usize).copied().unwrap_or(0.0)).collect();
        // The master: each group's worth and what it takes of each side row,
        // summed over its nodes, and what it needs of other groups.
        let mut objective = vec![0.0; groups];
        let mut takes: Vec<Vec<(usize, f64)>> = vec![Vec::new(); groups];
        let mut closed = vec![false; groups];
        for (index, block) in blocks.iter().enumerate() {
            for period in 0..spans {
                let node = index * spans + period;
                let held = group[node] as usize;
                if forbidden(node) {
                    closed[held] = true;
                }
                let later = if period + 1 < spans { block.value * discount[period + 1] } else { 0.0 };
                objective[held] += block.value * discount[period] - later;
                takes[held].extend_from_slice(uses_of(index, period));
                if period + 1 < spans {
                    takes[held].extend(uses_of(index, period + 1).iter().map(|&(row, used)| (row, -used)));
                }
            }
        }
        for entries in &mut takes {
            entries.sort_unstable_by_key(|(row, _)| *row);
            entries.dedup_by(|(row, used), (kept, sum)| {
                let same = row == kept;
                if same {
                    *sum += *used;
                }
                same
            });
            entries.retain(|(_, used)| used.abs() > 1e-12);
        }
        let mut needs: Vec<(u32, u32)> = edges
            .iter()
            .map(|(from, to)| (group[*from as usize], group[*to as usize]))
            .filter(|(from, to)| from != to)
            .collect();
        needs.sort_unstable();
        needs.dedup();
        // Only the pairs no others imply, as each closure's split leaves
        // chains of groups.
        if let Some(reduced) = reduce_needs(groups, &needs) {
            needs = reduced;
        }
        // The master is solved as a mix of plans, each a closure of the
        // group graph: a small program over the side rows, and a closure of
        // the groups at its prices for the next plan, until none is worth
        // more than the mix. Every mix can be dug, so the budget running out
        // part way keeps the best so far. The plans already found are the
        // same ground dug by the same periods in the new groups, and this
        // round's closure is one more.
        for plan in &mut plans {
            plan.held = parent.iter().map(|old| plan.held[*old as usize]).collect();
        }
        let digs = |held: &[bool]| -> Plan {
            let mut uses = vec![0.0; limits.len()];
            let mut size = vec![0.0_f64; limits.len()];
            let mut worth = 0.0;
            for (at, _) in held.iter().enumerate().filter(|(_, held)| **held) {
                worth += objective[at];
                for (row, used) in &takes[at] {
                    uses[*row] += used;
                    size[*row] += used.abs();
                }
            }
            // A group's period and the next cancel in a closure holding both:
            // what rounding leaves of that is nothing, and the master cannot
            // be solved with it.
            for (used, size) in uses.iter_mut().zip(&size) {
                if used.abs() <= 1e-9 * size {
                    *used = 0.0;
                }
            }
            Plan {
                held: held.to_vec(),
                worth,
                uses,
                idle: 0,
            }
        };
        // The seed's side is its period and the node's: dug by the node's
        // period when the first is no later.
        let found: Vec<bool> = if sides == 2 {
            side.iter().map(|side| *side == 1).collect()
        } else {
            side.iter().map(|side| side / spans <= side % spans).collect()
        };
        if !plans.iter().any(|plan| plan.held == found) {
            plans.push(digs(&found));
        }
        // The last plan, as the closures its shares stack up from: the
        // nodes dug to at least each share it gives. Ground the edit closed
        // and what waits on it are left out of each.
        if let Some(prior) = prior_share.take() {
            let mut of_group = vec![0.0; groups];
            for (node, at) in group.iter().enumerate() {
                of_group[*at as usize] = prior[node];
            }
            let mut levels: Vec<f64> = of_group.iter().copied().filter(|share| *share > 1e-6).collect();
            levels.sort_by(f64::total_cmp);
            levels.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            if levels.len() > PRIOR_LEVELS {
                levels = (0..PRIOR_LEVELS).map(|at| levels[at * levels.len() / PRIOR_LEVELS]).collect();
            }
            let mut needed_by: Vec<Vec<u32>> = vec![Vec::new(); groups];
            for (from, to) in &needs {
                needed_by[*to as usize].push(*from);
            }
            for level in levels {
                let mut held: Vec<bool> = of_group.iter().zip(&closed).map(|(share, closed)| *share >= level - 1e-9 && !closed).collect();
                // Whatever needs a group left out is left out too.
                let mut stack: Vec<usize> = (0..groups).filter(|at| !held[*at]).collect();
                while let Some(at) = stack.pop() {
                    for from in &needed_by[at] {
                        if std::mem::replace(&mut held[*from as usize], false) {
                            stack.push(*from as usize);
                        }
                    }
                }
                if held.iter().any(|held| *held) && !plans.iter().any(|plan| plan.held == held) {
                    plans.push(digs(&held));
                }
            }
        }
        let graph = mineflow::Precedence::explicit(groups as i64, &needs.iter().map(|(from, to)| (i64::from(*from), i64::from(*to))).collect::<Vec<_>>()).ok()?;
        let mut inner = mineflow::Solver::new(&graph, &vec![0; groups]).ok()?;
        let mut basis: Option<solver::Basis> = None;
        let mut mixed: Option<Vec<f64>> = None;
        let mut stop = false;
        // The prices that gave the lowest bound on these groups, and it.
        let mut centre: Option<(Vec<f64>, f64)> = None;
        // Near enough the best on these groups: a fraction of how far the
        // plan is from the bound, as the next split moves the best anyway.
        let enough = if bound.is_finite() { INNER * (bound - value).max(0.0) } else { 1e-3 * value.abs() }.max(IMPROVEMENT * value.abs().max(1.0));
        loop {
            // In units the solver's tolerances suit: the plans' worth and
            // each row's largest amount as one.
            let unit = plans.iter().map(|plan| plan.worth.abs()).chain(hinges.iter().map(|hinge| hinge.cost)).fold(1e-9, f64::max);
            let row_unit: Vec<f64> = (0..limits.len())
                .map(|row| plans.iter().map(|plan| plan.uses[row].abs()).fold(limits[row].abs(), f64::max))
                .map(|largest| if largest > 1e-9 { 1.0 / largest } else { 1.0 })
                .collect();
            let mut master = solver::Problem::new(solver::Sense::Maximise);
            // How far each period's feed is past each hinge, at its cost.
            let mut excess: BTreeMap<usize, usize> = BTreeMap::new();
            for hinge in &hinges {
                excess.insert(hinge.row, master.add_col(-hinge.cost / unit, 0.0, f64::INFINITY));
            }
            let first = master.num_cols();
            for plan in &plans {
                master.add_col(plan.worth / unit, 0.0, f64::INFINITY);
            }
            for (row, limit) in limits.iter().enumerate() {
                let scale = row_unit[row];
                master.add_row(
                    f64::NEG_INFINITY,
                    limit * scale,
                    plans
                        .iter()
                        .enumerate()
                        .filter(|(_, plan)| plan.uses[row] != 0.0)
                        .map(|(at, plan)| (first + at, plan.uses[row] * scale))
                        .chain(excess.get(&row).map(|col| (*col, -scale))),
                );
            }
            master.add_row(f64::NEG_INFINITY, 1.0, (0..plans.len()).map(|at| (first + at, 1.0)));
            // From the last basis, the new plan out of it.
            if let Some(basis) = basis.as_mut() {
                basis.col_status.resize(master.num_cols(), solver::BasisStatus::Lower);
            }
            // A new plan leaves the last mix as it was, so the primal method
            // goes on from it. Presolve gains nothing on a program this
            // small and has failed to give one back solved.
            let options = solver::Options {
                time_limit: match request.stop {
                    PlanStop::Seconds(seconds) => (seconds - started.elapsed().as_secs_f64()).max(0.1),
                    _ => solver::Options::default().time_limit,
                },
                presolve: false,
                simplex_strategy: solver::SimplexStrategy::Primal,
                ..solver::Options::default()
            };
            let mut solution = solver::solve_from(&master, &options, basis.as_ref(), None);
            if solution.status != solver::Status::Optimal && basis.is_some() {
                solution = solver::solve(&master, &options);
            }
            if solution.status != solver::Status::Optimal {
                solution = solver::solve(
                    &master,
                    &solver::Options {
                        simplex_strategy: solver::SimplexStrategy::Dual,
                        ..options.clone()
                    },
                );
            }
            if solution.status != solver::Status::Optimal {
                // The last mix stands; the time limit ends the plan.
                stop = out_of_time();
                break;
            }
            basis = solution.basis.clone();
            value = solution.objective * unit;
            mixed = Some(solution.col_value[first..].to_vec());
            // A limit's price is what one more tonne of it would add.
            for ((price, dual), scale) in prices.iter_mut().zip(&solution.row_dual).zip(&row_unit) {
                *price = dual.abs() * unit * scale;
            }
            // Never more than the hinge costs: past that the excess pays it.
            for hinge in &hinges {
                prices[hinge.row] = prices[hinge.row].min(hinge.cost);
            }
            if out_of_time() || cancelled() {
                stop = true;
                break;
            }
            // The best plan at prices between the program's and those that
            // gave the lowest bound yet (Wentges): the program's swing from
            // one extreme to another as plans come in, and a closure at them
            // overshoots. When that plan is worth nothing at the program's
            // own prices, the closure at those is the next.
            let allowance = |at: &[f64]| limits.iter().zip(at).map(|(limit, price)| limit * price).sum::<f64>();
            let mut weight = SMOOTHING;
            let mut found: Option<Plan> = None;
            loop {
                let at: Vec<f64> = match &centre {
                    Some((centre, _)) if weight > 0.0 => centre.iter().zip(&prices).map(|(centre, price)| weight * centre + (1.0 - weight) * price).collect(),
                    _ => prices.clone(),
                };
                let gains: Vec<f64> = (0..groups)
                    .map(|group| objective[group] - takes[group].iter().map(|(row, used)| at[*row] * used).sum::<f64>())
                    .collect();
                // As integers well inside what the closure solver sums exactly,
                // and none as far down as a closed group.
                let total: f64 = gains.iter().map(|gain| gain.abs()).sum::<f64>().max(1e-9);
                let scale = 1e16 / total;
                let values: Vec<i64> = gains
                    .iter()
                    .zip(&closed)
                    .map(|(gain, closed)| if *closed { FORBIDDEN } else { (gain * scale).round() as i64 })
                    .collect();
                inner.update_values(&values).ok()?;
                inner.solve_with_cancel(cancelled).ok()?;
                let held = inner.pit().ok()?.as_slice().to_vec();
                let gain: f64 = gains.iter().zip(&held).filter(|(_, held)| **held).map(|(gain, _)| gain).sum();
                let low = allowance(&at) + gain.max(0.0);
                if centre.as_ref().is_none_or(|(_, lowest)| low < *lowest) {
                    centre = Some((at, low));
                }
                let plan = digs(&held);
                let reduced = plan.worth - plan.uses.iter().zip(&prices).map(|(used, price)| used * price).sum::<f64>() - (value - allowance(&prices));
                if reduced > IMPROVEMENT * value.abs().max(1.0) && !plans.iter().any(|kept| kept.held == plan.held) {
                    found = Some(plan);
                    break;
                }
                if weight == 0.0 || centre.is_none() {
                    break;
                }
                weight = 0.0;
            }
            let lowest = centre.as_ref().map_or(f64::INFINITY, |(_, lowest)| *lowest);
            let Some(plan) = found.filter(|_| lowest - value > enough) else {
                break;
            };
            plans.push(plan);
        }
        // Without a master solved on this split, the last shares stand in it.
        let Some(mixed) = mixed else {
            if stop {
                break;
            }
            continue;
        };
        share = vec![0.0; groups];
        for (plan, amount) in plans.iter_mut().zip(&mixed) {
            for (at, _) in plan.held.iter().enumerate().filter(|(_, held)| **held) {
                share[at] += amount;
            }
            plan.idle = if *amount > 1e-12 { 0 } else { plan.idle + 1 };
        }
        // Plans the mix has long gone without only slow its program.
        plans.retain(|plan| plan.idle < IDLE_ROUNDS);
        if stop {
            break;
        }
        // Each round's split can only add to the value; one that adds less
        // than the setting asks ends the plan.
        if let PlanStop::Improvement(least) = request.stop {
            if last_value.is_some_and(|last| value - last < least * value.abs().max(1.0)) {
                break;
            }
            last_value = Some(value);
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
    let crushed_of = |block: &Block| block.crushed.iter().map(|(_, tonnes)| tonnes).sum::<f64>();
    // Each block's lean off the crushers' grade targets, for the order
    // within each period.
    let blend = {
        let mut aims: Vec<(usize, usize, f64, f64)> = Vec::new();
        for target in &input.grade_targets {
            let Some(crusher) = crusher_ids.iter().position(|id| *id == target.destination) else {
                continue;
            };
            if aims.iter().any(|(at, grade, _, _)| *at == crusher && *grade == target.grade) {
                continue;
            }
            let slope = target.specification.hinges().first().map_or(0.0, |(_, _, slope)| *slope);
            aims.push((crusher, target.grade, target.specification.target, slope));
        }
        let crushers: Vec<&super::Destination> = input.destinations.iter().filter(|entry| entry.kind == DestinationKind::Crusher).collect();
        let daily: Vec<f64> = crushers.iter().flat_map(|entry| entry.crusher_daily_t.iter().flatten().copied()).collect();
        let day_t = if daily.is_empty() {
            0.0
        } else {
            daily.iter().sum::<f64>() / daily.len() as f64 * crushers.len() as f64
        };
        (!aims.is_empty() && day_t > 0.0).then(|| Blend {
            lean: blocks
                .iter()
                .map(|block| {
                    aims.iter()
                        .map(|(crusher, grade, aim, slope)| {
                            let tonnes: f64 = block.crushed.iter().filter(|(at, _)| at == crusher).map(|(_, tonnes)| tonnes).sum();
                            let contained: f64 = block
                                .assay
                                .iter()
                                .filter(|(at, held, _)| at == crusher && held == grade)
                                .map(|(_, _, contained)| contained)
                                .sum();
                            (contained - aim * tonnes) * slope
                        })
                        .collect()
                })
                .collect(),
            day_t,
        })
    };
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
    // How far the feed placed so far is past its grade targets, at their
    // cost, carried from one period to the next.
    let mut running: Vec<f64> = blend.as_ref().and_then(|blend| blend.lean.first()).map_or_else(Vec::new, |first| vec![0.0; first.len()]);
    // How far off its targets the feed would be with a block's added.
    let score = |drift: &[f64], lean: Option<&Vec<f64>>| -> f64 { lean.map_or(0.0, |lean| drift.iter().zip(lean).map(|(drift, lean)| (drift + lean).abs()).sum()) };
    let add_lean = |drift: &mut [f64], lean: Option<&Vec<f64>>| {
        for (drift, lean) in drift.iter_mut().zip(lean.into_iter().flatten()) {
            *drift += lean;
        }
    };
    for period in 0..spans {
        let first = fill.order.len();
        let mut drift = running.clone();
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
        let mut tried = vec![false; candidates.len()];
        let mut next = 0;
        loop {
            if room <= 0.0 {
                break;
            }
            while next < candidates.len() && (tried[next] || fill.placed[candidates[next]]) {
                next += 1;
            }
            if next >= candidates.len() {
                break;
            }
            // Of the next few the plan is surest of, the one that brings the
            // feed nearest its grade targets: the plan holds each period's
            // average on them with a share of many blocks, rich and lean,
            // and whole blocks taken by how sure the plan is of each alone
            // left the feed off them for quarters at a time.
            let pick = match blend.as_ref() {
                Some(blend) => {
                    let score = |at: &usize| score(&drift, Some(&blend.lean[candidates[*at]]));
                    (next..candidates.len())
                        .filter(|at| !tried[*at] && !fill.placed[candidates[*at]])
                        .take(BLEND_CHOICES)
                        .min_by(|a, b| score(a).total_cmp(&score(b)))
                        .unwrap_or(next)
                }
                None => next,
            };
            tried[pick] = true;
            let index = candidates[pick];
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
                add_lean(&mut drift, blend.as_ref().map(|blend| &blend.lean[at]));
            }
        }
        // Then the rest of the period, by how sure the plan is of each.
        let mut heap: std::collections::BinaryHeap<(u64, std::cmp::Reverse<usize>, usize)> =
            fill.freed.drain(..).filter(|index| !fill.placed[*index]).map(|index| key(index, period)).collect();
        let mut held: Vec<usize> = Vec::new();
        let lean_of = |index: usize| blend.as_ref().map(|blend| &blend.lean[index]);
        while let Some(index) = next_feed_for_grade(&mut heap, &fill.placed, &|index| crushed_of(&blocks[index]) > 0.0, &|index| score(&drift, lean_of(index))) {
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
            add_lean(&mut drift, lean_of(index));
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
            while let Some(index) = next_feed_for_grade(&mut heap, &fill.placed, &|index| crushed_of(&blocks[index]) > 0.0, &|index| score(&drift, lean_of(index))) {
                if fill.placed[index] {
                    continue;
                }
                if room <= 0.0 || by[index][period] < 1e-6 {
                    held.push(index);
                    continue;
                }
                take(&blocks[index], &mut room, &mut other_left, &mut crushing);
                fill.place(index, &below);
                add_lean(&mut drift, lean_of(index));
                heap.extend(fill.freed.drain(..).map(|index| key(index, period)));
            }
        }
        fill.freed = held;
        running = drift;
        interleave(&mut fill.order[first..], blocks, &rank, &crushed_of, blend.as_ref());
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
    let summary = PlanSummary {
        blocks: count,
        periods: spans,
        rounds,
        seconds: started.elapsed().as_secs_f64(),
        value,
        bound,
        reused: false,
    };
    let memory = PlanMemory {
        key: problem,
        order: order.clone(),
        summary,
        dug_by: blocks
            .iter()
            .zip(&by)
            .filter_map(|(block, shares)| Some((stable(block)?, shares.iter().map(|share| *share as f32).collect())))
            .collect(),
        // The first rows are the loaders' tonnes, one a period.
        dig_prices: dig_prices(blocks, &by, &periods, &discount, &prices[..spans], &dig, request.discount_rate),
    };
    Some((order, summary, memory))
}

/// What a tonne of the automatic loaders' digging is worth in each period,
/// in that period's money, by when the period ends: the more of what the
/// plan's last prices make one more tonne of their capacity worth, and what
/// the hour a tonne takes is worth to digging in an order. The plan may put
/// off whichever block is worth least, so the first can be next to nothing
/// while the loaders are busy; an order cannot, and an hour taken from its
/// digging puts everything after it an hour later - all the plan digs from
/// then on discounted an hour more, and the last hour's digging past the
/// horizon.
fn dig_prices(blocks: &[Block], by: &[Vec<f64>], periods: &[(f64, f64)], discount: &[f64], prices: &[f64], capacity: &[f64], rate: f64) -> Vec<(f64, f64)> {
    let spans = periods.len();
    // The plan's discounted value dug in each period.
    let mut earned = vec![0.0; spans];
    for (block, shares) in blocks.iter().zip(by) {
        for period in 0..spans {
            let before = if period > 0 { shares[period - 1] } else { 0.0 };
            earned[period] += block.value * discount[period] * (shares[period] - before);
        }
    }
    let per_hour = (1.0 + rate.max(0.0)).ln() / YEAR_H;
    let last = spans - 1;
    let last_hour = earned[last].max(0.0) / (periods[last].1 - periods[last].0).max(1.0);
    let mut after = 0.0;
    let mut out = vec![(0.0, 0.0); spans];
    for period in (0..spans).rev() {
        after += earned[period];
        let (start_h, end_h) = periods[period];
        let tonnes_h = capacity[period] / (end_h - start_h).max(1.0);
        let delay = if tonnes_h > 0.0 { (per_hour * after.max(0.0) + last_hour) / tonnes_h } else { 0.0 };
        let marginal = prices.get(period).copied().unwrap_or(0.0);
        out[period] = (end_h, if discount[period] > 0.0 { marginal.max(delay) / discount[period] } else { 0.0 });
    }
    out
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

/// How each block's crusher feed leans off the crushers' grade targets,
/// and how much feed the crushers take in a day: what the order keeps in
/// step so each day's blend is near its target.
struct Blend {
    /// By block: for each crusher and grade a target prices, what its feed
    /// contains past the target's grade, at what the target charges for it.
    lean: Vec<Vec<f64>>,
    day_t: f64,
}

/// The next block off `heap`, but of crusher feed, whichever of the next
/// few feed blocks on it brings the feed nearest its grade targets; the
/// rest go back.
fn next_feed_for_grade(
    heap: &mut std::collections::BinaryHeap<(u64, std::cmp::Reverse<usize>, usize)>,
    placed: &[bool],
    feed: &dyn Fn(usize) -> bool,
    score: &dyn Fn(usize) -> f64,
) -> Option<usize> {
    let top = heap.pop()?;
    if placed[top.2] || !feed(top.2) {
        return Some(top.2);
    }
    let mut popped = vec![top];
    let mut feeds = 1;
    while feeds < BLEND_CHOICES {
        let Some(entry) = heap.pop() else { break };
        if !placed[entry.2] && feed(entry.2) {
            feeds += 1;
        }
        popped.push(entry);
    }
    let chosen = popped
        .iter()
        .enumerate()
        .filter(|(_, entry)| !placed[entry.2] && feed(entry.2))
        .min_by(|(_, a), (_, b)| score(a.2).total_cmp(&score(b.2)))
        .map_or(0, |(at, _)| at);
    let picked = popped.swap_remove(chosen);
    heap.extend(popped.into_iter().filter(|entry| !placed[entry.2]));
    Some(picked.2)
}

/// A period's blocks put in the order they are dug in: crusher feed and the
/// rest in step, each as far through its own tonnes as the other, so the
/// crusher is fed through the period rather than all at its start - it
/// takes only so much a day - and nothing before what it waits on. Of the
/// feed free to come next, the one that brings the last day or so of feed
/// nearest its grade targets: the targets are priced day by day, and what
/// a period averages does not stop its days from swinging.
fn interleave(slice: &mut [usize], blocks: &[Block], rank: &[usize], crushed_of: &dyn Fn(&Block) -> f64, blend: Option<&Blend>) {
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
    // What is free to come next, feed and the rest.
    let mut ready: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
    for (at, count) in waiting.iter().enumerate() {
        if *count == 0 {
            ready[usize::from(feed(at))].push(at);
        }
    }
    let width = blend.map_or(0, |blend| blend.lean.first().map_or(0, Vec::len));
    let mut running = vec![0.0; width];
    let (mut fed, mut other) = (0.0, 0.0);
    let mut out = Vec::with_capacity(slice.len());
    while out.len() < slice.len() {
        let want_feed = fed / feed_total <= other / other_total;
        let pick = if ready[usize::from(want_feed)].is_empty() {
            usize::from(!want_feed)
        } else {
            usize::from(want_feed)
        };
        let first_listed = |list: &[usize]| list.iter().enumerate().min_by_key(|(_, at)| rank[slice[**at]]).map(|(position, _)| position);
        let chosen = match blend.filter(|_| pick == 1 && width > 0) {
            Some(blend) => {
                let off = |at: usize| {
                    let lean = &blend.lean[slice[at]];
                    running.iter().zip(lean).map(|(run, lean)| (run + lean).abs()).sum::<f64>()
                };
                ready[1]
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| off(**a).total_cmp(&off(**b)).then(rank[slice[**a]].cmp(&rank[slice[**b]])))
                    .map(|(position, _)| position)
            }
            None => first_listed(&ready[pick]),
        };
        let Some(position) = chosen else { break };
        let at = ready[pick].swap_remove(position);
        let tonnes = blocks[slice[at]].tonnes;
        if pick == 1 {
            fed += tonnes;
            if let Some(blend) = blend.filter(|_| width > 0) {
                // The blend so far, older feed fading over about a day.
                let keep = (-tonnes / blend.day_t.max(1.0)).exp();
                for (run, lean) in running.iter_mut().zip(&blend.lean[slice[at]]) {
                    *run = *run * keep + lean;
                }
            }
        } else {
            other += tonnes;
        }
        out.push(slice[at]);
        for under in &below[at] {
            waiting[*under] -= 1;
            if waiting[*under] == 0 {
                ready[usize::from(feed(*under))].push(*under);
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
        // Only bars that may take it: one limited to other pits, benches or
        // flitches is never dealt it.
        let may = |bar: usize| request.allowed.get(&bars[bar].task).is_none_or(|allowed| allowed.contains(&ground));
        let Some(soonest) = (0..bars.len()).filter(|bar| may(*bar)).min_by(|a, b| start(*a).total_cmp(&start(*b))) else {
            finished[*index] = ready;
            continue;
        };
        let group = request.groups.get(&ground).copied();
        let bar = group
            .and_then(|group| holder.get(&group).copied())
            .filter(|held| may(*held) && start(*held) <= start(soonest) + block.tonnes / bars[*held].tph)
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
    // A day of the crushers' feed, on average.
    let feed_day_t: f64 = input
        .destinations
        .iter()
        .filter(|destination| destination.kind == DestinationKind::Crusher)
        .map(|destination| {
            let days: Vec<f64> = destination.crusher_daily_t.iter().flatten().copied().collect();
            if days.is_empty() { 0.0 } else { days.iter().sum::<f64>() / days.len() as f64 }
        })
        .sum();
    for bar in bars {
        if let Some(task) = input.tasks.iter_mut().find(|task| task.id == bar.task) {
            task.kind = TaskKind::Dig { sequence: bar.sequence };
            // The crushers' grade targets are priced day by day, so the
            // loader blends among a day of its digging, or a day of the
            // crushers' feed where that takes more of its ground.
            task.lookahead_t = bar.tph * DAY_H;
            task.lookahead_feed_t = feed_day_t;
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
