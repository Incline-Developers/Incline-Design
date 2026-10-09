//! A Lagrangian bound on the whole-horizon plan, loader by loader.
//!
//! The plan's linear relaxation is a weak bound on long horizons: with its
//! finished flags fractional, a sliver of every block of a sequence may be
//! dug at once, so ore deep in a sequence looks reachable on the first day.
//! Here the rows loaders share - fleet hours, crusher days, dump room, daily
//! grade targets, pile inventories, and a loader's own dig and reclaim hours -
//! are priced instead of enforced. What is left falls apart by loader, and
//! each loader's part is solved exactly: how far along its sequence it is at
//! the end of each day, its blocks dug in order and each dug through before
//! the next is started, by a dynamic program over that progress. Reclaim
//! bars and pile inventories are each a sort. Any prices give an upper bound
//! on the plan, and so on every schedule (see `plan`); the prices start at
//! the relaxation's own and are moved by subgradient steps aimed at the best
//! schedule known (Polyak's), and the least value seen is the bound.
//!
//! Progress is solved on a grid of cells along each sequence. Rounding each
//! day's progress up to the grid keeps every true plan within reach of a
//! grid plan worth at most two cells' value a day less, and that much is
//! added to the bound, which so stays one.
//!
//! A block more than one loader may dig is left to the plan's own bound: its
//! tonnes are shared between the loaders' sequences, which does not split.
//! This is Bienstock and Zuckerberg's observation about precedence
//! constrained scheduling - priced side constraints leave a problem that is
//! easy at any size - on sequences, which are easier still.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use super::plan::{Carried, Prepared, UNIT};
use crate::model::schedule::optimisation::{DestinationId, DestinationKind, MaterialId, StockpileId, TaskKind};

/// The rows whose prices the bound moves into the objective, by name prefix
/// in the plan.
pub(crate) const PRICED: [&str; 6] = ["fleet_", "crush_", "room_", "grade_", "inv_", "hours_"];

/// Grid cells along each loader's sequence.
const CELLS: usize = 20_000;

/// Polyak step scale: halved after this many steps without a better bound,
/// and the search stops once it falls below the floor.
const STALL: usize = 8;
const STEP_FLOOR: f64 = 1e-4;

/// One hinge of a daily grade target, as a plan row.
struct Hinge {
    /// The plan row's name: `grade_{target}_{hinge}`.
    name: String,
    destination: DestinationId,
    p: usize,
    grade: usize,
    boundary: f64,
    direction: f64,
    /// The hinge's slope per kilotonne: its price never exceeds it.
    slope: f64,
}

/// One loader's sequence: each block's length in kilotonnes and the days it
/// may be dug.
struct Chain {
    loader: usize,
    blocks: Vec<(crate::model::schedule::optimisation::GroundId, f64, BTreeSet<usize>)>,
}

#[derive(Clone, Default)]
struct Prices {
    fleet: Vec<f64>,
    crusher: BTreeMap<(DestinationId, usize), f64>,
    dump: BTreeMap<DestinationId, f64>,
    grade: Vec<f64>,
    pile: BTreeMap<(StockpileId, usize), f64>,
    hours: BTreeMap<(usize, usize), f64>,
}

impl Prices {
    fn norm(&self) -> f64 {
        self.fleet
            .iter()
            .chain(self.crusher.values())
            .chain(self.dump.values())
            .chain(&self.grade)
            .chain(self.pile.values())
            .chain(self.hours.values())
            .map(|v| v * v)
            .sum()
    }
}

/// Per day, each block's best value per kilotonne and where its materials
/// go - destination, material, share and truck hours - or `None` on a day it
/// may not be dug.
type Densities = Vec<Vec<Option<(f64, Vec<(DestinationId, MaterialId, f64, f64)>)>>>;

/// What the priced rows' slacks are under a solution of the subproblems:
/// right side less left. A subgradient of the Lagrangian.
type Slacks = Prices;

struct Lagrangian<'p, 'a> {
    prepared: &'p Prepared<'a>,
    hinges: Vec<Hinge>,
    hinges_at: BTreeMap<(DestinationId, usize), Vec<usize>>,
    budgets: BTreeMap<(DestinationId, usize), f64>,
    rooms: BTreeMap<DestinationId, f64>,
    fleet: Vec<f64>,
    /// Loader-days whose dig and reclaim share hours: fastest dig and
    /// reclaim rates, and the hours.
    hours: BTreeMap<(usize, usize), (f64, f64, f64)>,
    /// Each pile's capacity and opening tonnes, in kilotonnes, and the
    /// destinations that deliver to it.
    piles: Vec<(StockpileId, f64, f64, Vec<DestinationId>)>,
    chains: Vec<Chain>,
}

impl<'p, 'a> Lagrangian<'p, 'a> {
    fn new(prepared: &'p Prepared<'a>) -> Option<Self> {
        let input = prepared.input;
        let periods = prepared.periods();
        // A block shared between loaders does not split.
        let mut diggers: BTreeMap<_, usize> = BTreeMap::new();
        for (_, ground) in prepared.allowed.keys() {
            *diggers.entry(*ground).or_default() += 1;
        }
        if diggers.values().any(|count| *count > 1) {
            return None;
        }
        let mut hinges = Vec::new();
        for (index, target) in input.grade_targets.iter().enumerate() {
            let Some(p) = prepared.days.iter().position(|day| *day == target.day) else { continue };
            for (hinge, (boundary, direction, slope)) in target.specification.hinges().into_iter().enumerate() {
                if slope != 0.0 {
                    hinges.push(Hinge {
                        name: format!("grade_{index}_{hinge}"),
                        destination: target.destination,
                        p,
                        grade: target.grade,
                        boundary,
                        direction,
                        slope: slope * UNIT,
                    });
                }
            }
        }
        let mut hinges_at: BTreeMap<(DestinationId, usize), Vec<usize>> = BTreeMap::new();
        for (index, hinge) in hinges.iter().enumerate() {
            hinges_at.entry((hinge.destination, hinge.p)).or_default().push(index);
        }
        let mut budgets = BTreeMap::new();
        let mut rooms = BTreeMap::new();
        for entry in &input.destinations {
            match entry.kind {
                DestinationKind::Crusher => {
                    for (p, day) in prepared.days.iter().enumerate() {
                        if let Some(Some(budget)) = entry.crusher_daily_t.get(*day as usize) {
                            budgets.insert((entry.id, p), budget / UNIT);
                        }
                    }
                }
                DestinationKind::Dump => {
                    if let Some(capacity) = entry.capacity_t {
                        rooms.insert(entry.id, (capacity - prepared.before(entry.id)).max(0.0) / UNIT);
                    }
                }
                DestinationKind::Stockpile(_) => {}
            }
        }
        let mut hours = BTreeMap::new();
        for source in &prepared.reclaims {
            for (p, cap) in source.caps.iter().enumerate() {
                if *cap > 0.0 {
                    hours.insert((source.loader, p), prepared.loader_hours(source.loader, p));
                }
            }
        }
        let piles = input
            .piles
            .iter()
            .map(|pile| {
                let opening = prepared.pile_opening.get(&pile.id).map_or(0.0, |(tonnes, _)| *tonnes) / UNIT;
                let deliveries = input
                    .destinations
                    .iter()
                    .filter(|entry| entry.kind == DestinationKind::Stockpile(pile.id))
                    .map(|entry| entry.id)
                    .collect();
                (pile.id, pile.capacity_t / UNIT, opening, deliveries)
            })
            .collect();
        let chains = prepared
            .chains
            .iter()
            .enumerate()
            .map(|(loader, chain)| Chain {
                loader,
                blocks: chain
                    .iter()
                    .map(|(ground, _)| {
                        let days = prepared.allowed.get(&(loader, *ground)).map(|list| list.iter().copied().collect()).unwrap_or_default();
                        (*ground, prepared.tonnes[ground], days)
                    })
                    .collect(),
            })
            .collect();
        Some(Self {
            prepared,
            hinges,
            hinges_at,
            budgets,
            rooms,
            fleet: (0..periods).map(|p| prepared.fleet_hours(p)).collect(),
            hours,
            piles,
            chains,
        })
    }

    fn zero(&self) -> Prices {
        Prices {
            fleet: vec![0.0; self.fleet.len()],
            crusher: self.budgets.keys().map(|key| (*key, 0.0)).collect(),
            dump: self.rooms.keys().map(|key| (*key, 0.0)).collect(),
            grade: vec![0.0; self.hinges.len()],
            pile: self.piles.iter().flat_map(|(pile, ..)| (0..self.fleet.len()).map(move |p| ((*pile, p), 0.0))).collect(),
            hours: self.hours.keys().map(|key| (*key, 0.0)).collect(),
        }
    }

    /// The relaxation's prices, `sign` setting which way round its
    /// inventory prices are read; the others are prices of rows that only
    /// limit, so their size is all that is taken.
    fn priced_at(&self, duals: &BTreeMap<String, f64>, sign: f64) -> Prices {
        let mut prices = self.zero();
        let read = |name: String| duals.get(&name).copied().unwrap_or(0.0);
        for (p, price) in prices.fleet.iter_mut().enumerate() {
            *price = read(format!("fleet_{p}")).abs();
        }
        for ((id, p), price) in &mut prices.crusher {
            *price = read(format!("crush_{}_{p}", id.0)).abs();
        }
        for (id, price) in &mut prices.dump {
            *price = read(format!("room_{}", id.0)).abs();
        }
        for (hinge, price) in self.hinges.iter().zip(&mut prices.grade) {
            *price = read(hinge.name.clone()).abs().min(hinge.slope);
        }
        for ((pile, p), price) in &mut prices.pile {
            *price = sign * read(format!("inv_{}_{p}", pile.0));
        }
        for ((loader, p), price) in &mut prices.hours {
            *price = read(format!("hours_{loader}_{p}")).abs();
        }
        prices
    }

    /// What delivering a kilotonne to `destination` on day `p` costs at
    /// `prices`, carrying `of`'s grade with `truck_h` truck hours.
    fn delivery(&self, prices: &Prices, destination: DestinationId, p: usize, of: Carried, truck_h: f64) -> f64 {
        let mut cost = prices.fleet[p] * truck_h;
        cost += prices.crusher.get(&(destination, p)).copied().unwrap_or(0.0);
        cost += prices.dump.get(&destination).copied().unwrap_or(0.0);
        for &index in self.hinges_at.get(&(destination, p)).into_iter().flatten() {
            let hinge = &self.hinges[index];
            cost += prices.grade[index] * hinge.direction * (self.prepared.carried(of, hinge.grade, hinge.direction) - hinge.boundary);
        }
        if let Some(DestinationKind::Stockpile(pile)) = self.prepared.destination(destination).map(|entry| entry.kind) {
            cost -= prices.pile.get(&(pile, p)).copied().unwrap_or(0.0);
        }
        cost
    }

    /// The Lagrangian's value at `prices` and a subgradient there.
    fn evaluate(&self, prices: &Prices) -> (f64, Slacks) {
        let prepared = self.prepared;
        let periods = prepared.periods();
        let mut value = 0.0;
        let mut slack = self.zero();
        // The priced rows' right sides.
        for (p, hours) in self.fleet.iter().enumerate() {
            value += prices.fleet[p] * hours;
            slack.fleet[p] = *hours;
        }
        for (key, budget) in &self.budgets {
            value += prices.crusher[key] * budget;
            slack.crusher.insert(*key, *budget);
        }
        for (id, room) in &self.rooms {
            value += prices.dump[id] * room;
            slack.dump.insert(*id, *room);
        }
        for (key, (_, _, hours)) in &self.hours {
            value += prices.hours[key] * hours;
            slack.hours.insert(*key, *hours);
        }
        for (pile, _, opening, _) in &self.piles {
            value += prices.pile[&(*pile, 0)] * opening;
            *slack.pile.get_mut(&(*pile, 0)).expect("every pile day is priced") += opening;
        }
        // A delivery's left-side terms.
        let deliver = |slack: &mut Slacks, destination: DestinationId, p: usize, of: Carried, truck_h: f64, tonnes: f64| {
            slack.fleet[p] -= truck_h * tonnes;
            if let Some(entry) = slack.crusher.get_mut(&(destination, p)) {
                *entry -= tonnes;
            }
            if let Some(entry) = slack.dump.get_mut(&destination) {
                *entry -= tonnes;
            }
            for &index in self.hinges_at.get(&(destination, p)).into_iter().flatten() {
                let hinge = &self.hinges[index];
                slack.grade[index] -= hinge.direction * (prepared.carried(of, hinge.grade, hinge.direction) - hinge.boundary) * tonnes;
            }
            if let Some(DestinationKind::Stockpile(pile)) = prepared.destination(destination).map(|entry| entry.kind)
                && let Some(entry) = slack.pile.get_mut(&(pile, p))
            {
                *entry += tonnes;
            }
        };

        // ---- each loader's sequence --------------------------------------------
        for chain in &self.chains {
            // Per day, each block's best value per kilotonne: each material
            // to its best destination open that day, less the loader's time.
            let mut best: Densities = vec![Vec::new(); periods];
            for (p, row) in best.iter_mut().enumerate() {
                let time = self
                    .hours
                    .get(&(chain.loader, p))
                    .filter(|(dig, ..)| *dig > 0.0)
                    .map_or(0.0, |(dig, ..)| prices.hours[&(chain.loader, p)] * UNIT / dig);
                for (ground, _, days) in &chain.blocks {
                    if !days.contains(&p) {
                        row.push(None);
                        continue;
                    }
                    let mut density = -time;
                    let mut routes = Vec::new();
                    let mut open = true;
                    for (material, fraction) in prepared.materials(*ground) {
                        let choice = prepared.outlets[&(*ground, material)]
                            .iter()
                            .filter(|outlet| prepared.building(outlet.destination, p))
                            .map(|outlet| {
                                (
                                    outlet.value - self.delivery(prices, outlet.destination, p, Carried::Material(material), outlet.truck_h),
                                    outlet,
                                )
                            })
                            .max_by(|a, b| a.0.total_cmp(&b.0));
                        let Some((net, outlet)) = choice else {
                            open = false;
                            break;
                        };
                        density += fraction * net;
                        routes.push((outlet.destination, material, fraction, outlet.truck_h));
                    }
                    row.push(open.then_some((density, routes)));
                }
            }
            let (worth, path) = progress(chain, &prepared.capacity[chain.loader], &best);
            value += worth;
            // Its digs' terms, block by block and day by day.
            let starts: Vec<f64> = chain
                .blocks
                .iter()
                .scan(0.0, |at, (_, length, _)| {
                    let start = *at;
                    *at += length;
                    Some(start)
                })
                .collect();
            let mut previous: f64 = 0.0;
            for (p, reached) in path.iter().enumerate() {
                let mut dug_today = 0.0;
                for (k, (_, length, _)) in chain.blocks.iter().enumerate() {
                    let tonnes = (reached.min(starts[k] + length) - previous.max(starts[k])).max(0.0);
                    if tonnes <= 0.0 {
                        continue;
                    }
                    dug_today += tonnes;
                    if let Some((_, routes)) = &best[p][k] {
                        for (destination, material, fraction, truck_h) in routes {
                            deliver(&mut slack, *destination, p, Carried::Material(*material), *truck_h, fraction * tonnes);
                        }
                    }
                }
                if let Some((dig, ..)) = self.hours.get(&(chain.loader, p)).filter(|(dig, ..)| *dig > 0.0) {
                    *slack.hours.get_mut(&(chain.loader, p)).expect("priced") -= dug_today * UNIT / dig;
                }
                previous = *reached;
            }
        }

        // ---- reclaim bars ------------------------------------------------------
        let mut by_task: BTreeMap<usize, Vec<(f64, f64, usize, usize)>> = BTreeMap::new();
        for (index, source) in prepared.reclaims.iter().enumerate() {
            for (p, cap) in source.caps.iter().enumerate() {
                if *cap <= 0.0 {
                    continue;
                }
                let route = source
                    .routes
                    .iter()
                    .enumerate()
                    .filter(|(_, route)| prepared.building(route.destination, p))
                    .map(|(at, route)| (route.value - self.delivery(prices, route.destination, p, Carried::Pile(source.pile), route.truck_h), at))
                    .max_by(|a, b| a.0.total_cmp(&b.0));
                let Some((net, at)) = route else { continue };
                let time = self
                    .hours
                    .get(&(source.loader, p))
                    .map_or(0.0, |(_, reclaim, _)| prices.hours[&(source.loader, p)] * UNIT / reclaim.max(1e-9));
                let net = net - prices.pile[&(source.pile, p)] - time;
                if net > 0.0 {
                    by_task.entry(source.task).or_default().push((net, *cap, index, at * periods + p));
                }
            }
        }
        for (task, mut items) in by_task {
            let mut left = match prepared.input.tasks[task].kind {
                TaskKind::Reclaim { maximum_t: Some(maximum), .. } => (maximum - prepared.reclaimed_before(task)).max(0.0) / UNIT,
                _ => f64::INFINITY,
            };
            items.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (net, cap, index, code) in items {
                if left <= 0.0 {
                    break;
                }
                let tonnes = cap.min(left);
                left -= tonnes;
                value += net * tonnes;
                let source = &prepared.reclaims[index];
                let (at, p) = (code / periods, code % periods);
                let route = &source.routes[at];
                deliver(&mut slack, route.destination, p, Carried::Pile(source.pile), route.truck_h, tonnes);
                *slack.pile.get_mut(&(source.pile, p)).expect("priced") -= tonnes;
                if let Some((_, reclaim, _)) = self.hours.get(&(source.loader, p)) {
                    *slack.hours.get_mut(&(source.loader, p)).expect("priced") -= tonnes * UNIT / reclaim.max(1e-9);
                }
            }
        }

        // ---- pile inventories --------------------------------------------------
        for (pile, capacity, ..) in &self.piles {
            for p in 0..periods {
                let next = if p + 1 < periods { prices.pile[&(*pile, p + 1)] } else { 0.0 };
                if next - prices.pile[&(*pile, p)] > 0.0 {
                    value += (next - prices.pile[&(*pile, p)]) * capacity;
                    *slack.pile.get_mut(&(*pile, p)).expect("priced") -= capacity;
                    if p + 1 < periods {
                        *slack.pile.get_mut(&(*pile, p + 1)).expect("priced") += capacity;
                    }
                }
            }
        }
        (value, slack)
    }

    /// Prices moved against the subgradient by `step`, kept where they may be.
    fn stepped(&self, prices: &Prices, slack: &Slacks, step: f64) -> Prices {
        let mut next = prices.clone();
        for (price, g) in next.fleet.iter_mut().zip(&slack.fleet) {
            *price = (*price - step * g).max(0.0);
        }
        for (key, price) in &mut next.crusher {
            *price = (*price - step * slack.crusher[key]).max(0.0);
        }
        for (key, price) in &mut next.dump {
            *price = (*price - step * slack.dump[key]).max(0.0);
        }
        for ((price, g), hinge) in next.grade.iter_mut().zip(&slack.grade).zip(&self.hinges) {
            *price = (*price - step * g).clamp(0.0, hinge.slope);
        }
        for (key, price) in &mut next.pile {
            *price -= step * slack.pile[key];
        }
        for (key, price) in &mut next.hours {
            *price = (*price - step * slack.hours[key]).max(0.0);
        }
        next
    }
}

/// The best progress along `chain` at `best`'s values: its worth, with the
/// grid's allowance added, and how far along the sequence it is at the end
/// of each day, in kilotonnes.
fn progress(chain: &Chain, capacity: &[f64], best: &Densities) -> (f64, Vec<f64>) {
    let total: f64 = chain.blocks.iter().map(|(_, length, _)| length).sum();
    let periods = capacity.len();
    if total <= 0.0 {
        return (0.0, vec![0.0; periods]);
    }
    let cells = CELLS;
    let cell = total / cells as f64;
    // Block boundaries, and which block each cell's points fall in.
    let ends: Vec<f64> = chain
        .blocks
        .iter()
        .scan(0.0, |at, (_, length, _)| {
            *at += length;
            Some(*at)
        })
        .collect();
    let mut value = vec![f64::NEG_INFINITY; cells + 1];
    value[0] = 0.0;
    let mut from: Vec<Vec<u32>> = Vec::with_capacity(periods);
    let mut allowance = 0.0;
    for p in 0..periods {
        let densities: Vec<Option<f64>> = best[p].iter().map(|entry| entry.as_ref().map(|(density, _)| *density)).collect();
        allowance += 2.0 * cell * densities.iter().flatten().fold(0.0_f64, |most, density| most.max(density.abs()));
        // The value of digging from the start to each grid point today.
        let mut gained = vec![0.0; cells + 1];
        let mut block = 0;
        for (j, slot) in gained.iter_mut().enumerate().skip(1) {
            let (left, right) = ((j - 1) as f64 * cell, j as f64 * cell);
            let mut through = 0.0;
            let mut at = left;
            while at < right - 1e-12 {
                while block + 1 < ends.len() && ends[block] <= at + 1e-12 {
                    block += 1;
                }
                let upto = right.min(ends[block]);
                through += (upto - at) * densities[block].unwrap_or(0.0);
                at = upto;
                if block + 1 >= ends.len() {
                    break;
                }
            }
            *slot = through;
        }
        for j in 1..=cells {
            gained[j] += gained[j - 1];
        }
        // A cell may be dug today when any block in it may.
        let open: Vec<bool> = (0..=cells)
            .map(|j| {
                if j == 0 {
                    return false;
                }
                let (left, right) = ((j - 1) as f64 * cell, j as f64 * cell);
                let first = ends.partition_point(|end| *end <= left + 1e-12);
                let last = ends.partition_point(|end| *end < right - 1e-12).min(ends.len() - 1);
                (first..=last.max(first)).any(|k| densities.get(k).is_some_and(Option::is_some))
            })
            .collect();
        let reach = if capacity[p] > 0.0 { (capacity[p] / cell).ceil() as usize } else { 0 };
        let mut next = vec![f64::NEG_INFINITY; cells + 1];
        let mut pointers = vec![0_u32; cells + 1];
        let mut window: VecDeque<usize> = VecDeque::new();
        let mut run_start = 0;
        for j in 0..=cells {
            if j > 0 && !open[j] {
                run_start = j;
            }
            let lowest = j.saturating_sub(reach).max(run_start);
            // Bring in j as a place to have come from.
            if value[j] > f64::NEG_INFINITY {
                let worth = value[j] - gained[j];
                while window.back().is_some_and(|&i| value[i] - gained[i] <= worth) {
                    window.pop_back();
                }
                window.push_back(j);
            }
            while window.front().is_some_and(|&i| i < lowest) {
                window.pop_front();
            }
            if let Some(&i) = window.front() {
                next[j] = gained[j] + value[i] - gained[i];
                pointers[j] = i as u32;
            }
        }
        value = next;
        from.push(pointers);
    }
    let (mut j, worth) = value
        .iter()
        .enumerate()
        .fold((0, f64::NEG_INFINITY), |held, (j, v)| if *v > held.1 { (j, *v) } else { held });
    let mut path = vec![0.0; periods];
    for p in (0..periods).rev() {
        path[p] = (j as f64 * cell).min(total);
        j = from[p][j] as usize;
    }
    (worth + allowance, path)
}

/// The Lagrangian bound on `prepared`'s plan of the whole horizon, from
/// the relaxation's `duals`, aiming its steps at `best`, a schedule's value,
/// within `limit`. `None` when the plan does not split by loader.
pub(crate) fn bound(prepared: &Prepared, duals: &BTreeMap<String, f64>, best: f64, limit: Duration, stop: &AtomicBool) -> Option<f64> {
    if !prepared.optimistic {
        return None;
    }
    let started = Instant::now();
    let lagrangian = Lagrangian::new(prepared)?;
    // Which way round the relaxation reports its inventory prices is the
    // solver's convention: the reading that bounds lower is the right one.
    let mut prices = lagrangian.priced_at(duals, 1.0);
    let (mut value, mut slack) = lagrangian.evaluate(&prices);
    let flipped = lagrangian.priced_at(duals, -1.0);
    let (other, other_slack) = lagrangian.evaluate(&flipped);
    if other < value {
        (prices, value, slack) = (flipped, other, other_slack);
    }
    let mut lowest = value;
    let (mut scale, mut stalled) = (1.0, 0);
    while scale > STEP_FLOOR && started.elapsed() < limit && !stop.load(Ordering::Relaxed) {
        let norm = slack.norm();
        if norm <= 0.0 {
            break;
        }
        let step = scale * (value - best).max(1e-9 * value.abs()) / norm;
        prices = lagrangian.stepped(&prices, &slack, step);
        (value, slack) = lagrangian.evaluate(&prices);
        if value < lowest - 1e-9 * lowest.abs() {
            lowest = value;
            stalled = 0;
        } else {
            stalled += 1;
            if stalled >= STALL {
                scale /= 2.0;
                stalled = 0;
            }
        }
    }
    Some(lowest)
}
