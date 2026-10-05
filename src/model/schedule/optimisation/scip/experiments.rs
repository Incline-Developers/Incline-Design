//! Reading a solved SCIP blended model back into owned, published rows.

use std::collections::BTreeMap;

use russcip::{Solved, Variable, prelude::*};

use super::super::blended::{
    formulation::BlendColumns,
    replay::{BlendSolution, ChunkRow, ExtractionAdjustments, MovementRow},
};
use crate::model::schedule::optimisation::{MaterialId, StockpileId};

/// Extract published rows in bounded loops. The worker checks cancellation
/// here.
pub(crate) fn extract_solution(solved: &Model<Solved>, columns: &BlendColumns<Variable>, cancelled: impl Fn() -> bool) -> Result<Option<BlendSolution>, ()> {
    let Some(best) = solved.best_sol() else { return Ok(None) };
    if cancelled() {
        return Err(());
    }
    let mut adjustments = ExtractionAdjustments::default();
    let mut movements = Vec::new();
    for (&(candidate, interval, segment), variable) in &columns.movement {
        if cancelled() {
            return Err(());
        }
        let tonnes_t = best.val(variable);
        if tonnes_t.abs() <= 1e-9 {
            if tonnes_t != 0.0 {
                adjustments.movement(tonnes_t);
            }
        } else {
            movements.push(MovementRow {
                candidate,
                interval,
                segment,
                tonnes_t,
            });
        }
    }
    let mut durations = std::collections::BTreeMap::new();
    for (&cell, variable) in &columns.duration {
        if cancelled() {
            return Err(());
        }
        durations.insert(cell, best.val(variable));
    }
    // A chunk's receipts are solved per material. Each material's published
    // movement tonnes in an interval are handed to its chunks in chunk order,
    // so every chunk takes the material it was solved to take and every
    // movement delivers what it was published to deliver.
    let mut moved: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for row in &movements {
        *moved.entry((row.candidate, row.interval)).or_default() += row.tonnes_t;
    }
    let mut split: BTreeMap<(StockpileId, usize, usize), Vec<(usize, f64)>> = BTreeMap::new();
    let mut by_material: BTreeMap<(StockpileId, usize, MaterialId), Vec<(usize, f64)>> = BTreeMap::new();
    for (&(pile, chunk, interval, material), variable) in &columns.chunk_recv {
        if cancelled() {
            return Err(());
        }
        let tonnes = best.val(variable);
        if tonnes.abs() <= 1e-9 {
            if tonnes != 0.0 {
                adjustments.chunk_receipt(tonnes);
            }
        } else {
            by_material.entry((pile, interval, material)).or_default().push((chunk, tonnes));
        }
    }
    for ((pile, interval, material), chunks) in by_material {
        let candidates = columns.chunk_recv_candidates.get(&(pile, material)).map(Vec::as_slice).unwrap_or_default();
        let mut left: Vec<(usize, f64)> = candidates
            .iter()
            .map(|&candidate| (candidate, moved.get(&(candidate, interval)).copied().unwrap_or(0.0)))
            .collect();
        let mut at = 0;
        for (chunk, mut tonnes) in chunks {
            let receipts = split.entry((pile, chunk, interval)).or_default();
            while tonnes > 1e-9 && at < left.len() {
                let taken = tonnes.min(left[at].1);
                if taken > 1e-9 {
                    receipts.push((left[at].0, taken));
                    tonnes -= taken;
                    left[at].1 -= taken;
                }
                if left[at].1 <= 1e-9 {
                    at += 1;
                }
            }
            // A chunk solved to take a hair more than the movements carry:
            // the last movement it took from carries it, so the chunk's own
            // tonnes are published as solved.
            if tonnes > 1e-9 {
                match receipts.last_mut() {
                    Some((_, taken)) => *taken += tonnes,
                    None => receipts.push((candidates.last().copied().unwrap_or_default(), tonnes)),
                }
            }
        }
    }
    let mut chunks = Vec::new();
    for (&(pile, chunk, interval), open) in &columns.chunk_open_t {
        if cancelled() {
            return Err(());
        }
        let reclaimed_t = columns.chunk_recl_t.get(&(pile, chunk, interval)).map(|variable| best.val(variable)).unwrap_or(0.0);
        let closed = columns.chunk_closed.get(&(pile, chunk, interval)).map(|variable| best.val(variable) > 0.5).unwrap_or(false);
        let receipts = split.remove(&(pile, chunk, interval)).unwrap_or_default();
        let received_t = receipts.iter().map(|(_, tonnes)| tonnes).sum();
        chunks.push(ChunkRow {
            pile,
            chunk,
            interval,
            open_t: best.val(open),
            reclaimed_t,
            closed,
            received_t,
            receipts,
        });
    }
    Ok(Some(BlendSolution {
        movements,
        durations,
        chunks,
        reported_objective: solved.obj_val(),
        adjustments,
        drill_blast: None,
    }))
}
