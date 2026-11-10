//! HiGHS's `HighsConflictPool`: conflicts found by conflict analysis, each
//! a set of bound changes that cannot all hold, aged out when unused.

use std::collections::BTreeSet;

use super::{BoundType, DomainChange, change_log::ChangeLog};

/// A change to the pool, for the domains propagating its conflicts.
#[derive(Clone, Debug)]
pub(crate) enum ConflictEvent {
    /// `conflict` was added with these entries.
    Added {
        conflict: usize,
        entries: Vec<DomainChange>,
    },
    Deleted {
        conflict: usize,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ConflictPool {
    agelim: usize,
    softlimit: usize,
    age_distribution: Vec<usize>,
    ages: Vec<i16>,
    modification: Vec<u32>,
    conflict_entries: Vec<DomainChange>,
    /// Each conflict's range in `conflict_entries`; `None` once deleted.
    conflict_ranges: Vec<Option<(usize, usize)>>,
    /// Free storage as `(length, start)`.
    free_spaces: BTreeSet<(usize, usize)>,
    deleted_conflicts: Vec<usize>,
    pub log: ChangeLog<ConflictEvent>,
}

impl ConflictPool {
    pub fn new(agelim: usize, softlimit: usize) -> Self {
        Self {
            agelim,
            softlimit,
            age_distribution: vec![0; agelim + 1],
            ages: Vec::new(),
            modification: Vec::new(),
            conflict_entries: Vec::new(),
            conflict_ranges: Vec::new(),
            free_spaces: BTreeSet::new(),
            deleted_conflicts: Vec::new(),
            log: ChangeLog::default(),
        }
    }

    pub fn conflict(&self, conflict: usize) -> Option<&[DomainChange]> {
        self.conflict_ranges[conflict].map(|(start, end)| &self.conflict_entries[start..end])
    }

    pub fn num_conflicts(&self) -> usize {
        self.conflict_ranges.len() - self.deleted_conflicts.len()
    }

    pub fn num_conflict_slots(&self) -> usize {
        self.conflict_ranges.len()
    }

    pub fn modification_count(&self, conflict: usize) -> u32 {
        self.modification[conflict]
    }

    pub fn set_age_limit(&mut self, agelim: usize) {
        self.agelim = agelim;
        self.age_distribution.resize(agelim + 1, 0);
    }

    pub fn reset_age(&mut self, conflict: usize) {
        if self.ages[conflict] > 0 {
            self.age_distribution[self.ages[conflict] as usize] -= 1;
            self.age_distribution[0] += 1;
            self.ages[conflict] = 0;
        }
    }

    /// Room for a conflict of `len` entries: its index and start.
    fn allocate(&mut self, len: usize) -> (usize, usize) {
        let found = self.free_spaces.range((len, 0)..).next().copied();
        let (start, end) = match found {
            None => {
                let start = self.conflict_entries.len();
                self.conflict_entries.resize(start + len, DomainChange::default());
                (start, start + len)
            }
            Some(slot) => {
                self.free_spaces.remove(&slot);
                let start = slot.1;
                let end = start + len;
                if slot.0 > len {
                    self.free_spaces.insert((slot.0 - len, end));
                }
                (start, end)
            }
        };
        let conflict = match self.deleted_conflicts.pop() {
            None => {
                self.conflict_ranges.push(Some((start, end)));
                self.ages.push(0);
                self.modification.push(0);
                self.conflict_ranges.len() - 1
            }
            Some(conflict) => {
                self.conflict_ranges[conflict] = Some((start, end));
                conflict
            }
        };
        self.modification[conflict] += 1;
        self.ages[conflict] = 0;
        self.age_distribution[0] += 1;
        (conflict, start)
    }

    fn added(&mut self, conflict: usize) {
        let (start, end) = self.conflict_ranges[conflict].expect("an added conflict");
        self.log.push(ConflictEvent::Added {
            conflict,
            entries: self.conflict_entries[start..end].to_vec(),
        });
    }

    /// HiGHS's `addConflictCut`: the frontier's bound changes, in position
    /// order, those of continuous columns tightened by `feastol`.
    pub fn add_conflict_cut(&mut self, frontier: impl ExactSizeIterator<Item = DomainChange>, is_continuous: impl Fn(usize) -> bool, feastol: f64) {
        let (conflict, start) = self.allocate(frontier.len());
        for (i, domchg) in frontier.enumerate() {
            let mut entry = domchg;
            if is_continuous(entry.column) {
                if entry.boundtype == BoundType::Lower {
                    entry.boundval += feastol;
                } else {
                    entry.boundval -= feastol;
                }
            }
            self.conflict_entries[start + i] = entry;
        }
        self.added(conflict);
    }

    /// HiGHS's `addReconvergenceCut`: `flipped` (the reconvergence bound
    /// change, flipped) then the frontier as in
    /// [`ConflictPool::add_conflict_cut`].
    pub fn add_reconvergence_cut(&mut self, frontier: impl ExactSizeIterator<Item = DomainChange>, flipped: DomainChange, is_continuous: impl Fn(usize) -> bool, feastol: f64) {
        let (conflict, start) = self.allocate(frontier.len() + 1);
        self.conflict_entries[start] = flipped;
        for (i, domchg) in frontier.enumerate() {
            let mut entry = domchg;
            if is_continuous(entry.column) {
                if entry.boundtype == BoundType::Lower {
                    entry.boundval += feastol;
                } else {
                    entry.boundval -= feastol;
                }
            }
            self.conflict_entries[start + 1 + i] = entry;
        }
        self.added(conflict);
    }

    pub fn remove_conflict(&mut self, conflict: usize) {
        self.log.push(ConflictEvent::Deleted { conflict });
        if self.ages[conflict] >= 0 {
            self.age_distribution[self.ages[conflict] as usize] -= 1;
            self.ages[conflict] = -1;
        }
        let (start, end) = self.conflict_ranges[conflict].expect("a live conflict");
        self.deleted_conflicts.push(conflict);
        self.free_spaces.insert((end - start, start));
        self.conflict_ranges[conflict] = None;
        self.modification[conflict] += 1;
    }

    pub fn perform_aging(&mut self) {
        let conflict_max_index = self.conflict_ranges.len();
        let mut agelim = self.agelim;
        let mut num_active_conflicts = self.num_conflicts() as isize;
        while agelim > 5 && num_active_conflicts > self.softlimit as isize {
            num_active_conflicts -= self.age_distribution[agelim] as isize;
            agelim -= 1;
        }
        for i in 0..conflict_max_index {
            if self.ages[i] < 0 {
                continue;
            }
            self.age_distribution[self.ages[i] as usize] -= 1;
            self.ages[i] += 1;
            if self.ages[i] as usize > agelim {
                self.ages[i] = -1;
                self.remove_conflict(i);
            } else {
                self.age_distribution[self.ages[i] as usize] += 1;
            }
        }
    }
}
