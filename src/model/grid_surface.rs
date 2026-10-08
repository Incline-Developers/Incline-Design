//! A grid surface read back from its own triangles: the regular lattice its
//! nodes sit on and their heights, so a surface from a saved project can be
//! measured against as well as one built this session.
//!
//! The slope at a point is a least-squares plane over the nodes the surface
//! has in the 5 x 5 window around the nearest node; at an edge or a cut the
//! window holds fewer nodes, and none is ever counted twice.

use std::sync::Arc;

use glam::DVec2;

use crate::model::{
    formats::mesh_data,
    kernel::{self, PolyContainment},
    rbf::{Dip, NODE_BUDGET},
    spatial::TriangleBvh,
};

/// Nodes either side of the centre of the slope window.
const WINDOW_HALF: i64 = 2;

/// How far a coordinate may sit from a whole number of spacings and still
/// be a node, as a fraction of the spacing.
const NODE_TOLERANCE: f64 = 1e-6;

/// Heights two vertices at one node may differ by, in metres.
const HEIGHT_TOLERANCE: f64 = 1e-6;

/// Why a surface cannot be read as one regular grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotAGrid {
    /// Too few of its triangles are halves of square lattice cells.
    NoCells,
    /// Two vertices at one node at different heights.
    TwoHeights,
    /// The lattice around its nodes would pass the node budget.
    TooLarge,
}

/// A regular grid surface: its lattice, node heights and triangles.
#[derive(Debug)]
pub(crate) struct GridSurface {
    /// Position of the lattice's first node, lowest x and y.
    origin: DVec2,
    spacing: f64,
    columns: usize,
    rows: usize,
    /// Row by row from the lowest y; NaN where the surface has no node.
    heights: Vec<f64>,
    mesh: Arc<mesh_data::Triangulation>,
    spatial: Arc<TriangleBvh>,
}

impl GridSurface {
    /// Read the lattice off a surface's triangles. The spacing is the leg
    /// most of its axis-aligned right triangles share, the halves of
    /// lattice cells; every vertex on that lattice is a node.
    pub(crate) fn read(mesh: Arc<mesh_data::Triangulation>, spatial: Arc<TriangleBvh>) -> Result<Self, NotAGrid> {
        let vertices = mesh.vertices();
        let plan = |index: usize| DVec2::new(vertices[index].x, vertices[index].y);
        let mut legs: Vec<(f64, DVec2)> = Vec::new();
        for face in mesh.face_vertex_indices_iter() {
            if let Some(leg) = cell_leg([plan(face[0]), plan(face[1]), plan(face[2])]) {
                legs.push(leg);
            }
        }
        legs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut best: Option<(usize, f64, DVec2)> = None;
        let mut start = 0;
        while start < legs.len() {
            let length = legs[start].0;
            let end = start + legs[start..].iter().take_while(|(other, _)| (other - length).abs() <= length * NODE_TOLERANCE).count();
            if best.is_none_or(|(count, _, _)| end - start > count) {
                best = Some((end - start, length, legs[start].1));
            }
            start = end;
        }
        let Some((cells, spacing, corner)) = best else {
            return Err(NotAGrid::NoCells);
        };
        if cells < 2 || cells * 4 < mesh.face_count() {
            return Err(NotAGrid::NoCells);
        }
        let offset = DVec2::new(corner.x - spacing * (corner.x / spacing).floor(), corner.y - spacing * (corner.y / spacing).floor());
        let index_of = |at: DVec2| -> Option<[i64; 2]> {
            let local = (at - offset) / spacing;
            let rounded = local.round();
            ((local - rounded).abs().max_element() <= NODE_TOLERANCE).then_some([rounded.x as i64, rounded.y as i64])
        };
        let found: Vec<([i64; 2], f64)> = vertices
            .iter()
            .filter_map(|vertex| index_of(DVec2::new(vertex.x, vertex.y)).map(|index| (index, vertex.z)))
            .collect();
        let low = found.iter().fold([i64::MAX; 2], |low, (index, _)| [low[0].min(index[0]), low[1].min(index[1])]);
        let high = found.iter().fold([i64::MIN; 2], |high, (index, _)| [high[0].max(index[0]), high[1].max(index[1])]);
        let columns = usize::try_from(high[0] - low[0] + 1).map_err(|_| NotAGrid::NoCells)?;
        let rows = usize::try_from(high[1] - low[1] + 1).map_err(|_| NotAGrid::NoCells)?;
        let count = columns.checked_mul(rows).filter(|&count| count <= NODE_BUDGET).ok_or(NotAGrid::TooLarge)?;
        let mut heights = Vec::new();
        heights.try_reserve_exact(count).map_err(|_| NotAGrid::TooLarge)?;
        heights.resize(count, f64::NAN);
        for (index, z) in found {
            let slot = (index[1] - low[1]) as usize * columns + (index[0] - low[0]) as usize;
            let height = &mut heights[slot];
            if height.is_nan() {
                *height = z;
            } else if (*height - z).abs() > HEIGHT_TOLERANCE {
                return Err(NotAGrid::TwoHeights);
            }
        }
        Ok(Self {
            origin: offset + DVec2::new(low[0] as f64, low[1] as f64) * spacing,
            spacing,
            columns,
            rows,
            heights,
            mesh,
            spatial,
        })
    }

    pub(crate) fn spacing(&self) -> f64 {
        self.spacing
    }

    pub(crate) fn mesh(&self) -> &mesh_data::Triangulation {
        &self.mesh
    }

    /// The node's height, where the surface has that node.
    pub(crate) fn height(&self, column: usize, row: usize) -> Option<f64> {
        (column < self.columns && row < self.rows)
            .then(|| self.heights[row * self.columns + column])
            .filter(|height| !height.is_nan())
    }

    /// The column and row of the node at `at`, when `at` is one.
    pub(crate) fn node_at(&self, at: DVec2) -> Option<(usize, usize)> {
        let local = (at - self.origin) / self.spacing;
        let rounded = local.round();
        if (local - rounded).abs().max_element() > NODE_TOLERANCE || rounded.min_element() < 0.0 {
            return None;
        }
        let (column, row) = (rounded.x as usize, rounded.y as usize);
        self.height(column, row).map(|_| (column, row))
    }

    /// Whether `at` falls on the surface in plan, its edge included.
    pub(crate) fn covers(&self, at: DVec2) -> bool {
        let mut covered = false;
        self.spatial.for_each_xy_bounds_candidate_index(at, at, |face| {
            if covered {
                return;
            }
            if let Some(corners) = self.mesh.face_vertex_indices(face) {
                let ring = corners.map(|index| {
                    let vertex = self.mesh.vertices()[index];
                    DVec2::new(vertex.x, vertex.y)
                });
                covered = matches!(kernel::point_in_polyline(at, ring), PolyContainment::Inside | PolyContainment::OnBoundary);
            }
        });
        covered
    }

    /// Slope dz/dx, dz/dy at `at`: the plane z = a + gx x + gy y fitted by
    /// least squares to the nodes in the window around the nearest node.
    /// Where those nodes lie on one line the slope across it is zero.
    pub(crate) fn slope(&self, at: DVec2) -> DVec2 {
        let Some((column, row)) = self.nearest_node(at) else {
            return DVec2::ZERO;
        };
        let centre = self.heights[row * self.columns + column];
        // Sums over the nodes, in offsets from the centre node in metres.
        let (mut n, mut sx, mut sy, mut sxx, mut sxy, mut syy, mut sz, mut sxz, mut syz) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for down in -WINDOW_HALF..=WINDOW_HALF {
            for across in -WINDOW_HALF..=WINDOW_HALF {
                let (Ok(c), Ok(r)) = (usize::try_from(column as i64 + across), usize::try_from(row as i64 + down)) else {
                    continue;
                };
                let Some(height) = self.height(c, r) else {
                    continue;
                };
                let (x, y, z) = (across as f64 * self.spacing, down as f64 * self.spacing, height - centre);
                n += 1.0;
                sx += x;
                sy += y;
                sxx += x * x;
                sxy += x * y;
                syy += y * y;
                sz += z;
                sxz += x * z;
                syz += y * z;
            }
        }
        // Centred, the constant drops out and two equations are left.
        let (vxx, vxy, vyy) = (sxx - sx * sx / n, sxy - sx * sy / n, syy - sy * sy / n);
        let (vxz, vyz) = (sxz - sx * sz / n, syz - sy * sz / n);
        let determinant = vxx * vyy - vxy * vxy;
        let flat = self.spacing * self.spacing * 1e-9;
        if determinant.abs() > flat * (vxx + vyy) {
            return DVec2::new((vxz * vyy - vyz * vxy) / determinant, (vyz * vxx - vxz * vxy) / determinant);
        }
        DVec2::new(if vxx > flat { vxz / vxx } else { 0.0 }, if vyy > flat { vyz / vyy } else { 0.0 })
    }

    /// Dip in degrees and the azimuth of steepest descent at `at`, from
    /// [`Self::slope`].
    pub(crate) fn dip(&self, at: DVec2) -> Dip {
        let slope = self.slope(at);
        let steepness = (slope.x * slope.x + slope.y * slope.y).sqrt();
        let dip = libm::atan(steepness).to_degrees();
        if steepness == 0.0 {
            return Dip { dip, direction: 0.0 };
        }
        let mut direction = libm::atan2(-slope.x, -slope.y).to_degrees();
        if direction < 0.0 {
            direction += 360.0;
        }
        if direction >= 360.0 - 1e-9 {
            direction = 0.0;
        }
        Dip { dip, direction }
    }

    /// The node nearest `at`: the lattice position it rounds to when the
    /// surface has that node, else the nearest on the first ring outward
    /// that holds one.
    fn nearest_node(&self, at: DVec2) -> Option<(usize, usize)> {
        let local = ((at - self.origin) / self.spacing).round();
        let column = (local.x.max(0.0) as usize).min(self.columns - 1);
        let row = (local.y.max(0.0) as usize).min(self.rows - 1);
        for reach in 0..self.columns.max(self.rows) {
            let mut best: Option<(f64, (usize, usize))> = None;
            let (low_column, high_column) = (column.saturating_sub(reach), (column + reach).min(self.columns - 1));
            let (low_row, high_row) = (row.saturating_sub(reach), (row + reach).min(self.rows - 1));
            for r in low_row..=high_row {
                for c in low_column..=high_column {
                    if c.abs_diff(column).max(r.abs_diff(row)) != reach || self.height(c, r).is_none() {
                        continue;
                    }
                    let distance = (self.origin + DVec2::new(c as f64, r as f64) * self.spacing).distance_squared(at);
                    if best.is_none_or(|(nearest, _)| distance < nearest) {
                        best = Some((distance, (c, r)));
                    }
                }
            }
            if let Some((_, node)) = best {
                return Some(node);
            }
        }
        None
    }
}

/// The leg length, and the corner where the legs meet, when the triangle is
/// half of a square lattice cell: two equal legs, one along x and one along
/// y.
fn cell_leg(corners: [DVec2; 3]) -> Option<(f64, DVec2)> {
    for at in 0..3 {
        let (corner, next, previous) = (corners[at], corners[(at + 1) % 3], corners[(at + 2) % 3]);
        let (a, b) = (next - corner, previous - corner);
        let along = |leg: DVec2| (leg.y == 0.0 && leg.x != 0.0) || (leg.x == 0.0 && leg.y != 0.0);
        if along(a) && along(b) && (a.x == 0.0) != (b.x == 0.0) {
            let (first, second) = (a.length(), b.length());
            if (first - second).abs() <= first * NODE_TOLERANCE {
                return Some((first, corner));
            }
        }
    }
    None
}
