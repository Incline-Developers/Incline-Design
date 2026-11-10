//! The drill and blast chain as the solver sees it: blasts, the dozers,
//! drills and MPUs working them, and an hour-by-hour simulation of it that
//! the hourly dispatch runs beside digging.
//!
//! A blast is clear once every block above it within the buffer is dug. A
//! dozer may then prep it, a drill drill it once prepped, an MPU charge it
//! once drilled; each step uses the machine's whole rate, and machines on one
//! blast add. A charged blast fires in the first window ending after it was
//! charged, and its ground is available from that window's end. The
//! simulation needs only which ground is still standing at the start of each
//! interval, which is why it can run inside the dispatch: clearance depends
//! on what the loaders have dug, and what they may dig depends on what has
//! fired.
//!
//! Every later solve - Improve, the day-by-day windows - takes the release
//! times the dispatch found as fixed data (`releases`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::schedule::{
    BlastActivity, BlastStage,
    optimisation::{GroundId, Interval},
};

/// Work below this is none: a quantity done to within it is done.
const DONE: f64 = 1e-6;

/// The release hour of ground that never fires: finite, because capture
/// hashes this input through JSON, where infinity would become `null`.
pub(crate) const NEVER: f64 = f64::MAX;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillBlastInput {
    pub(crate) blasts: Vec<BlastJob>,
    pub(crate) agents: Vec<BlastAgent>,
    pub(crate) tasks: Vec<BlastTask>,
    /// End of the daily blast window, in hours of the day.
    pub(crate) window_end_h: f64,
    pub(crate) windows: Option<Vec<crate::model::schedule::drill_blast::BlastWindow>>,
    /// The timeline later solves keep, once the dispatch has found it.
    /// Also holds the clearance deadlines for overlying ground.
    pub(crate) fixed: Option<DrillBlastTimeline>,
    /// Where an unfixed chain starts: a tail of a horizon resumes the chain
    /// as the schedule before it left it. Never sent to a solver process.
    #[serde(skip)]
    pub(crate) resume: Option<ChainState>,
}

/// A chain partway through a horizon: what [`Chain`] carries from one
/// interval to the next.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ChainState {
    left: Vec<[f64; 3]>,
    events: Vec<BlastEvents>,
    work: Vec<BlastWork>,
}

/// Whether a blast's ground is available as a checkpoint opens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Firing {
    Fired,
    /// Charged by then, and firing at this hour: the chain records a firing
    /// as soon as charging finishes, before the window it fires in ends.
    Scheduled(f64),
    Open,
}

/// A blast as a checkpoint opens, as a plan reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BlastOpening {
    /// Work left on each step.
    pub(crate) left: [f64; 3],
    pub(crate) cleared: bool,
    /// Each step done by then. A step with no work left is not done until
    /// the one before it is.
    pub(crate) done: [bool; 3],
    pub(crate) firing: Firing,
}

/// One blast: how much of each step it needs, how far it starts, the ground
/// it releases and the ground that must be dug before it is clear.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BlastJob {
    /// Square metres to prep, metres to drill and tonnes to charge.
    pub(crate) quantity: [f64; 3],
    pub(crate) stage: BlastStage,
    pub(crate) releases: Vec<GroundId>,
    pub(crate) above: Vec<GroundId>,
    /// Ground above it that no bar digs stands for good, so it never clears.
    pub(crate) never_clear: bool,
}

/// A dozer, drill or MPU: what it does, and how much an hour in each
/// interval (zero while it is unavailable or delayed).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BlastAgent {
    pub(crate) activity: BlastActivity,
    pub(crate) rates: Vec<f64>,
}

/// One authored bar on a drill and blast machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BlastTask {
    pub(crate) agent: usize,
    pub(crate) priority: u32,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    /// Positions in [`DrillBlastInput::blasts`], in authored order.
    pub(crate) sequence: Vec<usize>,
    pub(crate) delay: bool,
    /// A follow bar: the agent whose blast bars it works, in place of a
    /// sequence of its own.
    pub(crate) follow: Option<usize>,
}

/// What the chain did: each machine's work, and each blast's milestones.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillBlastTimeline {
    pub(crate) work: Vec<BlastWork>,
    pub(crate) blasts: Vec<BlastEvents>,
}

/// A stretch of one machine working one step of one blast.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BlastWork {
    pub(crate) agent: usize,
    pub(crate) blast: usize,
    pub(crate) activity: BlastActivity,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    pub(crate) quantity: f64,
}

/// When a blast reached each milestone, `None` for never in the horizon.
/// A blast that started past a milestone has it at hour zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct BlastEvents {
    pub(crate) cleared_h: Option<f64>,
    /// Prep, drill and charge finished.
    pub(crate) done_h: [Option<f64>; 3],
    /// Its ground available: the end of the window it fired in.
    pub(crate) fired_h: Option<f64>,
}

impl DrillBlastInput {
    /// Blasts a dig bar needs that no machine will bring to firing, each with
    /// the first step left that no bar works. A blast is worked only from the
    /// sequences of machine bars, so one missing from every bar of a step's
    /// machines stands at its starting stage all horizon, and its loaders
    /// with it. A follow bar works its leader's sequence, so the leader's
    /// bars already say whether a blast is covered.
    pub(crate) fn unworked(&self) -> Vec<(usize, BlastActivity)> {
        self.blasts
            .iter()
            .enumerate()
            .filter(|(_, blast)| !blast.releases.is_empty() && blast.stage < BlastStage::Fired)
            .filter_map(|(index, blast)| {
                BlastActivity::ALL
                    .into_iter()
                    .filter(|activity| !blast.stage.has_done(*activity))
                    .find(|activity| {
                        !self
                            .tasks
                            .iter()
                            .any(|task| !task.delay && self.agents.get(task.agent).is_some_and(|agent| agent.activity == *activity) && task.sequence.contains(&index))
                    })
                    .map(|activity| (index, activity))
            })
            .collect()
    }

    /// Whether any blast's release could move: one that releases ground and
    /// is not yet charged, so work is left that machines might do sooner or
    /// later. A charged blast fires at the first window's end whatever is dug.
    pub(crate) fn movable(&self) -> bool {
        self.blasts.iter().any(|blast| !blast.releases.is_empty() && blast.stage < BlastStage::Charged)
    }

    /// The end of the first window ending after `charged_h`.
    pub(crate) fn fires_at(&self, charged_h: f64) -> f64 {
        if let Some(windows) = &self.windows {
            return windows.iter().filter_map(|window| window.next_end(charged_h)).min_by(f64::total_cmp).unwrap_or(NEVER);
        }
        let today = (charged_h / 24.0).floor() * 24.0 + self.window_end_h;
        if today > charged_h { today } else { today + 24.0 }
    }

    fn firing(&self, charged_h: f64) -> Option<f64> {
        let at = self.fires_at(charged_h);
        (at < NEVER).then_some(at)
    }

    /// When each blast's ground is available: what the dispatch found, or
    /// failing that what the starting stages alone give - fired ground at
    /// once, charged ground at the first window's end, the rest never.
    pub(crate) fn releases(&self) -> BTreeMap<GroundId, f64> {
        if let Some(timeline) = &self.fixed {
            return timeline.releases(self).into_iter().collect();
        }
        let mut releases = BTreeMap::new();
        for blast in &self.blasts {
            let at = match blast.stage {
                BlastStage::Fired => 0.0,
                BlastStage::Charged => self.fires_at(0.0),
                _ => NEVER,
            };
            for ground in &blast.releases {
                releases.insert(*ground, at);
            }
        }
        releases
    }

    /// The earliest each blast's ground could be released, whatever is dug
    /// and whatever else the machines do: each blast on its own, clear from
    /// the start, worked by every machine with a bar that could ever take it,
    /// at once and at full rate. Bar order, priorities, delay bars and other
    /// blasts are left out, since contention is not monotone (clearing one
    /// blast early can take a machine from another). No schedule finishes a
    /// step sooner, and firing windows only end later for later charges,
    /// so a bound may allow each blast this release.
    pub(crate) fn earliest_releases(&self, intervals: &[Interval]) -> BTreeMap<GroundId, f64> {
        let mut releases = BTreeMap::new();
        for (index, blast) in self.blasts.iter().enumerate() {
            let at = match blast.stage {
                BlastStage::Fired => 0.0,
                BlastStage::NotStarted if blast.never_clear => NEVER,
                _ => {
                    let mut done = 0.0;
                    for activity in BlastActivity::ALL.into_iter().filter(|activity| !blast.stage.has_done(*activity)) {
                        done = self.earliest_done(index, activity, blast.quantity[activity as usize], done, intervals);
                        if done >= NEVER {
                            break;
                        }
                    }
                    if done >= NEVER { NEVER } else { self.fires_at(done) }
                }
            };
            for ground in &blast.releases {
                releases.insert(*ground, at);
            }
        }
        releases
    }

    /// When `quantity` of `activity` on `blast`, started at `from_h`, could
    /// be done by every machine able to work it, all at once.
    fn earliest_done(&self, blast: usize, activity: BlastActivity, quantity: f64, from_h: f64, intervals: &[Interval]) -> f64 {
        if quantity <= DONE {
            return from_h;
        }
        let eligible = self.eligible(blast, activity);
        let mut left = quantity;
        for interval in intervals.iter().filter(|interval| interval.end_h > from_h + 1e-9) {
            let open = interval.start_h.max(from_h);
            let mut cuts = vec![open, interval.end_h];
            for task in &eligible {
                cuts.extend([task.start_h, task.end_h].into_iter().filter(|cut| *cut > open && *cut < interval.end_h));
            }
            cuts.sort_by(f64::total_cmp);
            for span in cuts.windows(2).filter(|span| span[1] > span[0] + 1e-9) {
                let middle = 0.5 * (span[0] + span[1]);
                let mut agents: Vec<usize> = eligible
                    .iter()
                    .filter(|task| task.start_h <= middle && middle < task.end_h)
                    .map(|task| task.agent)
                    .collect();
                agents.sort_unstable();
                agents.dedup();
                let rate: f64 = agents.iter().map(|&agent| self.agents[agent].rates.get(interval.index).copied().unwrap_or(0.0)).sum();
                if rate <= 0.0 {
                    continue;
                }
                if rate * (span[1] - span[0]) >= left - DONE {
                    return span[0] + left / rate;
                }
                left -= rate * (span[1] - span[0]);
            }
        }
        NEVER
    }

    /// The bars that could ever work `activity` on `blast`: those of a
    /// machine doing it, not delays, holding the blast or following a
    /// machine with a bar that holds it.
    fn eligible(&self, blast: usize, activity: BlastActivity) -> Vec<&BlastTask> {
        let holds = |task: &BlastTask| task.sequence.contains(&blast);
        self.tasks
            .iter()
            .filter(|task| !task.delay && self.agents.get(task.agent).is_some_and(|agent| agent.activity == activity))
            .filter(|task| match task.follow {
                None => holds(task),
                Some(leader) => self.tasks.iter().any(|lead| lead.agent == leader && !lead.delay && lead.follow.is_none() && holds(lead)),
            })
            .collect()
    }

    /// What `agent` can do over `intervals` while one of its bars that
    /// could take `blast` is open, or with no blast while any bar of its
    /// own (a delay aside) is: its rate times those hours.
    pub(crate) fn work_capacity(&self, agent: usize, blast: Option<usize>, intervals: &[Interval]) -> f64 {
        let Some(machine) = self.agents.get(agent) else { return 0.0 };
        let bars: Vec<&BlastTask> = match blast {
            Some(blast) => self.eligible(blast, machine.activity).into_iter().filter(|task| task.agent == agent).collect(),
            None => self.tasks.iter().filter(|task| task.agent == agent && !task.delay).collect(),
        };
        let mut total = 0.0;
        for interval in intervals {
            let rate = machine.rates.get(interval.index).copied().unwrap_or(0.0);
            if rate <= 0.0 {
                continue;
            }
            // The bars' union within the interval.
            let mut spans: Vec<(f64, f64)> = bars
                .iter()
                .map(|task| (task.start_h.max(interval.start_h), task.end_h.min(interval.end_h)))
                .filter(|(start, end)| end > start)
                .collect();
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut reach = f64::NEG_INFINITY;
            for (start, end) in spans {
                let start = start.max(reach);
                if end > start {
                    total += rate * (end - start);
                    reach = end;
                }
            }
        }
        total
    }

    /// Ground that must be exhausted by the clearance times later solves keep.
    pub(crate) fn clearance_deadlines(&self, timeline: &DrillBlastTimeline) -> BTreeMap<GroundId, f64> {
        let mut deadlines = BTreeMap::<GroundId, f64>::new();
        for (blast, events) in self.blasts.iter().zip(&timeline.blasts) {
            if blast.stage != BlastStage::NotStarted {
                continue;
            }
            if let Some(cleared) = events.cleared_h {
                for ground in &blast.above {
                    deadlines.entry(*ground).and_modify(|at| *at = at.min(cleared)).or_insert(cleared);
                }
            }
        }
        deadlines
    }
}

impl DrillBlastTimeline {
    /// When each blast's ground is available, [`NEVER`] for never.
    pub(crate) fn releases(&self, input: &DrillBlastInput) -> Vec<(GroundId, f64)> {
        input
            .blasts
            .iter()
            .zip(&self.blasts)
            .flat_map(|(blast, events)| blast.releases.iter().map(move |ground| (*ground, events.fired_h.unwrap_or(NEVER))))
            .collect()
    }

    /// Where this timeline leaves the chain at `at_h`, the start of an
    /// interval: its work clipped there and the milestones reached by then,
    /// as [`Chain`] would carry them once it has processed that interval's
    /// opening; a firing stays once its charge is done.
    pub(crate) fn state_at(&self, input: &DrillBlastInput, at_h: f64) -> ChainState {
        let mut left: Vec<[f64; 3]> = input
            .blasts
            .iter()
            .map(|blast| {
                let mut remaining = blast.quantity;
                for activity in BlastActivity::ALL.into_iter().filter(|activity| blast.stage.has_done(*activity)) {
                    remaining[activity as usize] = 0.0;
                }
                remaining
            })
            .collect();
        let mut work = Vec::new();
        for row in self.work.iter().filter(|row| row.start_h < at_h - 1e-9) {
            let mut clipped = *row;
            if row.end_h > at_h {
                clipped.end_h = at_h;
                clipped.quantity = row.quantity * (at_h - row.start_h) / (row.end_h - row.start_h);
            }
            if let Some(slot) = left.get_mut(row.blast) {
                let step = &mut slot[row.activity as usize];
                *step = (*step - clipped.quantity).max(0.0);
            }
            work.push(clipped);
        }
        let reached = |at: Option<f64>| at.filter(|at| *at <= at_h + 1e-9);
        // The state after `at_h`'s opening is processed: a blast cleared then,
        // on ground dug before it, is clear, as are the empty steps that
        // finished with it.
        let events = self
            .blasts
            .iter()
            .map(|events| {
                let done_h = events.done_h.map(reached);
                BlastEvents {
                    cleared_h: reached(events.cleared_h),
                    done_h,
                    fired_h: events.fired_h.filter(|_| done_h[BlastActivity::Charge as usize].is_some()),
                }
            })
            .collect();
        for slot in &mut left {
            for step in slot.iter_mut().filter(|step| **step <= DONE) {
                *step = 0.0;
            }
        }
        ChainState { left, events, work }
    }
}

impl ChainState {
    /// Each blast as the chain stands at `at_h`, its milestones read by
    /// their times: one recorded for later has not happened yet.
    pub(crate) fn checkpoint(&self, at_h: f64) -> Vec<BlastOpening> {
        let by = |at: Option<f64>| at.is_some_and(|at| at <= at_h + 1e-9);
        self.left
            .iter()
            .zip(&self.events)
            .map(|(left, events)| {
                let done = events.done_h.map(by);
                let firing = match events.fired_h {
                    Some(at) if at <= at_h + 1e-9 => Firing::Fired,
                    Some(at) if done[BlastActivity::Charge as usize] => Firing::Scheduled(at),
                    _ => Firing::Open,
                };
                BlastOpening {
                    left: *left,
                    cleared: by(events.cleared_h),
                    done,
                    firing,
                }
            })
            .collect()
    }
}

/// The chain, walked forward an interval at a time.
pub(crate) struct Chain<'a> {
    input: &'a DrillBlastInput,
    /// Work left on each step of each blast.
    left: Vec<[f64; 3]>,
    events: Vec<BlastEvents>,
    work: Vec<BlastWork>,
    /// Which blast each ground belongs to.
    owner: BTreeMap<GroundId, usize>,
}

impl<'a> Chain<'a> {
    pub(crate) fn new(input: &'a DrillBlastInput) -> Self {
        let mut left = Vec::with_capacity(input.blasts.len());
        let mut events = Vec::with_capacity(input.blasts.len());
        for blast in &input.blasts {
            let mut remaining = blast.quantity;
            let mut milestones = BlastEvents::default();
            if blast.stage > BlastStage::NotStarted {
                milestones.cleared_h = Some(0.0);
            }
            for activity in BlastActivity::ALL {
                if blast.stage.has_done(activity) {
                    remaining[activity as usize] = 0.0;
                    milestones.done_h[activity as usize] = Some(0.0);
                }
            }
            milestones.fired_h = match blast.stage {
                BlastStage::Fired => Some(0.0),
                BlastStage::Charged => input.firing(0.0),
                _ => None,
            };
            left.push(remaining);
            events.push(milestones);
        }
        if let Some(fixed) = &input.fixed {
            events.clone_from(&fixed.blasts);
        }
        let mut work = Vec::new();
        if input.fixed.is_none()
            && let Some(state) = input.resume.as_ref().filter(|state| state.left.len() == input.blasts.len())
        {
            left.clone_from(&state.left);
            events.clone_from(&state.events);
            work.clone_from(&state.work);
        }
        let owner = input
            .blasts
            .iter()
            .enumerate()
            .flat_map(|(index, blast)| blast.releases.iter().map(move |ground| (*ground, index)))
            .collect();
        Self { input, left, events, work, owner }
    }

    /// Where the chain stands, to resume a later part of the horizon from.
    pub(crate) fn state(&self) -> ChainState {
        ChainState {
            left: self.left.clone(),
            events: self.events.clone(),
            work: self.work.clone(),
        }
    }

    /// Whether `ground` may be dug at `at_h`: not part of any blast, or its
    /// blast's ground released by then.
    pub(crate) fn available(&self, ground: GroundId, at_h: f64) -> bool {
        self.owner
            .get(&ground)
            .is_none_or(|&blast| self.events[blast].fired_h.is_some_and(|fired| fired <= at_h + 1e-9))
    }

    /// Whether a blast is ready for `activity` at `at_h`: clear for prep,
    /// the step before finished by then otherwise, and this step not done.
    fn ready(&self, blast: usize, activity: BlastActivity, at_h: f64) -> bool {
        let events = &self.events[blast];
        if self.left[blast][activity as usize] <= DONE {
            return false;
        }
        let before = match activity {
            BlastActivity::Prep => events.cleared_h,
            BlastActivity::Drill => events.done_h[BlastActivity::Prep as usize],
            BlastActivity::Charge => events.done_h[BlastActivity::Drill as usize],
        };
        before.is_some_and(|done| done <= at_h + 1e-9)
    }

    /// Work one interval. `standing` says whether ground still holds material
    /// as the interval opens.
    pub(crate) fn advance(&mut self, interval: Interval, standing: impl Fn(GroundId) -> bool) {
        let input = self.input;
        if input.fixed.is_some() {
            return;
        }
        let now = interval.start_h;
        for (index, blast) in input.blasts.iter().enumerate() {
            if self.events[index].cleared_h.is_none() && !blast.never_clear && blast.above.iter().all(|ground| !standing(*ground)) {
                self.events[index].cleared_h = Some(now);
            }
        }
        // Every machine advances on the same clock. Reassign only at a bar
        // boundary or when the combined rates finish a step.
        let mut at = now;
        while at < interval.end_h - 1e-9 {
            self.finish_empty_steps(at);
            let mut until = interval.end_h;
            for task in &input.tasks {
                for boundary in [task.start_h, task.end_h] {
                    if boundary > at + 1e-9 {
                        until = until.min(boundary);
                    }
                }
            }
            let mut assigned = Vec::new();
            let mut rates = BTreeMap::<(usize, usize), f64>::new();
            for (agent_index, agent) in input.agents.iter().enumerate() {
                let rate = agent.rates.get(interval.index).copied().unwrap_or(0.0);
                if rate <= 0.0 {
                    continue;
                }
                let step = agent.activity as usize;
                let Some(task) = self.open_task(agent_index, at, step, true).filter(|task| !task.delay) else {
                    continue;
                };
                let Some(blast) = self.sequence_of(task, at, step).iter().copied().find(|&blast| self.left[blast][step] > DONE) else {
                    continue;
                };
                if self.ready(blast, agent.activity, at) {
                    assigned.push((agent_index, blast, agent.activity, rate));
                    *rates.entry((blast, step)).or_default() += rate;
                }
            }
            for (&(blast, step), &rate) in &rates {
                until = until.min(at + self.left[blast][step] / rate);
            }
            for (agent, blast, activity, rate) in assigned {
                self.record(agent, blast, activity, at, until, rate * (until - at));
            }
            for ((blast, step), rate) in rates {
                self.left[blast][step] = (self.left[blast][step] - rate * (until - at)).max(0.0);
                if self.left[blast][step] <= DONE {
                    self.left[blast][step] = 0.0;
                    self.events[blast].done_h[step] = Some(until);
                    if step == BlastActivity::Charge as usize {
                        self.events[blast].fired_h = input.firing(until);
                    }
                }
            }
            at = until;
            self.finish_empty_steps(at);
        }
    }

    /// The bar `agent` works at `at` on `step`: its highest-priority open bar
    /// with work ready there, or a delay. Without `follow`, only its own blast
    /// bars count - what a follower looks for on its leader, so a leader
    /// standing for a delay, or following someone itself, still leads.
    ///
    /// As a loader's dig bar does, a bar works its blasts in order: the first
    /// with work left on `step` is its current one, and while that is not
    /// ready - not yet clear, or its step before unfinished - the bar has no
    /// work and the machine works its next bar, coming back once it is.
    fn open_task(&self, agent: usize, at: f64, step: usize, follow: bool) -> Option<&'a BlastTask> {
        let activity = BlastActivity::ALL[step];
        self.input
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| task.agent == agent && task.start_h <= at + 1e-9 && at < task.end_h - 1e-9)
            .filter(|(_, task)| follow || (task.follow.is_none() && !task.delay))
            .filter(|(_, task)| {
                task.delay
                    || self
                        .sequence_of(task, at, step)
                        .iter()
                        .copied()
                        .find(|&blast| self.left[blast][step] > DONE)
                        .is_some_and(|blast| self.ready(blast, activity, at))
            })
            .min_by(|a, b| a.1.priority.cmp(&b.1.priority).then(a.1.start_h.total_cmp(&b.1.start_h)).then(a.0.cmp(&b.0)))
            .map(|(_, task)| task)
    }

    /// The blasts `task` works at `at` on `step`: its own sequence, or a
    /// follow bar's leader's open blast bar's.
    fn sequence_of(&self, task: &'a BlastTask, at: f64, step: usize) -> &'a [usize] {
        match task.follow {
            None => &task.sequence,
            Some(leader) => self.open_task(leader, at, step, false).map_or(&[], |task| task.sequence.as_slice()),
        }
    }

    /// Zero-quantity steps complete as soon as their predecessor does.
    fn finish_empty_steps(&mut self, at: f64) {
        for blast in 0..self.input.blasts.len() {
            for activity in BlastActivity::ALL {
                let step = activity as usize;
                let before = if step == 0 { self.events[blast].cleared_h } else { self.events[blast].done_h[step - 1] };
                if self.left[blast][step] <= DONE && self.events[blast].done_h[step].is_none() && before.is_some_and(|done| done <= at + 1e-9) {
                    self.events[blast].done_h[step] = Some(at);
                    if activity == BlastActivity::Charge {
                        self.events[blast].fired_h = self.input.firing(at);
                    }
                }
            }
        }
    }

    /// Add a stretch of work, joining it to the machine's last stretch on
    /// the same step of the same blast when it carries straight on.
    fn record(&mut self, agent: usize, blast: usize, activity: BlastActivity, start_h: f64, end_h: f64, quantity: f64) {
        if let Some(last) = self.work.iter_mut().rev().find(|row| row.agent == agent).filter(|row| {
            row.blast == blast
                && row.activity == activity
                && (row.end_h - start_h).abs() < 1e-6
                && ((row.quantity / (row.end_h - row.start_h)) - quantity / (end_h - start_h)).abs() < 1e-9
        }) {
            last.end_h = end_h;
            last.quantity += quantity;
            return;
        }
        self.work.push(BlastWork {
            agent,
            blast,
            activity,
            start_h,
            end_h,
            quantity,
        });
    }

    pub(crate) fn finish(self) -> DrillBlastTimeline {
        self.input.fixed.clone().unwrap_or(DrillBlastTimeline {
            work: self.work,
            blasts: self.events,
        })
    }
}
