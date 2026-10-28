//! A schedule's dug material rerouted over the whole horizon, its digging,
//! each pile's receipts and every reclaim held, so that only which material
//! goes where can change.
//!
//! A pile is fully mixed and reclaim draws its opening blend, so a reclaim
//! of a fixed fraction `a` of the pile's opening stock takes the same
//! fraction of its metal. With each interval's fraction held at the
//! schedule's, the piles' metal is linear in the receipts, and a receipt's
//! grade reaches the crusher's targets weeks later inside one LP. The LP is
//! exact: solved, its routing replays to the value it modelled.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use super::{
    input::BlendInput,
    lp::{Col, LinearProgram},
    replay::{BlendSolution, MovementRow},
};
use crate::model::schedule::optimisation::{Activity, DestinationId, DestinationKind, SourceId, StockpileId, TruckClassId};

/// Tolerance on the held capacities, which the schedule meets to the
/// replay's own tolerance.
const SLACK_T: f64 = 1e-6;

/// Time kept back from a reroute's deadline for the replay that judges it.
pub(crate) const REPLAY_S: f64 = 1.5;

/// The dig candidates by what they dig - loader, source and material - each
/// group's candidates the destinations that material may take.
pub(crate) struct Groups {
    of: Vec<Option<usize>>,
    options: Vec<Vec<usize>>,
}

impl Groups {
    pub(crate) fn new(input: &BlendInput) -> Self {
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        let mut of = vec![None; input.movements.len()];
        let mut options: Vec<Vec<usize>> = Vec::new();
        for (at, candidate) in input.movements.iter().enumerate() {
            if candidate.activity != Activity::Dig {
                continue;
            }
            let key = format!("{:?}/{:?}/{:?}", candidate.loader, candidate.source, candidate.material);
            let group = *index.entry(key).or_insert_with(|| {
                options.push(Vec::new());
                options.len() - 1
            });
            options[group].push(at);
            of[at] = Some(group);
        }
        Self { of, options }
    }
}

/// A schedule's digging by group, interval and segment, each with the
/// destinations it may still take, and its other rows, which stay as they are.
struct Grouped {
    /// (interval, segment, tonnes, candidates): one candidate is held, more
    /// are the LP's to choose between.
    dug: Vec<(usize, usize, f64, Vec<usize>)>,
    fixed: Vec<MovementRow>,
    /// The schedule's receipts by pile, interval and segment.
    receipts: BTreeMap<(StockpileId, usize, usize), f64>,
}

fn pile_of(input: &BlendInput, destination: DestinationId) -> Option<StockpileId> {
    match input.destinations.iter().find(|entry| entry.id == destination).map(|entry| entry.kind) {
        Some(DestinationKind::Stockpile(pile)) => Some(pile),
        _ => None,
    }
}

fn group(input: &BlendInput, groups: &Groups, schedule: &BlendSolution) -> Result<Grouped, String> {
    let mut dug: BTreeMap<(usize, usize, usize), f64> = BTreeMap::new();
    let mut fixed: Vec<MovementRow> = Vec::new();
    let mut held: BTreeMap<(usize, usize, usize), f64> = BTreeMap::new();
    let mut receipts: BTreeMap<(StockpileId, usize, usize), f64> = BTreeMap::new();
    for row in schedule.movements.iter().filter(|row| row.tonnes_t > 0.0) {
        let Some(group) = groups.of[row.candidate] else {
            fixed.push(*row);
            continue;
        };
        *dug.entry((group, row.interval, row.segment)).or_default() += row.tonnes_t;
        *held.entry((row.candidate, row.interval, row.segment)).or_default() += row.tonnes_t;
        if let Some(pile) = pile_of(input, input.movements[row.candidate].destination) {
            *receipts.entry((pile, row.interval, row.segment)).or_default() += row.tonnes_t;
        }
    }
    let mut choices = Vec::with_capacity(dug.len());
    for ((group, interval, segment), tonnes) in dug {
        // A pile cell the schedule left empty is held empty. Of destinations
        // alike in every constraint - one destination, one truck class at one
        // rate - only the best paid can be worth taking.
        let mut alike: BTreeMap<(DestinationId, TruckClassId, u64), usize> = BTreeMap::new();
        for &option in &groups.options[group] {
            let movement = &input.movements[option];
            if pile_of(input, movement.destination).is_some_and(|pile| !receipts.contains_key(&(pile, interval, segment))) {
                continue;
            }
            let rank = |index: usize| {
                (
                    input.movements[index].value_per_tonne().unwrap_or(0.0),
                    held.get(&(index, interval, segment)).copied().unwrap_or(0.0),
                )
            };
            alike
                .entry((movement.destination, movement.truck, movement.truck_hours_per_tonne.to_bits()))
                .and_modify(|kept| {
                    if rank(option) > rank(*kept) {
                        *kept = option;
                    }
                })
                .or_insert(option);
        }
        if alike.is_empty() {
            return Err(format!("no destination left for dig group {group} in interval {interval} segment {segment}"));
        }
        choices.push((interval, segment, tonnes, alike.into_values().collect()));
    }
    Ok(Grouped { dug: choices, fixed, receipts })
}

/// The time to leave a search's end for rerouting `schedule`, judged by the
/// number of routing choices it has: what the interior point method took on
/// the test worlds alone, doubled for a machine shared by other solves, and
/// never more than 45% of `budget`.
pub(crate) fn reserve(input: &BlendInput, groups: &Groups, schedule: &BlendSolution, budget: Duration) -> Duration {
    let Ok(grouped) = group(input, groups, schedule) else {
        return Duration::ZERO;
    };
    let columns: usize = grouped.dug.iter().map(|(.., options)| options.len()).filter(|&count| count > 1).sum();
    let seconds = (2.0 * (2.1e-4 * columns as f64 - 8.0)).max(6.0) + REPLAY_S;
    Duration::from_secs_f64(seconds).min(budget.mul_f64(0.45))
}

/// `schedule` with its dug material rerouted to the most it can earn, or an
/// error when the LP stops (`stop`) or fails.
pub(crate) fn reroute(input: &BlendInput, groups: &Groups, schedule: &BlendSolution, stop: solver::Interrupt) -> Result<BlendSolution, String> {
    let Grouped { dug, fixed, receipts } = group(input, groups, schedule)?;
    let intervals = input.intervals.len();
    let mut problem = LinearProgram::default();
    // Each choice's entries: the candidate, its interval and segment, and its
    // column or, where there is nothing to choose, its held tonnes.
    let mut entries: Vec<(usize, usize, usize, Option<Col>, f64)> = Vec::new();
    for (interval, segment, tonnes, options) in &dug {
        if let [only] = options.as_slice() {
            entries.push((*only, *interval, *segment, None, *tonnes));
            continue;
        }
        let mut sum = Vec::with_capacity(options.len());
        for &option in options {
            let col = problem.add_column(input.movements[option].value_per_tonne().unwrap_or(0.0), 0.0..);
            sum.push((col, 1.0));
            entries.push((option, *interval, *segment, Some(col), 0.0));
        }
        problem.add_row(*tonnes..=*tonnes, sum);
    }

    // The entries and the held rows of each interval, and the intervals of
    // each day, so that each row reads only its own intervals.
    let mut by_interval: Vec<Vec<usize>> = vec![Vec::new(); intervals];
    for (index, entry) in entries.iter().enumerate() {
        by_interval[entry.1].push(index);
    }
    let mut fixed_by_interval: Vec<Vec<usize>> = vec![Vec::new(); intervals];
    for (index, row) in fixed.iter().enumerate() {
        fixed_by_interval[row.interval].push(index);
    }
    let held_rows = |at: &[usize]| at.iter().flat_map(|&k| &fixed_by_interval[k]).map(|&index| &fixed[index]).collect::<Vec<_>>();
    let mut day_intervals: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut target_intervals: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (k, interval) in input.intervals.iter().enumerate() {
        day_intervals.entry(interval.day()).or_default().push(k);
        target_intervals
            .entry(crate::model::schedule::grade_targets::target_day(interval.start_h))
            .or_default()
            .push(k);
    }
    // A linear expression over the entries of some intervals: its columns
    // and its constant.
    let expression = |at: &[usize], weight: &dyn Fn(usize, usize, usize) -> Option<f64>| {
        let mut terms: Vec<(Col, f64)> = Vec::new();
        let mut constant = 0.0;
        for &(candidate, interval, segment, col, tonnes) in at.iter().flat_map(|&k| &by_interval[k]).map(|&index| &entries[index]) {
            let Some(coefficient) = weight(candidate, interval, segment) else { continue };
            match col {
                Some(col) => terms.push((col, coefficient)),
                None => constant += coefficient * tonnes,
            }
        }
        (terms, constant)
    };

    // Each pile's receipts held per interval and segment, wherever a column
    // could deliver.
    let cells: BTreeSet<(StockpileId, usize, usize)> = entries
        .iter()
        .filter(|entry| entry.3.is_some())
        .filter_map(|&(candidate, interval, segment, ..)| Some((pile_of(input, input.movements[candidate].destination)?, interval, segment)))
        .collect();
    for &(pile, at, cell) in &cells {
        let weight = |candidate: usize, _: usize, segment: usize| (segment == cell && pile_of(input, input.movements[candidate].destination) == Some(pile)).then_some(1.0);
        let held = receipts.get(&(pile, at, cell)).copied().unwrap_or(0.0);
        let (terms, constant) = expression(&[at], &weight);
        problem.add_row(held - constant..=held - constant, terms);
    }
    // Crusher budgets per day, reclaim included.
    for destination in &input.destinations {
        for (&day, at) in &day_intervals {
            let Some(Some(budget)) = destination.crusher_daily_t.get(day as usize) else { continue };
            let weight = |candidate: usize, _: usize, _: usize| (input.movements[candidate].destination == destination.id).then_some(1.0);
            let (terms, constant) = expression(at, &weight);
            if terms.is_empty() {
                continue;
            }
            let reclaimed: f64 = held_rows(at)
                .into_iter()
                .filter(|row| input.movements[row.candidate].destination == destination.id)
                .map(|row| row.tonnes_t)
                .sum();
            problem.add_row(..=budget + SLACK_T - constant - reclaimed, terms);
        }
    }
    // Truck hours per class, interval and segment.
    let truck_cells: BTreeSet<(usize, usize)> = entries.iter().filter(|entry| entry.3.is_some()).map(|entry| (entry.1, entry.2)).collect();
    for truck in &input.trucks {
        for &(at, cell) in &truck_cells {
            let Some(hours) = truck.hours.get(at).copied() else { continue };
            let interval = input.intervals[at];
            if interval.duration_h() <= 0.0 {
                continue;
            }
            let available = hours / interval.duration_h() * schedule.durations.get(&(at, cell)).copied().unwrap_or(0.0);
            let weight = |candidate: usize, _: usize, segment: usize| {
                let movement = &input.movements[candidate];
                (segment == cell && movement.truck == truck.id).then_some(movement.truck_hours_per_tonne)
            };
            let (terms, constant) = expression(&[at], &weight);
            if terms.is_empty() {
                continue;
            }
            let reclaim: f64 = held_rows(&[at])
                .into_iter()
                .filter(|row| row.segment == cell && input.movements[row.candidate].truck == truck.id)
                .map(|row| row.tonnes_t * input.movements[row.candidate].truck_hours_per_tonne)
                .sum();
            problem.add_row(..=available + SLACK_T - constant - reclaim, terms);
        }
    }

    // Each pile's stock, held: opening, receipts and reclaim per interval.
    let grades: BTreeSet<usize> = input.grade_targets.iter().map(|target| target.grade).collect();
    let mut drawn: BTreeMap<(StockpileId, usize), f64> = BTreeMap::new();
    for row in &fixed {
        if let SourceId::Stockpile(pile) = input.movements[row.candidate].source {
            *drawn.entry((pile, row.interval)).or_default() += row.tonnes_t;
        }
    }
    let mut stock: BTreeMap<StockpileId, Vec<f64>> = BTreeMap::new();
    for pile in &input.piles {
        let mut received = vec![0.0; intervals];
        for (&(at_pile, interval, _), &tonnes) in &receipts {
            if at_pile == pile.id {
                received[interval] += tonnes;
            }
        }
        let mut held = vec![pile.total_opening(input.grades.count()).0];
        for interval in 0..intervals {
            let next = held[interval] + received[interval] - drawn.get(&(pile.id, interval)).copied().unwrap_or(0.0);
            held.push(next.max(0.0));
        }
        stock.insert(pile.id, held);
    }
    // Each pile's metal of each targeted grade at the opening of each
    // interval, a column plus a constant: what it held, less the held
    // fraction reclaimed, plus what it received.
    let mut metal: BTreeMap<(StockpileId, usize), Vec<(Option<Col>, f64)>> = BTreeMap::new();
    for pile in &input.piles {
        let opening = pile.total_opening(input.grades.count()).1;
        for &grade in &grades {
            let mut states: Vec<(Option<Col>, f64)> = vec![(None, opening.get(grade).copied().unwrap_or(0.0))];
            for at in 0..intervals {
                let (col, offset) = states[at];
                let held = stock[&pile.id][at];
                let fraction = if held > 1e-9 {
                    (drawn.get(&(pile.id, at)).copied().unwrap_or(0.0) / held).min(1.0)
                } else {
                    0.0
                };
                // A pile drawn to its last tonne keeps nothing, not rounding.
                let kept = if fraction > 1.0 - 1e-9 { 0.0 } else { 1.0 - fraction };
                let weight = |candidate: usize, _: usize, _: usize| {
                    let movement = &input.movements[candidate];
                    (pile_of(input, movement.destination) == Some(pile.id)).then(|| input.grades.fraction(movement.material, grade).unwrap_or(0.0))
                };
                let (mut terms, constant) = expression(&[at], &weight);
                // next - receipts' metal - kept previous = constant + kept offset
                let next = problem.add_column(0.0, ..);
                for term in &mut terms {
                    term.1 = -term.1;
                }
                terms.push((next, 1.0));
                if let Some(col) = col {
                    terms.push((col, -kept));
                }
                let rhs = constant + kept * offset;
                problem.add_row(rhs..=rhs, terms);
                states.push((Some(next), 0.0));
            }
            metal.insert((pile.id, grade), states);
        }
    }

    // The crusher's daily grade targets on direct feed and reclaim, the
    // reclaim carrying its held fraction of the pile's metal.
    for (index, target) in input.grade_targets.iter().enumerate() {
        let at: &[usize] = target_intervals.get(&target.day).map_or(&[], Vec::as_slice);
        let tonnes_weight = |candidate: usize, _: usize, _: usize| (input.movements[candidate].destination == target.destination).then_some(1.0);
        let metal_weight = |candidate: usize, _: usize, _: usize| {
            let movement = &input.movements[candidate];
            (movement.destination == target.destination).then(|| input.grades.fraction(movement.material, target.grade).unwrap_or(0.0))
        };
        let (tonnes_terms, tonnes_constant) = expression(at, &tonnes_weight);
        let (metal_terms, metal_constant) = expression(at, &metal_weight);
        let mut reclaim_t = 0.0;
        let mut reclaim_q: Vec<(Col, f64)> = Vec::new();
        let mut reclaim_q_constant = 0.0;
        for row in held_rows(at).into_iter().filter(|row| input.movements[row.candidate].destination == target.destination) {
            let SourceId::Stockpile(pile) = input.movements[row.candidate].source else { continue };
            reclaim_t += row.tonnes_t;
            let held = stock[&pile][row.interval];
            if held > 1e-9 {
                let (col, offset) = metal[&(pile, target.grade)][row.interval];
                let share = row.tonnes_t / held;
                if let Some(col) = col {
                    reclaim_q.push((col, share));
                }
                reclaim_q_constant += share * offset;
            }
        }
        let (opening_t, opening_q) = input
            .target_opening
            .iter()
            .find(|(at, day, ..)| *at == index && *day == target.day)
            .map_or((0.0, 0.0), |(_, _, tonnes, contained)| (*tonnes, *contained));
        for (boundary, direction, slope) in target.specification.hinges() {
            if slope == 0.0 {
                continue;
            }
            // slack >= direction * (Q - boundary T)
            let slack = problem.add_column(-slope, 0.0..);
            let mut terms = vec![(slack, 1.0)];
            terms.extend(metal_terms.iter().chain(&reclaim_q).map(|&(col, coefficient)| (col, -direction * coefficient)));
            terms.extend(tonnes_terms.iter().map(|&(col, coefficient)| (col, direction * boundary * coefficient)));
            let constant = direction * ((metal_constant + opening_q + reclaim_q_constant) - boundary * (tonnes_constant + reclaim_t + opening_t));
            problem.add_row(constant.., terms);
        }
    }

    let solution = problem.maximise_ipm(Some(stop))?;
    let mut movements = fixed;
    let mut routed: BTreeMap<(usize, usize, usize), f64> = BTreeMap::new();
    for &(candidate, interval, segment, col, tonnes) in &entries {
        let value = col.map_or(tonnes, |col| solution[col.index()].max(0.0));
        if value > 1e-9 {
            *routed.entry((candidate, interval, segment)).or_default() += value;
        }
    }
    movements.extend(routed.into_iter().map(|((candidate, interval, segment), tonnes_t)| MovementRow {
        candidate,
        interval,
        segment,
        tonnes_t,
    }));
    Ok(BlendSolution { movements, ..schedule.clone() })
}
