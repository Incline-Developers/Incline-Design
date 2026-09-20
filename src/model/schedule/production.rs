//! Per-period production totals read back off a published dispatch result.
//!
//! This is pure presentation arithmetic and nothing more. Availability,
//! utilisation and the rate calendar were all applied when the schedule was
//! calculated, and every figure here comes from one loader's own
//! [`ExecutionSegment::tonnes`] - never from a block's combined depletion, and
//! never by multiplying a rate back out.
//!
//! Dispatch merges compatible segments, so a segment may span any number of
//! calendar periods. Splitting one is a straight proportion of its duration,
//! with the rounding remainder placed in the last period it touched so the
//! segment total survives the split exactly.

use std::collections::BTreeMap;

use super::{CalendarPeriod, DispatchSchedule, LoaderAgentId, SCHEDULE_PERIOD_H};

/// Which period an hour falls in. Periods are half-open, so hour 24 belongs to
/// the second period and an execution *ending* there belongs to the first.
fn period_at(hour: f64) -> CalendarPeriod {
    CalendarPeriod((hour / SCHEDULE_PERIOD_H).floor().max(0.0) as u32)
}

/// The last period a half-open interval ending at `end_h` touches.
fn last_period_before(end_h: f64) -> CalendarPeriod {
    CalendarPeriod(((end_h / SCHEDULE_PERIOD_H).ceil().max(0.0) as u32).saturating_sub(1))
}

/// One published calculation's production, by loader and period.
///
/// Nonzero production only: a covered period with no entry produced nothing,
/// which is a different statement from a period the run never reached. That
/// distinction is [`Self::covers`], not the absence of a key.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PeriodProduction {
    /// Elapsed hour the calculation is answerable through. A zero-length
    /// interval covers no periods at all.
    coverage_end_h: f64,
    tonnes: BTreeMap<(LoaderAgentId, CalendarPeriod), f64>,
}

impl PeriodProduction {
    pub(crate) fn aggregate(schedule: &DispatchSchedule, coverage_end_h: f64) -> Self {
        let coverage_end_h = if coverage_end_h.is_finite() { coverage_end_h.max(0.0) } else { 0.0 };
        let mut tonnes: BTreeMap<(LoaderAgentId, CalendarPeriod), f64> = BTreeMap::new();
        for segment in &schedule.execution {
            if segment.tonnes == 0.0 || !segment.tonnes.is_finite() {
                continue;
            }
            let duration = segment.end_h - segment.start_h;
            let first = period_at(segment.start_h);
            let last = last_period_before(segment.end_h).max(first);
            if duration <= 0.0 || first == last {
                *tonnes.entry((segment.agent, first)).or_default() += segment.tonnes;
                continue;
            }
            let mut assigned = 0.0;
            for period in first.0..last.0 {
                let start = f64::from(period) * SCHEDULE_PERIOD_H;
                let end = f64::from(period.saturating_add(1)) * SCHEDULE_PERIOD_H;
                let overlap = end.min(segment.end_h) - start.max(segment.start_h);
                let share = segment.tonnes * overlap / duration;
                assigned += share;
                *tonnes.entry((segment.agent, CalendarPeriod(period))).or_default() += share;
            }
            *tonnes.entry((segment.agent, last)).or_default() += segment.tonnes - assigned;
        }
        tonnes.retain(|_, value| *value != 0.0);
        Self { coverage_end_h, tonnes }
    }

    pub(crate) fn coverage_end_h(&self) -> f64 {
        self.coverage_end_h
    }

    /// How many periods the calculation reaches into, whole or partial.
    pub(crate) fn covered_periods(&self) -> u32 {
        if self.coverage_end_h <= 0.0 {
            return 0;
        }
        (self.coverage_end_h / SCHEDULE_PERIOD_H).ceil().max(0.0) as u32
    }

    pub(crate) fn covers(&self, period: CalendarPeriod) -> bool {
        self.coverage_end_h > 0.0 && f64::from(period.0) * SCHEDULE_PERIOD_H < self.coverage_end_h
    }

    /// Whether coverage stops part-way through this period, which the grid has
    /// to say out loud: the figure is real but the day is not finished.
    pub(crate) fn is_partial(&self, period: CalendarPeriod) -> bool {
        self.covers(period) && self.coverage_end_h < f64::from(period.0.saturating_add(1)) * SCHEDULE_PERIOD_H
    }

    /// This loader's tonnes for this period, or `None` where the calculation
    /// says nothing. A covered period with no production reads `Some(0.0)`.
    pub(crate) fn tonnes(&self, agent: LoaderAgentId, period: CalendarPeriod) -> Option<f64> {
        self.covers(period).then(|| self.tonnes.get(&(agent, period)).copied().unwrap_or(0.0))
    }
}

/// One published calculation's deliveries, by destination and period.
///
/// Read off the movement records the same way [`PeriodProduction`] is read off
/// the execution segments, and for the same reason: every figure here is a
/// delivery the evaluator made, never a rate multiplied back out and never a
/// destination's capacity minus what is left.
///
/// A movement merged across several periods is split by duration, with the
/// rounding remainder placed in the last period it touched, so what one
/// destination received over the run is exactly the sum of its periods.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DestinationProduction {
    coverage_end_h: f64,
    received: BTreeMap<(super::DestinationId, CalendarPeriod), f64>,
}

impl DestinationProduction {
    pub(crate) fn aggregate(schedule: &DispatchSchedule, coverage_end_h: f64) -> Self {
        let coverage_end_h = if coverage_end_h.is_finite() { coverage_end_h.max(0.0) } else { 0.0 };
        let mut received: BTreeMap<(super::DestinationId, CalendarPeriod), f64> = BTreeMap::new();
        for movement in &schedule.movements {
            if movement.tonnes == 0.0 || !movement.tonnes.is_finite() {
                continue;
            }
            let duration = movement.end_h - movement.start_h;
            let first = period_at(movement.start_h);
            let last = last_period_before(movement.end_h).max(first);
            if duration <= 0.0 || first == last {
                *received.entry((movement.destination, first)).or_default() += movement.tonnes;
                continue;
            }
            let mut assigned = 0.0;
            for period in first.0..last.0 {
                let start = f64::from(period) * SCHEDULE_PERIOD_H;
                let end = f64::from(period.saturating_add(1)) * SCHEDULE_PERIOD_H;
                let overlap = end.min(movement.end_h) - start.max(movement.start_h);
                let share = movement.tonnes * overlap / duration;
                assigned += share;
                *received.entry((movement.destination, CalendarPeriod(period))).or_default() += share;
            }
            *received.entry((movement.destination, last)).or_default() += movement.tonnes - assigned;
        }
        received.retain(|_, value| *value != 0.0);
        Self { coverage_end_h, received }
    }

    pub(crate) fn coverage_end_h(&self) -> f64 {
        self.coverage_end_h
    }

    pub(crate) fn covered_periods(&self) -> u32 {
        if self.coverage_end_h <= 0.0 {
            return 0;
        }
        (self.coverage_end_h / SCHEDULE_PERIOD_H).ceil().max(0.0) as u32
    }

    pub(crate) fn covers(&self, period: CalendarPeriod) -> bool {
        self.coverage_end_h > 0.0 && f64::from(period.0) * SCHEDULE_PERIOD_H < self.coverage_end_h
    }

    pub(crate) fn is_partial(&self, period: CalendarPeriod) -> bool {
        self.covers(period) && self.coverage_end_h < f64::from(period.0.saturating_add(1)) * SCHEDULE_PERIOD_H
    }

    /// What this destination received in this period, or `None` where the
    /// calculation says nothing. A covered period with no delivery reads
    /// `Some(0.0)`.
    pub(crate) fn received(&self, destination: super::DestinationId, period: CalendarPeriod) -> Option<f64> {
        self.covers(period).then(|| self.received.get(&(destination, period)).copied().unwrap_or(0.0))
    }

    /// What this destination holds at the end of this period: everything it has
    /// received up to and including it.
    ///
    /// Cumulative through the *covered* part of the period, which is what makes
    /// a partial period's figure real but unfinished rather than wrong. Nothing
    /// is reclaimed in this increment, so for a stockpile this is also its
    /// scheduled inventory.
    pub(crate) fn cumulative(&self, destination: super::DestinationId, period: CalendarPeriod) -> Option<f64> {
        self.covers(period).then(|| {
            self.received
                .range((destination, CalendarPeriod(0))..=(destination, period))
                .filter(|((held, _), _)| *held == destination)
                .map(|(_, tonnes)| tonnes)
                .sum()
        })
    }
}
