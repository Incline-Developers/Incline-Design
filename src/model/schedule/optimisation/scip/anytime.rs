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
//! Every candidate keeps the best schedule's hours before its window and is
//! dispatched from the state they leave.
//!
//! Schedules are ranked as the exact solve ranks them: cashflow and the
//! utilisation incentive. The plan values cashflow alone, so each of its
//! bounds has the most the incentive could pay added.
//!
//! The gap is measured against the plan's linear relaxation over the whole
//! horizon, which is an upper bound on any schedule's value while every
//! simplification of the plan is optimistic: free order within a day, each
//! route at its best value and least haul, the fleet pooled, a loader's
//! bars free of one another, each blast at its earliest release, a
//! reclaim's grade the one that suits each use of it best. Only the
//! relaxation is read that way; every other plan sets targets and bounds
//! nothing. The relaxation is solved in the background from the start,
//! beside the search rather than ahead of it: on a long horizon it can take
//! longer than the budget. So is the whole horizon's plan as a
//! mixed-integer program, for one more set of targets when it finishes.
//!
//! When the timeline the search holds takes none of a window's tails, the
//! tails are dispatched again with the drill and blast chain live, resumed
//! as the best schedule's hours leave it, so blasts move with the digging.
//! One that wins gives the search its timeline from then on.
//!
//! Cancelling the run, or its time running out, stops every solve the search
//! has under way and leaves it with the best schedule it has.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use rayon::prelude::*;
use web_time::Instant;

use super::{
    super::{
        background::{self, Background},
        blended::{
            drill_blast::{Chain, ChainState, DrillBlastTimeline, NEVER},
            greedy,
            input::{BlendInput, authored_tasks, interval_rate, task_active},
            plan::{PlanTargets, Route},
            replay::{BlendSolution, ChunkRow, MovementRow, ReplayReport, replay_as_published},
            rolling::{self, Carry, Stitched},
            routing, utilisation,
        },
    },
    plan::{self, PlanSolve, Settings, Window},
};
use crate::model::schedule::optimisation::{Activity, GroundId, SourceId, TaskKind};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AnytimeSettings {
    pub(crate) budget: Duration,
    pub(crate) window_days: u32,
    pub(crate) step_days: u32,
    pub(crate) window_limit: Duration,
    /// Solve long-term plans over the whole horizon in the background, for
    /// their targets.
    pub(crate) background_bound: bool,
    /// A gap to the bound at or below which there is nothing left worth
    /// searching for.
    pub(crate) closed_gap: f64,
}

impl AnytimeSettings {
    /// The settings Improve runs the search with, within `budget` and to the
    /// run's gap target.
    pub(crate) fn within(budget: Duration, gap_target: Option<f64>) -> Self {
        Self {
            budget,
            window_days: 5,
            step_days: 3,
            window_limit: Duration::from_secs(60),
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
    /// The drill and blast timeline the schedule keeps, when the search
    /// moved blasts off the first schedule's.
    pub(crate) drill_blast: Option<DrillBlastTimeline>,
}

/// The best schedule so far and the replay's report on it.
struct Best {
    solution: BlendSolution,
    replay: ReplayReport,
}

impl Best {
    /// What the search ranks schedules by: the replay's cashflow and the
    /// utilisation incentive, as the exact solve and the dispatch rank them.
    fn value(&self) -> f64 {
        self.replay.ranked_objective()
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

/// A tail tried: the schedule, or why it is none, `None` when it was not
/// dispatched or replayed at all.
type Tried = Result<Best, Option<String>>;

/// Time the tails spend dispatching and replaying, summed over the threads
/// that run them side by side.
#[derive(Default)]
struct Timing {
    dispatch_ns: std::sync::atomic::AtomicU64,
    replay_ns: std::sync::atomic::AtomicU64,
}

impl Timing {
    fn add(counter: &std::sync::atomic::AtomicU64, since: Instant) {
        counter.fetch_add(since.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
}

/// A hash of what a tail is dispatched to follow.
fn targets_fingerprint(targets: &PlanTargets) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for (&(loader, activity, destination, day), tonnes) in &targets.routes {
        (loader, activity, destination.0, day, tonnes.to_bits()).hash(&mut hasher);
    }
    (targets.follow.to_bits(), targets.overrun.to_bits()).hash(&mut hasher);
    for (key, value) in &targets.values {
        (key, value.to_bits()).hash(&mut hasher);
    }
    targets.planned.hash(&mut hasher);
    hasher.finish()
}

/// A hash of a schedule: its movements, durations and drill and blast
/// timeline.
fn schedule_fingerprint(solution: &BlendSolution) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for row in &solution.movements {
        (row.candidate, row.interval, row.segment, row.tonnes_t.to_bits()).hash(&mut hasher);
    }
    for (key, duration) in &solution.durations {
        (key, duration.to_bits()).hash(&mut hasher);
    }
    if let Some(timeline) = &solution.drill_blast {
        serde_json::to_string(timeline).unwrap_or_default().hash(&mut hasher);
    }
    hasher.finish()
}

/// `best`'s intervals before `first`, then `pieces` (each in its own
/// numbering), replayed against the whole horizon: `None` unless valid.
fn stitch(input: &BlendInput, best: &Best, first: usize, pieces: &[(rolling::Window, &BlendSolution)], cancel: &AtomicBool) -> Option<Best> {
    stitch_on(input, best, first, pieces, None, &Timing::default(), cancel).ok()
}

/// What `replay` rejects a schedule for, its numbers left out so that alike
/// reasons count together: the first physical rule it breaks, or else the
/// first grade condition.
fn rejection(replay: &ReplayReport) -> String {
    let issue = replay.issues.first().or(replay.grade_issues.first()).map_or("", String::as_str);
    let mut kind = String::new();
    for c in issue.chars() {
        if c.is_ascii_digit() || c == '.' || c == '-' {
            if !kind.ends_with('#') {
                kind.push('#');
            }
        } else {
            kind.push(c);
        }
    }
    kind
}

/// A replayed schedule, or why the replay rejects it.
fn accepted(solution: BlendSolution, replay: ReplayReport) -> Tried {
    if replay.is_valid() {
        Ok(Best { solution, replay })
    } else {
        Err(Some(rejection(&replay)))
    }
}

/// [`stitch`], the whole on `timeline` when the pieces moved the blasts.
fn stitch_on(
    input: &BlendInput,
    best: &Best,
    first: usize,
    pieces: &[(rolling::Window, &BlendSolution)],
    timeline: Option<DrillBlastTimeline>,
    timing: &Timing,
    cancel: &AtomicBool,
) -> Tried {
    let replaying = Instant::now();
    let mut stitched = Stitched::new();
    if first > 0 {
        stitched.keep(input, whole(0, first), &best.solution, 0.0);
    }
    for (window, solution) in pieces {
        stitched.keep(input, *window, solution, 0.0);
    }
    let mut solution = stitched.finish(input);
    // Each candidate carries the timeline it is replayed on: its own, or
    // the one the search holds.
    solution.drill_blast = timeline.or_else(|| input.drill_blast.as_ref().and_then(|chain| chain.fixed.clone()));
    // The pieces' own objectives do not add up to the whole's: grade
    // targets are priced by day over all of them. The replay's figure is
    // the one reported.
    let replay = replay_as_published(input, &mut solution, cancel);
    Timing::add(&timing.replay_ns, replaying);
    accepted(solution, replay.ok_or(None)?)
}

/// The dispatch from interval `first` to the end, following `targets`, after
/// `best`'s intervals before it.
fn redispatch(input: &BlendInput, best: &Best, first: usize, targets: &PlanTargets, timing: &Timing, cancel: &AtomicBool) -> Tried {
    let dispatching = Instant::now();
    if first == 0 {
        // Dispatched and replayed in one: counted as dispatch.
        let found = greedy::dispatch_following(input, Some(targets), cancel);
        Timing::add(&timing.dispatch_ns, dispatching);
        let found = found.map_err(Some)?.ok_or(None)?;
        return accepted(found.solution, found.replay);
    }
    let count = input.intervals.len();
    let tail = carried(input, best, first).window_input(input, whole(first, count));
    // Stitching replays the whole horizon, the tail with it.
    let found = greedy::dispatch_unreplayed(&tail, input, Some(&targets.rebased(&tail)), cancel);
    Timing::add(&timing.dispatch_ns, dispatching);
    let found = found.map_err(Some)?.ok_or(None)?;
    stitch_on(input, best, first, &[(whole(first, count), &found)], None, timing, cancel)
}

/// The drill and blast chain as `best`'s hours before interval `first` leave
/// it, simulated afresh on what they dig, as the dispatch walks it.
fn chain_at(input: &BlendInput, best: &Best, first: usize) -> Option<ChainState> {
    /// Ground below this stands no more, as the dispatch reads it.
    const STANDING_T: f64 = 1e-9;
    let mut free = input.drill_blast.clone()?;
    free.fixed = None;
    free.resume = None;
    let mut dug: Vec<Vec<(GroundId, f64)>> = vec![Vec::new(); first];
    for row in best.solution.movements.iter().filter(|row| row.interval < first) {
        if let Some(candidate) = input.movements.get(row.candidate)
            && candidate.activity == Activity::Dig
            && let SourceId::Ground(ground) = candidate.source
        {
            dug[row.interval].push((ground, row.tonnes_t));
        }
    }
    let mut left: BTreeMap<GroundId, f64> = input.ground.iter().map(|source| (source.id, source.tonnes_t)).collect();
    let mut chain = Chain::new(&free);
    for (interval, rows) in input.intervals[..first].iter().zip(&dug) {
        chain.advance(*interval, |ground| left.get(&ground).is_some_and(|tonnes| *tonnes > STANDING_T));
        for (ground, tonnes) in rows {
            if let Some(slot) = left.get_mut(ground) {
                *slot -= tonnes;
            }
        }
    }
    Some(chain.state())
}

/// [`redispatch`] with the chain live from `first`: resumed from `state`,
/// as `best`'s hours before it leave it, so the tail may move blasts.
#[allow(clippy::too_many_arguments)]
fn redispatch_live(
    input: &BlendInput,
    live: &BlendInput,
    best: &Best,
    first: usize,
    state: Option<&ChainState>,
    targets: &PlanTargets,
    timing: &Timing,
    cancel: &AtomicBool,
) -> Tried {
    let dispatching = Instant::now();
    if first == 0 {
        // Dispatched and replayed in one: counted as dispatch.
        let found = greedy::dispatch_following(live, Some(targets), cancel);
        Timing::add(&timing.dispatch_ns, dispatching);
        let found = found.ok().flatten().ok_or(None)?;
        return accepted(found.solution, found.replay);
    }
    let state = state.ok_or(None)?.clone();
    let count = input.intervals.len();
    let mut tail = carried(input, best, first).window_input(input, whole(first, count));
    if let Some(chain) = tail.drill_blast.as_mut() {
        chain.fixed = None;
        chain.resume = Some(state);
    }
    let found = greedy::dispatch_unreplayed(&tail, input, Some(&targets.rebased(&tail)), cancel);
    Timing::add(&timing.dispatch_ns, dispatching);
    let found = found.ok().flatten().ok_or(None)?;
    let timeline = found.drill_blast.clone();
    stitch_on(input, best, first, &[(whole(first, count), &found)], timeline, timing, cancel)
}

/// How `moved` fires the blasts against `held`: how many sooner, how many
/// later, and the days they moved by in all.
fn moved_blasts(held: Option<&DrillBlastTimeline>, moved: &DrillBlastTimeline) -> String {
    let (mut sooner, mut later, mut days) = (0, 0, 0.0);
    for (index, events) in moved.blasts.iter().enumerate() {
        let before = held.and_then(|held| held.blasts.get(index)).and_then(|events| events.fired_h).unwrap_or(NEVER);
        let after = events.fired_h.unwrap_or(NEVER);
        if (after - before).abs() < 1e-6 {
            continue;
        }
        if after < before {
            sooner += 1
        } else {
            later += 1
        }
        if after < NEVER && before < NEVER {
            days += (after - before).abs() / 24.0;
        }
    }
    format!("{sooner} fire sooner, {later} later, {days:.1} days moved")
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

/// What the plan windows have tried, reported when the search ends: whether
/// it found few candidates or poor ones.
#[derive(Default)]
struct Tally {
    plans: usize,
    failed: usize,
    timed_out: usize,
    /// Plans whose seed broke a row or bound of the window's model.
    seeds_broken: usize,
    columns: usize,
    rows: usize,
    solve_s: f64,
    /// Tails dispatched, those the replay accepted, and why it rejected
    /// the others.
    dispatched: usize,
    valid: usize,
    rejected: BTreeMap<String, usize>,
    /// Wall time the tails took, run side by side; and the CPU time their
    /// dispatches and replays took, summed over the threads.
    tails_s: f64,
    timing: Timing,
    /// Tails dispatched to follow targets already followed from the same
    /// schedule and interval, so bound to decode the same; valid tails that
    /// decode to the schedule they started from, or to one another tail
    /// from it decoded already.
    repeated: usize,
    unchanged: usize,
    duplicates: usize,
    /// Distinct valid tails worth the same as the schedule they started
    /// from, to the search's own tolerance, and those worth less.
    ties: usize,
    worse: usize,
    kept: usize,
    /// Over each window's best candidate: dug tonnes the plan set, the
    /// candidate dug, and dug in the same loader's bar as the plan set.
    planned_t: f64,
    executed_t: f64,
    matched_t: f64,
}

impl Tally {
    fn plan(&mut self, planned: &PlanSolve) {
        self.plans += 1;
        self.timed_out += usize::from(planned.status.contains("TimeLimit"));
        self.seeds_broken += usize::from(planned.seed_violations.is_some_and(|broken| broken > 0));
        self.columns += planned.variables;
        self.rows += planned.rows;
        self.solve_s += planned.solve_s;
    }

    /// Compare what `planned` set each loader's bars over `covered` with
    /// what `found` dug there; `bar` is each (loader, block)'s first bar.
    fn follow(&mut self, input: &BlendInput, bar: &BTreeMap<(usize, GroundId), usize>, planned: &PlanSolve, found: &BlendSolution, covered: &BTreeSet<u32>) {
        let loaders: BTreeMap<_, usize> = input.loaders.iter().enumerate().map(|(index, loader)| (loader.id, index)).collect();
        let mut set: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for (&(loader_index, ground), tonnes) in &planned.dug {
            if let Some(&task) = bar.get(&(loader_index, ground)) {
                *set.entry((loader_index, task)).or_default() += tonnes;
            }
        }
        let mut dug: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for row in &found.movements {
            let candidate = &input.movements[row.candidate];
            let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source) else {
                continue;
            };
            if !input.intervals.get(row.interval).is_some_and(|interval| covered.contains(&interval.day())) {
                continue;
            }
            let Some(&loader_index) = loaders.get(&candidate.loader) else { continue };
            if let Some(&task) = bar.get(&(loader_index, ground)) {
                *dug.entry((loader_index, task)).or_default() += row.tonnes_t;
            }
        }
        self.planned_t += set.values().sum::<f64>();
        self.executed_t += dug.values().sum::<f64>();
        self.matched_t += set.iter().map(|(key, tonnes)| tonnes.min(dug.get(key).copied().unwrap_or(0.0))).sum::<f64>();
    }

    fn summary(&self) -> String {
        let per = |total: usize| total.checked_div(self.plans).unwrap_or(0);
        let mut rejected: Vec<(&String, &usize)> = self.rejected.iter().collect();
        rejected.sort_by(|a, b| b.1.cmp(a.1));
        let rejected: Vec<String> = rejected.iter().take(3).map(|(kind, count)| format!("{count} × {kind}")).collect();
        format!(
            "windows: {} plans ({} failed, {} timed out, {} with a broken seed), {} columns and {} rows each on average, {:.1}s solving; \
             {} tails dispatched in {:.1}s wall ({:.1}s dispatching and {:.1}s replaying, CPU summed over threads), {} valid, {} kept; \
             {} repeating targets, {} unchanged, {} duplicates, {} ties, {} worse; rejected for: {}; dug by bar: {:.0} kt planned, {:.0} kt dug, {:.0} kt as planned",
            self.plans,
            self.failed,
            self.timed_out,
            self.seeds_broken,
            per(self.columns),
            per(self.rows),
            self.solve_s,
            self.dispatched,
            self.tails_s,
            self.timing.dispatch_ns.load(Ordering::Relaxed) as f64 / 1e9,
            self.timing.replay_ns.load(Ordering::Relaxed) as f64 / 1e9,
            self.valid,
            self.kept,
            self.repeated,
            self.unchanged,
            self.duplicates,
            self.ties,
            self.worse,
            if rejected.is_empty() { "nothing".to_owned() } else { rejected.join(", ") },
            self.planned_t / 1e3,
            self.executed_t / 1e3,
            self.matched_t / 1e3,
        )
    }
}

/// [`AnytimeSettings::closed_gap`] for a run with no gap target.
const DEFAULT_CLOSED_GAP: f64 = 1e-4;

/// The period length of the long-term plan solved beside the search. A
/// second plan over four days, after it, took 20-40 s on the worlds with
/// blasts and was kept on none of them; the core it took was worth more to
/// the search.
const LONG_TERM_DAYS: u32 = 7;

/// The gap a long-term plan stops at. Its answer is only targets, and
/// proving it took 40% of its solve: on Centralia the optimum was found at
/// 12 s and proved at 20 s.
const LONG_TERM_GAP: f64 = 2e-2;

/// The long-term plans solved beside the search (see [`plan::long_term`]),
/// over the schedule's ground in parcels, which a long horizon needs.
struct LongTerm {
    worker: Background<()>,
    planned: std::sync::mpsc::Receiver<(u32, Result<PlanSolve, String>)>,
    /// Plans received and not yet tried.
    pending: Vec<(u32, Result<PlanSolve, String>)>,
}

impl LongTerm {
    fn start(input: &BlendInput, first: &BlendSolution, limit: Duration, ending: &Arc<AtomicBool>) -> Option<Self> {
        let (input, first, ending) = (input.clone(), first.clone(), Arc::clone(ending));
        let (sender, planned) = std::sync::mpsc::channel();
        let worker = background::spawn("schedule-long-term", move || {
            let started = Instant::now();
            let (parcelled, candidates) = super::parcels::parcels(&input);
            // The first schedule starts it: its flags, fixed, leave the
            // solver a plan to improve on from the outset.
            let seed = super::parcels::seed(&first, &candidates);
            let left = limit.saturating_sub(started.elapsed());
            if left.is_zero() || ending.load(Ordering::Relaxed) {
                return;
            }
            let settings = Settings {
                time_limit: left,
                relative_gap: LONG_TERM_GAP,
                relax: false,
            };
            let _ = sender.send((LONG_TERM_DAYS, plan::long_term(&parcelled, Some(&seed), LONG_TERM_DAYS, settings, &ending)));
        });
        match worker {
            Ok(worker) => Some(Self {
                worker,
                planned,
                pending: Vec::new(),
            }),
            Err(error) => {
                log::warn!("the search's long-term plan was not started: {error}");
                None
            }
        }
    }

    /// Whether a plan has arrived that the search has not tried.
    fn ready(&mut self) -> bool {
        self.pending.extend(self.planned.try_iter());
        !self.pending.is_empty()
    }
}

/// A period plan's routes as daily targets, spread over the loader's
/// scheduled capacity rather than asking for deliveries while its bars are shut.
fn daily_routes(input: &BlendInput, routes: &BTreeMap<Route, f64>, periods: &BTreeMap<u32, Vec<u32>>) -> BTreeMap<Route, f64> {
    let mut capacity: BTreeMap<(usize, u32), f64> = BTreeMap::new();
    for (loader_index, loader) in input.loaders.iter().enumerate() {
        let tasks: Vec<_> = input.tasks.iter().filter(|task| task.loader == loader.id).collect();
        for interval in &input.intervals {
            let Some(rate) = interval_rate(loader, interval.index) else { continue };
            let speed = tasks
                .iter()
                .filter(|task| task_active(task, *interval))
                .map(|task| match &task.kind {
                    TaskKind::Dig { .. } => rate.dig_tph,
                    TaskKind::Reclaim { .. } => rate.reclaim_tph,
                    TaskKind::Delay => 0.0,
                })
                .fold(0.0_f64, f64::max);
            *capacity.entry((loader_index, interval.day())).or_default() += speed * interval.duration_h();
        }
    }
    let mut daily = BTreeMap::new();
    for (&(loader, activity, destination, label), &tonnes) in routes {
        let Some(span) = periods.get(&label) else { continue };
        let capacities: Vec<f64> = span.iter().map(|day| capacity.get(&(loader, *day)).copied().unwrap_or(0.0)).collect();
        let total: f64 = capacities.iter().sum();
        // Preserve uniform targets exactly when the calendar is uniform.
        let uniform = capacities.windows(2).all(|pair| (pair[0] - pair[1]).abs() <= 1e-9 * pair[0].max(pair[1]).max(1.0));
        for (index, day) in span.iter().enumerate() {
            let target = if uniform || total <= 0.0 {
                tonnes / span.len() as f64
            } else {
                tonnes * (capacities[index] / total)
            };
            *daily.entry((loader, activity, destination, *day)).or_default() += target;
        }
    }
    daily
}

/// The relaxations solved beside the search, coarsest first, and what each
/// has proved so far.
struct Relaxations {
    worker: Background<()>,
    proved: std::sync::mpsc::Receiver<(u32, Result<PlanSolve, String>)>,
    /// [`utilisation::ceiling`]: the plan values cashflow alone.
    incentive: f64,
}

impl Relaxations {
    /// Solve the whole horizon's relaxation over periods of each of
    /// [`PERIOD_DAYS`] in turn, within `limit` in all: a coarse one proves a
    /// bound in seconds on any horizon, and each finer one that finishes in
    /// time tightens it.
    fn start(input: &BlendInput, horizon_days: u32, limit: Duration, ending: &Arc<AtomicBool>) -> Option<Self> {
        let incentive = utilisation::ceiling(input);
        let (input, ending) = (input.clone(), Arc::clone(ending));
        let (sender, proved) = std::sync::mpsc::channel();
        let worker = background::spawn("schedule-relaxations", move || {
            let started = Instant::now();
            for period in PERIOD_DAYS.into_iter().filter(|period| *period == 1 || *period < horizon_days) {
                let left = limit.saturating_sub(started.elapsed());
                if left.is_zero() || ending.load(Ordering::Relaxed) {
                    return;
                }
                let solved = plan::relaxation_bound(&input, period, left, &ending);
                if sender.send((period, solved)).is_err() {
                    return;
                }
            }
        });
        match worker {
            Ok(worker) => Some(Self { worker, proved, incentive }),
            Err(error) => {
                log::warn!("the search's relaxation bound was not started: {error}");
                None
            }
        }
    }

    /// Tighten `bound` by every relaxation solved since last asked; what to
    /// report of them.
    fn take(&self, bound: &mut Option<f64>) -> Option<String> {
        let notes: Vec<String> = self
            .proved
            .try_iter()
            .map(|(period, solved)| match solved {
                Ok(relaxed) => {
                    if let Some(proved) = relaxed.bound.map(|cashflow| cashflow + self.incentive) {
                        *bound = Some(bound.map_or(proved, |held: f64| held.min(proved)));
                    }
                    format!("relaxation bound over {period}-day periods in {:.0}s, {}", relaxed.solve_s, relaxed.status)
                }
                Err(reason) => format!("relaxation bound over {period}-day periods: {reason}"),
            })
            .collect();
        (!notes.is_empty()).then(|| notes.join("; "))
    }
}

/// The period lengths the relaxation is solved over, coarsest first; see
/// [`plan::relaxation_bound`]. Over periods of a week, Centralia's 52 days
/// take 14 s, by the day they take more than five minutes.
const PERIOD_DAYS: [u32; 5] = [14, 7, 4, 2, 1];

fn relaxed(relaxations: &Option<Relaxations>, bound: &mut Option<f64>) -> Option<String> {
    relaxations.as_ref()?.take(bound)
}

/// The routing LP holds digging and durations, so intermediate steps keep
/// the same physical capacities. Reclaim grades are nonlinear, however:
/// judge a partial step rather than trusting the linearised full move.
fn routing_step(opening: &BlendSolution, proposed: &BlendSolution, fraction: f64) -> BlendSolution {
    let mut tonnes: BTreeMap<(usize, usize, usize), f64> = BTreeMap::new();
    for (solution, weight) in [(opening, 1.0 - fraction), (proposed, fraction)] {
        for row in &solution.movements {
            *tonnes.entry((row.candidate, row.interval, row.segment)).or_default() += weight * row.tonnes_t;
        }
    }
    let movements = tonnes
        .into_iter()
        .filter(|(_, tonnes)| *tonnes > 0.0)
        .map(|((candidate, interval, segment), tonnes_t)| MovementRow {
            candidate,
            interval,
            segment,
            tonnes_t,
        })
        .collect();
    BlendSolution { movements, ..opening.clone() }
}

/// `best` rerouted window by window by `reserve`'s program, each window kept
/// as it improves, so a deadline (`ends`) inside one keeps what the windows
/// before it found. What happened, for the record.
fn reroute_best(input: &BlendInput, groups: &routing::Groups, best: &mut Best, value: &mut f64, reserve: routing::Reserve, ends: Instant, cancel: &AtomicBool) -> String {
    let stop: solver::Interrupt = Arc::new(move || Instant::now() >= ends);
    let mut notes: Vec<String> = Vec::new();
    // First keep the reserved passes' gains. A wider linearised move can
    // give a worse direction even after backtracking, so try it only in the
    // time those passes leave, without cutting the digging search short.
    // Refine with smaller moves afterwards: the changed pile blends make
    // the next linearisation more accurate near its new opening schedule.
    let passes = reserve.passes + 3 * usize::from(reserve.program != routing::Program::Held);
    // Spare passes refine locally: a large drawing LP can spend the entire
    // remainder without completing even one move. Smaller routing windows
    // keep the horizon's pile chains while giving each move time to finish.
    let windows_for = |pass| reserve.windows * if pass >= reserve.passes { 2 } else { 1 };
    for (pass, index) in (0..passes).flat_map(|pass| (0..windows_for(pass)).map(move |index| (pass, index))) {
        if Instant::now() >= ends || cancel.load(Ordering::Relaxed) {
            break;
        }
        let window = routing::window(input, windows_for(pass), index);
        let extra = pass >= reserve.passes;
        let reach = if extra {
            1.0 / (pass - reserve.passes + 1) as f64
        } else if pass > 0 {
            0.3
        } else {
            0.0
        };
        let note = match routing::reroute(input, groups, &best.solution, reserve.program, reach, window.as_ref(), stop.clone()) {
            Ok(mut solution) => {
                let opening = extra.then(|| best.solution.clone());
                let proposed = opening.as_ref().map(|_| solution.clone());
                let mut step = 1.0;
                let mut judged = replay_as_published(input, &mut solution, cancel);
                if let (Some(opening), Some(proposed)) = (&opening, &proposed) {
                    for fraction in [0.5, 0.25, 0.75] {
                        if Instant::now() >= ends || cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        let mut partial = routing_step(opening, proposed, fraction);
                        if let Some(replay) = replay_as_published(input, &mut partial, cancel)
                            && replay.is_valid()
                            && judged.as_ref().is_none_or(|held| !held.is_valid() || replay.ranked_objective() > held.ranked_objective())
                        {
                            solution = partial;
                            judged = Some(replay);
                            step = fraction;
                        }
                    }
                }
                match judged {
                    Some(replay) if replay.is_valid() && replay.ranked_objective() > *value => {
                        let gain = replay.ranked_objective() - *value;
                        *value = replay.ranked_objective();
                        *best = Best { solution, replay };
                        format!("{gain:.0} better (step {step})")
                    }
                    Some(replay) => format!("not kept: valid {}, {:.0} better", replay.is_valid(), replay.ranked_objective() - *value),
                    None => "not kept: cancelled".into(),
                }
            }
            Err(error) => format!("not kept: {error}"),
        };
        notes.push(note);
        if Instant::now() >= ends {
            break;
        }
    }
    let program = match reserve.program {
        routing::Program::Held => "pile tonnes held",
        routing::Program::Moving => "pile tonnes moving",
        routing::Program::Opening => "pile tonnes moving, new cells open",
    };
    format!(
        "rerouted in {} of {} windows: {} ({program}, reserved {:.1}s)",
        notes.len(),
        (0..passes).map(windows_for).sum::<usize>(),
        notes.join("; "),
        reserve.time.as_secs_f64()
    )
}

/// Search from `start`, a schedule of `input` the replay accepted, until the
/// budget is spent, `cancel` is set or there is nothing left to find.
pub(crate) fn run(original: &BlendInput, start: (BlendSolution, ReplayReport), settings: AnytimeSettings, cancel: &Arc<AtomicBool>, mut report: impl FnMut(&Progress)) -> Found {
    let input = original;
    let started = Instant::now();
    // The search ends early enough to reroute its schedule's material over
    // the whole horizon, digging, piles and reclaim held: see `routing`.
    // Its size follows the schedule, so the time is judged again whenever
    // the search keeps a better one.
    let deadline = settings.budget;
    let groups = routing::Groups::new(input);
    let reserving = !cancel.load(Ordering::Relaxed);
    let reserve_for = |schedule: &BlendSolution| {
        if reserving {
            routing::reserve(input, &groups, schedule, deadline)
        } else {
            routing::Reserve::default()
        }
    };
    let reserve = std::cell::Cell::new(reserve_for(&start.0));
    let search_budget = || deadline.saturating_sub(reserve.get().time);
    let mut best = Best {
        solution: start.0,
        replay: start.1,
    };
    // When no tail on the timeline the search holds improves, tails with the
    // drill and blast chain simulated afresh are tried, so blasts move with
    // the digging; one that wins gives the search its timeline from then on.
    let live = input.drill_blast.is_some().then(|| {
        let mut live = input.clone();
        if let Some(chain) = live.drill_blast.as_mut() {
            chain.fixed = None;
        }
        live
    });
    let mut adopted: Option<Arc<BlendInput>> = None;
    let mut value = best.value();
    let mut kept = 0;
    let mut tally = Tally::default();
    // What tails have been dispatched to follow, from which schedule and
    // interval, and what they decoded to: (schedule, interval, live, targets)
    // and (schedule, decoded).
    let mut proposals: BTreeSet<(u64, usize, bool, u64)> = BTreeSet::new();
    let mut decoded_from: BTreeSet<(u64, u64)> = BTreeSet::new();
    // Each (loader, block)'s first bar, to compare plans and tails by bar.
    let mut bar: BTreeMap<(usize, GroundId), usize> = BTreeMap::new();
    for loader_index in 0..input.loaders.len() {
        for task in authored_tasks(input, loader_index) {
            if let TaskKind::Dig { sequence } = &input.tasks[task].kind {
                for &ground in sequence {
                    bar.entry((loader_index, ground)).or_insert(task);
                }
            }
        }
    }
    let mut record = |value: f64, bound: Option<f64>, note: String| {
        report(&Progress {
            elapsed_s: started.elapsed().as_secs_f64(),
            value,
            bound,
            note,
        });
    };
    record(value, None, "first schedule".into());

    let mut bound = None;
    // Raised when the search ends, so the background plans stop with it.
    let ending = Arc::new(AtomicBool::new(false));
    // The relaxation is solved beside the search, not ahead of it: on a long
    // horizon it can take longer than the whole budget, and the search has
    // no use for the bound until there is a gap to report.
    let horizon_days = input.intervals.iter().map(|interval| interval.day()).collect::<BTreeSet<_>>().len() as u32;
    let relaxation = (!cancel.load(Ordering::Relaxed))
        .then(|| Relaxations::start(input, horizon_days, settings.budget, &ending))
        .flatten();
    let mut long_term = (settings.background_bound && !cancel.load(Ordering::Relaxed))
        .then(|| LongTerm::start(input, &best.solution, settings.budget.saturating_sub(started.elapsed()), &ending))
        .flatten();

    let days: Vec<u32> = input.intervals.iter().map(|interval| interval.day()).collect::<BTreeSet<_>>().into_iter().collect();
    let spread = PlanTargets::value_spread(input);
    let mut targets = PlanTargets::new(input, BTreeMap::new(), spread, spread);
    let horizon = days.len() as u32;
    let mut length = settings.window_days.clamp(1, horizon);
    let mut step = (settings.step_days.max(1) as usize).min(length as usize);
    let mut limit = settings.window_limit;
    let mut offset = 0;
    let mut quiet_passes = 0;
    // Where a pass a long-term plan interrupted carries on.
    let mut resume: Option<usize> = None;

    'passes: loop {
        let held = adopted.clone();
        let input: &BlendInput = held.as_deref().unwrap_or(original);
        if let Some(note) = relaxed(&relaxation, &mut bound) {
            record(value, bound, note);
        }
        // Each long-term plan's targets for the whole horizon, dispatched on
        // the timeline the search holds and, when none of those improves,
        // with the chain live.
        let arrived = long_term.as_mut().map(|long_term| {
            long_term.ready();
            std::mem::take(&mut long_term.pending)
        });
        if let Some(arrived) = arrived.filter(|arrived| !arrived.is_empty()) {
            let all: BTreeSet<u32> = days.iter().copied().collect();
            for (period, outcome) in arrived {
                let planned = match outcome {
                    Ok(planned) => planned,
                    Err(reason) => {
                        record(value, bound, format!("long-term plan over {period}-day periods: {reason}"));
                        continue;
                    }
                };
                record(
                    value,
                    bound,
                    format!(
                        "long-term plan over {period}-day periods {:.0} in {:.1}s, {}",
                        planned.objective.unwrap_or(f64::NAN),
                        planned.solve_s,
                        planned.status
                    ),
                );
                let routes = daily_routes(input, &planned.routes, &planned.period_days);
                let mut followed = false;
                for live_round in [false, true] {
                    let Some(live) = live.as_ref().filter(|_| live_round).or((!live_round).then_some(input)) else {
                        break;
                    };
                    if followed {
                        break;
                    }
                    for (follow, overrun) in WEIGHTS {
                        let candidate = targets.replacing(&all, &routes, follow * spread, overrun * spread).valued(input);
                        let tried = if live_round {
                            redispatch_live(input, live, &best, 0, None, &candidate, &Timing::default(), cancel)
                        } else {
                            redispatch(input, &best, 0, &candidate, &Timing::default(), cancel)
                        };
                        let Ok(found) = tried else { continue };
                        if found.value() <= value + 1e-7 * value.abs().max(1.0) {
                            continue;
                        }
                        if let (Some(timeline), Some(chain)) = (found.solution.drill_blast.as_ref(), input.drill_blast.as_ref())
                            && chain.fixed.as_ref() != Some(timeline)
                        {
                            let mut moved = input.clone();
                            if let Some(chain) = moved.drill_blast.as_mut() {
                                chain.fixed = Some(timeline.clone());
                            }
                            adopted = Some(Arc::new(moved));
                            record(value, bound, format!("blasts moved: {}", moved_blasts(chain.fixed.as_ref(), timeline)));
                        }
                        value = found.value();
                        best = found;
                        reserve.set(reserve_for(&best.solution));
                        targets = candidate;
                        followed = true;
                        kept += 1;
                        record(
                            value,
                            bound,
                            format!("long-term plan's targets over {period}-day periods: kept{}", if live_round { ", chain live" } else { "" }),
                        );
                    }
                }
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
        let starts: Vec<usize> = if length == horizon {
            vec![0]
        } else {
            (resume.take().unwrap_or(offset)..days.len()).step_by(step).collect()
        };
        let mut interrupted = false;
        for start in starts {
            let left = search_budget().saturating_sub(started.elapsed());
            if left.is_zero() || cancel.load(Ordering::Relaxed) {
                break 'passes;
            }
            // A long-term plan that has arrived is tried at the top of the
            // next pass, which carries on from this window.
            if long_term.as_mut().is_some_and(|long_term| long_term.ready()) {
                resume = Some(start);
                interrupted = true;
                break;
            }
            // A window kept earlier in this pass may have moved the blasts:
            // every window plans and replays on the timeline `best` holds.
            let held = adopted.clone();
            let input: &BlendInput = held.as_deref().unwrap_or(original);
            let first_day = days[start];
            let window = Window::opening(input, &best.solution, Some(&best.replay), first_day, length);
            let from = input.intervals.iter().position(|interval| interval.day() >= first_day).unwrap_or(0);
            // The chain as the kept hours before the window leave it, once for
            // every weighting: on the timeline the best schedule was replayed
            // on, which its digging keeps to. Simulated afresh, the chain can
            // differ: a blast cleared sooner can take a machine from another,
            // which then fires after ground the schedule digs.
            let opening = match input
                .drill_blast
                .as_ref()
                .and_then(|chain| Some((chain, best.solution.drill_blast.as_ref().or(chain.fixed.as_ref())?)))
            {
                Some((chain, timeline)) => Some(timeline.state_at(chain, input.intervals[from].start_h)),
                None => chain_at(input, &best, from),
            };
            let solve = Settings {
                time_limit: limit.min(left),
                relative_gap: 1e-3,
                relax: false,
            };
            let planned = match plan::solve(input, Some(&best.solution), Some(&window), solve, cancel) {
                Ok(planned) => planned,
                Err(reason) => {
                    tally.plans += 1;
                    tally.failed += 1;
                    record(value, bound, format!("days {first_day}+: no plan ({reason})"));
                    continue;
                }
            };
            tally.plan(&planned);
            let covered: BTreeSet<u32> = (first_day..first_day + length).collect();
            // Each weight is dispatched at once, side by side; the first of the
            // best is chosen, as it would be one after another. A round is
            // the tails after the best schedule's hours on the timeline the
            // search holds, or, when none of those improves, the tails with
            // the chain live.
            let incumbent = schedule_fingerprint(&best.solution);
            let mut evaluate = |live_round: bool, tally: &mut Tally| {
                if started.elapsed() >= search_budget() || cancel.load(Ordering::Relaxed) || (live_round && live.is_none()) {
                    return None;
                }
                let dispatching = Instant::now();
                let timing = &tally.timing;
                let found: Vec<_> = WEIGHTS
                    .par_iter()
                    .map(|&(follow, overrun)| {
                        let candidate = targets.replacing(&covered, &planned.routes, follow * spread, overrun * spread);
                        let found = if live_round {
                            redispatch_live(input, live.as_ref()?, &best, from, opening.as_ref(), &candidate, timing, cancel)
                        } else {
                            redispatch(input, &best, from, &candidate, timing, cancel)
                        };
                        let proposal = targets_fingerprint(&candidate);
                        Some((found.map(|found| (found, candidate)), proposal))
                    })
                    .collect();
                tally.tails_s += dispatching.elapsed().as_secs_f64();
                let found: Vec<_> = found
                    .into_iter()
                    .flatten()
                    .map(|(found, proposal)| {
                        if !proposals.insert((incumbent, from, live_round, proposal)) {
                            tally.repeated += 1;
                        }
                        if let Ok((tail, _)) = &found {
                            let decoded = schedule_fingerprint(&tail.solution);
                            if decoded == incumbent {
                                tally.unchanged += 1;
                            } else if !decoded_from.insert((incumbent, decoded)) {
                                tally.duplicates += 1;
                            } else if (tail.value() - value).abs() <= 1e-7 * value.abs().max(1.0) {
                                tally.ties += 1;
                            } else if tail.value() < value {
                                tally.worse += 1;
                            }
                        }
                        found
                    })
                    .collect();
                tally.dispatched += found.len();
                tally.valid += found.iter().filter(|tried| tried.is_ok()).count();
                for reason in found.iter().filter_map(|tried| tried.as_ref().err().cloned().flatten()) {
                    *tally.rejected.entry(reason).or_default() += 1;
                }
                let chosen = found.into_iter().flatten().reduce(|kept, next| if next.0.value() > kept.0.value() { next } else { kept });
                if let Some((found, _)) = &chosen {
                    tally.follow(input, &bar, &planned, &found.solution, &covered);
                }
                chosen
            };
            // The chain goes live only when the held timeline takes none of
            // the tails. Where it takes one but values it lower, the live
            // tails have not been found to win, and they cost as much again.
            let mut chosen = evaluate(false, &mut tally);
            if chosen.is_none() {
                chosen = evaluate(true, &mut tally);
            }
            if let Some((found, candidate)) = chosen
                && found.value() > value + 1e-7 * value.abs().max(1.0)
            {
                // A schedule with a timeline of its own moves the search onto it.
                if let (Some(timeline), Some(chain)) = (found.solution.drill_blast.as_ref(), input.drill_blast.as_ref())
                    && chain.fixed.as_ref() != Some(timeline)
                {
                    let mut moved = input.clone();
                    if let Some(chain) = moved.drill_blast.as_mut() {
                        chain.fixed = Some(timeline.clone());
                    }
                    adopted = Some(Arc::new(moved));
                    record(value, bound, format!("blasts moved: {}", moved_blasts(chain.fixed.as_ref(), timeline)));
                }
                value = found.value();
                best = found;
                reserve.set(reserve_for(&best.solution));
                targets = candidate;
                improved = true;
                kept += 1;
                tally.kept += 1;
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
            if let Some(note) = relaxed(&relaxation, &mut bound) {
                record(value, bound, note);
            }
        }
        if interrupted {
            continue;
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
    while (long_term.as_ref().is_some_and(|long_term| !long_term.worker.is_finished()) || relaxation.as_ref().is_some_and(|relaxations| !relaxations.worker.is_finished()))
        && waiting.elapsed() < BACKGROUND_GRACE
    {
        background::pause(Duration::from_millis(20));
    }
    relaxed(&relaxation, &mut bound);
    let reserve = reserve.get();
    if !reserve.time.is_zero() && !cancel.load(Ordering::Relaxed) {
        let timed = adopted.as_deref().unwrap_or(input);
        let ends = started + deadline.saturating_sub(Duration::from_secs_f64(routing::REPLAY_S));
        let rerouting = Instant::now();
        let begun = started.elapsed().as_secs_f64();
        let note = reroute_best(timed, &groups, &mut best, &mut value, reserve, ends, cancel);
        record(value, bound, format!("{note}, began at {begun:.1}s, took {:.1}s", rerouting.elapsed().as_secs_f64()));
    }
    record(value, bound, tally.summary());
    record(value, bound, "done".into());
    Found {
        drill_blast: adopted.as_ref().and_then(|input| input.drill_blast.as_ref()).and_then(|chain| chain.fixed.clone()),
        solution: best.solution,
        replay: best.replay,
        bound,
        kept,
    }
}
