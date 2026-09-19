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
