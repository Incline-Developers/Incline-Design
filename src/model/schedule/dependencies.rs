//! Authored dependencies between ground: "this ground is not dug until that
//! ground is". Each rule names two scopes of the Solids run, a pit, a bench
//! or a flitch: the ground that waits, and the ground it waits for.
//!
//! A rule holds every bar, dig bars and automatic bars alike: capture turns
//! it into a dependency in the drill and blast chain, the one place ground is
//! held back, so the dispatch, replay and Improve all keep to it. Ground in
//! a blast waits through the blast: the blast is not started until the
//! ground it waits for is dug. Ground the rule's scopes share waits on
//! nothing, since it cannot wait for itself.

use serde::{Deserialize, Serialize};

use super::{ScheduleError, ScheduleResult, SourceScope, destinations::BAND_TOLERANCE};

/// Identity of one dependency within its project. Never reused, never
/// rewound by an undo, like every other id in the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct DependencyId(pub(crate) u64);

/// One rule. A side not chosen yet leaves the rule doing nothing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GroundDependency {
    pub(crate) id: DependencyId,
    /// The ground that waits.
    pub(crate) ground: Option<SourceScope>,
    /// The ground that has to be dug first.
    pub(crate) after: Option<SourceScope>,
}

impl GroundDependency {
    /// Both sides, once both are chosen.
    pub(crate) fn sides(&self) -> Option<(SourceScope, SourceScope)> {
        Some((self.ground?, self.after?))
    }
}

/// Every dependency in the plan, in the order they were added.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Dependencies {
    #[serde(default)]
    pub(crate) rules: Vec<GroundDependency>,
    #[serde(default)]
    next_id: u64,
}

impl Dependencies {
    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub(crate) fn is_pristine(&self) -> bool {
        self.is_empty() && self.next_id == 0
    }

    pub(crate) fn rule(&self, id: DependencyId) -> Option<&GroundDependency> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    /// A new rule with neither side chosen.
    pub(crate) fn add(&mut self) -> ScheduleResult<DependencyId> {
        let id = DependencyId(self.next_id);
        self.next_id = self.next_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        self.rules.push(GroundDependency { id, ground: None, after: None });
        Ok(id)
    }

    pub(crate) fn set(&mut self, id: DependencyId, ground: Option<SourceScope>, after: Option<SourceScope>) -> ScheduleResult<()> {
        let rule = self.rules.iter_mut().find(|rule| rule.id == id).ok_or(ScheduleError::UnknownDependency)?;
        if ground.is_some_and(|scope| !scope.is_valid()) || after.is_some_and(|scope| !scope.is_valid()) {
            return Err(ScheduleError::UnknownDependency);
        }
        rule.ground = ground;
        rule.after = after;
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: DependencyId) -> ScheduleResult<()> {
        let before = self.rules.len();
        self.rules.retain(|rule| rule.id != id);
        if self.rules.len() == before { Err(ScheduleError::UnknownDependency) } else { Ok(()) }
    }

    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_id = self.next_id.max(other.next_id);
    }

    pub(crate) fn validate_loaded(&mut self) -> ScheduleResult<()> {
        for (index, rule) in self.rules.iter().enumerate() {
            if self.rules[..index].iter().any(|other| other.id == rule.id) {
                return Err(ScheduleError::UnknownDependency);
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
            for side in [rule.ground, rule.after] {
                side.is_some().hash(hasher);
                if let Some(scope) = side {
                    super::destinations::hash_scope(scope, hasher);
                }
            }
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>() + self.rules.len() * size_of::<GroundDependency>()
    }

    /// Rules that wait, through some chain of rules, on themselves: none of
    /// their ground is ever dug. Each such rule once, in list order.
    pub(crate) fn in_cycles(&self) -> Vec<DependencyId> {
        let rules: Vec<(DependencyId, SourceScope, SourceScope)> = self.rules.iter().filter_map(|rule| rule.sides().map(|(ground, after)| (rule.id, ground, after))).collect();
        // One rule follows another when the ground it waits for is ground
        // the other holds back.
        let next = |from: usize| -> Vec<usize> { (0..rules.len()).filter(|to| overlaps(rules[from].2, rules[*to].1)).collect() };
        let mut out = Vec::new();
        for start in 0..rules.len() {
            let mut seen = vec![false; rules.len()];
            let mut stack = next(start);
            while let Some(at) = stack.pop() {
                if at == start {
                    out.push(rules[start].0);
                    break;
                }
                if !std::mem::replace(&mut seen[at], true) {
                    stack.extend(next(at));
                }
            }
        }
        out
    }
}

/// Whether two scopes share any ground: the same solid, and one band inside
/// the other or either the whole pit.
pub(crate) fn overlaps(left: SourceScope, right: SourceScope) -> bool {
    if left.solid() != right.solid() {
        return false;
    }
    let band = |scope: SourceScope| match scope {
        SourceScope::Pit(_) => None,
        SourceScope::Bench { base, top, .. } | SourceScope::Flitch { base, top, .. } => Some((base, top)),
    };
    match (band(left), band(right)) {
        (Some((a_base, a_top)), Some((b_base, b_top))) => a_base < b_top - BAND_TOLERANCE && b_base < a_top - BAND_TOLERANCE,
        _ => true,
    }
}
