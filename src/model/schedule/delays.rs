//! Machine delays: the time a loader is known not to be working.
//!
//! Three ways to say it, because planners know about delays in three ways:
//!
//! - A **delay list** is a titled table of one-off delays - "Maintenance",
//!   with one row per machine and outage - typed in or pasted from a
//!   spreadsheet.
//! - A **roster** is a delay that repeats: a shift change every 12 hours, a
//!   weekly service. One row describes every occurrence.
//! - A **delay bar** is a bar on the Gantt (see [`super::BarWork::Delay`]). It
//!   competes for the machine like any other bar: while it is the highest
//!   priority bar with its window open, the machine stands.
//!
//! The first two are the machine's calendar: the schedule gives the machine
//! no rate for those hours, whatever bars it holds. A delay bar is priority
//! instead, which is what lets work in a higher lane carry on through it.
//!
//! Every delay may name a *delay type*: a name and a colour for the Gantt.
//! The type says what kind of delay it is; it changes nothing about how the
//! schedule treats the time.

use serde::{Deserialize, Serialize};

use super::{LoaderAgentId, LoaderSelection, ScheduleError, ScheduleResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct DelayTypeId(pub(crate) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct DelayListId(pub(crate) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct RosterId(pub(crate) u64);

/// What kind of delay something is, and the colour the Gantt draws it in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DelayType {
    pub(crate) id: DelayTypeId,
    pub(crate) name: String,
    /// sRGB.
    pub(crate) color: [u8; 3],
}

/// One outage of one machine.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DelayEntry {
    pub(crate) agent: LoaderAgentId,
    /// Hours from the schedule origin; the end is exclusive.
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
}

impl DelayEntry {
    pub(crate) fn is_valid(self) -> bool {
        self.start_h.is_finite() && self.start_h >= 0.0 && self.end_h.is_finite() && self.end_h > self.start_h
    }
}

/// A titled table of one-off delays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DelayList {
    pub(crate) id: DelayListId,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) kind: Option<DelayTypeId>,
    #[serde(default)]
    pub(crate) entries: Vec<DelayEntry>,
}

/// A delay that repeats: `duration_h` long, starting at `first_start_h` and
/// again every `every_h`, on the machines `loaders` selects, until
/// `until_h` when there is one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Roster {
    pub(crate) id: RosterId,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) kind: Option<DelayTypeId>,
    #[serde(default)]
    pub(crate) loaders: LoaderSelection,
    pub(crate) first_start_h: f64,
    pub(crate) duration_h: f64,
    pub(crate) every_h: f64,
    #[serde(default)]
    pub(crate) until_h: Option<f64>,
}

impl Roster {
    /// A finite start at or after the origin, a positive duration shorter
    /// than the repeat, and an end - when there is one - after the start.
    /// A delay as long as its repeat is the machine never working, which is
    /// what availability says; it is refused here rather than read as that.
    pub(crate) fn is_valid(&self) -> bool {
        self.first_start_h.is_finite()
            && self.first_start_h >= 0.0
            && self.duration_h.is_finite()
            && self.duration_h > 0.0
            && self.every_h.is_finite()
            && self.every_h > self.duration_h
            && self.until_h.is_none_or(|until| until.is_finite() && until > self.first_start_h)
            && match &self.loaders {
                LoaderSelection::All => true,
                LoaderSelection::Only(agents) => !agents.is_empty(),
            }
    }

    pub(crate) fn applies_to(&self, agent: LoaderAgentId) -> bool {
        match &self.loaders {
            LoaderSelection::All => true,
            LoaderSelection::Only(agents) => agents.contains(&agent),
        }
    }

    /// Every occurrence that starts before `horizon_h`, clipped to it and to
    /// `until_h`.
    pub(crate) fn occurrences(&self, horizon_h: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
        let stop = self.until_h.map_or(horizon_h, |until| until.min(horizon_h));
        (0u64..)
            .map(move |index| self.first_start_h + index as f64 * self.every_h)
            .take_while(move |start| *start < stop)
            .map(move |start| (start, (start + self.duration_h).min(stop)))
    }
}

/// Where one span of delay came from, so the Gantt can name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DelaySource {
    List(DelayListId),
    Roster(RosterId),
}

/// One stretch of calendar delay on one machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DelaySpan {
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    pub(crate) source: DelaySource,
    pub(crate) kind: Option<DelayTypeId>,
}

/// Every delay type, delay list and roster in one project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DelayConfig {
    pub(crate) types: Vec<DelayType>,
    pub(crate) lists: Vec<DelayList>,
    pub(crate) rosters: Vec<Roster>,
    next_type_id: u64,
    next_list_id: u64,
    next_roster_id: u64,
}

/// Colours a new delay type starts with, in turn: distinct from the working
/// blue and the idle amber the Gantt already uses.
const PALETTE: [[u8; 3]; 6] = [
    [0xC8, 0x4B, 0x4B],
    [0x8E, 0x6F, 0xC9],
    [0x6B, 0x7A, 0x8F],
    [0xD0, 0x7A, 0x3A],
    [0xB0, 0x5C, 0x9A],
    [0x4E, 0x8C, 0x6E],
];

fn checked_title(name: &str) -> ScheduleResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ScheduleError::EmptyName);
    }
    Ok(name.to_owned())
}

fn next(counter: &mut u64) -> ScheduleResult<u64> {
    let id = *counter;
    *counter = counter.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
    Ok(id)
}

impl DelayConfig {
    pub(crate) fn is_empty(&self) -> bool {
        self.types.is_empty() && self.lists.is_empty() && self.rosters.is_empty()
    }

    pub(crate) fn delay_type(&self, id: DelayTypeId) -> Option<&DelayType> {
        self.types.iter().find(|entry| entry.id == id)
    }

    pub(crate) fn list(&self, id: DelayListId) -> Option<&DelayList> {
        self.lists.iter().find(|entry| entry.id == id)
    }

    pub(crate) fn roster(&self, id: RosterId) -> Option<&Roster> {
        self.rosters.iter().find(|entry| entry.id == id)
    }

    /// The colour a new type should start with.
    pub(crate) fn suggested_color(&self) -> [u8; 3] {
        PALETTE[self.types.len() % PALETTE.len()]
    }

    pub(crate) fn add_type(&mut self, name: &str, color: [u8; 3]) -> ScheduleResult<DelayTypeId> {
        let name = checked_title(name)?;
        if self.types.iter().any(|entry| super::same_name(&entry.name, &name)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = DelayTypeId(next(&mut self.next_type_id)?);
        self.types.push(DelayType { id, name, color });
        Ok(id)
    }

    pub(crate) fn rename_type(&mut self, id: DelayTypeId, name: &str) -> ScheduleResult {
        let name = checked_title(name)?;
        if self.types.iter().any(|entry| entry.id != id && super::same_name(&entry.name, &name)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.types.iter_mut().find(|entry| entry.id == id).ok_or(ScheduleError::UnknownDelay)?.name = name;
        Ok(())
    }

    pub(crate) fn set_type_color(&mut self, id: DelayTypeId, color: [u8; 3]) -> ScheduleResult {
        self.types.iter_mut().find(|entry| entry.id == id).ok_or(ScheduleError::UnknownDelay)?.color = color;
        Ok(())
    }

    /// Lists and rosters of this type, by name. Bars are the plan's to check.
    pub(crate) fn type_users(&self, id: DelayTypeId) -> Vec<String> {
        self.lists
            .iter()
            .filter(|list| list.kind == Some(id))
            .map(|list| list.title.clone())
            .chain(self.rosters.iter().filter(|roster| roster.kind == Some(id)).map(|roster| roster.name.clone()))
            .collect()
    }

    pub(crate) fn remove_type(&mut self, id: DelayTypeId) -> ScheduleResult {
        if !self.types.iter().any(|entry| entry.id == id) {
            return Err(ScheduleError::UnknownDelay);
        }
        self.types.retain(|entry| entry.id != id);
        Ok(())
    }

    pub(crate) fn add_list(&mut self, title: &str, kind: Option<DelayTypeId>) -> ScheduleResult<DelayListId> {
        let title = checked_title(title)?;
        if self.lists.iter().any(|entry| super::same_name(&entry.title, &title)) {
            return Err(ScheduleError::DuplicateName(title));
        }
        self.check_kind(kind)?;
        let id = DelayListId(next(&mut self.next_list_id)?);
        self.lists.push(DelayList {
            id,
            title,
            kind,
            entries: Vec::new(),
        });
        Ok(id)
    }

    pub(crate) fn rename_list(&mut self, id: DelayListId, title: &str) -> ScheduleResult {
        let title = checked_title(title)?;
        if self.lists.iter().any(|entry| entry.id != id && super::same_name(&entry.title, &title)) {
            return Err(ScheduleError::DuplicateName(title));
        }
        self.list_mut(id)?.title = title;
        Ok(())
    }

    pub(crate) fn set_list_kind(&mut self, id: DelayListId, kind: Option<DelayTypeId>) -> ScheduleResult {
        self.check_kind(kind)?;
        self.list_mut(id)?.kind = kind;
        Ok(())
    }

    /// Replace a list's whole table: what an edited cell, an added or
    /// deleted row and a paste all are, so each is one undo step. Every row
    /// is checked first and a refusal leaves the table untouched. Rows are
    /// kept in the order given - it is the table the planner is looking at.
    pub(crate) fn set_list_entries(&mut self, id: DelayListId, entries: Vec<DelayEntry>, agents: &[LoaderAgentId]) -> ScheduleResult {
        for entry in &entries {
            if !entry.is_valid() {
                return Err(ScheduleError::InvalidWindow);
            }
            if !agents.contains(&entry.agent) {
                return Err(ScheduleError::UnknownAgent);
            }
        }
        self.list_mut(id)?.entries = entries;
        Ok(())
    }

    pub(crate) fn remove_list(&mut self, id: DelayListId) -> ScheduleResult {
        if !self.lists.iter().any(|entry| entry.id == id) {
            return Err(ScheduleError::UnknownDelay);
        }
        self.lists.retain(|entry| entry.id != id);
        Ok(())
    }

    /// Add a roster: a daily one-hour delay at 06:00 on every machine to
    /// begin with, which is a shift change and something to edit.
    pub(crate) fn add_roster(&mut self, name: &str, kind: Option<DelayTypeId>) -> ScheduleResult<RosterId> {
        let name = checked_title(name)?;
        if self.rosters.iter().any(|entry| super::same_name(&entry.name, &name)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.check_kind(kind)?;
        let id = RosterId(next(&mut self.next_roster_id)?);
        self.rosters.push(Roster {
            id,
            name,
            kind,
            loaders: LoaderSelection::All,
            first_start_h: 6.0,
            duration_h: 1.0,
            every_h: 24.0,
            until_h: None,
        });
        Ok(id)
    }

    /// Replace one roster's settings - everything but its identity - after
    /// checking them.
    pub(crate) fn set_roster(&mut self, roster: Roster, agents: &[LoaderAgentId]) -> ScheduleResult {
        let name = checked_title(&roster.name)?;
        if self.rosters.iter().any(|entry| entry.id != roster.id && super::same_name(&entry.name, &name)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        if !roster.is_valid() {
            return Err(ScheduleError::InvalidWindow);
        }
        if let LoaderSelection::Only(selected) = &roster.loaders
            && selected.iter().any(|agent| !agents.contains(agent))
        {
            return Err(ScheduleError::UnknownAgent);
        }
        self.check_kind(roster.kind)?;
        let slot = self.rosters.iter_mut().find(|entry| entry.id == roster.id).ok_or(ScheduleError::UnknownDelay)?;
        *slot = Roster { name, ..roster };
        Ok(())
    }

    pub(crate) fn remove_roster(&mut self, id: RosterId) -> ScheduleResult {
        if !self.rosters.iter().any(|entry| entry.id == id) {
            return Err(ScheduleError::UnknownDelay);
        }
        self.rosters.retain(|entry| entry.id != id);
        Ok(())
    }

    /// A machine has been deleted: its rows go from every list and its name
    /// from every roster's selection. A roster left selecting nobody goes back
    /// to every machine rather than becoming one that matches nothing - which
    /// the editor would refuse to save.
    pub(crate) fn forget_agent(&mut self, agent: LoaderAgentId) {
        for list in &mut self.lists {
            list.entries.retain(|entry| entry.agent != agent);
        }
        for roster in &mut self.rosters {
            if let LoaderSelection::Only(selected) = &mut roster.loaders {
                selected.retain(|candidate| *candidate != agent);
                if selected.is_empty() {
                    roster.loaders = LoaderSelection::All;
                }
            }
        }
    }

    fn list_mut(&mut self, id: DelayListId) -> ScheduleResult<&mut DelayList> {
        self.lists.iter_mut().find(|entry| entry.id == id).ok_or(ScheduleError::UnknownDelay)
    }

    fn check_kind(&self, kind: Option<DelayTypeId>) -> ScheduleResult {
        if kind.is_some_and(|kind| self.delay_type(kind).is_none()) {
            return Err(ScheduleError::UnknownDelay);
        }
        Ok(())
    }

    /// Every stretch of calendar delay on `agent` before `horizon_h`, as
    /// authored: overlapping delays stay separate so each can be named.
    pub(crate) fn spans_for(&self, agent: LoaderAgentId, horizon_h: f64) -> Vec<DelaySpan> {
        let mut spans = Vec::new();
        for list in &self.lists {
            for entry in list.entries.iter().filter(|entry| entry.agent == agent && entry.start_h < horizon_h) {
                spans.push(DelaySpan {
                    start_h: entry.start_h,
                    end_h: entry.end_h.min(horizon_h),
                    source: DelaySource::List(list.id),
                    kind: list.kind,
                });
            }
        }
        for roster in self.rosters.iter().filter(|roster| roster.applies_to(agent)) {
            for (start_h, end_h) in roster.occurrences(horizon_h) {
                spans.push(DelaySpan {
                    start_h,
                    end_h,
                    source: DelaySource::Roster(roster.id),
                    kind: roster.kind,
                });
            }
        }
        spans.sort_by(|left, right| left.start_h.total_cmp(&right.start_h).then(left.end_h.total_cmp(&right.end_h)));
        spans
    }

    /// The same, merged into disjoint stretches: the hours the machine has no
    /// rate.
    pub(crate) fn merged_for(&self, agent: LoaderAgentId, horizon_h: f64) -> Vec<(f64, f64)> {
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for span in self.spans_for(agent, horizon_h) {
            match merged.last_mut() {
                Some(last) if span.start_h <= last.1 => last.1 = last.1.max(span.end_h),
                _ => merged.push((span.start_h, span.end_h)),
            }
        }
        merged
    }

    pub(crate) fn validate_loaded(&mut self, agents: &[LoaderAgentId]) -> ScheduleResult {
        for (index, entry) in self.types.iter().enumerate() {
            checked_title(&entry.name)?;
            if self.types[..index].iter().any(|earlier| earlier.id == entry.id) {
                return Err(ScheduleError::DuplicateId);
            }
        }
        for (index, list) in self.lists.iter().enumerate() {
            checked_title(&list.title)?;
            if self.lists[..index].iter().any(|earlier| earlier.id == list.id) {
                return Err(ScheduleError::DuplicateId);
            }
            self.check_kind(list.kind)?;
            for entry in &list.entries {
                if !entry.is_valid() {
                    return Err(ScheduleError::InvalidWindow);
                }
                if !agents.contains(&entry.agent) {
                    return Err(ScheduleError::UnknownAgent);
                }
            }
        }
        for (index, roster) in self.rosters.iter().enumerate() {
            checked_title(&roster.name)?;
            if self.rosters[..index].iter().any(|earlier| earlier.id == roster.id) {
                return Err(ScheduleError::DuplicateId);
            }
            self.check_kind(roster.kind)?;
            if !roster.is_valid() {
                return Err(ScheduleError::InvalidWindow);
            }
        }
        for (counter, highest) in [
            (&mut self.next_type_id, self.types.iter().map(|entry| entry.id.0).max()),
            (&mut self.next_list_id, self.lists.iter().map(|entry| entry.id.0).max()),
            (&mut self.next_roster_id, self.rosters.iter().map(|entry| entry.id.0).max()),
        ] {
            if let Some(highest) = highest {
                *counter = (*counter).max(highest.checked_add(1).ok_or(ScheduleError::IdsExhausted)?);
            }
        }
        Ok(())
    }

    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_type_id = self.next_type_id.max(other.next_type_id);
        self.next_list_id = self.next_list_id.max(other.next_list_id);
        self.next_roster_id = self.next_roster_id.max(other.next_roster_id);
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        for entry in &self.types {
            entry.id.hash(hasher);
            entry.name.hash(hasher);
            entry.color.hash(hasher);
        }
        for list in &self.lists {
            list.id.hash(hasher);
            list.title.hash(hasher);
            list.kind.hash(hasher);
            for entry in &list.entries {
                entry.agent.hash(hasher);
                entry.start_h.to_bits().hash(hasher);
                entry.end_h.to_bits().hash(hasher);
            }
        }
        for roster in &self.rosters {
            roster.id.hash(hasher);
            roster.name.hash(hasher);
            roster.kind.hash(hasher);
            match &roster.loaders {
                LoaderSelection::All => 0u8.hash(hasher),
                LoaderSelection::Only(agents) => agents.hash(hasher),
            }
            roster.first_start_h.to_bits().hash(hasher);
            roster.duration_h.to_bits().hash(hasher);
            roster.every_h.to_bits().hash(hasher);
            roster.until_h.map(f64::to_bits).hash(hasher);
        }
    }

    /// Only what the calculation reads: which hours each machine is delayed.
    /// Names, colours and types are presentation.
    pub(crate) fn hash_calendar<H: std::hash::Hasher>(&self, agents: &[LoaderAgentId], horizon_h: f64, hasher: &mut H) {
        use std::hash::Hash;
        for agent in agents {
            for (start, end) in self.merged_for(*agent, horizon_h) {
                agent.hash(hasher);
                start.to_bits().hash(hasher);
                end.to_bits().hash(hasher);
            }
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.types.iter().map(|entry| size_of::<DelayType>() + entry.name.len()).sum::<usize>()
            + self
                .lists
                .iter()
                .map(|list| size_of::<DelayList>() + list.title.len() + list.entries.len() * size_of::<DelayEntry>())
                .sum::<usize>()
            + self.rosters.iter().map(|roster| size_of::<Roster>() + roster.name.len()).sum::<usize>()
    }
}

/// Read an instant typed or pasted by a planner, in hours from the schedule
/// origin.
///
/// Accepts the Gantt's own form, `Day 3 06:00`, with or without the word
/// `Day` and with or without a time, and a plain number of hours. Day 1 is
/// the first day, as the Gantt labels it.
pub(crate) fn parse_instant(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(hours) = text.parse::<f64>() {
        return (hours.is_finite() && hours >= 0.0).then_some(hours);
    }
    let lower = text.to_ascii_lowercase();
    let rest = lower.strip_prefix("day").or_else(|| lower.strip_prefix('d')).unwrap_or(&lower).trim_start();
    let mut parts = rest.split(|c: char| c.is_whitespace() || c == ',' || c == 'T' || c == 't').filter(|part| !part.is_empty());
    let day: u32 = parts.next()?.parse().ok()?;
    if day == 0 {
        return None;
    }
    let minutes = match parts.next() {
        None => 0.0,
        Some(time) => {
            let (hours, minutes) = time.split_once(':').unwrap_or((time, "0"));
            let hours: u32 = hours.parse().ok()?;
            let minutes: u32 = minutes.parse().ok()?;
            if hours > 24 || minutes > 59 || (hours == 24 && minutes > 0) {
                return None;
            }
            f64::from(hours * 60 + minutes)
        }
    };
    if parts.next().is_some() {
        return None;
    }
    Some(f64::from(day - 1) * 24.0 + minutes / 60.0)
}

/// Write hours from the origin as the day number and `HH:MM` the Gantt uses.
pub(crate) fn instant_parts(hours: f64) -> (i64, String) {
    let total_minutes = (hours * 60.0).round() as i64;
    let day = total_minutes.div_euclid(24 * 60) + 1;
    let into_day = total_minutes.rem_euclid(24 * 60);
    (day, format!("{:02}:{:02}", into_day / 60, into_day % 60))
}

/// Write a duration in hours the way it is typed back: `1`, `0.5`, `12`.
pub(crate) fn hours_text(hours: f64) -> String {
    let rounded = (hours * 1000.0).round() / 1000.0;
    if rounded.fract() == 0.0 { format!("{rounded:.0}") } else { format!("{rounded}") }
}

/// Why one pasted row could not be read.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DelayRowProblem {
    /// Fewer than the three columns: machine, start, end.
    Columns,
    UnknownMachine(String),
    Start(String),
    End(String),
    /// An end at or before its start.
    Order,
}

/// Rows pasted from a spreadsheet or a CSV file: machine, start and end, one
/// delay per line, separated by tabs (what a spreadsheet copies), commas or
/// semicolons. Columns after the third are ignored. A first line that is not
/// a delay is read as the header and skipped. Machines are matched by name
/// the way the fleet's own names are compared.
///
/// All or nothing: one unreadable line refuses the paste, with every problem
/// reported by its 1-based line number, so a table is never half-filled.
pub(crate) fn parse_delay_rows(text: &str, agents: &[(LoaderAgentId, String)]) -> Result<Vec<DelayEntry>, Vec<(usize, DelayRowProblem)>> {
    let mut entries = Vec::new();
    let mut problems = Vec::new();
    let mut first = true;
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let separator = if line.contains('\t') {
            '\t'
        } else if line.contains(',') {
            ','
        } else {
            ';'
        };
        let columns: Vec<&str> = line.split(separator).map(|column| column.trim().trim_matches('"').trim()).collect();
        let header = std::mem::take(&mut first);
        if columns.len() < 3 {
            problems.push((index + 1, DelayRowProblem::Columns));
            continue;
        }
        let agent = agents.iter().find(|(_, name)| super::same_name(name, columns[0])).map(|(id, _)| *id);
        let start = parse_instant(columns[1]);
        let end = parse_instant(columns[2]);
        if header && agent.is_none() && start.is_none() {
            continue;
        }
        let Some(agent) = agent else {
            problems.push((index + 1, DelayRowProblem::UnknownMachine(columns[0].to_owned())));
            continue;
        };
        let Some(start_h) = start else {
            problems.push((index + 1, DelayRowProblem::Start(columns[1].to_owned())));
            continue;
        };
        let Some(end_h) = end else {
            problems.push((index + 1, DelayRowProblem::End(columns[2].to_owned())));
            continue;
        };
        if end_h <= start_h {
            problems.push((index + 1, DelayRowProblem::Order));
            continue;
        }
        entries.push(DelayEntry { agent, start_h, end_h });
    }
    if problems.is_empty() { Ok(entries) } else { Err(problems) }
}
