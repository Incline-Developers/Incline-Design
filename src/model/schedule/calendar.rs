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
    /// Tonnes per productive hour reclaiming, when this period overrides it.
    /// Independent of the dig rate above: the two are separate answers about
    /// the same machine, and a project saved before reclaim existed has
    /// neither.
    pub(crate) reclaim_rate_tph: Option<f64>,
    /// The scheduled utilisation the dig incentive pays up to; see
    /// [`LoaderCalendar::default_utilisation_target`]. Left out of a saved
    /// project when unset, so projects from before it read and hash as they did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) utilisation_target: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) utilisation_incentive: Option<f64>,
}

impl LoaderPeriodOverride {
    fn is_empty(&self) -> bool {
        self.availability.is_none()
            && self.utilisation.is_none()
            && self.rate_tph.is_none()
            && self.reclaim_rate_tph.is_none()
            && self.utilisation_target.is_none()
            && self.utilisation_incentive.is_none()
    }
}

fn one() -> f64 {
    1.0
}

fn is_one(value: &f64) -> bool {
    *value == 1.0
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LoaderCalendar {
    #[serde(default = "one")]
    pub(crate) default_availability: f64,
    #[serde(default = "one")]
    pub(crate) default_utilisation: f64,
    /// The scheduled utilisation, as a fraction, the dig incentive pays up
    /// to: the share of the day so far's productive hours the loader has
    /// worked. 100% unless set.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub(crate) default_utilisation_target: f64,
    /// Currency per dug tonne per percentage point the loader's scheduled
    /// utilisation so far today stands below its target. It steers the
    /// hourly schedule towards keeping loaders busy and is never counted as
    /// money. Zero unless set.
    #[serde(skip_serializing_if = "is_zero")]
    pub(crate) default_utilisation_incentive: f64,
    pub(crate) periods: BTreeMap<CalendarPeriod, LoaderPeriodOverride>,
}

impl Default for LoaderCalendar {
    fn default() -> Self {
        Self {
            default_availability: 1.0,
            default_utilisation: 1.0,
            default_utilisation_target: 1.0,
            default_utilisation_incentive: 0.0,
            periods: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CalendarField {
    Availability,
    Utilisation,
    /// Tonnes per productive hour digging.
    Rate,
    /// Tonnes per productive hour reclaiming from a stockpile. Availability and
    /// utilisation are shared: they are the machine's, not the activity's.
    ReclaimRate,
    /// The scheduled utilisation the dig incentive pays up to, a fraction.
    UtilisationTarget,
    /// Currency per dug tonne per point below the target.
    UtilisationIncentive,
}

impl CalendarField {
    /// Which of the two rates this field is, or `None` for the shared factors.
    fn rate_kind(self) -> Option<RateKind> {
        match self {
            Self::Rate => Some(RateKind::Dig),
            Self::ReclaimRate => Some(RateKind::Reclaim),
            Self::Availability | Self::Utilisation | Self::UtilisationTarget | Self::UtilisationIncentive => None,
        }
    }
}

/// Which rate a calculation is reading. The shared availability and
/// utilisation apply to both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RateKind {
    Dig,
    Reclaim,
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
            if let Some(rate) = value.reclaim_rate_tph {
                checked_override_rate(rate)?;
            }
            if let Some(target) = value.utilisation_target {
                checked_percentage(target)?;
            }
            if let Some(incentive) = value.utilisation_incentive {
                checked_incentive(incentive)?;
            }
        }
        checked_percentage(self.default_utilisation_target)?;
        checked_incentive(self.default_utilisation_incentive)?;
        Ok(())
    }

    /// The utilisation target and the incentive per dug tonne per point
    /// below it, for one period.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code, reason = "read by the native schedule capture; the browser build does not calculate"))]
    pub(crate) fn incentive_at(&self, period: CalendarPeriod) -> (f64, f64) {
        let held = self.periods.get(&period);
        (
            held.and_then(|value| value.utilisation_target).unwrap_or(self.default_utilisation_target),
            held.and_then(|value| value.utilisation_incentive).unwrap_or(self.default_utilisation_incentive),
        )
    }

    #[allow(
        dead_code,
        reason = "dig-rate convenience retained for the current dispatcher while shared rate selection serves future reclaim"
    )]
    pub(crate) fn values_at(&self, period: CalendarPeriod, class_rate: f64) -> ScheduleResult<CalendarValues> {
        self.rate_values_at(period, RateKind::Dig, class_rate)
    }

    /// The same, for the rate this machine reclaims at. Availability and
    /// utilisation are the shared ones: they describe the machine's time, which
    /// it spends on whichever activity it is given.
    pub(crate) fn rate_values_at(&self, period: CalendarPeriod, kind: RateKind, class_rate: f64) -> ScheduleResult<CalendarValues> {
        checked_override_rate(class_rate)?;
        self.validate()?;
        self.values_at_validated(period, kind, class_rate)
    }

    fn values_at_validated(&self, period: CalendarPeriod, kind: RateKind, class_rate: f64) -> ScheduleResult<CalendarValues> {
        let override_ = self.periods.get(&period);
        let availability = override_.and_then(|value| value.availability).unwrap_or(self.default_availability);
        let utilisation = override_.and_then(|value| value.utilisation).unwrap_or(self.default_utilisation);
        let held = override_.and_then(|value| match kind {
            RateKind::Dig => value.rate_tph,
            RateKind::Reclaim => value.reclaim_rate_tph,
        });
        let rate_tph = held.unwrap_or(class_rate);
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
        self.compile_rate(RateKind::Dig, class_rate)
    }

    /// The same, for reclaiming. Kept out of the dig-only calculation's inputs:
    /// adding a reclaim override must not retire a result that never read one.
    pub(crate) fn compile_rate(&self, kind: RateKind, class_rate: f64) -> ScheduleResult<CompiledRateCalendar> {
        checked_override_rate(class_rate)?;
        self.validate()?;
        let initial_rate_tph = self.values_at_validated(CalendarPeriod(0), kind, class_rate)?.effective_rate_tph;
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
            let rate_tph = self.values_at_validated(CalendarPeriod(period), kind, class_rate)?.effective_rate_tph;
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
            (CalendarCell::Default, CalendarField::Rate | CalendarField::ReclaimRate) => return Err(ScheduleError::ReadOnlyCalendarCell),
            (CalendarCell::Default, CalendarField::Availability) => self.default_availability = value.map(checked_percentage).transpose()?.unwrap_or(1.0),
            (CalendarCell::Default, CalendarField::Utilisation) => self.default_utilisation = value.map(checked_percentage).transpose()?.unwrap_or(1.0),
            (CalendarCell::Default, CalendarField::UtilisationTarget) => self.default_utilisation_target = value.map(checked_percentage).transpose()?.unwrap_or(1.0),
            (CalendarCell::Default, CalendarField::UtilisationIncentive) => self.default_utilisation_incentive = value.map(checked_incentive).transpose()?.unwrap_or(0.0),
            (CalendarCell::Period(period), field) => {
                if value.is_some() {
                    period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
                }
                let value = match (field.rate_kind(), field) {
                    (_, CalendarField::UtilisationIncentive) => value.map(checked_incentive).transpose()?,
                    (None, _) => value.map(checked_percentage).transpose()?,
                    (Some(_), _) => value.map(checked_override_rate).transpose()?,
                };
                let override_ = self.periods.entry(period).or_default();
                match field {
                    CalendarField::Availability => override_.availability = value,
                    CalendarField::Utilisation => override_.utilisation = value,
                    CalendarField::Rate => override_.rate_tph = value,
                    CalendarField::ReclaimRate => override_.reclaim_rate_tph = value,
                    CalendarField::UtilisationTarget => override_.utilisation_target = value,
                    CalendarField::UtilisationIncentive => override_.utilisation_incentive = value,
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
            value.utilisation_target = value.utilisation_target.map(canonical_percentage_zero);
        }
    }
}

// Compiled calendars are read by the native schedule capture.
#[cfg_attr(target_arch = "wasm32", allow(dead_code, reason = "read by the native schedule capture; the browser build does not calculate"))]
impl CompiledRateCalendar {
    pub(crate) fn rate_at(&self, hour: f64) -> f64 {
        let index = self.changes.partition_point(|change| change.at_h <= hour);
        index.checked_sub(1).map_or(self.initial_rate_tph, |index| self.changes[index].rate_tph)
    }

    pub(crate) fn next_change_after(&self, hour: f64) -> Option<f64> {
        self.changes.iter().find(|change| change.at_h > hour).map(|change| change.at_h)
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

fn checked_incentive(value: f64) -> ScheduleResult<f64> {
    if !value.is_finite() || value < 0.0 {
        return Err(ScheduleError::InvalidUtilisationIncentive);
    }
    Ok(canonical_percentage_zero(value))
}

fn checked_override_rate(value: f64) -> ScheduleResult<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ScheduleError::InvalidRate);
    }
    Ok(value)
}
