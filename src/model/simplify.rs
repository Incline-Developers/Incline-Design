//! Thinning a dense surface to fewer triangles that keep its shape.
//!
//! Sides are collapsed one at a time, the one that moves the surface least
//! first (Garland and Heckbert's quadric error: every vertex carries the
//! planes of the triangles it has stood for, weighted by their area, and a
//! collapse costs the squared distance of its new point from all of them).
//! Flat ground goes first and in bulk; crests, toes and breaks in slope hold
//! their vertices longest. The new point is put where that cost is least, so
//! a crest stays on its line rather than stepping from one survey point to
//! another.
//!
//! The open edge is held by planes stood upright along it, so its outline in
//! plan keeps its shape while the points along a straight run of it thin out.
//! No collapse may fold a triangle over, tip one past vertical, leave a
//! needle, pinch the surface to a point or close a hole.
//!
//! Every vertex taken out is remembered against the triangle now nearest
//! it, and handed on to whichever triangle replaces that one. The deviation
//! is the furthest any of the surface's own vertices has been left from the
//! triangle holding it, which is what the limit is held to: a collapse that
//! would leave one further off than that is not made.

use std::sync::atomic::{AtomicBool, Ordering};

use glam::DVec3;
use rayon::prelude::*;

use crate::model::{formats::mesh_data, progress::Phase};

const NONE: u32 = u32::MAX;
/// An open edge's upright plane, as a multiple of its length squared: about
/// twenty of the triangles beside it.
const EDGE_WEIGHT: f64 = 10.0;
/// What a collapse costs for its side's length to the fourth power, which
/// puts the short sides of flat ground first and keeps its triangles even.
const LENGTH_COST: f64 = 1e-9;
/// The least cosine between a triangle's normal before and after a collapse.
const MIN_TURN_COS: f64 = 0.1;
/// The thinnest triangle a collapse may leave, as twice its area over its
/// longest side squared: about a thousand times longer than wide.
const MIN_SHAPE: f64 = 1e-3;
/// Collapses between looks at the cancel flag.
const POLL: usize = 4096;

pub(crate) struct Settings {
    /// The share of the triangles to stop at, 0 to 1.
    pub(crate) keep: f64,
    /// The furthest the surface may be moved, if limited.
    pub(crate) max_deviation: Option<f64>,
}

pub(crate) struct Simplified {
    pub(crate) vertices: Vec<mesh_data::Vertex>,
    pub(crate) faces: Vec<[u32; 3]>,
    /// The furthest the surface was moved, by the measure the limit uses.
    pub(crate) deviation: f64,
}

/// `None` when cancelled.
pub(crate) fn simplify(source: &mesh_data::Triangulation, settings: &Settings, cancel: &AtomicBool, progress: &Phase) -> Option<Simplified> {
    let (vertices, mut faces) = crate::model::triangulation::weld_identical_points(source.vertices().to_vec(), source.face_indices().collect());
    faces.retain(|face| face[0] != face[1] && face[1] != face[2] && face[0] != face[2]);
    let origin = {
        let centre = source.bounds().center();
        DVec3::new(centre.x, centre.y, centre.z)
    };
    let positions: Vec<DVec3> = vertices.par_iter().map(|vertex| DVec3::new(vertex.x, vertex.y, vertex.z) - origin).collect();

    let start_faces = faces.len();
    let target = ((start_faces as f64 * settings.keep.clamp(0.0, 1.0)).round() as usize).max(1);
    let limit = settings.max_deviation.unwrap_or(f64::INFINITY);
    let limit_sq = limit * limit;
    let mut mesh = Mesh::new(positions, faces, vertices, origin);
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    progress.set_fraction(0.1);

    let keys: Vec<f32> = (0..mesh.positions.len() as u32)
        .into_par_iter()
        .map_init(Vec::new, |ring, vertex| mesh.key(vertex, ring))
        .collect();
    let mut heap = Heap::new(keys);
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    progress.set_fraction(0.2);
    let collapsing = progress.phase(0.2, 1.0);

    // The cost up to which a vertex's sides have been tried and refused, so
    // one that comes round again tries only the dearer ones. Cleared by any
    // collapse beside it, which may have opened a way.
    let mut refused = vec![f32::NEG_INFINITY; mesh.positions.len()];
    let mut scratch = Scratch::default();
    let mut candidates: Vec<(f64, u32, DVec3)> = Vec::new();
    let mut live = start_faces;
    let mut turn = 0usize;
    while live > target {
        let Some(vertex) = heap.top() else {
            break;
        };
        turn += 1;
        if turn.is_multiple_of(POLL) {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            collapsing.set_items((start_faces - live) as u64, (start_faces - target) as u64);
        }
        let key = heap.keys[vertex as usize];
        mesh.ring(vertex, &mut scratch.ring);
        candidates.clear();
        candidates.extend(scratch.ring.iter().map(|&other| {
            let (cost, point) = mesh.candidate(vertex, other);
            (cost, other, point)
        }));
        candidates.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

        // A vertex's key is the least any of its sides could cost. Collapses
        // round it only ever raise that, so it is brought up to date here,
        // when it reaches the top, and not at each of them.
        let mut next = Next::Drop;
        for &(cost, other, point) in &candidates {
            if cost as f32 <= refused[vertex as usize] {
                continue;
            }
            if cost as f32 > key {
                next = Next::Raise(cost as f32);
                break;
            }
            if mesh.check(vertex, other, point, limit_sq, &mut scratch) {
                next = Next::Collapse(other, point);
                break;
            }
            refused[vertex as usize] = cost as f32;
        }
        match next {
            Next::Drop => heap.remove(vertex),
            Next::Raise(key) => heap.set(vertex, key),
            Next::Collapse(other, point) => {
                heap.remove(vertex);
                live -= mesh.collapse(vertex, other, point, &mut scratch);
                let key = mesh.key(other, &mut scratch.ring);
                heap.set(other, key);
                refused[other as usize] = f32::NEG_INFINITY;
                // Vertices with nothing left to collapse were dropped from the
                // heap, and are put back.
                mesh.ring(other, &mut scratch.ring);
                for index in 0..scratch.ring.len() {
                    let beside = scratch.ring[index];
                    let retry = refused[beside as usize] > f32::NEG_INFINITY;
                    refused[beside as usize] = f32::NEG_INFINITY;
                    if retry || !heap.holds(beside) {
                        let key = mesh.key(beside, &mut scratch.other_ring);
                        heap.set(beside, key);
                    }
                }
            }
        }
    }
    drop(heap);

    let deviation = mesh.deviation();
    let (vertices, faces) = mesh.finish();
    Some(Simplified { vertices, faces, deviation })
}

enum Next {
    /// Nothing round the vertex can be collapsed.
    Drop,
    Raise(f32),
    Collapse(u32, DVec3),
}

#[derive(Default)]
struct Scratch {
    ring: Vec<u32>,
    other_ring: Vec<u32>,
    corners: Vec<u32>,
    pairs: Vec<[u32; 2]>,
    /// The triangles a collapse would leave round its point.
    fan: Vec<(u32, [DVec3; 3])>,
    /// The vertices already taken out that those triangles would hold: each
    /// with its place in `fan`.
    held: Vec<(u32, u32)>,
}

/// Squared distances from a set of weighted planes, as the symmetric form
/// `p·Ap + 2b·p + c`: A's upper triangle, then b, then c.
#[derive(Clone, Copy, Default)]
struct Quadric([f64; 10]);

impl Quadric {
    fn plane(normal: DVec3, offset: f64, weight: f64) -> Self {
        let DVec3 { x, y, z } = normal;
        Self([x * x, x * y, x * z, y * y, y * z, z * z, x * offset, y * offset, z * offset, offset * offset].map(|value| value * weight))
    }

    fn add(&mut self, other: &Self) {
        for (value, added) in self.0.iter_mut().zip(other.0) {
            *value += added;
        }
    }

    fn times(&self, p: DVec3) -> DVec3 {
        let [a, b, c, d, e, f, ..] = self.0;
        DVec3::new(a * p.x + b * p.y + c * p.z, b * p.x + d * p.y + e * p.z, c * p.x + e * p.y + f * p.z)
    }

    fn linear(&self) -> DVec3 {
        DVec3::new(self.0[6], self.0[7], self.0[8])
    }

    fn trace(&self) -> f64 {
        self.0[0] + self.0[3] + self.0[5]
    }

    fn error(&self, p: DVec3) -> f64 {
        (p.dot(self.times(p)) + 2.0 * self.linear().dot(p) + self.0[9]).max(0.0)
    }

    /// The point of least error, where the planes fix one: on flat ground and
    /// along a straight crest they do not.
    fn lowest(&self) -> Option<DVec3> {
        let [a, b, c, d, e, f, ..] = self.0;
        let (m00, m01, m02) = (d * f - e * e, c * e - b * f, b * e - c * d);
        let det = a * m00 + b * m01 + c * m02;
        let scale = self.trace() / 3.0;
        if det.abs().partial_cmp(&(1e-8 * scale * scale * scale)).is_none_or(std::cmp::Ordering::is_le) {
            return None;
        }
        let (m11, m12, m22) = (a * f - c * c, b * c - a * e, a * d - b * b);
        let g = self.linear();
        Some(-DVec3::new(m00 * g.x + m01 * g.y + m02 * g.z, m01 * g.x + m11 * g.y + m12 * g.z, m02 * g.x + m12 * g.y + m22 * g.z) / det)
    }
}

/// The surface being collapsed. Each vertex's triangles are a linked list of
/// corners (triangle × 3 + which corner), so a collapse joins two lists in
/// place and nothing is ever searched for. A removed triangle's indices are
/// all `NONE`, and no list holds one once the collapse that removed it ends.
struct Mesh {
    positions: Vec<DVec3>,
    faces: Vec<[u32; 3]>,
    first_corner: Vec<u32>,
    next_corner: Vec<u32>,
    quadrics: Vec<Quadric>,
    /// On the open edge, or on a side more than two triangles share.
    on_edge: Vec<bool>,
    /// The surface as it came, for the vertices taken out of it.
    originals: Vec<mesh_data::Vertex>,
    origin: DVec3,
    /// Whether the vertex is still where it came.
    unmoved: Vec<bool>,
    /// The vertices taken out, as a list for each triangle of those it now
    /// holds: the first, and each one's next.
    first_held: Vec<u32>,
    next_held: Vec<u32>,
}

impl Mesh {
    fn new(positions: Vec<DVec3>, faces: Vec<[u32; 3]>, originals: Vec<mesh_data::Vertex>, origin: DVec3) -> Self {
        let count = positions.len();
        let mut first_corner = vec![NONE; count];
        let mut next_corner = vec![NONE; faces.len() * 3];
        for (face, indices) in faces.iter().enumerate() {
            for (slot, &vertex) in indices.iter().enumerate() {
                let corner = (face * 3 + slot) as u32;
                next_corner[corner as usize] = first_corner[vertex as usize];
                first_corner[vertex as usize] = corner;
            }
        }
        let mut mesh = Self {
            positions,
            first_held: vec![NONE; faces.len()],
            faces,
            first_corner,
            next_corner,
            quadrics: vec![Quadric::default(); count],
            on_edge: vec![false; count],
            originals,
            origin,
            unmoved: vec![true; count],
            next_held: vec![NONE; count],
        };

        for face in 0..mesh.faces.len() {
            let indices = mesh.faces[face];
            let [a, b, c] = indices.map(|vertex| mesh.positions[vertex as usize]);
            let cross = (b - a).cross(c - a);
            let length = cross.length();
            if length > 0.0 {
                let normal = cross / length;
                let quadric = Quadric::plane(normal, -normal.dot(a), length * 0.5);
                for vertex in indices {
                    mesh.quadrics[vertex as usize].add(&quadric);
                }
            }
        }

        // Sides one triangle has to itself are the open edge; those more than
        // two share are held the same way.
        let edges: Vec<(u32, u32, u32)> = (0..mesh.faces.len() as u32)
            .into_par_iter()
            .flat_map_iter(|face| {
                let indices = mesh.faces[face as usize];
                let mesh = &mesh;
                (0..3).filter_map(move |slot| {
                    let (from, to) = (indices[slot], indices[(slot + 1) % 3]);
                    let sharing = mesh.corners(from).filter(|&corner| mesh.faces[corner as usize / 3].contains(&to)).count();
                    (sharing != 2).then_some((from, to, face))
                })
            })
            .collect();
        for (from, to, face) in edges {
            mesh.on_edge[from as usize] = true;
            mesh.on_edge[to as usize] = true;
            let [a, b, c] = mesh.faces[face as usize].map(|vertex| mesh.positions[vertex as usize]);
            let (start, end) = (mesh.positions[from as usize], mesh.positions[to as usize]);
            let side = end - start;
            let upright = side.cross((b - a).cross(c - a));
            let length = upright.length();
            if length > 0.0 {
                let normal = upright / length;
                let quadric = Quadric::plane(normal, -normal.dot(start), side.length_squared() * EDGE_WEIGHT);
                mesh.quadrics[from as usize].add(&quadric);
                mesh.quadrics[to as usize].add(&quadric);
            }
        }
        mesh
    }

    fn corners(&self, vertex: u32) -> impl Iterator<Item = u32> + '_ {
        let first = self.first_corner[vertex as usize];
        std::iter::successors((first != NONE).then_some(first), |&corner| {
            let next = self.next_corner[corner as usize];
            (next != NONE).then_some(next)
        })
    }

    /// The vertices joined to this one, in order of index.
    fn ring(&self, vertex: u32, ring: &mut Vec<u32>) {
        ring.clear();
        for corner in self.corners(vertex) {
            let face = self.faces[corner as usize / 3];
            let slot = corner as usize % 3;
            ring.push(face[(slot + 1) % 3]);
            ring.push(face[(slot + 2) % 3]);
        }
        ring.sort_unstable();
        ring.dedup();
    }

    /// What collapsing the side between two vertices would cost, and the
    /// point it would leave.
    fn candidate(&self, vertex: u32, other: u32) -> (f64, DVec3) {
        let mut quadric = self.quadrics[vertex as usize];
        quadric.add(&self.quadrics[other as usize]);
        let (a, b) = (self.positions[vertex as usize], self.positions[other as usize]);
        let side = b - a;
        let length_sq = side.length_squared();
        // Where the planes leave the point free, or put it far off the side
        // (nearly flat ground does), it stays on the side.
        let point = quadric
            .lowest()
            .filter(|point| point.distance_squared((a + b) * 0.5) <= 4.0 * length_sq)
            .unwrap_or_else(|| {
                let curve = side.dot(quadric.times(side));
                let along = if curve > 1e-12 * quadric.trace() * length_sq {
                    (-side.dot(quadric.times(a) + quadric.linear()) / curve).clamp(0.0, 1.0)
                } else {
                    0.5
                };
                a + side * along
            });
        (quadric.error(point) + LENGTH_COST * length_sq * length_sq, point)
    }

    /// The least any side at the vertex could cost; infinite if it has none.
    fn key(&self, vertex: u32, ring: &mut Vec<u32>) -> f32 {
        self.ring(vertex, ring);
        ring.iter().map(|&other| self.candidate(vertex, other).0 as f32).fold(f32::INFINITY, f32::min)
    }

    fn original(&self, vertex: u32) -> DVec3 {
        let original = self.originals[vertex as usize];
        DVec3::new(original.x, original.y, original.z) - self.origin
    }

    fn held(&self, face: u32) -> impl Iterator<Item = u32> + '_ {
        let first = self.first_held[face as usize];
        std::iter::successors((first != NONE).then_some(first), |&held| {
            let next = self.next_held[held as usize];
            (next != NONE).then_some(next)
        })
    }

    /// Whether the side may be collapsed to the point. If so, `scratch` is
    /// left holding the triangles round the point and the vertices each is
    /// to hold, for [`Mesh::collapse`].
    fn check(&self, vertex: u32, other: u32, point: DVec3, limit_sq: f64, scratch: &mut Scratch) -> bool {
        let Scratch {
            ring,
            other_ring,
            pairs,
            fan,
            held,
            ..
        } = scratch;
        let sharing = self.corners(vertex).filter(|&corner| self.faces[corner as usize / 3].contains(&other)).count();
        if !(1..=2).contains(&sharing) || (sharing == 2 && self.on_edge[vertex as usize] && self.on_edge[other as usize]) {
            return false;
        }
        // The two rings may meet only at the corners of the triangles on the
        // side, or the collapse pinches the surface.
        self.ring(vertex, ring);
        self.ring(other, other_ring);
        let (mut i, mut j, mut common) = (0, 0, 0);
        while i < ring.len() && j < other_ring.len() {
            match ring[i].cmp(&other_ring[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    common += 1;
                    i += 1;
                    j += 1;
                }
            }
        }
        if common != sharing {
            return false;
        }

        fan.clear();
        pairs.clear();
        for (end, gone) in [(vertex, other), (other, vertex)] {
            for corner in self.corners(end) {
                let face = self.faces[corner as usize / 3];
                if face.contains(&gone) {
                    continue;
                }
                let slot = corner as usize % 3;
                let (second, third) = (face[(slot + 1) % 3], face[(slot + 2) % 3]);
                let [a, b, c] = [end, second, third].map(|index| self.positions[index as usize]);
                let before = (b - a).cross(c - a);
                let after = (b - point).cross(c - point);
                if before.length_squared() > 0.0 && before.dot(after) <= MIN_TURN_COS * before.length() * after.length() {
                    return false;
                }
                // Nor leave a needle, unless it was a thinner one before.
                let shape = |cross: DVec3, corner: DVec3| cross.length() / (b - corner).length_squared().max((c - corner).length_squared()).max((c - b).length_squared());
                let thin = shape(after, point);
                if thin < MIN_SHAPE && thin < shape(before, a) {
                    return false;
                }
                // Nor tip one over in plan: a batter may steepen, but ground
                // that had one elevation at every point keeps to that.
                if before.z * after.z <= 0.0 && before.z != 0.0 {
                    return false;
                }
                fan.push((corner / 3, [point, b, c]));
                pairs.push([second.min(third), second.max(third)]);
            }
        }
        // Nothing left, or two triangles on the same three vertices (the
        // last of a closed shape folding flat).
        if fan.is_empty() {
            return false;
        }
        pairs.sort_unstable();
        if pairs.windows(2).any(|pair| pair[0] == pair[1]) {
            return false;
        }

        // Each vertex already taken out round here, and the two ends if they
        // have not moved, goes to the nearest of the new triangles.
        held.clear();
        let mut hold = |taken: u32| {
            let from = self.original(taken);
            let (mut nearest, mut least) = (0, f64::INFINITY);
            for (index, &(_, triangle)) in fan.iter().enumerate() {
                let distance_sq = distance_sq_to_triangle(from, triangle);
                if distance_sq < least {
                    (nearest, least) = (index, distance_sq);
                }
            }
            held.push((taken, nearest as u32));
            least <= limit_sq
        };
        for (end, gone) in [(vertex, other), (other, vertex)] {
            if self.unmoved[end as usize] && !hold(end) {
                return false;
            }
            for corner in self.corners(end) {
                let face = corner / 3;
                // The triangles on the side itself are met from both ends.
                if end == other && self.faces[face as usize].contains(&gone) {
                    continue;
                }
                for taken in self.held(face) {
                    if !hold(taken) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Collapse the side onto `other`, at the point. Returns the triangles
    /// removed.
    fn collapse(&mut self, vertex: u32, other: u32, point: DVec3, scratch: &mut Scratch) -> usize {
        scratch.corners.clear();
        scratch.ring.clear();
        let mut removed = 0;
        let mut corner = self.first_corner[vertex as usize];
        while corner != NONE {
            let next = self.next_corner[corner as usize];
            let face = corner as usize / 3;
            let slot = corner as usize % 3;
            if self.faces[face].contains(&other) {
                // Its third vertex's list still holds it.
                scratch.ring.extend(self.faces[face].iter().filter(|&&index| index != vertex && index != other));
                self.faces[face] = [NONE; 3];
                self.first_held[face] = NONE;
                removed += 1;
            } else {
                self.faces[face][slot] = other;
                scratch.corners.push(corner);
            }
            corner = next;
        }
        self.first_corner[vertex as usize] = NONE;
        scratch.corners.extend(self.corners(other).filter(|&corner| self.faces[corner as usize / 3][0] != NONE));
        self.relink(other, &scratch.corners);
        for index in 0..scratch.ring.len() {
            let third = scratch.ring[index];
            scratch.corners.clear();
            scratch.corners.extend(self.corners(third).filter(|&corner| self.faces[corner as usize / 3][0] != NONE));
            self.relink(third, &scratch.corners);
        }

        let quadric = self.quadrics[vertex as usize];
        self.quadrics[other as usize].add(&quadric);
        self.positions[other as usize] = point;
        self.on_edge[other as usize] |= self.on_edge[vertex as usize];

        self.unmoved[vertex as usize] = false;
        self.unmoved[other as usize] = false;
        for &(face, _) in &scratch.fan {
            self.first_held[face as usize] = NONE;
        }
        for &(taken, place) in &scratch.held {
            let face = scratch.fan[place as usize].0 as usize;
            self.next_held[taken as usize] = self.first_held[face];
            self.first_held[face] = taken;
        }
        removed
    }

    fn relink(&mut self, vertex: u32, corners: &[u32]) {
        self.first_corner[vertex as usize] = corners.first().copied().unwrap_or(NONE);
        for (index, &corner) in corners.iter().enumerate() {
            self.next_corner[corner as usize] = corners.get(index + 1).copied().unwrap_or(NONE);
        }
    }

    /// The furthest a vertex taken out is from the triangle holding it.
    fn deviation(&self) -> f64 {
        (0..self.faces.len() as u32)
            .into_par_iter()
            .filter(|&face| self.first_held[face as usize] != NONE)
            .map(|face| {
                let triangle = self.faces[face as usize].map(|vertex| self.positions[vertex as usize]);
                self.held(face).map(|taken| distance_sq_to_triangle(self.original(taken), triangle)).fold(0.0, f64::max)
            })
            .reduce(|| 0.0, f64::max)
            .sqrt()
    }

    /// The triangles left, and the vertices they use.
    fn finish(self) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
        let Self { positions, mut faces, origin, .. } = self;
        faces.retain(|face| face[0] != NONE);
        let mut renumbered = vec![NONE; positions.len()];
        let mut vertices = Vec::new();
        for face in &mut faces {
            for index in face {
                let slot = &mut renumbered[*index as usize];
                if *slot == NONE {
                    *slot = vertices.len() as u32;
                    let position = positions[*index as usize] + origin;
                    vertices.push(mesh_data::Vertex::new(position.x, position.y, position.z));
                }
                *index = *slot;
            }
        }
        (vertices, faces)
    }
}

/// Squared distance from a point to a triangle (Ericson, Real-Time Collision
/// Detection 5.1.5).
fn distance_sq_to_triangle(p: DVec3, [a, b, c]: [DVec3; 3]) -> f64 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return ap.length_squared();
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return bp.length_squared();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (ap - ab * (d1 / (d1 - d3))).length_squared();
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return cp.length_squared();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (ap - ac * (d2 / (d2 - d6))).length_squared();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let along = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (bp - (c - b) * along).length_squared();
    }
    let sum = va + vb + vc;
    if sum.partial_cmp(&0.0).is_none_or(std::cmp::Ordering::is_le) {
        return ap.length_squared().min(bp.length_squared()).min(cp.length_squared());
    }
    (ap - ab * (vb / sum) - ac * (vc / sum)).length_squared()
}

/// Vertices by key, least first, each knowing its place so its key can change.
struct Heap {
    order: Vec<u32>,
    place: Vec<u32>,
    keys: Vec<f32>,
}

impl Heap {
    fn new(keys: Vec<f32>) -> Self {
        let order: Vec<u32> = (0..keys.len() as u32).filter(|&vertex| keys[vertex as usize].is_finite()).collect();
        let mut place = vec![NONE; keys.len()];
        for (index, &vertex) in order.iter().enumerate() {
            place[vertex as usize] = index as u32;
        }
        let mut heap = Self { order, place, keys };
        for index in (0..heap.order.len() / 2).rev() {
            heap.sink(index);
        }
        heap
    }

    fn top(&self) -> Option<u32> {
        self.order.first().copied()
    }

    fn holds(&self, vertex: u32) -> bool {
        self.place[vertex as usize] != NONE
    }

    fn set(&mut self, vertex: u32, key: f32) {
        if !key.is_finite() {
            self.remove(vertex);
            return;
        }
        self.keys[vertex as usize] = key;
        let index = match self.place[vertex as usize] {
            NONE => {
                self.order.push(vertex);
                self.place[vertex as usize] = (self.order.len() - 1) as u32;
                self.order.len() - 1
            }
            index => index as usize,
        };
        let index = self.rise(index);
        self.sink(index);
    }

    fn remove(&mut self, vertex: u32) {
        let index = self.place[vertex as usize];
        if index == NONE {
            return;
        }
        let index = index as usize;
        self.place[vertex as usize] = NONE;
        let last = self.order.pop();
        if index < self.order.len()
            && let Some(last) = last
        {
            self.order[index] = last;
            self.place[last as usize] = index as u32;
            let index = self.rise(index);
            self.sink(index);
        }
    }

    fn key_at(&self, index: usize) -> f32 {
        self.keys[self.order[index] as usize]
    }

    fn swap(&mut self, a: usize, b: usize) {
        self.order.swap(a, b);
        self.place[self.order[a] as usize] = a as u32;
        self.place[self.order[b] as usize] = b as u32;
    }

    fn rise(&mut self, mut index: usize) -> usize {
        while index > 0 {
            let parent = (index - 1) / 2;
            if self.key_at(parent) <= self.key_at(index) {
                break;
            }
            self.swap(index, parent);
            index = parent;
        }
        index
    }

    fn sink(&mut self, mut index: usize) {
        loop {
            let mut least = index;
            for child in [2 * index + 1, 2 * index + 2] {
                if child < self.order.len() && self.key_at(child) < self.key_at(least) {
                    least = child;
                }
            }
            if least == index {
                break;
            }
            self.swap(index, least);
            index = least;
        }
    }
}
