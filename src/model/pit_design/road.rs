//! One bench's stretch of ramp: a road laid along a ring, climbing (or
//! falling) one bench at its gradient while it drifts across the bench.
//!
//! The ramp crosses each bench face on a slant. Where it leaves the ring it
//! starts from, its edge by that ring is on it; where it has climbed the
//! bench it has moved across by a face and a berm, so the next bench's
//! stretch carries straight on from the ring after. At each point in
//! between it has moved across by that share of the face and berm, so the
//! face below it and the face above it keep their angle all the way along.

use glam::DVec2;

use super::ring::{self, SAME_POINT};
use crate::model::kernel;

/// The most distance between the points a road's edges are worked out at.
const STATION_SPACING: f64 = 4.0;

/// A road's two edges, each point with where along the ring it was set off
/// from and how far up the stretch it is (0 where it leaves, 1 where it
/// arrives).
#[derive(Clone, Debug)]
pub(crate) struct RoadEdges {
    /// The edge by the ring it was laid along: the pit side when building
    /// up, the wall side when building down.
    pub(crate) near: Vec<(DVec2, f64)>,
    pub(crate) far: Vec<(DVec2, f64)>,
}

/// One bench's stretch of road, laid along a ring.
#[derive(Clone, Debug)]
pub(crate) struct Stretch {
    pub(crate) edges: RoadEdges,
    /// Where it leaves and arrives, as parameters of the ring.
    pub(crate) from: f64,
    pub(crate) to: f64,
    /// Which way round the ring it runs: 1 with the ring's order
    /// (anticlockwise), -1 against.
    pub(crate) direction: f64,
    /// How far up the stretch the ring is at each of `stations`' parameters
    /// (each 0..1, in the order the road runs).
    pub(crate) stations: Vec<(f64, f64)>,
    /// How many points the ring it was laid along has.
    pub(crate) count: usize,
    /// How far out its far edge is set from its near edge: a little more
    /// than its width, as its edges lean off the ring.
    pub(crate) across: f64,
}

impl Stretch {
    /// How far up the stretch the road is beside a parameter of the ring
    /// it was laid along, or `None` where it is not beside it.
    pub(crate) fn climbed_at(&self, parameter: f64) -> Option<f64> {
        let along = |at: f64| along_road(at, self.from, self.direction, self.count);
        let span = along(self.to);
        let here = along(parameter);
        if here > span + 1.0e-9 {
            return None;
        }
        let index = self.stations.partition_point(|(station, _)| along(*station) <= here);
        if index == 0 {
            return Some(self.stations[0].1);
        }
        if index >= self.stations.len() {
            return Some(1.0);
        }
        let (a, b) = (self.stations[index - 1], self.stations[index]);
        let (from, to) = (along(a.0), along(b.0));
        let t = if to > from { (here - from) / (to - from) } else { 1.0 };
        Some(a.1 + (b.1 - a.1) * t.clamp(0.0, 1.0))
    }
}

/// How far along a road running `direction` from `from` a ring parameter
/// is, in parameters: never negative, so it only grows along the road.
fn along_road(parameter: f64, from: f64, direction: f64, count: usize) -> f64 {
    let along = ((parameter - from) * direction).rem_euclid(count as f64);
    // A hair short of a whole lap is the start itself, read through rounding.
    if count as f64 - along < 1.0e-9 { 0.0 } else { along }
}

/// What a stretch is laid with.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lay {
    /// Out from the ring (1) or in (-1).
    pub(crate) side: f64,
    /// How far the road moves across over the stretch where it leaves: a
    /// face and a berm. What leans its edges off the ring.
    pub(crate) drift: f64,
    /// Whether the walls vary along the stretch, so it drifts across faster
    /// in some places than others: its far edge is then its near edge set
    /// off square, rather than the ring set off further, which would fold
    /// where the ring bends.
    pub(crate) varies: bool,
    /// Where the far edge starts, where the walls vary: where the last
    /// stretch's far edge arrived.
    pub(crate) far_start: Option<DVec2>,
    pub(crate) width: f64,
    /// The plan length (along its steeper edge) it climbs the bench over.
    pub(crate) climb_length: f64,
    /// The plan length (along its steeper edge) it runs level before it
    /// starts to climb: a flat where it leaves, laid where the bench would
    /// not be sound with the climb starting at once.
    pub(crate) flat: f64,
}

/// How far up a stretch a point is that lies `running` along its steeper
/// edge of `total`, the first `flat` of it level.
fn share(running: f64, total: f64, flat: f64) -> f64 {
    if total <= flat {
        return if running >= total { 1.0 } else { 0.0 };
    }
    ((running - flat) / (total - flat)).clamp(0.0, 1.0)
}

/// Why a stretch could not be laid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoRoom {
    /// The ring is too short to climb the bench along at the gradient.
    TooShort,
}

/// Lay a stretch along `ring` from `from`, running `direction`, climbing
/// over `lay.climb_length` of its steeper edge. `reach` says how far out
/// from the ring the road's near edge is at each point, given how far up the
/// stretch it is there and the ring's parameter: its drift so far across the
/// bench, as wide there as the walls are cut, and anything of its own (a
/// switchback's island, which keeps the road clear of the one below until
/// it has drifted far enough of its own accord).
pub(crate) fn lay(ring: &[DVec2], from: f64, direction: f64, lay: Lay, reach: &dyn Fn(f64, f64) -> f64) -> Result<Stretch, NoRoom> {
    let arcs = ring::arcs(ring);
    let around = arcs[ring.len()];
    // No stretch takes the whole way round: it would meet its own start.
    let longest = around * 0.9;
    let attempt = |run: f64| build(ring, &arcs, from, direction, run, lay, reach);
    let wanted = lay.climb_length + lay.flat;
    let close_enough = wanted * 1.0e-4;
    // The climb rises with the run, nearly in proportion to it: round a
    // bend the inside edge is shorter than the ring, and round a corner the
    // road is level. So each try scales the run by how far the last fell
    // short or over, kept inside the runs known to be too short and too
    // long, halving between them if a guess strays outside.
    let (mut low, mut high) = (0.0, f64::INFINITY);
    let mut run = wanted.min(longest);
    let mut best: Option<Stretch> = None;
    for _ in 0..40 {
        let stretch = attempt(run);
        let climbed = stretch_length(&stretch);
        if climbed >= wanted {
            high = run;
            // The longer side: a hair flatter, never steeper.
            best = Some(stretch);
            if climbed - wanted <= close_enough {
                break;
            }
        } else {
            low = run;
            if run >= longest {
                return Err(NoRoom::TooShort);
            }
        }
        let scaled = if climbed > 0.0 { run * wanted / climbed } else { run * 2.0 };
        let next = if scaled > low && scaled < high.min(longest) {
            scaled
        } else if high.is_finite() {
            (low + high) * 0.5
        } else {
            (run * 2.0).min(longest)
        };
        if (next - run).abs() <= 1.0e-6 * run.max(1.0) {
            break;
        }
        run = next;
    }
    best.ok_or(NoRoom::TooShort)
}

/// The climb a stretch makes: the length its stations' share-of-climb was
/// worked out over, along the shorter edge at each step.
fn stretch_length(stretch: &Stretch) -> f64 {
    let parameters: Vec<f64> = stretch.stations.iter().map(|station| station.0).collect();
    let near = chain_lengths(&stretch.edges.near, &parameters, stretch.direction, stretch.count);
    let far = chain_lengths(&stretch.edges.far, &parameters, stretch.direction, stretch.count);
    near.iter().zip(&far).map(|(a, b)| a.min(*b)).sum()
}

/// Lay a stretch `run` along the ring from `from`.
fn build(ring: &[DVec2], arcs: &[f64], from: f64, direction: f64, run: f64, lay: Lay, reach: &dyn Fn(f64, f64) -> f64) -> Stretch {
    let count = ring.len();
    let start_arc = ring::arc_at(arcs, from);
    // Stations: the start, every ring point passed, the end, and enough in
    // between that the drift is followed smoothly along long edges.
    let mut along: Vec<f64> = vec![0.0];
    let around = arcs[count];
    for arc in &arcs[..count] {
        let corner = ((arc - start_arc) * direction).rem_euclid(around);
        if corner > SAME_POINT && corner < run - SAME_POINT {
            along.push(corner);
        }
    }
    let steps = (run / STATION_SPACING).ceil() as usize;
    along.extend((1..steps).map(|step| run * step as f64 / steps as f64));
    along.push(run);
    along.sort_by(f64::total_cmp);
    along.dedup_by(|a, b| (*a - *b).abs() <= SAME_POINT);
    let parameters: Vec<f64> = along.iter().map(|distance| ring::parameter_at(arcs, start_arc + direction * distance)).collect();

    // How far up the stretch each station is: first by distance along the
    // ring, then by the length the road's own edges climb over, which the
    // edges depend on in turn. A few rounds settle it.
    let flat_share = lay.flat / (lay.climb_length + lay.flat).max(f64::MIN_POSITIVE);
    let mut climbed: Vec<f64> = along.iter().map(|distance| share(*distance, run, run * flat_share)).collect();
    let mut edges = RoadEdges {
        near: Vec::new(),
        far: Vec::new(),
    };
    // The road drifts across as it climbs, so its edges lean off the ring;
    // set apart by a little more than the width, they are the width apart
    // square across.
    let across = lay.width * (1.0 + (lay.drift / lay.climb_length).powi(2)).sqrt();
    for _ in 0..3 {
        let near_distance: Vec<f64> = climbed.iter().zip(&parameters).map(|(&t, &parameter)| lay.side * reach(t, parameter)).collect();
        let near = offset_open(ring, &parameters, direction, &near_distance);
        let far = if lay.varies {
            beside(&near, lay.width, lay.side * direction > 0.0, (from, direction, count), lay.far_start)
        } else {
            let far_distance: Vec<f64> = near_distance.iter().map(|distance| distance + lay.side * across).collect();
            offset_open(ring, &parameters, direction, &far_distance)
        };
        edges = RoadEdges { near, far };
        let near = chain_lengths(&edges.near, &parameters, direction, count);
        let far = chain_lengths(&edges.far, &parameters, direction, count);
        let steps: Vec<f64> = near.iter().zip(&far).map(|(a, b)| a.min(*b)).collect();
        let total: f64 = steps.iter().sum();
        let flat = lay.flat.min(total);
        let mut running = 0.0;
        climbed = std::iter::once(0.0)
            .chain(steps.iter().map(|step| {
                running += step;
                if total > 0.0 { share(running, total, flat) } else { 1.0 }
            }))
            .collect();
    }
    // The heights the edges take: a station at every point of either edge,
    // so each piece of each lies between two stations, and the climb
    // between them is never more than either edge's own length there at the
    // gradient.
    let last = along_road(*parameters.last().unwrap_or(&from), from, direction, count);
    let last = if last <= 0.0 { count as f64 } else { last };
    let mut fine: Vec<f64> = parameters.iter().map(|parameter| along_road(*parameter, from, direction, count)).collect();
    if let Some(end) = fine.last_mut() {
        *end = last;
    }
    fine.extend(
        edges
            .near
            .iter()
            .chain(&edges.far)
            .map(|point| along_road(point.1, from, direction, count))
            .filter(|along| *along > 0.0 && *along < last),
    );
    fine.sort_by(f64::total_cmp);
    fine.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-12);
    let fine: Vec<f64> = fine.iter().map(|along| (from + direction * along).rem_euclid(count as f64)).collect();
    let near = chain_lengths(&edges.near, &fine, direction, count);
    let far = chain_lengths(&edges.far, &fine, direction, count);
    let steps: Vec<f64> = near.iter().zip(&far).map(|(a, b)| a.min(*b)).collect();
    let total: f64 = steps.iter().sum();
    let flat = lay.flat.min(total);
    let mut running = 0.0;
    let climbed: Vec<f64> = std::iter::once(0.0)
        .chain(steps.iter().map(|step| {
            running += step;
            if total > 0.0 { share(running, total, flat) } else { 1.0 }
        }))
        .collect();
    Stretch {
        edges,
        from,
        to: *parameters.last().unwrap_or(&from),
        direction,
        stations: fine.into_iter().zip(climbed).collect(),
        count,
        across,
    }
}

/// A road's far edge beside its near edge where the walls vary: the near
/// edge set off square by the road's `width`, out from the ring (`side`;
/// to the near edge's right where `right`), so it is its width across however
/// the near edge turns to cross the bench. Each point takes the ring
/// parameter of the place on the near edge it stands off from (round a ring
/// of `count` points). Where it carries on from the stretch before, it
/// starts where the last far edge arrived (`start`) and eases on to its
/// own line over a road's width along.
fn beside(near: &[(DVec2, f64)], width: f64, right: bool, (from, direction, count): (f64, f64, usize), start: Option<DVec2>) -> Vec<(DVec2, f64)> {
    let points: Vec<DVec2> = near.iter().map(|point| point.0).collect();
    let around = count as f64;
    // The far edge runs along the near one, so each point is looked for
    // near where the last was found; the run along the near edge to it is
    // what the ease from the handover goes by.
    let mut runs = vec![0.0; near.len()];
    for index in 1..near.len() {
        runs[index] = runs[index - 1] + near[index].0.distance(near[index - 1].0);
    }
    let mut found: usize = 0;
    let segments = near.len().saturating_sub(1);
    let mut far: Vec<(DVec2, f64, f64)> = offset_chain(&points, &vec![width; points.len()], right)
        .into_iter()
        .map(|point| {
            let mut best = (f64::INFINITY, near.first().map_or(0.0, |first| first.1), 0.0);
            let window = found.saturating_sub(BESIDE_BACK)..(found + BESIDE_AHEAD).min(segments);
            for range in [window, 0..segments] {
                for index in range {
                    let (a, b) = (near[index], near[index + 1]);
                    let (foot, t) = kernel::project_onto_segment(point, a.0, b.0);
                    let distance = foot.distance(point);
                    if distance < best.0 {
                        found = index;
                        // Between the two sources the short way round the ring.
                        let (from, mut to) = (a.1, b.1);
                        if to - from > around * 0.5 {
                            to -= around;
                        } else if from - to > around * 0.5 {
                            to += around;
                        }
                        let t = t.clamp(0.0, 1.0);
                        best = (distance, (from + (to - from) * t).rem_euclid(around), runs[index] + (runs[index + 1] - runs[index]) * t);
                    }
                }
                // Square off the near edge, it stands its width away: found.
                if best.0 <= width * 1.5 {
                    break;
                }
            }
            (point, best.1, best.2)
        })
        .collect();
    if let (Some(start), Some(first)) = (start, far.first()) {
        let shift = start - first.0;
        for point in &mut far {
            let share = 1.0 - point.2 / width;
            if share <= 0.0 {
                break;
            }
            point.0 += shift * share;
        }
    }
    // Each point's place on the ring never goes back the way the road came:
    // where the far edge cuts across inside a bend, the places it stands
    // off from would, and its climb be shared out wrong.
    let mut furthest = f64::NEG_INFINITY;
    let mut far: Vec<(DVec2, f64)> = without_reversals(remove_loops(far.into_iter().map(|(point, source, _)| (point, source)).collect()));
    for point in &mut far {
        let along = along_road(point.1, from, direction, count);
        if along < furthest {
            point.1 = (from + direction * furthest).rem_euclid(around);
        } else {
            furthest = along;
        }
    }
    far
}

/// How far back and ahead along the near edge a point of the far edge is
/// first looked for, from where the last one was found.
const BESIDE_BACK: usize = 8;
const BESIDE_AHEAD: usize = 32;

/// The length of a chain between each pair of stations: the chain's length
/// so far, as a function of how far along the ring its points were set off
/// from, read at each station. An arc round a corner of the ring is all set
/// off from the corner: the road is level round it, and it counts nothing.
fn chain_lengths(chain: &[(DVec2, f64)], parameters: &[f64], direction: f64, count: usize) -> Vec<f64> {
    let gaps = parameters.len().saturating_sub(1);
    if chain.len() < 2 || gaps == 0 {
        return vec![0.0; gaps];
    }
    let from = parameters[0];
    // The last station is the far end of the run; on a road that ends where
    // it began a lap later its parameter would read as zero.
    let last = {
        let end = along_road(parameters[gaps], from, direction, count);
        if end <= 0.0 { count as f64 } else { end }
    };
    let at = |parameter: f64| {
        let along = along_road(parameter, from, direction, count);
        if along > last + 1.0e-9 { 0.0 } else { along }
    };
    // How far along each chain point is, and the chain's length up to it.
    let mut along: Vec<f64> = Vec::with_capacity(chain.len());
    let mut length: Vec<f64> = Vec::with_capacity(chain.len());
    let mut total = 0.0;
    for (index, point) in chain.iter().enumerate() {
        let here = at(point.1);
        if index > 0 && here > along[index - 1] + 1.0e-12 {
            total += chain[index - 1].0.distance(point.0);
        }
        // Never backwards: a point cut off a loop can read a little behind.
        along.push(here.max(along.last().copied().unwrap_or(here)));
        length.push(total);
    }
    let length_at = |parameter: f64| -> f64 {
        let here = if parameter == parameters[gaps] { last } else { at(parameter) };
        let index = along.partition_point(|value| *value <= here);
        if index == 0 {
            return length[0];
        }
        if index >= along.len() {
            return total;
        }
        let (a, b) = (along[index - 1], along[index]);
        let t = if b > a { (here - a) / (b - a) } else { 1.0 };
        length[index - 1] + (length[index] - length[index - 1]) * t
    };
    (0..gaps).map(|gap| (length_at(parameters[gap + 1]) - length_at(parameters[gap])).max(0.0)).collect()
}

/// Offset an open run of a ring, the stations at `parameters` (in the order
/// the road runs, `direction`), each by its own distance out from the ring.
/// The offset of each ring edge is a straight line between its stations'
/// offsets; corners the offset opens are rounded, corners it closes run to
/// where the two offsets meet, and any loop that leaves is cut out. Each
/// point keeps the ring parameter it came from.
pub(crate) fn offset_open(ring: &[DVec2], parameters: &[f64], direction: f64, distances: &[f64]) -> Vec<(DVec2, f64)> {
    let count = ring.len();
    let edge_of = |index: usize| -> usize {
        // The ring edge a station's step to the next lies on: the edge
        // holding the middle of the step.
        let (a, b) = (parameters[index], parameters[index + 1]);
        let mut middle = (a + b) * 0.5;
        if (b - a).abs() > count as f64 * 0.5 {
            middle += count as f64 * 0.5;
        }
        (middle.rem_euclid(count as f64).floor() as usize).min(count - 1)
    };
    let mut raw: Vec<(DVec2, f64)> = Vec::with_capacity(parameters.len() * 2);
    let push = |point: DVec2, source: f64, raw: &mut Vec<(DVec2, f64)>| {
        if raw.last().is_none_or(|last: &(DVec2, f64)| last.0.distance(point) > SAME_POINT) {
            raw.push((point, source));
        }
    };
    // The last offset segment: its start, its end, the ring normal it was
    // set off along, and its distance at its end.
    let mut previous: Option<(DVec2, DVec2, DVec2, f64)> = None;
    for index in 0..parameters.len().saturating_sub(1) {
        let edge = edge_of(index);
        // Walking against the ring's order, the ring's outward side is on
        // the walk's left rather than its right; the normal is the ring's.
        let normal = ring::outward(ring, edge);
        let (a, b) = (ring::point_at(ring, parameters[index]), ring::point_at(ring, parameters[index + 1]));
        let (start, end) = (a + normal * distances[index], b + normal * distances[index + 1]);
        if let Some((previous_start, previous_end, previous_normal, previous_distance)) = previous
            && previous_end.distance(start) > SAME_POINT
            && (previous_distance - distances[index]).abs() <= SAME_POINT
        {
            // A corner of the ring: the offset either opens it (an arc
            // round the corner point) or closes it (run on to meet).
            let corner = a;
            let turn = previous_normal.perp_dot(normal) * direction;
            let distance = distances[index];
            let gentle = previous_normal.perp_dot(normal).atan2(previous_normal.dot(normal)).abs() <= ring::ARC_STEP + 1.0e-9;
            if turn * distance > 0.0 && gentle {
                // Run the two offsets on to where they meet: their own
                // lines, which a taper tilts off the ring's.
                if let Some(point) = kernel::line_line(previous_end, previous_end - previous_start, start, end - start)
                        // Only where it lies on from the one and short of the
                        // other: a taper can put it behind, and the edge
                        // would double back.
                        && (point - previous_end).dot(previous_end - previous_start) >= 0.0
                        && (start - point).dot(end - start) >= 0.0
                {
                    push(point, parameters[index], &mut raw);
                }
            } else if turn * distance > 0.0 {
                let (from, to) = (previous_end - corner, start - corner);
                let sweep = from.perp_dot(to).atan2(from.dot(to));
                for point in ring::arc_points(corner, from.to_angle(), sweep, distance.abs()) {
                    push(point, parameters[index], &mut raw);
                }
            }
            // Closing: the straight join crosses the next offset, and
            // the loop it leaves is cut out below.
        }
        push(start, parameters[index], &mut raw);
        push(end, parameters[index + 1], &mut raw);
        previous = Some((start, end, normal, distances[index + 1]));
    }
    let chain = without_reversals(remove_loops(raw));
    standing_clear(ring, parameters, direction, distances, chain)
}

/// The points of a road edge that stand as far from the ring as they were
/// set off: where the offset closes a corner, the run of it past the corner
/// overshoots, coming nearer the ring's other side than it was set off,
/// and doubles back without crossing itself. Those points are dropped.
fn standing_clear(ring: &[DVec2], parameters: &[f64], direction: f64, distances: &[f64], chain: Vec<(DVec2, f64)>) -> Vec<(DVec2, f64)> {
    let count = ring.len();
    let from = parameters[0];
    let along: Vec<f64> = parameters.iter().map(|parameter| along_road(*parameter, from, direction, count)).collect();
    let last = match along.last() {
        Some(end) if *end > 0.0 => *end,
        _ => count as f64,
    };
    // The distance a point set off from a ring parameter was meant to stand.
    let meant = |source: f64| -> f64 {
        let here = along_road(source, from, direction, count).min(last);
        let index = along.partition_point(|value| *value <= here).clamp(1, along.len() - 1);
        let (a, b) = (along[index - 1], along[index]);
        let t = if b > a { ((here - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
        (distances[index - 1] + (distances[index] - distances[index - 1]) * t).abs()
    };
    // Only the ring edges the stretch is laid along: around a handover the
    // ring steps across the road the last stretch arrived on, and that step
    // is road, not wall.
    // The edge the stretch runs along first: against the ring's order from
    // a corner, that is the edge before it.
    let first_edge = if direction > 0.0 { from.floor() } else { from.ceil() - 1.0 }.rem_euclid(count as f64) as usize;
    let span = (last.ceil() as usize + 1).min(count);
    let edges: Vec<usize> = (0..=span)
        .map(|step| ((first_edge as f64 + direction * step as f64).rem_euclid(count as f64)) as usize)
        .collect();
    let nearest_wall = |point: DVec2| -> f64 {
        edges
            .iter()
            .map(|&edge| {
                let (foot, _) = kernel::project_onto_segment(point, ring[edge], ring[(edge + 1) % count]);
                foot.distance(point)
            })
            .fold(f64::INFINITY, f64::min)
    };
    let total = chain.len();
    let kept: Vec<(DVec2, f64)> = chain
        .into_iter()
        .enumerate()
        .filter(|(index, (point, source))| {
            // The ends are where the road leaves and arrives: kept whatever.
            if *index == 0 || *index + 1 == total {
                return true;
            }
            let needed = meant(*source);
            let nearest = nearest_wall(*point);
            nearest >= needed - (OVERSHOOT_TOLERANCE + 1.0e-3 * needed)
        })
        .map(|(_, point)| point)
        .collect();
    kept
}

/// How much nearer the ring a road edge's point may come than it was set
/// off and still be kept.
const OVERSHOOT_TOLERANCE: f64 = 0.05;

/// An open chain without the short steps back a corner the offset closes
/// leaves where its two offsets do not quite cross: a road edge never turns
/// back on itself.
pub(crate) fn without_reversals(mut chain: Vec<(DVec2, f64)>) -> Vec<(DVec2, f64)> {
    /// A step back shorter than this is a closing corner's leftover.
    const SHORT: f64 = 1.0;
    let mut index = 1;
    while index + 1 < chain.len() {
        let (before, here, after) = (chain[index - 1].0, chain[index].0, chain[index + 1].0);
        let (into, out) = (here - before, after - here);
        // Turning back more than this at one point is never road either.
        let sharp = into.dot(out) < -0.5 * into.length() * out.length();
        if into.dot(out) < 0.0 && (sharp || into.length().min(out.length()) < SHORT) {
            chain.remove(index);
            index = index.saturating_sub(1).max(1);
        } else {
            index += 1;
        }
    }
    chain
}

/// Cut out every loop an open chain makes by crossing itself: from where it
/// first reaches a crossing to where it comes back to it.
pub(crate) fn remove_loops(chain: Vec<(DVec2, f64)>) -> Vec<(DVec2, f64)> {
    if chain.len() < 4 {
        return chain;
    }
    // Segments binned by a grid, so each is tested only against those near it.
    let segments = chain.len() - 1;
    let (mut min, mut max) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    let mut length = 0.0;
    for pair in chain.windows(2) {
        min = min.min(pair[0].0.min(pair[1].0));
        max = max.max(pair[0].0.max(pair[1].0));
        length += pair[0].0.distance(pair[1].0);
    }
    let cell = (length / segments as f64).max(1.0e-3) * 2.0;
    let columns = (((max.x - min.x) / cell).floor() as usize + 1).max(1);
    let rows = (((max.y - min.y) / cell).floor() as usize + 1).max(1);
    let cell_of = |point: DVec2| {
        let column = (((point.x - min.x) / cell).floor().max(0.0) as usize).min(columns - 1);
        let row = (((point.y - min.y) / cell).floor().max(0.0) as usize).min(rows - 1);
        (column, row)
    };
    let mut cells: std::collections::HashMap<(usize, usize), Vec<usize>> = std::collections::HashMap::new();
    for index in 0..segments {
        let (low, high) = (cell_of(chain[index].0.min(chain[index + 1].0)), cell_of(chain[index].0.max(chain[index + 1].0)));
        for column in low.0..=high.0 {
            for row in low.1..=high.1 {
                cells.entry((column, row)).or_default().push(index);
            }
        }
    }
    // Walk the chain; at each segment, the furthest later segment it crosses
    // closes the biggest loop from here, which is cut out whole.
    let mut kept: Vec<(DVec2, f64)> = vec![chain[0]];
    let mut index = 0;
    while index < segments {
        let (a, b) = (chain[index].0, chain[index + 1].0);
        let (low, high) = (cell_of(a.min(b)), cell_of(a.max(b)));
        let mut furthest: Option<(usize, DVec2, f64)> = None;
        for column in low.0..=high.0 {
            for row in low.1..=high.1 {
                for &other in cells.get(&(column, row)).map_or(&[][..], Vec::as_slice) {
                    if other < index + 2 || furthest.is_some_and(|found| other <= found.0) {
                        continue;
                    }
                    if let kernel::SegSeg::Crossing { t, point, .. } = kernel::segment_segment(a, b, chain[other].0, chain[other + 1].0) {
                        furthest = Some((other, point, chain[index].1 + (chain[index + 1].1 - chain[index].1) * t));
                    }
                }
            }
        }
        match furthest {
            Some((other, point, source)) => {
                kept.push((point, source));
                index = other;
                // On from the crossing along the far segment.
                kept.push(chain[other + 1]);
                index += 1;
            }
            None => {
                kept.push(chain[index + 1]);
                index += 1;
            }
        }
    }
    kept
}

/// Offset an open chain, each point by its own distance, to its right
/// (`right`) or left. Corners the offset opens are rounded, corners it
/// closes run to where the two offsets meet, and loops and short steps back
/// are cut out.
pub(crate) fn offset_chain(points: &[DVec2], distances: &[f64], right: bool) -> Vec<DVec2> {
    let side = if right { 1.0 } else { -1.0 };
    let normal_of = |a: DVec2, b: DVec2| {
        let along = (b - a).try_normalize().unwrap_or(DVec2::X);
        DVec2::new(along.y, -along.x) * side
    };
    let mut raw: Vec<(DVec2, f64)> = Vec::with_capacity(points.len() * 2);
    let push = |point: DVec2, raw: &mut Vec<(DVec2, f64)>| {
        if raw.last().is_none_or(|last: &(DVec2, f64)| last.0.distance(point) > SAME_POINT) {
            let index = raw.len() as f64;
            raw.push((point, index));
        }
    };
    let mut previous: Option<(DVec2, DVec2)> = None;
    for index in 0..points.len().saturating_sub(1) {
        let (a, b) = (points[index], points[index + 1]);
        if a.distance(b) <= SAME_POINT {
            continue;
        }
        let normal = normal_of(a, b);
        let (start, end) = (a + normal * distances[index], b + normal * distances[index + 1]);
        if let Some((previous_start, previous_end)) = previous
            && previous_end.distance(start) > SAME_POINT
        {
            let (along_in, along_out) = (previous_end - previous_start, end - start);
            let turn = along_in.perp_dot(along_out).atan2(along_in.dot(along_out));
            // Turning away from the side it is set off to opens the corner.
            if -turn * side * distances[index] > 0.0 {
                if turn.abs() <= ring::ARC_STEP + 1.0e-9 {
                    if let Some(point) = kernel::line_line(previous_end, along_in, start, along_out)
                        && (point - previous_end).dot(along_in) >= 0.0
                        && (start - point).dot(along_out) >= 0.0
                    {
                        push(point, &mut raw);
                    }
                } else {
                    let (from, to) = (previous_end - a, start - a);
                    let sweep = from.perp_dot(to).atan2(from.dot(to));
                    for point in ring::arc_points(a, from.to_angle(), sweep, distances[index].abs()) {
                        push(point, &mut raw);
                    }
                }
            }
        }
        push(start, &mut raw);
        push(end, &mut raw);
        previous = Some((start, end));
    }
    without_reversals(remove_loops(raw)).into_iter().map(|point| point.0).collect()
}
