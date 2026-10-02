use std::collections::{BTreeMap, BinaryHeap};

use glam::DVec3;

use super::network::{HaulNetwork, NodeId, RoadId, RoadIndex, grade};
use crate::model::schedule::trucking::{CycleBreakdown, TruckClass};

#[derive(Clone, Debug, Default)]
struct Leg {
    hours: f64,
    km: f64,
    rise: f64,
    first_speed: f64,
    last_speed: f64,
    points: Vec<DVec3>,
    samples: Vec<[f64; 3]>,
}
impl Leg {
    fn append(&mut self, other: &Self) {
        if self.km == 0.0 {
            self.first_speed = other.first_speed;
        }
        if other.km > 0.0 {
            self.last_speed = other.last_speed;
        }
        let offset = self.km * 1000.0;
        self.samples.extend(other.samples.iter().map(|s| [offset + s[0], s[1], s[2]]));
        self.hours += other.hours;
        self.km += other.km;
        self.rise += other.rise;
        self.points.extend(other.points.iter().copied().skip(usize::from(!self.points.is_empty())));
    }
}

fn travel(points: &[DVec3], class: &TruckClass, loaded: bool, limit: Option<f64>) -> Leg {
    let mut leg = Leg {
        points: points.to_vec(),
        ..Leg::default()
    };
    for pair in points.windows(2) {
        let length = pair[0].distance(pair[1]);
        if length <= 1e-9 {
            continue;
        }
        let speed = class.speed(grade(pair[0], pair[1]), loaded, limit);
        if leg.km == 0.0 {
            leg.first_speed = speed;
        }
        leg.last_speed = speed;
        leg.samples.push([leg.km * 1000.0, pair[0].z, speed]);
        leg.samples.push([leg.km * 1000.0 + length, pair[1].z, speed]);
        leg.km += length / 1000.0;
        leg.hours += length / 1000.0 / speed;
        leg.rise += (pair[1].z - pair[0].z).max(0.0);
    }
    leg
}
fn loss(speed: f64, network: &HaulNetwork) -> f64 {
    speed / (2.0 * network.settings.acceleration_kph_s) / 3600.0
}

#[derive(Clone, Copy, Debug)]
struct Queue {
    time: f64,
    node: NodeId,
}
impl PartialEq for Queue {
    fn eq(&self, other: &Self) -> bool {
        self.time.to_bits() == other.time.to_bits() && self.node == other.node
    }
}
impl Eq for Queue {}
impl PartialOrd for Queue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Queue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.time.total_cmp(&self.time).then_with(|| self.node.cmp(&other.node))
    }
}

/// A destination search stores one predecessor per node, rather than copying
/// full routes into every block's candidate. Loaded searches traverse the
/// reversed graph; empty searches traverse the forward graph.
struct Search {
    times: BTreeMap<NodeId, f64>,
    next: BTreeMap<NodeId, (Option<NodeId>, Leg)>,
    outward: bool,
}
impl Search {
    fn new(network: &HaulNetwork, class: &TruckClass, target: (RoadId, usize, DVec3), outward: bool) -> Self {
        let mut search = Self {
            times: BTreeMap::new(),
            next: BTreeMap::new(),
            outward,
        };
        let road = network.road(target.0).expect("index road");
        let mut queue = BinaryHeap::new();
        for (node, points) in connectors(network, road, target.1, target.2) {
            let points = if outward { points } else { points.into_iter().rev().collect() };
            let leg = travel(&points, class, !outward, road.speed_limit_kph);
            let end_speed = if outward { leg.first_speed } else { leg.last_speed };
            let time = leg.hours + loss(end_speed, network);
            search.times.insert(node, time);
            search.next.insert(node, (None, leg));
            queue.push(Queue { time, node });
        }
        let mut adjacency: BTreeMap<NodeId, Vec<(NodeId, Leg)>> = BTreeMap::new();
        for r in &network.roads {
            let forward = network.points(r);
            let reverse: Vec<_> = forward.iter().rev().copied().collect();
            for (from, to, points) in [(r.from, r.to, forward), (r.to, r.from, reverse)] {
                let leg = travel(&points, class, !outward, r.speed_limit_kph);
                let (key, neighbor) = if outward { (from, to) } else { (to, from) };
                adjacency.entry(key).or_default().push((neighbor, leg));
            }
        }
        while let Some(Queue { time, node }) = queue.pop() {
            if time > search.times[&node] + 1e-12 {
                continue;
            }
            for (neighbor, leg) in adjacency.get(&node).into_iter().flatten() {
                let terminal_loss = if search.next.get(&node).is_some_and(|(next, seed)| next.is_none() && seed.km <= 1e-9) {
                    loss(if outward { leg.first_speed } else { leg.last_speed }, network)
                } else {
                    0.0
                };
                let cost = time + leg.hours + terminal_loss;
                if cost < *search.times.get(neighbor).unwrap_or(&f64::INFINITY) {
                    search.times.insert(*neighbor, cost);
                    search.next.insert(*neighbor, (Some(node), leg.clone()));
                    queue.push(Queue { time: cost, node: *neighbor });
                }
            }
        }
        search
    }
    fn path(&self, mut node: NodeId) -> Option<Leg> {
        let total = *self.times.get(&node)?;
        let mut pieces = Vec::new();
        loop {
            let (next, leg) = self.next.get(&node)?;
            pieces.push(leg);
            if let Some(next) = next {
                node = *next;
            } else {
                break;
            }
        }
        if self.outward {
            pieces.reverse();
        }
        let mut path = Leg::default();
        for piece in pieces {
            path.append(piece);
        }
        path.hours = total;
        Some(path)
    }
}

/// Projection -> each endpoint, retaining every intervening shape point.
fn connectors(network: &HaulNetwork, road: &super::HaulRoad, segment: usize, point: DVec3) -> [(NodeId, Vec<DVec3>); 2] {
    let points = network.points(road);
    let left = std::iter::once(point).chain(points[..=segment].iter().rev().copied()).collect();
    let right = std::iter::once(point).chain(points[segment + 1..].iter().copied()).collect();
    [(road.from, left), (road.to, right)]
}

#[derive(Clone, Debug)]
pub(crate) struct RouteCheck {
    pub(crate) cycle: CycleBreakdown,
    pub(crate) loaded_path: Vec<DVec3>,
    pub(crate) empty_path: Vec<DVec3>,
    pub(crate) connected: bool,
    pub(crate) access_m: f64,
    pub(crate) access_rise_m: f64,
    pub(crate) grade_lengthened: bool,
    pub(crate) profile: Vec<[f64; 3]>,
    pub(crate) uses_roads: bool,
}

pub(crate) struct DestinationSearch<'a> {
    network: &'a HaulNetwork,
    class: &'a TruckClass,
    target: (RoadId, usize, DVec3),
    /// The destination itself, when it sits off the road it joins.
    destination: DVec3,
    loaded: Search,
    empty: Search,
}
impl<'a> DestinationSearch<'a> {
    pub(crate) fn new(network: &'a HaulNetwork, index: &RoadIndex, class: &'a TruckClass, destination: DVec3) -> Option<Self> {
        if network
            .nodes
            .iter()
            .any(|n| n.pos.distance(destination) < 1e-6 && !network.roads.iter().any(|r| r.from == n.id || r.to == n.id))
        {
            return None;
        }
        let target = index.candidates(destination, 0.0).first().copied()?;
        Some(Self {
            network,
            class,
            target,
            destination,
            loaded: Search::new(network, class, target, false),
            empty: Search::new(network, class, target, true),
        })
    }
    fn road_leg(&self, join: (RoadId, usize, DVec3), outward: bool) -> Option<Leg> {
        let road = self.network.road(join.0)?;
        let mut best: Option<Leg> = None;
        // A source and destination on the same road can travel directly,
        // without an artificial detour through either endpoint.
        if join.0 == self.target.0 {
            let points = self.network.points(road);
            let (lo, hi, forward) = if join.1 <= self.target.1 {
                (join, self.target, true)
            } else {
                (self.target, join, false)
            };
            let mut direct: Vec<_> = std::iter::once(lo.2).chain(points[lo.1 + 1..=hi.1].iter().copied()).chain(std::iter::once(hi.2)).collect();
            if forward == outward {
                direct.reverse();
            }
            let mut leg = travel(&direct, self.class, !outward, road.speed_limit_kph);
            leg.hours += loss(if outward { leg.first_speed } else { leg.last_speed }, self.network);
            best = Some(leg);
        }
        for (node, points) in connectors(self.network, road, join.1, join.2) {
            let search = if outward { &self.empty } else { &self.loaded };
            let Some(mut path) = search.path(node) else { continue };
            let points: Vec<_> = if outward { points.into_iter().rev().collect() } else { points };
            let connector = travel(&points, self.class, !outward, road.speed_limit_kph);
            if outward {
                path.append(&connector);
            } else {
                let mut front = connector;
                front.append(&path);
                path = front;
            }
            if best.as_ref().is_none_or(|best| path.hours < best.hours) {
                best = Some(path);
            }
        }
        best
    }
    /// Length and rise of the leg from the joined road to the destination.
    fn destination_access(&self) -> (f64, f64) {
        let rise = self.destination.z - self.target.2.z;
        (self.destination.distance(self.target.2).max(rise.abs() / self.class.maximum_grade), rise)
    }
    /// Both searches are reused for all blocks/materials for this class and
    /// destination. Only the short access candidates are evaluated per block.
    pub(crate) fn route(&self, index: &RoadIndex, source: DVec3, bench_access: bool, loader_rate: f64, spot_s: f64, dump_s: Option<f64>) -> Option<RouteCheck> {
        let mut best: Option<RouteCheck> = None;
        for join in index.candidates(source, self.network.settings.auto_join_m) {
            let Some(mut loaded) = self.road_leg(join, false) else { continue };
            let Some(mut empty) = self.road_leg(join, true) else { continue };
            let direct = source.distance(join.2);
            let rise = join.2.z - source.z;
            let access_m = direct.max(rise.abs() / self.class.maximum_grade);
            let effective_grade = if access_m > 0.0 { rise / access_m } else { 0.0 };
            let cap = bench_access.then_some(self.network.settings.bench_speed_kph);
            let loaded_speed = self.class.speed(effective_grade, true, cap);
            let empty_speed = self.class.speed(-effective_grade, false, cap);
            loaded.hours += access_m / 1000.0 / loaded_speed + loss(if access_m > 1e-9 { loaded_speed } else { loaded.first_speed }, self.network);
            empty.hours += access_m / 1000.0 / empty_speed + loss(if access_m > 1e-9 { empty_speed } else { empty.last_speed }, self.network);
            loaded.km += access_m / 1000.0;
            empty.km += access_m / 1000.0;
            loaded.rise += rise.max(0.0);
            let mut profile = vec![[0.0, source.z, loaded_speed], [access_m, join.2.z, loaded_speed]];
            profile.extend(loaded.samples.iter().map(|s| [access_m + s[0], s[1], s[2]]));
            loaded.points.insert(0, source);
            empty.points.push(source);
            // A destination off its road (a dump's surface centre) is reached
            // by the same grade-limited straight leg as a block.
            let (off_m, off_rise) = self.destination_access();
            if off_m > 1e-9 {
                let off_grade = off_rise / off_m;
                let in_speed = self.class.speed(off_grade, true, None);
                let out_speed = self.class.speed(-off_grade, false, None);
                loaded.hours += off_m / 1000.0 / in_speed;
                empty.hours += off_m / 1000.0 / out_speed;
                loaded.km += off_m / 1000.0;
                empty.km += off_m / 1000.0;
                loaded.rise += off_rise.max(0.0);
                let travelled = profile.last().map_or(0.0, |p| p[0]);
                profile.push([travelled, self.target.2.z, in_speed]);
                profile.push([travelled + off_m, self.destination.z, in_speed]);
                loaded.points.push(self.destination);
                empty.points.insert(0, self.destination);
            }
            let cycle = CycleBreakdown {
                spot_h: spot_s / 3600.0,
                load_h: if loader_rate > 0.0 { self.class.payload_t / loader_rate } else { 0.0 },
                loaded_h: loaded.hours,
                dump_h: dump_s.unwrap_or(self.class.dump_time_s) / 3600.0,
                empty_h: empty.hours,
                loaded_km: loaded.km,
                empty_km: empty.km,
                rise_m: loaded.rise,
            };
            let lengthened = access_m > direct + 1e-6;
            let check = RouteCheck {
                cycle,
                loaded_path: loaded.points,
                empty_path: empty.points,
                // Connected means a road within reach; a grade-lengthened leg
                // still costs its full length but is not a missing road.
                connected: access_m <= self.network.settings.auto_join_m,
                access_m,
                access_rise_m: rise,
                grade_lengthened: lengthened,
                profile,
                uses_roads: true,
            };
            if best.as_ref().is_none_or(|best| cycle.total_h() < best.cycle.total_h()) {
                best = Some(check);
            }
        }
        best
    }
}
