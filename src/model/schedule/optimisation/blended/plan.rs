//! Targets from a coarse whole-horizon plan for the hourly dispatch to follow
//! (prototype).
//!
//! The plan (`scip::plan` with the `scip` feature) decides, per loader and
//! day, how many tonnes go to each destination. The dispatch still chooses
//! every hour's blocks, destinations and trucks itself, under authored bar
//! priority, and the replay still checks the result; the targets only steer
//! its objective. Each loader's route is paced through the day: tonnes up to
//! the planned share of the day so far earn [`PlanTargets::follow`] per
//! tonne, and tonnes past it cost [`PlanTargets::overrun`]. Neither is money.
#![allow(dead_code, reason = "prototype, not yet called by Improve")]

use std::collections::BTreeMap;

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
    /// Start and end hour of each day the horizon touches.
    days: BTreeMap<u32, (f64, f64)>,
}

impl PlanTargets {
    pub(crate) fn new(input: &BlendInput, routes: BTreeMap<(usize, DestinationId, u32), f64>, follow: f64, overrun: f64) -> Self {
        let mut days: BTreeMap<u32, (f64, f64)> = BTreeMap::new();
        for interval in &input.intervals {
            let span = days.entry(interval.day()).or_insert((interval.start_h, interval.end_h));
            span.0 = span.0.min(interval.start_h);
            span.1 = span.1.max(interval.end_h);
        }
        Self { routes, follow, overrun, days }
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
