//! NPV schedules: the time settings that turn a scenario's shells into a
//! schedule, kept as children of the scenario they schedule.
//!
//! Every shell is a candidate final pit, planned three ways: *best* (shells
//! mined one after another, the upper NPV bound), *worst* (the pit bench by
//! bench, the lower bound) and *fixed lead* (pushbacks a set number of benches
//! apart, the case to use). Periods are implicit: every per-period input uses
//! the same period, whatever the user means by it.
//!
//! Schedules are saved inside their scenario, so they go wherever it goes
//! (project, Export/Import, Duplicate). Running one is the job in
//! [`npv_schedule_run`](crate::model::npv_schedule_run). Its value cells
//! resolve against the parent scenario's constants.

use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::{
    i18n::tr,
    model::optimization::{BlockModelFields, FieldValue, OptimizationScenario, ShellFieldMode, format_factor, round_factor},
};

/// Which shells the fixed-lead case mines as pushbacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PushbackChoice {
    /// Each shell is a pushback.
    #[default]
    #[serde(alias = "EvenTonnage")]
    EveryShell,
    /// The parent's shells at the chosen revenue factors ([`NpvSchedule::pushback_factors`]).
    Factors,
}

impl PushbackChoice {
    pub(crate) const ALL: [Self; 2] = [Self::EveryShell, Self::Factors];

    pub(crate) fn label(self) -> String {
        match self {
            Self::EveryShell => tr!("sched-pushbacks-every-shell"),
            Self::Factors => tr!("sched-pushbacks-factors"),
        }
    }
}

// An "evenly spaced by tonnage" choice was dropped: picking among the shells a
// run made cannot split a gap between them. Files that hold it read it as
// every shell (the alias on `EveryShell`).

/// A capacity per period: no limit, or an amount. The amount is kept while
/// there is no limit, so switching back gives it back.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "CapacityRepr")]
pub(crate) struct Capacity {
    pub(crate) limited: bool,
    pub(crate) amount: FieldValue<f64>,
}

/// What a capacity was saved as: the current shape, or a plain amount from
/// before "no limit" existed (any amount above 0 or a constant was a limit).
#[derive(Deserialize)]
#[serde(untagged)]
enum CapacityRepr {
    Current {
        limited: bool,
        #[serde(default)]
        amount: FieldValue<f64>,
    },
    Amount(FieldValue<f64>),
}

impl From<CapacityRepr> for Capacity {
    fn from(repr: CapacityRepr) -> Self {
        match repr {
            CapacityRepr::Current { limited, amount } => Self { limited, amount },
            CapacityRepr::Amount(amount) => Self {
                limited: amount.constant.is_some() || amount.value > 0.0,
                amount,
            },
        }
    }
}

/// Which case's periods are written into the block model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PeriodCase {
    #[default]
    FixedLead,
    Best,
    Worst,
}

impl PeriodCase {
    pub(crate) const ALL: [Self; 3] = [Self::FixedLead, Self::Best, Self::Worst];

    pub(crate) fn label(self) -> String {
        match self {
            Self::FixedLead => tr!("sched-case-fixed-lead"),
            Self::Best => tr!("sched-case-best"),
            Self::Worst => tr!("sched-case-worst"),
        }
    }
}

/// Tonnes a processing method may take per period.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ProcessLimit {
    /// The method's name, as the scenario's methods grid (and the report) names it.
    pub(crate) method: String,
    pub(crate) max_tonnes: Capacity,
}

/// How much of an element may be sold per period, in the element's sales
/// unit (Revenues).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct SellLimit {
    pub(crate) element: String,
    pub(crate) max_sold: Capacity,
}

/// A grade of one element, as stockpiles held their starting grades before
/// they were of the main quality field only. Read from older files only.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct LegacyGrade {
    element: String,
    grade: FieldValue<f64>,
}

/// Ore set aside for one or more processing methods, kept apart from other
/// stockpiles and reclaimed at its average grade. It takes ore at or above
/// its min grade that a method it feeds cannot take in the period.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Stockpile {
    pub(crate) name: String,
    /// The methods it takes ore for and is reclaimed to, by name.
    pub(crate) feeds: Vec<String>,
    pub(crate) capacity: Capacity,
    /// Lowest grade of the main quality field sent here (its cut-off).
    pub(crate) min_grade: FieldValue<f64>,
    /// Cost per tonne reclaimed.
    pub(crate) rehandle_cost: FieldValue<f64>,
    /// Tonnes on it when the schedule starts.
    pub(crate) initial_tonnes: FieldValue<f64>,
    /// Their grade of the main quality field.
    pub(crate) initial_grade: FieldValue<f64>,
    /// Older files: the one method fed (now `feeds`). Never written.
    #[serde(skip_serializing)]
    method: String,
    /// Older files: starting grades per element (now `initial_grade`). Never written.
    #[serde(skip_serializing)]
    initial_grades: Vec<LegacyGrade>,
}

impl Stockpile {
    pub(crate) fn named(name: String) -> Self {
        Self { name, ..Self::default() }
    }

    /// Bring a stockpile saved by an earlier build up to date.
    fn migrate(&mut self, quality_field: &str) {
        let method = std::mem::take(&mut self.method);
        if !method.is_empty() && self.feeds.is_empty() {
            self.feeds.push(method);
        }
        if let Some(grade) = std::mem::take(&mut self.initial_grades).into_iter().find(|grade| grade.element == quality_field) {
            self.initial_grade = grade.grade;
        }
    }
}

/// What an event's change applies to.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum EventTarget {
    /// A cost only, no change of capacity.
    #[default]
    None,
    Mining,
    /// A processing method's maximum, by name.
    Method(String),
    TotalProcess,
    /// An element's selling limit.
    Selling(String),
}

impl EventTarget {
    pub(crate) fn label(&self) -> String {
        match self {
            Self::None => tr!("sched-event-target-none"),
            Self::Mining => tr!("sched-event-target-mining"),
            Self::Method(method) => tr!("sched-event-target-method", method = method.clone()),
            Self::TotalProcess => tr!("sched-event-target-total"),
            Self::Selling(element) => tr!("sched-event-target-selling", element = element.clone()),
        }
    }
}

/// Something that happens in a period: a capital cost, a change of capacity
/// from that period on, or both. "Period 3, +1 Mt mill, capex 15 M" is one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct NpvScheduleEvent {
    /// 1-based; capex in period 0 is the schedule's initial capex.
    pub(crate) period: u32,
    pub(crate) description: String,
    pub(crate) capex: FieldValue<f64>,
    pub(crate) target: EventTarget,
    /// The change to the target's capacity per period, from this period on.
    pub(crate) delta: FieldValue<f64>,
}

impl Default for NpvScheduleEvent {
    fn default() -> Self {
        Self {
            period: 1,
            description: String::new(),
            capex: FieldValue::new(0.0),
            target: EventTarget::None,
            delta: FieldValue::new(0.0),
        }
    }
}

/// One schedule of a scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct NpvSchedule {
    pub(crate) id: u64,
    pub(crate) name: String,

    // Pushbacks
    pub(crate) pushbacks: PushbackChoice,
    /// The parent's revenue factors chosen as pushbacks, for [`PushbackChoice::Factors`].
    pub(crate) pushback_factors: Vec<f64>,
    /// Benches each pushback stays ahead of the next one.
    pub(crate) lead_benches: u32,

    // Periods
    /// The most periods a schedule may take: a guard against a pit no
    /// capacity ever finishes.
    pub(crate) max_periods: u32,
    /// Percent per period; cash flows are discounted at the end of their period.
    pub(crate) discount_rate: FieldValue<f64>,

    // Capacities
    /// Tonnes mined per period, ore and waste.
    pub(crate) mining_capacity: Capacity,
    pub(crate) process_limits: Vec<ProcessLimit>,
    /// Every method together.
    pub(crate) total_process_limit: Capacity,
    pub(crate) sell_limits: Vec<SellLimit>,

    pub(crate) stockpiles: Vec<Stockpile>,

    // Costs and events
    /// Capital spent in period 0.
    pub(crate) capex_initial: FieldValue<f64>,
    /// Cost of every period the mine runs (Whittle's time cost).
    pub(crate) fixed_cost: FieldValue<f64>,
    pub(crate) events: Vec<NpvScheduleEvent>,

    // Outputs
    pub(crate) write_period_field: bool,
    pub(crate) period_case: PeriodCase,
    pub(crate) period_field_mode: ShellFieldMode,
    pub(crate) period_field: String,
    pub(crate) period_new_field: String,
    /// Into the parent's reports folder.
    pub(crate) create_reports: bool,
}

impl Default for NpvSchedule {
    fn default() -> Self {
        Self::new(0, String::new())
    }
}

/// Why a schedule cannot run as it is, one line each.
pub(crate) type Issues = Vec<String>;

impl NpvSchedule {
    pub(crate) fn new(id: u64, name: String) -> Self {
        Self {
            id,
            name,
            pushbacks: PushbackChoice::EveryShell,
            pushback_factors: Vec::new(),
            lead_benches: 2,
            max_periods: 50,
            discount_rate: FieldValue::new(10.0),
            mining_capacity: Capacity::default(),
            process_limits: Vec::new(),
            total_process_limit: Capacity::default(),
            sell_limits: Vec::new(),
            stockpiles: Vec::new(),
            capex_initial: FieldValue::new(0.0),
            fixed_cost: FieldValue::new(0.0),
            events: Vec::new(),
            write_period_field: false,
            period_case: PeriodCase::FixedLead,
            period_field_mode: ShellFieldMode::New,
            period_field: String::new(),
            period_new_field: tr!("sched-default-period-field"),
            create_reports: false,
        }
    }

    /// A key that changes when anything a run depends on changes: everything
    /// but the name.
    pub(crate) fn fingerprint(&self) -> u64 {
        let mut settings = self.clone();
        settings.name.clear();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&settings).unwrap_or_default().hash(&mut hasher);
        hasher.finish()
    }

    /// One row per processing method of `parent` and one selling row per
    /// element of its Revenues, skipping those the grids already have.
    pub(crate) fn add_all_limits(&mut self, parent: &OptimizationScenario) {
        for method in method_names(parent) {
            if !self.process_limits.iter().any(|limit| limit.method == method) {
                self.process_limits.push(ProcessLimit {
                    method,
                    ..ProcessLimit::default()
                });
            }
        }
        for element in element_names(parent) {
            if !self.sell_limits.iter().any(|limit| limit.element == element) {
                self.sell_limits.push(SellLimit { element, ..SellLimit::default() });
            }
        }
    }

    /// Bring a schedule saved by an earlier build up to date.
    pub(crate) fn migrate(&mut self, quality_field: &str) {
        for stockpile in &mut self.stockpiles {
            stockpile.migrate(quality_field);
        }
    }

    /// Whether mining, processing (every method and the total) and selling
    /// all have no limit: nothing then holds the schedule back, so it is not run.
    pub(crate) fn unlimited(&self) -> bool {
        !self.mining_capacity.limited
            && !self.total_process_limit.limited
            && self.process_limits.iter().all(|limit| !limit.max_tonnes.limited)
            && self.sell_limits.iter().all(|limit| !limit.max_sold.limited)
    }

    /// Everything that stops this schedule running under `parent`: names the
    /// parent does not have, constants it does not define, negative amounts,
    /// limits of 0, no limit at all, events past the last period or on a
    /// capacity with no limit, and capacities the events take below 0. The
    /// editor marks the same cells red.
    pub(crate) fn issues(&self, parent: &OptimizationScenario) -> Issues {
        let mut issues = Issues::new();
        let constants = &parent.constants;
        let methods = method_names(parent);
        let elements = element_names(parent);
        {
            let mut amount = |value: &FieldValue<f64>, what: String| match value.resolve(constants) {
                None => issues.push(tr!("sched-issue-constant", what = what, constant = value.constant.clone().unwrap_or_default())),
                Some(number) if !number.is_finite() || number < 0.0 => issues.push(tr!("sched-issue-negative", what = what)),
                Some(_) => {}
            };
            amount(&self.discount_rate, tr!("sched-discount-rate"));
            for stockpile in &self.stockpiles {
                amount(&stockpile.min_grade, stockpile.name.clone());
                amount(&stockpile.initial_tonnes, stockpile.name.clone());
                amount(&stockpile.initial_grade, stockpile.name.clone());
                amount(&stockpile.rehandle_cost, stockpile.name.clone());
            }
            amount(&self.capex_initial, tr!("sched-capex-initial"));
            amount(&self.fixed_cost, tr!("sched-fixed-cost"));
            for event in &self.events {
                amount(&event.capex, event_name(event));
            }
        }
        // A limit: a constant that resolves, above 0.
        let mut capacity = |capacity: &Capacity, what: String| {
            if !capacity.limited {
                return;
            }
            match capacity.amount.resolve(constants) {
                None => issues.push(tr!("sched-issue-constant", what = what, constant = capacity.amount.constant.clone().unwrap_or_default())),
                Some(number) if !number.is_finite() || number < 0.0 => issues.push(tr!("sched-issue-negative", what = what)),
                Some(0.0) => issues.push(tr!("sched-issue-zero-limit", what = what)),
                Some(_) => {}
            }
        };
        capacity(&self.mining_capacity, tr!("sched-mining-capacity"));
        capacity(&self.total_process_limit, tr!("sched-total-process-limit"));
        for limit in &self.process_limits {
            capacity(&limit.max_tonnes, limit.method.clone());
        }
        for limit in &self.sell_limits {
            capacity(&limit.max_sold, limit.element.clone());
        }
        for stockpile in &self.stockpiles {
            capacity(&stockpile.capacity, stockpile.name.clone());
        }
        if self.unlimited() {
            issues.push(tr!("sched-issue-no-limits"));
        }
        // A change may lower a capacity, so it may be negative.
        for event in self.events.iter().filter(|event| event.delta.resolve(constants).is_none()) {
            issues.push(tr!(
                "sched-issue-constant",
                what = event_name(event),
                constant = event.delta.constant.clone().unwrap_or_default()
            ));
        }

        let mut unknown = |name: &str, known: &[String], what: String| {
            if !known.iter().any(|known| known == name) {
                issues.push(tr!("sched-issue-unknown", what = what, name = name.to_owned()));
            }
        };
        for limit in &self.process_limits {
            unknown(&limit.method, &methods, tr!("sched-method"));
        }
        for limit in &self.sell_limits {
            unknown(&limit.element, &elements, tr!("sched-element"));
        }
        for stockpile in &self.stockpiles {
            for method in &stockpile.feeds {
                unknown(method, &methods, tr!("sched-method"));
            }
        }
        for event in &self.events {
            match &event.target {
                EventTarget::Method(method) => unknown(method, &methods, tr!("sched-method")),
                EventTarget::Selling(element) => unknown(element, &elements, tr!("sched-element")),
                _ => {}
            }
        }
        for stockpile in self.stockpiles.iter().filter(|stockpile| stockpile.feeds.is_empty()) {
            issues.push(tr!("sched-issue-no-feeds", name = stockpile.name.clone()));
        }
        if !self.stockpiles.is_empty() && parent.quality_field.is_empty() {
            issues.push(tr!("sched-issue-no-quality"));
        }

        self.event_issues(parent, &mut issues);

        if self.pushbacks == PushbackChoice::Factors {
            let factors = parent.output.factors().unwrap_or_default();
            if self.pushback_factors.is_empty() {
                issues.push(tr!("sched-issue-no-pushbacks"));
            }
            for factor in &self.pushback_factors {
                if !factors.iter().any(|known| round_factor(*known) == round_factor(*factor)) {
                    issues.push(tr!("sched-issue-factor", factor = format_factor(*factor)));
                }
            }
        }
        if self.max_periods == 0 {
            issues.push(tr!("sched-issue-no-periods"));
        }
        issues
    }

    /// The capacity an event changes, or `None` for a cost-only event or a
    /// target the schedule has no row for.
    pub(crate) fn event_capacity(&self, target: &EventTarget) -> Option<&Capacity> {
        match target {
            EventTarget::None => None,
            EventTarget::Mining => Some(&self.mining_capacity),
            EventTarget::TotalProcess => Some(&self.total_process_limit),
            EventTarget::Method(method) => self.process_limits.iter().find(|limit| limit.method == *method).map(|limit| &limit.max_tonnes),
            EventTarget::Selling(element) => self.sell_limits.iter().find(|limit| limit.element == *element).map(|limit| &limit.max_sold),
        }
    }

    /// Events past the last period, on a capacity with no limit, or taking a
    /// capacity below 0 in some period.
    fn event_issues(&self, parent: &OptimizationScenario, issues: &mut Issues) {
        let constants = &parent.constants;
        for event in self.events.iter().filter(|event| event.period == 0 || event.period > self.max_periods) {
            issues.push(tr!("sched-issue-event-period", what = event_name(event), max = self.max_periods.to_string()));
        }
        // Per target, the capacity's amount and every change to it, in period order.
        let mut checked: Vec<&EventTarget> = Vec::new();
        for event in self.events.iter().filter(|event| event.target != EventTarget::None) {
            if checked.contains(&&event.target) {
                continue;
            }
            checked.push(&event.target);
            let Some(capacity) = self.event_capacity(&event.target) else {
                // The target names no row of the grids: the unknown-name check
                // covers a method or element the parent lacks; a known one
                // without a row has no limit.
                if matches!(event.target, EventTarget::Method(_) | EventTarget::Selling(_)) {
                    issues.push(tr!("sched-issue-event-unlimited", what = event_name(event), target = event.target.label()));
                }
                continue;
            };
            if !capacity.limited {
                issues.push(tr!("sched-issue-event-unlimited", what = event_name(event), target = event.target.label()));
                continue;
            }
            let Some(start) = capacity.amount.resolve(constants) else {
                continue;
            };
            let mut changes: Vec<(u32, f64)> = self
                .events
                .iter()
                .filter(|other| other.target == event.target)
                .filter_map(|other| other.delta.resolve(constants).map(|delta| (other.period, delta)))
                .collect();
            changes.sort_by_key(|(period, _)| *period);
            let mut running = start;
            for (period, delta) in changes {
                running += delta;
                if running < 0.0 {
                    issues.push(tr!("sched-issue-event-below-zero", target = event.target.label(), period = period.to_string()));
                    break;
                }
            }
        }
    }

    /// The block model field the periods are written into: `Ok(None)` when
    /// none is wanted, otherwise its name, or why it cannot be written. As the
    /// parent's shell field: a text field, overwritten if it exists, never one
    /// the parent reads or writes.
    pub(crate) fn period_field_target(&self, parent: &OptimizationScenario, fields: &BlockModelFields) -> Result<Option<String>, String> {
        if !self.write_period_field {
            return Ok(None);
        }
        let name = match self.period_field_mode {
            ShellFieldMode::Existing => self.period_field.trim(),
            ShellFieldMode::New => self.period_new_field.trim(),
        };
        if name.is_empty() || (self.period_field_mode == ShellFieldMode::Existing && !fields.text.iter().any(|field| field == name)) {
            return Err(tr!("sched-period-field-missing"));
        }
        let numeric = fields.all.iter().any(|field| field == name) && !fields.text.iter().any(|field| field == name);
        let shell_field = parent.shell_field_target(fields).ok().flatten();
        if parent.used_fields().contains(name) || numeric || shell_field.as_deref() == Some(name) {
            return Err(tr!("sched-period-field-taken", field = name.to_owned()));
        }
        Ok(Some(name.to_owned()))
    }
}

/// A capacity per period as a run uses it: `None` has no limit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PeriodLimit(Option<Vec<f64>>);

impl PeriodLimit {
    pub(crate) const NONE: Self = Self(None);

    /// The limit in `period` (1-based), never below 0; infinite without one.
    pub(crate) fn at(&self, period: u32) -> f64 {
        match &self.0 {
            None => f64::INFINITY,
            Some(amounts) => amounts.get(period as usize - 1).or(amounts.last()).copied().unwrap_or(0.0).max(0.0),
        }
    }
}

/// A stockpile with its amounts resolved.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedStockpile {
    pub(crate) name: String,
    pub(crate) feeds: Vec<String>,
    /// Infinite without a limit.
    pub(crate) capacity: f64,
    pub(crate) min_grade: f64,
    pub(crate) rehandle_cost: f64,
    pub(crate) initial_tonnes: f64,
    pub(crate) initial_grade: f64,
}

/// A schedule as a run reads it: constants resolved, and every capacity per
/// period with the events' changes added from their period on.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedNpvSchedule {
    /// The chosen revenue factors, or `None` for every shell.
    pub(crate) pushback_factors: Option<Vec<f64>>,
    pub(crate) lead: u32,
    pub(crate) max_periods: u32,
    /// Discount rate as a fraction per period.
    pub(crate) rate: f64,
    pub(crate) mining: PeriodLimit,
    /// By method name; a method without a row has no limit.
    pub(crate) methods: Vec<(String, PeriodLimit)>,
    pub(crate) total: PeriodLimit,
    /// By element name; an element without a row has no limit.
    pub(crate) selling: Vec<(String, PeriodLimit)>,
    pub(crate) stockpiles: Vec<ResolvedStockpile>,
    pub(crate) capex_initial: f64,
    pub(crate) fixed_cost: f64,
    /// Capital of the events per period: `capex[0]` is period 1's.
    pub(crate) capex: Vec<f64>,
}

impl ResolvedNpvSchedule {
    pub(crate) fn capex_in(&self, period: u32) -> f64 {
        self.capex.get(period as usize - 1).copied().unwrap_or(0.0)
    }
}

impl NpvSchedule {
    /// The settings resolved against `parent`'s constants. Meant for a
    /// schedule without [issues](Self::issues): anything unresolved counts 0.
    pub(crate) fn resolve(&self, parent: &OptimizationScenario) -> ResolvedNpvSchedule {
        let constants = &parent.constants;
        let amount = |value: &FieldValue<f64>| value.resolve(constants).filter(|number| number.is_finite()).unwrap_or(0.0);
        let periods = self.max_periods as usize;
        let limit = |capacity: &Capacity, target: &EventTarget| -> PeriodLimit {
            if !capacity.limited {
                return PeriodLimit::NONE;
            }
            let mut amounts = vec![amount(&capacity.amount); periods];
            for event in self.events.iter().filter(|event| event.target == *target && event.period >= 1) {
                let delta = amount(&event.delta);
                for value in amounts.iter_mut().skip(event.period as usize - 1) {
                    *value += delta;
                }
            }
            PeriodLimit(Some(amounts))
        };
        let mut capex = vec![0.0; periods];
        for event in &self.events {
            if (1..=self.max_periods).contains(&event.period) {
                capex[event.period as usize - 1] += amount(&event.capex);
            }
        }
        ResolvedNpvSchedule {
            pushback_factors: (self.pushbacks == PushbackChoice::Factors).then(|| self.pushback_factors.clone()),
            lead: self.lead_benches,
            max_periods: self.max_periods,
            rate: amount(&self.discount_rate) / 100.0,
            mining: limit(&self.mining_capacity, &EventTarget::Mining),
            methods: self
                .process_limits
                .iter()
                .map(|row| (row.method.clone(), limit(&row.max_tonnes, &EventTarget::Method(row.method.clone()))))
                .collect(),
            total: limit(&self.total_process_limit, &EventTarget::TotalProcess),
            selling: self
                .sell_limits
                .iter()
                .map(|row| (row.element.clone(), limit(&row.max_sold, &EventTarget::Selling(row.element.clone()))))
                .collect(),
            stockpiles: self
                .stockpiles
                .iter()
                .map(|stockpile| ResolvedStockpile {
                    name: stockpile.name.clone(),
                    feeds: stockpile.feeds.clone(),
                    capacity: if stockpile.capacity.limited {
                        amount(&stockpile.capacity.amount)
                    } else {
                        f64::INFINITY
                    },
                    min_grade: amount(&stockpile.min_grade),
                    rehandle_cost: amount(&stockpile.rehandle_cost),
                    initial_tonnes: amount(&stockpile.initial_tonnes),
                    initial_grade: amount(&stockpile.initial_grade),
                })
                .collect(),
            capex_initial: amount(&self.capex_initial),
            fixed_cost: amount(&self.fixed_cost),
            capex,
        }
    }
}

fn event_name(event: &NpvScheduleEvent) -> String {
    if event.description.trim().is_empty() {
        tr!("sched-event-in-period", period = event.period.to_string())
    } else {
        event.description.clone()
    }
}

/// The parent's processing methods by name, once each (rows sharing a name
/// are one method, as in the report).
pub(crate) fn method_names(parent: &OptimizationScenario) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for method in &parent.methods {
        if !method.name.trim().is_empty() && !names.contains(&method.name) {
            names.push(method.name.clone());
        }
    }
    names
}

/// The elements the parent sells (its Revenues), once each.
pub(crate) fn element_names(parent: &OptimizationScenario) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for revenue in &parent.revenues {
        if !revenue.element.is_empty() && !names.contains(&revenue.element) {
            names.push(revenue.element.clone());
        }
    }
    names
}

/// Money in a few characters: `1.48 B`, `356.2 M`, `12.4 k`, `-3.1 M`.
pub(crate) fn format_money(value: f64) -> String {
    let magnitude = value.abs();
    let (scaled, suffix) = if magnitude >= 1e9 {
        (value / 1e9, " B")
    } else if magnitude >= 1e6 {
        (value / 1e6, " M")
    } else if magnitude >= 1e3 {
        (value / 1e3, " k")
    } else {
        (value, "")
    };
    let digits = if suffix.is_empty() {
        0
    } else if scaled.abs() >= 10.0 {
        1
    } else {
        2
    };
    format!("{scaled:.digits$}{suffix}")
}
