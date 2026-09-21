//! The three labelled experiments, kept apart on purpose.
//!
//! | experiment | question it answers |
//! |---|---|
//! | [`parcel_via_mps`] | Backend comparison. SCIP is handed the *existing* parcel MILP through MPS, so the mathematics is the same and any difference is the solver. |
//! | [`solve_blend`] | The blended stockpile model. Its stockpile semantics differ from the parcel model's, so its objective is **not** comparable with a parcel objective. |
//! | chunked blend | The cost of reintroducing order. Built on [`solve_blend`] with a small number of ordered chunks. |
//!
//! Anything that reports a number from more than one of these must say which
//! it came from.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use russcip::{Solved, Variable, prelude::*};

use super::{
    super::blended::{
        formulation::{BlendColumns, BlendSizes},
        input::BlendInput,
        replay::{BlendSolution, ChunkRow, ExtractionAdjustments, MovementRow, ReplayReport, replay},
    },
    adapter::{SolveReport, SolveTuning},
    blend::formulate_scip,
};

/// One finished blended run.
pub(crate) struct BlendRun {
    pub(crate) report: SolveReport,
    pub(crate) sizes: BlendSizes,
    pub(crate) formulation_time: Duration,
    pub(crate) solution: Option<BlendSolution>,
    pub(crate) replay: Option<ReplayReport>,
}

/// Formulate, solve and independently replay a blended scenario.
///
/// The replay is not optional and not a debug aid: a run whose replay reports
/// issues has *not* produced a usable schedule, whatever SCIP's status says.
pub(crate) fn solve_blend(input: &BlendInput, tuning: SolveTuning) -> BlendRun {
    let started = Instant::now();
    let formulation = formulate_scip(input);
    let formulation_time = started.elapsed();
    let sizes = formulation.sizes;
    let columns = formulation.columns;

    let model = tuning.apply(formulation.model);
    let solved = model.solve();
    let report = SolveReport::read(&solved);

    let solution = extract_solution(&solved, &columns, || false).expect("uncancelled extraction");

    let replay = solution.as_ref().map(|found| replay(input, found));

    BlendRun {
        report,
        sizes,
        formulation_time,
        solution,
        replay,
    }
}

/// Extract published rows in bounded loops. The worker checks cancellation
/// here; the synchronous benchmark passes a signal that is always false.
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
    }))
}

/// Experiment 1: solve the *existing* parcel MILP with SCIP.
///
/// The model is not rewritten. [`super::super::highs`] writes the formulation
/// it would otherwise solve to an MPS file, and SCIP reads that file back, so
/// both backends see the same columns, rows and objective and the comparison
/// is of solvers rather than of formulations.
///
/// Note what MPS does and does not carry: columns, bounds, integrality, rows
/// and the objective survive; the accepted backend's *solve strategy* - the
/// restricted seed and the second lexicographic preference stage - does not,
/// because those are staged solves rather than model content. So this
/// compares single-shot solves of the same model, which is the comparison
/// worth having.
pub(crate) fn parcel_via_mps(path: &Path, tuning: SolveTuning) -> Result<SolveReport, String> {
    let model = Model::new()
        .hide_output()
        .include_default_plugins()
        .read_prob(path.to_str().ok_or("MPS path is not valid UTF-8")?)
        .map_err(|error| format!("SCIP could not read {}: {error:?}", path.display()))?;
    let solved = tuning.apply(model).solve();
    Ok(SolveReport::read(&solved))
}
