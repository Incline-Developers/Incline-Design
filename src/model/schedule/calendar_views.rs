//! Saved Calendar views: which of the Calendar's rows a planner wants to see,
//! and in what order.
//!
//! The Calendar's own view shows everything, in the order the rows are
//! designed in. A saved view is a name and the differences from that: the
//! headings, machines, trucks and destinations it leaves out, the kinds of
//! row it leaves out, and the order it puts each set of rows in. Rows a view
//! does not mention - a machine added after it was saved, say - are shown, so
//! a view never hides something new.
//!
//! Views are kept with the project but are display only: no calculation reads
//! them, so switching or editing one never makes a schedule out of date. The
//! machines' order is the fleet's own (see [`super::SchedulePlan::ordered_agents`]),
//! shared with the Machines page and the Gantt.

use serde::{Deserialize, Serialize};

use super::{DestinationId, DestinationKind, LoaderAgentId, MachineKind, TruckClassId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct CalendarViewId(pub(crate) u64);

/// One kind of Calendar row, as a view names it. A crusher's grade rows are
/// one entry: which grades it shows follows the grades the schedule tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RowKind {
    Availability,
    Utilisation,
    Rate,
    ReclaimRate,
    UtilisationTarget,
    UtilisationIncentive,
    DigTonnes,
    ReclaimTonnes,
    ScheduledUtilisation,
    BlastWork,
    TruckUnits,
    TruckAvailability,
    TruckUtilisation,
    TruckHours,
    TruckCycle,
    TruckTonneKm,
    CrusherLimit,
    PileMode,
    Received,
    Reclaimed,
    Cumulative,
    Grades,
}

/// Rows that share one order and one set of switches: every machine of a
/// kind shows the same rows, as do the trucks, and each kind of destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RowSet {
    Machines(MachineKind),
    Trucks,
    Destinations(DestinationKind),
}

/// One heading of the Calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Section {
    /// The schedule's movement value.
    Value,
    Machines(MachineKind),
    Trucks,
    Destinations,
}

/// One machine, truck class or destination, with the rows beneath it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Owner {
    Machine(LoaderAgentId),
    Truck(TruckClassId),
    Destination(DestinationId),
}

/// What one view shows. The default is the Calendar's own: everything.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct CalendarView {
    pub(crate) id: CalendarViewId,
    pub(crate) name: String,
    #[serde(default)]
    hidden_sections: Vec<Section>,
    #[serde(default)]
    hidden_owners: Vec<Owner>,
    #[serde(default)]
    hidden_rows: Vec<(RowSet, RowKind)>,
    #[serde(default)]
    row_order: Vec<(RowSet, Vec<RowKind>)>,
}

impl CalendarView {
    /// Whether it shows what the Calendar's own view does.
    pub(crate) fn shows_everything(&self) -> bool {
        self.hidden_sections.is_empty() && self.hidden_owners.is_empty() && self.hidden_rows.is_empty() && self.row_order.is_empty()
    }

    pub(crate) fn shows_section(&self, section: Section) -> bool {
        !self.hidden_sections.contains(&section)
    }

    pub(crate) fn shows_owner(&self, owner: Owner) -> bool {
        !self.hidden_owners.contains(&owner)
    }

    pub(crate) fn shows_row(&self, set: RowSet, kind: RowKind) -> bool {
        !self.hidden_rows.contains(&(set, kind))
    }

    pub(crate) fn set_section_shown(&mut self, section: Section, shown: bool) {
        toggle(&mut self.hidden_sections, section, shown);
    }

    pub(crate) fn set_owner_shown(&mut self, owner: Owner, shown: bool) {
        toggle(&mut self.hidden_owners, owner, shown);
    }

    pub(crate) fn set_row_shown(&mut self, set: RowSet, kind: RowKind, shown: bool) {
        toggle(&mut self.hidden_rows, (set, kind), shown);
    }

    /// `set`'s rows in this view's order, shown or not. `designed` is every
    /// row the set has, in the Calendar's own order; one the view has not
    /// placed keeps its place after the row it follows there.
    pub(crate) fn ordered(&self, set: RowSet, designed: &[RowKind]) -> Vec<RowKind> {
        let Some((_, order)) = self.row_order.iter().find(|(entry, _)| *entry == set) else {
            return designed.to_vec();
        };
        let mut rows: Vec<RowKind> = order.iter().copied().filter(|kind| designed.contains(kind)).collect();
        for (index, kind) in designed.iter().enumerate() {
            if rows.contains(kind) {
                continue;
            }
            let after = designed[..index].iter().rev().find_map(|earlier| rows.iter().position(|row| row == earlier));
            rows.insert(after.map_or(0, |at| at + 1), *kind);
        }
        rows
    }

    /// Move one of `set`'s rows to just before `before`, or to the end.
    pub(crate) fn move_row(&mut self, set: RowSet, kind: RowKind, before: Option<RowKind>, designed: &[RowKind]) {
        let mut rows = self.ordered(set, designed);
        rows.retain(|row| *row != kind);
        let at = before.and_then(|before| rows.iter().position(|row| *row == before)).unwrap_or(rows.len());
        rows.insert(at, kind);
        self.row_order.retain(|(entry, _)| *entry != set);
        if rows != designed {
            self.row_order.push((set, rows));
        }
    }

    /// The same view, with what it shows taken from `other`.
    pub(crate) fn with_layout_of(&self, other: &Self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            ..other.clone()
        }
    }
}

fn toggle<T: PartialEq>(hidden: &mut Vec<T>, item: T, shown: bool) {
    hidden.retain(|entry| *entry != item);
    if !shown {
        hidden.push(item);
    }
}

/// The project's saved views, in the order they were saved.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct CalendarViews {
    #[serde(default)]
    views: Vec<CalendarView>,
    #[serde(default)]
    next_id: u64,
}

impl CalendarViews {
    pub(crate) fn views(&self) -> &[CalendarView] {
        &self.views
    }

    pub(crate) fn view(&self, id: CalendarViewId) -> Option<&CalendarView> {
        self.views.iter().find(|view| view.id == id)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.views.is_empty() && self.next_id == 0
    }

    /// Save `view` as a new view under its name. Its id.
    pub(crate) fn add(&mut self, mut view: CalendarView) -> CalendarViewId {
        let id = CalendarViewId(self.next_id);
        self.next_id += 1;
        view.id = id;
        self.views.push(view);
        id
    }

    /// Replace the saved view with `view`'s id. Whether there was one.
    pub(crate) fn replace(&mut self, view: CalendarView) -> bool {
        match self.views.iter_mut().find(|entry| entry.id == view.id) {
            Some(entry) => {
                *entry = view;
                true
            }
            None => false,
        }
    }

    pub(crate) fn remove(&mut self, id: CalendarViewId) -> bool {
        let before = self.views.len();
        self.views.retain(|view| view.id != id);
        self.views.len() != before
    }
}
