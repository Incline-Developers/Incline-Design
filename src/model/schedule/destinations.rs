//! Where extracted material goes: the destinations a schedule can deliver to,
//! and the ordered rules that decide which one each portion of material
//! reaches.
//!
//! # Two kinds of destination, one kind of identity
//!
//! A stockpile or dump usually already exists in the project as a
//! [`Solid`](crate::model::Solid) of that kind, drawn and named in the Solids
//! workspace. Those are *not* copied into the schedule: they are exposed
//! automatically, by [`DestinationId::Solid`], so a solid added later appears
//! here without an edit and a rename in Solids is the rename here. What the
//! schedule stores against them is only what Solids has no opinion about -
//! their receiving capacity - and it stores that *separately*, keyed by the
//! solid's durable id, so a solid that disappears or changes kind leaves its
//! settings behind rather than taking them with it. Undo the solid change and
//! the association is exactly as it was.
//!
//! Anything with no geometry - a crusher, a stockpile nobody has drawn yet -
//! is a [`DestinationId::Standalone`], allocated here and named here.
//!
//! Neither kind is ever referenced by name or by row: renaming a destination
//! or reordering the list moves nothing.
//!
//! # Capacity is tonnes, and blank is not zero
//!
//! Every capacity in this module is measured in the schedule's nominated
//! tonnes field. `None` means unlimited and is stored as an absent value
//! rather than as an infinity, so "nobody has said" and "the user typed a
//! number" never render alike. A finite capacity must be non-negative; zero
//! is a real answer - a destination that cannot receive.
//!
//! Volume is never converted into tonnes and no density is assumed: a linked
//! solid supplies identity and geometry, and nothing else. Opening
//! inventories and reclaim are not modelled, which is why a calculated
//! stockpile figure is *scheduled inventory* rather than stock on hand.
//!
//! # Rules are ordered, and order is the whole resolution
//!
//! Each rule names one destination and filters the material that may reach
//! it, by loader, by source ground and by field variable. Every enabled rule
//! that matches is a candidate; the first one, top down, whose destination can
//! still receive gets the material. A rule that matches but is full is passed
//! over - and a destination whose rule does *not* match is never used, however
//! much room it has. That asymmetry is deliberate: capacity decides between
//! destinations the user allowed, never which destinations the user allowed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{CalendarPeriod, LoaderAgentId, ScheduleError, ScheduleResult, checked_name, same_name};
use crate::{
    i18n::{tr, tr_format},
    model::{ReserveFieldId, SolidId},
};

/// How far a stored bench or flitch RL may move and still name the same band.
///
/// The same figure [`super::sequence`] matches its own references with, so a
/// scope authored against one run and the ground of the next agree on which
/// band they are talking about.
pub(crate) const BAND_TOLERANCE: f64 = 1e-6;

/// Identity of one standalone destination within its project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct StandaloneDestinationId(pub(crate) u64);

/// Identity of one rule within its project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct RuleId(pub(crate) u64);

/// Which destination a rule delivers to.
///
/// Two disjoint spaces on purpose: a solid-backed destination *is* the solid,
/// and a standalone one is allocated here. Neither can be mistaken for the
/// other, and neither depends on a name or a row index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum DestinationId {
    /// A [`crate::model::SolidKind::Stockpile`] or
    /// [`crate::model::SolidKind::Dump`] solid, exposed automatically.
    Solid(SolidId),
    Standalone(StandaloneDestinationId),
}

/// What a destination does with what it receives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum DestinationKind {
    /// Material placed and, in a later increment, reclaimable. Its calculated
    /// figure is scheduled inventory.
    #[default]
    Stockpile,
    /// Material placed and not taken out again.
    Dump,
    /// No storage in this increment: what arrives is processed at once,
    /// subject to a daily tonnage budget.
    Crusher,
}

impl DestinationKind {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Stockpile => tr!("destination-kind-stockpile"),
            Self::Dump => tr!("destination-kind-dump"),
            Self::Crusher => tr!("destination-kind-crusher"),
        }
    }

    /// The kind a solid of this kind is exposed as, or `None` for a pit.
    pub(crate) fn of_solid(kind: crate::model::SolidKind) -> Option<Self> {
        match kind {
            crate::model::SolidKind::Stockpile => Some(Self::Stockpile),
            crate::model::SolidKind::Dump => Some(Self::Dump),
            crate::model::SolidKind::Pit => None,
        }
    }
}

/// One period's crusher limit, where a period has one at all.
///
/// Three states, not two: a period with no entry *inherits* the default, and
/// one holding [`Self::Unlimited`] has been told to ignore a finite default.
/// Collapsing those would make an explicit "no limit today" unexpressible
/// whenever the default was finite.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum CrusherOverride {
    Unlimited,
    Limit(f64),
}

impl CrusherOverride {
    pub(crate) fn tonnes(self) -> Option<f64> {
        match self {
            Self::Unlimited => None,
            Self::Limit(tonnes) => Some(tonnes),
        }
    }
}

/// A crusher's daily budget: one default and sparse per-period overrides.
///
/// Sparse on purpose, exactly like [`super::LoaderCalendar`]: an override at
/// day 900 introduces two boundaries and leaves the 899 days before it one
/// interval, so nothing here enumerates periods.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CrusherCalendar {
    /// Tonnes per period, or `None` for unlimited.
    pub(crate) default_tpd: Option<f64>,
    pub(crate) periods: BTreeMap<CalendarPeriod, CrusherOverride>,
}

impl CrusherCalendar {
    pub(crate) fn validate(&self) -> ScheduleResult {
        if let Some(limit) = self.default_tpd {
            checked_capacity(limit)?;
        }
        for (period, value) in &self.periods {
            period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
            if let CrusherOverride::Limit(limit) = value {
                checked_capacity(*limit)?;
            }
        }
        Ok(())
    }

    /// This period's budget in tonnes, or `None` for unlimited.
    pub(crate) fn limit_at(&self, period: CalendarPeriod) -> Option<f64> {
        match self.periods.get(&period) {
            Some(value) => value.tonnes(),
            None => self.default_tpd,
        }
    }

    /// The first period after `period` whose budget differs from this one's,
    /// so a run can jump to the next change rather than walk the days between.
    ///
    /// Only periods carrying an override, and the period after each of them,
    /// can differ - everywhere else inherits the one default.
    pub(crate) fn next_change_after(&self, period: CalendarPeriod) -> Option<CalendarPeriod> {
        let current = self.limit_at(period);
        self.periods
            .range((std::ops::Bound::Excluded(period), std::ops::Bound::Unbounded))
            .flat_map(|(at, _)| [*at, CalendarPeriod(at.0.saturating_add(1))])
            .chain(std::iter::once(CalendarPeriod(period.0.saturating_add(1))))
            .filter(|candidate| candidate.0 > period.0)
            .find(|candidate| self.limit_at(*candidate) != current)
    }
}

/// Which cell of a crusher's calendar an edit addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CrusherCell {
    Default,
    Period(CalendarPeriod),
}

/// One crusher-calendar cell edit.
///
/// `value` distinguishes the three states a period can be in: `None` clears
/// the override and restores inheritance, `Some(Unlimited)` says this period
/// has no limit even though the default has one, and `Some(Limit)` is a
/// figure. On the default cell there is nothing to inherit, so `None` and
/// `Some(Unlimited)` both mean unlimited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CrusherCellEdit {
    pub(crate) destination: StandaloneDestinationId,
    pub(crate) cell: CrusherCell,
    pub(crate) value: Option<CrusherOverride>,
}

/// A destination with no geometry behind it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StandaloneDestination {
    pub(crate) id: StandaloneDestinationId,
    pub(crate) name: String,
    pub(crate) kind: DestinationKind,
    /// Maximum stored or deposited tonnes; `None` is unlimited. Meaningless
    /// for a crusher, which has no storage in this increment.
    #[serde(default)]
    pub(crate) capacity_t: Option<f64>,
    /// Crusher only.
    #[serde(default)]
    pub(crate) crusher: CrusherCalendar,
}

/// The scheduling settings of one solid-backed destination.
///
/// Kept even when the solid is gone or is no longer a stockpile or dump: the
/// solid change that broke the association is undoable, and a setting thrown
/// away on the way out does not come back on the way in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SolidDestination {
    pub(crate) solid: SolidId,
    #[serde(default)]
    pub(crate) capacity_t: Option<f64>,
}

/// Which loaders a rule applies to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum LoaderSelection {
    #[default]
    All,
    /// Any one of these, ascending and without repeats.
    Only(Vec<LoaderAgentId>),
}

/// One piece of ground a rule accepts material from: a whole pit, one bench of
/// it, or one flitch of that bench.
///
/// Identified by the solid's durable id and the band's own RLs, matched to
/// [`BAND_TOLERANCE`] - never by a displayed label, and never by an elevation
/// alone, which two pits can share.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum SourceScope {
    Pit(SolidId),
    Bench { solid: SolidId, base: f64, top: f64 },
    Flitch { solid: SolidId, base: f64, top: f64 },
}

impl SourceScope {
    pub(crate) fn solid(self) -> SolidId {
        match self {
            Self::Pit(solid) | Self::Bench { solid, .. } | Self::Flitch { solid, .. } => solid,
        }
    }

    fn bands(self) -> Option<(f64, f64)> {
        match self {
            Self::Pit(_) => None,
            Self::Bench { base, top, .. } | Self::Flitch { base, top, .. } => Some((base, top)),
        }
    }

    fn is_valid(self) -> bool {
        match self.bands() {
            None => true,
            Some((base, top)) => base.is_finite() && top.is_finite() && base < top,
        }
    }

    /// Whether this scope covers a block cut from these bands of this solid.
    pub(crate) fn covers(self, solid: SolidId, bench: (f64, f64), flitch: (f64, f64)) -> bool {
        if self.solid() != solid {
            return false;
        }
        let same = |left: (f64, f64), right: (f64, f64)| (left.0 - right.0).abs() <= BAND_TOLERANCE && (left.1 - right.1).abs() <= BAND_TOLERANCE;
        match self {
            Self::Pit(_) => true,
            Self::Bench { base, top, .. } => same((base, top), bench),
            Self::Flitch { base, top, .. } => same((base, top), flitch),
        }
    }
}

/// Which ground a rule applies to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum SourceSelection {
    #[default]
    All,
    /// Any one of these. Scopes may be mixed freely - a whole pit plus one
    /// bench of another is a normal thing to want.
    Only(Vec<SourceScope>),
}

/// One end of a numerical condition.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Bound {
    pub(crate) value: f64,
    /// Whether the bound itself passes. `60 < Fe < 70` is two exclusive
    /// bounds and is stored as exactly that: an endpoint is never quietly
    /// made inclusive.
    pub(crate) inclusive: bool,
}

/// What one field variable has to hold for a rule to match.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum ConditionTest {
    /// The mapped label is one of these. Selections that no longer occur in
    /// any model are kept, not dropped: a model reloaded tomorrow may hold
    /// them again, and a silently narrowed rule routes material somewhere
    /// nobody chose.
    Category { values: Vec<String> },
    /// A numerical interval, each end independently inclusive or exclusive.
    /// At least one end is required - a condition with neither is not a
    /// condition.
    Range { lower: Option<Bound>, upper: Option<Bound> },
}

impl ConditionTest {
    /// Whether a mapped numerical value satisfies this test.
    ///
    /// A value that is missing or not finite *fails*; it is never read as
    /// zero, and never passes a test it was not measured against.
    fn accepts_number(&self, value: f64) -> bool {
        let Self::Range { lower, upper } = self else {
            return false;
        };
        if !value.is_finite() {
            return false;
        }
        let above = lower.is_none_or(|bound| if bound.inclusive { value >= bound.value } else { value > bound.value });
        let below = upper.is_none_or(|bound| if bound.inclusive { value <= bound.value } else { value < bound.value });
        above && below
    }

    fn accepts_label(&self, label: &str) -> bool {
        match self {
            Self::Category { values } => values.iter().any(|value| value == label),
            Self::Range { .. } => false,
        }
    }
}

/// One field variable and what it has to hold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldCondition {
    pub(crate) field: ReserveFieldId,
    pub(crate) test: ConditionTest,
}

impl FieldCondition {
    /// Whether one portion of material satisfies this condition.
    ///
    /// `value` is the mapped value on the contributing block-model row, before
    /// any spatial proration - the grade of the material itself, not the dig
    /// block's average of it. `None` is a value that was not there, and fails.
    pub(crate) fn accepts(&self, value: Option<&PortionValue>) -> bool {
        match value {
            None | Some(PortionValue::Missing) => false,
            Some(PortionValue::Number(value)) => self.test.accepts_number(*value),
            Some(PortionValue::Category(label)) => self.test.accepts_label(label),
        }
    }
}

/// One contributing block-model row's value for one field, as routing reads it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PortionValue {
    Number(f64),
    Category(String),
    /// Mapped to nothing, or mapped to a value that is not a finite number.
    /// Deliberately its own case: it fails every condition and is not zero.
    Missing,
}

/// One routing rule: a destination and the material allowed to reach it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DestinationRule {
    pub(crate) id: RuleId,
    /// A disabled rule takes no part in routing *or* in readiness: it is a
    /// rule the user has switched off, not a rule that is broken.
    #[serde(default = "yes")]
    pub(crate) enabled: bool,
    pub(crate) name: String,
    pub(crate) destination: DestinationId,
    #[serde(default)]
    pub(crate) loaders: LoaderSelection,
    #[serde(default)]
    pub(crate) sources: SourceSelection,
    /// Every condition must hold. An empty list restricts nothing.
    #[serde(default)]
    pub(crate) conditions: Vec<FieldCondition>,
}

fn yes() -> bool {
    true
}

impl DestinationRule {
    /// Whether this rule accepts material of this composition, from this
    /// ground, dug by this loader.
    ///
    /// All three groups must hold; within the loader and source groups the
    /// entries are alternatives, and the conditions are conjunctive. Reading
    /// the values is the caller's job - `value` answers for one field at a
    /// time so a portion's payload is never copied to be tested.
    pub(crate) fn accepts(&self, loader: LoaderAgentId, ground: impl Fn(SourceScope) -> bool, value: impl Fn(ReserveFieldId) -> Option<PortionValue>) -> bool {
        if !self.enabled {
            return false;
        }
        match &self.loaders {
            LoaderSelection::All => {}
            LoaderSelection::Only(agents) if agents.contains(&loader) => {}
            LoaderSelection::Only(_) => return false,
        }
        match &self.sources {
            SourceSelection::All => {}
            SourceSelection::Only(scopes) if scopes.iter().copied().any(&ground) => {}
            SourceSelection::Only(_) => return false,
        }
        self.conditions.iter().all(|condition| condition.accepts(value(condition.field).as_ref()))
    }
}

/// Everything the Schedule workspace persists about destinations and routing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct RoutingConfig {
    /// Whether calculated production is routed at all.
    ///
    /// Off for every project that has not switched it on, including every
    /// project saved before routing existed, so their runs keep the dig-only
    /// behaviour they were authored against. Adding a destination or a rule
    /// does not switch it on: configuring something and committing to it are
    /// different decisions.
    pub(crate) enabled: bool,
    pub(crate) standalone: Vec<StandaloneDestination>,
    next_standalone_id: u64,
    /// Settings for the automatically exposed solid-backed destinations.
    pub(crate) solids: Vec<SolidDestination>,
    /// Rules in priority order, highest first.
    pub(crate) rules: Vec<DestinationRule>,
    next_rule_id: u64,
}

fn checked_capacity(value: f64) -> ScheduleResult<f64> {
    if !value.is_finite() || value < 0.0 {
        return Err(ScheduleError::InvalidCapacity);
    }
    Ok(value)
}

impl RoutingConfig {
    /// Whether nothing here has ever been configured, so an untouched project
    /// is not reported as holding routing setup.
    pub(crate) fn is_pristine(&self) -> bool {
        !self.enabled && self.standalone.is_empty() && self.rules.is_empty() && self.solids.iter().all(|entry| entry.capacity_t.is_none())
    }

    pub(crate) fn standalone(&self, id: StandaloneDestinationId) -> Option<&StandaloneDestination> {
        self.standalone.iter().find(|entry| entry.id == id)
    }

    fn standalone_mut(&mut self, id: StandaloneDestinationId) -> ScheduleResult<&mut StandaloneDestination> {
        self.standalone.iter_mut().find(|entry| entry.id == id).ok_or(ScheduleError::UnknownDestination)
    }

    pub(crate) fn rule(&self, id: RuleId) -> Option<&DestinationRule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    fn rule_mut(&mut self, id: RuleId) -> ScheduleResult<&mut DestinationRule> {
        self.rules.iter_mut().find(|rule| rule.id == id).ok_or(ScheduleError::UnknownRule)
    }

    /// The capacity stored against one destination, whichever kind it is.
    pub(crate) fn capacity_t(&self, id: DestinationId) -> Option<f64> {
        match id {
            DestinationId::Solid(solid) => self.solids.iter().find(|entry| entry.solid == solid).and_then(|entry| entry.capacity_t),
            DestinationId::Standalone(standalone) => self.standalone(standalone).and_then(|entry| entry.capacity_t),
        }
    }

    pub(crate) fn crusher(&self, id: StandaloneDestinationId) -> Option<&CrusherCalendar> {
        self.standalone(id).map(|entry| &entry.crusher)
    }

    fn standalone_name_taken(&self, name: &str, except: Option<StandaloneDestinationId>) -> bool {
        self.standalone.iter().any(|entry| Some(entry.id) != except && same_name(&entry.name, name))
    }

    fn rule_name_taken(&self, name: &str, except: Option<RuleId>) -> bool {
        self.rules.iter().any(|rule| Some(rule.id) != except && same_name(&rule.name, name))
    }

    pub(crate) fn add_standalone(&mut self, name: &str, kind: DestinationKind) -> ScheduleResult<StandaloneDestinationId> {
        let name = checked_name(name)?;
        if self.standalone_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = StandaloneDestinationId(self.next_standalone_id);
        self.next_standalone_id = self.next_standalone_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.standalone.push(StandaloneDestination {
            id,
            name,
            kind,
            capacity_t: None,
            crusher: CrusherCalendar::default(),
        });
        Ok(id)
    }

    pub(crate) fn rename_standalone(&mut self, id: StandaloneDestinationId, name: &str) -> ScheduleResult {
        let name = checked_name(name)?;
        if self.standalone_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.standalone_mut(id)?.name = name;
        Ok(())
    }

    pub(crate) fn set_standalone_capacity(&mut self, id: StandaloneDestinationId, capacity_t: Option<f64>) -> ScheduleResult {
        let capacity_t = capacity_t.map(checked_capacity).transpose()?;
        self.standalone_mut(id)?.capacity_t = capacity_t;
        Ok(())
    }

    pub(crate) fn set_crusher_default(&mut self, id: StandaloneDestinationId, limit: Option<f64>) -> ScheduleResult {
        let limit = limit.map(checked_capacity).transpose()?;
        let entry = self.standalone_mut(id)?;
        if entry.kind != DestinationKind::Crusher {
            return Err(ScheduleError::NotACrusher);
        }
        entry.crusher.default_tpd = limit;
        Ok(())
    }

    /// Set, clear or explicitly unlimit one period's crusher budget.
    /// `None` clears the override and restores inheritance.
    pub(crate) fn set_crusher_period(&mut self, id: StandaloneDestinationId, period: CalendarPeriod, value: Option<CrusherOverride>) -> ScheduleResult {
        if let Some(CrusherOverride::Limit(limit)) = value {
            checked_capacity(limit)?;
        }
        if value.is_some() {
            period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
        }
        let entry = self.standalone_mut(id)?;
        if entry.kind != DestinationKind::Crusher {
            return Err(ScheduleError::NotACrusher);
        }
        match value {
            Some(value) => entry.crusher.periods.insert(period, value),
            None => entry.crusher.periods.remove(&period),
        };
        Ok(())
    }

    /// Apply one atomic crusher-calendar edit, or one rectangular paste or
    /// clear as a batch. Refused as a whole: a paste that half-applied would
    /// leave a budget nobody typed.
    pub(crate) fn set_crusher_cells(&mut self, edits: &[CrusherCellEdit]) -> ScheduleResult {
        for (index, edit) in edits.iter().enumerate() {
            if edits[..index].iter().any(|earlier| earlier.destination == edit.destination && earlier.cell == edit.cell) {
                return Err(ScheduleError::DuplicateCalendarCell);
            }
        }
        for edit in edits {
            match edit.cell {
                CrusherCell::Default => self.set_crusher_default(edit.destination, edit.value.and_then(CrusherOverride::tonnes))?,
                CrusherCell::Period(period) => self.set_crusher_period(edit.destination, period, edit.value)?,
            }
        }
        Ok(())
    }

    /// Store a capacity against a solid-backed destination, creating its entry
    /// on first use and removing an entry that no longer says anything.
    pub(crate) fn set_solid_capacity(&mut self, solid: SolidId, capacity_t: Option<f64>) -> ScheduleResult {
        let capacity_t = capacity_t.map(checked_capacity).transpose()?;
        match self.solids.iter_mut().find(|entry| entry.solid == solid) {
            Some(entry) => entry.capacity_t = capacity_t,
            None if capacity_t.is_some() => self.solids.push(SolidDestination { solid, capacity_t }),
            None => {}
        }
        self.solids.retain(|entry| entry.capacity_t.is_some());
        Ok(())
    }

    /// Delete a standalone destination.
    ///
    /// Refused while any rule still names it, with those rules listed:
    /// deletion never cascades into the rule list, because a rule silently
    /// retargeted or removed is a routing decision the user did not make.
    pub(crate) fn remove_standalone(&mut self, id: StandaloneDestinationId) -> ScheduleResult {
        let users: Vec<String> = self
            .rules
            .iter()
            .filter(|rule| rule.destination == DestinationId::Standalone(id))
            .map(|rule| rule.name.clone())
            .collect();
        if !users.is_empty() {
            return Err(ScheduleError::DestinationInUse(users));
        }
        let before = self.standalone.len();
        self.standalone.retain(|entry| entry.id != id);
        if self.standalone.len() == before {
            return Err(ScheduleError::UnknownDestination);
        }
        Ok(())
    }

    pub(crate) fn add_rule(&mut self, name: &str, destination: DestinationId) -> ScheduleResult<RuleId> {
        let name = checked_name(name)?;
        if self.rule_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = RuleId(self.next_rule_id);
        self.next_rule_id = self.next_rule_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.rules.push(DestinationRule {
            id,
            enabled: true,
            name,
            destination,
            loaders: LoaderSelection::All,
            sources: SourceSelection::All,
            conditions: Vec::new(),
        });
        Ok(id)
    }

    /// Copy a rule into an independent one directly below the original, so the
    /// duplicate resolves after what it was copied from rather than replacing
    /// it in the order.
    pub(crate) fn duplicate_rule(&mut self, id: RuleId, name: &str) -> ScheduleResult<RuleId> {
        let name = checked_name(name)?;
        if self.rule_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let position = self.rules.iter().position(|rule| rule.id == id).ok_or(ScheduleError::UnknownRule)?;
        let new_id = RuleId(self.next_rule_id);
        self.next_rule_id = self.next_rule_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        let mut copy = self.rules[position].clone();
        copy.id = new_id;
        copy.name = name;
        self.rules.insert(position + 1, copy);
        Ok(new_id)
    }

    pub(crate) fn remove_rule(&mut self, id: RuleId) -> ScheduleResult {
        let before = self.rules.len();
        self.rules.retain(|rule| rule.id != id);
        if self.rules.len() == before {
            return Err(ScheduleError::UnknownRule);
        }
        Ok(())
    }

    pub(crate) fn rename_rule(&mut self, id: RuleId, name: &str) -> ScheduleResult {
        let name = checked_name(name)?;
        if self.rule_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.rule_mut(id)?.name = name;
        Ok(())
    }

    pub(crate) fn set_rule_enabled(&mut self, id: RuleId, enabled: bool) -> ScheduleResult {
        self.rule_mut(id)?.enabled = enabled;
        Ok(())
    }

    pub(crate) fn set_rule_destination(&mut self, id: RuleId, destination: DestinationId) -> ScheduleResult {
        self.rule_mut(id)?.destination = destination;
        Ok(())
    }

    pub(crate) fn set_rule_loaders(&mut self, id: RuleId, loaders: LoaderSelection) -> ScheduleResult {
        let loaders = match loaders {
            LoaderSelection::All => LoaderSelection::All,
            LoaderSelection::Only(mut agents) => {
                agents.sort();
                agents.dedup();
                if agents.is_empty() {
                    return Err(ScheduleError::EmptyRuleSelection);
                }
                LoaderSelection::Only(agents)
            }
        };
        self.rule_mut(id)?.loaders = loaders;
        Ok(())
    }

    pub(crate) fn set_rule_sources(&mut self, id: RuleId, sources: SourceSelection) -> ScheduleResult {
        let sources = match sources {
            SourceSelection::All => SourceSelection::All,
            SourceSelection::Only(scopes) => {
                if scopes.is_empty() {
                    return Err(ScheduleError::EmptyRuleSelection);
                }
                if !scopes.iter().copied().all(SourceScope::is_valid) {
                    return Err(ScheduleError::InvalidSourceScope);
                }
                let mut kept: Vec<SourceScope> = Vec::with_capacity(scopes.len());
                for scope in scopes {
                    if !kept.contains(&scope) {
                        kept.push(scope);
                    }
                }
                SourceSelection::Only(kept)
            }
        };
        self.rule_mut(id)?.sources = sources;
        Ok(())
    }

    /// Replace one rule's whole condition list, validated as a set.
    pub(crate) fn set_rule_conditions(&mut self, id: RuleId, conditions: Vec<FieldCondition>) -> ScheduleResult {
        for condition in &conditions {
            check_condition(&condition.test)?;
        }
        if let Some(condition) = conditions
            .iter()
            .enumerate()
            .find(|(index, condition)| conditions[..*index].iter().any(|earlier| earlier.field == condition.field))
        {
            return Err(ScheduleError::DuplicateCondition(condition.1.field));
        }
        self.rule_mut(id)?.conditions = conditions;
        Ok(())
    }

    /// Move a rule one place up or down the priority order.
    pub(crate) fn move_rule(&mut self, id: RuleId, later: bool) -> ScheduleResult {
        let position = self.rules.iter().position(|rule| rule.id == id).ok_or(ScheduleError::UnknownRule)?;
        let target = if later {
            position + 1
        } else {
            position.checked_sub(1).ok_or(ScheduleError::RuleAtEnd)?
        };
        if target >= self.rules.len() {
            return Err(ScheduleError::RuleAtEnd);
        }
        self.rules.swap(position, target);
        Ok(())
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check a plan as it came off disk, and raise the id allocators past
    /// every stored id so a hand-edited or future file cannot hand out one
    /// that is already in use.
    pub(crate) fn validate_loaded(&mut self) -> ScheduleResult {
        for (index, entry) in self.standalone.iter().enumerate() {
            if self.standalone[..index].iter().any(|earlier| earlier.id == entry.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&entry.name)?;
            if self.standalone[..index].iter().any(|earlier| same_name(&earlier.name, &entry.name)) {
                return Err(ScheduleError::DuplicateName(entry.name.clone()));
            }
            if let Some(capacity) = entry.capacity_t {
                checked_capacity(capacity)?;
            }
            entry.crusher.validate()?;
        }
        for (index, entry) in self.solids.iter().enumerate() {
            if self.solids[..index].iter().any(|earlier| earlier.solid == entry.solid) {
                return Err(ScheduleError::DuplicateId);
            }
            if let Some(capacity) = entry.capacity_t {
                checked_capacity(capacity)?;
            }
        }
        for (index, rule) in self.rules.iter().enumerate() {
            if self.rules[..index].iter().any(|earlier| earlier.id == rule.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&rule.name)?;
            if self.rules[..index].iter().any(|earlier| same_name(&earlier.name, &rule.name)) {
                return Err(ScheduleError::DuplicateName(rule.name.clone()));
            }
            if let DestinationId::Standalone(id) = rule.destination
                && !self.standalone.iter().any(|entry| entry.id == id)
            {
                return Err(ScheduleError::UnknownDestination);
            }
            match &rule.loaders {
                LoaderSelection::All => {}
                LoaderSelection::Only(agents) if !agents.is_empty() => {}
                LoaderSelection::Only(_) => return Err(ScheduleError::EmptyRuleSelection),
            }
            match &rule.sources {
                SourceSelection::All => {}
                SourceSelection::Only(scopes) if scopes.is_empty() => return Err(ScheduleError::EmptyRuleSelection),
                SourceSelection::Only(scopes) => {
                    if !scopes.iter().copied().all(SourceScope::is_valid) {
                        return Err(ScheduleError::InvalidSourceScope);
                    }
                }
            }
            for (position, condition) in rule.conditions.iter().enumerate() {
                check_condition(&condition.test)?;
                if rule.conditions[..position].iter().any(|earlier| earlier.field == condition.field) {
                    return Err(ScheduleError::DuplicateCondition(condition.field));
                }
            }
        }
        let highest_standalone = self.standalone.iter().map(|entry| entry.id.0).max();
        let highest_rule = self.rules.iter().map(|rule| rule.id.0).max();
        for (counter, highest) in [(&mut self.next_standalone_id, highest_standalone), (&mut self.next_rule_id, highest_rule)] {
            if let Some(highest) = highest {
                *counter = (*counter).max(highest.checked_add(1).ok_or(ScheduleError::IdsExhausted)?);
            }
        }
        Ok(())
    }

    /// Raise this config's allocators to another's, so an undo that restores a
    /// deletion never hands the next addition an id the redo still refers to.
    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_standalone_id = self.next_standalone_id.max(other.next_standalone_id);
        self.next_rule_id = self.next_rule_id.max(other.next_rule_id);
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.enabled.hash(hasher);
        for entry in &self.standalone {
            entry.id.hash(hasher);
            entry.name.hash(hasher);
            entry.kind.hash(hasher);
            entry.capacity_t.map(f64::to_bits).hash(hasher);
            entry.crusher.default_tpd.map(f64::to_bits).hash(hasher);
            for (period, value) in &entry.crusher.periods {
                period.hash(hasher);
                value.tonnes().map(f64::to_bits).hash(hasher);
                matches!(value, CrusherOverride::Unlimited).hash(hasher);
            }
        }
        for entry in &self.solids {
            entry.solid.hash(hasher);
            entry.capacity_t.map(f64::to_bits).hash(hasher);
        }
        for rule in &self.rules {
            rule.hash_content(hasher);
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self
                .standalone
                .iter()
                .map(|entry| size_of::<StandaloneDestination>() + entry.name.len() + entry.crusher.periods.len() * size_of::<(CalendarPeriod, CrusherOverride)>())
                .sum::<usize>()
            + self.solids.len() * size_of::<SolidDestination>()
            + self
                .rules
                .iter()
                .map(|rule| {
                    size_of::<DestinationRule>()
                        + rule.name.len()
                        + rule.conditions.iter().map(FieldCondition::estimated_bytes).sum::<usize>()
                        + match &rule.loaders {
                            LoaderSelection::All => 0,
                            LoaderSelection::Only(agents) => size_of_val(agents.as_slice()),
                        }
                        + match &rule.sources {
                            SourceSelection::All => 0,
                            SourceSelection::Only(scopes) => size_of_val(scopes.as_slice()),
                        }
                })
                .sum::<usize>()
    }
}

impl DestinationRule {
    /// The rule's own content hash, for the pipeline's input fingerprints.
    pub(crate) fn hash_content_public<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.hash_content(hasher);
    }

    fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.id.hash(hasher);
        self.enabled.hash(hasher);
        self.name.hash(hasher);
        self.destination.hash(hasher);
        match &self.loaders {
            LoaderSelection::All => 0u8.hash(hasher),
            LoaderSelection::Only(agents) => {
                1u8.hash(hasher);
                agents.hash(hasher);
            }
        }
        match &self.sources {
            SourceSelection::All => 0u8.hash(hasher),
            SourceSelection::Only(scopes) => {
                1u8.hash(hasher);
                for scope in scopes {
                    match scope {
                        SourceScope::Pit(solid) => (0u8, solid).hash(hasher),
                        SourceScope::Bench { solid, base, top } => (1u8, solid, base.to_bits(), top.to_bits()).hash(hasher),
                        SourceScope::Flitch { solid, base, top } => (2u8, solid, base.to_bits(), top.to_bits()).hash(hasher),
                    }
                }
            }
        }
        for condition in &self.conditions {
            condition.field.hash(hasher);
            match &condition.test {
                ConditionTest::Category { values } => {
                    0u8.hash(hasher);
                    values.hash(hasher);
                }
                ConditionTest::Range { lower, upper } => {
                    1u8.hash(hasher);
                    for bound in [lower, upper] {
                        bound.map(|bound| (bound.value.to_bits(), bound.inclusive)).hash(hasher);
                    }
                }
            }
        }
    }

    /// A one-line summary of what this rule matches, for the rule table.
    pub(crate) fn summary(&self, loader_name: impl Fn(LoaderAgentId) -> String, field_name: impl Fn(ReserveFieldId) -> String) -> String {
        let mut parts = Vec::new();
        match &self.loaders {
            LoaderSelection::All => {}
            LoaderSelection::Only(agents) => parts.push(agents.iter().copied().map(loader_name).collect::<Vec<_>>().join(" / ")),
        }
        match &self.sources {
            SourceSelection::All => {}
            SourceSelection::Only(scopes) => parts.push(tr!("destination-rule-sources-count", count = scopes.len().to_string())),
        }
        for condition in &self.conditions {
            parts.push(condition.summary(&field_name(condition.field)));
        }
        if parts.is_empty() {
            return tr!("destination-rule-matches-all");
        }
        parts.join(" · ")
    }
}

impl FieldCondition {
    fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + match &self.test {
                ConditionTest::Category { values } => values.iter().map(|value| size_of::<String>() + value.len()).sum(),
                ConditionTest::Range { .. } => 0,
            }
    }

    /// This condition as the user wrote it: `60 < Fe <= 70`, or `Ore / Waste`.
    pub(crate) fn summary(&self, field: &str) -> String {
        match &self.test {
            ConditionTest::Category { values } => tr_format!(literal = "%field% = %values%", field = field.to_owned(), values = values.join(" / ")),
            ConditionTest::Range { lower, upper } => {
                let number = |value: f64| value.to_string();
                match (lower, upper) {
                    (None, None) => field.to_owned(),
                    (Some(lower), None) => tr_format!(
                        literal = "%field% %operator% %value%",
                        field = field.to_owned(),
                        operator = if lower.inclusive { ">=" } else { ">" }.to_owned(),
                        value = number(lower.value)
                    ),
                    (None, Some(upper)) => tr_format!(
                        literal = "%field% %operator% %value%",
                        field = field.to_owned(),
                        operator = if upper.inclusive { "<=" } else { "<" }.to_owned(),
                        value = number(upper.value)
                    ),
                    (Some(lower), Some(upper)) => tr_format!(
                        literal = "%low% %lower% %field% %upper% %high%",
                        low = number(lower.value),
                        lower = if lower.inclusive { "<=" } else { "<" }.to_owned(),
                        field = field.to_owned(),
                        upper = if upper.inclusive { "<=" } else { "<" }.to_owned(),
                        high = number(upper.value)
                    ),
                }
            }
        }
    }
}

/// A condition has to be able to accept something and reject something.
fn check_condition(test: &ConditionTest) -> ScheduleResult {
    match test {
        ConditionTest::Category { values } => {
            if values.is_empty() {
                return Err(ScheduleError::EmptyCondition);
            }
            for (index, value) in values.iter().enumerate() {
                if values[..index].contains(value) {
                    return Err(ScheduleError::DuplicateConditionValue(value.clone()));
                }
            }
            Ok(())
        }
        ConditionTest::Range { lower, upper } => {
            // At least one bound: a range with neither restricts nothing and
            // would read on the page as a condition that was doing something.
            if lower.is_none() && upper.is_none() {
                return Err(ScheduleError::EmptyCondition);
            }
            for bound in [lower, upper].into_iter().flatten() {
                if !bound.value.is_finite() {
                    return Err(ScheduleError::InvalidBound);
                }
            }
            if let (Some(lower), Some(upper)) = (lower, upper) {
                // Empty and half-open intervals are both refused: `70 < x < 60`
                // accepts nothing, and so does `70 <= x < 70`.
                let ordered = if lower.inclusive && upper.inclusive {
                    lower.value <= upper.value
                } else {
                    lower.value < upper.value
                };
                if !ordered {
                    return Err(ScheduleError::EmptyInterval);
                }
            }
            Ok(())
        }
    }
}

/// One destination as the pages and the router see it: stable identity, the
/// name and kind it currently has, and where those came from.
///
/// Derived, never stored. A linked destination's name and kind are the solid's
/// own, so a rename in Solids is a rename here and there is no second copy to
/// fall out of step.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DestinationView {
    pub(crate) id: DestinationId,
    pub(crate) name: String,
    pub(crate) kind: DestinationKind,
    pub(crate) capacity_t: Option<f64>,
    /// The solid this destination is, when it is one. Its name and kind are
    /// read-only here and edited in Solids.
    pub(crate) solid: Option<SolidId>,
}

impl DestinationView {
    pub(crate) fn is_linked(&self) -> bool {
        self.solid.is_some()
    }
}

/// Every destination this project can deliver to: the stockpile and dump
/// solids it holds, in Solids order, then the standalone destinations.
///
/// Built from the document each time rather than stored, which is what makes a
/// solid added after these pages were last opened appear without an edit -
/// and what stops opening a page from writing anything.
pub(crate) fn available(solids: &[crate::model::Solid], routing: &RoutingConfig) -> Vec<DestinationView> {
    let mut views = Vec::with_capacity(solids.len() + routing.standalone.len());
    for solid in solids {
        let Some(kind) = DestinationKind::of_solid(solid.kind) else { continue };
        views.push(DestinationView {
            id: DestinationId::Solid(solid.id),
            name: solid.name.clone(),
            kind,
            capacity_t: routing.capacity_t(DestinationId::Solid(solid.id)),
            solid: Some(solid.id),
        });
    }
    for entry in &routing.standalone {
        views.push(DestinationView {
            id: DestinationId::Standalone(entry.id),
            name: entry.name.clone(),
            kind: entry.kind,
            capacity_t: entry.capacity_t,
            solid: None,
        });
    }
    views
}

/// Why a rule's destination cannot be delivered to.
///
/// Each case is reported against the rule that names it and nothing is
/// repaired: a rule whose stockpile became a pit is not quietly pointed at
/// another stockpile, and its settings are not converted into a pit's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DestinationProblem {
    /// The solid it named is no longer in the project.
    SolidMissing,
    /// The solid is there but is no longer a stockpile or a dump.
    SolidNotADestination,
    /// The standalone destination it named is gone. Only reachable from a
    /// file, since deleting one in use is refused.
    Missing,
}

impl DestinationProblem {
    pub(crate) fn message(self, rule: &str) -> String {
        match self {
            Self::SolidMissing => tr!("destination-problem-solid-missing", rule = rule.to_owned()),
            Self::SolidNotADestination => tr!("destination-problem-solid-kind", rule = rule.to_owned()),
            Self::Missing => tr!("destination-problem-missing", rule = rule.to_owned()),
        }
    }
}

/// Resolve one destination id against the project as it stands.
pub(crate) fn resolve(id: DestinationId, solids: &[crate::model::Solid], routing: &RoutingConfig) -> Result<DestinationView, DestinationProblem> {
    match id {
        DestinationId::Solid(solid_id) => {
            let solid = solids.iter().find(|solid| solid.id == solid_id).ok_or(DestinationProblem::SolidMissing)?;
            let kind = DestinationKind::of_solid(solid.kind).ok_or(DestinationProblem::SolidNotADestination)?;
            Ok(DestinationView {
                id,
                name: solid.name.clone(),
                kind,
                capacity_t: routing.capacity_t(id),
                solid: Some(solid_id),
            })
        }
        DestinationId::Standalone(standalone) => {
            let entry = routing.standalone(standalone).ok_or(DestinationProblem::Missing)?;
            Ok(DestinationView {
                id,
                name: entry.name.clone(),
                kind: entry.kind,
                capacity_t: entry.capacity_t,
                solid: None,
            })
        }
    }
}
