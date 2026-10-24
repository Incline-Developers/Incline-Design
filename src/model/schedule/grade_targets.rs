//! Soft grade targets on a crusher's daily receipts, authored in the Calendar.
//! Values use the grade's stored numbers; penalties are money per delivered
//! tonne at the band edge.

use serde::{Deserialize, Serialize};

use super::{DestinationId, ScheduleError, ScheduleResult};
use crate::{i18n::tr, model::ReserveFieldId};

/// A retired plan field, accepted in any shape and never written.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Retired;

impl<'de> Deserialize<'de> for Retired {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(deserializer).map(|_| Self)
    }
}

/// Every target is priced on one calendar day's receipts, aligned to hour 0.
pub(crate) const TARGET_PERIOD_H: f64 = super::SCHEDULE_PERIOD_H;

/// The calendar day an hour falls in.
pub(crate) fn target_day(hour: f64) -> u32 {
    (hour / TARGET_PERIOD_H).floor() as u32
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GradeTarget {
    pub(crate) destination: DestinationId,
    pub(crate) field: ReserveFieldId,
    pub(crate) lower: Option<f64>,
    pub(crate) target: f64,
    pub(crate) upper: Option<f64>,
    pub(crate) penalty_per_tonne: f64,
    pub(crate) outside_multiplier: f64,
}

impl GradeTarget {
    pub(crate) fn validate(&self) -> ScheduleResult {
        let valid = self.target.is_finite()
            && self.target >= 0.0
            && self.lower.is_none_or(|v| v.is_finite() && v >= 0.0 && v < self.target)
            && self.upper.is_none_or(|v| v.is_finite() && v > self.target)
            && (self.lower.is_some() || self.upper.is_some())
            && self.penalty_per_tonne.is_finite()
            && self.penalty_per_tonne >= 0.0
            && self.outside_multiplier.is_finite()
            && self.outside_multiplier >= 1.0
            && self.hinges().iter().all(|&(_, _, slope)| slope.is_finite())
            && self.hinges().iter().map(|&(_, _, slope)| slope).sum::<f64>().is_finite();
        if valid { Ok(()) } else { Err(ScheduleError::InvalidGradeTarget) }
    }

    /// Positive-part hinges, `(grade boundary, direction, money / content)`.
    /// Applied to Q - boundary * T, this is linear without dividing by T.
    pub(crate) fn hinges(&self) -> Vec<(f64, f64, f64)> {
        let mut result = Vec::new();
        for (limit, direction) in [(self.lower, -1.0), (self.upper, 1.0)] {
            if let Some(limit) = limit {
                let slope = self.penalty_per_tonne / (limit - self.target).abs();
                result.push((self.target, direction, slope));
                result.push((limit, direction, slope * (self.outside_multiplier - 1.0)));
            }
        }
        result
    }

    pub(crate) fn penalty(&self, tonnes: f64, contained: f64) -> f64 {
        self.hinges()
            .iter()
            .map(|&(boundary, direction, slope)| slope * (direction * (contained - boundary * tonnes)).max(0.0))
            .sum()
    }
}

/// Editable crusher specification cells in the daily Calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum GradeTargetInput {
    Target,
    Lower,
    Upper,
    Penalty,
}
impl GradeTargetInput {
    pub(crate) const ALL: [Self; 4] = [Self::Target, Self::Lower, Self::Upper, Self::Penalty];
    pub(crate) fn label(self) -> String {
        match self {
            Self::Target => tr!("grade-target-value"),
            Self::Lower => tr!("grade-target-lower"),
            Self::Upper => tr!("grade-target-upper"),
            Self::Penalty => tr!("grade-target-content-penalty"),
        }
    }
}

/// Missing target or both missing limits is an unfinished, inactive specification.
/// Cells can be entered individually; cross-field validation applies when values exist.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct GradeTargetValues {
    pub(crate) target: Option<f64>,
    pub(crate) lower: Option<f64>,
    pub(crate) upper: Option<f64>,
    pub(crate) penalty: Option<f64>,
}
impl GradeTargetValues {
    pub(crate) fn get(&self, field: GradeTargetInput) -> Option<f64> {
        match field {
            GradeTargetInput::Target => self.target,
            GradeTargetInput::Lower => self.lower,
            GradeTargetInput::Upper => self.upper,
            GradeTargetInput::Penalty => self.penalty,
        }
    }
    fn set(&mut self, field: GradeTargetInput, value: Option<f64>) {
        match field {
            GradeTargetInput::Target => self.target = value,
            GradeTargetInput::Lower => self.lower = value,
            GradeTargetInput::Upper => self.upper = value,
            GradeTargetInput::Penalty => self.penalty = value,
        }
    }
    fn validate(&self) -> ScheduleResult {
        if GradeTargetInput::ALL.iter().any(|&f| self.get(f).is_some_and(|v| !v.is_finite() || v < 0.0))
            || self.target.is_some_and(|t| self.lower.is_some_and(|v| v >= t) || self.upper.is_some_and(|v| v <= t))
        {
            Err(ScheduleError::InvalidGradeTarget)
        } else {
            Ok(())
        }
    }
}

/// A blank period cell inherits. An explicit clear (shown as “None”) suppresses
/// a default value, including a limit or the entire day's target.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GradeTargetValue {
    Clear,
    Number(f64),
}
impl GradeTargetValue {
    pub(crate) fn number(self) -> Option<f64> {
        match self {
            Self::Clear => None,
            Self::Number(v) => Some(v),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct GradeTargetOverride {
    pub(crate) target: Option<GradeTargetValue>,
    pub(crate) lower: Option<GradeTargetValue>,
    pub(crate) upper: Option<GradeTargetValue>,
    pub(crate) penalty: Option<GradeTargetValue>,
}
impl GradeTargetOverride {
    pub(crate) fn get(&self, field: GradeTargetInput) -> Option<GradeTargetValue> {
        match field {
            GradeTargetInput::Target => self.target,
            GradeTargetInput::Lower => self.lower,
            GradeTargetInput::Upper => self.upper,
            GradeTargetInput::Penalty => self.penalty,
        }
    }
    fn set(&mut self, field: GradeTargetInput, value: Option<GradeTargetValue>) {
        match field {
            GradeTargetInput::Target => self.target = value,
            GradeTargetInput::Lower => self.lower = value,
            GradeTargetInput::Upper => self.upper = value,
            GradeTargetInput::Penalty => self.penalty = value,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CrusherGradeCalendar {
    pub(crate) destination: DestinationId,
    pub(crate) field: ReserveFieldId,
    pub(crate) defaults: GradeTargetValues,
    pub(crate) periods: std::collections::BTreeMap<super::CalendarPeriod, GradeTargetOverride>,
    pub(crate) outside_multiplier: f64,
}
impl CrusherGradeCalendar {
    pub(crate) fn new(destination: DestinationId, field: ReserveFieldId) -> Self {
        Self {
            destination,
            field,
            defaults: GradeTargetValues::default(),
            periods: Default::default(),
            outside_multiplier: 2.0,
        }
    }
    pub(crate) fn resolved(&self, period: super::CalendarPeriod) -> GradeTargetValues {
        let mut values = self.defaults.clone();
        if let Some(overrides) = self.periods.get(&period) {
            for field in GradeTargetInput::ALL {
                if let Some(v) = overrides.get(field) {
                    values.set(field, v.number());
                }
            }
        }
        values
    }
    pub(crate) fn set_cell(&mut self, cell: super::CalendarCell, field: GradeTargetInput, value: Option<GradeTargetValue>) {
        match cell {
            super::CalendarCell::Default => self.defaults.set(field, value.and_then(GradeTargetValue::number)),
            super::CalendarCell::Period(period) => {
                let row = self.periods.entry(period).or_default();
                row.set(field, value);
                if *row == GradeTargetOverride::default() {
                    self.periods.remove(&period);
                }
            }
        }
    }
    pub(crate) fn specification(&self, period: super::CalendarPeriod) -> Option<GradeTarget> {
        let v = self.resolved(period);
        let target = v.target?;
        if v.lower.is_none() && v.upper.is_none() {
            return None;
        }
        Some(GradeTarget {
            destination: self.destination,
            field: self.field,
            lower: v.lower,
            target,
            upper: v.upper,
            penalty_per_tonne: v.penalty.unwrap_or(0.0),
            outside_multiplier: self.outside_multiplier,
        })
    }
    pub(crate) fn validate(&self) -> ScheduleResult {
        self.defaults.validate()?;
        if !self.outside_multiplier.is_finite() || self.outside_multiplier < 1.0 {
            return Err(ScheduleError::InvalidGradeTarget);
        }
        for period in std::iter::once(super::CalendarPeriod(0)).chain(self.periods.keys().copied()) {
            period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
            self.resolved(period).validate()?;
            if let Some(spec) = self.specification(period) {
                spec.validate()?;
            }
        }
        // Check the default independently even if day 1 overrides it.
        let default = Self {
            periods: Default::default(),
            ..self.clone()
        };
        if let Some(spec) = default.specification(super::CalendarPeriod(0)) {
            spec.validate()?;
        }
        Ok(())
    }
    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.destination.hash(hasher);
        self.field.hash(hasher);
        self.outside_multiplier.to_bits().hash(hasher);
        for field in GradeTargetInput::ALL {
            self.defaults.get(field).map(f64::to_bits).hash(hasher);
        }
        for (period, row) in &self.periods {
            period.hash(hasher);
            for field in GradeTargetInput::ALL {
                row.get(field)
                    .map(|v| match v {
                        GradeTargetValue::Clear => (0u8, 0),
                        GradeTargetValue::Number(v) => (1, v.to_bits()),
                    })
                    .hash(hasher);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GradeTargetCellEdit {
    pub(crate) destination: DestinationId,
    pub(crate) field: ReserveFieldId,
    pub(crate) cell: super::CalendarCell,
    pub(crate) input: GradeTargetInput,
    pub(crate) value: Option<GradeTargetValue>,
}
