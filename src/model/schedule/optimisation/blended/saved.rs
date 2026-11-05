//! A calculated schedule as a project file keeps it.
//!
//! What is kept is the decisions: the published rows and the blast timeline
//! they were worked against. Everything else about a schedule - tonnes on each
//! truck, stockpile tracks, its value - is worked out from those by the replay
//! and publication every other schedule goes through, so a saved schedule is
//! checked again on the way back rather than trusted.
//!
//! It is tied to the model it answered by [`SavedSchedule::fingerprint`]. Rows
//! name candidates by position, so against any other model they would mean
//! something else: one that does not match is left unused and the project
//! calculates as it would have.

use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};

use super::{
    drill_blast::DrillBlastTimeline,
    replay::{BlendSolution, ChunkRow, ExtractionAdjustments, MovementRow},
};
use crate::model::schedule::optimisation::StockpileId;

/// The file format of the saved rows, read back only when it matches.
const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SavedSchedule {
    /// The model's stable fingerprint: the same inputs give the same one in
    /// another session, unlike the run's own identity.
    pub(crate) fingerprint: u64,
    pub(crate) requested_end_h: f64,
    pub(crate) solution: Arc<BlendSolution>,
}

/// The rows column by column: a schedule holds hundreds of thousands, and
/// repeating four keys on each would be most of its size.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    fingerprint: u64,
    requested_end_h: f64,
    candidates: Vec<u32>,
    intervals: Vec<u32>,
    segments: Vec<u32>,
    tonnes: Vec<f64>,
    durations: Vec<(usize, usize, f64)>,
    chunks: Vec<StoredChunk>,
    drill_blast: Option<DrillBlastTimeline>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredChunk {
    pile: u32,
    chunk: usize,
    interval: usize,
    open_t: f64,
    reclaimed_t: f64,
    receiving: bool,
    received_t: f64,
    receipts: Vec<(usize, f64)>,
}

impl SavedSchedule {
    pub(crate) fn to_value(&self) -> serde_json::Result<serde_json::Value> {
        let solution = &self.solution;
        serde_json::to_value(Stored {
            version: VERSION,
            fingerprint: self.fingerprint,
            requested_end_h: self.requested_end_h,
            candidates: solution.movements.iter().map(|row| row.candidate as u32).collect(),
            intervals: solution.movements.iter().map(|row| row.interval as u32).collect(),
            segments: solution.movements.iter().map(|row| row.segment as u32).collect(),
            tonnes: solution.movements.iter().map(|row| row.tonnes_t).collect(),
            durations: solution.durations.iter().map(|(&(interval, segment), &hours)| (interval, segment, hours)).collect(),
            chunks: solution
                .chunks
                .iter()
                .map(|row| StoredChunk {
                    pile: row.pile.0,
                    chunk: row.chunk,
                    interval: row.interval,
                    open_t: row.open_t,
                    reclaimed_t: row.reclaimed_t,
                    receiving: row.receiving,
                    received_t: row.received_t,
                    receipts: row.receipts.clone(),
                })
                .collect(),
            drill_blast: solution.drill_blast.clone(),
        })
    }

    /// `None` for rows another version wrote or that do not line up: a saved
    /// schedule is only ever a shortcut, so losing one costs a recalculation.
    pub(crate) fn from_value(value: serde_json::Value) -> Option<Self> {
        let stored: Stored = serde_json::from_value(value).ok()?;
        let rows = stored.candidates.len();
        if stored.version != VERSION || stored.intervals.len() != rows || stored.segments.len() != rows || stored.tonnes.len() != rows {
            return None;
        }
        let movements = (0..rows)
            .map(|row| MovementRow {
                candidate: stored.candidates[row] as usize,
                interval: stored.intervals[row] as usize,
                segment: stored.segments[row] as usize,
                tonnes_t: stored.tonnes[row],
            })
            .collect();
        let chunks = stored
            .chunks
            .into_iter()
            .map(|row| ChunkRow {
                pile: StockpileId(row.pile),
                chunk: row.chunk,
                interval: row.interval,
                open_t: row.open_t,
                reclaimed_t: row.reclaimed_t,
                receiving: row.receiving,
                received_t: row.received_t,
                receipts: row.receipts,
            })
            .collect();
        Some(Self {
            fingerprint: stored.fingerprint,
            requested_end_h: stored.requested_end_h,
            solution: Arc::new(BlendSolution {
                movements,
                durations: stored
                    .durations
                    .into_iter()
                    .map(|(interval, segment, hours)| ((interval, segment), hours))
                    .collect::<BTreeMap<_, _>>(),
                chunks,
                reported_objective: 0.0,
                adjustments: ExtractionAdjustments::default(),
                drill_blast: stored.drill_blast,
            }),
        })
    }
}
