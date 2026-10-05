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
//! Back-to-back dig sources divide their cell in authored order, proportional
//! to their tonnes, so the timeline represents one block at a time per loader.

use std::collections::BTreeMap;

use crate::{
    app::{commands::schedule_capture::CaptureIdentities, jobs::CancelFlag, schedule_solve::ScheduleCompletion},
    model::schedule::{
        BarId, DestinationId as ProjectDestinationId, LoaderAgentId,
        cashflow::Activity as ProjectActivity,
        optimisation::{
            Activity, DestinationId, GroundId, LoaderId, SourceId, StockpileId, TaskId, TaskKind, TruckClassId,
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
    pub(crate) completion: &'a ScheduleCompletion,
    pub(crate) capture_s: f64,
    pub(crate) model_identity: u64,
    pub(crate) candidates: usize,
    pub(crate) ground_sources: usize,
    pub(crate) event_budget_restricted: bool,
    pub(crate) unworked_blasts: usize,
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

/// A loader can finish several blocks in one solved cell. They share its
/// effective dig rate, but occupy consecutive spans in the bar's authored
/// order. Material/destination/truck splits of one block share one span.
fn ordered_dig_spans(sequence: &[GroundId], tonnes: &BTreeMap<GroundId, f64>, start_h: f64, end_h: f64) -> Result<BTreeMap<GroundId, (f64, f64)>, String> {
    let total: f64 = tonnes.values().sum();
    let mut remaining = tonnes.clone();
    let mut spans = BTreeMap::new();
    let mut worked = 0.0;
    let mut at = start_h;
    for ground in sequence {
        let Some(tonnes) = remaining.remove(ground) else { continue };
        worked += tonnes;
        let end = if remaining.is_empty() {
            end_h
        } else {
            start_h + (end_h - start_h) * (worked / total).clamp(0.0, 1.0)
        };
        spans.insert(*ground, (at, end));
        at = end;
    }
    if !remaining.is_empty() {
        return Err("published digging names a block outside its authored sequence".to_owned());
    }
    Ok(spans)
}

type DigTimes = BTreeMap<(LoaderId, TaskId, GroundId, usize, usize), (f64, f64)>;
type ExecutionCellKey = (LoaderAgentId, BarId, u8, SourceKey, usize, usize);

fn dig_times(input: &BlendInput, solution: &BlendSolution, replay: &ReplayReport, times: &BTreeMap<(usize, usize), (f64, f64)>) -> Result<DigTimes, String> {
    let mut groups: BTreeMap<(LoaderId, TaskId, usize, usize), BTreeMap<GroundId, f64>> = BTreeMap::new();
    for (index, row) in solution.movements.iter().enumerate() {
        if row.tonnes_t <= DUST_T {
            continue;
        }
        let candidate = input.movements.get(row.candidate).ok_or("published movement names an unknown candidate")?;
        let SourceId::Ground(ground) = candidate.source else { continue };
        let task = replay.row_tasks[index].ok_or("published digging has no authored bar")?;
        *groups.entry((candidate.loader, task, row.interval, row.segment)).or_default().entry(ground).or_default() += row.tonnes_t;
    }
    let tasks: BTreeMap<_, _> = input.tasks.iter().map(|task| (task.id, task)).collect();
    let mut result = BTreeMap::new();
    for ((loader, task, interval, segment), tonnes) in groups {
        let TaskKind::Dig { sequence } = &tasks.get(&task).ok_or("published digging names an unknown bar")?.kind else {
            return Err("published digging belongs to a non-dig bar".to_owned());
        };
        let &(start_h, end_h) = times.get(&(interval, segment)).ok_or("published digging names an unknown cell")?;
        for (ground, span) in ordered_dig_spans(sequence, &tonnes, start_h, end_h)? {
            result.insert((loader, task, ground, interval, segment), span);
        }
    }
    Ok(result)
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

    let dig_times = dig_times(input, solution, replay, &times)?;
    let mut omitted_rows = 0;
    let mut omitted_tonnes_t = 0.0;
    // (agent, bar, activity, source, interval, segment) -> (start, end, tonnes)
    let mut cells: BTreeMap<ExecutionCellKey, (f64, f64, f64)> = BTreeMap::new();
    let cycles: Vec<_> = input.movements.iter().map(|c| std::sync::Arc::new(c.cycle)).collect();
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
        let (start_h, end_h) = match candidate.source {
            SourceId::Ground(ground) => dig_times[&(candidate.loader, task, ground, row.interval, row.segment)],
            SourceId::Stockpile(_) => (start_h, end_h),
        };
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
        cells
            .entry((agent, bar, activity as u8, SourceKey::of(source), row.interval, row.segment))
            .or_insert((start_h, end_h, 0.0))
            .2 += row.tonnes_t;
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
            cycle: std::sync::Arc::clone(&cycles[row.candidate]),
            value: row.tonnes_t * per_tonne,
            contained,
        });
    }

    // One execution span per loader, bar and source in each cell, then
    // merged across cells where it is one continuous span at one rate.
    let mut executions: Vec<Execution> = Vec::with_capacity(cells.len());
    for ((agent, bar, activity, key, _, _), (start_h, end_h, tonnes)) in cells {
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
            && last.cycle == delivery.cycle
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
    // Blocks of no tonnes never entered the model. Each is dug the moment its
    // bar reaches it: when the block before it ran out, or when the bar first
    // dug, or - a bar with nothing else to dig - at its earliest start.
    let mut instant: BTreeMap<crate::model::DigBlockId, f64> = BTreeMap::new();
    for &(bar, start_h, before, block) in &identities.empty_blocks {
        let blocks = identities.bar_blocks.iter().find(|(id, _)| *id == bar).map(|(_, blocks)| blocks.as_slice());
        let at = match (before, blocks) {
            (0, None) => Some(start_h),
            (0, Some(_)) => merged
                .iter()
                .filter(|execution| execution.bar == bar && matches!(execution.source, WorkSource::Block(_)))
                .map(|execution| execution.start_h)
                .reduce(f64::min),
            (before, Some(blocks)) => blocks
                .get(before - 1)
                .and_then(|previous| ground.iter().find(|balance| balance.block == *previous))
                .and_then(|balance| balance.emptied_h),
            (_, None) => None,
        };
        if let Some(at) = at.filter(|at| *at <= meta.requested_end_h) {
            instant.entry(block).and_modify(|held| *held = held.min(at)).or_insert(at);
        }
    }
    for (block, at) in instant {
        if !ground.iter().any(|balance| balance.block == block) {
            ground.push(GroundBalance {
                block,
                started_t: 0.0,
                remaining_t: 0.0,
                emptied_h: Some(at),
            });
        }
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
            crate::app::schedule_solve::SolveTermination::Optimal => SolveQuality::Optimal,
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
        unworked_blasts: meta.unworked_blasts,
        chunk_slots: input.piles.iter().map(|pile| pile.chunks.len()).sum(),
        chunk_slots_full,
        grade_margin: GRADE_MARGIN,
        boundary_rows: replay.boundary_rows,
        boundary_tonnes_t: replay.boundary_tonnes_t,
        boundary_value_slack: replay.boundary_value_slack,
        target_value_tolerance: replay.target_value_tolerance,
        indicator_leak_value: replay.indicator_leak_value,
        omitted_rows: omitted_rows + solution.adjustments.movement_count,
        omitted_tonnes_t: omitted_tonnes_t + solution.adjustments.movement_total_t,
        notes: meta.notes,
    };
    let mut schedule = CalculatedSchedule::new(ScheduleParts {
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
        grade_targets: replay
            .target_totals
            .iter()
            .map(|(&(index, period), &(tonnes, contained))| {
                let specification = input.grade_targets[index].specification.clone();
                let penalty = specification.penalty(tonnes, contained);
                crate::model::schedule::result::GradeTargetResult {
                    specification,
                    period,
                    tonnes,
                    contained,
                    penalty,
                }
            })
            .collect(),
        report,
        drill_blast: published_drill_blast(input, solution, identities),
    });
    explain_idle(&mut schedule, input, solution, &lookup);
    Ok(schedule)
}

/// Tonnes below which a block, a pile or the room left at a destination
/// counts as none: the replay's own negligible scale.
const IDLE_NEGLIGIBLE_T: f64 = 1e-3;

/// The part of `[start_h, end_h)` that falls inside `[from_h, to_h)`, as a
/// fraction of the former.
fn share_within(start_h: f64, end_h: f64, from_h: f64, to_h: f64) -> f64 {
    let duration = end_h - start_h;
    if duration <= 0.0 {
        return if start_h >= from_h && start_h < to_h { 1.0 } else { 0.0 };
    }
    ((end_h.min(to_h) - start_h.max(from_h)) / duration).clamp(0.0, 1.0)
}

/// Say why each idle span happened (see [`IdleReason`] for the order the
/// reasons are checked in), from the published schedule and the captured
/// input alone - so the same answer is given whichever optimiser made it.
fn explain_idle(schedule: &mut CalculatedSchedule, input: &BlendInput, solution: &BlendSolution, lookup: &Lookup<'_>) {
    use crate::model::schedule::{
        optimisation::{DestinationKind, TaskKind},
        result::IdleReason,
    };

    if schedule.idle.is_empty() {
        return;
    }
    let intervals: Vec<(f64, f64)> = input.intervals.iter().map(|interval| (interval.start_h, interval.end_h)).collect();
    let loaders: BTreeMap<LoaderAgentId, LoaderId> = lookup.loaders.iter().map(|(id, agent)| (*agent, *id)).collect();
    // What was taken out of each block, delivered to each destination and
    // hauled by each truck class, as (start, end, quantity) spans.
    let mut dug: BTreeMap<crate::model::DigBlockId, Vec<(f64, f64, f64)>> = BTreeMap::new();
    for execution in &schedule.executions {
        if let WorkSource::Block(block) = execution.source {
            dug.entry(block).or_default().push((execution.start_h, execution.end_h, execution.tonnes));
        }
    }
    let mut received: BTreeMap<ProjectDestinationId, Vec<(f64, f64, f64)>> = BTreeMap::new();
    let mut hauled: BTreeMap<trucking::TruckClassId, Vec<(f64, f64, f64)>> = BTreeMap::new();
    for delivery in &schedule.deliveries {
        received.entry(delivery.destination).or_default().push((delivery.start_h, delivery.end_h, delivery.tonnes));
        hauled.entry(delivery.truck).or_default().push((delivery.start_h, delivery.end_h, delivery.truck_hours));
    }
    let sum_within = |spans: Option<&Vec<(f64, f64, f64)>>, from_h: f64, to_h: f64| {
        spans.map_or(0.0, |spans| {
            spans.iter().map(|&(start, end, quantity)| quantity * share_within(start, end, from_h, to_h)).sum::<f64>()
        })
    };

    // Which chunked piles release something in each interval: a chunk closed,
    // rested and holding material.
    let mut closed_from: BTreeMap<(StockpileId, usize), usize> = BTreeMap::new();
    for row in solution.chunks.iter().filter(|row| row.closed) {
        let first = closed_from.entry((row.pile, row.chunk)).or_insert(row.interval);
        *first = (*first).min(row.interval);
    }
    let mut released: std::collections::BTreeSet<(StockpileId, usize)> = std::collections::BTreeSet::new();
    for row in solution.chunks.iter().filter(|row| row.closed && row.open_t > IDLE_NEGLIGIBLE_T) {
        let (Some(pile), Some(interval)) = (input.piles.iter().find(|pile| pile.id == row.pile), input.intervals.get(row.interval)) else {
            continue;
        };
        let rested = if pile.chunk_starts_closed(row.chunk) {
            pile.opening_chunk_rested(row.chunk, *interval)
        } else {
            closed_from
                .get(&(row.pile, row.chunk))
                .and_then(|first| input.intervals.get(*first))
                .is_some_and(|first| pile.rested(first.start_h, *interval))
        };
        if rested {
            released.insert((row.pile, row.interval));
        }
    }
    // Drill and blast: when each block's ground was released, and by which
    // blast, as the dispatch simulated it.
    let releases: BTreeMap<crate::model::schedule::optimisation::GroundId, (f64, usize)> = match (input.drill_blast.as_ref(), solution.drill_blast.as_ref()) {
        (Some(chain), Some(timeline)) => chain
            .blasts
            .iter()
            .zip(&timeline.blasts)
            .enumerate()
            .flat_map(|(index, (blast, events))| blast.releases.iter().map(move |ground| (*ground, (events.fired_h.unwrap_or(f64::MAX), index))))
            .collect(),
        _ => BTreeMap::new(),
    };
    // A loader waits on a blast when its highest-priority open bar is a dig
    // whose next block has not been released.
    let waiting = |agent: LoaderAgentId, position: usize| -> Option<usize> {
        if releases.is_empty() {
            return None;
        }
        let loader = *loaders.get(&agent)?;
        let start_h = input.intervals[position].start_h;
        let mut open: Vec<_> = input
            .tasks
            .iter()
            .filter(|task| task.loader == loader && task.window_start_h <= start_h + 1e-9 && task.window_end_h > start_h + 1e-9)
            .collect();
        open.sort_by(|left, right| {
            (left.priority, left.window_start_h, left.id)
                .partial_cmp(&(right.priority, right.window_start_h, right.id))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for task in open {
            let TaskKind::Dig { sequence } = &task.kind else { return None };
            let next = sequence.iter().find(|ground| {
                let Some(block) = lookup.identities.ground_blocks.get(ground) else { return false };
                let total = input.ground.iter().find(|source| source.id == **ground).map_or(0.0, |source| source.tonnes_t);
                total - sum_within(dug.get(block), f64::NEG_INFINITY, start_h) > IDLE_NEGLIGIBLE_T
            });
            if let Some(ground) = next {
                return releases.get(ground).filter(|(at, _)| *at > start_h + 1e-9).map(|(_, blast)| *blast);
            }
        }
        None
    };
    let explain = |schedule: &CalculatedSchedule, agent: LoaderAgentId, position: usize| {
        let Some(&loader) = loaders.get(&agent) else {
            return (IdleReason::NoWork, Vec::new());
        };
        let interval = input.intervals[position];
        let (start_h, end_h) = (interval.start_h, interval.end_h);
        if lookup
            .identities
            .delays
            .iter()
            .any(|(owner, from, to)| *owner == agent && *from <= start_h + 1e-9 && start_h < *to - 1e-9)
        {
            return (IdleReason::Delayed, Vec::new());
        }
        let (dig_tph, reclaim_tph) = input
            .loaders
            .iter()
            .find(|candidate| candidate.id == loader)
            .and_then(|found| found.rates.iter().find(|rate| rate.interval == interval.index))
            .map_or((0.0, 0.0), |rate| (rate.dig_tph, rate.reclaim_tph));
        if dig_tph <= 0.0 && reclaim_tph <= 0.0 {
            return (IdleReason::Unavailable, Vec::new());
        }
        let open: Vec<_> = input
            .tasks
            .iter()
            .filter(|task| task.loader == loader && task.window_start_h < end_h - 1e-9 && task.window_end_h > start_h + 1e-9)
            .collect();
        if open.is_empty() {
            return (IdleReason::NoWork, Vec::new());
        }
        let rate_for = |kind: &TaskKind| match kind {
            TaskKind::Dig { .. } => dig_tph,
            TaskKind::Reclaim { .. } => reclaim_tph,
            TaskKind::Delay => 0.0,
        };
        let pile_entry = |pile: StockpileId| input.piles.iter().find(|entry| entry.id == pile);
        let project_pile = |pile: StockpileId| lookup.piles.get(&pile).copied();
        // An unchunked pile with a rest is not reclaimable while anything it
        // received is still resting.
        let resting = |pile: StockpileId| {
            let Some(entry) = pile_entry(pile).filter(|entry| entry.rest_h > 0.0 && entry.chunks.is_empty()) else {
                return false;
            };
            let project = project_pile(pile);
            schedule.deliveries.iter().any(|delivery| {
                Some(delivery.destination) == project
                    && delivery.tonnes > IDLE_NEGLIGIBLE_T
                    && delivery.start_h < start_h - 1e-9
                    && !entry.rested(delivery.end_h.min(start_h), interval)
            })
        };
        // A chunked pile releases nothing while its chunks are filling or
        // resting.
        let unreleased = |pile: StockpileId| pile_entry(pile).is_some_and(|entry| !entry.chunks.is_empty()) && !released.contains(&(pile, position));
        let reclaims = |pile: StockpileId| pile_entry(pile).is_none_or(|entry| entry.reclaims(interval)) && !resting(pile) && !unreleased(pile);
        // A pile that may not build and reclaim at once, reclaimed here.
        let reclaimed_here = |pile: StockpileId| {
            let project = project_pile(pile);
            schedule.executions.iter().any(|execution| {
                matches!(execution.source, WorkSource::Stockpile(source) if Some(source) == project)
                    && execution.tonnes > IDLE_NEGLIGIBLE_T
                    && execution.start_h < end_h - 1e-9
                    && execution.end_h > start_h + 1e-9
            })
        };
        let builds = |pile: StockpileId| pile_entry(pile).is_none_or(|entry| entry.builds(interval) && !(entry.exclusive && reclaimed_here(pile)));
        // What each bar could work at the start of the interval.
        let task_sources = |task: &crate::model::schedule::optimisation::Task| -> Vec<SourceId> {
            let mut sources = Vec::new();
            match &task.kind {
                // Ground is dug in order: what the machine could be on is the
                // first block of the sequence that still holds any.
                TaskKind::Dig { sequence } => {
                    let next = sequence.iter().find(|ground| {
                        let Some(block) = lookup.identities.ground_blocks.get(ground) else { return false };
                        let total = input.ground.iter().find(|source| source.id == **ground).map_or(0.0, |source| source.tonnes_t);
                        total - sum_within(dug.get(block), f64::NEG_INFINITY, start_h) > IDLE_NEGLIGIBLE_T
                    });
                    sources.extend(next.map(|ground| SourceId::Ground(*ground)));
                }
                TaskKind::Reclaim { approved_sources, .. } => {
                    for pile in approved_sources {
                        if !reclaims(*pile) {
                            continue;
                        }
                        let held = lookup
                            .piles
                            .get(pile)
                            .and_then(|project| schedule.inventory_at(*project, start_h))
                            .map_or(0.0, |(tonnes, _)| tonnes);
                        if held > IDLE_NEGLIGIBLE_T {
                            sources.push(SourceId::Stockpile(*pile));
                        }
                    }
                }
                TaskKind::Delay => {}
            }
            sources
        };
        // A delay bar holds the machine when no bar above it had work.
        let mut ordered = open.clone();
        ordered.sort_by(|left, right| {
            (left.priority, left.window_start_h, left.id)
                .partial_cmp(&(right.priority, right.window_start_h, right.id))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for task in &ordered {
            if task.window_start_h > start_h + 1e-9 || task.window_end_h < end_h - 1e-9 {
                continue;
            }
            if matches!(task.kind, TaskKind::Delay) {
                return (IdleReason::Delayed, Vec::new());
            }
            if rate_for(&task.kind) > 0.0 && !task_sources(task).is_empty() {
                break;
            }
        }
        if open.iter().all(|task| rate_for(&task.kind) <= 0.0) {
            return (IdleReason::Unavailable, Vec::new());
        }
        // The bar holding the machine is the highest one with work: the
        // reason is that bar's, not one below it the machine may not work.
        let holding = ordered
            .iter()
            .find(|task| task.window_start_h <= start_h + 1e-9 && task.window_end_h >= end_h - 1e-9 && rate_for(&task.kind) > 0.0 && !task_sources(task).is_empty());
        let sources: Vec<SourceId> = match holding {
            Some(task) => task_sources(task),
            None => open.iter().filter(|task| rate_for(&task.kind) > 0.0).flat_map(|task| task_sources(task)).collect(),
        };
        if sources.is_empty() {
            // Stock was there, but the pile's mode keeps it from reclaiming.
            let mut closed: Vec<_> = open
                .iter()
                .filter(|task| rate_for(&task.kind) > 0.0)
                .filter_map(|task| match &task.kind {
                    TaskKind::Reclaim { approved_sources, .. } => Some(approved_sources),
                    _ => None,
                })
                .flatten()
                .filter(|pile| !reclaims(**pile))
                .filter_map(|pile| {
                    let project = lookup.piles.get(pile)?;
                    let held = schedule.inventory_at(*project, start_h).map_or(0.0, |(tonnes, _)| tonnes);
                    (held > IDLE_NEGLIGIBLE_T).then_some(*project)
                })
                .collect();
            if !closed.is_empty() {
                closed.sort_unstable();
                closed.dedup();
                return (IdleReason::PileMode, closed);
            }
            return (IdleReason::WorkFinished, Vec::new());
        }
        let candidates: Vec<_> = input
            .movements
            .iter()
            .filter(|movement| movement.loader == loader && sources.contains(&movement.source))
            .collect();
        let routed = |source: SourceId| match source {
            SourceId::Ground(ground) => input.ground.iter().find(|found| found.id == ground).is_some_and(|found| {
                found
                    .material
                    .iter()
                    .filter(|share| share.fraction > 0.0)
                    .all(|share| candidates.iter().any(|movement| movement.source == source && movement.material == share.material))
            }),
            SourceId::Stockpile(_) => candidates.iter().any(|movement| movement.source == source),
        };
        if !sources.iter().any(|source| routed(*source)) {
            return (IdleReason::NoRoute, Vec::new());
        }
        let has_room = |destination: DestinationId| {
            let Some(found) = input.destinations.iter().find(|candidate| candidate.id == destination) else {
                return false;
            };
            let project = lookup.destinations.get(&destination);
            match found.kind {
                DestinationKind::Crusher => {
                    let day = interval.day();
                    match found.crusher_daily_t.get(day as usize).copied().flatten() {
                        None => true,
                        Some(budget) => {
                            let day_start = f64::from(day) * 24.0;
                            budget - sum_within(project.and_then(|project| received.get(project)), day_start, day_start + 24.0) > IDLE_NEGLIGIBLE_T
                        }
                    }
                }
                DestinationKind::Dump => found
                    .capacity_t
                    .is_none_or(|capacity| capacity - sum_within(project.and_then(|project| received.get(project)), f64::NEG_INFINITY, start_h) > IDLE_NEGLIGIBLE_T),
                DestinationKind::Stockpile(pile) if !builds(pile) => false,
                DestinationKind::Stockpile(pile) => found.capacity_t.is_none_or(|capacity| {
                    let held = lookup
                        .piles
                        .get(&pile)
                        .and_then(|project| schedule.inventory_at(*project, start_h))
                        .map_or(0.0, |(tonnes, _)| tonnes);
                    capacity - held > IDLE_NEGLIGIBLE_T
                }),
            }
        };
        // Ground is dug whole, its materials in proportion, so a block can be
        // dug only while every material in it has somewhere to go - the
        // waste having room does not let the ore out. A source is open when
        // each of its materials has a candidate with room.
        let roomy: Vec<_> = candidates.iter().copied().filter(|movement| has_room(movement.destination)).collect();
        let open_source = |source: SourceId| {
            let materials: Vec<_> = match source {
                SourceId::Ground(ground) => input
                    .ground
                    .iter()
                    .find(|found| found.id == ground)
                    .map(|found| found.material.iter().filter(|share| share.fraction > 0.0).map(|share| share.material).collect())
                    .unwrap_or_default(),
                SourceId::Stockpile(_) => candidates.iter().filter(|movement| movement.source == source).map(|movement| movement.material).collect(),
            };
            !materials.is_empty()
                && materials
                    .iter()
                    .all(|material| roomy.iter().any(|movement| movement.source == source && movement.material == *material))
        };
        let open_sources: Vec<_> = sources.iter().copied().filter(|source| open_source(*source)).collect();
        if open_sources.is_empty() {
            let mut full: Vec<_> = candidates
                .iter()
                .filter(|movement| !has_room(movement.destination))
                .filter_map(|movement| lookup.destinations.get(&movement.destination).copied())
                .collect();
            full.sort_unstable();
            full.dedup();
            // A pile its mode keeps from building is the planner's own
            // setting, so it is the reason given, before any full one.
            let mut closed: Vec<_> = candidates
                .iter()
                .filter_map(|movement| match input.destinations.iter().find(|found| found.id == movement.destination)?.kind {
                    DestinationKind::Stockpile(pile) if !builds(pile) => lookup.destinations.get(&movement.destination).copied(),
                    _ => None,
                })
                .collect();
            if !closed.is_empty() {
                closed.sort_unstable();
                closed.dedup();
                return (IdleReason::PileMode, closed);
            }
            return (IdleReason::DestinationsFull, full);
        }
        let roomy: Vec<_> = roomy.into_iter().filter(|movement| open_sources.contains(&movement.source)).collect();
        let trucks_spent = |truck: TruckClassId| {
            let available = input
                .trucks
                .iter()
                .find(|class| class.id == truck)
                .and_then(|class| class.hours.get(position))
                .copied()
                .unwrap_or(0.0);
            let used = sum_within(lookup.trucks.get(&truck).and_then(|project| hauled.get(project)), start_h, end_h);
            available - used <= 1e-6 * available.max(1.0)
        };
        if roomy.iter().all(|movement| trucks_spent(movement.truck)) {
            return (IdleReason::NoTrucks, Vec::new());
        }
        (IdleReason::NotWorthIt, Vec::new())
    };
    schedule.classify_idle(&intervals, |schedule, agent, position| {
        if let Some(blast) = waiting(agent, position) {
            return (IdleReason::WaitingOnBlast, Vec::new(), Some(blast));
        }
        let (reason, full) = explain(schedule, agent, position);
        (reason, full, None)
    });
}

/// Drill and blast in project terms: the captured blasts with the
/// milestones the dispatch simulated, and each machine's work.
fn published_drill_blast(
    input: &BlendInput,
    solution: &BlendSolution,
    identities: &crate::app::commands::schedule_capture::CaptureIdentities,
) -> Option<crate::model::schedule::result::DrillBlastResult> {
    use crate::model::schedule::result::{DrillBlastResult, PublishedBlast, PublishedBlastWork};
    let chain = input.drill_blast.as_ref()?;
    let timeline = solution.drill_blast.as_ref();
    // Which blast releases each piece of ground, so a blast can name the
    // blasts standing over it.
    let released_by: BTreeMap<_, usize> = chain
        .blasts
        .iter()
        .enumerate()
        .flat_map(|(index, blast)| blast.releases.iter().map(move |ground| (*ground, index)))
        .collect();
    let unworked: BTreeMap<usize, _> = chain.unworked().into_iter().collect();
    let blasts = identities
        .blasts
        .iter()
        .enumerate()
        .map(|(index, blast)| {
            let events = timeline.and_then(|timeline| timeline.blasts.get(index)).copied().unwrap_or_default();
            PublishedBlast {
                reference: blast.reference,
                solid_name: blast.solid_name.clone(),
                name: blast.name.clone(),
                bench_base: blast.bench.base,
                bench_top: blast.bench.top,
                face: std::sync::Arc::clone(&blast.face),
                collars: blast.collars.clone(),
                quantity: blast.quantity,
                cleared_h: events.cleared_h,
                done_h: events.done_h,
                fired_h: events.fired_h.filter(|at| *at < f64::MAX),
                never_clear: chain.blasts.get(index).is_some_and(|job| job.never_clear),
                unworked: unworked.get(&index).copied(),
                above: chain.blasts.get(index).map_or_else(Vec::new, |job| {
                    let mut above: Vec<usize> = job.above.iter().filter_map(|ground| released_by.get(ground).copied()).collect();
                    above.sort_unstable();
                    above.dedup();
                    above
                }),
            }
        })
        .collect();
    let work = timeline
        .map(|timeline| {
            timeline
                .work
                .iter()
                .filter_map(|row| {
                    Some(PublishedBlastWork {
                        agent: *identities.blast_agents.get(row.agent)?,
                        blast: row.blast,
                        activity: row.activity,
                        start_h: row.start_h,
                        end_h: row.end_h,
                        quantity: row.quantity,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(DrillBlastResult { blasts, work })
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
