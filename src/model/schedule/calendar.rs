//! Persisted loader availability, utilisation, and sparse period rate overrides.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{ScheduleError, ScheduleResult};

/// Length of one scheduling period in elapsed project hours.
pub(crate) const SCHEDULE_PERIOD_H: f64 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct CalendarPeriod(pub(crate) u32);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LoaderPeriodOverride {
    pub(crate) availability: Option<f64>,
    pub(crate) utilisation: Option<f64>,
    pub(crate) rate_tph: Option<f64>,
}

impl LoaderPeriodOverride {
    fn is_empty(&self) -> bool {
        self.availability.is_none() && self.utilisation.is_none() && self.rate_tph.is_none()
    }
}

fn one() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LoaderCalendar {
    #[serde(default = "one")]
    pub(crate) default_availability: f64,
    #[serde(default = "one")]
    pub(crate) default_utilisation: f64,
    pub(crate) periods: BTreeMap<CalendarPeriod, LoaderPeriodOverride>,
}

impl Default for LoaderCalendar {
    fn default() -> Self {
        Self {
            default_availability: 1.0,
            default_utilisation: 1.0,
            periods: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CalendarField {
    Availability,
    Utilisation,
    Rate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CalendarCell {
    Default,
    Period(CalendarPeriod),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CalendarCellEdit {
    pub(crate) agent: super::LoaderAgentId,
    pub(crate) cell: CalendarCell,
    pub(crate) field: CalendarField,
    /// Domain units: percentages are fractions and rates are tonnes/hour.
    pub(crate) value: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CalendarValues {
    pub(crate) availability: f64,
    pub(crate) utilisation: f64,
    pub(crate) rate_tph: f64,
    pub(crate) effective_rate_tph: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RateChange {
    pub(crate) at_h: f64,
    pub(crate) rate_tph: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompiledRateCalendar {
    pub(crate) initial_rate_tph: f64,
    pub(crate) changes: Vec<RateChange>,
}

impl LoaderCalendar {
    pub(crate) fn validate(&self) -> ScheduleResult {
        checked_percentage(self.default_availability)?;
        checked_percentage(self.default_utilisation)?;
        for (period, value) in &self.periods {
            period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
            if value.is_empty() {
                return Err(ScheduleError::EmptyCalendarOverride);
            }
            if let Some(availability) = value.availability {
                checked_percentage(availability)?;
            }
            if let Some(utilisation) = value.utilisation {
                checked_percentage(utilisation)?;
            }
            if let Some(rate) = value.rate_tph {
                checked_override_rate(rate)?;
            }
        }
        Ok(())
    }

    pub(crate) fn values_at(&self, period: CalendarPeriod, class_rate: f64) -> ScheduleResult<CalendarValues> {
        checked_override_rate(class_rate)?;
        self.validate()?;
        self.values_at_validated(period, class_rate)
    }

    fn values_at_validated(&self, period: CalendarPeriod, class_rate: f64) -> ScheduleResult<CalendarValues> {
        let override_ = self.periods.get(&period);
        let availability = override_.and_then(|value| value.availability).unwrap_or(self.default_availability);
        let utilisation = override_.and_then(|value| value.utilisation).unwrap_or(self.default_utilisation);
        let rate_tph = override_.and_then(|value| value.rate_tph).unwrap_or(class_rate);
        let effective_rate_tph = rate_tph * availability * utilisation;
        if effective_rate_tph == 0.0 && availability != 0.0 && utilisation != 0.0 {
            return Err(ScheduleError::UnrepresentableEffectiveRate);
        }
        if !effective_rate_tph.is_finite() {
            return Err(ScheduleError::UnrepresentableEffectiveRate);
        }
        Ok(CalendarValues {
            availability,
            utilisation,
            rate_tph,
            effective_rate_tph,
        })
    }

    /// Compile only boundaries introduced by explicit overrides. Long blank
    /// spans remain one interval regardless of how distant the next override is.
    pub(crate) fn compile(&self, class_rate: f64) -> ScheduleResult<CompiledRateCalendar> {
        checked_override_rate(class_rate)?;
        self.validate()?;
        let initial_rate_tph = self.values_at_validated(CalendarPeriod(0), class_rate)?.effective_rate_tph;
        let mut boundaries = BTreeSet::new();
        for period in self.periods.keys() {
            let start = f64::from(period.0) * SCHEDULE_PERIOD_H;
            if start > 0.0 {
                boundaries.insert(period.0);
            }
            boundaries.insert(period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?);
        }
        let mut previous = initial_rate_tph;
        let mut changes = Vec::new();
        for period in boundaries {
            let rate_tph = self.values_at_validated(CalendarPeriod(period), class_rate)?.effective_rate_tph;
            if rate_tph != previous {
                changes.push(RateChange {
                    at_h: f64::from(period) * SCHEDULE_PERIOD_H,
                    rate_tph,
                });
                previous = rate_tph;
            }
        }
        Ok(CompiledRateCalendar { initial_rate_tph, changes })
    }

    pub(crate) fn set(&mut self, cell: CalendarCell, field: CalendarField, value: Option<f64>) -> ScheduleResult {
        match (cell, field) {
            (CalendarCell::Default, CalendarField::Rate) => return Err(ScheduleError::ReadOnlyCalendarCell),
            (CalendarCell::Default, CalendarField::Availability) => self.default_availability = value.map(checked_percentage).transpose()?.unwrap_or(1.0),
            (CalendarCell::Default, CalendarField::Utilisation) => self.default_utilisation = value.map(checked_percentage).transpose()?.unwrap_or(1.0),
            (CalendarCell::Period(period), field) => {
                if value.is_some() {
                    period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
                }
                let value = match field {
                    CalendarField::Availability | CalendarField::Utilisation => value.map(checked_percentage).transpose()?,
                    CalendarField::Rate => value.map(checked_override_rate).transpose()?,
                };
                let override_ = self.periods.entry(period).or_default();
                match field {
                    CalendarField::Availability => override_.availability = value,
                    CalendarField::Utilisation => override_.utilisation = value,
                    CalendarField::Rate => override_.rate_tph = value,
                }
                if override_.is_empty() {
                    self.periods.remove(&period);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn canonicalize_percentages(&mut self) {
        self.default_availability = canonical_percentage_zero(self.default_availability);
        self.default_utilisation = canonical_percentage_zero(self.default_utilisation);
        for value in self.periods.values_mut() {
            value.availability = value.availability.map(canonical_percentage_zero);
            value.utilisation = value.utilisation.map(canonical_percentage_zero);
        }
    }
}

impl CompiledRateCalendar {
    pub(crate) fn rate_at(&self, hour: f64) -> f64 {
        let index = self.changes.partition_point(|change| change.at_h <= hour);
        index.checked_sub(1).map_or(self.initial_rate_tph, |index| self.changes[index].rate_tph)
    }

    pub(crate) fn next_change_after(&self, hour: f64) -> Option<f64> {
        self.changes.iter().find(|change| change.at_h > hour).map(|change| change.at_h)
    }

    pub(crate) fn has_positive_rate_between(&self, start_h: f64, end_h: Option<f64>) -> bool {
        if end_h.is_some_and(|end| end <= start_h) {
            return false;
        }
        if self.rate_at(start_h) > 0.0 {
            return true;
        }
        self.changes
            .iter()
            .any(|change| change.at_h > start_h && end_h.is_none_or(|end| change.at_h < end) && change.rate_tph > 0.0)
    }
}

fn checked_percentage(value: f64) -> ScheduleResult<f64> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ScheduleError::InvalidCalendarPercentage);
    }
    Ok(canonical_percentage_zero(value))
}

fn canonical_percentage_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn checked_override_rate(value: f64) -> ScheduleResult<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ScheduleError::InvalidRate);
    }
    Ok(value)
}
