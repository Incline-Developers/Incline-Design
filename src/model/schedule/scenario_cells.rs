//! A scenario's own calendar: the Calendar cells it sets differently from
//! the mine's - a loader down for a month, another truck from year two, a
//! crusher limit or a grade band of its own.
//!
//! The mine's calendars are shared by every scenario, as Setup describes
//! them. A scenario holds only the cells it changes ([`Scenario::cells`]),
//! each one as the edit that sets it, and a scenario that starts from
//! another sets that one's cells too, under its own. While a scenario is
//! open its cells are laid over the mine's, so every page and calculation
//! reads them as the plan's own calendar, and what they cover is kept
//! ([`super::scenarios::Scenarios::beneath`]) to be put back when it closes.
//! An edit of the mine runs with them lifted ([`SchedulePlan::on_mine`]), so
//! it changes the mine and not the scenario.
//!
//! [`Scenario::cells`]: super::scenarios::Scenario::cells

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{
    CalendarCell, CalendarCellEdit, CalendarField, DestinationId, DestinationKind, LoaderAgentId, SchedulePlan, ScheduleResult, StandaloneDestinationId, TruckCellEdit,
    TruckClassId, TruckField,
    destinations::{CrusherCell, CrusherCellEdit},
    grade_targets::{GradeTargetCellEdit, GradeTargetInput, GradeTargetValue},
    scenarios::ScenarioId,
    stockpile_operation::PileModeCellEdit,
};
use crate::model::ReserveFieldId;

/// One cell a scenario sets: the edit that sets it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CellOverride {
    Loader(CalendarCellEdit),
    Truck(TruckCellEdit),
    Crusher(CrusherCellEdit),
    Grade(GradeTargetCellEdit),
    PileMode(PileModeCellEdit),
}

/// The Calendar row a cell is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CellRow {
    Loader(LoaderAgentId, CalendarField),
    Truck(TruckClassId, TruckField),
    Crusher(StandaloneDestinationId),
    Grade(DestinationId, ReserveFieldId, GradeTargetInput),
    PileMode(DestinationId),
}

/// Which cell: its row and its day, or its row's Default.
pub(crate) type CellAddress = (CellRow, CalendarCell);

/// Whose a cell the open scenario sets is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellOwner {
    /// The open scenario's own.
    Open,
    /// A scenario the open one starts from.
    Base(ScenarioId),
}

impl CellOverride {
    pub(crate) fn row(&self) -> CellRow {
        match self {
            Self::Loader(edit) => CellRow::Loader(edit.agent, edit.field),
            Self::Truck(edit) => CellRow::Truck(edit.class, edit.field),
            Self::Crusher(edit) => CellRow::Crusher(edit.destination),
            Self::Grade(edit) => CellRow::Grade(edit.destination, edit.field, edit.input),
            Self::PileMode(edit) => CellRow::PileMode(edit.destination),
        }
    }

    pub(crate) fn cell(&self) -> CalendarCell {
        match self {
            Self::Loader(edit) => edit.cell,
            Self::Truck(edit) => edit.cell,
            Self::Crusher(edit) => match edit.cell {
                CrusherCell::Default => CalendarCell::Default,
                CrusherCell::Period(period) => CalendarCell::Period(period),
            },
            Self::Grade(edit) => edit.cell,
            Self::PileMode(edit) => edit.cell,
        }
    }

    pub(crate) fn address(&self) -> CellAddress {
        (self.row(), self.cell())
    }

    /// The edit that puts back what `plan` holds in this cell now, or `None`
    /// when the plan has no such cell to set: its machine, truck class or
    /// crusher is gone, or the cell is one nothing may type into.
    fn current(&self, plan: &SchedulePlan) -> Option<Self> {
        Some(match *self {
            Self::Loader(edit) => {
                let calendar = &plan.agents.iter().find(|agent| agent.id == edit.agent)?.calendar;
                let value = match edit.cell {
                    CalendarCell::Default => Some(match edit.field {
                        CalendarField::Rate | CalendarField::ReclaimRate => return None,
                        CalendarField::Availability => calendar.default_availability,
                        CalendarField::Utilisation => calendar.default_utilisation,
                        CalendarField::UtilisationTarget => calendar.default_utilisation_target,
                        CalendarField::UtilisationIncentive => calendar.default_utilisation_incentive,
                    }),
                    CalendarCell::Period(period) => calendar.periods.get(&period).and_then(|held| match edit.field {
                        CalendarField::Availability => held.availability,
                        CalendarField::Utilisation => held.utilisation,
                        CalendarField::Rate => held.rate_tph,
                        CalendarField::ReclaimRate => held.reclaim_rate_tph,
                        CalendarField::UtilisationTarget => held.utilisation_target,
                        CalendarField::UtilisationIncentive => held.utilisation_incentive,
                    }),
                };
                Self::Loader(CalendarCellEdit { value, ..edit })
            }
            Self::Truck(edit) => {
                let calendar = &plan.trucks.classes.iter().find(|class| class.id == edit.class)?.calendar;
                let value = match edit.cell {
                    CalendarCell::Default => Some(match edit.field {
                        TruckField::Units => f64::from(calendar.default_units),
                        TruckField::Availability => calendar.default_availability,
                        TruckField::Utilisation => calendar.default_utilisation,
                    }),
                    CalendarCell::Period(period) => calendar.periods.get(&period).and_then(|held| match edit.field {
                        TruckField::Units => held.units.map(f64::from),
                        TruckField::Availability => held.availability,
                        TruckField::Utilisation => held.utilisation,
                    }),
                };
                Self::Truck(TruckCellEdit { value, ..edit })
            }
            Self::Crusher(edit) => {
                let crusher = plan.routing.standalone(edit.destination).filter(|entry| entry.kind == DestinationKind::Crusher)?;
                let value = match edit.cell {
                    CrusherCell::Default => crusher.crusher.default_tpd.map(super::destinations::CrusherOverride::Limit),
                    CrusherCell::Period(period) => crusher.crusher.periods.get(&period).copied(),
                };
                Self::Crusher(CrusherCellEdit { value, ..edit })
            }
            Self::Grade(edit) => {
                let DestinationId::Standalone(id) = edit.destination else { return None };
                plan.routing.standalone(id).filter(|entry| entry.kind == DestinationKind::Crusher)?;
                let calendar = plan
                    .crusher_grade_calendars
                    .iter()
                    .find(|calendar| calendar.destination == edit.destination && calendar.field == edit.field);
                let value = calendar.and_then(|calendar| match edit.cell {
                    CalendarCell::Default => calendar.defaults.get(edit.input).map(GradeTargetValue::Number),
                    CalendarCell::Period(period) => calendar.periods.get(&period).and_then(|held| held.get(edit.input)),
                });
                Self::Grade(GradeTargetCellEdit { value, ..edit })
            }
            Self::PileMode(edit) => {
                let operation = plan.stockpile_operation(edit.destination);
                let mode = match edit.cell {
                    CalendarCell::Default => Some(operation.default_mode),
                    CalendarCell::Period(period) => operation.periods.get(&period).copied(),
                };
                Self::PileMode(PileModeCellEdit { mode, ..edit })
            }
        })
    }
}

/// Set `cells` in `plan` as one batch of each kind, refused as a whole as
/// the Calendar's own edits are.
pub(crate) fn apply(plan: &mut SchedulePlan, cells: &[CellOverride]) -> ScheduleResult {
    let mut loader = Vec::new();
    let mut truck = Vec::new();
    let mut crusher = Vec::new();
    let mut grade = Vec::new();
    let mut mode = Vec::new();
    for cell in cells {
        match *cell {
            CellOverride::Loader(edit) => loader.push(edit),
            CellOverride::Truck(edit) => truck.push(edit),
            CellOverride::Crusher(edit) => crusher.push(edit),
            CellOverride::Grade(edit) => grade.push(edit),
            CellOverride::PileMode(edit) => mode.push(edit),
        }
    }
    if !loader.is_empty() {
        plan.set_calendar_cells(&loader)?;
    }
    if !truck.is_empty() {
        plan.trucks.set_truck_cells(&truck)?;
    }
    if !crusher.is_empty() {
        plan.routing.set_crusher_cells(&crusher)?;
    }
    if !grade.is_empty() {
        plan.set_grade_target_cells(&grade)?;
    }
    if !mode.is_empty() {
        plan.set_pile_mode_cells(&mode)?;
    }
    Ok(())
}

/// Set as many of `cells` as can be: as one batch, or each one alone when
/// that is refused (a machine gone, a band the mine has since changed under
/// it), leaving out those refused.
fn apply_what_fits(plan: &mut SchedulePlan, cells: &[CellOverride]) {
    if cells.is_empty() {
        return;
    }
    let mut trial = plan.clone();
    if apply(&mut trial, cells).is_ok() {
        *plan = trial;
        return;
    }
    for cell in cells {
        let _ = apply(plan, std::slice::from_ref(cell));
    }
}

/// `cells` with each address once, the last of each kept, in the order each
/// address first came.
fn deduplicated(cells: impl IntoIterator<Item = CellOverride>) -> Vec<CellOverride> {
    let mut at: HashMap<CellAddress, usize> = HashMap::new();
    let mut out: Vec<CellOverride> = Vec::new();
    for cell in cells {
        match at.get(&cell.address()) {
            Some(index) => out[*index] = cell,
            None => {
                at.insert(cell.address(), out.len());
                out.push(cell);
            }
        }
    }
    out
}

impl SchedulePlan {
    /// The scenarios whose cells the open one sets, farthest base first and
    /// the open one last.
    fn cell_layers(&self) -> Vec<ScenarioId> {
        let open = self.scenarios.open;
        let mut layers = self.scenarios.bases(open);
        layers.reverse();
        layers.push(open);
        layers
    }

    /// The cells the open scenario sets, each once: its own over those of
    /// the scenarios it starts from, the nearest of those over the rest.
    pub(crate) fn scenario_cells(&self) -> HashMap<CellAddress, CellOwner> {
        let open = self.scenarios.open;
        let mut out = HashMap::new();
        for id in self.cell_layers() {
            let Some(scenario) = self.scenarios.get(id) else { continue };
            for cell in &scenario.cells {
                out.insert(cell.address(), if id == open { CellOwner::Open } else { CellOwner::Base(id) });
            }
        }
        out
    }

    /// Put the mine's cells back where the open scenario's lie.
    fn lift_cells(&mut self) {
        let beneath = std::mem::take(&mut self.scenarios.beneath);
        apply_what_fits(self, &beneath);
    }

    /// Lay the open scenario's cells over the mine's, keeping what they cover.
    fn lay_cells(&mut self) {
        let layers = self.cell_layers();
        let cells = deduplicated(layers.iter().filter_map(|id| self.scenarios.get(*id)).flat_map(|scenario| scenario.cells.iter().copied()));
        let mut laid = Vec::with_capacity(cells.len());
        let mut beneath = Vec::with_capacity(cells.len());
        for cell in cells {
            if let Some(current) = cell.current(self) {
                beneath.push(current);
                laid.push(cell);
            }
        }
        apply_what_fits(self, &laid);
        self.scenarios.beneath = beneath;
    }

    /// Run `edit` on the mine: the open scenario's cells lifted while it
    /// runs and laid again after, over whatever it changed. Every edit of
    /// the plan goes through here, so the mine's calendars are only ever
    /// changed as the mine's.
    pub(crate) fn on_mine<T>(&mut self, edit: impl FnOnce(&mut Self) -> ScheduleResult<T>) -> ScheduleResult<T> {
        if self.scenarios.is_empty() {
            return edit(self);
        }
        self.lift_cells();
        let result = edit(self);
        self.lay_cells();
        result
    }

    /// Set Calendar cells for every scenario: the mine's, which the open
    /// scenario then reads too, no longer setting those cells itself.
    /// Run inside [`Self::on_mine`].
    pub(crate) fn set_mine_cells(&mut self, cells: &[CellOverride]) -> ScheduleResult {
        apply(self, cells)?;
        let addresses: std::collections::HashSet<CellAddress> = cells.iter().map(CellOverride::address).collect();
        let open = self.scenarios.open;
        if let Some(scenario) = self.scenarios.get_mut(open) {
            scenario.cells.retain(|cell| !addresses.contains(&cell.address()));
        }
        Ok(())
    }

    /// Set Calendar cells in the open scenario alone. Run inside
    /// [`Self::on_mine`], which lays them.
    pub(crate) fn set_scenario_cells(&mut self, cells: &[CellOverride]) -> ScheduleResult {
        // Checked as the Calendar's own edits are, over the calendar the
        // open scenario reads.
        // A project of one scenario has no mine apart from it.
        if self.scenarios.is_empty() {
            return apply(self, cells);
        }
        let mut probe = self.clone();
        probe.lay_cells();
        apply(&mut probe, cells)?;
        let open = self.scenarios.open;
        let scenario = self.scenarios.get_mut(open).ok_or(super::ScheduleError::UnknownScenario)?;
        scenario.cells = deduplicated(scenario.cells.iter().copied().chain(cells.iter().copied()));
        Ok(())
    }

    /// Stop the open scenario setting `addresses`: it reads them from the
    /// mine, or from the scenario it starts from, again.
    pub(crate) fn reset_scenario_cells(&mut self, addresses: &[CellAddress]) -> ScheduleResult {
        let addresses: std::collections::HashSet<CellAddress> = addresses.iter().copied().collect();
        let open = self.scenarios.open;
        if let Some(scenario) = self.scenarios.get_mut(open) {
            scenario.cells.retain(|cell| !addresses.contains(&cell.address()));
        }
        Ok(())
    }
}
