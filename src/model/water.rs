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

use std::sync::atomic::{AtomicBool, Ordering};

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
const SHARED_ROWS: usize = 100_000;
/// Depths below this do not flow.
const FILM: f64 = 1e-6;
/// The fall the ground is taken to have beyond the open edge, least and most.
const EDGE_SLOPE: (f64, f64) = (0.001, 0.05);
/// Memory the kept frames may take, and the most of them.
const FRAME_BYTES: usize = 384 << 20;
const FRAMES: (usize, usize) = (24, 240);
/// Water at least this deep is part of a pond, and the least volume listed.
const POND_DEPTH: f64 = 0.05;
const POND_VOLUME: f64 = 1.0;
const MAX_PONDS: usize = 40;
/// Water at least this deep is drawn: the film of a rained-on slope is not,
/// only where it has gathered. The viewport lifts it [`DRAWN_LIFT`] clear of
/// the ground, so a shallow sheet does not flicker against it.
pub(crate) const DRAWN_DEPTH: f32 = 0.02;
pub(crate) const DRAWN_LIFT: f64 = 0.03;
/// The depths, in metres, the drawn water's bands start at.
pub(crate) const BAND_DEPTHS: [f32; 5] = [DRAWN_DEPTH, 0.1, 0.5, 1.5, 4.0];

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
            rain_mm: 100.0,
            rain_hours: 6.0,
            run_on_hours: 6.0,
            roughness: 0.035,
            infiltration_mm_h: 2.0,
            step_s: 30.0,
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

/// A body of standing water at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pond {
    /// Cubic metres.
    pub(crate) volume: f64,
    /// Square metres, in plan.
    pub(crate) area: f64,
    pub(crate) max_depth: f64,
    /// The water's level at its deepest point, and where that is.
    pub(crate) level: f64,
    pub(crate) deepest: [f64; 3],
}

/// The water at one kept instant of a run.
pub(crate) struct WaterFrame {
    pub(crate) time_s: f64,
    /// Metres of water at each vertex.
    pub(crate) depth: Vec<f32>,
    /// Cubic metres lying on the surface, and since the start: fallen as
    /// rain, soaked in, and gone over the open edge.
    pub(crate) on_surface: f64,
    pub(crate) rained: f64,
    pub(crate) infiltrated: f64,
    pub(crate) left: f64,
    /// The largest ponds, largest first.
    pub(crate) ponds: Vec<Pond>,
}

pub(crate) struct WaterRun {
    pub(crate) ground: Ground,
    pub(crate) frames: Vec<WaterFrame>,
    /// The deepest each vertex was at any step.
    pub(crate) max_depth: Vec<f32>,
    /// Plan area rained on, square metres.
    pub(crate) area: f64,
}

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
    entries: Vec<(u32, f64)>,
    diagonal: Vec<f64>,
}

impl Pools {
    /// Gather the sides with any `conductance` and the vertices on them.
    fn gather(&mut self, operators: &Operators, conductance: &[f64], storage: &[f64]) {
        self.row_of.clear();
        self.row_of.resize(operators.area.len(), u32::MAX);
        self.vertices.clear();
        for (edge, _) in conductance.iter().enumerate().filter(|(_, conductance)| **conductance > 0.0) {
            for vertex in operators.edges[edge] {
                if self.row_of[vertex as usize] == u32::MAX {
                    self.row_of[vertex as usize] = self.vertices.len() as u32;
                    self.vertices.push(vertex);
                }
            }
        }
        self.starts.clear();
        self.entries.clear();
        self.diagonal.clear();
        self.starts.push(0);
        for &vertex in &self.vertices {
            let mut sum = storage[vertex as usize];
            for &(other, edge) in operators.sides_of(vertex as usize) {
                let conductance = conductance[edge as usize];
                if conductance > 0.0 {
                    self.entries.push((self.row_of[other as usize], conductance));
                    sum += conductance;
                }
            }
            self.starts.push(self.entries.len() as u32);
            self.diagonal.push(sum);
        }
    }

    fn row(&self, row: usize) -> &[(u32, f64)] {
        &self.entries[self.starts[row] as usize..self.starts[row + 1] as usize]
    }

    fn apply(&self, x: &[f64], out: &mut Vec<f64>) {
        let row = |row: usize| {
            let across: f64 = self.row(row).iter().map(|&(other, conductance)| conductance * x[other as usize]).sum();
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
        for (residual, rhs) in residual.iter_mut().zip(rhs) {
            *residual = rhs - *residual;
        }
        if dot(&residual, &residual) <= target {
            return x;
        }
        let mut scaled: Vec<f64> = residual.iter().zip(&self.diagonal).map(|(r, d)| r / d).collect();
        let mut direction = scaled.clone();
        let mut along = Vec::with_capacity(count);
        let mut rz = dot(&residual, &scaled);
        for _ in 0..CG_ITERATIONS {
            self.apply(&direction, &mut along);
            let curvature = dot(&direction, &along);
            if curvature.partial_cmp(&0.0).is_none_or(std::cmp::Ordering::is_le) {
                break;
            }
            let alpha = rz / curvature;
            for ((x, r), (p, q)) in x.iter_mut().zip(&mut residual).zip(direction.iter().zip(&along)) {
                *x += alpha * p;
                *r -= alpha * q;
            }
            if dot(&residual, &residual) <= target {
                break;
            }
            for (z, (r, d)) in scaled.iter_mut().zip(residual.iter().zip(&self.diagonal)) {
                *z = r / d;
            }
            let next = dot(&residual, &scaled);
            let beta = next / rz;
            rz = next;
            for (p, z) in direction.iter_mut().zip(&scaled) {
                *p = z + beta * *p;
            }
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
pub(crate) fn simulate(ground: Ground, settings: &WaterSettings, cancel: &AtomicBool, progress: &Progress) -> Result<WaterRun, WaterError> {
    let count = ground.vertices.len();
    let operators = Operators::new(&ground);
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
    let mut max_depth = vec![0.0f32; count];
    let (mut rained, mut infiltrated, mut left) = (0.0, 0.0, 0.0);
    let mut frames = vec![frame(&ground, &operators, &depth, 0.0, [rained, infiltrated, left])];

    let mut level = vec![0.0; count];
    let mut order: Vec<u32> = (0..count as u32).collect();
    let mut inflow = vec![0.0; count];
    let mut rates: Vec<f64> = Vec::new();
    let mut conductance = vec![0.0; operators.edges.len()];
    let mut pools = Pools::default();
    // The cube root of the depth each vertex last drained to.
    let mut roots = vec![0.0; count];
    // Each vertex's change of level in the last pool solve it was part of.
    let mut carried = vec![0.0; count];

    for index in 0..steps {
        if cancel.load(Ordering::Relaxed) {
            return Err(WaterError::Cancelled);
        }
        let time = index as f64 * step;

        // Rain on, then what the ground takes in.
        let falling = rain_rate * (rain_end - time).clamp(0.0, step);
        for (vertex, depth) in depth.iter_mut().enumerate() {
            *depth += falling * operators.catchment[vertex] / operators.area[vertex];
            let soaked = depth.min(soak);
            *depth -= soaked;
            rained += falling * operators.catchment[vertex];
            infiltrated += soaked * operators.area[vertex];
        }

        // Free-draining water, handed down in order of level: everything a
        // vertex is given has arrived by the time its own turn comes.
        level.par_iter_mut().zip(bed.par_iter().zip(&depth)).for_each(|(level, (bed, depth))| *level = bed + depth);
        order.par_sort_unstable_by(|a, b| level[*b as usize].total_cmp(&level[*a as usize]).then(a.cmp(b)));
        inflow.fill(0.0);
        for &vertex in &order {
            let vertex = vertex as usize;
            let held = operators.area[vertex] * depth[vertex] + inflow[vertex];
            if held <= operators.area[vertex] * FILM {
                depth[vertex] = held / operators.area[vertex];
                continue;
            }
            // Each side's rate: Manning's, for a metre of depth to the
            // five-thirds, down the fall to the water below.
            rates.clear();
            let mut rate = outlet[vertex];
            let sides = operators.starts[vertex] as usize..operators.starts[vertex + 1] as usize;
            for (&(other, _), &(carry, per_metre)) in operators.adjacent[sides.clone()].iter().zip(&reach[sides]) {
                let clear = level[other as usize] < bed[vertex];
                let side = if clear {
                    let fall = ((level[vertex] - level[other as usize]) * per_metre).max(MIN_SLOPE);
                    carry * fall.sqrt()
                } else {
                    0.0
                };
                rates.push(side);
                rate += side;
            }
            let root = drained_root(held, operators.area[vertex], rate, step, roots[vertex]);
            roots[vertex] = root;
            // No more than it held: the rest is what it passes on.
            let kept = (root * root * root).min(held / operators.area[vertex]);
            depth[vertex] = kept;
            if rate > 0.0 {
                let passed = (held - operators.area[vertex] * kept).max(0.0) / rate;
                for (&(other, _), side) in operators.sides_of(vertex).iter().zip(&rates) {
                    inflow[other as usize] += passed * side;
                }
                left += passed * outlet[vertex];
            }
        }

        // Backed-up water: sides whose lower water stands over the higher
        // ground. Their levels are solved together.
        level.par_iter_mut().zip(bed.par_iter().zip(&depth)).for_each(|(level, (bed, depth))| *level = bed + depth);
        conductance.par_iter_mut().enumerate().for_each(|(edge, conductance)| {
            let [a, b] = operators.edges[edge].map(|vertex| vertex as usize);
            let (high, low) = if level[a] >= level[b] { (a, b) } else { (b, a) };
            let deep = level[high] - bed[a].max(bed[b]);
            *conductance = if level[low] >= bed[high] && deep > FILM {
                let fall = ((level[high] - level[low]) / operators.length[edge]).max(MIN_SLOPE);
                (operators.weight[edge] * deep.powf(5.0 / 3.0) / (roughness * fall.sqrt())).min(POOL_EXCHANGE * storage[a].min(storage[b]))
            } else {
                0.0
            };
        });
        pools.gather(&operators, &conductance, &storage);
        if !pools.vertices.is_empty() {
            // Solved for the change in level, whose right-hand side is the
            // flow the levels as they stand would drive.
            let rhs: Vec<f64> = (0..pools.vertices.len())
                .map(|row| {
                    let here = level[pools.vertices[row] as usize];
                    pools
                        .row(row)
                        .iter()
                        .map(|&(other, conductance)| conductance * (level[pools.vertices[other as usize] as usize] - here))
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
            // passing on more than it holds and was given.
            let mut rows: Vec<u32> = (0..pools.vertices.len() as u32).collect();
            rows.par_sort_unstable_by(|a, b| solved[*b as usize].total_cmp(&solved[*a as usize]).then(a.cmp(b)));
            let mut given = vec![0.0; rows.len()];
            for &row in &rows {
                let row = row as usize;
                let vertex = pools.vertices[row] as usize;
                let held = operators.area[vertex] * depth[vertex] + given[row];
                let out: f64 = pools
                    .row(row)
                    .iter()
                    .map(|&(other, conductance)| conductance * (solved[row] - solved[other as usize]).max(0.0) * step)
                    .sum();
                let share = if out > held { held / out } else { 1.0 };
                for &(other, conductance) in pools.row(row) {
                    given[other as usize] += share * conductance * (solved[row] - solved[other as usize]).max(0.0) * step;
                }
                depth[vertex] = ((held - share * out) / operators.area[vertex]).max(0.0);
            }
        }

        max_depth.par_iter_mut().zip(&depth).for_each(|(max, depth)| *max = max.max(*depth as f32));
        if (index + 1) % frame_every == 0 || index + 1 == steps {
            frames.push(frame(&ground, &operators, &depth, (index + 1) as f64 * step, [rained, infiltrated, left]));
        }
        progress.set_items(index as u64 + 1, steps as u64);
    }

    Ok(WaterRun {
        ground,
        frames,
        max_depth,
        area: total_area,
    })
}

fn frame(ground: &Ground, operators: &Operators, depth: &[f64], time_s: f64, [rained, infiltrated, left]: [f64; 3]) -> WaterFrame {
    WaterFrame {
        time_s,
        depth: depth.iter().map(|depth| *depth as f32).collect(),
        on_surface: depth.iter().zip(&operators.area).map(|(depth, area)| depth * area).sum(),
        rained,
        infiltrated,
        left,
        ponds: ponds(ground, operators, depth),
    }
}

/// The bodies of water at least [`POND_DEPTH`] deep, joined along the
/// triangles' sides.
fn ponds(ground: &Ground, operators: &Operators, depth: &[f64]) -> Vec<Pond> {
    let mut parent: Vec<u32> = (0..depth.len() as u32).collect();
    fn root(parent: &mut [u32], mut vertex: u32) -> u32 {
        while parent[vertex as usize] != vertex {
            parent[vertex as usize] = parent[parent[vertex as usize] as usize];
            vertex = parent[vertex as usize];
        }
        vertex
    }
    for &[a, b] in &operators.edges {
        if depth[a as usize] >= POND_DEPTH && depth[b as usize] >= POND_DEPTH {
            let (a, b) = (root(&mut parent, a), root(&mut parent, b));
            if a != b {
                parent[a.max(b) as usize] = a.min(b);
            }
        }
    }
    let mut found: std::collections::HashMap<u32, Pond> = std::collections::HashMap::new();
    for vertex in 0..depth.len() {
        let deep = depth[vertex];
        if deep < POND_DEPTH {
            continue;
        }
        let pond = found.entry(root(&mut parent, vertex as u32)).or_insert(Pond {
            volume: 0.0,
            area: 0.0,
            max_depth: 0.0,
            level: 0.0,
            deepest: ground.vertices[vertex],
        });
        pond.volume += deep * operators.area[vertex];
        pond.area += operators.area[vertex];
        if deep > pond.max_depth {
            pond.max_depth = deep;
            pond.level = ground.vertices[vertex][2] + deep;
            pond.deepest = ground.vertices[vertex];
        }
    }
    let mut ponds: Vec<Pond> = found.into_values().filter(|pond| pond.volume >= POND_VOLUME).collect();
    ponds.sort_unstable_by(|a, b| {
        b.volume
            .total_cmp(&a.volume)
            .then(a.deepest[0].total_cmp(&b.deepest[0]))
            .then(a.deepest[1].total_cmp(&b.deepest[1]))
    });
    ponds.truncate(MAX_PONDS);
    ponds
}

/// The water's surface as loose triangles, one list per band of
/// [`BAND_DEPTHS`], for `depth` at each of the ground's vertices.
///
/// A triangle wholly under water is lifted to the water's level at each
/// corner. One the waterline crosses is cut along it: its dry corners are
/// taken to be at the level of its wet ones, and the water stops where the
/// ground comes up through that. Every level is raised by `lift`.
pub(crate) fn water_surface(ground: &Ground, depth: &[f32], lift: f64) -> [Vec<[f64; 3]>; BAND_DEPTHS.len()] {
    const CHUNK: usize = 8192;
    let parts: Vec<[Vec<[f64; 3]>; BAND_DEPTHS.len()]> = ground
        .faces
        .par_chunks(CHUNK)
        .map(|faces| {
            let mut bands: [Vec<[f64; 3]>; BAND_DEPTHS.len()] = Default::default();
            for face in faces {
                let corners = face.map(|index| ground.vertices[index as usize]);
                let deep = face.map(|index| depth[index as usize]);
                let wet = deep.map(|deep| deep >= DRAWN_DEPTH);
                let wet_count = wet.iter().filter(|wet| **wet).count();
                if wet_count == 0 {
                    continue;
                }
                let mean_depth = (0..3).filter(|&corner| wet[corner]).map(|corner| deep[corner]).sum::<f32>() / wet_count as f32;
                let mean_level = (0..3).filter(|&corner| wet[corner]).map(|corner| corners[corner][2] + f64::from(deep[corner])).sum::<f64>() / wet_count as f64;
                // Each corner's water level, and how far that stands over its
                // ground: negative where the ground is above the water.
                let level: [f64; 3] = std::array::from_fn(|corner| {
                    if wet[corner] {
                        corners[corner][2] + f64::from(deep[corner])
                    } else {
                        mean_level.max(corners[corner][2])
                    }
                });
                let over: [f64; 3] = std::array::from_fn(|corner| {
                    if wet[corner] {
                        f64::from(deep[corner])
                    } else {
                        (mean_level - corners[corner][2]).min(0.0)
                    }
                });
                let mut outline: [[f64; 3]; 4] = [[0.0; 3]; 4];
                let mut points = 0;
                for corner in 0..3 {
                    let next = (corner + 1) % 3;
                    if over[corner] >= 0.0 {
                        outline[points] = [corners[corner][0], corners[corner][1], level[corner] + lift];
                        points += 1;
                    }
                    if (over[corner] < 0.0) != (over[next] < 0.0) {
                        let along = over[corner] / (over[corner] - over[next]);
                        let mix = |from: f64, to: f64| from + (to - from) * along;
                        // The dry end's level is the water's, carried across.
                        let (from, to) = (
                            if over[corner] < 0.0 { mean_level } else { level[corner] },
                            if over[next] < 0.0 { mean_level } else { level[next] },
                        );
                        outline[points] = [mix(corners[corner][0], corners[next][0]), mix(corners[corner][1], corners[next][1]), mix(from, to) + lift];
                        points += 1;
                    }
                }
                if points < 3 {
                    continue;
                }
                let band = BAND_DEPTHS.iter().rposition(|start| mean_depth >= *start).unwrap_or(0);
                bands[band].extend([outline[0], outline[1], outline[2]]);
                if points == 4 {
                    bands[band].extend([outline[0], outline[2], outline[3]]);
                }
            }
            bands
        })
        .collect();
    let mut bands: [Vec<[f64; 3]>; BAND_DEPTHS.len()] = Default::default();
    for part in parts {
        for (band, triangles) in bands.iter_mut().zip(part) {
            band.extend(triangles);
        }
    }
    bands
}
