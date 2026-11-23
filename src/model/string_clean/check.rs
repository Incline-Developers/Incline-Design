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
        BoxGrid, CONTROL_AGREEMENT, Crossing, CrossingSide, MINIMUM_CONTROL_VERTICES, MisshapenControls, PointsCheck, canonical_order, control_crossings, control_points,
        crossing_side, pieces_along, segment_box, segments_at, turning_back, validate_controls,
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
    /// The string's own height at `at`, for a crossing, or the height its
    /// point of a near miss was compared at, which for a point beside a
    /// crossing may be the crossing's; `None` for every other problem.
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
    /// It has a place, so it is ringed.
    pub(crate) fn placed(&self) -> bool {
        self.at.is_finite()
    }

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
    let crossings = string_crossings(&read, cancelled)?;
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
        // Read beside where it meets the others, as the build reads it.
        let own = crossings.own(piece);
        let entered = control_points(std::slice::from_ref(string), &[piece], &own, cancelled);
        stop_if(cancelled)?;
        if entered.is_ok() {
            sound[piece] = true;
            continue;
        }
        match first_clash(string, &own) {
            // Its own points pass; a point against a place it meets another
            // is for the searches below and the build's own check to name.
            Some(clash) if clash.against_crossing => sound[piece] = true,
            Some(clash) => found.push(Problem {
                kind: ProblemKind::PointsDisagree { miss: clash.miss },
                at: clash.at,
                sides: vec![side(strings, piece, clash.at.truncate())],
            }),
            None => {
                sound[piece] = control_points(std::slice::from_ref(string), &[piece], &[], cancelled).is_ok();
                stop_if(cancelled)?;
            }
        }
    }
    found.extend(meetings(strings, &read, cancelled)?);
    found.extend(near_misses(strings, &read, &sound, &found, &crossings, cancelled)?);
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
fn near_misses(
    strings: &[Vec<DVec3>],
    read: &[Vec<DVec3>],
    sound: &[bool],
    found: &[Problem],
    crossings: &StringCrossings,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Problem>, Cancelled> {
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
            let Some(clash) = pair_clash(read, crossings, (a, i), (b, j)) else {
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
                    height: Some(clash.heights.0),
                    ..side(strings, one, clash.kept.truncate())
                },
                Side {
                    height: Some(clash.heights.1),
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

/// The place the build's points check refuses for segment `i` of string `a`
/// and segment `j` of string `b`, `a`'s points read first. It is what the
/// build finds on the two strings there: where either meets any string on
/// those segments or the ones beside them, as a vertex ends two segments,
/// at the heights every string there gives; then each segment's ends with
/// the points between them at the grid spacing. Only the points that can be
/// within the merge distance of one of the other segment are read, so the
/// cost does not grow with the segments' length. A string's own points are
/// not checked against each other: its own check read them beside every
/// place it meets the others, not only the places these two segments see.
fn pair_clash(read: &[Vec<DVec3>], crossings: &StringCrossings, (a, i): (usize, usize), (b, j): (usize, usize)) -> Option<Clash> {
    let (first, second) = (read[a].as_slice(), read[b].as_slice());
    let (one, other) = ([first[i], first[i + 1]], [second[j], second[j + 1]]);
    let crossings = pair_crossings(read, crossings, (a, i), (b, j))?;
    let mut check = PointsCheck::between_controls();
    // Crossings too close at two heights are the build's own refusal; the one
    // refused is not read, and the points are read beside the rest.
    for index in 0..crossings.len() {
        let _ = check.read_crossing(&crossings, index);
    }
    // A segment shorter than the merge distance is entered whole; any other
    // is read only where its points can come near the other segment.
    let tiny = |segment: &[DVec3; 2]| segment[0].truncate().distance(segment[1].truncate()) < MERGE_DISTANCE;
    let near = |segment: &[DVec3; 2], other: &[DVec3; 2]| {
        if tiny(segment) {
            Some(None)
        } else {
            reach_along(segment[0].truncate(), segment[1].truncate(), other[0].truncate(), other[1].truncate(), NEAR_REACH).map(Some)
        }
    };
    let (mut first_range, mut second_range) = (near(&one, &other), near(&other, &one));
    if tiny(&one) || tiny(&other) {
        // The other's points are left out when it lies far from the tiny one.
        first_range = first_range.or(Some(Some((1.0, 0.0))));
        second_range = second_range.or(Some(Some((1.0, 0.0))));
    }
    let (Some(first_range), Some(second_range)) = (first_range, second_range) else {
        return None;
    };
    let lists = [(first, windowed_points(first, i, first_range)), (second, windowed_points(second, j, second_range))];
    clash_in(check, &crossings, &lists)
}

/// The places segment `i` of string `a` and segment `j` of string `b` can
/// read their points beside, as [`StringCrossings::pair`] gives them, near
/// both segments. `None` where the two segments themselves run along each
/// other or meet at two heights, which the build refuses on its own before
/// any point is entered.
fn pair_crossings(read: &[Vec<DVec3>], crossings: &StringCrossings, (a, i): (usize, usize), (b, j): (usize, usize)) -> Option<Vec<Crossing>> {
    let (one, other) = ([read[a][i], read[a][i + 1]], [read[b][j], read[b][j + 1]]);
    match kernel::segment_segment(one[0].truncate(), one[1].truncate(), other[0].truncate(), other[1].truncate()) {
        SegSeg::Disjoint => {}
        SegSeg::CollinearOverlap { .. } => return None,
        SegSeg::Crossing { point, t, u } | SegSeg::Touching { point, t, u } => {
            let (_, za) = crossing_side(&one, 0, 0, t, point);
            let (_, zb) = crossing_side(&other, 1, 0, u, point);
            if (za - zb).abs() > CONTROL_AGREEMENT {
                return None;
            }
        }
    }
    // Only a crossing this close to both segments can lie within the merge
    // distance of points of the two that are read together.
    Some(crossings.pair(read, (a, i), (b, j), NEAR_REACH + MERGE_DISTANCE))
}

/// One string's points as the check reads them: the string, and each point
/// with where it lies on it.
type StringPoints<'a> = (&'a [DVec3], Vec<(DVec3, On)>);

/// Where a point of a string's densification lies: on a vertex, or between
/// the two vertices of a segment.
#[derive(Clone, Copy)]
enum On {
    Vertex(usize),
    Segment(usize),
}

/// The points of the densification of segment `at` of `string` that fall in
/// `window`, a span of the segment as fractions of its length (`None` for
/// all of it), widened by one point each side, each with where it lies. An
/// empty span gives no points.
fn windowed_points(string: &[DVec3], at: usize, window: Option<(f64, f64)>) -> Vec<(DVec3, On)> {
    let (start, end) = (string[at], string[at + 1]);
    let Some(pieces) = segment_pieces(start, end) else {
        return Vec::new();
    };
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
    (low..=high).map(|step| (point_along(start, end, pieces, step), on_step(at, pieces, step))).collect()
}

/// Where the `step`th of `pieces` points along segment `at` lies.
fn on_step(at: usize, pieces: usize, step: usize) -> On {
    match step {
        0 => On::Vertex(at),
        step if step >= pieces => On::Vertex(at + 1),
        _ => On::Segment(at),
    }
}

/// How many parts the build cuts a segment into, at the grid spacing; `None`
/// for a segment it refuses as too long, which has no points here.
fn segment_pieces(start: DVec3, end: DVec3) -> Option<usize> {
    pieces_along(start, end, DEFAULT_SPACING)
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
/// the one entered earlier it meets, the heights they were compared at and
/// how far apart those are, and which of the strings given entered each.
struct Clash {
    at: DVec3,
    kept: DVec3,
    /// The earlier's height and the entered one's as compared: a crossing's
    /// for a point beside one.
    heights: (f64, f64),
    miss: f64,
    kept_by: usize,
    entered_by: usize,
    /// The earlier point is a place the strings meet.
    against_crossing: bool,
}

/// The first two points the build's points check finds within the merge
/// distance at heights further apart than it allows, entered in the
/// build's own order: where the string meets the others, `crossings`, first,
/// then its vertices with its segments densified at the grid spacing between.
/// `None` also where the crossings are too close at two heights, which the
/// build refuses on its own.
fn first_clash(string: &[DVec3], crossings: &[Crossing]) -> Option<Clash> {
    let mut check = PointsCheck::default();
    for index in 0..crossings.len() {
        check.read_crossing(crossings, index).ok()?;
    }
    let mut points: Vec<(DVec3, On)> = vec![(string[0], On::Vertex(0))];
    for (at, segment) in string.windows(2).enumerate() {
        let (start, end) = (segment[0], segment[1]);
        // The build refuses a segment too long to cut; that reaches the
        // report as its own refusal, with no points to look through here.
        let pieces = segment_pieces(start, end)?;
        points.extend((1..=pieces).map(|step| (point_along(start, end, pieces, step), on_step(at, pieces, step))));
    }
    clash_in(check, crossings, &[(string, points)])
}

/// Where the strings meet at one height, as the build reads them: each
/// crossing with the segments of every string through it, its controls
/// numbered as the strings, at the mean of one height per string; and the
/// crossings filed by string and by metre cell.
struct StringCrossings {
    all: Vec<Crossing>,
    /// By string, the crossings it is a side of.
    of: Vec<Vec<usize>>,
    /// By metre cell, the crossings in it.
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl StringCrossings {
    /// The crossings of string `piece`, its own segments the only sides, as
    /// its own check reads them.
    fn own(&self, piece: usize) -> Vec<Crossing> {
        self.of[piece].iter().map(|&index| self.view(index, &[piece])).collect()
    }

    /// The crossings on segment `i` of string `a`, segment `j` of string
    /// `b`, or the segments beside each, within `reach` of both segments,
    /// with `a`'s sides numbered 0 and `b`'s 1 and other strings' left off:
    /// what points of those two segments can stand beside.
    fn pair(&self, read: &[Vec<DVec3>], (a, i): (usize, usize), (b, j): (usize, usize), reach: f64) -> Vec<Crossing> {
        let (one, other) = ((read[a][i].truncate(), read[a][i + 1].truncate()), (read[b][j].truncate(), read[b][j + 1].truncate()));
        let near = |(start, end): (DVec2, DVec2), at: DVec2| kernel::project_onto_segment(at, start, end).0.distance(at) <= reach;
        let on_pair = |side: &CrossingSide| {
            let beside = |piece: usize, at: usize| side.control == piece && side.segment + 1 >= at && side.segment <= at + 1;
            beside(a, i) || beside(b, j)
        };
        // A crossing near both segments lies near where the first comes
        // within twice `reach` of the second, so only the cells around that
        // stretch are read, at steps no more than a metre apart; a segment
        // with no length in plan is its one point.
        let stretch = if one.0 == one.1 {
            Some((0.0, 0.0))
        } else {
            reach_along(one.0, one.1, other.0, other.1, 2.0 * reach)
        };
        let Some((from, to)) = stretch else {
            return Vec::new();
        };
        let (start, end) = (one.0.lerp(one.1, from), one.0.lerp(one.1, to));
        let steps = start.distance(end).ceil().max(1.0);
        let mut indices: Vec<usize> = (0..=steps as usize)
            .flat_map(|step| {
                let at = start.lerp(end, step as f64 / steps);
                let (column, row) = (at.x.floor() as i64, at.y.floor() as i64);
                (column.saturating_sub(1)..=column.saturating_add(1)).flat_map(move |column| (row.saturating_sub(1)..=row.saturating_add(1)).map(move |row| (column, row)))
            })
            .filter_map(|key| self.cells.get(&key))
            .flatten()
            .copied()
            .filter(|&index| {
                let crossing = &self.all[index];
                crossing.sides.iter().any(on_pair) && near(one, crossing.position) && near(other, crossing.position)
            })
            .collect();
        indices.sort_unstable();
        indices.dedup();
        indices.into_iter().map(|index| self.view(index, &[a, b])).collect()
    }

    /// Crossing `index` with the sides of `pieces` alone, each numbered by
    /// its place in `pieces`.
    fn view(&self, index: usize, pieces: &[usize]) -> Crossing {
        let crossing = &self.all[index];
        Crossing {
            position: crossing.position,
            z: crossing.z,
            sides: crossing
                .sides
                .iter()
                .filter_map(|side| {
                    Some(CrossingSide {
                        control: pieces.iter().position(|&piece| piece == side.control)?,
                        segment: side.segment,
                    })
                })
                .collect(),
        }
    }
}

/// Where the strings meet at one height, as [`StringCrossings`]. `Err` once
/// `cancelled` says so.
fn string_crossings(read: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<StringCrossings, Cancelled> {
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
    // Each crossing's height from each string through it, one per string
    // as the build counts them; and the crossings filed by metre cell.
    let mut all: Vec<Crossing> = Vec::new();
    let mut heights: Vec<Vec<(usize, f64)>> = Vec::new();
    let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let cell = |at: DVec2| (at.x.floor() as i64, at.y.floor() as i64);
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
            let (SegSeg::Crossing { point, t, u } | SegSeg::Touching { point, t, u }) = kernel::segment_segment(a0.truncate(), a1.truncate(), b0.truncate(), b1.truncate()) else {
                continue;
            };
            let (_, za) = crossing_side(&[a0, a1], 0, 0, t, point);
            let (_, zb) = crossing_side(&[b0, b1], 0, 0, u, point);
            if (za - zb).abs() > CONTROL_AGREEMENT {
                continue;
            }
            let (column, row) = cell(point);
            let known = (column.saturating_sub(1)..=column.saturating_add(1))
                .flat_map(|column| (row.saturating_sub(1)..=row.saturating_add(1)).map(move |row| (column, row)))
                .filter_map(|key| cells.get(&key))
                .flatten()
                .copied()
                .find(|&index| all[index].position.distance(point) <= kernel::XY_TOL);
            let index = known.unwrap_or_else(|| {
                cells.entry((column, row)).or_default().push(all.len());
                all.push(Crossing {
                    position: point,
                    z: za,
                    sides: Vec::new(),
                });
                heights.push(Vec::new());
                all.len() - 1
            });
            for (piece, segment, z) in [(a, i, za), (b, j, zb)] {
                if !all[index].sides.iter().any(|kept| (kept.control, kept.segment) == (piece, segment)) {
                    all[index].sides.push(CrossingSide { control: piece, segment });
                }
                if !heights[index].iter().any(|&(kept, _)| kept == piece) {
                    heights[index].push((piece, z));
                }
            }
        }
    }
    let mut of: Vec<Vec<usize>> = vec![Vec::new(); read.len()];
    for (index, (crossing, heights)) in all.iter_mut().zip(&heights).enumerate() {
        crossing.z = heights.iter().map(|&(_, z)| z).sum::<f64>() / heights.len() as f64;
        for side in &crossing.sides {
            if of[side.control].last() != Some(&index) {
                of[side.control].push(index);
            }
        }
    }
    Ok(StringCrossings { all, of, cells })
}

/// [`first_clash`] on points already densified, one list per string beside
/// the string it is of, read after the places the strings meet, `crossings`,
/// which `check` has read.
fn clash_in(mut check: PointsCheck, crossings: &[Crossing], lists: &[StringPoints]) -> Option<Clash> {
    for (index, (string, points)) in lists.iter().enumerate() {
        for &(point, on) in points {
            let (segments, count) = match on {
                On::Vertex(at) => segments_at(string, at),
                On::Segment(at) => ([[string[at], string[at + 1]]; 2], 1),
            };
            if let Err(disagreement) = check.read_point(crossings, point, index, &segments[..count]) {
                let (kept, kept_by, crossing) = check.point(disagreement.earlier);
                return Some(Clash {
                    at: point,
                    kept,
                    heights: (disagreement.earlier_z, disagreement.z),
                    miss: (disagreement.earlier_z - disagreement.z).abs(),
                    kept_by,
                    entered_by: index,
                    against_crossing: crossing.is_some(),
                });
            }
        }
    }
    None
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
