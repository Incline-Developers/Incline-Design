//! One economic value per grid cell.
//!
//! Each block is valued as already uncovered (Whittle's first rule) and sent
//! where it pays best: to waste, or to any processing method that takes its
//! rock and grade.
//!
//! ```text
//! waste       W   = -T * (mining + waste haulage + rehabilitation)
//! method m    O_m = RF * revenue_m - T * (mining + processing + G&A + ore haulage * factor_m) - element costs_m
//! block value V   = max(W, max_m O_m)
//! ```
//!
//! Revenue is never negative, so `V` never falls as the revenue factor rises:
//! shells at rising factors nest. Revenue and cost are kept per route rather
//! than as one value, because which route wins changes with the factor.

use anyhow::{Result, bail};
use rayon::prelude::*;

use super::{
    grid::{Grid, NO_BLOCK},
    prepare::{Air, Economics, code_index, finite_or_zero},
};
use crate::app::jobs::CancelFlag;

/// One way a cell can be processed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Route {
    pub(crate) method: u16,
    /// Revenue at revenue factor 1.
    pub(crate) revenue: f64,
    /// Every cost of mining and processing the block this way.
    pub(crate) cost: f64,
}

impl Route {
    pub(crate) fn value(&self, factor: f64) -> f64 {
        factor * self.revenue - self.cost
    }
}

/// Values of every cell of the grid.
#[derive(Clone, Debug, Default)]
pub(crate) struct BlockValues {
    /// The waste route's value; 0 for air.
    pub(crate) waste: Vec<f64>,
    pub(crate) tonnes: Vec<f64>,
    /// Air, or no block: never valued, never part of a shell's solid.
    pub(crate) air: Vec<bool>,
    /// `routes[offsets[c]..offsets[c + 1]]` are cell `c`'s processing routes.
    pub(crate) offsets: Vec<u32>,
    pub(crate) routes: Vec<Route>,
    /// Blocks whose density field was blank, zero or negative, so the default
    /// density stood in.
    pub(crate) blocks_with_default_density: usize,
}

impl BlockValues {
    pub(crate) fn routes_of(&self, cell: usize) -> &[Route] {
        &self.routes[self.offsets[cell] as usize..self.offsets[cell + 1] as usize]
    }

    pub(crate) fn value(&self, cell: usize, factor: f64) -> f64 {
        self.routes_of(cell).iter().fold(self.waste[cell], |best, route| best.max(route.value(factor)))
    }

    /// The method a cell goes to at `factor`, or `None` for waste.
    pub(crate) fn destination(&self, cell: usize, factor: f64) -> Option<u16> {
        let mut best = (self.waste[cell], None);
        for route in self.routes_of(cell) {
            let value = route.value(factor);
            if value > best.0 {
                best = (value, Some(route.method));
            }
        }
        best.1
    }
}

const CHUNK: usize = 1 << 16;

struct Chunk {
    waste: Vec<f64>,
    tonnes: Vec<f64>,
    air: Vec<bool>,
    counts: Vec<u32>,
    routes: Vec<Route>,
    default_density: usize,
}

/// Value every cell. `heights` are the topography's height per column, when
/// air is cut by a surface.
pub(crate) fn compute(economics: &Economics, grid: &Grid, block_of_cell: &[u32], heights: Option<&[f64]>, cancel: &CancelFlag) -> Result<BlockValues> {
    let volume = grid.cell.x * grid.cell.y * grid.cell.z;
    let chunks: Vec<Chunk> = block_of_cell
        .par_chunks(CHUNK)
        .enumerate()
        .map(|(chunk_index, blocks)| {
            let mut chunk = Chunk {
                waste: Vec::with_capacity(blocks.len()),
                tonnes: Vec::with_capacity(blocks.len()),
                air: Vec::with_capacity(blocks.len()),
                counts: Vec::with_capacity(blocks.len()),
                routes: Vec::new(),
                default_density: 0,
            };
            if cancel.is_cancelled() {
                return chunk;
            }
            for (offset, &block) in blocks.iter().enumerate() {
                let cell = chunk_index * CHUNK + offset;
                let before = chunk.routes.len();
                let (waste, tonnes, air) = if block == NO_BLOCK {
                    (0.0, 0.0, true)
                } else {
                    value_block(economics, grid, heights, volume, cell, block as usize, &mut chunk)
                };
                chunk.waste.push(waste);
                chunk.tonnes.push(tonnes);
                chunk.air.push(air);
                chunk.counts.push((chunk.routes.len() - before) as u32);
            }
            chunk
        })
        .collect();
    if cancel.is_cancelled() {
        bail!("Cancelled");
    }

    let cells = block_of_cell.len();
    let mut values = BlockValues {
        waste: Vec::with_capacity(cells),
        tonnes: Vec::with_capacity(cells),
        air: Vec::with_capacity(cells),
        offsets: Vec::with_capacity(cells + 1),
        routes: Vec::with_capacity(chunks.iter().map(|chunk| chunk.routes.len()).sum()),
        blocks_with_default_density: 0,
    };
    values.offsets.push(0);
    for chunk in chunks {
        values.waste.extend(chunk.waste);
        values.tonnes.extend(chunk.tonnes);
        values.air.extend(chunk.air);
        let mut total = *values.offsets.last().expect("starts with 0");
        for count in chunk.counts {
            total = total.checked_add(count).ok_or_else(|| anyhow::anyhow!("too many processing routes"))?;
            values.offsets.push(total);
        }
        values.routes.extend(chunk.routes);
        values.blocks_with_default_density += chunk.default_density;
    }
    Ok(values)
}

/// Value one block; pushes its processing routes. Returns (waste value, tonnes, is air).
fn value_block(economics: &Economics, grid: &Grid, heights: Option<&[f64]>, volume: f64, cell: usize, block: usize, chunk: &mut Chunk) -> (f64, f64, bool) {
    let rock = economics.rock.as_ref().map(|codes| codes[block]);
    let is_air = match &economics.air {
        Air::None => false,
        Air::Rock(air) => rock.and_then(|code| code_index(code, air.len())).is_some_and(|code| air[code]),
        Air::Topography(_) => heights.is_some_and(|heights| {
            let surface = heights[grid.column(cell)];
            surface.is_finite() && grid.centroid(cell).z > surface
        }),
    };
    if is_air {
        return (0.0, 0.0, true);
    }
    let density = match &economics.density {
        None => economics.default_density,
        Some(values) if values[block].is_finite() && values[block] > 0.0 => values[block],
        Some(_) => {
            chunk.default_density += 1;
            economics.default_density
        }
    };
    let tonnes = volume * density;
    let mining = tonnes * economics.mining_cost(block);
    let waste = -mining - tonnes * (economics.waste_haulage.at(block) + economics.rehab.at(block));

    let grade = economics.quality[block];
    for (index, method) in economics.methods.iter().enumerate() {
        if !method.takes(rock, grade) {
            continue;
        }
        let mut revenue = 0.0;
        let mut element_costs = 0.0;
        for element in &method.elements {
            let grade = finite_or_zero(element.values[block]);
            let contained = tonnes * grade * element.factor;
            let recoverable = tonnes * (grade - element.threshold).max(0.0) * element.factor;
            revenue += recoverable * element.recovery * element.net_price;
            element_costs += contained * element.cost;
        }
        let processing = tonnes * (method.per_tonne + economics.ore_haulage.at(block) * method.ore_haulage_factor);
        chunk.routes.push(Route {
            method: index as u16,
            revenue,
            cost: mining + processing + element_costs,
        });
    }
    (waste, tonnes, false)
}
