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
/// A closure must beat the combination by this share of its value to be
/// worth another round.
const IMPROVEMENT: f64 = 1e-6;
/// The weight of a node no closure may hold: far past anything the scaled
/// values sum to, and far inside what the closure solver adds up exactly.
const FORBIDDEN: i64 = -(1 << 56);
/// Hours in a year, for discounting.
const YEAR_H: f64 = 8760.0;

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
pub(crate) fn assign(input: &mut BlendInput, request: &PlanRequest<'_>, cancelled: &dyn Fn() -> bool) -> PlanSummary {
    let Some(first) = input.tasks.iter().find(|task| request.tasks.contains(&task.id)) else {
        return PlanSummary::default();
    };
    let TaskKind::Dig { sequence } = &first.kind else { return PlanSummary::default() };
    let grounds: Vec<GroundId> = sequence.clone();
    let blocks = blocks(input, request, &grounds);
    let (order, summary) = match planned_order(input, request, &blocks, cancelled) {
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
    // can fetch from any automatic loader, and where that sends it.
    let mut best: BTreeMap<(GroundId, super::MaterialId), (f64, DestinationId)> = BTreeMap::new();
    for movement in &input.movements {
        let SourceId::Ground(ground) = movement.source else { continue };
        if !position.contains_key(&ground) || !loaders.contains(&movement.loader) {
            continue;
        }
        let value = movement.value_per_tonne().unwrap_or(0.0);
        best.entry((ground, movement.material))
            .and_modify(|held| {
                if value > held.0 {
                    *held = (value, movement.destination);
                }
            })
            .or_insert((value, movement.destination));
    }
    let sources: BTreeMap<GroundId, &super::GroundSource> = input.ground.iter().map(|source| (source.id, source)).collect();
    let lead: BTreeMap<GroundId, f64> = input.drill_blast.as_ref().map_or_else(BTreeMap::new, |chain| {
        chain
            .earliest_releases(&input.intervals)
            .into_iter()
            .filter(|(_, at)| *at < super::blended::drill_blast::NEVER)
            .collect()
    });
    let mut out: Vec<Block> = grounds
        .iter()
        .map(|ground| {
            let source = sources.get(ground);
            let tonnes = source.map_or(0.0, |source| source.tonnes_t);
            let mut value = 0.0;
            let mut crushed: Vec<(usize, f64)> = Vec::new();
            for share in source.into_iter().flat_map(|source| source.material.iter()) {
                let Some((per_tonne, destination)) = best.get(&(*ground, share.material)) else {
                    continue;
                };
                let share_t = share.fraction * tonnes;
                value += per_tonne * share_t;
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
                above: Vec::new(),
                lead_h: lead.get(ground).copied().unwrap_or(0.0),
                earliest_h: 0.0,
            }
        })
        .collect();
    // What holds each block. A dependency capture put in the chain holds
    // its block until the ground above it is dug. A blast's ground waits on
    // the blast's clearance, which waits on the ground above the blast -
    // and, because a machine bar works its blasts strictly in order, on the
    // clearance of every blast ahead of it in a bar: a plan that ignored
    // that would send a loader to ground no drill can reach yet.
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
            }
            out.push(Block {
                ground: None,
                tonnes: 0.0,
                value: 0.0,
                crushed: Vec::new(),
                above: over,
                lead_h: 0.0,
                earliest_h: 0.0,
            });
        }
        for task in chain.tasks.iter().filter(|task| !task.delay && task.follow.is_none()) {
            let listed: Vec<usize> = task.sequence.iter().filter_map(|blast| clearance.get(blast).copied()).collect();
            for pair in listed.windows(2) {
                out[pair[1]].above.push(pair[0]);
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
    // A place for each block after everything it waits on. Blasts authored
    // to wait on one another in a ring have none, and neither has a plan.
    let rank = topological_rank(blocks)?;
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
    let opens: Vec<usize> = blocks.iter().zip(&lag).map(|(block, lag)| whole(block.earliest_h).max(*lag)).collect();
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
    let started = std::time::Instant::now();
    let (mut value, mut bound, mut rounds) = (0.0, f64::INFINITY, 0);
    while rounds < MAX_ROUNDS && started.elapsed() < request.budget {
        if cancelled() {
            return None;
        }
        // The best closure at these prices. A block dug in a period earns
        // its discounted value less the capacity it pays for; as "dug by"
        // nodes that is the difference between one period and the next.
        let gain = |block: &Block, period: usize| {
            let mut gain = block.value * discount[period] - prices[period] * block.tonnes;
            for (crusher, tonnes) in &block.crushed {
                if let Some(row) = crush_row.get(&(*crusher, period)) {
                    gain -= prices[*row] * tonnes;
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
        let priced = worth(&column) - used.iter().zip(&prices).map(|(used, price)| used * price).sum::<f64>();
        let allowance: f64 = limits.iter().zip(&prices).map(|(limit, price)| limit * price).sum();
        bound = bound.min(priced + allowance);
        if bound - value <= IMPROVEMENT * value.abs().max(1.0) || columns.contains(&column) {
            break;
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
            return None;
        }
        value = solution.objective;
        theta = solution.col_value;
        // A limit's price is what one more tonne of it would add.
        for (price, dual) in prices.iter_mut().zip(&solution.row_dual) {
            *price = dual.abs();
        }
    }
    // The period each block is expected in, which is no sooner than that of
    // anything it waits on, since that holds in every closure. Ties go by
    // an order that itself puts each block after what it waits on.
    let expected: Vec<f64> = (0..count)
        .map(|index| columns.iter().zip(&theta).map(|(column, weight)| weight * f64::from(column[index])).sum())
        .collect();
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by(|a, b| expected[*a].total_cmp(&expected[*b]).then(rank[*a].cmp(&rank[*b])));
    Some((
        order,
        PlanSummary {
            blocks: count,
            periods: spans,
            rounds,
            value,
            bound,
        },
    ))
}

/// Each block's place in an order that puts it after everything it waits
/// on, earlier-listed blocks first where they are free to be; `None` when
/// the dependencies form a ring.
fn topological_rank(blocks: &[Block]) -> Option<Vec<usize>> {
    let mut waiting: Vec<usize> = blocks.iter().map(|block| block.above.len()).collect();
    let mut below: Vec<Vec<usize>> = vec![Vec::new(); blocks.len()];
    for (index, block) in blocks.iter().enumerate() {
        for over in &block.above {
            below[*over].push(index);
        }
    }
    let mut ready: std::collections::BinaryHeap<std::cmp::Reverse<usize>> = waiting
        .iter()
        .enumerate()
        .filter(|(_, count)| **count == 0)
        .map(|(index, _)| std::cmp::Reverse(index))
        .collect();
    let mut rank = vec![usize::MAX; blocks.len()];
    let mut placed = 0;
    while let Some(std::cmp::Reverse(index)) = ready.pop() {
        rank[index] = placed;
        placed += 1;
        for under in &below[index] {
            waiting[*under] -= 1;
            if waiting[*under] == 0 {
                ready.push(std::cmp::Reverse(*under));
            }
        }
    }
    (placed == blocks.len()).then_some(rank)
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
