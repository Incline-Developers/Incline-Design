//! The schedule's periods as calendar dates, and the colour each is shown in.
//!
//! The schedule itself counts hours from Day 1, 00:00, and nothing here moves
//! that: a start date only names Day 1, so every hour also reads as a date and
//! a date can be typed where an hour is wanted. Colours are the planner's own
//! markings, held per period (Day 1 is period 0) so moving the start date
//! keeps "the first week is green". Neither changes what is calculated.
//!
//! Periods are days. Weeks or months would be a different period length over
//! the same hours, which is why a period is addressed by its number rather
//! than its date.

use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

use super::SCHEDULE_PERIOD_H;

/// How a date is written and read: day, month, year.
pub(crate) const DATE_FORMAT: &str = "%d/%m/%Y";

/// The start date and the colours periods are marked with.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct SchedulePeriods {
    /// The date of Day 1. `None` until one is chosen: the schedule then reads
    /// in day numbers only, as projects from before periods did.
    #[serde(with = "iso_date", skip_serializing_if = "Option::is_none")]
    pub(crate) start_date: Option<NaiveDate>,
    /// Period number (Day 1 is 0) to its colour. A period with none is drawn
    /// as the Gantt and Animate draw an unmarked day.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) colors: BTreeMap<u32, [u8; 3]>,
}

impl SchedulePeriods {
    pub(crate) fn color(&self, period: u32) -> Option<[u8; 3]> {
        self.colors.get(&period).copied()
    }

    /// The date `period` falls on, when there is a start date.
    pub(crate) fn date_of_period(&self, period: u32) -> Option<NaiveDate> {
        self.start_date?.checked_add_signed(Duration::days(i64::from(period)))
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
