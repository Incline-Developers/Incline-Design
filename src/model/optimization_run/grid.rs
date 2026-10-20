//! The block model laid onto the optimizer's dense grid.
//!
//! mineflow wants a value for every cell of a regular grid, indexed x first,
//! then y, then z upward. A block model may be stored in another order (voxel
//! tiling) or hold only some cells (a sparse CSV), so each block is placed by
//! its bounds, and cells no block covers are air.

use std::sync::Arc;

use anyhow::{Result, bail};
use glam::DVec3;
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    model::{
        block_model::{BlockBoundsSource, UniformBlockGrid},
        formats::mesh_data,
        spatial::TriangleBvh,
    },
};

/// No block sits in this cell.
pub(crate) const NO_BLOCK: u32 = u32::MAX;

/// The optimizer's grid, in world coordinates (the block model is unrotated).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Grid {
    pub(crate) dims: [usize; 3],
    /// World position of the lower corner of cell (0, 0, 0).
    pub(crate) origin: DVec3,
    pub(crate) cell: DVec3,
}

impl Grid {
    pub(crate) fn cell_count(&self) -> usize {
        self.dims[0] * self.dims[1] * self.dims[2]
    }

    pub(crate) fn column_count(&self) -> usize {
        self.dims[0] * self.dims[1]
    }

    pub(crate) fn index(&self, x: usize, y: usize, z: usize) -> usize {
        x + self.dims[0] * (y + self.dims[1] * z)
    }

    pub(crate) fn xyz(&self, index: usize) -> [usize; 3] {
        let layer = self.dims[0] * self.dims[1];
        [index % self.dims[0], (index % layer) / self.dims[0], index / layer]
    }

    /// The column (x + y * nx) a cell stands in.
    pub(crate) fn column(&self, index: usize) -> usize {
        index % (self.dims[0] * self.dims[1])
    }

    pub(crate) fn centroid(&self, index: usize) -> DVec3 {
        let [x, y, z] = self.xyz(index);
        self.origin + self.cell * DVec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5)
    }

    /// World position of grid corner (x, y, z).
    pub(crate) fn corner(&self, x: usize, y: usize, z: usize) -> DVec3 {
        self.origin + self.cell * DVec3::new(x as f64, y as f64, z as f64)
    }

    pub(crate) fn mineflow(&self) -> mineflow::BlockModel {
        mineflow::BlockModel::new(self.dims[0] as i64, self.dims[1] as i64, self.dims[2] as i64)
            .with_block_size(self.cell.x, self.cell.y, self.cell.z)
            .with_origin(self.origin.x, self.origin.y, self.origin.z)
    }
}

/// What a run needs of the block model's geometry, owned so it can go to a worker.
#[derive(Clone)]
pub(crate) struct BlockLayout {
    pub(crate) blocks: Arc<BlockBoundsSource>,
    pub(crate) uniform: UniformBlockGrid,
    pub(crate) grid: Grid,
}

impl BlockLayout {
    /// `local_origin_to_world` turns the uniform grid's local origin into a world
    /// position. `None` when the model has no blocks to take a cell size from.
    pub(crate) fn new(blocks: Arc<BlockBoundsSource>, uniform: UniformBlockGrid, local_to_world: impl Fn(DVec3) -> DVec3) -> Option<Self> {
        let first = blocks.get(0)?;
        let cell = first.upper - first.lower;
        if !cell.is_finite() || cell.min_element() <= 0.0 || u32::try_from(uniform.cell_count()).is_err() {
            return None;
        }
        let grid = Grid {
            dims: uniform.dims,
            origin: local_to_world(uniform.origin),
            cell,
        };
        Some(Self { blocks, uniform, grid })
    }

    /// For every cell, the block in it, or [`NO_BLOCK`].
    pub(crate) fn block_of_cell(&self, cancel: &CancelFlag) -> Result<Vec<u32>> {
        let count = self.blocks.len();
        let cells: Vec<Option<u32>> = (0..count)
            .into_par_iter()
            .map(|block| {
                let bounds = self.blocks.get(block)?;
                let coords = self.uniform.cell_coords(bounds.lower)?;
                Some(self.uniform.linear_index(coords) as u32)
            })
            .collect();
        if cancel.is_cancelled() {
            bail!("Cancelled");
        }
        let mut block_of_cell = vec![NO_BLOCK; self.grid.cell_count()];
        for (block, cell) in cells.into_iter().enumerate() {
            if let Some(cell) = cell {
                block_of_cell[cell as usize] = block as u32;
            }
        }
        Ok(block_of_cell)
    }
}

/// A topography surface, for telling air from rock.
#[derive(Clone)]
pub(crate) struct Topography {
    pub(crate) mesh: Arc<mesh_data::Triangulation>,
    pub(crate) spatial: Arc<TriangleBvh>,
}

impl Topography {
    /// The surface's height over each grid column's centre; `NaN` where the
    /// column is off the surface. One ray per column, not per block.
    pub(crate) fn column_heights(&self, grid: &Grid) -> Vec<f64> {
        let bounds = self.mesh.bounds();
        let top = bounds.max.z + ((bounds.max.z - bounds.min.z).abs() * 1.0e-6).max(1.0);
        (0..grid.column_count())
            .into_par_iter()
            .map(|column| {
                let centre = grid.centroid(column);
                if centre.x < bounds.min.x || centre.x > bounds.max.x || centre.y < bounds.min.y || centre.y > bounds.max.y {
                    return f64::NAN;
                }
                self.spatial
                    .ray_hit(&self.mesh, DVec3::new(centre.x, centre.y, top), -DVec3::Z)
                    .map_or(f64::NAN, |hit| hit.z)
            })
            .collect()
    }
}
