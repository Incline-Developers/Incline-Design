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

use serde::{Deserialize, Serialize};

use crate::i18n::{tr, tr_format};

/// Identity of one loader class within its project.
///
/// Allocated once and never reused, so a rename leaves every agent pointing
/// at the same machine type. Serialized, so it survives save and reopen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct LoaderClassId(pub(crate) u64);

/// Identity of one loader agent within its project. See [`LoaderClassId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct LoaderAgentId(pub(crate) u64);

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
}

impl ScheduleError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::EmptyName => tr!("schedule-error-empty-name"),
            Self::DuplicateName(name) => tr!("schedule-error-duplicate-name", name = name.clone()),
            Self::InvalidRate => tr!("schedule-error-invalid-rate"),
            Self::UnknownClass => tr!("schedule-error-unknown-class"),
            Self::UnknownAgent => tr!("schedule-error-unknown-agent"),
            Self::ClassInUse(agents) => tr!("schedule-error-class-in-use", agents = agents.join(", ")),
            Self::IdsExhausted => tr!("schedule-error-ids-exhausted"),
            Self::DuplicateId => tr!("schedule-error-duplicate-id"),
        }
    }
}

pub(crate) type ScheduleResult<T = ()> = Result<T, ScheduleError>;

/// Everything the Schedule workspace persists for one project.
///
/// Ordered lists, not maps: the Gantt draws one row per agent in this order,
/// and a file that round-trips must bring the same order back.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
        self.name.is_empty() && self.classes.is_empty() && self.agents.is_empty()
    }

    /// No visible content or retired identities to preserve in a save/import.
    pub(crate) fn is_pristine(&self) -> bool {
        self.is_empty() && self.next_class_id == 0 && self.next_agent_id == 0
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
        self.agents.push(LoaderAgent { id, name, class_id });
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

    pub(crate) fn remove_agent(&mut self, id: LoaderAgentId) -> ScheduleResult {
        if !self.agents.iter().any(|agent| agent.id == id) {
            return Err(ScheduleError::UnknownAgent);
        }
        self.agents.retain(|agent| agent.id != id);
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
            if !self.classes.iter().any(|class| class.id == agent.class_id) {
                return Err(ScheduleError::UnknownClass);
            }
        }
        let highest_class = self.classes.iter().map(|class| class.id.0).max();
        let highest_agent = self.agents.iter().map(|agent| agent.id.0).max();
        for (counter, highest) in [(&mut self.next_class_id, highest_class), (&mut self.next_agent_id, highest_agent)] {
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
        }
    }

    /// An estimate of what one snapshot of this plan costs the undo history.
    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.name.len()
            + self.classes.iter().map(|class| size_of::<LoaderClass>() + class.name.len()).sum::<usize>()
            + self.agents.iter().map(|agent| size_of::<LoaderAgent>() + agent.name.len()).sum::<usize>()
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
