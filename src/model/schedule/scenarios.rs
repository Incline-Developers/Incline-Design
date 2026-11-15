//! Scenarios: several schedules of the one mine, each with its own Gantt and
//! its own horizon.
//!
//! The mine - its fleet, calendars, destinations, haulage, values, drill and
//! blast - is shared by every scenario, as Setup describes it once. What a
//! scenario holds of its own is what a schedule is: its bars, and the
//! horizon it is read and calculated over - where it ends, the step it is
//! calculated at and the periods it is read in. A short-term scenario of
//! days, worked bar by bar, and a long-term one of months, mostly automatic
//! bars, are two scenarios of one project.
//!
//! A scenario may start from another ([`Scenario::base`]): it works the
//! other's bars as they are, and its own besides. A long-term scenario that
//! starts from the short-term one takes the planner's next weeks as given
//! and plans the years after around them, its automatic bars taking the
//! ground those bars leave. The bars it starts from are edited in the
//! scenario that owns them, never in one that starts from it.
//!
//! One scenario is open. Its bars and horizon are the plan's own, which
//! every page and calculation reads, so nothing outside this module and the
//! plan's switch needs to know there are others: the rest wait here
//! ([`Scenario::stash`]) until they are opened. A project with one scenario
//! holds no list at all.

use serde::{Deserialize, Serialize};

use super::{ScheduleBar, periods::HorizonProfile};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ScenarioId(pub(crate) u64);

/// What a scenario holds of its own while another is open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScenarioStash {
    #[serde(default)]
    pub(crate) bars: Vec<ScheduleBar>,
    pub(crate) horizon: HorizonProfile,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Scenario {
    pub(crate) id: ScenarioId,
    pub(crate) name: String,
    /// The scenario whose bars this one starts from, worked as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base: Option<ScenarioId>,
    /// Its bars and horizon while it is not open; `None` for the open one,
    /// whose are the plan's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) stash: Option<ScenarioStash>,
}

/// The project's scenarios. Empty for a project of one scenario.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Scenarios {
    pub(crate) open: ScenarioId,
    pub(crate) list: Vec<Scenario>,
    pub(crate) next_id: u64,
}

impl Scenarios {
    pub(crate) fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub(crate) fn get(&self, id: ScenarioId) -> Option<&Scenario> {
        self.list.iter().find(|scenario| scenario.id == id)
    }

    pub(crate) fn get_mut(&mut self, id: ScenarioId) -> Option<&mut Scenario> {
        self.list.iter_mut().find(|scenario| scenario.id == id)
    }

    pub(crate) fn open_scenario(&self) -> Option<&Scenario> {
        self.get(self.open)
    }

    pub(crate) fn allocate(&mut self) -> ScenarioId {
        let id = ScenarioId(self.next_id);
        self.next_id += 1;
        id
    }

    /// The scenarios `id` starts from, nearest first. Stops at a scenario
    /// already met, so a ring - which edits refuse, but a file might hold -
    /// ends rather than repeats.
    pub(crate) fn bases(&self, id: ScenarioId) -> Vec<ScenarioId> {
        let mut out: Vec<ScenarioId> = Vec::new();
        let mut at = self.get(id).and_then(|scenario| scenario.base);
        while let Some(base) = at {
            if base == id || out.contains(&base) {
                break;
            }
            out.push(base);
            at = self.get(base).and_then(|scenario| scenario.base);
        }
        out
    }

    /// Whether `id` starting from `base` would make a ring.
    pub(crate) fn would_ring(&self, id: ScenarioId, base: ScenarioId) -> bool {
        base == id || self.bases(base).contains(&id)
    }

    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_id = self.next_id.max(other.next_id);
    }
}

/// One scenario as the scenario list shows it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScenarioRow {
    pub(crate) id: ScenarioId,
    pub(crate) name: String,
    pub(crate) base: Option<ScenarioId>,
    pub(crate) open: bool,
    pub(crate) horizon: HorizonProfile,
    /// Its own bars, not those it starts from.
    pub(crate) bars: usize,
}

use super::{ScheduleError, SchedulePlan, ScheduleResult, checked_name, same_name};
use crate::i18n::tr;

impl SchedulePlan {
    pub(crate) fn scenarios(&self) -> &Scenarios {
        &self.scenarios
    }

    /// The open scenario's horizon: the plan's own.
    pub(crate) fn open_horizon(&self) -> HorizonProfile {
        HorizonProfile {
            end_day: self.experiment.planning_end_day,
            interval_h: self.experiment.interval_h,
            unit: self.periods.unit,
        }
    }

    fn take_horizon(&mut self, horizon: HorizonProfile) -> ScheduleResult {
        self.experiment.set_planning_end_day(horizon.end_day)?;
        self.experiment.set_interval_h(horizon.interval_h)?;
        self.periods.unit = horizon.unit;
        Ok(())
    }

    /// The scenarios as listed: a project that has never had a second reads
    /// as one, the open one, unnamed until it is given a name.
    pub(crate) fn scenario_rows(&self) -> Vec<ScenarioRow> {
        if self.scenarios.is_empty() {
            return vec![ScenarioRow {
                id: ScenarioId::default(),
                name: tr!("scenario-first-name"),
                base: None,
                open: true,
                horizon: self.open_horizon(),
                bars: self.bars.len(),
            }];
        }
        self.scenarios
            .list
            .iter()
            .map(|scenario| {
                let open = scenario.id == self.scenarios.open;
                ScenarioRow {
                    id: scenario.id,
                    name: scenario.name.clone(),
                    base: scenario.base,
                    open,
                    horizon: if open {
                        self.open_horizon()
                    } else {
                        scenario.stash.as_ref().map_or_else(|| self.open_horizon(), |stash| stash.horizon)
                    },
                    bars: if open {
                        self.bars.len()
                    } else {
                        scenario.stash.as_ref().map_or(0, |stash| stash.bars.len())
                    },
                }
            })
            .collect()
    }

    /// A list of one for a project that holds none yet, so a second can join it.
    fn ensure_scenarios(&mut self) {
        if self.scenarios.is_empty() {
            let id = self.scenarios.allocate();
            self.scenarios.list.push(Scenario {
                id,
                name: tr!("scenario-first-name"),
                base: None,
                stash: None,
            });
            self.scenarios.open = id;
        }
    }

    fn checked_scenario_name(&self, name: &str, except: Option<ScenarioId>) -> ScheduleResult<String> {
        let name = checked_name(name)?;
        if self.scenarios.list.iter().any(|scenario| Some(scenario.id) != except && same_name(&scenario.name, &name)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        Ok(name)
    }

    /// A name no scenario has: "Scenario 2", "Scenario 3" and so on.
    pub(crate) fn next_scenario_name(&self) -> String {
        let taken: Vec<String> = self.scenario_rows().into_iter().map(|row| row.name).collect();
        (2..)
            .map(|number| tr!("scenario-numbered-name", number = number.to_string()))
            .find(|name| !taken.iter().any(|other| same_name(other, name)))
            .unwrap_or_default()
    }

    /// Add a scenario and open it: a copy of the open one - its bars, as
    /// bars of its own, its horizon and what it starts from - or, with
    /// `copy` off, one with no bars, starting from the open one, over the
    /// same horizon.
    pub(crate) fn add_scenario(&mut self, name: &str, copy: bool) -> ScheduleResult<ScenarioId> {
        self.ensure_scenarios();
        let name = self.checked_scenario_name(name, None)?;
        let open = self.scenarios.open;
        let bars = if copy {
            let mut bars = self.bars.clone();
            for bar in &mut bars {
                bar.id = super::BarId(self.next_bar_id);
                self.next_bar_id = self.next_bar_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
            }
            bars
        } else {
            Vec::new()
        };
        let base = if copy {
            self.scenarios.get(open).and_then(|scenario| scenario.base)
        } else {
            Some(open)
        };
        let id = self.scenarios.allocate();
        self.scenarios.list.push(Scenario {
            id,
            name,
            base,
            stash: Some(ScenarioStash {
                bars,
                horizon: self.open_horizon(),
            }),
        });
        self.open_scenario(id)?;
        Ok(id)
    }

    /// Open `id`: the open scenario's bars and horizon wait in it, and
    /// `id`'s take their place.
    pub(crate) fn open_scenario(&mut self, id: ScenarioId) -> ScheduleResult {
        if self.scenarios.is_empty() || id == self.scenarios.open {
            return Ok(());
        }
        let stash = self
            .scenarios
            .get_mut(id)
            .ok_or(ScheduleError::UnknownScenario)?
            .stash
            .take()
            .ok_or(ScheduleError::UnknownScenario)?;
        let leaving = ScenarioStash {
            bars: std::mem::take(&mut self.bars),
            horizon: self.open_horizon(),
        };
        let open = self.scenarios.open;
        if let Some(scenario) = self.scenarios.get_mut(open) {
            scenario.stash = Some(leaving);
        }
        self.scenarios.open = id;
        self.bars = stash.bars;
        self.take_horizon(stash.horizon)
    }

    pub(crate) fn rename_scenario(&mut self, id: ScenarioId, name: &str) -> ScheduleResult {
        self.ensure_scenarios();
        let id = if self.scenarios.get(id).is_none() && self.scenarios.list.len() == 1 {
            self.scenarios.open
        } else {
            id
        };
        let name = self.checked_scenario_name(name, Some(id))?;
        self.scenarios.get_mut(id).ok_or(ScheduleError::UnknownScenario)?.name = name;
        Ok(())
    }

    /// Delete a scenario, never the last. The open one gives way to the
    /// one listed before it; scenarios that started from it start from
    /// what it started from.
    pub(crate) fn delete_scenario(&mut self, id: ScenarioId) -> ScheduleResult {
        if self.scenarios.list.len() < 2 {
            return Err(ScheduleError::LastScenario);
        }
        let at = self.scenarios.list.iter().position(|scenario| scenario.id == id).ok_or(ScheduleError::UnknownScenario)?;
        if id == self.scenarios.open {
            let next = self.scenarios.list[if at == 0 { 1 } else { at - 1 }].id;
            self.open_scenario(next)?;
        }
        let removed = self.scenarios.list.remove(at);
        for scenario in &mut self.scenarios.list {
            if scenario.base == Some(id) {
                scenario.base = removed.base;
            }
        }
        // Rebasing never rings: a base of the removed one was never one
        // of its dependants.
        Ok(())
    }

    /// What `id` starts from, or nothing.
    pub(crate) fn set_scenario_base(&mut self, id: ScenarioId, base: Option<ScenarioId>) -> ScheduleResult {
        if self.scenarios.get(id).is_none() {
            return Err(ScheduleError::UnknownScenario);
        }
        if let Some(base) = base
            && (self.scenarios.get(base).is_none() || self.scenarios.would_ring(id, base))
        {
            return Err(ScheduleError::ScenarioRing);
        }
        self.scenarios.get_mut(id).ok_or(ScheduleError::UnknownScenario)?.base = base;
        Ok(())
    }

    /// The bars the open scenario starts from, nearest scenario first:
    /// worked as they are, edited only where they belong.
    pub(crate) fn inherited_bars(&self) -> Vec<(&str, &ScheduleBar)> {
        self.scenarios
            .bases(self.scenarios.open)
            .into_iter()
            .filter_map(|base| self.scenarios.get(base))
            .filter_map(|scenario| Some((scenario.name.as_str(), scenario.stash.as_ref()?)))
            .flat_map(|(name, stash)| stash.bars.iter().map(move |bar| (name, bar)))
            .collect()
    }

    /// The bars the open scenario starts from, then its own.
    pub(crate) fn bars_with_inherited(&self) -> impl Iterator<Item = &ScheduleBar> {
        self.inherited_bars().into_iter().map(|(_, bar)| bar).chain(self.bars.iter())
    }

    /// The steps the scenarios the open one starts from are calculated at,
    /// over their horizons, as (end, step) in hours: the planner's weeks of
    /// a short-term scenario stay hour by hour in the long-term one that
    /// starts from it, the years after at its own coarser step.
    pub(crate) fn inherited_steps(&self) -> Vec<(f64, f64)> {
        self.scenarios
            .bases(self.scenarios.open)
            .into_iter()
            .filter_map(|base| self.scenarios.get(base)?.stash.as_ref())
            .map(|stash| (f64::from(stash.horizon.end_day) * super::SCHEDULE_PERIOD_H, stash.horizon.interval_h))
            .filter(|(_, step)| *step < self.experiment.interval_h)
            .collect()
    }

    /// The plan a calculation reads: the open scenario's, with the bars it
    /// starts from ahead of its own. They come first on each machine - its
    /// own bars take the lanes below theirs - so what it starts from is
    /// worked as it is, and its own bars take what that leaves.
    pub(crate) fn for_calculation(&self) -> Self {
        let mut plan = self.clone();
        let inherited: Vec<ScheduleBar> = self.inherited_bars().into_iter().map(|(_, bar)| bar.clone()).collect();
        if inherited.is_empty() {
            return plan;
        }
        let mut lanes: std::collections::BTreeMap<super::LoaderAgentId, u32> = std::collections::BTreeMap::new();
        for bar in &inherited {
            if let Some(agent) = bar.agent {
                let lane = lanes.entry(agent).or_default();
                *lane = (*lane).max(bar.priority.saturating_add(1));
            }
        }
        for bar in &mut plan.bars {
            if let Some(below) = bar.agent.and_then(|agent| lanes.get(&agent)) {
                bar.priority = bar.priority.saturating_add(*below);
            }
        }
        plan.bars.splice(0..0, inherited);
        plan
    }

    /// Run `edit` on every scenario's bars, the open one's first, each as
    /// the plan's own while it runs.
    pub(super) fn each_scenario<T>(&mut self, mut edit: impl FnMut(&mut Self) -> ScheduleResult<T>) -> ScheduleResult<Vec<T>> {
        let mut out = vec![edit(self)?];
        for index in 0..self.scenarios.list.len() {
            let Some(mut stash) = self.scenarios.list[index].stash.take() else { continue };
            std::mem::swap(&mut self.bars, &mut stash.bars);
            let result = edit(self);
            std::mem::swap(&mut self.bars, &mut stash.bars);
            self.scenarios.list[index].stash = Some(stash);
            out.push(result?);
        }
        Ok(out)
    }

    /// Projects from before scenarios kept a short-term and a long-term
    /// horizon over the one set of bars: those become two scenarios, each
    /// with a copy of the bars.
    pub(super) fn migrate_horizons(&mut self) {
        use super::periods::Horizon;
        if !self.scenarios.is_empty() {
            return;
        }
        let Some(other) = self.periods.other.take() else {
            self.periods.horizon = Horizon::Short;
            return;
        };
        let label = |horizon: Horizon| match horizon {
            Horizon::Short => tr!("planning-horizon-short"),
            Horizon::Long => tr!("planning-horizon-long"),
        };
        let open = self.periods.horizon;
        let waiting = if open == Horizon::Short { Horizon::Long } else { Horizon::Short };
        // Both horizons showed the one set of bars, editable in either: each
        // scenario keeps a copy of its own.
        let mut copies = self.bars.clone();
        // Above every id the file holds, whatever its counter says.
        if let Some(highest) = self.bars.iter().map(|bar| bar.id.0).max() {
            self.next_bar_id = self.next_bar_id.max(highest.saturating_add(1));
        }
        for bar in &mut copies {
            bar.id = super::BarId(self.next_bar_id);
            self.next_bar_id = self.next_bar_id.saturating_add(1);
        }
        let opened = self.scenarios.allocate();
        let other_id = self.scenarios.allocate();
        let mut list = vec![
            Scenario {
                id: opened,
                name: label(open),
                base: None,
                stash: None,
            },
            Scenario {
                id: other_id,
                name: label(waiting),
                base: None,
                stash: Some(ScenarioStash { bars: copies, horizon: other }),
            },
        ];
        // Listed short term first, whichever was open.
        if open == Horizon::Long {
            list.reverse();
        }
        let first = opened;
        self.scenarios.list = list;
        self.scenarios.open = first;
        self.periods.horizon = Horizon::Short;
    }
}
