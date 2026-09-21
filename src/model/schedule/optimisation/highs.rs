//! Native `good_lp` formulation and the narrow HiGHS status boundary.
//!
//! # Execution segments
//!
//! Calendar intervals carry only input constancy (rates, fleets, windows,
//! daily budgets). Execution happens in *segments*: ordered event positions
//! inside each interval, shared by every loader, whose durations are decision
//! variables summing to the interval. Within one segment a loader works at
//! most one source at a constant rate; finishing a block, lot or parcel and
//! continuing to the next authored source is a segment boundary, not a
//! calendar boundary. The per-interval segment budget is derived from the
//! authored work - the union of every loader's possible boundary times, since
//! loaders finish at different times - so it cannot silently restrict
//! production; the result carries the budget, the ceiling that clamps it, and
//! a restriction heuristic so a binding budget is visible.
//!
//! # Solving: a seed, then two objective stages
//!
//! [`solve`] builds the model once and runs it three times through the same
//! HiGHS instance.
//!
//! First a *seed*: one block of rows holds every interval to its first
//! execution segment, which is the conservative schedule a planner would
//! recognise - no within-interval source transitions. Because it is the same
//! columns with strictly more constraints, its answer is feasible for the
//! unrestricted model by construction, so the rows are then relaxed in place
//! and the answer handed back as a starting point rather than re-validated.
//!
//! Then the objective stages. Movement value is maximised alone; only
//! afterwards are the tie preferences - routing preference, shorter haul
//! cycles, earlier receipt positions - optimised under a row holding the
//! primary objective at what the first stage reached. They are deliberately
//! *not* an epsilon-weighted addition to the first objective: see the note
//! above [`solve`] for what that cost. A caller whose budget runs out before
//! the second stage gets a primary-optimal answer with
//! `tie_preferences_applied` false.
//!
//! # Numerical note: the pinned tolerances and the inventory dust scale
//!
//! This formulation mixes dust-scale activation rows (parcel occupancy and
//! negligible-inventory thresholds) with tonnage rows in the thousands, so
//! the two are pinned together rather than chosen independently.
//!
//! The reproducer kept for this note is a two-interval fixture whose
//! stockpile opens with a lot of exactly the parcel target while a reclaim
//! bar works interval 0 and a dig bar feeds the pile. It once returned a
//! suboptimal answer *labelled optimal* - the dig collapsed to one parcel's
//! worth. The cause was this model, not the solver: the emptiness and
//! selection rows carried the whole pile's capacity as their big-M, so a
//! binary within the integrality tolerance of 1 could hide capacity x
//! tolerance of inventory, which at 1e3 tonnes and the default 1e-6 is 1e-3
//! tonnes - twenty times the dust scale those same rows switch on. With each
//! row's big-M now the unit's own bound (an opening lot's tonnage or the
//! parcel target) that fixture solves correctly at HiGHS's default
//! tolerances too.
//!
//! The pinned pair below is still load-bearing, for a different and stated
//! reason: [`inventory_dust`] derives the negligible-inventory scale from
//! [`MIP_FEASIBILITY_TOLERANCE`] and the largest unit bound, and the model,
//! the publication and the independent validator all compare against that
//! one figure. Loosening the tolerance without re-deriving the scale lets a
//! unit sit above the threshold while the model treats it as empty, and the
//! replayed FIFO/LIFO order then disagrees with the solved timeline (the
//! acceptance audit reproduced exactly that at the defaults). Change the two
//! together or not at all.
//!
//! # Known limitation: horizon scale
//!
//! Receipt parcels become inventory units, and every unit carries balance,
//! emptiness and chain columns in every segment after its release, so the
//! inventory block grows with (intervals x segments)^2 while the dig-only
//! model grows linearly.
//!
//! What binds first, though, is not the model's size but the solver's root
//! node. HiGHS caps root separation at about the square root of the integer
//! column count in rounds, and every round re-solves the root relaxation, so
//! integer columns cost twice. On the audit fixture (three dig loaders
//! feeding a pile, one reclaim bar, one-hour intervals) twelve hours solves
//! within two percent of its bound inside a sixty second budget, and
//! twenty-four hours never leaves the root: zero nodes explored, no
//! heuristic run, and therefore no incumbent - though the dual bound is
//! finite and correct throughout, so "no incumbent" stays distinct from
//! infeasible.
//!
//! That ceiling is not close enough to be reached by trimming the encoding:
//! a twenty-four hour model with a 200 t parcel target is *smaller* than the
//! twelve hour model that solves, and still returns nothing. The reasoning,
//! the alternatives measured and the one product approximation that would
//! change it are in `docs/scheduling-optimisation-backend.md`.
//!
//! An earlier revision was tractable further out only because it pruned
//! "deep" parcel positions from the FIFO order. That prune was unsound (see
//! the note above [`solve`]) and has been removed; the tractability it
//! bought was not real.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

use ::highs::{HighsModelStatus, HighsSolutionStatus};
use good_lp::{Expression, ProblemVariables, SolverModel, Variable, VariableDefinition, variable};

use super::*;

/// Pinned HiGHS primal feasibility tolerance (see the module note).
const PRIMAL_FEASIBILITY_TOLERANCE: f64 = 1e-9;

/// Pinned HiGHS MIP feasibility tolerance, which HiGHS also applies as the
/// integrality tolerance: a binary may sit this far from 0 or 1.
const MIP_FEASIBILITY_TOLERANCE: f64 = 1e-7;

/// Safety factor over [`MIP_FEASIBILITY_TOLERANCE`] when deriving the
/// inventory dust scale from the formulation's big-M coefficients.
const INTEGRALITY_MARGIN: f64 = 10.0;

/// Wall-time ceiling for the tie-preference stage when the caller set no
/// time limit, and the ceiling on the share of a caller's budget that stage
/// may consume. The stage only breaks ties, so it never earns an unbounded
/// share of a bounded budget.
const PREFERENCE_STAGE_BUDGET: Duration = Duration::from_secs(10);

/// Share of the caller's budget the seed stage may take, and its ceiling
/// when no budget was given. The seed only has to produce *a* schedule, so
/// it never earns the majority of a bounded budget.
const SEED_STAGE_SHARE: f64 = 0.25;

/// Ceiling on the seed stage's wall time.
const SEED_STAGE_BUDGET: Duration = Duration::from_secs(15);

/// Below this remaining budget a stage is skipped rather than started and
/// abandoned.
///
/// Note on budgets generally: HiGHS checks its own time limit between root
/// separation rounds, and on horizons large enough that a round takes tens of
/// seconds it overshoots. Every stage here is given an honest slice, and the
/// measured overshoot on the audit fixture is nil up to twelve hours and
/// about half as much again at twenty-four - which is the horizon that
/// already returns no incumbent.
const PREFERENCE_STAGE_MINIMUM: Duration = Duration::from_millis(250);

/// Experimental MPS export, used only by the SCIP investigation (see
/// `super::scip::experiments::parcel_via_mps`).
///
/// This exists so the *existing* formulation can be handed to another backend
/// without being rewritten for it. It changes nothing about how this module
/// solves: when no path has been requested the hook is a single `Option`
/// check, and the whole module compiles away without the feature.
#[cfg(feature = "scip-code")]
pub(super) mod mps_export {
    use std::{cell::RefCell, path::PathBuf};

    thread_local! {
        static TARGET: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    }

    /// Ask the next [`super::solve`] on this thread to write its model.
    pub(crate) fn request(path: PathBuf) {
        TARGET.with(|slot| *slot.borrow_mut() = Some(path));
    }

    pub(super) fn take() -> Option<PathBuf> {
        TARGET.with(|slot| slot.borrow_mut().take())
    }
}

#[derive(Clone, Copy)]
struct Column {
    variable: Variable,
    index: usize,
}

struct Columns {
    variables: ProblemVariables,
    count: usize,
    binaries: usize,
    binary_indices: Vec<usize>,
}

impl Columns {
    fn new() -> Self {
        Self {
            variables: ProblemVariables::new(),
            count: 0,
            binaries: 0,
            binary_indices: Vec::new(),
        }
    }
    fn add(&mut self, definition: VariableDefinition) -> Column {
        let index = self.count;
        self.count += 1;
        Column {
            variable: self.variables.add(definition),
            index,
        }
    }
    fn continuous(&mut self, upper: f64) -> Column {
        self.add(variable().min(0.0).max(upper))
    }
    fn binary(&mut self) -> Column {
        self.binaries += 1;
        let column = self.add(variable().binary());
        self.binary_indices.push(column.index);
        column
    }
}

fn sum(columns: impl IntoIterator<Item = (Column, f64)>) -> Expression {
    columns
        .into_iter()
        .fold(Expression::from(0.0), |expression, (column, coefficient)| expression + coefficient * column.variable)
}

#[derive(Clone)]
struct DigVariable {
    interval: usize,
    segment: usize,
    task: usize,
    stage: usize,
    source: GroundId,
    column: Column,
}

#[derive(Clone)]
struct MoveVariable {
    interval: usize,
    segment: usize,
    task: usize,
    candidate: usize,
    source: SourceId,
    material: MaterialId,
    unit: Option<UnitKey>,
    column: Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct StreamKey {
    pile: StockpileId,
    interval: usize,
    segment: usize,
    source: SourceId,
    material: MaterialId,
}

#[derive(Clone)]
struct ParcelVariable {
    stream: StreamKey,
    position: usize,
    quantity: Column,
    assigned: Column,
    remainder: Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum UnitKey {
    Opening {
        pile: StockpileId,
        opening: usize,
    },
    Receipt {
        pile: StockpileId,
        receipt_interval: usize,
        segment: usize,
        position: usize,
    },
}

#[derive(Clone)]
struct ReclaimVariable {
    interval: usize,
    segment: usize,
    task: usize,
    source: StockpileId,
    unit: UnitKey,
    material: MaterialId,
    fraction: f64,
    column: Column,
}

struct Formulation {
    columns: Columns,
    constraints: usize,
    objective: Expression,
    durations: BTreeMap<(usize, usize), Column>,
    dig: Vec<DigVariable>,
    movements: Vec<MoveVariable>,
    parcels: Vec<ParcelVariable>,
    reclaim: Vec<ReclaimVariable>,
    task_selected: BTreeMap<(usize, usize, usize), Column>,
    ground_remaining: BTreeMap<(GroundId, usize, usize), Column>,
    pile_closing: BTreeMap<(StockpileId, usize, usize), Column>,
    unit_start: BTreeMap<(UnitKey, usize, usize), Column>,
}

impl Formulation {
    fn new() -> Self {
        Self {
            columns: Columns::new(),
            constraints: 0,
            objective: Expression::from(0.0),
            durations: BTreeMap::new(),
            dig: Vec::new(),
            movements: Vec::new(),
            parcels: Vec::new(),
            reclaim: Vec::new(),
            task_selected: BTreeMap::new(),
            ground_remaining: BTreeMap::new(),
            pile_closing: BTreeMap::new(),
            unit_start: BTreeMap::new(),
        }
    }
}

fn empty_result(
    input: &OptimisationInput,
    budgets: &[usize],
    status: SolveStatus,
    message: Option<String>,
    started: Instant,
    formulation_time: Duration,
    formulation: &Formulation,
) -> OptimisationResult {
    OptimisationResult {
        summary: SolveSummary {
            status,
            objective: None,
            raw_solver_objective: None,
            best_bound: None,
            relative_gap: None,
            backend: "HiGHS 1.11.0 via highs 2.4.0 / good_lp 1.15.3",
            message,
            tie_preferences_applied: false,
            statistics: SolveStatistics {
                variables: formulation.columns.count,
                binaries: formulation.columns.binaries,
                constraints: formulation.constraints,
                formulation_time,
                solve_time: started.elapsed() - formulation_time,
            },
        },
        resolution: metadata(input, budgets),
        segment_durations_h: budgets.iter().map(|budget| vec![0.0; *budget]).collect(),
        used_segments: vec![0; budgets.len()],
        event_budget_restricted: false,
        publication_audit: PublicationAudit::default(),
        activities: Vec::new(),
        movements: Vec::new(),
        parcels: Vec::new(),
        reclaims: Vec::new(),
        balances: Vec::new(),
    }
}

/// The negligible-inventory scale, derived rather than chosen. A binary may
/// sit [`MIP_FEASIBILITY_TOLERANCE`] from integral, so a unit the model calls
/// "empty" can still retain up to its big-M coefficient times that tolerance.
/// The big-M on those rows is the unit's own bound (an opening lot's tonnage
/// or the parcel target, never the whole pile's capacity), so this figure
/// stays small; it is published in the resolution metadata because the model,
/// the publication snapping and the independent validator must agree on what
/// counts as live inventory.
fn inventory_dust(input: &OptimisationInput) -> f64 {
    let tolerance = input.horizon.tolerances;
    let dust = tolerance.tonnes_t.max(input.horizon.parcel_target_t * 1e-6);
    let largest_unit_bound = input
        .stockpiles
        .iter()
        .flat_map(|pile| {
            pile.opening
                .iter()
                .map(|lot| lot.tonnes_t.min(pile.capacity_t))
                .chain(std::iter::once(input.horizon.parcel_target_t.min(pile.capacity_t)))
        })
        .fold(0.0_f64, f64::max);
    dust.max(tolerance.parcel_occupancy_t)
        .max(largest_unit_bound * MIP_FEASIBILITY_TOLERANCE * INTEGRALITY_MARGIN)
}

fn metadata(input: &OptimisationInput, budgets: &[usize]) -> ResolutionMetadata {
    let mut edges = input.intervals.iter().map(|interval| interval.start_h).collect::<Vec<_>>();
    edges.push(input.horizon.end_h);
    ResolutionMetadata {
        horizon_h: input.horizon.end_h,
        regular_step_h: input.horizon.regular_step_h,
        parcel_target_t: input.horizon.parcel_target_t,
        segment_counts: budgets.to_vec(),
        segment_ceiling: input
            .horizon
            .event_segments
            .map_or(SEGMENT_CEILING, |override_count| (override_count as usize).clamp(1, SEGMENT_OVERRIDE_CEILING)),
        interval_edges_h: edges,
        inventory_dust_t: inventory_dust(input),
        tolerances: input.horizon.tolerances,
    }
}

/// A unit's balance carried out of one cell: what it started that cell with
/// less everything drawn from it there. Held as an expression rather than a
/// column - a closing column per unit per segment is one of the four families
/// that scale with (units x segments), and this one is pure substitution.
fn carried_balance(formulation: &Formulation, unit: UnitKey, cell: (usize, usize)) -> Expression {
    let start = formulation.unit_start[&(unit, cell.0, cell.1)];
    let mut drawn = BTreeMap::new();
    for reclaim in formulation
        .reclaim
        .iter()
        .filter(|reclaim| reclaim.unit == unit && reclaim.interval == cell.0 && reclaim.segment == cell.1)
    {
        drawn.entry(reclaim.column.index).or_insert(reclaim.column);
    }
    Expression::from(start.variable) - sum(drawn.values().copied().map(|column| (column, 1.0)))
}

fn task_order(input: &OptimisationInput, loader: LoaderId) -> Vec<usize> {
    let mut tasks: Vec<_> = input.tasks.iter().enumerate().filter(|(_, task)| task.loader == loader).map(|(index, _)| index).collect();
    tasks.sort_by(|&left, &right| {
        let left = &input.tasks[left];
        let right = &input.tasks[right];
        (left.priority, left.window_start_h, left.id)
            .partial_cmp(&(right.priority, right.window_start_h, right.id))
            .expect("finite windows")
    });
    tasks
}

fn active(task: &Task, interval: Interval) -> bool {
    task.window_start_h <= interval.start_h + 1e-9 && task.window_end_h >= interval.end_h - 1e-9
}

fn candidate_is_fixed_operable(input: &OptimisationInput, candidate: &MovementCandidate, interval: usize) -> bool {
    let Some(destination) = input.destinations.iter().find(|entry| entry.id == candidate.destination) else {
        return false;
    };
    let receiving = match destination.kind {
        DestinationKind::Crusher => destination
            .crusher_daily_t
            .get(input.intervals[interval].day() as usize)
            .copied()
            .flatten()
            .is_none_or(|limit| limit > 0.0),
        DestinationKind::Dump | DestinationKind::Stockpile(_) => destination.capacity_t.is_none_or(|capacity| capacity > 0.0),
    };
    let truck = input
        .trucks
        .iter()
        .find(|entry| entry.id == candidate.truck)
        .is_some_and(|entry| entry.hours[interval] > 0.0);
    receiving && truck
}

fn routes_for<'a>(
    input: &'a OptimisationInput,
    loader: LoaderId,
    activity: Activity,
    source: SourceId,
    material: MaterialId,
    interval: usize,
) -> impl Iterator<Item = (usize, &'a MovementCandidate)> + 'a {
    input.movements.iter().enumerate().filter(move |(_, candidate)| {
        candidate.loader == loader
            && candidate.activity == activity
            && candidate.source == source
            && candidate.material == material
            && candidate_is_fixed_operable(input, candidate, interval)
    })
}

fn source_operable(input: &OptimisationInput, task: &Task, source: SourceId, shares: &[MaterialShare], interval: usize) -> bool {
    let activity = match source {
        SourceId::Ground(_) => Activity::Dig,
        SourceId::Stockpile(_) => Activity::Reclaim,
    };
    shares
        .iter()
        .all(|share| routes_for(input, task.loader, activity, source, share.material, interval).next().is_some())
}

fn add_constraint<M: SolverModel>(model: &mut M, formulation: &mut Formulation, constraint: good_lp::Constraint) {
    model.add_constraint(constraint);
    formulation.constraints += 1;
}

/// How many event positions each interval needs.
///
/// Ground and inventory are shared: a source depletes at the *combined* rate
/// of every loader that can work it, so a loader's transitions are bounded by
/// walking its authored stages in order, charging each stage the time its
/// fastest possible joint depletion takes. A stage is counted as touched when
/// the stages before it fit strictly inside the interval - it may then be
/// started even if it cannot be finished. Reclaim adds one position of slack
/// for a stream remainder parcel, which is smaller than the target.
///
/// The event clock is shared and loaders finish at different times: each of a
/// loader's sources after its first needs a boundary, and distinct loaders'
/// boundary times need not coincide. The sound interval budget is therefore
/// the union bound - one plus the sum over loaders of each loader's internal
/// transitions - not the per-loader maximum. The derived budget is clamped by
/// [`SEGMENT_CEILING`] as a model-size guard; an explicit override may raise
/// it as far as [`SEGMENT_OVERRIDE_CEILING`] so a flagged run can be re-solved
/// with more event positions. Both the per-interval budget and the applied
/// ceiling are published in the result metadata.
fn segment_budgets(input: &OptimisationInput, ground_index: &BTreeMap<GroundId, &GroundSource>) -> Vec<usize> {
    if let Some(override_count) = input.horizon.event_segments {
        let count = (override_count as usize).clamp(1, SEGMENT_OVERRIDE_CEILING);
        return vec![count; input.intervals.len()];
    }
    input
        .intervals
        .iter()
        .map(|interval| {
            let duration = interval.duration_h();
            let combined_dig = |ground: GroundId| -> f64 {
                input
                    .loaders
                    .iter()
                    .filter(|loader| {
                        loader.rates[interval.index].dig_tph > 0.0
                            && input
                                .tasks
                                .iter()
                                .any(|task| task.loader == loader.id && active(task, *interval) && matches!(&task.kind, TaskKind::Dig { sequence } if sequence.contains(&ground)))
                    })
                    .map(|loader| loader.rates[interval.index].dig_tph)
                    .sum()
            };
            let combined_reclaim = |pile: StockpileId| -> f64 {
                input
                    .loaders
                    .iter()
                    .filter(|loader| {
                        loader.rates[interval.index].reclaim_tph > 0.0
                            && input.tasks.iter().any(|task| {
                                task.loader == loader.id
                                    && active(task, *interval)
                                    && matches!(&task.kind, TaskKind::Reclaim { approved_sources, .. } if approved_sources.contains(&pile))
                            })
                    })
                    .map(|loader| loader.rates[interval.index].reclaim_tph)
                    .sum()
            };
            let mut extra_boundaries = 0usize;
            let mut any_work = false;
            for loader in &input.loaders {
                let mut touched = 0usize;
                let mut reclaim_task = false;
                let mut time_used = 0.0;
                for &task_index in &task_order(input, loader.id) {
                    let task = &input.tasks[task_index];
                    if !active(task, *interval) {
                        continue;
                    }
                    match &task.kind {
                        TaskKind::Dig { sequence } => {
                            if loader.rates[interval.index].dig_tph <= 0.0 {
                                continue;
                            }
                            for source in sequence {
                                let Some(ground) = ground_index.get(source) else { continue };
                                let combined = combined_dig(*source);
                                if combined <= 0.0 || time_used >= duration - 1e-9 {
                                    break;
                                }
                                touched += 1;
                                time_used += ground.tonnes_t / combined;
                            }
                        }
                        TaskKind::Reclaim { approved_sources, .. } => {
                            if loader.rates[interval.index].reclaim_tph <= 0.0 {
                                continue;
                            }
                            reclaim_task = true;
                            for pile_id in approved_sources {
                                let Some(pile) = input.stockpiles.iter().find(|pile| pile.id == *pile_id) else {
                                    continue;
                                };
                                let combined = combined_reclaim(*pile_id);
                                if combined <= 0.0 {
                                    continue;
                                }
                                for lot in &pile.opening {
                                    if time_used >= duration - 1e-9 {
                                        break;
                                    }
                                    touched += 1;
                                    time_used += lot.tonnes_t / combined;
                                }
                                // Receipt parcels of earlier intervals are
                                // target-sized; the interval's time budget
                                // stops the walk long before capacity could
                                // provision every slot.
                                let slots = ((pile.capacity_t / input.horizon.parcel_target_t).ceil() as usize).min(SEGMENT_CEILING);
                                for _ in 0..slots {
                                    if time_used >= duration - 1e-9 {
                                        break;
                                    }
                                    touched += 1;
                                    time_used += input.horizon.parcel_target_t / combined;
                                }
                            }
                        }
                    }
                }
                if reclaim_task {
                    touched += 1;
                }
                if touched > 0 {
                    any_work = true;
                    extra_boundaries += touched - 1;
                }
            }
            let budget = usize::from(any_work) + extra_boundaries;
            budget.clamp(1, SEGMENT_CEILING)
        })
        .collect()
}

// Why the FIFO order admits no sound prefix pruning: a unit deep in the
// fixed order is reachable cheaply whenever the groups before it received no
// supply, because their provisioned positions sit at zero balance and FIFO
// skips empty units without expending anything. Reachability therefore
// depends on the assignment - a decision - not on any input-derived prefix:
// a pile that receives nothing until interval 6 has its interval-6 parcels
// far down the order, yet they are the first reclaimable stock. The
// quadratic pairwise blocker web this file once carried was therefore
// replaced by the exact linear conjunction chain in the inventory-ordering
// constraints, which preserves FIFO/LIFO semantics with a constraint count
// proportional to the unit count.
//
// Why tie preferences are a second objective stage and not an epsilon: they
// used to be added to the primary objective with a coefficient small enough
// that their total possible effect stayed below the documented objective
// tolerance. That bound is a sum over every movement and parcel column, so
// the coefficient fell to about 1e-13 beside primary costs of one to two.
// It is not a tie-break at that scale, it is noise on thousands of columns:
// HiGHS warns about the cost range, the root relaxation becomes degenerate
// enough that its cut loop never terminates, and on the eight-hour audit
// fixture the solver spent a full sixty seconds at the root, produced 5792
// cuts, explored zero nodes, ran zero heuristic iterations and returned no
// incumbent at all. The staged form below is also strictly stronger: the
// epsilon could in principle move the primary objective by up to the
// tolerance, the stage cannot move it at all.

/// Build and solve. Cancellation is checked before formulation and immediately
/// before entering HiGHS. The pinned safe wrapper exposes no concurrent
/// interrupt handle, so cancellation after that point can only discard the
/// eventual result at the caller; it cannot honestly be reported as an
/// interrupted solve.
pub(super) fn solve(input: &OptimisationInput, limits: SolveLimits, cancellation: &CancellationToken) -> OptimisationResult {
    let started = Instant::now();
    let mut formulation = Formulation::new();
    let cancel_with = |status, message: Option<String>, elapsed: Duration, formulation: &Formulation| {
        empty_result(input, &segment_budgets_static(input), status, message, started, elapsed, formulation)
    };
    if cancellation.is_cancelled() {
        return cancel_with(SolveStatus::Cancelled, None, started.elapsed(), &formulation);
    }
    if let Err(error) = input.validate() {
        return cancel_with(
            SolveStatus::BackendFailure,
            Some(format!("invalid optimisation input: {error:?}")),
            started.elapsed(),
            &formulation,
        );
    }
    if limits.relative_gap.is_some_and(|gap| !gap.is_finite() || gap < 0.0) || limits.threads == Some(0) {
        return cancel_with(SolveStatus::BackendFailure, Some("invalid solve limits".into()), started.elapsed(), &formulation);
    }

    let tolerance = input.horizon.tolerances;
    let ground_index: BTreeMap<_, _> = input.ground.iter().map(|entry| (entry.id, entry)).collect();
    let pile_index: BTreeMap<_, _> = input.stockpiles.iter().map(|entry| (entry.id, entry)).collect();
    let destination_index: BTreeMap<_, _> = input.destinations.iter().map(|entry| (entry.id, entry)).collect();
    let total_material_bound =
        input.ground.iter().map(|source| source.tonnes_t).sum::<f64>() + input.stockpiles.iter().flat_map(|pile| &pile.opening).map(|lot| lot.tonnes_t).sum::<f64>();
    let budgets = segment_budgets(input, &ground_index);
    let segments_of = |interval: usize| budgets[interval];
    // The negligible-inventory scale shared by the model and the independent
    // validator: a unit or ground source whose balance falls to this level is
    // effectively exhausted. It must match the publication snap tolerance so
    // the solver, the published rows, and the replay agree on what is live.
    let dust = tolerance.tonnes_t.max(input.horizon.parcel_target_t * 1e-6);
    // An inventory unit's true bound: an opening lot holds its own tonnage, a
    // receipt parcel at most the parcel target. Using this rather than the
    // whole pile's capacity as the big-M on the emptiness and selection rows
    // keeps the slack a nearly-integral binary can hide proportional to what
    // the unit can actually hold.
    // Whether a unit carries inventory columns in this interval: opening lots
    // from the start, receipt parcels from the interval after their receipt.
    let released_at = |unit: UnitKey, interval: usize| -> bool {
        match unit {
            UnitKey::Opening { .. } => true,
            UnitKey::Receipt { receipt_interval, .. } => interval > receipt_interval,
        }
    };
    let unit_bound = |pile: &Stockpile, unit: UnitKey| -> f64 {
        match unit {
            UnitKey::Opening { opening, .. } => pile.opening[opening].tonnes_t,
            UnitKey::Receipt { .. } => input.horizon.parcel_target_t,
        }
        .min(pile.capacity_t)
    };
    // The negligible-inventory scale is derived, not chosen: see
    // [`inventory_dust`].
    let unit_dust = inventory_dust(input);
    // An assigned receipt parcel must hold strictly more than the negligible
    // scale, so a parcel is never born inside the dead band where the model
    // may call it empty: a unit the formulation cannot distinguish from empty
    // must never appear in the published timeline as live stock blocking the
    // FIFO/LIFO order. Unassigned positions stay at exactly zero, so no
    // receipt unit is ever published between zero and this floor.
    let parcel_floor = tolerance.parcel_occupancy_t.max(2.0 * unit_dust);

    // Shared event clock: every loader agrees on these segment durations.
    for interval in &input.intervals {
        for segment in 0..segments_of(interval.index) {
            formulation
                .durations
                .insert((interval.index, segment), formulation.columns.continuous(interval.duration_h()));
        }
    }

    // Shared ground balances and exact-at-tolerance completion flags at every
    // segment boundary. A source depleted by any loader is complete for every
    // sequence that names it.
    let mut ground_done = BTreeMap::new();
    for ground in &input.ground {
        for interval in &input.intervals {
            for segment in 0..segments_of(interval.index) {
                formulation
                    .ground_remaining
                    .insert((ground.id, interval.index, segment), formulation.columns.continuous(ground.tonnes_t));
                ground_done.insert((ground.id, interval.index, segment), formulation.columns.binary());
            }
        }
    }

    // Task and source selection per segment. The ready variables are exact
    // conjunctions of fixed operational eligibility, predecessor completion
    // and current work remaining; selection takes the first ready task in
    // authored order, re-evaluated at every event boundary.
    let mut task_ready = BTreeMap::new();
    let mut dig_stage_selected = BTreeMap::new();
    for loader in &input.loaders {
        let ordered = task_order(input, loader.id);
        for interval in &input.intervals {
            let rate = &loader.rates[interval.index];
            for &task_index in &ordered {
                let task = &input.tasks[task_index];
                for segment in 0..segments_of(interval.index) {
                    let selected = formulation.columns.binary();
                    let ready = formulation.columns.binary();
                    formulation.task_selected.insert((task_index, interval.index, segment), selected);
                    task_ready.insert((task_index, interval.index, segment), ready);
                    if let TaskKind::Dig { sequence } = &task.kind {
                        for (stage, source) in sequence.iter().enumerate() {
                            let stage_selected = formulation.columns.binary();
                            dig_stage_selected.insert((task_index, interval.index, segment, stage), stage_selected);
                            let source_data = ground_index[source];
                            let operable =
                                active(task, *interval) && rate.dig_tph > 0.0 && source_operable(input, task, SourceId::Ground(*source), &source_data.material, interval.index);
                            if operable {
                                let upper = rate.dig_tph * interval.duration_h();
                                let extraction = formulation.columns.continuous(upper.min(source_data.tonnes_t));
                                formulation.dig.push(DigVariable {
                                    interval: interval.index,
                                    segment,
                                    task: task_index,
                                    stage,
                                    source: *source,
                                    column: extraction,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Movement variables are created only for route combinations belonging to
    // a feasible task/source/material/interval tuple, for every segment.
    for dig in &formulation.dig.clone() {
        let task = &input.tasks[dig.task];
        let ground = ground_index[&dig.source];
        for share in &ground.material {
            for (candidate, _) in routes_for(input, task.loader, Activity::Dig, SourceId::Ground(dig.source), share.material, dig.interval) {
                let column = formulation.columns.continuous(ground.tonnes_t * share.fraction);
                formulation.movements.push(MoveVariable {
                    interval: dig.interval,
                    segment: dig.segment,
                    task: dig.task,
                    candidate,
                    source: SourceId::Ground(dig.source),
                    material: share.material,
                    unit: None,
                    column,
                });
            }
        }
    }

    // How much ground material can reach one stockpile in one interval: only
    // loaders that both hold an active dig task and have an authored route
    // into this pile can deliver to it, so a reclaim-only loader's rate is
    // not part of the bound. This sizes the receipt positions, and through
    // them every inventory unit the pile carries for the rest of the horizon.
    let pile_dig_bound = |pile: StockpileId, interval: &Interval| -> f64 {
        input
            .loaders
            .iter()
            .filter(|loader| {
                input
                    .tasks
                    .iter()
                    .any(|task| task.loader == loader.id && matches!(task.kind, TaskKind::Dig { .. }) && active(task, *interval))
                    && input.movements.iter().any(|candidate| {
                        candidate.loader == loader.id
                            && candidate.activity == Activity::Dig
                            && matches!(destination_index[&candidate.destination].kind, DestinationKind::Stockpile(target) if target == pile)
                    })
            })
            .map(|loader| loader.rates[interval.index].dig_tph * interval.duration_h())
            .sum()
    };

    // Upper bounds for generated receipt positions are physical supply bounds:
    // source tonnes, pile capacity, all-loader interval production, and the
    // fleet's interval tonnage ceiling - never a universal M. Parcels are
    // grouped per (pile, interval, segment) so receipt order follows execution
    // timing: sequential supplies stay ordered, overlapping supplies interleave.
    let mut stream_bounds: BTreeMap<StreamKey, f64> = BTreeMap::new();
    for movement in &formulation.movements {
        let candidate = &input.movements[movement.candidate];
        if let Some(destination) = destination_index.get(&candidate.destination)
            && let DestinationKind::Stockpile(pile) = destination.kind
        {
            let key = StreamKey {
                pile,
                interval: movement.interval,
                segment: movement.segment,
                source: movement.source,
                material: movement.material,
            };
            let upper = match movement.source {
                SourceId::Ground(id) => ground_index[&id].tonnes_t,
                SourceId::Stockpile(id) => pile_index[&id].capacity_t,
            };
            *stream_bounds.entry(key).or_default() += upper;
        }
    }
    let mut positions_per_group = BTreeMap::new();
    for ((pile, interval, segment), streams) in stream_bounds.keys().fold(BTreeMap::<_, Vec<StreamKey>>::new(), |mut grouped, key| {
        grouped.entry((key.pile, key.interval, key.segment)).or_default().push(*key);
        grouped
    }) {
        let interval_data = &input.intervals[interval];
        let fleet_bound: f64 = input
            .trucks
            .iter()
            .map(|truck| {
                let coefficient = input
                    .movements
                    .iter()
                    .filter(|candidate| {
                        candidate.source != SourceId::Stockpile(pile)
                            && matches!(destination_index[&candidate.destination].kind, DestinationKind::Stockpile(target) if target == pile)
                            && candidate.truck == truck.id
                    })
                    .map(|candidate| candidate.truck_hours_per_tonne)
                    .reduce(f64::min)
                    .unwrap_or(f64::INFINITY);
                truck.hours[interval] / coefficient
            })
            .sum::<f64>();
        let loader_bound = pile_dig_bound(pile, interval_data);
        let bound: f64 = streams
            .iter()
            .map(|key| stream_bounds[key])
            .sum::<f64>()
            .min(pile_index[&pile].capacity_t)
            .min(fleet_bound)
            .min(loader_bound);
        let positions = (bound / input.horizon.parcel_target_t).ceil() as usize + streams.len();
        positions_per_group.insert((pile, interval, segment), positions);
        for stream in streams {
            for position in 0..positions {
                formulation.parcels.push(ParcelVariable {
                    stream,
                    position,
                    quantity: formulation.columns.continuous(input.horizon.parcel_target_t),
                    assigned: formulation.columns.binary(),
                    remainder: formulation.columns.binary(),
                });
            }
        }
    }

    // Inventory unit reclaim variables per segment. Opening lots keep fixed
    // mixtures; receipt positions carry one fixed-composition stream each.
    for (task_index, task) in input.tasks.iter().enumerate() {
        let TaskKind::Reclaim { approved_sources, .. } = &task.kind else { continue };
        for interval in &input.intervals {
            if !active(task, *interval) {
                continue;
            }
            let loader = input.loaders.iter().find(|loader| loader.id == task.loader).expect("validated loader");
            if loader.rates[interval.index].reclaim_tph <= 0.0 {
                continue;
            }
            for segment in 0..segments_of(interval.index) {
                for pile_id in approved_sources {
                    let pile = pile_index[pile_id];
                    for (opening, lot) in pile.opening.iter().enumerate() {
                        if source_operable(input, task, SourceId::Stockpile(*pile_id), &lot.material, interval.index) {
                            let total = formulation
                                .columns
                                .continuous(lot.tonnes_t.min(loader.rates[interval.index].reclaim_tph * interval.duration_h()));
                            for share in &lot.material {
                                formulation.reclaim.push(ReclaimVariable {
                                    interval: interval.index,
                                    segment,
                                    task: task_index,
                                    source: *pile_id,
                                    unit: UnitKey::Opening { pile: *pile_id, opening },
                                    material: share.material,
                                    fraction: share.fraction,
                                    column: total,
                                });
                            }
                        }
                    }
                    for (&(receipt_pile, receipt_interval, receipt_segment), &positions) in &positions_per_group {
                        if receipt_pile != *pile_id || receipt_interval >= interval.index {
                            continue;
                        }
                        for position in 0..positions {
                            let materials: BTreeSet<_> = formulation
                                .parcels
                                .iter()
                                .filter(|parcel| {
                                    parcel.stream.pile == *pile_id
                                        && parcel.stream.interval == receipt_interval
                                        && parcel.stream.segment == receipt_segment
                                        && parcel.position == position
                                })
                                .map(|parcel| parcel.stream.material)
                                .collect();
                            for material in materials {
                                if routes_for(input, task.loader, Activity::Reclaim, SourceId::Stockpile(*pile_id), material, interval.index)
                                    .next()
                                    .is_some()
                                {
                                    let total = formulation
                                        .columns
                                        .continuous(input.horizon.parcel_target_t.min(loader.rates[interval.index].reclaim_tph * interval.duration_h()));
                                    formulation.reclaim.push(ReclaimVariable {
                                        interval: interval.index,
                                        segment,
                                        task: task_index,
                                        source: *pile_id,
                                        unit: UnitKey::Receipt {
                                            pile: *pile_id,
                                            receipt_interval,
                                            segment: receipt_segment,
                                            position,
                                        },
                                        material,
                                        fraction: 1.0,
                                        column: total,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Reclaim destination movements, again only feasible route combinations.
    // A draw that takes its whole unit-material share down exactly one route
    // is that route: rather than a second column tied to the first by an
    // equality, the movement *is* the reclaim column. On the audit fixture
    // that is every receipt draw, and it removes a quarter of the columns at
    // a twenty-four hour horizon.
    for reclaim in &formulation.reclaim.clone() {
        let task = &input.tasks[reclaim.task];
        let candidates: Vec<usize> = routes_for(
            input,
            task.loader,
            Activity::Reclaim,
            SourceId::Stockpile(reclaim.source),
            reclaim.material,
            reclaim.interval,
        )
        .map(|(candidate, _)| candidate)
        .filter(|candidate| {
            matches!(
                destination_index[&input.movements[*candidate].destination].kind,
                DestinationKind::Crusher | DestinationKind::Dump
            )
        })
        .collect();
        let shared = candidates.len() == 1 && reclaim.fraction == 1.0;
        for candidate in candidates {
            let column = if shared {
                reclaim.column
            } else {
                formulation.columns.continuous(pile_index[&reclaim.source].capacity_t)
            };
            formulation.movements.push(MoveVariable {
                interval: reclaim.interval,
                segment: reclaim.segment,
                task: reclaim.task,
                candidate,
                source: SourceId::Stockpile(reclaim.source),
                material: reclaim.material,
                unit: Some(reclaim.unit),
                column,
            });
        }
    }

    // All remaining binary and balance columns are declared before the
    // good_lp problem is materialised. Keeping this as an explicit collection
    // phase makes the variable count trustworthy and prevents accidental
    // backend-only variables.
    let mut occupied_count = BTreeMap::new();
    for (&(pile, interval, segment), &positions) in &positions_per_group {
        for count in 0..=positions {
            occupied_count.insert((pile, interval, segment, count), formulation.columns.binary());
        }
    }
    // A stockpile no task can reclaim from carries no ordering state: with
    // no draw, every unit's balance is its receipt for the rest of the
    // horizon and its place in the FIFO/LIFO order can never be consulted.
    // Only the pile's inventory total is still needed, for capacity. This is
    // structural, not a prune - the omitted cells have a single feasible
    // value - and it removes the whole inventory block for dump-only or
    // rehandle-only piles.
    let reclaimable: BTreeSet<StockpileId> = formulation.reclaim.iter().map(|reclaim| reclaim.source).collect();
    let mut unit_empty = BTreeMap::new();
    let mut unit_chain_empty = BTreeMap::new();
    let mut pile_available = BTreeMap::new();
    let mut chain_sequence_end = BTreeMap::new();
    let mut destination_remaining = BTreeMap::new();
    let mut destination_available = BTreeMap::new();
    let mut material_route_available = BTreeMap::new();
    for pile in &input.stockpiles {
        let mut units: Vec<UnitKey> = (0..pile.opening.len()).map(|opening| UnitKey::Opening { pile: pile.id, opening }).collect();
        let mut receipt_groups: Vec<_> = positions_per_group
            .iter()
            .filter(|((receipt_pile, _, _), _)| *receipt_pile == pile.id)
            .map(|(&(receipt_pile, receipt_interval, receipt_segment), &positions)| (receipt_pile, receipt_interval, receipt_segment, positions))
            .collect();
        receipt_groups.sort_by_key(|(_, interval, segment, _)| (*interval, *segment));
        for (_, receipt_interval, receipt_segment, positions) in receipt_groups {
            units.extend((0..positions).map(|position| UnitKey::Receipt {
                pile: pile.id,
                receipt_interval,
                segment: receipt_segment,
                position,
            }));
        }
        for unit in units.iter().copied().filter(|_| reclaimable.contains(&pile.id)) {
            let bound = unit_bound(pile, unit);
            // A receipt unit holds nothing until the interval after its
            // receipt (the retained release approximation), so the intervals
            // before that carry no columns at all: the unit is a constant
            // empty there. This is structural, not a heuristic prune - the
            // omitted cells have a single feasible value.
            for interval in input.intervals.iter().filter(|interval| released_at(unit, interval.index)) {
                for segment in 0..segments_of(interval.index) {
                    formulation.unit_start.insert((unit, interval.index, segment), formulation.columns.continuous(bound));
                    unit_empty.insert((unit, interval.index, segment), formulation.columns.binary());
                    // "Every unit on the blocking side of this one is empty":
                    // the FIFO prefix / LIFO suffix conjunction, chained
                    // linearly instead of the quadratic pairwise blocker web.
                    //
                    // Kept binary although the three rows below are the exact
                    // linearisation of an AND and therefore force it integral
                    // anyway. Relaxing it removes two fifths of the model's
                    // integer columns, but the branching it supports is worth
                    // more than the smaller search space: on the twelve-hour
                    // audit fixture the relaxed form went from a 1.8% gap to
                    // 78% in the same budget.
                    unit_chain_empty.insert((unit, interval.index, segment), formulation.columns.binary());
                }
            }
        }
        for interval in &input.intervals {
            for segment in 0..segments_of(interval.index) {
                formulation
                    .pile_closing
                    .insert((pile.id, interval.index, segment), formulation.columns.continuous(pile.capacity_t));
                pile_available.insert((pile.id, interval.index, segment), formulation.columns.binary());
                chain_sequence_end.insert((pile.id, interval.index, segment), formulation.columns.binary());
            }
        }
    }
    for destination in &input.destinations {
        for interval in &input.intervals {
            for segment in 0..segments_of(interval.index) {
                let upper = match destination.kind {
                    DestinationKind::Crusher => destination.crusher_daily_t.get(interval.day() as usize).copied().flatten().unwrap_or(total_material_bound),
                    DestinationKind::Dump | DestinationKind::Stockpile(_) => destination.capacity_t.unwrap_or(total_material_bound),
                };
                destination_remaining.insert((destination.id, interval.index, segment), formulation.columns.continuous(upper));
                destination_available.insert((destination.id, interval.index, segment), formulation.columns.binary());
            }
        }
    }
    for dig in &formulation.dig {
        for share in &ground_index[&dig.source].material {
            material_route_available.insert((dig.task, dig.interval, dig.segment, dig.stage, share.material), formulation.columns.binary());
        }
    }

    // Two-stage lexicographic objective. The primary objective is movement
    // value. Tie preferences - routing preference, shorter haul cycles, and
    // filling earlier receipt positions among equals - are a *second* stage,
    // not an epsilon-weighted addition to the first.
    //
    // They used to be added with a coefficient small enough that their total
    // possible effect stayed below the documented objective tolerance. That
    // is unusable in double precision here: the bound is a sum over every
    // movement and parcel column, so the coefficient fell to around 1e-13
    // beside primary costs of 1..2. HiGHS warns about it ("excessively small
    // costs"), and the root relaxation becomes so degenerate that its cut
    // loop never terminates - on the eight-hour audit fixture the solver
    // spent a full sixty seconds at the root, generated 5792 cuts, branched
    // zero times and returned no incumbent at all. With the same preferences
    // expressed as a second stage the same fixture returns a validated
    // incumbent. See the stage note above `solve`.
    let parcel_flat_index = |parcel: &ParcelVariable| -> f64 {
        let positions = positions_per_group
            .get(&(parcel.stream.pile, parcel.stream.interval, parcel.stream.segment))
            .copied()
            .unwrap_or(0);
        parcel.stream.segment as f64 * (positions + 1) as f64 + parcel.position as f64
    };
    let shortest_cycle = |stream: StreamKey| -> f64 {
        input
            .movements
            .iter()
            .filter(|candidate| {
                candidate.source == stream.source
                    && candidate.material == stream.material
                    && matches!(destination_index[&candidate.destination].kind, DestinationKind::Stockpile(id) if id == stream.pile)
            })
            .map(|candidate| candidate.truck_hours_per_tonne)
            .reduce(f64::min)
            .unwrap_or(0.0)
    };
    let mut primary_costs = vec![0.0; formulation.columns.count];
    let mut secondary_costs = vec![0.0; formulation.columns.count];
    for movement in &formulation.movements {
        let candidate = &input.movements[movement.candidate];
        let value = candidate.value_per_tonne().expect("validated value");
        primary_costs[movement.column.index] += value;
        secondary_costs[movement.column.index] += 1.0 - f64::from(candidate.routing_preference) - candidate.truck_hours_per_tonne;
        formulation.objective += value * movement.column.variable;
    }
    for parcel in &formulation.parcels {
        secondary_costs[parcel.quantity.index] -= (parcel_flat_index(parcel) + 1.0) * shortest_cycle(parcel.stream);
    }

    let mut model = std::mem::replace(&mut formulation.columns.variables, ProblemVariables::new())
        .maximise(formulation.objective.clone())
        .using(good_lp::solvers::highs::highs);

    // The shared event clock fills its interval exactly.
    for interval in &input.intervals {
        let durations: Vec<_> = (0..segments_of(interval.index))
            .map(|segment| (formulation.durations[&(interval.index, segment)], 1.0))
            .collect();
        add_constraint(&mut model, &mut formulation, sum(durations).eq(interval.duration_h()));
    }

    // Ground conservation and completion state at every segment boundary.
    //
    // State is carried from the previous boundary rather than re-summed from
    // the start of the horizon. The two forms are algebraically identical,
    // but the cumulative one writes a row whose length grows with the cell
    // index, so the matrix grows with the square of the horizon; on the
    // twenty-four hour audit fixture the three cumulative families - ground,
    // receiving capacity and stockpile inventory - were most of the nonzeros
    // in the model. The same substitution is applied to all three.
    let cell_of = |interval: usize, segment: usize| -> Option<(usize, usize)> {
        if segment > 0 {
            Some((interval, segment - 1))
        } else {
            interval.checked_sub(1).map(|previous| (previous, segments_of(previous) - 1))
        }
    };
    for ground in &input.ground {
        for interval in &input.intervals {
            for segment in 0..segments_of(interval.index) {
                let here: Vec<_> = formulation
                    .dig
                    .iter()
                    .filter(|dig| dig.source == ground.id && dig.interval == interval.index && dig.segment == segment)
                    .map(|dig| (dig.column, 1.0))
                    .collect();
                let remaining = formulation.ground_remaining[&(ground.id, interval.index, segment)];
                // `prior` is the source's state at the start of this cell:
                // what the previous boundary left, or the whole source.
                let prior = cell_of(interval.index, segment).map_or_else(
                    || Expression::from(ground.tonnes_t),
                    |previous| Expression::from(formulation.ground_remaining[&(ground.id, previous.0, previous.1)].variable),
                );
                add_constraint(&mut model, &mut formulation, (remaining.variable + sum(here)).eq(prior.clone()));
                let done = ground_done[&(ground.id, interval.index, segment)];
                let exhausted = (ground.tonnes_t - dust).max(0.0);
                // With done = 0 what was dug before this cell must stay below
                // the exhaustion level; the slack cannot exceed half the
                // source so tiny sources remain representable. "Dug before" is
                // the source less what the previous boundary left.
                let slack = dust.min(ground.tonnes_t / 2.0).max(tolerance.tonnes_t);
                add_constraint(&mut model, &mut formulation, (ground.tonnes_t - prior.clone()).geq(exhausted * done.variable));
                add_constraint(&mut model, &mut formulation, prior.geq(slack * (1.0 - done.variable)));
            }
        }
        let all: Vec<_> = formulation.dig.iter().filter(|dig| dig.source == ground.id).map(|dig| (dig.column, 1.0)).collect();
        add_constraint(&mut model, &mut formulation, sum(all).leq(ground.tonnes_t));
    }

    // Receiving capacity at each segment start is a model state. Consuming it
    // earlier may genuinely block a task; merely reserving it for later leaves
    // the availability binary one and cannot manufacture permission to bypass
    // priority. Crusher budgets span every segment of their day; dumps are
    // cumulative; a stockpile's used capacity is its inventory.
    for destination in &input.destinations {
        for interval in &input.intervals {
            for segment in 0..segments_of(interval.index) {
                let remaining = destination_remaining[&(destination.id, interval.index, segment)];
                let available = destination_available[&(destination.id, interval.index, segment)];
                let unlimited = match destination.kind {
                    DestinationKind::Crusher => destination.crusher_daily_t.get(interval.day() as usize).copied().flatten().is_none(),
                    DestinationKind::Dump => destination.capacity_t.is_none(),
                    DestinationKind::Stockpile(_) => false,
                };
                let upper = match destination.kind {
                    DestinationKind::Crusher => destination.crusher_daily_t.get(interval.day() as usize).copied().flatten().unwrap_or(total_material_bound),
                    DestinationKind::Dump => destination.capacity_t.unwrap_or(total_material_bound),
                    DestinationKind::Stockpile(pile) => pile_index[&pile].capacity_t,
                };
                if unlimited {
                    add_constraint(&mut model, &mut formulation, Expression::from(available.variable).eq(1.0));
                    add_constraint(&mut model, &mut formulation, Expression::from(remaining.variable).eq(upper));
                } else {
                    // What is left at the start of this cell: for a stockpile
                    // it is the capacity less the inventory the previous
                    // boundary closed on; for a crusher or dump it is the
                    // previous boundary's remainder less what that boundary's
                    // own segment consumed. A crusher's budget is daily, so
                    // the carry stops at a day boundary.
                    let opening = match destination.kind {
                        DestinationKind::Stockpile(pile) => cell_of(interval.index, segment).map_or_else(
                            || Expression::from(upper - pile_index[&pile].opening.iter().map(|lot| lot.tonnes_t).sum::<f64>()),
                            |previous| upper - formulation.pile_closing[&(pile, previous.0, previous.1)].variable,
                        ),
                        DestinationKind::Crusher | DestinationKind::Dump => {
                            let carries = |previous: (usize, usize)| match destination.kind {
                                DestinationKind::Crusher => input.intervals[previous.0].day() == interval.day(),
                                _ => true,
                            };
                            match cell_of(interval.index, segment).filter(|previous| carries(*previous)) {
                                Some(previous) => {
                                    let spent: Vec<_> = formulation
                                        .movements
                                        .iter()
                                        .filter(|movement| {
                                            input.movements[movement.candidate].destination == destination.id && movement.interval == previous.0 && movement.segment == previous.1
                                        })
                                        .map(|movement| (movement.column, 1.0))
                                        .collect();
                                    destination_remaining[&(destination.id, previous.0, previous.1)].variable - sum(spent)
                                }
                                None => Expression::from(upper),
                            }
                        }
                    };
                    add_constraint(&mut model, &mut formulation, Expression::from(remaining.variable).eq(opening));
                    add_constraint(&mut model, &mut formulation, Expression::from(remaining.variable).leq(upper * available.variable));
                    add_constraint(
                        &mut model,
                        &mut formulation,
                        Expression::from(remaining.variable).geq(tolerance.tonnes_t * available.variable),
                    );
                }
            }
        }
    }

    for (&(task_index, interval, segment, stage, material), &route_available) in &material_route_available {
        let task = &input.tasks[task_index];
        let source = match &task.kind {
            TaskKind::Dig { sequence } => SourceId::Ground(sequence[stage]),
            TaskKind::Reclaim { .. } => unreachable!(),
        };
        let destinations: BTreeSet<_> = routes_for(input, task.loader, Activity::Dig, source, material, interval)
            .map(|(_, candidate)| candidate.destination)
            .collect();
        let available: Vec<_> = destinations
            .iter()
            .map(|destination| (destination_available[&(*destination, interval, segment)], 1.0))
            .collect();
        add_constraint(&mut model, &mut formulation, Expression::from(route_available.variable).leq(sum(available.clone())));
        for (destination, _) in available {
            add_constraint(&mut model, &mut formulation, Expression::from(route_available.variable).geq(destination.variable));
        }
    }

    // Dig task readiness, stage selection and loader rate, re-evaluated at
    // every segment. A task whose current stage is eligible must be picked;
    // the first ready task in authored order must be selected; extraction is
    // bounded by the loader rate applied to the segment's own duration.
    for loader in &input.loaders {
        let ordered = task_order(input, loader.id);
        for interval in &input.intervals {
            for segment in 0..segments_of(interval.index) {
                let duration = formulation.durations[&(interval.index, segment)];
                for (rank, &task_index) in ordered.iter().enumerate() {
                    let task = &input.tasks[task_index];
                    let ready = task_ready[&(task_index, interval.index, segment)];
                    let selected = formulation.task_selected[&(task_index, interval.index, segment)];
                    match &task.kind {
                        TaskKind::Dig { sequence } => {
                            let mut stages = Vec::new();
                            for (stage, source) in sequence.iter().enumerate() {
                                let pick = dig_stage_selected[&(task_index, interval.index, segment, stage)];
                                let source_data = ground_index[source];
                                let fixed = active(task, *interval)
                                    && loader.rates[interval.index].dig_tph > 0.0
                                    && source_operable(input, task, SourceId::Ground(*source), &source_data.material, interval.index);
                                if !fixed {
                                    add_constraint(&mut model, &mut formulation, Expression::from(pick.variable).eq(0.0));
                                } else {
                                    let done = ground_done[&(*source, interval.index, segment)];
                                    add_constraint(&mut model, &mut formulation, Expression::from(pick.variable).leq(1.0 - done.variable));
                                    for predecessor in &sequence[..stage] {
                                        add_constraint(
                                            &mut model,
                                            &mut formulation,
                                            Expression::from(pick.variable).leq(ground_done[&(*predecessor, interval.index, segment)].variable),
                                        );
                                    }
                                    let route_columns: Vec<_> = source_data
                                        .material
                                        .iter()
                                        .map(|share| material_route_available[&(task_index, interval.index, segment, stage, share.material)])
                                        .collect();
                                    for route in &route_columns {
                                        add_constraint(&mut model, &mut formulation, Expression::from(pick.variable).leq(route.variable));
                                    }
                                    let predecessor_sum = sum(sequence[..stage].iter().map(|id| (ground_done[&(*id, interval.index, segment)], 1.0)));
                                    let route_sum = sum(route_columns.iter().map(|column| (*column, 1.0)));
                                    add_constraint(
                                        &mut model,
                                        &mut formulation,
                                        Expression::from(pick.variable).geq(predecessor_sum + route_sum - (stage + route_columns.len()) as f64 - done.variable + 1.0),
                                    );
                                }
                                stages.push((pick, 1.0));
                                if let Some(extraction) = formulation
                                    .dig
                                    .iter()
                                    .find(|dig| dig.task == task_index && dig.interval == interval.index && dig.segment == segment && dig.stage == stage)
                                    .map(|dig| dig.column)
                                {
                                    let rate = loader.rates[interval.index].dig_tph * interval.duration_h();
                                    add_constraint(&mut model, &mut formulation, Expression::from(extraction.variable).leq(rate * pick.variable));
                                    add_constraint(&mut model, &mut formulation, Expression::from(extraction.variable).leq(rate * selected.variable));
                                    add_constraint(
                                        &mut model,
                                        &mut formulation,
                                        Expression::from(extraction.variable).leq(loader.rates[interval.index].dig_tph * duration.variable),
                                    );
                                    for share in &source_data.material {
                                        let routed: Vec<_> = formulation
                                            .movements
                                            .iter()
                                            .filter(|movement| {
                                                movement.task == task_index
                                                    && movement.interval == interval.index
                                                    && movement.segment == segment
                                                    && movement.source == SourceId::Ground(*source)
                                                    && movement.material == share.material
                                            })
                                            .map(|movement| (movement.column, 1.0))
                                            .collect();
                                        add_constraint(&mut model, &mut formulation, sum(routed).eq(share.fraction * extraction.variable));
                                    }
                                }
                            }
                            add_constraint(&mut model, &mut formulation, Expression::from(ready.variable).eq(sum(stages)));
                        }
                        TaskKind::Reclaim { .. } => {
                            // Inventory-linked readiness is tightened below.
                            // Until then it may be one only while the
                            // window/rate allows.
                            if !active(task, *interval) || loader.rates[interval.index].reclaim_tph <= 0.0 {
                                add_constraint(&mut model, &mut formulation, Expression::from(ready.variable).eq(0.0));
                            }
                        }
                    }
                    add_constraint(&mut model, &mut formulation, Expression::from(selected.variable).leq(ready.variable));
                    for &earlier in &ordered[..rank] {
                        add_constraint(
                            &mut model,
                            &mut formulation,
                            Expression::from(selected.variable).leq(1.0 - task_ready[&(earlier, interval.index, segment)].variable),
                        );
                    }
                    let earlier = sum(ordered[..rank].iter().map(|earlier| (task_ready[&(*earlier, interval.index, segment)], 1.0)));
                    add_constraint(&mut model, &mut formulation, Expression::from(selected.variable).geq(ready.variable - earlier));
                }
                let selected: Vec<_> = ordered.iter().map(|task| (formulation.task_selected[&(*task, interval.index, segment)], 1.0)).collect();
                add_constraint(&mut model, &mut formulation, sum(selected).leq(1.0));
            }
        }
    }

    // Each stream becomes target-sized fixed-composition parcels with at most
    // one remainder, and that remainder follows the stream's full parcels -
    // within the (pile, interval, segment) group the stream supplied.
    for (&(pile, interval, segment), &positions) in &positions_per_group {
        let streams: Vec<_> = stream_bounds
            .keys()
            .filter(|key| key.pile == pile && key.interval == interval && key.segment == segment)
            .copied()
            .collect();
        for position in 0..positions {
            let assignments: Vec<_> = formulation
                .parcels
                .iter()
                .filter(|parcel| parcel.stream.pile == pile && parcel.stream.interval == interval && parcel.stream.segment == segment && parcel.position == position)
                .map(|parcel| (parcel.assigned, 1.0))
                .collect();
            add_constraint(&mut model, &mut formulation, sum(assignments.clone()).leq(1.0));
            if position + 1 < positions {
                let next: Vec<_> = formulation
                    .parcels
                    .iter()
                    .filter(|parcel| parcel.stream.pile == pile && parcel.stream.interval == interval && parcel.stream.segment == segment && parcel.position == position + 1)
                    .map(|parcel| (parcel.assigned, 1.0))
                    .collect();
                add_constraint(&mut model, &mut formulation, sum(next).leq(sum(assignments)));
            }
        }
        for stream in &streams {
            let parcels: Vec<_> = formulation.parcels.iter().filter(|parcel| parcel.stream == *stream).cloned().collect();
            let incoming: Vec<_> = formulation
                .movements
                .iter()
                .filter(|movement| {
                    let candidate = &input.movements[movement.candidate];
                    movement.interval == interval
                        && movement.segment == segment
                        && movement.source == stream.source
                        && movement.material == stream.material
                        && matches!(destination_index[&candidate.destination].kind, DestinationKind::Stockpile(id) if id == pile)
                })
                .map(|movement| (movement.column, 1.0))
                .collect();
            add_constraint(&mut model, &mut formulation, sum(parcels.iter().map(|parcel| (parcel.quantity, 1.0))).eq(sum(incoming)));
            add_constraint(&mut model, &mut formulation, sum(parcels.iter().map(|parcel| (parcel.remainder, 1.0))).leq(1.0));
            for (index, parcel) in parcels.iter().enumerate() {
                add_constraint(
                    &mut model,
                    &mut formulation,
                    Expression::from(parcel.quantity.variable).leq(input.horizon.parcel_target_t * parcel.assigned.variable),
                );
                add_constraint(
                    &mut model,
                    &mut formulation,
                    Expression::from(parcel.quantity.variable).geq(parcel_floor * parcel.assigned.variable),
                );
                add_constraint(&mut model, &mut formulation, Expression::from(parcel.remainder.variable).leq(parcel.assigned.variable));
                add_constraint(
                    &mut model,
                    &mut formulation,
                    Expression::from(parcel.quantity.variable).geq(input.horizon.parcel_target_t * (parcel.assigned.variable - parcel.remainder.variable)),
                );
                for later in &parcels[index + 1..] {
                    add_constraint(&mut model, &mut formulation, (parcel.remainder.variable + later.assigned.variable).leq(1.0));
                }
            }
        }
        // Bounded-discrepancy interleaving among overlapping supplies of this
        // segment. N is selected from constant candidates, so no decision
        // variables are multiplied or divided.
        let selectors: Vec<_> = (0..=positions).map(|count| (occupied_count[&(pile, interval, segment, count)], 1.0)).collect();
        add_constraint(&mut model, &mut formulation, sum(selectors).eq(1.0));
        let occupied_total = sum((0..positions).flat_map(|position| {
            formulation
                .parcels
                .iter()
                .filter(move |parcel| parcel.stream.pile == pile && parcel.stream.interval == interval && parcel.stream.segment == segment && parcel.position == position)
                .map(|parcel| (parcel.assigned, 1.0))
        }));
        let selected_count = sum((0..=positions).map(|count| (occupied_count[&(pile, interval, segment, count)], count as f64)));
        add_constraint(&mut model, &mut formulation, occupied_total.eq(selected_count));

        let group_bound = stream_bounds
            .iter()
            .filter(|(key, _)| key.pile == pile && key.interval == interval && key.segment == segment)
            .map(|(_, bound)| *bound)
            .sum::<f64>()
            .min(pile_index[&pile].capacity_t);
        for stream in &streams {
            let total = sum(formulation.parcels.iter().filter(|parcel| parcel.stream == *stream).map(|parcel| (parcel.quantity, 1.0)));
            for count in 1..=positions {
                let selector = occupied_count[&(pile, interval, segment, count)];
                let big_m = (count as f64 * group_bound).max(input.horizon.parcel_target_t);
                for prefix in 1..=count {
                    let supplied = sum(formulation
                        .parcels
                        .iter()
                        .filter(|parcel| parcel.stream == *stream && parcel.position < prefix)
                        .map(|parcel| (parcel.quantity, 1.0)));
                    let discrepancy = count as f64 * supplied - prefix as f64 * total.clone();
                    let allowance = count as f64 * input.horizon.parcel_target_t;
                    add_constraint(&mut model, &mut formulation, discrepancy.clone().leq(allowance + big_m * (1.0 - selector.variable)));
                    add_constraint(&mut model, &mut formulation, discrepancy.geq(-allowance - big_m * (1.0 - selector.variable)));
                }
            }
        }
    }

    // A whole interval's receipts are bounded by that interval's production,
    // however its segments divide it, so the assigned positions of all its
    // groups together cannot exceed what one interval can deliver plus one
    // remainder per stream. Each group is provisioned for the case where it
    // carries the whole interval; this row says they cannot all do so. It
    // removes no feasible receipt pattern - it is the same physical bound the
    // per-group provisioning already applies, stated across the interval -
    // and it cuts the symmetric assignments that otherwise flood the search.
    for pile in &input.stockpiles {
        for interval in &input.intervals {
            let groups: Vec<_> = positions_per_group
                .keys()
                .filter(|(group_pile, group_interval, _)| *group_pile == pile.id && *group_interval == interval.index)
                .copied()
                .collect();
            if groups.is_empty() {
                continue;
            }
            let streams: BTreeSet<_> = stream_bounds
                .keys()
                .filter(|key| key.pile == pile.id && key.interval == interval.index)
                .map(|key| (key.source, key.material))
                .collect();
            let bound = pile_dig_bound(pile.id, interval).min(pile.capacity_t);
            let cap = (bound / input.horizon.parcel_target_t).ceil() as usize + streams.len();
            let assigned: Vec<_> = formulation
                .parcels
                .iter()
                .filter(|parcel| parcel.stream.pile == pile.id && parcel.stream.interval == interval.index)
                .map(|parcel| (parcel.assigned, 1.0))
                .collect();
            if assigned.len() > cap {
                add_constraint(&mut model, &mut formulation, sum(assigned).leq(cap as f64));
            }
        }
    }

    // Inventory-unit balances and ordering at every segment boundary. Receipt
    // parcels occupy their stockpile's capacity in the segment they are
    // delivered, but their unit balance stays zero until the following
    // interval - receive-while-reclaiming without same-interval reclaim.
    for pile in &input.stockpiles {
        let mut units: Vec<UnitKey> = (0..pile.opening.len()).map(|opening| UnitKey::Opening { pile: pile.id, opening }).collect();
        let mut receipt_groups: Vec<_> = positions_per_group
            .iter()
            .filter(|((receipt_pile, _, _), _)| *receipt_pile == pile.id)
            .map(|(&(receipt_pile, receipt_interval, receipt_segment), &positions)| (receipt_pile, receipt_interval, receipt_segment, positions))
            .collect();
        receipt_groups.sort_by_key(|(_, interval, segment, _)| (*interval, *segment));
        for (_, receipt_interval, receipt_segment, positions) in receipt_groups {
            units.extend((0..positions).map(|position| UnitKey::Receipt {
                pile: pile.id,
                receipt_interval,
                segment: receipt_segment,
                position,
            }));
        }
        for interval in &input.intervals {
            // Units not yet released carry no columns in this interval; they
            // are constant-empty and cannot be selected. Neither does any
            // unit of a pile nothing can reclaim.
            let live: Vec<(usize, UnitKey)> = units
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, unit)| reclaimable.contains(&pile.id) && released_at(*unit, interval.index))
                .collect();
            for segment in 0..segments_of(interval.index) {
                if reclaimable.contains(&pile.id) {
                    let total_start = sum(live.iter().map(|(_, unit)| (formulation.unit_start[&(*unit, interval.index, segment)], 1.0)));
                    let available = pile_available[&(pile.id, interval.index, segment)];
                    add_constraint(&mut model, &mut formulation, total_start.clone().leq(pile.capacity_t * available.variable));
                    add_constraint(&mut model, &mut formulation, total_start.geq(unit_dust * available.variable));
                }

                // The ordering sequence for this cell: FIFO draws the
                // oldest released unit first, LIFO the newest.
                let order: Vec<UnitKey> = match pile.order {
                    ReclaimOrder::Fifo => live.iter().map(|(_, unit)| *unit).collect(),
                    ReclaimOrder::Lifo => live.iter().rev().map(|(_, unit)| *unit).collect(),
                };
                // `chain(u)` says every unit ahead of u in that sequence is
                // empty. It is the exact linear form of the FIFO/LIFO rule:
                // chain of the first unit is one, and chain(u) is the
                // conjunction of the previous unit's chain and emptiness.
                // `chain_end` closes the sequence so the selection indicator
                // below telescopes.
                let chain_end = chain_sequence_end[&(pile.id, interval.index, segment)];
                for (index, unit) in order.iter().enumerate() {
                    let chain = unit_chain_empty[&(*unit, interval.index, segment)];
                    let empty = unit_empty[&(*unit, interval.index, segment)];
                    if index == 0 {
                        add_constraint(&mut model, &mut formulation, Expression::from(chain.variable).eq(1.0));
                    } else {
                        let ahead = order[index - 1];
                        let ahead_chain = unit_chain_empty[&(ahead, interval.index, segment)];
                        let ahead_empty = unit_empty[&(ahead, interval.index, segment)];
                        add_constraint(&mut model, &mut formulation, Expression::from(chain.variable).leq(ahead_chain.variable));
                        add_constraint(&mut model, &mut formulation, Expression::from(chain.variable).leq(ahead_empty.variable));
                        add_constraint(
                            &mut model,
                            &mut formulation,
                            Expression::from(chain.variable).geq(ahead_chain.variable + ahead_empty.variable - 1.0),
                        );
                    }
                    if index + 1 == order.len() {
                        add_constraint(&mut model, &mut formulation, Expression::from(chain_end.variable).leq(chain.variable));
                        add_constraint(&mut model, &mut formulation, Expression::from(chain_end.variable).leq(empty.variable));
                        add_constraint(
                            &mut model,
                            &mut formulation,
                            Expression::from(chain_end.variable).geq(chain.variable + empty.variable - 1.0),
                        );
                    }
                }
                if order.is_empty() {
                    add_constraint(&mut model, &mut formulation, Expression::from(chain_end.variable).eq(1.0));
                }
                // The eligible unit is the first non-empty one in the
                // sequence, so its indicator is chain(u) - chain(successor):
                // the chain telescopes, which is why no separate selection
                // binary and no "at most one selected" row are needed.
                let selected_of = |index: usize, order: &[UnitKey]| -> Expression {
                    let chain = unit_chain_empty[&(order[index], interval.index, segment)];
                    let next = order.get(index + 1).map_or(chain_end, |unit| unit_chain_empty[&(*unit, interval.index, segment)]);
                    Expression::from(chain.variable) - next.variable
                };
                for (position, unit) in live.iter().map(|(position, unit)| (*position, unit)) {
                    let _ = position;
                    let start = formulation.unit_start[&(*unit, interval.index, segment)];
                    let empty = unit_empty[&(*unit, interval.index, segment)];
                    let bound = unit_bound(pile, *unit);
                    let selected = selected_of(order.iter().position(|entry| entry == unit).expect("live units are ordered"), &order);
                    // `empty` is an exact indicator of "at or below the
                    // negligible scale", not a choice: both rows use the same
                    // threshold, so a unit holding more than it cannot be
                    // called empty and a unit holding less cannot be called
                    // live. That makes emptiness monotone in time (balances
                    // only fall), which the cut below exploits, and it stops
                    // the solver from parking dust in a unit to skip it.
                    add_constraint(
                        &mut model,
                        &mut formulation,
                        Expression::from(start.variable).leq(unit_dust + bound * (1.0 - empty.variable)),
                    );
                    add_constraint(&mut model, &mut formulation, Expression::from(start.variable).geq(unit_dust * (1.0 - empty.variable)));
                    // Once empty, always empty - and therefore once the chain
                    // holds it keeps holding. Both are valid because a unit's
                    // balance never rises after release; stating them removes
                    // the time symmetry the branch-and-bound would otherwise
                    // explore.
                    let earlier = if segment > 0 {
                        Some((interval.index, segment - 1))
                    } else {
                        interval
                            .index
                            .checked_sub(1)
                            .filter(|previous| released_at(*unit, *previous))
                            .map(|previous| (previous, segments_of(previous) - 1))
                    };
                    if let Some(earlier) = earlier {
                        let previous_empty = unit_empty[&(*unit, earlier.0, earlier.1)];
                        add_constraint(&mut model, &mut formulation, Expression::from(empty.variable).geq(previous_empty.variable));
                        let previous_chain = unit_chain_empty[&(*unit, earlier.0, earlier.1)];
                        let chain = unit_chain_empty[&(*unit, interval.index, segment)];
                        add_constraint(&mut model, &mut formulation, Expression::from(chain.variable).geq(previous_chain.variable));
                    }

                    let mut reclaim_by_column = BTreeMap::new();
                    let unit_reclaims: Vec<_> = formulation
                        .reclaim
                        .iter()
                        .filter(|reclaim| reclaim.unit == *unit && reclaim.interval == interval.index && reclaim.segment == segment)
                        .map(|reclaim| reclaim.column)
                        .collect();
                    for reclaim in unit_reclaims {
                        reclaim_by_column.entry(reclaim.index).or_insert(reclaim);
                        add_constraint(&mut model, &mut formulation, Expression::from(reclaim.variable).leq(bound * selected.clone()));
                    }
                    // A unit cannot be drawn below empty. Every cell but the
                    // last says so through the next cell's opening equality,
                    // whose left-hand column is bounded below by zero; the
                    // last cell has no successor, so it states it directly.
                    if interval.index + 1 == input.intervals.len() && segment + 1 == segments_of(interval.index) {
                        let carried = carried_balance(&formulation, *unit, (interval.index, segment));
                        add_constraint(&mut model, &mut formulation, carried.geq(0.0));
                    }

                    match *unit {
                        UnitKey::Opening { opening, .. } if interval.index == 0 && segment == 0 => {
                            add_constraint(&mut model, &mut formulation, Expression::from(start.variable).eq(pile.opening[opening].tonnes_t));
                        }
                        UnitKey::Opening { .. } => {
                            let previous = if segment > 0 {
                                (interval.index, segment - 1)
                            } else {
                                (interval.index - 1, segments_of(interval.index - 1) - 1)
                            };
                            let carried = carried_balance(&formulation, *unit, previous);
                            add_constraint(&mut model, &mut formulation, Expression::from(start.variable).eq(carried));
                        }
                        UnitKey::Receipt { receipt_interval, .. } if interval.index <= receipt_interval => {
                            add_constraint(&mut model, &mut formulation, Expression::from(start.variable).eq(0.0));
                        }
                        UnitKey::Receipt {
                            receipt_interval,
                            segment: receipt_segment,
                            position,
                            ..
                        } if interval.index == receipt_interval + 1 && segment == 0 => {
                            let delivered: Vec<_> = formulation
                                .parcels
                                .iter()
                                .filter(|parcel| {
                                    parcel.stream.pile == pile.id
                                        && parcel.stream.interval == receipt_interval
                                        && parcel.stream.segment == receipt_segment
                                        && parcel.position == position
                                })
                                .map(|parcel| (parcel.quantity, 1.0))
                                .collect();
                            add_constraint(&mut model, &mut formulation, Expression::from(start.variable).eq(sum(delivered)));
                        }
                        UnitKey::Receipt { .. } => {
                            let previous = if segment > 0 {
                                (interval.index, segment - 1)
                            } else {
                                (interval.index - 1, segments_of(interval.index - 1) - 1)
                            };
                            let carried = carried_balance(&formulation, *unit, previous);
                            add_constraint(&mut model, &mut formulation, Expression::from(start.variable).eq(carried));
                        }
                    }
                }

                // Stockpile inventory carried from the previous boundary:
                // what it closed on, plus this segment's receipts, less this
                // segment's draws.
                let receipts: Vec<_> = formulation
                    .parcels
                    .iter()
                    .filter(|parcel| parcel.stream.pile == pile.id && parcel.stream.interval == interval.index && parcel.stream.segment == segment)
                    .map(|parcel| (parcel.quantity, 1.0))
                    .collect();
                let mut reclaimed = BTreeMap::new();
                for reclaim in formulation
                    .reclaim
                    .iter()
                    .filter(|reclaim| reclaim.source == pile.id && reclaim.interval == interval.index && reclaim.segment == segment)
                {
                    reclaimed.entry(reclaim.column.index).or_insert(reclaim.column);
                }
                let previous = cell_of(interval.index, segment).map_or_else(
                    || Expression::from(pile.opening.iter().map(|lot| lot.tonnes_t).sum::<f64>()),
                    |cell| Expression::from(formulation.pile_closing[&(pile.id, cell.0, cell.1)].variable),
                );
                let closing = formulation.pile_closing[&(pile.id, interval.index, segment)];
                add_constraint(
                    &mut model,
                    &mut formulation,
                    Expression::from(closing.variable).eq(previous + sum(receipts) - sum(reclaimed.values().copied().map(|column| (column, 1.0)))),
                );
            }
        }
    }

    // Reclaim readiness per segment is fixed by released start inventory and
    // the authored window, never by whether the solver later chooses to move
    // tonnes. The selected task gates its shared lot extraction, and the bar
    // maximum is cumulative across every segment.
    for (task_index, task) in input.tasks.iter().enumerate() {
        let TaskKind::Reclaim { approved_sources, maximum_t } = &task.kind else { continue };
        let loader = input.loaders.iter().find(|loader| loader.id == task.loader).expect("validated loader");
        let mut cumulative = BTreeMap::new();
        for interval in &input.intervals {
            for segment in 0..segments_of(interval.index) {
                let ready = task_ready[&(task_index, interval.index, segment)];
                let selected_task = formulation.task_selected[&(task_index, interval.index, segment)];
                let duration = formulation.durations[&(interval.index, segment)];
                let fixed = active(task, *interval) && loader.rates[interval.index].reclaim_tph > 0.0;
                if !fixed {
                    add_constraint(&mut model, &mut formulation, Expression::from(ready.variable).eq(0.0));
                    continue;
                }
                let available: Vec<_> = approved_sources.iter().map(|pile| (pile_available[&(*pile, interval.index, segment)], 1.0)).collect();
                add_constraint(&mut model, &mut formulation, Expression::from(ready.variable).leq(sum(available.clone())));
                for (pile, _) in &available {
                    add_constraint(&mut model, &mut formulation, Expression::from(ready.variable).geq(pile.variable));
                }
                let mut segment_reclaim = BTreeMap::new();
                for reclaim in formulation
                    .reclaim
                    .iter()
                    .filter(|reclaim| reclaim.task == task_index && reclaim.interval == interval.index && reclaim.segment == segment)
                {
                    segment_reclaim.entry(reclaim.column.index).or_insert(reclaim.column);
                    cumulative.entry(reclaim.column.index).or_insert(reclaim.column);
                }
                let total = sum(segment_reclaim.values().copied().map(|column| (column, 1.0)));
                let rate = loader.rates[interval.index].reclaim_tph * interval.duration_h();
                add_constraint(&mut model, &mut formulation, total.clone().leq(rate * selected_task.variable));
                add_constraint(&mut model, &mut formulation, total.leq(loader.rates[interval.index].reclaim_tph * duration.variable));
            }
        }
        if let Some(maximum) = maximum_t {
            add_constraint(&mut model, &mut formulation, sum(cumulative.values().copied().map(|column| (column, 1.0))).leq(*maximum));
        }
    }

    // A mixed opening lot is removed in its authored proportions. A receipt
    // position contains exactly one fixed material stream. In both cases every
    // extracted portion must take a permitted crusher/dump route.
    let reclaim_rows = formulation.reclaim.clone();
    for reclaim in reclaim_rows {
        if let UnitKey::Receipt {
            receipt_interval,
            segment: receipt_segment,
            position,
            ..
        } = reclaim.unit
        {
            let supplied: Vec<_> = formulation
                .parcels
                .iter()
                .filter(|parcel| {
                    parcel.stream.pile == reclaim.source
                        && parcel.stream.interval == receipt_interval
                        && parcel.stream.segment == receipt_segment
                        && parcel.position == position
                        && parcel.stream.material == reclaim.material
                })
                .map(|parcel| (parcel.quantity, 1.0))
                .collect();
            add_constraint(&mut model, &mut formulation, Expression::from(reclaim.column.variable).leq(sum(supplied)));
        }
        let routed: Vec<_> = formulation
            .movements
            .iter()
            .filter(|movement| {
                movement.task == reclaim.task
                    && movement.interval == reclaim.interval
                    && movement.segment == reclaim.segment
                    && movement.unit == Some(reclaim.unit)
                    && movement.material == reclaim.material
            })
            .map(|movement| (movement.column, 1.0))
            .collect();
        // Skipped when the single route shares the draw's own column: the
        // row would say `x = x`.
        if routed.len() != 1 || routed[0].0.index != reclaim.column.index {
            add_constraint(&mut model, &mut formulation, sum(routed).eq(reclaim.fraction * reclaim.column.variable));
        }
    }

    // Shared fleet capacity per segment: movement truck-hours within the
    // duration-proportional share of every class's interval budget, so two
    // loaders can never overallocate the fleet at the same instant even when
    // the interval total would have permitted it.
    for truck in &input.trucks {
        for interval in &input.intervals {
            let units = truck.hours[interval.index] / interval.duration_h();
            for segment in 0..segments_of(interval.index) {
                let duration = formulation.durations[&(interval.index, segment)];
                let use_hours: Vec<_> = formulation
                    .movements
                    .iter()
                    .filter(|movement| movement.interval == interval.index && movement.segment == segment && input.movements[movement.candidate].truck == truck.id)
                    .map(|movement| (movement.column, input.movements[movement.candidate].truck_hours_per_tonne))
                    .collect();
                add_constraint(&mut model, &mut formulation, sum(use_hours).leq(units * duration.variable));
            }
        }
    }

    // Daily crusher budgets and dump capacities span every segment.
    for destination in &input.destinations {
        match destination.kind {
            DestinationKind::Crusher => {
                for (day, limit) in destination.crusher_daily_t.iter().enumerate() {
                    if let Some(limit) = limit {
                        let receipts: Vec<_> = formulation
                            .movements
                            .iter()
                            .filter(|movement| input.movements[movement.candidate].destination == destination.id && input.intervals[movement.interval].day() as usize == day)
                            .map(|movement| (movement.column, 1.0))
                            .collect();
                        add_constraint(&mut model, &mut formulation, sum(receipts).leq(*limit));
                    }
                }
            }
            DestinationKind::Dump => {
                if let Some(capacity) = destination.capacity_t {
                    let receipts: Vec<_> = formulation
                        .movements
                        .iter()
                        .filter(|movement| input.movements[movement.candidate].destination == destination.id)
                        .map(|movement| (movement.column, 1.0))
                        .collect();
                    add_constraint(&mut model, &mut formulation, sum(receipts).leq(capacity));
                }
            }
            DestinationKind::Stockpile(_) => {}
        }
    }

    if cancellation.is_cancelled() {
        return empty_result(input, &budgets, SolveStatus::Cancelled, None, started, started.elapsed(), &formulation);
    }
    // Seed stage restriction, added last so its rows form one contiguous
    // block that can be relaxed in place afterwards. Holding every interval
    // to its first execution segment leaves a model with the same columns -
    // so its solution is a valid starting point for the unrestricted one -
    // but roughly half the live cells, and it is the conservative schedule a
    // planner would recognise: no within-interval source transitions.
    let mut seed_rows = Vec::new();
    for interval in &input.intervals {
        for segment in 1..segments_of(interval.index) {
            let duration = formulation.durations[&(interval.index, segment)];
            seed_rows.push(formulation.constraints);
            add_constraint(&mut model, &mut formulation, Expression::from(duration.variable).leq(0.0));
        }
    }

    let formulation_time = started.elapsed();
    let mut backend = match model.try_into_inner() {
        Ok(model) => model,
        Err(error) => {
            return empty_result(
                input,
                &budgets,
                SolveStatus::BackendFailure,
                Some(error.to_string()),
                started,
                formulation_time,
                &formulation,
            );
        }
    };
    backend.make_quiet();
    backend.set_option("allow_unbounded_or_infeasible", false);
    // The formulation deliberately mixes dust-scale activation rows (parcel
    // occupancy, negligible-inventory thresholds around 1e-5..1e-4) with
    // tonnage rows in the thousands. At HiGHS's default 1e-7/1e-6 feasibility
    // tolerances its presolve made invalid reductions on such rows and
    // returned suboptimal solutions labelled optimal (reproduced by a fixture
    // whose whole-horizon optimum was cut off); 1e-9 restores correctness
    // while keeping presolve enabled.
    // Interior-point root relaxation. With the simplex root LP this model's
    // cut loop does not terminate: the dual bound is reached by the first
    // relaxation and never moves, yet HiGHS keeps separating - ten thousand
    // cuts, zero nodes explored, zero heuristic iterations and no incumbent
    // at all on horizons of eight hours and up. The degeneracy is structural
    // - thousands of zero-cost indicator columns over a shared event clock -
    // not a scaling accident, and an interior-point root relaxation steps
    // around it: the twelve-hour audit fixture goes from no incumbent in
    // sixty seconds to proven optimal in under thirty.
    backend.set_option("mip_lp_solver", "ipm");
    backend.set_option("primal_feasibility_tolerance", PRIMAL_FEASIBILITY_TOLERANCE);
    backend.set_option("mip_feasibility_tolerance", MIP_FEASIBILITY_TOLERANCE);
    if let Some(limit) = limits.time {
        backend.set_option("time_limit", limit.as_secs_f64());
    }
    if let Some(gap) = limits.relative_gap {
        backend.set_option("mip_rel_gap", gap);
    }
    if let Some(threads) = limits.threads {
        backend.set_option("threads", threads as i32);
    }
    if let Some(nodes) = limits.nodes {
        backend.set_option("mip_max_nodes", i32::try_from(nodes).unwrap_or(i32::MAX));
    }
    if let Some(solutions) = limits.solutions {
        backend.set_option("mip_max_improving_sols", i32::try_from(solutions).unwrap_or(i32::MAX));
    }
    let solve_started = Instant::now();
    let seed_budget = limits.time.map(|limit| limit.mul_f64(SEED_STAGE_SHARE).min(SEED_STAGE_BUDGET)).unwrap_or(SEED_STAGE_BUDGET);
    if !seed_rows.is_empty() && seed_budget >= PREFERENCE_STAGE_MINIMUM {
        backend.set_option("time_limit", seed_budget.as_secs_f64());
        let mut restricted = match backend.try_solve() {
            Ok(solution) => solution,
            Err(status) => {
                return empty_result(
                    input,
                    &budgets,
                    SolveStatus::BackendFailure,
                    Some(format!("HiGHS call failed: {status:?}")),
                    started,
                    formulation_time,
                    &formulation,
                );
            }
        };
        let seed = (restricted.primal_solution_status() == HighsSolutionStatus::Feasible).then(|| restricted.get_solution().columns().to_vec());
        let pointer = restricted.as_mut_ptr();
        unsafe {
            for row in &seed_rows {
                highs_sys::Highs_changeRowBounds(pointer, *row as i32, f64::NEG_INFINITY, f64::INFINITY);
            }
            if let Some(values) = &seed {
                highs_sys::Highs_setSolution(pointer, values.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null());
            }
        }
        backend = ::highs::Model::from(restricted);
    }
    if let Some(limit) = limits.time {
        backend.set_option("time_limit", limit.saturating_sub(solve_started.elapsed()).as_secs_f64().max(0.1));
    }
    // Experimental: hand this exact model to another backend (see
    // `mps_export`). Unrestricted here - the seed rows have been relaxed - so
    // the file holds the model this call is about to solve.
    #[cfg(feature = "scip-code")]
    if let Some(path) = mps_export::take()
        && let Ok(target) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
    {
        unsafe { highs_sys::Highs_writeModel(backend.as_mut_ptr(), target.as_ptr()) };
    }
    let mut solved = match backend.try_solve() {
        Ok(solution) => solution,
        Err(status) => {
            return empty_result(
                input,
                &budgets,
                SolveStatus::BackendFailure,
                Some(format!("HiGHS call failed: {status:?}")),
                started,
                formulation_time,
                &formulation,
            );
        }
    };
    let status = solved.status();
    let feasible = solved.primal_solution_status() == HighsSolutionStatus::Feasible;
    let bound = solved.double_info_value(c"mip_dual_bound").ok().filter(|value| value.is_finite());
    let gap = solved.mip_gap();
    let mapped = match status {
        HighsModelStatus::Optimal => SolveStatus::Optimal,
        HighsModelStatus::ReachedTimeLimit
        | HighsModelStatus::ReachedIterationLimit
        | HighsModelStatus::ReachedSolutionLimit
        | HighsModelStatus::ReachedMemoryLimit
        | HighsModelStatus::ObjectiveBound
        | HighsModelStatus::ObjectiveTarget => {
            if feasible {
                SolveStatus::FeasibleLimit
            } else {
                SolveStatus::LimitNoIncumbent
            }
        }
        HighsModelStatus::ReachedInterrupt => {
            if cancellation.is_cancelled() {
                SolveStatus::Cancelled
            } else if feasible {
                SolveStatus::FeasibleLimit
            } else {
                SolveStatus::LimitNoIncumbent
            }
        }
        HighsModelStatus::Infeasible => SolveStatus::Infeasible,
        HighsModelStatus::Unbounded => SolveStatus::Unbounded,
        HighsModelStatus::UnboundedOrInfeasible => SolveStatus::BackendFailure,
        _ => SolveStatus::BackendFailure,
    };
    if !feasible {
        return empty_result(input, &budgets, mapped, Some(format!("HiGHS status: {status:?}")), started, formulation_time, &formulation);
    }
    let mut values = solved.get_solution().columns().to_vec();

    // Second lexicographic stage: hold the primary objective at what stage
    // one achieved (to the documented objective tolerance) and optimise the
    // tie preferences under that floor. The stage-one solution is handed back
    // as a starting point, so the stage can only improve on it and never
    // reports a worse primal. If the remaining budget is exhausted the
    // stage-one solution is published and `tie_preferences_applied` is false,
    // which is why that flag is part of the summary rather than implied.
    let mut tie_preferences_applied = false;
    let stage_one_primary: f64 = primary_costs.iter().zip(&values).map(|(cost, value)| cost * value).sum();
    let preference_budget = limits
        .time
        .map(|limit| limit.saturating_sub(solve_started.elapsed()))
        .unwrap_or(PREFERENCE_STAGE_BUDGET)
        .min(PREFERENCE_STAGE_BUDGET);
    if secondary_costs.iter().any(|cost| *cost != 0.0) && preference_budget >= PREFERENCE_STAGE_MINIMUM && !cancellation.is_cancelled() {
        // The pinned `highs` wrapper cannot re-cost an existing model or seed
        // a starting solution, and rebuilding the whole formulation for one
        // tie-break stage would double the formulation time on the horizons
        // that need it least. The three C entry points used here are the ones
        // the wrapper is missing; everything afterwards goes back through the
        // safe wrapper.
        let floor = stage_one_primary - tolerance.objective.max(stage_one_primary.abs() * MIP_FEASIBILITY_TOLERANCE);
        let row: Vec<(i32, f64)> = primary_costs
            .iter()
            .enumerate()
            .filter(|(_, cost)| **cost != 0.0)
            .map(|(index, cost)| (index as i32, *cost))
            .collect();
        let indices: Vec<i32> = row.iter().map(|(index, _)| *index).collect();
        let coefficients: Vec<f64> = row.iter().map(|(_, cost)| *cost).collect();
        let seed = values.clone();
        let pointer = solved.as_mut_ptr();
        let staged = unsafe {
            let costs = highs_sys::Highs_changeColsCostByRange(pointer, 0, secondary_costs.len() as i32 - 1, secondary_costs.as_ptr());
            let added = highs_sys::Highs_addRow(pointer, floor, f64::INFINITY, indices.len() as i32, indices.as_ptr(), coefficients.as_ptr());
            let seeded = highs_sys::Highs_setSolution(pointer, seed.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null());
            costs == highs_sys::STATUS_OK && added == highs_sys::STATUS_OK && seeded != highs_sys::STATUS_ERROR
        };
        let mut model = ::highs::Model::from(solved);
        if staged {
            model.set_option("time_limit", preference_budget.as_secs_f64());
            if let Ok(second) = model.try_solve()
                && second.primal_solution_status() == HighsSolutionStatus::Feasible
            {
                let candidate = second.get_solution().columns().to_vec();
                let primary: f64 = primary_costs.iter().zip(&candidate).map(|(cost, value)| cost * value).sum();
                if primary >= floor - tolerance.objective {
                    values = candidate;
                    tie_preferences_applied = true;
                }
            }
        }
    }
    let solve_time = solve_started.elapsed();
    if let Some(violation) = formulation
        .columns
        .binary_indices
        .iter()
        .map(|index| (values[*index] - values[*index].round()).abs())
        .find(|violation| *violation > tolerance.integrality)
    {
        return empty_result(
            input,
            &budgets,
            SolveStatus::BackendFailure,
            Some(format!("integrality violation {violation}")),
            started,
            formulation_time,
            &formulation,
        );
    }
    let raw_solver_objective: f64 = formulation
        .movements
        .iter()
        .map(|movement| values[movement.column.index] * input.movements[movement.candidate].value_per_tonne().expect("validated"))
        .sum();

    // Parcel quantities are snapped to the documented modelling tolerance: a
    // parcel within τ of the target is a full parcel; a parcel at or below τ is
    // dust from LP feasibility slack. Dust never creates a receipt position -
    // its tonnes merge into the stream's last published parcel (or publish as
    // the stream's single remainder when the stream is otherwise empty), and
    // positions compact within the (pile, interval, segment) group. τ is a
    // modelling quantity, never a physical production quantum.
    let snap = tolerance.tonnes_t.max(input.horizon.parcel_target_t * 1e-6);
    // The same τ-derived threshold governs every published tonnage row:
    // tonnes at or below it are LP feasibility slack, not production, and
    // publishing them would break parcel and balance reconciliation.
    let negligible = tolerance.tonnes_t.max(snap);

    let mut movements = Vec::new();
    for movement in &formulation.movements {
        let tonnes = values[movement.column.index];
        if tonnes <= negligible {
            continue;
        }
        let candidate = &input.movements[movement.candidate];
        movements.push(MovementRecord {
            interval: movement.interval,
            segment: movement.segment,
            loader: candidate.loader,
            task: input.tasks[movement.task].id,
            source: movement.source,
            material: movement.material,
            destination: candidate.destination,
            truck: candidate.truck,
            tonnes_t: tonnes,
            truck_hours: tonnes * candidate.truck_hours_per_tonne,
            routing_rule: candidate.routing_rule,
            cashflow: candidate.cashflow.clone(),
            value: tonnes * candidate.value_per_tonne().expect("validated"),
        });
    }
    // The published objective is the sum of published movements, so the two
    // reconcile exactly rather than up to a solver-internal dust difference.
    let primary_objective: f64 = movements.iter().map(|movement| movement.value).sum();

    let mut audit = PublicationAudit::default();
    let mut parcels = Vec::new();
    let mut dust_by_stream: BTreeMap<StreamKey, f64> = BTreeMap::new();
    let mut dust_position_by_stream: BTreeMap<StreamKey, usize> = BTreeMap::new();
    for parcel in &formulation.parcels {
        let raw = values[parcel.quantity.index];
        let tonnes = if (raw - input.horizon.parcel_target_t).abs() <= snap {
            input.horizon.parcel_target_t
        } else if raw <= snap || raw < tolerance.parcel_occupancy_t * 0.5 {
            0.0
        } else {
            raw
        };
        let adjustment = (raw - tonnes).abs();
        audit.parcel_max_adjustment_t = audit.parcel_max_adjustment_t.max(adjustment);
        audit.parcel_total_adjustment_t += adjustment;
        if tonnes == 0.0 {
            *dust_by_stream.entry(parcel.stream).or_insert(0.0) += raw;
            dust_position_by_stream.insert(parcel.stream, parcel.position);
            continue;
        }
        parcels.push(ReceiptParcel {
            stockpile: parcel.stream.pile,
            receipt_interval: parcel.stream.interval,
            segment: parcel.stream.segment,
            release_interval: parcel.stream.interval + 1,
            position: parcel.position,
            source: parcel.stream.source,
            material: parcel.stream.material,
            tonnes_t: tonnes,
        });
    }
    for (stream, dust) in dust_by_stream {
        if dust <= negligible {
            continue;
        }
        let in_stream = |parcel: &ReceiptParcel| {
            parcel.stockpile == stream.pile
                && parcel.receipt_interval == stream.interval
                && parcel.segment == stream.segment
                && parcel.source == stream.source
                && parcel.material == stream.material
        };
        match parcels.iter().rposition(in_stream) {
            Some(index) => parcels[index].tonnes_t += dust,
            None => parcels.push(ReceiptParcel {
                stockpile: stream.pile,
                receipt_interval: stream.interval,
                segment: stream.segment,
                release_interval: stream.interval + 1,
                position: dust_position_by_stream[&stream],
                source: stream.source,
                material: stream.material,
                tonnes_t: dust,
            }),
        }
    }
    parcels.sort_by_key(|parcel| (parcel.stockpile, parcel.receipt_interval, parcel.segment, parcel.position));
    // Positions compact per group, but a position is one inventory unit: the
    // streams that supplied the same position share it, exactly as the model
    // holds them (one balance, one emptiness, one place in the FIFO order),
    // so their parcels keep the same compacted position and the published
    // rows stay in one-to-one correspondence with the solved units.
    let mut parcel_positions: BTreeMap<(StockpileId, usize, usize, usize), usize> = BTreeMap::new();
    let mut next_position = 0;
    let mut current_group = None;
    for parcel in &mut parcels {
        let group = (parcel.stockpile, parcel.receipt_interval, parcel.segment);
        if current_group != Some(group) {
            current_group = Some(group);
            next_position = 0;
        }
        let key = (parcel.stockpile, parcel.receipt_interval, parcel.segment, parcel.position);
        let position = match parcel_positions.get(&key) {
            Some(position) => *position,
            None => {
                let position = next_position;
                parcel_positions.insert(key, position);
                next_position += 1;
                position
            }
        };
        parcel.position = position;
    }

    let segment_durations_h: Vec<Vec<f64>> = input
        .intervals
        .iter()
        .map(|interval| {
            (0..segments_of(interval.index))
                .map(|segment| values[formulation.durations[&(interval.index, segment)].index])
                .collect()
        })
        .collect();
    let used_segments: Vec<usize> = segment_durations_h
        .iter()
        .map(|durations| durations.iter().filter(|duration| **duration > 1e-6).count())
        .collect();

    // Dig rows get the same treatment as reclaims: a row that exhausts its
    // ground source within the dust scale publishes the source's remaining
    // published balance, so depleted blocks balance to exactly zero instead
    // of leaving a crumb that the replay would treat as live work, and rows
    // snapped to nothing (residual crumbs) do not publish at all.
    let mut dig_rows: Vec<ActivityRecord> = formulation
        .dig
        .iter()
        .filter_map(|dig| {
            let tonnes = values[dig.column.index];
            (tonnes > negligible).then(|| ActivityRecord {
                interval: dig.interval,
                segment: dig.segment,
                loader: input.tasks[dig.task].loader,
                task: input.tasks[dig.task].id,
                activity: Activity::Dig,
                source: SourceId::Ground(dig.source),
                tonnes_t: tonnes,
            })
        })
        .collect();
    {
        let mut ground_balance: BTreeMap<GroundId, f64> = input.ground.iter().map(|ground| (ground.id, ground.tonnes_t)).collect();
        dig_rows.sort_by_key(|row| (row.interval, row.segment, row.loader, row.task));
        for row in &mut dig_rows {
            let Some(balance) = ground_balance.get_mut(&match row.source {
                SourceId::Ground(id) => id,
                _ => continue,
            }) else {
                continue;
            };
            if row.tonnes_t + 3.0 * dust >= *balance {
                let snapped = (*balance).max(0.0);
                let adjustment = (row.tonnes_t - snapped).abs();
                if adjustment > 0.0 {
                    audit.dig_rows_snapped += 1;
                    audit.dig_max_adjustment_t = audit.dig_max_adjustment_t.max(adjustment);
                    audit.dig_total_adjustment_t += adjustment;
                }
                row.tonnes_t = snapped;
            }
            *balance -= row.tonnes_t;
        }
        dig_rows.retain(|row| row.tonnes_t > negligible);
    }
    let mut activities = dig_rows;
    let mut reclaims = Vec::new();
    let mut reclaim_seen = BTreeSet::new();
    for reclaim in &formulation.reclaim {
        if !reclaim_seen.insert(reclaim.column.index) {
            continue;
        }
        let tonnes = values[reclaim.column.index];
        if tonnes <= 0.0 {
            continue;
        }
        let unit = match reclaim.unit {
            UnitKey::Opening { opening, .. } => InventoryUnit::Opening(pile_index[&reclaim.source].opening[opening].id),
            UnitKey::Receipt {
                receipt_interval,
                segment,
                position,
                ..
            } => {
                let Some(published) = parcel_positions.get(&(reclaim.source, receipt_interval, segment, position)) else {
                    // The parcel this row draws from was itself at or below
                    // the publication threshold, so it did not publish and
                    // its tonnes merged into the stream's last published
                    // parcel. The draw against it is dust of the same scale:
                    // record it as a publication correction rather than
                    // emitting a row against a unit the timeline does not
                    // contain.
                    audit.reclaim_rows_snapped += 1;
                    audit.reclaim_max_adjustment_t = audit.reclaim_max_adjustment_t.max(tonnes);
                    audit.reclaim_total_adjustment_t += tonnes;
                    continue;
                };
                InventoryUnit::Receipt {
                    interval: receipt_interval,
                    segment,
                    position: *published,
                }
            }
        };
        reclaims.push(ReclaimRecord {
            interval: reclaim.interval,
            segment: reclaim.segment,
            loader: input.tasks[reclaim.task].loader,
            task: input.tasks[reclaim.task].id,
            stockpile: reclaim.source,
            unit,
            tonnes_t: tonnes,
        });
    }
    // Snap each reclaim row to the published balance of its unit when the row
    // exhausts it within the dust scale. The model exhausts units against raw
    // solver values while published parcels carry snap-adjusted quantities;
    // without this pass the published residual of an exhausted unit can land
    // a hair above the negligible-inventory threshold and the replayed FIFO/
    // LIFO order would disagree with the solved timeline. Snapped rows make
    // exhausted units balance to exactly zero; partial draws keep raw values.
    {
        let mut published_balance: BTreeMap<InventoryUnit, f64> = BTreeMap::new();
        for pile in &input.stockpiles {
            for lot in &pile.opening {
                published_balance.insert(InventoryUnit::Opening(lot.id), lot.tonnes_t);
            }
        }
        for parcel in &parcels {
            // Several streams can share one position: they are one unit.
            *published_balance
                .entry(InventoryUnit::Receipt {
                    interval: parcel.receipt_interval,
                    segment: parcel.segment,
                    position: parcel.position,
                })
                .or_insert(0.0) += parcel.tonnes_t;
        }
        reclaims.sort_by_key(|row| (row.interval, row.segment, row.loader, row.task));
        // A draw below the publication threshold is dust and does not
        // publish - unless it is the row that takes its unit into the
        // negligible band, which is decided by the balance the row *leaves*,
        // not the one it found.
        //
        // Both directions of that test are load-bearing, and each has its own
        // failure. A unit holding twice the dust scale can be retired by a
        // draw of exactly the dust scale, leaving a residual the model calls
        // empty; drop that row and the published unit keeps the whole
        // parcel, and the replayed FIFO/LIFO order demands the phantom be
        // reclaimed before anything newer. Conversely a selection binary may
        // sit an integrality tolerance away from zero, so the model can carry
        // a sub-picogram draw against a unit the order has *not* reached;
        // publish that row and the timeline claims a loader dug out of turn.
        // The balance after the row separates the two exactly.
        let empty_limit = unit_dust + PRIMAL_FEASIBILITY_TOLERANCE;
        let mut keep = Vec::with_capacity(reclaims.len());
        for row in &mut reclaims {
            let Some(balance) = published_balance.get_mut(&row.unit) else {
                keep.push(row.tonnes_t > negligible);
                continue;
            };
            // A row retires its unit when the balance it leaves is inside the
            // negligible band. Such a row publishes whatever its size, and
            // snaps to the whole remaining balance so the unit lands on
            // exactly zero; a row that leaves real stock behind publishes
            // only if it carries real stock itself.
            let retires = row.tonnes_t > 0.0 && *balance - row.tonnes_t <= empty_limit;
            let publishes = row.tonnes_t > negligible || retires;
            keep.push(publishes);
            if !publishes {
                // Slack, not production: it must not consume the published
                // balance either, or the next row would find the unit short.
                audit.reclaim_rows_snapped += 1;
                audit.reclaim_max_adjustment_t = audit.reclaim_max_adjustment_t.max(row.tonnes_t);
                audit.reclaim_total_adjustment_t += row.tonnes_t;
                continue;
            }
            if retires || row.tonnes_t + 3.0 * unit_dust >= *balance {
                let snapped = (*balance).max(0.0);
                let adjustment = (row.tonnes_t - snapped).abs();
                if adjustment > 0.0 {
                    audit.reclaim_rows_snapped += 1;
                    audit.reclaim_max_adjustment_t = audit.reclaim_max_adjustment_t.max(adjustment);
                    audit.reclaim_total_adjustment_t += adjustment;
                }
                row.tonnes_t = snapped;
            }
            *balance -= row.tonnes_t;
        }
        let mut kept = keep.into_iter();
        reclaims.retain(|row| kept.next().unwrap_or(true) && row.tonnes_t > 0.0);
        for row in &reclaims {
            activities.push(ActivityRecord {
                interval: row.interval,
                segment: row.segment,
                loader: row.loader,
                task: row.task,
                activity: Activity::Reclaim,
                source: SourceId::Stockpile(row.stockpile),
                tonnes_t: row.tonnes_t,
            });
        }
    }
    activities.sort_by_key(|activity| (activity.interval, activity.segment, activity.loader, activity.task, activity.source));

    // Budget-restriction heuristic: a loader that worked an interval's final
    // segment while any of its authored work for that interval remains
    // unfinished - the stage it was on, or a later stage it never reached -
    // may have found another transition had more event positions been
    // granted. A true flag does not prove production was lost (the loader may
    // have been legitimately throttled or the work may be booked to a later
    // interval); it says the budget may bind, and the run should be repeated
    // with a larger override before its optimality is trusted.
    let mut event_budget_restricted = false;
    let extracted_by_ground: BTreeMap<GroundId, f64> = input
        .ground
        .iter()
        .map(|ground| {
            let extracted: f64 = activities
                .iter()
                .filter(|row| row.activity == Activity::Dig && row.source == SourceId::Ground(ground.id))
                .map(|row| row.tonnes_t)
                .sum();
            (ground.id, ground.tonnes_t - extracted)
        })
        .collect();
    let stock_by_pile: BTreeMap<StockpileId, f64> = input
        .stockpiles
        .iter()
        .map(|pile| {
            let held: f64 = pile.opening.iter().map(|lot| lot.tonnes_t).sum::<f64>()
                + parcels.iter().filter(|parcel| parcel.stockpile == pile.id).map(|parcel| parcel.tonnes_t).sum::<f64>()
                - reclaims.iter().filter(|row| row.stockpile == pile.id).map(|row| row.tonnes_t).sum::<f64>();
            (pile.id, held)
        })
        .collect();
    for (interval, budget) in budgets.iter().enumerate() {
        if *budget == 0 {
            continue;
        }
        let last = budget - 1;
        let interval_data = &input.intervals[interval];
        for loader in &input.loaders {
            let worked_last = activities
                .iter()
                .any(|row| row.loader == loader.id && row.interval == interval && row.segment == last && row.tonnes_t > 0.0);
            if !worked_last {
                continue;
            }
            let unfinished = task_order(input, loader.id).iter().any(|&task_index| {
                let task = &input.tasks[task_index];
                active(task, *interval_data)
                    && match &task.kind {
                        TaskKind::Dig { sequence } => sequence.iter().any(|ground| extracted_by_ground.get(ground).copied().unwrap_or(0.0) > tolerance.tonnes_t),
                        TaskKind::Reclaim { approved_sources, .. } => approved_sources.iter().any(|pile| stock_by_pile.get(pile).copied().unwrap_or(0.0) > tolerance.tonnes_t),
                    }
            });
            if unfinished {
                event_budget_restricted = true;
            }
        }
    }

    // Balances are derived from the published rows rather than read back from
    // solver variables, so every published figure reconciles with the records
    // around it; solver dust never reaches the output.
    let pile_destination: BTreeMap<StockpileId, DestinationId> = input
        .destinations
        .iter()
        .filter_map(|destination| match destination.kind {
            DestinationKind::Stockpile(pile) => Some((pile, destination.id)),
            _ => None,
        })
        .collect();
    let balances = input
        .intervals
        .iter()
        .map(|interval| {
            let ground_remaining_t: BTreeMap<GroundId, f64> = input
                .ground
                .iter()
                .map(|ground| {
                    let extracted: f64 = activities
                        .iter()
                        .filter(|row| row.interval <= interval.index && row.source == SourceId::Ground(ground.id))
                        .map(|row| row.tonnes_t)
                        .sum();
                    (ground.id, ground.tonnes_t - extracted)
                })
                .collect();
            let stockpile_closing_t: BTreeMap<StockpileId, f64> = input
                .stockpiles
                .iter()
                .map(|pile| {
                    let receipts = pile_destination.get(&pile.id).map_or(0.0, |destination| {
                        movements
                            .iter()
                            .filter(|row| row.interval <= interval.index && row.destination == *destination)
                            .map(|row| row.tonnes_t)
                            .sum()
                    });
                    let reclaimed: f64 = reclaims
                        .iter()
                        .filter(|row| row.interval <= interval.index && row.stockpile == pile.id)
                        .map(|row| row.tonnes_t)
                        .sum();
                    (pile.id, pile.opening.iter().map(|lot| lot.tonnes_t).sum::<f64>() + receipts - reclaimed)
                })
                .collect();
            BalanceRecord {
                interval: interval.index,
                ground_remaining_t,
                stockpile_closing_t,
                destination_cumulative_t: input
                    .destinations
                    .iter()
                    .map(|destination| {
                        let tonnes = movements
                            .iter()
                            .filter(|movement| movement.destination == destination.id && movement.interval <= interval.index)
                            .map(|movement| movement.tonnes_t)
                            .sum();
                        (destination.id, tonnes)
                    })
                    .collect(),
                crusher_daily_used_t: input
                    .destinations
                    .iter()
                    .filter(|destination| matches!(destination.kind, DestinationKind::Crusher))
                    .map(|destination| {
                        let day = interval.day();
                        let tonnes = movements
                            .iter()
                            .filter(|movement| movement.destination == destination.id && input.intervals[movement.interval].day() == day)
                            .map(|movement| movement.tonnes_t)
                            .sum();
                        ((destination.id, day), tonnes)
                    })
                    .collect(),
            }
        })
        .collect();
    // Largest excess of the published timeline against the original hard
    // capacities, computed from the published rows themselves: per-segment
    // loader rates and fleet shares, daily crusher budgets, dump capacities,
    // stockpile capacity at every segment end, and reclaim bar maxima.
    {
        let mut excess = 0.0_f64;
        for loader in &input.loaders {
            for interval in &input.intervals {
                for segment in 0..segments_of(interval.index) {
                    let duration = segment_durations_h[interval.index].get(segment).copied().unwrap_or(0.0);
                    let rows: Vec<&ActivityRecord> = activities
                        .iter()
                        .filter(|row| row.loader == loader.id && row.interval == interval.index && row.segment == segment)
                        .collect();
                    let dig: f64 = rows.iter().filter(|row| row.activity == Activity::Dig).map(|row| row.tonnes_t).sum();
                    let reclaim: f64 = rows.iter().filter(|row| row.activity == Activity::Reclaim).map(|row| row.tonnes_t).sum();
                    excess = excess.max(dig - loader.rates[interval.index].dig_tph * duration);
                    excess = excess.max(reclaim - loader.rates[interval.index].reclaim_tph * duration);
                }
            }
        }
        for truck in &input.trucks {
            for interval in &input.intervals {
                let units = truck.hours[interval.index] / interval.duration_h();
                for segment in 0..segments_of(interval.index) {
                    let duration = segment_durations_h[interval.index].get(segment).copied().unwrap_or(0.0);
                    let used: f64 = movements
                        .iter()
                        .filter(|movement| movement.interval == interval.index && movement.segment == segment && movement.truck == truck.id)
                        .map(|movement| movement.truck_hours)
                        .sum();
                    excess = excess.max(used - units * duration);
                }
            }
        }
        for destination in &input.destinations {
            match destination.kind {
                DestinationKind::Crusher => {
                    for (day, limit) in destination.crusher_daily_t.iter().enumerate() {
                        if let Some(limit) = limit {
                            let used: f64 = movements
                                .iter()
                                .filter(|movement| movement.destination == destination.id && input.intervals[movement.interval].day() as usize == day)
                                .map(|movement| movement.tonnes_t)
                                .sum();
                            excess = excess.max(used - limit);
                        }
                    }
                }
                DestinationKind::Dump => {
                    if let Some(capacity) = destination.capacity_t {
                        let used: f64 = movements
                            .iter()
                            .filter(|movement| movement.destination == destination.id)
                            .map(|movement| movement.tonnes_t)
                            .sum();
                        excess = excess.max(used - capacity);
                    }
                }
                DestinationKind::Stockpile(_) => {}
            }
        }
        for pile in &input.stockpiles {
            let pile_destination = input
                .destinations
                .iter()
                .find(|destination| matches!(destination.kind, DestinationKind::Stockpile(id) if id == pile.id));
            let mut inventory: f64 = pile.opening.iter().map(|lot| lot.tonnes_t).sum();
            excess = excess.max(inventory - pile.capacity_t);
            for interval in &input.intervals {
                for segment in 0..segments_of(interval.index) {
                    if let Some(destination) = pile_destination {
                        inventory += movements
                            .iter()
                            .filter(|movement| movement.interval == interval.index && movement.segment == segment && movement.destination == destination.id)
                            .map(|movement| movement.tonnes_t)
                            .sum::<f64>();
                    }
                    inventory -= reclaims
                        .iter()
                        .filter(|row| row.interval == interval.index && row.segment == segment && row.stockpile == pile.id)
                        .map(|row| row.tonnes_t)
                        .sum::<f64>();
                    excess = excess.max(inventory - pile.capacity_t);
                }
            }
        }
        for task in &input.tasks {
            if let TaskKind::Reclaim { maximum_t: Some(maximum), .. } = &task.kind {
                let reclaimed: f64 = reclaims.iter().filter(|row| row.task == task.id).map(|row| row.tonnes_t).sum();
                excess = excess.max(reclaimed - maximum);
            }
        }
        audit.max_capacity_excess_t = excess.max(0.0);
        audit.objective_adjustment = raw_solver_objective - primary_objective;
    }
    OptimisationResult {
        publication_audit: audit,
        summary: SolveSummary {
            status: mapped,
            objective: Some(primary_objective),
            raw_solver_objective: Some(raw_solver_objective),
            best_bound: bound,
            relative_gap: gap.is_finite().then_some(gap),
            backend: "HiGHS 1.11.0 via highs 2.4.0 / good_lp 1.15.3",
            message: None,
            tie_preferences_applied,
            statistics: SolveStatistics {
                variables: formulation.columns.count,
                binaries: formulation.columns.binaries,
                constraints: formulation.constraints,
                formulation_time,
                solve_time,
            },
        },
        resolution: metadata(input, &budgets),
        segment_durations_h,
        used_segments,
        event_budget_restricted,
        activities,
        movements,
        parcels,
        reclaims,
        balances,
    }
}

/// Budgets for the early-return paths, where the full derivation inputs are
/// already available from `input` alone.
fn segment_budgets_static(input: &OptimisationInput) -> Vec<usize> {
    let ground_index: BTreeMap<_, _> = input.ground.iter().map(|entry| (entry.id, entry)).collect();
    segment_budgets(input, &ground_index)
}
