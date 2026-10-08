//! The one calculated schedule every view reads.
//!
//! Run Period and Run All Periods publish exactly one of these per accepted
//! calculation. The Gantt, the Calendar and the animation all read it; none
//! of them reads the solver, the replay or the project's rules, and none of
//! them re-derives a figure this already holds.
//!
//! # What is in it
//!
//! Everything here is in project terms - loader agents, bars, dig blocks and
//! destinations by their stable ids - so a rename changes a label and never a
//! number. Presentation names are resolved by whoever draws.
//!
//! - [`Execution`]: one loader working one source over one solved span. Dig
//!   and reclaim are both executions, told apart by [`Activity`] and by
//!   [`WorkSource`]; reclaim is never dressed up as a dig block.
//! - [`Delivery`]: one movement to one destination on one truck class, with
//!   its truck-hours, its movement value and its contained quantity.
//! - [`GroundBalance`], [`PileTrack`], [`ChunkDraw`]: the balances.
//! - [`SolveReport`]: how the answer was found and what it does not claim.
//!
//! # Timing
//!
//! Every span is a solved execution segment: its start and end are the
//! model's own event times, and a loader's rate across it is its tonnes over
//! its duration. Nothing here divides tonnes by a nominal loader rate - a
//! truck-limited loader works below that rate for the whole span, and the
//! span says so. Adjacent spans are merged only when their identities and
//! rates agree, so a merge never smears one rate across another.
//!
//! Indexes the views need are built once, in [`CalculatedSchedule::new`], and
//! the result is shared immutably behind an `Arc` from then on.

#![cfg_attr(
    target_arch = "wasm32",
    allow(dead_code, reason = "the browser build draws this type but never builds one: schedule calculation is desktop-only")
)]

use std::collections::{BTreeMap, HashMap};

use super::{BarId, DestinationId, LoaderAgentId, SCHEDULE_PERIOD_H, cashflow::Activity, experiment::GradeUnit, trucking::TruckClassId};
use crate::model::{DigBlockId, ReserveFieldId};

/// What an execution span or delivery takes its material from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WorkSource {
    /// Ground: one physical dig block of the Solids run.
    Block(DigBlockId),
    /// Material taken back out of a stockpile.
    Stockpile(DestinationId),
}

/// One loader working one source over one solved span.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Execution {
    pub(crate) agent: LoaderAgentId,
    pub(crate) bar: BarId,
    pub(crate) activity: Activity,
    pub(crate) source: WorkSource,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    pub(crate) tonnes: f64,
}

impl Execution {
    /// The rate this loader actually worked at across the span.
    pub(crate) fn rate_tph(&self) -> f64 {
        let duration = self.end_h - self.start_h;
        if duration > 0.0 { self.tonnes / duration } else { 0.0 }
    }
}

/// Why a loader did nothing over one idle span.
///
/// Read off the published schedule and the captured input, not the solver:
/// whichever optimiser produced the schedule, the first of these that holds
/// is the reason given, checked in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum IdleReason {
    /// A delay list or roster takes the machine out, or a delay bar holds it
    /// with priority. The Gantt draws those itself, so it does not mark this
    /// as idle.
    Delayed,
    /// The calendar gives the machine no rate: availability, utilisation or
    /// the rate itself is zero.
    Unavailable,
    /// None of its bars' windows is open.
    NoWork,
    /// Its open bars' ground is all dug, or their stockpiles are empty.
    WorkFinished,
    /// Ground is left, but no routing rule sends its material anywhere this
    /// machine can take it.
    NoRoute,
    /// Every destination its material may go to is full, or at its crusher
    /// budget for the day.
    DestinationsFull,
    /// A stockpile's authored mode stopped it: the piles it would reclaim
    /// are not reclaiming today, or the piles its material would go to are
    /// not building.
    PileMode,
    /// Every truck class that can haul for it is fully used.
    NoTrucks,
    /// Work, room and trucks were all there: moving the material was worth
    /// less than leaving it.
    NotWorthIt,
    /// Its bar's next block has not been blasted yet; the span names the
    /// blast. See [`super::drill_blast`].
    WaitingOnBlast,
}

/// What drill and blast did in a calculated schedule: every blast with its
/// milestones, and each dozer, drill and MPU's work.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DrillBlastResult {
    pub(crate) blasts: Vec<PublishedBlast>,
    pub(crate) work: Vec<PublishedBlastWork>,
}

/// One blast, where it is and when it got there.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PublishedBlast {
    pub(crate) reference: super::BlastRef,
    pub(crate) solid_name: String,
    pub(crate) name: String,
    pub(crate) bench_base: f64,
    pub(crate) bench_top: f64,
    pub(crate) face: std::sync::Arc<crate::model::arrangement::Face>,
    /// Hole collars in plan, in the order they are drilled.
    pub(crate) collars: Vec<glam::DVec2>,
    /// Square metres to prep, metres to drill and tonnes to charge.
    pub(crate) quantity: [f64; 3],
    pub(crate) cleared_h: Option<f64>,
    /// Prep, drill and charge finished.
    pub(crate) done_h: [Option<f64>; 3],
    /// Its ground available: the end of the window it fired in.
    pub(crate) fired_h: Option<f64>,
    /// Standing ground above it, within the buffer, is in no dig bar, so it
    /// never clears.
    pub(crate) never_clear: bool,
    /// The first step it still needs that no machine bar works.
    pub(crate) unworked: Option<super::BlastActivity>,
    /// The blasts whose ground has to be dug before it is clear, by position
    /// in [`DrillBlastResult::blasts`].
    pub(crate) above: Vec<usize>,
}

/// What one blast is waiting for at an instant: why a loader waiting on it
/// is still waiting.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BlastHold {
    /// Ground above it that no bar digs stands for good.
    NeverClears,
    /// Ground above it is still being dug: these blasts' ground, by position.
    AboveStanding(Vec<usize>),
    /// The step it is ready for has no machine bar working it.
    NoMachine(super::BlastActivity),
    /// A machine is on this step now.
    Working(super::BlastActivity),
    /// Ready for this step, but its machines are busy elsewhere or not yet
    /// rostered on.
    Queued(super::BlastActivity),
    /// Charged, and waiting for a blast window.
    AwaitingWindow,
    Fired,
}

impl DrillBlastResult {
    /// What the blast at `index` is waiting for at `hour`.
    pub(crate) fn hold(&self, index: usize, hour: f64) -> Option<BlastHold> {
        let blast = self.blasts.get(index)?;
        let by = |at: Option<f64>| at.is_some_and(|at| at <= hour + 1e-9);
        if by(blast.fired_h) {
            return Some(BlastHold::Fired);
        }
        let Some(step) = super::BlastActivity::ALL.into_iter().find(|step| !by(blast.done_h[*step as usize])) else {
            return Some(BlastHold::AwaitingWindow);
        };
        if step == super::BlastActivity::Prep && !by(blast.cleared_h) {
            return Some(if blast.never_clear {
                BlastHold::NeverClears
            } else {
                BlastHold::AboveStanding(blast.above.clone())
            });
        }
        if blast.unworked == Some(step) {
            return Some(BlastHold::NoMachine(step));
        }
        let working = self
            .work
            .iter()
            .any(|work| work.blast == index && work.activity == step && work.start_h <= hour + 1e-9 && hour < work.end_h - 1e-9);
        Some(if working { BlastHold::Working(step) } else { BlastHold::Queued(step) })
    }
}

/// A stretch of one machine on one step of one blast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PublishedBlastWork {
    pub(crate) agent: LoaderAgentId,
    /// Position in [`DrillBlastResult::blasts`].
    pub(crate) blast: usize,
    pub(crate) activity: super::BlastActivity,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    pub(crate) quantity: f64,
}

impl DrillBlastResult {
    /// How much of `activity` a blast has had done by `at_h`, as a share of
    /// what it needs.
    pub(crate) fn done_share(&self, blast: usize, activity: super::BlastActivity, at_h: f64) -> f64 {
        let Some(entry) = self.blasts.get(blast) else { return 0.0 };
        if entry.done_h[activity as usize].is_some_and(|done| done <= at_h) {
            return 1.0;
        }
        let needed = entry.quantity[activity as usize];
        if needed <= 0.0 {
            return 0.0;
        }
        let worked: f64 = self
            .work
            .iter()
            .filter(|row| row.blast == blast && row.activity == activity && row.start_h < at_h)
            .map(|row| {
                let span = row.end_h - row.start_h;
                if span > 0.0 {
                    row.quantity * ((at_h.min(row.end_h) - row.start_h) / span)
                } else {
                    row.quantity
                }
            })
            .sum();
        // A blast that started part way along has the rest done already.
        let started_done = entry.done_h[activity as usize] == Some(0.0);
        if started_done { 1.0 } else { (worked / needed).clamp(0.0, 1.0) }
    }

    /// What one machine got done between `from_h` and `to_h`, in its unit.
    pub(crate) fn worked(&self, agent: LoaderAgentId, from_h: f64, to_h: f64) -> Option<f64> {
        let rows: Vec<_> = self.work.iter().filter(|row| row.agent == agent).collect();
        if rows.is_empty() {
            return None;
        }
        Some(
            rows.iter()
                .map(|row| {
                    let span = row.end_h - row.start_h;
                    let overlap = (row.end_h.min(to_h) - row.start_h.max(from_h)).max(0.0);
                    if span > 0.0 { row.quantity * overlap / span } else { 0.0 }
                })
                .sum(),
        )
    }

    /// The stage a blast has reached by `at_h`.
    pub(crate) fn stage_at(&self, blast: usize, at_h: f64) -> super::BlastStage {
        use super::BlastStage;
        let Some(entry) = self.blasts.get(blast) else { return BlastStage::NotStarted };
        let reached = |time: Option<f64>| time.is_some_and(|time| time <= at_h + 1e-9);
        if reached(entry.fired_h) {
            BlastStage::Fired
        } else if reached(entry.done_h[2]) {
            BlastStage::Charged
        } else if reached(entry.done_h[1]) {
            BlastStage::Drilled
        } else if reached(entry.done_h[0]) {
            BlastStage::Prepped
        } else {
            BlastStage::NotStarted
        }
    }
}

/// Loader time with no execution inside the requested horizon.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IdleSpan {
    pub(crate) agent: LoaderAgentId,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    /// `None` until [`CalculatedSchedule::classify_idle`] has explained it.
    pub(crate) reason: Option<IdleReason>,
    /// For [`IdleReason::DestinationsFull`], the destinations that had no
    /// room, and for [`IdleReason::PileMode`] the stockpiles its mode
    /// closed, so the reason can name them. Empty otherwise.
    pub(crate) full: Vec<DestinationId>,
    /// For [`IdleReason::WaitingOnBlast`], the blast waited for, by position
    /// in [`CalculatedSchedule::drill_blast`].
    pub(crate) blast: Option<usize>,
}

/// One movement of material to one destination.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Delivery {
    pub(crate) agent: LoaderAgentId,
    pub(crate) bar: BarId,
    pub(crate) activity: Activity,
    pub(crate) source: WorkSource,
    pub(crate) destination: DestinationId,
    pub(crate) truck: TruckClassId,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    pub(crate) tonnes: f64,
    pub(crate) truck_hours: f64,
    pub(crate) cycle: std::sync::Arc<super::trucking::CycleBreakdown>,
    /// Signed movement value, every matching rule added, conditional rules
    /// valued at their authored boundaries.
    pub(crate) value: f64,
    /// Weighted quantity (tonnes × stored grade) per tracked grade,
    /// aligned with [`CalculatedSchedule::grades`].
    pub(crate) contained: Vec<f64>,
}

/// Tonnes-weighted haul figures. Summable across deliveries, hours and days.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct HaulSummary {
    pub(crate) tonnes: f64,
    pub(crate) cycle_t_h: f64,
    pub(crate) loaded_t_km: f64,
    pub(crate) rise_t_m: f64,
    pub(crate) spot_t_h: f64,
    pub(crate) load_t_h: f64,
    pub(crate) loaded_t_h: f64,
    pub(crate) dump_t_h: f64,
    pub(crate) empty_t_h: f64,
}
impl HaulSummary {
    pub(crate) fn add(&mut self, delivery: &Delivery, share: f64) {
        let t = delivery.tonnes * share;
        self.tonnes += t;
        self.cycle_t_h += delivery.cycle.total_h() * t;
        self.loaded_t_km += delivery.cycle.loaded_km * t;
        self.rise_t_m += delivery.cycle.rise_m * t;
        self.spot_t_h += delivery.cycle.spot_h * t;
        self.load_t_h += delivery.cycle.load_h * t;
        self.loaded_t_h += delivery.cycle.loaded_h * t;
        self.dump_t_h += delivery.cycle.dump_h * t;
        self.empty_t_h += delivery.cycle.empty_h * t;
    }
    pub(crate) fn average(self, value: f64) -> f64 {
        if self.tonnes > 0.0 { value / self.tonnes } else { 0.0 }
    }
    pub(crate) fn cycle_minutes(self) -> f64 {
        self.average(self.cycle_t_h) * 60.0
    }
    pub(crate) fn distance_km(self) -> f64 {
        self.average(self.loaded_t_km)
    }
}

/// What one dig block started with and what it had left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GroundBalance {
    pub(crate) block: DigBlockId,
    pub(crate) started_t: f64,
    pub(crate) remaining_t: f64,
    /// When the block ran out, if it did.
    pub(crate) emptied_h: Option<f64>,
}

/// One stockpile's balance over the calculation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PileTrack {
    pub(crate) destination: DestinationId,
    pub(crate) opening_t: f64,
    /// Contained quantity per grade at hour zero. Empty when the pile was not
    /// in the model and so has no captured composition.
    pub(crate) opening_q: Vec<f64>,
    /// Balance at the close of each calendar interval, in time order.
    pub(crate) steps: Vec<PileStep>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PileStep {
    pub(crate) end_h: f64,
    pub(crate) closing_t: f64,
    pub(crate) closing_q: Vec<f64>,
}

/// Balance and net movement rate at a solved event boundary. Receipts enter
/// physical inventory immediately even though reclaim eligibility changes
/// only at calendar interval boundaries.
#[derive(Clone, Debug)]
struct InventoryKnot {
    hour: f64,
    tonnes: f64,
    contained: Vec<f64>,
    rate: f64,
    contained_rate: Vec<f64>,
}

fn inventory_curves(piles: &[PileTrack], deliveries: &[Delivery]) -> HashMap<DestinationId, Vec<InventoryKnot>> {
    let mut events: HashMap<DestinationId, Vec<(f64, f64, Vec<f64>)>> = HashMap::new();
    for delivery in deliveries {
        let duration = delivery.end_h - delivery.start_h;
        if duration <= 0.0 {
            continue;
        }
        let incoming = std::iter::once((delivery.destination, 1.0));
        let outgoing = match delivery.source {
            WorkSource::Stockpile(pile) => Some((pile, -1.0)),
            WorkSource::Block(_) => None,
        };
        for (pile, sign) in incoming.chain(outgoing) {
            let rate = sign * delivery.tonnes / duration;
            let contained: Vec<_> = delivery.contained.iter().map(|quantity| sign * quantity / duration).collect();
            let entries = events.entry(pile).or_default();
            entries.push((delivery.start_h, rate, contained.clone()));
            entries.push((delivery.end_h, -rate, contained.into_iter().map(|quantity| -quantity).collect()));
        }
    }
    piles
        .iter()
        .map(|pile| {
            let mut entries = events.remove(&pile.destination).unwrap_or_default();
            entries.sort_by(|left, right| left.0.total_cmp(&right.0));
            let mut curve = vec![InventoryKnot {
                hour: 0.0,
                tonnes: pile.opening_t,
                contained: pile.opening_q.clone(),
                rate: 0.0,
                contained_rate: vec![0.0; pile.opening_q.len()],
            }];
            for (hour, rate, contained_rate) in entries {
                let last = curve.last_mut().expect("opening inventory");
                if hour > last.hour {
                    let elapsed = hour - last.hour;
                    let next = InventoryKnot {
                        hour,
                        tonnes: last.tonnes + elapsed * last.rate,
                        contained: last.contained.iter().zip(&last.contained_rate).map(|(quantity, rate)| quantity + elapsed * rate).collect(),
                        rate: last.rate,
                        contained_rate: last.contained_rate.clone(),
                    };
                    curve.push(next);
                }
                let last = curve.last_mut().expect("inventory event");
                last.rate += rate;
                for (net, change) in last.contained_rate.iter_mut().zip(contained_rate) {
                    *net += change;
                }
            }
            (pile.destination, curve)
        })
        .collect()
}

impl PileTrack {
    /// The balance at `hour`, read off the last interval closed by then.
    ///
    /// Intervals split at every midnight, so a period end is always an
    /// interval end and this is exact there.
    pub(crate) fn balance_at(&self, hour: f64) -> (f64, &[f64]) {
        let closed = self.steps.partition_point(|step| step.end_h <= hour + 1e-9);
        match closed.checked_sub(1).and_then(|index| self.steps.get(index)) {
            Some(step) => (step.closing_t, &step.closing_q),
            None => (self.opening_t, &self.opening_q),
        }
    }
}

/// Material a reclaim drew from one chunk of an ordered chunked pile.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChunkDraw {
    pub(crate) pile: DestinationId,
    pub(crate) chunk: usize,
    /// "Opening lot <name>" or "Receiving chunk <n>", resolved at capture.
    pub(crate) label: String,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    pub(crate) tonnes: f64,
}

/// How the backend finished, for an accepted calculation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SolveQuality {
    /// Optimal for the encoded model within the configured gap.
    Optimal,
    /// A time or gap limit was reached with a valid schedule in hand.
    Limited,
}

/// Solver observations, not independent feasibility or optimality claims.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SolveDiagnostics {
    /// SCIP elapsed solve time at its first solution event; replay occurs later.
    pub(crate) first_incumbent_s: Option<f64>,
    pub(crate) incumbent_improvements: u64,
    pub(crate) presolve_rounds: u64,
    pub(crate) presolve_s: f64,
    /// First root initial LP event with a completed (not time/iteration-limited) LP.
    pub(crate) root_initial_lp_s: Option<f64>,
    /// First completed cut-and-price LP event at the root. Not root-node completion.
    pub(crate) root_cut_price_s: Option<f64>,
    /// A root NODE_SOLVED event was observed, or a non-root node was focused.
    pub(crate) root_node_finished: bool,
    /// Active transformed problem at first root focus, after presolve.
    pub(crate) presolved_variables: Option<usize>,
    pub(crate) presolved_constraints: Option<usize>,
    /// Initial completed root LP snapshot; absent if no such event occurred.
    pub(crate) root_lp_columns: Option<usize>,
    pub(crate) root_lp_rows: Option<usize>,
    pub(crate) root_lp_nonzeros: Option<usize>,
    pub(crate) nodes: usize,
    pub(crate) lp_iterations: u64,
    pub(crate) root_lp_iterations: u64,
    /// Final transformed size is distinct from the post-presolve snapshot.
    pub(crate) final_variables: usize,
    pub(crate) final_constraints: usize,
    pub(crate) final_nonzeros: u64,
}

/// The first schedule of a run, replayed against the whole horizon, shown
/// at once and used to seed the whole-horizon solve: the hourly dispatch
/// schedule, or when that fails, days solved one at a time
/// and stitched.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct DayByDaySummary {
    #[serde(default)]
    pub(crate) method: StartMethod,
    /// Day-by-day windows, or the dispatch schedule's intervals.
    pub(crate) windows: usize,
    pub(crate) seconds: f64,
    /// Replayed value of the stitched schedule; `None` when no window
    /// sequence produced one.
    pub(crate) value: Option<f64>,
    pub(crate) role: DayByDayRole,
    pub(crate) failure: Option<String>,
}

/// How a run's first schedule was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum StartMethod {
    /// Days solved one at a time by SCIP and stitched.
    #[default]
    DayByDay,
    /// Interval by interval, one linear program each.
    Hourly,
}

/// What became of the first schedule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum DayByDayRole {
    /// It seeded the whole-horizon solve, whose schedule is the one
    /// published.
    #[default]
    Improved,
    /// It is the published schedule: the whole-horizon solve found nothing
    /// better in its time.
    Kept,
    /// It is published while the whole-horizon solve still looks for a
    /// better one.
    Early,
    /// It is published because the user stopped the whole-horizon solve.
    Stopped,
    /// It is published as within the gap target: the relaxation bound proved
    /// it, so no whole-horizon solve ran.
    Proven,
    /// It is published as asked for: the run was a recalculation that does
    /// not look for a better schedule. Improve does.
    Only,
}

/// Which solve proved a published bound.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum BoundSource {
    /// SCIP's own dual bound on the model.
    #[default]
    Scip,
    /// HiGHS's optimum of the model's linear relaxation, which is looser by
    /// construction but can be had long before SCIP's first LP finishes.
    Relaxation,
}

/// How the answer was found, and the approximations it rests on.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SolveReport {
    pub(crate) quality: Option<SolveQuality>,
    /// Movement value recomputed from the published rows, authored
    /// boundaries applied.
    pub(crate) objective: f64,
    pub(crate) raw_objective: Option<f64>,
    pub(crate) bound: Option<f64>,
    pub(crate) gap: Option<f64>,
    pub(crate) bound_source: BoundSource,
    pub(crate) capture_s: f64,
    pub(crate) formulation_s: f64,
    pub(crate) solve_s: f64,
    pub(crate) extraction_s: f64,
    pub(crate) replay_s: f64,
    pub(crate) publication_s: f64,
    pub(crate) backend: String,
    /// The captured model's own fingerprint: every rate, window, capacity,
    /// coefficient and grade conversion the solver was handed.
    pub(crate) model_identity: u64,
    pub(crate) candidates: usize,
    pub(crate) ground_sources: usize,
    pub(crate) variables: usize,
    pub(crate) binaries: usize,
    pub(crate) constraints: usize,
    pub(crate) linear_coefficient_entries: usize,
    pub(crate) diagnostics: SolveDiagnostics,
    pub(crate) day_by_day: Option<DayByDaySummary>,
    pub(crate) intervals: usize,
    pub(crate) segments_per_interval: usize,
    /// The derived execution-event budget hit its ceiling, so some source
    /// transitions inside an interval may have been unavailable.
    pub(crate) event_budget_restricted: bool,
    /// Blasts a dig bar needs that no machine bar works, so their loaders
    /// wait on them all horizon.
    pub(crate) unworked_blasts: usize,
    pub(crate) chunk_slots: usize,
    /// Every receiving chunk of some pile filled, which is the point at which
    /// non-reusable slots can start to limit receipts.
    pub(crate) chunk_slots_full: bool,
    pub(crate) grade_margin: f64,
    pub(crate) boundary_rows: usize,
    pub(crate) boundary_tonnes_t: f64,
    pub(crate) boundary_value_slack: f64,
    pub(crate) target_value_tolerance: f64,
    /// How far the solver's own objective may sit above the published one
    /// through conditional-value indicator tolerance.
    pub(crate) indicator_leak_value: f64,
    /// Tiny solver columns and zero-duration rows left out of the timeline.
    pub(crate) omitted_rows: usize,
    pub(crate) omitted_tonnes_t: f64,
    /// Stated approximations from capture, verbatim.
    pub(crate) notes: Vec<String>,
}

/// Per-period aggregates, built once per accepted calculation.
///
/// Every figure is a sum of published spans, never a rate multiplied back
/// out. A covered period with nothing in it reads zero; a period the
/// calculation does not reach reads `None`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PeriodTotals {
    coverage_end_h: f64,
    dig: BTreeMap<(LoaderAgentId, u32), f64>,
    reclaim: BTreeMap<(LoaderAgentId, u32), f64>,
    received: BTreeMap<(DestinationId, u32), f64>,
    received_contained: BTreeMap<(DestinationId, u32), Vec<f64>>,
    reclaimed: BTreeMap<(DestinationId, u32), f64>,
    closing: BTreeMap<(DestinationId, u32), (f64, Vec<f64>)>,
    truck_hours: BTreeMap<(TruckClassId, u32), f64>,
    truck_haul: BTreeMap<(TruckClassId, u32), HaulSummary>,
    value: BTreeMap<u32, f64>,
}

/// Which period an instant belongs to. Periods are half-open.
fn period_of(hour: f64) -> u32 {
    (hour / SCHEDULE_PERIOD_H).floor().max(0.0) as u32
}

/// Split `[start, end)` across the periods it touches, by duration.
///
/// Solved spans never cross a midnight - the calendar splits there - so this
/// is almost always one period; the split is kept so a merged span that did
/// cross one is still apportioned by time, with the rounding remainder in the
/// last period so the span's total survives.
fn apportion(start_h: f64, end_h: f64, amount: f64, mut add: impl FnMut(u32, f64)) {
    let first = period_of(start_h);
    let last = (((end_h / SCHEDULE_PERIOD_H).ceil().max(0.0)) as u32).saturating_sub(1).max(first);
    let duration = end_h - start_h;
    if first == last || duration <= 0.0 {
        add(first, amount);
        return;
    }
    let mut assigned = 0.0;
    for period in first..last {
        let from = f64::from(period) * SCHEDULE_PERIOD_H;
        let to = from + SCHEDULE_PERIOD_H;
        let share = amount * (to.min(end_h) - from.max(start_h)).max(0.0) / duration;
        assigned += share;
        add(period, share);
    }
    add(last, amount - assigned);
}

impl PeriodTotals {
    fn build(coverage_end_h: f64, executions: &[Execution], deliveries: &[Delivery], piles: &[PileTrack]) -> Self {
        let mut totals = Self {
            coverage_end_h: if coverage_end_h.is_finite() { coverage_end_h.max(0.0) } else { 0.0 },
            ..Self::default()
        };
        for execution in executions {
            let map = match execution.activity {
                Activity::Dig => &mut totals.dig,
                Activity::Reclaim => &mut totals.reclaim,
            };
            apportion(execution.start_h, execution.end_h, execution.tonnes, |period, share| {
                *map.entry((execution.agent, period)).or_default() += share;
            });
        }
        for delivery in deliveries {
            apportion(delivery.start_h, delivery.end_h, delivery.tonnes, |period, share| {
                *totals.received.entry((delivery.destination, period)).or_default() += share;
            });
            for (grade, &quantity) in delivery.contained.iter().enumerate() {
                apportion(delivery.start_h, delivery.end_h, quantity, |period, share| {
                    let quantities = totals.received_contained.entry((delivery.destination, period)).or_default();
                    quantities.resize(quantities.len().max(grade + 1), 0.0);
                    quantities[grade] += share;
                });
            }
            if let WorkSource::Stockpile(pile) = delivery.source {
                apportion(delivery.start_h, delivery.end_h, delivery.tonnes, |period, share| {
                    *totals.reclaimed.entry((pile, period)).or_default() += share;
                });
            }
            apportion(delivery.start_h, delivery.end_h, 1.0, |period, share| {
                totals.truck_haul.entry((delivery.truck, period)).or_default().add(delivery, share)
            });
            apportion(delivery.start_h, delivery.end_h, delivery.truck_hours, |period, share| {
                *totals.truck_hours.entry((delivery.truck, period)).or_default() += share;
            });
            apportion(delivery.start_h, delivery.end_h, delivery.value, |period, share| {
                *totals.value.entry(period).or_default() += share;
            });
        }
        let periods = totals.covered_periods();
        for pile in piles {
            for period in 0..periods {
                let close = (f64::from(period + 1) * SCHEDULE_PERIOD_H).min(totals.coverage_end_h);
                let (tonnes, contained) = pile.balance_at(close);
                totals.closing.insert((pile.destination, period), (tonnes, contained.to_vec()));
            }
        }
        totals
    }

    pub(crate) fn coverage_end_h(&self) -> f64 {
        self.coverage_end_h
    }

    /// How many periods the calculation reaches into, whole or partial.
    pub(crate) fn covered_periods(&self) -> u32 {
        if self.coverage_end_h <= 0.0 {
            return 0;
        }
        (self.coverage_end_h / SCHEDULE_PERIOD_H).ceil().max(0.0) as u32
    }

    pub(crate) fn covers(&self, period: u32) -> bool {
        self.coverage_end_h > 0.0 && f64::from(period) * SCHEDULE_PERIOD_H < self.coverage_end_h
    }

    /// Whether coverage stops part-way through this period.
    /// Hours of `period` the calculation covers: the whole day, or the part
    /// of it before the horizon ends.
    pub(crate) fn covered_hours(&self, period: u32) -> Option<f64> {
        let start = f64::from(period) * SCHEDULE_PERIOD_H;
        self.covers(period).then(|| (self.coverage_end_h.min(start + SCHEDULE_PERIOD_H) - start).max(0.0))
    }

    pub(crate) fn is_partial(&self, period: u32) -> bool {
        self.covers(period) && self.coverage_end_h < f64::from(period.saturating_add(1)) * SCHEDULE_PERIOD_H
    }

    fn flow<K: Ord>(&self, map: &BTreeMap<K, f64>, key: K, period: u32) -> Option<f64> {
        self.covers(period).then(|| map.get(&key).copied().unwrap_or(0.0))
    }

    pub(crate) fn dig(&self, agent: LoaderAgentId, period: u32) -> Option<f64> {
        self.flow(&self.dig, (agent, period), period)
    }

    pub(crate) fn reclaim(&self, agent: LoaderAgentId, period: u32) -> Option<f64> {
        self.flow(&self.reclaim, (agent, period), period)
    }

    pub(crate) fn received(&self, destination: DestinationId, period: u32) -> Option<f64> {
        self.flow(&self.received, (destination, period), period)
    }

    /// Tonnes-weighted actual delivered grade, including mining and reclaim.
    pub(crate) fn received_grade(&self, destination: DestinationId, period: u32, grade: usize) -> Option<f64> {
        let tonnes = self.received(destination, period)?;
        if tonnes <= 1e-6 {
            return None;
        }
        Some(*self.received_contained.get(&(destination, period))?.get(grade)? / tonnes)
    }

    pub(crate) fn reclaimed(&self, pile: DestinationId, period: u32) -> Option<f64> {
        self.flow(&self.reclaimed, (pile, period), period)
    }

    /// Everything a destination has received up to and including `period`.
    pub(crate) fn cumulative(&self, destination: DestinationId, period: u32) -> Option<f64> {
        self.covers(period)
            .then(|| self.received.range((destination, 0)..=(destination, period)).map(|(_, tonnes)| tonnes).sum())
    }

    /// A stockpile's balance at the period's close, opening stock included.
    /// A balance, not a flow: a period with no movement holds what the one
    /// before it closed with.
    pub(crate) fn closing(&self, pile: DestinationId, period: u32) -> Option<(f64, &[f64])> {
        self.covers(period)
            .then(|| self.closing.get(&(pile, period)).map(|(tonnes, contained)| (*tonnes, contained.as_slice())))
            .flatten()
    }

    pub(crate) fn truck_haul(&self, truck: TruckClassId, period: u32) -> Option<HaulSummary> {
        self.covers(period).then(|| self.truck_haul.get(&(truck, period)).copied().unwrap_or_default())
    }

    pub(crate) fn truck_hours(&self, truck: TruckClassId, period: u32) -> Option<f64> {
        self.flow(&self.truck_hours, (truck, period), period)
    }

    pub(crate) fn value(&self, period: u32) -> Option<f64> {
        self.covers(period).then(|| self.value.get(&period).copied().unwrap_or(0.0))
    }
}

/// What one destination received in each whole hour from hour zero, built
/// once per calculation for the charts. Each delivery is spread over the
/// hours it overlaps, in proportion to the overlap.
#[derive(Clone, Debug, Default)]
pub(crate) struct HourlyReceipts {
    pub(crate) tonnes: Vec<f64>,
    /// Contained quantity per tracked grade, `[grade][hour]`.
    pub(crate) contained: Vec<Vec<f64>>,
}

fn hourly_receipts(requested_end_h: f64, grades: usize, deliveries: &[Delivery]) -> HashMap<DestinationId, HourlyReceipts> {
    let hours = if requested_end_h.is_finite() { requested_end_h.max(0.0).ceil() as usize } else { 0 };
    let mut receipts: HashMap<DestinationId, HourlyReceipts> = HashMap::new();
    for delivery in deliveries {
        let duration = delivery.end_h - delivery.start_h;
        if duration <= 0.0 || hours == 0 {
            continue;
        }
        let entry = receipts.entry(delivery.destination).or_insert_with(|| HourlyReceipts {
            tonnes: vec![0.0; hours],
            contained: vec![vec![0.0; hours]; grades],
        });
        let first = delivery.start_h.max(0.0).floor() as usize;
        let last = (delivery.end_h.ceil().max(0.0) as usize).min(hours);
        for hour in first..last {
            let overlap = delivery.end_h.min(hour as f64 + 1.0) - delivery.start_h.max(hour as f64);
            if overlap <= 0.0 {
                continue;
            }
            let share = overlap / duration;
            entry.tonnes[hour] += delivery.tonnes * share;
            for (series, quantity) in entry.contained.iter_mut().zip(&delivery.contained) {
                series[hour] += quantity * share;
            }
        }
    }
    receipts
}

/// One target day, recomputed from actual deliveries by independent replay.
#[derive(Clone, Debug)]
pub(crate) struct GradeTargetResult {
    pub(crate) specification: super::grade_targets::GradeTarget,
    /// The absolute calendar day.
    pub(crate) period: u32,
    pub(crate) tonnes: f64,
    /// Tonnes times grade, in the grade's stored scale.
    pub(crate) contained: f64,
    pub(crate) penalty: f64,
}

/// One accepted calculation, in project terms.
#[derive(Clone, Debug)]
pub(crate) struct CalculatedSchedule {
    /// Which run produced it, so a held result can be named.
    pub(crate) run: u64,
    /// The semantic input identity it was calculated from.
    pub(crate) semantic: u64,
    /// The Solids run its ground came from.
    pub(crate) generation: u64,
    /// The horizon the run was asked to cover, from hour zero. Recorded
    /// independently of when the last movement happened: a requested day
    /// with nothing in it is still a calculated day.
    pub(crate) requested_end_h: f64,
    pub(crate) executions: Vec<Execution>,
    pub(crate) deliveries: Vec<Delivery>,
    pub(crate) ground: Vec<GroundBalance>,
    pub(crate) piles: Vec<PileTrack>,
    pub(crate) chunk_draws: Vec<ChunkDraw>,
    /// Loader time inside the requested horizon with no execution, per agent,
    /// each with the reason publication found for it.
    pub(crate) idle: Vec<IdleSpan>,
    /// Each dig bar's resolved blocks, in authored order.
    pub(crate) bar_blocks: Vec<(BarId, Vec<DigBlockId>)>,
    /// Each reclaim bar's cap, as captured.
    pub(crate) reclaim_caps: Vec<(BarId, Option<f64>)>,
    pub(crate) grades: Vec<(ReserveFieldId, GradeUnit)>,
    pub(crate) grade_targets: Vec<GradeTargetResult>,
    pub(crate) report: SolveReport,
    /// Drill and blast, when the project sequences it.
    pub(crate) drill_blast: Option<DrillBlastResult>,
    pub(crate) periods: PeriodTotals,
    by_bar: HashMap<BarId, Vec<usize>>,
    by_block: HashMap<DigBlockId, usize>,
    inventory: HashMap<DestinationId, Vec<InventoryKnot>>,
    hourly: HashMap<DestinationId, HourlyReceipts>,
    truck_use: HashMap<TruckClassId, Vec<(f64, f64)>>,
}

/// Everything [`CalculatedSchedule::new`] indexes.
pub(crate) struct ScheduleParts {
    pub(crate) run: u64,
    pub(crate) semantic: u64,
    pub(crate) generation: u64,
    pub(crate) requested_end_h: f64,
    pub(crate) executions: Vec<Execution>,
    pub(crate) deliveries: Vec<Delivery>,
    pub(crate) ground: Vec<GroundBalance>,
    pub(crate) piles: Vec<PileTrack>,
    pub(crate) chunk_draws: Vec<ChunkDraw>,
    pub(crate) agents: Vec<LoaderAgentId>,
    pub(crate) bar_blocks: Vec<(BarId, Vec<DigBlockId>)>,
    pub(crate) reclaim_caps: Vec<(BarId, Option<f64>)>,
    pub(crate) grades: Vec<(ReserveFieldId, GradeUnit)>,
    pub(crate) grade_targets: Vec<GradeTargetResult>,
    pub(crate) report: SolveReport,
    pub(crate) drill_blast: Option<DrillBlastResult>,
}

impl CalculatedSchedule {
    pub(crate) fn new(parts: ScheduleParts) -> Self {
        let mut executions = parts.executions;
        executions.sort_by(|left, right| (left.agent, left.start_h).partial_cmp(&(right.agent, right.start_h)).unwrap_or(std::cmp::Ordering::Equal));
        let mut idle = Vec::new();
        for agent in &parts.agents {
            let mut at = 0.0_f64;
            for execution in executions.iter().filter(|execution| execution.agent == *agent) {
                if execution.start_h > at + 1e-6 {
                    idle.push(IdleSpan {
                        agent: *agent,
                        start_h: at,
                        end_h: execution.start_h,
                        reason: None,
                        full: Vec::new(),
                        blast: None,
                    });
                }
                at = at.max(execution.end_h);
            }
            if parts.requested_end_h > at + 1e-6 {
                idle.push(IdleSpan {
                    agent: *agent,
                    start_h: at,
                    end_h: parts.requested_end_h,
                    reason: None,
                    full: Vec::new(),
                    blast: None,
                });
            }
        }
        let mut by_bar: HashMap<BarId, Vec<usize>> = HashMap::new();
        for (index, execution) in executions.iter().enumerate() {
            by_bar.entry(execution.bar).or_default().push(index);
        }
        let by_block = parts.ground.iter().enumerate().map(|(index, balance)| (balance.block, index)).collect();
        let periods = PeriodTotals::build(parts.requested_end_h, &executions, &parts.deliveries, &parts.piles);
        let inventory = inventory_curves(&parts.piles, &parts.deliveries);
        let hourly = hourly_receipts(parts.requested_end_h, parts.grades.len(), &parts.deliveries);
        let mut truck_use: HashMap<TruckClassId, Vec<(f64, f64)>> = HashMap::new();
        for delivery in &parts.deliveries {
            let duration = delivery.end_h - delivery.start_h;
            if duration > 0.0 {
                let units = delivery.truck_hours / duration;
                let events = truck_use.entry(delivery.truck).or_default();
                events.push((delivery.start_h, units));
                events.push((delivery.end_h, -units));
            }
        }
        for events in truck_use.values_mut() {
            events.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut knots: Vec<(f64, f64)> = Vec::new();
            let mut units = 0.0;
            for &(hour, delta) in events.iter() {
                units += delta;
                if let Some(last) = knots.last_mut()
                    && last.0 == hour
                {
                    last.1 = units.max(0.0);
                } else {
                    knots.push((hour, units.max(0.0)));
                }
            }
            *events = knots;
        }
        Self {
            run: parts.run,
            semantic: parts.semantic,
            generation: parts.generation,
            requested_end_h: parts.requested_end_h,
            executions,
            deliveries: parts.deliveries,
            ground: parts.ground,
            piles: parts.piles,
            chunk_draws: parts.chunk_draws,
            idle,
            bar_blocks: parts.bar_blocks,
            reclaim_caps: parts.reclaim_caps,
            grades: parts.grades,
            grade_targets: parts.grade_targets,
            report: parts.report,
            drill_blast: parts.drill_blast,
            periods,
            by_bar,
            by_block,
            inventory,
            hourly,
            truck_use,
        }
    }

    /// The spans worked under one bar, in time order per loader.
    /// Give every idle span a reason, interval by interval: each span is cut
    /// at the calendar intervals (`(start_h, end_h)`, in order) it crosses,
    /// `reason` is asked about each piece, and neighbouring pieces with the
    /// same reason are joined again.
    pub(crate) fn classify_idle(&mut self, intervals: &[(f64, f64)], reason: impl Fn(&Self, LoaderAgentId, usize) -> (IdleReason, Vec<DestinationId>, Option<usize>)) {
        let mut classified: Vec<IdleSpan> = Vec::with_capacity(self.idle.len());
        for span in &self.idle {
            let first = intervals.partition_point(|&(_, end_h)| end_h <= span.start_h + 1e-9);
            for (position, &(start_h, end_h)) in intervals.iter().enumerate().skip(first) {
                if start_h >= span.end_h - 1e-9 {
                    break;
                }
                let (found, full, blast) = reason(self, span.agent, position);
                let piece = IdleSpan {
                    agent: span.agent,
                    start_h: start_h.max(span.start_h),
                    end_h: end_h.min(span.end_h),
                    reason: Some(found),
                    full,
                    blast,
                };
                match classified.last_mut() {
                    Some(last)
                        if last.agent == piece.agent
                            && last.reason == piece.reason
                            && last.full == piece.full
                            && last.blast == piece.blast
                            && (last.end_h - piece.start_h).abs() < 1e-9 =>
                    {
                        last.end_h = piece.end_h;
                    }
                    _ => classified.push(piece),
                }
            }
        }
        self.idle = classified;
    }

    pub(crate) fn bar_executions(&self, bar: BarId) -> impl Iterator<Item = &Execution> {
        self.by_bar.get(&bar).into_iter().flatten().map(|index| &self.executions[*index])
    }

    pub(crate) fn bar_tonnes(&self, bar: BarId) -> f64 {
        self.bar_executions(bar).map(|execution| execution.tonnes).sum()
    }

    pub(crate) fn ground(&self, block: DigBlockId) -> Option<&GroundBalance> {
        self.by_block.get(&block).map(|index| &self.ground[*index])
    }

    fn blocks_of(&self, bar: BarId) -> Option<&[DigBlockId]> {
        self.bar_blocks.iter().find(|(id, _)| *id == bar).map(|(_, blocks)| blocks.as_slice())
    }

    /// What this bar's ground still holds at the end of the calculation.
    /// Material left in the block, not lost work: any bar naming that ground
    /// may take it later.
    pub(crate) fn bar_left_behind(&self, bar: BarId) -> f64 {
        let Some(blocks) = self.blocks_of(bar) else { return 0.0 };
        let mut seen = Vec::with_capacity(blocks.len());
        blocks
            .iter()
            .filter(|block| {
                let fresh = !seen.contains(*block);
                seen.push(**block);
                fresh
            })
            .filter_map(|block| self.ground(*block))
            .map(|balance| balance.remaining_t)
            .sum()
    }

    /// When every block this bar names had run out, through any bar or
    /// loader, or `None` while one still holds material.
    pub(crate) fn bar_completion_h(&self, bar: BarId) -> Option<f64> {
        let blocks = self.blocks_of(bar)?;
        let mut completion = 0.0_f64;
        for block in blocks {
            let balance = self.ground(*block)?;
            if balance.remaining_t > 1e-6 {
                return None;
            }
            completion = completion.max(balance.emptied_h.unwrap_or(0.0));
        }
        (!blocks.is_empty()).then_some(completion)
    }

    /// What one reclaim bar had drawn by `hour`, and its cap.
    pub(crate) fn reclaim_progress(&self, bar: BarId, hour: f64) -> (f64, Option<f64>) {
        let drawn = self
            .bar_executions(bar)
            .filter(|execution| execution.activity == Activity::Reclaim)
            .map(|execution| {
                if execution.end_h <= hour {
                    execution.tonnes
                } else if execution.start_h >= hour {
                    0.0
                } else {
                    execution.rate_tph() * (hour - execution.start_h)
                }
            })
            .sum();
        let cap = self.reclaim_caps.iter().find(|(id, _)| *id == bar).and_then(|(_, cap)| *cap);
        (drawn, cap)
    }

    /// Deliveries made by one loader under one bar from one source over one
    /// span, for a tooltip.
    pub(crate) fn deliveries_of<'a>(&'a self, execution: &'a Execution) -> impl Iterator<Item = &'a Delivery> + 'a {
        self.deliveries.iter().filter(move |delivery| {
            delivery.agent == execution.agent
                && delivery.bar == execution.bar
                && delivery.source == execution.source
                && delivery.start_h < execution.end_h - 1e-9
                && delivery.end_h > execution.start_h + 1e-9
        })
    }

    /// Physical inventory at the cursor, interpolated at actual solved rates.
    /// Binary search over an index built once when the result is published.
    pub(crate) fn inventory_at(&self, pile: DestinationId, hour: f64) -> Option<(f64, Vec<f64>)> {
        let curve = self.inventory.get(&pile)?;
        let hour = hour.clamp(0.0, self.requested_end_h);
        let index = curve.partition_point(|knot| knot.hour <= hour).saturating_sub(1);
        let knot = &curve[index];
        let elapsed = hour - knot.hour;
        Some((
            (knot.tonnes + elapsed * knot.rate).max(0.0),
            knot.contained
                .iter()
                .zip(&knot.contained_rate)
                .map(|(quantity, rate)| (quantity + elapsed * rate).max(0.0))
                .collect(),
        ))
    }

    /// The most a stockpile held at any instant of the calculation. Exact:
    /// the balance is linear between knots.
    pub(crate) fn inventory_peak(&self, pile: DestinationId) -> f64 {
        self.inventory
            .get(&pile)
            .into_iter()
            .flatten()
            .map(|knot| knot.tonnes)
            .chain(
                self.inventory
                    .get(&pile)
                    .and_then(|curve| curve.last())
                    .map(|knot| knot.tonnes + (self.requested_end_h - knot.hour).max(0.0) * knot.rate),
            )
            .fold(0.0, f64::max)
    }

    /// What a destination received hour by hour, or `None` when it received
    /// nothing at all.
    pub(crate) fn truck_use_knots(&self, class: TruckClassId) -> &[(f64, f64)] {
        self.truck_use.get(&class).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn trucks_in_use(&self, class: TruckClassId, hour: f64) -> f64 {
        let Some(knots) = self.truck_use.get(&class) else { return 0.0 };
        let at = knots.partition_point(|(h, _)| *h <= hour);
        at.checked_sub(1).map_or(0.0, |i| knots[i].1)
    }

    pub(crate) fn hourly_receipts(&self, destination: DestinationId) -> Option<&HourlyReceipts> {
        self.hourly.get(&destination)
    }

    /// Which chunks of an ordered chunked pile were drawn during the calendar
    /// intervals a span falls in. Pile-wide per interval: when two loaders
    /// reclaim one pile in the same interval, the draw is theirs together.
    pub(crate) fn chunk_draws_during(&self, pile: DestinationId, start_h: f64, end_h: f64) -> impl Iterator<Item = &ChunkDraw> {
        self.chunk_draws
            .iter()
            .filter(move |draw| draw.pile == pile && draw.start_h < end_h - 1e-9 && draw.end_h > start_h + 1e-9)
    }

    /// The spans being worked at `hour`. Half-open, so an instant on a
    /// boundary belongs to the span that starts there.
    pub(crate) fn executions_at(&self, hour: f64) -> impl Iterator<Item = &Execution> {
        self.executions.iter().filter(move |execution| execution.start_h <= hour && hour < execution.end_h)
    }

    /// The movements under way at `hour`, half-open like [`Self::executions_at`].
    pub(crate) fn deliveries_at(&self, hour: f64) -> impl Iterator<Item = &Delivery> {
        self.deliveries.iter().filter(move |delivery| delivery.start_h <= hour && hour < delivery.end_h)
    }

    /// The idle span one loader is in at `hour`, if it is idle then.
    pub(crate) fn idle_at(&self, agent: LoaderAgentId, hour: f64) -> Option<&IdleSpan> {
        self.idle.iter().find(|span| span.agent == agent && span.start_h <= hour && hour < span.end_h)
    }

    /// Tonnes and contained quantity per grade a destination received over
    /// `[from_h, to_h)`, each delivery counted for the part of it inside.
    pub(crate) fn received_between(&self, destination: DestinationId, from_h: f64, to_h: f64) -> (f64, Vec<f64>) {
        let mut tonnes = 0.0;
        let mut contained = vec![0.0; self.grades.len()];
        for delivery in self.deliveries.iter().filter(|delivery| delivery.destination == destination) {
            let duration = delivery.end_h - delivery.start_h;
            let overlap = delivery.end_h.min(to_h) - delivery.start_h.max(from_h);
            if duration <= 0.0 || overlap <= 0.0 {
                continue;
            }
            let share = overlap / duration;
            tonnes += delivery.tonnes * share;
            for (total, quantity) in contained.iter_mut().zip(&delivery.contained) {
                *total += quantity * share;
            }
        }
        (tonnes, contained)
    }

    /// When a destination last finished receiving, at or before `hour`.
    pub(crate) fn last_receipt_before(&self, destination: DestinationId, hour: f64) -> Option<f64> {
        self.deliveries
            .iter()
            .filter(|delivery| delivery.destination == destination && delivery.tonnes > 0.0 && delivery.end_h <= hour + 1e-9)
            .map(|delivery| delivery.end_h)
            .reduce(f64::max)
    }

    /// The last hour anything was worked, for framing a view.
    pub(crate) fn last_activity_h(&self) -> f64 {
        self.executions.iter().map(|execution| execution.end_h).fold(0.0, f64::max)
    }
}
