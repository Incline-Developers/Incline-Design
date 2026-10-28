//! Targets from a coarse whole-horizon plan for the hourly dispatch to follow.
//!
//! The plan (`scip::plan`) decides, per loader and
//! day, how many tonnes go to each destination, dug or reclaimed. The dispatch still chooses
//! every hour's blocks, destinations and trucks itself, under authored bar
//! priority, and the replay still checks the result; the targets only steer
//! its objective. Each loader's route is paced through the day, carrying
//! earlier days' shortfalls forward: tonnes up to the cumulative planned
//! share earn [`PlanTargets::follow`] per tonne, and tonnes past it cost
//! [`PlanTargets::overrun`]. Neither is money.

use std::collections::{BTreeMap, BTreeSet};

use super::input::BlendInput;
use crate::model::schedule::optimisation::{DestinationId, Interval};

#[derive(Clone, Debug, Default)]
pub(crate) struct PlanTargets {
    /// Planned dug tonnes per (loader index, destination, day).
    pub(crate) routes: BTreeMap<(usize, DestinationId, u32), f64>,
    /// Objective weight per tonne up to the paced target.
    pub(crate) follow: f64,
    /// Objective cost per tonne past the paced target.
    pub(crate) overrun: f64,
    /// Relative route values on days still guided by the long-term plan.
    pub(crate) values: BTreeMap<(usize, DestinationId, u32), f64>,
    /// Start and end hour of each day the horizon touches.
    days: BTreeMap<u32, (f64, f64)>,
    /// Days the plan covers. Any other day is worked as if there were no
    /// plan.
    pub(crate) planned: BTreeSet<u32>,
}

impl PlanTargets {
    pub(crate) fn new(input: &BlendInput, routes: BTreeMap<(usize, DestinationId, u32), f64>, follow: f64, overrun: f64) -> Self {
        let mut days: BTreeMap<u32, (f64, f64)> = BTreeMap::new();
        for interval in &input.intervals {
            let span = days.entry(interval.day()).or_insert((interval.start_h, interval.end_h));
            span.0 = span.0.min(interval.start_h);
            span.1 = span.1.max(interval.end_h);
        }
        let planned = routes.keys().map(|(_, _, day)| *day).collect();
        Self {
            routes,
            follow,
            overrun,
            values: BTreeMap::new(),
            days,
            planned,
        }
    }

    /// These targets for a part of the horizon `input` holds: the same
    /// routes and planned days, paced through its days.
    pub(crate) fn rebased(&self, input: &BlendInput) -> Self {
        let mut rebased = Self::new(input, self.routes.clone(), self.follow, self.overrun);
        rebased.planned = self.planned.clone();
        rebased.values = self.values.clone();
        rebased
    }

    /// Weekly plans have more freedom than the hourly dispatch. When their
    /// routes compete, preserve valuable deliveries before cheap production.
    /// Daily window targets retain their full strength to guide stripping.
    pub(crate) fn valued(mut self, input: &BlendInput) -> Self {
        let loaders: BTreeMap<_, _> = input.loaders.iter().enumerate().map(|(index, loader)| (loader.id, index)).collect();
        let mut values = BTreeMap::<(usize, DestinationId), f64>::new();
        for movement in &input.movements {
            let Some(&loader) = loaders.get(&movement.loader) else { continue };
            let value = movement.value_per_tonne().unwrap_or(0.0).max(0.0);
            let entry = values.entry((loader, movement.destination)).or_default();
            *entry = entry.max(value);
        }
        let highest = values.values().copied().fold(0.0_f64, f64::max);
        if highest > 0.0 {
            self.values.clear();
            for ((loader, destination), value) in values {
                for &day in &self.planned {
                    self.values.insert((loader, destination, day), value / highest);
                }
            }
        }
        self
    }

    /// These targets with `days` replaced by `routes`' for them.
    pub(crate) fn replacing(&self, days: &BTreeSet<u32>, routes: &BTreeMap<(usize, DestinationId, u32), f64>, follow: f64, overrun: f64) -> Self {
        let mut merged: BTreeMap<_, _> = self
            .routes
            .iter()
            .filter(|((_, _, day), _)| !days.contains(day))
            .map(|(key, tonnes)| (*key, *tonnes))
            .collect();
        merged.extend(routes.iter().filter(|((_, _, day), _)| days.contains(day)).map(|(key, tonnes)| (*key, *tonnes)));
        let mut planned: BTreeSet<u32> = self.planned.difference(days).copied().collect();
        planned.extend(days.iter().copied());
        Self {
            routes: merged,
            follow,
            overrun,
            values: self
                .values
                .iter()
                .filter(|((_, _, day), _)| !days.contains(day))
                .map(|(key, value)| (*key, *value))
                .collect(),
            days: self.days.clone(),
            planned,
        }
    }

    /// The share of `interval`'s day that has passed by its end.
    pub(crate) fn paced(&self, interval: Interval) -> f64 {
        self.days
            .get(&interval.day())
            .filter(|(start, end)| end > start)
            .map_or(1.0, |(start, end)| ((interval.end_h - start) / (end - start)).clamp(0.0, 1.0))
    }

    /// A follow weight in proportion to what the dispatch weighs: the spread
    /// of the dig candidates' values per tonne.
    pub(crate) fn value_spread(input: &BlendInput) -> f64 {
        let values = input.movements.iter().filter_map(|candidate| candidate.value_per_tonne().ok());
        let (low, high) = values.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| (low.min(value), high.max(value)));
        if high >= low { high - low + 1.0 } else { 1.0 }
    }
}
