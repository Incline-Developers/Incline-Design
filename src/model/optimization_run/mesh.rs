//! Shells as triangulations: a blocky solid of the cells a shell holds, or a
//! blocky surface of its floor and walls (the solid without its cap).
//!
//! Both follow block faces exactly. Smoothing (Whittle joins the centres of the
//! column bottoms) is left for later.

use std::collections::{HashMap, HashSet};

use super::{grid::Grid, shells::OUTSIDE};
use crate::model::formats::mesh_data::Vertex;

/// Vertices, triangles and the edges worth drawing (block outlines, no diagonals).
#[derive(Clone, Debug, Default)]
pub(crate) struct ShellMesh {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) faces: Vec<[u32; 3]>,
    pub(crate) edges: Vec<[u32; 2]>,
}

#[derive(Default)]
struct Builder {
    mesh: ShellMesh,
    index: HashMap<(u32, u32, u64), u32>,
    edges: HashSet<[u32; 2]>,
}

impl Builder {
    /// A vertex at grid corner (x, y) and world height `z`, shared with every
    /// face that uses the same point.
    fn vertex(&mut self, grid: &Grid, x: usize, y: usize, z: f64) -> u32 {
        let next = self.mesh.vertices.len() as u32;
        *self.index.entry((x as u32, y as u32, z.to_bits())).or_insert_with(|| {
            let corner = grid.corner(x, y, 0);
            self.mesh.vertices.push(Vertex { x: corner.x, y: corner.y, z });
            next
        })
    }

    /// A quad, corners in counter-clockwise order seen from its outside.
    fn quad(&mut self, corners: [u32; 4]) {
        let [a, b, c, d] = corners;
        self.mesh.faces.push([a, b, c]);
        self.mesh.faces.push([a, c, d]);
        for (from, to) in [(a, b), (b, c), (c, d), (d, a)] {
            self.edges.insert([from.min(to), from.max(to)]);
        }
    }

    /// A quad through grid corners (x, y) at world heights z.
    fn quad_at(&mut self, grid: &Grid, corners: [(usize, usize, f64); 4]) {
        let ids = corners.map(|(x, y, z)| self.vertex(grid, x, y, z));
        self.quad(ids);
    }

    fn finish(mut self) -> ShellMesh {
        let mut edges: Vec<[u32; 2]> = self.edges.into_iter().collect();
        edges.sort_unstable();
        self.mesh.edges = edges;
        self.mesh
    }
}

fn in_shell(label: &[u16], air: &[bool], shell: u16, cell: usize) -> bool {
    label[cell] != OUTSIDE && label[cell] <= shell && !air[cell]
}

/// What lies across a face of a shell cell.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Beyond {
    /// Rock the shell leaves in place.
    Rock,
    Air,
    /// Off the grid at its sides or bottom.
    Edge,
    /// Off the top of the grid.
    Top,
}

/// One quad for every face between a cell in shell `shell` and one outside
/// it (or off the grid) for which `keep` says yes.
fn faces(grid: &Grid, label: &[u16], air: &[bool], shell: u16, keep: impl Fn(Beyond) -> bool) -> ShellMesh {
    let [nx, ny, nz] = grid.dims;
    let mut builder = Builder::default();
    let height = |z: usize| grid.corner(0, 0, z).z;
    let inside = |x: usize, y: usize, z: usize| in_shell(label, air, shell, grid.index(x, y, z));
    // `None` when the neighbour is in the shell, so no face is there.
    let beyond = |neighbour: Option<[usize; 3]>, off_grid: Beyond| match neighbour {
        None => Some(off_grid),
        Some([x, y, z]) if inside(x, y, z) => None,
        Some([x, y, z]) if air[grid.index(x, y, z)] => Some(Beyond::Air),
        Some(_) => Some(Beyond::Rock),
    };
    let wanted = |neighbour: Option<[usize; 3]>, off_grid: Beyond| beyond(neighbour, off_grid).is_some_and(&keep);
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                if !inside(x, y, z) {
                    continue;
                }
                let (z0, z1) = (height(z), height(z + 1));
                if wanted((x > 0).then(|| [x - 1, y, z]), Beyond::Edge) {
                    let corners = [(x, y, z0), (x, y, z1), (x, y + 1, z1), (x, y + 1, z0)];
                    builder.quad_at(grid, corners);
                }
                if wanted((x + 1 < nx).then(|| [x + 1, y, z]), Beyond::Edge) {
                    let corners = [(x + 1, y, z0), (x + 1, y + 1, z0), (x + 1, y + 1, z1), (x + 1, y, z1)];
                    builder.quad_at(grid, corners);
                }
                if wanted((y > 0).then(|| [x, y - 1, z]), Beyond::Edge) {
                    let corners = [(x, y, z0), (x + 1, y, z0), (x + 1, y, z1), (x, y, z1)];
                    builder.quad_at(grid, corners);
                }
                if wanted((y + 1 < ny).then(|| [x, y + 1, z]), Beyond::Edge) {
                    let corners = [(x, y + 1, z0), (x, y + 1, z1), (x + 1, y + 1, z1), (x + 1, y + 1, z0)];
                    builder.quad_at(grid, corners);
                }
                if wanted((z > 0).then(|| [x, y, z - 1]), Beyond::Edge) {
                    let corners = [(x, y, z0), (x, y + 1, z0), (x + 1, y + 1, z0), (x + 1, y, z0)];
                    builder.quad_at(grid, corners);
                }
                if wanted((z + 1 < nz).then(|| [x, y, z + 1]), Beyond::Top) {
                    let corners = [(x, y, z1), (x + 1, y, z1), (x + 1, y + 1, z1), (x, y + 1, z1)];
                    builder.quad_at(grid, corners);
                }
            }
        }
    }
    builder.finish()
}

/// The closed solid of shell `shell`'s rock cells: one quad for every face
/// between a cell in the shell and one outside it (or off the grid).
pub(crate) fn solid(grid: &Grid, label: &[u16], air: &[bool], shell: u16) -> ShellMesh {
    faces(grid, label, air, shell, |_| true)
}

/// The pit outline of shell `shell`: the solid without its cap - only the
/// faces against rock the shell leaves (and the grid's sides and bottom), so
/// the floor and walls, open where the pit meets air or the top of the grid.
pub(crate) fn surface(grid: &Grid, label: &[u16], air: &[bool], shell: u16) -> ShellMesh {
    faces(grid, label, air, shell, |beyond| matches!(beyond, Beyond::Rock | Beyond::Edge))
}
