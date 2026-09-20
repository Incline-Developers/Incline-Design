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
//! When destination routing is on there is a **second shared ledger** beside
//! the ground, and it works the same way and for the same reasons:
//!
//! - **Capacity belongs to the destination**, not to a loader or a rule. Two
//!   loaders feeding one stockpile fill it at the sum of their rates, and
//!   neither may consume the free space the other is already using.
//! - **Routing is decided per assignment, against the ledger as it stands.**
//!   A loader is assigned a block only if *every* positive portion of that
//!   block has somewhere to go; otherwise that bar is blocked and the loader
//!   considers its other bars. The blocked block is never skipped within its
//!   own sequence - the authored dig order is the user's, and a scheduler that
//!   stepped over a block to keep busy would quietly re-order it.
//! - **A full destination is passed over; a non-matching one is not used.**
//!   Capacity decides between the destinations a rule allowed, and never
//!   decides which destinations were allowed.
//! - **A crusher's limit is a daily budget, not a rate.** It resets at each
//!   24-hour boundary and unused capacity does not carry forward, so a crusher
//!   is an event at those boundaries while it is being fed - and, when it is
//!   exhausted and work is waiting, the run jumps to the next period whose
//!   budget is positive rather than stepping through the days between.
//!
//! The simulation is resumable - [`DispatchRun::advance`] does a bounded
//! number of events and hands back control - so an explicit Run can be
//! cancelled part way through and publish nothing.

use std::collections::{HashMap, HashSet};

use super::{BarId, CalendarPeriod, CompiledRateCalendar, CrusherCalendar, DestinationId, DestinationKind, DigBlockRef, LoaderAgentId, RuleId, SCHEDULE_PERIOD_H, WorkWindow};
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

/// One destination the evaluator may deliver to, with the capacity it is
/// allowed to receive.
///
/// Capacity is tonnes of the schedule's nominated field. `None` is unlimited and
/// is deliberately not an infinity: a destination nobody has put a figure on and
/// one somebody typed a very large figure into are different statements.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DispatchDestination {
    pub(crate) id: DestinationId,
    pub(crate) kind: DestinationKind,
    /// Cumulative tonnes over the whole calculation, for a stockpile or dump.
    /// Every calculation starts them empty: opening inventories are not
    /// modelled, which is why the figure read back is *scheduled* inventory.
    pub(crate) capacity_t: Option<f64>,
    /// A crusher's daily budget and its sparse overrides. A crusher has no
    /// storage in this increment: what it receives is processed at once,
    /// subject to this.
    pub(crate) crusher: Option<CrusherCalendar>,
}

/// One portion of a block's material and the destinations it may reach.
///
/// The material itself is deliberately absent: what a grade or a rock type was
/// has already decided which rules matched, and the evaluator needs only the
/// ordered answer. Keeping the property payload here would put a copy of it
/// behind every loader and every execution segment.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DispatchPortion {
    /// Tonnes of this portion within its block, at the start of the run.
    pub(crate) tonnes: f64,
    /// Candidate destinations in rule order, each with the rule that offered
    /// it: indices into [`DispatchInput::destinations`]. Several rules may name
    /// one destination, and the order is kept exactly as authored.
    pub(crate) routes: Vec<(usize, RuleId)>,
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
    /// What this block is made of, and where each part of it may go when routing
    /// is on. Empty when it is off.
    ///
    /// Positions are shared ground: portion *i* of one reference names the same
    /// material as portion *i* of another reference to the same block, because
    /// both came from one measurement. The routes may differ between them, since
    /// a rule may name one loader and not another.
    pub(crate) portions: Vec<DispatchPortion>,
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
    /// Whether extracted material is routed. Off is the behaviour every project
    /// had before routing existed: the ground is dug and nothing is delivered.
    pub(crate) routing: bool,
    /// Every destination, in a fixed order the portions index into.
    pub(crate) destinations: Vec<DispatchDestination>,
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

/// One delivery: material of one block, dug by one loader in one bar, arriving
/// at one destination under one rule, over one interval.
///
/// Derived output like every segment beside it. Movements are never persisted
/// with the project: they belong to the captured inputs of the calculation that
/// produced them, and a new run produces its own.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Movement {
    pub(crate) agent: LoaderAgentId,
    pub(crate) bar: BarId,
    pub(crate) block: DigBlockRef,
    pub(crate) resolved: DigBlockId,
    pub(crate) destination: DestinationId,
    /// The rule that sent it here, so a delivery can be traced to the decision
    /// that made it rather than re-derived from the material.
    pub(crate) rule: RuleId,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    pub(crate) tonnes: f64,
}

/// What one destination received over the run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DestinationBalance {
    pub(crate) destination: DestinationId,
    pub(crate) kind: DestinationKind,
    pub(crate) capacity_t: Option<f64>,
    /// Cumulative tonnes received. For a stockpile this is also its scheduled
    /// inventory, because nothing is reclaimed in this increment.
    pub(crate) received_t: f64,
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
    /// Every delivery, in the same order the execution bands are in. Empty when
    /// routing is off.
    pub(crate) movements: Vec<Movement>,
    /// What each destination received. Present whenever routing is on, including
    /// destinations nothing reached, so a page can say "nothing went here"
    /// rather than leaving the row out.
    pub(crate) destination_balances: Vec<DestinationBalance>,
    /// Why simulation stopped. This distinguishes a period boundary from
    /// material that no authored work window can reach.
    pub(crate) outcome: DispatchOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DispatchOutcome {
    /// Every distinct block in the shared ledger is empty.
    Exhausted,
    /// Material remains and at least one bar can work it after the requested
    /// period boundary.
    Limited,
    /// Material remains, but no bar has a future window in which to work it.
    Stranded,
    /// Material remains and a bar could work it, but every destination its
    /// material may go to is full and no future capacity will appear.
    ///
    /// Deliberately its own answer rather than a kind of Stranded: the work is
    /// executable and the ground is reachable - what has run out is somewhere to
    /// put it, and the reason names which destination.
    CapacityBlocked {
        /// A short reason, e.g. "SP01 is full" or "CR01 daily limit reached".
        reason: String,
    },
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
    /// A portion names a destination that is not in the table it was built
    /// against. Only reachable from an inconsistent input.
    UnknownDestination {
        bar: BarId,
        block: usize,
    },
    /// A destination capacity or crusher budget that is negative or not finite.
    InvalidCapacity(DestinationId),
    /// Two references to one piece of ground disagree about what it is made of.
    /// Like [`Self::InconsistentTonnes`], they come from one measurement, so a
    /// ledger seeded from whichever was seen first would describe material
    /// nobody measured.
    InconsistentPortions {
        block: DigBlockId,
    },
    /// A block whose portions do not add up to its own tonnage. Refused rather
    /// than scaled: routing part of a block and digging all of it would deliver
    /// less than was extracted.
    UnbalancedPortions {
        bar: BarId,
        block: usize,
    },
}

/// What makes two contiguous deliveries one band: the rate material was
/// arriving at, as bits.
///
/// A band is read back per period by splitting it in proportion to duration,
/// which is only right while the rate is constant across it. Two intervals
/// therefore merge only when they delivered at the same rate - and the rate
/// changes whenever the loader's own rate changes *or* another portion of the
/// block starts or stops going to the same destination, which is exactly when a
/// merged band would otherwise start misplacing tonnes across a day boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BandKey(u64);

/// Where one portion of an assigned block is going at this instant.
#[derive(Clone, Copy, Debug)]
struct Routed {
    /// Index into [`DispatchInput::destinations`].
    destination: usize,
    rule: RuleId,
    /// This portion's fraction of the block, constant for the run: mining a
    /// block removes its portions proportionally, so their shares never move.
    share: f64,
}

/// One loader's assignment at one instant.
#[derive(Clone, Debug)]
struct Assignment {
    agent: LoaderAgentId,
    rate_tph: f64,
    bar: BarId,
    block: DigBlockRef,
    resolved: DigBlockId,
    /// Empty when routing is off.
    routed: Vec<Routed>,
}

/// One block's shared balance.
#[derive(Clone, Debug)]
struct Balance {
    block: DigBlockRef,
    references: Vec<DigBlockRef>,
    started_t: f64,
    remaining_t: f64,
    /// Each portion's constant fraction of the block. Empty when routing is off.
    ///
    /// Shares rather than remaining tonnes, because extraction is uniform: a
    /// block that is 30% qualifying material is still 30% qualifying material
    /// after 1,000 t has come out of it. Keeping remaining tonnes per portion
    /// would be the same numbers with an extra place to drift.
    shares: Vec<f64>,
}

/// One destination's shared ledger entry.
#[derive(Clone, Debug)]
struct DestinationLedger {
    id: DestinationId,
    kind: DestinationKind,
    capacity_t: Option<f64>,
    crusher: Option<CrusherCalendar>,
    /// Cumulative tonnes over the whole run.
    received_t: f64,
    /// Crusher only: the period its budget is being spent in, and how much of
    /// that budget has gone. Reset at each period boundary; unused capacity does
    /// not carry forward, which is what makes it a budget rather than a rate.
    period: u32,
    period_received_t: f64,
}

impl DestinationLedger {
    /// Tonnes it can still receive at this instant, or `None` for unlimited.
    fn free_t(&self) -> Option<f64> {
        match &self.crusher {
            Some(calendar) => calendar.limit_at(CalendarPeriod(self.period)).map(|limit| (limit - self.period_received_t).max(0.0)),
            None => self.capacity_t.map(|capacity| (capacity - self.received_t).max(0.0)),
        }
    }

    /// Whether it can take anything at all right now.
    ///
    /// Compared against the tonne epsilon rather than zero: a destination with a
    /// millionth of a tonne free would otherwise be assignable, produce an
    /// interval too short to advance the clock, and be assignable again.
    fn can_receive(&self) -> bool {
        self.free_t().is_none_or(|free| free > TONNE_EPSILON)
    }

    fn receive(&mut self, tonnes: f64) {
        self.received_t += tonnes;
        if self.crusher.is_some() {
            self.period_received_t += tonnes;
        }
    }

    /// Whether this destination can ever receive again after now.
    ///
    /// A stockpile or dump accumulates, so once it is full it stays full. A
    /// crusher's budget resets, so it can, provided some future period's budget
    /// is positive.
    fn has_future_capacity(&self) -> bool {
        match &self.crusher {
            None => self.can_receive(),
            Some(calendar) => self.can_receive() || next_positive_period(calendar, self.period).is_some(),
        }
    }

    /// A short reason this destination is refusing material, for the run's own
    /// termination message.
    fn blocked_reason(&self, name: &str) -> String {
        match self.kind {
            DestinationKind::Crusher => crate::i18n::tr!("dispatch-blocked-crusher", destination = name.to_owned()),
            _ => crate::i18n::tr!("dispatch-blocked-full", destination = name.to_owned()),
        }
    }
}

/// The first period after `period` whose crusher budget can receive anything.
///
/// The budget *resets* at every boundary, so the very next period is the first
/// candidate however the limit behaves - a crusher that has spent today's
/// allowance can receive again tomorrow even though nothing about its calendar
/// changed. Only a period whose budget is zero sends this to the calendar, and
/// then it jumps straight to the next period that differs rather than walking
/// the days between: a budget that only opens at day 900 is found in a step or
/// two.
fn next_positive_period(calendar: &CrusherCalendar, period: u32) -> Option<u32> {
    let mut at = CalendarPeriod(period.checked_add(1)?);
    // One step per stored override, plus its following period, plus the
    // immediate next one: the budget is constant everywhere else, so no further
    // period can differ from one already examined.
    for _ in 0..calendar.periods.len() * 2 + 2 {
        if calendar.limit_at(at).is_none_or(|limit| limit > TONNE_EPSILON) {
            return Some(at.0);
        }
        at = calendar.next_change_after(at)?;
    }
    None
}

/// Which period an hour falls in. Half-open, like every other period reading in
/// this module: hour 24 is the second period.
fn period_of(hour: f64) -> u32 {
    (hour / SCHEDULE_PERIOD_H).floor().max(0.0) as u32
}

/// A simulation in progress.
///
/// Held rather than run straight through so an explicit Run can be cancelled:
/// [`Self::advance_events`] does a bounded number of events and returns, and
/// dropping this publishes nothing.
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
    destinations: Vec<DestinationLedger>,
    time_h: f64,
    events: usize,
    cap: usize,
    execution: Vec<ExecutionSegment>,
    /// Movements with the same band discriminants the execution bands merge on,
    /// so a delivery band never spans an interval its own work was split at.
    movements: Vec<(Movement, BandKey)>,
    stopped_at_limit: bool,
    /// Set when an instant passed with a loader that had ground to work, a
    /// window to work it in, and nowhere to put what it would dig.
    capacity_blocked: Option<String>,
    finished: bool,
}

impl DispatchRun {
    /// Validate the whole input and seed both ledgers, or refuse all of it.
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
        for destination in &input.destinations {
            if destination.capacity_t.is_some_and(|capacity| !capacity.is_finite() || capacity < 0.0) {
                errors.push(DispatchError::InvalidCapacity(destination.id));
            }
            if let Some(calendar) = &destination.crusher
                && calendar.validate().is_err()
            {
                errors.push(DispatchError::InvalidCapacity(destination.id));
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
                if !input.routing {
                    continue;
                }
                // Routed material has to be all of the block's material: the
                // ground is depleted by the block's tonnage, and delivering a
                // different figure would put the two ledgers out of step.
                let mut portioned = 0.0;
                for portion in &block.portions {
                    if !portion.tonnes.is_finite() || portion.tonnes < 0.0 {
                        errors.push(DispatchError::UnbalancedPortions { bar: bar.bar, block: index });
                        continue;
                    }
                    portioned += portion.tonnes;
                    if portion.routes.is_empty() || portion.routes.iter().any(|(target, _)| *target >= input.destinations.len()) {
                        errors.push(DispatchError::UnknownDestination { bar: bar.bar, block: index });
                    }
                }
                let tolerance = PORTION_ABSOLUTE + PORTION_RELATIVE * block.tonnes;
                if (portioned - block.tonnes).abs() > tolerance {
                    errors.push(DispatchError::UnbalancedPortions { bar: bar.bar, block: index });
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
                let shares = shares_of(block);
                match balances.get_mut(&block.resolved) {
                    Some(existing) if (existing.started_t - block.tonnes).abs() > TONNE_EPSILON => {
                        errors.push(DispatchError::InconsistentTonnes { block: block.resolved });
                    }
                    // The portions are a property of the ground, not of the bar
                    // that names it: two references measured from one run agree,
                    // and a ledger that took whichever it saw first would
                    // silently prefer one bar's account of the material.
                    Some(existing) if !same_shares(&existing.shares, &shares) => {
                        errors.push(DispatchError::InconsistentPortions { block: block.resolved });
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
                                shares,
                            },
                        );
                    }
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let destinations: Vec<DestinationLedger> = input
            .destinations
            .iter()
            .map(|destination| DestinationLedger {
                id: destination.id,
                kind: destination.kind,
                capacity_t: destination.capacity_t,
                crusher: destination.crusher.clone(),
                received_t: 0.0,
                period: 0,
                period_received_t: 0.0,
            })
            .collect();

        // Every block can be exhausted once, every window has two edges, every
        // compiled rate change can become an event, and every destination can
        // fill once. A crusher adds a daily reset for as long as it is being
        // fed - bounded by the total tonnage over the smallest positive budget,
        // because no run can need more daily budgets than that.
        let changes = input
            .agents
            .iter()
            .try_fold(0_usize, |total, agent| total.checked_add(agent.calendar.changes.len()))
            .ok_or_else(|| vec![DispatchError::IterationCap])?;
        let resets = crusher_reset_allowance(&input, &balances);
        let terms = input
            .bars
            .len()
            .checked_mul(2)
            .and_then(|bars| bars.checked_add(ledger_order.len()))
            .and_then(|events| events.checked_add(changes))
            .and_then(|events| events.checked_add(destinations.len()))
            .and_then(|events| events.checked_add(resets))
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
            destinations,
            time_h: 0.0,
            events: 0,
            cap,
            execution: Vec::new(),
            movements: Vec::new(),
            stopped_at_limit: false,
            capacity_blocked: None,
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

    /// One instant: roll the crusher budgets to this period, assign every
    /// loader against both ledgers, find the next event, and advance every
    /// worked block and every receiving destination together. Returns whether
    /// the run is over.
    fn step(&mut self) -> Result<bool, DispatchError> {
        self.roll_crusher_periods();
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
        // A destination filling is an event of its own, at the *combined*
        // incoming rate: two loaders feeding one stockpile must not each
        // consume the same free space.
        if let Some(fill) = self.next_fill(&assignments) {
            next = Some(next.map_or(fill, |current| current.min(fill)));
        }
        // A receiving crusher's budget resets at the next period boundary, and
        // an exhausted one that work is waiting on opens at the next period
        // whose budget is positive.
        if let Some(reset) = self.next_crusher_event(&assignments) {
            next = Some(next.map_or(reset, |current| current.min(reset)));
        }
        if let Some(limit) = self.input.horizon_limit_h
            && limit > self.time_h
        {
            next = Some(next.map_or(limit, |current| current.min(limit)));
        }

        // Nothing is being worked and no window, rate change or destination
        // opens later: the run is over, whatever is still in the ground.
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
                let assignment = assignments[*index].clone();
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
                // Every tonne out of the ground is a tonne into a destination,
                // split by the portions' constant shares, so the two ledgers
                // advance together and reconcile by construction.
                for routed in &assignment.routed {
                    let delivered = tonnes * routed.share;
                    if delivered <= 0.0 {
                        continue;
                    }
                    self.destinations[routed.destination].receive(delivered);
                    self.movements.push((
                        Movement {
                            agent: assignment.agent,
                            bar: assignment.bar,
                            block: assignment.block,
                            resolved: assignment.resolved,
                            destination: self.destinations[routed.destination].id,
                            rule: routed.rule,
                            start_h: self.time_h,
                            end_h: next,
                            tonnes: delivered,
                        },
                        BandKey((delivered / elapsed).to_bits()),
                    ));
                }
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

    /// Bring every crusher's budget to the period the clock is now in.
    ///
    /// Jumped to rather than stepped through: the clock may have advanced past
    /// several empty periods, and none of them can have received anything.
    fn roll_crusher_periods(&mut self) {
        let period = period_of(self.time_h);
        for destination in &mut self.destinations {
            if destination.crusher.is_some() && destination.period != period {
                destination.period = period;
                destination.period_received_t = 0.0;
            }
        }
    }

    /// Every loader's block at the current instant, all chosen against the
    /// same two ledgers before any of them is applied.
    ///
    /// A bar whose next undepleted block cannot be routed is *blocked*, and the
    /// loader moves on to its next eligible bar. It never looks further down the
    /// blocked bar's own sequence: the dig order is the user's, and stepping over
    /// a block to keep a machine busy would re-order it.
    fn assign(&mut self) -> Vec<Assignment> {
        let mut assignments: Vec<Assignment> = Vec::new();
        let mut blocked: Option<String> = None;
        for agent in &self.fleet {
            let rate_tph = agent.calendar.rate_at(self.time_h);
            if rate_tph <= 0.0 {
                continue;
            }
            let mut chosen = None;
            for index in &self.bars {
                let bar = &self.input.bars[*index];
                if bar.agent != agent.agent || !bar.window.contains(self.time_h) {
                    continue;
                }
                // The next undepleted block of this sequence, and only it. A
                // block already at zero is skipped here rather than taken and
                // completed in no time: an assignment that could only produce a
                // zero-length segment would make that block an event at the same
                // instant forever.
                let Some(block) = bar.blocks.iter().find(|block| self.balances[&block.resolved].remaining_t > 0.0) else {
                    continue;
                };
                let routed = match self.route(block, agent.agent, &assignments) {
                    Ok(routed) => routed,
                    Err(reason) => {
                        // Remembered, not returned: another bar may still be
                        // workable, and this only becomes the run's answer if
                        // nothing at all could be assigned.
                        blocked = blocked.or(Some(reason));
                        continue;
                    }
                };
                chosen = Some(Assignment {
                    agent: agent.agent,
                    rate_tph,
                    bar: bar.bar,
                    block: block.block,
                    resolved: block.resolved,
                    routed,
                });
                break;
            }
            if let Some(assignment) = chosen {
                assignments.push(assignment);
            }
        }
        // Only a loader that had ground and a window and still could not be
        // given work is a capacity block. One that simply had nothing to dig is
        // idle, which is a different thing and already reported as such.
        if let Some(reason) = blocked {
            self.capacity_blocked = Some(reason);
        }
        assignments
    }

    /// Choose a destination for every positive portion of one block, or say
    /// which destination refused it.
    ///
    /// The first matching destination with room, top down. A matching
    /// destination that is full is passed over in favour of the next *matching*
    /// one; a destination no rule offered is never used however much room it
    /// has.
    ///
    /// `pending` is the assignments already made at this instant. They are not
    /// yet applied to the ledger, so their claims are counted here: two loaders
    /// choosing at the same instant must not both be handed the last of one
    /// destination's free space.
    fn route(&self, block: &DispatchBlock, agent: LoaderAgentId, pending: &[Assignment]) -> Result<Vec<Routed>, String> {
        if !self.input.routing {
            return Ok(Vec::new());
        }
        let _ = agent;
        let shares = shares_of(block);
        let mut routed = Vec::with_capacity(block.portions.len());
        for (index, portion) in block.portions.iter().enumerate() {
            if portion.tonnes <= 0.0 {
                continue;
            }
            let claimed = |target: usize| pending.iter().flat_map(|assignment| &assignment.routed).any(|entry| entry.destination == target);
            let choice = portion.routes.iter().find(|(target, _)| {
                let ledger = &self.destinations[*target];
                // An unlimited destination is always available. A finite one
                // must have room now, and must not already be the last hope of
                // an assignment made at this same instant unless it has room for
                // both - which the fill event, not this choice, resolves.
                ledger.can_receive() && (ledger.free_t().is_none() || !claimed(*target) || ledger.can_receive())
            });
            match choice {
                Some((target, rule)) => routed.push(Routed {
                    destination: *target,
                    rule: *rule,
                    share: shares.get(index).copied().unwrap_or(0.0),
                }),
                None => {
                    // Named by the last destination the rules offered, which is
                    // the one that has just refused it.
                    let last = portion.routes.last().map(|(target, _)| &self.destinations[*target]);
                    return Err(match last {
                        Some(ledger) => ledger.blocked_reason(&self.destination_name(ledger.id)),
                        None => crate::i18n::tr!("dispatch-blocked-unrouted"),
                    });
                }
            }
        }
        Ok(routed)
    }

    /// A destination's name for a termination message.
    ///
    /// The evaluator does not know project names - it is handed ids - so this is
    /// the id spelled out, and the caller replaces it with the real name when it
    /// reports the outcome.
    fn destination_name(&self, id: DestinationId) -> String {
        match id {
            DestinationId::Solid(solid) => format!("#{}", solid.0),
            DestinationId::Standalone(standalone) => format!("#{}", standalone.0),
        }
    }

    /// When the next destination with finite room fills, at the combined rate of
    /// everything feeding it.
    fn next_fill(&self, assignments: &[Assignment]) -> Option<f64> {
        let mut earliest: Option<f64> = None;
        for (index, ledger) in self.destinations.iter().enumerate() {
            let Some(free) = ledger.free_t() else { continue };
            let rate: f64 = assignments
                .iter()
                .flat_map(|assignment| assignment.routed.iter().map(move |routed| (assignment, routed)))
                .filter(|(_, routed)| routed.destination == index)
                .map(|(assignment, routed)| assignment.rate_tph * routed.share)
                .sum();
            if rate <= 0.0 {
                continue;
            }
            let at = self.time_h + free / rate;
            if at.is_finite() && at > self.time_h {
                earliest = Some(earliest.map_or(at, |current: f64| current.min(at)));
            }
        }
        earliest
    }

    /// The next instant a crusher's budget changes in a way that matters.
    ///
    /// Two cases, and the second is the one that is easy to miss. A crusher that
    /// is being fed resets at the next boundary, so that is an event. A crusher
    /// that has spent its budget also has to come back into service at the next
    /// period whose budget is positive - and it has to do so whether the
    /// material it refused is *waiting* or is going to a fallback in the
    /// meantime. Without the second case a crusher would be used for one day and
    /// then never again for as long as anything else could absorb the material.
    ///
    /// A crusher with no limit is never either of those, and an exhausted one is
    /// jumped forward through the calendar's own changes rather than stepped
    /// through the days between.
    fn next_crusher_event(&self, assignments: &[Assignment]) -> Option<f64> {
        let mut earliest: Option<f64> = None;
        let fed: Vec<usize> = assignments
            .iter()
            .flat_map(|assignment| assignment.routed.iter())
            .map(|routed| routed.destination)
            .collect();
        for (index, ledger) in self.destinations.iter().enumerate() {
            let Some(calendar) = ledger.crusher.as_ref() else { continue };
            let at = if calendar.limit_at(CalendarPeriod(ledger.period)).is_none() {
                // Unlimited this period: nothing about it changes at the
                // boundary unless the calendar says so, and that is already a
                // change this finds on the period it happens.
                next_positive_period(calendar, ledger.period)
                    .filter(|period| calendar.limit_at(CalendarPeriod(*period)) != calendar.limit_at(CalendarPeriod(ledger.period)))
                    .map(|period| f64::from(period) * SCHEDULE_PERIOD_H)
            } else if fed.contains(&index) {
                Some(f64::from(ledger.period.saturating_add(1)) * SCHEDULE_PERIOD_H)
            } else if !ledger.can_receive() {
                next_positive_period(calendar, ledger.period).map(|period| f64::from(period) * SCHEDULE_PERIOD_H)
            } else {
                None
            };
            if let Some(at) = at.filter(|at| at.is_finite() && *at > self.time_h) {
                earliest = Some(earliest.map_or(at, |current: f64| current.min(at)));
            }
        }
        earliest
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
            return DispatchOutcome::Limited;
        }
        // Executable work, reachable ground, and nowhere to put what it would
        // dig. Distinguished from Stranded, which is about windows and rates:
        // here the machine could work and the ground is there.
        if let Some(reason) = self
            .capacity_blocked
            .clone()
            .filter(|_| future_eligible && !self.destinations.is_empty() && !self.destinations.iter().all(DestinationLedger::has_future_capacity))
        {
            return DispatchOutcome::CapacityBlocked { reason };
        }
        DispatchOutcome::Stranded
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
        // Movements merge on the same rule as the execution bands, and in the
        // same order, so a delivery band lines up with the work that produced it.
        let mut movements = std::mem::take(&mut self.movements);
        movements.sort_by(|(left, _), (right, _)| {
            left.agent
                .cmp(&right.agent)
                .then(left.bar.cmp(&right.bar))
                .then(left.resolved.cmp(&right.resolved))
                .then(left.destination.cmp(&right.destination))
                .then(left.rule.cmp(&right.rule))
                .then(left.start_h.total_cmp(&right.start_h))
        });
        // Merged on the delivery's own identity *and* on the rate it arrived at;
        // see [`BandKey`]. A band merged across a rate change is apportioned
        // wrongly across a period boundary, which would put the Calendar's
        // destination rows hundreds of tonnes away from its loader rows on any
        // day a destination filled or a crusher's budget turned over.
        let mut merged_movements: Vec<(Movement, BandKey)> = Vec::with_capacity(movements.len());
        for (movement, key) in movements {
            match merged_movements.last_mut() {
                Some((last, last_key))
                    if last.agent == movement.agent
                        && last.bar == movement.bar
                        && last.resolved == movement.resolved
                        && last.destination == movement.destination
                        && last.rule == movement.rule
                        && *last_key == key
                        && (last.end_h - movement.start_h).abs() <= f64::EPSILON =>
                {
                    last.end_h = movement.end_h;
                    last.tonnes += movement.tonnes;
                }
                _ => merged_movements.push((movement, key)),
            }
        }
        let mut merged_movements: Vec<Movement> = merged_movements.into_iter().map(|(movement, _)| movement).collect();
        merged_movements.sort_by(|left, right| {
            left.start_h
                .total_cmp(&right.start_h)
                .then(left.agent.cmp(&right.agent))
                .then(left.destination.cmp(&right.destination))
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
        // Every destination, including the ones nothing reached: "nothing went
        // here" is an answer, and an absent row is not one.
        let destination_balances = self
            .destinations
            .iter()
            .map(|ledger| DestinationBalance {
                destination: ledger.id,
                kind: ledger.kind,
                capacity_t: ledger.capacity_t,
                received_t: ledger.received_t,
            })
            .collect();
        let outcome = self.outcome();
        DispatchSchedule {
            generation: self.input.generation,
            horizon_h,
            execution: merged,
            idle,
            balances,
            movements: merged_movements,
            destination_balances,
            outcome,
        }
    }
}

/// How far the portions of one block may sum away from its tonnage.
const PORTION_RELATIVE: f64 = 1e-6;
const PORTION_ABSOLUTE: f64 = 1e-6;

/// Each portion's constant fraction of its block.
///
/// A block of no tonnes has no shares to speak of, which is why a zero-tonnage
/// block routes nothing rather than dividing by zero.
fn shares_of(block: &DispatchBlock) -> Vec<f64> {
    if block.tonnes <= 0.0 {
        return vec![0.0; block.portions.len()];
    }
    block.portions.iter().map(|portion| portion.tonnes / block.tonnes).collect()
}

/// Whether two references to one block describe the same material.
fn same_shares(left: &[f64], right: &[f64]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(left, right)| (left - right).abs() <= PORTION_ABSOLUTE)
}

/// An allowance for the daily crusher resets one run can legitimately need.
///
/// No run can need more daily budgets than its total tonnage divided by the
/// smallest positive budget any crusher carries, so that is the bound - and it
/// is zero when no crusher has a finite budget.
fn crusher_reset_allowance(input: &DispatchInput, balances: &HashMap<DigBlockId, Balance>) -> usize {
    if !input.routing {
        return 0;
    }
    let smallest = input
        .destinations
        .iter()
        .filter_map(|destination| destination.crusher.as_ref())
        .flat_map(|calendar| calendar.default_tpd.into_iter().chain(calendar.periods.values().filter_map(|value| value.tonnes())))
        .filter(|limit| *limit > 0.0)
        .fold(f64::INFINITY, f64::min);
    if !smallest.is_finite() {
        return 0;
    }
    let total: f64 = balances.values().map(|balance| balance.started_t).sum();
    let periods = (total / smallest).ceil();
    if !periods.is_finite() || periods < 0.0 {
        return 0;
    }
    (periods as usize).saturating_add(2)
}
