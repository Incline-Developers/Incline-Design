//! The truck fleet: classes of haul truck, the rules that say which of them
//! may serve which movement, and the arithmetic that turns both into a
//! resource coefficient.
//!
//! # A class is a pool, not a vehicle
//!
//! One [`TruckClass`] is a *type* of truck and the pool of them on site, sized
//! by its calendar. There is no record per vehicle: the schedule never asks
//! which truck made which trip, only how many truck-hours a class can supply.
//! A new class starts at zero units for that reason - inventing operating
//! fleet nobody entered would put capacity into a schedule the user never
//! authored.
//!
//! # Rules say what is permitted, never what is preferred
//!
//! A [`TruckingRule`] filters by loader, by source and by destination, and
//! names the classes allowed to serve whatever it matches. Every enabled rule
//! that matches contributes its classes to a union, so rule order means
//! nothing here - unlike the destination rules, where order *is* the
//! resolution. No matching rule is not a shorthand for "anything goes": it is
//! no permitted class at all, which is a configuration answer rather than an
//! invitation to guess.
//!
//! # What the coefficients are, and what they are not
//!
//! The cycle here is *travel only*: out loaded, back empty, over one
//! nominated one-way distance. Loading, spotting, dumping and queuing are all
//! excluded, and the figure is named Travel cycle time so nothing reads it as
//! a complete cycle. `travel_limited_tonnes` exists to explain one class on
//! one route and nothing else: a fleet is one resource shared across every
//! movement it is permitted to serve, and handing each route its own copy of
//! that allowance would multiply the fleet by the number of routes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    CalendarCell, CalendarPeriod, DestinationId, DestinationSelection, LoaderAgentId, LoaderSelection, MovementSourceScope, MovementSourceSelection, SCHEDULE_PERIOD_H,
    ScheduleError, ScheduleResult, checked_name, same_name,
};
/// What a truck class starts at. Zero units is deliberate; see the module
/// documentation.
pub(crate) const DEFAULT_PAYLOAD_T: f64 = 250.0;
pub(crate) const DEFAULT_LOADED_KPH: f64 = 20.0;
pub(crate) const DEFAULT_UNLOADED_KPH: f64 = 50.0;
pub(crate) const DEFAULT_UNITS: u32 = 0;

/// Identity of one truck class within its project. Allocated and protected
/// exactly like [`super::LoaderClassId`]: never reused, never rewound by an
/// undo, and never a name or a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct TruckClassId(pub(crate) u64);

/// Identity of one trucking rule within its project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct TruckingRuleId(pub(crate) u64);

/// Which of a truck class's calendar settings a cell holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TruckField {
    Units,
    Availability,
    Utilisation,
}

/// One atomic truck-calendar edit, addressed the way the loader calendar's are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TruckCellEdit {
    pub(crate) class: TruckClassId,
    pub(crate) cell: CalendarCell,
    pub(crate) field: TruckField,
    /// Domain units: percentages are fractions, and units are a whole number
    /// carried as an `f64` so one edit type serves all three rows.
    pub(crate) value: Option<f64>,
}

/// One period's overrides. Each is independent: a day may set units alone and
/// inherit both percentages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TruckPeriodOverride {
    pub(crate) units: Option<u32>,
    pub(crate) availability: Option<f64>,
    pub(crate) utilisation: Option<f64>,
}

impl TruckPeriodOverride {
    fn is_empty(&self) -> bool {
        self.units.is_none() && self.availability.is_none() && self.utilisation.is_none()
    }
}

fn one() -> f64 {
    1.0
}

/// A class's fleet size and time percentages: one default and sparse per-period
/// overrides.
///
/// Sparse exactly like [`super::LoaderCalendar`], and for the same reason: an
/// override at day 900 must not cost 900 records. Overrides never carry
/// forward - a day that says nothing inherits the default, not yesterday.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TruckCalendar {
    pub(crate) default_units: u32,
    #[serde(default = "one")]
    pub(crate) default_availability: f64,
    #[serde(default = "one")]
    pub(crate) default_utilisation: f64,
    pub(crate) periods: BTreeMap<CalendarPeriod, TruckPeriodOverride>,
}

impl Default for TruckCalendar {
    fn default() -> Self {
        Self {
            default_units: DEFAULT_UNITS,
            default_availability: 1.0,
            default_utilisation: 1.0,
            periods: BTreeMap::new(),
        }
    }
}

/// One period's resolved fleet settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TruckFleet {
    pub(crate) units: u32,
    pub(crate) availability: f64,
    pub(crate) utilisation: f64,
}

impl TruckFleet {
    /// Units scaled by the two time percentages: the fleet actually turning a
    /// wheel. Both percentages apply once, to truck supply, and never to a
    /// loader's rate.
    #[allow(dead_code, reason = "the optimised run in a later stage is what consumes truck supply")]
    pub(crate) fn effective_units(self) -> f64 {
        f64::from(self.units) * self.availability * self.utilisation
    }
}

impl TruckCalendar {
    pub(crate) fn validate(&self) -> ScheduleResult {
        checked_percentage(self.default_availability)?;
        checked_percentage(self.default_utilisation)?;
        for (period, value) in &self.periods {
            period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
            if value.is_empty() {
                return Err(ScheduleError::EmptyCalendarOverride);
            }
            if let Some(availability) = value.availability {
                checked_percentage(availability)?;
            }
            if let Some(utilisation) = value.utilisation {
                checked_percentage(utilisation)?;
            }
        }
        Ok(())
    }

    pub(crate) fn values_at(&self, period: CalendarPeriod) -> TruckFleet {
        let held = self.periods.get(&period);
        TruckFleet {
            units: held.and_then(|value| value.units).unwrap_or(self.default_units),
            availability: held.and_then(|value| value.availability).unwrap_or(self.default_availability),
            utilisation: held.and_then(|value| value.utilisation).unwrap_or(self.default_utilisation),
        }
    }

    /// The first period after `period` whose settings differ from this one's,
    /// or `None` when nothing changes again.
    ///
    /// The whole point of a sparse calendar: an interval that spans a year of
    /// default days crosses no change at all, and integrating over it costs one
    /// step rather than 365.
    #[allow(dead_code, reason = "read by the capacity integration, which the optimised run consumes")]
    pub(crate) fn next_change_after(&self, period: CalendarPeriod) -> Option<CalendarPeriod> {
        let current = self.values_at(period);
        // The period *after* an override ends is itself a change back to the
        // default, so both an override's own period and its successor are
        // candidates.
        let mut candidates: Vec<CalendarPeriod> = self
            .periods
            .keys()
            .copied()
            .chain(self.periods.keys().filter_map(|key| key.0.checked_add(1).map(CalendarPeriod)))
            .filter(|candidate| *candidate > period)
            .collect();
        candidates.sort_unstable();
        candidates.dedup();
        candidates.into_iter().find(|candidate| self.values_at(*candidate) != current)
    }

    pub(crate) fn set(&mut self, cell: CalendarCell, field: TruckField, value: Option<f64>) -> ScheduleResult {
        match cell {
            CalendarCell::Default => {
                // A default is not a thing that can be inherited from anywhere,
                // so clearing one returns it to the value a new class starts at.
                match field {
                    TruckField::Units => self.default_units = value.map(checked_units).transpose()?.unwrap_or(DEFAULT_UNITS),
                    TruckField::Availability => self.default_availability = value.map(checked_percentage).transpose()?.unwrap_or(1.0),
                    TruckField::Utilisation => self.default_utilisation = value.map(checked_percentage).transpose()?.unwrap_or(1.0),
                }
            }
            CalendarCell::Period(period) => {
                period.0.checked_add(1).ok_or(ScheduleError::CalendarPeriodOverflow)?;
                let entry = self.periods.entry(period).or_default();
                match field {
                    TruckField::Units => entry.units = value.map(checked_units).transpose()?,
                    TruckField::Availability => entry.availability = value.map(checked_percentage).transpose()?,
                    TruckField::Utilisation => entry.utilisation = value.map(checked_percentage).transpose()?,
                }
                // An override holding nothing is no override: left behind it
                // would be a record for a day that says only "as usual".
                if entry.is_empty() {
                    self.periods.remove(&period);
                }
            }
        }
        Ok(())
    }

    fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.default_units.hash(hasher);
        self.default_availability.to_bits().hash(hasher);
        self.default_utilisation.to_bits().hash(hasher);
        for (period, value) in &self.periods {
            period.hash(hasher);
            value.units.hash(hasher);
            value.availability.map(f64::to_bits).hash(hasher);
            value.utilisation.map(f64::to_bits).hash(hasher);
        }
    }
}

/// One type of haul truck, and the pool of them on site.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TruckClass {
    pub(crate) id: TruckClassId,
    pub(crate) name: String,
    /// Tonnes per load, on the schedule's nominated tonnes basis. There is no
    /// separate truck tonnage field and no wet/dry conversion: two tonnages
    /// that could disagree would make a haulage figure that cannot be
    /// reconciled with the production it came from.
    pub(crate) payload_t: f64,
    pub(crate) loaded_speed_kph: f64,
    pub(crate) unloaded_speed_kph: f64,
    #[serde(default)]
    pub(crate) calendar: TruckCalendar,
}

/// Where one movement runs from and to, as the coefficients see it.
///
/// A whole context rather than a distance, so a later haulage model can
/// replace the fixed one-way figure without any rule semantics changing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RouteContext {
    pub(crate) loader: LoaderAgentId,
    pub(crate) source: RouteSource,
    pub(crate) destination: DestinationId,
}

/// The source half of a [`RouteContext`], as the run knows it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code, reason = "the ground half is named by the optimised run in a later stage")]
pub(crate) enum RouteSource {
    Ground {
        solid: crate::model::SolidId,
        bench: (f64, f64),
        flitch: (f64, f64),
    },
    Stockpile(DestinationId),
}

/// One rule: which movements it describes, and which classes may serve them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TruckingRule {
    pub(crate) id: TruckingRuleId,
    #[serde(default = "yes")]
    pub(crate) enabled: bool,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) loaders: LoaderSelection,
    #[serde(default)]
    pub(crate) sources: MovementSourceSelection,
    #[serde(default)]
    pub(crate) destinations: DestinationSelection,
    /// The classes this rule permits. Never empty: a rule that matches
    /// movements and permits nothing is a hole, not a restriction.
    pub(crate) classes: Vec<TruckClassId>,
}

fn yes() -> bool {
    true
}

impl TruckingRule {
    /// Whether this rule describes this movement.
    ///
    /// Alternatives within each selector, conjunction across them: a rule
    /// naming two loaders and two destinations covers either loader delivering
    /// to either destination, and nothing else.
    #[allow(dead_code, reason = "matching is consumed by the optimised run in a later stage")]
    pub(crate) fn matches(&self, route: RouteContext) -> bool {
        if !self.enabled {
            return false;
        }
        match &self.loaders {
            LoaderSelection::All => {}
            LoaderSelection::Only(agents) if agents.contains(&route.loader) => {}
            LoaderSelection::Only(_) => return false,
        }
        match &self.sources {
            MovementSourceSelection::All => {}
            MovementSourceSelection::Only(scopes) if scopes.iter().any(|scope| scope_covers(*scope, route.source)) => {}
            MovementSourceSelection::Only(_) => return false,
        }
        match &self.destinations {
            DestinationSelection::All => {}
            DestinationSelection::Only(ids) if ids.contains(&route.destination) => {}
            DestinationSelection::Only(_) => return false,
        }
        true
    }

    /// The rule's own content hash, for the pipeline's input fingerprints.
    pub(crate) fn hash_content_public<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.hash_content(hasher);
    }

    fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.id.hash(hasher);
        self.enabled.hash(hasher);
        self.name.hash(hasher);
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
                            super::destinations::hash_scope(*ground, hasher);
                        }
                        MovementSourceScope::Stockpile(id) => (1u8, id).hash(hasher),
                    }
                }
            }
        }
        match &self.destinations {
            DestinationSelection::All => 0u8.hash(hasher),
            DestinationSelection::Only(ids) => {
                1u8.hash(hasher);
                ids.hash(hasher);
            }
        }
        self.classes.hash(hasher);
    }

    fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.name.len()
            + match &self.loaders {
                LoaderSelection::All => 0,
                LoaderSelection::Only(agents) => size_of_val(agents.as_slice()),
            }
            + match &self.sources {
                MovementSourceSelection::All => 0,
                MovementSourceSelection::Only(scopes) => size_of_val(scopes.as_slice()),
            }
            + match &self.destinations {
                DestinationSelection::All => 0,
                DestinationSelection::Only(ids) => size_of_val(ids.as_slice()),
            }
            + size_of_val(self.classes.as_slice())
    }
}

#[allow(dead_code, reason = "reached through TruckingRule::matches")]
fn scope_covers(scope: MovementSourceScope, source: RouteSource) -> bool {
    match (scope, source) {
        (MovementSourceScope::Ground(ground), RouteSource::Ground { solid, bench, flitch }) => ground.covers(solid, bench, flitch),
        (MovementSourceScope::Stockpile(held), RouteSource::Stockpile(from)) => held == from,
        _ => false,
    }
}

/// Everything the Schedule workspace persists about trucks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TruckFleetConfig {
    pub(crate) classes: Vec<TruckClass>,
    next_class_id: u64,
    pub(crate) rules: Vec<TruckingRule>,
    next_rule_id: u64,
}

impl TruckFleetConfig {
    pub(crate) fn is_empty(&self) -> bool {
        self.classes.is_empty() && self.rules.is_empty()
    }

    pub(crate) fn is_pristine(&self) -> bool {
        self.is_empty() && self.next_class_id == 0 && self.next_rule_id == 0
    }

    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_class_id = self.next_class_id.max(other.next_class_id);
        self.next_rule_id = self.next_rule_id.max(other.next_rule_id);
    }

    pub(crate) fn class(&self, id: TruckClassId) -> Option<&TruckClass> {
        self.classes.iter().find(|class| class.id == id)
    }

    fn class_mut(&mut self, id: TruckClassId) -> ScheduleResult<&mut TruckClass> {
        self.classes.iter_mut().find(|class| class.id == id).ok_or(ScheduleError::UnknownTruckClass)
    }

    pub(crate) fn rule(&self, id: TruckingRuleId) -> Option<&TruckingRule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    fn rule_mut(&mut self, id: TruckingRuleId) -> ScheduleResult<&mut TruckingRule> {
        self.rules.iter_mut().find(|rule| rule.id == id).ok_or(ScheduleError::UnknownTruckingRule)
    }

    fn class_name_taken(&self, name: &str, except: Option<TruckClassId>) -> bool {
        self.classes.iter().any(|class| Some(class.id) != except && same_name(&class.name, name))
    }

    fn rule_name_taken(&self, name: &str, except: Option<TruckingRuleId>) -> bool {
        self.rules.iter().any(|rule| Some(rule.id) != except && same_name(&rule.name, name))
    }

    pub(crate) fn add_class(&mut self, name: &str) -> ScheduleResult<TruckClassId> {
        let name = checked_name(name)?;
        if self.class_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = TruckClassId(self.next_class_id);
        self.next_class_id = self.next_class_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.classes.push(TruckClass {
            id,
            name,
            payload_t: DEFAULT_PAYLOAD_T,
            loaded_speed_kph: DEFAULT_LOADED_KPH,
            unloaded_speed_kph: DEFAULT_UNLOADED_KPH,
            calendar: TruckCalendar::default(),
        });
        Ok(id)
    }

    /// Copy a class, calendar overrides included, under a fresh id.
    pub(crate) fn duplicate_class(&mut self, id: TruckClassId, name: &str) -> ScheduleResult<TruckClassId> {
        let name = checked_name(name)?;
        if self.class_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let position = self.classes.iter().position(|class| class.id == id).ok_or(ScheduleError::UnknownTruckClass)?;
        let new_id = TruckClassId(self.next_class_id);
        self.next_class_id = self.next_class_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        let mut copy = self.classes[position].clone();
        copy.id = new_id;
        copy.name = name;
        self.classes.insert(position + 1, copy);
        Ok(new_id)
    }

    pub(crate) fn rename_class(&mut self, id: TruckClassId, name: &str) -> ScheduleResult {
        let name = checked_name(name)?;
        if self.class_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.class_mut(id)?.name = name;
        Ok(())
    }

    pub(crate) fn set_class_payload(&mut self, id: TruckClassId, payload_t: f64) -> ScheduleResult {
        let payload = checked_positive(payload_t, ScheduleError::InvalidPayload)?;
        self.class_mut(id)?.payload_t = payload;
        Ok(())
    }

    pub(crate) fn set_class_speeds(&mut self, id: TruckClassId, loaded_kph: f64, unloaded_kph: f64) -> ScheduleResult {
        let loaded = checked_positive(loaded_kph, ScheduleError::InvalidSpeed)?;
        let unloaded = checked_positive(unloaded_kph, ScheduleError::InvalidSpeed)?;
        let class = self.class_mut(id)?;
        class.loaded_speed_kph = loaded;
        class.unloaded_speed_kph = unloaded;
        Ok(())
    }

    /// Remove a class no rule names.
    ///
    /// Refused while any does, with those rules listed: deletion never cascades
    /// into the rules, because a rule silently left permitting nothing is a
    /// fleet decision the user did not make.
    pub(crate) fn remove_class(&mut self, id: TruckClassId) -> ScheduleResult {
        if !self.classes.iter().any(|class| class.id == id) {
            return Err(ScheduleError::UnknownTruckClass);
        }
        let users: Vec<String> = self.rules.iter().filter(|rule| rule.classes.contains(&id)).map(|rule| rule.name.clone()).collect();
        if !users.is_empty() {
            return Err(ScheduleError::TruckClassInUse(users));
        }
        self.classes.retain(|class| class.id != id);
        Ok(())
    }

    /// Apply an all-or-nothing truck-calendar batch. Duplicate addresses are
    /// rejected so paste order cannot decide the saved value.
    pub(crate) fn set_truck_cells(&mut self, edits: &[TruckCellEdit]) -> ScheduleResult {
        for (index, edit) in edits.iter().enumerate() {
            if edits[..index]
                .iter()
                .any(|earlier| earlier.class == edit.class && earlier.cell == edit.cell && earlier.field == edit.field)
            {
                return Err(ScheduleError::DuplicateCalendarCell);
            }
            let class = self.classes.iter_mut().find(|class| class.id == edit.class).ok_or(ScheduleError::UnknownTruckClass)?;
            class.calendar.set(edit.cell, edit.field, edit.value)?;
        }
        Ok(())
    }

    pub(crate) fn add_rule(&mut self, name: &str, classes: Vec<TruckClassId>) -> ScheduleResult<TruckingRuleId> {
        let name = checked_name(name)?;
        if self.rule_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let classes = self.checked_classes(classes)?;
        let id = TruckingRuleId(self.next_rule_id);
        self.next_rule_id = self.next_rule_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.rules.push(TruckingRule {
            id,
            enabled: true,
            name,
            loaders: LoaderSelection::All,
            sources: MovementSourceSelection::All,
            destinations: DestinationSelection::All,
            classes,
        });
        Ok(id)
    }

    pub(crate) fn duplicate_rule(&mut self, id: TruckingRuleId, name: &str) -> ScheduleResult<TruckingRuleId> {
        let name = checked_name(name)?;
        if self.rule_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let position = self.rules.iter().position(|rule| rule.id == id).ok_or(ScheduleError::UnknownTruckingRule)?;
        let new_id = TruckingRuleId(self.next_rule_id);
        self.next_rule_id = self.next_rule_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        let mut copy = self.rules[position].clone();
        copy.id = new_id;
        copy.name = name;
        self.rules.insert(position + 1, copy);
        Ok(new_id)
    }

    pub(crate) fn remove_rule(&mut self, id: TruckingRuleId) -> ScheduleResult {
        let before = self.rules.len();
        self.rules.retain(|rule| rule.id != id);
        if self.rules.len() == before {
            return Err(ScheduleError::UnknownTruckingRule);
        }
        Ok(())
    }

    pub(crate) fn rename_rule(&mut self, id: TruckingRuleId, name: &str) -> ScheduleResult {
        let name = checked_name(name)?;
        if self.rule_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.rule_mut(id)?.name = name;
        Ok(())
    }

    pub(crate) fn set_rule_enabled(&mut self, id: TruckingRuleId, enabled: bool) -> ScheduleResult {
        self.rule_mut(id)?.enabled = enabled;
        Ok(())
    }

    pub(crate) fn set_rule_loaders(&mut self, id: TruckingRuleId, loaders: LoaderSelection) -> ScheduleResult {
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

    pub(crate) fn set_rule_sources(&mut self, id: TruckingRuleId, sources: MovementSourceSelection) -> ScheduleResult {
        let sources = match sources {
            MovementSourceSelection::All => MovementSourceSelection::All,
            MovementSourceSelection::Only(scopes) => {
                if scopes.is_empty() {
                    return Err(ScheduleError::EmptyRuleSelection);
                }
                let mut ordered: Vec<MovementSourceScope> = Vec::with_capacity(scopes.len());
                for scope in scopes {
                    if let MovementSourceScope::Ground(ground) = scope
                        && !ground.is_valid()
                    {
                        return Err(ScheduleError::InvalidSourceScope);
                    }
                    if !ordered.contains(&scope) {
                        ordered.push(scope);
                    }
                }
                MovementSourceSelection::Only(ordered)
            }
        };
        self.rule_mut(id)?.sources = sources;
        Ok(())
    }

    pub(crate) fn set_rule_destinations(&mut self, id: TruckingRuleId, destinations: DestinationSelection) -> ScheduleResult {
        let destinations = match destinations {
            DestinationSelection::All => DestinationSelection::All,
            DestinationSelection::Only(ids) => {
                if ids.is_empty() {
                    return Err(ScheduleError::EmptyRuleSelection);
                }
                let mut ordered: Vec<DestinationId> = Vec::with_capacity(ids.len());
                for id in ids {
                    if !ordered.contains(&id) {
                        ordered.push(id);
                    }
                }
                DestinationSelection::Only(ordered)
            }
        };
        self.rule_mut(id)?.destinations = destinations;
        Ok(())
    }

    pub(crate) fn set_rule_classes(&mut self, id: TruckingRuleId, classes: Vec<TruckClassId>) -> ScheduleResult {
        let classes = self.checked_classes(classes)?;
        self.rule_mut(id)?.classes = classes;
        Ok(())
    }

    /// A rule's class list: in the order the fleet lists them, without repeats,
    /// and never empty.
    fn checked_classes(&self, classes: Vec<TruckClassId>) -> ScheduleResult<Vec<TruckClassId>> {
        let mut ordered: Vec<TruckClassId> = Vec::with_capacity(classes.len());
        for class in classes {
            if !self.classes.iter().any(|entry| entry.id == class) {
                return Err(ScheduleError::UnknownTruckClass);
            }
            if !ordered.contains(&class) {
                ordered.push(class);
            }
        }
        if ordered.is_empty() {
            return Err(ScheduleError::EmptyRuleSelection);
        }
        ordered.sort_by_key(|class| self.classes.iter().position(|entry| entry.id == *class));
        Ok(ordered)
    }

    /// Every class permitted to serve this movement: the union over the enabled
    /// rules that match it, in fleet order and without repeats.
    ///
    /// An empty answer means no rule described this movement, which is a
    /// configuration answer and not permission to use anything.
    #[allow(dead_code, reason = "consumed by the optimised run in a later stage")]
    pub(crate) fn allowed_classes(&self, route: RouteContext) -> Vec<TruckClassId> {
        let mut allowed: Vec<TruckClassId> = Vec::new();
        for rule in self.rules.iter().filter(|rule| rule.matches(route)) {
            for class in &rule.classes {
                if !allowed.contains(class) {
                    allowed.push(*class);
                }
            }
        }
        allowed.sort_by_key(|class| self.classes.iter().position(|entry| entry.id == *class));
        allowed
    }

    pub(crate) fn validate_loaded(&mut self) -> ScheduleResult {
        for (index, class) in self.classes.iter().enumerate() {
            if self.classes[..index].iter().any(|earlier| earlier.id == class.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&class.name)?;
            if self.classes[..index].iter().any(|earlier| same_name(&earlier.name, &class.name)) {
                return Err(ScheduleError::DuplicateName(class.name.clone()));
            }
            checked_positive(class.payload_t, ScheduleError::InvalidPayload)?;
            checked_positive(class.loaded_speed_kph, ScheduleError::InvalidSpeed)?;
            checked_positive(class.unloaded_speed_kph, ScheduleError::InvalidSpeed)?;
            class.calendar.validate()?;
        }
        for (index, rule) in self.rules.iter().enumerate() {
            if self.rules[..index].iter().any(|earlier| earlier.id == rule.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&rule.name)?;
            if self.rules[..index].iter().any(|earlier| same_name(&earlier.name, &rule.name)) {
                return Err(ScheduleError::DuplicateName(rule.name.clone()));
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
                    for scope in scopes {
                        if let MovementSourceScope::Ground(ground) = scope
                            && !ground.is_valid()
                        {
                            return Err(ScheduleError::InvalidSourceScope);
                        }
                    }
                }
            }
            match &rule.destinations {
                DestinationSelection::All => {}
                DestinationSelection::Only(ids) if ids.is_empty() => return Err(ScheduleError::EmptyRuleSelection),
                DestinationSelection::Only(_) => {}
            }
            if rule.classes.is_empty() {
                return Err(ScheduleError::EmptyRuleSelection);
            }
            for (position, class) in rule.classes.iter().enumerate() {
                if rule.classes[..position].contains(class) {
                    return Err(ScheduleError::DuplicateId);
                }
                if !self.classes.iter().any(|entry| entry.id == *class) {
                    return Err(ScheduleError::UnknownTruckClass);
                }
            }
        }
        let highest_class = self.classes.iter().map(|class| class.id.0).max();
        let highest_rule = self.rules.iter().map(|rule| rule.id.0).max();
        for (counter, highest) in [(&mut self.next_class_id, highest_class), (&mut self.next_rule_id, highest_rule)] {
            if let Some(highest) = highest {
                *counter = (*counter).max(highest.checked_add(1).ok_or(ScheduleError::IdsExhausted)?);
            }
        }
        Ok(())
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        for class in &self.classes {
            class.id.hash(hasher);
            class.name.hash(hasher);
            class.payload_t.to_bits().hash(hasher);
            class.loaded_speed_kph.to_bits().hash(hasher);
            class.unloaded_speed_kph.to_bits().hash(hasher);
            class.calendar.hash_content(hasher);
        }
        for rule in &self.rules {
            rule.hash_content(hasher);
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self
                .classes
                .iter()
                .map(|class| size_of::<TruckClass>() + class.name.len() + class.calendar.periods.len() * size_of::<(CalendarPeriod, TruckPeriodOverride)>())
                .sum::<usize>()
            + self.rules.iter().map(TruckingRule::estimated_bytes).sum::<usize>()
    }
}

/// What one class costs per tonne on one route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TransportCoefficients {
    /// Out loaded and back empty. Travel only - see the module documentation.
    pub(crate) travel_cycle_h: f64,
    /// Truck-hours one tonne of this class's payload consumes on this route.
    pub(crate) truck_hours_per_tonne: f64,
}

/// The travel coefficients for one class over one one-way distance.
///
/// Takes the whole [`RouteContext`] even though only the distance is read from
/// it today: the distance is a stand-in for a haulage calculation, and when
/// that arrives it must be able to consult the loader and the source without
/// any rule or caller changing.
pub(crate) fn coefficients(class: &TruckClass, _route: RouteContext, one_way_km: f64) -> ScheduleResult<TransportCoefficients> {
    let distance = checked_positive(one_way_km, ScheduleError::InvalidDistance)?;
    let loaded = checked_positive(class.loaded_speed_kph, ScheduleError::InvalidSpeed)?;
    let unloaded = checked_positive(class.unloaded_speed_kph, ScheduleError::InvalidSpeed)?;
    let payload = checked_positive(class.payload_t, ScheduleError::InvalidPayload)?;
    let travel_cycle_h = distance / loaded + distance / unloaded;
    let truck_hours_per_tonne = travel_cycle_h / payload;
    if !travel_cycle_h.is_finite() || !truck_hours_per_tonne.is_finite() || truck_hours_per_tonne <= 0.0 {
        return Err(ScheduleError::UnrepresentableTransport);
    }
    Ok(TransportCoefficients {
        travel_cycle_h,
        truck_hours_per_tonne,
    })
}

/// Truck-hours one class can supply between two elapsed project hours.
///
/// Integrated over the calendar rather than sampled at the start: an interval
/// that crosses midnight into a day with half the fleet supplies half the
/// hours for that half, and taking the opening day's settings for the whole
/// span would invent trucks that were never rostered. Sparse, so a span of
/// unchanged days costs one step.
#[allow(dead_code, reason = "consumed by the optimised run in a later stage")]
pub(crate) fn available_truck_hours(calendar: &TruckCalendar, from_h: f64, to_h: f64) -> ScheduleResult<f64> {
    if !from_h.is_finite() || !to_h.is_finite() || from_h < 0.0 {
        return Err(ScheduleError::UnrepresentableTransport);
    }
    if to_h <= from_h {
        return Ok(0.0);
    }
    let mut hours = 0.0;
    let mut at = from_h;
    let mut period = CalendarPeriod((from_h / SCHEDULE_PERIOD_H).floor() as u32);
    loop {
        let units = calendar.values_at(period).effective_units();
        // The settings hold until the next period that differs, so one step
        // covers every unchanged day between here and there.
        let next = calendar.next_change_after(period);
        let boundary = match next {
            Some(next) => f64::from(next.0) * SCHEDULE_PERIOD_H,
            None => to_h,
        };
        let end = boundary.min(to_h);
        hours += units * (end - at).max(0.0);
        if end >= to_h {
            break;
        }
        at = end;
        period = match next {
            Some(next) => next,
            None => break,
        };
    }
    if !hours.is_finite() {
        return Err(ScheduleError::UnrepresentableTransport);
    }
    Ok(hours)
}

/// What one class could move on one route if it served nothing else.
///
/// Explanatory only. The optimiser shares one class across every movement it
/// may serve, subject to
/// `Σ(movement_tonnes × truck_hours_per_tonne) ≤ class_available_truck_hours`;
/// treating this as a per-route allowance would hand every route the whole
/// fleet.
#[allow(dead_code, reason = "explanatory; see this function's own documentation")]
pub(crate) fn travel_limited_tonnes(truck_hours: f64, coefficients: TransportCoefficients) -> f64 {
    if coefficients.truck_hours_per_tonne <= 0.0 {
        return 0.0;
    }
    truck_hours / coefficients.truck_hours_per_tonne
}

fn checked_positive(value: f64, error: ScheduleError) -> ScheduleResult<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(error);
    }
    Ok(value)
}

fn checked_percentage(value: f64) -> ScheduleResult<f64> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ScheduleError::InvalidCalendarPercentage);
    }
    Ok(value)
}

/// A whole number of trucks. A fractional count is refused rather than rounded:
/// half a truck is a typo, and rounding one would quietly change the fleet.
fn checked_units(value: f64) -> ScheduleResult<u32> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(ScheduleError::InvalidTruckUnits);
    }
    Ok(value as u32)
}

/// A one-way haul distance in kilometres.
pub(crate) fn checked_distance(value: f64) -> ScheduleResult<f64> {
    checked_positive(value, ScheduleError::InvalidDistance)
}
