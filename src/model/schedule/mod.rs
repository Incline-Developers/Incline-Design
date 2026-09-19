//! The Schedule workspace's persistent setup: the loader fleet.
//!
//! A *loader class* is a machine type with a dig rate - `Liebherr 9400` at
//! 3000 tph. A *loader agent* is one machine of that type on this site -
//! `EX7001` - and gets its rate from the class rather than carrying a second
//! copy of it, so changing the class rate changes every machine of that type
//! at once.
//!
//! The plan is a plain document field: every edit is a whole-plan snapshot
//! swapped in by [`crate::model::Command::SetSchedulePlan`], which is what
//! makes one committed cell edit exactly one undo step. Validation lives here
//! rather than in the UI, so a command that arrives from anywhere - a dialog,
//! a replayed undo, a future script - is checked the same way.

pub(crate) mod animation;
pub(crate) mod calendar;
pub(crate) mod dispatch;
pub(crate) mod sequence;

pub(crate) use calendar::{CalendarCell, CalendarCellEdit, CalendarField, CalendarPeriod, CompiledRateCalendar, LoaderCalendar, SCHEDULE_PERIOD_H};
pub(crate) use dispatch::{DispatchAgent, DispatchBar, DispatchBlock, DispatchError, DispatchInput, DispatchOutcome, DispatchSchedule};
pub(crate) use sequence::{DigBlockRef, DigOrder, Footprint};

/// A pick of one dig block, as the 3D editor submits it: a block of *this*
/// run. The command boundary checks both halves against the current snapshot
/// and constructs the persistent reference there - the UI never builds a
/// reference or its provenance itself.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct DigBlockPick {
    pub(crate) block: crate::model::DigBlockId,
    pub(crate) generation: u64,
}
use serde::{Deserialize, Serialize};

use crate::{
    i18n::{tr, tr_format},
    model::ReserveFieldId,
};

/// Identity of one loader class within its project.
///
/// Allocated once and never reused, so a rename leaves every agent pointing
/// at the same machine type. Serialized, so it survives save and reopen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct LoaderClassId(pub(crate) u64);

/// Identity of one loader agent within its project. See [`LoaderClassId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct LoaderAgentId(pub(crate) u64);

/// Identity of one Gantt bar within its project. Allocated like the fleet ids
/// beside it: never reused, and not rewound by an undo - which is what lets a
/// copy be told from its original across one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct BarId(pub(crate) u64);

/// The period a bar is allowed to be worked in.
///
/// Start-inclusive and end-exclusive: a bar is eligible at `start_h` and not
/// at `end_h`, which is what stops a bar that ends where another begins from
/// being worked twice at the boundary instant.
///
/// An absent `end_h` is open-ended - the bar is available from `start_h`
/// onwards. It is not a very large number: "until the work runs out" and
/// "until Tuesday" are different statements, and only one of them survives a
/// change of horizon.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkWindow {
    pub(crate) start_h: f64,
    #[serde(default)]
    pub(crate) end_h: Option<f64>,
}

impl Default for WorkWindow {
    fn default() -> Self {
        Self { start_h: 0.0, end_h: None }
    }
}

impl WorkWindow {
    /// Whether this is a window at all: a finite, non-negative start, and an
    /// end that is finite and strictly after it. Checked at the command
    /// boundary and again on load, never clamped into shape.
    pub(crate) fn is_valid(self) -> bool {
        self.start_h.is_finite() && self.start_h >= 0.0 && self.end_h.is_none_or(|end| end.is_finite() && end > self.start_h)
    }

    /// Start-inclusive, end-exclusive.
    pub(crate) fn contains(self, hour: f64) -> bool {
        hour >= self.start_h && self.end_h.is_none_or(|end| hour < end)
    }
}

/// One authored bar on the Gantt: a named dig order, the machine asked to
/// work it, the lane it competes in, and the period it may be worked in.
///
/// A bar carries no duration of its own work. Its window is the period a
/// loader is *allowed* to work it; what is actually executed in that period
/// is the evaluator's answer, drawn as a separate indicator, and a bar whose
/// work outlasts its window simply leaves the rest in the ground for a later
/// bar to take.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScheduleBar {
    pub(crate) id: BarId,
    /// What this bar is called and the ground it works, in order. Owned
    /// outright: a copy holds its own, so editing one never reaches another.
    pub(crate) order: DigOrder,
    /// The loader this bar is assigned to, or `None` while it is being built,
    /// and after the machine it named was deleted: deleting a machine
    /// unassigns its bars rather than taking their work out of the project.
    #[serde(default)]
    pub(crate) agent: Option<LoaderAgentId>,
    /// Lane within that loader. Lower is higher priority; ties break on the
    /// window's start and then `id`, so the order is never arbitrary.
    #[serde(default)]
    pub(crate) priority: u32,
    /// The period this bar may be worked in, in hours from the schedule
    /// origin. This *is* the bar's extent on the Gantt: dragging it moves the
    /// window and dragging an edge resizes it.
    #[serde(default)]
    pub(crate) window: WorkWindow,
}

impl ScheduleBar {
    pub(crate) fn name(&self) -> &str {
        &self.order.name
    }

    pub(crate) fn has_custom_name(&self) -> bool {
        !self.order.name.trim().is_empty()
    }

    pub(crate) fn members(&self) -> &[DigBlockRef] {
        self.order.members()
    }
}

/// A machine type: what it is called, and how fast it digs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoaderClass {
    pub(crate) id: LoaderClassId,
    pub(crate) name: String,
    /// Tonnes per hour. Strictly positive and finite; see [`ScheduleError`].
    pub(crate) default_dig_rate_tph: f64,
}

/// One machine on site, of one class.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoaderAgent {
    pub(crate) id: LoaderAgentId,
    pub(crate) name: String,
    /// The class this machine is, which is also where its rate comes from.
    /// Never optional: an agent with no class has no rate, and stage 1 has
    /// no per-agent override to fall back on.
    pub(crate) class_id: LoaderClassId,
    /// Availability, utilisation, and sparse per-period rate overrides.
    #[serde(default)]
    pub(crate) calendar: LoaderCalendar,
}

/// Why an edit to the plan was refused.
///
/// Every variant carries enough to say which name or machine is at fault, so
/// the message a user reads names the thing they typed rather than the rule
/// it broke.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ScheduleError {
    /// A name that is empty once trimmed.
    EmptyName,
    /// A name another class or agent already has, compared case-insensitively.
    DuplicateName(String),
    /// A dig rate that is zero, negative, infinite or NaN.
    InvalidRate,
    InvalidCalendarPercentage,
    EmptyCalendarOverride,
    ReadOnlyCalendarCell,
    DuplicateCalendarCell,
    CalendarPeriodOverflow,
    UnrepresentableEffectiveRate,
    /// An id that names nothing in this plan - a command from a stale UI, or
    /// a file whose agent outlived its class.
    UnknownClass,
    UnknownAgent,
    /// A class still assigned to the named agents, which must be reassigned
    /// or deleted first. Deletion never cascades.
    ClassInUse(Vec<String>),
    /// All 2^64 ids of one kind handed out. Reported rather than wrapped: a
    /// reused id would silently rebind an agent to the wrong class.
    IdsExhausted,
    /// Two entries share one id. Only reachable from a file.
    DuplicateId,
    /// An id that names no bar in this plan.
    UnknownBar,
    /// A position that is past the end of a bar's dig order.
    UnknownMember,
    /// Ground already in this dig order. The order is what the bar *is*, so
    /// the same block cannot hold two places in it.
    DuplicateMember,
    /// An earliest start that is not a finite number of hours at or after the
    /// schedule origin. Refused rather than clamped: a bar that silently
    /// moved to hour zero would be scheduled against a time nobody chose.
    /// A window that is not one: a start that is negative or not finite, or
    /// an end that is not finite or does not come after its start.
    InvalidWindow,
    /// Gantt bars must remain large enough to interact with and small enough
    /// not to make a single loader consume the whole workspace.
    InvalidBarHeight,
    /// A stored reference whose numbers could not describe any ground.
    MalformedReference,
}

impl ScheduleError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::EmptyName => tr!("schedule-error-empty-name"),
            Self::DuplicateName(name) => tr!("schedule-error-duplicate-name", name = name.clone()),
            Self::InvalidRate => tr!("schedule-error-invalid-rate"),
            Self::InvalidCalendarPercentage => tr!("schedule-calendar-invalid-percentage"),
            Self::EmptyCalendarOverride => tr!("schedule-calendar-empty-override"),
            Self::ReadOnlyCalendarCell => tr!("schedule-calendar-read-only"),
            Self::DuplicateCalendarCell => tr!("schedule-calendar-duplicate-cell"),
            Self::CalendarPeriodOverflow => tr!("schedule-calendar-period-overflow"),
            Self::UnrepresentableEffectiveRate => tr!("schedule-calendar-effective-rate"),
            Self::UnknownClass => tr!("schedule-error-unknown-class"),
            Self::UnknownAgent => tr!("schedule-error-unknown-agent"),
            Self::ClassInUse(agents) => tr!("schedule-error-class-in-use", agents = agents.join(", ")),
            Self::IdsExhausted => tr!("schedule-error-ids-exhausted"),
            Self::DuplicateId => tr!("schedule-error-duplicate-id"),
            Self::UnknownBar => tr!("schedule-error-unknown-bar"),
            Self::UnknownMember => tr!("schedule-error-unknown-member"),
            Self::DuplicateMember => tr!("schedule-error-duplicate-member"),
            Self::MalformedReference => tr!("schedule-error-malformed-reference"),
            Self::InvalidWindow => tr!("schedule-error-invalid-window"),
            Self::InvalidBarHeight => tr!("schedule-error-invalid-bar-height"),
        }
    }
}

pub(crate) type ScheduleResult<T = ()> = Result<T, ScheduleError>;

/// Everything the Schedule workspace persists for one project.
///
/// Ordered lists, not maps: the Gantt draws one row per agent in this order,
/// and a file that round-trips must bring the same order back.
pub(crate) const DEFAULT_BAR_HEIGHT: f32 = 40.0;
pub(crate) const MIN_BAR_HEIGHT: f32 = 20.0;
pub(crate) const MAX_BAR_HEIGHT: f32 = 160.0;

fn default_bar_height() -> f32 {
    DEFAULT_BAR_HEIGHT
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchedulePlan {
    /// What this project's schedule is called. Free text, may be empty.
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    classes: Vec<LoaderClass>,
    #[serde(default)]
    agents: Vec<LoaderAgent>,
    #[serde(default)]
    next_class_id: u64,
    #[serde(default)]
    next_agent_id: u64,
    /// The Gantt bars this project has authored, in the order they were
    /// created. Ordering here carries no priority - each bar names its own -
    /// but it is the order the Gantt lists them in, and a file that
    /// round-trips brings the same order back.
    #[serde(default)]
    bars: Vec<ScheduleBar>,
    #[serde(default)]
    next_bar_id: u64,
    /// Which reserve field this schedule reads as tonnes.
    ///
    /// Chosen explicitly and never inferred from a field's name: "Tonnes",
    /// "tonnage" and "TONNES_WET" are all plausible names for fields that
    /// mean different things, and a schedule that guessed would produce
    /// durations that look right and are not. `None` until the user picks
    /// one, which is a readiness problem rather than a reason to assume.
    #[serde(default)]
    tonnage_field: Option<ReserveFieldId>,
    /// Height of a dig-sequence bar in the Gantt, in logical UI points.
    #[serde(default = "default_bar_height")]
    bar_height: f32,
}

impl Default for SchedulePlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            classes: Vec::new(),
            agents: Vec::new(),
            next_class_id: 0,
            next_agent_id: 0,
            bars: Vec::new(),
            next_bar_id: 0,
            tonnage_field: None,
            bar_height: DEFAULT_BAR_HEIGHT,
        }
    }
}

/// Whether two names are the same name, for the uniqueness rules. Trimmed and
/// case-insensitive: `ex7001` and `EX7001 ` are one machine, not two.
///
/// Case folding is ASCII-only, deliberately and consistently: the dialogs and
/// property tables apply the same rule, so what the UI calls a duplicate is
/// exactly what this refuses. Two names differing only outside ASCII -
/// `Löffel` and `LÖFFEL` - are therefore two names. Widening that is a single
/// change here plus the two UI comparisons that mirror it.
fn same_name(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn checked_name(name: &str) -> ScheduleResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ScheduleError::EmptyName);
    }
    Ok(trimmed.to_owned())
}

fn checked_rate(rate: f64) -> ScheduleResult<f64> {
    if !rate.is_finite() || rate <= 0.0 {
        return Err(ScheduleError::InvalidRate);
    }
    Ok(rate)
}

impl SchedulePlan {
    pub(crate) fn is_empty(&self) -> bool {
        self.name.is_empty() && self.classes.is_empty() && self.agents.is_empty() && self.bars.is_empty() && self.tonnage_field.is_none() && self.bar_height == DEFAULT_BAR_HEIGHT
    }

    /// No visible content or retired identities to preserve in a save/import.
    pub(crate) fn is_pristine(&self) -> bool {
        self.is_empty() && self.next_class_id == 0 && self.next_agent_id == 0 && self.next_bar_id == 0
    }

    /// Never hand out an id `other` has already issued.
    ///
    /// Undo restores a whole plan, id counters included, so adding a class
    /// and undoing it would otherwise leave that id free for the next
    /// addition to take. The two classes would be different equipment with
    /// one id, and a stage 2 assignment captured on either branch could not
    /// tell them apart. The counters are therefore a high-water mark: every
    /// plan installed on a document keeps the highest id either side has
    /// reached, which is why they are deliberately *not* part of
    /// [`SchedulePlan::hash_content`] - raising one is bookkeeping, not an
    /// edit the user made, and must not leave the project looking unsaved.
    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_class_id = self.next_class_id.max(other.next_class_id);
        self.next_agent_id = self.next_agent_id.max(other.next_agent_id);
        self.next_bar_id = self.next_bar_id.max(other.next_bar_id);
    }

    pub(crate) fn classes(&self) -> &[LoaderClass] {
        &self.classes
    }

    pub(crate) fn agents(&self) -> &[LoaderAgent] {
        &self.agents
    }

    pub(crate) fn class(&self, id: LoaderClassId) -> Option<&LoaderClass> {
        self.classes.iter().find(|class| class.id == id)
    }

    pub(crate) fn agent(&self, id: LoaderAgentId) -> Option<&LoaderAgent> {
        self.agents.iter().find(|agent| agent.id == id)
    }

    /// The rate one agent digs at, resolved through its class. `None` only
    /// when the class reference is dangling, which a loaded plan reports
    /// rather than repairs.
    pub(crate) fn effective_rate_tph(&self, id: LoaderAgentId) -> Option<f64> {
        let agent = self.agent(id)?;
        self.class(agent.class_id).map(|class| class.default_dig_rate_tph)
    }

    /// Every agent of one class, in list order. The rename and rate edits
    /// need no such lookup - they change the class - but deletion does, to
    /// name who is still using it.
    pub(crate) fn agents_of(&self, class: LoaderClassId) -> impl Iterator<Item = &LoaderAgent> {
        self.agents.iter().filter(move |agent| agent.class_id == class)
    }

    pub(crate) fn set_name(&mut self, name: &str) {
        self.name = name.trim().to_owned();
    }

    fn class_name_taken(&self, name: &str, except: Option<LoaderClassId>) -> bool {
        self.classes.iter().any(|class| Some(class.id) != except && same_name(&class.name, name))
    }

    fn agent_name_taken(&self, name: &str, except: Option<LoaderAgentId>) -> bool {
        self.agents.iter().any(|agent| Some(agent.id) != except && same_name(&agent.name, name))
    }

    pub(crate) fn add_class(&mut self, name: &str, rate_tph: f64) -> ScheduleResult<LoaderClassId> {
        let name = checked_name(name)?;
        let rate = checked_rate(rate_tph)?;
        if self.class_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = LoaderClassId(self.next_class_id);
        self.next_class_id = self.next_class_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.classes.push(LoaderClass {
            id,
            name,
            default_dig_rate_tph: rate,
        });
        Ok(id)
    }

    pub(crate) fn rename_class(&mut self, id: LoaderClassId, name: &str) -> ScheduleResult {
        let name = checked_name(name)?;
        if !self.classes.iter().any(|class| class.id == id) {
            return Err(ScheduleError::UnknownClass);
        }
        if self.class_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.classes.iter_mut().find(|class| class.id == id).expect("checked above").name = name;
        Ok(())
    }

    pub(crate) fn set_class_rate(&mut self, id: LoaderClassId, rate_tph: f64) -> ScheduleResult {
        let rate = checked_rate(rate_tph)?;
        let class = self.classes.iter_mut().find(|class| class.id == id).ok_or(ScheduleError::UnknownClass)?;
        class.default_dig_rate_tph = rate;
        Ok(())
    }

    /// Remove a class nothing uses. A class with agents is refused, naming
    /// them: reassigning or deleting those agents is the user's decision, and
    /// cascading it here would take machines off the Gantt without asking.
    pub(crate) fn remove_class(&mut self, id: LoaderClassId) -> ScheduleResult {
        if !self.classes.iter().any(|class| class.id == id) {
            return Err(ScheduleError::UnknownClass);
        }
        let users: Vec<String> = self.agents_of(id).map(|agent| agent.name.clone()).collect();
        if !users.is_empty() {
            return Err(ScheduleError::ClassInUse(users));
        }
        self.classes.retain(|class| class.id != id);
        Ok(())
    }

    pub(crate) fn add_agent(&mut self, name: &str, class_id: LoaderClassId) -> ScheduleResult<LoaderAgentId> {
        let name = checked_name(name)?;
        if !self.classes.iter().any(|class| class.id == class_id) {
            return Err(ScheduleError::UnknownClass);
        }
        if self.agent_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = LoaderAgentId(self.next_agent_id);
        self.next_agent_id = self.next_agent_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.agents.push(LoaderAgent {
            id,
            name,
            class_id,
            calendar: LoaderCalendar::default(),
        });
        Ok(id)
    }

    pub(crate) fn rename_agent(&mut self, id: LoaderAgentId, name: &str) -> ScheduleResult {
        let name = checked_name(name)?;
        if !self.agents.iter().any(|agent| agent.id == id) {
            return Err(ScheduleError::UnknownAgent);
        }
        if self.agent_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.agents.iter_mut().find(|agent| agent.id == id).expect("checked above").name = name;
        Ok(())
    }

    pub(crate) fn set_agent_class(&mut self, id: LoaderAgentId, class_id: LoaderClassId) -> ScheduleResult {
        if !self.classes.iter().any(|class| class.id == class_id) {
            return Err(ScheduleError::UnknownClass);
        }
        let agent = self.agents.iter_mut().find(|agent| agent.id == id).ok_or(ScheduleError::UnknownAgent)?;
        agent.class_id = class_id;
        Ok(())
    }

    /// Apply an all-or-nothing calendar batch. Duplicate addresses are
    /// rejected so paste order cannot decide the saved value.
    pub(crate) fn set_calendar_cells(&mut self, edits: &[CalendarCellEdit]) -> ScheduleResult {
        let mut addresses = std::collections::HashSet::new();
        for edit in edits {
            if !addresses.insert((edit.agent, edit.cell, edit.field)) {
                return Err(ScheduleError::DuplicateCalendarCell);
            }
            let agent = self.agents.iter_mut().find(|agent| agent.id == edit.agent).ok_or(ScheduleError::UnknownAgent)?;
            agent.calendar.set(edit.cell, edit.field, edit.value)?;
        }
        Ok(())
    }

    /// Remove one machine, and unassign every bar that named it.
    ///
    /// The bars survive: a bar is the user's own authored work, and deleting
    /// a machine is a statement about the fleet, not about the ground. They
    /// come back as unassigned - which the Gantt shows in its own lane and
    /// the readiness report states - rather than being deleted or left
    /// pointing at equipment that is gone.
    pub(crate) fn remove_agent(&mut self, id: LoaderAgentId) -> ScheduleResult {
        if !self.agents.iter().any(|agent| agent.id == id) {
            return Err(ScheduleError::UnknownAgent);
        }
        self.agents.retain(|agent| agent.id != id);
        for bar in &mut self.bars {
            if bar.agent == Some(id) {
                bar.agent = None;
            }
        }
        Ok(())
    }

    pub(crate) fn bars(&self) -> &[ScheduleBar] {
        &self.bars
    }

    pub(crate) fn bar(&self, id: BarId) -> Option<&ScheduleBar> {
        self.bars.iter().find(|bar| bar.id == id)
    }

    /// Which reserve field this schedule reads as tonnes, if one was chosen.
    pub(crate) fn tonnage_field(&self) -> Option<ReserveFieldId> {
        self.tonnage_field
    }

    /// Choose the field whose summed value is read as tonnes, or clear it.
    ///
    /// The plan cannot check the field exists or sums - it has no document -
    /// so that is a readiness question, answered against the project every
    /// time a bar is measured rather than once when it is picked.
    pub(crate) fn set_tonnage_field(&mut self, field: Option<ReserveFieldId>) {
        self.tonnage_field = field;
    }

    pub(crate) fn bar_height(&self) -> f32 {
        self.bar_height
    }

    pub(crate) fn set_bar_height(&mut self, height: f32) -> ScheduleResult {
        if !height.is_finite() || !(MIN_BAR_HEIGHT..=MAX_BAR_HEIGHT).contains(&height) {
            return Err(ScheduleError::InvalidBarHeight);
        }
        self.bar_height = height;
        Ok(())
    }

    /// Every bar holding this ground, for a report that must say where a
    /// block is already committed - including when the duplication arrived by
    /// copying a bar.
    #[allow(dead_code, reason = "read by the checkpoint 4 dispatch rules, which refuse the same ground to two executable assignments")]
    pub(crate) fn bars_holding(&self, block: &DigBlockRef) -> impl Iterator<Item = &ScheduleBar> {
        self.bars.iter().filter(move |bar| bar.order.contains(block))
    }

    fn bar_name_taken(&self, name: &str, except: Option<BarId>) -> bool {
        !name.trim().is_empty() && self.bars.iter().any(|bar| Some(bar.id) != except && bar.has_custom_name() && same_name(bar.name(), name))
    }

    fn allocate_bar_id(&mut self) -> ScheduleResult<BarId> {
        let id = BarId(self.next_bar_id);
        self.next_bar_id = self.next_bar_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        Ok(id)
    }

    /// Add an empty bar to one machine's lane, at the instant it was asked
    /// for. Its ground is chosen afterwards.
    ///
    /// The earliest start is a parameter rather than always zero because a bar
    /// is created by right-clicking a place on the timeline: adding it at the
    /// origin instead would put new work somewhere the user is not looking,
    /// and moving it back would be a second undo step.
    pub(crate) fn add_bar(&mut self, name: &str, agent: Option<LoaderAgentId>, priority: u32, window: WorkWindow) -> ScheduleResult<BarId> {
        let name = name.trim().to_owned();
        if self.bar_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        if agent.is_some_and(|agent| !self.agents.iter().any(|existing| existing.id == agent)) {
            return Err(ScheduleError::UnknownAgent);
        }
        if !window.is_valid() {
            return Err(ScheduleError::InvalidWindow);
        }
        let id = self.allocate_bar_id()?;
        self.bars.push(ScheduleBar {
            id,
            order: DigOrder::new(name),
            agent,
            priority,
            window,
        });
        Ok(id)
    }

    pub(crate) fn rename_bar(&mut self, id: BarId, name: &str) -> ScheduleResult {
        let name = name.trim().to_owned();
        if !self.bars.iter().any(|bar| bar.id == id) {
            return Err(ScheduleError::UnknownBar);
        }
        if self.bar_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.bars.iter_mut().find(|bar| bar.id == id).expect("checked above").order.name = name;
        Ok(())
    }

    /// Delete a bar and the dig order it held.
    ///
    /// Unlike a loader class, a bar owns its members rather than being
    /// referred to by them, so nothing is orphaned. The ground itself is
    /// untouched - only this bar's claim on it goes.
    pub(crate) fn remove_bar(&mut self, id: BarId) -> ScheduleResult {
        if !self.bars.iter().any(|bar| bar.id == id) {
            return Err(ScheduleError::UnknownBar);
        }
        self.bars.retain(|bar| bar.id != id);
        Ok(())
    }

    /// Copy a bar into an independent one, placed directly after it.
    ///
    /// Independent from the moment it exists: a fresh id and a cloned
    /// membership list, so editing either one never reaches the other. The
    /// copy keeps its original's machine, lane and earliest start - the point
    /// of copying is to start from the same work - and the two then hold the
    /// same ground, which the readiness report names as a conflict rather
    /// than resolving on the user's behalf.
    pub(crate) fn copy_bar(&mut self, id: BarId, name: &str) -> ScheduleResult<BarId> {
        let name = name.trim().to_owned();
        if self.bar_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let Some(index) = self.bars.iter().position(|bar| bar.id == id) else {
            return Err(ScheduleError::UnknownBar);
        };
        let new_id = self.allocate_bar_id()?;
        let mut copy = self.bars[index].clone();
        copy.id = new_id;
        copy.order.name = name;
        self.bars.insert(index + 1, copy);
        Ok(new_id)
    }

    /// Assign a bar to a machine, or take it off the fleet entirely.
    pub(crate) fn set_bar_agent(&mut self, id: BarId, agent: Option<LoaderAgentId>) -> ScheduleResult {
        if agent.is_some_and(|agent| !self.agents.iter().any(|existing| existing.id == agent)) {
            return Err(ScheduleError::UnknownAgent);
        }
        let bar = self.bars.iter_mut().find(|bar| bar.id == id).ok_or(ScheduleError::UnknownBar)?;
        bar.agent = agent;
        Ok(())
    }

    /// Put a bar in a priority lane. Lower is higher priority.
    pub(crate) fn set_bar_priority(&mut self, id: BarId, priority: u32) -> ScheduleResult {
        let bar = self.bars.iter_mut().find(|bar| bar.id == id).ok_or(ScheduleError::UnknownBar)?;
        bar.priority = priority;
        Ok(())
    }

    /// Put a bar on a machine, in a lane, over a period, in one edit.
    ///
    /// With `insert_lane`, `priority` is a lane to *open* rather than one to
    /// join: that machine's bars from `priority` down move one lane further
    /// out first, so the bar lands in a lane of its own. Everything is checked
    /// before anything moves - a refusal leaves every bar's lane as it found
    /// it, rather than a schedule shifted to make room for a bar that never
    /// arrived.
    pub(crate) fn set_bar_placement(&mut self, id: BarId, agent: Option<LoaderAgentId>, priority: u32, window: WorkWindow, insert_lane: bool) -> ScheduleResult {
        if agent.is_some_and(|agent| !self.agents.iter().any(|existing| existing.id == agent)) {
            return Err(ScheduleError::UnknownAgent);
        }
        if !window.is_valid() {
            return Err(ScheduleError::InvalidWindow);
        }
        if !self.bars.iter().any(|bar| bar.id == id) {
            return Err(ScheduleError::UnknownBar);
        }
        if insert_lane {
            // Saturating rather than wrapping: at the very last lane number
            // there is nowhere further out to go, and the two bars share a
            // lane instead of one of them reappearing at the top of the row.
            for bar in self.bars.iter_mut().filter(|bar| bar.id != id && bar.agent == agent && bar.priority >= priority) {
                bar.priority = bar.priority.saturating_add(1);
            }
        }
        let bar = self.bars.iter_mut().find(|bar| bar.id == id).expect("checked above");
        bar.agent = agent;
        bar.priority = priority;
        bar.window = window;
        Ok(())
    }

    /// Set the period a bar may be worked in, in hours from the schedule
    /// origin.
    ///
    /// Refused rather than clamped: an end at or before its start, or either
    /// bound not finite, is a window that says nothing, and quietly widening
    /// one into something workable would schedule work the user never asked
    /// for.
    pub(crate) fn set_bar_window(&mut self, id: BarId, window: WorkWindow) -> ScheduleResult {
        if !window.is_valid() {
            return Err(ScheduleError::InvalidWindow);
        }
        let bar = self.bars.iter_mut().find(|bar| bar.id == id).ok_or(ScheduleError::UnknownBar)?;
        bar.window = window;
        Ok(())
    }

    fn bar_mut(&mut self, id: BarId) -> ScheduleResult<&mut ScheduleBar> {
        self.bars.iter_mut().find(|bar| bar.id == id).ok_or(ScheduleError::UnknownBar)
    }

    /// Append ground to the end of a bar's dig order.
    #[allow(
        dead_code,
        reason = "insert_bar_member covers it today; the tail-append reads better where the checkpoint 3 picker adds blocks"
    )]
    pub(crate) fn add_bar_member(&mut self, id: BarId, block: DigBlockRef) -> ScheduleResult {
        self.insert_bar_member(id, usize::MAX, block)
    }

    /// Put ground at `position` in a bar's dig order, clamped to the end.
    pub(crate) fn insert_bar_member(&mut self, id: BarId, position: usize, block: DigBlockRef) -> ScheduleResult {
        self.bar_mut(id)?.order.insert(position, block)
    }

    pub(crate) fn remove_bar_member(&mut self, id: BarId, position: usize) -> ScheduleResult {
        self.bar_mut(id)?.order.remove(position)
    }

    /// Move one block to a different place in a bar's dig order.
    pub(crate) fn move_bar_member(&mut self, id: BarId, from: usize, to: usize) -> ScheduleResult {
        self.bar_mut(id)?.order.move_member(from, to)
    }

    /// Replace a bar's whole dig order in one edit.
    ///
    /// What the floating sequence editor applies: an editing session is one
    /// command and therefore one undo step, rather than one per pick. The
    /// list is checked the way the incremental edits are - well-formed
    /// references, no ground twice - so a draft cannot enter the project
    /// through a door the other edits are guarded on.
    #[allow(dead_code, reason = "the checkpoint 3 sequence editor's Apply; the Gantt edits membership one block at a time")]
    pub(crate) fn set_bar_members(&mut self, id: BarId, members: Vec<DigBlockRef>) -> ScheduleResult {
        let mut order = DigOrder::new(String::new());
        for block in members {
            order.insert(usize::MAX, block)?;
        }
        self.bar_mut(id)?.order.members = order.members;
        Ok(())
    }

    /// Check a plan read back from a file, and bring its id counters up to
    /// what it actually contains.
    ///
    /// A saved plan carries its own counters, but a file written by a future
    /// version - or edited by hand - could hand out an id that is already in
    /// use, which would rebind an agent to the wrong class the next time one
    /// is added. Raising the counters past every stored id costs nothing and
    /// removes that possibility.
    pub(crate) fn validate_loaded(&mut self) -> ScheduleResult {
        if !self.bar_height.is_finite() || !(MIN_BAR_HEIGHT..=MAX_BAR_HEIGHT).contains(&self.bar_height) {
            return Err(ScheduleError::InvalidBarHeight);
        }
        for agent in &mut self.agents {
            agent.calendar.canonicalize_percentages();
        }
        for (index, class) in self.classes.iter().enumerate() {
            if self.classes[..index].iter().any(|earlier| earlier.id == class.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&class.name)?;
            checked_rate(class.default_dig_rate_tph)?;
            if self.classes[..index].iter().any(|earlier| same_name(&earlier.name, &class.name)) {
                return Err(ScheduleError::DuplicateName(class.name.clone()));
            }
        }
        for (index, agent) in self.agents.iter().enumerate() {
            if self.agents[..index].iter().any(|earlier| earlier.id == agent.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&agent.name)?;
            if self.agents[..index].iter().any(|earlier| same_name(&earlier.name, &agent.name)) {
                return Err(ScheduleError::DuplicateName(agent.name.clone()));
            }
            let class = self.classes.iter().find(|class| class.id == agent.class_id).ok_or(ScheduleError::UnknownClass)?;
            agent.calendar.compile(class.default_dig_rate_tph)?;
        }
        for (index, bar) in self.bars.iter().enumerate() {
            if self.bars[..index].iter().any(|earlier| earlier.id == bar.id) {
                return Err(ScheduleError::DuplicateId);
            }
            if bar.has_custom_name() && self.bars[..index].iter().any(|earlier| earlier.has_custom_name() && same_name(earlier.name(), bar.name())) {
                return Err(ScheduleError::DuplicateName(bar.name().to_owned()));
            }
            if bar.agent.is_some_and(|agent| !self.agents.iter().any(|existing| existing.id == agent)) {
                return Err(ScheduleError::UnknownAgent);
            }
            if !bar.window.is_valid() {
                return Err(ScheduleError::InvalidWindow);
            }
            bar.order.check_loaded()?;
        }
        let highest_class = self.classes.iter().map(|class| class.id.0).max();
        let highest_agent = self.agents.iter().map(|agent| agent.id.0).max();
        let highest_bar = self.bars.iter().map(|bar| bar.id.0).max();
        for (counter, highest) in [
            (&mut self.next_class_id, highest_class),
            (&mut self.next_agent_id, highest_agent),
            (&mut self.next_bar_id, highest_bar),
        ] {
            if let Some(highest) = highest {
                *counter = (*counter).max(highest.checked_add(1).ok_or(ScheduleError::IdsExhausted)?);
            }
        }
        Ok(())
    }

    /// Fold this plan into the project's saved-content fingerprint.
    ///
    /// Written by hand rather than derived because a dig rate is an `f64`.
    /// The id counters are deliberately left out: they only ever rise, and a
    /// project whose fleet is back exactly as it was saved is not unsaved
    /// work merely because an addition was undone. See
    /// [`SchedulePlan::raise_allocator_to`].
    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.name.hash(hasher);
        for class in &self.classes {
            class.id.hash(hasher);
            class.name.hash(hasher);
            class.default_dig_rate_tph.to_bits().hash(hasher);
        }
        for agent in &self.agents {
            agent.id.hash(hasher);
            agent.name.hash(hasher);
            agent.class_id.hash(hasher);
            agent.calendar.default_availability.to_bits().hash(hasher);
            agent.calendar.default_utilisation.to_bits().hash(hasher);
            for (period, override_) in &agent.calendar.periods {
                period.hash(hasher);
                override_.availability.map(f64::to_bits).hash(hasher);
                override_.utilisation.map(f64::to_bits).hash(hasher);
                override_.rate_tph.map(f64::to_bits).hash(hasher);
            }
        }
        self.tonnage_field.hash(hasher);
        self.bar_height.to_bits().hash(hasher);
        for bar in &self.bars {
            bar.id.hash(hasher);
            bar.order.name.hash(hasher);
            bar.agent.hash(hasher);
            bar.priority.hash(hasher);
            bar.window.start_h.to_bits().hash(hasher);
            bar.window.end_h.map(f64::to_bits).hash(hasher);
            for member in bar.members() {
                member.solid.hash(hasher);
                member.source.hash(hasher);
                member.flitch_base.to_bits().hash(hasher);
                member.flitch_top.to_bits().hash(hasher);
                member.anchor[0].to_bits().hash(hasher);
                member.anchor[1].to_bits().hash(hasher);
                member.plan_area.to_bits().hash(hasher);
                member.footprint.hash(hasher);
                member.volume.map(f64::to_bits).hash(hasher);
            }
        }
    }

    /// Test-only: pretend the id allocators were never advanced, so plan
    /// snapshots taken across an undo (which never rewinds them) compare on
    /// content alone.
    #[cfg(test)]
    pub(crate) fn rewind_allocators_for_test(&mut self) {
        self.next_class_id = 0;
        self.next_agent_id = 0;
        self.next_bar_id = 0;
    }

    /// An estimate of what one snapshot of this plan costs the undo history.
    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.name.len()
            + self.classes.iter().map(|class| size_of::<LoaderClass>() + class.name.len()).sum::<usize>()
            + self
                .agents
                .iter()
                .map(|agent| size_of::<LoaderAgent>() + agent.name.len() + agent.calendar.periods.len() * size_of::<(CalendarPeriod, calendar::LoaderPeriodOverride)>())
                .sum::<usize>()
            + self
                .bars
                .iter()
                .map(|bar| size_of::<ScheduleBar>() + bar.name().len() + size_of_val(bar.members()))
                .sum::<usize>()
    }
}

/// A name that no class or agent in `existing` already has, by appending a
/// number the way [`crate::model::project::unique_item_name`] does for items.
///
/// Used only to seed a new-item dialog, never to silently rename a committed
/// edit: an edit that would collide is refused and reported instead.
pub(crate) fn suggested_name(base: &str, existing: impl Iterator<Item = String>) -> String {
    let taken: Vec<String> = existing.collect();
    let free = |candidate: &str| !taken.iter().any(|name| same_name(name, candidate));
    if free(base) {
        return base.to_owned();
    }
    (2u32..)
        .map(|index| tr_format!(literal = "%base% %index%", base = base.to_owned(), index = index.to_string()))
        .find(|candidate| free(candidate))
        .expect("the range is unbounded")
}
