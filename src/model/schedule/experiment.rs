//! Explicit configuration for the schedule optimiser.
//!
//! Everything here is authored, never inferred. The blended model in
//! [`crate::model::schedule::optimisation::blended`] needs things a project
//! does not otherwise state:
//!
//! - **How long to plan for, and at what resolution.** Run All Periods solves
//!   a bounded horizon and has to be told where it ends, because a bar may be
//!   open-ended and "the last bar's end" is then not a number.
//! - **What a stockpile *is*.** Authored FIFO/LIFO lots are an ordered
//!   inventory. The blended model is not, and the chunked variant is a third
//!   thing again. Converting one into another silently would change what a
//!   planner authored, so the representation is chosen per pile and defaults
//!   to [`StockpileRepresentation::NotConfigured`] - which blocks a run that
//!   uses that pile and blocks nothing else.
//! - **Which grade columns to track.** Numeric tonnes-weighted fields can be
//!   carried through blends, targets and actuals. Their stored numbers are
//!   used directly, whether a value is written as `0.62` or `62`.
//!
//! This is persisted with the plan and edited through ordinary undoable
//! commands in every build, including the browser, where schedule
//! calculation itself is unavailable. The persisted field keeps its original
//! name, `experiment`, so older files open unchanged.

use std::hash::Hash;

use serde::{Deserialize, Serialize};

use super::{DestinationId, ScheduleError, ScheduleResult};
use crate::{i18n::tr, model::ReserveFieldId};

/// Default planning horizon, in whole schedule periods (days).
pub(crate) const DEFAULT_END_DAY: u32 = 1;
/// Default calendar discretisation.
pub(crate) const DEFAULT_INTERVAL_H: f64 = 1.0;
/// Default solver wall-clock budget.
pub(crate) const DEFAULT_SOLVE_SECONDS: f64 = 60.0;
/// The relative MIP gap Stage 5A verified and this reuses rather than
/// restating: one default, in one place.
pub(crate) const DEFAULT_RELATIVE_GAP: f64 = 1e-4;

/// Legacy unit settings remain readable. New tracking uses Stored and capture
/// always blends in the field's stored scale, including for legacy selections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum GradeUnit {
    Fraction,
    Percent,
    Stored,
}

/// How one stockpile's inventory is represented to the optimiser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum StockpileRepresentation {
    /// No choice has been made. Every existing project starts here, and a run
    /// that needs this pile refuses with the pile named rather than picking
    /// one of the two answers below on the planner's behalf.
    #[default]
    NotConfigured,
    /// One blend. Opening lots are combined by tonnes and contained quantity,
    /// and FIFO/LIFO stops meaning anything - a pile with one composition has
    /// no oldest end to draw from.
    Blended,
    /// Ordered blended chunks. Each authored opening lot becomes a closed,
    /// immediately reclaimable chunk of its own actual composition, and the
    /// configured receiving chunks fill in order behind them.
    Chunks,
}

impl StockpileRepresentation {
    pub(crate) fn label(self) -> String {
        match self {
            Self::NotConfigured => tr!("experiment-representation-none"),
            Self::Blended => tr!("experiment-representation-blended"),
            Self::Chunks => tr!("experiment-representation-chunks"),
        }
    }
}

/// One stockpile's optimisation settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct StockpileExperiment {
    pub(crate) representation: StockpileRepresentation,
    /// Capacities of the *receiving* chunks, in fill order. The opening lots
    /// supply their own closed chunks ahead of these and are not listed here.
    ///
    /// Never derived from a truck payload or a tonnage: how many chunks a
    /// pile is divided into is a modelling decision with a throughput
    /// consequence - an emptied chunk slot is not reused within the horizon -
    /// and inventing them to make a solve succeed would hide that.
    pub(crate) receiving_chunks: Vec<f64>,
}

impl StockpileExperiment {
    fn is_pristine(&self) -> bool {
        self.representation == StockpileRepresentation::NotConfigured && self.receiving_chunks.is_empty()
    }
}

/// Everything the schedule optimiser is told that the rest of the plan does
/// not already say.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ExperimentConfig {
    /// The horizon, in whole schedule periods from project hour zero.
    pub(crate) planning_end_day: u32,
    /// The regular calendar step. This is the resolution at which input
    /// settings may change and at which stockpile receipts are released - it
    /// is *not* a limit of one dig block per interval, and a loader may move
    /// through several sources inside one interval.
    pub(crate) interval_h: f64,
    /// Explicit maximum execution positions per calendar interval. None keeps
    /// the derived budget and its existing model-size ceiling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) event_capacity: Option<usize>,
    /// Solver wall-clock budget. Preparation and replay sit outside it.
    pub(crate) solve_seconds: f64,
    pub(crate) relative_gap: f64,
    /// The tracked grade columns. Legacy unit tags are retained for file
    /// compatibility; capture now uses stored numbers directly. Ordered,
    /// because the model indexes grades by position.
    pub(crate) grades: Vec<(ReserveFieldId, GradeUnit)>,
    /// Per-stockpile representation, keyed by destination. Sorted by key so a
    /// file that round-trips hashes the same.
    pub(crate) stockpiles: Vec<(DestinationId, StockpileExperiment)>,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            planning_end_day: DEFAULT_END_DAY,
            interval_h: DEFAULT_INTERVAL_H,
            event_capacity: None,
            solve_seconds: DEFAULT_SOLVE_SECONDS,
            relative_gap: DEFAULT_RELATIVE_GAP,
            grades: Vec::new(),
            stockpiles: Vec::new(),
        }
    }
}

fn checked_positive(value: f64) -> ScheduleResult<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ScheduleError::InvalidExperimentSetting);
    }
    Ok(value)
}

impl ExperimentConfig {
    /// Whether this says nothing a default would not, so an untouched project
    /// carries no optimisation settings into its file.
    pub(crate) fn is_pristine(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn stockpile(&self, id: DestinationId) -> Option<&StockpileExperiment> {
        self.stockpiles.iter().find(|(key, _)| *key == id).map(|(_, entry)| entry)
    }

    pub(crate) fn representation(&self, id: DestinationId) -> StockpileRepresentation {
        self.stockpile(id).map_or(StockpileRepresentation::NotConfigured, |entry| entry.representation)
    }

    pub(crate) fn grade_unit(&self, field: ReserveFieldId) -> Option<GradeUnit> {
        self.grades.iter().find(|(id, _)| *id == field).map(|(_, unit)| *unit)
    }

    pub(crate) fn set_planning_end_day(&mut self, day: u32) -> ScheduleResult {
        if day == 0 {
            return Err(ScheduleError::InvalidExperimentSetting);
        }
        // The horizon is `day * SCHEDULE_PERIOD_H` and must stay a number the
        // interval builder can split, so the overflow is refused here rather
        // than surfacing as an empty calendar later.
        if !(f64::from(day) * super::SCHEDULE_PERIOD_H).is_finite() {
            return Err(ScheduleError::InvalidExperimentSetting);
        }
        self.planning_end_day = day;
        Ok(())
    }

    pub(crate) fn set_interval_h(&mut self, hours: f64) -> ScheduleResult {
        self.interval_h = checked_positive(hours)?;
        Ok(())
    }

    pub(crate) fn set_event_capacity(&mut self, capacity: Option<usize>) -> ScheduleResult {
        if capacity.is_some_and(|value| !(1..=super::optimisation::SEGMENT_CEILING).contains(&value)) {
            return Err(ScheduleError::InvalidExperimentSetting);
        }
        self.event_capacity = capacity;
        Ok(())
    }

    pub(crate) fn set_solve_seconds(&mut self, seconds: f64) -> ScheduleResult {
        self.solve_seconds = checked_positive(seconds)?;
        Ok(())
    }

    pub(crate) fn set_relative_gap(&mut self, gap: f64) -> ScheduleResult {
        if !gap.is_finite() || !(0.0..=1.0).contains(&gap) {
            return Err(ScheduleError::InvalidExperimentSetting);
        }
        self.relative_gap = gap;
        Ok(())
    }

    /// Map one field to a unit, or clear the mapping. Ordered by field id so
    /// the grade positions a capture produces do not depend on edit order.
    pub(crate) fn set_grade_unit(&mut self, field: ReserveFieldId, unit: Option<GradeUnit>) -> ScheduleResult {
        self.grades.retain(|(id, _)| *id != field);
        if let Some(unit) = unit {
            self.grades.push((field, unit));
            self.grades.sort_by_key(|(id, _)| *id);
        }
        Ok(())
    }

    pub(crate) fn set_representation(&mut self, id: DestinationId, representation: StockpileRepresentation) -> ScheduleResult {
        self.entry(id).representation = representation;
        self.prune();
        Ok(())
    }

    pub(crate) fn set_receiving_chunks(&mut self, id: DestinationId, capacities: Vec<f64>) -> ScheduleResult {
        if capacities.iter().any(|capacity| !capacity.is_finite() || *capacity <= 0.0) {
            return Err(ScheduleError::InvalidExperimentSetting);
        }
        self.entry(id).receiving_chunks = capacities;
        self.prune();
        Ok(())
    }

    fn entry(&mut self, id: DestinationId) -> &mut StockpileExperiment {
        if !self.stockpiles.iter().any(|(key, _)| *key == id) {
            self.stockpiles.push((id, StockpileExperiment::default()));
            self.stockpiles.sort_by_key(|(key, _)| *key);
        }
        self.stockpiles.iter_mut().find(|(key, _)| *key == id).map(|(_, entry)| entry).expect("just inserted")
    }

    /// Drop entries that have fallen back to saying nothing, so a pile
    /// configured and then unconfigured leaves no trace in the file.
    fn prune(&mut self) {
        self.stockpiles.retain(|(_, entry)| !entry.is_pristine());
    }

    /// Every persisted setting, for the plan's own content hash. Ordered and
    /// id-based; no name reaches this.
    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.planning_end_day.hash(hasher);
        self.interval_h.to_bits().hash(hasher);
        self.event_capacity.hash(hasher);
        self.solve_seconds.to_bits().hash(hasher);
        self.relative_gap.to_bits().hash(hasher);
        for (field, unit) in &self.grades {
            field.0.hash(hasher);
            unit.hash(hasher);
        }
        for (id, entry) in &self.stockpiles {
            id.hash(hasher);
            entry.representation.hash(hasher);
            for capacity in &entry.receiving_chunks {
                capacity.to_bits().hash(hasher);
            }
        }
    }

    /// Everything that can change what a calculated schedule *means*:
    /// resolution, grade units and stockpile representation.
    ///
    /// Deliberately without the solve limits, which change how hard the
    /// optimiser looks rather than the model it looks at, and without the
    /// planning end day, which bounds a run's horizon rather than entering
    /// the model.
    pub(crate) fn hash_semantics<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.interval_h.to_bits().hash(hasher);
        self.event_capacity.hash(hasher);
        for (field, unit) in &self.grades {
            field.0.hash(hasher);
            unit.hash(hasher);
        }
        for (id, entry) in &self.stockpiles {
            id.hash(hasher);
            entry.representation.hash(hasher);
            for capacity in &entry.receiving_chunks {
                capacity.to_bits().hash(hasher);
            }
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + size_of_val(self.grades.as_slice())
            + self
                .stockpiles
                .iter()
                .map(|(_, entry)| size_of::<(DestinationId, StockpileExperiment)>() + size_of_val(entry.receiving_chunks.as_slice()))
                .sum::<usize>()
    }
}
