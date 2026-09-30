//! Turning one validated blended solution into the application's calculated
//! schedule.
//!
//! Runs on the solver worker, after the independent replay has accepted the
//! answer and before anything is handed to the UI thread. It reads the
//! solution's own rows and durations, the replay's own reconstruction and the
//! capture's own identities - never the live project - and translates dense
//! model ids back to project ids once, here.
//!
//! What it does not do is decide anything. Which bar a movement was worked
//! under is the replay's reconstruction; when a span started and ended is
//! the solved event clock; what a stockpile held is the replay's balance.
//! Publication only renames and indexes.

use std::collections::BTreeMap;

use crate::{
    app::{commands::schedule_capture::CaptureIdentities, jobs::CancelFlag, scip_blend::ScipCompletion},
    model::schedule::{
        BarId, DestinationId as ProjectDestinationId, LoaderAgentId,
        cashflow::Activity as ProjectActivity,
        optimisation::{
            Activity, DestinationId, GroundId, LoaderId, SourceId, StockpileId, TaskId, TruckClassId,
            blended::{
                input::{BlendInput, GRADE_MARGIN},
                replay::{BlendSolution, ReplayReport},
            },
        },
        result::{CalculatedSchedule, ChunkDraw, Delivery, Execution, GroundBalance, PileStep, PileTrack, ScheduleParts, SolveQuality, SolveReport, WorkSource},
        trucking,
    },
};

/// A span shorter than this carries no time to put tonnes in. The replay
/// already bounds what such a span can hold by its loader's rate; anything
/// that remains is counted, not drawn.
const MINIMUM_SPAN_H: f64 = 1e-9;
/// Tonnes below this are solver noise rather than a movement: the replay's
/// own negligible scale, below which it does not hold a row to authored
/// priority either. Such rows are left off the timeline and counted in
/// [`SolveReport::omitted_rows`] - drawing a microgram as a working span would
/// hide a loader that was in fact idle.
const DUST_T: f64 = 1e-5;

/// Everything publication is told about the run besides the model.
pub(crate) struct PublishMeta<'a> {
    pub(crate) run: u64,
    pub(crate) semantic: u64,
    pub(crate) generation: u64,
    pub(crate) requested_end_h: f64,
    pub(crate) completion: &'a ScipCompletion,
    pub(crate) capture_s: f64,
    pub(crate) model_identity: u64,
    pub(crate) candidates: usize,
    pub(crate) ground_sources: usize,
    pub(crate) event_budget_restricted: bool,
    pub(crate) notes: Vec<String>,
}

struct Lookup<'a> {
    identities: &'a CaptureIdentities,
    loaders: BTreeMap<LoaderId, LoaderAgentId>,
    tasks: BTreeMap<TaskId, BarId>,
    piles: BTreeMap<StockpileId, ProjectDestinationId>,
    destinations: BTreeMap<DestinationId, ProjectDestinationId>,
    trucks: BTreeMap<TruckClassId, trucking::TruckClassId>,
}

impl<'a> Lookup<'a> {
    fn new(identities: &'a CaptureIdentities) -> Self {
        Self {
            identities,
            loaders: identities.loaders.iter().map(|(id, agent, _)| (*id, *agent)).collect(),
            tasks: identities.tasks.iter().map(|(id, bar, _)| (*id, *bar)).collect(),
            piles: identities.piles.iter().map(|(id, project, _)| (*id, *project)).collect(),
            destinations: identities.destinations.iter().map(|(id, project, _)| (*id, *project)).collect(),
            trucks: identities.trucks.iter().map(|(id, project, _)| (*id, *project)).collect(),
        }
    }

    fn source(&self, source: SourceId) -> Option<WorkSource> {
        match source {
            SourceId::Ground(ground) => self.identities.ground_blocks.get(&ground).map(|block| WorkSource::Block(*block)),
            SourceId::Stockpile(pile) => self.piles.get(&pile).map(|pile| WorkSource::Stockpile(*pile)),
        }
    }
}

/// When each execution cell starts and how long it lasts, from the solved
/// event clock.
fn cell_times(input: &BlendInput, solution: &BlendSolution) -> BTreeMap<(usize, usize), (f64, f64)> {
    let segments = input.segments_per_interval.max(1);
    let mut times = BTreeMap::new();
    for interval in &input.intervals {
        let mut at = interval.start_h;
        for segment in 0..segments {
            let duration = solution.durations.get(&(interval.index, segment)).copied().unwrap_or(0.0).max(0.0);
            // The last segment closes exactly on the interval edge, so floating
            // point drift in the clock cannot open a sliver between intervals.
            let end = if segment + 1 == segments {
                interval.end_h.max(at)
            } else {
                (at + duration).min(interval.end_h)
            };
            times.insert((interval.index, segment), (at, end));
            at = end;
        }
    }
    times
}

/// Merge `next` into `last` when they are one continuous span of the same
/// work at the same rate. Rates that differ stay separate spans: combining
/// them and spreading the total evenly would move tonnes in time.
fn same_rate(left_tonnes: f64, left_h: f64, right_tonnes: f64, right_h: f64) -> bool {
    if left_h <= 0.0 || right_h <= 0.0 {
        return false;
    }
    let left = left_tonnes / left_h;
    let right = right_tonnes / right_h;
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

/// Build the published schedule, or say why the solution cannot be one.
pub(crate) fn publish(
    input: &BlendInput,
    solution: &BlendSolution,
    replay: &ReplayReport,
    identities: &CaptureIdentities,
    meta: PublishMeta<'_>,
    cancel: &CancelFlag,
) -> Result<CalculatedSchedule, String> {
    let started = web_time::Instant::now();
    let lookup = Lookup::new(identities);
    let times = cell_times(input, solution);
    let grades = input.grades.count();
    if replay.row_tasks.len() != solution.movements.len() {
        return Err("the replay did not attribute every published movement to a bar".to_owned());
    }

    let mut omitted_rows = 0;
    let mut omitted_tonnes_t = 0.0;
    // (agent, bar, activity, source, interval, segment) -> tonnes
    let mut cells: BTreeMap<(LoaderAgentId, BarId, u8, SourceKey, usize, usize), f64> = BTreeMap::new();
    let mut deliveries: Vec<Delivery> = Vec::with_capacity(solution.movements.len());
    let mut dug: BTreeMap<GroundId, (f64, f64)> = BTreeMap::new();
    for (index, row) in solution.movements.iter().enumerate() {
        if index % 4096 == 0 && cancel.is_cancelled() {
            return Err("cancelled during publication".to_owned());
        }
        let Some(candidate) = input.movements.get(row.candidate) else {
            return Err(format!("movement row names unknown candidate {}", row.candidate));
        };
        let Some(&(start_h, end_h)) = times.get(&(row.interval, row.segment)) else {
            return Err(format!("movement row names an unknown cell ({}, {})", row.interval, row.segment));
        };
        if row.tonnes_t <= DUST_T || end_h - start_h < MINIMUM_SPAN_H {
            omitted_rows += 1;
            omitted_tonnes_t += row.tonnes_t.max(0.0);
            continue;
        }
        let task = replay.row_tasks[index].ok_or_else(|| format!("movement row {index} was worked under no authored bar"))?;
        let bar = *lookup.tasks.get(&task).ok_or("a movement names a bar capture did not record")?;
        let agent = *lookup.loaders.get(&candidate.loader).ok_or("a movement names a loader capture did not record")?;
        let source = lookup.source(candidate.source).ok_or("a movement names a source capture did not record")?;
        let destination = *lookup
            .destinations
            .get(&candidate.destination)
            .ok_or("a movement names a destination capture did not record")?;
        let truck = *lookup.trucks.get(&candidate.truck).ok_or("a movement names a truck class capture did not record")?;
        let activity = match candidate.activity {
            Activity::Dig => ProjectActivity::Dig,
            Activity::Reclaim => ProjectActivity::Reclaim,
        };
        // Contained quantity: a dig carries its captured material; a reclaim
        // carries the blend the replay says this interval actually drew.
        let contained: Vec<f64> = match candidate.source {
            SourceId::Ground(_) => (0..grades).map(|g| row.tonnes_t * input.grades.fraction(candidate.material, g).unwrap_or(0.0)).collect(),
            SourceId::Stockpile(pile) => {
                let blend = replay
                    .pile_intervals
                    .get(&(pile, row.interval))
                    .map(|state| state.delivered_blend.as_slice())
                    .unwrap_or_default();
                (0..grades).map(|g| row.tonnes_t * blend.get(g).copied().unwrap_or(0.0)).collect()
            }
        };
        // Movement value at the authored boundaries, exactly as the replay
        // valued it.
        let mut per_tonne = candidate.value_per_tonne().map_err(|_| "a movement value is not finite".to_owned())?;
        if let SourceId::Stockpile(pile) = candidate.source {
            let blend = replay
                .pile_intervals
                .get(&(pile, row.interval))
                .map(|state| state.delivered_blend.as_slice())
                .unwrap_or_default();
            for payment in input.conditional_values.iter().filter(|payment| payment.candidate == row.candidate) {
                if payment.holds(blend) {
                    per_tonne += payment.value_per_tonne;
                }
            }
        }
        if let SourceId::Ground(ground) = candidate.source {
            let entry = dug.entry(ground).or_insert((0.0, 0.0));
            entry.0 += row.tonnes_t;
            entry.1 = entry.1.max(end_h);
        }
        *cells.entry((agent, bar, activity as u8, SourceKey::of(source), row.interval, row.segment)).or_default() += row.tonnes_t;
        deliveries.push(Delivery {
            agent,
            bar,
            activity,
            source,
            destination,
            truck,
            start_h,
            end_h,
            tonnes: row.tonnes_t,
            truck_hours: row.tonnes_t * candidate.truck_hours_per_tonne,
            value: row.tonnes_t * per_tonne,
            contained,
        });
    }

    // One execution span per loader, bar and source in each cell, then
    // merged across cells where it is one continuous span at one rate.
    let mut executions: Vec<Execution> = Vec::with_capacity(cells.len());
    for ((agent, bar, activity, key, interval, segment), tonnes) in cells {
        let (start_h, end_h) = times[&(interval, segment)];
        executions.push(Execution {
            agent,
            bar,
            activity: if activity == ProjectActivity::Dig as u8 {
                ProjectActivity::Dig
            } else {
                ProjectActivity::Reclaim
            },
            source: key.source(),
            start_h,
            end_h,
            tonnes,
        });
    }
    executions.sort_by(|left, right| {
        (left.agent, left.bar, SourceKey::of(left.source), left.start_h)
            .partial_cmp(&(right.agent, right.bar, SourceKey::of(right.source), right.start_h))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut merged: Vec<Execution> = Vec::with_capacity(executions.len());
    for execution in executions {
        if let Some(last) = merged.last_mut()
            && last.agent == execution.agent
            && last.bar == execution.bar
            && last.source == execution.source
            && last.activity == execution.activity
            && (last.end_h - execution.start_h).abs() <= 1e-9
            && same_rate(last.tonnes, last.end_h - last.start_h, execution.tonnes, execution.end_h - execution.start_h)
        {
            last.end_h = execution.end_h;
            last.tonnes += execution.tonnes;
            continue;
        }
        merged.push(execution);
    }

    deliveries.sort_by(|left, right| {
        (left.agent, left.bar, SourceKey::of(left.source), left.destination, left.truck, left.start_h)
            .partial_cmp(&(right.agent, right.bar, SourceKey::of(right.source), right.destination, right.truck, right.start_h))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut merged_deliveries: Vec<Delivery> = Vec::with_capacity(deliveries.len());
    for delivery in deliveries {
        if let Some(last) = merged_deliveries.last_mut()
            && last.agent == delivery.agent
            && last.bar == delivery.bar
            && last.activity == delivery.activity
            && last.source == delivery.source
            && last.destination == delivery.destination
            && last.truck == delivery.truck
            && (last.end_h - delivery.start_h).abs() <= 1e-9
            && same_rate(last.tonnes, last.end_h - last.start_h, delivery.tonnes, delivery.end_h - delivery.start_h)
            && same_rate(last.value, last.end_h - last.start_h, delivery.value, delivery.end_h - delivery.start_h)
            && same_rate(last.truck_hours, last.end_h - last.start_h, delivery.truck_hours, delivery.end_h - delivery.start_h)
            && last.contained.len() == delivery.contained.len()
            && last
                .contained
                .iter()
                .zip(&delivery.contained)
                .all(|(left, right)| same_rate(*left, last.end_h - last.start_h, *right, delivery.end_h - delivery.start_h))
        {
            last.end_h = delivery.end_h;
            last.tonnes += delivery.tonnes;
            last.truck_hours += delivery.truck_hours;
            last.value += delivery.value;
            for (slot, value) in last.contained.iter_mut().zip(&delivery.contained) {
                *slot += value;
            }
            continue;
        }
        merged_deliveries.push(delivery);
    }

    // Ground: what each captured block started with, less what the published
    // spans took out of it.
    let mut ground = Vec::with_capacity(input.ground.len());
    for source in &input.ground {
        let Some(block) = identities.ground_blocks.get(&source.id) else { continue };
        let (taken, last) = dug.get(&source.id).copied().unwrap_or((0.0, 0.0));
        let mut remaining_t = source.tonnes_t - taken;
        if remaining_t.abs() <= 1e-6 * source.tonnes_t.max(1.0) {
            remaining_t = 0.0;
        }
        if remaining_t < 0.0 {
            return Err(format!("dig block {} was depleted past its measured tonnes", block.0));
        }
        ground.push(GroundBalance {
            block: *block,
            started_t: source.tonnes_t,
            remaining_t,
            emptied_h: (remaining_t == 0.0).then_some(last),
        });
    }

    // Stockpiles: the modelled ones off the replay's own balances, and every
    // other pile holding its authored opening stock throughout.
    let mut piles = Vec::new();
    for pile in &input.piles {
        let Some(project) = lookup.piles.get(&pile.id) else { continue };
        let (opening_t, opening_q) = pile.total_opening(grades);
        let steps = input
            .intervals
            .iter()
            .filter_map(|interval| {
                replay.pile_intervals.get(&(pile.id, interval.index)).map(|state| PileStep {
                    end_h: interval.end_h,
                    closing_t: state.closing_t.max(0.0),
                    closing_q: state.closing_q.clone(),
                })
            })
            .collect();
        piles.push(PileTrack {
            destination: *project,
            opening_t,
            opening_q,
            steps,
        });
    }
    for (project, opening_t) in &identities.pile_openings {
        if piles.iter().any(|track| track.destination == *project) {
            continue;
        }
        piles.push(PileTrack {
            destination: *project,
            opening_t: *opening_t,
            opening_q: Vec::new(),
            steps: Vec::new(),
        });
    }

    // Which chunk a reclaim actually drew, per interval.
    let mut chunk_draws = Vec::new();
    let mut chunk_slots_full = false;
    for pile in input.piles.iter().filter(|pile| !pile.chunks.is_empty()) {
        let Some(project) = lookup.piles.get(&pile.id) else { continue };
        let labels = identities.chunk_labels.get(&pile.id);
        let opening = identities.opening_chunks.get(&pile.id).copied().unwrap_or(0);
        let mut received = 0.0;
        for row in solution.chunks.iter().filter(|row| row.pile == pile.id) {
            received += if row.chunk >= opening { row.received_t } else { 0.0 };
            if row.reclaimed_t <= DUST_T {
                continue;
            }
            let Some(interval) = input.intervals.get(row.interval) else { continue };
            chunk_draws.push(ChunkDraw {
                pile: *project,
                chunk: row.chunk,
                label: labels.and_then(|labels| labels.get(row.chunk)).cloned().unwrap_or_else(|| format!("{}", row.chunk + 1)),
                start_h: interval.start_h,
                end_h: interval.end_h,
                tonnes: row.reclaimed_t,
            });
        }
        let receiving: f64 = pile.chunks.iter().skip(opening).sum();
        if receiving > 0.0 && received >= receiving - 1e-6 * receiving.max(1.0) {
            chunk_slots_full = true;
        }
    }
    chunk_draws.sort_by(|left, right| {
        (left.pile, left.start_h, left.chunk)
            .partial_cmp(&(right.pile, right.start_h, right.chunk))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let completion = meta.completion;
    let report = SolveReport {
        quality: Some(match completion.termination {
            crate::app::scip_blend::ScipTermination::Optimal => SolveQuality::Optimal,
            _ => SolveQuality::Limited,
        }),
        objective: replay.replayed_objective,
        raw_objective: completion.raw_objective,
        bound: completion.primary_bound,
        gap: completion.primary_gap,
        bound_source: completion.bound_source,
        capture_s: meta.capture_s,
        formulation_s: completion.timings.formulation.as_secs_f64(),
        solve_s: completion.timings.solver.as_secs_f64(),
        extraction_s: completion.timings.extraction.as_secs_f64(),
        replay_s: completion.timings.replay.as_secs_f64(),
        publication_s: started.elapsed().as_secs_f64(),
        backend: format!("{} · {}", completion.backend_version, completion.wrapper_version),
        model_identity: meta.model_identity,
        candidates: meta.candidates,
        ground_sources: meta.ground_sources,
        variables: completion.sizes.variables,
        binaries: completion.sizes.binaries,
        constraints: completion.sizes.linear_constraints + completion.sizes.nonlinear_constraints,
        linear_coefficient_entries: completion.sizes.linear_coefficient_entries,
        diagnostics: completion.diagnostics.clone(),
        day_by_day: completion.day_by_day.clone(),
        intervals: input.intervals.len(),
        segments_per_interval: input.segments_per_interval,
        event_budget_restricted: meta.event_budget_restricted,
        chunk_slots: input.piles.iter().map(|pile| pile.chunks.len()).sum(),
        chunk_slots_full,
        grade_margin: GRADE_MARGIN,
        boundary_rows: replay.boundary_rows,
        boundary_tonnes_t: replay.boundary_tonnes_t,
        boundary_value_slack: replay.boundary_value_slack,
        indicator_leak_value: replay.indicator_leak_value,
        omitted_rows: omitted_rows + solution.adjustments.movement_count,
        omitted_tonnes_t: omitted_tonnes_t + solution.adjustments.movement_total_t,
        notes: meta.notes,
    };
    Ok(CalculatedSchedule::new(ScheduleParts {
        run: meta.run,
        semantic: meta.semantic,
        generation: meta.generation,
        requested_end_h: meta.requested_end_h,
        executions: merged,
        deliveries: merged_deliveries,
        ground,
        piles,
        chunk_draws,
        agents: identities.loaders.iter().map(|(_, agent, _)| *agent).collect(),
        bar_blocks: identities.bar_blocks.clone(),
        reclaim_caps: identities.reclaim_caps.clone(),
        grades: identities.grades.iter().map(|(field, _, unit)| (*field, *unit)).collect(),
        report,
    }))
}

/// An orderable stand-in for [`WorkSource`], for grouping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SourceKey {
    Block(u64),
    Stockpile(ProjectDestinationId),
}

impl SourceKey {
    fn of(source: WorkSource) -> Self {
        match source {
            WorkSource::Block(block) => Self::Block(block.0),
            WorkSource::Stockpile(pile) => Self::Stockpile(pile),
        }
    }

    fn source(self) -> WorkSource {
        match self {
            Self::Block(block) => WorkSource::Block(crate::model::DigBlockId(block)),
            Self::Stockpile(pile) => WorkSource::Stockpile(pile),
        }
    }
}
