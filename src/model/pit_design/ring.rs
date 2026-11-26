//! Closed plan rings, and offsetting them.
//!
//! A pit is built ring by ring: each bench's toe and crest are an offset of
//! the ring before, by distances that change along it where the ramp passes.
//! So the offset here takes a distance at each end of each edge. Where two
//! edges' distances differ, the larger steps straight back along its own
//! normal to meet the smaller; corners an offset opens are rounded, and
//! corners it closes are cut where the two offsets cross. Whatever folds
//! back over itself is cut away. A step in a ring is offset like any other
//! edge, so the short wall it makes is benched back like the rest.
//!
//! A ring is its points in anticlockwise order, the closing edge implied.
//! A place on a ring is a parameter: the edge it is on plus how far along
//! that edge, so 2.5 is half way along the edge from point 2 to point 3.

use glam::DVec2;

use crate::model::kernel;

/// Points closer than this are one point.
pub(crate) const SAME_POINT: f64 = 1.0e-6;

/// The most an arc rounding a corner turns between its points, in radians.
pub(crate) const ARC_STEP: f64 = std::f64::consts::PI / 18.0;

/// A point this close to the line through its neighbours is on it.
const ON_LINE: f64 = 1.0e-4;

pub(crate) fn signed_area(points: &[DVec2]) -> f64 {
    let mut doubled = 0.0;
    for (index, a) in points.iter().enumerate() {
        doubled += a.perp_dot(points[(index + 1) % points.len()]);
    }
    doubled * 0.5
}

/// `points` with repeats dropped and turned anticlockwise.
pub(crate) fn normalised(points: impl IntoIterator<Item = DVec2>) -> Vec<DVec2> {
    let mut ring: Vec<DVec2> = Vec::new();
    for point in points {
        if ring.last().is_none_or(|last| last.distance(point) > SAME_POINT) {
            ring.push(point);
        }
    }
    while ring.len() > 1 && ring[0].distance(ring[ring.len() - 1]) <= SAME_POINT {
        ring.pop();
    }
    if signed_area(&ring) < 0.0 {
        ring.reverse();
    }
    cleaned(ring)
}

/// The unit direction of edge `index`.
pub(crate) fn direction(points: &[DVec2], index: usize) -> DVec2 {
    (points[(index + 1) % points.len()] - points[index]).try_normalize().unwrap_or(DVec2::X)
}

/// The normal pointing out of an anticlockwise ring from edge `index`.
pub(crate) fn outward(points: &[DVec2], index: usize) -> DVec2 {
    let along = direction(points, index);
    DVec2::new(along.y, -along.x)
}

/// The distance round the ring to each point, and the perimeter last.
pub(crate) fn arcs(points: &[DVec2]) -> Vec<f64> {
    let mut arcs = Vec::with_capacity(points.len() + 1);
    let mut total = 0.0;
    arcs.push(0.0);
    for index in 0..points.len() {
        total += points[index].distance(points[(index + 1) % points.len()]);
        arcs.push(total);
    }
    arcs
}

/// The parameter at a distance round the ring from its first point.
pub(crate) fn parameter_at(arcs: &[f64], arc: f64) -> f64 {
    let count = arcs.len() - 1;
    let total = arcs[count];
    if total <= 0.0 {
        return 0.0;
    }
    let arc = arc.rem_euclid(total);
    let index = arcs.partition_point(|start| *start <= arc).saturating_sub(1).min(count - 1);
    let length = arcs[index + 1] - arcs[index];
    index as f64 + if length > 0.0 { (arc - arcs[index]) / length } else { 0.0 }
}

/// The distance round the ring to a parameter.
pub(crate) fn arc_at(arcs: &[f64], parameter: f64) -> f64 {
    let count = arcs.len() - 1;
    let parameter = parameter.rem_euclid(count as f64);
    let index = (parameter.floor() as usize).min(count - 1);
    arcs[index] + (arcs[index + 1] - arcs[index]) * (parameter - index as f64)
}

/// The point at a parameter.
pub(crate) fn point_at(points: &[DVec2], parameter: f64) -> DVec2 {
    let count = points.len();
    let parameter = parameter.rem_euclid(count as f64);
    let index = (parameter.floor() as usize).min(count - 1);
    points[index].lerp(points[(index + 1) % count], parameter - index as f64)
}

/// The point on the ring nearest `point`, as its parameter, and how far it is.
pub(crate) fn nearest(points: &[DVec2], point: DVec2) -> (f64, f64) {
    let mut best = (0.0, f64::INFINITY);
    for index in 0..points.len() {
        let (foot, t) = kernel::project_onto_segment(point, points[index], points[(index + 1) % points.len()]);
        let distance = foot.distance(point);
        if distance < best.1 {
            best = (index as f64 + t.clamp(0.0, 1.0), distance);
        }
    }
    best
}

/// The ring with a point at each of `parameters` (where none is already),
/// and for each point the parameter on the old ring it stands at.
pub(crate) fn with_points_at(points: &[DVec2], parameters: &[f64]) -> (Vec<DVec2>, Vec<f64>) {
    let count = points.len();
    let mut cuts: Vec<f64> = parameters.iter().map(|parameter| parameter.rem_euclid(count as f64)).collect();
    cuts.sort_by(f64::total_cmp);
    let mut result = Vec::with_capacity(count + cuts.len());
    let mut at = Vec::with_capacity(count + cuts.len());
    let mut cut = 0;
    for index in 0..count {
        let (a, b) = (points[index], points[(index + 1) % count]);
        let mut push = |point: DVec2, parameter: f64| {
            if result.last().is_none_or(|last: &DVec2| last.distance(point) > SAME_POINT) {
                result.push(point);
                at.push(parameter);
            }
        };
        push(a, index as f64);
        while cut < cuts.len() && cuts[cut] < (index + 1) as f64 {
            let t = cuts[cut] - index as f64;
            push(a.lerp(b, t), cuts[cut]);
            cut += 1;
        }
    }
    while result.len() > 1 && result[0].distance(result[result.len() - 1]) <= SAME_POINT {
        result.pop();
        at.pop();
    }
    (result, at)
}

/// How far each end of an edge is moved out (negative is in).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EdgeOffset {
    pub(crate) start: f64,
    pub(crate) end: f64,
}

impl EdgeOffset {
    pub(crate) fn even(distance: f64) -> Self {
        Self { start: distance, end: distance }
    }
}

/// The points of an arc round `centre` from `from_angle` turning `sweep`,
/// between its two ends (which are not included). The arc is drawn with
/// sides that touch the circle rather than cut inside it, so no point of it
/// is nearer the centre than `radius`: a berm rounding a corner is never
/// narrower than it is along the walls.
pub(crate) fn arc_points(centre: DVec2, from_angle: f64, sweep: f64, radius: f64) -> Vec<DVec2> {
    let steps = (sweep.abs() / ARC_STEP).ceil().max(1.0) as usize;
    let step = sweep / steps as f64;
    let reach = radius / (step * 0.5).cos();
    (0..steps)
        .map(|index| centre + DVec2::from_angle(from_angle + step * (index as f64 + 0.5)) * reach)
        .collect()
}

/// One piece of the raw offset, before what folds back is cut away: a
/// straight run.
#[derive(Clone, Copy, Debug)]
struct Piece {
    from: DVec2,
    to: DVec2,
}

/// Offset an anticlockwise ring, each edge by its own distances, outward
/// where positive. Returns the simple rings that result, anticlockwise and
/// largest first: none when the offset leaves no room.
pub(crate) fn offset(ring: &[DVec2], distances: &[EdgeOffset]) -> Vec<Vec<DVec2>> {
    offset_joined(ring, distances, false)
}

/// [`offset`] for distances that change along the ring, from point to
/// point, as a pit's walls do where they are cut differently round it: at a
/// gentle bend, where the two edges' distances agree, the point is moved
/// out by its own distance square to the bend (its mitre), rather than the
/// two edges joined as parallel offsets are. Edges set off further at one
/// end than the other lean, and the usual join between two leaning edges
/// can double back on itself, or where they cross, wander off the point
/// they stand for, a little more each time the ring is set off again.
pub(crate) fn offset_varying(ring: &[DVec2], distances: &[EdgeOffset]) -> Vec<Vec<DVec2>> {
    offset_joined(ring, distances, true)
}

fn offset_joined(ring: &[DVec2], distances: &[EdgeOffset], meet: bool) -> Vec<Vec<DVec2>> {
    let count = ring.len();
    if count < 3 || distances.len() != count {
        return Vec::new();
    }
    let rings = untangled_offset(ring, distances, meet);
    // Set off outward, a ring always leaves one. Where it seems to leave
    // none, its folds met in what is all but one point (an arc set off by
    // its own radius, made by setting off by as much the other way), too
    // close to untangle: a hair further, they cross cleanly.
    if rings.is_empty() && distances.iter().all(|distance| distance.start >= 0.0 && distance.end >= 0.0) {
        let further: Vec<EdgeOffset> = distances
            .iter()
            .map(|distance| EdgeOffset {
                start: distance.start + UNFOLD_NUDGE,
                end: distance.end + UNFOLD_NUDGE,
            })
            .collect();
        return untangled_offset(ring, &further, meet);
    }
    rings
}

/// How much further a ring is set off outward when its folds meet too
/// nearly in one point to untangle.
const UNFOLD_NUDGE: f64 = 1.0e-4;

fn untangled_offset(ring: &[DVec2], distances: &[EdgeOffset], meet: bool) -> Vec<Vec<DVec2>> {
    let pieces = raw_offset(ring, distances, meet);
    if pieces.len() < 3 {
        return Vec::new();
    }
    let mut rings: Vec<Vec<DVec2>> = untangle(&pieces)
        .into_iter()
        .map(cleaned)
        .filter(|candidate| candidate.len() >= 3 && signed_area(candidate) > SAME_POINT)
        .collect();
    rings.sort_by(|a, b| signed_area(b).total_cmp(&signed_area(a)));
    rings
}

/// The offset edges joined round the ring, crossings and all: at gentle
/// bends where the distances agree, meeting in one point where `meet`.
fn raw_offset(ring: &[DVec2], distances: &[EdgeOffset], meet: bool) -> Vec<Piece> {
    let count = ring.len();
    let mut points: Vec<DVec2> = Vec::with_capacity(count * 3);
    let push = |points: &mut Vec<DVec2>, point: DVec2| {
        if points.last().is_none_or(|last: &DVec2| last.distance(point) > SAME_POINT * 0.1) {
            points.push(point);
        }
    };
    // Whether the first corner's two offsets met in one point, so the last
    // edge's end there is not wanted.
    let mut first_met = None;
    for index in 0..count {
        let previous = (index + count - 1) % count;
        let corner = ring[index];
        let (normal_in, normal_out) = (outward(ring, previous), outward(ring, index));
        let (distance_in, distance_out) = (distances[previous].end, distances[index].start);
        if meet && (distance_in - distance_out).abs() <= SAME_POINT {
            let (along_in, along_out) = (direction(ring, previous), direction(ring, index));
            let turn = along_in.perp_dot(along_out).atan2(along_in.dot(along_out));
            if turn.abs() <= MEET_TURN {
                let in_end = corner + normal_in * distance_in;
                let out_end = ring[(index + 1) % count] + normal_out * distances[index].end;
                let point = corner + (normal_in + normal_out).normalize_or(normal_out) * distance_out / (turn * 0.5).cos();
                // The edge before ends here rather than at its own end.
                if index == 0 {
                    first_met = Some(in_end);
                } else if points.last().is_some_and(|last| last.distance(in_end) <= SAME_POINT) {
                    points.pop();
                }
                push(&mut points, point);
                push(&mut points, out_end);
                continue;
            }
        }
        // The join at this corner, from where the edge before ends to where
        // this one starts. Where their distances differ, the larger steps
        // straight back along its own normal to the smaller first (or after).
        let common = if distance_in.abs() <= distance_out.abs() { distance_in } else { distance_out };
        let stepped = (distance_in - distance_out).abs() > SAME_POINT;
        if stepped && distance_in.abs() > distance_out.abs() {
            push(&mut points, corner + normal_in * common);
        }
        corner_join(ring, distances, previous, index, common, &mut |point| push(&mut points, point));
        if stepped && distance_out.abs() > distance_in.abs() {
            push(&mut points, corner + normal_out * common);
        }
        push(&mut points, corner + normal_out * distance_out);
        push(&mut points, ring[(index + 1) % count] + normal_out * distances[index].end);
    }
    if let Some(end) = first_met
        && points.last().is_some_and(|last| last.distance(end) <= SAME_POINT)
    {
        points.pop();
    }
    let total = points.len();
    (0..total)
        .map(|index| Piece {
            from: points[index],
            to: points[(index + 1) % total],
        })
        .filter(|piece| piece.from.distance(piece.to) > SAME_POINT * 0.1)
        .collect()
}

/// The join at a corner of the ring between two edges both offset by
/// `distance`: an arc where the offset opens the corner, the two offsets
/// run on to where they meet where the corner is gentle enough to be one
/// side of an arc already, and nothing where it closes the corner (the two
/// offsets cross, and the cut takes the rest).
fn corner_join(ring: &[DVec2], distances: &[EdgeOffset], previous: usize, index: usize, distance: f64, push: &mut impl FnMut(DVec2)) {
    let corner = ring[index];
    let (along_in, along_out) = (direction(ring, previous), direction(ring, index));
    // The offsets' own directions, which an edge set off by more at one end
    // than the other tilts away from the edge's.
    let offset_direction = |edge: usize| {
        let normal = outward(ring, edge);
        let from = ring[edge] + normal * distances[edge].start;
        let to = ring[(edge + 1) % ring.len()] + normal * distances[edge].end;
        (to - from).try_normalize().unwrap_or(direction(ring, edge))
    };
    let (normal_in, normal_out) = (outward(ring, previous), outward(ring, index));
    let (from, to) = (corner + normal_in * distance, corner + normal_out * distance);
    push(from);
    if from.distance(to) <= SAME_POINT {
        return;
    }
    let turn = along_in.perp_dot(along_out).atan2(along_in.dot(along_out));
    // An anticlockwise ring turns left at a convex corner; moving out opens
    // a convex corner and closes a concave one, and moving in the reverse.
    if turn * distance <= 0.0 {
        // A corner the offset closes. A gentle one's two offsets cross just
        // past their ends, and the cut takes the overlap. A sharp one is
        // joined through the corner itself, so whatever the two offsets leave
        // between them always closes into a loop that crosses them (however
        // short an edge beside it), which the winding then takes away.
        if turn.abs() > ARC_STEP + 1.0e-9 {
            push(corner);
        }
        return;
    }
    if turn.abs() <= ARC_STEP + 1.0e-9 {
        // On an arc drawn with sides touching the circle, the two sides'
        // offsets meet at the next arc out.
        let (direction_in, direction_out) = (offset_direction(previous), offset_direction(index));
        if let Some(point) = kernel::line_line(from, direction_in, to, direction_out)
            // Only where it lies on from the one and short of the other: a
            // taper can put it behind, and the ring would double back.
            && (point - from).dot(direction_in) >= 0.0
            && (to - point).dot(direction_out) >= 0.0
        {
            push(point);
        }
        return;
    }
    let (from_offset, to_offset) = (from - corner, to - corner);
    let sweep = from_offset.perp_dot(to_offset).atan2(from_offset.dot(to_offset));
    for point in arc_points(corner, from_offset.to_angle(), sweep, distance.abs()) {
        push(point);
    }
}

/// A uniform grid of segments, for finding those near a point or another
/// segment without testing every one.
struct SegmentGrid {
    origin: DVec2,
    cell: f64,
    columns: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
}

impl SegmentGrid {
    fn new(segments: &[(DVec2, DVec2)], pad: f64) -> Self {
        let (mut min, mut max) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
        let mut length = 0.0;
        for (a, b) in segments {
            min = min.min(a.min(*b));
            max = max.max(a.max(*b));
            length += a.distance(*b);
        }
        min -= DVec2::splat(pad);
        max += DVec2::splat(pad);
        let extent = (max - min).max(DVec2::splat(1.0e-3));
        let mut cell = (length / segments.len().max(1) as f64).max(pad).max(1.0e-3);
        let budget = (segments.len() * 4).max(64) as f64;
        if (extent.x / cell).ceil() * (extent.y / cell).ceil() > budget {
            cell = (extent.x * extent.y / budget).sqrt();
        }
        let columns = ((extent.x / cell).floor() as usize + 1).max(1);
        let rows = ((extent.y / cell).floor() as usize + 1).max(1);
        let mut grid = Self {
            origin: min,
            cell,
            columns,
            rows,
            cells: vec![Vec::new(); columns * rows],
        };
        for (index, (a, b)) in segments.iter().enumerate() {
            let (x0, y0, x1, y1) = grid.span(a.min(*b) - DVec2::splat(pad), a.max(*b) + DVec2::splat(pad));
            for row in y0..=y1 {
                for column in x0..=x1 {
                    grid.cells[row * columns + column].push(index as u32);
                }
            }
        }
        grid
    }

    fn span(&self, low: DVec2, high: DVec2) -> (usize, usize, usize, usize) {
        let clamp = |value: f64, count: usize| ((value / self.cell).floor().max(0.0) as usize).min(count - 1);
        let (low, high) = (low - self.origin, high - self.origin);
        (clamp(low.x, self.columns), clamp(low.y, self.rows), clamp(high.x, self.columns), clamp(high.y, self.rows))
    }

    /// The segments in the cells a box touches, each once.
    fn near(&self, low: DVec2, high: DVec2, found: &mut Vec<u32>) {
        found.clear();
        let (x0, y0, x1, y1) = self.span(low, high);
        for row in y0..=y1 {
            for column in x0..=x1 {
                found.extend_from_slice(&self.cells[row * self.columns + column]);
            }
        }
        found.sort_unstable();
        found.dedup();
    }
}

/// A run of the raw offset between two crossings (or the whole of it).
struct Slice {
    points: Vec<DVec2>,
    from: Option<usize>,
    to: Option<usize>,
}

/// Cut the raw offset where it crosses itself, keep the slices that stand
/// far enough from the source ring, and join them into rings.
fn untangle(pieces: &[Piece]) -> Vec<Vec<DVec2>> {
    let segments: Vec<(DVec2, DVec2)> = pieces.iter().map(|piece| (piece.from, piece.to)).collect();
    let grid = SegmentGrid::new(&segments, SAME_POINT);
    let count = pieces.len();
    // Each piece's crossings: how far along it, and which crossing.
    let mut cuts: Vec<Vec<(f64, usize)>> = vec![Vec::new(); count];
    let mut crossings: Vec<DVec2> = Vec::new();
    let mut near = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        grid.near(piece.from.min(piece.to), piece.from.max(piece.to), &mut near);
        for &other in &near {
            let other = other as usize;
            // Each pair once, and never a piece with its neighbours, which
            // meet it at their shared end.
            if other <= index || other == index + 1 || (index == 0 && other == count - 1) {
                continue;
            }
            let found = pieces[other];
            if !super::road::boxes_meet(piece.from, piece.to, found.from, found.to) {
                continue;
            }
            let (t, u, point) = match kernel::segment_segment(piece.from, piece.to, found.from, found.to) {
                kernel::SegSeg::Crossing { t, u, point } => (t, u, point),
                kernel::SegSeg::Touching { point, .. } => match through_vertex(pieces, index, other, point) {
                    Some((t, u)) => (t, u, point),
                    None => continue,
                },
                _ => continue,
            };
            let crossing = crossings.len();
            crossings.push(point);
            cuts[index].push((t, crossing));
            cuts[other].push((u, crossing));
        }
    }

    // The raw offset as one run of points, crossings put in, each with the
    // piece it starts.
    let mut run: Vec<(DVec2, Option<usize>)> = Vec::with_capacity(count + crossings.len() * 2);
    for (index, piece) in pieces.iter().enumerate() {
        run.push((piece.from, None));
        let mut here = cuts[index].clone();
        here.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, crossing) in here {
            run.push((crossings[crossing], Some(crossing)));
        }
    }

    // Slices between crossings.
    let mut slices: Vec<Slice> = Vec::new();
    match run.iter().position(|point| point.1.is_some()) {
        None => slices.push(Slice {
            points: run.iter().map(|point| point.0).collect(),
            from: None,
            to: None,
        }),
        Some(first) => {
            let total = run.len();
            let mut current: Option<Slice> = None;
            for step in 0..=total {
                let (point, crossing) = run[(first + step) % total];
                if let Some(mut slice) = current.take() {
                    slice.points.push(point);
                    if crossing.is_some() {
                        slice.to = crossing;
                        slices.push(slice);
                    } else {
                        current = Some(slice);
                        continue;
                    }
                }
                if step < total {
                    current = Some(Slice {
                        points: vec![point],
                        from: crossing,
                        to: None,
                    });
                }
            }
        }
    }

    // Keep the slices that bound the ground the raw offset winds round: the
    // offset goes round what it keeps anticlockwise, and round a fold the
    // other way, so where it folds back the winding is never positive.
    // Each slice is tested half way along, just either side of itself: a
    // slice is wholly kept or wholly not.
    let scale = pieces.iter().map(|piece| piece.from.abs().max_element()).fold(1.0, f64::max);
    let probe = (scale * 1.0e-9).max(1.0e-7);
    let bands = Bands::new(pieces);
    let mut kept: Vec<Slice> = slices
        .into_iter()
        .filter(|slice| {
            let (middle, along) = middle_of(slice);
            let left = DVec2::new(-along.y, along.x) * probe;
            bands.winding(pieces, middle + left) > 0 && bands.winding(pieces, middle - left) <= 0
        })
        .collect();

    // Join slices end to start at the crossings.
    let mut rings = Vec::new();
    while let Some(start) = kept.pop() {
        let mut points = start.points;
        let first = start.from;
        let mut end = start.to;
        let mut closed = end == first;
        while !closed {
            let Some(next) = kept.iter().position(|slice| slice.from == end) else {
                break;
            };
            let slice = kept.swap_remove(next);
            points.extend(slice.points.into_iter().skip(1));
            end = slice.to;
            closed = end == first;
        }
        if closed {
            if signed_area(&points) < 0.0 {
                points.reverse();
            }
            rings.push(points);
        }
    }
    rings
}

/// Where the raw offset passes through one of its own pieces at a vertex
/// of the other: the two pieces touch rather than cross, the vertex within
/// [`kernel::XY_TOL`] of the piece it meets, but the run through the vertex
/// goes from one side of that piece to the other. Set off again, a ring's
/// points left where offsets crossed keep lying on the next offsets' lines,
/// so this is no rarity: missed, the fold it closes is never cut away. The
/// cut is on the piece the vertex ends, at its end, and on the piece it
/// meets, where; `(t, u)` along `pieces[first]` and `pieces[second]`. A touch
/// that turns back (a tangent) is no crossing, nor one at both pieces' ends.
fn through_vertex(pieces: &[Piece], first: usize, second: usize, point: DVec2) -> Option<(f64, f64)> {
    let count = pieces.len();
    let near = |a: DVec2| a.distance(point) <= kernel::XY_TOL * 2.0;
    let along = |piece: &Piece| {
        let span = piece.to - piece.from;
        ((point - piece.from).dot(span) / span.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0)
    };
    // The vertex the run passes through, as the end of piece `ends`, and
    // the piece `met` it touches part way along.
    let passes = |ends: usize, met: usize| {
        let (ending, meeting) = (&pieces[ends], &pieces[met]);
        if !near(ending.to) || near(meeting.from) || near(meeting.to) {
            return false;
        }
        let after = pieces[(ends + 1) % count].to;
        let side = |p: DVec2| kernel::orient2d(meeting.from, meeting.to, p);
        side(ending.from) * side(after) < 0.0
    };
    if passes(first, second) {
        Some((1.0, along(&pieces[second])))
    } else if passes(second, first) {
        Some((along(&pieces[first]), 1.0))
    } else {
        None
    }
}

/// The point half way along a slice by length, and its direction there.
fn middle_of(slice: &Slice) -> (DVec2, DVec2) {
    let points = &slice.points;
    let total: f64 = points.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    let mut travelled = 0.0;
    for pair in points.windows(2) {
        let length = pair[0].distance(pair[1]);
        if travelled + length >= total * 0.5 && length > 0.0 {
            return (pair[0].lerp(pair[1], (total * 0.5 - travelled) / length), (pair[1] - pair[0]) / length);
        }
        travelled += length;
    }
    (points[0], DVec2::X)
}

/// The raw offset's pieces binned by the band of height (y) they span: only
/// a piece spanning a point's height can wind round it, so only its band's
/// are counted.
struct Bands {
    low: f64,
    high: f64,
    size: f64,
    bands: Vec<Vec<u32>>,
}

impl Bands {
    fn new(pieces: &[Piece]) -> Self {
        let (low, high) = pieces.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), piece| {
            (low.min(piece.from.y.min(piece.to.y)), high.max(piece.from.y.max(piece.to.y)))
        });
        let count = (pieces.len() / 2).max(1);
        let size = ((high - low) / count as f64).max(f64::MIN_POSITIVE);
        let mut bands = Self {
            low,
            high,
            size,
            bands: vec![Vec::new(); count],
        };
        for (index, piece) in pieces.iter().enumerate() {
            let (first, last) = (bands.band(piece.from.y.min(piece.to.y)), bands.band(piece.from.y.max(piece.to.y)));
            for band in &mut bands.bands[first..=last] {
                band.push(index as u32);
            }
        }
        bands
    }

    /// The band a height falls in. Never lower for a greater height, so a
    /// piece spanning a height is in that height's band.
    fn band(&self, y: f64) -> usize {
        (((y - self.low) / self.size).floor().max(0.0) as usize).min(self.bands.len() - 1)
    }

    /// How many times the raw offset winds round a point, anticlockwise: as
    /// [`winding`] counts it.
    fn winding(&self, pieces: &[Piece], point: DVec2) -> i32 {
        // Nothing spans a height outside them all.
        if !(point.y >= self.low && point.y < self.high) {
            return 0;
        }
        winding(self.bands[self.band(point.y)].iter().map(|&index| &pieces[index as usize]), point)
    }
}

/// How many times the raw offset winds round a point, anticlockwise.
fn winding<'a>(pieces: impl IntoIterator<Item = &'a Piece>, point: DVec2) -> i32 {
    let mut count = 0;
    for piece in pieces {
        let (a, b) = (piece.from, piece.to);
        if a.y <= point.y {
            if b.y > point.y && kernel::orient2d(a, b, point) > 0.0 {
                count += 1;
            }
        } else if b.y <= point.y && kernel::orient2d(a, b, point) < 0.0 {
            count -= 1;
        }
    }
    count
}

/// A ring without repeated points, points on a straight run between their
/// neighbours, or spikes (out and straight back), which enclose nothing.
pub(crate) fn cleaned(mut ring: Vec<DVec2>) -> Vec<DVec2> {
    let mut index = 0;
    let mut unchanged = 0;
    while ring.len() >= 3 && unchanged < ring.len() {
        let count = ring.len();
        let at = index % count;
        let (before, here, after) = (ring[(at + count - 1) % count], ring[at], ring[(at + 1) % count]);
        let span = after - before;
        let length = span.length();
        let off_line = if length > SAME_POINT {
            span.perp_dot(here - before).abs() / length
        } else {
            here.distance(before)
        };
        if here.distance(before) <= SAME_POINT || off_line <= ON_LINE {
            ring.remove(at);
            index = (at + ring.len() - 1) % ring.len().max(1);
            unchanged = 0;
        } else {
            index = (at + 1) % count;
            unchanged += 1;
        }
    }
    ring
}

/// A ring conditioned: with points that barely bend it (less than
/// [`CONDITION_HEIGHT`] off the line between their neighbours), or that
/// crowd their neighbours (closer than [`CONDITION_SPACING`]) and bend it
/// little, taken out, round after round, but only where the line left in a
/// point's place moves the ring `toward` (outward where positive, inward
/// where negative), or not at all. Set off toward the waste side of a
/// face, the face only comes out flatter. Points `keep` says to keep stay.
pub(crate) fn conditioned(ring: &[DVec2], toward: f64, keep: &dyn Fn(DVec2) -> bool) -> Vec<DVec2> {
    let mut points = ring.to_vec();
    for _ in 0..CONDITION_ROUNDS {
        let mut changed = false;
        let mut index = 0;
        while index < points.len() && points.len() > 3 {
            let count = points.len();
            let (a, b, c) = (points[(index + count - 1) % count], points[index], points[(index + 1) % count]);
            let chord = c - a;
            let length = chord.length();
            // How far out of the line between its neighbours the point
            // stands: the ring moves the other way where it goes.
            let out = if length > SAME_POINT { -(chord.perp_dot(b - a) / length) } else { 0.0 };
            let height = out.abs();
            let crowded = a.distance(b) < CONDITION_SPACING || b.distance(c) < CONDITION_SPACING;
            let small = height < CONDITION_HEIGHT || (crowded && height < CONDITION_CROWDED_HEIGHT);
            if small && out * toward <= CONDITION_SLACK && !keep(b) {
                points.remove(index);
                changed = true;
                continue;
            }
            index += 1;
        }
        if !changed {
            break;
        }
    }
    points
}

/// A ring with the points that do not bend it at all (within
/// [`CONDITION_SLACK`] of the line between their neighbours) taken out:
/// the same ring, in fewer points.
pub(crate) fn without_straight_points(ring: &[DVec2]) -> Vec<DVec2> {
    let mut points = ring.to_vec();
    let mut index = 0;
    while index < points.len() && points.len() > 3 {
        let count = points.len();
        let (a, b, c) = (points[(index + count - 1) % count], points[index], points[(index + 1) % count]);
        let chord = c - a;
        let length = chord.length();
        let on_line = length > SAME_POINT && (chord.perp_dot(b - a) / length).abs() <= CONDITION_SLACK && (b - a).dot(chord) > 0.0 && (c - b).dot(chord) > 0.0;
        if on_line {
            points.remove(index);
        } else {
            index += 1;
        }
    }
    points
}

/// Points of a ring closer together than this crowd each other.
const CONDITION_SPACING: f64 = 1.0;

/// A point nearer than this to the line between its neighbours barely bends
/// the ring. An arc rounding a corner 15 m out stands further off.
const CONDITION_HEIGHT: f64 = 0.05;

/// A point crowding its neighbours goes if it stands no further than this
/// off the line between them: further, it is the ring's shape (the arc
/// rounding a tight corner, say), and taking it out would sharpen the ring.
const CONDITION_CROWDED_HEIGHT: f64 = 0.2;

/// How far the wrong way a ring may move in conditioning it, and how far
/// off the line a point may stand and still be on it: rounding.
const CONDITION_SLACK: f64 = 1.0e-6;

/// How many times a ring is gone round in conditioning it.
const CONDITION_ROUNDS: usize = 4;

/// The sharpest bend at which [`offset_varying`]'s two offsets meet in one
/// point.
const MEET_TURN: f64 = std::f64::consts::PI / 6.0;

/// A ring with its folds taken out: where it turns back on itself by more
/// than [`FOLD_TURN`] beside an edge shorter than `shortest`, the point it
/// turns at goes. An offset whose distance changes quickly along it can
/// leave such a fold where two edges leaning differently are joined, a
/// zig-zag a few centimetres across that the next offset could not follow.
pub(crate) fn without_folds(mut ring: Vec<DVec2>, shortest: f64) -> Vec<DVec2> {
    let mut index = 0;
    let mut unchanged = 0;
    while ring.len() > 3 && unchanged < ring.len() {
        let count = ring.len();
        let at = index % count;
        let (before, here, after) = (ring[(at + count - 1) % count], ring[at], ring[(at + 1) % count]);
        let folded = match ((here - before).try_normalize(), (after - here).try_normalize()) {
            (Some(along_in), Some(along_out)) => {
                along_in.perp_dot(along_out).atan2(along_in.dot(along_out)).abs() > FOLD_TURN && (here.distance(before) < shortest || here.distance(after) < shortest)
            }
            _ => true,
        };
        if folded {
            ring.remove(at);
            index = (at + ring.len() - 1) % ring.len();
            unchanged = 0;
        } else {
            index = (at + 1) % count;
            unchanged += 1;
        }
    }
    ring
}

/// A ring turning back on itself by more than this at a point is folded
/// there.
const FOLD_TURN: f64 = std::f64::consts::PI * 5.0 / 6.0;

/// Round each corner of a ring that the next offset `side` would close
/// (a nose of rock jutting into the pit building out, a corner of the pit
/// building in) to `radius`, cutting it on the side that offset moves
/// toward. Corners within `radius` of a point in `keep` are left sharp.
/// Where an edge is too short for the whole radius, the corner is rounded
/// as far as half of each edge allows.
pub(crate) fn filleted(ring: &[DVec2], radius: f64, side: f64, keep: &[DVec2]) -> Vec<DVec2> {
    let count = ring.len();
    if radius <= 0.0 || count < 3 {
        return ring.to_vec();
    }
    let mut result: Vec<DVec2> = Vec::with_capacity(count * 2);
    for index in 0..count {
        let corner = ring[index];
        let (before, after) = (ring[(index + count - 1) % count], ring[(index + 1) % count]);
        let (along_in, along_out) = ((corner - before).try_normalize(), (after - corner).try_normalize());
        let (Some(along_in), Some(along_out)) = (along_in, along_out) else {
            result.push(corner);
            continue;
        };
        let turn = along_in.perp_dot(along_out).atan2(along_in.dot(along_out));
        let closes = turn * side < 0.0 && turn.abs() > ARC_STEP + 1.0e-9;
        if !closes || keep.iter().any(|point| point.distance(corner) < radius) {
            result.push(corner);
            continue;
        }
        let half = (turn.abs() * 0.5).tan();
        let reach = (radius * half).min(0.5 * corner.distance(before)).min(0.5 * corner.distance(after));
        let fitted = reach / half;
        let from = corner - along_in * reach;
        let to = corner + along_out * reach;
        // The centre lies off the side the ring turns to.
        let normal = if turn > 0.0 {
            DVec2::new(-along_in.y, along_in.x)
        } else {
            DVec2::new(along_in.y, -along_in.x)
        };
        let centre = from + normal * fitted;
        let (start, end) = (from - centre, to - centre);
        let sweep = start.perp_dot(end).atan2(start.dot(end));
        let steps = (sweep.abs() / ARC_STEP).ceil().max(1.0) as usize;
        result.push(from);
        for step in 1..steps {
            result.push(centre + DVec2::from_angle(start.to_angle() + sweep * step as f64 / steps as f64) * fitted);
        }
        result.push(to);
    }
    cleaned(result)
}

/// `ring` (anticlockwise) with the corners an offset to `side` (outward at
/// 1, inward at -1) would close rounded to `radius`, but only those met
/// along the stretch from parameter `from` running `direction` (1 with the
/// ring's order) for `length` in plan: whole corners, however far each runs
/// past the stretch's ends. A road set off to that side from a corner so
/// rounded turns no tighter than the radius less its own offset. Rounded
/// so, a corner only moves toward that side: a closing (outward at a
/// concave corner, inward at a convex one, as a morphological closing or
/// opening). A corner within `clear` of the stretch's start is left as it
/// is: there the ring steps across the road arriving from the bench below,
/// and that step is road, not wall. `None` where no corner along the
/// stretch is tighter.
pub(crate) fn rounded_along(ring: &[DVec2], side: f64, radius: f64, (from, direction, length): (f64, f64, f64), clear: f64) -> Option<Vec<DVec2>> {
    let count = ring.len();
    if count < 3 || radius <= 0.0 || length <= 0.0 {
        return None;
    }
    let arcs = arcs(ring);
    let around = arcs[count];
    let start = arc_at(&arcs, from);
    // How far along the stretch a parameter is, the way the road runs.
    let along = |parameter: f64| ((arc_at(&arcs, parameter) - start) * direction).rem_euclid(around);
    // Only the corners the stretch meets, clear of its start: a corner
    // whose span reaches back past the start, or lies within `clear` of it
    // either way, is left.
    rounded_where(ring, side, radius, &|before, after, _| {
        // Before and after run the ring's way; the road may run the other,
        // meeting a corner's after first.
        let (first, last) = if direction < 0.0 {
            (along(after), along(before))
        } else {
            (along(before), along(after))
        };
        // Its span runs on through the start where its end comes round
        // before its beginning.
        let spans_start = last < first;
        !spans_start && first >= clear && first <= length && around - last >= clear
    })
}

/// `ring` (anticlockwise) with the corners an offset to `side` would close
/// rounded to `radius` (see [`rounded_along`]), but only those `wanted`
/// says: each by the ring parameters its rounding spans, from the last
/// point of the ring before it to the first after, and its rounded points.
/// `None` where no corner wanted is tighter.
pub(crate) fn rounded_where(ring: &[DVec2], side: f64, radius: f64, wanted: &dyn Fn(f64, f64, &[DVec2]) -> bool) -> Option<Vec<DVec2>> {
    let count = ring.len();
    if count < 3 || radius <= 0.0 {
        return None;
    }
    let even = |distance: f64, points: &[DVec2]| vec![EdgeOffset::even(distance); points.len()];
    let there = offset(ring, &even(side * radius, ring)).into_iter().next()?;
    let rounded = offset(&there, &even(-side * radius, &there)).into_iter().next()?;
    // Each piece's middle too: a straight piece back along the ring between
    // two rounded corners has its ends off it, where the corners' arcs
    // leave, and only its middle shows the corners are two.
    let rounded: Vec<DVec2> = (0..rounded.len())
        .flat_map(|index| [rounded[index], rounded[index].lerp(rounded[(index + 1) % rounded.len()], 0.5)])
        .collect();
    // Where the rounded ring leaves the ring: runs of its points off it.
    let placed: Vec<(f64, bool)> = rounded
        .iter()
        .map(|point| {
            let (parameter, distance) = nearest(ring, *point);
            (parameter, distance > ROUNDED_OFF)
        })
        .collect();
    let total = rounded.len();
    let first_on = placed.iter().position(|(_, off)| !off)?;
    // The runs off the ring, each with the parameters it spans on the ring:
    // from the last point on it before to the first after.
    let mut runs: Vec<(f64, f64, Vec<DVec2>)> = Vec::new();
    let mut step = 0;
    while step < total {
        let index = (first_on + step) % total;
        if !placed[index].1 {
            step += 1;
            continue;
        }
        let before = placed[(index + total - 1) % total].0;
        let mut points = Vec::new();
        while step < total && placed[(first_on + step) % total].1 {
            points.push(rounded[(first_on + step) % total]);
            step += 1;
        }
        let after = placed[(first_on + step) % total].0;
        runs.push((before, after, points));
    }
    let met: Vec<&(f64, f64, Vec<DVec2>)> = runs.iter().filter(|(before, after, points)| wanted(*before, *after, points)).collect();
    if met.is_empty() {
        return None;
    }
    // The ring, each met corner's span of it replaced by the corner rounded.
    let inside = |parameter: f64, (before, after): (f64, f64)| {
        let span = (after - before).rem_euclid(count as f64);
        let into = (parameter - before).rem_euclid(count as f64);
        into > 1.0e-9 && into < span - 1.0e-9
    };
    let mut out: Vec<DVec2> = Vec::with_capacity(count + met.iter().map(|run| run.2.len()).sum::<usize>());
    let mut placed_runs = vec![false; met.len()];
    for index in 0..count {
        let parameter = index as f64;
        if let Some(run) = met.iter().position(|(before, after, _)| inside(parameter, (*before, *after))) {
            if !placed_runs[run] {
                placed_runs[run] = true;
                out.push(point_at(ring, met[run].0));
                out.extend(met[run].2.iter().copied());
                out.push(point_at(ring, met[run].1));
            }
            continue;
        }
        out.push(ring[index]);
    }
    // A corner between two of the ring's points, holding none of them.
    for (run, done) in met.iter().zip(&placed_runs) {
        if !done {
            let at = (run.0.floor() as usize + 1) % count;
            let insert = out.iter().position(|point| point.distance(ring[at]) <= SAME_POINT).unwrap_or(out.len());
            let mut corner = vec![point_at(ring, run.0)];
            corner.extend(run.2.iter().copied());
            corner.push(point_at(ring, run.1));
            out.splice(insert..insert, corner);
        }
    }
    let out = cleaned(out);
    (out.len() >= 3 && signed_area(&out) > 0.0).then_some(out)
}

/// How far a point of a rounded ring may stand off the ring it was rounded
/// from and still count as on it.
const ROUNDED_OFF: f64 = 1.0e-3;

#[cfg(test)]
mod unfold_tests {
    use super::*;

    /// A ring whose bay is an arc made by setting off by some radius, set
    /// off outward by that radius again: the arc's offsets all but meet in
    /// its centre, and the ring must still come out.
    #[test]
    fn a_bay_set_off_by_its_own_radius_still_leaves_a_ring() {
        let ring: Vec<DVec2> = [
            (222.69941317490844, 81.98076211353316),
            (227.59419384786833, 88.42338659456394),
            (234.288243628983, 94.76866356896798),
            (241.92030754733662, 99.94789543287493),
            (250.2894724939772, 103.82473956823378),
            (259.17542126942254, 106.297138547789),
            (268.34423240582834, 107.30000678383249),
            (292.97760588427235, 105.98128810148117),
            (292.97760588427235, 168.01923788646678),
            (81.98076211353316, 168.0192378864668),
            (81.98076211353316, 81.98076211353316),
        ]
        .iter()
        .map(|&(x, y)| DVec2::new(x, y))
        .collect();
        let out = offset(&ring, &vec![EdgeOffset::even(56.66025403784439); ring.len()]);
        assert_eq!(out.len(), 1);
        assert!(signed_area(&out[0]) > signed_area(&ring) * 3.0, "{}", signed_area(&out[0]));
    }
}
