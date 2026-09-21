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
pub(crate) mod cashflow;
pub(crate) mod destinations;
pub(crate) mod dispatch;
pub(crate) mod experiment;
pub(crate) mod inventory;
pub(crate) mod optimisation;
pub(crate) mod production;
pub(crate) mod sequence;
pub(crate) mod trucking;

pub(crate) use calendar::{CalendarCell, CalendarCellEdit, CalendarField, CalendarPeriod, CompiledRateCalendar, LoaderCalendar, RateKind, SCHEDULE_PERIOD_H};
pub(crate) use cashflow::{Activity, ActivitySelection, CashflowConfig, CashflowRuleId};
pub(crate) use destinations::{
    Bound, ConditionTest, CrusherCalendar, CrusherCell, CrusherCellEdit, CrusherOverride, DestinationId, DestinationKind, DestinationSelection, FieldCondition, LoaderSelection,
    MovementSourceScope, MovementSourceSelection, PortionValue, RoutingConfig, RuleId, SourceScope, StandaloneDestinationId,
};
pub(crate) use dispatch::{DispatchAgent, DispatchBar, DispatchBlock, DispatchDestination, DispatchError, DispatchInput, DispatchOutcome, DispatchPortion, DispatchSchedule};
pub(crate) use inventory::{OpeningLotId, OpeningPortionId, OpeningValue, ReclaimOrder};
pub(crate) use production::{DestinationProduction, PeriodProduction};
pub(crate) use sequence::{DigBlockRef, DigOrder, Footprint};
pub(crate) use trucking::{RouteContext, RouteSource, TruckCellEdit, TruckClassId, TruckField, TruckFleetConfig, TruckingRuleId};

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

/// What one bar's work actually is.
///
/// Two activities, one bar: everything a bar carries besides this - its
/// identity, its name, its machine, its lane and its window - means the same
/// thing whichever it is, so only the work itself is typed. A reclaim bar
/// holds no dig order at all rather than an empty one, which is what makes
/// "edit this bar's dig sequence" unavailable on it by construction rather
/// than by a check someone has to remember.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum BarWork {
    /// Ground, in the order it is dug.
    Dig(DigOrder),
    /// Material taken back out of a stockpile.
    Reclaim(ReclaimWork),
}

impl Default for BarWork {
    fn default() -> Self {
        Self::Dig(DigOrder::default())
    }
}

/// Reclaiming from an explicitly permitted set of stockpiles, up to an
/// optional total.
///
/// # The list is permission, not priority
///
/// Every stockpile named here is one the planner has *approved* this bar to
/// draw on. Which of them a given segment actually takes from is an
/// optimisation answer, so the list's display order carries no economic
/// meaning and must never become a hidden preference: it is stored in the
/// order it was authored so a hash and a rebuilt model are deterministic, and
/// nothing reads position 0 as "the one to try first". That is the difference
/// between this and a dig order, where position *is* the instruction.
///
/// The cap is the bar's own and is cumulative across every permitted source
/// together, so approving a second stockpile widens where the tonnes may come
/// from and never how many there are.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, from = "ReclaimRepr")]
pub(crate) struct ReclaimWork {
    /// The stockpiles this bar may take from, in authored order and without
    /// repeats. Never empty. An id that no longer names a stockpile stays
    /// exactly as authored and is reported unresolved: a bar silently
    /// repointed at another pile, or silently narrowed to the sources that
    /// still resolve, would reclaim material nobody chose.
    pub(crate) sources: Vec<DestinationId>,
    /// The most this bar may reclaim over the whole calculation, or `None` for
    /// no cap of its own. Cumulative across the bar rather than per period, and
    /// per bar rather than per stockpile - a copy of the bar gets its own.
    #[serde(default)]
    pub(crate) maximum_t: Option<f64>,
}

/// What a saved reclaim bar is read through.
///
/// A bar used to name exactly one stockpile, and projects written then are
/// still on disk; the singular field is accepted and folded into the list so
/// those projects load rather than being refused a field they were right to
/// write. Only reading goes through this - what is written is the list.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReclaimRepr {
    #[serde(default)]
    source: Option<DestinationId>,
    #[serde(default)]
    sources: Vec<DestinationId>,
    #[serde(default)]
    maximum_t: Option<f64>,
}

impl From<ReclaimRepr> for ReclaimWork {
    fn from(repr: ReclaimRepr) -> Self {
        let ReclaimRepr { source, mut sources, maximum_t } = repr;
        if let Some(source) = source
            && !sources.contains(&source)
        {
            sources.insert(0, source);
        }
        Self { sources, maximum_t }
    }
}

impl ReclaimWork {
    /// Whether this is work at all: at least one permitted stockpile, no
    /// repeats, and a cap that is absent or a finite positive tonnage. Zero
    /// is refused rather than read as "reclaim nothing", which is what
    /// deleting the bar says.
    pub(crate) fn is_valid(&self) -> bool {
        !self.sources.is_empty()
            && self.sources.iter().enumerate().all(|(index, source)| !self.sources[..index].contains(source))
            && self.maximum_t.is_none_or(|maximum| maximum.is_finite() && maximum > 0.0)
    }
}

/// One authored bar on the Gantt: some work, the machine asked to do it, the
/// lane it competes in, and the period it may be worked in.
///
/// A bar carries no duration of its own work. Its window is the period a
/// loader is *allowed* to work it; what is actually executed in that period
/// is the evaluator's answer, drawn as a separate indicator, and a bar whose
/// work outlasts its window simply leaves the rest behind for a later bar to
/// take.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "BarRepr")]
pub(crate) struct ScheduleBar {
    pub(crate) id: BarId,
    /// What the user calls this bar, or empty to take the label its work
    /// derives.
    pub(crate) name: String,
    /// What this bar does. Owned outright: a copy holds its own, so editing one
    /// never reaches another.
    pub(crate) work: BarWork,
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

/// What a saved bar is read through.
///
/// Every bar used to be a dig sequence, and its name lived inside the dig
/// order; projects written then are still on disk, so that shape is accepted
/// and read as [`BarWork::Dig`] with its membership intact. Only reading goes
/// through this - what is written is the typed form.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BarRepr {
    id: BarId,
    #[serde(default)]
    name: String,
    #[serde(default)]
    order: Option<LegacyOrder>,
    #[serde(default)]
    work: Option<BarWork>,
    #[serde(default)]
    agent: Option<LoaderAgentId>,
    #[serde(default)]
    priority: u32,
    #[serde(default)]
    window: WorkWindow,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyOrder {
    #[serde(default)]
    name: String,
    #[serde(default)]
    members: Vec<DigBlockRef>,
}

impl From<BarRepr> for ScheduleBar {
    fn from(repr: BarRepr) -> Self {
        let BarRepr {
            id,
            name,
            order,
            work,
            agent,
            priority,
            window,
        } = repr;
        let legacy_name = order.as_ref().map(|order| order.name.clone()).unwrap_or_default();
        Self {
            id,
            name: if name.trim().is_empty() { legacy_name } else { name },
            work: work.unwrap_or_else(|| {
                BarWork::Dig(DigOrder {
                    members: order.map(|order| order.members).unwrap_or_default(),
                })
            }),
            agent,
            priority,
            window,
        }
    }
}

impl ScheduleBar {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn has_custom_name(&self) -> bool {
        !self.name.trim().is_empty()
    }

    /// The ground this bar digs, in order. Empty for a reclaim bar: it has no
    /// dig order, which is a different statement from having an empty one.
    pub(crate) fn members(&self) -> &[DigBlockRef] {
        match &self.work {
            BarWork::Dig(order) => order.members(),
            BarWork::Reclaim(_) => &[],
        }
    }

    pub(crate) fn dig_order(&self) -> Option<&DigOrder> {
        match &self.work {
            BarWork::Dig(order) => Some(order),
            BarWork::Reclaim(_) => None,
        }
    }

    pub(crate) fn reclaim(&self) -> Option<&ReclaimWork> {
        match &self.work {
            BarWork::Dig(_) => None,
            BarWork::Reclaim(work) => Some(work),
        }
    }

    pub(crate) fn is_reclaim(&self) -> bool {
        matches!(self.work, BarWork::Reclaim(_))
    }
}

/// A machine type: what it is called, how fast it digs, and how fast it
/// reclaims.
///
/// Two rates, not one scaled from the other: loading a stockpile back into a
/// truck is a different job from digging a face, and nothing here knows the
/// ratio. They begin equal only because that is the least surprising starting
/// point; from then on each is edited on its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "ClassRepr")]
pub(crate) struct LoaderClass {
    pub(crate) id: LoaderClassId,
    pub(crate) name: String,
    /// Tonnes per hour. Strictly positive and finite; see [`ScheduleError`].
    pub(crate) default_dig_rate_tph: f64,
    /// Tonnes per productive hour reclaiming. Strictly positive and finite.
    pub(crate) default_reclaim_rate_tph: f64,
}

/// What a saved class is read through: a project written before reclaim existed
/// has no reclaim rate, and starts with its dig rate rather than with a figure
/// nobody chose.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassRepr {
    id: LoaderClassId,
    name: String,
    default_dig_rate_tph: f64,
    #[serde(default)]
    default_reclaim_rate_tph: Option<f64>,
}

impl From<ClassRepr> for LoaderClass {
    fn from(repr: ClassRepr) -> Self {
        Self {
            id: repr.id,
            name: repr.name,
            default_dig_rate_tph: repr.default_dig_rate_tph,
            default_reclaim_rate_tph: repr.default_reclaim_rate_tph.unwrap_or(repr.default_dig_rate_tph),
        }
    }
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
    /// A capacity that is negative or not a finite number. Blank is a
    /// legitimate answer and never reaches this; zero is a real capacity.
    InvalidCapacity,
    /// An id that names no destination in this plan.
    UnknownDestination,
    /// An id that names no routing rule in this plan.
    UnknownRule,
    /// A destination still named by the listed rules. Deletion never
    /// cascades into the rule list.
    DestinationInUse(Vec<String>),
    /// A crusher setting addressed to a destination that is not a crusher.
    NotACrusher,
    /// A loader or source selection that lists nothing. "Only these, and
    /// there are none" matches nothing and is not what "All" means.
    EmptyRuleSelection,
    /// A source scope whose band is not a finite, non-empty elevation range.
    InvalidSourceScope,
    /// A condition that restricts nothing: no category values, or a numerical
    /// range with neither bound.
    EmptyCondition,
    /// A bound that is not a finite number.
    InvalidBound,
    /// Bounds describing an interval no value can be in.
    EmptyInterval,
    /// Two conditions on one field within a rule. They would be ANDed, and a
    /// single condition can already express any interval or value set.
    DuplicateCondition(ReserveFieldId),
    /// One category value listed twice in one condition.
    DuplicateConditionValue(String),
    /// A rule already at the top or bottom of the priority order.
    RuleAtEnd,
    /// A truck payload that is zero, negative, infinite or NaN.
    InvalidPayload,
    /// A travel speed that is zero, negative, infinite or NaN.
    InvalidSpeed,
    /// A haul distance that is zero, negative, infinite or NaN.
    InvalidDistance,
    /// A truck count that is fractional, negative or not a number. Refused
    /// rather than rounded: half a truck is a typo.
    InvalidTruckUnits,
    /// An id that names no truck class in this plan.
    UnknownTruckClass,
    /// An id that names no trucking rule in this plan.
    UnknownTruckingRule,
    /// A truck class still named by the listed trucking rules.
    TruckClassInUse(Vec<String>),
    /// Derived transport figures that no finite number can express.
    UnrepresentableTransport,
    /// A value per tonne that is not a finite number. Negative and zero are
    /// both legitimate answers and never reach this.
    InvalidValue,
    /// An id that names no cashflow rule in this plan.
    UnknownCashflowRule,
    /// A summed coefficient or movement value no finite number can express.
    UnrepresentableValue,
    /// A currency label that is empty once trimmed.
    EmptyCurrency,
    /// A lot or portion tonnage that is zero, negative, infinite or NaN. Blank
    /// is not a legitimate answer here: a portion of nothing is not a portion.
    InvalidLotTonnes,
    /// An authored property value that is not a finite number, or a category
    /// label that is empty once trimmed. A value nobody entered is *missing*,
    /// which is a different state and never reaches this.
    InvalidLotValue,
    /// Two values for one field on one portion.
    DuplicateLotValue,
    /// An id that names no opening lot in this stockpile.
    UnknownLot,
    /// An id that names no portion in this lot.
    UnknownPortion,
    /// The last portion of a lot. An empty lot is not stock; deleting the lot
    /// is the edit that says so.
    LastPortion,
    /// A lot already at the oldest or newest end of the order.
    LotAtEnd,
    /// Opening stock that would not fit the stockpile's capacity. Equality is
    /// valid - a pile that starts exactly full is a real answer.
    OpeningOverCapacity,
    /// A receipt with nowhere to go: the pile is at its capacity.
    StockpileFull,
    /// More tonnes asked of a lot than it has left. Refused rather than
    /// clamped: the caller decided how much to ask for.
    LotOverdrawn,
    /// A model interval that is not one, or an instant before the grid's
    /// origin.
    InvalidInterval,
    /// A reclaim source that is not a stockpile.
    NotAStockpile,
    /// A maximum reclaimed tonnage that is zero, negative or not finite. Blank
    /// is a legitimate answer - no cap of this bar's own - and never reaches
    /// this.
    InvalidReclaimLimit,
    /// A dig edit addressed to a reclaim bar, or the other way round.
    WrongActivity,
    /// A category condition on a rule whose sources include stockpiles, where
    /// the blended representation has no category left to test. Refused when
    /// authored; an existing one is preserved and the rule reported invalid
    /// instead. See [`destinations::category_conditions_available`].
    CategoryConditionUnsupported,
    /// An experimental optimiser setting outside what the model accepts: a
    /// non-positive horizon, interval, time limit or chunk capacity, or a
    /// relative gap outside `0..=1`.
    InvalidExperimentSetting,
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
            Self::InvalidCapacity => tr!("destination-error-invalid-capacity"),
            Self::UnknownDestination => tr!("destination-error-unknown"),
            Self::UnknownRule => tr!("destination-error-unknown-rule"),
            Self::DestinationInUse(rules) => tr!("destination-error-in-use", rules = rules.join(", ")),
            Self::NotACrusher => tr!("destination-error-not-a-crusher"),
            Self::EmptyRuleSelection => tr!("destination-error-empty-selection"),
            Self::InvalidSourceScope => tr!("destination-error-invalid-scope"),
            Self::EmptyCondition => tr!("destination-error-empty-condition"),
            Self::InvalidBound => tr!("destination-error-invalid-bound"),
            Self::EmptyInterval => tr!("destination-error-empty-interval"),
            Self::DuplicateCondition(_) => tr!("destination-error-duplicate-condition"),
            Self::DuplicateConditionValue(value) => tr!("destination-error-duplicate-value", value = value.clone()),
            Self::RuleAtEnd => tr!("destination-error-rule-at-end"),
            Self::InvalidPayload => tr!("truck-error-invalid-payload"),
            Self::InvalidSpeed => tr!("truck-error-invalid-speed"),
            Self::InvalidDistance => tr!("truck-error-invalid-distance"),
            Self::InvalidTruckUnits => tr!("truck-error-invalid-units"),
            Self::UnknownTruckClass => tr!("truck-error-unknown-class"),
            Self::UnknownTruckingRule => tr!("truck-error-unknown-rule"),
            Self::TruckClassInUse(rules) => tr!("truck-error-class-in-use", rules = rules.join(", ")),
            Self::UnrepresentableTransport => tr!("truck-error-unrepresentable"),
            Self::InvalidValue => tr!("cashflow-error-invalid-value"),
            Self::UnknownCashflowRule => tr!("cashflow-error-unknown-rule"),
            Self::UnrepresentableValue => tr!("cashflow-error-unrepresentable"),
            Self::EmptyCurrency => tr!("cashflow-error-empty-currency"),
            Self::InvalidLotTonnes => tr!("inventory-error-invalid-tonnes"),
            Self::InvalidLotValue => tr!("inventory-error-invalid-value"),
            Self::DuplicateLotValue => tr!("inventory-error-duplicate-value"),
            Self::UnknownLot => tr!("inventory-error-unknown-lot"),
            Self::UnknownPortion => tr!("inventory-error-unknown-portion"),
            Self::LastPortion => tr!("inventory-error-last-portion"),
            Self::LotAtEnd => tr!("inventory-error-lot-at-end"),
            Self::OpeningOverCapacity => tr!("inventory-error-over-capacity"),
            Self::StockpileFull => tr!("inventory-error-stockpile-full"),
            Self::LotOverdrawn => tr!("inventory-error-overdrawn"),
            Self::InvalidInterval => tr!("inventory-error-invalid-interval"),
            Self::NotAStockpile => tr!("reclaim-error-not-a-stockpile"),
            Self::InvalidReclaimLimit => tr!("reclaim-error-invalid-limit"),
            Self::WrongActivity => tr!("reclaim-error-wrong-activity"),
            Self::CategoryConditionUnsupported => tr!("destination-error-category-unsupported"),
            Self::InvalidExperimentSetting => tr!("experiment-error-invalid-setting"),
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
    /// Destinations, their capacities and the ordered routing rules; see
    /// [`destinations`]. Default-empty and default-off, so a project saved
    /// before routing existed opens with the dig-only behaviour it was
    /// authored against.
    #[serde(default)]
    routing: RoutingConfig,
    /// Truck classes, their fleet calendars and the trucking rules. Empty and
    /// inert: nothing in the current dispatcher reads them, so a project that
    /// has never opened the Trucks pages behaves exactly as it did.
    #[serde(default)]
    trucks: TruckFleetConfig,
    /// What movements are worth, and what the money is called. Empty and
    /// inert: nothing in the current dispatcher reads a value.
    #[serde(default)]
    cashflow: CashflowConfig,
    #[serde(default = "default_currency")]
    currency: String,
    /// Explicit settings for the experimental blended optimiser; see
    /// [`experiment`]. Persisted in every build so a project written by one
    /// with the experiment enabled round-trips through one without it.
    #[serde(default)]
    experiment: experiment::ExperimentConfig,
}

fn default_currency() -> String {
    cashflow::DEFAULT_CURRENCY.to_owned()
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
            routing: RoutingConfig::default(),
            trucks: TruckFleetConfig::default(),
            cashflow: CashflowConfig::default(),
            currency: default_currency(),
            experiment: experiment::ExperimentConfig::default(),
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
        self.name.is_empty()
            && self.classes.is_empty()
            && self.agents.is_empty()
            && self.bars.is_empty()
            && self.tonnage_field.is_none()
            && self.bar_height == DEFAULT_BAR_HEIGHT
            && self.routing.is_pristine()
            && self.trucks.is_empty()
            && self.cashflow.is_empty()
            && self.currency == cashflow::DEFAULT_CURRENCY
            && self.experiment.is_pristine()
    }

    /// No visible content or retired identities to preserve in a save/import.
    pub(crate) fn is_pristine(&self) -> bool {
        self.is_empty() && self.next_class_id == 0 && self.next_agent_id == 0 && self.next_bar_id == 0 && self.trucks.is_pristine() && self.cashflow.is_pristine()
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
        self.routing.raise_allocator_to(&other.routing);
        self.trucks.raise_allocator_to(&other.trucks);
        self.cashflow.raise_allocator_to(&other.cashflow);
    }

    pub(crate) fn routing(&self) -> &RoutingConfig {
        &self.routing
    }

    /// Edit the routing configuration. Every caller goes through
    /// [`crate::model::Command::SetSchedulePlan`] like the rest of the plan,
    /// so one committed routing edit is one undo step.
    pub(crate) fn routing_mut(&mut self) -> &mut RoutingConfig {
        &mut self.routing
    }

    pub(crate) fn trucks(&self) -> &TruckFleetConfig {
        &self.trucks
    }

    /// Edit the truck fleet. Every caller goes through
    /// [`crate::model::Command::SetSchedulePlan`] like the rest of the plan,
    /// so one committed truck edit is one undo step.
    pub(crate) fn trucks_mut(&mut self) -> &mut TruckFleetConfig {
        &mut self.trucks
    }

    pub(crate) fn cashflow(&self) -> &CashflowConfig {
        &self.cashflow
    }

    /// Edit the cashflow rules. One committed edit is one undo step, like the
    /// rest of the plan.
    #[allow(dead_code, reason = "read by the feature-gated experimental capture")]
    pub(crate) fn experiment(&self) -> &experiment::ExperimentConfig {
        &self.experiment
    }

    /// Mutable access for the command layer, which snapshots the whole plan
    /// either side of an edit exactly as every other setting here does.
    pub(crate) fn experiment_mut(&mut self) -> &mut experiment::ExperimentConfig {
        &mut self.experiment
    }

    pub(crate) fn cashflow_mut(&mut self) -> &mut CashflowConfig {
        &mut self.cashflow
    }

    pub(crate) fn currency(&self) -> &str {
        &self.currency
    }

    /// The label figures are shown with. Display only - nothing is converted.
    pub(crate) fn set_currency(&mut self, currency: &str) -> ScheduleResult {
        let trimmed = currency.trim();
        if trimmed.is_empty() {
            return Err(ScheduleError::EmptyCurrency);
        }
        self.currency = trimmed.to_owned();
        Ok(())
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

    /// Add a machine type. Its reclaim rate starts at the dig rate it was
    /// created with, and is its own from then on.
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
            default_reclaim_rate_tph: rate,
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

    /// Set the rate this type digs at. Deliberately leaves the reclaim rate
    /// alone: once the two exist they are separate answers, and moving one
    /// because the other moved would overwrite a figure the user chose.
    pub(crate) fn set_class_rate(&mut self, id: LoaderClassId, rate_tph: f64) -> ScheduleResult {
        let rate = checked_rate(rate_tph)?;
        let class = self.classes.iter_mut().find(|class| class.id == id).ok_or(ScheduleError::UnknownClass)?;
        class.default_dig_rate_tph = rate;
        Ok(())
    }

    pub(crate) fn set_class_reclaim_rate(&mut self, id: LoaderClassId, rate_tph: f64) -> ScheduleResult {
        let rate = checked_rate(rate_tph)?;
        let class = self.classes.iter_mut().find(|class| class.id == id).ok_or(ScheduleError::UnknownClass)?;
        class.default_reclaim_rate_tph = rate;
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

    /// Whether the current dig-only dispatcher must refuse this plan.
    pub(crate) fn has_reclaim_bars(&self) -> bool {
        self.bars.iter().any(|bar| bar.reclaim().is_some())
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
        self.bars.iter().filter(move |bar| bar.dig_order().is_some_and(|order| order.contains(block)))
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
            name,
            work: BarWork::Dig(DigOrder::default()),
            agent,
            priority,
            window,
        });
        Ok(id)
    }

    /// Add a bar that may reclaim from any of an explicitly permitted set of
    /// stockpiles.
    ///
    /// Each source is checked to be a stockpile *of this plan's standalone
    /// destinations* only when it is one of them: a solid-backed stockpile's
    /// kind is the solid's, which the plan cannot see, so that half is checked
    /// where the document is - and an id that stops naming a stockpile later
    /// stays as authored and is reported unresolved.
    pub(crate) fn add_reclaim_bar(
        &mut self,
        name: &str,
        agent: Option<LoaderAgentId>,
        priority: u32,
        window: WorkWindow,
        sources: Vec<DestinationId>,
        maximum_t: Option<f64>,
    ) -> ScheduleResult<BarId> {
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
        let sources = self.checked_reclaim_sources(sources)?;
        let work = ReclaimWork { sources, maximum_t };
        if !work.is_valid() {
            return Err(ScheduleError::InvalidReclaimLimit);
        }
        let id = self.allocate_bar_id()?;
        self.bars.push(ScheduleBar {
            id,
            name,
            work: BarWork::Reclaim(work),
            agent,
            priority,
            window,
        });
        Ok(id)
    }

    /// Whether a destination can be a reclaim source, as far as the plan can
    /// tell: a standalone destination it holds must be a stockpile, and a
    /// solid-backed one is the document's to judge.
    fn check_reclaim_source(&self, source: DestinationId) -> ScheduleResult {
        if let DestinationId::Standalone(id) = source {
            let entry = self.routing.standalone(id).ok_or(ScheduleError::UnknownDestination)?;
            if entry.kind != DestinationKind::Stockpile {
                return Err(ScheduleError::NotAStockpile);
            }
        }
        Ok(())
    }

    /// The permitted stockpiles of one reclaim bar, deduplicated and never
    /// empty.
    ///
    /// Order is preserved because it is the authored order and because a
    /// deterministic list is what makes the built model and its fingerprint
    /// reproducible - not because position means anything.
    fn checked_reclaim_sources(&self, sources: Vec<DestinationId>) -> ScheduleResult<Vec<DestinationId>> {
        if sources.is_empty() {
            return Err(ScheduleError::EmptyRuleSelection);
        }
        let mut kept: Vec<DestinationId> = Vec::with_capacity(sources.len());
        for source in sources {
            self.check_reclaim_source(source)?;
            if !kept.contains(&source) {
                kept.push(source);
            }
        }
        Ok(kept)
    }

    /// Replace the set of stockpiles a reclaim bar is permitted to draw on.
    ///
    /// Atomic: either the whole set is accepted or the bar is untouched, so a
    /// selection containing one unusable entry never leaves the bar half
    /// edited. A set equal to the one already held is not an edit, which is
    /// what keeps a popup that closes unchanged out of the undo history.
    pub(crate) fn set_reclaim_sources(&mut self, id: BarId, sources: Vec<DestinationId>) -> ScheduleResult<bool> {
        let sources = self.checked_reclaim_sources(sources)?;
        let bar = self.bar_mut(id)?;
        match &mut bar.work {
            BarWork::Reclaim(work) => {
                if work.sources == sources {
                    return Ok(false);
                }
                work.sources = sources;
                Ok(true)
            }
            BarWork::Dig(_) => Err(ScheduleError::WrongActivity),
        }
    }

    /// Set or clear the most one reclaim bar may take over the calculation.
    pub(crate) fn set_reclaim_maximum(&mut self, id: BarId, maximum_t: Option<f64>) -> ScheduleResult {
        if maximum_t.is_some_and(|maximum| !maximum.is_finite() || maximum <= 0.0) {
            return Err(ScheduleError::InvalidReclaimLimit);
        }
        let bar = self.bar_mut(id)?;
        match &mut bar.work {
            BarWork::Reclaim(work) => work.maximum_t = maximum_t,
            BarWork::Dig(_) => return Err(ScheduleError::WrongActivity),
        }
        Ok(())
    }

    pub(crate) fn rename_bar(&mut self, id: BarId, name: &str) -> ScheduleResult {
        let name = name.trim().to_owned();
        if !self.bars.iter().any(|bar| bar.id == id) {
            return Err(ScheduleError::UnknownBar);
        }
        if self.bar_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.bars.iter_mut().find(|bar| bar.id == id).expect("checked above").name = name;
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
        copy.name = name;
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

    /// One bar's dig order, refusing a bar that has none. A reclaim bar is not
    /// a dig sequence with nothing in it, so an edit addressed to its ground is
    /// refused rather than applied to an order invented on the spot.
    fn dig_order_mut(&mut self, id: BarId) -> ScheduleResult<&mut DigOrder> {
        match &mut self.bar_mut(id)?.work {
            BarWork::Dig(order) => Ok(order),
            BarWork::Reclaim(_) => Err(ScheduleError::WrongActivity),
        }
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
        self.dig_order_mut(id)?.insert(position, block)
    }

    pub(crate) fn remove_bar_member(&mut self, id: BarId, position: usize) -> ScheduleResult {
        self.dig_order_mut(id)?.remove(position)
    }

    /// Move one block to a different place in a bar's dig order.
    pub(crate) fn move_bar_member(&mut self, id: BarId, from: usize, to: usize) -> ScheduleResult {
        self.dig_order_mut(id)?.move_member(from, to)
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
        let mut order = DigOrder::default();
        for block in members {
            order.insert(usize::MAX, block)?;
        }
        self.dig_order_mut(id)?.members = order.members;
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
            checked_rate(class.default_reclaim_rate_tph)?;
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
            agent.calendar.compile_rate(RateKind::Reclaim, class.default_reclaim_rate_tph)?;
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
            match &bar.work {
                BarWork::Dig(order) => order.check_loaded()?,
                // The sources are deliberately *not* required to resolve: a
                // stockpile deleted or retyped since leaves the bar holding an
                // unresolved reference, which the readiness report names. A file
                // is not broken because the project moved on.
                BarWork::Reclaim(work) if work.is_valid() => {}
                BarWork::Reclaim(_) => return Err(ScheduleError::InvalidReclaimLimit),
            }
        }
        self.routing.validate_loaded()?;
        self.trucks.validate_loaded()?;
        self.cashflow.validate_loaded()?;
        if self.currency.trim().is_empty() {
            return Err(ScheduleError::EmptyCurrency);
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
            class.default_reclaim_rate_tph.to_bits().hash(hasher);
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
                override_.reclaim_rate_tph.map(f64::to_bits).hash(hasher);
            }
        }
        self.tonnage_field.hash(hasher);
        self.bar_height.to_bits().hash(hasher);
        self.routing.hash_content(hasher);
        self.routing.hash_inventory_names(hasher);
        self.trucks.hash_content(hasher);
        // Both halves: the coefficients an optimisation would use, and the
        // names it would not. A rename is unsaved work without being a change
        // of coefficient.
        self.cashflow.hash_content(hasher);
        self.cashflow.hash_names(hasher);
        self.currency.hash(hasher);
        self.experiment.hash_content(hasher);
        for bar in &self.bars {
            bar.id.hash(hasher);
            bar.name.hash(hasher);
            if let Some(work) = bar.reclaim() {
                1u8.hash(hasher);
                work.sources.hash(hasher);
                work.maximum_t.map(f64::to_bits).hash(hasher);
            } else {
                0u8.hash(hasher);
            }
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
                .map(|bar| size_of::<ScheduleBar>() + bar.name.len() + size_of_val(bar.members()))
                .sum::<usize>()
            + self.routing.estimated_bytes()
            + self.trucks.estimated_bytes()
            + self.cashflow.estimated_bytes()
            + self.currency.len()
            + self.experiment.estimated_bytes()
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
