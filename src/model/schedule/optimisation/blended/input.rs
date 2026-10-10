//! The blended-stockpile scenario contract, shared by the hourly dispatch,
//! Improve's SCIP formulation and the independent replay.
//!
//! Nothing in this file knows about a solver. [`BlendInput`] is the scenario;
//! [`super::replay`] is the verdict on a schedule. The hourly dispatch in
//! [`super::greedy`] and the SCIP formulation behind Improve are two ways of
//! getting from one to the other, and they are only comparable because they
//! are handed the *same* `BlendInput`.
//!
//! # The blended-stockpile semantics this contract encodes
//!
//! # Stockpile semantics (the discrete boundary-mixing approximation)
//!
//! A blended pile has no ordered positions. Its reclaimed composition is the
//! pile's own average, which makes the relationship nonlinear. This model
//! adopts the receipt-release approximation the brief fixes for the
//! experiment, and it is an approximation with a name and stated edges - not
//! continuous perfect mixing:
//!
//! - Opening stock is available immediately.
//! - A reclaim during interval `k` draws from interval `k`'s *released
//!   opening* blend.
//! - Receipts during `k` occupy capacity as they arrive, but join the
//!   reclaimable blend only at the boundary into `k + 1`.
//! - Every reclaim movement inside `k` therefore sees the *same* blend,
//!   whatever order it happens in.
//! - A pile cannot reclaim more than its released opening tonnes.
//! - Execution segments continue to govern loaders, trucks and capacity; only
//!   the inventory state lives at interval granularity.
//!
//! For a single blended pile FIFO/LIFO has no meaning, so the authored
//! [`ReclaimOrder`] is deliberately *not* consulted. Opening lots are combined
//! by the input builder, explicitly, into one tonnage and one contained
//! quantity per grade.
//!
//! # The mass balance
//!
//! For pile `p`, interval `k`, grade `g`, writing `T` for tonnes and `Q` for
//! contained quantity (units: `Q` is tonnes of the graded component, so a
//! tonnes times the grade in its captured numeric scale):
//!
//! ```text
//! T_open[p, k+1] = T_open[p, k] + T_recv[p, k] - T_recl[p, k]
//! Q_open[p, k+1, g] = Q_open[p, k, g] + Q_recv[p, k, g] - Q_recl[p, k, g]
//! ```
//!
//! Receipts are known composition - they arrive from a dig or a rehandle of
//! *identified* material - so `Q_recv` is a linear function of the movement
//! tonnes and that material's grade fraction. The pile's own draw is not:
//!
//! ```text
//! Q_recl[p, k, g] / T_recl[p, k] = Q_open[p, k, g] / T_open[p, k]
//! ```
//!
//! Division by a variable that can legitimately be zero is inadmissible, so
//! the model carries the cleared form, which is a nonconvex bilinear
//! equality:
//!
//! ```text
//! Q_recl[p, k, g] * T_open[p, k] - T_recl[p, k] * Q_open[p, k, g] = 0
//! ```
//!
//! # Why that equality alone is not enough
//!
//! At `T_open = 0` the cleared equality degenerates to `0 = 0` and stops
//! constraining `Q_recl` at all - an empty pile could supply contained metal
//! from nothing. The model therefore also carries *grade-box* rows, which are
//! physically true independently and are what actually forbids phantom metal:
//!
//! ```text
//! 0 <= Q_recl[p, k, g] <= gmax[g] * T_recl[p, k]
//! 0 <= Q_open[p, k, g] <= gmax[g] * T_open[p, k]
//! ```
//!
//! with `gmax[g]` the highest fraction any material in the problem carries -
//! a finite, physically derived bound, not a big-M. Together with
//! `T_recl <= T_open` these make "zero inventory supplies nothing" structural
//! rather than something the solver has to discover, and they tighten the
//! bilinear relaxation considerably.

use std::collections::BTreeMap;

use super::grade::GradeTable;
use crate::model::schedule::optimisation::{
    Activity, CashflowRuleId, Destination, DestinationId, DestinationKind, GroundId, GroundSource, Interval, IntervalRate, Loader, LoaderId, MaterialId, MovementCandidate,
    ReclaimOrder, RoutingRuleId, SourceId, StockpileId, Task, TaskKind, TruckClass,
};
pub(crate) use crate::model::schedule::stockpile_operation::PileMode;

/// Hours by which a rest may fall short, so a boundary computed two ways
/// agrees.
pub(crate) const REST_TOLERANCE_H: f64 = 1e-6;

/// Tonnes short of its capacity at which a chunk counts as full and may
/// close. Looser than the dispatcher's own snap, so a chunk it fills is full
/// to the model as SCIP checks it.
pub(crate) const CHUNK_FULL_T: f64 = 1e-3;

/// The least an interval's deliveries to a pile may total and still restart
/// its rest, as the replay and the dispatcher count it. The model needs twice
/// it to count a delivery (see `formulation::REST_FLAG_T`).
pub(crate) const REST_RECEIPT_T: f64 = 1e-4;

/// A blended pile: opening lots already combined by tonnes and contained
/// quantity, because a blend has no ordered lots to preserve.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct BlendPile {
    pub(crate) id: StockpileId,
    pub(crate) capacity_t: f64,
    pub(crate) opening_t: f64,
    /// Contained quantity per tracked grade, in tonnes of the component.
    pub(crate) opening_q: Vec<f64>,
    /// Authored chunk capacities, oldest first. Empty means one blended pile
    /// with no internal order, which is the §4 model; a non-empty list
    /// selects the §8 chunked variant, whose lifecycle is documented on
    /// [`super::chunks`].
    pub(crate) chunks: Vec<f64>,
    /// Which end of the chunk list a reclaim must draw from. Meaningless for
    /// an unchunked pile, where it is ignored rather than reinterpreted.
    pub(crate) order: ReclaimOrder,
    /// Opening material already sitting in each chunk, as
    /// `(tonnes, contained per grade)`, aligned with [`Self::chunks`]. A
    /// chunk opening full is closed from the start; one opening partly filled
    /// is the first to receive. Either is reclaimable once rested. Empty
    /// means every chunk starts empty and [`Self::opening_t`] goes into
    /// chunk 0.
    pub(crate) chunk_opening: Vec<(f64, Vec<f64>)>,
    /// Which chunks start closed - full, and taking no more - aligned with
    /// [`Self::chunks`]. Empty means the authored rule: a chunk opening full
    /// starts closed and every other chunk starts open. A day-by-day window
    /// sets it from the state the day before left, where an emptied chunk
    /// that once filled is closed for good.
    pub(crate) chunk_closed: Vec<bool>,
    /// The authored mode of each calendar day from hour 0. A day past the
    /// end builds and reclaims.
    #[serde(default)]
    pub(crate) modes: Vec<PileMode>,
    /// No deliveries in an interval the pile is reclaimed in.
    #[serde(default)]
    pub(crate) exclusive: bool,
    /// Hours new material rests before reclaim: since the last delivery to
    /// the pile, or for a chunked one to the chunk.
    #[serde(default)]
    pub(crate) rest_h: f64,
    /// End of the last interval before this input's first in which the pile
    /// received anything; `None` when opening stock is all it holds, which is
    /// rested. Set by a day-by-day window.
    #[serde(default)]
    pub(crate) last_receipt_h: Option<f64>,
    /// End of the last interval before this input's first in which each
    /// chunk received anything, aligned with [`Self::chunks`]; `None` or
    /// missing means long enough ago to be rested. Set by a day-by-day
    /// window.
    #[serde(default)]
    pub(crate) chunk_received_h: Vec<Option<f64>>,
}

impl BlendPile {
    pub(crate) fn mode(&self, interval: Interval) -> PileMode {
        self.modes.get(interval.day() as usize).copied().unwrap_or_default()
    }

    /// Whether the pile may take deliveries in `interval`.
    pub(crate) fn builds(&self, interval: Interval) -> bool {
        self.mode(interval).builds()
    }

    /// Whether the pile may be reclaimed in `interval`.
    pub(crate) fn reclaims(&self, interval: Interval) -> bool {
        self.mode(interval).reclaims()
    }

    /// Whether material delivered in an interval ending at `received_end_h`
    /// has rested by `interval`.
    pub(crate) fn rested(&self, received_end_h: f64, interval: Interval) -> bool {
        received_end_h <= interval.start_h - self.rest_h + REST_TOLERANCE_H
    }

    /// Whether chunk `chunk` has rested by `interval`, given the end of the
    /// last interval in this input it received in, if any; before that, the
    /// last receipt the input opens with.
    pub(crate) fn chunk_rested(&self, chunk: usize, received_h: Option<f64>, interval: Interval) -> bool {
        received_h
            .or_else(|| self.chunk_received_h.get(chunk).copied().flatten())
            .is_none_or(|received_h| self.rested(received_h, interval))
    }

    /// The pile's total opening state, which is the per-chunk opening when
    /// one was authored and [`Self::opening_t`] otherwise.
    ///
    /// One source of truth on purpose: the formulation reads the per-chunk
    /// figures and the replay reads the pile total, and an earlier revision
    /// let the two disagree - the replay reported a chunked pile supplying
    /// material "from an empty pile" because it had only counted chunk 0's
    /// opening stock.
    pub(crate) fn total_opening(&self, grades: usize) -> (f64, Vec<f64>) {
        if self.chunk_opening.is_empty() {
            let mut contained = self.opening_q.clone();
            contained.resize(grades, 0.0);
            return (self.opening_t, contained);
        }
        let mut tonnes = 0.0;
        let mut contained = vec![0.0; grades];
        for (chunk_t, chunk_q) in &self.chunk_opening {
            tonnes += chunk_t;
            for (slot, value) in contained.iter_mut().zip(chunk_q) {
                *slot += value;
            }
        }
        (tonnes, contained)
    }

    /// Whether chunk `c` is closed from the start: full, unless a window
    /// says otherwise.
    pub(crate) fn chunk_starts_closed(&self, c: usize) -> bool {
        if self.chunk_closed.is_empty() {
            let opening_t = match self.chunk_opening.get(c) {
                Some((tonnes, _)) => *tonnes,
                None if self.chunk_opening.is_empty() && c == 0 => self.opening_t,
                None => 0.0,
            };
            self.chunks.get(c).is_some_and(|capacity| opening_t > 0.0 && opening_t >= capacity - CHUNK_FULL_T)
        } else {
            self.chunk_closed.get(c).copied().unwrap_or(false)
        }
    }
}

/// One end of a grade interval in the captured numeric scale.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GradeEndpoint {
    pub(crate) value: f64,
    /// Whether the boundary value itself satisfies the test. Carried, never
    /// normalised away: `Fe >= 60` and `Fe > 60` are different instructions
    /// and the numerical convention below keeps them different.
    pub(crate) inclusive: bool,
}

/// One elementary half-space test on a blended grade.
///
/// Both an authored bound and its negation are one of these, which is what
/// lets a *truth* indicator be built for a condition rather than merely a
/// permission; see [`GradeBound`].
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GradeHalfSpace {
    pub(crate) grade: usize,
    /// `true` for `blend >= value` (or `>` when exclusive), `false` for
    /// `blend <= value` (or `<`).
    pub(crate) above: bool,
    pub(crate) endpoint: GradeEndpoint,
}

impl GradeHalfSpace {
    /// Whether a replayed blend satisfies this test at the *authored*
    /// boundary, with only enough slack for floating-point arithmetic.
    ///
    /// Deliberately not the formulation's safety margin: the replay's job is
    /// to catch a violated boundary, not to agree with the model's cushion.
    pub(crate) fn holds(self, blend: f64) -> bool {
        const ARITHMETIC_SLACK: f64 = 1e-9;
        match (self.above, self.endpoint.inclusive) {
            (true, true) => blend >= self.endpoint.value - ARITHMETIC_SLACK,
            (true, false) => blend > self.endpoint.value + ARITHMETIC_SLACK,
            (false, true) => blend <= self.endpoint.value + ARITHMETIC_SLACK,
            (false, false) => blend < self.endpoint.value - ARITHMETIC_SLACK,
        }
    }

    /// The test that is true exactly when this one is false.
    ///
    /// Exact set complement: the negation of `>= v` is `< v`, and the
    /// negation of `> v` is `<= v`. Inclusivity flips with the direction, so
    /// nothing is lost and no boundary is quietly reassigned to both sides.
    #[cfg_attr(not(feature = "scip"), allow(dead_code, reason = "used by Improve"))]
    pub(crate) fn negated(self) -> Self {
        Self {
            grade: self.grade,
            above: !self.above,
            endpoint: GradeEndpoint {
                value: self.endpoint.value,
                inclusive: !self.endpoint.inclusive,
            },
        }
    }
}

/// One authored numerical condition: a grade and the interval it must fall
/// in, either end independently open, closed or absent.
///
/// `60 < Fe < 70` is one of these with two exclusive endpoints; `Fe >= 60` is
/// one with a lower endpoint only. Nothing here widens an interval or makes
/// an endpoint inclusive.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GradeBound {
    pub(crate) grade: usize,
    pub(crate) lower: Option<GradeEndpoint>,
    pub(crate) upper: Option<GradeEndpoint>,
}

impl GradeBound {
    /// The elementary tests this bound is the conjunction of: one, two, or -
    /// for a bound with neither end, which capture refuses - none.
    pub(crate) fn half_spaces(self) -> Vec<GradeHalfSpace> {
        let mut tests = Vec::with_capacity(2);
        if let Some(endpoint) = self.lower {
            tests.push(GradeHalfSpace {
                grade: self.grade,
                above: true,
                endpoint,
            });
        }
        if let Some(endpoint) = self.upper {
            tests.push(GradeHalfSpace {
                grade: self.grade,
                above: false,
                endpoint,
            });
        }
        tests
    }

    pub(crate) fn holds(self, blend: &[f64]) -> bool {
        let value = blend.get(self.grade).copied().unwrap_or(0.0);
        self.half_spaces().into_iter().all(|test| test.holds(value))
    }
}

/// One authored rule's complete grade predicate: its bounds, ANDed.
///
/// An empty bound list is a rule that permits on identity alone, which is a
/// predicate that is always true rather than a rule with no effect.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GradePredicate {
    /// Which authored rule this came from, so an explanation can name it and
    /// so two rules that happen to be numerically identical stay two
    /// alternatives in the result.
    pub(crate) rule: RoutingRuleId,
    pub(crate) bounds: Vec<GradeBound>,
}

impl GradePredicate {
    pub(crate) fn holds(&self, blend: &[f64]) -> bool {
        self.bounds.iter().all(|bound| bound.holds(blend))
    }

    #[cfg_attr(not(feature = "scip"), allow(dead_code, reason = "used by Improve"))]
    pub(crate) fn unconditional(&self) -> bool {
        self.bounds.is_empty()
    }

    /// The elementary tests this predicate is the conjunction of.
    pub(crate) fn half_spaces(&self) -> Vec<GradeHalfSpace> {
        self.bounds.iter().flat_map(|bound| bound.half_spaces()).collect()
    }
}

/// What admits reclaimed material from one pile, moved by one loader, into
/// one destination.
///
/// # OR between rules, AND within a rule
///
/// Every enabled rule whose identity filters accept this loader, this pile
/// and this destination contributes one alternative. The destination is
/// permitted when **at least one** alternative holds on the interval's blend,
/// and each alternative holds when **all** of its bounds do.
///
/// The alternatives are kept apart rather than merged into one interval, and
/// that is the whole point of the structure. `Fe <= 55` and `Fe >= 65` into
/// one destination admit 50 and 70 and must not admit 60; a single widened
/// range `Fe <= 55 or... <= 70` would admit it. Two rules are two statements.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GradeQualification {
    pub(crate) loader: LoaderId,
    pub(crate) pile: StockpileId,
    pub(crate) destination: DestinationId,
    /// Never empty: a destination nothing admits produces no movement
    /// candidate, so there is nothing to qualify.
    pub(crate) alternatives: Vec<GradePredicate>,
}

/// Minimum-only fixture contract retained for the older developer scenarios.
/// Real-project capture emits [`GradeQualification`] instead.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GradeLimit {
    pub(crate) destination: DestinationId,
    pub(crate) grade: usize,
    pub(crate) minimum: f64,
    pub(crate) inclusive: bool,
}

impl GradeQualification {
    /// Whether any alternative holds - the disjunction, evaluated directly.
    pub(crate) fn holds(&self, blend: &[f64]) -> bool {
        self.alternatives.iter().any(|alternative| alternative.holds(blend))
    }

    /// Whether some alternative permits unconditionally, in which case the
    /// model needs no rows at all for this destination.
    #[cfg_attr(not(feature = "scip"), allow(dead_code, reason = "used by Improve"))]
    pub(crate) fn unconditional(&self) -> bool {
        self.alternatives.iter().any(GradePredicate::unconditional)
    }
}

/// One grade-conditional cashflow contribution to one movement candidate.
///
/// Kept out of [`crate::model::schedule::optimisation::CashflowContribution`]
/// on purpose: the accepted backend resolves every condition at capture,
/// because the material it moves has a known composition. Only a blended
/// pile's grade is a decision variable, so only this contract needs a value
/// that is contingent on one.
///
/// # Conservative sign-aware permissions
///
/// The rule applies exactly when its bounds hold. A positive value may not be
/// claimed on a blend that fails them, and a negative one may not be avoided
/// on a blend that meets them - the optimiser does not get to decide whether
/// an otherwise matching rule applies. The model uses inward reward permission
/// and outward cost-escape permission rather than two-sided truth indicators,
/// so near-boundary blends remain feasible and are priced conservatively.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConditionalValue {
    /// Index into [`BlendInput::movements`].
    pub(crate) candidate: usize,
    pub(crate) rule: CashflowRuleId,
    /// Signed, on the schedule's tonnes basis, exactly as authored.
    pub(crate) value_per_tonne: f64,
    /// Every bound must hold. Never empty - an unconditional contribution is
    /// carried on the candidate itself.
    pub(crate) bounds: Vec<GradeBound>,
}

impl ConditionalValue {
    pub(crate) fn holds(&self, blend: &[f64]) -> bool {
        self.bounds.iter().all(|bound| bound.holds(blend))
    }

    pub(crate) fn half_spaces(&self) -> Vec<GradeHalfSpace> {
        self.bounds.iter().flat_map(|bound| bound.half_spaces()).collect()
    }

    /// Whether this blend lies within [`GRADE_MARGIN`] of any of this rule's
    /// boundaries - the band in which the model values the rule
    /// conservatively and the published (authored) value can differ from it.
    ///
    /// `tonnes` is what the interval reclaimed from the pile, over which the
    /// contained-quantity cushion [`GRADE_CUSHION_T`] is spread.
    pub(crate) fn near_boundary(&self, blend: &[f64], tonnes: f64) -> bool {
        let band = GRADE_MARGIN + GRADE_CUSHION_T / tonnes.max(GRADE_CUSHION_T) + 1e-9;
        self.half_spaces().into_iter().any(|test| {
            let value = blend.get(test.grade).copied().unwrap_or(0.0);
            (value - test.endpoint.value).abs() <= band
        })
    }
}

/// One day's soft grade target on a destination's receipts, in the captured
/// grade scale.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct BlendGradeTarget {
    pub(crate) destination: DestinationId,
    pub(crate) grade: usize,
    pub(crate) specification: crate::model::schedule::grade_targets::GradeTarget,
    /// The absolute calendar day whose receipts this target prices.
    pub(crate) day: u32,
}

impl BlendGradeTarget {
    pub(crate) fn applies(&self, hour: f64) -> bool {
        crate::model::schedule::grade_targets::target_day(hour) == self.day
    }
}

/// Shared scenario input for the blended experiments. Deliberately a separate
/// contract from the accepted `OptimisationInput`: its stockpile semantics
/// differ, so reusing the same type would invite comparing objectives that do
/// not measure the same thing.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct BlendInput {
    pub(crate) intervals: Vec<Interval>,
    pub(crate) segments_per_interval: usize,
    pub(crate) grades: GradeTable,
    pub(crate) piles: Vec<BlendPile>,
    pub(crate) loaders: Vec<Loader>,
    pub(crate) tasks: Vec<Task>,
    pub(crate) ground: Vec<GroundSource>,
    pub(crate) destinations: Vec<Destination>,
    pub(crate) trucks: Vec<TruckClass>,
    pub(crate) movements: Vec<MovementCandidate>,
    /// Destination eligibility for reclaimed material, one entry per
    /// (loader, pile, destination) the routing rules admit under a grade
    /// condition. A combination admitted unconditionally needs no entry.
    pub(crate) qualifications: Vec<GradeQualification>,
    /// Cashflow contributions whose predicate is on a blended grade.
    pub(crate) conditional_values: Vec<ConditionalValue>,
    /// Older developer scenarios only. Real project capture leaves this empty.
    pub(crate) grade_limits: Vec<GradeLimit>,
    #[serde(default)]
    pub(crate) grade_targets: Vec<BlendGradeTarget>,
    /// Prior receipts, keyed by target position and absolute period. A vector
    /// keeps the solver-process JSON contract independent of map key encoding.
    #[serde(default)]
    pub(crate) target_opening: Vec<(usize, u32, f64, f64)>,
    /// The drill and blast chain, when the project sequences it. Its ground
    /// is dug only once released; see [`super::drill_blast`].
    #[serde(default)]
    pub(crate) drill_blast: Option<super::drill_blast::DrillBlastInput>,
}

/// Numerical convention for a grade boundary in the captured numeric scale.
///
/// SCIP satisfies constraints to `numerics/feastol` (1e-6 by default), so a
/// bare `>=` on a blended grade can be *met* with up to that much violation:
/// ask for 0.62 and the answer can genuinely be 0.619999. Every model row that
/// tests a blend is therefore written one margin inside the authored
/// boundary, so that after the solver's own slack the replayed blend is still
/// on the side the row claimed.
///
/// The replay deliberately does **not** use this margin. It checks the
/// authored boundary itself, with only enough slack for floating-point
/// arithmetic, because its job is to catch a violated boundary rather than to
/// agree with the model's cushion.
///
/// # What the margin does to each kind of rule
///
/// Operators are never rewritten: `>=`, `>`, `<=` and `<` keep their meaning
/// in the replay and in every published figure. In the model, every operator
/// takes the same single margin - the boundary value itself sits inside the
/// cushion either way, so inclusive and exclusive differ only in which side
/// the replay counts the exact boundary on. For a threshold `v`:
///
/// | rule | model row | effect of the margin |
/// |---|---|---|
/// | route admitted by `Fe >= v` or `Fe > v` | open only when `blend >= v + m` | the route is **closed** for blends in `[v, v + m)`; a restriction, never an admission |
/// | route admitted by `Fe <= v` or `Fe < v` | open only when `blend <= v - m` | closed for blends in `(v - m, v]` |
/// | conditional **reward** on `Fe >= v` | earned only when `blend >= v + m` | a blend in `[v, v + m)` is not credited *by the optimiser*; the published value pays it |
/// | conditional **cost** on `Fe >= v` | escaped only when `blend <= v - m` | a blend in `(v - m, v)` is charged *by the optimiser*; the published value does not charge it |
///
/// So eligibility carries a genuine modelling restriction of width `m` inside
/// each authored bound, and conditional values carry none: no blend is
/// forbidden by a cashflow boundary, the optimiser merely values a blend
/// within `m` of one conservatively. The replay counts those deliveries and
/// the value they could shift (`ReplayReport::boundary_value_slack`) so the
/// difference between the model's objective and the published one is
/// reported rather than hidden.
///
/// At `m = 1e-6` in mass fraction the band is 0.0001 percentage points of a
/// percent-unit grade - far below assay precision, but stated because it is
/// not zero.
pub(crate) const GRADE_MARGIN: f64 = 1e-6;

/// The same convention in contained quantity: every model row testing a
/// blend also clears its boundary by this many tonnes of the graded
/// component. See `formulation::implies_half_space` for why a fraction margin
/// alone does not protect a small reclaim. For a delivery of `T` tonnes the
/// band this adds is `GRADE_CUSHION_T / T` in grade: 1e-8 at 1,000 t.
pub(crate) const GRADE_CUSHION_T: f64 = 1e-5;
/// The highest numeric grade any material in the scenario can carry, per
/// grade: the upper bound the model places on contained quantity.
///
/// Every source of material counts - the captured dig materials *and* the
/// authored opening stock of every pile and chunk. An earlier revision read
/// the dig materials alone, so a pile opening richer than every block in scope
/// (or any pile in a reclaim-only run, where there are no dig materials at
/// all) had its opening contained quantity bounded below its own authored
/// value, and every run was infeasible at hour zero.
pub(crate) fn grade_ceilings(input: &BlendInput) -> Vec<f64> {
    let grades = input.grades.count();
    let mut ceilings = input.grades.ceilings();
    ceilings.resize(grades, 0.0);
    let mut raise = |tonnes: f64, contained: &[f64]| {
        if tonnes > 0.0 {
            for (ceiling, quantity) in ceilings.iter_mut().zip(contained) {
                *ceiling = ceiling.max((quantity / tonnes).max(0.0));
            }
        }
    };
    for pile in &input.piles {
        let (tonnes, contained) = pile.total_opening(grades);
        raise(tonnes, &contained);
        for (tonnes, contained) in &pile.chunk_opening {
            raise(*tonnes, contained);
        }
    }
    ceilings
}

#[cfg_attr(not(feature = "scip"), allow(dead_code, reason = "used by Improve"))]
pub(crate) fn flat_cell(interval: usize, segment: usize, segments: usize) -> usize {
    interval * segments + segment
}

pub(crate) fn interval_rate(loader: &Loader, interval: usize) -> Option<&IntervalRate> {
    loader.rates.iter().find(|rate| rate.interval == interval)
}

pub(crate) fn loader_rate(input: &BlendInput, candidate: &MovementCandidate, interval: usize) -> Option<f64> {
    let loader = input.loaders.iter().find(|entry| entry.id == candidate.loader)?;
    let rate = interval_rate(loader, interval)?;
    Some(match candidate.activity {
        Activity::Dig => rate.dig_tph,
        Activity::Reclaim => rate.reclaim_tph,
    })
}

pub(crate) fn authored_tasks(input: &BlendInput, loader_index: usize) -> Vec<usize> {
    let loader = input.loaders[loader_index].id;
    let mut tasks: Vec<usize> = input.tasks.iter().enumerate().filter(|(_, task)| task.loader == loader).map(|(index, _)| index).collect();
    // The accepted backend's authored order, reproduced exactly: priority,
    // then window start, then task id.
    tasks.sort_by(|&left, &right| {
        let left = &input.tasks[left];
        let right = &input.tasks[right];
        (left.priority, left.window_start_h, left.id)
            .partial_cmp(&(right.priority, right.window_start_h, right.id))
            .expect("authored windows are finite")
    });
    tasks
}

/// The bar a dig of `ground` in `interval` is held to when the loader's
/// highest-priority ready bar does not hold the block: the first of the
/// loader's bars in authored order (`ordered`, from [`authored_tasks`]) whose
/// window covers the interval and whose sequence holds the block.
///
/// Only one bar's sequence orders a dig: the one it was worked under. Ground
/// is shared but membership is authored per bar, so a bar listing the block
/// in another order does not hold it back while it waits - on a blast, or
/// for room - and the loader digs it under another. A dig under no ready bar
/// holding it is a priority break in its own right, which this only keeps
/// ordered.
pub(crate) fn dig_authority(input: &BlendInput, ordered: &[usize], ground: GroundId, interval: Interval) -> Option<usize> {
    ordered.iter().copied().find(|&index| {
        let task = &input.tasks[index];
        task_active(task, interval) && matches!(&task.kind, TaskKind::Dig { sequence } if sequence.contains(&ground))
    })
}

/// Whether an authored bar covers a whole calendar interval.
///
/// The accepted backend requires *containment*, not overlap: a bar that
/// covers only part of an interval is not worked in that interval at all.
/// The blended model previously used overlap, which quietly widened every
/// authored window to the interval grid and let a loader work a bar outside
/// the hours a planner gave it. Same convention, same tolerance, so the two
/// models agree about where a window starts and stops.
pub(crate) fn task_active(task: &Task, interval: Interval) -> bool {
    task.window_start_h <= interval.start_h + WINDOW_TOLERANCE_H && task.window_end_h >= interval.end_h - WINDOW_TOLERANCE_H
}

/// Matches the accepted backend's window comparison tolerance.
pub(crate) const WINDOW_TOLERANCE_H: f64 = 1e-9;

/// Whether the loader can physically perform this bar's activity in this
/// interval. A zero rate means the bar has no work available, exactly as in
/// the accepted backend.
#[cfg_attr(not(feature = "scip"), allow(dead_code, reason = "used by Improve"))]
pub(crate) fn task_operable(loader: &Loader, task: &Task, interval: usize) -> bool {
    let Some(rate) = interval_rate(loader, interval) else { return false };
    match task.kind {
        TaskKind::Dig { .. } => rate.dig_tph > 0.0,
        TaskKind::Reclaim { .. } => rate.reclaim_tph > 0.0,
        // A delay holds the loader whatever its rate.
        TaskKind::Delay => true,
    }
}

/// Room at or below which a destination counts as full when judging whether
/// a dig bar has work. A machine is not held for an hour by a few tonnes.
pub(crate) const DIG_ROOM_T: f64 = 1.0;

/// Where a bar can send each material of a block, one list of destinations
/// per material the block holds ([`OutletIndex::of`]).
///
/// A dig bar's work is its current block - the first of its sequence with
/// ground left - and a block is dug whole, its materials in proportion. So
/// the bar has work only while every one of those materials has somewhere
/// with room ([`block_diggable`]). Otherwise the machine works its next bar
/// with work, and comes back as soon as there is room again. The hourly
/// dispatch, the formulation and the replay all judge it so, on room as the
/// interval opens: a stockpile building that day and more than
/// [`DIG_ROOM_T`] below its capacity, a dump more than that below its
/// capacity, a crusher more than that below its day's budget; unlimited ones
/// always have room.
pub(crate) struct OutletIndex {
    /// Dig candidates' destinations by (loader, block, material), sorted.
    /// Indexed once: walking every candidate for every readiness check is the
    /// candidate count times the horizon.
    digs: BTreeMap<(LoaderId, GroundId, MaterialId), Vec<DestinationId>>,
}

impl OutletIndex {
    pub(crate) fn new(input: &BlendInput) -> Self {
        let mut digs: BTreeMap<_, Vec<DestinationId>> = BTreeMap::new();
        for candidate in &input.movements {
            if let SourceId::Ground(ground) = candidate.source
                && candidate.activity == Activity::Dig
            {
                digs.entry((candidate.loader, ground, candidate.material)).or_default().push(candidate.destination);
            }
        }
        for outlets in digs.values_mut() {
            outlets.sort_unstable();
            outlets.dedup();
        }
        Self { digs }
    }

    pub(crate) fn of(&self, input: &BlendInput, task: &Task, ground: GroundId) -> Vec<Vec<DestinationId>> {
        let Some(source) = input.ground.iter().find(|source| source.id == ground) else {
            return Vec::new();
        };
        let authorised = matches!(&task.kind, TaskKind::Dig { sequence } if sequence.contains(&ground));
        source
            .material
            .iter()
            .filter(|share| share.fraction > 0.0)
            .map(|share| {
                authorised
                    .then(|| self.digs.get(&(task.loader, ground, share.material)).cloned())
                    .flatten()
                    .unwrap_or_default()
            })
            .collect()
    }
}

/// Whether every material of a block has a destination with room.
pub(crate) fn block_diggable(outlets: &[Vec<DestinationId>], has_room: impl Fn(DestinationId) -> bool) -> bool {
    outlets.iter().all(|destinations| destinations.iter().any(|destination| has_room(*destination)))
}

/// Whether an authored bar permits this movement's source.
pub(crate) fn task_authorises(task: &Task, candidate: &MovementCandidate) -> bool {
    if task.loader != candidate.loader {
        return false;
    }
    match (&task.kind, candidate.source) {
        (TaskKind::Dig { sequence }, SourceId::Ground(ground)) => candidate.activity == Activity::Dig && sequence.contains(&ground),
        (TaskKind::Reclaim { approved_sources, .. }, SourceId::Stockpile(pile)) => candidate.activity == Activity::Reclaim && approved_sources.contains(&pile),
        _ => false,
    }
}

/// The reclaim bars that `tonnes` of `candidate`, reclaimed in `interval`,
/// count against, given what each bar has already reclaimed.
///
/// The one attribution dispatch, replay and rolling carry share: the
/// loader's active bars that authorise the movement, in authored order, each
/// taking up to what is left of its cap. Anything past every cap falls to the
/// first, so an overdraw is still charged somewhere a check will see it.
pub(crate) fn attribute_reclaim(input: &BlendInput, candidate: &MovementCandidate, interval: Interval, mut tonnes: f64, reclaimed: &BTreeMap<usize, f64>) -> Vec<(usize, f64)> {
    let Some(loader) = input.loaders.iter().position(|loader| loader.id == candidate.loader) else {
        return Vec::new();
    };
    let tasks: Vec<usize> = authored_tasks(input, loader)
        .into_iter()
        .filter(|&task| task_active(&input.tasks[task], interval) && task_authorises(&input.tasks[task], candidate))
        .collect();
    let mut shares = Vec::new();
    for &task in &tasks {
        if tonnes <= 0.0 {
            break;
        }
        let room = match input.tasks[task].kind {
            TaskKind::Reclaim { maximum_t: Some(maximum), .. } => (maximum - reclaimed.get(&task).copied().unwrap_or(0.0)).max(0.0),
            _ => f64::INFINITY,
        };
        let share = tonnes.min(room);
        if share > 0.0 {
            shares.push((task, share));
            tonnes -= share;
        }
    }
    if tonnes > 0.0
        && let Some(&first) = tasks.first()
    {
        shares.push((first, tonnes));
    }
    shares
}

#[cfg_attr(not(feature = "scip"), allow(dead_code, reason = "used by Improve"))]
pub(crate) fn delivers_to_pile(candidate: &MovementCandidate, pile: StockpileId, destinations: &BTreeMap<DestinationId, &Destination>) -> bool {
    destinations
        .get(&candidate.destination)
        .is_some_and(|destination| matches!(destination.kind, DestinationKind::Stockpile(target) if target == pile))
}
