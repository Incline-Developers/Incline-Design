//! Where a design falls short of what it was asked for.
//!
//! Worked out once the design is built, from what it came to: its road's
//! grade, width and turns, its berms and bench faces, and its walls between
//! ramps. Each place it falls short is one [`Flag`], at its worst point,
//! however far along the design it stays short, and on the bench it is on,
//! so the bench table and the viewport can point at the same thing.
//!
//! The berm under a road narrows to nothing by design, and the faces beside
//! it are cut round it, so walls are not checked near a road: the road's own
//! checks cover it there.

use std::collections::HashMap;

use glam::{DVec2, DVec3};

use super::{Cut, Design, RampSpec, Road, Spec, face_run, walls::Walls};

/// What a flag says is short of what was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Check {
    /// The road steeper than its gradient.
    Steep,
    /// The road flatter than its gradient: laid longer to fit.
    Flatter,
    /// The road narrower than its width.
    Narrow,
    /// The road turning tighter than its radius.
    Tight,
    /// A berm narrower than asked.
    Berm,
    /// A bench face steeper than asked.
    Face,
    /// Walls steeper between ramps than they should come to.
    InterRamp,
}

/// One place a design falls short of what was asked.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Flag {
    pub(crate) check: Check,
    /// Its worst point.
    pub(crate) at: DVec3,
    /// The bench it is on, by its place in the design's report.
    pub(crate) bench: usize,
    /// The ramp it is on, for the road's checks.
    pub(crate) ramp: Option<usize>,
    /// What the design comes to there: a grade as rise over run, a width or
    /// radius in metres, an angle in degrees.
    pub(crate) value: f64,
    /// What was asked, in the same terms.
    pub(crate) asked: f64,
    /// How far along the design it stays short, in plan; none at a point.
    pub(crate) length: f64,
}

/// How far off its gradient, as rise over run, a stretch of road may be
/// before it is flagged.
pub(crate) const GRADE_TOLERANCE: f64 = 0.005;

/// How much narrower than its width a road may be before it is flagged, as a
/// share of the width.
pub(crate) const WIDTH_TOLERANCE: f64 = 0.01;

/// How much tighter than its radius a road may turn before it is flagged, as
/// a share of the radius.
const RADIUS_TOLERANCE: f64 = 0.1;

/// How far along a road either side of a point its turn is measured over,
/// at the least: a bend, not the jitter of its points. A circle measures
/// the same over any span within it, so the span is as long as half the
/// radius checked for: a corner shows as tight, a small kink or a jog as
/// gentle, as they are to a truck.
const RADIUS_SPAN: f64 = 5.0;

/// How much narrower than asked a berm may be before it is flagged: the
/// greater of this share of it and [`BERM_SLACK`].
const BERM_TOLERANCE: f64 = 0.03;
const BERM_SLACK: f64 = 0.25;

/// How much steeper than asked a bench face may be before it is flagged, in
/// degrees.
const FACE_TOLERANCE: f64 = 1.0;

/// How far apart along a ring its berms and faces are measured.
const WALL_STEP: f64 = 2.0;

/// How far beyond a road's edges walls are left to the road's checks: the
/// berm narrows into the road, and the face is cut round it, over about a
/// bench's width either side.
const ROAD_CLEAR: f64 = 4.0;

/// The size of a cell of the grid strings are binned in to measure against.
const CELL: f64 = 10.0;

/// Every place `design`, built to `spec` with `roads`, falls short of it.
pub(super) fn flags(spec: &Spec, design: &Design, roads: &[Road]) -> Vec<Flag> {
    let mut flags = Vec::new();
    road_flags(spec, design, &mut flags);
    turn_flags(spec, design, roads, &mut flags);
    wall_flags(spec, design, roads, &mut flags);
    inter_ramp_flags(spec, design, &mut flags);
    flags
}

/// The point half way along `points`, by plan length.
pub(super) fn half_way(points: &[DVec3]) -> Option<DVec3> {
    let total: f64 = points.windows(2).map(|pair| pair[0].truncate().distance(pair[1].truncate())).sum();
    let mut left = total * 0.5;
    for pair in points.windows(2) {
        let length = pair[0].truncate().distance(pair[1].truncate());
        if length >= left && length > 0.0 {
            return Some(pair[0].lerp(pair[1], left / length));
        }
        left -= length;
    }
    points.first().copied()
}

/// How wide ramp `ramp` should be on bench `bench` of `design`: as its band
/// asks, or where it narrows from the bench before, the narrower of the two
/// as it tapers.
pub(crate) fn wanted_width(ramp: &RampSpec, design: &Design, bench: usize, sign: f64) -> f64 {
    let width_at = |elevation: f64| ramp.band(elevation, sign).and_then(|band| band.width).unwrap_or(ramp.width);
    let here = design.report.get(bench).map_or(ramp.width, |report| width_at(report.elevation));
    match bench.checked_sub(1).and_then(|before| design.report.get(before)) {
        Some(before) => here.min(width_at(before.elevation)),
        None => here,
    }
}

/// The bench of `design` an RL `z` is on: the one whose height holds it, or
/// the nearest.
fn bench_at(design: &Design, sign: f64, z: f64) -> usize {
    let mut best = (f64::INFINITY, 0);
    for (index, bench) in design.report.iter().enumerate() {
        let (low, high) = (bench.elevation, bench.elevation + sign * bench.height);
        let (low, high) = (low.min(high), low.max(high));
        let off = if z < low {
            low - z
        } else if z > high {
            z - high
        } else {
            0.0
        };
        if off < best.0 {
            best = (off, index);
        }
    }
    best.1
}

/// Each stretch of road against its ramp: its grade and width.
fn road_flags(spec: &Spec, design: &Design, flags: &mut Vec<Flag>) {
    let sign = spec.direction.sign();
    for (bench, report) in design.report.iter().enumerate() {
        for road in &report.ramps {
            let Some(ramp) = spec.ramps.get(road.ramp) else {
                continue;
            };
            let mut flag = |check: Check, at: Option<DVec3>, value: f64, asked: f64| {
                if let Some(at) = at {
                    flags.push(Flag {
                        check,
                        at,
                        bench,
                        ramp: Some(road.ramp),
                        value,
                        asked,
                        length: 0.0,
                    });
                }
            };
            if road.steepest > ramp.gradient + GRADE_TOLERANCE {
                flag(Check::Steep, road.steepest_at, road.steepest, ramp.gradient);
            } else if road.lengthened > 0.0 || road.steepest < ramp.gradient - GRADE_TOLERANCE {
                flag(Check::Flatter, road.middle, road.steepest, ramp.gradient);
            }
            let wanted = wanted_width(ramp, design, bench, sign);
            if road.narrowest < wanted * (1.0 - WIDTH_TOLERANCE) {
                flag(Check::Narrow, road.narrowest_at, road.narrowest, wanted);
            }
        }
    }
}

/// Runs of consecutive places along a string that fall short, as
/// `(index, worst, length)`: the index of the worst place in each run, how
/// short it falls there (the less the worse), and how far along the run
/// goes. Places are `step` apart; a closed string's run may wrap round.
fn runs(short: &[Option<f64>], step: impl Fn(usize) -> f64, closed: bool) -> Vec<(usize, f64, f64)> {
    let count = short.len();
    if count == 0 {
        return Vec::new();
    }
    // A closed string all short is one run from its start; otherwise each
    // run starts after a place that is not short.
    let start = if closed {
        (0..count).find(|&index| short[index].is_none()).map_or(0, |index| (index + 1) % count)
    } else {
        0
    };
    let mut found = Vec::new();
    let mut current: Option<(usize, f64, f64)> = None;
    for offset in 0..count {
        let index = (start + offset) % count;
        match (short[index], current.as_mut()) {
            (Some(value), Some(run)) => {
                run.2 += step(index);
                if value < run.1 {
                    (run.0, run.1) = (index, value);
                }
            }
            (Some(value), None) => current = Some((index, value, 0.0)),
            (None, _) => found.extend(current.take()),
        }
    }
    found.extend(current);
    found
}

/// Each road's centreline against the tightest its ramp may turn: its
/// inside edge no tighter than its turning radius, so its middle no tighter
/// than that and half its width. The middle, as trucks drive it: a hook in
/// an edge where it meets a ring is no turn.
fn turn_flags(spec: &Spec, design: &Design, roads: &[Road], flags: &mut Vec<Flag>) {
    let sign = spec.direction.sign();
    for (index, (road, ramp)) in roads.iter().zip(&spec.ramps).enumerate() {
        if ramp.turning_radius <= 0.0 {
            continue;
        }
        let centre = &road.centreline;
        let half: Vec<f64> = centre.iter().map(|point| 0.5 * wanted_width(ramp, design, bench_at(design, sign, point.z), sign)).collect();
        let widest = half.iter().copied().fold(0.0, f64::max);
        let radii = radii(centre, (0.5 * (ramp.turning_radius + widest)).max(RADIUS_SPAN));
        let short: Vec<Option<f64>> = radii
            .iter()
            .zip(&half)
            .map(|(radius, half)| {
                let inside = radius - half;
                (inside < ramp.turning_radius * (1.0 - RADIUS_TOLERANCE)).then_some(inside)
            })
            .collect();
        let step = |at: usize| centre.get(at + 1).map_or(0.0, |next| next.truncate().distance(centre[at].truncate()));
        for (at, radius, length) in runs(&short, step, false) {
            flags.push(Flag {
                check: Check::Tight,
                at: centre[at],
                bench: bench_at(design, sign, centre[at].z),
                ramp: Some(index),
                value: radius.max(0.0),
                asked: ramp.turning_radius,
                length,
            });
        }
    }
}

/// How tightly an open string turns at each of its points in plan: the
/// radius of the circle through the points `span` back along it and as far
/// ahead. Straight, and where the string is too short either side to tell,
/// infinite.
fn radii(points: &[DVec3], span: f64) -> Vec<f64> {
    let plan: Vec<DVec2> = points.iter().map(|point| point.truncate()).collect();
    let mut arcs = Vec::with_capacity(plan.len());
    let mut total = 0.0;
    for (index, point) in plan.iter().enumerate() {
        if index > 0 {
            total += point.distance(plan[index - 1]);
        }
        arcs.push(total);
    }
    let at = |arc: f64| -> Option<DVec2> {
        if arc < 0.0 || arc > total {
            return None;
        }
        let after = arcs.partition_point(|value| *value < arc).min(plan.len() - 1);
        let before = after.saturating_sub(1);
        let span = arcs[after] - arcs[before];
        Some(if span > 0.0 {
            plan[before].lerp(plan[after], (arc - arcs[before]) / span)
        } else {
            plan[after]
        })
    };
    (0..plan.len())
        .map(|index| match (at(arcs[index] - span), at(arcs[index] + span)) {
            (Some(back), Some(ahead)) => circumradius(back, plan[index], ahead),
            _ => f64::INFINITY,
        })
        .collect()
}

/// The radius of the circle through three points in plan: infinite where
/// they are in line.
fn circumradius(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    let cross = (b - a).perp_dot(c - a);
    if cross.abs() < 1.0e-12 {
        return f64::INFINITY;
    }
    a.distance(b) * b.distance(c) * c.distance(a) / (2.0 * cross.abs())
}

/// Segments in plan, binned in a grid, to find how near a point comes to
/// them.
struct Segments {
    segments: Vec<(DVec3, DVec3)>,
    cells: HashMap<(i64, i64), Vec<u32>>,
}

impl Segments {
    fn new(segments: Vec<(DVec3, DVec3)>) -> Self {
        let mut cells: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
        for (index, (a, b)) in segments.iter().enumerate() {
            let (low, high) = (a.truncate().min(b.truncate()), a.truncate().max(b.truncate()));
            for x in cell(low.x)..=cell(high.x) {
                for y in cell(low.y)..=cell(high.y) {
                    cells.entry((x, y)).or_default().push(index as u32);
                }
            }
        }
        Self { segments, cells }
    }

    /// The segments of a closed ring at RL `z`.
    fn of_ring(ring: &[DVec2], z: f64) -> Self {
        let count = ring.len();
        Self::new((0..count).map(|index| (ring[index].extend(z), ring[(index + 1) % count].extend(z))).collect())
    }

    /// How near `point` comes in plan to the segments within `reach` of it
    /// that `keep` takes, if any are.
    fn nearest(&self, point: DVec2, reach: f64, keep: impl Fn(&(DVec3, DVec3)) -> bool) -> Option<f64> {
        let mut best: Option<f64> = None;
        let mut seen = std::collections::HashSet::new();
        for x in cell(point.x - reach)..=cell(point.x + reach) {
            for y in cell(point.y - reach)..=cell(point.y + reach) {
                for &index in self.cells.get(&(x, y)).into_iter().flatten() {
                    if !seen.insert(index) {
                        continue;
                    }
                    let segment = &self.segments[index as usize];
                    if !keep(segment) {
                        continue;
                    }
                    let distance = crate::model::kernel::project_onto_segment(point, segment.0.truncate(), segment.1.truncate())
                        .0
                        .distance(point);
                    if distance <= reach && best.is_none_or(|best| distance < best) {
                        best = Some(distance);
                    }
                }
            }
        }
        best
    }
}

fn cell(value: f64) -> i64 {
    (value / CELL).floor() as i64
}

/// Points round a closed ring every [`WALL_STEP`] or nearer, as ring
/// parameters (a point's index plus how far along its edge).
fn ring_samples(ring: &[DVec2]) -> Vec<f64> {
    let count = ring.len();
    let mut samples = Vec::new();
    for index in 0..count {
        let length = ring[index].distance(ring[(index + 1) % count]);
        let pieces = (length / WALL_STEP).ceil().max(1.0) as usize;
        samples.extend((0..pieces).map(|piece| index as f64 + piece as f64 / pieces as f64));
    }
    samples
}

/// Each bench's berm and face against what its walls were cut to, away from
/// the road.
fn wall_flags(spec: &Spec, design: &Design, roads: &[Road], flags: &mut Vec<Flag>) {
    let sign = spec.direction.sign();
    let rings: Vec<Vec<DVec2>> = design
        .strings
        .iter()
        .filter(|string| string.closed)
        .map(|string| string.points.iter().map(|point| point.truncate()).collect())
        .collect();
    // Each road's middle, and half the widest it gets.
    let roads: Vec<(Segments, f64)> = roads
        .iter()
        .zip(&spec.ramps)
        .map(|(road, ramp)| {
            let widest = ramp.bands.iter().filter_map(|band| band.width).fold(ramp.width, f64::max);
            let widening = match ramp.layout {
                super::Layout::Switchback { widening, .. } => widening,
                super::Layout::Spiral => 0.0,
            };
            (Segments::new(road.centreline.windows(2).map(|pair| (pair[0], pair[1])).collect()), 0.5 * widest + widening)
        })
        .collect();
    let tallest = design.report.iter().map(|bench| bench.height).fold(0.0, f64::max);
    // On or near a road passing within a bench's height of `z`.
    let near_road = |point: DVec2, z: f64, clear: f64| {
        roads.iter().any(|(middle, half)| {
            middle
                .nearest(point, half + clear, |(a, b)| a.z.min(b.z) <= z + tallest && a.z.max(b.z) >= z - tallest)
                .is_some()
        })
    };
    let Some(mut previous_end) = rings.first() else {
        return;
    };
    let mut next = 1;
    for (bench, report) in design.report.iter().enumerate() {
        let cut = Cut {
            height: report.height,
            ..spec.cut(u32::try_from(bench).unwrap_or(u32::MAX), report.elevation)
        };
        let walls = cut.walls(spec);
        let start = if walls.no_berm() {
            previous_end
        } else {
            let Some(start) = rings.get(next) else {
                return;
            };
            next += 1;
            start
        };
        let Some(end) = rings.get(next) else {
            return;
        };
        next += 1;
        // The berm left before it, from the ring the last bench ended on.
        if bench > 0 && !std::ptr::eq(start, previous_end) {
            wall_check(&walls, (start.as_slice(), previous_end.as_slice()), report.elevation, Check::Berm, &near_road, bench, flags);
        }
        // Its face, from the ring it starts on to the one it ends on.
        let top = report.elevation + sign * report.height;
        wall_check(&walls, (end.as_slice(), start.as_slice()), top, Check::Face, &near_road, bench, flags);
        previous_end = end;
    }
}

/// One bench's berm (`check` [`Check::Berm`], `rings` its start ring and the
/// ring before it) or face ([`Check::Face`], its end ring and its start
/// ring), measured round the first ring of `rings` at RL `z`.
fn wall_check(walls: &Walls<'_>, (ring, from): (&[DVec2], &[DVec2]), z: f64, check: Check, near_road: &dyn Fn(DVec2, f64, f64) -> bool, bench: usize, flags: &mut Vec<Flag>) {
    let on = walls.on(ring);
    let other = Segments::of_ring(from, z);
    let samples = ring_samples(ring);
    // What each place comes to, and what was asked there, where it is short.
    let mut asked = vec![0.0; samples.len()];
    let short: Vec<Option<f64>> = samples
        .iter()
        .enumerate()
        .map(|(index, &parameter)| {
            let point = super::ring::point_at(ring, parameter);
            let (wanted, reach) = match check {
                Check::Berm => {
                    let width = on.berm(parameter);
                    (width, width)
                }
                _ => {
                    let (angle, _) = walls.at(point, on.facing(parameter));
                    (angle, face_run(walls.height, angle))
                }
            };
            asked[index] = wanted;
            if reach <= 0.0 || near_road(point, z, reach + ROAD_CLEAR) {
                return None;
            }
            // How near the other ring comes; beyond reach, far enough.
            let across = other.nearest(point, reach, |_| true)?;
            match check {
                Check::Berm => (across < wanted - (wanted * BERM_TOLERANCE).max(BERM_SLACK)).then_some(across),
                _ => {
                    let angle = walls.height.atan2(across).to_degrees();
                    // The steeper, the worse: kept as less to stay worst.
                    (angle > wanted + FACE_TOLERANCE).then_some(-angle)
                }
            }
        })
        .collect();
    let points: Vec<DVec2> = samples.iter().map(|&parameter| super::ring::point_at(ring, parameter)).collect();
    let step = |at: usize| points[at].distance(points[(at + 1) % points.len()]);
    for (at, worst, length) in runs(&short, step, true) {
        flags.push(Flag {
            check,
            at: super::ring::point_at(ring, samples[at]).extend(z),
            bench,
            ramp: None,
            value: if check == Check::Berm { worst } else { -worst },
            asked: asked[at],
            length,
        });
    }
}

/// The walls facing each way that come out too steep between ramps, at the
/// steepest place they were measured.
fn inter_ramp_flags(spec: &Spec, design: &Design, flags: &mut Vec<Flag>) {
    let sign = spec.direction.sign();
    for wall in design.walls.iter().filter(|wall| wall.too_steep()) {
        let Some(at) = wall.steepest_at else {
            continue;
        };
        flags.push(Flag {
            check: Check::InterRamp,
            at,
            bench: bench_at(design, sign, at.z),
            ramp: None,
            value: wall.steepest,
            asked: wall.target.unwrap_or(wall.designed),
            length: 0.0,
        });
    }
}
