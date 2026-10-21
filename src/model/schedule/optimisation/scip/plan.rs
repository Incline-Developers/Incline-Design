//! A coarse whole-horizon plan for the hourly dispatch to follow.
//!
//! Each loader digs its authored sequence in order, so where it stands is one
//! number: how far along that sequence it has dug. Per loader, block and day
//! the plan decides the tonnes dug, with a 0/1 per block and day saying it is
//! finished, which the next block of a sequence waits on. Each material of a
//! block with more than one outlet is split between its destinations per
//! day. Material, grade, value and truck hours are then all linear in the
//! tonnes, so the plan sees daily crusher budgets, dump and pile room, fleet
//! hours and daily grade targets across the whole horizon.
//!
//! Reclaim is planned the same way: per reclaim bar, pile and day the tonnes
//! drawn, within the loader's reclaim rate on the days the bar and the pile
//! allow it and the bar's authored cap, out of a daily pile inventory that
//! the plan's own deliveries fill. A loader's dig and reclaim share its
//! hours. A blend is a ratio of two decisions, so a reclaim's grade is
//! fixed: in a window, the blend the schedule it improves leaves the pile
//! with as the window opens; over the whole horizon, for a bound, the one
//! that suits each grade target best and every grade-conditional value
//! earned when it pays and not when it costs.
//!
//! A loader's bars are read apart, each in its own order, and held to
//! the authored priority as the dispatch keeps it: in a period, a loader
//! works a lower bar only if every higher one ran out of released ground
//! in it. Off a bound, on the timeline the search holds, a blast's ground is
//! dug only if the ground above it is gone by the end of the period the
//! timeline clears it in.
//!
//! What it does not see: anything inside a day, a bar blocked by room at a
//! destination, truck classes (a route uses
//! its best candidate's truck hours against the whole fleet), per-loader
//! values (a route takes its best candidate's value), chunks, rests, and a
//! reclaimed blend's route qualifications. Each only constrains a schedule
//! more, so the bound stands. Its answer is only targets; see
//! [`super::super::blended::plan`].
//!
//! Solved by the `solver` crate's port of HiGHS's MIP solver; HiGHS found plans and
//! bounds on these models where SCIP did not.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use web_time::Instant;

use super::{
    super::blended::{
        input::{BlendInput, attribute_reclaim, authored_tasks, grade_ceilings, interval_rate, task_active},
        replay::{BlendSolution, ReplayReport},
        rolling::FINISHED_T,
    },
    blasts::BlastModel,
};
use crate::model::schedule::{
    BlastActivity, BlastStage,
    optimisation::{Activity, DestinationId, DestinationKind, GroundId, Interval, MaterialId, SourceId, StockpileId, TaskKind},
};

/// Where a plan starts: the first day it plans, how many days, and the
/// state the schedule it improves left as that day opens.
#[derive(Clone, Debug, Default)]
pub(crate) struct Window {
    pub(crate) first_day: u32,
    pub(crate) days: u32,
    /// Tonnes left in each block; a block not here is gone.
    pub(crate) remaining: BTreeMap<GroundId, f64>,
    /// What each destination had received before the first day.
    pub(crate) received: BTreeMap<DestinationId, f64>,
    /// Each pile's tonnes and contained quantity per grade as the first day
    /// opens.
    pub(crate) piles: BTreeMap<StockpileId, (f64, Vec<f64>)>,
    /// What each reclaim bar (by task index) had drawn before the first day.
    pub(crate) reclaimed: BTreeMap<usize, f64>,
}

impl Window {
    /// The state `solution` leaves as `first_day` opens; its piles from
    /// `replay`, the replay's report on it, when there is one.
    pub(crate) fn opening(input: &BlendInput, solution: &BlendSolution, replay: Option<&ReplayReport>, first_day: u32, days: u32) -> Self {
        let mut remaining: BTreeMap<GroundId, f64> = input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect();
        let mut received: BTreeMap<DestinationId, f64> = BTreeMap::new();
        let mut reclaimed: BTreeMap<usize, f64> = BTreeMap::new();
        let mut rows: Vec<_> = solution
            .movements
            .iter()
            .filter(|row| input.intervals.get(row.interval).is_some_and(|interval| interval.day() < first_day))
            .collect();
        rows.sort_by_key(|row| row.interval);
        for row in rows {
            let candidate = &input.movements[row.candidate];
            if let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source)
                && let Some(left) = remaining.get_mut(&ground)
            {
                *left -= row.tonnes_t;
            }
            if candidate.activity == Activity::Reclaim {
                for (task, share) in attribute_reclaim(input, candidate, input.intervals[row.interval], row.tonnes_t, &reclaimed) {
                    *reclaimed.entry(task).or_default() += share;
                }
            }
            *received.entry(candidate.destination).or_default() += row.tonnes_t;
        }
        // Dust the replay counts as dug is gone: left in, a block with
        // nowhere to be dug in the window would hold back the next.
        remaining.retain(|_, left| *left > FINISHED_T);
        let grades = input.grades.count();
        let first = input.intervals.iter().position(|interval| interval.day() >= first_day).unwrap_or(0);
        let piles = input
            .piles
            .iter()
            .map(|pile| {
                let state = replay
                    .filter(|_| first > 0)
                    .and_then(|replay| replay.pile_intervals.get(&(pile.id, first - 1)))
                    .map_or_else(|| pile.total_opening(grades), |state| (state.closing_t, state.closing_q.clone()));
                (pile.id, state)
            })
            .collect();
        Self {
            first_day,
            days,
            remaining,
            received,
            piles,
            reclaimed,
        }
    }
}

/// How to solve a plan.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Settings {
    pub(crate) time_limit: Duration,
    pub(crate) relative_gap: f64,
    /// Solve the linear relaxation only, by HiGHS's interior point method:
    /// with no window, an upper bound on any schedule's value.
    pub(crate) relax: bool,
}

/// The solved plan.
pub(crate) struct PlanSolve {
    /// Planned dug tonnes per (loader index, destination, day).
    pub(crate) routes: BTreeMap<(usize, DestinationId, u32), f64>,
    pub(crate) objective: Option<f64>,
    pub(crate) bound: Option<f64>,
    pub(crate) variables: usize,
    pub(crate) binaries: usize,
    pub(crate) rows: usize,
    pub(crate) build_s: f64,
    pub(crate) solve_s: f64,
    /// What the plan valued its seed at, when it had one.
    pub(crate) seeded: Option<f64>,
    /// How many rows and bounds the seed breaks, when it had one.
    pub(crate) seed_violations: Option<usize>,
    /// Planned dug tonnes per (loader index, block) over the plan.
    pub(crate) dug: BTreeMap<(usize, GroundId), f64>,
    /// What the plan's movements earn on each day it plans.
    pub(crate) day_values: BTreeMap<u32, f64>,
    pub(crate) status: String,
}

/// The plan's columns and rows, kept row-wise.
#[derive(Default)]
struct Builder {
    names: Vec<String>,
    cost: Vec<f64>,
    upper: Vec<f64>,
    integer: Vec<bool>,
    row_lower: Vec<f64>,
    row_upper: Vec<f64>,
    starts: Vec<usize>,
    index: Vec<usize>,
    value: Vec<f64>,
    binaries: usize,
}

impl Builder {
    fn cont(&mut self, upper: f64, value: f64, name: &str) -> usize {
        self.names.push(name.to_owned());
        self.cost.push(value);
        self.upper.push(upper);
        self.integer.push(false);
        self.cost.len() - 1
    }

    fn binary(&mut self, name: &str) -> usize {
        self.binaries += 1;
        let column = self.cont(1.0, 0.0, name);
        self.integer[column] = true;
        column
    }

    /// One row, its repeated columns merged.
    fn row(&mut self, terms: &[(usize, f64)], lhs: f64, rhs: f64, _name: &str) {
        if terms.is_empty() {
            return;
        }
        let mut merged: Vec<(usize, f64)> = terms.iter().copied().filter(|(_, coefficient)| *coefficient != 0.0).collect();
        merged.sort_unstable_by_key(|(column, _)| *column);
        self.starts.push(self.index.len());
        let first = self.index.len();
        for (column, coefficient) in merged {
            if self.index.len() > first && self.index.last() == Some(&column) {
                *self.value.last_mut().expect("an entry was pushed") += coefficient;
            } else {
                self.index.push(column);
                self.value.push(coefficient);
            }
        }
        self.row_lower.push(lhs);
        self.row_upper.push(rhs);
    }

    /// How many rows and column bounds `values` breaks, by the solver's
    /// tolerance.
    fn violations(&self, values: &[f64]) -> usize {
        const TOLERANCE: f64 = solver::MIP_FEASIBILITY_TOLERANCE;
        let rows = (0..self.row_lower.len())
            .filter(|&row| {
                let end = self.starts.get(row + 1).copied().unwrap_or(self.index.len());
                let activity: f64 = (self.starts[row]..end).map(|entry| self.value[entry] * values[self.index[entry]]).sum();
                activity < self.row_lower[row] - TOLERANCE || activity > self.row_upper[row] + TOLERANCE
            })
            .count();
        rows + values
            .iter()
            .zip(&self.upper)
            .filter(|(value, upper)| **value < -TOLERANCE || **value > **upper + TOLERANCE)
            .count()
    }
}

/// A solve's column values, objective, bound and status.
struct Answer {
    values: Vec<f64>,
    objective: f64,
    bound: Option<f64>,
    status: String,
}

/// The plan's linear relaxation, by the `solver` crate's interior point method: a bound,
/// and no seed to start from.
fn solve_relaxation(builder: &Builder, time_limit: Duration, stop: &Arc<AtomicBool>) -> Result<Answer, String> {
    let problem = plan_problem(builder, true);
    let options = solver::Options {
        solver: solver::Solver::Ipm,
        time_limit: time_limit.as_secs_f64(),
        ..solver::Options::default()
    };
    let stop = Arc::clone(stop);
    let solution = solver::solve_from(&problem, &options, None, Some(Arc::new(move || stop.load(Ordering::Relaxed))));
    if !solution.objective.is_finite() {
        return Err(format!("no plan found: solver status {:?}", solution.status));
    }
    // A relaxation interrupted before its optimum bounds nothing.
    Ok(Answer {
        bound: (solution.status == solver::Status::Optimal).then_some(solution.objective),
        objective: solution.objective,
        status: format!("solver status {:?}", solution.status),
        values: solution.col_value,
    })
}

/// The plan's linear relaxation or rows as `builder` has them in one
/// [`solver::Problem`], integer columns marked unless `relax`.
fn plan_problem(builder: &Builder, relax: bool) -> solver::Problem {
    let mut problem = solver::Problem::new(solver::Sense::Maximise);
    for (&cost, &upper) in builder.cost.iter().zip(&builder.upper) {
        problem.add_col(cost, 0.0, if upper.is_finite() { upper } else { f64::INFINITY });
    }
    for row in 0..builder.row_lower.len() {
        let range = builder.starts[row]..builder.starts.get(row + 1).copied().unwrap_or(builder.index.len());
        let entries = builder.index[range.clone()].iter().copied().zip(builder.value[range].iter().copied());
        problem.add_row(builder.row_lower[row], builder.row_upper[row], entries);
    }
    if !relax {
        problem.integrality = builder.integer.clone();
    }
    problem
}

fn solve_plan(builder: &Builder, seed: Option<&[f64]>, time_limit: Duration, relative_gap: f64, relax: bool, stop: &Arc<AtomicBool>) -> Result<Answer, String> {
    if relax {
        return solve_relaxation(builder, time_limit, stop);
    }
    let problem = plan_problem(builder, false);
    // With `INCLINE_LP_DUMP` set to a directory, write each plan there, with
    // its seed beside it (a value per line).
    if let Some(dir) = std::env::var_os("INCLINE_LP_DUMP") {
        static DUMPED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let name = format!("plan-{}-{:03}", std::process::id(), DUMPED.fetch_add(1, Ordering::Relaxed));
        let path = std::path::Path::new(&dir).join(format!("{name}.mps"));
        if let Err(error) = std::fs::write(&path, solver::mps::write(&problem)) {
            log::warn!("could not write {}: {error}", path.display());
        }
        if let Some(seed) = seed {
            let path = std::path::Path::new(&dir).join(format!("{name}.seed"));
            if let Err(error) = std::fs::write(&path, seed.iter().map(|value| format!("{value:e}\n")).collect::<String>()) {
                log::warn!("could not write {}: {error}", path.display());
            }
        }
    }
    let settings = solver::MipSettings {
        time_limit: time_limit.as_secs_f64(),
        mip_rel_gap: relative_gap,
        ..solver::MipSettings::default()
    };
    let stop = Arc::clone(stop);
    let solution = solver::solve_mip(&problem, &settings, seed, Some(Arc::new(move || stop.load(Ordering::Relaxed))));
    if !solution.objective.is_finite() {
        return Err(format!("no plan found: solver status {:?}", solution.status));
    }
    // A mixed-integer solve stopped inside its first relaxation bounds
    // nothing useful: its dual bound is then only what each column's own
    // bounds allow, valid but many times any schedule's value.
    let bound = (solution.dual_bound.is_finite() && (solution.status == solver::MipStatus::Optimal || solution.nodes > 0)).then_some(solution.dual_bound);
    Ok(Answer {
        status: format!("solver status {:?}, {} nodes", solution.status, solution.nodes),
        objective: solution.objective,
        bound,
        values: solution.col_value,
    })
}

/// One destination a block's material can go to, on its best candidate.
#[derive(Clone, Copy)]
pub(super) struct Outlet {
    pub(super) destination: DestinationId,
    pub(super) value: f64,
    pub(super) truck_h: f64,
}

/// Whose grade a flow carries.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Carried {
    /// A dug material's own.
    Material(MaterialId),
    /// A reclaim's: the pile's blend, fixed (see the module docs).
    Pile(StockpileId),
}

/// A day's tonnes going somewhere: the column, its tonnes per unit, and
/// whose grade they carry.
type Flow = (usize, f64, Carried);

/// Tonnes per unit of a plan column: kilotonnes keep its coefficients within
/// a few orders of magnitude of each other, which both solvers need.
const UNIT: f64 = 1000.0;

/// Tonnes below which a block is dust to a plan.
const DUST_T: f64 = 1.0;

/// A column carrying a block material's tonnes to a destination on a day:
/// (block, material, destination, period), the column, tonnes per unit, and
/// the loader when it is one loader's own dig.
type Routed = ((GroundId, MaterialId, DestinationId, usize), usize, f64, Option<usize>);

/// Everything a plan of `input` is built from, worked out once: the days it
/// covers, what each block holds and where its materials can go, each
/// loader's sequence and dig capacity, each reclaim bar's piles, routes and
/// capacity, and the piles as the plan opens.
pub(super) struct Prepared<'a> {
    pub(super) input: &'a BlendInput,
    pub(super) window: Option<&'a Window>,
    /// Each period's label: its day, or for a plan over periods of several
    /// days the first of them.
    pub(super) days: Vec<u32>,
    pub(super) day_intervals: BTreeMap<u32, Vec<Interval>>,
    /// The calendar days each period covers.
    pub(super) period_days: Vec<Vec<u32>>,
    /// Kilotonnes left in each block the plan may dig.
    pub(super) tonnes: BTreeMap<GroundId, f64>,
    /// Whether this plans the whole horizon from scratch, and so must bound
    /// every schedule.
    pub(super) optimistic: bool,
    pub(super) ceilings: Vec<f64>,
    pub(super) pile_opening: BTreeMap<StockpileId, (f64, Vec<f64>)>,
    pub(super) blend: BTreeMap<StockpileId, Vec<f64>>,
    pub(super) outlets: BTreeMap<(GroundId, MaterialId), Vec<Outlet>>,
    /// Each loader's blocks in authored order, with their bars.
    pub(super) chains: Vec<Vec<(GroundId, usize)>>,
    /// (loader, block) for blocks in more than one of the loader's dig bars,
    /// which may then be worked in either bar's order: no order row holds
    /// them.
    pub(super) repeated: BTreeSet<(usize, GroundId)>,
    /// Kilotonnes each loader can dig each day.
    pub(super) capacity: Vec<Vec<f64>>,
    pub(super) allowed: BTreeMap<(usize, GroundId), Vec<usize>>,
    pub(super) reclaims: Vec<ReclaimSource>,
    /// The drill and blast chain, when the plan bounds and there is one.
    pub(super) blasts: Option<BlastModel>,
}

/// One reclaim bar's draw from one pile.
pub(super) struct ReclaimSource {
    pub(super) task: usize,
    pub(super) loader: usize,
    pub(super) pile: StockpileId,
    pub(super) routes: Vec<Outlet>,
    /// Kilotonnes it can draw each day.
    pub(super) caps: Vec<f64>,
}

/// The materials of a block and their shares, those it holds any of.
fn ground_materials(input: &BlendInput, ground: GroundId) -> Vec<(MaterialId, f64)> {
    input
        .ground
        .iter()
        .find(|source| source.id == ground)
        .map(|source| {
            source
                .material
                .iter()
                .filter(|share| share.fraction > 0.0)
                .map(|share| (share.material, share.fraction))
                .collect()
        })
        .unwrap_or_default()
}

impl<'a> Prepared<'a> {
    /// The plan's preparation, over periods of `period_days` days each:
    /// one, but for the whole-horizon relaxation's bound, which may join days
    /// (see [`relaxation_bound`]).
    pub(super) fn new(input: &'a BlendInput, window: Option<&'a Window>, period_days: u32, bounding: bool) -> Self {
        // ---- days ---------------------------------------------------------------
        let period_days = period_days.max(1);
        let mut day_intervals: BTreeMap<u32, Vec<Interval>> = BTreeMap::new();
        let mut covered: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for interval in &input.intervals {
            if window.is_none_or(|window| (window.first_day..window.first_day + window.days).contains(&interval.day())) {
                let label = interval.day() / period_days * period_days;
                day_intervals.entry(label).or_default().push(*interval);
                covered.entry(label).or_default().insert(interval.day());
            }
        }
        let days: Vec<u32> = day_intervals.keys().copied().collect();
        let period_days: Vec<Vec<u32>> = covered.into_values().map(|days| days.into_iter().collect()).collect();
        let day_end: Vec<f64> = days
            .iter()
            .map(|day| day_intervals[day].iter().map(|interval| interval.end_h).fold(f64::MIN, f64::max))
            .collect();
        let periods = days.len();

        // A block of dust is left out: the dispatch digs it at once, and in
        // the plan its tonnes bound columns and scale rows a million times
        // below the rest, which left a long-term relaxation's basis singular.
        let tonnes: BTreeMap<GroundId, f64> = match window {
            Some(window) => window.remaining.iter().map(|(ground, left)| (*ground, left / UNIT)).collect(),
            None => input.ground.iter().map(|source| (source.id, source.tonnes_t / UNIT)).collect(),
        };
        let tonnes: BTreeMap<GroundId, f64> = tonnes.into_iter().filter(|(_, kilotonnes)| window.is_some() || *kilotonnes * UNIT >= DUST_T).collect();
        // Over the whole horizon a reclaim's grade is the one that suits each
        // use of it best. A bound's relaxation also reads every route, order
        // and reach as the most any schedule could have, and each blast at its
        // earliest release, as Improve may move blasts. Any other plan only
        // sets targets, so it keeps the tighter reading the dispatch mostly
        // follows: one candidate's value and haul, and the timeline the search
        // holds.
        let optimistic = window.is_none();
        let bounding = optimistic && bounding;
        let grades = input.grades.count();
        let ceilings = grade_ceilings(input);
        let pile_opening: BTreeMap<StockpileId, (f64, Vec<f64>)> = match window {
            Some(window) => window.piles.clone(),
            None => input.piles.iter().map(|pile| (pile.id, pile.total_opening(grades))).collect(),
        };
        let blend: BTreeMap<StockpileId, Vec<f64>> = pile_opening
            .iter()
            .map(|(pile, (tonnes, contained))| {
                (
                    *pile,
                    (0..grades)
                        .map(|grade| if *tonnes > 1e-9 { contained.get(grade).copied().unwrap_or(0.0) / tonnes } else { 0.0 })
                        .collect(),
                )
            })
            .collect();
        // Improve may move blasts off the timeline it starts from, so a plan
        // that bounds every schedule allows each blast its earliest release;
        // a window's plan keeps the timeline the search holds.
        let releases = input
            .drill_blast
            .as_ref()
            .map(|chain| if bounding { chain.earliest_releases(&input.intervals) } else { chain.releases() })
            .unwrap_or_default();

        // ---- outlets: each block material's destinations, at their best
        let mut outlets: BTreeMap<(GroundId, MaterialId), Vec<Outlet>> = BTreeMap::new();
        for candidate in &input.movements {
            let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source) else {
                continue;
            };
            let value = candidate.value_per_tonne().unwrap_or(0.0) * UNIT;
            let truck_h = candidate.truck_hours_per_tonne * UNIT;
            let list = outlets.entry((ground, candidate.material)).or_default();
            // To bound, the best value and the least truck time of every
            // candidate to the destination, each on its own: one candidate's
            // pair would hide a cheaper haul worth less a tonne but more in
            // all. Otherwise the best candidate's.
            match list.iter_mut().find(|outlet| outlet.destination == candidate.destination) {
                Some(outlet) if bounding => {
                    outlet.value = outlet.value.max(value);
                    outlet.truck_h = outlet.truck_h.min(truck_h);
                }
                Some(outlet) if value > outlet.value || (value == outlet.value && truck_h < outlet.truck_h) => {
                    outlet.value = value;
                    outlet.truck_h = truck_h;
                }
                Some(_) => {}
                None => list.push(Outlet {
                    destination: candidate.destination,
                    value,
                    truck_h,
                }),
            }
        }
        let materials = |ground: GroundId| ground_materials(input, ground);
        // A block is dug whole, so it is diggable only when every material has
        // somewhere to go.
        let diggable = |ground: GroundId| {
            materials(ground)
                .iter()
                .all(|(material, _)| outlets.get(&(ground, *material)).is_some_and(|list| !list.is_empty()))
        };

        // ---- each loader's chain and the days each of its blocks may be dug -----
        // chain[l] = (block, task) in authored order, each bar cut at the first
        // block the plan cannot dig, as the dispatch moves on to the next.
        let mut chains: Vec<Vec<(GroundId, usize)>> = Vec::new();
        let mut capacity: Vec<Vec<f64>> = Vec::new();
        for (loader_index, loader) in input.loaders.iter().enumerate() {
            let mut chain = Vec::new();
            let mut seen = BTreeSet::new();
            for task_index in authored_tasks(input, loader_index) {
                let TaskKind::Dig { sequence } = &input.tasks[task_index].kind else { continue };
                for ground in sequence {
                    if !tonnes.contains_key(ground) {
                        continue;
                    }
                    if !diggable(*ground) {
                        break;
                    }
                    if seen.insert(*ground) {
                        chain.push((*ground, task_index));
                    }
                }
            }
            let caps = days
                .iter()
                .map(|day| {
                    day_intervals[day]
                        .iter()
                        .filter(|interval| {
                            input
                                .tasks
                                .iter()
                                .any(|task| task.loader == loader.id && matches!(task.kind, TaskKind::Dig { .. }) && task_active(task, **interval))
                        })
                        .filter_map(|interval| interval_rate(loader, interval.index).map(|rate| rate.dig_tph.max(0.0) * interval.duration_h() / UNIT))
                        .sum::<f64>()
                })
                .collect();
            chains.push(chain);
            capacity.push(caps);
        }
        // The loaders whose chains hold each block, and each loader's bars
        // holding it: a block another loader can dig need not be dug by this
        // one first, and one in two bars is worked under either.
        let mut diggers: BTreeMap<GroundId, BTreeSet<usize>> = BTreeMap::new();
        for (loader_index, chain) in chains.iter().enumerate() {
            for &(ground, _) in chain {
                diggers.entry(ground).or_default().insert(loader_index);
            }
        }
        // A plan that bounds every schedule also says when each blast could
        // clear and fire, and holds its ground back until then.
        let blasts = input.drill_blast.as_ref().filter(|_| bounding).map(|chain| {
            let periods: Vec<&[Interval]> = days.iter().map(|day| day_intervals[day].as_slice()).collect();
            let dug: BTreeSet<GroundId> = diggers.keys().copied().collect();
            BlastModel::new(chain, &periods, &tonnes, &dug)
        });
        let mut holding: BTreeMap<(usize, GroundId), Vec<usize>> = BTreeMap::new();
        for loader_index in 0..input.loaders.len() {
            for task in authored_tasks(input, loader_index) {
                let TaskKind::Dig { sequence } = &input.tasks[task].kind else { continue };
                for &ground in sequence {
                    let bars = holding.entry((loader_index, ground)).or_default();
                    if bars.last() != Some(&task) {
                        bars.push(task);
                    }
                }
            }
        }
        let bars_holding = |loader_index: usize, ground: GroundId| -> Vec<usize> { holding.get(&(loader_index, ground)).cloned().unwrap_or_default() };
        // Days each (loader, block) may be dug: one of its bars active and
        // the loader able to dig, not before the loader could have reached it
        // at full rate, and not before its blast is released. Reach counts
        // only the blocks ahead of it in the same bar that it alone digs, and a
        // block in two bars may be dug in either's window.
        let mut allowed: BTreeMap<(usize, GroundId), Vec<usize>> = BTreeMap::new();
        let mut repeated: BTreeSet<(usize, GroundId)> = BTreeSet::new();
        for (loader_index, chain) in chains.iter().enumerate() {
            let mut prefix = 0.0;
            let mut bar = None;
            for &(ground, task_index) in chain {
                let bars = bars_holding(loader_index, ground);
                if bar != Some(task_index) {
                    (prefix, bar) = (0.0, Some(task_index));
                }
                if bars.len() > 1 {
                    repeated.insert((loader_index, ground));
                    prefix = 0.0;
                }
                let mut reached = 0.0;
                let mut list = Vec::new();
                for p in 0..periods {
                    reached += capacity[loader_index][p];
                    if reached <= prefix + 1e-6 || capacity[loader_index][p] <= 0.0 {
                        continue;
                    }
                    if !day_intervals[&days[p]]
                        .iter()
                        .any(|interval| bars.iter().any(|&task| task_active(&input.tasks[task], *interval)))
                    {
                        continue;
                    }
                    if releases.get(&ground).is_some_and(|released| *released >= day_end[p] - 1e-9) {
                        continue;
                    }
                    list.push(p);
                }
                if diggers.get(&ground).is_some_and(|loaders| loaders.len() == 1) {
                    prefix += tonnes[&ground];
                }
                allowed.insert((loader_index, ground), list);
            }
        }

        // Each reclaim bar's piles: their destinations at their best,
        // with the conditional values the blend earns, and what the bar can draw
        // from each a day.
        let mut reclaims = Vec::new();
        for (task_index, task) in input.tasks.iter().enumerate() {
            let TaskKind::Reclaim { approved_sources, .. } = &task.kind else { continue };
            let Some(loader_index) = input.loaders.iter().position(|loader| loader.id == task.loader) else {
                continue;
            };
            let loader = &input.loaders[loader_index];
            for pile in approved_sources {
                let Some(entry) = input.piles.iter().find(|found| found.id == *pile) else { continue };
                let mut routes: Vec<Outlet> = Vec::new();
                for (index, candidate) in input.movements.iter().enumerate() {
                    if candidate.loader != task.loader || candidate.activity != Activity::Reclaim || candidate.source != SourceId::Stockpile(*pile) {
                        continue;
                    }
                    let conditional: f64 = input
                        .conditional_values
                        .iter()
                        .filter(|value| value.candidate == index)
                        .map(|value| match optimistic {
                            true => value.value_per_tonne.max(0.0),
                            false if blend.get(pile).is_some_and(|blend| value.holds(blend)) => value.value_per_tonne,
                            false => 0.0,
                        })
                        .sum();
                    let value = (candidate.value_per_tonne().unwrap_or(0.0) + conditional) * UNIT;
                    let truck_h = candidate.truck_hours_per_tonne * UNIT;
                    // As a dig's outlets: to bound, best value and least truck
                    // time apart.
                    match routes.iter_mut().find(|route| route.destination == candidate.destination) {
                        Some(route) if bounding => {
                            route.value = route.value.max(value);
                            route.truck_h = route.truck_h.min(truck_h);
                        }
                        Some(route) if value > route.value || (value == route.value && truck_h < route.truck_h) => {
                            route.value = value;
                            route.truck_h = truck_h;
                        }
                        Some(_) => {}
                        None => routes.push(Outlet {
                            destination: candidate.destination,
                            value,
                            truck_h,
                        }),
                    }
                }
                if routes.is_empty() {
                    continue;
                }
                let caps = (0..periods)
                    .map(|p| {
                        day_intervals[&days[p]]
                            .iter()
                            .filter(|interval| task_active(task, **interval) && entry.reclaims(**interval))
                            .filter_map(|interval| interval_rate(loader, interval.index).map(|rate| rate.reclaim_tph.max(0.0) * interval.duration_h() / UNIT))
                            .sum()
                    })
                    .collect();
                reclaims.push(ReclaimSource {
                    task: task_index,
                    loader: loader_index,
                    pile: *pile,
                    routes,
                    caps,
                });
            }
        }
        Self {
            input,
            window,
            days,
            day_intervals,
            period_days,
            tonnes,
            optimistic,
            ceilings,
            pile_opening,
            blend,
            outlets,
            chains,
            repeated,
            capacity,
            allowed,
            reclaims,
            blasts,
        }
    }

    pub(super) fn periods(&self) -> usize {
        self.days.len()
    }

    /// What a destination had received before the plan's first day.
    pub(super) fn before(&self, id: DestinationId) -> f64 {
        self.window.and_then(|window| window.received.get(&id)).copied().unwrap_or(0.0)
    }

    /// What a reclaim bar had drawn before the plan's first day.
    pub(super) fn reclaimed_before(&self, task: usize) -> f64 {
        self.window.and_then(|window| window.reclaimed.get(&task)).copied().unwrap_or(0.0)
    }

    pub(super) fn materials(&self, ground: GroundId) -> Vec<(MaterialId, f64)> {
        ground_materials(self.input, ground)
    }

    /// The grade a flow carries, for a row of `direction`: a reclaim's, over
    /// the whole horizon, the one that adds least to it.
    pub(super) fn carried(&self, of: Carried, grade: usize, direction: f64) -> f64 {
        match of {
            Carried::Material(material) => self.input.grades.fraction(material, grade).unwrap_or(0.0),
            Carried::Pile(_) if self.optimistic => {
                if direction > 0.0 {
                    0.0
                } else {
                    self.ceilings.get(grade).copied().unwrap_or(0.0)
                }
            }
            Carried::Pile(pile) => self.blend.get(&pile).and_then(|blend| blend.get(grade)).copied().unwrap_or(0.0),
        }
    }

    pub(super) fn destination(&self, id: DestinationId) -> Option<&'a crate::model::schedule::optimisation::Destination> {
        self.input.destinations.iter().find(|entry| entry.id == id)
    }

    /// Whether a destination takes deliveries on day `p`: a pile only on a
    /// day it builds.
    pub(super) fn building(&self, id: DestinationId, p: usize) -> bool {
        match self.destination(id).map(|entry| entry.kind) {
            Some(DestinationKind::Stockpile(pile)) => self
                .input
                .piles
                .iter()
                .find(|entry| entry.id == pile)
                .is_some_and(|entry| self.day_intervals[&self.days[p]].iter().any(|interval| entry.builds(*interval))),
            _ => true,
        }
    }

    /// A loader's fastest dig and reclaim rates on day `p`, and the hours it
    /// has to work then: the hours of the day's intervals any of its bars
    /// covers that it has a rate in.
    pub(super) fn loader_hours(&self, loader_index: usize, p: usize) -> (f64, f64, f64) {
        let loader = &self.input.loaders[loader_index];
        let intervals = &self.day_intervals[&self.days[p]];
        let (dig_tph, reclaim_tph) = intervals
            .iter()
            .filter_map(|interval| interval_rate(loader, interval.index))
            .fold((0.0_f64, 0.0_f64), |(dig, reclaim), rate| (dig.max(rate.dig_tph), reclaim.max(rate.reclaim_tph)));
        let hours = intervals
            .iter()
            .filter(|interval| {
                interval_rate(loader, interval.index).is_some_and(|rate| rate.dig_tph > 0.0 || rate.reclaim_tph > 0.0)
                    && self.input.tasks.iter().any(|task| task.loader == loader.id && task_active(task, **interval))
            })
            .map(|interval| interval.duration_h())
            .sum();
        (dig_tph, reclaim_tph, hours)
    }

    /// The fleet's hours on day `p`, every class together.
    pub(super) fn fleet_hours(&self, p: usize) -> f64 {
        self.day_intervals[&self.days[p]]
            .iter()
            .map(|interval| self.input.trucks.iter().filter_map(|truck| truck.hours.get(interval.index)).sum::<f64>())
            .sum()
    }
}

/// A flag of the blast rows: a column, or settled as the plan opens.
#[derive(Clone, Copy)]
enum Flag {
    Column(usize),
    Fixed(f64),
}

impl Builder {
    /// A row over flags: those settled go into its bounds, and one with
    /// nothing left to decide is left out.
    fn flag_row(&mut self, terms: &[(Flag, f64)], lhs: f64, rhs: f64, name: &str) {
        let settled: f64 = terms
            .iter()
            .map(|(flag, coefficient)| if let Flag::Fixed(value) = flag { value * coefficient } else { 0.0 })
            .sum();
        let columns: Vec<(usize, f64)> = terms
            .iter()
            .filter_map(|(flag, coefficient)| if let Flag::Column(column) = flag { Some((*column, *coefficient)) } else { None })
            .collect();
        self.row(&columns, lhs - settled, rhs - settled, name);
    }
}

/// The drill and blast chain's rows (see `docs/scheduling-blast-aware-plan-design.md`):
/// each machine's work within what it could do on each blast and in all,
/// each step done only once its work is and the step before it is, a blast
/// clear only once the ground above it is dug, firing only once charged
/// and in a period a firing window ends in, and the ground it releases dug
/// only once it has fired. The work is in fractions of each step's work
/// left. Flags are continuous: the rows only bound.
fn blast_rows(builder: &mut Builder, model: &BlastModel, block_so_far: &BTreeMap<(GroundId, usize), usize>, tonnes: &BTreeMap<GroundId, f64>, periods: usize) {
    let flag = |builder: &mut Builder, name: String| Flag::Column(builder.cont(1.0, 0.0, &name));
    // Clear flags first, for every blast, so a blast whose ground above
    // holds another's can lean on it: once the larger is clear, so is the
    // smaller, and the larger's row needs only the ground the smaller
    // lacks. The sets nest deeply on a bench (each blast holds those behind
    // it), and their full sums are dense, nearly parallel rows.
    let mut clears: Vec<Vec<Flag>> = Vec::with_capacity(model.blasts.len());
    for planned in &model.blasts {
        let b = planned.index;
        clears.push(if planned.opening.cleared {
            vec![Flag::Fixed(1.0); periods]
        } else if planned.never_clear {
            vec![Flag::Fixed(0.0); periods]
        } else {
            let flags: Vec<Flag> = (0..periods).map(|p| flag(builder, format!("bclr_{b}_{p}"))).collect();
            for (p, pair) in flags.windows(2).enumerate() {
                builder.flag_row(&[(pair[0], 1.0), (pair[1], -1.0)], f64::NEG_INFINITY, 0.0, &format!("bclrm_{b}_{}", p + 1));
            }
            flags
        });
    }
    let deciding: Vec<usize> = (0..model.blasts.len()).filter(|&i| matches!(clears[i][0], Flag::Column(_))).collect();
    let above: Vec<BTreeSet<GroundId>> = model.blasts.iter().map(|planned| planned.above.iter().copied().collect()).collect();
    for &i in &deciding {
        let b = model.blasts[i].index;
        // The largest set inside this one; of equal sets, the first.
        let inner = deciding
            .iter()
            .copied()
            .filter(|&j| j != i && above[j].is_subset(&above[i]) && (above[j].len() < above[i].len() || j < i))
            .max_by_key(|&j| (above[j].len(), std::cmp::Reverse(j)));
        let rest: Vec<GroundId> = match inner {
            Some(j) => above[i].difference(&above[j]).copied().collect(),
            None => above[i].iter().copied().collect(),
        };
        let standing: f64 = rest.iter().filter_map(|ground| tonnes.get(ground)).sum();
        for p in 0..periods {
            if let Some(j) = inner {
                builder.flag_row(&[(clears[i][p], 1.0), (clears[j][p], -1.0)], f64::NEG_INFINITY, 0.0, &format!("bnest_{b}_{p}"));
            }
            if standing > 1e-9 {
                let mut terms: Vec<(Flag, f64)> = rest
                    .iter()
                    .filter_map(|ground| block_so_far.get(&(*ground, p)).map(|column| (Flag::Column(*column), 1.0 / standing)))
                    .collect();
                terms.push((clears[i][p], -1.0));
                builder.flag_row(&terms, 0.0, f64::INFINITY, &format!("bclear_{b}_{p}"));
            }
        }
    }
    // Each machine's work on each blast per period, for its capacity rows.
    let mut work: BTreeMap<(usize, usize), Vec<(usize, f64)>> = BTreeMap::new();
    for (i, planned) in model.blasts.iter().enumerate() {
        let b = planned.index;
        let opening = &planned.opening;
        let monotone = |builder: &mut Builder, flags: &[Flag], name: &str| {
            for (p, pair) in flags.windows(2).enumerate() {
                builder.flag_row(&[(pair[0], 1.0), (pair[1], -1.0)], f64::NEG_INFINITY, 0.0, &format!("{name}_{}", p + 1));
            }
        };
        let clear = clears[i].clone();
        // Each step: its work so far, and done.
        let mut before = clear;
        let mut done_flags: Vec<Vec<Flag>> = Vec::new();
        for activity in BlastActivity::ALL {
            let step = activity as usize;
            let done: Vec<Flag> = if opening.done[step] {
                vec![Flag::Fixed(1.0); periods]
            } else {
                let flags: Vec<Flag> = (0..periods).map(|p| flag(builder, format!("bdone_{b}_{step}_{p}"))).collect();
                monotone(builder, &flags, &format!("bdonem_{b}_{step}"));
                flags
            };
            // Done only once the step before it is, whatever work is left.
            for p in 0..periods {
                builder.flag_row(&[(done[p], 1.0), (before[p], -1.0)], f64::NEG_INFINITY, 0.0, &format!("bafter_{b}_{step}_{p}"));
            }
            if !opening.done[step] && opening.left[step] > 0.0 {
                let mut so_far: Option<usize> = None;
                for p in 0..periods {
                    let column = builder.cont(1.0, 0.0, &format!("bw_{b}_{step}_{p}"));
                    let mut terms = vec![(column, 1.0)];
                    if let Some(previous) = so_far {
                        terms.push((previous, -1.0));
                    }
                    for (agent, available) in planned.machines.iter().filter(|(agent, _)| model.activity(*agent) == activity) {
                        if available[p] <= 0.0 {
                            continue;
                        }
                        let share = builder.cont((available[p] / opening.left[step]).min(1.0), 0.0, &format!("bm_{agent}_{b}_{p}"));
                        terms.push((share, -1.0));
                        work.entry((*agent, p)).or_default().push((share, opening.left[step]));
                    }
                    builder.row(&terms, 0.0, 0.0, &format!("bwsum_{b}_{step}_{p}"));
                    // Worked only once the step before it is done, and done
                    // only once all its work is.
                    builder.flag_row(&[(Flag::Column(column), 1.0), (before[p], -1.0)], f64::NEG_INFINITY, 0.0, &format!("bready_{b}_{step}_{p}"));
                    builder.flag_row(&[(Flag::Column(column), 1.0), (done[p], -1.0)], 0.0, f64::INFINITY, &format!("bfinish_{b}_{step}_{p}"));
                    so_far = Some(column);
                }
            }
            before = done.clone();
            done_flags.push(done);
        }
        // Fired: once charged, in a period a firing window ends in, or as
        // the plan opened scheduled.
        let fire: Vec<Flag> = match planned.fires_in {
            Some(from) => (0..periods).map(|p| Flag::Fixed(if p >= from { 1.0 } else { 0.0 })).collect(),
            None => {
                let flags: Vec<Flag> = (0..periods).map(|p| flag(builder, format!("bfire_{b}_{p}"))).collect();
                monotone(builder, &flags, &format!("bfirem_{b}"));
                for p in 0..periods {
                    builder.flag_row(&[(flags[p], 1.0), (done_flags[2][p], -1.0)], f64::NEG_INFINITY, 0.0, &format!("bcharged_{b}_{p}"));
                    if !model.fires[p] {
                        let previous = if p == 0 { Flag::Fixed(0.0) } else { flags[p - 1] };
                        builder.flag_row(&[(flags[p], 1.0), (previous, -1.0)], f64::NEG_INFINITY, 0.0, &format!("bwindow_{b}_{p}"));
                    }
                }
                flags
            }
        };
        // Its ground, all of it together, dug only once it has fired.
        for ground in &planned.releases {
            let Some(&left) = tonnes.get(ground) else { continue };
            for (p, fired) in fire.iter().enumerate() {
                if let Some(&dug) = block_so_far.get(&(*ground, p)) {
                    builder.flag_row(&[(Flag::Column(dug), 1.0), (*fired, -left)], f64::NEG_INFINITY, 0.0, &format!("brelease_{}_{p}", ground.0));
                }
            }
        }
    }
    // Each machine within what it could do in each period.
    for ((agent, p), terms) in work {
        let capacity = model.capacity[agent][p];
        if capacity > 0.0 {
            let scaled: Vec<(usize, f64)> = terms.into_iter().map(|(column, quantity)| (column, quantity / capacity)).collect();
            builder.row(&scaled, f64::NEG_INFINITY, 1.0, &format!("bcap_{agent}_{p}"));
        }
    }
}

/// The plan for `input`, started from `seed` (the hourly dispatch's schedule)
/// when there is one, so it is never worse than the schedule it improves.
pub(crate) fn solve(input: &BlendInput, seed: Option<&BlendSolution>, window: Option<&Window>, settings: Settings, stop: &Arc<AtomicBool>) -> Result<PlanSolve, String> {
    solve_periods(input, seed, window, settings, 1, false, stop)
}

/// The plan over the whole horizon in periods of `period_days` days, read
/// as the window plans read it, for targets rather than a bound: its linear
/// relaxation when `settings.relax`.
pub(crate) fn long_term(input: &BlendInput, seed: Option<&BlendSolution>, period_days: u32, settings: Settings, stop: &Arc<AtomicBool>) -> Result<PlanSolve, String> {
    solve_periods(input, seed, None, settings, period_days, false, stop)
}

/// The whole horizon's linear relaxation over periods of `period_days` days,
/// an upper bound on every schedule's value as the daily one is, and looser.
///
/// Every row of the daily plan only gets weaker when days join: a period's
/// loader hours, fleet hours and crusher budgets are its days' summed; a
/// block is released for the period if it is by the period's end; a block
/// may follow the one ahead of it in the same period, as in the same day; and
/// piles are held to their room and stock only at each period's end. A
/// grade target is kept for a period only where every day of it has the same
/// one, and then priced on the period's blend, which costs no more than the
/// days' blends do apart; any other is left off, and so costs nothing. So
/// the plan over periods allows every plan by day, and its optimum is no
/// less - for a model a fraction of the size, which a long horizon needs.
pub(crate) fn relaxation_bound(input: &BlendInput, period_days: u32, time_limit: Duration, stop: &Arc<AtomicBool>) -> Result<PlanSolve, String> {
    let settings = Settings {
        time_limit,
        relative_gap: 0.0,
        relax: true,
    };
    solve_periods(input, None, None, settings, period_days, true, stop)
}

fn solve_periods(
    input: &BlendInput,
    seed: Option<&BlendSolution>,
    window: Option<&Window>,
    settings: Settings,
    period_days: u32,
    bounding: bool,
    stop: &Arc<AtomicBool>,
) -> Result<PlanSolve, String> {
    let started = Instant::now();
    let prepared = Prepared::new(input, window, period_days, bounding);
    // Nothing seeds a relaxation: its blast columns have no values in a
    // schedule, and joined days none per period.
    let seed = seed.filter(|_| !settings.relax);
    let Prepared {
        days,
        period_days: covered,
        tonnes,
        pile_opening,
        outlets,
        chains,
        repeated,
        capacity,
        allowed,
        ..
    } = &prepared;
    let periods = prepared.periods();
    let materials = |ground: GroundId| prepared.materials(ground);
    let carried = |of: Carried, grade: usize, direction: f64| prepared.carried(of, grade, direction);
    let before = |id: DestinationId| prepared.before(id);

    // ---- model ----------------------------------------------------------------
    let mut builder = Builder::default();

    // Value per dug tonne of the materials with a single outlet, which need
    // no split column.
    let single_value = |ground: GroundId| -> f64 {
        materials(ground)
            .iter()
            .filter_map(|(material, fraction)| match outlets.get(&(ground, *material)).map(Vec::as_slice) {
                Some([only]) => Some(fraction * only.value),
                _ => None,
            })
            .sum()
    };

    // x[(loader, block, period)]
    let mut dig: BTreeMap<(usize, GroundId, usize), usize> = BTreeMap::new();
    for (&(loader_index, ground), list) in allowed {
        let value = single_value(ground);
        for &p in list {
            let upper = capacity[loader_index][p].min(tonnes[&ground]);
            let column = builder.cont(upper, value, &format!("x_{loader_index}_{}_{p}", ground.0));
            dig.insert((loader_index, ground, p), column);
        }
    }
    // Loader capacity per day.
    let mut by_loader_day: BTreeMap<(usize, usize), Vec<(usize, f64)>> = BTreeMap::new();
    for (&(loader_index, _, p), &column) in &dig {
        by_loader_day.entry((loader_index, p)).or_default().push((column, 1.0));
    }
    for (&(loader_index, p), terms) in &by_loader_day {
        builder.row(terms, f64::NEG_INFINITY, capacity[loader_index][p], &format!("cap_{loader_index}_{p}"));
    }
    // Each block's digs by day, and running totals, carried day to day so
    // every row stays a few terms long: dug so far by each loader of each
    // block, by all its loaders, and by each loader of its whole chain.
    let mut by_block: BTreeMap<GroundId, BTreeMap<usize, Vec<(usize, usize)>>> = BTreeMap::new();
    for (&(loader_index, ground, p), &column) in &dig {
        by_block.entry(ground).or_default().entry(p).or_default().push((loader_index, column));
    }
    let mut loader_so_far: BTreeMap<(usize, GroundId, usize), usize> = BTreeMap::new();
    let mut block_loaders: BTreeMap<(GroundId, usize), Vec<usize>> = BTreeMap::new();
    let mut first_day: BTreeMap<GroundId, usize> = BTreeMap::new();
    for (&(loader_index, ground), list) in allowed {
        let Some(&first) = list.first() else { continue };
        let mut previous: Option<usize> = None;
        for p in first..periods {
            let column = builder.cont(tonnes[&ground], 0.0, &format!("e_{loader_index}_{}_{p}", ground.0));
            let mut terms = vec![(column, 1.0)];
            if let Some(previous) = previous {
                terms.push((previous, -1.0));
            }
            if let Some(&today) = dig.get(&(loader_index, ground, p)) {
                terms.push((today, -1.0));
            }
            builder.row(&terms, 0.0, 0.0, &format!("ecarry_{loader_index}_{}_{p}", ground.0));
            loader_so_far.insert((loader_index, ground, p), column);
            // Indexed by block too, for the block totals below; loaders in
            // order, as `allowed` is.
            block_loaders.entry((ground, p)).or_default().push(column);
            first_day.entry(ground).and_modify(|day: &mut usize| *day = (*day).min(first)).or_insert(first);
            previous = Some(column);
        }
    }
    let mut block_so_far: BTreeMap<(GroundId, usize), usize> = BTreeMap::new();
    for ground in by_block.keys() {
        let Some(&first) = first_day.get(ground) else {
            continue;
        };
        for p in first..periods {
            // Within a block's own tonnes.
            let column = builder.cont(tonnes[ground], 0.0, &format!("d_{}_{p}", ground.0));
            let mut terms = vec![(column, 1.0)];
            terms.extend(block_loaders.get(&(*ground, p)).into_iter().flatten().map(|&e| (e, -1.0)));
            builder.row(&terms, 0.0, 0.0, &format!("dsum_{}_{p}", ground.0));
            block_so_far.insert((*ground, p), column);
        }
    }
    // ---- drill and blast: when each blast clears, finishes its steps and
    // fires, and the ground it holds back until then.
    if let Some(model) = &prepared.blasts {
        blast_rows(&mut builder, model, &block_so_far, tonnes, periods);
    }
    // ---- off the bound, on the held timeline: a blast's ground is dug only
    // if the ground above it is gone by the end of the period the timeline
    // clears it in. Otherwise the dispatch would fire it late, and the
    // ground the plan dug from it would not be there when the plan said.
    // Each such blast's on-time flag, and the blast each block is released
    // by, which the priority rows below read too.
    let mut on_time_of: BTreeMap<usize, usize> = BTreeMap::new();
    let released_by: BTreeMap<GroundId, usize> = input
        .drill_blast
        .as_ref()
        .map(|chain| {
            chain
                .blasts
                .iter()
                .enumerate()
                .flat_map(|(index, blast)| blast.releases.iter().map(move |ground| (*ground, index)))
                .collect()
        })
        .unwrap_or_default();
    if !bounding
        && let Some(chain) = input.drill_blast.as_ref()
        && let Some(timeline) = chain.fixed.as_ref()
    {
        let spans: Vec<(f64, f64)> = days
            .iter()
            .map(|day| {
                let intervals = &prepared.day_intervals[day];
                (
                    intervals.first().map_or(0.0, |interval| interval.start_h),
                    intervals.last().map_or(0.0, |interval| interval.end_h),
                )
            })
            .collect();
        let so_far = |ground: GroundId, p: Option<usize>| p.and_then(|p| block_so_far.get(&(ground, p)).copied());
        for (index, (blast, events)) in chain.blasts.iter().zip(&timeline.blasts).enumerate() {
            let Some(cleared_h) = events.cleared_h else { continue };
            if blast.stage != BlastStage::NotStarted || spans.first().is_none_or(|(start, _)| cleared_h <= *start) {
                continue;
            }
            let standing: Vec<GroundId> = blast.above.iter().copied().filter(|ground| tonnes.get(ground).is_some_and(|t| *t > 1e-9)).collect();
            let released: Vec<(usize, f64)> = blast
                .releases
                .iter()
                .filter_map(|ground| Some((so_far(*ground, Some(periods - 1))?, tonnes[ground])))
                .collect();
            if standing.is_empty() || released.is_empty() {
                continue;
            }
            let on_time = builder.binary(&format!("ontime_{index}"));
            on_time_of.insert(index, on_time);
            for &(dug, t) in &released {
                builder.row(&[(dug, 1.0), (on_time, -t)], f64::NEG_INFINITY, 0.0, &format!("onlyclear_{index}"));
            }
            // All of it gone by the end of the period the clearance hour
            // falls in. Pro rata within it ruled out every blast whose
            // clearance ground is itself released in that period.
            let p = spans.iter().position(|(_, end)| cleared_h <= *end + 1e-9).unwrap_or(periods - 1);
            let total: f64 = standing.iter().map(|ground| tonnes[ground]).sum();
            let mut terms = vec![(on_time, -total)];
            terms.extend(standing.iter().filter_map(|ground| so_far(*ground, Some(p))).map(|column| (column, 1.0)));
            builder.row(&terms, 0.0, f64::INFINITY, &format!("clearby_{index}"));
        }
    }

    let mut chain_so_far: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for loader_index in 0..chains.len() {
        let mut previous: Option<usize> = None;
        for p in 0..periods {
            let column = builder.cont(f64::INFINITY, 0.0, &format!("c_{loader_index}_{p}"));
            let mut terms = vec![(column, 1.0)];
            if let Some(previous) = previous {
                terms.push((previous, -1.0));
            }
            terms.extend(by_loader_day.get(&(loader_index, p)).into_iter().flatten().map(|(x, _)| (*x, -1.0)));
            builder.row(&terms, 0.0, 0.0, &format!("ccarry_{loader_index}_{p}"));
            chain_so_far.insert((loader_index, p), column);
            previous = Some(column);
        }
    }

    // The orders a loader is held to: each block after the one before it in
    // its bar, neither in another of its bars too.
    let orders = |loader_index: usize| {
        chains[loader_index]
            .windows(2)
            .filter(move |pair| pair[0].1 == pair[1].1 && !repeated.contains(&(loader_index, pair[0].0)) && !repeated.contains(&(loader_index, pair[1].0)))
    };
    // Finished flags for every block a chain waits on: all of it dug by the
    // end of the day, and staying finished.
    let waited: BTreeSet<GroundId> = (0..chains.len()).flat_map(|loader_index| orders(loader_index).map(|pair| pair[0].0)).collect();
    let mut finished: BTreeMap<(GroundId, usize), usize> = BTreeMap::new();
    for &ground in &waited {
        let mut previous: Option<usize> = None;
        for p in 0..periods {
            let Some(&so_far) = block_so_far.get(&(ground, p)) else { continue };
            let flag = builder.binary(&format!("fin_{}_{p}", ground.0));
            builder.row(&[(so_far, 1.0), (flag, -tonnes[&ground])], 0.0, f64::INFINITY, &format!("done_{}_{p}", ground.0));
            if let Some(previous) = previous {
                builder.row(&[(previous, 1.0), (flag, -1.0)], f64::NEG_INFINITY, 0.0, &format!("stay_{}_{p}", ground.0));
            }
            previous = Some(flag);
            finished.insert((ground, p), flag);
        }
    }
    // A loader's next block waits for this one to be finished.
    for loader_index in 0..chains.len() {
        for pair in orders(loader_index) {
            let (ahead, next) = (pair[0].0, pair[1].0);
            for p in 0..periods {
                let Some(&so_far) = loader_so_far.get(&(loader_index, next, p)) else { continue };
                let mut terms = vec![(so_far, 1.0)];
                if let Some(&flag) = finished.get(&(ahead, p)) {
                    terms.push((flag, -tonnes[&next]));
                }
                builder.row(&terms, f64::NEG_INFINITY, 0.0, &format!("order_{loader_index}_{}_{p}", next.0));
            }
        }
    }

    // Finishing a block takes everything ahead of it in its bar: the loader
    // has dug at least those tonnes and the block's own. Implied by the order rows in whole numbers, but not in the
    // relaxation, where fractional flags otherwise let a sliver of every
    // block be dug at once and ore deep in a sequence look reachable on the
    // first day. Valid while no block up to it is dug by another loader too
    // or sits in another of the loader's bars.
    let mut diggers: BTreeMap<GroundId, usize> = BTreeMap::new();
    for &(_, ground) in allowed.keys() {
        *diggers.entry(ground).or_default() += 1;
    }
    for (loader_index, chain) in chains.iter().enumerate() {
        let mut prefix = 0.0;
        let mut held = true;
        for (position, &(ground, task_index)) in chain.iter().enumerate() {
            let same_bar = position > 0 && chain[position - 1].1 == task_index;
            if !same_bar {
                (prefix, held) = (0.0, true);
            }
            prefix += tonnes[&ground];
            held &= diggers.get(&ground) == Some(&1) && !repeated.contains(&(loader_index, ground));
            if !held {
                continue;
            }
            for p in 0..periods {
                let Some(&flag) = finished.get(&(ground, p)) else { continue };
                builder.row(
                    &[(chain_so_far[&(loader_index, p)], 1.0), (flag, -prefix)],
                    0.0,
                    f64::INFINITY,
                    &format!("reach_{loader_index}_{}_{p}", ground.0),
                );
                if same_bar && let Some(&ahead) = finished.get(&(chain[position - 1].0, p)) {
                    builder.row(&[(flag, 1.0), (ahead, -1.0)], f64::NEG_INFINITY, 0.0, &format!("inorder_{loader_index}_{}_{p}", ground.0));
                }
            }
        }
    }

    // ---- where each day's dug material goes -------------------------------
    let destination = |id: DestinationId| prepared.destination(id);
    let building = |id: DestinationId, p: usize| prepared.building(id, p);
    let mut flows: BTreeMap<(DestinationId, usize), Vec<Flow>> = BTreeMap::new();
    let mut trucks: BTreeMap<usize, Vec<(usize, f64)>> = BTreeMap::new();
    // Read back: (block, material, destination, period), its column, tonnes
    // per unit, and the loader when the column is one loader's own dig.
    let mut routed: Vec<Routed> = Vec::new();
    for (ground, per_day) in &by_block {
        for (&p, list) in per_day {
            for (material, fraction) in materials(*ground) {
                let outlets = &outlets[&(*ground, material)];
                if let [only] = outlets.as_slice() {
                    for (loader_index, column) in list {
                        flows.entry((only.destination, p)).or_default().push((*column, fraction, Carried::Material(material)));
                        if only.truck_h > 0.0 {
                            trucks.entry(p).or_default().push((*column, fraction * only.truck_h));
                        }
                        routed.push(((*ground, material, only.destination, p), *column, fraction, Some(*loader_index)));
                    }
                    continue;
                }
                let mut split: Vec<(usize, f64)> = list.iter().map(|(_, column)| (*column, -fraction)).collect();
                for outlet in outlets.iter().filter(|outlet| building(outlet.destination, p)) {
                    let column = builder.cont(f64::INFINITY, outlet.value, &format!("r_{}_{}_{}_{p}", ground.0, material.0, outlet.destination.0));
                    split.push((column, 1.0));
                    flows.entry((outlet.destination, p)).or_default().push((column, 1.0, Carried::Material(material)));
                    if outlet.truck_h > 0.0 {
                        trucks.entry(p).or_default().push((column, outlet.truck_h));
                    }
                    routed.push(((*ground, material, outlet.destination, p), column, 1.0, None));
                }
                builder.row(&split, 0.0, 0.0, &format!("split_{}_{}_{p}", ground.0, material.0));
            }
        }
    }
    // ---- reclaim ------------------------------------------------------------
    let mut reclaim: BTreeMap<(usize, StockpileId, usize), usize> = BTreeMap::new();
    // Read back: the reclaiming loader, the destination, the period, the
    // column.
    let mut reclaim_routes: Vec<(usize, DestinationId, usize, usize)> = Vec::new();
    let mut drawn: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for source in &prepared.reclaims {
        let (task_index, pile, routes) = (source.task, &source.pile, &source.routes);
        for (p, &cap) in source.caps.iter().enumerate() {
            if cap <= 0.0 {
                continue;
            }
            let single = if let [only] = routes.as_slice() { only.value } else { 0.0 };
            let column = builder.cont(cap, single, &format!("rc_{task_index}_{}_{p}", pile.0));
            reclaim.insert((task_index, *pile, p), column);
            drawn.entry(task_index).or_default().push(column);
            if let [only] = routes.as_slice() {
                flows.entry((only.destination, p)).or_default().push((column, 1.0, Carried::Pile(*pile)));
                if only.truck_h > 0.0 {
                    trucks.entry(p).or_default().push((column, only.truck_h));
                }
                reclaim_routes.push((source.loader, only.destination, p, column));
                continue;
            }
            let mut split = vec![(column, -1.0)];
            for route in routes.iter().filter(|route| building(route.destination, p)) {
                let to = builder.cont(f64::INFINITY, route.value, &format!("rq_{task_index}_{}_{}_{p}", pile.0, route.destination.0));
                split.push((to, 1.0));
                flows.entry((route.destination, p)).or_default().push((to, 1.0, Carried::Pile(*pile)));
                if route.truck_h > 0.0 {
                    trucks.entry(p).or_default().push((to, route.truck_h));
                }
                reclaim_routes.push((source.loader, route.destination, p, to));
            }
            builder.row(&split, 0.0, 0.0, &format!("rsplit_{task_index}_{}_{p}", pile.0));
        }
    }
    // Each bar within its authored cap, less what it drew before.
    for (task_index, columns) in &drawn {
        if let TaskKind::Reclaim { maximum_t: Some(maximum), .. } = input.tasks[*task_index].kind {
            let left = maximum - prepared.reclaimed_before(*task_index);
            let terms: Vec<(usize, f64)> = columns.iter().map(|column| (*column, 1.0)).collect();
            builder.row(&terms, f64::NEG_INFINITY, left.max(0.0) / UNIT, &format!("rcap_{task_index}"));
        }
    }
    // A loader's dig and reclaim share its hours: each at the day's fastest
    // rate, which only ever leaves it more time.
    for (loader_index, loader) in input.loaders.iter().enumerate() {
        for p in 0..periods {
            let drawing: Vec<usize> = reclaim
                .iter()
                .filter(|((task, _, at), _)| *at == p && input.tasks[*task].loader == loader.id)
                .map(|(_, column)| *column)
                .collect();
            if drawing.is_empty() {
                continue;
            }
            let (dig_tph, reclaim_tph, hours) = prepared.loader_hours(loader_index, p);
            let mut terms: Vec<(usize, f64)> = drawing.iter().map(|column| (*column, UNIT / reclaim_tph.max(1e-9))).collect();
            if dig_tph > 0.0 {
                terms.extend(by_loader_day.get(&(loader_index, p)).into_iter().flatten().map(|(x, _)| (*x, UNIT / dig_tph)));
            }
            builder.row(&terms, f64::NEG_INFINITY, hours, &format!("hours_{loader_index}_{p}"));
        }
    }
    let mut inventory: BTreeMap<(StockpileId, usize), usize> = BTreeMap::new();
    // Each pile's inventory, day by day: what it held, plus what the plan
    // delivers, less what it reclaims, within its capacity. Reclaim may draw
    // the day's own deliveries, which the dispatch's rests and release
    // forbid: that only leaves the plan more.
    for pile in &input.piles {
        let deliveries: Vec<DestinationId> = input
            .destinations
            .iter()
            .filter(|entry| entry.kind == DestinationKind::Stockpile(pile.id))
            .map(|entry| entry.id)
            .collect();
        let opening = pile_opening.get(&pile.id).map_or(0.0, |(tonnes, _)| *tonnes) / UNIT;
        let mut previous: Option<usize> = None;
        for p in 0..periods {
            let held = builder.cont(pile.capacity_t / UNIT, 0.0, &format!("inv_{}_{p}", pile.id.0));
            inventory.insert((pile.id, p), held);
            let mut terms = vec![(held, 1.0)];
            if let Some(previous) = previous {
                terms.push((previous, -1.0));
            }
            for id in &deliveries {
                terms.extend(flows.get(&(*id, p)).into_iter().flatten().map(|(column, per_unit, _)| (*column, -per_unit)));
            }
            terms.extend(reclaim.iter().filter(|((_, from, at), _)| *from == pile.id && *at == p).map(|(_, column)| (*column, 1.0)));
            let rhs = if previous.is_none() { opening } else { 0.0 };
            builder.row(&terms, rhs, rhs, &format!("inv_{}_{p}", pile.id.0));
            previous = Some(held);
        }
    }

    // Destination room.
    let mut totals: BTreeMap<DestinationId, Vec<(usize, f64)>> = BTreeMap::new();
    for (&(id, p), list) in &flows {
        let terms: Vec<(usize, f64)> = list.iter().map(|(v, c, _)| (*v, *c)).collect();
        totals.entry(id).or_default().extend(terms.iter().cloned());
        let Some(entry) = destination(id) else { continue };
        if !building(id, p) {
            builder.row(&terms, f64::NEG_INFINITY, 0.0, &format!("closed_{}_{p}", id.0));
        }
        // A period's budget is its days', and none if any day has none.
        if entry.kind == DestinationKind::Crusher
            && let Some(budget) = covered[p]
                .iter()
                .map(|day| entry.crusher_daily_t.get(*day as usize).copied().flatten())
                .sum::<Option<f64>>()
        {
            builder.row(&terms, f64::NEG_INFINITY, budget / UNIT, &format!("crush_{}_{p}", id.0));
        }
    }
    for (id, terms) in &totals {
        let Some(entry) = destination(*id) else { continue };
        let room = match entry.kind {
            // A pile's room is its inventory's, above.
            DestinationKind::Crusher | DestinationKind::Stockpile(_) => None,
            DestinationKind::Dump => entry.capacity_t.map(|capacity| capacity - before(*id)),
        };
        if let Some(room) = room {
            builder.row(terms, f64::NEG_INFINITY, room.max(0.0) / UNIT, &format!("room_{}", id.0));
        }
    }
    // Fleet hours per day, every class together.
    for (p, terms) in &trucks {
        builder.row(terms, f64::NEG_INFINITY, prepared.fleet_hours(*p), &format!("fleet_{p}"));
    }
    // What a reclaim carries, over the whole horizon. Any blend the pile
    // could hold would do for a bound, so each reclaimed flow carries its own
    // metal of each targeted grade, at most its tonnes at the grade's
    // ceiling - but a pile gives up no more metal than it was given, and
    // keeps no more than its stock at the ceiling holds. A schedule keeps
    // both, so the plan still allows every one; it only loses the freedom
    // to reclaim off-grade stock as though it were on grade.
    let mut contained: HashMap<(usize, usize), usize> = HashMap::new();
    if prepared.optimistic {
        let targeted: BTreeSet<usize> = input.grade_targets.iter().map(|target| target.grade).collect();
        for (&(_, p), list) in &flows {
            for &(column, per_unit, of) in list {
                if let Carried::Pile(_) = of {
                    for &grade in &targeted {
                        let ceiling = prepared.ceilings.get(grade).copied().unwrap_or(0.0);
                        let q = builder.cont(f64::INFINITY, 0.0, &format!("cq_{column}_{grade}_{p}"));
                        builder.row(&[(q, 1.0), (column, -per_unit * ceiling)], f64::NEG_INFINITY, 0.0, &format!("cqcap_{column}_{grade}"));
                        contained.insert((column, grade), q);
                    }
                }
            }
        }
        for pile in &input.piles {
            let delivering: BTreeSet<DestinationId> = input
                .destinations
                .iter()
                .filter(|entry| entry.kind == DestinationKind::Stockpile(pile.id))
                .map(|entry| entry.id)
                .collect();
            let opening = pile_opening.get(&pile.id).map(|(_, contained)| contained.clone()).unwrap_or_default();
            for &grade in &targeted {
                let ceiling = prepared.ceilings.get(grade).copied().unwrap_or(0.0);
                let mut previous: Option<usize> = None;
                for p in 0..periods {
                    // Metal held at the end of the period, in grade-kilotonnes.
                    let held = builder.cont(f64::INFINITY, 0.0, &format!("metal_{}_{grade}_{p}", pile.id.0));
                    let mut terms = vec![(held, 1.0)];
                    if let Some(previous) = previous {
                        terms.push((previous, -1.0));
                    }
                    for (&(id, at), list) in &flows {
                        if at != p {
                            continue;
                        }
                        for &(column, per_unit, of) in list {
                            let into = delivering.contains(&id);
                            let from = of == Carried::Pile(pile.id);
                            if !into && !from {
                                continue;
                            }
                            let metal: Vec<(usize, f64)> = match of {
                                Carried::Material(material) => vec![(column, per_unit * input.grades.fraction(material, grade).unwrap_or(0.0))],
                                Carried::Pile(_) => contained.get(&(column, grade)).map(|q| vec![(*q, 1.0)]).unwrap_or_default(),
                            };
                            for (variable, coefficient) in metal {
                                terms.push((variable, if into { -coefficient } else { coefficient }));
                            }
                        }
                    }
                    let rhs = if previous.is_none() { opening.get(grade).copied().unwrap_or(0.0) / UNIT } else { 0.0 };
                    builder.row(&terms, rhs, rhs, &format!("metal_{}_{grade}_{p}", pile.id.0));
                    if let Some(stock) = inventory.get(&(pile.id, p)) {
                        builder.row(&[(held, 1.0), (*stock, -ceiling)], f64::NEG_INFINITY, 0.0, &format!("metalcap_{}_{grade}_{p}", pile.id.0));
                    }
                    previous = Some(held);
                }
            }
        }
    }
    // Daily soft grade targets, priced as the dispatch and the replay price
    // them.
    for (index, target) in input.grade_targets.iter().enumerate() {
        let Some(p) = covered.iter().position(|days| days.contains(&target.day)) else {
            continue;
        };
        // Days joined, a period carries one target per destination and grade,
        // the one every day of it has; see `relaxation_bound`.
        if covered[p].len() > 1 {
            let same: Vec<_> = input
                .grade_targets
                .iter()
                .enumerate()
                .filter(|(_, other)| other.destination == target.destination && other.grade == target.grade && covered[p].contains(&other.day))
                .collect();
            let shared = same.len() == covered[p].len() && same.iter().all(|(_, other)| other.specification.hinges() == target.specification.hinges());
            if !shared || same.first().is_some_and(|(first, _)| *first != index) {
                continue;
            }
        }
        let Some(list) = flows.get(&(target.destination, p)) else { continue };
        for (hinge, (boundary, direction, slope)) in target.specification.hinges().into_iter().enumerate() {
            if slope == 0.0 {
                continue;
            }
            let slack = builder.cont(f64::INFINITY, -slope * UNIT, &format!("pen_{index}_{hinge}"));
            let mut terms: Vec<(usize, f64)> = Vec::with_capacity(list.len() + 1);
            for &(v, c, of) in list {
                match contained.get(&(v, target.grade)) {
                    // A reclaim's own metal, above.
                    Some(&q) => terms.extend([(q, direction), (v, -c * direction * boundary)]),
                    None => terms.push((v, c * direction * (carried(of, target.grade, direction) - boundary))),
                }
            }
            terms.push((slack, -1.0));
            builder.row(&terms, f64::NEG_INFINITY, 0.0, &format!("grade_{index}_{hinge}"));
        }
    }

    // ---- the seed, in the plan's own columns ------------------------------------
    let mut seeded = None;
    let mut seed_values: Option<Vec<f64>> = None;
    if let Some(seed) = seed {
        // Every day of a period maps to it, not just its first.
        let day_index: BTreeMap<u32, usize> = covered.iter().enumerate().flat_map(|(p, list)| list.iter().map(move |day| (*day, p))).collect();
        let mut values: HashMap<String, f64> = HashMap::new();
        let mut done: BTreeMap<(GroundId, usize), f64> = BTreeMap::new();
        let mut received: BTreeMap<(DestinationId, usize), Vec<(Carried, f64)>> = BTreeMap::new();
        let mut worth = 0.0;
        for row in &seed.movements {
            let candidate = &input.movements[row.candidate];
            let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source) else {
                continue;
            };
            let Some(loader_index) = input.loaders.iter().position(|loader| loader.id == candidate.loader) else {
                continue;
            };
            let Some(&p) = input.intervals.get(row.interval).and_then(|interval| day_index.get(&interval.day())) else {
                continue;
            };
            let tonnes_t = row.tonnes_t / UNIT;
            *values.entry(format!("x_{loader_index}_{}_{p}", ground.0)).or_default() += tonnes_t;
            *done.entry((ground, p)).or_default() += tonnes_t;
            if outlets.get(&(ground, candidate.material)).is_some_and(|list| list.len() > 1) {
                *values
                    .entry(format!("r_{}_{}_{}_{p}", ground.0, candidate.material.0, candidate.destination.0))
                    .or_default() += tonnes_t;
            }
            received
                .entry((candidate.destination, p))
                .or_default()
                .push((Carried::Material(candidate.material), tonnes_t));
            if let Some(outlet) = outlets
                .get(&(ground, candidate.material))
                .and_then(|list| list.iter().find(|outlet| outlet.destination == candidate.destination))
            {
                worth += outlet.value * tonnes_t;
            }
        }
        // Reclaim, each row shared between the bars it counts against as
        // the dispatch and the replay share it.
        let mut drawn: BTreeMap<usize, f64> = window.map(|window| window.reclaimed.clone()).unwrap_or_default();
        let mut rows: Vec<_> = seed.movements.iter().filter(|row| input.movements[row.candidate].activity == Activity::Reclaim).collect();
        rows.sort_by_key(|row| row.interval);
        for row in rows {
            let candidate = &input.movements[row.candidate];
            let SourceId::Stockpile(pile) = candidate.source else { continue };
            let Some(&p) = input.intervals.get(row.interval).and_then(|interval| day_index.get(&interval.day())) else {
                continue;
            };
            for (task, share) in attribute_reclaim(input, candidate, input.intervals[row.interval], row.tonnes_t, &drawn) {
                *drawn.entry(task).or_default() += share;
                let share = share / UNIT;
                *values.entry(format!("rc_{task}_{}_{p}", pile.0)).or_default() += share;
                *values.entry(format!("rq_{task}_{}_{}_{p}", pile.0, candidate.destination.0)).or_default() += share;
            }
            received.entry((candidate.destination, p)).or_default().push((Carried::Pile(pile), row.tonnes_t / UNIT));
        }
        for pile in &input.piles {
            let mut held = pile_opening.get(&pile.id).map_or(0.0, |(tonnes, _)| *tonnes) / UNIT;
            for p in 0..periods {
                for entry in input.destinations.iter().filter(|entry| entry.kind == DestinationKind::Stockpile(pile.id)) {
                    held += received.get(&(entry.id, p)).into_iter().flatten().map(|(_, tonnes)| tonnes).sum::<f64>();
                }
                held -= values
                    .iter()
                    .filter(|(name, _)| name.starts_with("rc_") && name.ends_with(&format!("_{}_{p}", pile.id.0)))
                    .map(|(_, tonnes)| tonnes)
                    .sum::<f64>();
                values.insert(format!("inv_{}_{p}", pile.id.0), held.max(0.0));
            }
        }
        for (&(loader_index, ground), list) in allowed {
            let Some(&first) = list.first() else { continue };
            let mut cumulative = 0.0;
            for p in first..periods {
                cumulative += values.get(&format!("x_{loader_index}_{}_{p}", ground.0)).copied().unwrap_or(0.0);
                values.insert(format!("e_{loader_index}_{}_{p}", ground.0), cumulative);
                *values.entry(format!("d_{}_{p}", ground.0)).or_default() += cumulative;
            }
        }
        for loader_index in 0..chains.len() {
            let mut cumulative = 0.0;
            for p in 0..periods {
                cumulative += dig
                    .iter()
                    .filter(|((l, _, at), _)| *l == loader_index && *at == p)
                    .map(|((_, ground, _), _)| values.get(&format!("x_{loader_index}_{}_{p}", ground.0)).copied().unwrap_or(0.0))
                    .sum::<f64>();
                values.insert(format!("c_{loader_index}_{p}"), cumulative);
            }
        }
        for &ground in &waited {
            let mut cumulative = 0.0;
            for p in 0..periods {
                cumulative += done.get(&(ground, p)).copied().unwrap_or(0.0);
                // Finished once its `done_` row holds with the flag set, as the
                // solver reads that row: any looser and a block with a sliver
                // left finishes before the day that digs it.
                if cumulative - tonnes[&ground] >= -solver::MIP_FEASIBILITY_TOLERANCE {
                    values.insert(format!("fin_{}_{p}", ground.0), 1.0);
                }
            }
        }
        for (index, target) in input.grade_targets.iter().enumerate() {
            let Some(p) = days.iter().position(|day| *day == target.day) else { continue };
            let list = received.get(&(target.destination, p)).map(Vec::as_slice).unwrap_or_default();
            for (hinge, (boundary, direction, slope)) in target.specification.hinges().into_iter().enumerate() {
                let excess: f64 = list.iter().map(|(of, t)| t * direction * (carried(*of, target.grade, direction) - boundary)).sum();
                values.insert(format!("pen_{index}_{hinge}"), excess.max(0.0));
                worth -= slope * UNIT * excess.max(0.0);
            }
        }
        seed_values = Some(builder.names.iter().map(|name| values.get(name).copied().unwrap_or(0.0)).collect::<Vec<f64>>());
        seeded = Some(worth);
    }
    // ---- priority, as the dispatch keeps it: a bar has work while its
    // current block - the first of its sequence not finished - is released.
    // It has work for a whole period exactly when every block it passes in
    // it was released as the period opens; one finished before then was
    // released before then. So a bar held its loader all period, and the
    // loader worked none of its lower bars, unless every block ahead of its
    // first one still unreleased at the period's start is finished by the
    // period's end. Blocked by room at a destination is not read here.
    if !bounding {
        let releases = input.drill_blast.as_ref().map(|chain| chain.releases()).unwrap_or_default();
        let starts: Vec<f64> = days
            .iter()
            .map(|day| prepared.day_intervals[day].first().map_or(0.0, |interval| interval.start_h))
            .collect();
        let mut by_loader_period: BTreeMap<(usize, usize), Vec<(GroundId, usize)>> = BTreeMap::new();
        for (&(loader_index, ground, p), &column) in &dig {
            by_loader_period.entry((loader_index, p)).or_default().push((ground, column));
        }
        for loader_index in 0..chains.len() {
            let order = authored_tasks(input, loader_index);
            let rank: BTreeMap<usize, usize> = order.iter().enumerate().map(|(r, task)| (*task, r)).collect();
            let mut bars: BTreeMap<usize, Vec<GroundId>> = BTreeMap::new();
            let mut bar_of: BTreeMap<GroundId, usize> = BTreeMap::new();
            for &(ground, task) in &chains[loader_index] {
                let r = rank[&task];
                bar_of.entry(ground).or_insert(r);
                bars.entry(r).or_default().push(ground);
            }
            for (&r, blocks) in &bars {
                let task = &input.tasks[order[r]];
                for p in 0..periods {
                    let cap = capacity[loader_index][p];
                    if cap <= 0.0 || !prepared.day_intervals[&days[p]].iter().all(|interval| task_active(task, *interval)) {
                        continue;
                    }
                    // The blocks ahead of the first still unreleased.
                    let ready: Vec<GroundId> = blocks
                        .iter()
                        .copied()
                        .take_while(|ground| releases.get(ground).is_none_or(|released| *released <= starts[p] + 1e-9))
                        .collect();
                    if ready.is_empty() {
                        continue;
                    }
                    let mut lower: Vec<(usize, f64)> = by_loader_period
                        .get(&(loader_index, p))
                        .into_iter()
                        .flatten()
                        .filter(|(ground, _)| bar_of.get(ground).is_some_and(|b| *b > r))
                        .map(|(_, column)| (*column, 1.0))
                        .collect();
                    for (&(task_index, _, at), &column) in &reclaim {
                        if at == p && rank.get(&task_index).is_some_and(|b| *b > r) {
                            lower.push((column, 1.0));
                        }
                    }
                    if lower.is_empty() {
                        continue;
                    }
                    // A block of a blast the plan makes late is not released:
                    // the ready run is cut at the first block of each blast
                    // whose clearance the plan decides, and each longer run
                    // holds the loader only while all those blasts are on time.
                    let mut cuts: Vec<(usize, usize)> = Vec::new();
                    let mut seen: BTreeSet<usize> = BTreeSet::new();
                    for (position, ground) in ready.iter().enumerate() {
                        if let Some(&blast) = released_by.get(ground)
                            && let Some(&flag) = on_time_of.get(&blast)
                            && seen.insert(blast)
                        {
                            cuts.push((position, flag));
                        }
                    }
                    let mut runs: Vec<(usize, Vec<usize>)> = Vec::new();
                    for k in 0..=cuts.len() {
                        let end = cuts.get(k).map_or(ready.len(), |(position, _)| *position);
                        if end > 0 {
                            runs.push((end, cuts[..k].iter().map(|(_, flag)| *flag).collect()));
                        }
                    }
                    for (run, (end, needs)) in runs.into_iter().enumerate() {
                        let ready = &ready[..end];
                        let total: f64 = ready.iter().map(|ground| tonnes[ground]).sum();
                        let done = builder.binary(&format!("pdone_{loader_index}_{r}_{p}_{run}"));
                        if let Some(values) = seed_values.as_mut() {
                            let dug: f64 = ready.iter().filter_map(|ground| block_so_far.get(&(*ground, p))).map(|column| values[*column]).sum();
                            values.push(if dug >= total - 1e-6 { 1.0 } else { 0.0 });
                        }
                        let mut terms: Vec<(usize, f64)> = ready.iter().filter_map(|ground| block_so_far.get(&(*ground, p))).map(|column| (*column, 1.0)).collect();
                        terms.push((done, -total));
                        builder.row(&terms, 0.0, f64::INFINITY, &format!("pready_{loader_index}_{r}_{p}"));
                        // Held unless the run is finished or one of its blasts is late.
                        let mut row = lower.clone();
                        row.push((done, -cap));
                        for &flag in &needs {
                            row.push((flag, cap));
                        }
                        builder.row(&row, f64::NEG_INFINITY, cap * needs.len() as f64, &format!("prio_{loader_index}_{r}_{p}"));
                    }
                }
            }
        }
    }
    let seed_violations = seed_values.as_deref().map(|values| builder.violations(values));

    let build_s = started.elapsed().as_secs_f64();
    let (variables, binaries, rows) = (builder.cost.len(), builder.binaries, builder.row_lower.len());
    let solving = Instant::now();
    let answer = solve_plan(&builder, seed_values.as_deref(), settings.time_limit, settings.relative_gap, settings.relax, stop)?;
    let solve_s = solving.elapsed().as_secs_f64();

    // ---- read back ----------------------------------------------------------
    let mut dug: BTreeMap<(GroundId, usize), Vec<(usize, f64)>> = BTreeMap::new();
    let mut by_loader_block: BTreeMap<(usize, GroundId), f64> = BTreeMap::new();
    for (&(loader_index, ground, p), column) in &dig {
        let value = answer.values[*column].max(0.0) * UNIT;
        if value > 1e-6 {
            dug.entry((ground, p)).or_default().push((loader_index, value));
            *by_loader_block.entry((loader_index, ground)).or_default() += value;
        }
    }
    let mut routes: BTreeMap<(usize, DestinationId, u32), f64> = BTreeMap::new();
    // A split column is the block's, shared between its diggers by what each
    // dug that day.
    for ((ground, _, id, p), column, per_unit, owner) in &routed {
        let tonnes_t = answer.values[*column].max(0.0) * per_unit * UNIT;
        if tonnes_t <= 1e-6 {
            continue;
        }
        if let Some(loader_index) = owner {
            *routes.entry((*loader_index, *id, days[*p])).or_default() += tonnes_t;
        } else if let Some(diggers) = dug.get(&(*ground, *p)) {
            let total: f64 = diggers.iter().map(|(_, t)| t).sum();
            for (loader_index, share) in diggers {
                *routes.entry((*loader_index, *id, days[*p])).or_default() += tonnes_t * share / total;
            }
        }
    }
    for (loader_index, id, p, column) in &reclaim_routes {
        let tonnes_t = answer.values[*column].max(0.0) * UNIT;
        if tonnes_t > 1e-6 {
            *routes.entry((*loader_index, *id, days[*p])).or_default() += tonnes_t;
        }
    }
    let objective = Some(answer.objective);
    let bound = answer.bound;
    // A movement column's name ends in its period: x_l_b_p, r_b_m_d_p,
    // rc_t_s_p and rq_t_s_d_p.
    let mut day_values: BTreeMap<u32, f64> = BTreeMap::new();
    for (column, name) in builder.names.iter().enumerate() {
        if ["x_", "r_", "rc_", "rq_"].iter().any(|prefix| name.starts_with(prefix))
            && let Some(p) = name.rsplit('_').next().and_then(|p| p.parse::<usize>().ok())
        {
            *day_values.entry(days[p]).or_default() += builder.cost[column] * answer.values[column];
        }
    }
    Ok(PlanSolve {
        day_values,
        routes,
        objective,
        bound,
        variables,
        binaries,
        rows,
        build_s,
        solve_s,
        seeded,
        seed_violations,
        dug: by_loader_block,
        status: answer.status,
    })
}
