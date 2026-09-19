//! Deterministic loader dispatch over validated scheduling inputs.
//!
//! This module knows nothing about Solids caches or the Gantt. Its caller must
//! first resolve persistent ground references against one current run and
//! supply complete measured tonnes. In return it produces the only timed data
//! the Gantt and future schedule animation consume: execution segments,
//! explicit idle spans, and what each block had left when the run stopped.
//!
//! The evaluator is a **joint event-driven simulation** over one shared
//! depletion ledger, which is the whole point of it:
//!
//! - **Remaining material belongs to the block**, not to a bar or a sequence.
//!   Every authored reference that resolves to the same ground shares one
//!   balance, seeded once however many bars and loaders name it.
//! - **One instant, all loaders.** Every worked block is decremented together
//!   and every loader is reassigned together. Advancing one loader to its next
//!   event before the others would let it claim material a second loader was
//!   working at the same instant - an advantage the order of a list must not
//!   confer.
//! - **A block worked by several loaders depletes at the sum of their rates**,
//!   and each loader's own contribution is recorded as its own segment. The
//!   segments over one block sum to that block's depletion and never exceed
//!   what it started with.
//! - **Windows are start-inclusive and end-exclusive**, so a bar that ends
//!   where another begins is not worked twice at the boundary instant.
//!
//! The simulation is resumable - [`DispatchRun::advance`] does a bounded
//! number of events and hands back control - so an explicit Run can be
//! cancelled part way through and publish nothing.

use std::collections::{HashMap, HashSet};

use super::{BarId, CompiledRateCalendar, DigBlockRef, LoaderAgentId, WorkWindow};
use crate::model::DigBlockId;

/// Tonnes below which a balance is treated as gone.
///
/// Not a tolerance on the answer: depletion is computed exactly and then
/// flushed to zero here, so accumulated floating-point dust cannot leave a
/// block holding a millionth of a tonne and generate an event at every
/// instant forever.
const TONNE_EPSILON: f64 = 1e-9;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DispatchAgent {
    pub(crate) agent: LoaderAgentId,
    /// Class rate retained for input validation and context.
    pub(crate) rate_tph: f64,
    pub(crate) calendar: CompiledRateCalendar,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DispatchBlock {
    pub(crate) block: DigBlockRef,
    /// Identity in the run named by [`DispatchInput::generation`]. This is
    /// what two authored references sharing one piece of ground have in
    /// common, and therefore what the shared balance is keyed by; the
    /// persistent reference is what leaves the module.
    pub(crate) resolved: DigBlockId,
    pub(crate) tonnes: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DispatchBar {
    pub(crate) bar: BarId,
    pub(crate) agent: LoaderAgentId,
    pub(crate) priority: u32,
    pub(crate) window: WorkWindow,
    pub(crate) blocks: Vec<DispatchBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DispatchInput {
    pub(crate) generation: u64,
    /// Where the run is asked to stop, in hours, or `None` to run to
    /// completion. A run stopped here keeps everything it calculated and
    /// distinguishes executable remaining work from material stranded behind
    /// closed work windows.
    pub(crate) horizon_limit_h: Option<f64>,
    pub(crate) agents: Vec<DispatchAgent>,
    pub(crate) bars: Vec<DispatchBar>,
}

/// One loader's own contribution to one block over one interval.
///
/// Never a whole block and never a whole bar: the interval is bounded by the
/// events either side of it, so a loader joining or leaving a shared block
/// splits the band there and nowhere else.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExecutionSegment {
    pub(crate) agent: LoaderAgentId,
    pub(crate) bar: BarId,
    pub(crate) block: DigBlockRef,
    pub(crate) resolved: DigBlockId,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    /// This loader's own tonnes, not the block's depletion.
    pub(crate) tonnes: f64,
    /// This loader's effective rate over the segment.
    pub(crate) rate_tph: f64,
    /// What the block was depleting at over this interval - this loader's
    /// rate when it was alone on it, and the sum when it was not.
    pub(crate) combined_rate_tph: f64,
    /// The *other* loaders on this block over this interval, ascending.
    pub(crate) sharers: Vec<LoaderAgentId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IdleSegment {
    pub(crate) agent: LoaderAgentId,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
}

/// What one block started the run with and what it had left at the end.
///
/// The ledger is derived output like everything else here: a run reseeds from
/// the input reserves every time and never writes back into them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockBalance {
    pub(crate) resolved: DigBlockId,
    pub(crate) block: DigBlockRef,
    /// Every authored reference that resolved to this balance. References to
    /// the same ground may carry different pick anchors, so presentation must
    /// not rely on equality with whichever reference seeded the ledger first.
    references: Vec<DigBlockRef>,
    pub(crate) started_t: f64,
    pub(crate) remaining_t: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DispatchSchedule {
    /// The Solids run all resolved ground and tonnes came from.
    pub(crate) generation: u64,
    pub(crate) horizon_h: f64,
    pub(crate) execution: Vec<ExecutionSegment>,
    pub(crate) idle: Vec<IdleSegment>,
    /// Every resolved block the run touched, in first-seen order.
    pub(crate) balances: Vec<BlockBalance>,
    /// Why simulation stopped. This distinguishes a period boundary from
    /// material that no authored work window can reach.
    pub(crate) outcome: DispatchOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DispatchOutcome {
    /// Every distinct block in the shared ledger is empty.
    Exhausted,
    /// Material remains and at least one bar can work it after the requested
    /// period boundary.
    Limited,
    /// Material remains, but no bar has a future window in which to work it.
    Stranded,
}

impl DispatchSchedule {
    pub(crate) fn balance(&self, resolved: DigBlockId) -> Option<&BlockBalance> {
        self.balances.iter().find(|balance| balance.resolved == resolved)
    }

    /// What one bar's loader actually took out of the ground within it.
    pub(crate) fn bar_tonnes(&self, bar: BarId) -> f64 {
        self.execution.iter().filter(|segment| segment.bar == bar).map(|segment| segment.tonnes).sum()
    }

    /// When every distinct block named by a bar became empty in the shared
    /// ledger, including depletion contributed through another bar or loader.
    /// A block that started at zero is complete at the schedule origin.
    pub(crate) fn bar_completion_h(&self, blocks: &[DigBlockRef]) -> Option<f64> {
        let mut seen = HashSet::new();
        let mut completion = 0.0_f64;
        for block in blocks {
            let balance = self.balances.iter().find(|balance| balance.references.contains(block))?;
            if !seen.insert(balance.resolved) {
                continue;
            }
            if balance.remaining_t > 0.0 {
                return None;
            }
            if balance.started_t > 0.0 {
                let emptied = self
                    .execution
                    .iter()
                    .filter(|segment| segment.resolved == balance.resolved)
                    .map(|segment| segment.end_h)
                    .max_by(f64::total_cmp)?;
                completion = completion.max(emptied);
            }
        }
        (!seen.is_empty()).then_some(completion)
    }

    /// What this bar's ground still holds at the end of the run: the blocks
    /// it names that are not empty, and their remaining tonnes.
    ///
    /// Described as material left in the block rather than work that was
    /// lost, because that is what it is - any bar referencing that ground,
    /// including one worked by another machine, may take it later.
    pub(crate) fn bar_left_behind(&self, blocks: &[DigBlockRef]) -> f64 {
        let mut seen = HashSet::new();
        self.balances
            .iter()
            .filter(|balance| balance.references.iter().any(|reference| blocks.contains(reference)) && seen.insert(balance.resolved))
            .map(|balance| balance.remaining_t)
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DispatchError {
    DuplicateAgent(LoaderAgentId),
    InvalidRate(LoaderAgentId),
    UnknownAgent {
        bar: BarId,
        agent: LoaderAgentId,
    },
    InvalidWindow(BarId),
    EmptyBar(BarId),
    InvalidTonnes {
        bar: BarId,
        block: usize,
    },
    /// Two references to one piece of ground arrived with different tonnages.
    /// They are measured from one snapshot through one field, so this cannot
    /// happen from a consistent input - and a shared balance seeded from
    /// whichever was seen first would be a number nobody chose.
    InconsistentTonnes {
        block: DigBlockId,
    },
    /// An event time that does not advance the clock: a duration too small to
    /// be represented at the instant it starts from. Refused rather than
    /// rounded away, because rounding it away loses the material.
    ClockDidNotAdvance(BarId),
    /// More events than this input can legitimately produce. A degenerate
    /// input says so instead of spinning.
    IterationCap,
}

/// One loader's assignment at one instant.
#[derive(Clone, Copy, Debug)]
struct Assignment {
    agent: LoaderAgentId,
    rate_tph: f64,
    bar: BarId,
    block: DigBlockRef,
    resolved: DigBlockId,
}

/// One block's shared balance.
#[derive(Clone, Debug)]
struct Balance {
    block: DigBlockRef,
    references: Vec<DigBlockRef>,
    started_t: f64,
    remaining_t: f64,
}

/// A simulation in progress.
///
/// Held rather than run straight through so an explicit Run can be cancelled:
/// [`Self::advance`] does a bounded number of events and returns, and dropping
/// this publishes nothing.
pub(crate) struct DispatchRun {
    input: DispatchInput,
    /// Agents in a fixed order with their compiled effective-rate curves.
    fleet: Vec<DispatchAgent>,
    /// Bars in their selection order, so the choice at every instant walks the
    /// same list.
    bars: Vec<usize>,
    balances: HashMap<DigBlockId, Balance>,
    /// First-seen order, so the reported ledger does not depend on hashing.
    ledger_order: Vec<DigBlockId>,
    time_h: f64,
    events: usize,
    cap: usize,
    execution: Vec<ExecutionSegment>,
    stopped_at_limit: bool,
    finished: bool,
}

impl DispatchRun {
    /// Validate the whole input and seed the ledger, or refuse all of it.
    ///
    /// Validation is all-or-nothing: no segment is returned when one input is
    /// unusable, so the display can never look like an invalid bar was
    /// silently omitted.
    pub(crate) fn start(input: DispatchInput) -> Result<Self, Vec<DispatchError>> {
        let mut errors = Vec::new();
        let mut rates: HashMap<LoaderAgentId, f64> = HashMap::new();
        for agent in &input.agents {
            if rates.insert(agent.agent, agent.rate_tph).is_some() {
                errors.push(DispatchError::DuplicateAgent(agent.agent));
            }
            if !agent.rate_tph.is_finite() || agent.rate_tph <= 0.0 {
                errors.push(DispatchError::InvalidRate(agent.agent));
            }
            let mut previous = 0.0;
            if !agent.calendar.initial_rate_tph.is_finite() || agent.calendar.initial_rate_tph < 0.0 {
                errors.push(DispatchError::InvalidRate(agent.agent));
            }
            for change in &agent.calendar.changes {
                if !change.at_h.is_finite() || change.at_h <= previous || !change.rate_tph.is_finite() || change.rate_tph < 0.0 {
                    errors.push(DispatchError::InvalidRate(agent.agent));
                    break;
                }
                previous = change.at_h;
            }
        }
        for bar in &input.bars {
            if !rates.contains_key(&bar.agent) {
                errors.push(DispatchError::UnknownAgent { bar: bar.bar, agent: bar.agent });
            }
            if !bar.window.is_valid() {
                errors.push(DispatchError::InvalidWindow(bar.bar));
            }
            if bar.blocks.is_empty() {
                errors.push(DispatchError::EmptyBar(bar.bar));
            }
            for (index, block) in bar.blocks.iter().enumerate() {
                if !block.tonnes.is_finite() || block.tonnes < 0.0 {
                    errors.push(DispatchError::InvalidTonnes { bar: bar.bar, block: index });
                }
            }
        }
        if let Some(limit) = input.horizon_limit_h
            && (!limit.is_finite() || limit <= 0.0)
        {
            errors.push(DispatchError::IterationCap);
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let mut fleet = input.agents.clone();
        fleet.sort_by_key(|agent| agent.agent);

        // Priority, then the window's start, then the bar's own identity:
        // the same total order the tie-break in the event search uses, so a
        // run is reproducible whatever order the caller assembled its bars in.
        let mut bars: Vec<usize> = (0..input.bars.len()).collect();
        bars.sort_by(|left, right| {
            let (left, right) = (&input.bars[*left], &input.bars[*right]);
            left.priority
                .cmp(&right.priority)
                .then(left.window.start_h.total_cmp(&right.window.start_h))
                .then(left.bar.cmp(&right.bar))
        });

        // One balance per resolved block, seeded once however many bars and
        // loaders reference it. Two bars sharing ground is valid and is the
        // whole reason the ledger is keyed this way.
        //
        // Seeded in the bars' *selection* order rather than the order the
        // caller happened to assemble them in, so the reported ledger is the
        // same run to run - the answer must not depend on how the input list
        // was built.
        let mut balances: HashMap<DigBlockId, Balance> = HashMap::new();
        let mut ledger_order = Vec::new();
        for bar in bars.iter().map(|index| &input.bars[*index]) {
            for block in &bar.blocks {
                match balances.get_mut(&block.resolved) {
                    Some(existing) if (existing.started_t - block.tonnes).abs() > TONNE_EPSILON => {
                        errors.push(DispatchError::InconsistentTonnes { block: block.resolved });
                    }
                    Some(existing) => {
                        if !existing.references.contains(&block.block) {
                            existing.references.push(block.block);
                        }
                    }
                    None => {
                        ledger_order.push(block.resolved);
                        balances.insert(
                            block.resolved,
                            Balance {
                                block: block.block,
                                references: vec![block.block],
                                started_t: block.tonnes,
                                remaining_t: block.tonnes,
                            },
                        );
                    }
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Every block can be exhausted once, every window has two edges, and
        // every compiled rate change can become an event.
        let changes = input
            .agents
            .iter()
            .try_fold(0_usize, |total, agent| total.checked_add(agent.calendar.changes.len()))
            .ok_or_else(|| vec![DispatchError::IterationCap])?;
        let terms = input
            .bars
            .len()
            .checked_mul(2)
            .and_then(|bars| bars.checked_add(ledger_order.len()))
            .and_then(|events| events.checked_add(changes))
            .ok_or_else(|| vec![DispatchError::IterationCap])?;
        let cap = terms
            .checked_mul(8)
            .and_then(|events| events.checked_add(16))
            .ok_or_else(|| vec![DispatchError::IterationCap])?;
        Ok(Self {
            input,
            fleet,
            bars,
            balances,
            ledger_order,
            time_h: 0.0,
            events: 0,
            cap,
            execution: Vec::new(),
            stopped_at_limit: false,
            finished: false,
        })
    }

    /// Advance without finalizing output, for background runners that measure
    /// simulation and final sorting/merging as separate phases.
    pub(crate) fn advance_events(&mut self, budget: usize) -> Result<bool, Vec<DispatchError>> {
        for _ in 0..budget {
            if self.finished {
                return Ok(true);
            }
            self.events += 1;
            if self.events > self.cap {
                return Err(vec![DispatchError::IterationCap]);
            }
            match self.step() {
                Ok(true) => self.finished = true,
                Ok(false) => {}
                Err(error) => return Err(vec![error]),
            }
        }
        Ok(self.finished)
    }

    pub(crate) fn complete(mut self) -> DispatchSchedule {
        debug_assert!(self.finished);
        self.finish()
    }

    /// One instant: assign every loader, find the next event, advance every
    /// worked block together. Returns whether the run is over.
    fn step(&mut self) -> Result<bool, DispatchError> {
        let assignments = self.assign();
        let window_next = self.next_window_boundary();
        let rate_next = self.next_rate_boundary();

        // Group the loaders by the block they are on, so a block depletes at
        // the sum of the rates actually on it and at no other rate.
        let mut groups: Vec<(DigBlockId, Vec<usize>)> = Vec::new();
        for (index, assignment) in assignments.iter().enumerate() {
            match groups.iter_mut().find(|(resolved, _)| *resolved == assignment.resolved) {
                Some((_, members)) => members.push(index),
                None => groups.push((assignment.resolved, vec![index])),
            }
        }

        let mut next = match (window_next, rate_next) {
            (Some(window), Some(rate)) => Some(window.min(rate)),
            (window, rate) => window.or(rate),
        };
        for (resolved, members) in &groups {
            let combined: f64 = members.iter().map(|index| assignments[*index].rate_tph).sum();
            let remaining = self.balances[resolved].remaining_t;
            let exhaust = self.time_h + remaining / combined;
            if !exhaust.is_finite() || exhaust <= self.time_h {
                return Err(DispatchError::ClockDidNotAdvance(assignments[members[0]].bar));
            }
            next = Some(next.map_or(exhaust, |current: f64| current.min(exhaust)));
        }
        if let Some(limit) = self.input.horizon_limit_h
            && limit > self.time_h
        {
            next = Some(next.map_or(limit, |current| current.min(limit)));
        }

        // Nothing is being worked and no window opens later: the run is over,
        // whatever is still in the ground.
        let Some(next) = next else {
            return Ok(true);
        };
        if next <= self.time_h {
            return Err(DispatchError::ClockDidNotAdvance(assignments.first().map_or(BarId(0), |assignment| assignment.bar)));
        }
        let elapsed = next - self.time_h;

        for (resolved, members) in &groups {
            let combined: f64 = members.iter().map(|index| assignments[*index].rate_tph).sum();
            let balance = self.balances.get_mut(resolved).expect("assigned blocks are in the ledger");
            // The block's depletion first, then each loader's share of it.
            // Taking each loader's own rate × elapsed independently would let
            // rounding push their sum past what the block held.
            let depletion = (combined * elapsed).min(balance.remaining_t);
            balance.remaining_t -= depletion;
            if balance.remaining_t < TONNE_EPSILON {
                balance.remaining_t = 0.0;
            }
            let mut sharers: Vec<LoaderAgentId> = members.iter().map(|index| assignments[*index].agent).collect();
            sharers.sort();
            let mut contributed = 0.0;
            for (position, index) in members.iter().enumerate() {
                let assignment = assignments[*index];
                let mut others = sharers.clone();
                others.retain(|agent| *agent != assignment.agent);
                // Allocate the shared depletion proportionally, with the last
                // contribution taking the floating-point remainder. Their sum
                // is therefore exactly the material removed from the ledger.
                let tonnes = if position + 1 == members.len() {
                    (depletion - contributed).max(0.0)
                } else {
                    let tonnes = depletion * assignment.rate_tph / combined;
                    contributed += tonnes;
                    tonnes
                };
                self.execution.push(ExecutionSegment {
                    agent: assignment.agent,
                    bar: assignment.bar,
                    block: assignment.block,
                    resolved: assignment.resolved,
                    start_h: self.time_h,
                    end_h: next,
                    tonnes,
                    rate_tph: assignment.rate_tph,
                    combined_rate_tph: combined,
                    sharers: others,
                });
            }
        }

        self.time_h = next;
        if let Some(limit) = self.input.horizon_limit_h
            && self.time_h >= limit
        {
            self.stopped_at_limit = true;
            return Ok(true);
        }
        Ok(false)
    }

    /// Every loader's block at the current instant, all chosen against the
    /// same ledger before any of them is applied.
    fn assign(&self) -> Vec<Assignment> {
        let mut assignments = Vec::new();
        for agent in &self.fleet {
            let rate_tph = agent.calendar.rate_at(self.time_h);
            if rate_tph <= 0.0 {
                continue;
            }
            let mut chosen = None;
            'bars: for index in &self.bars {
                let bar = &self.input.bars[*index];
                if bar.agent != agent.agent || !bar.window.contains(self.time_h) {
                    continue;
                }
                for block in &bar.blocks {
                    // A block already at zero is skipped here rather than
                    // taken and completed in no time: an assignment that can
                    // only produce a zero-length segment would make that
                    // block an event at the same instant forever.
                    if self.balances[&block.resolved].remaining_t > 0.0 {
                        chosen = Some(Assignment {
                            agent: agent.agent,
                            rate_tph,
                            bar: bar.bar,
                            block: block.block,
                            resolved: block.resolved,
                        });
                        break 'bars;
                    }
                }
            }
            if let Some(assignment) = chosen {
                assignments.push(assignment);
            }
        }
        assignments
    }

    /// The next instant any window opens or closes, strictly after now.
    fn next_window_boundary(&self) -> Option<f64> {
        self.input
            .bars
            .iter()
            .flat_map(|bar| [Some(bar.window.start_h), bar.window.end_h])
            .flatten()
            .filter(|instant| *instant > self.time_h)
            .min_by(f64::total_cmp)
    }

    fn next_rate_boundary(&self) -> Option<f64> {
        self.fleet.iter().filter_map(|agent| agent.calendar.next_change_after(self.time_h)).min_by(f64::total_cmp)
    }

    fn outcome(&self) -> DispatchOutcome {
        if self.balances.values().all(|balance| balance.remaining_t <= 0.0) {
            return DispatchOutcome::Exhausted;
        }
        let future_eligible = self.input.bars.iter().any(|bar| {
            let start = self.time_h.max(bar.window.start_h);
            let calendar = self.fleet.iter().find(|agent| agent.agent == bar.agent).map(|agent| &agent.calendar);
            bar.window.end_h.is_none_or(|end| end > start)
                && calendar.is_some_and(|calendar| calendar.has_positive_rate_between(start, bar.window.end_h))
                && bar.blocks.iter().any(|block| self.balances[&block.resolved].remaining_t > 0.0)
        });
        if self.stopped_at_limit && future_eligible {
            DispatchOutcome::Limited
        } else {
            DispatchOutcome::Stranded
        }
    }

    /// Merge the interval-by-interval output into bands, derive the idle
    /// spans from what is left, and put everything in one stable order.
    fn finish(&mut self) -> DispatchSchedule {
        let mut execution = std::mem::take(&mut self.execution);
        execution.sort_by(|left, right| left.agent.cmp(&right.agent).then(left.start_h.total_cmp(&right.start_h)).then(left.bar.cmp(&right.bar)));
        // Contiguous work on the same ground, at the same combined rate, by
        // the same loader is one band. A loader joining or leaving the block
        // changes the combined rate and therefore splits it, which is exactly
        // where the drawn band should change.
        let mut merged: Vec<ExecutionSegment> = Vec::with_capacity(execution.len());
        for segment in execution {
            match merged.last_mut() {
                Some(last)
                    if last.agent == segment.agent
                        && last.bar == segment.bar
                        && last.resolved == segment.resolved
                        && last.sharers == segment.sharers
                        && (last.rate_tph - segment.rate_tph).abs() <= f64::EPSILON
                        && (last.combined_rate_tph - segment.combined_rate_tph).abs() <= f64::EPSILON
                        && (last.end_h - segment.start_h).abs() <= f64::EPSILON =>
                {
                    last.end_h = segment.end_h;
                    last.tonnes += segment.tonnes;
                }
                _ => merged.push(segment),
            }
        }

        let horizon_h = merged
            .iter()
            .map(|segment| segment.end_h)
            .fold(0.0_f64, f64::max)
            .max(if self.stopped_at_limit { self.input.horizon_limit_h.unwrap_or(0.0) } else { 0.0 });

        // Idle is the complement of a loader's execution over the run, so it
        // is exactly the time it had no available work - never inferred from
        // where a bar happens to sit.
        let mut idle = Vec::new();
        for fleet_agent in &self.fleet {
            let agent = fleet_agent.agent;
            let mut spans: Vec<(f64, f64)> = merged
                .iter()
                .filter(|segment| segment.agent == agent)
                .map(|segment| (segment.start_h, segment.end_h))
                .collect();
            spans.sort_by(|left, right| left.0.total_cmp(&right.0));
            let mut cursor = 0.0_f64;
            for (start, end) in spans {
                if start > cursor {
                    idle.push(IdleSegment {
                        agent,
                        start_h: cursor,
                        end_h: start,
                    });
                }
                cursor = cursor.max(end);
            }
            if horizon_h > cursor {
                idle.push(IdleSegment {
                    agent,
                    start_h: cursor,
                    end_h: horizon_h,
                });
            }
        }

        merged.sort_by(|left, right| {
            left.start_h
                .total_cmp(&right.start_h)
                .then(left.agent.cmp(&right.agent))
                .then(left.bar.cmp(&right.bar))
                .then(left.end_h.total_cmp(&right.end_h))
        });
        idle.sort_by(|left, right| {
            left.start_h
                .total_cmp(&right.start_h)
                .then(left.agent.cmp(&right.agent))
                .then(left.end_h.total_cmp(&right.end_h))
        });
        let balances = self
            .ledger_order
            .iter()
            .map(|resolved| {
                let balance = &self.balances[resolved];
                BlockBalance {
                    resolved: *resolved,
                    block: balance.block,
                    references: balance.references.clone(),
                    started_t: balance.started_t,
                    remaining_t: balance.remaining_t,
                }
            })
            .collect();
        let outcome = self.outcome();
        DispatchSchedule {
            generation: self.input.generation,
            horizon_h,
            execution: merged,
            idle,
            balances,
            outcome,
        }
    }
}
