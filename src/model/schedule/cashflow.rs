//! What a movement is worth: the authored rules that put a signed value on
//! every tonne moved, and the arithmetic that adds them up.
//!
//! # Rules add, they do not resolve
//!
//! Unlike the destination rules - where order *is* the resolution and the
//! first match wins - every enabled cashflow rule that matches a movement
//! contributes its value, and the movement is worth their sum. Order therefore
//! means nothing here, which is why the rule list has no move-up and
//! move-down. Two rules that happen to describe the same movement are two
//! contributions, not a conflict: "anything to the crusher is worth 100" and
//! "digging from the pit is worth 35" are separate statements about the same
//! haul, and a pit-to-crusher tonne is worth 135.
//!
//! # Value is not eligibility
//!
//! Nothing here decides where material may go. A cashflow rule that matches a
//! movement no destination rule permits simply never applies, and a movement
//! with no matching cashflow rule is worth nothing rather than being refused.
//! Keeping the two apart is what lets the value of a haul be edited without
//! re-deciding what is allowed to happen.
//!
//! Values may be negative, and are not clamped. A negative enough movement
//! should be able to make idling the better answer; a floor at zero would
//! quietly remove that option from whatever consumes these numbers.
//!
//! # The source is where the material is now
//!
//! A reclaimed tonne's source is the stockpile it is being loaded from, never
//! the pit it originally came from. A rule that says "digging from the pit is
//! worth 35" must not pay again when that same material later leaves the
//! stockpile. Where it originally came from is a reporting question, and is
//! deliberately not part of matching.

use serde::{Deserialize, Serialize};

use super::{
    DestinationId, DestinationSelection, FieldCondition, LoaderAgentId, LoaderSelection, MovementSourceScope, MovementSourceSelection, PortionValue, RouteSource, ScheduleError,
    ScheduleResult, checked_name, destinations, same_name,
};
use crate::{i18n::tr, model::ReserveFieldId};

/// What a project's money is called. Display only - nothing here converts
/// between currencies, and a schedule's figures are all in this one.
pub(crate) const DEFAULT_CURRENCY: &str = "USD";

/// Identity of one cashflow rule within its project. Allocated and protected
/// like every other id here: never reused, never rewound by an undo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct CashflowRuleId(pub(crate) u64);

/// What a movement is doing with the material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum Activity {
    /// Material taken out of the ground.
    Dig,
    /// Material taken back out of a stockpile.
    Reclaim,
}

impl Activity {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Dig => tr!("cashflow-activity-dig"),
            Self::Reclaim => tr!("cashflow-activity-reclaim"),
        }
    }
}

/// Which activities a rule applies to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum ActivitySelection {
    /// Both, including any activity added later.
    #[default]
    All,
    Only(Activity),
}

impl ActivitySelection {
    #[allow(dead_code, reason = "reached through CashflowRule::matches")]
    fn accepts(self, activity: Activity) -> bool {
        match self {
            Self::All => true,
            Self::Only(only) => only == activity,
        }
    }

    pub(crate) fn label(self) -> String {
        match self {
            Self::All => tr!("cashflow-activity-all"),
            Self::Only(activity) => activity.label(),
        }
    }
}

/// One movement, as the value rules see it.
///
/// Identity only: the loader, where the material is now, where it is going and
/// what it is doing. The material's own properties are read through a separate
/// lookup rather than carried here, exactly as [`super::DestinationRule::accepts`]
/// reads them - a movement context that owned a property payload would be
/// copied once per rule and once per loader.
#[allow(dead_code, reason = "built by the optimised run in a later stage")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MovementContext {
    pub(crate) activity: Activity,
    pub(crate) loader: LoaderAgentId,
    /// Where the material is being loaded from *now*. For a reclaim this is
    /// the stockpile, never the pit the material originally came from.
    pub(crate) source: RouteSource,
    pub(crate) destination: DestinationId,
}

/// One rule: which movements it describes, and what each of their tonnes is
/// worth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CashflowRule {
    pub(crate) id: CashflowRuleId,
    #[serde(default = "yes")]
    pub(crate) enabled: bool,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) activity: ActivitySelection,
    #[serde(default)]
    pub(crate) loaders: LoaderSelection,
    #[serde(default)]
    pub(crate) sources: MovementSourceSelection,
    #[serde(default)]
    pub(crate) destinations: DestinationSelection,
    /// Every condition must hold. An empty list restricts nothing.
    #[serde(default)]
    pub(crate) conditions: Vec<FieldCondition>,
    /// On the schedule's nominated tonnes basis. Signed: a cost is a negative
    /// value, and zero is a rule that describes a movement without paying for
    /// it.
    pub(crate) value_per_tonne: f64,
}

fn yes() -> bool {
    true
}

impl CashflowRule {
    /// Whether this rule describes this movement of this material.
    ///
    /// Alternatives within each selector, conjunction across them. A rule
    /// matches at most *once*, however many of its selected scopes the
    /// movement happens to satisfy - it is one statement about the movement,
    /// not one per scope that happens to cover it.
    #[allow(dead_code, reason = "matching is consumed by the optimised run in a later stage")]
    pub(crate) fn matches(&self, movement: MovementContext, value: impl Fn(ReserveFieldId) -> Option<PortionValue>) -> bool {
        self.matches_identity(movement) && self.conditions.iter().all(|condition| condition.accepts(value(condition.field).as_ref()))
    }

    /// The identity half alone: enabled, this activity, loader, source and
    /// destination - and nothing about the material.
    ///
    /// For the same reason [`super::DestinationRule::accepts_identity`]
    /// exists: a blended stockpile's grade is a decision variable, so a
    /// caller pricing a reclaim has to be able to ask whether a conditional
    /// rule *would* describe this movement, in order to refuse rather than
    /// quietly price it as though the condition failed.
    pub(crate) fn matches_identity(&self, movement: MovementContext) -> bool {
        if !self.enabled {
            return false;
        }
        if !self.activity.accepts(movement.activity) {
            return false;
        }
        match &self.loaders {
            LoaderSelection::All => {}
            LoaderSelection::Only(agents) if agents.contains(&movement.loader) => {}
            LoaderSelection::Only(_) => return false,
        }
        match &self.sources {
            MovementSourceSelection::All => {}
            MovementSourceSelection::Only(scopes) if scopes.iter().any(|scope| covers(*scope, movement.source)) => {}
            MovementSourceSelection::Only(_) => return false,
        }
        match &self.destinations {
            DestinationSelection::All => {}
            DestinationSelection::Only(ids) if ids.contains(&movement.destination) => {}
            DestinationSelection::Only(_) => return false,
        }
        true
    }

    fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.id.hash(hasher);
        self.enabled.hash(hasher);
        // The name is presentation: it names the rule in the table and in an
        // explanation, and changing it must not make an optimised result look
        // as though its coefficients moved.
        self.activity.hash(hasher);
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
                            destinations::hash_scope(*ground, hasher);
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
        for condition in &self.conditions {
            condition.hash_content(hasher);
        }
        self.value_per_tonne.to_bits().hash(hasher);
    }

    /// A one-line summary of what this rule describes, for the rule table.
    pub(crate) fn summary(&self, loader_name: impl Fn(LoaderAgentId) -> String, field_name: impl Fn(ReserveFieldId) -> String) -> String {
        let mut parts = Vec::new();
        if let ActivitySelection::Only(activity) = self.activity {
            parts.push(activity.label());
        }
        match &self.loaders {
            LoaderSelection::All => {}
            LoaderSelection::Only(agents) => parts.push(agents.iter().copied().map(loader_name).collect::<Vec<_>>().join(" / ")),
        }
        if let MovementSourceSelection::Only(scopes) = &self.sources {
            parts.push(tr!("destination-rule-sources-count", count = scopes.len().to_string()));
        }
        if let DestinationSelection::Only(ids) = &self.destinations {
            parts.push(tr!("cashflow-rule-destinations-count", count = ids.len().to_string()));
        }
        for condition in &self.conditions {
            parts.push(condition.summary(&field_name(condition.field)));
        }
        if parts.is_empty() {
            return tr!("cashflow-rule-matches-all");
        }
        parts.join(" · ")
    }

    fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.name.len()
            + self.conditions.iter().map(FieldCondition::estimated_bytes_public).sum::<usize>()
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
    }
}

/// Whether one selected scope covers where the material is being loaded from.
///
/// Ground and stockpiles are disjoint: a stockpile scope never matches ex-pit
/// ground, and an ex-pit scope never matches a reclaim, however the material in
/// that stockpile originally got there.
#[allow(dead_code, reason = "reached through CashflowRule::matches")]
fn covers(scope: MovementSourceScope, source: RouteSource) -> bool {
    match (scope, source) {
        (MovementSourceScope::Ground(ground), RouteSource::Ground { solid, bench, flitch }) => ground.covers(solid, bench, flitch),
        (MovementSourceScope::Stockpile(held), RouteSource::Stockpile(from)) => held == from,
        _ => false,
    }
}

/// What one movement's tonne is worth, and which rules said so.
///
/// The contributions are kept beside the total so an explanation can be given
/// later without matching again - by then the configuration may have been
/// edited, and re-deriving the reason from it would describe a different
/// schedule than the one being explained.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Valuation {
    pub(crate) value_per_tonne: f64,
    pub(crate) contributions: Vec<(CashflowRuleId, f64)>,
}

/// Everything the Schedule workspace persists about what movements are worth.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CashflowConfig {
    pub(crate) rules: Vec<CashflowRule>,
    next_rule_id: u64,
}

impl CashflowConfig {
    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub(crate) fn is_pristine(&self) -> bool {
        self.is_empty() && self.next_rule_id == 0
    }

    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_rule_id = self.next_rule_id.max(other.next_rule_id);
    }

    pub(crate) fn rule(&self, id: CashflowRuleId) -> Option<&CashflowRule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    fn rule_mut(&mut self, id: CashflowRuleId) -> ScheduleResult<&mut CashflowRule> {
        self.rules.iter_mut().find(|rule| rule.id == id).ok_or(ScheduleError::UnknownCashflowRule)
    }

    fn name_taken(&self, name: &str, except: Option<CashflowRuleId>) -> bool {
        self.rules.iter().any(|rule| Some(rule.id) != except && same_name(&rule.name, name))
    }

    /// Add a rule worth nothing. A new rule that paid something would put a
    /// figure nobody typed into the valuation.
    pub(crate) fn add_rule(&mut self, name: &str) -> ScheduleResult<CashflowRuleId> {
        let name = checked_name(name)?;
        if self.name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = CashflowRuleId(self.next_rule_id);
        self.next_rule_id = self.next_rule_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.rules.push(CashflowRule {
            id,
            enabled: true,
            name,
            activity: ActivitySelection::All,
            loaders: LoaderSelection::All,
            sources: MovementSourceSelection::All,
            destinations: DestinationSelection::All,
            conditions: Vec::new(),
            value_per_tonne: 0.0,
        });
        Ok(id)
    }

    /// Copy a rule into an independent one directly below it. A duplicate is a
    /// second contribution, deliberately: two identical rules pay twice.
    pub(crate) fn duplicate_rule(&mut self, id: CashflowRuleId, name: &str) -> ScheduleResult<CashflowRuleId> {
        let name = checked_name(name)?;
        if self.name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let position = self.rules.iter().position(|rule| rule.id == id).ok_or(ScheduleError::UnknownCashflowRule)?;
        let new_id = CashflowRuleId(self.next_rule_id);
        self.next_rule_id = self.next_rule_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        let mut copy = self.rules[position].clone();
        copy.id = new_id;
        copy.name = name;
        self.rules.insert(position + 1, copy);
        Ok(new_id)
    }

    pub(crate) fn remove_rule(&mut self, id: CashflowRuleId) -> ScheduleResult {
        let before = self.rules.len();
        self.rules.retain(|rule| rule.id != id);
        if self.rules.len() == before {
            return Err(ScheduleError::UnknownCashflowRule);
        }
        Ok(())
    }

    pub(crate) fn rename_rule(&mut self, id: CashflowRuleId, name: &str) -> ScheduleResult {
        let name = checked_name(name)?;
        if self.name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.rule_mut(id)?.name = name;
        Ok(())
    }

    pub(crate) fn set_rule_enabled(&mut self, id: CashflowRuleId, enabled: bool) -> ScheduleResult {
        self.rule_mut(id)?.enabled = enabled;
        Ok(())
    }

    pub(crate) fn set_rule_activity(&mut self, id: CashflowRuleId, activity: ActivitySelection) -> ScheduleResult {
        self.rule_mut(id)?.activity = activity;
        Ok(())
    }

    pub(crate) fn set_rule_loaders(&mut self, id: CashflowRuleId, loaders: LoaderSelection) -> ScheduleResult {
        let loaders = checked_loaders(loaders)?;
        self.rule_mut(id)?.loaders = loaders;
        Ok(())
    }

    pub(crate) fn set_rule_sources(&mut self, id: CashflowRuleId, sources: MovementSourceSelection) -> ScheduleResult {
        let sources = checked_sources(sources)?;
        self.rule_mut(id)?.sources = sources;
        Ok(())
    }

    pub(crate) fn set_rule_destinations(&mut self, id: CashflowRuleId, destinations: DestinationSelection) -> ScheduleResult {
        let destinations = checked_destinations(destinations)?;
        self.rule_mut(id)?.destinations = destinations;
        Ok(())
    }

    pub(crate) fn set_rule_conditions(&mut self, id: CashflowRuleId, conditions: Vec<FieldCondition>) -> ScheduleResult {
        for (position, condition) in conditions.iter().enumerate() {
            destinations::check_condition(&condition.test)?;
            if conditions[..position].iter().any(|earlier| earlier.field == condition.field) {
                return Err(ScheduleError::DuplicateCondition(condition.field));
            }
        }
        // The same policy the destination rules apply, from the same helper:
        // a category is not retained by a blended pile, wherever the rule that
        // tests it lives.
        let rule = self.rule(id).ok_or(ScheduleError::UnknownCashflowRule)?;
        destinations::check_category_conditions(&rule.sources, &conditions, &rule.conditions)?;
        self.rule_mut(id)?.conditions = conditions;
        Ok(())
    }

    /// Set what one rule pays per tonne. Signed and unclamped; only a figure
    /// no number can express is refused.
    pub(crate) fn set_rule_value(&mut self, id: CashflowRuleId, value_per_tonne: f64) -> ScheduleResult {
        if !value_per_tonne.is_finite() {
            return Err(ScheduleError::InvalidValue);
        }
        self.rule_mut(id)?.value_per_tonne = value_per_tonne;
        Ok(())
    }

    /// What one tonne of this movement is worth, and which rules said so.
    ///
    /// The sum of every enabled matching rule. No match is zero - a movement
    /// nobody has priced is worth nothing, which is a different statement from
    /// a movement that is not allowed.
    #[allow(dead_code, reason = "consumed by the optimised run in a later stage")]
    pub(crate) fn valuation(&self, movement: MovementContext, value: impl Fn(ReserveFieldId) -> Option<PortionValue> + Copy) -> ScheduleResult<Valuation> {
        let mut contributions = Vec::new();
        let mut total = 0.0;
        for rule in self.rules.iter().filter(|rule| rule.matches(movement, value)) {
            total += rule.value_per_tonne;
            contributions.push((rule.id, rule.value_per_tonne));
        }
        if !total.is_finite() {
            return Err(ScheduleError::UnrepresentableValue);
        }
        Ok(Valuation {
            value_per_tonne: total,
            contributions,
        })
    }

    pub(crate) fn validate_loaded(&mut self) -> ScheduleResult {
        for (index, rule) in self.rules.iter().enumerate() {
            if self.rules[..index].iter().any(|earlier| earlier.id == rule.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&rule.name)?;
            if self.rules[..index].iter().any(|earlier| same_name(&earlier.name, &rule.name)) {
                return Err(ScheduleError::DuplicateName(rule.name.clone()));
            }
            if !rule.value_per_tonne.is_finite() {
                return Err(ScheduleError::InvalidValue);
            }
            checked_loaders(rule.loaders.clone())?;
            checked_sources(rule.sources.clone())?;
            checked_destinations(rule.destinations.clone())?;
            for (position, condition) in rule.conditions.iter().enumerate() {
                destinations::check_condition(&condition.test)?;
                if rule.conditions[..position].iter().any(|earlier| earlier.field == condition.field) {
                    return Err(ScheduleError::DuplicateCondition(condition.field));
                }
            }
        }
        if let Some(highest) = self.rules.iter().map(|rule| rule.id.0).max() {
            self.next_rule_id = self.next_rule_id.max(highest.checked_add(1).ok_or(ScheduleError::IdsExhausted)?);
        }
        Ok(())
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        for rule in &self.rules {
            rule.hash_content(hasher);
        }
    }

    /// The names, which the content hash deliberately leaves out. Folded into
    /// the project's saved-content fingerprint so a rename is unsaved work,
    /// while leaving the optimisation coefficients unmoved.
    pub(crate) fn hash_names<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        for rule in &self.rules {
            rule.name.hash(hasher);
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>() + self.rules.iter().map(CashflowRule::estimated_bytes).sum::<usize>()
    }
}

fn checked_loaders(loaders: LoaderSelection) -> ScheduleResult<LoaderSelection> {
    match loaders {
        LoaderSelection::All => Ok(LoaderSelection::All),
        LoaderSelection::Only(mut agents) => {
            agents.sort();
            agents.dedup();
            if agents.is_empty() {
                return Err(ScheduleError::EmptyRuleSelection);
            }
            Ok(LoaderSelection::Only(agents))
        }
    }
}

fn checked_sources(sources: MovementSourceSelection) -> ScheduleResult<MovementSourceSelection> {
    match sources {
        MovementSourceSelection::All => Ok(MovementSourceSelection::All),
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
            Ok(MovementSourceSelection::Only(ordered))
        }
    }
}

fn checked_destinations(destinations: DestinationSelection) -> ScheduleResult<DestinationSelection> {
    match destinations {
        DestinationSelection::All => Ok(DestinationSelection::All),
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
            Ok(DestinationSelection::Only(ordered))
        }
    }
}

/// What a movement of this many tonnes is worth.
///
/// Refused rather than saturated when the product is not a finite number: a
/// value that silently became an infinity would be compared against real ones.
#[allow(dead_code, reason = "consumed by the optimised run in a later stage")]
pub(crate) fn movement_value(tonnes: f64, valuation: &Valuation) -> ScheduleResult<f64> {
    if !tonnes.is_finite() {
        return Err(ScheduleError::UnrepresentableValue);
    }
    let value = tonnes * valuation.value_per_tonne;
    if !value.is_finite() {
        return Err(ScheduleError::UnrepresentableValue);
    }
    Ok(value)
}

/// A signed figure with its currency and basis: `+135 USD/t`.
pub(crate) fn format_value(value: f64, currency: &str) -> String {
    let rounded = if (value * 100.0).round() == 0.0 { 0.0 } else { value };
    let sign = if rounded > 0.0 { "+" } else { "" };
    let digits = {
        let mut text = format!("{:.2}", rounded);
        while text.contains('.') && text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        text
    };
    crate::i18n::tr_format!(
        literal = "%sign%%value% %currency%/t",
        sign = sign.to_owned(),
        value = digits,
        currency = currency.to_owned()
    )
}
