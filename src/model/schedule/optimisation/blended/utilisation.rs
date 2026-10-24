//! The utilisation incentive, measured the one way every part of a run
//! measures it.
//!
//! A loader with an incentive is worth more, in the objective only, for each
//! tonne it digs while its scheduled utilisation stands below its target:
//! the incentive per tonne for every percentage point of the gap. The gap
//! closes as it digs, so the reward is priced in bands of [`BAND`] of the
//! utilisation scale, each paying the rate at its midpoint and none above
//! the target. Lower bands pay more, which makes the reward concave in the
//! tonnes dug: a linear program fills the bands from the bottom by itself,
//! with no integer to say so.
//!
//! The hourly dispatch prices each interval against the day so far (see
//! [`super::greedy`]). Improve sees whole days, so it prices each day's
//! utilisation as a whole; and so that its answer and the dispatch's can be
//! compared on the same terms, this module's [`value`] - what a finished
//! schedule earns, a day at a time - is what both are judged by, and what
//! the replay reconciles a solver's objective against. It is never money.

use std::collections::BTreeMap;

use super::{input::BlendInput, replay::MovementRow};
use crate::model::schedule::optimisation::{Activity, IntervalRate};

/// Width of one band, as a share of the productive hours it is measured
/// against.
pub(crate) const BAND: f64 = 0.05;

/// One loader's day with an incentive: the intervals it can dig in at their
/// effective rates, and the bands its dug hours fill.
pub(crate) struct LoaderDay {
    /// Position in [`BlendInput::loaders`].
    pub(crate) loader: usize,
    /// (interval index, effective dig rate) for every interval of the day
    /// with one.
    pub(crate) intervals: Vec<(usize, f64)>,
    /// (hours, value per hour) from the bottom of the scale to the target.
    pub(crate) bands: Vec<(f64, f64)>,
}

/// The bands utilisation from `lower` up to `upto` (at most `target`) falls
/// into, over `available` productive hours dug at `rate` tonnes an hour:
/// each `(hours, value per hour)`, priced against `target`.
pub(crate) fn bands(mut lower: f64, upto: f64, target: f64, available: f64, incentive: f64, rate: f64) -> Vec<(f64, f64)> {
    let upto = upto.min(target);
    let mut bands = Vec::new();
    while lower < upto - 1e-12 {
        let upper = (lower + BAND).min(upto);
        bands.push(((upper - lower) * available, incentive * 100.0 * (target - (lower + upper) / 2.0) * rate));
        lower = upper;
    }
    bands
}

/// Every loader-day an incentive applies to.
pub(crate) fn loader_days(input: &BlendInput) -> Vec<LoaderDay> {
    let mut days = Vec::new();
    for (position, loader) in input.loaders.iter().enumerate() {
        let hours = |rate: &IntervalRate| input.intervals.get(rate.interval).map_or(0.0, |interval| interval.duration_h());
        let mut by_day: BTreeMap<u32, Vec<&IntervalRate>> = BTreeMap::new();
        for rate in loader.rates.iter().filter(|rate| rate.dig_tph > 0.0) {
            if let Some(interval) = input.intervals.get(rate.interval) {
                by_day.entry(interval.day()).or_default().push(rate);
            }
        }
        for rates in by_day.into_values() {
            // Capture sets both per day, so any interval of it says.
            let (target, incentive) = (rates[0].utilisation_target, rates[0].utilisation_incentive);
            let available: f64 = rates.iter().map(|rate| hours(rate)).sum();
            if incentive <= 0.0 || available <= 0.0 {
                continue;
            }
            // A dug hour is worth the day's mean rate in tonnes, so the
            // reward per tonne is the incentive times the gap.
            let mean_rate = rates.iter().map(|rate| rate.dig_tph * hours(rate)).sum::<f64>() / available;
            days.push(LoaderDay {
                loader: position,
                intervals: rates.iter().map(|rate| (rate.interval, rate.dig_tph)).collect(),
                bands: bands(0.0, target, target, available, incentive, mean_rate),
            });
        }
    }
    days
}

/// What a schedule's dug tonnes earn under the incentive, each loader-day's
/// dug hours filling its bands from the bottom.
pub(crate) fn value(input: &BlendInput, movements: &[MovementRow]) -> f64 {
    let days = loader_days(input);
    if days.is_empty() {
        return 0.0;
    }
    let mut dug: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for row in movements {
        let Some(movement) = input.movements.get(row.candidate).filter(|movement| movement.activity == Activity::Dig) else {
            continue;
        };
        let Some(loader) = input.loaders.iter().position(|loader| loader.id == movement.loader) else {
            continue;
        };
        *dug.entry((loader, row.interval)).or_default() += row.tonnes_t;
    }
    days.iter()
        .map(|day| {
            let mut hours: f64 = day.intervals.iter().map(|&(k, rate)| dug.get(&(day.loader, k)).copied().unwrap_or(0.0) / rate).sum();
            let mut earned = 0.0;
            for &(width, per_hour) in &day.bands {
                let filled = hours.min(width).max(0.0);
                earned += filled * per_hour;
                hours -= filled;
            }
            earned
        })
        .sum()
}
