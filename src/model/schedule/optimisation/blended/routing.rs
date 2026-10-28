//! A schedule's dug material rerouted over the whole horizon, its digging
//! and every reclaim's share of its pile held.
//!
//! A pile is fully mixed and reclaim draws its opening blend, so a reclaim
//! of a fixed fraction `a` of the pile's opening stock takes the same
//! fraction of its metal. With each interval's fraction held at the
//! schedule's, the piles' stock and metal are linear in the receipts, and a
//! receipt's grade reaches the crusher's targets weeks later inside one LP.
//! Where the program fits the time, pile intake, stock and reclaim tonnes
//! move with the routing (receipts only where the schedule received, so
//! every pile's modes and rests hold); otherwise each pile's receipts are
//! held too and only which material goes where can change. Either LP is
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
use crate::model::schedule::optimisation::{Activity, DestinationId, DestinationKind, GroundId, LoaderId, SourceId, StockpileId, TaskKind, TruckClassId};

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

fn group(input: &BlendInput, groups: &Groups, schedule: &BlendSolution, window: Option<&std::ops::Range<usize>>, open: bool) -> Result<Grouped, String> {
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
    // When `open`, a pile may also receive where the schedule did not, as
    // its rules allow: on a day it builds, not while an exclusive pile is
    // reclaimed, and rested again before its next reclaim.
    let mut reclaimed_at: BTreeMap<StockpileId, BTreeSet<usize>> = BTreeMap::new();
    for row in schedule.movements.iter().filter(|row| row.tonnes_t > 0.0) {
        if let SourceId::Stockpile(pile) = input.movements[row.candidate].source {
            reclaimed_at.entry(pile).or_default().insert(row.interval);
        }
    }
    let mut receivable: BTreeMap<(StockpileId, usize), bool> = BTreeMap::new();
    let mut may_receive = |pile: StockpileId, at: usize| {
        *receivable.entry((pile, at)).or_insert_with(|| {
            let Some(blend) = input.piles.iter().find(|entry| entry.id == pile) else { return false };
            let interval = input.intervals[at];
            let reclaims = reclaimed_at.get(&pile);
            blend.builds(interval)
                && !(blend.exclusive && reclaims.is_some_and(|reclaims| reclaims.contains(&at)))
                && reclaims
                    .and_then(|reclaims| reclaims.range(at + 1..).next())
                    .is_none_or(|&next| blend.rested(interval.end_h, input.intervals[next]))
        })
    };
    let mut choices = Vec::with_capacity(dug.len());
    for ((group, interval, segment), tonnes) in dug {
        // Outside the window, each candidate keeps what it moved.
        if window.is_some_and(|window| !window.contains(&interval)) {
            for &option in &groups.options[group] {
                if let Some(&moved) = held.get(&(option, interval, segment)) {
                    choices.push((interval, segment, moved, vec![option]));
                }
            }
            continue;
        }
        // A pile cell the schedule left empty is held empty. Of destinations
        // alike in every constraint - one destination, one truck class at one
        // rate - only the best paid can be worth taking.
        let mut alike: BTreeMap<(DestinationId, TruckClassId, u64), usize> = BTreeMap::new();
        for &option in &groups.options[group] {
            let movement = &input.movements[option];
            if pile_of(input, movement.destination).is_some_and(|pile| !receipts.contains_key(&(pile, interval, segment)) && !(open && may_receive(pile, interval))) {
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
        choices.push((interval, segment, tonnes, alike.into_values().collect::<Vec<usize>>()));
    }
    Ok(Grouped { dug: choices, fixed, receipts })
}

/// Whether pile tonnes may move with the routing: only what the LP models
/// exactly - unchunked piles, unconditional reclaim routing and payments,
/// and no cap on any bar's reclaim.
fn tonnes_may_move(input: &BlendInput) -> bool {
    input.piles.iter().all(|pile| pile.chunks.is_empty())
        && input.conditional_values.is_empty()
        && input
            .qualifications
            .iter()
            .all(|entry| entry.alternatives.iter().any(|alternative| alternative.unconditional()))
        && !input.tasks.iter().any(|task| matches!(task.kind, TaskKind::Reclaim { maximum_t: Some(_), .. }))
}

/// What a reroute may change beyond which destination each dug tonne takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Program {
    /// Nothing: each pile's receipts are held.
    #[default]
    Held,
    /// Pile intake, stock and reclaim tonnes, in the cells the schedule
    /// received in.
    Moving,
    /// Those, and receipts in any cell the pile's rules allow.
    Opening,
}

/// The time a search leaves at its end for rerouting, what the reroute may
/// change, and how many windows the horizon is rerouted in.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Reserve {
    pub(crate) time: Duration,
    pub(crate) program: Program,
    pub(crate) windows: usize,
    /// How many times the horizon is rerouted: once exactly, then with each
    /// reclaim's tonnes free within 30% of the last pass's.
    pub(crate) passes: usize,
}

/// Routing choices one window takes at most. The interior point method's
/// time grows about as their square; one window a pass costs a build, the
/// whole horizon's pile chains and a replay besides.
const WINDOW_CHOICES: f64 = 130_000.0;

/// The time to leave a search's end for rerouting `schedule`, judged by the
/// number of routing choices it has: what the interior point method took on
/// the test worlds side by side, with a margin, window by window. The
/// widest program that fits in 60% of `budget` is chosen; the held one,
/// over the whole horizon at once, when nothing wider does.
pub(crate) fn reserve(input: &BlendInput, groups: &Groups, schedule: &BlendSolution, budget: Duration) -> Reserve {
    let most = budget.mul_f64(0.6);
    let choices = |open: bool| {
        group(input, groups, schedule, None, open).map(|grouped| grouped.dug.iter().map(|(.., options)| options.len()).filter(|&count| count > 1).sum::<usize>() as f64)
    };
    if tonnes_may_move(input) {
        for (program, passes) in [(Program::Opening, 2), (Program::Moving, 2), (Program::Opening, 1), (Program::Moving, 1)] {
            let Ok(columns) = choices(program == Program::Opening) else {
                return Reserve::default();
            };
            let windows = (columns / WINDOW_CHOICES).ceil().max(1.0);
            let each = columns / windows;
            let time = Duration::from_secs_f64(passes as f64 * (1.5 * windows * (1.9e-9 * each * each + 2.0)).max(6.0) + REPLAY_S);
            if time <= most {
                return Reserve {
                    time,
                    program,
                    windows: windows as usize,
                    passes,
                };
            }
        }
    }
    let Ok(columns) = choices(false) else {
        return Reserve::default();
    };
    let held = Duration::from_secs_f64((2.0 * (2.1e-4 * columns - 8.0)).max(6.0) + REPLAY_S);
    Reserve {
        time: held.min(most),
        program: Program::Held,
        windows: 1,
        passes: 1,
    }
}

/// The intervals the `index`th of `windows` windows frees, in time order;
/// one window is the whole horizon.
pub(crate) fn window(input: &BlendInput, windows: usize, index: usize) -> Option<std::ops::Range<usize>> {
    let count = input.intervals.len();
    (windows > 1).then(|| index * count / windows..(index + 1) * count / windows)
}

/// `schedule` with its dug material rerouted to the most it can earn, or an
/// error when the LP stops (`stop`) or fails.
pub(crate) fn reroute(
    input: &BlendInput,
    groups: &Groups,
    schedule: &BlendSolution,
    program: Program,
    draw_reach: f64,
    window: Option<&std::ops::Range<usize>>,
    stop: solver::Interrupt,
) -> Result<BlendSolution, String> {
    let Grouped { dug, fixed, receipts } = group(input, groups, schedule, window, program == Program::Opening && tonnes_may_move(input))?;
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

    // The schedule's reclaim from each pile per interval, and its stock at
    // the opening of each interval.
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
    // Each reclaim takes its share of the pile's opening stock in its
    // interval, held at the schedule's; where the pile was empty it is held.
    let share: Vec<Option<(StockpileId, f64)>> = fixed
        .iter()
        .map(|row| {
            let SourceId::Stockpile(pile) = input.movements[row.candidate].source else {
                return None;
            };
            let held = stock[&pile][row.interval];
            (held > 1e-9).then(|| (pile, row.tonnes_t / held))
        })
        .collect();
    // The fraction of each pile's opening stock drawn in each interval.
    let kept = |pile: StockpileId, at: usize| {
        let held = stock[&pile][at];
        let fraction = if held > 1e-9 {
            (drawn.get(&(pile, at)).copied().unwrap_or(0.0) / held).min(1.0)
        } else {
            0.0
        };
        // A pile drawn to its last tonne keeps nothing, not rounding.
        if fraction > 1.0 - 1e-9 { 0.0 } else { 1.0 - fraction }
    };
    let free = program != Program::Held && tonnes_may_move(input);
    // Each reclaim's tonnes as a column, boxed about the schedule's, where
    // the reclaim may move.
    let draw: Option<f64> = (free && draw_reach > 0.0).then_some(draw_reach);
    let drawing: Vec<Option<Col>> = fixed
        .iter()
        .zip(&share)
        .map(|(row, share)| {
            let reach = draw?;
            share.as_ref()?;
            Some(problem.add_column(
                input.movements[row.candidate].value_per_tonne().unwrap_or(0.0),
                row.tonnes_t * (1.0 - reach).max(0.0)..=row.tonnes_t * (1.0 + reach),
            ))
        })
        .collect();
    let mut drawn_by: BTreeMap<(StockpileId, usize), Vec<usize>> = BTreeMap::new();
    for (index, row) in fixed.iter().enumerate() {
        if drawing[index].is_some()
            && let Some((pile, _)) = share[index]
        {
            drawn_by.entry((pile, row.interval)).or_default().push(index);
        }
    }
    let tonnes_kept = |pile: StockpileId, at: usize| if draw.is_some() && drawn_by.contains_key(&(pile, at)) { 1.0 } else { kept(pile, at) };

    // Each pile's stock at the opening of each interval, a column plus a
    // constant: what it held, less the held fraction reclaimed, plus what it
    // received. Held at the schedule's unless the tonnes may move, when each
    // column earns what its interval's reclaim is paid.
    let mut paid: BTreeMap<(StockpileId, usize), f64> = BTreeMap::new();
    for (row, share) in fixed.iter().zip(&share).filter(|_| draw.is_none()) {
        if let Some((pile, share)) = share {
            *paid.entry((*pile, row.interval)).or_default() += share * input.movements[row.candidate].value_per_tonne().unwrap_or(0.0);
        }
    }
    let into = |pile: StockpileId| move |candidate: usize, _: usize, _: usize| (pile_of(input, input.movements[candidate].destination) == Some(pile)).then_some(1.0);
    let mut tonnage: BTreeMap<StockpileId, Vec<(Option<Col>, f64)>> = BTreeMap::new();
    for pile in &input.piles {
        let mut states: Vec<(Option<Col>, f64)> = vec![(None, stock[&pile.id][0])];
        for at in 0..intervals {
            if !free {
                states.push((None, stock[&pile.id][at + 1]));
                continue;
            }
            let (col, offset) = states[at];
            let kept = tonnes_kept(pile.id, at);
            let (mut terms, constant) = expression(&[at], &into(pile.id));
            let rows = drawn_by.get(&(pile.id, at)).map_or(&[][..], Vec::as_slice);
            // A pile neither receiving nor reclaimed holds what it held,
            // unless the next interval's reclaim needs a column to earn on.
            if kept == 1.0 && rows.is_empty() && terms.is_empty() && constant == 0.0 && !paid.contains_key(&(pile.id, at + 1)) {
                states.push((col, offset));
                continue;
            }
            let next = problem.add_column(paid.get(&(pile.id, at + 1)).copied().unwrap_or(0.0), 0.0..);
            for term in &mut terms {
                term.1 = -term.1;
            }
            terms.push((next, 1.0));
            if let Some(col) = col {
                terms.push((col, -kept));
            }
            // The reclaim takes what it draws, and no more than the pile holds.
            if !rows.is_empty() {
                let mut most: Vec<(Col, f64)> = rows.iter().map(|&index| (drawing[index].expect("drawn"), 1.0)).collect();
                terms.extend(most.iter().copied());
                if let Some(col) = col {
                    most.push((col, -1.0));
                }
                problem.add_row(..=offset, most);
            }
            let rhs = constant + kept * offset;
            problem.add_row(rhs..=rhs, terms);
            states.push((Some(next), 0.0));
        }
        tonnage.insert(pile.id, states);
    }
    // What a held row moves: its share of the pile's stock, or its tonnes.
    let moved = |index: usize| -> (Option<(Col, f64)>, f64) {
        if let Some(col) = drawing[index] {
            return (Some((col, 1.0)), 0.0);
        }
        match share[index] {
            Some((pile, share)) => {
                let (col, offset) = tonnage[&pile][fixed[index].interval];
                (col.map(|col| (col, share)), share * offset)
            }
            None => (None, fixed[index].tonnes_t),
        }
    };
    let held_moved = |at: &[usize], keep: &dyn Fn(&MovementRow) -> Option<f64>| {
        let mut terms: Vec<(Col, f64)> = Vec::new();
        let mut constant = 0.0;
        for &k in at {
            for &index in &fixed_by_interval[k] {
                let Some(coefficient) = keep(&fixed[index]) else { continue };
                let (term, offset) = moved(index);
                if let Some((col, share)) = term {
                    terms.push((col, coefficient * share));
                }
                constant += coefficient * offset;
            }
        }
        (terms, constant)
    };

    if !free {
        // Each pile's receipts held per interval and segment, wherever a
        // column could deliver.
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
    } else {
        // The blocks each loader dug per interval and segment: a loader that
        // worked two back to back delivered at an uneven rate.
        let mut blocks: BTreeMap<(LoaderId, usize, usize), BTreeSet<GroundId>> = BTreeMap::new();
        for row in schedule.movements.iter().filter(|row| row.tonnes_t > 1e-5) {
            let movement = &input.movements[row.candidate];
            if let (Activity::Dig, SourceId::Ground(ground)) = (movement.activity, movement.source) {
                blocks.entry((movement.loader, row.interval, row.segment)).or_default().insert(ground);
            }
        }
        let uneven = |pile: StockpileId, at: usize, cell: usize| {
            by_interval[at]
                .iter()
                .map(|&index| &entries[index])
                .filter(|entry| entry.2 == cell)
                .any(|&(candidate, ..)| {
                    let movement = &input.movements[candidate];
                    pile_of(input, movement.destination) == Some(pile) && blocks.get(&(movement.loader, at, cell)).is_some_and(|blocks| blocks.len() > 1)
                })
        };
        // Each pile's occupancy at the end of each segment, and, as the
        // replay checks it, before a segment's receipts where a loader
        // delivering there worked blocks back to back and it is reclaimed.
        for pile in input.piles.iter().filter(|pile| pile.capacity_t.is_finite()) {
            for at in 0..intervals {
                let (col, offset) = tonnage[&pile.id][at];
                let mut terms: Vec<(Col, f64)> = col.map(|col| vec![(col, 1.0)]).unwrap_or_default();
                let mut constant = offset;
                for cell in 0..input.segments_per_interval {
                    let into_cell =
                        |candidate: usize, _: usize, segment: usize| (segment == cell && pile_of(input, input.movements[candidate].destination) == Some(pile.id)).then_some(1.0);
                    let (received, received_constant) = expression(&[at], &into_cell);
                    let drawn_cell = |row: &MovementRow| (row.segment == cell && input.movements[row.candidate].source == SourceId::Stockpile(pile.id)).then_some(1.0);
                    let (reclaimed, reclaimed_constant) = held_moved(&[at], &drawn_cell);
                    if (!reclaimed.is_empty() || reclaimed_constant > 1e-5) && uneven(pile.id, at, cell) {
                        let mut before = terms.clone();
                        before.extend(received.iter().copied());
                        if !before.is_empty() {
                            problem.add_row(..=pile.capacity_t + SLACK_T - constant - received_constant, before);
                        }
                    }
                    // Occupancy rises only where the pile receives: a segment
                    // without receipts ends no fuller than the last one did.
                    let receives = !received.is_empty() || received_constant > 0.0;
                    terms.extend(received);
                    terms.extend(reclaimed.into_iter().map(|(col, coefficient)| (col, -coefficient)));
                    constant += received_constant - reclaimed_constant;
                    if receives && !terms.is_empty() {
                        problem.add_row(..=pile.capacity_t + SLACK_T - constant, terms.clone());
                    }
                }
            }
        }
        // Each loader's reclaim rate per interval and segment.
        let mut reclaim_cells: BTreeSet<(usize, usize, usize)> = BTreeSet::new();
        for (row, share) in fixed.iter().zip(&share) {
            if share.is_some()
                && let Some(loader) = input.loaders.iter().position(|loader| loader.id == input.movements[row.candidate].loader)
            {
                reclaim_cells.insert((loader, row.interval, row.segment));
            }
        }
        for &(loader, at, cell) in &reclaim_cells {
            let Some(rate) = super::input::interval_rate(&input.loaders[loader], at) else {
                continue;
            };
            let duration = schedule.durations.get(&(at, cell)).copied().unwrap_or(0.0);
            let id = input.loaders[loader].id;
            let by_loader = |row: &MovementRow| {
                let movement = &input.movements[row.candidate];
                (row.segment == cell && movement.loader == id && movement.activity == Activity::Reclaim).then_some(1.0)
            };
            let (terms, constant) = held_moved(&[at], &by_loader);
            if !terms.is_empty() {
                problem.add_row(..=rate.reclaim_tph * duration + SLACK_T - constant, terms);
            }
        }
    }
    // Dumps' capacities over the horizon.
    let all: Vec<usize> = (0..intervals).collect();
    for destination in &input.destinations {
        let (DestinationKind::Dump, Some(capacity)) = (destination.kind, destination.capacity_t) else {
            continue;
        };
        let weight = |candidate: usize, _: usize, _: usize| (input.movements[candidate].destination == destination.id).then_some(1.0);
        let (terms, constant) = expression(&all, &weight);
        if !terms.is_empty() {
            problem.add_row(..=capacity + SLACK_T - constant, terms);
        }
    }
    // Each targeted crusher's tonnes per target day, direct feed and
    // reclaim, as one column: every target of the day and its budget read
    // it, rather than each repeating the day's thousands of terms.
    let mut day_tonnes: BTreeMap<(DestinationId, u32), Col> = BTreeMap::new();
    for target in &input.grade_targets {
        if day_tonnes.contains_key(&(target.destination, target.day)) {
            continue;
        }
        let at: &[usize] = target_intervals.get(&target.day).map_or(&[], Vec::as_slice);
        let weight = |candidate: usize, _: usize, _: usize| (input.movements[candidate].destination == target.destination).then_some(1.0);
        let (mut terms, constant) = expression(at, &weight);
        let to_target = |row: &MovementRow| {
            (input.movements[row.candidate].destination == target.destination && matches!(input.movements[row.candidate].source, SourceId::Stockpile(_))).then_some(1.0)
        };
        let (reclaimed, reclaimed_constant) = held_moved(at, &to_target);
        terms.extend(reclaimed);
        for term in &mut terms {
            term.1 = -term.1;
        }
        let tonnes = problem.add_column(0.0, 0.0..);
        terms.push((tonnes, 1.0));
        let rhs = constant + reclaimed_constant;
        problem.add_row(rhs..=rhs, terms);
        day_tonnes.insert((target.destination, target.day), tonnes);
    }
    // Crusher budgets per day, reclaim included.
    for destination in &input.destinations {
        for (&day, at) in &day_intervals {
            let Some(Some(budget)) = destination.crusher_daily_t.get(day as usize) else { continue };
            if let Some(&tonnes) = day_tonnes.get(&(destination.id, day))
                && target_intervals.get(&day) == Some(at)
            {
                problem.add_row(..=*budget, vec![(tonnes, 1.0)]);
                continue;
            }
            let weight = |candidate: usize, _: usize, _: usize| (input.movements[candidate].destination == destination.id).then_some(1.0);
            let (mut terms, constant) = expression(at, &weight);
            let (reclaimed, reclaimed_constant) = held_moved(at, &|row: &MovementRow| (input.movements[row.candidate].destination == destination.id).then_some(1.0));
            terms.extend(reclaimed);
            if terms.is_empty() {
                continue;
            }
            problem.add_row(..=budget - constant - reclaimed_constant, terms);
        }
    }
    // Truck hours per class, interval and segment.
    let mut truck_cells: BTreeSet<(usize, usize)> = entries.iter().filter(|entry| entry.3.is_some()).map(|entry| (entry.1, entry.2)).collect();
    if free {
        truck_cells.extend(fixed.iter().zip(&share).filter(|(_, share)| share.is_some()).map(|(row, _)| (row.interval, row.segment)));
    }
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
            let (mut terms, constant) = expression(&[at], &weight);
            let by_truck = |row: &MovementRow| {
                let movement = &input.movements[row.candidate];
                (row.segment == cell && movement.truck == truck.id).then_some(movement.truck_hours_per_tonne)
            };
            let (reclaim, reclaim_constant) = held_moved(&[at], &by_truck);
            terms.extend(reclaim);
            if terms.is_empty() {
                continue;
            }
            problem.add_row(..=available + SLACK_T - constant - reclaim_constant, terms);
        }
    }

    // Each pile's metal of each targeted grade at the opening of each
    // interval, a column plus a constant: what it held, less the held
    // fraction reclaimed, plus what it received.
    let mut metal: BTreeMap<(StockpileId, usize), Vec<(Option<Col>, f64)>> = BTreeMap::new();
    // The schedule's metal in each pile at each interval's opening, where
    // the reclaimed metal is linearised: r·M/S about the schedule's r, M, S.
    let mut metal_held: BTreeMap<(StockpileId, usize), Vec<f64>> = BTreeMap::new();
    if draw.is_some() {
        for pile in &input.piles {
            let opening = pile.total_opening(input.grades.count()).1;
            for &grade in &grades {
                let mut received = vec![0.0; intervals];
                for row in &schedule.movements {
                    let movement = &input.movements[row.candidate];
                    if pile_of(input, movement.destination) == Some(pile.id) {
                        received[row.interval] += row.tonnes_t * input.grades.fraction(movement.material, grade).unwrap_or(0.0);
                    }
                }
                let mut held = vec![opening.get(grade).copied().unwrap_or(0.0)];
                for at in 0..intervals {
                    held.push((held[at] * kept(pile.id, at) + received[at]).max(0.0));
                }
                metal_held.insert((pile.id, grade), held);
            }
        }
    }
    // A drawn reclaim's metal: its terms and constant, given the pile's
    // stock and metal states at its interval.
    let drawn_metal = |index: usize, grade: usize, tonnes: (Option<Col>, f64), contained: (Option<Col>, f64)| -> (Vec<(Col, f64)>, f64) {
        let (pile, share) = share[index].expect("drawn rows have a share");
        let at = fixed[index].interval;
        let grade_held = metal_held[&(pile, grade)][at] / stock[&pile][at];
        let mut terms = vec![(drawing[index].expect("drawn"), grade_held)];
        if let Some(col) = contained.0 {
            terms.push((col, share));
        }
        if let Some(col) = tonnes.0 {
            terms.push((col, -share * grade_held));
        }
        (terms, share * contained.1 - share * grade_held * tonnes.1)
    };
    for pile in &input.piles {
        let opening = pile.total_opening(input.grades.count()).1;
        for &grade in &grades {
            let mut states: Vec<(Option<Col>, f64)> = vec![(None, opening.get(grade).copied().unwrap_or(0.0))];
            for at in 0..intervals {
                let (col, offset) = states[at];
                let kept = kept(pile.id, at);
                let weight = |candidate: usize, _: usize, _: usize| {
                    let movement = &input.movements[candidate];
                    (pile_of(input, movement.destination) == Some(pile.id)).then(|| input.grades.fraction(movement.material, grade).unwrap_or(0.0))
                };
                let (mut terms, constant) = expression(&[at], &weight);
                if kept == 1.0 && terms.is_empty() && constant == 0.0 {
                    states.push((col, offset));
                    continue;
                }
                // next - receipts' metal - kept previous = constant + kept offset
                let next = problem.add_column(0.0, 0.0..);
                for term in &mut terms {
                    term.1 = -term.1;
                }
                terms.push((next, 1.0));
                let mut rhs = constant + kept * offset;
                // A drawn reclaim takes its linearised metal, net of what the
                // held fraction already took.
                if let Some(rows) = drawn_by.get(&(pile.id, at)) {
                    let mut sum_share = 0.0;
                    for &index in rows {
                        let (out, out_constant) = drawn_metal(index, grade, tonnage[&pile.id][at], (col, offset));
                        terms.extend(out);
                        rhs -= out_constant;
                        sum_share += share[index].expect("drawn").1;
                    }
                    if let Some(col) = col {
                        terms.push((col, -(kept + sum_share)));
                    }
                    rhs += sum_share * offset;
                } else if let Some(col) = col {
                    terms.push((col, -kept));
                }
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
        let metal_weight = |candidate: usize, _: usize, _: usize| {
            let movement = &input.movements[candidate];
            (movement.destination == target.destination).then(|| input.grades.fraction(movement.material, target.grade).unwrap_or(0.0))
        };
        let (metal_terms, metal_constant) = expression(at, &metal_weight);
        let to_target = |row: &MovementRow| {
            (input.movements[row.candidate].destination == target.destination && matches!(input.movements[row.candidate].source, SourceId::Stockpile(_))).then_some(1.0)
        };
        let mut reclaim_q: Vec<(Col, f64)> = Vec::new();
        let mut reclaim_q_constant = 0.0;
        for &k in at {
            for &row_index in &fixed_by_interval[k] {
                let row = &fixed[row_index];
                if to_target(row).is_none() {
                    continue;
                }
                let Some((pile, share)) = share[row_index] else { continue };
                let (col, offset) = metal[&(pile, target.grade)][row.interval];
                if drawing[row_index].is_some() {
                    let (out, out_constant) = drawn_metal(row_index, target.grade, tonnage[&pile][row.interval], (col, offset));
                    reclaim_q.extend(out);
                    reclaim_q_constant += out_constant;
                    continue;
                }
                if let Some(col) = col {
                    reclaim_q.push((col, share));
                }
                reclaim_q_constant += share * offset;
            }
        }
        // The day's metal as one column, like its tonnes.
        let contained = problem.add_column(0.0, 0.0..);
        let mut terms: Vec<(Col, f64)> = metal_terms.iter().chain(&reclaim_q).map(|&(col, coefficient)| (col, -coefficient)).collect();
        terms.push((contained, 1.0));
        let rhs = metal_constant + reclaim_q_constant;
        problem.add_row(rhs..=rhs, terms);
        let tonnes = day_tonnes[&(target.destination, target.day)];
        let (opening_t, opening_q) = input
            .target_opening
            .iter()
            .find(|(at, day, ..)| *at == index && *day == target.day)
            .map_or((0.0, 0.0), |(_, _, tonnes, contained)| (*tonnes, *contained));
        for (boundary, direction, slope) in target.specification.hinges() {
            if slope == 0.0 {
                continue;
            }
            // slack >= direction * (Q - boundary T), opening included
            let slack = problem.add_column(-slope, 0.0..);
            let terms = vec![(slack, 1.0), (contained, -direction), (tonnes, direction * boundary)];
            problem.add_row(direction * (opening_q - boundary * opening_t).., terms);
        }
    }

    let solution = problem.maximise_ipm(Some(stop))?;
    // Each choice's tonnes, clipped at zero and scaled back to exactly what
    // was dug: a column a hair below zero would otherwise leave its excess on
    // the others.
    let mut routed: BTreeMap<(usize, usize, usize), f64> = BTreeMap::new();
    let mut next = 0;
    for (_, _, tonnes, options) in &dug {
        let group = &entries[next..next + options.len().max(1)];
        next += group.len();
        let values: Vec<f64> = group.iter().map(|&(.., col, held)| col.map_or(held, |col| solution[col.index()].max(0.0))).collect();
        let total: f64 = values.iter().sum();
        let scale = if total > 0.0 { tonnes / total } else { 0.0 };
        for (&(candidate, interval, segment, ..), value) in group.iter().zip(values) {
            if value * scale > 1e-9 {
                *routed.entry((candidate, interval, segment)).or_default() += value * scale;
            }
        }
    }
    // Each reclaim's share of its pile's stock as the routed rows leave it,
    // run forward exactly: the LP's stock is right only to its tolerance, and
    // a pile drawn out on that would close below empty.
    let mut reclaimed: Vec<f64> = fixed.iter().map(|row| row.tonnes_t).collect();
    let mut received: BTreeMap<(StockpileId, usize), f64> = BTreeMap::new();
    for (&(candidate, interval, _), &tonnes) in &routed {
        if let Some(pile) = pile_of(input, input.movements[candidate].destination) {
            *received.entry((pile, interval)).or_default() += tonnes;
        }
    }
    for pile in &input.piles {
        let mut held = pile.total_opening(input.grades.count()).0;
        for at in 0..intervals {
            let rows: Vec<usize> = fixed_by_interval[at]
                .iter()
                .copied()
                .filter(|&index| matches!(share[index], Some((of, _)) if of == pile.id))
                .collect();
            let want = |index: usize| match drawing[index] {
                Some(col) => solution[col.index()].max(0.0),
                None => share[index].map_or(0.0, |(_, share)| share) * held,
            };
            let wanted: f64 = rows.iter().map(|&index| want(index)).sum();
            let scale = if wanted > held { held / wanted } else { 1.0 };
            let mut drawn = 0.0;
            for &index in &rows {
                let tonnes = want(index) * scale;
                reclaimed[index] = tonnes;
                drawn += tonnes;
            }
            held = (held + received.get(&(pile.id, at)).copied().unwrap_or(0.0) - drawn).max(0.0);
        }
    }
    // A crusher's day meets its budget in the LP only to the solver's
    // tolerance, which a whole day's rows can take past the replay's. The
    // excess goes from direct feed to another destination it may take that
    // is no pile, or, where reclaim alone feeds the day, stays in the piles.
    for destination in input.destinations.iter().filter(|destination| destination.kind == DestinationKind::Crusher) {
        for (&day, at) in &day_intervals {
            let Some(Some(budget)) = destination.crusher_daily_t.get(day as usize) else { continue };
            let reclaims: Vec<usize> = at
                .iter()
                .flat_map(|&k| &fixed_by_interval[k])
                .copied()
                .filter(|&index| input.movements[fixed[index].candidate].destination == destination.id)
                .collect();
            let reclaim_t: f64 = reclaims.iter().map(|&index| reclaimed[index]).sum();
            let fed: f64 = routed
                .iter()
                .filter(|&(&(candidate, interval, _), _)| at.contains(&interval) && input.movements[candidate].destination == destination.id)
                .map(|(_, tonnes)| tonnes)
                .sum();
            let mut excess = fed + reclaim_t - budget;
            if excess <= 0.0 {
                continue;
            }
            let feeding: Vec<(usize, usize, usize)> = routed
                .keys()
                .copied()
                .filter(|&(candidate, interval, _)| at.contains(&interval) && input.movements[candidate].destination == destination.id)
                .collect();
            for (candidate, interval, segment) in feeding {
                let Some(group) = groups.of[candidate] else { continue };
                let Some(&elsewhere) = groups.options[group].iter().find(|&&option| {
                    let movement = &input.movements[option];
                    movement.destination != destination.id && pile_of(input, movement.destination).is_none()
                }) else {
                    continue;
                };
                let moved_t = routed[&(candidate, interval, segment)].min(excess);
                *routed.get_mut(&(candidate, interval, segment)).expect("the row is routed") -= moved_t;
                *routed.entry((elsewhere, interval, segment)).or_default() += moved_t;
                excess -= moved_t;
                if excess <= 0.0 {
                    break;
                }
            }
            if excess > 0.0 && reclaim_t > 0.0 {
                let scale = (reclaim_t - excess).max(0.0) / reclaim_t;
                for &index in &reclaims {
                    reclaimed[index] *= scale;
                }
            }
        }
    }
    let mut movements: Vec<MovementRow> = fixed.iter().zip(&reclaimed).map(|(row, &tonnes_t)| MovementRow { tonnes_t, ..*row }).collect();
    movements.extend(routed.into_iter().map(|((candidate, interval, segment), tonnes_t)| MovementRow {
        candidate,
        interval,
        segment,
        tonnes_t,
    }));
    Ok(BlendSolution { movements, ..schedule.clone() })
}
