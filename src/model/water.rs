//! Rain on a triangulated surface: where it runs, where it ponds, and how
//! deep.
//!
//! The flow is solved on the surface's own vertices. Each vertex holds a
//! depth of water and stands for a third of the plan area of every triangle
//! round it; water moves along the triangles' sides, from the higher water
//! level to the lower, as fast as Manning's formula lets a sheet of that depth
//! run down that fall (the diffusive wave: the fall of the water surface
//! drives the flow and friction holds it back, with no momentum carried from
//! one step to the next).
//!
//! A step is stable at any length: slivers and near-vertical batter faces
//! cost accuracy in time, never a blow-up. What a long step blurs is when
//! water arrives, not where it ends up or how much of it. See [`simulate`]
//! for how a step is taken. No depth goes below zero and the water balance
//! is exact.
//!
//! The ground is one roughness and one infiltration rate for the whole
//! surface. Water leaves over the surface's open edge, at the rate the same
//! sheet would carry on down the ground behind it.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use rayon::prelude::*;

use crate::model::{formats::mesh_data, progress::Progress};

/// The least fall of the water surface Manning's formula is asked about: a
/// pond's surface is flat, and the formula's conductance has no limit there.
const MIN_SLOPE: f64 = 1e-4;
/// The most a triangle's corner counts towards a side's conductance, as a
/// cotangent: a sliver a hundred times longer than wide.
const COT_LIMIT: f64 = 100.0;
/// The least conductance weight a side keeps, so no vertex is cut off by the
/// obtuse triangles round it.
const MIN_WEIGHT: f64 = 0.01;
const CG_TOLERANCE: f64 = 1e-4;
const CG_ITERATIONS: usize = 200;
/// The most a backed-up side conducts in a step, as a multiple of the
/// volume a metre of depth is at its smaller end. Manning's conductance has
/// no limit as a pond's surface flattens, and what it would add past this is
/// only how many steps a wide pond takes to come level, at many times the
/// work.
const POOL_EXCHANGE: f64 = 200.0;
/// The least area a vertex holds water over, as a share of the mean.
const MIN_AREA_SHARE: f64 = 0.05;
/// Rows below which a pool solve stays on one thread.
const SHARED_ROWS: usize = 16_384;
/// Depths below this do not flow.
const FILM: f64 = 1e-6;
/// The fall the ground is taken to have beyond the open edge, least and most.
const EDGE_SLOPE: (f64, f64) = (0.001, 0.05);
/// Memory the kept frames may take, and the most of them.
const FRAME_BYTES: usize = 384 << 20;
const FRAMES: (usize, usize) = (24, 240);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WaterSettings {
    /// Rain over the whole storm, millimetres, falling evenly over
    /// `rain_hours`.
    pub(crate) rain_mm: f64,
    pub(crate) rain_hours: f64,
    /// Hours the run carries on for once the rain has stopped.
    pub(crate) run_on_hours: f64,
    /// Manning's n of the ground.
    pub(crate) roughness: f64,
    /// Millimetres an hour the ground takes in wherever water lies on it.
    pub(crate) infiltration_mm_h: f64,
    /// Seconds a step.
    pub(crate) step_s: f64,
    /// Split any triangle side longer than this in plan, metres.
    pub(crate) subdivide_m: Option<f64>,
}

impl Default for WaterSettings {
    fn default() -> Self {
        Self {
            rain_mm: 20.0,
            rain_hours: 4.0,
            run_on_hours: 4.0,
            roughness: 0.035,
            infiltration_mm_h: 2.0,
            step_s: 10.0,
            subdivide_m: None,
        }
    }
}

impl WaterSettings {
    pub(crate) fn is_valid(&self) -> bool {
        let positive = |value: f64| value.is_finite() && value > 0.0;
        positive(self.rain_mm)
            && positive(self.rain_hours)
            && self.run_on_hours.is_finite()
            && self.run_on_hours >= 0.0
            && positive(self.roughness)
            && self.infiltration_mm_h.is_finite()
            && self.infiltration_mm_h >= 0.0
            && positive(self.step_s)
            && self.subdivide_m.is_none_or(positive)
    }

    pub(crate) fn duration_s(&self) -> f64 {
        (self.rain_hours + self.run_on_hours) * 3600.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WaterError {
    /// The surface has no area in plan to rain on.
    Empty,
    Cancelled,
}

/// The surface a run flows over: the source mesh with coincident vertices
/// made one, and its long sides split.
pub(crate) struct Ground {
    pub(crate) vertices: Vec<[f64; 3]>,
    pub(crate) faces: Vec<[u32; 3]>,
}

impl Ground {
    pub(crate) fn from_mesh(mesh: &mesh_data::Triangulation) -> Self {
        // A surface read as loose faces has a vertex of its own at every
        // corner, and no water would cross from one face to the next.
        let mut welded = std::collections::HashMap::with_capacity(mesh.vertex_count());
        let mut vertices = Vec::new();
        let remap: Vec<u32> = mesh
            .vertices()
            .iter()
            .map(|vertex| {
                *welded.entry([vertex.x.to_bits(), vertex.y.to_bits(), vertex.z.to_bits()]).or_insert_with(|| {
                    vertices.push([vertex.x, vertex.y, vertex.z]);
                    (vertices.len() - 1) as u32
                })
            })
            .collect();
        let faces = mesh
            .face_indices()
            .map(|face| face.map(|index| remap[index as usize]))
            .filter(|[a, b, c]| a != b && b != c && c != a)
            .collect();
        Self { vertices, faces }
    }

    /// Split every side longer than `longest` in plan, and again until none
    /// is. A midpoint is shared by the triangles either side of it, so the
    /// surface stays in one piece.
    pub(crate) fn subdivided(mut self, longest: f64, cancel: &AtomicBool) -> Result<Self, WaterError> {
        let limit = longest * longest;
        for _ in 0..32 {
            if cancel.load(Ordering::Relaxed) {
                return Err(WaterError::Cancelled);
            }
            let mut midpoints: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
            let mut faces = Vec::with_capacity(self.faces.len());
            let vertices = &mut self.vertices;
            let is_long = |vertices: &[[f64; 3]], a: u32, b: u32| {
                let (p, q) = (vertices[a as usize], vertices[b as usize]);
                (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) > limit
            };
            let mut midpoint = |vertices: &mut Vec<[f64; 3]>, a: u32, b: u32| {
                *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    let (p, q) = (vertices[a as usize], vertices[b as usize]);
                    vertices.push([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, (p[2] + q[2]) / 2.0]);
                    (vertices.len() - 1) as u32
                })
            };
            for &[a, b, c] in &self.faces {
                let long = [is_long(vertices, a, b), is_long(vertices, b, c), is_long(vertices, c, a)];
                match long.iter().filter(|long| **long).count() {
                    0 => faces.push([a, b, c]),
                    3 => {
                        let (ab, bc, ca) = (midpoint(vertices, a, b), midpoint(vertices, b, c), midpoint(vertices, c, a));
                        faces.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
                    }
                    1 => {
                        // Turned so the long side is the first.
                        let [a, b, c] = if long[0] {
                            [a, b, c]
                        } else if long[1] {
                            [b, c, a]
                        } else {
                            [c, a, b]
                        };
                        let ab = midpoint(vertices, a, b);
                        faces.extend([[a, ab, c], [ab, b, c]]);
                    }
                    _ => {
                        // Turned so the short side is the last.
                        let [a, b, c] = if !long[2] {
                            [a, b, c]
                        } else if !long[0] {
                            [b, c, a]
                        } else {
                            [c, a, b]
                        };
                        let (ab, bc) = (midpoint(vertices, a, b), midpoint(vertices, b, c));
                        faces.extend([[ab, b, bc], [a, ab, bc], [a, bc, c]]);
                    }
                }
            }
            let done = midpoints.is_empty();
            self.faces = faces;
            if done {
                break;
            }
        }
        Ok(self)
    }

    pub(crate) fn to_mesh(&self) -> Option<mesh_data::Triangulation> {
        let vertices = self.vertices.iter().map(|&[x, y, z]| mesh_data::Vertex::new(x, y, z)).collect();
        mesh_data::Triangulation::from_vertices_and_faces(vertices, self.faces.clone()).ok()
    }
}

/// What the flow is solved over, worked out once from the ground.
struct Operators {
    /// Plan area each vertex stands for, which is what it catches rain
    /// over.
    catchment: Vec<f64>,
    /// The area its water is held over: the same, but never a sliver. A
    /// vertex among faces that stand on end has next to no plan area of its
    /// own, and whatever passed through it would stand kilometres deep there
    /// and run off in every direction.
    area: Vec<f64>,
    /// Each triangle side once, lower index first.
    edges: Vec<[u32; 2]>,
    /// The side's share of the flow between its ends (the cotangent weight).
    weight: Vec<f64>,
    /// The side's length in plan.
    length: Vec<f64>,
    /// Each vertex's sides: `adjacent[starts[v]..starts[v + 1]]`, as the
    /// vertex at the other end and the side's index.
    starts: Vec<u32>,
    adjacent: Vec<(u32, u32)>,
    /// Metres of open edge a vertex stands on, zero inside the surface, and
    /// the fall of the ground taken to carry on beyond it.
    outlet_width: Vec<f64>,
    outlet_slope: Vec<f64>,
}

impl Operators {
    fn new(ground: &Ground) -> Self {
        let vertices = &ground.vertices;
        let count = vertices.len();
        let mut area = vec![0.0; count];
        // Every side of every face, as its ends and the cotangent of the
        // corner across from it; sorted, the two faces' copies of a side lie
        // together.
        let mut sides: Vec<(u32, u32, f64)> = Vec::with_capacity(ground.faces.len() * 3);
        for &face in &ground.faces {
            let corner = face.map(|index| vertices[index as usize]);
            let cross = (corner[1][0] - corner[0][0]) * (corner[2][1] - corner[0][1]) - (corner[1][1] - corner[0][1]) * (corner[2][0] - corner[0][0]);
            for index in face {
                area[index as usize] += cross.abs() / 6.0;
            }
            for apex in 0..3 {
                let (a, b, c) = (corner[apex], corner[(apex + 1) % 3], corner[(apex + 2) % 3]);
                let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
                let dot = u[0] * v[0] + u[1] * v[1];
                let reach = u[0] * u[0] + u[1] * u[1] + v[0] * v[0] + v[1] * v[1];
                let cot = (dot / cross.abs().max(1e-12 + 1e-9 * reach)).clamp(-COT_LIMIT, COT_LIMIT);
                let (from, to) = (face[(apex + 1) % 3], face[(apex + 2) % 3]);
                sides.push((from.min(to), from.max(to), cot / 2.0));
            }
        }
        sides.par_sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(a.2.total_cmp(&b.2)));

        let mut edges = Vec::new();
        let mut weight = Vec::new();
        let mut length = Vec::new();
        let mut outlet_width = vec![0.0; count];
        let mut at = 0;
        while at < sides.len() {
            let (a, b, _) = sides[at];
            let mut end = at;
            let mut sum = 0.0;
            while end < sides.len() && sides[end].0 == a && sides[end].1 == b {
                sum += sides[end].2;
                end += 1;
            }
            let (p, q) = (vertices[a as usize], vertices[b as usize]);
            let plan = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt();
            if end - at == 1 {
                outlet_width[a as usize] += plan / 2.0;
                outlet_width[b as usize] += plan / 2.0;
            }
            edges.push([a, b]);
            weight.push(sum.max(MIN_WEIGHT));
            length.push(plan.max(1e-3));
            at = end;
        }

        let mut starts = vec![0u32; count + 1];
        for &[a, b] in &edges {
            starts[a as usize + 1] += 1;
            starts[b as usize + 1] += 1;
        }
        for vertex in 0..count {
            starts[vertex + 1] += starts[vertex];
        }
        let mut next = starts.clone();
        let mut adjacent = vec![(0u32, 0u32); edges.len() * 2];
        for (edge, &[a, b]) in edges.iter().enumerate() {
            adjacent[next[a as usize] as usize] = (b, edge as u32);
            next[a as usize] += 1;
            adjacent[next[b as usize] as usize] = (a, edge as u32);
            next[b as usize] += 1;
        }

        let outlet_slope = (0..count)
            .map(|vertex| {
                if outlet_width[vertex] <= 0.0 {
                    return 0.0;
                }
                let steepest = adjacent[starts[vertex] as usize..starts[vertex + 1] as usize]
                    .iter()
                    .map(|&(other, edge)| (vertices[other as usize][2] - vertices[vertex][2]) / length[edge as usize])
                    .fold(0.0, f64::max);
                steepest.clamp(EDGE_SLOPE.0, EDGE_SLOPE.1)
            })
            .collect();

        let catchment = area.clone();
        let least = MIN_AREA_SHARE * area.iter().sum::<f64>() / count.max(1) as f64;
        for area in &mut area {
            *area = area.max(least).max(1e-6);
        }
        Self {
            catchment,
            area,
            edges,
            weight,
            length,
            starts,
            adjacent,
            outlet_width,
            outlet_slope,
        }
    }

    fn sides_of(&self, vertex: usize) -> &[(u32, u32)] {
        &self.adjacent[self.starts[vertex] as usize..self.starts[vertex + 1] as usize]
    }
}

/// The water at one kept instant of a run.
pub(crate) struct WaterFrame {
    pub(crate) time_s: f64,
    /// Metres of water at each vertex.
    pub(crate) depth: Vec<f32>,
    /// How far from the kept instant before this one to this one the water
    /// last reached each vertex, in 255ths: 0 where it was there all along,
    /// or never came. What lets the drawn water advance between two kept
    /// instants as it did, not appear everywhere it got to at once.
    pub(crate) arrival: Vec<u8>,
    /// Cubic metres lying on the surface.
    pub(crate) on_surface: f64,
}

/// Water at least this deep counts as having reached a vertex, in metres:
/// the shallowest that is drawn.
pub(crate) const ARRIVED_DEPTH: f64 = 0.003;

/// Sum of the products of two vectors, added up in the same order whatever
/// the thread count.
fn dot(a: &[f64], b: &[f64]) -> f64 {
    const CHUNK: usize = 8192;
    // Too little to be worth waking the pool for, and in the same chunks
    // either way.
    if a.len() < SHARED_ROWS {
        return a.chunks(CHUNK).zip(b.chunks(CHUNK)).map(|(a, b)| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>()).sum();
    }
    let parts: Vec<f64> = a
        .par_chunks(CHUNK)
        .zip(b.par_chunks(CHUNK))
        .map(|(a, b)| a.iter().zip(b).map(|(a, b)| a * b).sum())
        .collect();
    parts.iter().sum()
}

/// One number per vertex that the threads of a hand-down write as they reach
/// each vertex, and read once it has been written.
struct Cells(Vec<AtomicU64>);

impl Cells {
    fn new(count: usize) -> Self {
        Self((0..count).map(|_| AtomicU64::new(0)).collect())
    }

    fn get(&self, index: usize) -> f64 {
        f64::from_bits(self.0[index].load(Ordering::Relaxed))
    }

    fn set(&self, index: usize, value: f64) {
        self.0[index].store(value.to_bits(), Ordering::Relaxed);
    }
}

/// Fronts shorter than this are visited on one thread: a valley floor's long
/// tail of a few vertices each is not worth waking the pool for.
const SHARED_FRONT: usize = 256;
/// How many of the vertices it sets free a thread goes straight on to, for
/// each vertex of the front it was handed. Following the water down keeps
/// the fronts few; past this the rest wait for the next front, so one thread
/// is not left with a whole hillside.
const CHASE: usize = 4;

/// Hand water down from the highest level to the lowest, on every thread.
///
/// `waiting[v]` is how many of `v`'s neighbours give to it, and `visit` is
/// called on a vertex once all of them have been visited: with the vertices
/// none give to first, then those the first set free, and so on, each front
/// shared among the threads, and each thread going a little way on down from
/// the vertices it visits. `visit` sets a vertex's takers free with
/// [`release`]. What it works out for a vertex must depend only on the
/// vertices that give to it, so the answer is the same in any order and on any
/// number of threads.
fn hand_down(waiting: &[AtomicU32], visit: impl Fn(u32, &mut Vec<u32>) + Sync) {
    let mut front: Vec<u32> = (0..waiting.len() as u32)
        .into_par_iter()
        .with_min_len(SHARED_FRONT)
        .filter(|vertex| waiting[*vertex as usize].load(Ordering::Relaxed) == 0)
        .collect();
    // Visit a share of a front, and add what it leaves free to `next`.
    let run = |chunk: &[u32], mut next: Vec<u32>, mut budget: usize| {
        let mut freed = Vec::new();
        for &vertex in chunk {
            visit(vertex, &mut freed);
            while budget > 0
                && let Some(vertex) = freed.pop()
            {
                visit(vertex, &mut freed);
                budget -= 1;
            }
        }
        next.append(&mut freed);
        next
    };
    while !front.is_empty() {
        front = if front.len() < SHARED_FRONT {
            run(&front, Vec::new(), SHARED_FRONT * CHASE)
        } else {
            front
                .par_chunks(SHARED_FRONT / 8)
                .fold(Vec::new, |next, chunk| run(chunk, next, chunk.len() * CHASE))
                .reduce(Vec::new, |mut all, mut more| {
                    all.append(&mut more);
                    all
                })
        };
    }
}

/// One of `taker`'s givers has been visited: it joins the next front once
/// the last has.
fn release(waiting: &[AtomicU32], taker: u32, next: &mut Vec<u32>) {
    if waiting[taker as usize].fetch_sub(1, Ordering::AcqRel) == 1 {
        next.push(taker);
    }
}

/// The backed-up water of one step: the vertices joined by a side whose
/// water stands over the ground at both ends, and the linear system that
/// levels them.
#[derive(Default)]
struct Pools {
    /// Each vertex's row here, or `u32::MAX`.
    row_of: Vec<u32>,
    /// The vertex of each row.
    vertices: Vec<u32>,
    /// Each row's sides: `entries[starts[row]..starts[row + 1]]`, as the row
    /// at the other end and the side's conductance.
    starts: Vec<u32>,
    entries: Vec<(u32, f32)>,
    diagonal: Vec<f64>,
}

impl Pools {
    /// Gather the sides with any `conductance` and the vertices on them.
    fn gather(&mut self, operators: &Operators, conductance: &[f32], storage: &[f64]) {
        let count = operators.area.len();
        self.row_of.resize(count, u32::MAX);
        for &vertex in &self.vertices {
            self.row_of[vertex as usize] = u32::MAX;
        }
        let wet = |vertex: usize| operators.sides_of(vertex).iter().filter(move |(_, edge)| conductance[*edge as usize] > 0.0);
        self.vertices.clear();
        self.vertices
            .par_extend((0..count as u32).into_par_iter().with_min_len(4096).filter(|vertex| wet(*vertex as usize).next().is_some()));
        for (row, &vertex) in self.vertices.iter().enumerate() {
            self.row_of[vertex as usize] = row as u32;
        }
        let row_of = &self.row_of;
        self.entries.clear();
        self.entries.par_extend(
            self.vertices
                .par_iter()
                .with_min_len(4096)
                .flat_map_iter(|vertex| wet(*vertex as usize).map(|&(other, edge)| (row_of[other as usize], conductance[edge as usize]))),
        );
        self.vertices
            .par_iter()
            .with_min_len(4096)
            .map(|vertex| wet(*vertex as usize).fold(storage[*vertex as usize], |sum, (_, edge)| sum + f64::from(conductance[*edge as usize])))
            .collect_into_vec(&mut self.diagonal);
        self.starts.clear();
        self.starts.push(0);
        let mut end = 0;
        for &vertex in &self.vertices {
            end += wet(vertex as usize).count() as u32;
            self.starts.push(end);
        }
    }

    fn row(&self, row: usize) -> &[(u32, f32)] {
        &self.entries[self.starts[row] as usize..self.starts[row + 1] as usize]
    }

    fn apply(&self, x: &[f64], out: &mut Vec<f64>) {
        let row = |row: usize| {
            let across: f64 = self.row(row).iter().map(|&(other, conductance)| f64::from(conductance) * x[other as usize]).sum();
            self.diagonal[row] * x[row] - across
        };
        if x.len() < SHARED_ROWS {
            out.clear();
            out.extend((0..x.len()).map(row));
        } else {
            (0..x.len()).into_par_iter().with_min_len(4096).map(row).collect_into_vec(out);
        }
    }

    /// Conjugate gradients, preconditioned by the diagonal, from `start`.
    fn solve(&self, rhs: &[f64], start: Vec<f64>) -> Vec<f64> {
        let count = rhs.len();
        let mut x = start;
        let target = CG_TOLERANCE * CG_TOLERANCE * dot(rhs, rhs);
        if target <= 0.0 {
            return vec![0.0; count];
        }
        let mut residual = Vec::with_capacity(count);
        self.apply(&x, &mut residual);
        residual
            .par_iter_mut()
            .zip(rhs)
            .with_min_len(SHARED_ROWS)
            .for_each(|(residual, rhs)| *residual = rhs - *residual);
        if dot(&residual, &residual) <= target {
            return x;
        }
        let mut scaled: Vec<f64> = residual.par_iter().zip(&self.diagonal).with_min_len(SHARED_ROWS).map(|(r, d)| r / d).collect();
        let mut direction = scaled.clone();
        let mut along = Vec::with_capacity(count);
        let mut rz = dot(&residual, &scaled);
        // Each sum is taken in the pass that makes its vectors, in chunks
        // added up in the same order whatever the thread count.
        const CHUNK: usize = 8192;
        along.resize(count, 0.0);
        for _ in 0..CG_ITERATIONS {
            let parts: Vec<f64> = along
                .par_chunks_mut(CHUNK)
                .enumerate()
                .map(|(chunk, along)| {
                    let first = chunk * CHUNK;
                    let mut sum = 0.0;
                    for (row, along) in (first..).zip(along) {
                        let across: f64 = self.row(row).iter().map(|&(other, conductance)| f64::from(conductance) * direction[other as usize]).sum();
                        *along = self.diagonal[row] * direction[row] - across;
                        sum += direction[row] * *along;
                    }
                    sum
                })
                .collect();
            let curvature: f64 = parts.iter().sum();
            if curvature.partial_cmp(&0.0).is_none_or(std::cmp::Ordering::is_le) {
                break;
            }
            let alpha = rz / curvature;
            let parts: Vec<(f64, f64)> = (
                x.par_chunks_mut(CHUNK),
                residual.par_chunks_mut(CHUNK),
                scaled.par_chunks_mut(CHUNK),
                direction.par_chunks(CHUNK),
                along.par_chunks(CHUNK),
                self.diagonal.par_chunks(CHUNK),
            )
                .into_par_iter()
                .map(|(x, residual, scaled, direction, along, diagonal)| {
                    let (mut rr, mut rz) = (0.0, 0.0);
                    for (((x, r), z), ((p, q), d)) in x.iter_mut().zip(residual).zip(scaled).zip(direction.iter().zip(along).zip(diagonal)) {
                        *x += alpha * p;
                        *r -= alpha * q;
                        *z = *r / d;
                        rr += *r * *r;
                        rz += *r * *z;
                    }
                    (rr, rz)
                })
                .collect();
            if parts.iter().map(|(rr, _)| rr).sum::<f64>() <= target {
                break;
            }
            let next: f64 = parts.iter().map(|(_, rz)| rz).sum();
            let beta = next / rz;
            rz = next;
            direction.par_iter_mut().zip(&scaled).with_min_len(CHUNK).for_each(|(p, z)| *p = z + beta * *p);
        }
        x
    }
}

/// The cube root of the depth a vertex is left with after a step of
/// draining freely: it held `volume` over `area`, and loses
/// `rate * depth^(5/3)` a second for `step` seconds, at the depth it ends the
/// step with. `guess` is the answer it had last step, or zero.
///
/// In the cube root the balance is a polynomial, `area t^3 + step rate t^5 =
/// volume`, so a flowing sheet's depth is found without a root or a power
/// taken: Newton's steps from the last answer, which is nearly this one.
fn drained_root(volume: f64, area: f64, rate: f64, step: f64, guess: f64) -> f64 {
    let run = step * rate;
    if run <= 0.0 {
        return (volume / area).cbrt();
    }
    let newton = |mut root: f64, steps: usize| {
        for _ in 0..steps {
            let square = root * root;
            let next = root - (square * root * (area + run * square) - volume) / (square * (3.0 * area + 5.0 * run * square));
            if (root - next).abs() <= 1e-7 * root {
                return Ok(next);
            }
            root = next;
        }
        Err(root)
    };
    // The balance rises and curves upward, so a step from below the answer
    // lands above it and every one after comes down towards it. From a poor
    // guess that descent is slow; start again from above, where the two terms
    // each bound the answer.
    let near = if guess > 0.0 { newton(guess, 6) } else { Err(0.0) };
    let root = near.or_else(|_| newton((volume / area).cbrt().min((volume / run).powf(0.2)), 40));
    root.unwrap_or_else(|root| root).max(0.0)
}

/// Run the storm in `settings` over `ground`.
///
/// A step has two halves. Water whose way down is clear - the water below
/// stands lower than the ground it is on - is handed downhill from the highest
/// level to the lowest, each vertex passing on what a sheet of the depth it
/// ends the step with would carry, so a thin film can cross any number of
/// triangles in a step and still settle at the depth its flow needs. Water
/// that is backed up - standing over the ground at both ends of a side - has
/// its levels solved together, which is what flattens a pond.
///
/// Both halves run on every thread (see [`hand_down`]), and the answer is the
/// same on any number of them.
pub(crate) fn simulate(ground: &Ground, settings: &WaterSettings, cancel: &AtomicBool, progress: &Progress, keep: &mut dyn FnMut(WaterFrame)) -> Result<(), WaterError> {
    let count = ground.vertices.len();
    let operators = Operators::new(ground);
    let total_area: f64 = operators.catchment.iter().sum();
    if ground.faces.is_empty() || total_area <= 1e-3 {
        return Err(WaterError::Empty);
    }
    let bed: Vec<f64> = ground.vertices.iter().map(|vertex| vertex[2]).collect();
    let roughness = settings.roughness;
    let step = settings.step_s;
    let steps = (settings.duration_s() / step).ceil().max(1.0) as usize;
    let frames_kept = (FRAME_BYTES / (count * size_of::<f32>()).max(1)).clamp(FRAMES.0, FRAMES.1);
    let frame_every = steps.div_ceil(frames_kept).max(1);
    let rain_end = settings.rain_hours * 3600.0;
    let rain_rate = settings.rain_mm / 1000.0 / rain_end;
    let soak = settings.infiltration_mm_h / 1000.0 / 3600.0 * step;
    let storage: Vec<f64> = operators.area.iter().map(|area| area / step).collect();
    // What a metre of depth to the five-thirds carries over the open edge.
    let outlet: Vec<f64> = (0..count)
        .map(|vertex| operators.outlet_width[vertex] * operators.outlet_slope[vertex].sqrt() / roughness)
        .collect();

    // Beside each vertex's sides, in the same order, so the cascade reads
    // them in one run of memory: what the side carries for a metre of depth
    // to the five-thirds down a fall of one in one, and one over its length.
    let reach: Vec<(f64, f64)> = operators
        .adjacent
        .iter()
        .map(|&(_, edge)| {
            (
                operators.weight[edge as usize] * operators.length[edge as usize] / roughness,
                1.0 / operators.length[edge as usize],
            )
        })
        .collect();

    let mut depth = vec![0.0f64; count];
    let mut arrival = vec![0u8; count];
    let mut reached = vec![false; count];
    keep(frame(&operators, &depth, std::mem::take(&mut arrival), 0.0));
    arrival = vec![0u8; count];

    let mut level = vec![0.0; count];
    let waiting: Vec<AtomicU32> = (0..count).map(|_| AtomicU32::new(0)).collect();
    // What each vertex is left with by the hand-down, and what it passed on
    // for each unit of its sides' rates.
    let kept = Cells::new(count);
    let passed = Cells::new(count);
    let mut conductance = vec![0.0f32; operators.edges.len()];
    let mut pools = Pools::default();
    // The cube root of the depth each vertex last drained to.
    let roots = Cells::new(count);
    // Each vertex's change of level in the last pool solve it was part of.
    let mut carried = vec![0.0; count];

    for index in 0..steps {
        if cancel.load(Ordering::Relaxed) {
            return Err(WaterError::Cancelled);
        }
        let time = index as f64 * step;

        // Rain on, then what the ground takes in.
        let falling = rain_rate * (rain_end - time).clamp(0.0, step);
        depth.par_iter_mut().enumerate().with_min_len(4096).for_each(|(vertex, depth)| {
            *depth = (*depth + falling * operators.catchment[vertex] / operators.area[vertex] - soak).max(0.0);
        });

        // Free-draining water, handed down from the highest level to the
        // lowest: everything a vertex is given has arrived by the time its
        // own turn comes. A vertex takes from the neighbours whose ground is
        // above its water, so it waits on those and no others.
        level.par_iter_mut().zip(bed.par_iter().zip(&depth)).for_each(|(level, (bed, depth))| *level = bed + depth);
        waiting.par_iter().enumerate().with_min_len(4096).for_each(|(vertex, waiting)| {
            let givers = operators.sides_of(vertex).iter().filter(|(other, _)| level[vertex] < bed[*other as usize]).count();
            waiting.store(givers as u32, Ordering::Relaxed);
        });
        hand_down(&waiting, |vertex, next| {
            let vertex = vertex as usize;
            let sides = operators.starts[vertex] as usize..operators.starts[vertex + 1] as usize;
            let area = operators.area[vertex];
            // Each side's rate: Manning's, for a metre of depth to the
            // five-thirds, down the fall to the water below. A side that
            // gives carries what its upper end passed on at the same rate.
            let mut given = 0.0;
            let mut rate = outlet[vertex];
            for (&(other, _), &(carry, per_metre)) in operators.adjacent[sides.clone()].iter().zip(&reach[sides.clone()]) {
                let other = other as usize;
                if level[vertex] < bed[other] {
                    let passed = passed.get(other);
                    if passed > 0.0 {
                        given += passed * (carry * ((level[other] - level[vertex]) * per_metre).max(MIN_SLOPE).sqrt());
                    }
                } else if level[other] < bed[vertex] {
                    rate += carry * ((level[vertex] - level[other]) * per_metre).max(MIN_SLOPE).sqrt();
                }
            }
            let held = area * depth[vertex] + given;
            if held <= area * FILM {
                kept.set(vertex, held / area);
                passed.set(vertex, 0.0);
            } else {
                let root = drained_root(held, area, rate, step, roots.get(vertex));
                roots.set(vertex, root);
                // No more than it held: the rest is what it passes on.
                let left_here = (root * root * root).min(held / area);
                kept.set(vertex, left_here);
                passed.set(vertex, if rate > 0.0 { (held - area * left_here).max(0.0) / rate } else { 0.0 });
            }
            for &(other, _) in &operators.adjacent[sides] {
                if level[other as usize] < bed[vertex] {
                    release(&waiting, other, next);
                }
            }
        });
        depth.par_iter_mut().enumerate().with_min_len(4096).for_each(|(vertex, depth)| *depth = kept.get(vertex));

        // Backed-up water: sides whose lower water stands over the higher
        // ground. Their levels are solved together.
        level.par_iter_mut().zip(bed.par_iter().zip(&depth)).for_each(|(level, (bed, depth))| *level = bed + depth);
        conductance.par_iter_mut().enumerate().for_each(|(edge, conductance)| {
            let [a, b] = operators.edges[edge].map(|vertex| vertex as usize);
            let (high, low) = if level[a] >= level[b] { (a, b) } else { (b, a) };
            let deep = level[high] - bed[a].max(bed[b]);
            *conductance = if level[low] >= bed[high] && deep > FILM {
                let fall = ((level[high] - level[low]) / operators.length[edge]).max(MIN_SLOPE);
                (operators.weight[edge] * deep.powf(5.0 / 3.0) / (roughness * fall.sqrt())).min(POOL_EXCHANGE * storage[a].min(storage[b])) as f32
            } else {
                0.0
            };
        });
        pools.gather(&operators, &conductance, &storage);
        if !pools.vertices.is_empty() {
            // Solved for the change in level, whose right-hand side is the
            // flow the levels as they stand would drive.
            let rows = pools.vertices.len();
            let rhs: Vec<f64> = (0..rows)
                .into_par_iter()
                .with_min_len(4096)
                .map(|row| {
                    let here = level[pools.vertices[row] as usize];
                    pools
                        .row(row)
                        .iter()
                        .map(|&(other, conductance)| f64::from(conductance) * (level[pools.vertices[other as usize] as usize] - here))
                        .sum()
                })
                .collect();
            // From the change each vertex made last step: a pond filling
            // steadily rises much as it just did.
            let change = pools.solve(&rhs, pools.vertices.iter().map(|vertex| carried[*vertex as usize]).collect());
            for (change, vertex) in change.iter().zip(&pools.vertices) {
                carried[*vertex as usize] = *change;
            }
            let solved: Vec<f64> = change.iter().zip(&pools.vertices).map(|(change, vertex)| level[*vertex as usize] + change).collect();
            // Handed down in order of the solved levels, a vertex never
            // passing on more than it holds and was given: `share` is how
            // much of what the solved levels ask of it a row could pass on.
            let waiting: Vec<AtomicU32> = (0..rows)
                .into_par_iter()
                .with_min_len(4096)
                .map(|row| AtomicU32::new(pools.row(row).iter().filter(|(other, _)| solved[*other as usize] > solved[row]).count() as u32))
                .collect();
            let share = Cells::new(rows);
            let settled = Cells::new(rows);
            hand_down(&waiting, |row, next| {
                let row = row as usize;
                let vertex = pools.vertices[row] as usize;
                let (mut given, mut out) = (0.0, 0.0);
                for &(other, conductance) in pools.row(row) {
                    let (other, conductance) = (other as usize, f64::from(conductance));
                    if solved[other] > solved[row] {
                        given += share.get(other) * conductance * (solved[other] - solved[row]) * step;
                    } else {
                        out += conductance * (solved[row] - solved[other]).max(0.0) * step;
                    }
                }
                let held = operators.area[vertex] * depth[vertex] + given;
                let passed = if out > held { held / out } else { 1.0 };
                share.set(row, passed);
                settled.set(row, ((held - passed * out) / operators.area[vertex]).max(0.0));
                for &(other, _) in pools.row(row) {
                    if solved[row] > solved[other as usize] {
                        release(&waiting, other, next);
                    }
                }
            });
            for (row, vertex) in pools.vertices.iter().enumerate() {
                depth[*vertex as usize] = settled.get(row);
            }
        }

        // When, between the last kept instant and the next, water came.
        let since = index % frame_every;
        let part = ((since * 255) / frame_every).max(1) as u8;
        arrival.par_iter_mut().zip(reached.par_iter_mut()).zip(&depth).for_each(|((arrival, reached), depth)| {
            let wet = *depth >= ARRIVED_DEPTH;
            if wet && !*reached && since > 0 {
                *arrival = part;
            }
            *reached = wet;
        });
        if (index + 1) % frame_every == 0 || index + 1 == steps {
            keep(frame(&operators, &depth, std::mem::replace(&mut arrival, vec![0u8; count]), (index + 1) as f64 * step));
        }
        progress.set_items(index as u64 + 1, steps as u64);
    }

    Ok(())
}

fn frame(operators: &Operators, depth: &[f64], arrival: Vec<u8>, time_s: f64) -> WaterFrame {
    WaterFrame {
        time_s,
        arrival,
        depth: depth.iter().map(|depth| *depth as f32).collect(),
        on_surface: depth.iter().zip(&operators.area).map(|(depth, area)| depth * area).sum(),
    }
}

/// The shallowest water that is taken to run at all, in metres.
const FLOW_DEPTH: f32 = 1e-3;
/// The fastest the drawn water is taken to run, in metres a second: a film
/// down a batter face is all slope and no depth, and Manning's formula was
/// never meant for it.
const FLOW_LIMIT: f64 = 8.0;

/// How fast the water at each of the ground's vertices is running in plan,
/// in metres a second, for `depth` at each: Manning's speed for the sheet on
/// each triangle down the fall of its water level, averaged at each vertex
/// over the triangles round it by their plan area.
///
/// For drawing only. The run itself moves water along the triangles' sides
/// and keeps no velocities.
pub(crate) fn flow_velocity(ground: &Ground, depth: &[f32], roughness: f64) -> Vec<[f32; 2]> {
    // Each triangle's velocity times its plan area, and that area.
    let per_face: Vec<[f32; 3]> = ground
        .faces
        .par_iter()
        .map(|face| {
            let corners = face.map(|index| ground.vertices[index as usize]);
            let deep = face.map(|index| depth[index as usize]);
            let wet = deep.map(|deep| deep >= FLOW_DEPTH);
            let wet_count = wet.iter().filter(|wet| **wet).count();
            if wet_count == 0 {
                return [0.0; 3];
            }
            let mean_depth = (0..3).filter(|&corner| wet[corner]).map(|corner| f64::from(deep[corner])).sum::<f64>() / wet_count as f64;
            let mean_level = (0..3).filter(|&corner| wet[corner]).map(|corner| corners[corner][2] + f64::from(deep[corner])).sum::<f64>() / wet_count as f64;
            // Dry ground standing over the water gives it nothing to run
            // down from; dry ground under it is where it is running to.
            let level: [f64; 3] = std::array::from_fn(|corner| {
                if wet[corner] {
                    corners[corner][2] + f64::from(deep[corner])
                } else {
                    corners[corner][2].min(mean_level)
                }
            });
            let (ux, uy) = (corners[1][0] - corners[0][0], corners[1][1] - corners[0][1]);
            let (vx, vy) = (corners[2][0] - corners[0][0], corners[2][1] - corners[0][1]);
            let cross = ux * vy - uy * vx;
            if cross.abs() < 1e-9 {
                return [0.0; 3];
            }
            let (rise_u, rise_v) = (level[1] - level[0], level[2] - level[0]);
            let fall_x = -(rise_u * vy - rise_v * uy) / cross;
            let fall_y = -(rise_v * ux - rise_u * vx) / cross;
            let fall = fall_x.hypot(fall_y);
            if fall < 1e-9 {
                return [0.0, 0.0, (cross.abs() / 2.0) as f32];
            }
            let speed = (mean_depth.powf(2.0 / 3.0) * fall.sqrt() / roughness).min(FLOW_LIMIT);
            let area = cross.abs() / 2.0;
            [(fall_x / fall * speed * area) as f32, (fall_y / fall * speed * area) as f32, area as f32]
        })
        .collect();
    let mut summed = vec![[0.0f32; 3]; ground.vertices.len()];
    for (face, share) in ground.faces.iter().zip(&per_face) {
        if share[2] > 0.0 {
            for &corner in face {
                let sum = &mut summed[corner as usize];
                sum[0] += share[0];
                sum[1] += share[1];
                sum[2] += share[2];
            }
        }
    }
    summed
        .into_iter()
        .zip(depth)
        .map(|([x, y, area], deep)| if area > 0.0 && *deep >= FLOW_DEPTH { [x / area, y / area] } else { [0.0; 2] })
        .collect()
}
