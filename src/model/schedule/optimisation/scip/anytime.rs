//! An Improve that gets better the longer it runs (prototype).
//!
//! The hourly dispatch gives the first schedule. A daily plan over a window
//! of days, started from the state the best schedule so far leaves as the
//! window opens and seeded with that schedule's own days, sets targets for
//! those days; the dispatch works the whole horizon again following every
//! target kept so far, and the replay's value decides whether it is kept.
//! The window slides through the horizon, pass after pass. When a whole
//! cycle of passes keeps nothing, the windows double in length, and their
//! time with them, up to the whole horizon, which is solved again until a
//! whole-horizon plan keeps nothing more, the gap is closed or the budget is
//! spent. Every kept schedule is a replayed one, so the value only rises.
//!
//! When a cycle of plan windows keeps nothing, the days are polished, those
//! where the schedule earns least against what their plan promised first,
//! before the windows grow: the exact hourly model of a day, opening in the
//! state the best schedule leaves, is solved from that schedule's own hours,
//! and the dispatch works the days after it again from where it ends. That
//! goes after what following a plan loses - the plan sees days, not hours.
//!
//! Every candidate keeps the best schedule's hours before its window and is
//! dispatched from the state they leave, so a polished day is not undone by
//! a later plan window starting after it.
//!
//! The gap is measured against the plan's linear relaxation over the whole
//! horizon, which is an upper bound on any schedule's value while every
//! simplification of the plan is optimistic: free order within a day, each
//! route at its best candidate, the fleet pooled. Reclaim is not planned, so
//! with stockpiles to reclaim it is no bound, and none is reported. Once a
//! window is the whole horizon, the plan's own dual bound is one as well,
//! and the tighter of the two is kept. The whole horizon's plan is also
//! solved as a mixed-integer program in the background from the start, for
//! its dual bound and for one more set of targets when it finishes.
#![allow(dead_code, reason = "prototype, not yet called by Improve")]

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
    thread::JoinHandle,
    time::{Duration, Instant},
};

use super::{
    super::blended::{
        greedy,
        input::BlendInput,
        plan::PlanTargets,
        replay::{BlendSolution, ChunkRow, MovementRow, ReplayReport, replay_cancellable},
        rolling::{self, Carry, Stitched},
    },
    plan::{self, Backend, PlanSolve, Settings, Window},
};
use crate::model::schedule::optimisation::TaskKind;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AnytimeSettings {
    pub(crate) budget: Duration,
    pub(crate) window_days: u32,
    pub(crate) step_days: u32,
    pub(crate) window_limit: Duration,
    pub(crate) bound_limit: Duration,
    pub(crate) backend: Backend,
    /// Days per polished window, and the time each gets.
    pub(crate) polish_days: u32,
    pub(crate) polish_limit: Duration,
    /// Windows whose exact model would have more movement columns than this
    /// are not polished.
    pub(crate) polish_columns: usize,
    /// Solve the whole horizon's plan in the background for a bound.
    pub(crate) background_bound: bool,
}

/// Solves one window's exact hourly model from a schedule of it, within a
/// time limit: the window's own input and schedule, in its own interval
/// numbering. The SCIP side lives with the app's other solves.
pub(crate) type Polisher<'a> = &'a (dyn Fn(&BlendInput, &BlendSolution, Duration) -> Option<BlendSolution> + Sync);

/// The best schedule so far and the replay's report on it.
struct Best {
    solution: BlendSolution,
    replay: ReplayReport,
}

impl Best {
    fn value(&self) -> f64 {
        self.replay.replayed_objective
    }
}

/// The state `best` leaves as interval `first` opens.
fn carried(input: &BlendInput, best: &Best, first: usize) -> Carry {
    let mut carry = Carry::opening(input);
    if first > 0 {
        carry.advance(input, whole(0, first), &best.solution, &best.replay);
    }
    carry
}

fn whole(first: usize, end: usize) -> rolling::Window {
    rolling::Window {
        first,
        committed: end - first,
        end,
    }
}

/// `best`'s intervals before `first`, then `pieces` (each in its own
/// numbering), replayed against the whole horizon: `None` unless valid.
fn stitch(input: &BlendInput, best: &Best, first: usize, pieces: &[(rolling::Window, &BlendSolution)], cancel: &AtomicBool) -> Option<Best> {
    let mut stitched = Stitched::new();
    if first > 0 {
        stitched.keep(input, whole(0, first), &best.solution, 0.0);
    }
    for (window, solution) in pieces {
        stitched.keep(input, *window, solution, 0.0);
    }
    let mut solution = stitched.finish(input);
    // The pieces' own objectives do not add up to the whole's: grade
    // targets are priced by day over all of them. The replay's figure is
    // the one reported.
    let first_look = replay_cancellable(input, &solution, cancel)?;
    solution.reported_objective = first_look.ranked_objective();
    let replay = replay_cancellable(input, &solution, cancel)?;
    replay.is_valid().then_some(Best { solution, replay })
}

/// The dispatch from interval `first` to the end, following `targets`, after
/// `best`'s intervals before it.
fn redispatch(input: &BlendInput, best: &Best, first: usize, targets: &PlanTargets, cancel: &AtomicBool) -> Option<Best> {
    if first == 0 {
        let found = greedy::dispatch_following(input, Some(targets), cancel).ok()??;
        return found.replay.is_valid().then_some(Best {
            solution: found.solution,
            replay: found.replay,
        });
    }
    let count = input.intervals.len();
    let tail = carried(input, best, first).window_input(input, whole(first, count));
    let found = greedy::dispatch_following(&tail, Some(&targets.rebased(&tail)), cancel).ok()??;
    stitch(input, best, first, &[(whole(first, count), &found.solution)], cancel)
}

/// `best`'s rows in intervals `first..end`, renumbered from `first`.
fn slice(best: &Best, first: usize, end: usize) -> BlendSolution {
    let range = first..end;
    BlendSolution {
        movements: best
            .solution
            .movements
            .iter()
            .filter(|row| range.contains(&row.interval))
            .map(|row| MovementRow {
                interval: row.interval - first,
                ..*row
            })
            .collect(),
        durations: best
            .solution
            .durations
            .iter()
            .filter(|((interval, _), _)| range.contains(interval))
            .map(|(&(interval, segment), &duration)| ((interval - first, segment), duration))
            .collect(),
        chunks: best
            .solution
            .chunks
            .iter()
            .filter(|row| range.contains(&row.interval))
            .map(|row| ChunkRow {
                interval: row.interval - first,
                ..row.clone()
            })
            .collect(),
        reported_objective: 0.0,
        adjustments: Default::default(),
        drill_blast: None,
    }
}

/// Polish `days` days from `first_day`: their exact hourly model from
/// `best`'s hours, then the dispatch again from where they end.
#[allow(clippy::too_many_arguments)]
fn polish(
    input: &BlendInput,
    best: &Best,
    first_day: u32,
    days: u32,
    targets: &PlanTargets,
    polisher: Polisher,
    limit: Duration,
    columns: usize,
    cancel: &AtomicBool,
) -> Result<Option<Best>, String> {
    let first = input.intervals.iter().position(|interval| interval.day() >= first_day).ok_or("past the horizon")?;
    let end = input
        .intervals
        .iter()
        .position(|interval| interval.day() >= first_day + days)
        .unwrap_or(input.intervals.len());
    let size = input.movements.len() * (end - first) * input.segments_per_interval.max(1);
    if size > columns {
        return Err(format!("{size} movement columns"));
    }
    let mut carry = carried(input, best, first);
    let window = whole(first, end);
    let local = carry.window_input(input, window);
    let Some(polished) = polisher(&local, &slice(best, first, end), limit) else {
        return Ok(None);
    };
    let Some(checked) = replay_cancellable(&local, &polished, cancel) else {
        return Ok(None);
    };
    if !checked.is_valid() {
        return Ok(None);
    }
    let count = input.intervals.len();
    if end == count {
        return Ok(stitch(input, best, first, &[(window, &polished)], cancel));
    }
    carry.advance(input, window, &polished, &checked);
    let tail = carry.window_input(input, whole(end, count));
    let Some(found) = greedy::dispatch_following(&tail, Some(&targets.rebased(&tail)), cancel)? else {
        return Ok(None);
    };
    Ok(stitch(input, best, first, &[(window, &polished), (whole(end, count), &found.solution)], cancel))
}

#[derive(Clone, Debug)]
pub(crate) struct Progress {
    pub(crate) elapsed_s: f64,
    pub(crate) value: f64,
    pub(crate) bound: Option<f64>,
    pub(crate) note: String,
}

impl Progress {
    pub(crate) fn gap(&self) -> Option<f64> {
        self.bound.filter(|bound| *bound > 0.0).map(|bound| (bound - self.value).max(0.0) / bound)
    }
}

/// Target weights tried for each window, relative to the spread of values
/// per tonne: (follow, overrun).
const WEIGHTS: [(f64, f64); 3] = [(1.0, 0.0), (1.0, 1.0), (2.0, 2.0)];

/// A gap at or below which there is nothing left worth searching for.
const CLOSED_GAP: f64 = 1e-4;

/// What `solution` earns from its movements on each day.
fn earned(input: &BlendInput, solution: &BlendSolution) -> BTreeMap<u32, f64> {
    let mut earned: BTreeMap<u32, f64> = BTreeMap::new();
    for row in &solution.movements {
        if let (Some(interval), Some(candidate)) = (input.intervals.get(row.interval), input.movements.get(row.candidate)) {
            *earned.entry(interval.day()).or_default() += row.tonnes_t * candidate.value_per_tonne().unwrap_or(0.0);
        }
    }
    earned
}

/// The background plan, once it has finished; waited for when `wait`.
fn finished(background: &mut Option<JoinHandle<Result<PlanSolve, String>>>, wait: bool) -> Option<Result<PlanSolve, String>> {
    if background.as_ref().is_some_and(|handle| wait || handle.is_finished()) {
        return background.take().map(|handle| handle.join().unwrap_or_else(|_| Err("the background plan panicked".into())));
    }
    None
}

pub(crate) fn run(input: &BlendInput, settings: AnytimeSettings, polisher: Option<Polisher>, mut report: impl FnMut(&Progress)) -> Result<(BlendSolution, Vec<Progress>), String> {
    let started = Instant::now();
    let cancel = AtomicBool::new(false);
    let first = greedy::dispatch_cancellable(input, &cancel)?.ok_or("cancelled")?;
    if !first.replay.is_valid() {
        return Err("the hourly dispatch schedule was rejected".into());
    }
    let mut best = Best {
        solution: first.solution,
        replay: first.replay,
    };
    let mut value = best.value();
    let mut log = Vec::new();
    let mut record = |log: &mut Vec<Progress>, value: f64, bound: Option<f64>, note: String| {
        let progress = Progress {
            elapsed_s: started.elapsed().as_secs_f64(),
            value,
            bound,
            note,
        };
        report(&progress);
        log.push(progress);
    };
    record(&mut log, value, None, "hourly dispatch".into());

    let reclaims = input.tasks.iter().any(|task| matches!(task.kind, TaskKind::Reclaim { .. }));
    let bound = if reclaims {
        None
    } else {
        let relaxed = plan::solve(
            input,
            None,
            None,
            Settings {
                backend: Backend::Highs,
                time_limit: settings.bound_limit,
                relative_gap: 0.0,
                relax: true,
            },
        );
        relaxed.ok().and_then(|relaxed| relaxed.bound)
    };
    record(&mut log, value, bound, "relaxation bound".into());
    let mut background = (settings.background_bound && !reclaims).then(|| {
        let (input, seed) = (input.clone(), best.solution.clone());
        let limit = settings.budget.saturating_sub(started.elapsed()).mul_f64(0.9);
        std::thread::spawn(move || {
            plan::solve(
                &input,
                Some(&seed),
                None,
                Settings {
                    backend: Backend::Highs,
                    time_limit: limit,
                    relative_gap: 1e-4,
                    relax: false,
                },
            )
        })
    });
    // What the latest plan of each day promised from its movements.
    let mut promised: BTreeMap<u32, f64> = BTreeMap::new();

    let days: Vec<u32> = input.intervals.iter().map(|interval| interval.day()).collect::<BTreeSet<_>>().into_iter().collect();
    let spread = PlanTargets::value_spread(input);
    let mut targets = PlanTargets::new(input, BTreeMap::new(), spread, spread);
    let horizon = days.len() as u32;
    let mut bound = bound;
    let mut length = settings.window_days.clamp(1, horizon);
    let mut step = (settings.step_days.max(1) as usize).min(length as usize);
    let mut limit = settings.window_limit;
    let mut offset = 0;
    let mut quiet_passes = 0;
    // Whether the days have been polished since a plan window was last kept.
    let mut polished_since = false;
    'passes: loop {
        if let Some(outcome) = finished(&mut background, false) {
            match outcome {
                Ok(whole_plan) => {
                    if let Some(proved) = whole_plan.bound {
                        bound = Some(bound.map_or(proved, |held: f64| held.min(proved)));
                    }
                    record(
                        &mut log,
                        value,
                        bound,
                        format!(
                            "whole-horizon plan {:.0} in {:.0}s, {}",
                            whole_plan.objective.unwrap_or(f64::NAN),
                            whole_plan.solve_s,
                            whole_plan.status
                        ),
                    );
                    let all: BTreeSet<u32> = days.iter().copied().collect();
                    for (follow, overrun) in WEIGHTS {
                        let candidate = targets.replacing(&all, &whole_plan.routes, follow * spread, overrun * spread);
                        if let Some(found) = redispatch(input, &best, 0, &candidate, &cancel)
                            && found.value() > value + 1e-7 * value.abs().max(1.0)
                        {
                            value = found.value();
                            best = found;
                            targets = candidate;
                            polished_since = false;
                            record(&mut log, value, bound, "whole-horizon plan's targets: kept".into());
                        }
                    }
                }
                Err(reason) => record(&mut log, value, bound, format!("whole-horizon plan: {reason}")),
            }
        }
        let closed = Progress {
            elapsed_s: 0.0,
            value,
            bound,
            note: String::new(),
        }
        .gap()
        .is_some_and(|gap| gap <= CLOSED_GAP);
        if closed {
            record(&mut log, value, bound, "gap closed".into());
            break;
        }
        if quiet_passes >= step
            && let Some(polisher) = polisher
            && !polished_since
        {
            // A whole cycle of plan windows kept nothing: polish each day
            // before the windows grow.
            polished_since = true;
            let mut kept_any = false;
            // The days furthest short of their plan first; days no plan has
            // covered last, in order.
            let earning = earned(input, &best.solution);
            let mut order: Vec<(f64, u32)> = days
                .iter()
                .step_by(settings.polish_days.max(1) as usize)
                .map(|day| {
                    (
                        promised.get(day).map_or(f64::NEG_INFINITY, |promise| promise - earning.get(day).copied().unwrap_or(0.0)),
                        *day,
                    )
                })
                .collect();
            order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for (short, day) in order {
                let left = settings.budget.saturating_sub(started.elapsed());
                if left.is_zero() {
                    break 'passes;
                }
                let polishing = Instant::now();
                match polish(
                    input,
                    &best,
                    day,
                    settings.polish_days.max(1),
                    &targets,
                    polisher,
                    settings.polish_limit.min(left),
                    settings.polish_columns,
                    &cancel,
                ) {
                    Ok(Some(found)) if found.value() > value + 1e-7 * value.abs().max(1.0) => {
                        value = found.value();
                        best = found;
                        kept_any = true;
                        record(
                            &mut log,
                            value,
                            bound,
                            format!("polished day {day} (short {short:.0}) in {:.1}s: kept", polishing.elapsed().as_secs_f64()),
                        );
                    }
                    Ok(Some(found)) => record(
                        &mut log,
                        value,
                        bound,
                        format!(
                            "polished day {day} (short {short:.0}) in {:.1}s: {:.0}, no better",
                            polishing.elapsed().as_secs_f64(),
                            found.value()
                        ),
                    ),
                    Ok(None) => record(
                        &mut log,
                        value,
                        bound,
                        format!("polished day {day} (short {short:.0}) in {:.1}s: no schedule", polishing.elapsed().as_secs_f64()),
                    ),
                    Err(reason) => {
                        record(&mut log, value, bound, format!("days not polished: {reason}"));
                        break;
                    }
                }
                if let Some(outcome) = finished(&mut background, false) {
                    // Picked up at the top of the next pass.
                    background = Some(std::thread::spawn(move || outcome));
                    break;
                }
            }
            if kept_any {
                quiet_passes = 0;
                continue;
            }
        }
        if quiet_passes >= step {
            // A whole cycle of offsets kept nothing: longer windows.
            length = (length * 2).min(horizon);
            step = ((length as usize * 3) / 5).max(1);
            limit *= 2;
            quiet_passes = 0;
            offset = 0;
            record(&mut log, value, bound, format!("windows of {length} days, {:.0}s each", limit.as_secs_f64()));
        }
        let mut improved = false;
        let starts: Vec<usize> = if length == horizon { vec![0] } else { (offset..days.len()).step_by(step).collect() };
        for start in starts {
            let left = settings.budget.saturating_sub(started.elapsed());
            if left.is_zero() {
                break 'passes;
            }
            let first_day = days[start];
            let window = Window::opening(input, &best.solution, first_day, length);
            let solve = Settings {
                backend: settings.backend,
                time_limit: limit.min(left),
                relative_gap: 1e-3,
                relax: false,
            };
            let planned = match plan::solve(input, Some(&best.solution), Some(&window), solve) {
                Ok(planned) => planned,
                Err(reason) => {
                    record(&mut log, value, bound, format!("days {first_day}+: no plan ({reason})"));
                    continue;
                }
            };
            // The whole horizon from its first day is the plan itself, whose
            // dual bound bounds every schedule.
            if length == horizon
                && start == 0
                && !reclaims
                && let Some(proved) = planned.bound
            {
                bound = Some(bound.map_or(proved, |held: f64| held.min(proved)));
            }
            promised.extend(planned.day_values.iter().map(|(day, earned)| (*day, *earned)));
            let covered: BTreeSet<u32> = (first_day..first_day + length).collect();
            let from = input.intervals.iter().position(|interval| interval.day() >= first_day).unwrap_or(0);
            let mut chosen: Option<(Best, PlanTargets)> = None;
            for (follow, overrun) in WEIGHTS {
                if started.elapsed() >= settings.budget {
                    break;
                }
                let candidate = targets.replacing(&covered, &planned.routes, follow * spread, overrun * spread);
                let Some(found) = redispatch(input, &best, from, &candidate, &cancel) else { continue };
                if chosen.as_ref().is_none_or(|(kept, _)| found.value() > kept.value()) {
                    chosen = Some((found, candidate));
                }
            }
            if let Some((found, candidate)) = chosen
                && found.value() > value + 1e-7 * value.abs().max(1.0)
            {
                value = found.value();
                best = found;
                targets = candidate;
                improved = true;
                polished_since = false;
                record(
                    &mut log,
                    value,
                    bound,
                    format!(
                        "days {first_day}..{}: kept, plan {:.0} (seed {:.0}) in {:.1}s, {}",
                        first_day + length,
                        planned.objective.unwrap_or(f64::NAN),
                        planned.seeded.unwrap_or(f64::NAN),
                        planned.solve_s,
                        planned.status
                    ),
                );
            }
        }
        quiet_passes = if improved { 0 } else { quiet_passes + 1 };
        offset = (offset + 1) % step;
        if length == horizon {
            if !improved {
                // Planned again from the same schedule, it would plan the
                // same.
                break;
            }
            quiet_passes = 0;
        }
    }
    if let Some(Ok(whole_plan)) = finished(&mut background, true)
        && let Some(proved) = whole_plan.bound
    {
        bound = Some(bound.map_or(proved, |held: f64| held.min(proved)));
    }
    record(&mut log, value, bound, "done".into());
    Ok((best.solution, log))
}
