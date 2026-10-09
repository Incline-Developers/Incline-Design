//! An Improve that gets better the longer it runs (prototype).
//!
//! The hourly dispatch gives the first schedule. A daily plan over a window
//! of days, started from the state the best schedule so far leaves as the
//! window opens and seeded with that schedule's own days, sets targets for
//! those days; the dispatch works the whole horizon again following every
//! target kept so far, and the replay's value decides whether it is kept.
//! The window slides through the horizon, pass after pass. When a whole
//! cycle of passes keeps nothing, the windows double in length, and their
//! time with them, up to the whole horizon, which is then solved again with
//! ever more time until the budget is spent. Every kept schedule is a
//! replayed one, so the value only rises.
//!
//! The gap is measured against the plan's linear relaxation over the whole
//! horizon, which is an upper bound on any schedule's value while every
//! simplification of the plan is optimistic: free order within a day, each
//! route at its best candidate, the fleet pooled. Reclaim is not planned, so
//! with stockpiles to reclaim it is no bound, and none is reported. Once a
//! window is the whole horizon, the plan's own dual bound is one as well,
//! and the tighter of the two is kept.
#![allow(dead_code, reason = "prototype, not yet called by Improve")]

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use super::{
    super::blended::{greedy, input::BlendInput, plan::PlanTargets, replay::BlendSolution},
    plan::{self, Backend, Settings, Window},
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

pub(crate) fn run(input: &BlendInput, settings: AnytimeSettings, mut report: impl FnMut(&Progress)) -> Result<(BlendSolution, Vec<Progress>), String> {
    let started = Instant::now();
    let cancel = AtomicBool::new(false);
    let first = greedy::dispatch_cancellable(input, &cancel)?.ok_or("cancelled")?;
    if !first.replay.is_valid() {
        return Err("the hourly dispatch schedule was rejected".into());
    }
    let mut best = first.solution;
    let mut value = first.replay.replayed_objective;
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
    'passes: loop {
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
            let window = Window::opening(input, &best, first_day, length);
            let solve = Settings {
                backend: settings.backend,
                time_limit: limit.min(left),
                relative_gap: 1e-3,
                relax: false,
            };
            let planned = match plan::solve(input, Some(&best), Some(&window), solve) {
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
            let covered: BTreeSet<u32> = (first_day..first_day + length).collect();
            let mut chosen: Option<(f64, BlendSolution, PlanTargets)> = None;
            for (follow, overrun) in WEIGHTS {
                if started.elapsed() >= settings.budget {
                    break;
                }
                let candidate = targets.replacing(&covered, &planned.routes, follow * spread, overrun * spread);
                let Ok(Some(found)) = greedy::dispatch_following(input, Some(&candidate), &cancel) else {
                    continue;
                };
                let worth = found.replay.replayed_objective;
                if found.replay.is_valid() && chosen.as_ref().is_none_or(|(kept, _, _)| worth > *kept) {
                    chosen = Some((worth, found.solution, candidate));
                }
            }
            if let Some((worth, solution, candidate)) = chosen
                && worth > value + 1e-7 * value.abs().max(1.0)
            {
                value = worth;
                best = solution;
                targets = candidate;
                improved = true;
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
            // The whole horizon again, with more time, until the budget.
            quiet_passes = 0;
            limit *= 2;
            record(&mut log, value, bound, format!("whole horizon, {:.0}s", limit.as_secs_f64()));
        }
    }
    record(&mut log, value, bound, "done".into());
    Ok((best, log))
}
