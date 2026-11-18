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
//! A step is taken implicitly, the flows being those of the levels it ends
//! at: slivers and near-vertical batter faces cost accuracy in time, never a
//! blow-up, and a channel in flood runs steadily however much deeper than
//! the fall from one vertex to the next. A step too long for the water it
//! is moving is taken in parts, so any length of step gives an answer.
//! What a long step blurs is when water arrives, not where it ends up or how
//! much of it. See [`simulate`] for how a step is taken. No depth goes below
//! zero, and the water is kept exactly, vertex by vertex: a step ends with
//! each vertex holding what it had and what its sides brought it, less what
//! they and the open edge took, every flow at the solved levels.
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
const SOLVE_TOLERANCE: f64 = 1e-2;
const SOLVE_ITERATIONS: usize = 200;
/// The most a side as flat as a pond conducts in a step, as a multiple of the
/// volume a metre of depth is at its smaller end. Manning's conductance has
/// no limit as a pond's surface flattens, and what it would add past this is
/// only how many steps a wide pond takes to come level, at many times the
/// work.
const POOL_EXCHANGE: f64 = 200.0;
/// How far out the water held by a step's solved levels may be from what
/// there is, as a share of it, before the step is taken again in two parts;
/// and the shortest part taken, in seconds.
const BALANCE_LIMIT: f64 = 2e-3;
const LEAST_STEP: f64 = 0.5;
/// The most turns of Newton's method a step is given.
const NEWTON_TURNS: usize = 6;
/// What a vertex may be left short of its balance over a step, as a depth
/// in metres and as a share of the depth it stands at.
const SETTLED_DEPTH: f64 = 1e-4;
const SETTLED_SHARE: f64 = 1e-3;
/// The fall of a pond's surface. The limit on what a side conducts is for
/// water lying that flat: it rises in step with any fall steeper, so it
/// never holds back water that is on its way somewhere.
const POND_FALL: f64 = 1e-3;
/// The least area a vertex holds water over, as a share of the mean.
const MIN_AREA_SHARE: f64 = 0.05;
/// Rows below which a solve stays on one thread.
const SHARED_ROWS: usize = 16_384;
/// The sides a vertex hands water down by that there is room for without
/// asking for memory: a vertex of a triangulated surface has six sides or so
/// round it.
const OUT_SIDES: usize = 16;
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

/// The least rows of a product with the system a thread takes.
const PRODUCT_ROWS: usize = 1024;
/// The rows a sum is added up over at a time, each run on one thread.
const SUM_ROWS: usize = 8192;

/// The vertices looked through at once for any short of its balance: most
/// runs of them have none.
const SETTLED_RUN: usize = 256;
/// The fewest listed vertices or sides a thread takes.
const LISTED_ITEMS: usize = 1024;

/// `value` of each of `items`, in their order.
fn each<T: Send>(items: &[u32], value: impl Fn(usize) -> T + Sync) -> Vec<T> {
    items.par_iter().with_min_len(LISTED_ITEMS).map(|item| value(*item as usize)).collect()
}

/// `value` of each side at the `vertices`, which are those `listed`, with the
/// side's index: a side with both its ends listed is taken from the lower.
fn each_side<T: Send>(operators: &Operators, vertices: &[u32], listed: &[bool], value: impl Fn(usize) -> T + Sync) -> Vec<(u32, T)> {
    vertices
        .par_iter()
        .with_min_len(LISTED_ITEMS / 4)
        .flat_map_iter(|vertex| {
            let sides = operators.sides_of(*vertex as usize).iter();
            sides
                .filter(move |(other, _)| !listed[*other as usize] || vertex < other)
                .map(|&(_, edge)| (edge, value(edge as usize)))
        })
        .collect()
}

/// The sums each run of rows came to, added up in the runs' order.
fn sums<const N: usize>(runs: impl IndexedParallelIterator<Item = [f64; N]>) -> [f64; N] {
    let parts: Vec<[f64; N]> = runs.collect();
    std::array::from_fn(|sum| parts.iter().map(|part| part[sum]).sum())
}

/// Sum of the products of two vectors, added up in the same order whatever
/// the thread count.
fn dot(a: &[f64], b: &[f64]) -> f64 {
    const CHUNK: usize = SUM_ROWS;
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

/// The sum of `value` over `0..count`, added up in the same order whatever
/// the thread count.
fn total(count: usize, value: impl Fn(usize) -> f64 + Sync) -> f64 {
    const CHUNK: usize = 8192;
    let parts: Vec<f64> = (0..count.div_ceil(CHUNK))
        .into_par_iter()
        .map(|chunk| (chunk * CHUNK..((chunk + 1) * CHUNK).min(count)).map(&value).sum())
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
/// tail of a few vertices each is not worth waking the pool for. A vertex
/// with water on it is a few hundred nanoseconds' work, so it does not take
/// many to be.
const SHARED_FRONT: usize = 64;
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

/// One turn of a step's solve: the vertices whose levels it moves, and the
/// linear system for how far.
#[derive(Default)]
struct Pools {
    /// Each vertex's row here, or `u32::MAX`.
    row_of: Vec<u32>,
    /// The vertex of each row.
    vertices: Vec<u32>,
    /// Each row's sides to another row: `entries[starts[row]..starts[row + 1]]`,
    /// as the row at the other end and what a metre's rise there sends this
    /// way: the side's conductance, and where the other end is the upper,
    /// its deepening too. A side to a vertex with no row, whose level stays
    /// as it is, is only in the diagonal.
    starts: Vec<u32>,
    entries: Vec<(u32, f32)>,
    diagonal: Vec<f64>,
    /// How many of those sides each row has.
    ends: Vec<u32>,
}

impl Pools {
    /// Gather those of the `active` vertices, which are in order, that have
    /// a side with any `conductance`, and those sides. `deepening` is each
    /// side's, what more it carries for each metre its upper end's level
    /// rises, positive where its first end is the upper.
    ///
    /// The system is the step taken implicitly with every side's flow made
    /// linear about the levels as they stand, in the fall along it and in
    /// the depth it runs at, which is its upper end's: the upwind scheme of
    /// the implicit diffusive-wave models. The depth's part is what the
    /// lower end's row has and the upper end's has not, so the system is not
    /// symmetric; each column's diagonal outweighs the rest of it by the
    /// vertex's storage.
    fn gather(&mut self, operators: &Operators, conductance: &[f32], deepening: &[f32], active: &[u32], storage: &[f64]) {
        self.row_of.resize(operators.area.len(), u32::MAX);
        for &vertex in &self.vertices {
            self.row_of[vertex as usize] = u32::MAX;
        }
        let wet = |vertex: u32| {
            operators
                .sides_of(vertex as usize)
                .iter()
                .filter(move |(_, edge)| conductance[*edge as usize] > 0.0 || deepening[*edge as usize] != 0.0)
        };
        self.vertices.clear();
        self.vertices
            .par_extend(active.par_iter().with_min_len(4096).copied().filter(|vertex| wet(*vertex).next().is_some()));
        for (row, &vertex) in self.vertices.iter().enumerate() {
            self.row_of[vertex as usize] = row as u32;
        }
        let row_of = &self.row_of;
        // The side's deepening as `vertex`'s row holds it: positive in the
        // upper end's row, negative in the lower end's.
        let signed = |vertex: u32, edge: u32| {
            let value = deepening[edge as usize];
            if (value > 0.0) == (operators.edges[edge as usize][0] == vertex) {
                value.abs()
            } else {
                -value.abs()
            }
        };
        let across = |vertex: u32| {
            wet(vertex)
                .filter(|(other, _)| row_of[*other as usize] != u32::MAX)
                .map(move |&(other, edge)| (row_of[other as usize], conductance[edge as usize] + (-signed(vertex, edge)).max(0.0)))
        };
        self.entries.clear();
        self.entries.par_extend(self.vertices.par_iter().with_min_len(4096).flat_map_iter(|vertex| across(*vertex)));
        // Each row's diagonal, and where its sides end once the rows' counts
        // of them are added up.
        self.starts.clear();
        self.starts.push(0);
        self.vertices
            .par_iter()
            .with_min_len(4096)
            .map(|vertex| {
                let diagonal = wet(*vertex).fold(storage[*vertex as usize], |sum, (_, edge)| {
                    sum + f64::from(conductance[*edge as usize]) + f64::from(signed(*vertex, *edge).max(0.0))
                });
                (diagonal, across(*vertex).count() as u32)
            })
            .unzip_into_vecs(&mut self.diagonal, &mut self.ends);
        let mut end = 0;
        self.starts.extend(self.ends.iter().map(|sides| {
            end += sides;
            end
        }));
    }

    fn row(&self, row: usize) -> &[(u32, f32)] {
        &self.entries[self.starts[row] as usize..self.starts[row + 1] as usize]
    }

    /// One row of the system times `x`.
    fn product(&self, x: &[f64], row: usize) -> f64 {
        let across: f64 = self.row(row).iter().map(|&(other, sends)| f64::from(sends) * x[other as usize]).sum();
        self.diagonal[row] * x[row] - across
    }

    fn apply(&self, x: &[f64], out: &mut Vec<f64>) {
        // A metre's rise at the other end sends its conductance this way,
        // and where the other end is the upper, its deepening too.
        let row = |row: usize| self.product(x, row);
        if x.len() < SHARED_ROWS {
            out.clear();
            out.extend((0..x.len()).map(row));
        } else {
            (0..x.len()).into_par_iter().with_min_len(PRODUCT_ROWS).map(row).collect_into_vec(out);
        }
    }

    /// `x` over the diagonal.
    fn scaled(&self, x: &[f64], out: &mut Vec<f64>) {
        x.par_iter()
            .zip(&self.diagonal)
            .with_min_len(SHARED_ROWS)
            .map(|(x, diagonal)| x / diagonal)
            .collect_into_vec(out);
    }

    /// The stabilised biconjugate gradient method, preconditioned by the
    /// diagonal, from `start`. Every sum is added up in the same order
    /// whatever the thread count. The method's residual does not fall at
    /// every turn: the answer is the best it reached, and no change at all
    /// if it never bettered that.
    fn solve(&self, rhs: &[f64], start: Vec<f64>) -> Vec<f64> {
        let count = rhs.len();
        let size = dot(rhs, rhs);
        let target = SOLVE_TOLERANCE * SOLVE_TOLERANCE * size;
        if target <= 0.0 || !size.is_finite() {
            return vec![0.0; count];
        }
        let mut x = start;
        let mut residual = Vec::with_capacity(count);
        self.apply(&x, &mut residual);
        residual
            .par_iter_mut()
            .zip(rhs)
            .with_min_len(SHARED_ROWS)
            .for_each(|(residual, rhs)| *residual = rhs - *residual);
        let mut left = dot(&residual, &residual);
        // A start worse than none is not one.
        if left.partial_cmp(&size).is_none_or(std::cmp::Ordering::is_gt) {
            x = vec![0.0; count];
            residual.copy_from_slice(rhs);
            left = size;
        }
        let mut best = (left, x.clone());
        let shadow = residual.clone();
        let mut direction = residual.clone();
        // Between its two products with the system, each turn does all that
        // can be done before the next number it needs in one pass over the
        // rows. A sum is added up over runs
        // of `SUM_ROWS` rows and then over the runs, as `dot` does.
        let diagonal = &self.diagonal[..];
        let shadow = &shadow[..];
        let mut scaled = Vec::new();
        self.scaled(&direction, &mut scaled);
        let (mut along, mut half, mut across) = (vec![0.0; count], vec![0.0; count], vec![0.0; count]);
        let mut rho = dot(shadow, &residual);
        for _ in 0..SOLVE_ITERATIONS {
            if best.0 <= target {
                break;
            }
            self.apply(&scaled, &mut along);
            let curvature = dot(shadow, &along);
            if curvature == 0.0 || !curvature.is_finite() {
                break;
            }
            let alpha = rho / curvature;
            (x.par_chunks_mut(SUM_ROWS), residual.par_chunks_mut(SUM_ROWS), half.par_chunks_mut(SUM_ROWS))
                .into_par_iter()
                .enumerate()
                .for_each(|(run, (x, residual, half))| {
                    let first = run * SUM_ROWS;
                    for (at, ((x, residual), half)) in x.iter_mut().zip(residual).zip(half).enumerate() {
                        *x += alpha * scaled[first + at];
                        *residual += -alpha * along[first + at];
                        *half = *residual / diagonal[first + at];
                    }
                });
            self.apply(&half, &mut across);
            let [span, with_residual] = sums(across.par_chunks(SUM_ROWS).zip(residual.par_chunks(SUM_ROWS)).map(|(across, residual)| {
                let (mut span, mut with_residual) = (-0.0, -0.0);
                for (across, residual) in across.iter().zip(residual) {
                    span += across * across;
                    with_residual += across * residual;
                }
                [span, with_residual]
            }));
            if span == 0.0 || !span.is_finite() {
                break;
            }
            let omega = with_residual / span;
            let [remaining, next] = sums(
                (x.par_chunks_mut(SUM_ROWS), residual.par_chunks_mut(SUM_ROWS))
                    .into_par_iter()
                    .enumerate()
                    .map(|(run, (x, residual))| {
                        let first = run * SUM_ROWS;
                        let (mut remaining, mut next) = (-0.0, -0.0);
                        for (at, (x, residual)) in x.iter_mut().zip(residual).enumerate() {
                            *x += omega * half[first + at];
                            *residual += -omega * across[first + at];
                            remaining += *residual * *residual;
                            next += shadow[first + at] * *residual;
                        }
                        [remaining, next]
                    }),
            );
            left = remaining;
            if !left.is_finite() {
                break;
            }
            if left < best.0 {
                best.0 = left;
                best.1.copy_from_slice(&x);
            }
            if next == 0.0 || omega == 0.0 || !next.is_finite() {
                break;
            }
            let beta = next / rho * alpha / omega;
            rho = next;
            // direction = residual + beta (direction - omega along)
            (direction.par_chunks_mut(SUM_ROWS), scaled.par_chunks_mut(SUM_ROWS))
                .into_par_iter()
                .enumerate()
                .for_each(|(run, (direction, scaled))| {
                    let first = run * SUM_ROWS;
                    for (at, (direction, scaled)) in direction.iter_mut().zip(scaled).enumerate() {
                        *direction = beta * (*direction + -omega * along[first + at]) + residual[first + at];
                        *scaled = *direction / diagonal[first + at];
                    }
                });
        }
        best.1
    }
}

/// What a side hands down in cubic metres a second, and how much more for
/// each metre deeper its upper end stands: Manning's, for water standing
/// `over` the sill it has to get out by, whose cube root is `root`, and
/// falling `fall` to the level at the lower end, both above zero. `carry` is
/// what the side carries for a metre of depth to the five-thirds down a fall
/// of one in one, and `per_metre` one over its length.
///
/// The fall is the water's own, so a vertex that is given more than it can
/// pass on stands higher and passes on more: nothing piles up behind a fall
/// that was shallow when the step began.
fn flow_over(over: f64, root: f64, fall: f64, carry: f64, per_metre: f64) -> (f64, f64) {
    let depth = over * root * root;
    let slope = fall * per_metre;
    // Below the least fall Manning's formula is asked about, in proportion
    // to the fall, so the flow comes to nothing as the fall does.
    let (run, more) = if slope > MIN_SLOPE {
        (slope.sqrt(), 0.5 * per_metre / slope.sqrt())
    } else {
        (slope / MIN_SLOPE.sqrt(), per_metre / MIN_SLOPE.sqrt())
    };
    (carry * depth * run, carry * (5.0 / 3.0 * root * root * run + depth * more))
}

/// The depth a vertex is left with after a step of handing water down: it
/// held `volume` over `area`, and `out` is what leaves it over the step at a
/// depth and how much more for each metre deeper. The balance rises with the
/// depth, so it has one answer, between nothing and all of the water
/// staying: Newton's steps from `guess`, the answer it had last step, held
/// between the two depths the answer is known to lie between. With the depth
/// comes what leaves at it, where that was the last thing worked out.
fn settled_depth(volume: f64, area: f64, guess: f64, out: impl Fn(f64) -> (f64, f64)) -> (f64, Option<f64>) {
    let (mut low, mut high) = (0.0, volume / area);
    let mut depth = if guess > 0.0 && guess < high { guess } else { high };
    for _ in 0..48 {
        let (leaves, more) = out(depth);
        let over = area * depth + leaves - volume;
        if over.abs() <= 1e-7 * volume {
            return (depth, Some(leaves));
        }
        if over > 0.0 {
            high = depth;
        } else {
            low = depth;
        }
        let next = depth - over / (area + more);
        depth = if next > low && next < high { next } else { 0.5 * (low + high) };
        if high - low <= 1e-8 * high {
            break;
        }
    }
    (depth, None)
}

/// The depth a vertex of `area` ends a step at, holding `have` cubic metres
/// before anything leaves it, where its solved depth is `solved` and `outs`
/// are its ways out, each its sill over the vertex's ground and what it asks
/// over the step at the solved depth. Each runs for the share of the solved
/// head over its sill that the water stands at, so the balance
/// `area · depth + Σ asked · share(depth) = have` is piecewise linear and
/// rising in the depth: past the solved depth all of each runs, and below it
/// the pieces are taken from the lowest sill up.
fn kept_depth(area: f64, have: f64, solved: f64, outs: &mut [(f64, f64)]) -> f64 {
    let asked: f64 = outs.iter().map(|out| out.1).sum();
    let whole = (have - asked) / area;
    if whole >= solved {
        return whole;
    }
    outs.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    // Between one sill and the next the balance is `rise · depth - offset`.
    let (mut rise, mut offset) = (area, 0.0);
    let mut low = 0.0;
    for at in 0..=outs.len() {
        let high = outs.get(at).map_or(solved, |out| out.0.min(solved));
        let balance = rise * high - offset - have;
        if balance >= 0.0 {
            return ((have + offset) / rise).clamp(low, high);
        }
        if let Some(&(sill, asked)) = outs.get(at)
            && sill < solved
        {
            let per_metre = asked / (solved - sill);
            rise += per_metre;
            offset += per_metre * sill;
        }
        low = high;
    }
    solved
}

/// A side's flow at the levels `level` at its two ends, whose grounds are
/// `bed`: what it carries from its first end to its second in cubic metres a
/// second, how that answers to the fall along it, and what it would carry
/// for a metre of depth to the five-thirds, signed as the flow is and
/// nothing where the pond limit holds the side.
///
/// Manning's, at the depth of the upper end's water over the higher of the
/// two grounds. `pond_limit` is the most a side as flat as a pond conducts.
fn side_flow(level: [f64; 2], bed: [f64; 2], weight: f64, length: f64, roughness: f64, pond_limit: f64) -> (f64, f64, f64) {
    let (upper, lower, sign) = if level[0] >= level[1] { (level[0], level[1], 1.0) } else { (level[1], level[0], -1.0) };
    let drop = upper - lower;
    let fall = drop / length;
    let per_depth = weight / (roughness * fall.max(MIN_SLOPE).sqrt());
    let deep = upper - bed[0].max(bed[1]);
    let manning = if deep > FILM { per_depth * deep.cbrt().powi(2) * deep } else { 0.0 };
    // The limit is for ponds: it rises with the fall, so it never holds back
    // water that is running.
    let limit = pond_limit * (fall / POND_FALL).max(1.0);
    // How the flow answers to the fall is taken as in proportion, which past
    // the least fall Manning's formula is asked about is twice what it is:
    // each turn then goes half as far as it might, and none goes too far
    // where the water runs deeper than the ground falls from one vertex to
    // the next and a level a little out turns a flow about.
    (
        sign * manning.min(limit) * drop,
        manning.min(limit),
        if manning < limit { sign * per_depth * drop } else { 0.0 },
    )
}

/// Run the storm in `settings` over `ground`.
///
/// A step ends at the levels at which every side's flow, at those levels,
/// leaves every vertex with the water it had and what the flows brought it:
/// the implicit upwind scheme of the diffusive-wave models, which is stable
/// at any step. Those levels are found in two parts.
///
/// Water whose way down is clear - the water below stands lower than the
/// ground it is on - is handed downhill from the highest level to the lowest,
/// each vertex passing on what a sheet of the depth it ends the step with
/// carries, so a thin film can cross any number of triangles in a step and
/// still settle at the depth its flow needs. Where water only runs downhill
/// that is the answer.
///
/// Where it is backed up - standing over the ground at both ends of a side,
/// as in a pond, on a level bench, or in a channel running deeper than its
/// floor falls from one vertex to the next - the level below matters, and
/// the hand-down took it as it was. There the levels are solved together by
/// Newton's method, a turn at a time, each turn over only the vertices still
/// short of their balance and the ones round them.
///
/// Both parts run on every thread (see [`hand_down`]), and the answer is the
/// same on any number of them.
pub(crate) fn simulate(ground: &Ground, settings: &WaterSettings, cancel: &AtomicBool, progress: &Progress, keep: &mut (dyn FnMut(WaterFrame) + Send)) -> Result<(), WaterError> {
    // On a thread of the pool: a step is thousands of short shared loops, and
    // from outside the pool each one is handed over and waited for, which
    // costs more than most of them do.
    rayon::scope(|_| simulate_on_pool(ground, settings, cancel, progress, keep))
}

fn simulate_on_pool(ground: &Ground, settings: &WaterSettings, cancel: &AtomicBool, progress: &Progress, keep: &mut (dyn FnMut(WaterFrame) + Send)) -> Result<(), WaterError> {
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
    let soak_rate = settings.infiltration_mm_h / 1000.0 / 3600.0;
    // What each vertex holds for a metre of depth, over the length of the
    // part of a step being taken, and that length.
    let mut storage_now: Vec<f64> = operators.area.iter().map(|area| area / step).collect();
    let mut stored_for = step;
    // The length of the part of a step to try next, and the depths to go
    // back to if it does not hold.
    let mut part = step;
    let mut before = vec![0.0f64; count];
    // What a metre of depth to the five-thirds carries over the open edge.
    let outlet: Vec<f64> = (0..count)
        .map(|vertex| operators.outlet_width[vertex] * operators.outlet_slope[vertex].sqrt() / roughness)
        .collect();

    // Beside each vertex's sides, in the same order, so the hand-down reads
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
    let mut conductance = vec![0.0f32; operators.edges.len()];
    let mut deepening = vec![0.0f32; operators.edges.len()];
    let mut flowing = vec![0.0f32; operators.edges.len()];
    // Each side's flow at the step's solved levels, from its first end to its
    // second, in cubic metres a second.
    let mut final_flow = vec![0.0f64; operators.edges.len()];
    let mut unit = vec![0.0f32; operators.edges.len()];
    let mut target = vec![0.0f64; count];
    // The levels the step started from.
    let mut started = vec![0.0f64; count];
    // Each row's diagonal before its sides: its storage, and how much more
    // leaves it over the open edge for each metre deeper.
    let mut holding = vec![0.0f64; count];
    let mut pools = Pools::default();
    // The hand-down: how many givers each vertex still waits on, what it is
    // left with, the share of its sides' flows it passed on, and the depth
    // it last settled at.
    let waiting: Vec<AtomicU32> = (0..count).map(|_| AtomicU32::new(0)).collect();
    let kept = Cells::new(count);
    let passed = Cells::new(count);
    let roots = Cells::new(count);
    // The cube root of what each vertex is left with, which every vertex it
    // gives to needs.
    let kept_root = Cells::new(count);
    // What the levels as they are taken to be leave each vertex short of
    // its balance, in cubic metres a second, and which vertices that is too
    // much for.
    let mut short = vec![0.0f64; count];
    let mut unsettled = vec![false; count];
    // Those vertices and the ones round them, in order, and which they are.
    let mut active: Vec<u32> = Vec::new();
    let mut listed = vec![false; count];
    // The vertices whose levels the last turn moved, and those and the ones
    // round them: which they are, and a list of each.
    let mut moved = vec![false; count];
    let mut near = vec![false; count];
    let (mut moved_vertices, mut near_vertices): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());

    for index in 0..steps {
        if cancel.load(Ordering::Relaxed) {
            return Err(WaterError::Cancelled);
        }
        // A step is taken whole if it holds, and in parts if not. The solve
        // for the levels can fail to settle when a step is long for the
        // water it is moving, and it shows: the levels it is left at hold a
        // different amount of water from what there is. Such a part is taken
        // again as two of half the length, and the parts lengthen again once
        // they hold.
        let whole = step;
        let mut done = 0.0;
        while done < whole * (1.0 - 1e-12) {
            if cancel.load(Ordering::Relaxed) {
                return Err(WaterError::Cancelled);
            }
            let step = part.min(whole - done);
            if step != stored_for {
                storage_now
                    .par_iter_mut()
                    .zip(&operators.area)
                    .with_min_len(4096)
                    .for_each(|(storage, area)| *storage = area / step);
                stored_for = step;
            }
            let storage = &storage_now;
            let soak = soak_rate * step;
            let time = index as f64 * whole + done;
            before.copy_from_slice(&depth);

            // Rain on, then what the ground takes in.
            let falling = rain_rate * (rain_end - time).clamp(0.0, step);
            depth.par_iter_mut().enumerate().with_min_len(4096).for_each(|(vertex, depth)| {
                *depth = (*depth + falling * operators.catchment[vertex] / operators.area[vertex] - soak).max(0.0);
            });

            // A step is taken implicitly: the levels at its end are those at
            // which every side's flow, at those levels, leaves every vertex with
            // the water it had and what the flows brought it. It is found in two
            // parts. The first hands the water down the surface once, which is
            // all there is to it wherever water only runs downhill. The second
            // puts right what that leaves: see below.
            //
            // Water with somewhere to run to is handed down from the highest level
            // to the lowest: everything a vertex is given has arrived by the time
            // its own turn comes. See [`hands`] for which sides hand down.
            level.par_iter_mut().zip(bed.par_iter().zip(&depth)).for_each(|(level, (bed, depth))| *level = bed + depth);
            started.copy_from_slice(&level);
            // A side hands down from `from` to `to` when the way is clear: the
            // water at `to` stands lower than the ground at `from`, so nothing
            // below can hold back what comes down.
            let gives = |from: usize, to: usize| level[to] < bed[from];
            waiting.par_iter().enumerate().with_min_len(4096).for_each(|(vertex, waiting)| {
                let givers = operators.sides_of(vertex).iter().filter(|(other, _)| gives(*other as usize, vertex)).count();
                waiting.store(givers as u32, Ordering::Relaxed);
            });
            hand_down(&waiting, |vertex, next| {
                let vertex = vertex as usize;
                let sides = operators.starts[vertex] as usize..operators.starts[vertex + 1] as usize;
                let area = operators.area[vertex];
                // What its givers passed on: each one's flow at the depth it
                // settled at, down to this vertex's level as the step began.
                let mut given = 0.0;
                for (&(other, _), &(carry, per_metre)) in operators.adjacent[sides.clone()].iter().zip(&reach[sides.clone()]) {
                    let other = other as usize;
                    if gives(other, vertex) {
                        let passed = passed.get(other);
                        if passed > 0.0 {
                            let sill = (bed[vertex] - bed[other]).max(0.0);
                            let deep = kept.get(other);
                            let (over, fall) = (deep - sill, deep + bed[other] - level[vertex]);
                            if over > 0.0 && fall > 0.0 {
                                let root = if sill == 0.0 { kept_root.get(other) } else { over.cbrt() };
                                given += passed * step * flow_over(over, root, fall, carry, per_metre).0;
                            }
                        }
                    }
                }
                let held = area * depth[vertex] + given;
                if held <= area * FILM {
                    kept.set(vertex, held / area);
                    passed.set(vertex, 0.0);
                } else {
                    // The sides it gives by, gathered once: each one's sill, the
                    // fall to it from this vertex's ground, and what it carries.
                    let mut outs = [(0.0f64, 0.0f64, 0.0f64, 0.0f64); OUT_SIDES];
                    let mut gathered = 0;
                    // More than the room made for them go on the heap.
                    let mut spilled = Vec::new();
                    for (&(other, _), &(carry, per_metre)) in operators.adjacent[sides.clone()].iter().zip(&reach[sides.clone()]) {
                        let other = other as usize;
                        if gives(vertex, other) {
                            let side = ((bed[other] - bed[vertex]).max(0.0), bed[vertex] - level[other], carry, per_metre);
                            if gathered < OUT_SIDES {
                                outs[gathered] = side;
                                gathered += 1;
                            } else {
                                if spilled.is_empty() {
                                    spilled.extend_from_slice(&outs);
                                }
                                spilled.push(side);
                            }
                        }
                    }
                    let outs = if spilled.is_empty() { &outs[..gathered] } else { &spilled[..] };
                    // What leaves over the step at a depth: over the open edge,
                    // and down each of those.
                    let leaves = |deep: f64, root: f64| {
                        let (mut flow, mut more) = (outlet[vertex] * deep * root * root, 5.0 / 3.0 * outlet[vertex] * root * root);
                        for &(sill, drop, carry, per_metre) in outs {
                            let (over, fall) = (deep - sill, deep + drop);
                            if over > 0.0 && fall > 0.0 {
                                let side = flow_over(over, if sill == 0.0 { root } else { over.cbrt() }, fall, carry, per_metre);
                                flow += side.0;
                                more += side.1;
                            }
                        }
                        (flow * step, more * step)
                    };
                    let (left_here, left) = settled_depth(held, area, roots.get(vertex), |deep| leaves(deep, deep.cbrt()));
                    let root = left_here.cbrt();
                    roots.set(vertex, left_here);
                    kept.set(vertex, left_here);
                    kept_root.set(vertex, root);
                    // What it passes on is what it no longer holds, whatever the
                    // last place of the depth it settled at: the share of each
                    // side's flow that makes them add up to that.
                    let leaving = left.unwrap_or_else(|| leaves(left_here, root).0);
                    passed.set(vertex, if leaving > 0.0 { (held - area * left_here).max(0.0) / leaving } else { 0.0 });
                }
                for &(other, _) in &operators.adjacent[sides] {
                    if gives(vertex, other as usize) {
                        release(&waiting, other, next);
                    }
                }
            });
            depth.par_iter_mut().enumerate().with_min_len(4096).for_each(|(vertex, depth)| *depth = kept.get(vertex));

            // The hand-down takes each vertex's level below it to be as it was
            // when the step began. Where the water backs up that is not so: a
            // pond rises under what runs into it, and a channel in flood stands
            // on the water ahead of it. So the levels are now solved together,
            // from the hand-down's: Newton's method, the flows made linear about
            // the levels as they are taken to be, solved, and made linear again
            // about the answer, until the levels stop moving.
            // A vertex on a side that is backed up starts from its level as the
            // step began: what the hand-down brought it stands on it as a pile no
            // level was ever at, and the solve has what comes down to it from
            // the levels above.
            level.par_iter_mut().enumerate().with_min_len(4096).for_each(|(vertex, level)| {
                let backed_up = operators.sides_of(vertex).iter().any(|&(other, _)| {
                    let other = other as usize;
                    let (upper, lower) = if started[vertex] >= started[other] { (vertex, other) } else { (other, vertex) };
                    started[lower] >= bed[upper] && started[upper] - bed[upper] > FILM
                });
                *level = if backed_up { started[vertex] } else { bed[vertex] + depth[vertex] };
            });
            let held_before = total(count, |vertex| operators.area[vertex] * (started[vertex] - bed[vertex]));
            for turn in 0..NEWTON_TURNS {
                // After the first turn only the vertices whose levels the last
                // one moved, and the ones round them, have anything to work out
                // again: those, and the sides at them, were listed as the
                // levels were moved.
                let whole = turn == 0;
                // Each side at the levels as they are taken to be: what it
                // carries, how that answers to the fall along it, and what it
                // would carry for a metre of depth to the five-thirds (`unit`,
                // signed as `flowing` is, and nothing where the pond limit holds
                // the side).
                let side = |edge: usize| {
                    let [a, b] = operators.edges[edge].map(|vertex| vertex as usize);
                    let side = side_flow(
                        [level[a], level[b]],
                        [bed[a], bed[b]],
                        operators.weight[edge],
                        operators.length[edge],
                        roughness,
                        POOL_EXCHANGE * storage[a].min(storage[b]),
                    );
                    (side.0 as f32, side.1 as f32, side.2 as f32)
                };
                if whole {
                    (flowing.par_iter_mut(), conductance.par_iter_mut(), unit.par_iter_mut())
                        .into_par_iter()
                        .enumerate()
                        .for_each(|(edge, (flowing, conductance, unit))| (*flowing, *conductance, *unit) = side(edge));
                } else {
                    for (edge, side) in each_side(&operators, &moved_vertices, &moved, side) {
                        let edge = edge as usize;
                        (flowing[edge], conductance[edge], unit[edge]) = side;
                    }
                }
                // How much more a flow of `carries` a metre to the five-thirds
                // runs for each metre deeper than `deep`: the slope of that curve
                // there, or of the chord to `aim` where that is deeper. A vertex
                // given far more than it is passing on has a long way up the
                // curve to go, and by its slope where it stands it would be
                // taken to rise many times too far.
                let steepening = |carries: f64, deep: f64, aim: f64| {
                    if aim > deep * 1.01 + 1e-9 {
                        carries * (aim.powf(5.0 / 3.0) - deep.powf(5.0 / 3.0)) / (aim - deep)
                    } else {
                        5.0 / 3.0 * carries * deep.cbrt().powi(2)
                    }
                };
                // Each vertex at those levels and flows.
                let stand = |vertex: usize| {
                    // The depth at which it would pass on all that is coming to
                    // it: what its sides bring, and what it has let go of since
                    // the step started.
                    let (mut coming, mut carries) = ((storage[vertex] * (started[vertex] - level[vertex])).max(0.0), outlet[vertex]);
                    // What its sides bring it, less what they take.
                    let mut toward = -0.0;
                    for &(_, edge) in operators.sides_of(vertex) {
                        let first = operators.edges[edge as usize][0] as usize == vertex;
                        let flow = if first { -flowing[edge as usize] } else { flowing[edge as usize] };
                        coming += f64::from(flow.max(0.0));
                        toward += f64::from(flow);
                        let unit = unit[edge as usize];
                        if unit != 0.0 && (unit > 0.0) == first {
                            carries += f64::from(unit.abs());
                        }
                    }
                    // No deeper than all of it staying here would make it.
                    let most = (level[vertex] - bed[vertex]).max(0.0) + coming / storage[vertex];
                    let target = if carries > 0.0 && coming > 0.0 { (coming / carries).powf(0.6).min(most) } else { 0.0 };
                    // What leaves over the open edge, and how much more for
                    // each metre deeper; with its storage, the row's diagonal
                    // before its sides.
                    let deep = (level[vertex] - bed[vertex]).max(0.0);
                    let root = deep.cbrt();
                    let drain = outlet[vertex] * deep * root * root;
                    let draining = if outlet[vertex] > 0.0 { steepening(outlet[vertex], deep, target) } else { 0.0 };
                    // What it is short of its balance over the step, and
                    // whether that is more than it can be left at: a depth's
                    // worth of it over the step.
                    let short = toward - drain + storage[vertex] * (started[vertex] - level[vertex]);
                    let unsettled = short.abs() > storage[vertex] * (SETTLED_DEPTH + SETTLED_SHARE * (level[vertex] - bed[vertex]));
                    (target, storage[vertex] + draining, short, unsettled)
                };
                if whole {
                    (target.par_iter_mut(), holding.par_iter_mut(), short.par_iter_mut(), unsettled.par_iter_mut())
                        .into_par_iter()
                        .enumerate()
                        .with_min_len(4096)
                        .for_each(|(vertex, (target, holding, short, unsettled))| (*target, *holding, *short, *unsettled) = stand(vertex));
                } else {
                    for (vertex, stand) in near_vertices.iter().zip(each(&near_vertices, stand)) {
                        let vertex = *vertex as usize;
                        (target[vertex], holding[vertex], short[vertex], unsettled[vertex]) = stand;
                    }
                }
                // Each side's deepening: what more it carries for each metre
                // its upper end's level rises, by the depth it then runs at.
                let deepen = |edge: usize| {
                    let unit = unit[edge];
                    if unit == 0.0 {
                        0.0
                    } else {
                        let [a, b] = operators.edges[edge].map(|vertex| vertex as usize);
                        let high = if unit > 0.0 { a } else { b };
                        // The sill the side's water has to stand over.
                        let sill = bed[a].max(bed[b]) - bed[high];
                        let deep = (level[high] - bed[high] - sill).max(0.0);
                        let slope = steepening(f64::from(unit.abs()), deep, target[high] - sill) as f32;
                        if unit > 0.0 { slope } else { -slope }
                    }
                };
                if whole {
                    deepening.par_iter_mut().enumerate().for_each(|(edge, deepening)| *deepening = deepen(edge));
                } else {
                    for (edge, deepen) in each_side(&operators, &near_vertices, &near, deepen) {
                        deepening[edge as usize] = deepen;
                    }
                }
                // The system is of the vertices short of their balance and the
                // ones round them; the rest have their levels, and stay there.
                for vertex in active.drain(..) {
                    listed[vertex as usize] = false;
                }
                let mut list = |vertex: u32| {
                    if !std::mem::replace(&mut listed[vertex as usize], true) {
                        active.push(vertex);
                    }
                };
                for (run, unsettled) in unsettled.chunks(SETTLED_RUN).enumerate().filter(|(_, run)| run.contains(&true)) {
                    for vertex in (run * SETTLED_RUN..).zip(unsettled).filter(|(_, unsettled)| **unsettled).map(|(vertex, _)| vertex) {
                        list(vertex as u32);
                        for &(other, _) in operators.sides_of(vertex) {
                            list(other);
                        }
                    }
                }
                if active.len() < SHARED_ROWS {
                    active.sort_unstable();
                } else {
                    active.par_sort_unstable();
                }
                pools.gather(&operators, &conductance, &deepening, &active, &holding);
                if pools.vertices.is_empty() {
                    break;
                }
                let rows = pools.vertices.len();
                let rhs: Vec<f64> = pools.vertices.par_iter().with_min_len(4096).map(|vertex| short[*vertex as usize]).collect();
                let mut change = pools.solve(&rhs, vec![0.0; rows]);
                // Water runs down the levels, so no vertex ends a step higher
                // than both where it started and every vertex round it, nor under
                // its own ground. Newton's method does not know that, and from a
                // poor start will go anywhere: each turn is held to it, taking
                // the levels round a vertex as this turn asks for them, so a
                // pond rises as one rather than a ring of vertices a turn.
                let asked: Vec<f64> = change
                    .par_iter()
                    .zip(&pools.vertices)
                    .with_min_len(4096)
                    .map(|(change, vertex)| level[*vertex as usize] + change)
                    .collect();
                let row_of = &pools.row_of;
                let asked_at = |vertex: usize| match row_of[vertex] {
                    u32::MAX => level[vertex],
                    row => asked[row as usize].max(level[vertex]),
                };
                change.par_iter_mut().enumerate().with_min_len(4096).for_each(|(row, change)| {
                    let vertex = pools.vertices[row] as usize;
                    let highest = operators
                        .sides_of(vertex)
                        .iter()
                        .fold(started[vertex], |highest, (other, _)| highest.max(asked_at(*other as usize)));
                    let asked = asked[row];
                    // The comparison is false for a change that is no number.
                    *change = if asked <= highest { asked.max(bed[vertex]) } else { highest.max(bed[vertex]) };
                });
                // The levels move, and what the next turn has to work out
                // again is listed: the vertices moved, and those and the ones
                // round them.
                for vertex in moved_vertices.drain(..) {
                    moved[vertex as usize] = false;
                }
                for vertex in near_vertices.drain(..) {
                    near[vertex as usize] = false;
                }
                moved_vertices.extend_from_slice(&pools.vertices);
                for (solved, vertex) in change.iter().zip(&pools.vertices) {
                    level[*vertex as usize] = *solved;
                    moved[*vertex as usize] = true;
                    for other in std::iter::once(*vertex).chain(operators.sides_of(*vertex as usize).iter().map(|side| side.0)) {
                        if !std::mem::replace(&mut near[other as usize], true) {
                            near_vertices.push(other);
                        }
                    }
                }
            }
            // The step ends with each vertex holding what it began with, and
            // what its sides brought it, less what they and the open edge took
            // from it, every flow at the solved levels. What a side takes from
            // one end it gives the other, so the water is kept vertex by
            // vertex, and what the levels were left short of stays where it
            // is rather than being taken from or given to water elsewhere.
            // The flows run from the higher level to the lower, so the
            // vertices are taken from the highest down.
            //
            // Where a vertex has less water than its solved level says, each
            // side runs for the share of its solved head over its sill that
            // the water really stands at: a pond left short of its level does
            // not send water over a sill it is below, and no vertex goes dry
            // by more than it had.
            final_flow.par_iter_mut().enumerate().with_min_len(4096).for_each(|(edge, flow)| {
                let [a, b] = operators.edges[edge].map(|vertex| vertex as usize);
                *flow = side_flow(
                    [level[a], level[b]],
                    [bed[a], bed[b]],
                    operators.weight[edge],
                    operators.length[edge],
                    roughness,
                    POOL_EXCHANGE * storage[a].min(storage[b]),
                )
                .0;
            });
            let final_flow = &final_flow;
            // What `edge` brings `vertex`, in cubic metres a second: negative
            // where it takes.
            let inflow = |vertex: usize, edge: u32| {
                let flow = final_flow[edge as usize];
                if operators.edges[edge as usize][0] as usize == vertex { -flow } else { flow }
            };
            waiting.par_iter().enumerate().with_min_len(4096).for_each(|(vertex, waiting)| {
                let givers = operators.sides_of(vertex).iter().filter(|&&(_, edge)| inflow(vertex, edge) > 0.0).count();
                waiting.store(givers as u32, Ordering::Relaxed);
            });
            // The share of a side's flow its upper end `vertex` runs it for,
            // standing `deep` over its ground, where the side's sill stands
            // `sill` over it: in proportion to the head over the sill, up to
            // all of it at the solved level.
            let running = |vertex: usize, deep: f64, sill: f64| {
                let solved = level[vertex] - bed[vertex];
                if solved <= sill { 0.0 } else { ((deep - sill) / (solved - sill)).clamp(0.0, 1.0) }
            };
            hand_down(&waiting, |vertex, next| {
                let vertex = vertex as usize;
                let sides = operators.sides_of(vertex);
                let area = operators.area[vertex];
                let solved = (level[vertex] - bed[vertex]).max(0.0);
                let mut have = area * (started[vertex] - bed[vertex]).max(0.0);
                // What each way out asks over the step at the solved level,
                // and the sill it runs over.
                let mut outs = [(0.0f64, 0.0f64); OUT_SIDES + 1];
                let mut gathered = 0;
                let mut spilled = Vec::new();
                let mut out = |sill: f64, asked: f64| {
                    if gathered < outs.len() {
                        outs[gathered] = (sill, asked);
                        gathered += 1;
                    } else {
                        if spilled.is_empty() {
                            spilled.extend_from_slice(&outs);
                        }
                        spilled.push((sill, asked));
                    }
                };
                for &(other, edge) in sides {
                    let other = other as usize;
                    let flow = inflow(vertex, edge);
                    if flow > 0.0 {
                        have += step * flow * running(other, kept.get(other), (bed[vertex] - bed[other]).max(0.0));
                    } else if flow < 0.0 {
                        out((bed[other] - bed[vertex]).max(0.0), -step * flow);
                    }
                }
                let root = solved.cbrt();
                let drain = step * outlet[vertex] * solved * root * root;
                if drain > 0.0 {
                    out(0.0, drain);
                }
                let outs = if spilled.is_empty() { &mut outs[..gathered] } else { &mut spilled[..] };
                kept.set(vertex, kept_depth(area, have, solved, outs));
                for &(other, edge) in sides {
                    if inflow(vertex, edge) < 0.0 {
                        release(&waiting, other, next);
                    }
                }
            });
            // Where the levels hold far from the water there is, the step was
            // too long for the solve to settle: it is taken again in two parts.
            let unsettled_water = total(count, |vertex| operators.area[vertex] * (kept.get(vertex) - (level[vertex] - bed[vertex]).max(0.0)));
            if step > LEAST_STEP && unsettled_water.abs().partial_cmp(&(BALANCE_LIMIT * held_before)).is_none_or(std::cmp::Ordering::is_gt) {
                depth.copy_from_slice(&before);
                part = step / 2.0;
                continue;
            }
            depth.par_iter_mut().enumerate().with_min_len(4096).for_each(|(vertex, depth)| *depth = kept.get(vertex));
            done += step;
            part = (step * 2.0).min(whole);
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
