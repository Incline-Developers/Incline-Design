//! Improve's search: a schedule that gets better the longer it runs.
//!
//! The run's first schedule is where it starts. A daily plan over a window
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
//! When a cycle of plan windows keeps nothing, the days are polished, in
//! order, before the windows grow: the exact hourly model of a day and a
//! look-ahead past it, opening in the state the best schedule leaves, is
//! solved from that schedule's own hours; only the day is kept, and the
//! dispatch works the days after it again from where it ends. That goes
//! after what following a plan loses - the plan sees days, not hours. The
//! look-ahead is what makes it pay on long horizons: a day solved as though
//! the horizon ended with it takes value now that the days after it lose
//! more than, and the dispatch after it has to live with that. Even a day
//! ahead is too short to see why the plan strips waste now, so a polished
//! day must also leave every block exactly as the best schedule leaves it:
//! polishing rearranges the hours, the plan windows decide the strategy.
//! The ground the days after it open with is then the same, so they can
//! keep the best schedule's own hours, which the dispatch rarely matches:
//! those hours came from dispatches following earlier targets. The dispatch
//! from the day's end is tried as well, and the better kept.
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
//!
//! Cancelling the run, or its time running out, stops every solve the search
//! has under way and leaves it with the best schedule it has.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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
    plan::{self, PlanSolve, Settings, Window},
};
use crate::model::schedule::optimisation::TaskKind;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AnytimeSettings {
    pub(crate) budget: Duration,
    pub(crate) window_days: u32,
    pub(crate) step_days: u32,
    pub(crate) window_limit: Duration,
    pub(crate) bound_limit: Duration,
    /// Days per polished window, and the time each gets.
    pub(crate) polish_days: u32,
    /// Days solved past each polished window and then discarded.
    pub(crate) polish_lookahead_days: u32,
    pub(crate) polish_limit: Duration,
    /// Windows whose exact model would have more movement columns than this
    /// are not polished.
    pub(crate) polish_columns: usize,
    /// Solve the whole horizon's plan in the background for a bound.
    pub(crate) background_bound: bool,
    /// A gap to the bound at or below which there is nothing left worth
    /// searching for.
    pub(crate) closed_gap: f64,
}

/// Solves one window's exact hourly model from a schedule of it, within a
/// time limit: the window's own input and schedule, in its own interval
/// numbering, and what each block of its input (by position) must hold at
/// the end of interval `kept`. The SCIP side lives with the app's other
/// solves.
pub(crate) type Polisher<'a> = &'a (dyn Fn(&BlendInput, &BlendSolution, &[(usize, f64)], usize, Duration) -> Option<BlendSolution> + Sync);

impl AnytimeSettings {
    /// The settings Improve runs the search with, within `budget` and to the
    /// run's gap target.
    pub(crate) fn within(budget: Duration, gap_target: Option<f64>) -> Self {
        Self {
            budget,
            window_days: 5,
            step_days: 3,
            window_limit: Duration::from_secs(60),
            bound_limit: Duration::from_secs(240),
            polish_days: 1,
            polish_lookahead_days: 1,
            polish_limit: Duration::from_secs(60),
            polish_columns: 400_000,
            background_bound: true,
            closed_gap: gap_target.unwrap_or(DEFAULT_CLOSED_GAP),
        }
    }
}

/// What a search found: its best schedule and the replay's report on it, the
/// tightest bound it proved and how many improvements it kept.
pub(crate) struct Found {
    pub(crate) solution: BlendSolution,
    pub(crate) replay: ReplayReport,
    pub(crate) bound: Option<f64>,
    pub(crate) kept: usize,
}

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

/// Polish `days` days from `first_day`: their exact hourly model, with
/// `lookahead` days more solved and discarded, from `best`'s hours, then the
/// dispatch again from where the kept days end.
#[allow(clippy::too_many_arguments)]
fn polish(
    input: &BlendInput,
    best: &Best,
    first_day: u32,
    days: u32,
    lookahead: u32,
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
    let solved_to = input
        .intervals
        .iter()
        .position(|interval| interval.day() >= first_day + days + lookahead)
        .unwrap_or(input.intervals.len());
    let size = input.movements.len() * (solved_to - first) * input.segments_per_interval.max(1);
    if size > columns {
        return Err(format!("{size} movement columns"));
    }
    let mut carry = carried(input, best, first);
    let window = rolling::Window {
        first,
        committed: end - first,
        end: solved_to,
    };
    let local = carry.window_input(input, window);
    // Every block where the best schedule leaves it by the end of the kept
    // days.
    let left_by_then = Window::opening(input, &best.solution, first_day + days, 0).remaining;
    let progress: Vec<(usize, f64)> = local
        .ground
        .iter()
        .enumerate()
        .map(|(index, source)| (index, left_by_then.get(&source.id).copied().unwrap_or(0.0)))
        .collect();
    let Some(polished) = polisher(&local, &slice(best, first, solved_to), &progress, end - first - 1, limit) else {
        return Ok(None);
    };
    // The window's own objective prices its grade targets on its days
    // alone; the replay's figure is the one reported, as when stitching.
    let mut polished = polished;
    let Some(first_look) = replay_cancellable(&local, &polished, cancel) else {
        return Ok(None);
    };
    polished.reported_objective = first_look.ranked_objective();
    let Some(checked) = replay_cancellable(&local, &polished, cancel) else {
        return Ok(None);
    };
    if !checked.is_valid() {
        return Ok(None);
    }
    let count = input.intervals.len();
    if end == count {
        return Ok(stitch(input, best, first, &[(whole(first, end), &polished)], cancel));
    }
    let kept_after = stitch(input, best, first, &[(window, &polished), (whole(end, count), &slice(best, end, count))], cancel);
    carry.advance(input, window, &polished, &checked);
    let tail = carry.window_input(input, whole(end, count));
    let dispatched_after = greedy::dispatch_following(&tail, Some(&targets.rebased(&tail)), cancel)?
        .and_then(|found| stitch(input, best, first, &[(window, &polished), (whole(end, count), &found.solution)], cancel));
    Ok([kept_after, dispatched_after].into_iter().flatten().max_by(|a, b| a.value().total_cmp(&b.value())))
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

/// How long the search waits for its background plan to stop.
const BACKGROUND_GRACE: Duration = Duration::from_secs(2);

/// [`AnytimeSettings::closed_gap`] for a run with no gap target.
const DEFAULT_CLOSED_GAP: f64 = 1e-4;

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

/// Search from `start`, a schedule of `input` the replay accepted, until the
/// budget is spent, `cancel` is set or there is nothing left to find.
pub(crate) fn run(
    input: &BlendInput,
    start: (BlendSolution, ReplayReport),
    settings: AnytimeSettings,
    polisher: Option<Polisher>,
    cancel: &AtomicBool,
    mut report: impl FnMut(&Progress),
) -> Found {
    let started = Instant::now();
    let mut best = Best {
        solution: start.0,
        replay: start.1,
    };
    let mut value = best.value();
    let mut kept = 0;
    let mut record = |value: f64, bound: Option<f64>, note: String| {
        report(&Progress {
            elapsed_s: started.elapsed().as_secs_f64(),
            value,
            bound,
            note,
        });
    };
    record(value, None, "first schedule".into());

    let reclaims = input.tasks.iter().any(|task| matches!(task.kind, TaskKind::Reclaim { .. }));
    let bound = if reclaims {
        None
    } else {
        let relaxed = plan::solve(
            input,
            None,
            None,
            Settings {
                time_limit: settings.bound_limit,
                relative_gap: 0.0,
                relax: true,
            },
            cancel,
        );
        relaxed.ok().and_then(|relaxed| relaxed.bound)
    };
    record(value, bound, "relaxation bound".into());
    // Raised when the search ends, so the background plan stops with it.
    let ending = Arc::new(AtomicBool::new(false));
    let mut background = (settings.background_bound && !reclaims && !cancel.load(Ordering::Relaxed)).then(|| {
        let (input, seed, ending) = (input.clone(), best.solution.clone(), Arc::clone(&ending));
        let limit = settings.budget.saturating_sub(started.elapsed());
        std::thread::spawn(move || {
            plan::solve(
                &input,
                Some(&seed),
                None,
                Settings {
                    time_limit: limit,
                    relative_gap: 1e-4,
                    relax: false,
                },
                &ending,
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
                        if let Some(found) = redispatch(input, &best, 0, &candidate, cancel)
                            && found.value() > value + 1e-7 * value.abs().max(1.0)
                        {
                            value = found.value();
                            best = found;
                            targets = candidate;
                            polished_since = false;
                            kept += 1;
                            record(value, bound, "whole-horizon plan's targets: kept".into());
                        }
                    }
                }
                Err(reason) => record(value, bound, format!("whole-horizon plan: {reason}")),
            }
        }
        let closed = Progress {
            elapsed_s: 0.0,
            value,
            bound,
            note: String::new(),
        }
        .gap()
        .is_some_and(|gap| gap <= settings.closed_gap);
        if closed {
            record(value, bound, "gap closed".into());
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
            // In order: a day polished first can shut off what the day before
            // it would have found. How far each falls short of its plan is
            // logged.
            let earning = earned(input, &best.solution);
            let order: Vec<(f64, u32)> = days
                .iter()
                .step_by(settings.polish_days.max(1) as usize)
                .map(|day| {
                    (
                        promised.get(day).map_or(f64::NEG_INFINITY, |promise| promise - earning.get(day).copied().unwrap_or(0.0)),
                        *day,
                    )
                })
                .collect();
            for (short, day) in order {
                let left = settings.budget.saturating_sub(started.elapsed());
                if left.is_zero() || cancel.load(Ordering::Relaxed) {
                    break 'passes;
                }
                let polishing = Instant::now();
                match polish(
                    input,
                    &best,
                    day,
                    settings.polish_days.max(1),
                    settings.polish_lookahead_days,
                    &targets,
                    polisher,
                    settings.polish_limit.min(left),
                    settings.polish_columns,
                    cancel,
                ) {
                    Ok(Some(found)) if found.value() > value + 1e-7 * value.abs().max(1.0) => {
                        value = found.value();
                        best = found;
                        kept_any = true;
                        kept += 1;
                        record(
                            value,
                            bound,
                            format!("polished day {day} (short {short:.0}) in {:.1}s: kept", polishing.elapsed().as_secs_f64()),
                        );
                    }
                    Ok(Some(found)) => record(
                        value,
                        bound,
                        format!(
                            "polished day {day} (short {short:.0}) in {:.1}s: {:.0}, no better",
                            polishing.elapsed().as_secs_f64(),
                            found.value()
                        ),
                    ),
                    Ok(None) => record(
                        value,
                        bound,
                        format!("polished day {day} (short {short:.0}) in {:.1}s: no schedule", polishing.elapsed().as_secs_f64()),
                    ),
                    Err(reason) => {
                        record(value, bound, format!("days not polished: {reason}"));
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
            record(value, bound, format!("windows of {length} days, {:.0}s each", limit.as_secs_f64()));
        }
        let mut improved = false;
        let starts: Vec<usize> = if length == horizon { vec![0] } else { (offset..days.len()).step_by(step).collect() };
        for start in starts {
            let left = settings.budget.saturating_sub(started.elapsed());
            if left.is_zero() || cancel.load(Ordering::Relaxed) {
                break 'passes;
            }
            let first_day = days[start];
            let window = Window::opening(input, &best.solution, first_day, length);
            let solve = Settings {
                time_limit: limit.min(left),
                relative_gap: 1e-3,
                relax: false,
            };
            let planned = match plan::solve(input, Some(&best.solution), Some(&window), solve, cancel) {
                Ok(planned) => planned,
                Err(reason) => {
                    record(value, bound, format!("days {first_day}+: no plan ({reason})"));
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
                if started.elapsed() >= settings.budget || cancel.load(Ordering::Relaxed) {
                    break;
                }
                let candidate = targets.replacing(&covered, &planned.routes, follow * spread, overrun * spread);
                let Some(found) = redispatch(input, &best, from, &candidate, cancel) else { continue };
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
                kept += 1;
                polished_since = false;
                record(
                    value,
                    bound,
                    format!(
                        "days {first_day}..{}: kept, plan {:.0} (seed {:.0}) of {} columns ({} binary) and {} rows, built in {:.1}s and solved in {:.1}s, {}",
                        first_day + length,
                        planned.objective.unwrap_or(f64::NAN),
                        planned.seeded.unwrap_or(f64::NAN),
                        planned.variables,
                        planned.binaries,
                        planned.rows,
                        planned.build_s,
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
    // Stopped, the background plan still reports the bound it has proved.
    // HiGHS reads the stop only between steps of its search, not inside its
    // first LP, so it gets a moment; one that takes longer is left to end on
    // its own time limit, which is the search's, and its bound goes unused.
    ending.store(true, Ordering::Release);
    let waiting = Instant::now();
    while background.as_ref().is_some_and(|handle| !handle.is_finished()) && waiting.elapsed() < BACKGROUND_GRACE {
        std::thread::sleep(Duration::from_millis(20));
    }
    if let Some(Ok(whole_plan)) = finished(&mut background, false)
        && let Some(proved) = whole_plan.bound
    {
        bound = Some(bound.map_or(proved, |held: f64| held.min(proved)));
    }
    record(value, bound, "done".into());
    Found {
        solution: best.solution,
        replay: best.replay,
        bound,
        kept,
    }
}
