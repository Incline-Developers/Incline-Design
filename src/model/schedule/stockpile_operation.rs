//! How a stockpile may be worked, day by day: whether it takes deliveries,
//! whether it may be reclaimed, or both. Authored in the Calendar as one Mode
//! row per stockpile - a Default plus sparse day overrides, like a crusher's
//! limit - and enforced by every schedule: a pile not building is passed over
//! by routing as if it were full, and a reclaim bar on a pile not reclaiming
//! has no work, so its loader moves on to its next bar.
//!
//! Two per-pile settings sit beside the calendar, in Setup: whether the pile
//! may take deliveries and be reclaimed in the same hour, and how many hours
//! new material must rest before it is reclaimed.

use serde::{Deserialize, Serialize};

use super::{CalendarCell, CalendarPeriod, DestinationId, ScheduleError, ScheduleResult};
use crate::i18n::tr;

/// What a stockpile may do on one day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PileMode {
    /// Deliveries and reclaim both allowed, as every pile was before modes.
    #[default]
    BuildAndReclaim,
    BuildOnly,
    ReclaimOnly,
    /// Neither: the pile stands.
    Off,
}

impl PileMode {
    pub(crate) const ALL: [Self; 4] = [Self::BuildAndReclaim, Self::BuildOnly, Self::ReclaimOnly, Self::Off];

    pub(crate) fn builds(self) -> bool {
        matches!(self, Self::BuildAndReclaim | Self::BuildOnly)
    }

    pub(crate) fn reclaims(self) -> bool {
        matches!(self, Self::BuildAndReclaim | Self::ReclaimOnly)
    }

    pub(crate) fn label(self) -> String {
        match self {
            Self::BuildAndReclaim => tr!("pile-mode-both"),
            Self::BuildOnly => tr!("pile-mode-build"),
            Self::ReclaimOnly => tr!("pile-mode-reclaim"),
            Self::Off => tr!("pile-mode-off"),
        }
    }

    /// Read a typed or pasted mode: its label, or one word of it - `both`,
    /// `build`, `reclaim` or `off`.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let typed = text.trim().to_lowercase();
        if let Some(mode) = Self::ALL.into_iter().find(|mode| mode.label().to_lowercase() == typed) {
            return Some(mode);
        }
        match typed.as_str() {
            "both" | "build & reclaim" | "build and reclaim" => Some(Self::BuildAndReclaim),
            "build" => Some(Self::BuildOnly),
            "reclaim" => Some(Self::ReclaimOnly),
            "off" | "none" => Some(Self::Off),
            _ => None,
        }
    }
}

/// One stockpile's operating calendar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StockpileOperation {
    pub(crate) destination: DestinationId,
    #[serde(default)]
    pub(crate) default_mode: PileMode,
    #[serde(default)]
    pub(crate) periods: std::collections::BTreeMap<CalendarPeriod, PileMode>,
    /// Whether deliveries and reclaim may happen in the same interval.
    #[serde(default = "simultaneous_default")]
    pub(crate) simultaneous: bool,
    /// Hours material must rest before it is reclaimed: since the last
    /// delivery for a blended pile, since its chunk closed for a chunked one.
    #[serde(default)]
    pub(crate) rest_h: f64,
}

fn simultaneous_default() -> bool {
    true
}

impl StockpileOperation {
    pub(crate) fn new(destination: DestinationId) -> Self {
        Self {
            destination,
            default_mode: PileMode::default(),
            periods: Default::default(),
            simultaneous: true,
            rest_h: 0.0,
        }
    }

    pub(crate) fn mode_at(&self, period: CalendarPeriod) -> PileMode {
        self.periods.get(&period).copied().unwrap_or(self.default_mode)
    }

    /// `None` on a day clears its override; on the Default it restores
    /// building and reclaiming.
    pub(crate) fn set_cell(&mut self, cell: CalendarCell, mode: Option<PileMode>) {
        match cell {
            CalendarCell::Default => self.default_mode = mode.unwrap_or_default(),
            CalendarCell::Period(period) => match mode {
                Some(mode) => {
                    self.periods.insert(period, mode);
                }
                None => {
                    self.periods.remove(&period);
                }
            },
        }
    }

    pub(crate) fn is_pristine(&self) -> bool {
        self.default_mode == PileMode::default() && self.periods.is_empty() && self.simultaneous && self.rest_h == 0.0
    }

    pub(crate) fn validate(&self) -> ScheduleResult {
        checked_rest(self.rest_h)?;
        for period in self.periods.keys() {
            period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
        }
        Ok(())
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.destination.hash(hasher);
        self.default_mode.hash(hasher);
        self.simultaneous.hash(hasher);
        self.rest_h.to_bits().hash(hasher);
        for (period, mode) in &self.periods {
            period.hash(hasher);
            mode.hash(hasher);
        }
    }
}

pub(crate) fn checked_rest(hours: f64) -> ScheduleResult<f64> {
    if hours.is_finite() && hours >= 0.0 {
        Ok(hours)
    } else {
        Err(ScheduleError::InvalidExperimentSetting)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PileModeCellEdit {
    pub(crate) destination: DestinationId,
    pub(crate) cell: CalendarCell,
    pub(crate) mode: Option<PileMode>,
}
