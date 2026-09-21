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

use super::{
    CalendarPeriod, LoaderAgentId, ScheduleError, ScheduleResult, checked_name,
    inventory::{OpeningLotId, OpeningPortionId, OpeningValue, ReclaimOrder, StockpileInventory},
    same_name,
};
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
    /// One-way haul distance in kilometres. The return trip uses the same
    /// figure, and a reclaim from here will too. Never derived from geometry:
    /// a straight line between two solids is not a haul road.
    #[serde(default = "default_distance")]
    pub(crate) distance_km: f64,
    /// Stockpile only: what it already holds, and which end reclaim takes
    /// from. Empty and FIFO for every project that has not authored any.
    #[serde(default)]
    pub(crate) inventory: StockpileInventory,
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
    /// One-way haul distance in kilometres; see
    /// [`StandaloneDestination::distance_km`]. Projects saved before trucks
    /// existed have no stored figure and resolve to the default.
    #[serde(default = "default_distance")]
    pub(crate) distance_km: f64,
    /// Stockpile only; see [`StandaloneDestination::inventory`].
    #[serde(default)]
    pub(crate) inventory: StockpileInventory,
}

/// One place material moves *from*.
///
/// Ground and stockpiles in one space because the rules built on it - trucking
/// today, cashflow beside it - may name either. Reclaim is not executed yet,
/// but a rule authored for it now must resolve to the same stockpile when it
/// is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", from = "ScopeRepr")]
pub(crate) enum MovementSourceScope {
    /// A pit, a bench of it, or a flitch of that bench - the same identities
    /// and tolerances the destination rules use.
    Ground(SourceScope),
    /// A stockpile the material is reclaimed from.
    Stockpile(DestinationId),
}

/// What a saved scope is read through.
///
/// A destination rule's sources used to be ground only, written as a bare
/// [`SourceScope`], and projects written then are still on disk. The two forms
/// are disjoint by key - `pit` and `bench` against `ground` and `stockpile` -
/// so the older one is accepted and read as ground rather than being refused a
/// field it was right to write. Only reading goes through this; what is written
/// is the tagged form.
#[derive(Deserialize)]
#[serde(untagged)]
enum ScopeRepr {
    Tagged(TaggedScope),
    Ground(SourceScope),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
enum TaggedScope {
    Ground(SourceScope),
    Stockpile(DestinationId),
}

impl From<ScopeRepr> for MovementSourceScope {
    fn from(repr: ScopeRepr) -> Self {
        match repr {
            ScopeRepr::Tagged(TaggedScope::Ground(scope)) | ScopeRepr::Ground(scope) => Self::Ground(scope),
            ScopeRepr::Tagged(TaggedScope::Stockpile(id)) => Self::Stockpile(id),
        }
    }
}

impl MovementSourceScope {
    /// Whether this scope is one at all: a ground band that is a band, or any
    /// stockpile.
    pub(crate) fn is_valid(self) -> bool {
        match self {
            Self::Ground(scope) => scope.is_valid(),
            Self::Stockpile(_) => true,
        }
    }

    /// Whether this scope covers where material is being loaded from *now*.
    ///
    /// Ground and stockpiles are disjoint: a stockpile scope never matches
    /// ex-pit ground, and an ex-pit scope never matches a reclaim, however the
    /// material in that pile originally got there.
    pub(crate) fn covers(self, source: super::RouteSource) -> bool {
        match (self, source) {
            (Self::Ground(ground), super::RouteSource::Ground { solid, bench, flitch }) => ground.covers(solid, bench, flitch),
            (Self::Stockpile(held), super::RouteSource::Stockpile(from)) => held == from,
            _ => false,
        }
    }

    /// The ground half, for the checks that only ground can answer - whether a
    /// scope is placeable against the blocks a run produced, say.
    pub(crate) fn ground(self) -> Option<SourceScope> {
        match self {
            Self::Ground(scope) => Some(scope),
            Self::Stockpile(_) => None,
        }
    }
}

/// Which sources a rule applies to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum MovementSourceSelection {
    /// Every source, ground and stockpile alike - including any added later.
    #[default]
    All,
    /// Any one of these. Never empty: "only these, and there are none" matches
    /// nothing and is not what All means.
    Only(Vec<MovementSourceScope>),
}

/// Which destinations a rule applies to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum DestinationSelection {
    #[default]
    All,
    Only(Vec<DestinationId>),
}

/// Fold one scope into a fingerprint. Shared with the trucking rules, which
/// name the same ground by the same identities.
pub(crate) fn hash_scope<H: std::hash::Hasher>(scope: SourceScope, hasher: &mut H) {
    use std::hash::Hash;
    match scope {
        SourceScope::Pit(solid) => (0u8, solid).hash(hasher),
        SourceScope::Bench { solid, base, top } => (1u8, solid, base.to_bits(), top.to_bits()).hash(hasher),
        SourceScope::Flitch { solid, base, top } => (2u8, solid, base.to_bits(), top.to_bits()).hash(hasher),
    }
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

    pub(crate) fn is_valid(self) -> bool {
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

/// One routing rule: the destinations it may deliver to, and the material
/// allowed to reach them.
///
/// The destinations are ordered and are tried in the order they are listed,
/// which is the same resolution the rule list itself uses one level up: the
/// first with room takes the material. Listing two here rather than writing
/// two identical rules says only that this material may go to either.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "RuleRepr")]
pub(crate) struct DestinationRule {
    pub(crate) id: RuleId,
    /// A disabled rule takes no part in routing *or* in readiness: it is a
    /// rule the user has switched off, not a rule that is broken.
    pub(crate) enabled: bool,
    pub(crate) name: String,
    /// In the order they are tried. Never empty.
    pub(crate) destinations: Vec<DestinationId>,
    pub(crate) loaders: LoaderSelection,
    /// Ground *and* stockpiles: a rule may route what a loader digs out of a
    /// pit and what it reclaims out of a pile, and the same selection model
    /// serves both here, in the trucking rules and in the cashflow rules.
    pub(crate) sources: MovementSourceSelection,
    /// Every condition must hold. An empty list restricts nothing.
    pub(crate) conditions: Vec<FieldCondition>,
}

fn yes() -> bool {
    true
}

/// What a saved rule is read through.
///
/// A rule used to name exactly one destination, and projects written then are
/// still on disk; the singular field is accepted and folded into the list so
/// those projects load rather than being refused a field they were right to
/// write. Only reading goes through this - what is written is the list.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleRepr {
    id: RuleId,
    #[serde(default = "yes")]
    enabled: bool,
    name: String,
    #[serde(default)]
    destination: Option<DestinationId>,
    #[serde(default)]
    destinations: Vec<DestinationId>,
    #[serde(default)]
    loaders: LoaderSelection,
    #[serde(default)]
    sources: MovementSourceSelection,
    #[serde(default)]
    conditions: Vec<FieldCondition>,
}

impl From<RuleRepr> for DestinationRule {
    fn from(repr: RuleRepr) -> Self {
        let RuleRepr {
            id,
            enabled,
            name,
            destination,
            mut destinations,
            loaders,
            sources,
            conditions,
        } = repr;
        if let Some(destination) = destination
            && !destinations.contains(&destination)
        {
            destinations.insert(0, destination);
        }
        Self {
            id,
            enabled,
            name,
            destinations,
            loaders,
            sources,
            conditions,
        }
    }
}

impl DestinationRule {
    /// Whether this rule accepts material of this composition, from this
    /// source, moved by this loader.
    ///
    /// All three groups must hold; within the loader and source groups the
    /// entries are alternatives, and the conditions are conjunctive. Reading
    /// the values is the caller's job - `value` answers for one field at a
    /// time so a portion's payload is never copied to be tested.
    ///
    /// `source` is where the material is being loaded from *now*: a reclaim's
    /// source is its stockpile, so a rule written for ex-pit ground cannot
    /// route material a second time on its way out of a pile.
    pub(crate) fn accepts(&self, loader: LoaderAgentId, source: super::RouteSource, value: impl Fn(ReserveFieldId) -> Option<PortionValue>) -> bool {
        self.accepts_identity(loader, source) && self.conditions.iter().all(|condition| condition.accepts(value(condition.field).as_ref()))
    }

    /// The identity half alone: enabled, this loader, this source - and
    /// nothing about the material.
    ///
    /// Split out for a caller whose material composition is not known at the
    /// time it asks, which is exactly the blended stockpile case: a pile's
    /// grade is a decision variable, so "would this rule apply if the grade
    /// allowed it" is a different and answerable question from "does it
    /// apply". Callers that know the material use [`Self::accepts`], which is
    /// this plus the conditions and is the only path routing takes.
    pub(crate) fn accepts_identity(&self, loader: LoaderAgentId, source: super::RouteSource) -> bool {
        if !self.enabled {
            return false;
        }
        match &self.loaders {
            LoaderSelection::All => {}
            LoaderSelection::Only(agents) if agents.contains(&loader) => {}
            LoaderSelection::Only(_) => return false,
        }
        match &self.sources {
            MovementSourceSelection::All => {}
            MovementSourceSelection::Only(scopes) if scopes.iter().any(|scope| scope.covers(source)) => {}
            MovementSourceSelection::Only(_) => return false,
        }
        true
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

/// What a destination's one-way haul distance is until somebody says otherwise.
pub(crate) const DEFAULT_DISTANCE_KM: f64 = 2.0;

fn default_distance() -> f64 {
    DEFAULT_DISTANCE_KM
}

/// The destinations one rule may deliver to, in order, with repeats dropped.
///
/// An empty list is refused rather than stored: a rule that matches material
/// and names nowhere to send it would read as a restriction and behave as a
/// hole in the configuration.
fn checked_destinations(destinations: Vec<DestinationId>) -> ScheduleResult<Vec<DestinationId>> {
    let mut ordered: Vec<DestinationId> = Vec::with_capacity(destinations.len());
    for destination in destinations {
        if !ordered.contains(&destination) {
            ordered.push(destination);
        }
    }
    if ordered.is_empty() {
        return Err(ScheduleError::EmptyRuleSelection);
    }
    Ok(ordered)
}

/// The movement sources one rule names, deduplicated and never empty.
///
/// Shared by the destination, trucking and cashflow rules: "only these, and
/// there are none" matches nothing and is not what All means, wherever it is
/// written.
pub(crate) fn checked_sources(sources: MovementSourceSelection) -> ScheduleResult<MovementSourceSelection> {
    match sources {
        MovementSourceSelection::All => Ok(MovementSourceSelection::All),
        MovementSourceSelection::Only(scopes) => {
            if scopes.is_empty() {
                return Err(ScheduleError::EmptyRuleSelection);
            }
            if !scopes.iter().copied().all(MovementSourceScope::is_valid) {
                return Err(ScheduleError::InvalidSourceScope);
            }
            let mut kept: Vec<MovementSourceScope> = Vec::with_capacity(scopes.len());
            for scope in scopes {
                if !kept.contains(&scope) {
                    kept.push(scope);
                }
            }
            Ok(MovementSourceSelection::Only(kept))
        }
    }
}

/// Whether a rule selecting these sources may carry **category** conditions.
///
/// # Why a source selection decides this
///
/// The accepted blended-stockpile representation keeps tonnes and contained
/// quantity per tracked grade and *discards categories*: once material is in a
/// pile there is no ore-type left to test, only a blend. So a condition on a
/// category can be evaluated for material coming straight out of the ground
/// and cannot be evaluated for material coming out of a stockpile.
///
/// That makes availability a property of the sources a rule names, and the
/// same table applies to destination rules and to cashflow rules:
///
/// | sources | category conditions |
/// |---|---|
/// | explicit pit, bench or flitch scopes only | available |
/// | any stockpile scope | unavailable |
/// | a mix of ground and stockpile scopes | unavailable |
/// | `All` | unavailable, because `All` includes stockpiles |
///
/// `All` is unavailable even in a project that has no stockpiles today.
/// `All` is a standing instruction that covers whatever is added tomorrow, so
/// reading it as "ground only, for now" would make a rule's meaning depend on
/// when it was last looked at. A planner who wants the ground half says so by
/// naming the pits.
///
/// A mixed rule is not split automatically. Which half keeps the category and
/// which loses it is the planner's decision, and guessing it would reroute
/// material nobody chose.
pub(crate) fn category_conditions_available(sources: &MovementSourceSelection) -> bool {
    match sources {
        MovementSourceSelection::All => false,
        MovementSourceSelection::Only(scopes) => !scopes.is_empty() && scopes.iter().all(|scope| scope.ground().is_some()),
    }
}

/// The fields whose category conditions this combination cannot evaluate.
///
/// Empty for every compatible rule. A non-empty answer is a rule to mark
/// invalid and a condition to name - never a condition to drop, because a
/// category silently ignored is a rule that stopped restricting and routes
/// material somewhere nobody chose.
pub(crate) fn conflicting_category_conditions(sources: &MovementSourceSelection, conditions: &[FieldCondition]) -> Vec<ReserveFieldId> {
    if category_conditions_available(sources) {
        return Vec::new();
    }
    conditions
        .iter()
        .filter(|condition| matches!(condition.test, ConditionTest::Category { .. }))
        .map(|condition| condition.field)
        .collect()
}

/// Refuse a condition list containing a category the sources cannot evaluate.
///
/// This is the *condition edit* boundary. Changing sources on a rule that
/// already carries a category preserves that condition and marks the rule
/// invalid; any later condition edit must repair the conflict explicitly.
pub(crate) fn check_category_conditions(sources: &MovementSourceSelection, conditions: &[FieldCondition], _held: &[FieldCondition]) -> ScheduleResult {
    if !conflicting_category_conditions(sources, conditions).is_empty() {
        return Err(ScheduleError::CategoryConditionUnsupported);
    }
    Ok(())
}

/// Whether a pile's opening stock fits its capacity. Equality is valid: a pile
/// that starts exactly full is a real answer, and one tonne over is not.
fn check_opening_fits(opening_t: f64, capacity_t: Option<f64>) -> ScheduleResult {
    if !opening_t.is_finite() {
        return Err(ScheduleError::InvalidLotTonnes);
    }
    if capacity_t.is_some_and(|capacity| opening_t > capacity) {
        return Err(ScheduleError::OpeningOverCapacity);
    }
    Ok(())
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
        !self.enabled && self.standalone.is_empty() && self.rules.is_empty() && self.solids.iter().all(|entry| entry.capacity_t.is_none() && entry.inventory.is_pristine())
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

    /// One destination's one-way haul distance, whichever kind it is. A
    /// destination nothing has been stored against is at the default.
    pub(crate) fn distance_km(&self, id: DestinationId) -> f64 {
        match id {
            DestinationId::Solid(solid) => self.solids.iter().find(|entry| entry.solid == solid).map_or(DEFAULT_DISTANCE_KM, |entry| entry.distance_km),
            DestinationId::Standalone(standalone) => self.standalone(standalone).map_or(DEFAULT_DISTANCE_KM, |entry| entry.distance_km),
        }
    }

    /// Store a one-way haul distance against any destination.
    pub(crate) fn set_distance_km(&mut self, id: DestinationId, distance_km: f64) -> ScheduleResult {
        let distance_km = super::trucking::checked_distance(distance_km)?;
        match id {
            DestinationId::Standalone(standalone) => self.standalone_mut(standalone)?.distance_km = distance_km,
            DestinationId::Solid(solid) => match self.solids.iter_mut().find(|entry| entry.solid == solid) {
                Some(entry) => entry.distance_km = distance_km,
                None => self.solids.push(SolidDestination {
                    solid,
                    capacity_t: None,
                    distance_km,
                    inventory: StockpileInventory::default(),
                }),
            },
        }
        Ok(())
    }

    /// One stockpile's opening stock and reclaim order. A stockpile nothing has
    /// been stored against is empty and FIFO.
    pub(crate) fn inventory(&self, id: DestinationId) -> Option<&StockpileInventory> {
        match id {
            DestinationId::Solid(solid) => self.solids.iter().find(|entry| entry.solid == solid).map(|entry| &entry.inventory),
            DestinationId::Standalone(standalone) => self.standalone(standalone).map(|entry| &entry.inventory),
        }
    }

    /// What one stockpile holds at hour zero.
    pub(crate) fn opening_tonnes(&self, id: DestinationId) -> f64 {
        self.inventory(id).map_or(0.0, StockpileInventory::opening_tonnes)
    }

    pub(crate) fn reclaim_order(&self, id: DestinationId) -> ReclaimOrder {
        self.inventory(id).map_or_else(ReclaimOrder::default, |inventory| inventory.order)
    }

    /// Apply one edit to a stockpile's inventory, all or nothing.
    ///
    /// Every inventory edit goes through here, and so does every capacity edit,
    /// which is what makes "opening stock must fit" one check rather than two
    /// that could disagree: the edit is made on a copy, the copy is weighed
    /// against the capacity, and only then does it become the stored answer.
    fn edit_inventory<T>(&mut self, id: DestinationId, edit: impl FnOnce(&mut StockpileInventory) -> ScheduleResult<T>) -> ScheduleResult<T> {
        let capacity_t = self.capacity_t(id);
        let mut draft = self.inventory(id).cloned().unwrap_or_default();
        // A destination that holds nothing yet still has to exist: an edit to a
        // stockpile that is gone is a stale command, not a new stockpile.
        if self.inventory(id).is_none()
            && let DestinationId::Standalone(standalone) = id
            && self.standalone(standalone).is_none()
        {
            return Err(ScheduleError::UnknownDestination);
        }
        let outcome = edit(&mut draft)?;
        check_opening_fits(draft.opening_tonnes(), capacity_t)?;
        match id {
            DestinationId::Standalone(standalone) => self.standalone_mut(standalone)?.inventory = draft,
            DestinationId::Solid(solid) => match self.solids.iter_mut().find(|entry| entry.solid == solid) {
                Some(entry) => entry.inventory = draft,
                None => self.solids.push(SolidDestination {
                    solid,
                    capacity_t: None,
                    distance_km: DEFAULT_DISTANCE_KM,
                    inventory: draft,
                }),
            },
        }
        self.solids
            .retain(|entry| entry.capacity_t.is_some() || entry.distance_km != DEFAULT_DISTANCE_KM || !entry.inventory.is_pristine());
        Ok(outcome)
    }

    pub(crate) fn set_reclaim_order(&mut self, id: DestinationId, order: ReclaimOrder) -> ScheduleResult {
        self.edit_inventory(id, |inventory| {
            inventory.order = order;
            Ok(())
        })
    }

    pub(crate) fn add_opening_lot(&mut self, id: DestinationId, name: &str, tonnes_t: f64) -> ScheduleResult<OpeningLotId> {
        self.edit_inventory(id, |inventory| inventory.add_lot(name, tonnes_t))
    }

    pub(crate) fn duplicate_opening_lot(&mut self, id: DestinationId, lot: OpeningLotId, name: &str) -> ScheduleResult<OpeningLotId> {
        self.edit_inventory(id, |inventory| inventory.duplicate_lot(lot, name))
    }

    pub(crate) fn remove_opening_lot(&mut self, id: DestinationId, lot: OpeningLotId) -> ScheduleResult {
        self.edit_inventory(id, |inventory| inventory.remove_lot(lot))
    }

    pub(crate) fn rename_opening_lot(&mut self, id: DestinationId, lot: OpeningLotId, name: &str) -> ScheduleResult {
        self.edit_inventory(id, |inventory| inventory.rename_lot(lot, name))
    }

    pub(crate) fn move_opening_lot(&mut self, id: DestinationId, lot: OpeningLotId, newer: bool) -> ScheduleResult {
        self.edit_inventory(id, |inventory| inventory.move_lot(lot, newer))
    }

    pub(crate) fn add_opening_portion(&mut self, id: DestinationId, lot: OpeningLotId, tonnes_t: f64) -> ScheduleResult<OpeningPortionId> {
        self.edit_inventory(id, |inventory| inventory.add_portion(lot, tonnes_t))
    }

    pub(crate) fn remove_opening_portion(&mut self, id: DestinationId, lot: OpeningLotId, portion: OpeningPortionId) -> ScheduleResult {
        self.edit_inventory(id, |inventory| inventory.remove_portion(lot, portion))
    }

    pub(crate) fn set_opening_portion_tonnes(&mut self, id: DestinationId, lot: OpeningLotId, portion: OpeningPortionId, tonnes_t: f64) -> ScheduleResult {
        self.edit_inventory(id, |inventory| inventory.set_portion_tonnes(lot, portion, tonnes_t))
    }

    pub(crate) fn set_opening_portion_value(
        &mut self,
        id: DestinationId,
        lot: OpeningLotId,
        portion: OpeningPortionId,
        field: ReserveFieldId,
        value: Option<OpeningValue>,
    ) -> ScheduleResult {
        self.edit_inventory(id, |inventory| inventory.set_portion_value(lot, portion, field, value))
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
            distance_km: DEFAULT_DISTANCE_KM,
            inventory: StockpileInventory::default(),
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
        let entry = self.standalone_mut(id)?;
        // Checked before it is stored, against the stock that is already there:
        // a capacity typed below the opening inventory is refused rather than
        // leaving a pile that starts the schedule overfull.
        check_opening_fits(entry.inventory.opening_tonnes(), capacity_t)?;
        entry.capacity_t = capacity_t;
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
        check_opening_fits(self.opening_tonnes(DestinationId::Solid(solid)), capacity_t)?;
        match self.solids.iter_mut().find(|entry| entry.solid == solid) {
            Some(entry) => entry.capacity_t = capacity_t,
            None if capacity_t.is_some() => self.solids.push(SolidDestination {
                solid,
                capacity_t,
                distance_km: DEFAULT_DISTANCE_KM,
                inventory: StockpileInventory::default(),
            }),
            None => {}
        }
        // An entry that says nothing the defaults do not is not kept: it would
        // be a stored setting for a solid nobody has configured.
        self.solids
            .retain(|entry| entry.capacity_t.is_some() || entry.distance_km != DEFAULT_DISTANCE_KM || !entry.inventory.is_pristine());
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
            .filter(|rule| rule.destinations.contains(&DestinationId::Standalone(id)))
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

    pub(crate) fn add_rule(&mut self, name: &str, destinations: Vec<DestinationId>) -> ScheduleResult<RuleId> {
        let name = checked_name(name)?;
        if self.rule_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = RuleId(self.next_rule_id);
        self.next_rule_id = self.next_rule_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        let destinations = checked_destinations(destinations)?;
        self.rules.push(DestinationRule {
            id,
            enabled: true,
            name,
            destinations,
            loaders: LoaderSelection::All,
            sources: MovementSourceSelection::All,
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

    /// Replace the destinations one rule may deliver to.
    ///
    /// Ordered, so the caller's order is the order they are tried; deduplicated,
    /// because naming one twice would say nothing the first mention has not; and
    /// never empty, which would be a rule that matches material and then has
    /// nowhere to put it.
    pub(crate) fn set_rule_destinations(&mut self, id: RuleId, destinations: Vec<DestinationId>) -> ScheduleResult {
        let destinations = checked_destinations(destinations)?;
        self.rule_mut(id)?.destinations = destinations;
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

    pub(crate) fn set_rule_sources(&mut self, id: RuleId, sources: MovementSourceSelection) -> ScheduleResult {
        let sources = checked_sources(sources)?;
        self.rule_mut(id)?.sources = sources;
        Ok(())
    }

    /// Replace one rule's whole condition list, validated as a set.
    pub(crate) fn set_rule_conditions(&mut self, id: RuleId, conditions: Vec<FieldCondition>) -> ScheduleResult {
        for condition in &conditions {
            check_condition(&condition.test)?;
        }
        let rule = self.rule(id).ok_or(ScheduleError::UnknownRule)?;
        check_category_conditions(&rule.sources, &conditions, &rule.conditions)?;
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
            super::trucking::checked_distance(entry.distance_km)?;
            entry.crusher.validate()?;
        }
        for (index, entry) in self.solids.iter().enumerate() {
            if self.solids[..index].iter().any(|earlier| earlier.solid == entry.solid) {
                return Err(ScheduleError::DuplicateId);
            }
            if let Some(capacity) = entry.capacity_t {
                checked_capacity(capacity)?;
            }
            super::trucking::checked_distance(entry.distance_km)?;
        }
        for entry in &mut self.standalone {
            entry.inventory.validate_loaded()?;
            check_opening_fits(entry.inventory.opening_tonnes(), entry.capacity_t)?;
        }
        for entry in &mut self.solids {
            entry.inventory.validate_loaded()?;
            check_opening_fits(entry.inventory.opening_tonnes(), entry.capacity_t)?;
        }
        for (index, rule) in self.rules.iter().enumerate() {
            if self.rules[..index].iter().any(|earlier| earlier.id == rule.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&rule.name)?;
            if self.rules[..index].iter().any(|earlier| same_name(&earlier.name, &rule.name)) {
                return Err(ScheduleError::DuplicateName(rule.name.clone()));
            }
            if rule.destinations.is_empty() {
                return Err(ScheduleError::EmptyRuleSelection);
            }
            for (position, destination) in rule.destinations.iter().enumerate() {
                if rule.destinations[..position].contains(destination) {
                    return Err(ScheduleError::DuplicateId);
                }
                if let DestinationId::Standalone(id) = destination
                    && !self.standalone.iter().any(|entry| entry.id == *id)
                {
                    return Err(ScheduleError::UnknownDestination);
                }
            }
            match &rule.loaders {
                LoaderSelection::All => {}
                LoaderSelection::Only(agents) if !agents.is_empty() => {}
                LoaderSelection::Only(_) => return Err(ScheduleError::EmptyRuleSelection),
            }
            match &rule.sources {
                MovementSourceSelection::All => {}
                MovementSourceSelection::Only(scopes) if scopes.is_empty() => return Err(ScheduleError::EmptyRuleSelection),
                MovementSourceSelection::Only(scopes) => {
                    if !scopes.iter().copied().all(MovementSourceScope::is_valid) {
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
        // Lot and portion ids are allocated per stockpile, and are protected
        // the same way: an undone lot must not hand its id to the next one.
        for entry in &mut self.standalone {
            if let Some(source) = other.standalone.iter().find(|held| held.id == entry.id) {
                entry.inventory.raise_allocator_to(&source.inventory);
            }
        }
        for entry in &mut self.solids {
            if let Some(source) = other.solids.iter().find(|held| held.solid == entry.solid) {
                entry.inventory.raise_allocator_to(&source.inventory);
            }
        }
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.enabled.hash(hasher);
        for entry in &self.standalone {
            entry.id.hash(hasher);
            entry.name.hash(hasher);
            entry.kind.hash(hasher);
            entry.capacity_t.map(f64::to_bits).hash(hasher);
            entry.distance_km.to_bits().hash(hasher);
            entry.inventory.hash_content(hasher);
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
            entry.distance_km.to_bits().hash(hasher);
            entry.inventory.hash_content(hasher);
        }
        for rule in &self.rules {
            rule.hash_content(hasher);
        }
    }

    /// Opening-lot labels are saved content but do not alter future movement
    /// coefficients or inventory feasibility.
    pub(crate) fn hash_inventory_names<H: std::hash::Hasher>(&self, hasher: &mut H) {
        for entry in &self.standalone {
            entry.inventory.hash_names(hasher);
        }
        for entry in &self.solids {
            entry.inventory.hash_names(hasher);
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self
                .standalone
                .iter()
                .map(|entry| {
                    size_of::<StandaloneDestination>()
                        + entry.name.len()
                        + entry.crusher.periods.len() * size_of::<(CalendarPeriod, CrusherOverride)>()
                        + entry.inventory.estimated_bytes()
                })
                .sum::<usize>()
            + self
                .solids
                .iter()
                .map(|entry| size_of::<SolidDestination>() + entry.inventory.estimated_bytes())
                .sum::<usize>()
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
                            MovementSourceSelection::All => 0,
                            MovementSourceSelection::Only(scopes) => size_of_val(scopes.as_slice()),
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
        self.destinations.hash(hasher);
        match &self.loaders {
            LoaderSelection::All => 0u8.hash(hasher),
            LoaderSelection::Only(agents) => {
                1u8.hash(hasher);
                agents.hash(hasher);
            }
        }
        match &self.sources {
            MovementSourceSelection::All => 0u8.hash(hasher),
            MovementSourceSelection::Only(scopes) => {
                1u8.hash(hasher);
                for scope in scopes {
                    match scope {
                        MovementSourceScope::Ground(ground) => {
                            0u8.hash(hasher);
                            hash_scope(*ground, hasher);
                        }
                        MovementSourceScope::Stockpile(id) => (1u8, id).hash(hasher),
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
            MovementSourceSelection::All => {}
            MovementSourceSelection::Only(scopes) => parts.push(tr!("destination-rule-sources-count", count = scopes.len().to_string())),
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
    /// This condition's own content hash, shared with the cashflow rules,
    /// which carry the same conditions.
    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.field.hash(hasher);
        match &self.test {
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

    pub(crate) fn estimated_bytes_public(&self) -> usize {
        self.estimated_bytes()
    }

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
pub(crate) fn check_condition(test: &ConditionTest) -> ScheduleResult {
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
    /// One-way haul distance in kilometres.
    pub(crate) distance_km: f64,
    /// Stockpile only: what it opens holding, and which end reclaim takes from.
    pub(crate) opening_t: f64,
    pub(crate) reclaim_order: ReclaimOrder,
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
            distance_km: routing.distance_km(DestinationId::Solid(solid.id)),
            opening_t: routing.opening_tonnes(DestinationId::Solid(solid.id)),
            reclaim_order: routing.reclaim_order(DestinationId::Solid(solid.id)),
            solid: Some(solid.id),
        });
    }
    for entry in &routing.standalone {
        views.push(DestinationView {
            id: DestinationId::Standalone(entry.id),
            name: entry.name.clone(),
            kind: entry.kind,
            capacity_t: entry.capacity_t,
            distance_km: entry.distance_km,
            opening_t: entry.inventory.opening_tonnes(),
            reclaim_order: entry.inventory.order,
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
                distance_km: routing.distance_km(id),
                opening_t: routing.opening_tonnes(id),
                reclaim_order: routing.reclaim_order(id),
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
                distance_km: entry.distance_km,
                opening_t: entry.inventory.opening_tonnes(),
                reclaim_order: entry.inventory.order,
                solid: None,
            })
        }
    }
}
