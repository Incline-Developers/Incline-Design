//! A prismoid grid: the prismoids between a roof horizon and a floor horizon
//! on one plan lattice, with a mask of the active cells and any number of
//! values per cell.

use std::{num::NonZeroUsize, sync::Arc};

use anyhow::{Result, bail};
use glam::{DMat2, DMat3, DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::{
    app::jobs::CancelFlag,
    model::{
        block_model::BlockBounds,
        drill_hole::DrillColorPreset,
        formats::mesh_data,
        grid_surface::{GridSurface, HEIGHT_TOLERANCE, NODE_TOLERANCE, surface_z_single},
        progress::Phase,
        project::{ModelEnvelope, ProjectItemState},
        solid_reserves::NumericTotal,
        spatial::TriangleBvh,
        triangulation::{quality_key, same_quality_name},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PrismoidGridId(pub(crate) u64);

/// The grey a built prismoid grid starts in.
pub(crate) const DEFAULT_PRISMOID_GRID_COLOR: [f32; 4] = [0.72, 0.72, 0.75, 1.0];

/// The ramp a grid is drawn on until another is chosen, low blue to high red.
pub(crate) const THICKNESS_RAMP: DrillColorPreset = DrillColorPreset::Rainbow;

/// What a prismoid grid is coloured by: one of its values by name, or its
/// vertical thickness with `None`, on one of Leo's colour scales.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PrismoidColorBy {
    pub(crate) value: Option<String>,
    pub(crate) preset: DrillColorPreset,
}

impl Default for PrismoidColorBy {
    fn default() -> Self {
        Self {
            value: None,
            preset: THICKNESS_RAMP,
        }
    }
}

impl PrismoidColorBy {
    /// The position of the first of `names` that is the chosen value, by the
    /// rule qualities are matched by; `None` for thickness or a name not held.
    pub(crate) fn index<'a>(&self, mut names: impl Iterator<Item = &'a str>) -> Option<usize> {
        let value = self.value.as_deref()?;
        names.position(|name| same_quality_name(name, value))
    }
}

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
    /// The surfaces' nodes are no whole number of spacings from the model
    /// envelope's corner.
    OffEnvelopeLattice,
    /// An active cell leaves the model envelope.
    OutsideEnvelope,
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

    /// The (column, row) of the cell holding a plan point; a point on a line
    /// between cells belongs to the cell beyond it, and the far edge to the
    /// last cell. `None` outside the lattice.
    pub(crate) fn cell_at(&self, at: DVec2) -> Option<(usize, usize)> {
        self.cell_at_in(at, &edges(&self.u_sizes), &edges(&self.v_sizes))
    }

    /// [`Self::cell_at`] from the lattice's edge distances.
    fn cell_at_in(&self, at: DVec2, u_edges: &[f64], v_edges: &[f64]) -> Option<(usize, usize)> {
        let DVec2 { x: s, y: t } = self.to_lattice(at);
        let find = |edges: &[f64], along: f64| {
            let total = edges[edges.len() - 1];
            // Written so NaN is outside.
            if !(0.0..=total).contains(&along) {
                return None;
            }
            Some((edges.partition_point(|edge| *edge <= along) - 1).min(edges.len() - 2))
        };
        Some((find(u_edges, s)?, find(v_edges, t)?))
    }

    /// A plan point as distances along `u` and `v` from the origin: the exact
    /// inverse of `corners`, also for axes `check` admits a hair off unit and
    /// perpendicular.
    fn to_lattice(&self, at: DVec2) -> DVec2 {
        DMat2::from_cols(self.u, self.v).inverse() * (at - self.origin)
    }

    /// The plan position of the middle of a cell.
    pub(crate) fn cell_centre(&self, column: usize, row: usize) -> DVec2 {
        self.cell_centre_in(column, row, &edges(&self.u_sizes), &edges(&self.v_sizes))
    }

    /// [`Self::cell_centre`] from the lattice's edge distances.
    fn cell_centre_in(&self, column: usize, row: usize, u_edges: &[f64], v_edges: &[f64]) -> DVec2 {
        let s = (u_edges[column] + u_edges[column + 1]) / 2.0;
        let t = (v_edges[row] + v_edges[row + 1]) / 2.0;
        self.origin + self.u * s + self.v * t
    }

    /// Where a plan point lies in a cell, from 0 to 1 along `u` and along `v`.
    fn local_in(&self, column: usize, row: usize, at: DVec2, u_edges: &[f64], v_edges: &[f64]) -> DVec2 {
        let DVec2 { x: s, y: t } = self.to_lattice(at);
        DVec2::new((s - u_edges[column]) / self.u_sizes[column], (t - v_edges[row]) / self.v_sizes[row])
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
    /// As the user typed it; empty when the values have none.
    pub(crate) unit: String,
    pub(crate) role: CellValueRole,
    pub(crate) values: Vec<Option<f64>>,
}

/// What a grid's values are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellValueRole {
    Quality,
    /// The density tonnes are measured with, in t/m³; one per grid.
    Density,
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
    /// with the roof never below the floor; a mask and values for every cell,
    /// each set of values named, no name twice, at most one density.
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
        if self.cell_values.iter().any(|values| values.name.trim().is_empty()) {
            bail!("has values without a name");
        }
        let mut names = std::collections::HashSet::new();
        if let Some(values) = self.cell_values.iter().find(|values| !names.insert(quality_key(&values.name))) {
            bail!("has '{}' values twice", values.name);
        }
        if self.cell_values.iter().filter(|values| values.role == CellValueRole::Density).count() > 1 {
            bail!("has more than one density");
        }
        Ok(())
    }

    /// The prismoids between `roof` and `floor` over the cells both cover,
    /// each `step` of the surfaces' cells along a side and read at its four
    /// corners only. A cell is active where both surfaces have triangles
    /// over every surface cell inside it, none of its corners lacks a height
    /// or has the floor above the roof, and at least one has thickness. With
    /// an `envelope`, every active cell must lie inside it.
    pub(crate) fn between(
        roof: &GridSurface,
        floor: &GridSurface,
        step: NonZeroUsize,
        envelope: Option<&ModelEnvelope>,
        cancel: &CancelFlag,
        progress: &Phase,
    ) -> Result<PrismoidBuild, BuildRefusal> {
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
        // from the envelope's corner, else the coordinate origin. A lattice
        // off the spacing's multiples starts at its first node; under an
        // envelope it is refused.
        let step = step.get();
        let anchor = envelope.map_or(DVec2::ZERO, |envelope| envelope.lower().truncate());
        let kept_nodes = |at: f64, anchor: f64, count: i64| {
            let nodes = (at - anchor) / spacing;
            let whole = nodes.round();
            let skip = if (nodes - whole).abs() <= NODE_TOLERANCE {
                (step - (whole as i64).rem_euclid(step as i64) as usize) % step
            } else if envelope.is_none() {
                0
            } else {
                return None;
            };
            Some((skip, (count as usize).saturating_sub(skip).div_ceil(step)))
        };
        let (Some((skip_column, columns)), Some((skip_row, rows))) = (kept_nodes(first.x, anchor.x, columns), kept_nodes(first.y, anchor.y, rows)) else {
            return Err(BuildRefusal::OffEnvelopeLattice);
        };
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
        if envelope.is_some_and(|envelope| !grid.fits(envelope)) {
            return Err(BuildRefusal::OutsideEnvelope);
        }
        progress.set_fraction(1.0);
        Ok(PrismoidBuild { grid, crossed, holes })
    }

    /// Whether the grid lies in `envelope`: its lattice on the envelope's
    /// axes, and every active cell's corners inside it, roof and floor.
    pub(crate) fn fits(&self, envelope: &ModelEnvelope) -> bool {
        let lattice = &self.lattice;
        if (lattice.u, lattice.v) != (DVec2::X, DVec2::Y) {
            return false;
        }
        let (corners, columns) = (lattice.corners(), lattice.columns());
        self.active.iter().enumerate().filter(|(_, active)| **active).all(|(cell, _)| {
            let first = cell / columns * (columns + 1) + cell % columns;
            [first, first + 1, first + columns + 1, first + columns + 2]
                .into_iter()
                .all(|corner| envelope.contains_xy(corners[corner]) && envelope.contains_z(self.roof[corner]) && envelope.contains_z(self.floor[corner]))
        })
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

    /// Roof and floor segments round the rim of a set of cells: `cells` is a
    /// mask in cell order, and each side of a selected cell whose neighbour
    /// is not selected (or is off the lattice) gives one roof and one floor
    /// segment.
    pub(crate) fn selection_outline(&self, cells: &[bool]) -> Vec<[DVec3; 2]> {
        let lattice = &self.lattice;
        let (columns, rows) = (lattice.columns(), lattice.rows());
        let corners = lattice.corners();
        let selected = |column: Option<usize>, row: Option<usize>| match (column, row) {
            (Some(column), Some(row)) if column < columns && row < rows => cells[row * columns + column],
            _ => false,
        };
        let mut segments = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                if !selected(Some(column), Some(row)) {
                    continue;
                }
                let sides = [
                    ((column, row), (column + 1, row), (Some(column), row.checked_sub(1))),
                    ((column + 1, row), (column + 1, row + 1), (Some(column + 1), Some(row))),
                    ((column + 1, row + 1), (column, row + 1), (Some(column), Some(row + 1))),
                    ((column, row + 1), (column, row), (column.checked_sub(1), Some(row))),
                ];
                for (from, to, (beside_column, beside_row)) in sides {
                    if selected(beside_column, beside_row) {
                        continue;
                    }
                    let [from, to] = [from, to].map(|(corner_column, corner_row)| corner_row * (columns + 1) + corner_column);
                    segments.push([corners[from].extend(self.roof[from]), corners[to].extend(self.roof[to])]);
                    segments.push([corners[from].extend(self.floor[from]), corners[to].extend(self.floor[to])]);
                }
            }
        }
        segments
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
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        let mut blocks = Vec::new();
        for (cell, _) in self.active.iter().enumerate().filter(|(_, active)| **active) {
            if let Some(bounds) = self.cell_box(cell, &u_edges, &v_edges) {
                blocks.push((cell, bounds));
            }
        }
        blocks
    }

    /// The box of one cell as [`Self::blocks`] gives it, from the lattice's
    /// edge distances; `None` when it has no height.
    fn cell_box(&self, cell: usize, u_edges: &[f64], v_edges: &[f64]) -> Option<BlockBounds> {
        let columns = self.lattice.columns();
        let mean = |heights: &[f64], [a, b, c, d]: [usize; 4]| (2.0 * heights[a] + heights[b] + 2.0 * heights[c] + heights[d]) / 6.0;
        let (column, row) = (cell % columns, cell / columns);
        let a = row * (columns + 1) + column;
        let d = a + columns + 1;
        let corners = [a, a + 1, d + 1, d];
        let (top, bottom) = (mean(&self.roof, corners), mean(&self.floor, corners));
        (top > bottom).then(|| BlockBounds {
            lower: DVec3::new(u_edges[column], v_edges[row], bottom),
            upper: DVec3::new(u_edges[column + 1], v_edges[row + 1], top),
        })
    }

    /// The active cell a hit point on the drawn body (roof, floor or an
    /// outline wall) belongs to. A wall stands only where the neighbour is
    /// inactive or off the lattice, so of its two sides only one is a body.
    pub(crate) fn picked_cell(&self, at: DVec3) -> Option<usize> {
        let lattice = &self.lattice;
        let columns = lattice.columns();
        let slack = 1e-6 * lattice.least_size();
        let plan = at.truncate();
        let (u, v) = (lattice.u * slack, lattice.v * slack);
        let mut found: Vec<usize> = [DVec2::ZERO, u, -u, v, -v, u + v, u - v, v - u, -u - v]
            .into_iter()
            .filter_map(|shift| lattice.cell_at(plan + shift))
            .map(|(column, row)| row * columns + column)
            .filter(|cell| self.active[*cell])
            .collect();
        found.sort_unstable();
        found.dedup();
        let mut best: Option<(usize, f64)> = None;
        for cell in found {
            let distance = (lattice.cell_centre(cell % columns, cell / columns) - plan).length_squared();
            if best.is_none_or(|(_, nearest)| distance < nearest) {
                best = Some((cell, distance));
            }
        }
        best.map(|(cell, _)| cell)
    }

    /// Where the vertical plane through `point` with horizontal unit `normal`
    /// crosses the active cells, in order along the plane: distances from
    /// `point` towards `(-normal.y, normal.x)`, as a section frames them.
    /// Heights lie on the triangles [`Self::closed_mesh`] draws, so a piece
    /// across a cell's a-c diagonal gains a point on it.
    pub(crate) fn section_crossings(&self, point: DVec2, normal: DVec2) -> Vec<SectionCrossing> {
        const LEVEL: f64 = 1e-12;
        let lattice = &self.lattice;
        let columns = lattice.columns();
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let along = DVec2::new(-normal.y, normal.x);
        let inverse = DMat2::from_cols(lattice.u, lattice.v).inverse();
        let (start, slope) = (inverse * (point - lattice.origin), inverse * along);
        let axes = [(start.x, slope.x, &u_edges), (start.y, slope.y, &v_edges)];
        let (mut enter, mut leave) = (f64::NEG_INFINITY, f64::INFINITY);
        for (from, rate, edges) in axes {
            let total = edges[edges.len() - 1];
            if rate.abs() <= LEVEL {
                if !(0.0..=total).contains(&from) {
                    return Vec::new();
                }
            } else {
                let (first, last) = (-from / rate, (total - from) / rate);
                enter = enter.max(first.min(last));
                leave = leave.min(first.max(last));
            }
        }
        let crossed = enter.is_finite() && leave.is_finite() && enter < leave;
        if !crossed {
            return Vec::new();
        }
        let mut stops = vec![enter, leave];
        for (from, rate, edges) in axes {
            if rate.abs() > LEVEL {
                stops.extend(edges.iter().map(|edge| (edge - from) / rate).filter(|stop| enter < *stop && *stop < leave));
            }
        }
        stops.sort_by(f64::total_cmp);
        let shortest = 1e-9 * lattice.least_size();
        let mut crossings = Vec::new();
        for pair in stops.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if to - from <= shortest {
                continue;
            }
            // The middle of the piece, so a trace on a lattice line takes the
            // cells beyond it, as `cell_at` does.
            let Some((column, row)) = lattice.cell_at_in(point + along * ((from + to) / 2.0), &u_edges, &v_edges) else {
                continue;
            };
            let cell = row * columns + column;
            if !self.active[cell] {
                continue;
            }
            let ends = [from, to].map(|stop| point + along * stop);
            let [first, last] = ends.map(|plan| {
                let local = lattice.local_in(column, row, plan, &u_edges, &v_edges);
                local.x - local.y
            });
            let mut plans = vec![ends[0]];
            if first * last < 0.0 {
                plans.push(point + along * (from + (to - from) * first / (first - last)));
            }
            plans.push(ends[1]);
            let points = plans
                .into_iter()
                .map(|plan| {
                    let (roof, floor) = self.heights_at(cell, plan, &u_edges, &v_edges);
                    SectionPoint { plan, roof, floor }
                })
                .collect();
            crossings.push(SectionCrossing { cell, along: [from, to], points });
        }
        crossings
    }

    /// Roof and floor over a plan point in a cell, on the triangles
    /// [`Self::closed_mesh`] draws: with corners a, b (along `u`), c and d
    /// (along `v`), `[a, b, c]` where the point is at least as far across the
    /// cell along `u` as along `v`, else `[a, c, d]`.
    fn heights_at(&self, cell: usize, plan: DVec2, u_edges: &[f64], v_edges: &[f64]) -> (f64, f64) {
        let lattice = &self.lattice;
        let columns = lattice.columns();
        let (column, row) = (cell % columns, cell / columns);
        let local = lattice.local_in(column, row, plan, u_edges, v_edges).clamp(DVec2::ZERO, DVec2::ONE);
        let (fs, ft) = (local.x, local.y);
        let a = row * (columns + 1) + column;
        let d = a + columns + 1;
        let (b, c) = (a + 1, d + 1);
        let height = |z: &[f64]| {
            if fs >= ft {
                z[a] + fs * (z[b] - z[a]) + ft * (z[c] - z[b])
            } else {
                z[a] + ft * (z[d] - z[a]) + fs * (z[c] - z[d])
            }
        };
        (height(&self.roof), height(&self.floor))
    }

    /// The active cell a point on a section lies in, between the floor and
    /// the roof as drawn; `None` above the roof, below the floor, off the
    /// lattice or on an inactive cell.
    #[allow(dead_code, reason = "picked from the section in a later commit")]
    pub(crate) fn section_cell_at(&self, at: DVec3) -> Option<usize> {
        let lattice = &self.lattice;
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let plan = at.truncate();
        let (column, row) = lattice.cell_at_in(plan, &u_edges, &v_edges)?;
        let cell = row * lattice.columns() + column;
        if !self.active[cell] {
            return None;
        }
        let (roof, floor) = self.heights_at(cell, plan, &u_edges, &v_edges);
        (floor <= at.z && at.z <= roof).then_some(cell)
    }

    /// The boxes [`Self::blocks`] gives in the frame of a block model made
    /// from the grid: the lattice's, or with an `envelope` (the grid on its
    /// axes) the envelope's, from its corner at the base RL.
    pub(crate) fn model_blocks(&self, envelope: Option<&ModelEnvelope>) -> Vec<(usize, BlockBounds)> {
        let blocks = self.blocks();
        let Some(envelope) = envelope else { return blocks };
        let shift = self.lattice.origin.extend(0.0) - envelope.lower();
        blocks
            .into_iter()
            .map(|(cell, block)| {
                let (lower, upper) = (block.lower + shift, block.upper + shift);
                (cell, BlockBounds { lower, upper })
            })
            .collect()
    }

    /// One active cell's box in the lattice's frame, as [`Self::blocks`]
    /// gives it, measured as `block_volumes` and `per_volume_values` measure
    /// a block model's; `None` when the box has no height.
    pub(crate) fn cell_measure(&self, cell: usize) -> Option<(BlockBounds, CellMeasure)> {
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        self.measure_in(cell, &u_edges, &v_edges, self.stretch())
    }

    /// [`Self::cell_measure`] from the lattice's edge distances and stretch.
    fn measure_in(&self, cell: usize, u_edges: &[f64], v_edges: &[f64], stretch: f64) -> Option<(BlockBounds, CellMeasure)> {
        let bounds = self.cell_box(cell, u_edges, v_edges)?;
        let measure = CellMeasure {
            cell,
            sides: (bounds.upper - bounds.lower).element_product(),
            stretch,
        };
        Some((bounds, measure))
    }

    /// Each active cell with its box's centre: the cell's plan centre at the
    /// middle of its box's height. A cell whose box has no height is left out.
    pub(crate) fn cell_centres_3d(&self) -> impl Iterator<Item = (usize, DVec3)> + '_ {
        let lattice = &self.lattice;
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let columns = lattice.columns();
        self.active.iter().enumerate().filter(|(_, active)| **active).filter_map(move |(cell, _)| {
            let bounds = self.cell_box(cell, &u_edges, &v_edges)?;
            let centre = lattice.cell_centre_in(cell % columns, cell / columns, &u_edges, &v_edges);
            Some((cell, centre.extend((bounds.lower.z + bounds.upper.z) / 2.0)))
        })
    }

    /// How much the lattice's frame scales a volume.
    fn stretch(&self) -> f64 {
        let lattice = &self.lattice;
        DMat3::from_cols(lattice.u.extend(0.0), lattice.v.extend(0.0), DVec3::Z).determinant().abs()
    }

    /// Thickness at every corner as a position from 0 to 1 across `range`,
    /// 0.5 where the range is flat. `None` unless the lattice is regular: the
    /// map to the texture is affine, and a tensor lattice's corners are not
    /// evenly spaced.
    pub(crate) fn thickness_map(&self, range: (f64, f64), max_side: u32) -> Option<ThicknessMap> {
        let lattice = &self.lattice;
        if !lattice.is_regular() {
            return None;
        }
        // A side too long for a texture keeps every `step`-th corner, its
        // last texel clamped to the last corner.
        let (along_u, along_v) = (texel_fit(lattice.columns(), max_side, 1), texel_fit(lattice.rows(), max_side, 1));
        let ((column_step, width), (row_step, height)) = (along_u, along_v);
        let (columns, rows) = (lattice.columns() + 1, lattice.rows() + 1);
        let positions = (0..height)
            .flat_map(|row| (0..width).map(move |column| (column, row)))
            .map(|(column, row)| {
                let corner = (row * row_step).min(rows - 1) * columns + (column * column_step).min(columns - 1);
                ramp_position(self.roof[corner] - self.floor[corner], range)
            })
            .collect();
        // Texel centres on the corners kept.
        Some(ThicknessMap {
            size: [width as u32, height as u32],
            positions,
            world_to_uv: texel_world_to_uv(lattice, along_u, along_v, 0.5),
        })
    }

    /// One cell value as positions from 0 to 1 across `range` (0.5 where the
    /// range is flat), a texel per cell inside a ring of one. `None` unless
    /// the lattice is regular, or when `index` names no set of values.
    pub(crate) fn cell_value_map(&self, index: usize, range: (f64, f64), max_side: u32) -> Option<CellValueMap> {
        let lattice = &self.lattice;
        if !lattice.is_regular() {
            return None;
        }
        let values = &self.cell_values.get(index)?.values;
        let columns = lattice.columns();
        // A side too long for a texture keeps every `step`-th cell.
        let (along_u, along_v) = (texel_fit(columns, max_side, 2), texel_fit(lattice.rows(), max_side, 2));
        let ((column_step, width), (row_step, height)) = (along_u, along_v);
        let position = |cell: usize| Some(ramp_position(values[cell].filter(|value| value.is_finite())?, range));
        // Outer `None`: the texel is not an active cell.
        let held: Vec<Option<Option<f32>>> = (0..width * height)
            .map(|texel| {
                let (i, j) = (texel % width, texel / width);
                if i == 0 || j == 0 || i == width - 1 || j == height - 1 {
                    return None;
                }
                let cell = (j - 1) * row_step * columns + (i - 1) * column_step;
                self.active[cell].then(|| position(cell))
            })
            .collect();
        // A wall stands on a cell edge, a texel boundary, so its fragments
        // fall either side by rounding; the neighbour holding the same
        // colour makes a straight wall read as its cell.
        const AROUND: [(isize, isize); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
        let positions = (0..width * height)
            .map(|texel| {
                if let Some(own) = held[texel] {
                    return own;
                }
                let (i, j) = (texel % width, texel / width);
                AROUND
                    .iter()
                    .find_map(|(di, dj)| {
                        let (x, y) = (i.checked_add_signed(*di)?, j.checked_add_signed(*dj)?);
                        if x >= width || y >= height {
                            return None;
                        }
                        held[y * width + x]
                    })
                    .flatten()
            })
            .collect();
        // Texel i lies over kept column i - 1, so a cell's centre lands on
        // its texel's centre.
        Some(CellValueMap {
            size: [width as u32, height as u32],
            positions,
            world_to_uv: texel_world_to_uv(lattice, along_u, along_v, 1.0),
        })
    }

    /// The lattice size, the active cells, the range of roof minus floor
    /// over every corner of an active cell, and the totals: a derived block
    /// model's bit for bit without an envelope, to rounding with one.
    pub(crate) fn summary(&self) -> PrismoidGridSummary {
        self.summary_of(self.active.iter().enumerate().filter(|(_, active)| **active).map(|(cell, _)| cell))
    }

    /// [`Self::summary`] over the given active cells, taken in cell order:
    /// `active_cells` counts them.
    pub(crate) fn summary_of(&self, cells: impl Iterator<Item = usize>) -> PrismoidGridSummary {
        let (columns, rows) = (self.lattice.columns(), self.lattice.rows());
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        let stretch = self.stretch();
        let mut thickness: Option<(f64, f64)> = None;
        let mut active_cells = 0;
        let density = self.cell_values.iter().find(|values| values.role == CellValueRole::Density);
        let mut tonnes = density.map(|_| NumericTotal::default());
        let mut qualities: Vec<(&CellValues, NumericTotal)> = self
            .cell_values
            .iter()
            .filter(|values| values.role == CellValueRole::Quality)
            .map(|values| (values, NumericTotal::default()))
            .collect();
        let mut spreads: Vec<ValueSpread> = self
            .cell_values
            .iter()
            .map(|values| ValueSpread {
                name: values.name.clone(),
                unit: values.unit.clone(),
                range: None,
                missing: 0,
            })
            .collect();
        let mut volume = 0.0;
        for cell in cells {
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
            for (values, spread) in self.cell_values.iter().zip(&mut spreads) {
                spread.take(values.values[cell]);
            }
            // A box with no height holds nothing, as `blocks` leaves it out.
            let Some((_, measure)) = self.measure_in(cell, &u_edges, &v_edges, stretch) else {
                continue;
            };
            let cell_volume = measure.volume();
            volume += cell_volume;
            let cell_tonnes = density.map(|density| density.values[measure.cell].map(|value| measure.per_volume(value)));
            if let Some(total) = tonnes.as_mut() {
                add_summed(total, cell_tonnes.flatten());
            }
            // By tonnes where the grid has a density, never mixed with volume.
            let weight = cell_tonnes.unwrap_or(Some(cell_volume));
            for (values, total) in &mut qualities {
                add_weighted(total, values.values[measure.cell], weight);
            }
        }
        PrismoidGridSummary {
            columns,
            rows,
            active_cells,
            vertical_thickness: thickness,
            volume,
            weighting: if tonnes.is_some() { Weighting::ByTonnes } else { Weighting::ByVolume },
            tonnes,
            density: density.map(|density| density.name.clone()),
            qualities: qualities
                .into_iter()
                .map(|(values, total)| QualityTotal {
                    name: values.name.clone(),
                    unit: values.unit.clone(),
                    total,
                })
                .collect(),
            spreads,
        }
    }

    /// Bytes held by the heights, the mask and the cell values.
    pub(crate) fn estimated_bytes(&self) -> usize {
        let values: usize = self
            .cell_values
            .iter()
            .map(|cell_values| cell_values.name.len() + cell_values.unit.len() + cell_values.values.len() * size_of::<Option<f64>>())
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

/// Where a vertical plane crosses one active prismoid: the cell, the span of
/// distances along the plane, and two points (three across the drawn
/// diagonal) in order along it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SectionCrossing {
    pub(crate) cell: usize,
    pub(crate) along: [f64; 2],
    pub(crate) points: Vec<SectionPoint>,
}

/// A point of a [`SectionCrossing`]: its plan position and the drawn roof
/// and floor over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SectionPoint {
    pub(crate) plan: DVec2,
    pub(crate) roof: f64,
    pub(crate) floor: f64,
}

/// A grid's vertical thickness as positions on the grid's colour scale, row
/// by row, with the map from world XY to their texture coordinates.
pub(crate) struct ThicknessMap {
    pub(crate) size: [u32; 2],
    pub(crate) positions: Vec<f32>,
    pub(crate) world_to_uv: [f64; 6],
}

/// One value as positions on a colour scale, a texel per cell inside a ring
/// of one texel, row by row, `None` where a cell has no value, with the map
/// from world XY to texture coordinates.
pub(crate) struct CellValueMap {
    pub(crate) size: [u32; 2],
    pub(crate) positions: Vec<Option<f32>>,
    pub(crate) world_to_uv: [f64; 6],
}

/// Every `step`-th of a side's `cells` kept so that they, and `extra`
/// texels more, fit `max_side`: the step and the texels on that side.
fn texel_fit(cells: usize, max_side: u32, extra: usize) -> (usize, usize) {
    let step = cells.div_ceil((max_side as usize).saturating_sub(extra).max(1));
    (step, cells.div_ceil(step) + extra)
}

/// The map from world XY to texture coordinates for the steps and texel
/// counts `texel_fit` gave, the lattice origin `offset` texels in.
fn texel_world_to_uv(lattice: &Lattice, (column_step, width): (usize, usize), (row_step, height): (usize, usize), offset: f64) -> [f64; 6] {
    let along = |axis: DVec2, size: f64, step: usize, count: usize| {
        let (spacing, count) = (size * step as f64, count as f64);
        [
            axis.x / (spacing * count),
            axis.y / (spacing * count),
            (offset - lattice.origin.dot(axis) / spacing) / count,
        ]
    };
    let [a, b, c] = along(lattice.u, lattice.u_sizes[0], column_step, width);
    let [d, e, f] = along(lattice.v, lattice.v_sizes[0], row_step, height);
    [a, b, c, d, e, f]
}

/// `value` as a position from 0 to 1 across `(min, max)`, 0.5 where the
/// range is flat.
pub(crate) fn ramp_position(value: f64, (min, max): (f64, f64)) -> f32 {
    if (max - min).abs() <= HEIGHT_TOLERANCE {
        0.5
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
    }
}

fn mesh_vertex(position: DVec3) -> mesh_data::Vertex {
    mesh_data::Vertex::new(position.x, position.y, position.z)
}

// One whole cell into a total by the rules the reserve scan's `add` adds a
// block with: a value that is absent is missing; a weight absent, zero or
// negative is unusable; a negative value reads as zero.
fn add_summed(total: &mut NumericTotal, value: Option<f64>) {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        total.missing += 1;
        return;
    };
    if value < 0.0 {
        total.negatives += 1;
    } else {
        total.sum += value;
    }
    total.contributions += 1;
}

fn add_weighted(total: &mut NumericTotal, value: Option<f64>, weight: Option<f64>) {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        total.missing += 1;
        return;
    };
    let Some(weight) = weight.filter(|weight| weight.is_finite() && *weight > 0.0) else {
        total.unusable_weights += 1;
        return;
    };
    if value < 0.0 {
        total.negatives += 1;
    } else {
        total.sum += value * weight;
    }
    total.weight += weight;
    total.contributions += 1;
}

/// One active cell's box, measured as a block model's: its sides'
/// product, then the frame's stretch.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CellMeasure {
    pub(crate) cell: usize,
    sides: f64,
    stretch: f64,
}

impl CellMeasure {
    pub(crate) fn volume(self) -> f64 {
        self.sides * self.stretch
    }

    /// A `value` per cubic metre over the box, multiplied in
    /// `per_volume_values`' order.
    pub(crate) fn per_volume(self, value: f64) -> f64 {
        value * self.sides * self.stretch
    }
}

/// What a grid's quality means are weighted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Weighting {
    /// Cell tonnes: the grid has a density.
    ByTonnes,
    ByVolume,
}

/// One quality's mean over a grid's cells.
#[derive(Clone, Debug)]
pub(crate) struct QualityTotal {
    pub(crate) name: String,
    pub(crate) unit: String,
    pub(crate) total: NumericTotal,
}

/// One set of cell values' smallest and largest over the cells summed, and
/// how many of those cells have none.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ValueSpread {
    pub(crate) name: String,
    pub(crate) unit: String,
    pub(crate) range: Option<(f64, f64)>,
    pub(crate) missing: usize,
}

impl ValueSpread {
    fn take(&mut self, value: Option<f64>) {
        match value.filter(|value| value.is_finite()) {
            Some(value) => self.range = Some(self.range.map_or((value, value), |(min, max)| (min.min(value), max.max(value)))),
            None => self.missing += 1,
        }
    }
}

/// What a prismoid grid's row reports about it.
#[derive(Clone, Debug)]
pub(crate) struct PrismoidGridSummary {
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    pub(crate) active_cells: usize,
    /// Roof minus floor, smallest and largest; `None` with no active cell.
    pub(crate) vertical_thickness: Option<(f64, f64)>,
    /// Cubic metres in the active prismoids.
    pub(crate) volume: f64,
    /// Volume times density, summed; `None` without a density.
    pub(crate) tonnes: Option<NumericTotal>,
    /// The density's name, with `tonnes`.
    pub(crate) density: Option<String>,
    pub(crate) weighting: Weighting,
    /// Each quality's mean, density aside, in the grid's order.
    pub(crate) qualities: Vec<QualityTotal>,
    /// Every set of values, density included, in the grid's order.
    pub(crate) spreads: Vec<ValueSpread>,
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
    pub(crate) color_by: PrismoidColorBy,
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
    pub(crate) color_by: PrismoidColorBy,
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

/// How a quality surface's values reach a grid's cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellReading {
    /// The mean of the surface's heights at a prismoid's four corners: the
    /// grid's corners are nodes of the surface.
    Corners,
    /// Bilinear at the cell's centre, the surface being a grid on another lattice.
    Centre,
    /// Off the surface's triangles at the cell's centre: it is no grid.
    Triangles,
}

/// A quality surface as the cells read it.
pub(crate) enum QualitySurface {
    Grid(GridSurface),
    Triangles { mesh: Arc<mesh_data::Triangulation>, spatial: Arc<TriangleBvh> },
}

impl QualitySurface {
    /// A grid where `GridSurface::read` takes the surface, else its triangles.
    pub(crate) fn read(mesh: Arc<mesh_data::Triangulation>, spatial: Arc<TriangleBvh>) -> Self {
        match GridSurface::read(Arc::clone(&mesh), Arc::clone(&spatial)) {
            Ok(grid) => Self::Grid(grid),
            Err(_) => Self::Triangles { mesh, spatial },
        }
    }
}

/// One quality surface read into a grid's cells.
#[derive(Debug)]
pub(crate) struct ReadCells {
    pub(crate) reading: CellReading,
    /// One per cell, in cell order; `None` for inactive cells too.
    pub(crate) values: Vec<Option<f64>>,
    /// Active cells given a value, and active cells left without one.
    pub(crate) read: usize,
    pub(crate) missing: usize,
}

/// Values read from one quality surface, with the surface's name for words.
#[derive(Clone, Debug)]
pub(crate) struct AddedValues {
    pub(crate) surface: String,
    pub(crate) values: CellValues,
}

/// Values about to join a grid as the refusals that need no cells see them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ValuesMark<'a> {
    pub(crate) surface: &'a str,
    pub(crate) name: &'a str,
    pub(crate) unit: &'a str,
    pub(crate) role: CellValueRole,
}

/// Why values cannot join a grid. Names are surface names unless said;
/// quality names match as `same_quality_name` does.
#[derive(Debug)]
pub(crate) enum ValuesRefusal {
    /// Two surfaces give one quality (by `CellValues::name`).
    Twice { first: String, second: String, quality: String },
    /// A name the grid keeps for its own values: a derived block model
    /// column, the file's mask, or `density` for anything but the density.
    Reserved { surface: String, quality: String },
    /// The grid already holds a density (`held`: that quality's name) and another is added.
    TwoDensities { held: String, surface: String },
    /// A density in a unit naming kg.
    DensityInKg { surface: String, unit: String },
    /// No active cell got a value.
    NoCover { surface: String },
    /// The merged grid failed [`PrismoidGrid::check`].
    Malformed(anyhow::Error),
}

/// Names the derived block model and the file keep for their own columns.
const RESERVED_NAMES: [&str; 4] = ["volume", "vertical_thickness", "prismoid_grid", "active"];

/// Whether `value` is a whole number, zero included, within `NODE_TOLERANCE`.
fn is_whole_or_zero(value: f64) -> bool {
    (value - value.round()).abs() <= NODE_TOLERANCE
}

/// As [`is_whole_or_zero`], one or more.
fn is_whole(value: f64) -> bool {
    value.round() >= 1.0 && is_whole_or_zero(value)
}

impl PrismoidGrid {
    /// Reads `surface` into one value per cell, or `None` if cancelled.
    pub(crate) fn read_cells(&self, surface: &QualitySurface, cancel: &CancelFlag) -> Option<ReadCells> {
        match surface {
            QualitySurface::Triangles { mesh, spatial } => self.read_cells_triangles(mesh, spatial, cancel),
            QualitySurface::Grid(grid) if self.corners_are_nodes(grid) => self.read_cells_corners(grid, cancel),
            QualitySurface::Grid(grid) => self.read_cells_centre(grid, cancel),
        }
    }

    /// Whether the lattice is north-aligned and every corner can be a node of `grid`.
    fn corners_are_nodes(&self, grid: &GridSurface) -> bool {
        let lattice = &self.lattice;
        if (lattice.u, lattice.v) != (DVec2::X, DVec2::Y) {
            return false;
        }
        let spacing = grid.spacing();
        let offset = (lattice.origin - grid.lattice().0) / spacing;
        is_whole_or_zero(offset.x) && is_whole_or_zero(offset.y) && lattice.u_sizes.iter().chain(&lattice.v_sizes).all(|size| is_whole(size / spacing))
    }

    /// The plan position of every cell's centre, row by row.
    fn cell_centres(&self) -> impl Iterator<Item = (usize, DVec2)> + '_ {
        let lattice = &self.lattice;
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let columns = lattice.columns();
        (0..lattice.cell_count()).map(move |cell| {
            let (column, row) = (cell % columns, cell / columns);
            let along_u = u_edges[column] + lattice.u_sizes[column] / 2.0;
            let along_v = v_edges[row] + lattice.v_sizes[row] / 2.0;
            (cell, lattice.origin + lattice.u * along_u + lattice.v * along_v)
        })
    }

    /// Reads each active cell with `value`, checking `cancel` once per row.
    fn read_each(&self, reading: CellReading, cancel: &CancelFlag, mut value: impl FnMut(usize, DVec2) -> Option<f64>) -> Option<ReadCells> {
        let columns = self.lattice.columns();
        let mut values = vec![None; self.active.len()];
        let (mut read, mut missing) = (0, 0);
        for (cell, centre) in self.cell_centres() {
            if cell % columns == 0 && cancel.is_cancelled() {
                return None;
            }
            if !self.active[cell] {
                continue;
            }
            values[cell] = value(cell, centre);
            if values[cell].is_some() {
                read += 1;
            } else {
                missing += 1;
            }
        }
        Some(ReadCells { reading, values, read, missing })
    }

    fn read_cells_triangles(&self, mesh: &mesh_data::Triangulation, spatial: &TriangleBvh, cancel: &CancelFlag) -> Option<ReadCells> {
        let mut stack = Vec::new();
        self.read_each(CellReading::Triangles, cancel, |_, centre| surface_z_single(mesh, spatial, &mut stack, centre))
    }

    fn read_cells_centre(&self, grid: &GridSurface, cancel: &CancelFlag) -> Option<ReadCells> {
        self.read_each(CellReading::Centre, cancel, |_, centre| grid.bilinear(centre))
    }

    fn read_cells_corners(&self, grid: &GridSurface, cancel: &CancelFlag) -> Option<ReadCells> {
        let heights: Vec<Option<f64>> = self
            .lattice
            .corners()
            .into_iter()
            .map(|at| grid.node_at(at).and_then(|(column, row)| grid.height(column, row)))
            .collect();
        let columns = self.lattice.columns();
        self.read_each(CellReading::Corners, cancel, |cell, _| {
            let first = cell / columns * (columns + 1) + cell % columns;
            let corners = [heights[first], heights[first + 1], heights[first + columns + 1], heights[first + columns + 2]];
            corners.iter().try_fold(0.0, |sum, height| height.map(|height| sum + height)).map(|sum| sum / 4.0)
        })
    }

    /// A copy of the grid with `added` merged in: a quality the grid already
    /// holds is replaced in place, new ones follow in order.
    pub(crate) fn with_values(&self, added: Vec<AddedValues>) -> Result<PrismoidGrid, ValuesRefusal> {
        let held: Vec<_> = self.cell_values.iter().map(|values| (values.name.as_str(), values.role)).collect();
        let marks: Vec<_> = added
            .iter()
            .map(|one| ValuesMark {
                surface: &one.surface,
                name: &one.values.name,
                unit: &one.values.unit,
                role: one.values.role,
            })
            .collect();
        check_marks(&held, &marks)?;
        if let Some(one) = added
            .iter()
            .find(|one| !self.active.iter().zip(&one.values.values).any(|(active, value)| *active && value.is_some()))
        {
            return Err(ValuesRefusal::NoCover { surface: one.surface.clone() });
        }
        let mut merged = self.clone();
        for one in added {
            match merged.cell_values.iter_mut().find(|held| same_quality_name(&held.name, &one.values.name)) {
                Some(held) => *held = one.values,
                None => merged.cell_values.push(one.values),
            }
        }
        merged.check().map_err(ValuesRefusal::Malformed)?;
        Ok(merged)
    }
}

/// The refusal `added` meets against the grid's `held` names and roles
/// before any cell is read, so a dialog can say it before the work starts.
pub(crate) fn check_marks(held: &[(&str, CellValueRole)], added: &[ValuesMark<'_>]) -> Result<(), ValuesRefusal> {
    for one in added {
        let reserved = RESERVED_NAMES.iter().any(|name| same_quality_name(name, one.name));
        if reserved || (same_quality_name(one.name, "density") && one.role != CellValueRole::Density) {
            return Err(ValuesRefusal::Reserved {
                surface: one.surface.to_owned(),
                quality: one.name.to_owned(),
            });
        }
    }
    if let Some(one) = added.iter().find(|one| one.role == CellValueRole::Density && one.unit.to_ascii_lowercase().contains("kg")) {
        return Err(ValuesRefusal::DensityInKg {
            surface: one.surface.to_owned(),
            unit: one.unit.to_owned(),
        });
    }
    for (later, one) in added.iter().enumerate() {
        if let Some(earlier) = added[..later].iter().find(|earlier| same_quality_name(earlier.name, one.name)) {
            return Err(ValuesRefusal::Twice {
                first: earlier.surface.to_owned(),
                second: one.surface.to_owned(),
                quality: one.name.to_owned(),
            });
        }
    }
    // A held density stays unless one of `added` replaces it.
    let kept = held
        .iter()
        .filter(|(name, role)| *role == CellValueRole::Density && !added.iter().any(|one| same_quality_name(one.name, name)))
        .map(|(name, _)| *name);
    let mut densities = kept.chain(added.iter().filter(|one| one.role == CellValueRole::Density).map(|one| one.name));
    if let (Some(first), Some(_)) = (densities.next(), densities.next())
        && let Some(one) = added.iter().rev().find(|one| one.role == CellValueRole::Density)
    {
        return Err(ValuesRefusal::TwoDensities {
            held: first.to_owned(),
            surface: one.surface.to_owned(),
        });
    }
    Ok(())
}
