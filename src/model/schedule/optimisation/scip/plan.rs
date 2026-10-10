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
//! What it does not see: anything inside a day, truck classes (a route uses
//! its best candidate's truck hours against the whole fleet), per-loader
//! values (a route takes its best candidate's value), chunks, rests, and a
//! reclaimed blend's route qualifications. Each only constrains a schedule
//! more, so the bound stands. Its answer is only targets; see
//! [`super::super::blended::plan`].
//!
//! Solved by HiGHS, which on these models found plans and bounds where SCIP
//! did not.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ffi::{c_char, c_int, c_void},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use highs_sys::{HighsCallbackDataIn, HighsCallbackDataOut, HighsInt};

use super::super::blended::{
    input::{BlendInput, attribute_reclaim, authored_tasks, grade_ceilings, interval_rate, task_active},
    replay::{BlendSolution, ReplayReport},
};
use crate::model::schedule::optimisation::{Activity, DestinationId, DestinationKind, GroundId, Interval, MaterialId, SourceId, StockpileId, TaskKind};

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
        remaining.retain(|_, left| *left > 1e-6);
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
    /// What the plan's movements earn on each day it plans.
    pub(crate) day_values: BTreeMap<u32, f64>,
    pub(crate) status: String,
}

/// The plan's columns and rows, kept row-wise for HiGHS.
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
}

/// A solve's column values, objective, bound and status.
struct Answer {
    values: Vec<f64>,
    objective: f64,
    bound: Option<f64>,
    status: String,
}

fn solve_highs(builder: &Builder, seed: Option<&[f64]>, time_limit: Duration, relative_gap: f64, relax: bool, stop: &AtomicBool) -> Result<Answer, String> {
    let fits = |count: usize| HighsInt::try_from(count).map_err(|_| format!("the plan has {count} entries, more than HiGHS can index"));
    let columns = builder.cost.len();
    let starts = builder.starts.iter().map(|&start| fits(start)).collect::<Result<Vec<_>, _>>()?;
    let index = builder.index.iter().map(|&column| fits(column)).collect::<Result<Vec<_>, _>>()?;
    let integrality: Vec<HighsInt> = builder.integer.iter().map(|&integer| HighsInt::from(integer && !relax)).collect();
    let lower = vec![0.0; columns];
    let upper: Vec<f64> = builder.upper.iter().map(|&upper| if upper.is_finite() { upper } else { f64::INFINITY }).collect();
    struct Instance(*mut c_void);
    impl Drop for Instance {
        fn drop(&mut self) {
            // SAFETY: created by `Highs_create` and destroyed once, here.
            unsafe { highs_sys::Highs_destroy(self.0) };
        }
    }
    // SAFETY: one HiGHS instance on this thread, freed by `Instance`; every
    // array has the length its count says.
    unsafe {
        let highs = Instance(highs_sys::Highs_create());
        let ok = |code: HighsInt, what: &str| {
            if code == highs_sys::STATUS_ERROR {
                Err(format!("HiGHS rejected {what}"))
            } else {
                Ok(())
            }
        };
        ok(highs_sys::Highs_setBoolOptionValue(highs.0, c"output_flag".as_ptr(), 0), "output_flag")?;
        ok(
            highs_sys::Highs_setDoubleOptionValue(highs.0, c"time_limit".as_ptr(), time_limit.as_secs_f64()),
            "the time limit",
        )?;
        ok(highs_sys::Highs_setDoubleOptionValue(highs.0, c"mip_rel_gap".as_ptr(), relative_gap), "the gap")?;
        if relax {
            ok(highs_sys::Highs_setStringOptionValue(highs.0, c"solver".as_ptr(), c"ipm".as_ptr()), "the ipm solver")?;
        }
        ok(
            highs_sys::Highs_passMip(
                highs.0,
                fits(columns)?,
                fits(builder.row_lower.len())?,
                fits(builder.index.len())?,
                highs_sys::MATRIX_FORMAT_ROW_WISE,
                highs_sys::OBJECTIVE_SENSE_MAXIMIZE,
                0.0,
                builder.cost.as_ptr(),
                lower.as_ptr(),
                upper.as_ptr(),
                builder.row_lower.as_ptr(),
                builder.row_upper.as_ptr(),
                starts.as_ptr(),
                index.as_ptr(),
                builder.value.as_ptr(),
                integrality.as_ptr(),
            ),
            "the plan",
        )?;
        if let Some(seed) = seed {
            ok(
                highs_sys::Highs_setSolution(highs.0, seed.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null()),
                "the seed",
            )?;
        }
        let data = stop as *const AtomicBool as *mut c_void;
        ok(highs_sys::Highs_setCallback(highs.0, Some(interrupt), data), "the interrupt callback")?;
        for kind in [
            highs_sys::kHighsCallbackMipInterrupt,
            highs_sys::kHighsCallbackIpmInterrupt,
            highs_sys::kHighsCallbackSimplexInterrupt,
        ] {
            ok(highs_sys::Highs_startCallback(highs.0, kind), "the interrupt callback")?;
        }
        ok(highs_sys::Highs_run(highs.0), "the plan solve")?;
        let status = highs_sys::Highs_getModelStatus(highs.0);
        let mut values = vec![0.0; columns];
        let mut duals = vec![0.0; columns];
        let mut row_values = vec![0.0; builder.row_lower.len()];
        let mut row_duals = vec![0.0; builder.row_lower.len()];
        ok(
            highs_sys::Highs_getSolution(highs.0, values.as_mut_ptr(), duals.as_mut_ptr(), row_values.as_mut_ptr(), row_duals.as_mut_ptr()),
            "reading the plan",
        )?;
        let objective = highs_sys::Highs_getObjectiveValue(highs.0);
        let mut bound = 0.0;
        let found = highs_sys::Highs_getDoubleInfoValue(highs.0, c"mip_dual_bound".as_ptr(), &mut bound);
        let mut nodes: i64 = 0;
        highs_sys::Highs_getInt64InfoValue(highs.0, c"mip_node_count".as_ptr(), &mut nodes);
        if !objective.is_finite() {
            return Err(format!("no plan found: HiGHS model status {status}"));
        }
        // A relaxation interrupted before its optimum bounds nothing. Nor,
        // usefully, does a mixed-integer solve stopped inside its first
        // relaxation: its dual bound is then only what each column's own
        // bounds allow, valid but many times any schedule's value.
        let bound = if relax {
            (status == highs_sys::MODEL_STATUS_OPTIMAL).then_some(objective)
        } else {
            (found != highs_sys::STATUS_ERROR && bound.is_finite() && (status == highs_sys::MODEL_STATUS_OPTIMAL || nodes > 0)).then_some(bound)
        };
        Ok(Answer {
            values,
            objective,
            bound,
            status: format!("HiGHS status {status}, {nodes} nodes"),
        })
    }
}

/// HiGHS's interrupt callback: `data` is the `stop` flag [`solve_highs`]
/// passed.
unsafe extern "C" fn interrupt(_kind: c_int, _message: *const c_char, _out: *const HighsCallbackDataOut, input: *mut HighsCallbackDataIn, data: *mut c_void) {
    // SAFETY: `data` points at `solve_highs`' `stop`, alive for the whole
    // `Highs_run`; `input` is HiGHS's own and may be written.
    unsafe {
        let stop = &*(data as *const AtomicBool);
        if !input.is_null() && stop.load(Ordering::Relaxed) {
            (*input).user_interrupt = 1;
        }
    }
}

/// One destination a block's material can go to, on its best candidate.
#[derive(Clone, Copy)]
pub(super) struct Outlet {
    pub(super) destination: DestinationId,
    pub(super) value: f64,
    pub(super) truck_h: f64,
}

/// Whose grade a flow carries.
#[derive(Clone, Copy)]
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
    /// Kilotonnes each loader can dig each day.
    pub(super) capacity: Vec<Vec<f64>>,
    pub(super) allowed: BTreeMap<(usize, GroundId), Vec<usize>>,
    pub(super) reclaims: Vec<ReclaimSource>,
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
    pub(super) fn new(input: &'a BlendInput, window: Option<&'a Window>, period_days: u32) -> Self {
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

        let tonnes: BTreeMap<GroundId, f64> = match window {
            Some(window) => window.remaining.iter().map(|(ground, left)| (*ground, left / UNIT)).collect(),
            None => input.ground.iter().map(|source| (source.id, source.tonnes_t / UNIT)).collect(),
        };
        // Over the whole horizon the plan bounds every schedule, so a reclaim's
        // grade is the one that suits each use of it best.
        let optimistic = window.is_none();
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
        let releases = input.drill_blast.as_ref().map(|chain| chain.releases()).unwrap_or_default();

        // ---- outlets: each block material's destinations, on its best candidate
        let mut outlets: BTreeMap<(GroundId, MaterialId), Vec<Outlet>> = BTreeMap::new();
        for candidate in &input.movements {
            let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source) else {
                continue;
            };
            let value = candidate.value_per_tonne().unwrap_or(0.0) * UNIT;
            let truck_h = candidate.truck_hours_per_tonne * UNIT;
            let list = outlets.entry((ground, candidate.material)).or_default();
            match list.iter_mut().find(|outlet| outlet.destination == candidate.destination) {
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
        // chain[l] = (block, task) in authored order, cut at the first block the
        // plan cannot dig.
        let mut chains: Vec<Vec<(GroundId, usize)>> = Vec::new();
        let mut capacity: Vec<Vec<f64>> = Vec::new();
        for (loader_index, loader) in input.loaders.iter().enumerate() {
            let mut chain = Vec::new();
            let mut seen = BTreeSet::new();
            'tasks: for task_index in authored_tasks(input, loader_index) {
                let TaskKind::Dig { sequence } = &input.tasks[task_index].kind else { continue };
                for ground in sequence {
                    if !tonnes.contains_key(ground) || !seen.insert(*ground) {
                        continue;
                    }
                    if !diggable(*ground) {
                        break 'tasks;
                    }
                    chain.push((*ground, task_index));
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
        // Days each (loader, block) may be dug: its bar active and the loader
        // able to dig, not before the loader could have reached it at full rate,
        // and not before its blast is released.
        let mut allowed: BTreeMap<(usize, GroundId), Vec<usize>> = BTreeMap::new();
        for (loader_index, chain) in chains.iter().enumerate() {
            let mut prefix = 0.0;
            for &(ground, task_index) in chain {
                let task = &input.tasks[task_index];
                let mut reached = 0.0;
                let mut list = Vec::new();
                for p in 0..periods {
                    reached += capacity[loader_index][p];
                    if reached <= prefix + 1e-6 || capacity[loader_index][p] <= 0.0 {
                        continue;
                    }
                    if !day_intervals[&days[p]].iter().any(|interval| task_active(task, *interval)) {
                        continue;
                    }
                    if releases.get(&ground).is_some_and(|released| *released >= day_end[p] - 1e-9) {
                        continue;
                    }
                    list.push(p);
                }
                prefix += tonnes[&ground];
                allowed.insert((loader_index, ground), list);
            }
        }

        // Each reclaim bar's piles: their destinations on their best candidate,
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
                    match routes.iter_mut().find(|route| route.destination == candidate.destination) {
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
            capacity,
            allowed,
            reclaims,
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

/// The plan for `input`, started from `seed` (the hourly dispatch's schedule)
/// when there is one, so it is never worse than the schedule it improves.
pub(crate) fn solve(input: &BlendInput, seed: Option<&BlendSolution>, window: Option<&Window>, settings: Settings, stop: &AtomicBool) -> Result<PlanSolve, String> {
    solve_periods(input, seed, window, settings, 1, stop)
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
pub(crate) fn relaxation_bound(input: &BlendInput, period_days: u32, time_limit: Duration, stop: &AtomicBool) -> Result<PlanSolve, String> {
    let settings = Settings {
        time_limit,
        relative_gap: 0.0,
        relax: true,
    };
    solve_periods(input, None, None, settings, period_days, stop)
}

fn solve_periods(input: &BlendInput, seed: Option<&BlendSolution>, window: Option<&Window>, settings: Settings, period_days: u32, stop: &AtomicBool) -> Result<PlanSolve, String> {
    let started = Instant::now();
    let prepared = Prepared::new(input, window, period_days);
    let joined = prepared.period_days.iter().any(|days| days.len() > 1);
    // Days joined, the seed would have to be mapped onto periods; nothing
    // seeds a relaxation.
    let seed = seed.filter(|_| !joined);
    let Prepared {
        days,
        period_days: covered,
        tonnes,
        pile_opening,
        outlets,
        chains,
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
            previous = Some(column);
        }
    }
    let mut block_so_far: BTreeMap<(GroundId, usize), usize> = BTreeMap::new();
    for ground in by_block.keys() {
        let Some(first) = allowed.iter().filter(|((_, block), _)| block == ground).filter_map(|(_, list)| list.first().copied()).min() else {
            continue;
        };
        for p in first..periods {
            // Within a block's own tonnes.
            let column = builder.cont(tonnes[ground], 0.0, &format!("d_{}_{p}", ground.0));
            let mut terms = vec![(column, 1.0)];
            terms.extend(
                loader_so_far
                    .range((0, *ground, p)..)
                    .filter(|((_, block, at), _)| block == ground && *at == p)
                    .map(|(_, &e)| (e, -1.0)),
            );
            builder.row(&terms, 0.0, 0.0, &format!("dsum_{}_{p}", ground.0));
            block_so_far.insert((*ground, p), column);
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

    // Finished flags for every block a chain waits on: all of it dug by the
    // end of the day, and staying finished.
    let waited: BTreeSet<GroundId> = chains.iter().flat_map(|chain| chain.windows(2).map(|pair| pair[0].0)).collect();
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
    for (loader_index, chain) in chains.iter().enumerate() {
        for pair in chain.windows(2) {
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

    // Finishing a block takes everything ahead of it in its loader's chain:
    // the loader has dug at least those tonnes and the block's own. Implied
    // by the order rows in whole numbers, but not in the relaxation, where
    // fractional flags otherwise let a sliver of every block be dug at once
    // and ore deep in a sequence look reachable on the first day. Valid while
    // no block up to it in the chain is dug by another loader too.
    let mut diggers: BTreeMap<GroundId, usize> = BTreeMap::new();
    for &(_, ground) in allowed.keys() {
        *diggers.entry(ground).or_default() += 1;
    }
    for (loader_index, chain) in chains.iter().enumerate() {
        let mut prefix = 0.0;
        for (position, &(ground, _)) in chain.iter().enumerate() {
            prefix += tonnes[&ground];
            if diggers.get(&ground) != Some(&1) {
                break;
            }
            for p in 0..periods {
                let Some(&flag) = finished.get(&(ground, p)) else { continue };
                builder.row(
                    &[(chain_so_far[&(loader_index, p)], 1.0), (flag, -prefix)],
                    0.0,
                    f64::INFINITY,
                    &format!("reach_{loader_index}_{}_{p}", ground.0),
                );
                if position > 0
                    && let Some(&ahead) = finished.get(&(chain[position - 1].0, p))
                {
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
            let mut terms: Vec<(usize, f64)> = list
                .iter()
                .map(|(v, c, of)| (*v, c * direction * (carried(*of, target.grade, direction) - boundary)))
                .collect();
            terms.push((slack, -1.0));
            builder.row(&terms, f64::NEG_INFINITY, 0.0, &format!("grade_{index}_{hinge}"));
        }
    }

    // ---- the seed, in the plan's own columns ------------------------------------
    let mut seeded = None;
    let mut seed_values: Option<Vec<f64>> = None;
    if let Some(seed) = seed {
        let day_index: BTreeMap<u32, usize> = days.iter().enumerate().map(|(p, day)| (*day, p)).collect();
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
                if cumulative >= tonnes[&ground] - 1e-4 {
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

    let build_s = started.elapsed().as_secs_f64();
    let (variables, binaries, rows) = (builder.cost.len(), builder.binaries, builder.row_lower.len());
    let solving = Instant::now();
    let answer = solve_highs(&builder, seed_values.as_deref(), settings.time_limit, settings.relative_gap, settings.relax, stop)?;
    let solve_s = solving.elapsed().as_secs_f64();

    // ---- read back ----------------------------------------------------------
    let mut dug: BTreeMap<(GroundId, usize), Vec<(usize, f64)>> = BTreeMap::new();
    for (&(loader_index, ground, p), column) in &dig {
        let value = answer.values[*column].max(0.0) * UNIT;
        if value > 1e-6 {
            dug.entry((ground, p)).or_default().push((loader_index, value));
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
        status: answer.status,
    })
}
