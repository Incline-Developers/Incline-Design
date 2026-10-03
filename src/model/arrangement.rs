//! Cutting a plan outline into the pieces a set of lines leaves it in.
//!
//! The Blasting step does not store its blast shapes. It stores the lines the
//! user drew across a bench, and works the shapes back out of them every time
//! one moves - so a cut line stays an ordinary design polyline that ordinary
//! undo, snapping and editing apply to. This module is that derivation: the
//! outline rings and the cut lines together form a planar arrangement, and the
//! bounded faces of that arrangement are the shapes.
//!
//! The construction is the textbook one. Every segment is split at every point
//! it meets another, the fragments are welded into a graph, and each face is
//! walked by always taking the next edge clockwise from the one just arrived
//! along - which keeps the face on the left throughout. Walks that come back
//! with positive area enclose ground; the negative one each connected
//! component produces is that component's outer boundary, and belongs as a
//! hole to whichever face encloses it.

use std::collections::{HashMap, HashSet};

use glam::DVec2;

use crate::model::kernel::{SegSeg, XY_TOL, segment_segment};

/// A face of the subdivision: its outer ring first, then its holes. Rings are
/// closed implicitly - the last point is not a repeat of the first - with the
/// outer ring wound counter-clockwise and the holes clockwise.
pub(crate) type Face = Vec<Vec<DVec2>>;

/// Cut `outline` into the faces `cuts` leave it in.
///
/// `outline` is one solid's plan rings in any order and any winding: what is
/// inside it is decided by the even-odd rule, so an outer ring, a hole in it
/// and an island in that hole all behave without being told apart first.
/// `cuts` are open polylines - close one by repeating its first point at the
/// end. A cut that stops short of the boundary closes nothing and simply
/// leaves the face it crosses into whole.
///
/// Faces are returned in no particular order. The outline rings are assumed
/// not to cross themselves or each other - they come from a mesh boundary
/// trace, which cannot produce a crossing - so only pairs involving a cut are
/// tested, which is what keeps this affordable on an outline of some thousands
/// of points.
///
/// The same ground and the same cuts must give the same faces every time they
/// are subdivided, whatever order the rings happen to arrive in: a ring's
/// starting point is not data - a mesh boundary trace can begin anywhere along
/// it - but the graph this builds welds near-coincident points in segment
/// order, so a rotated ring can weld differently and occasionally lose a cut
/// outright. Every ring is therefore rotated to start at its own lowest point
/// before anything is built, which makes the whole construction a function of
/// the rings' points rather than of their order of arrival.
pub(crate) fn subdivide(outline: &[Vec<DVec2>], cuts: &[Vec<DVec2>]) -> Vec<Face> {
    // Work near the origin. Mine coordinates leave an absolute tolerance of a
    // tenth of a millimetre only a few decimal digits of headroom, and the
    // angular sort that walks the faces is the last place to spend them. The
    // origin is the lowest corner of the whole outline rather than its first
    // point, so it is a property of the ground rather than of the order the
    // rings arrived in.
    let Some(origin) = outline.iter().flatten().copied().reduce(DVec2::min).filter(|origin| origin.x.is_finite()) else {
        return Vec::new();
    };
    let outline: Vec<Vec<DVec2>> = outline
        .iter()
        .map(|ring| canonical_rotation(ring, true).iter().map(|point| *point - origin).collect())
        .collect();

    let mut segments: Vec<[DVec2; 2]> = Vec::new();
    for ring in &outline {
        push_ring(&mut segments, ring, true);
    }
    let cut_start = segments.len();
    for cut in cuts {
        let cut: Vec<DVec2> = canonical_rotation(cut, false).iter().map(|point| *point - origin).collect();
        push_ring(&mut segments, &cut, false);
    }

    let mut graph = Graph::default();
    for [a, b] in split_at_intersections(segments, cut_start) {
        graph.add_edge(a, b);
    }
    graph.prune_dangling();

    let components = graph.components();
    let mut outer = Vec::new();
    let mut boundaries = Vec::new();
    for cycle in graph.cycles() {
        let component = cycle.first().map_or(usize::MAX, |&node| components[node]);
        let ring: Vec<DVec2> = cycle.into_iter().map(|node| graph.nodes[node]).collect();
        let area = signed_area(&ring);
        if area > 0.0 {
            outer.push((ring, area, component));
        } else {
            boundaries.push((ring, component));
        }
    }

    // Each connected component walks out exactly one clockwise cycle: its own
    // outer boundary. Whichever face encloses that component has it sitting in
    // a hole, so the cycle is that face's hole ring. Components with nothing
    // around them are the unbounded face, and are dropped.
    //
    // Only faces of *other* components are candidates. A component's own
    // boundary vertices lie on its own faces' rings, where a containment test
    // has no answer; no other component touches them.
    let mut faces: Vec<Face> = outer.iter().map(|(ring, ..)| vec![ring.clone()]).collect();
    for (boundary, component) in boundaries {
        let Some(point) = boundary.first().copied() else { continue };
        let container = outer
            .iter()
            .enumerate()
            .filter(|(_, (ring, _, owner))| *owner != component && point_in_ring(ring, point))
            .min_by(|(_, (_, a, _)), (_, (_, b, _))| a.total_cmp(b))
            .map(|(index, _)| index);
        if let Some(index) = container {
            faces[index].push(boundary);
        }
    }

    faces
        .into_iter()
        .filter(|face| representative_point(face).is_some_and(|point| point_in_rings(&outline, point)))
        .map(|face| face.into_iter().map(|ring| ring.into_iter().map(|point| point + origin).collect()).collect())
        .collect()
}

/// Whether `point` lies inside a face: in its outer ring and in none of its
/// holes, which the even-odd rule over all of them says in one pass.
pub(crate) fn point_in_face(face: &[Vec<DVec2>], point: DVec2) -> bool {
    point_in_rings(face, point)
}

/// A point known to lie inside a face, used to hold a blast's name to its
/// ground across an edit.
///
/// Prefer the area centre for a centered label; if it falls outside a concave
/// face or inside a hole, use the middle of the largest interior triangle.
pub(crate) fn representative_point(face: &[Vec<DVec2>]) -> Option<DVec2> {
    let outer = face.first()?;
    if outer.len() < 3 {
        return None;
    }
    // Prefer the area centre when it lies on ground. Rebase before the
    // shoelace sums so mine coordinates do not lose the small local area.
    let origin = outer[0];
    let mut weighted = DVec2::ZERO;
    let mut weight = 0.0;
    for (index, ring) in face.iter().enumerate() {
        let mut sum = DVec2::ZERO;
        let mut cross_sum = 0.0;
        for i in 0..ring.len() {
            let a = ring[i] - origin;
            let b = ring[(i + 1) % ring.len()] - origin;
            let cross = a.perp_dot(b);
            sum += (a + b) * cross;
            cross_sum += cross;
        }
        if cross_sum.abs() > 1e-12 {
            let area = cross_sum.abs() * if index == 0 { 1.0 } else { -1.0 };
            weighted += sum / (3.0 * cross_sum) * area;
            weight += area;
        }
    }
    if weight > 1e-12 {
        let center = origin + weighted / weight;
        if point_in_face(face, center) {
            return Some(center);
        }
    }
    let mut points: Vec<DVec2> = outer.clone();
    let mut holes = Vec::new();
    for hole in face.iter().skip(1).filter(|hole| hole.len() >= 3) {
        holes.push(points.len());
        points.extend(hole.iter().copied());
    }
    let mut indices = Vec::new();
    earcut::Earcut::new().earcut(points.iter().map(|point| [point.x, point.y]), &holes, &mut indices);
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|triangle| triangle.map(|index| points[index]))
        .map(|[a, b, c]| (((b - a).perp_dot(c - a)).abs(), (a + b + c) / 3.0))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point)| point)
}

/// Signed area of a ring in XY. Positive is counter-clockwise.
pub(crate) fn signed_area(ring: &[DVec2]) -> f64 {
    let mut area = 0.0;
    for index in 0..ring.len() {
        let a = ring[index];
        let b = ring[(index + 1) % ring.len()];
        area += a.perp_dot(b);
    }
    area / 2.0
}

/// Even-odd containment over a set of rings.
fn point_in_rings(rings: &[Vec<DVec2>], point: DVec2) -> bool {
    !rings.iter().filter(|ring| point_in_ring(ring, point)).count().is_multiple_of(2)
}

/// Crossing-number test against one closed ring. Points on the boundary are
/// undefined, which costs nothing here: every point tested is an interior
/// representative or a vertex of another component.
fn point_in_ring(ring: &[DVec2], point: DVec2) -> bool {
    let mut inside = false;
    for index in 0..ring.len() {
        let a = ring[index];
        let b = ring[(index + 1) % ring.len()];
        if (a.y > point.y) != (b.y > point.y) && point.x < a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
}

/// The same points, started from a canonical place: a closed ring from its
/// lowest vertex, an open line from its lower end.
///
/// Where a ring starts is not data - a mesh boundary trace can begin anywhere
/// along it - but the arrangement's weld takes points in segment order, so two
/// rotations of one ring can weld near-coincident points differently and even
/// lose a cut outright. Canonicalising the start makes the subdivision a
/// function of the rings' points alone. `closed` says the points are a cycle
/// whether or not the caller repeated the first point at the end.
fn canonical_rotation(points: &[DVec2], closed: bool) -> Vec<DVec2> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let lowest = |index: usize| (points[index].x, points[index].y);
    let repeats = points[0].distance_squared(points[points.len() - 1]) <= XY_TOL * XY_TOL;
    if (closed || repeats) && points.len() > 2 {
        // The repeated end point, where the caller drew one, is not part of
        // the cycle; rotate the cycle and append it back.
        let cycle = if repeats { &points[..points.len() - 1] } else { points };
        let start = (0..cycle.len())
            .min_by(|a, b| lowest(*a).0.total_cmp(&lowest(*b).0).then_with(|| lowest(*a).1.total_cmp(&lowest(*b).1)))
            .unwrap_or(0);
        let mut rotated: Vec<DVec2> = cycle.iter().cycle().skip(start).take(cycle.len()).copied().collect();
        // Direction is not data either - a boundary trace can walk either way
        // round - and the even-odd rule this module answers to does not read
        // it. Canonicalising it as well makes a ring and its reverse the same
        // input, which is what "the same ground" has to mean here.
        if signed_area(&rotated) < 0.0 {
            rotated.reverse();
        }
        if repeats {
            rotated.push(rotated[0]);
        }
        rotated
    } else if lowest(points.len() - 1) < lowest(0) {
        // An open line's two ends are interchangeable to the arrangement.
        let mut reversed = points.to_vec();
        reversed.reverse();
        reversed
    } else {
        points.to_vec()
    }
}

/// Append a polyline's edges, closing it when it is a ring.
fn push_ring(segments: &mut Vec<[DVec2; 2]>, points: &[DVec2], closed: bool) {
    if points.len() < 2 {
        return;
    }
    let last = if closed { points.len() } else { points.len() - 1 };
    for index in 0..last {
        let (a, b) = (points[index], points[(index + 1) % points.len()]);
        if a.distance_squared(b) > XY_TOL * XY_TOL {
            segments.push([a, b]);
        }
    }
}

/// Break every segment at every point another segment meets it.
///
/// Segments before `cut_start` came from the outline and are taken on trust
/// not to cross each other, so only pairs with a cut on at least one side are
/// tested.
fn split_at_intersections(segments: Vec<[DVec2; 2]>, cut_start: usize) -> Vec<[DVec2; 2]> {
    let mut splits: Vec<Vec<f64>> = vec![Vec::new(); segments.len()];
    let pairs = (0..cut_start).flat_map(|i| (cut_start..segments.len()).map(move |j| (i, j)));
    let pairs = pairs.chain((cut_start..segments.len()).flat_map(|i| (i + 1..segments.len()).map(move |j| (i, j))));
    for (i, j) in pairs.collect::<Vec<_>>() {
        let ([a, b], [c, d]) = (segments[i], segments[j]);
        match segment_segment(a, b, c, d) {
            SegSeg::Crossing { t, u, .. } | SegSeg::Touching { t, u, .. } => {
                splits[i].push(t);
                splits[j].push(u);
            }
            // Two segments running along each other: break both at the ends of
            // the stretch they share, so the fragments in between coincide and
            // weld into one edge instead of a two-sided sliver.
            SegSeg::CollinearOverlap { t0, t1 } => {
                for t in [t0, t1] {
                    splits[i].push(t);
                    splits[j].push(parameter_of(c, d, a.lerp(b, t)));
                }
            }
            SegSeg::Disjoint => {}
        }
    }

    let mut pieces = Vec::new();
    for (index, [a, b]) in segments.into_iter().enumerate() {
        let mut cuts = std::mem::take(&mut splits[index]);
        cuts.push(0.0);
        cuts.push(1.0);
        cuts.sort_by(f64::total_cmp);
        // The final parameter is always 1.0, so a piece dropped for being
        // shorter than tolerance leaves `previous` within tolerance of the far
        // end - where welding puts the two on the same node anyway.
        let mut previous = a;
        for parameter in cuts.into_iter().skip(1) {
            let point = a.lerp(b, parameter.clamp(0.0, 1.0));
            if previous.distance(point) > XY_TOL {
                pieces.push([previous, point]);
                previous = point;
            }
        }
    }
    pieces
}

/// Where `point` falls along `a -> b`, clamped to the segment.
fn parameter_of(a: DVec2, b: DVec2, point: DVec2) -> f64 {
    let span = b - a;
    let length_squared = span.length_squared();
    if length_squared <= 0.0 {
        return 0.0;
    }
    ((point - a).dot(span) / length_squared).clamp(0.0, 1.0)
}

/// The welded planar graph the faces are walked on.
#[derive(Default)]
struct Graph {
    nodes: Vec<DVec2>,
    /// Nodes by [`XY_TOL`]-sized cell, so welding a point is a look at nine
    /// cells rather than a scan of every node placed so far.
    buckets: HashMap<(i64, i64), Vec<usize>>,
    /// Neighbours of each node, sorted counter-clockwise by the direction the
    /// edge leaves in. The sort is what makes the face walk possible.
    adjacency: Vec<Vec<usize>>,
    edges: HashSet<(usize, usize)>,
}

impl Graph {
    fn cell(point: DVec2) -> (i64, i64) {
        ((point.x / XY_TOL).floor() as i64, (point.y / XY_TOL).floor() as i64)
    }

    fn node(&mut self, point: DVec2) -> usize {
        let (cx, cy) = Self::cell(point);
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for &index in self.buckets.get(&(x, y)).into_iter().flatten() {
                    if self.nodes[index].distance(point) <= XY_TOL {
                        return index;
                    }
                }
            }
        }
        let index = self.nodes.len();
        self.nodes.push(point);
        self.adjacency.push(Vec::new());
        self.buckets.entry((cx, cy)).or_default().push(index);
        index
    }

    fn add_edge(&mut self, a: DVec2, b: DVec2) {
        let (a, b) = (self.node(a), self.node(b));
        if a == b || !self.edges.insert((a.min(b), a.max(b))) {
            return;
        }
        self.adjacency[a].push(b);
        self.adjacency[b].push(a);
    }

    /// Drop the chains that lead nowhere: a cut line that stops short of the
    /// boundary, or the stub left where one overshoots it. They bound no face,
    /// and left in they would be walked out and back as a zero-width spike in
    /// the ring of the face they reach into.
    fn prune_dangling(&mut self) {
        let mut stack: Vec<usize> = (0..self.nodes.len()).filter(|&node| self.adjacency[node].len() == 1).collect();
        while let Some(node) = stack.pop() {
            let [other] = self.adjacency[node][..] else { continue };
            self.adjacency[node].clear();
            self.adjacency[other].retain(|&neighbour| neighbour != node);
            self.edges.remove(&(node.min(other), node.max(other)));
            if self.adjacency[other].len() == 1 {
                stack.push(other);
            }
        }
    }

    /// Which connected component each node belongs to. Nodes left isolated by
    /// pruning get a component of their own, which nothing ever asks about.
    fn components(&self) -> Vec<usize> {
        let mut labels = vec![usize::MAX; self.nodes.len()];
        let mut next = 0;
        for start in 0..self.nodes.len() {
            if labels[start] != usize::MAX {
                continue;
            }
            let mut stack = vec![start];
            labels[start] = next;
            while let Some(node) = stack.pop() {
                for &neighbour in &self.adjacency[node] {
                    if labels[neighbour] == usize::MAX {
                        labels[neighbour] = next;
                        stack.push(neighbour);
                    }
                }
            }
            next += 1;
        }
        labels
    }

    /// Every cycle of the arrangement, as node indices.
    ///
    /// Each of the two directions of each edge belongs to exactly one cycle,
    /// so walking from every unvisited directed edge enumerates them all
    /// without repeats.
    fn cycles(&mut self) -> Vec<Vec<usize>> {
        for node in 0..self.nodes.len() {
            let origin = self.nodes[node];
            self.adjacency[node].sort_by(|&a, &b| {
                let angle = |other: usize| {
                    let delta = self.nodes[other] - origin;
                    delta.y.atan2(delta.x)
                };
                angle(a).total_cmp(&angle(b))
            });
        }

        let mut visited: HashSet<(usize, usize)> = HashSet::new();
        let mut cycles = Vec::new();
        for from in 0..self.nodes.len() {
            for index in 0..self.adjacency[from].len() {
                let start = (from, self.adjacency[from][index]);
                if visited.contains(&start) {
                    continue;
                }
                let mut cycle = Vec::new();
                let mut edge = start;
                loop {
                    if !visited.insert(edge) {
                        break;
                    }
                    cycle.push(edge.0);
                    edge = self.next_half_edge(edge);
                    if edge == start {
                        break;
                    }
                }
                if cycle.len() >= 3 {
                    cycles.push(cycle);
                }
            }
        }
        cycles
    }

    /// The edge a face walk takes after arriving at `to` along `from -> to`:
    /// the first one clockwise from the way it came, which is what keeps the
    /// face being traced on the left of every edge of it.
    fn next_half_edge(&self, (from, to): (usize, usize)) -> (usize, usize) {
        let neighbours = &self.adjacency[to];
        let position = neighbours.iter().position(|&node| node == from).unwrap_or(0);
        (to, neighbours[(position + neighbours.len() - 1) % neighbours.len()])
    }
}

/// Tessellate stored planning polylines into open or explicitly closed XY cuts.
pub(crate) fn cut_lines(objects: &[crate::model::Object]) -> Vec<Vec<DVec2>> {
    objects
        .iter()
        .filter_map(|object| {
            let crate::model::Object::Polyline { verts, closed, .. } = object else { return None };
            let mut points: Vec<_> = crate::model::geometry::tessellate_polyline_bulges(verts, *closed)
                .into_iter()
                .map(|point| point.truncate())
                .collect();
            if *closed && points.len() >= 3 {
                points.push(points[0]);
            }
            (points.len() >= 2).then_some(points)
        })
        .collect()
}
