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

use super::{input::BlendInput, replay::BlendSolution};
use crate::model::schedule::optimisation::{Activity, DestinationId, Interval};

/// A route a plan sets a target for: a loader digging or reclaiming to a
/// destination on a day. Digging and reclaiming are apart, so neither's
/// shortfall becomes room for the other.
pub(crate) type Route = (usize, Activity, DestinationId, u32);

#[derive(Clone, Debug, Default)]
pub(crate) struct PlanTargets {
    /// Planned tonnes per route.
    pub(crate) routes: BTreeMap<Route, f64>,
    /// Objective weight per tonne up to the paced target.
    pub(crate) follow: f64,
    /// Objective cost per tonne past the paced target.
    pub(crate) overrun: f64,
    /// Relative route values on days still guided by the long-term plan.
    pub(crate) values: BTreeMap<(usize, DestinationId, u32), f64>,
    /// Cumulative hourly guidance from the incumbent's route deliveries,
    /// scaled and blended with even pacing inside a planned window.
    pub(crate) timing: BTreeMap<Route, Vec<(f64, f64)>>,
    /// Start and end hour of each day the horizon touches.
    days: BTreeMap<u32, (f64, f64)>,
    /// Days the plan covers. Any other day is worked as if there were no
    /// plan.
    pub(crate) planned: BTreeSet<u32>,
}

impl PlanTargets {
    pub(crate) fn new(input: &BlendInput, routes: BTreeMap<Route, f64>, follow: f64, overrun: f64) -> Self {
        let mut days: BTreeMap<u32, (f64, f64)> = BTreeMap::new();
        for interval in &input.intervals {
            let span = days.entry(interval.day()).or_insert((interval.start_h, interval.end_h));
            span.0 = span.0.min(interval.start_h);
            span.1 = span.1.max(interval.end_h);
        }
        let planned = routes.keys().map(|(_, _, _, day)| *day).collect();
        Self {
            routes,
            follow,
            overrun,
            values: BTreeMap::new(),
            timing: BTreeMap::new(),
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
        rebased.timing = self.timing.clone();
        rebased
    }

    pub(crate) fn timed(mut self, input: &BlendInput, solution: &BlendSolution) -> Self {
        let loaders: BTreeMap<_, _> = input.loaders.iter().enumerate().map(|(index, loader)| (loader.id, index)).collect();
        let mut routes = BTreeMap::<Route, BTreeMap<usize, f64>>::new();
        for row in &solution.movements {
            let movement = &input.movements[row.candidate];
            if let Some(&loader) = loaders.get(&movement.loader) {
                let day = input.intervals[row.interval].day();
                *routes
                    .entry((loader, movement.activity, movement.destination, day))
                    .or_default()
                    .entry(row.interval)
                    .or_default() += row.tonnes_t;
            }
        }
        for (route, intervals) in routes {
            let mut total = 0.0;
            let curve = intervals
                .into_iter()
                .map(|(interval, tonnes)| {
                    total += tonnes;
                    (input.intervals[interval].end_h, total)
                })
                .collect();
            self.timing.insert(route, curve);
        }
        self
    }

    /// A daily plan changes route totals, but the incumbent's hourly shares
    /// still show when each route can work. Use those shares as soft guidance.
    pub(crate) fn time_window(&mut self, input: &BlendInput, incumbent: &Self, days: &BTreeSet<u32>) {
        let mut hours = BTreeMap::<u32, Vec<Interval>>::new();
        for &interval in &input.intervals {
            if days.contains(&interval.day()) {
                hours.entry(interval.day()).or_default().push(interval);
            }
        }
        let mut changed = 0;
        let mut count = 0;
        for (route, &tonnes) in &self.routes {
            if !days.contains(&route.3) {
                continue;
            }
            let Some(curve) = incumbent.timing.get(route) else { continue };
            let Some(&(_, total)) = curve.last() else { continue };
            if total <= 1e-6 {
                continue;
            }
            let ratio = tonnes / total;
            let scaled = hours
                .get(&route.3)
                .into_iter()
                .flatten()
                .map(|&interval| {
                    let end = curve.partition_point(|&(hour, _)| hour <= interval.end_h);
                    let cumulative = end.checked_sub(1).map_or(0.0, |last| curve[last].1);
                    // The old route's peaks may not fit its changed total.
                    // Retain half of the daily plan's freedom to move work.
                    (interval.end_h, 0.5 * cumulative * ratio + 0.5 * tonnes * self.paced(interval))
                })
                .collect();
            self.timing.insert(*route, scaled);
            count += 1;
            changed += usize::from((tonnes - total).abs() > 1e-3);
        }
        log::debug!("window timing shaped {count} routes, {changed} with changed daily tonnes");
    }

    /// Timed routes become due at the incumbent's hours, scaled to new
    /// window totals where present. Other routes retain even daily pacing.
    pub(crate) fn due_today(&self, loader: usize, activity: Activity, destination: DestinationId, interval: Interval, tonnes: f64) -> f64 {
        if let Some(curve) = self.timing.get(&(loader, activity, destination, interval.day())) {
            let end = curve.partition_point(|&(hour, _)| hour <= interval.end_h);
            return end.checked_sub(1).map_or(0.0, |last| curve[last].1);
        }
        tonnes * self.paced(interval)
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
    pub(crate) fn replacing(&self, days: &BTreeSet<u32>, routes: &BTreeMap<Route, f64>, follow: f64, overrun: f64) -> Self {
        let mut merged: BTreeMap<_, _> = self
            .routes
            .iter()
            .filter(|((_, _, _, day), _)| !days.contains(day))
            .map(|(key, tonnes)| (*key, *tonnes))
            .collect();
        merged.extend(routes.iter().filter(|((_, _, _, day), _)| days.contains(day)).map(|(key, tonnes)| (*key, *tonnes)));
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
            timing: self
                .timing
                .iter()
                .filter(|(route, _)| !days.contains(&route.3))
                .map(|(route, curve)| (*route, curve.clone()))
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
