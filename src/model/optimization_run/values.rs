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

/// What mining a block costs, whichever way it goes.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BlockBase {
    pub(crate) tonnes: f64,
    pub(crate) mining: f64,
    /// Waste haulage and rehabilitation: paid only when the block goes to waste.
    pub(crate) waste_haulage: f64,
    pub(crate) rehab: f64,
    /// The default density stood in for a blank, zero or negative one.
    pub(crate) default_density: bool,
}

impl BlockBase {
    /// The waste route's value.
    pub(crate) fn waste_value(&self) -> f64 {
        -self.mining - (self.waste_haulage + self.rehab)
    }
}

/// What processing a block one way costs and earns at revenue factor 1.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RouteCosts {
    pub(crate) ore_haulage: f64,
    pub(crate) processing: f64,
    pub(crate) ga: f64,
    pub(crate) revenue: f64,
    /// Every cost of mining and processing the block this way.
    pub(crate) cost: f64,
}

/// One element's share of a route.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ElementTake {
    /// Sales units recovered.
    pub(crate) recovered: f64,
    /// At revenue factor 1, net of selling costs.
    pub(crate) revenue: f64,
    pub(crate) cost: f64,
}

/// The tonnes and the mining, waste haulage and rehabilitation costs of a
/// (non-air) block. The one place block costs are worked out: the solver's
/// values and the reports both come from here and [`route_costs`].
pub(crate) fn block_base(economics: &Economics, volume: f64, block: usize) -> BlockBase {
    let (density, default_density) = match &economics.density {
        None => (economics.default_density, false),
        Some(values) if values[block].is_finite() && values[block] > 0.0 => (values[block], false),
        Some(_) => (economics.default_density, true),
    };
    let tonnes = volume * density;
    BlockBase {
        tonnes,
        mining: tonnes * economics.mining_cost(block),
        waste_haulage: tonnes * economics.waste_haulage.at(block),
        rehab: tonnes * economics.rehab.at(block),
        default_density,
    }
}

/// What processing method `method` makes of a block, or `None` when the
/// method does not take it. `take` hears each of the method's elements (its
/// index in [`Economics::elements`]) and its share.
pub(crate) fn route_costs(economics: &Economics, method: usize, block: usize, base: &BlockBase, mut take: impl FnMut(usize, ElementTake)) -> Option<RouteCosts> {
    let rock = economics.rock.as_ref().map(|codes| codes[block]);
    let method = &economics.methods[method];
    if !method.takes(rock, economics.quality[block]) {
        return None;
    }
    let tonnes = base.tonnes;
    let mut revenue = 0.0;
    let mut element_cost = 0.0;
    for element in &method.elements {
        let grade = finite_or_zero(element.values[block]);
        let contained = tonnes * grade * element.factor;
        let recoverable = tonnes * (grade - element.threshold).max(0.0) * element.factor;
        let recovered = recoverable * element.recovery;
        let share = ElementTake {
            recovered,
            revenue: recovered * element.net_price,
            cost: contained * element.cost,
        };
        revenue += share.revenue;
        element_cost += share.cost;
        take(element.index, share);
    }
    let ore_haulage = tonnes * economics.ore_haulage.at(block) * method.ore_haulage_factor;
    let processing_and_ga = tonnes * (method.per_tonne + economics.ore_haulage.at(block) * method.ore_haulage_factor);
    Some(RouteCosts {
        ore_haulage,
        processing: tonnes * method.processing,
        ga: tonnes * method.ga,
        revenue,
        cost: base.mining + processing_and_ga + element_cost,
    })
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
    let base = block_base(economics, volume, block);
    if base.default_density {
        chunk.default_density += 1;
    }
    for method in 0..economics.methods.len() {
        if let Some(costs) = route_costs(economics, method, block, &base, |_, _| {}) {
            chunk.routes.push(Route {
                method: method as u16,
                revenue: costs.revenue,
                cost: costs.cost,
            });
        }
    }
    (base.waste_value(), base.tonnes, false)
}
