//! The drill and blast chain as the plan sees it: per blast still to fire,
//! where it stands as the plan opens, the ground above it that must be dug
//! before it is clear, the ground it releases, and what each machine able
//! to work it could do in each period. See
//! `docs/scheduling-blast-aware-plan-design.md`.

use std::collections::{BTreeMap, BTreeSet};

use super::super::blended::drill_blast::{BlastOpening, Chain, DrillBlastInput, Firing};
use crate::model::schedule::{
    BlastActivity,
    optimisation::{GroundId, Interval},
};

/// One blast the plan decides: not fired by the time it opens, with ground
/// left to release.
pub(super) struct PlannedBlast {
    /// Its position in [`DrillBlastInput::blasts`].
    pub(super) index: usize,
    pub(super) opening: BlastOpening,
    /// The period its ground is available from, when it is charged and
    /// scheduled to fire as the plan opens.
    pub(super) fires_in: Option<usize>,
    /// The standing ground above it, when it is not clear yet.
    pub(super) above: Vec<GroundId>,
    /// Ground above it that no loader's chain digs: it never clears.
    pub(super) never_clear: bool,
    /// The standing ground it releases.
    pub(super) releases: Vec<GroundId>,
    /// Each machine that could work one of its steps still to do, with what
    /// it could do on it in each period.
    pub(super) machines: Vec<(usize, Vec<f64>)>,
}

/// The plan's drill and blast chain.
pub(super) struct BlastModel {
    pub(super) blasts: Vec<PlannedBlast>,
    /// What each machine could do in each period while any of its bars is
    /// open.
    pub(super) capacity: Vec<Vec<f64>>,
    /// Whether a firing window ends within each period.
    pub(super) fires: Vec<bool>,
    activities: Vec<BlastActivity>,
}

impl BlastModel {
    /// The step `agent` works.
    pub(super) fn activity(&self, agent: usize) -> BlastActivity {
        self.activities[agent]
    }

    /// The chain over `periods` (each period's intervals), from its
    /// starting stages. `standing` is the ground the plan opens with, and
    /// `dug` the ground some loader's chain digs.
    pub(super) fn new(chain: &DrillBlastInput, periods: &[&[Interval]], standing: &BTreeMap<GroundId, f64>, dug: &BTreeSet<GroundId>) -> Self {
        let mut free = chain.clone();
        free.fixed = None;
        free.resume = None;
        let opening = Chain::new(&free).state().checkpoint(0.0);
        let starts: Vec<f64> = periods
            .iter()
            .map(|intervals| intervals.iter().map(|interval| interval.start_h).fold(f64::MAX, f64::min))
            .collect();
        let ends: Vec<f64> = periods
            .iter()
            .map(|intervals| intervals.iter().map(|interval| interval.end_h).fold(f64::MIN, f64::max))
            .collect();
        let capacity = (0..chain.agents.len())
            .map(|agent| periods.iter().map(|intervals| chain.work_capacity(agent, None, intervals)).collect())
            .collect();
        let fires = starts.iter().zip(&ends).map(|(start, end)| chain.fires_at(start - 1e-9) < *end).collect();
        let mut blasts = Vec::new();
        for (index, (blast, opening)) in chain.blasts.iter().zip(&opening).enumerate() {
            let releases: Vec<GroundId> = blast.releases.iter().copied().filter(|ground| standing.contains_key(ground)).collect();
            if opening.firing == Firing::Fired || releases.is_empty() {
                continue;
            }
            let fires_in = match opening.firing {
                Firing::Scheduled(at) => Some(ends.iter().position(|end| at < *end).unwrap_or(periods.len())),
                _ => None,
            };
            let above: Vec<GroundId> = if opening.cleared {
                Vec::new()
            } else {
                blast
                    .above
                    .iter()
                    .copied()
                    .filter(|ground| standing.contains_key(ground))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            };
            let never_clear = !opening.cleared && (blast.never_clear || above.iter().any(|ground| !dug.contains(ground)));
            let machines = (0..chain.agents.len())
                .filter(|&agent| opening.left[chain.agents[agent].activity as usize] > 0.0)
                .map(|agent| {
                    (
                        agent,
                        periods.iter().map(|intervals| chain.work_capacity(agent, Some(index), intervals)).collect::<Vec<f64>>(),
                    )
                })
                .filter(|(_, available)| available.iter().any(|work| *work > 0.0))
                .collect();
            blasts.push(PlannedBlast {
                index,
                opening: *opening,
                fires_in,
                above,
                never_clear,
                releases,
                machines,
            });
        }
        let activities = chain.agents.iter().map(|agent| agent.activity).collect();
        Self {
            blasts,
            capacity,
            fires,
            activities,
        }
    }
}
