//! Authored limits on how many loaders may work in one area at a time: "at
//! most two loaders on each bench of Pit A", "one loader in each blast".
//!
//! Each rule names a scope of the Solids run - a pit, a bench or a flitch -
//! and whether the limit is on all of it together, on each bench in it or
//! on each blast in it. Capture turns the rules into areas of ground, each
//! with the most loaders it takes. The hourly dispatch gives an area to no
//! more loaders than that in an interval, the loaders that worked there in
//! the interval before keeping their places, and the replay checks it; the
//! long-term plan caps what an area gives in a period at what that many
//! loaders dig.

use serde::{Deserialize, Serialize};

use super::{ScheduleError, ScheduleResult, SourceScope};
use crate::i18n::tr;

/// Identity of one limit within its project. Never reused, never rewound by
/// an undo, like every other id in the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct LoaderLimitId(pub(crate) u64);

/// What a limit counts loaders over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum LimitArea {
    /// The whole scope together.
    Whole,
    /// Each bench in the scope on its own.
    #[default]
    EachBench,
    /// Each blast in the scope on its own. Ground in no blast is not held.
    EachBlast,
}

impl LimitArea {
    pub(crate) const ALL: [Self; 3] = [Self::Whole, Self::EachBench, Self::EachBlast];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Whole => tr!("loader-limit-area-whole"),
            Self::EachBench => tr!("loader-limit-area-bench"),
            Self::EachBlast => tr!("loader-limit-area-blast"),
        }
    }
}

/// One rule. A rule with no scope chosen yet does nothing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoaderLimit {
    pub(crate) id: LoaderLimitId,
    pub(crate) scope: Option<SourceScope>,
    #[serde(default)]
    pub(crate) area: LimitArea,
    /// The most loaders working in one area at a time.
    pub(crate) loaders: u32,
}

/// Every limit in the plan, in the order they were added.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoaderLimits {
    #[serde(default)]
    pub(crate) rules: Vec<LoaderLimit>,
    #[serde(default)]
    next_id: u64,
}

impl LoaderLimits {
    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub(crate) fn is_pristine(&self) -> bool {
        self.is_empty() && self.next_id == 0
    }

    pub(crate) fn rule(&self, id: LoaderLimitId) -> Option<&LoaderLimit> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    /// A new rule: one loader on each bench, its scope not chosen.
    pub(crate) fn add(&mut self) -> ScheduleResult<LoaderLimitId> {
        let id = LoaderLimitId(self.next_id);
        self.next_id = self.next_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.rules.push(LoaderLimit {
            id,
            scope: None,
            area: LimitArea::EachBench,
            loaders: 1,
        });
        Ok(id)
    }

    pub(crate) fn set(&mut self, changed: LoaderLimit) -> ScheduleResult<()> {
        if changed.scope.is_some_and(|scope| !scope.is_valid()) {
            return Err(ScheduleError::UnknownLoaderLimit);
        }
        let rule = self.rules.iter_mut().find(|rule| rule.id == changed.id).ok_or(ScheduleError::UnknownLoaderLimit)?;
        *rule = changed;
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: LoaderLimitId) -> ScheduleResult<()> {
        let before = self.rules.len();
        self.rules.retain(|rule| rule.id != id);
        if self.rules.len() == before { Err(ScheduleError::UnknownLoaderLimit) } else { Ok(()) }
    }

    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_id = self.next_id.max(other.next_id);
    }

    pub(crate) fn validate_loaded(&mut self) -> ScheduleResult<()> {
        for (index, rule) in self.rules.iter().enumerate() {
            if self.rules[..index].iter().any(|other| other.id == rule.id) {
                return Err(ScheduleError::UnknownLoaderLimit);
            }
        }
        if let Some(highest) = self.rules.iter().map(|rule| rule.id.0).max() {
            self.next_id = self.next_id.max(highest.checked_add(1).ok_or(ScheduleError::IdsExhausted)?);
        }
        Ok(())
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.rules.len().hash(hasher);
        for rule in &self.rules {
            rule.id.hash(hasher);
            rule.scope.is_some().hash(hasher);
            if let Some(scope) = rule.scope {
                super::destinations::hash_scope(scope, hasher);
            }
            rule.area.hash(hasher);
            rule.loaders.hash(hasher);
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>() + self.rules.len() * size_of::<LoaderLimit>()
    }
}
