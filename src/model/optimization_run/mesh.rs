//! Shells as triangulations: a blocky solid of the cells a shell holds, or a
//! blocky surface of its floor and the ground round it.
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

/// The closed solid of shell `shell`'s rock cells: one quad for every face
/// between a cell in the shell and one outside it (or off the grid).
pub(crate) fn solid(grid: &Grid, label: &[u16], air: &[bool], shell: u16) -> ShellMesh {
    let [nx, ny, nz] = grid.dims;
    let mut builder = Builder::default();
    let height = |z: usize| grid.corner(0, 0, z).z;
    let inside = |x: usize, y: usize, z: usize| in_shell(label, air, shell, grid.index(x, y, z));
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                if !inside(x, y, z) {
                    continue;
                }
                let (z0, z1) = (height(z), height(z + 1));
                if x == 0 || !inside(x - 1, y, z) {
                    let corners = [(x, y, z0), (x, y, z1), (x, y + 1, z1), (x, y + 1, z0)];
                    builder.quad_at(grid, corners);
                }
                if x + 1 == nx || !inside(x + 1, y, z) {
                    let corners = [(x + 1, y, z0), (x + 1, y + 1, z0), (x + 1, y + 1, z1), (x + 1, y, z1)];
                    builder.quad_at(grid, corners);
                }
                if y == 0 || !inside(x, y - 1, z) {
                    let corners = [(x, y, z0), (x + 1, y, z0), (x + 1, y, z1), (x, y, z1)];
                    builder.quad_at(grid, corners);
                }
                if y + 1 == ny || !inside(x, y + 1, z) {
                    let corners = [(x, y + 1, z0), (x, y + 1, z1), (x + 1, y + 1, z1), (x + 1, y + 1, z0)];
                    builder.quad_at(grid, corners);
                }
                if z == 0 || !inside(x, y, z - 1) {
                    let corners = [(x, y, z0), (x, y + 1, z0), (x + 1, y + 1, z0), (x + 1, y, z0)];
                    builder.quad_at(grid, corners);
                }
                if z + 1 == nz || !inside(x, y, z + 1) {
                    let corners = [(x, y, z1), (x + 1, y, z1), (x + 1, y + 1, z1), (x, y + 1, z1)];
                    builder.quad_at(grid, corners);
                }
            }
        }
    }
    builder.finish()
}

/// The ground once shell `shell` is mined: each column's pit floor where the
/// shell reaches it, otherwise the top of its highest rock cell; columns of
/// air alone are left open. Risers join neighbouring columns of different height.
pub(crate) fn surface(grid: &Grid, label: &[u16], air: &[bool], shell: u16) -> ShellMesh {
    let [nx, ny, nz] = grid.dims;
    let heights: Vec<Option<f64>> = (0..nx * ny)
        .map(|column| {
            let (x, y) = (column % nx, column / nx);
            let floor = (0..nz).find(|&z| in_shell(label, air, shell, grid.index(x, y, z)));
            match floor {
                Some(z) => Some(grid.corner(0, 0, z).z),
                None => (0..nz).rev().find(|&z| !air[grid.index(x, y, z)]).map(|z| grid.corner(0, 0, z + 1).z),
            }
        })
        .collect();
    let height = |x: usize, y: usize| heights[x + nx * y];
    let mut builder = Builder::default();
    for y in 0..ny {
        for x in 0..nx {
            let Some(z) = height(x, y) else {
                continue;
            };
            let corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
            builder.quad_at(grid, corners.map(|(x, y)| (x, y, z)));
            // Risers to the east and north neighbours, facing the lower side.
            if x + 1 < nx
                && let Some(east) = height(x + 1, y)
                && east != z
            {
                let (low, high) = (z.min(east), z.max(east));
                let corners = if east < z {
                    [(x + 1, y, low), (x + 1, y + 1, low), (x + 1, y + 1, high), (x + 1, y, high)]
                } else {
                    [(x + 1, y, low), (x + 1, y, high), (x + 1, y + 1, high), (x + 1, y + 1, low)]
                };
                builder.quad_at(grid, corners);
            }
            if y + 1 < ny
                && let Some(north) = height(x, y + 1)
                && north != z
            {
                let (low, high) = (z.min(north), z.max(north));
                let corners = if north < z {
                    [(x, y + 1, low), (x, y + 1, high), (x + 1, y + 1, high), (x + 1, y + 1, low)]
                } else {
                    [(x, y + 1, low), (x + 1, y + 1, low), (x + 1, y + 1, high), (x, y + 1, high)]
                };
                builder.quad_at(grid, corners);
            }
        }
    }
    builder.finish()
}
