//! The schedule's periods as calendar dates, and the colour each is shown in.
//!
//! The schedule itself counts hours from Day 1, 00:00, and nothing here moves
//! that: a start date only names Day 1, so every hour also reads as a date and
//! a date can be typed where an hour is wanted. Colours are the planner's own
//! markings, held per period (Day 1 is period 0) so moving the start date
//! keeps "the first week is green". Neither changes what is calculated.
//!
//! A period is a day, a week or a month ([`PeriodUnit`]): a different length
//! over the same hours. Everything authored per period - loader calendars,
//! grade bands, stockpile modes, colours - is held per day whatever the unit,
//! so a week or a month reads and writes the days it covers ([`PeriodScale`])
//! and a schedule means the same thing under either.
//!
//! Each scenario has its own horizon - end, calculation step and period unit;
//! see [`super::scenarios`]. Projects from before scenarios kept two here, a
//! short term and a long term ([`SchedulePeriods::other`]), which become two
//! scenarios when they are opened.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

use super::SCHEDULE_PERIOD_H;

/// How a date is written and read: day, month, year.
pub(crate) const DATE_FORMAT: &str = "%d/%m/%Y";

/// How long a period is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum PeriodUnit {
    #[default]
    Day,
    /// Seven days, counted from Day 1.
    Week,
    /// A calendar month once there is a start date, the first running from
    /// Day 1 to the end of its month; four weeks before there is one, so
    /// months still nest the weeks, as on the Gantt's ruler.
    Month,
}

impl PeriodUnit {
    pub(crate) const ALL: [Self; 3] = [Self::Day, Self::Week, Self::Month];

    fn is_day(&self) -> bool {
        *self == Self::Day
    }
}

/// Days in a month before the schedule has a start date to count real ones from.
const UNDATED_MONTH_DAYS: u32 = 28;

/// Which days each period covers: the unit, and the date months are cut at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PeriodScale {
    pub(crate) unit: PeriodUnit,
    pub(crate) start: Option<NaiveDate>,
}

/// Months since year zero, so consecutive months are consecutive numbers.
fn month_number(date: NaiveDate) -> i64 {
    i64::from(date.year()) * 12 + i64::from(date.month0())
}

impl PeriodScale {
    /// The first day (Day 1 is 0) of `period`.
    pub(crate) fn first_day(self, period: u32) -> u32 {
        match (self.unit, self.start) {
            (PeriodUnit::Day, _) => period,
            (PeriodUnit::Week, _) => period.saturating_mul(7),
            (PeriodUnit::Month, None) => period.saturating_mul(UNDATED_MONTH_DAYS),
            (PeriodUnit::Month, Some(_)) if period == 0 => 0,
            (PeriodUnit::Month, Some(start)) => {
                let month = month_number(start) + i64::from(period);
                let first = i32::try_from(month.div_euclid(12))
                    .ok()
                    .and_then(|year| NaiveDate::from_ymd_opt(year, month.rem_euclid(12) as u32 + 1, 1));
                first.map_or(u32::MAX, |first| u32::try_from((first - start).num_days()).unwrap_or(u32::MAX))
            }
        }
    }

    /// The period `day` (Day 1 is 0) falls in.
    pub(crate) fn period_of_day(self, day: u32) -> u32 {
        match (self.unit, self.start) {
            (PeriodUnit::Day, _) => day,
            (PeriodUnit::Week, _) => day / 7,
            (PeriodUnit::Month, None) => day / UNDATED_MONTH_DAYS,
            (PeriodUnit::Month, Some(start)) => start
                .checked_add_signed(Duration::days(i64::from(day)))
                .map_or(0, |date| u32::try_from(month_number(date) - month_number(start)).unwrap_or(0)),
        }
    }

    /// The days `period` covers.
    pub(crate) fn days(self, period: u32) -> std::ops::Range<u32> {
        self.first_day(period)..self.first_day(period.saturating_add(1))
    }

    /// How many periods `days` days reach into, the last perhaps in part.
    pub(crate) fn count(self, days: u32) -> u32 {
        if days == 0 { 0 } else { self.period_of_day(days - 1) + 1 }
    }

    /// When `period` starts, in hours from Day 1, 00:00.
    pub(crate) fn start_h(self, period: u32) -> f64 {
        f64::from(self.first_day(period)) * SCHEDULE_PERIOD_H
    }

    /// The date `period` starts on, when there is a start date.
    pub(crate) fn date(self, period: u32) -> Option<NaiveDate> {
        self.start?.checked_add_signed(Duration::days(i64::from(self.first_day(period))))
    }
}

/// The two horizons of a project from before scenarios.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum Horizon {
    /// Days to weeks, worked out bar by bar by the planner.
    #[default]
    Short,
    /// Years, at a coarser step and in longer periods.
    Long,
}

impl Horizon {
    fn is_short(&self) -> bool {
        *self == Self::Short
    }
}

/// One horizon's settings: where it ends, the step it is calculated at and
/// the periods it is read in.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HorizonProfile {
    pub(crate) end_day: u32,
    pub(crate) interval_h: f64,
    pub(crate) unit: PeriodUnit,
}

impl Eq for HorizonProfile {}

impl std::hash::Hash for HorizonProfile {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.end_day.hash(hasher);
        self.interval_h.to_bits().hash(hasher);
        self.unit.hash(hasher);
    }
}

/// The start date, the colours days are marked with, and the open scenario's
/// period unit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct SchedulePeriods {
    /// The date of Day 1. `None` until one is chosen: the schedule then reads
    /// in day numbers only, as projects from before periods did.
    #[serde(with = "iso_date", skip_serializing_if = "Option::is_none")]
    pub(crate) start_date: Option<NaiveDate>,
    /// Day number (Day 1 is 0) to its colour. A day with none is drawn as
    /// the Gantt and Animate draw an unmarked day.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) colors: BTreeMap<u32, [u8; 3]>,
    /// Which of a project from before scenarios' two horizons was open.
    /// Read, turned into scenarios, and back to short when written.
    #[serde(skip_serializing_if = "Horizon::is_short")]
    pub(crate) horizon: Horizon,
    /// The open scenario's period unit.
    #[serde(skip_serializing_if = "PeriodUnit::is_day")]
    pub(crate) unit: PeriodUnit,
    /// The other horizon of a project from before scenarios. Read, turned
    /// into a scenario of its own, and never written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) other: Option<HorizonProfile>,
}

impl SchedulePeriods {
    /// The colour `day` (Day 1 is 0) is marked with.
    pub(crate) fn color(&self, day: u32) -> Option<[u8; 3]> {
        self.colors.get(&day).copied()
    }

    /// Which days each period of the open horizon covers.
    pub(crate) fn scale(&self) -> PeriodScale {
        PeriodScale {
            unit: self.unit,
            start: self.start_date,
        }
    }
}

/// Day `period`, and the hour within it, of `hours` from Day 1, 00:00.
fn split(hours: f64) -> (i64, u32) {
    let minutes = (hours.max(0.0) * 60.0).round() as i64;
    let period_minutes = (SCHEDULE_PERIOD_H * 60.0) as i64;
    (minutes / period_minutes, (minutes % period_minutes) as u32)
}

/// `hours` from Day 1, 00:00 as a date and time, when there is a start date.
pub(crate) fn date_time_at(start: NaiveDate, hours: f64) -> Option<NaiveDateTime> {
    let (day, minutes) = split(hours);
    let date = start.checked_add_signed(Duration::days(day))?;
    Some(date.and_time(NaiveTime::from_hms_opt(minutes / 60, minutes % 60, 0)?))
}

/// A date as it is written everywhere: 17/04/2026.
pub(crate) fn date_text(date: NaiveDate) -> String {
    date.format(DATE_FORMAT).to_string()
}

/// A date and time as it is written everywhere: 17/04/2026 12:00.
pub(crate) fn date_time_text(at: NaiveDateTime) -> String {
    format!("{} {:02}:{:02}", date_text(at.date()), at.hour(), at.minute())
}

/// Hours from Day 1, 00:00 of a typed date: `17/04/2026`, `17/04/2026 12:00`
/// or with seconds, the way a spreadsheet writes one; `2026-04-17` and
/// `2026-04-17 12:00` too. `None` for anything else, or a date before Day 1.
pub(crate) fn parse_date_time(text: &str, start: NaiveDate) -> Option<f64> {
    let text = text.trim();
    let (date, time) = match text.split_once([' ', 'T']) {
        Some((date, time)) => (date.trim(), Some(time.trim())),
        None => (text, None),
    };
    let date = NaiveDate::parse_from_str(date, DATE_FORMAT).or_else(|_| NaiveDate::parse_from_str(date, "%Y-%m-%d")).ok()?;
    let time = match time {
        None | Some("") => NaiveTime::MIN,
        Some(time) => NaiveTime::parse_from_str(time, "%H:%M").or_else(|_| NaiveTime::parse_from_str(time, "%H:%M:%S")).ok()?,
    };
    let days = (date - start).num_days();
    if days < 0 {
        return None;
    }
    Some(days as f64 * SCHEDULE_PERIOD_H + f64::from(time.num_seconds_from_midnight()) / 3600.0)
}

/// The date `days` whole periods after `start`, for an end date shown from a
/// horizon: the last day the schedule covers.
pub(crate) fn last_date(start: NaiveDate, days: u32) -> Option<NaiveDate> {
    start.checked_add_signed(Duration::days(i64::from(days.max(1)) - 1))
}

/// A date saved as `2026-04-17`, which reads the same in every locale.
mod iso_date {
    use chrono::NaiveDate;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(date: &Option<NaiveDate>, serializer: S) -> Result<S::Ok, S::Error> {
        match date {
            Some(date) => serializer.serialize_str(&date.format("%Y-%m-%d").to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<NaiveDate>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;
        text.map(|text| NaiveDate::parse_from_str(&text, "%Y-%m-%d").map_err(serde::de::Error::custom)).transpose()
    }
}
