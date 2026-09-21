//! The blended-stockpile scenario contract, shared by both experimental
//! blended solvers and by the independent replay.
//!
//! Nothing in this file knows about a solver. [`BlendInput`] is the scenario;
//! [`super::replay`] is the verdict on a schedule. The SCIP formulation in
//! [`crate::model::schedule::optimisation::scip::blend`] and the iterative
//! HiGHS method in [`super::iterative`] are two ways of getting from one to
//! the other, and the comparison between them is only meaningful because they
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
//! dimensionless mass fraction times tonnes):
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
    Activity, CashflowRuleId, Destination, DestinationId, DestinationKind, GroundSource, Interval, IntervalRate, Loader, LoaderId, MovementCandidate, ReclaimOrder, RoutingRuleId,
    SourceId, StockpileId, Task, TaskKind, TruckClass,
};

/// A blended pile: opening lots already combined by tonnes and contained
/// quantity, because a blend has no ordered lots to preserve.
#[derive(Clone, Debug, PartialEq)]
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
    /// chunk holding opening material is closed from the start, so it is
    /// immediately reclaimable. Empty means every chunk starts empty and
    /// [`Self::opening_t`] goes into chunk 0.
    pub(crate) chunk_opening: Vec<(f64, Vec<f64>)>,
}

impl BlendPile {
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

    /// Combine authored opening lots explicitly. The caller must pass the
    /// lots' material grades; this never guesses one.
    pub(crate) fn combine(id: StockpileId, capacity_t: f64, lots: &[(f64, Vec<f64>)], grades: usize) -> Self {
        let mut opening_t = 0.0;
        let mut opening_q = vec![0.0; grades];
        for (tonnes, fractions) in lots {
            opening_t += tonnes;
            for (slot, fraction) in opening_q.iter_mut().zip(fractions) {
                *slot += tonnes * fraction;
            }
        }
        Self {
            id,
            capacity_t,
            opening_t,
            opening_q,
            chunks: Vec::new(),
            order: ReclaimOrder::Fifo,
            chunk_opening: Vec::new(),
        }
    }
}

/// One end of a grade interval, in mass fraction.
#[derive(Clone, Copy, Debug, PartialEq)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
/// # Both directions, not permission
///
/// The rule applies exactly when its bounds hold. A positive value may not be
/// claimed on a blend that fails them, and a negative one may not be avoided
/// on a blend that meets them - the optimiser does not get to decide whether
/// an otherwise matching rule applies. That is why the model builds a *truth*
/// indicator for the predicate rather than a one-way permission.
#[derive(Clone, Debug, PartialEq)]
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
}

/// Shared scenario input for the blended experiments. Deliberately a separate
/// contract from the accepted `OptimisationInput`: its stockpile semantics
/// differ, so reusing the same type would invite comparing objectives that do
/// not measure the same thing.
#[derive(Clone, Debug, PartialEq)]
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
}

/// Numerical convention for a grade boundary.
///
/// SCIP satisfies constraints to `numerics/feastol` (1e-6 by default), so a
/// bare `>=` on a blended grade can be *met* with up to that much violation:
/// ask for 0.62 and the answer can genuinely be 0.619999.
///
/// The condition is therefore always tightened, never relaxed - the model
/// asks for `minimum + GRADE_MARGIN` so that after the solver's own slack the
/// delivered grade still clears `minimum`. An exclusive bound is tightened by
/// a further margin so the boundary value itself fails.
///
/// The replay deliberately does **not** use this margin. It checks the
/// authored boundary itself, with only enough slack for floating-point
/// arithmetic, because its job is to catch a violated boundary rather than to
/// agree with the model's safety cushion. An earlier revision had the sign of
/// this margin the wrong way round; the replay reported deliveries at
/// 0.619999 against a 0.62 minimum, which is how the error was found.
///
/// # The separation convention, stated once
///
/// One margin is used for every grade boundary in this model - destination
/// eligibility, conditional cashflow, and both backends - so there is a
/// single documented answer to "where exactly is the boundary".
///
/// * **Eligibility** (§5) is tightened only. A route admitted by `Fe >= 0.62`
///   is modelled as `Fe >= 0.62 + margin`, and by `Fe > 0.62` as
///   `Fe >= 0.62 + 2 x margin`, so after the solver's own feasibility slack
///   the delivered grade still clears what was authored. An upper bound is
///   tightened downwards by the same amounts. A destination is never opened
///   on a grade that fails the rule; it may, by at most one margin, be closed
///   on a grade that passes it.
///
/// * **Conditional cashflow** (§6) needs the predicate's *truth*, not a
///   permission, because a negative rule the solver could decline to apply
///   would not be a cost at all. The indicator is therefore exact on both
///   sides, and that has a price which is stated rather than hidden: a blend
///   within one margin of a payment boundary satisfies neither the "true" nor
///   the "false" rows, so the model excludes a band of width `2 x margin`
///   around each such boundary. At 1e-6 in mass fraction that is 0.0001% Fe.
///   It is a real restriction on the blends a schedule may hold, and it is
///   the price of making a conditional cost impossible to avoid.
///
/// Strict inequalities are not representable as open sets by ordinary solver
/// rows, so `>` is modelled as `>=` one margin further in. The operator the
/// planner wrote is preserved; only the numerical separation is introduced,
/// and the replay still checks the authored boundary itself.
pub(crate) const GRADE_MARGIN: f64 = 1e-6;
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
pub(crate) fn task_operable(loader: &Loader, task: &Task, interval: usize) -> bool {
    let Some(rate) = interval_rate(loader, interval) else { return false };
    match task.kind {
        TaskKind::Dig { .. } => rate.dig_tph > 0.0,
        TaskKind::Reclaim { .. } => rate.reclaim_tph > 0.0,
    }
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

pub(crate) fn delivers_to_pile(candidate: &MovementCandidate, pile: StockpileId, destinations: &BTreeMap<DestinationId, &Destination>) -> bool {
    destinations
        .get(&candidate.destination)
        .is_some_and(|destination| matches!(destination.kind, DestinationKind::Stockpile(target) if target == pile))
}
