//! Reading a solved SCIP blended model back into owned, published rows.

use russcip::{Solved, Variable, prelude::*};

use super::super::blended::{
    formulation::BlendColumns,
    replay::{BlendSolution, ChunkRow, ExtractionAdjustments, MovementRow},
};

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
    let mut chunks = Vec::new();
    for (&(pile, chunk, interval), open) in &columns.chunk_open_t {
        if cancelled() {
            return Err(());
        }
        let reclaimed_t = columns.chunk_recl_t.get(&(pile, chunk, interval)).map(|variable| best.val(variable)).unwrap_or(0.0);
        let closed = columns.chunk_closed.get(&(pile, chunk, interval)).map(|variable| best.val(variable) > 0.5).unwrap_or(false);
        let mut receipts = Vec::new();
        for (&(p, c, k, movement), variable) in &columns.chunk_recv {
            if cancelled() {
                return Err(());
            }
            if p != pile || c != chunk || k != interval {
                continue;
            }
            let tonnes = best.val(variable);
            if tonnes.abs() <= 1e-9 {
                if tonnes != 0.0 {
                    adjustments.chunk_receipt(tonnes);
                }
            } else {
                receipts.push((movement, tonnes));
            }
        }
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
