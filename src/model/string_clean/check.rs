//! Step 5 of Clean Strings: what Build Surface's own checks still find.
//!
//! The build stops at its first refusal, which names one place among
//! perhaps hundreds. Here its own check functions from
//! [`crate::model::control_checks`] are run on the smallest pieces that can
//! fail on their own, so every place is found: each string alone for its
//! shape and its points, each pair of segments of two strings for where they
//! meet and for points of one too near the other's. The
//! build decides whether a piece fails; this module only locates the place
//! and measures the miss. Last, the strings no problem names are run
//! through the build's checks together, exactly as the build runs them, so
//! anything the pieces could not show is still named.

use std::collections::{HashMap, HashSet};

use glam::{DVec2, DVec3};

use super::{Cancelled, stop_if};
use crate::model::{
    control_checks::{
        BoxGrid, CONTROL_AGREEMENT, MINIMUM_CONTROL_VERTICES, MisshapenControls, canonical_order, control_crossings, control_points, crossing_side, segment_box, turning_back,
        validate_controls,
    },
    kernel::{self, SegSeg},
    rbf::{DEFAULT_SPACING, MERGE_DISTANCE},
};

/// One thing still in the way of a build, ringed on the canvas: what it is,
/// where, and the strings it concerns.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Problem {
    pub(crate) kind: ProblemKind,
    /// Where it is. Not a place for [`ProblemKind::BuildRefuses`], whose
    /// `at` is NaN: that one is reported, never ringed.
    pub(crate) at: DVec3,
    /// The strings it concerns, by place in the strings checked, each with
    /// its vertex at `at` when it has one there.
    pub(crate) sides: Vec<Side>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Side {
    pub(crate) piece: usize,
    pub(crate) vertex: Option<usize>,
    /// The string's own height at `at`, for a crossing, or at its own point
    /// of a near miss; `None` for every other problem.
    pub(crate) height: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProblemKind {
    /// Fewer than two distinct vertices.
    TooShort,
    /// An open string ending where it starts.
    EndsWhereItStarts,
    /// The string turns straight back at `at`.
    TurnsBack,
    /// The string crosses or touches itself at `at`.
    CrossesItself,
    /// Two points of one string within the merge distance, `miss` metres
    /// apart in height.
    PointsDisagree { miss: f64 },
    /// Two strings run along each other.
    Along,
    /// Two strings meet at `at`, `miss` metres apart in height.
    Crossing { miss: f64 },
    /// Two strings pass within the merge distance of each other without
    /// meeting, `miss` metres apart in height.
    NearMiss { miss: f64 },
    /// The build's own checks refuse the strings with nothing above naming
    /// why: its refusal, word for word.
    BuildRefuses(String),
}

impl Problem {
    /// The miss in height a problem measures, for ordering the biggest first.
    fn miss(&self) -> Option<f64> {
        match self.kind {
            ProblemKind::PointsDisagree { miss } | ProblemKind::Crossing { miss } | ProblemKind::NearMiss { miss } => Some(miss),
            _ => None,
        }
    }
}

/// A string as the build reads it: a point repeating the one before it
/// exactly is read once. Beside it, each point's place in the string given.
fn as_read(string: &[DVec3]) -> (Vec<DVec3>, Vec<usize>) {
    let mut read: Vec<DVec3> = Vec::with_capacity(string.len());
    let mut places = Vec::with_capacity(string.len());
    for (place, &vertex) in string.iter().enumerate() {
        if read.last() != Some(&vertex) {
            read.push(vertex);
            places.push(place);
        }
    }
    (read, places)
}

/// The vertex of `string` at a plan position, within the merge distance,
/// the nearest when more than one is.
fn vertex_at(string: &[DVec3], at: DVec2) -> Option<usize> {
    string
        .iter()
        .enumerate()
        .map(|(index, vertex)| (index, vertex.truncate().distance(at)))
        .filter(|&(_, distance)| distance < MERGE_DISTANCE)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(index, _)| index)
}

fn side(strings: &[Vec<DVec3>], piece: usize, at: DVec2) -> Side {
    Side {
        piece,
        vertex: vertex_at(&strings[piece], at),
        height: None,
    }
}

/// Everything Build Surface's checks find in `strings`, each place once:
/// the crossings biggest miss first, then the rest in the order found.
/// `Err` once `cancelled` says so.
pub(super) fn problems(strings: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<Vec<Problem>, Cancelled> {
    let read: Vec<Vec<DVec3>> = strings.iter().map(|string| as_read(string).0).collect();
    let mut found = Vec::new();
    // Strings whose own points pass, so a refusal of a pair of them can
    // only be about the pair.
    let mut sound = vec![false; strings.len()];
    for (piece, string) in read.iter().enumerate() {
        stop_if(cancelled)?;
        if string.is_empty() {
            continue;
        }
        // The build's checks bail on a cancel too; read after each so a
        // cancel is never taken for a refusal.
        let validated = validate_controls(std::slice::from_ref(string), &[piece], cancelled);
        stop_if(cancelled)?;
        if let Err(error) = validated {
            let problem = if string.len() < MINIMUM_CONTROL_VERTICES {
                Problem {
                    kind: ProblemKind::TooShort,
                    at: string[0],
                    sides: vec![side(strings, piece, string[0].truncate())],
                }
            } else if let Some(misshapen) = error.downcast_ref::<MisshapenControls>() {
                let at = misshapen.positions[0];
                let plan: Vec<DVec2> = string.iter().map(|vertex| vertex.truncate()).collect();
                let kind = if turning_back(&plan).is_some() {
                    ProblemKind::TurnsBack
                } else {
                    ProblemKind::CrossesItself
                };
                Problem {
                    kind,
                    at,
                    sides: vec![side(strings, piece, at.truncate())],
                }
            } else {
                Problem {
                    kind: ProblemKind::EndsWhereItStarts,
                    at: string[0],
                    sides: vec![side(strings, piece, string[0].truncate())],
                }
            };
            found.push(problem);
            if string.len() < MINIMUM_CONTROL_VERTICES {
                continue;
            }
        }
        let entered = control_points(std::slice::from_ref(string), &[piece], &[], cancelled);
        stop_if(cancelled)?;
        if entered.is_err() {
            if let Some(clash) = first_clash(string) {
                found.push(Problem {
                    kind: ProblemKind::PointsDisagree { miss: clash.miss },
                    at: clash.at,
                    sides: vec![side(strings, piece, clash.at.truncate())],
                });
            }
        } else {
            sound[piece] = true;
        }
    }
    found.extend(meetings(strings, &read, cancelled)?);
    found.extend(near_misses(strings, &read, &sound, &found, cancelled)?);
    // Whatever the pieces could not show: the strings nothing above names
    // must pass the build's checks together.
    let named: Vec<bool> = (0..strings.len())
        .map(|piece| found.iter().any(|problem| problem.sides.iter().any(|side| side.piece == piece)))
        .collect();
    let rest: Vec<Vec<DVec3>> = strings.iter().zip(&named).filter(|(_, named)| !**named).map(|(string, _)| string.clone()).collect();
    if let Some(refusal) = build_refusal(&rest, cancelled)? {
        found.push(Problem {
            kind: ProblemKind::BuildRefuses(refusal),
            at: DVec3::NAN,
            sides: Vec::new(),
        });
    }
    let (mut crossings, rest): (Vec<Problem>, Vec<Problem>) = found.into_iter().partition(|problem| matches!(problem.kind, ProblemKind::Crossing { .. }));
    crossings.sort_by(|left, right| right.miss().unwrap_or(0.0).total_cmp(&left.miss().unwrap_or(0.0)));
    crossings.extend(rest);
    Ok(crossings)
}

/// Where two strings meet at two heights or run along each other, found by
/// the build's crossing check run on each pair of their segments that come
/// near: a pair it refuses is located and measured here. One problem per
/// pair of strings and place; for running along, one per pair.
fn meetings(strings: &[Vec<DVec3>], read: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<Vec<Problem>, Cancelled> {
    let segments: Vec<(usize, usize)> = read
        .iter()
        .enumerate()
        .flat_map(|(piece, string)| (0..string.len().saturating_sub(1)).map(move |segment| (piece, segment)))
        .collect();
    let grid = BoxGrid::new(
        segments
            .iter()
            .map(|&(piece, segment)| segment_box(read[piece][segment].truncate(), read[piece][segment + 1].truncate()))
            .collect(),
    );
    let mut found: Vec<Problem> = Vec::new();
    let mut along: HashSet<(usize, usize)> = HashSet::new();
    let mut crossed = PlaceSet::default();
    let mut near = Vec::new();
    for (first, &(a, i)) in segments.iter().enumerate() {
        stop_if(cancelled)?;
        let (a0, a1) = (read[a][i], read[a][i + 1]);
        let (low, high) = segment_box(a0.truncate(), a1.truncate());
        grid.overlapping(low, high, &mut near);
        for &second in &near {
            let (b, j) = segments[second];
            if b <= a || second <= first {
                continue;
            }
            let (b0, b1) = (read[b][j], read[b][j + 1]);
            let meeting = kernel::segment_segment(a0.truncate(), a1.truncate(), b0.truncate(), b1.truncate());
            // The build decides; a pair that does not meet cannot fail.
            if meeting == SegSeg::Disjoint || control_crossings(&[vec![a0, a1], vec![b0, b1]], &[a, b], &|| false).is_ok() {
                continue;
            }
            let problem = match meeting {
                SegSeg::Disjoint => continue,
                SegSeg::CollinearOverlap { t0, .. } => {
                    if !along.insert((a, b)) {
                        continue;
                    }
                    let at = a0.lerp(a1, t0);
                    Problem {
                        kind: ProblemKind::Along,
                        at,
                        sides: vec![side(strings, a, at.truncate()), side(strings, b, at.truncate())],
                    }
                }
                SegSeg::Crossing { point, t, u } | SegSeg::Touching { point, t, u } => {
                    let (_, za) = crossing_side(&[a0, a1], a, 0, t, point);
                    let (_, zb) = crossing_side(&[b0, b1], b, 0, u, point);
                    if crossed.holds((a, b), point) {
                        continue;
                    }
                    crossed.add((a, b), point);
                    Problem {
                        kind: ProblemKind::Crossing { miss: (za - zb).abs() },
                        at: point.extend((za + zb) / 2.0),
                        sides: vec![
                            Side {
                                height: Some(za),
                                ..side(strings, a, point)
                            },
                            Side {
                                height: Some(zb),
                                ..side(strings, b, point)
                            },
                        ],
                    }
                }
            };
            found.push(problem);
        }
    }
    Ok(found)
}

/// How many places each string meets another in plan, at one height or
/// two: one per pair of strings and place, as [`meetings`] counts them.
/// Strings running along each other are not counted there. `Err` once
/// `cancelled` says so.
pub(super) fn crossing_counts(strings: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<Vec<usize>, Cancelled> {
    let mut counts = vec![0; strings.len()];
    for (a, b) in crossing_places(strings, cancelled)? {
        counts[a] += 1;
        counts[b] += 1;
    }
    Ok(counts)
}

/// Every place two strings meet in plan, at one height or two, as the pair
/// of strings meeting there, the lower place first: one per pair and place,
/// as [`meetings`] counts them. `Err` once `cancelled` says so.
pub(super) fn crossing_places(strings: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<Vec<(usize, usize)>, Cancelled> {
    let read: Vec<Vec<DVec3>> = strings.iter().map(|string| as_read(string).0).collect();
    let segments: Vec<(usize, usize)> = read
        .iter()
        .enumerate()
        .flat_map(|(piece, string)| (0..string.len().saturating_sub(1)).map(move |segment| (piece, segment)))
        .collect();
    let grid = BoxGrid::new(
        segments
            .iter()
            .map(|&(piece, segment)| segment_box(read[piece][segment].truncate(), read[piece][segment + 1].truncate()))
            .collect(),
    );
    let mut places: Vec<(usize, usize)> = Vec::new();
    let mut seen = PlaceSet::default();
    let mut near = Vec::new();
    for (first, &(a, i)) in segments.iter().enumerate() {
        stop_if(cancelled)?;
        let (a0, a1) = (read[a][i].truncate(), read[a][i + 1].truncate());
        let (low, high) = segment_box(a0, a1);
        grid.overlapping(low, high, &mut near);
        for &second in &near {
            let (b, j) = segments[second];
            if b <= a || second <= first {
                continue;
            }
            let (SegSeg::Crossing { point, .. } | SegSeg::Touching { point, .. }) = kernel::segment_segment(a0, a1, read[b][j].truncate(), read[b][j + 1].truncate()) else {
                continue;
            };
            if !seen.holds((a, b), point) {
                seen.add((a, b), point);
                places.push((a, b));
            }
        }
    }
    Ok(places)
}

/// Points of one string too near another string's, at another height,
/// where the two do not meet: the build's points check applied to each pair
/// of segments of two strings that come within the merge distance, both
/// strings sound on their own and not already found meeting at two
/// heights. One problem per pair of strings and place.
fn near_misses(strings: &[Vec<DVec3>], read: &[Vec<DVec3>], sound: &[bool], found: &[Problem], cancelled: &dyn Fn() -> bool) -> Result<Vec<Problem>, Cancelled> {
    let reach = DVec2::splat(MERGE_DISTANCE);
    let segments: Vec<(usize, usize)> = read
        .iter()
        .enumerate()
        .filter(|&(piece, _)| sound[piece])
        .flat_map(|(piece, string)| (0..string.len().saturating_sub(1)).map(move |segment| (piece, segment)))
        .collect();
    let widened = |piece: usize, segment: usize| {
        let (low, high) = segment_box(read[piece][segment].truncate(), read[piece][segment + 1].truncate());
        (low - reach, high + reach)
    };
    let grid = BoxGrid::new(segments.iter().map(|&(piece, segment)| widened(piece, segment)).collect());
    let mut met: HashSet<(usize, usize)> = HashSet::new();
    for problem in found.iter().filter(|problem| matches!(problem.kind, ProblemKind::Crossing { .. } | ProblemKind::Along)) {
        for (index, one) in problem.sides.iter().enumerate() {
            for other in &problem.sides[index + 1..] {
                met.insert((one.piece.min(other.piece), one.piece.max(other.piece)));
            }
        }
    }
    let mut misses: Vec<Problem> = Vec::new();
    let mut seen = PlaceSet::default();
    let mut near = Vec::new();
    for &(a, i) in &segments {
        stop_if(cancelled)?;
        let (low, high) = widened(a, i);
        grid.overlapping(low, high, &mut near);
        for &other in &near {
            let (b, j) = segments[other];
            if b <= a || met.contains(&(a, b)) {
                continue;
            }
            let Some(clash) = pair_clash([read[a][i], read[a][i + 1]], [read[b][j], read[b][j + 1]]) else {
                continue;
            };
            let (one, other) = ([a, b][clash.kept_by], [a, b][clash.entered_by]);
            let pair = (one.min(other), one.max(other));
            if seen.holds(pair, clash.at.truncate()) {
                continue;
            }
            seen.add(pair, clash.at.truncate());
            let mut sides = vec![
                Side {
                    height: Some(clash.kept.z),
                    ..side(strings, one, clash.kept.truncate())
                },
                Side {
                    height: Some(clash.at.z),
                    ..side(strings, other, clash.at.truncate())
                },
            ];
            sides.sort_by_key(|side| side.piece);
            misses.push(Problem {
                kind: ProblemKind::NearMiss { miss: clash.miss },
                at: clash.at,
                sides,
            });
        }
    }
    Ok(misses)
}

/// Places filed by the pair of strings they belong to and a metre cell, so
/// whether one lies within the merge distance of an earlier one is asked of
/// nine cells, however many places there are.
#[derive(Default)]
struct PlaceSet {
    cells: HashMap<(usize, usize, i64, i64), Vec<DVec2>>,
}

impl PlaceSet {
    fn cell(at: DVec2) -> (i64, i64) {
        (at.x.floor() as i64, at.y.floor() as i64)
    }

    fn add(&mut self, pair: (usize, usize), at: DVec2) {
        let (column, row) = Self::cell(at);
        self.cells.entry((pair.0, pair.1, column, row)).or_default().push(at);
    }

    fn holds(&self, pair: (usize, usize), at: DVec2) -> bool {
        let (column, row) = Self::cell(at);
        (column.saturating_sub(1)..=column.saturating_add(1))
            .flat_map(|column| (row.saturating_sub(1)..=row.saturating_add(1)).map(move |row| (column, row)))
            .filter_map(|(column, row)| self.cells.get(&(pair.0, pair.1, column, row)))
            .flatten()
            .any(|place| place.distance(at) < MERGE_DISTANCE)
    }
}

/// How far from the other segment a point may lie and still be looked at
/// when a segment is densified only near the other: twice the merge
/// distance, so rounding can never hide a point the check would pair.
const NEAR_REACH: f64 = 2.0 * MERGE_DISTANCE;

/// The place the build's points check refuses for two segments of two
/// strings, the first given entered first. It is what the build finds on the
/// pair as two strings: where they meet, then each segment's ends with the
/// points between them at the grid spacing. Only the points that can be
/// within the merge distance of one of the other segment are entered, so
/// the cost does not grow with the segments' length.
fn pair_clash(first: [DVec3; 2], second: [DVec3; 2]) -> Option<Clash> {
    let mut crossings: Vec<(DVec3, usize)> = Vec::new();
    match kernel::segment_segment(first[0].truncate(), first[1].truncate(), second[0].truncate(), second[1].truncate()) {
        SegSeg::Disjoint => {}
        // The build refuses these on their own, before any point is entered.
        SegSeg::CollinearOverlap { .. } => return None,
        SegSeg::Crossing { point, t, u } | SegSeg::Touching { point, t, u } => {
            let (_, za) = crossing_side(&first, 0, 0, t, point);
            let (_, zb) = crossing_side(&second, 1, 0, u, point);
            if (za - zb).abs() > CONTROL_AGREEMENT {
                return None;
            }
            crossings.push((point.extend(za), 0));
        }
    }
    // A segment shorter than the merge distance can clash with itself, so it
    // is entered whole; any other has points far enough apart not to.
    let tiny = |segment: &[DVec3; 2]| segment[0].truncate().distance(segment[1].truncate()) < MERGE_DISTANCE;
    let near = |segment: &[DVec3; 2], other: &[DVec3; 2]| {
        if tiny(segment) {
            Some(None)
        } else {
            reach_along(segment[0].truncate(), segment[1].truncate(), other[0].truncate(), other[1].truncate(), NEAR_REACH).map(Some)
        }
    };
    let (mut first_range, mut second_range) = (near(&first, &second), near(&second, &first));
    if tiny(&first) || tiny(&second) {
        // The other's points are left out when it lies far from the tiny one.
        first_range = first_range.or(Some(Some((1.0, 0.0))));
        second_range = second_range.or(Some(Some((1.0, 0.0))));
    }
    let (Some(first_range), Some(second_range)) = (first_range, second_range) else {
        return None;
    };
    let lists = [windowed_points(&first, first_range), windowed_points(&second, second_range)];
    clash_in(&lists, &crossings)
}

/// The points of one segment's densification that fall in `window`, a span
/// of the segment as fractions of its length (`None` for all of it), widened
/// by one point each side. An empty span gives no points.
fn windowed_points(segment: &[DVec3; 2], window: Option<(f64, f64)>) -> Vec<DVec3> {
    let (start, end) = (segment[0], segment[1]);
    let pieces = segment_pieces(start, end);
    let (low, high) = match window {
        None => (0, pieces),
        Some((from, to)) if from > to => return Vec::new(),
        Some((from, to)) => {
            let scale = pieces as f64;
            (
                ((from * scale).floor().max(0.0) as usize).saturating_sub(1),
                ((to * scale).ceil().max(0.0) as usize).saturating_add(1).min(pieces),
            )
        }
    };
    (low..=high).map(|step| point_along(start, end, pieces, step)).collect()
}

/// How many parts the build cuts a segment into, at the grid spacing.
fn segment_pieces(start: DVec3, end: DVec3) -> usize {
    (start.truncate().distance(end.truncate()) / DEFAULT_SPACING).ceil().max(1.0) as usize
}

/// The `step`th of `pieces` points along a segment, ends included.
fn point_along(start: DVec3, end: DVec3, pieces: usize, step: usize) -> DVec3 {
    if step == 0 {
        start
    } else if step >= pieces {
        end
    } else {
        start + (end - start) * step as f64 / pieces as f64
    }
}

/// The stretch of the segment `from`-`to`, as fractions of its length
/// clamped to the segment, that lies within `reach` in plan of the segment
/// `other_from`-`other_to`; `None` when none does. The segment must have a
/// length. The set within reach of a segment is convex, so the line meets it
/// in one stretch: the span covering where it meets the strip beside the
/// segment and the two discs at its ends.
fn reach_along(from: DVec2, to: DVec2, other_from: DVec2, other_to: DVec2, reach: f64) -> Option<(f64, f64)> {
    let direction = to - from;
    let start = from - other_from;
    let span = other_to - other_from;
    let length = span.length();
    let mut stretches: Vec<(f64, f64)> = Vec::new();
    if length > 0.0 {
        let (along, across) = (span / length, span.perp() / length);
        let strip = within(across.dot(start), across.dot(direction), -reach, reach)
            .and_then(|(low, high)| within(along.dot(start), along.dot(direction), 0.0, length).map(|(from, to)| (low.max(from), high.min(to))));
        stretches.extend(strip.filter(|(low, high)| low <= high));
    }
    // The discs are read off the line's own length and sideways offset, as
    // squaring far-away coordinates would lose what a metre-scale reach needs.
    let run = direction.length();
    let (along, across) = (direction / run, direction.perp() / run);
    for centre in [start, from - other_to] {
        let sideways = -across.dot(centre);
        if sideways.abs() <= reach {
            let (middle, half) = (-along.dot(centre) / run, (reach * reach - sideways * sideways).sqrt() / run);
            stretches.push((middle - half, middle + half));
        }
    }
    let low = stretches.iter().map(|stretch| stretch.0).fold(f64::INFINITY, f64::min);
    let high = stretches.iter().map(|stretch| stretch.1).fold(f64::NEG_INFINITY, f64::max);
    let (low, high) = (low.max(0.0), high.min(1.0));
    (low <= high).then_some((low, high))
}

/// Where `constant + rate * s` lies between `low` and `high`, as a range of `s`.
fn within(constant: f64, rate: f64, low: f64, high: f64) -> Option<(f64, f64)> {
    if rate == 0.0 {
        return (low..=high).contains(&constant).then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let (one, other) = ((low - constant) / rate, (high - constant) / rate);
    Some((one.min(other), one.max(other)))
}

/// The place the build's points check refuses: the point being entered,
/// the one entered earlier it meets and how far apart their heights are,
/// and which of the strings given entered each.
struct Clash {
    at: DVec3,
    kept: DVec3,
    miss: f64,
    kept_by: usize,
    entered_by: usize,
}

/// The first two points the build's points check finds within the merge
/// distance at heights further apart than it allows, entered in the
/// build's own order: where the strings meet first, then each string's
/// vertices with its segments densified at the grid spacing between.
fn first_clash(string: &[DVec3]) -> Option<Clash> {
    let mut points: Vec<DVec3> = vec![string[0]];
    for segment in string.windows(2) {
        let (start, end) = (segment[0], segment[1]);
        let pieces = segment_pieces(start, end);
        points.extend((1..=pieces).map(|step| point_along(start, end, pieces, step)));
    }
    clash_in(&[points], &[])
}

/// [`first_clash`] on points already densified, one list per string, and the
/// places the strings meet with the string that enters each.
fn clash_in(lists: &[Vec<DVec3>], crossings: &[(DVec3, usize)]) -> Option<Clash> {
    let mut entered = Entered::default();
    for &(point, owner) in crossings {
        entered.add(point, owner);
    }
    for (index, points) in lists.iter().enumerate() {
        for &point in points {
            match entered.first_near(point.truncate()) {
                None => entered.add(point, index),
                Some(kept) if (entered.points[kept].z - point.z).abs() > CONTROL_AGREEMENT => {
                    return Some(Clash {
                        at: point,
                        kept: entered.points[kept],
                        miss: (entered.points[kept].z - point.z).abs(),
                        kept_by: entered.owners[kept],
                        entered_by: index,
                    });
                }
                Some(_) => {}
            }
        }
    }
    None
}

/// The points entered so far, filed by plan position as the build files
/// them: metre cells, the earliest point within the merge distance found.
#[derive(Default)]
struct Entered {
    points: Vec<DVec3>,
    owners: Vec<usize>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl Entered {
    fn key(at: DVec2) -> (i64, i64) {
        (at.x.floor() as i64, at.y.floor() as i64)
    }

    fn add(&mut self, point: DVec3, owner: usize) {
        self.cells.entry(Self::key(point.truncate())).or_default().push(self.points.len());
        self.points.push(point);
        self.owners.push(owner);
    }

    fn first_near(&self, at: DVec2) -> Option<usize> {
        let (column, row) = Self::key(at);
        (column.saturating_sub(1)..=column.saturating_add(1))
            .flat_map(|column| (row.saturating_sub(1)..=row.saturating_add(1)).map(move |row| (column, row)))
            .filter_map(|cell| self.cells.get(&cell))
            .flatten()
            .copied()
            .filter(|&index| self.points[index].truncate().distance(at) < MERGE_DISTANCE)
            .min()
    }
}

/// Build Surface's checks on `strings` run exactly as the build runs them,
/// each string read as the build reads it: `None` when they pass, else the
/// build's refusal. Strings are numbered by their place in `strings`.
/// `Err` once `cancelled` says so, never a cancel read as a refusal.
pub(super) fn build_refusal(strings: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<Option<String>, Cancelled> {
    let read: Vec<Vec<DVec3>> = strings.iter().map(|string| as_read(string).0).collect();
    let names = canonical_order(&read);
    let ordered: Vec<Vec<DVec3>> = names.iter().map(|&index| read[index].clone()).collect();
    let checked = validate_controls(&ordered, &names, cancelled)
        .and_then(|()| control_crossings(&ordered, &names, cancelled))
        .and_then(|crossings| control_points(&ordered, &names, &crossings, cancelled));
    stop_if(cancelled)?;
    Ok(checked.err().map(|error| format!("{error:#}")))
}
