//! A prismoid grid: the prismoids between a roof horizon and a floor horizon
//! on one plan lattice, with a mask of the active cells and any number of
//! values per cell.

use std::{num::NonZeroUsize, sync::Arc};

use anyhow::{Result, bail};
use glam::{DVec2, DVec3};

use crate::{
    app::jobs::CancelFlag,
    model::{
        block_model::BlockBounds,
        drill_hole::DrillColorPreset,
        formats::mesh_data,
        grid_surface::{GridSurface, HEIGHT_TOLERANCE, NODE_TOLERANCE},
        progress::Phase,
        project::ProjectItemState,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PrismoidGridId(pub(crate) u64);

/// The grey a built prismoid grid starts in.
pub(crate) const DEFAULT_PRISMOID_GRID_COLOR: [f32; 4] = [0.72, 0.72, 0.75, 1.0];

/// The ramp vertical thickness is drawn on, thin blue to thick red.
pub(crate) const THICKNESS_RAMP: DrillColorPreset = DrillColorPreset::Rainbow;

/// Why two grid surfaces make no prismoid grid.
#[derive(Debug)]
pub(crate) enum BuildRefusal {
    /// Different spacings, or nodes that do not line up.
    NotOneLattice,
    /// Fewer than two node columns or rows in common, or no cell in
    /// common that both cover.
    NoOverlap,
    /// Fewer than two kept nodes along a side at the chosen cell size.
    CellTooLarge,
    /// No active cell, and more cells crossed than closed.
    RoofBelow,
    /// No active cell: roof and floor meet wherever both cover.
    NoThickness,
    Cancelled,
    /// The built grid failed [`PrismoidGrid::check`].
    Malformed(anyhow::Error),
}

/// A prismoid grid built between two surfaces, with the cells of the
/// overlap left out for a crossing and for a missing height or triangle.
#[derive(Debug)]
pub(crate) struct PrismoidBuild {
    pub(crate) grid: PrismoidGrid,
    pub(crate) crossed: usize,
    pub(crate) holes: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Corner {
    Missing,
    Crossed,
    Closed,
    Open,
}

/// A plan lattice: an origin, two horizontal unit axes and the cell sizes
/// along each.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Lattice {
    pub(crate) origin: DVec2,
    pub(crate) u: DVec2,
    pub(crate) v: DVec2,
    pub(crate) u_sizes: Vec<f64>,
    pub(crate) v_sizes: Vec<f64>,
}

impl Lattice {
    pub(crate) fn columns(&self) -> usize {
        self.u_sizes.len()
    }

    pub(crate) fn rows(&self) -> usize {
        self.v_sizes.len()
    }

    pub(crate) fn corner_count(&self) -> usize {
        (self.columns() + 1) * (self.rows() + 1)
    }

    pub(crate) fn cell_count(&self) -> usize {
        self.columns() * self.rows()
    }

    /// The plan position of every corner, in corner order.
    pub(crate) fn corners(&self) -> Vec<DVec2> {
        let (u_edges, v_edges) = (edges(&self.u_sizes), edges(&self.v_sizes));
        v_edges
            .iter()
            .flat_map(|along_v| u_edges.iter().map(move |along_u| self.origin + self.u * *along_u + self.v * *along_v))
            .collect()
    }

    /// Whether every column has one size and every row one size.
    pub(crate) fn is_regular(&self) -> bool {
        let uniform = |sizes: &[f64]| sizes.iter().all(|size| Some(size) == sizes.first());
        uniform(&self.u_sizes) && uniform(&self.v_sizes)
    }

    /// The smallest cell side.
    pub(crate) fn least_size(&self) -> f64 {
        self.u_sizes.iter().chain(&self.v_sizes).copied().fold(f64::INFINITY, f64::min)
    }
}

/// Distances from the lattice origin to each line between cells, from 0.
fn edges(sizes: &[f64]) -> Vec<f64> {
    let mut edges = vec![0.0];
    for size in sizes {
        edges.push(edges[edges.len() - 1] + size);
    }
    edges
}

/// One named value per cell, `None` where a cell has none.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CellValues {
    pub(crate) name: String,
    pub(crate) values: Vec<Option<f64>>,
}

/// Roof and floor elevations at every corner, row by row from the first
/// row: corner (`column`, `row`) at `row * (columns + 1) + column`, cell
/// (`column`, `row`) at `row * columns + column`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PrismoidGrid {
    pub(crate) lattice: Lattice,
    pub(crate) roof: Vec<f64>,
    pub(crate) floor: Vec<f64>,
    pub(crate) active: Vec<bool>,
    pub(crate) cell_values: Vec<CellValues>,
}

impl PrismoidGrid {
    /// Whether the grid is well formed: unit, perpendicular, right-handed
    /// axes; at least one cell, each size finite and positive; finite heights
    /// with the roof never below the floor; a mask and values for every cell.
    pub(crate) fn check(&self) -> Result<()> {
        let lattice = &self.lattice;
        if lattice.columns() == 0 || lattice.rows() == 0 {
            bail!("has no cells");
        }
        // The tolerances the OMF writer validates an orientation with.
        const THRESHOLD: f64 = 1e-6;
        if [lattice.u, lattice.v].iter().any(|axis| (1.0 - axis.length_squared()).abs() >= THRESHOLD) {
            bail!("lattice axes are not unit length");
        }
        let direction = |axis: DVec2| if axis.length() == 0.0 { DVec2::ZERO } else { axis / axis.length() };
        if direction(lattice.u).dot(direction(lattice.v)).abs() >= THRESHOLD {
            bail!("lattice axes are not perpendicular");
        }
        if lattice.u.perp_dot(lattice.v) <= 0.0 {
            bail!("lattice is not right-handed");
        }
        if !lattice.u_sizes.iter().chain(&lattice.v_sizes).all(|size| size.is_finite() && *size > 0.0) {
            bail!("has a cell size that is not finite and positive");
        }
        let corners = lattice.corner_count();
        if self.roof.len() != corners || self.floor.len() != corners {
            bail!("has {} roof and {} floor heights for {corners} corners", self.roof.len(), self.floor.len());
        }
        if !self.roof.iter().chain(&self.floor).all(|height| height.is_finite()) {
            bail!("has a height that is not finite");
        }
        if let Some(corner) = self.roof.iter().zip(&self.floor).position(|(roof, floor)| roof < floor) {
            bail!("has its floor above its roof at corner {corner}");
        }
        let cells = lattice.cell_count();
        if self.active.len() != cells {
            bail!("has {} mask values for {cells} cells", self.active.len());
        }
        if let Some(values) = self.cell_values.iter().find(|values| values.values.len() != cells) {
            bail!("has {} '{}' values for {cells} cells", values.values.len(), values.name);
        }
        Ok(())
    }

    /// The prismoids between `roof` and `floor` over the cells both cover,
    /// each `step` of the surfaces' cells along a side and read at its four
    /// corners only. A cell is active where both surfaces have triangles
    /// over every surface cell inside it, none of its corners lacks a height
    /// or has the floor above the roof, and at least one has thickness.
    pub(crate) fn between(roof: &GridSurface, floor: &GridSurface, step: NonZeroUsize, cancel: &CancelFlag, progress: &Phase) -> Result<PrismoidBuild, BuildRefusal> {
        let spacing = roof.spacing();
        if (spacing - floor.spacing()).abs() > spacing * NODE_TOLERANCE {
            return Err(BuildRefusal::NotOneLattice);
        }
        let (roof_origin, roof_columns, roof_rows) = roof.lattice();
        let (floor_origin, floor_columns, floor_rows) = floor.lattice();
        let offset = (floor_origin - roof_origin) / spacing;
        let whole = offset.round();
        if (offset - whole).abs().max_element() > NODE_TOLERANCE {
            return Err(BuildRefusal::NotOneLattice);
        }
        // The shared node box, in roof node indices.
        let span = |shift: f64, roof_count: usize, floor_count: usize| {
            let shift = shift as i64;
            let low = shift.max(0);
            let high = (shift + floor_count as i64).min(roof_count as i64);
            (low, high - low, shift)
        };
        let (low_column, columns, shift_column) = span(whole.x, roof_columns, floor_columns);
        let (low_row, rows, shift_row) = span(whole.y, roof_rows, floor_rows);
        if columns < 2 || rows < 2 {
            return Err(BuildRefusal::NoOverlap);
        }
        let (low_column, low_row) = (low_column as usize, low_row as usize);
        let first = roof_origin + DVec2::new(low_column as f64, low_row as f64) * spacing;
        // Along each side, the first node kept and how many are kept: every
        // `step`-th node, from a whole multiple of the cell size measured
        // from the coordinate origin, or from the first node of a lattice
        // off the spacing's multiples.
        let step = step.get();
        let kept_nodes = |at: f64, count: i64| {
            let nodes = at / spacing;
            let whole = nodes.round();
            let skip = if (nodes - whole).abs() > NODE_TOLERANCE {
                0
            } else {
                (step - (whole as i64).rem_euclid(step as i64) as usize) % step
            };
            (skip, (count as usize).saturating_sub(skip).div_ceil(step))
        };
        let ((skip_column, columns), (skip_row, rows)) = (kept_nodes(first.x, columns), kept_nodes(first.y, rows));
        if columns < 2 || rows < 2 {
            return Err(BuildRefusal::CellTooLarge);
        }
        let size = spacing * step as f64;
        let origin = first + DVec2::new(skip_column as f64, skip_row as f64) * spacing;
        // Roof and floor heights at every kept node, NaN where missing.
        let heights: Vec<(f64, f64)> = (0..rows)
            .flat_map(|row| (0..columns).map(move |column| (column, row)))
            .map(|(column, row)| {
                let (roof_column, roof_row) = (low_column + skip_column + column * step, low_row + skip_row + row * step);
                let floor_column = (roof_column as i64 - shift_column) as usize;
                let floor_row = (roof_row as i64 - shift_row) as usize;
                let height = |surface: &GridSurface, column: usize, row: usize| surface.height(column, row).unwrap_or(f64::NAN);
                (height(roof, roof_column, roof_row), height(floor, floor_column, floor_row))
            })
            .collect();
        if cancel.is_cancelled() {
            return Err(BuildRefusal::Cancelled);
        }
        progress.set_fraction(0.3);
        // Every surface cell inside the cell, at a point inside each of its
        // two triangles, whichever way its diagonal runs.
        let covered = |column: usize, row: usize| {
            (0..step * step).all(|inside| {
                let (fine_column, fine_row) = ((column * step + inside % step) as f64, (row * step + inside / step) as f64);
                [0.25, 0.75].into_iter().all(|across| {
                    let at = origin + DVec2::new(fine_column + across, fine_row + 0.5) * spacing;
                    roof.covers(at) && floor.covers(at)
                })
            })
        };
        let (cell_columns, cell_rows) = (columns - 1, rows - 1);
        let (mut crossed, mut holes, mut closed) = (0, 0, 0);
        let mut active = vec![false; cell_columns * cell_rows];
        // The smallest box of cells holding every active cell.
        let mut kept: Option<[usize; 4]> = None;
        for row in 0..cell_rows {
            if cancel.is_cancelled() {
                return Err(BuildRefusal::Cancelled);
            }
            for column in 0..cell_columns {
                let at = [
                    row * columns + column,
                    row * columns + column + 1,
                    (row + 1) * columns + column,
                    (row + 1) * columns + column + 1,
                ]
                .map(|index| corner(heights[index]));
                if at.contains(&Corner::Crossed) {
                    crossed += 1;
                } else if at.contains(&Corner::Missing) || !covered(column, row) {
                    holes += 1;
                } else if at.contains(&Corner::Open) {
                    active[row * cell_columns + column] = true;
                    kept = Some(kept.map_or([column, row, column, row], |[c0, r0, c1, r1]| [c0.min(column), r0.min(row), c1.max(column), r1.max(row)]));
                } else {
                    closed += 1;
                }
            }
        }
        let Some([first_column, first_row, last_column, last_row]) = kept else {
            return Err(if crossed == 0 && closed == 0 {
                BuildRefusal::NoOverlap
            } else if crossed > closed {
                BuildRefusal::RoofBelow
            } else {
                BuildRefusal::NoThickness
            });
        };
        progress.set_fraction(0.6);
        let (kept_columns, kept_rows) = (last_column - first_column + 1, last_row - first_row + 1);
        let (corner_columns, corner_rows) = (kept_columns + 1, kept_rows + 1);
        let (mut roof_heights, mut floor_heights): (Vec<f64>, Vec<f64>) = (first_row..=last_row + 1)
            .flat_map(|row| (first_column..=last_column + 1).map(move |column| row * columns + column))
            .map(|index| heights[index])
            .unzip();
        for values in [&mut roof_heights, &mut floor_heights] {
            fill_missing(values, corner_columns, corner_rows);
        }
        // Every corner of an active cell is closed or open, so this moves
        // only a closed corner's floor, by at most the tolerance, and the
        // corners no active cell reads.
        for (top, bottom) in roof_heights.iter().zip(&mut floor_heights) {
            *bottom = bottom.min(*top);
        }
        let active = (first_row..=last_row)
            .flat_map(|row| (first_column..=last_column).map(move |column| row * cell_columns + column))
            .map(|cell| active[cell])
            .collect();
        let grid = PrismoidGrid {
            lattice: Lattice {
                origin: origin + DVec2::new(first_column as f64, first_row as f64) * size,
                u: DVec2::X,
                v: DVec2::Y,
                u_sizes: vec![size; kept_columns],
                v_sizes: vec![size; kept_rows],
            },
            roof: roof_heights,
            floor: floor_heights,
            active,
            cell_values: Vec::new(),
        };
        grid.check().map_err(BuildRefusal::Malformed)?;
        progress.set_fraction(1.0);
        Ok(PrismoidBuild { grid, crossed, holes })
    }

    /// The vertical walls along every edge of an active cell whose neighbour
    /// is inactive or off the lattice, two outward-facing triangles each.
    pub(crate) fn outline_walls(&self) -> (Vec<DVec3>, Vec<[u32; 3]>) {
        let lattice = &self.lattice;
        let (columns, rows) = (lattice.columns(), lattice.rows());
        let corners = lattice.corners();
        let active = |column: Option<usize>, row: Option<usize>| match (column, row) {
            (Some(column), Some(row)) if column < columns && row < rows => self.active[row * columns + column],
            _ => false,
        };
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                if !active(Some(column), Some(row)) {
                    continue;
                }
                // Anticlockwise round the cell, each edge with the neighbour
                // across it.
                let edges = [
                    ((column, row), (column + 1, row), (Some(column), row.checked_sub(1))),
                    ((column + 1, row), (column + 1, row + 1), (Some(column + 1), Some(row))),
                    ((column + 1, row + 1), (column, row + 1), (Some(column), Some(row + 1))),
                    ((column, row + 1), (column, row), (column.checked_sub(1), Some(row))),
                ];
                for (from, to, (beside_column, beside_row)) in edges {
                    if active(beside_column, beside_row) {
                        continue;
                    }
                    let first = vertices.len() as u32;
                    for (corner_column, corner_row) in [from, to] {
                        let index = corner_row * (columns + 1) + corner_column;
                        let plan = corners[index];
                        vertices.push(plan.extend(self.roof[index]));
                        vertices.push(plan.extend(self.floor[index]));
                    }
                    let [from_roof, from_floor, to_roof, to_floor] = [first, first + 1, first + 2, first + 3];
                    triangles.extend([[to_roof, from_roof, from_floor], [to_roof, from_floor, to_floor]]);
                }
            }
        }
        (vertices, triangles)
    }

    /// The closed body of the active cells: roof, floor facing down, and
    /// [`Self::outline_walls`] less the triangles a closed corner flattens.
    /// `None` with no active cell.
    pub(crate) fn closed_mesh(&self) -> Option<PrismoidMesh> {
        const UNSET: u32 = u32::MAX;
        let columns = self.lattice.columns();
        let plan = self.lattice.corners();
        let mut vertices = Vec::new();
        let mut roof_at = vec![UNSET; plan.len()];
        let mut floor_at = vec![UNSET; plan.len()];
        let mut place = |at: &mut [u32], heights: &[f64], corner: usize| {
            if at[corner] == UNSET {
                at[corner] = vertices.len() as u32;
                vertices.push(mesh_vertex(plan[corner].extend(heights[corner])));
            }
            at[corner]
        };
        let mut faces = Vec::new();
        for (cell, _) in self.active.iter().enumerate().filter(|(_, active)| **active) {
            let a = (cell / columns) * (columns + 1) + cell % columns;
            let d = a + columns + 1;
            let corners = [a, a + 1, d + 1, d];
            let [ra, rb, rc, rd] = corners.map(|corner| place(&mut roof_at, &self.roof, corner));
            let [fa, fb, fc, fd] = corners.map(|corner| place(&mut floor_at, &self.floor, corner));
            faces.extend([[ra, rb, rc], [ra, rc, rd], [fa, fc, fb], [fa, fd, fc]]);
        }
        if faces.is_empty() {
            return None;
        }
        let (walls, wall_faces) = self.outline_walls();
        let offset = vertices.len() as u32;
        vertices.extend(walls.iter().map(|position| mesh_vertex(*position)));
        faces.extend(
            wall_faces
                .into_iter()
                .filter(|face| {
                    let [a, b, c] = face.map(|index| walls[index as usize]);
                    a != b && b != c && a != c
                })
                .map(|face| face.map(|index| index + offset)),
        );
        // Each wall quad runs from-roof, from-floor, to-roof, to-floor.
        let outline = (0..walls.len() as u32 / 4)
            .flat_map(|quad| {
                let first = offset + 4 * quad;
                [[first, first + 2], [first + 1, first + 3]]
            })
            .collect();
        Some(PrismoidMesh { vertices, faces, outline })
    }

    /// One box per active prismoid with its cell, in cell order, in the
    /// lattice's frame: x along `u` and y along `v` from the origin, z the
    /// elevation. Top and bottom are the roof's and floor's mean heights over
    /// the two triangles [`Self::closed_mesh`] draws, so a box holds its
    /// prismoid's volume. A cell whose box has no height is left out.
    pub(crate) fn blocks(&self) -> Vec<(usize, BlockBounds)> {
        let columns = self.lattice.columns();
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        let mean = |heights: &[f64], [a, b, c, d]: [usize; 4]| (2.0 * heights[a] + heights[b] + 2.0 * heights[c] + heights[d]) / 6.0;
        let mut blocks = Vec::new();
        for (cell, _) in self.active.iter().enumerate().filter(|(_, active)| **active) {
            let (column, row) = (cell % columns, cell / columns);
            let a = row * (columns + 1) + column;
            let d = a + columns + 1;
            let corners = [a, a + 1, d + 1, d];
            let (top, bottom) = (mean(&self.roof, corners), mean(&self.floor, corners));
            if top > bottom {
                blocks.push((
                    cell,
                    BlockBounds {
                        lower: DVec3::new(u_edges[column], v_edges[row], bottom),
                        upper: DVec3::new(u_edges[column + 1], v_edges[row + 1], top),
                    },
                ));
            }
        }
        blocks
    }

    /// Thickness at every corner as a position from 0 to 1 across `range`,
    /// 0.5 where the range is flat. `None` unless the lattice is regular: the
    /// map to the texture is affine, and a tensor lattice's corners are not
    /// evenly spaced.
    pub(crate) fn thickness_map(&self, (min, max): (f64, f64), max_side: u32) -> Option<ThicknessMap> {
        let lattice = &self.lattice;
        if !lattice.is_regular() {
            return None;
        }
        // A side too long for a texture keeps every `step`-th corner, its
        // last texel clamped to the last corner.
        let fit = |side: usize| {
            let step = (side - 1).div_ceil(max_side as usize - 1);
            (step, (side - 1).div_ceil(step) + 1)
        };
        let (columns, rows) = (lattice.columns() + 1, lattice.rows() + 1);
        let ((column_step, width), (row_step, height)) = (fit(columns), fit(rows));
        let position = |corner: usize| {
            if (max - min).abs() <= HEIGHT_TOLERANCE {
                0.5
            } else {
                ((self.roof[corner] - self.floor[corner] - min) / (max - min)).clamp(0.0, 1.0) as f32
            }
        };
        let positions = (0..height)
            .flat_map(|row| (0..width).map(move |column| (column, row)))
            .map(|(column, row)| position((row * row_step).min(rows - 1) * columns + (column * column_step).min(columns - 1)))
            .collect();
        // Texel centres on the corners kept.
        let along = |axis: DVec2, size: f64, step: usize, count: usize| {
            let (spacing, count) = (size * step as f64, count as f64);
            [axis.x / (spacing * count), axis.y / (spacing * count), (0.5 - lattice.origin.dot(axis) / spacing) / count]
        };
        let [a, b, c] = along(lattice.u, lattice.u_sizes[0], column_step, width);
        let [d, e, f] = along(lattice.v, lattice.v_sizes[0], row_step, height);
        Some(ThicknessMap {
            size: [width as u32, height as u32],
            positions,
            world_to_uv: [a, b, c, d, e, f],
        })
    }

    /// The lattice size, the active cells, and the range of roof minus floor
    /// over every corner of an active cell.
    pub(crate) fn summary(&self) -> PrismoidGridSummary {
        let (columns, rows) = (self.lattice.columns(), self.lattice.rows());
        let mut thickness: Option<(f64, f64)> = None;
        let mut active_cells = 0;
        for (cell, _) in self.active.iter().enumerate().filter(|(_, active)| **active) {
            active_cells += 1;
            let (column, row) = (cell % columns, cell / columns);
            for corner in [
                row * (columns + 1) + column,
                row * (columns + 1) + column + 1,
                (row + 1) * (columns + 1) + column,
                (row + 1) * (columns + 1) + column + 1,
            ] {
                let height = self.roof[corner] - self.floor[corner];
                thickness = Some(thickness.map_or((height, height), |(min, max)| (min.min(height), max.max(height))));
            }
        }
        PrismoidGridSummary {
            columns,
            rows,
            cells: columns * rows,
            active_cells,
            vertical_thickness: thickness,
        }
    }

    /// Bytes held by the heights, the mask and the cell values.
    pub(crate) fn estimated_bytes(&self) -> usize {
        let values: usize = self
            .cell_values
            .iter()
            .map(|cell_values| cell_values.name.len() + cell_values.values.len() * size_of::<Option<f64>>())
            .sum();
        (self.roof.len() + self.floor.len()) * size_of::<f64>() + self.active.len() + values
    }
}

/// A prismoid grid as one closed mesh, with the line round its roof and floor.
pub(crate) struct PrismoidMesh {
    pub(crate) vertices: Vec<mesh_data::Vertex>,
    pub(crate) faces: Vec<[u32; 3]>,
    pub(crate) outline: Vec<[u32; 2]>,
}

/// A grid's vertical thickness as positions on [`THICKNESS_RAMP`], row by
/// row, with the map from world XY to their texture coordinates.
pub(crate) struct ThicknessMap {
    pub(crate) size: [u32; 2],
    pub(crate) positions: Vec<f32>,
    pub(crate) world_to_uv: [f64; 6],
}

fn mesh_vertex(position: DVec3) -> mesh_data::Vertex {
    mesh_data::Vertex::new(position.x, position.y, position.z)
}

/// What a prismoid grid's row reports about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PrismoidGridSummary {
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    pub(crate) cells: usize,
    pub(crate) active_cells: usize,
    /// Roof minus floor, smallest and largest; `None` with no active cell.
    pub(crate) vertical_thickness: Option<(f64, f64)>,
}

/// A prismoid grid open in the project.
#[derive(Clone, Debug)]
pub(crate) struct OpenPrismoidGrid {
    pub(crate) id: PrismoidGridId,
    pub(crate) state: ProjectItemState,
    pub(crate) name: String,
    pub(crate) color: [f32; 4],
    pub(crate) grid: Arc<PrismoidGrid>,
    /// Computed once: `grid` is an immutable `Arc`, so it cannot go stale.
    pub(crate) summary: PrismoidGridSummary,
}

impl OpenPrismoidGrid {
    pub(crate) fn entity_id(&self) -> crate::model::SceneEntityId {
        crate::model::SceneEntityId::PrismoidGrid(self.id)
    }
}

/// A prismoid grid read from a file, before the project gives it an id.
pub(crate) struct LoadedPrismoidGrid {
    pub(crate) name: String,
    pub(crate) color: [f32; 4],
    pub(crate) grid: Arc<PrismoidGrid>,
    pub(crate) summary: PrismoidGridSummary,
}

/// How a node's roof and floor stand, from their heights (NaN where
/// missing).
fn corner((top, bottom): (f64, f64)) -> Corner {
    if top.is_nan() || bottom.is_nan() {
        Corner::Missing
    } else if top - bottom < -HEIGHT_TOLERANCE {
        Corner::Crossed
    } else if top - bottom <= HEIGHT_TOLERANCE {
        Corner::Closed
    } else {
        Corner::Open
    }
}

/// Give every NaN the value of the nearest known one in steps across the
/// lattice, breadth first from every known value at once.
fn fill_missing(values: &mut [f64], columns: usize, rows: usize) {
    let mut queue: std::collections::VecDeque<usize> = (0..values.len()).filter(|&index| !values[index].is_nan()).collect();
    while let Some(index) = queue.pop_front() {
        let (column, row) = (index % columns, index / columns);
        let neighbours = [
            column.checked_sub(1).map(|column| row * columns + column),
            (column + 1 < columns).then(|| row * columns + column + 1),
            row.checked_sub(1).map(|row| row * columns + column),
            (row + 1 < rows).then(|| (row + 1) * columns + column),
        ];
        for next in neighbours.into_iter().flatten() {
            if values[next].is_nan() {
                values[next] = values[index];
                queue.push_back(next);
            }
        }
    }
}
