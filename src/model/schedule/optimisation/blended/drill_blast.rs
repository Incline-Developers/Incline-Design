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
//! times the dispatch found as fixed data (`releases`); see
//! `docs/activity-sequencing-plan.md`, D7.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::schedule::{
    BlastActivity, BlastStage,
    optimisation::{GroundId, Interval},
};

/// Work below this is none: a quantity done to within it is done.
const DONE: f64 = 1e-6;

/// The release hour of ground that never fires: finite, so it survives the
/// solver process's JSON.
pub(crate) const NEVER: f64 = f64::MAX;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillBlastInput {
    pub(crate) blasts: Vec<BlastJob>,
    pub(crate) agents: Vec<BlastAgent>,
    pub(crate) tasks: Vec<BlastTask>,
    /// End of the daily blast window, in hours of the day.
    pub(crate) window_end_h: f64,
    #[serde(default)]
    pub(crate) windows: Option<Vec<crate::model::schedule::drill_blast::BlastWindow>>,
    /// The timeline later solves keep, once the dispatch has found it.
    /// Also holds the clearance deadlines for overlying ground.
    #[serde(default)]
    pub(crate) fixed: Option<DrillBlastTimeline>,
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
    #[serde(default)]
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
    #[serde(default)]
    pub(crate) delay: bool,
    /// A follow bar: the agent whose blast bars it works, in place of a
    /// sequence of its own.
    #[serde(default)]
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
        let owner = input
            .blasts
            .iter()
            .enumerate()
            .flat_map(|(index, blast)| blast.releases.iter().map(move |ground| (*ground, index)))
            .collect();
        Self {
            input,
            left,
            events,
            work: Vec::new(),
            owner,
        }
    }

    /// Whether `ground` may be dug at `at_h`: not part of any blast, or its
    /// blast's ground released by then.
    pub(crate) fn available(&self, ground: GroundId, at_h: f64) -> bool {
        self.owner
            .get(&ground)
            .is_none_or(|&blast| self.events[blast].fired_h.is_some_and(|fired| fired <= at_h + 1e-9))
    }

    /// The blast `ground` belongs to, if any.
    pub(crate) fn blast_of(&self, ground: GroundId) -> Option<usize> {
        self.owner.get(&ground).copied()
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
    /// with work left there, or a delay. Without `follow`, only its own blast
    /// bars count - what a follower looks for on its leader, so a leader
    /// standing for a delay, or following someone itself, still leads.
    fn open_task(&self, agent: usize, at: f64, step: usize, follow: bool) -> Option<&'a BlastTask> {
        self.input
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| task.agent == agent && task.start_h <= at + 1e-9 && at < task.end_h - 1e-9)
            .filter(|(_, task)| follow || (task.follow.is_none() && !task.delay))
            .filter(|(_, task)| task.delay || self.sequence_of(task, at, step).iter().any(|&blast| self.left[blast][step] > DONE))
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
