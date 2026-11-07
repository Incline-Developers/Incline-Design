//! The panel table: what each shell holds on each bench, in parcels.
//!
//! A parcel is waste, or one method's ore in one grade bin. A method's bins
//! are split at the min grades of the stockpiles feeding it (main quality
//! field), so a parcel either has stockpiles that may take it or none. Each
//! parcel keeps its sums as a run of numbers ([`Slot`]), so panels add,
//! scale and split as plain vectors.
//!
//! A bench is one block layer of the grid, counted from the top: bench 0 is
//! the grid's highest layer.

use anyhow::{Result, bail};
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    model::{
        npv_schedule::ResolvedStockpile,
        optimization_run::{
            grid::{Grid, NO_BLOCK},
            prepare::{Economics, finite_or_zero},
            shells::{OUTSIDE, Shells},
            values::{BlockValues, block_base, route_costs},
        },
    },
};

/// The numbers kept per parcel, in this order, then per element its
/// [`ELEMENT_GRADE_TONNES`] and [`ELEMENT_RECOVERED`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Tonnes = 0,
    /// Paid when mined: mining, and waste haulage and rehabilitation (waste)
    /// or ore haulage (ore).
    MineCost = 1,
    /// Paid when processed: processing, G&A and element costs.
    ProcessCost = 2,
    /// Earned when processed, net of selling costs.
    Revenue = 3,
}

const LEAD_SLOTS: usize = 4;
/// Offsets within an element's pair of slots.
pub(crate) const ELEMENT_GRADE_TONNES: usize = 0;
pub(crate) const ELEMENT_RECOVERED: usize = 1;

/// One parcel of a panel.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Parcel {
    /// 0 is waste; otherwise the report's destination (a method).
    pub(crate) destination: usize,
    /// Stockpiles that may take it, highest min grade first. Empty for waste
    /// and for ore below every min grade of the stockpiles feeding its method.
    pub(crate) stockpiles: Vec<usize>,
}

/// How a panel's numbers are laid out.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub(crate) elements: usize,
    /// Numbers per parcel.
    pub(crate) stride: usize,
    pub(crate) parcels: Vec<Parcel>,
    /// Per destination, its first parcel; its bins follow.
    first_parcel: Vec<usize>,
    /// Per destination, the min grades its bins are split at, rising.
    thresholds: Vec<Vec<f64>>,
    /// Per method row of the economics, its destination.
    method_destination: Vec<usize>,
}

impl Layout {
    /// The parcels for `destinations` (waste first, as
    /// `report::destinations` gives them with `method_destination`) and the
    /// stockpiles feeding them by name.
    pub(crate) fn new(destinations: &[String], method_destination: Vec<usize>, elements: usize, stockpiles: &[ResolvedStockpile]) -> Self {
        let mut parcels = vec![Parcel {
            destination: 0,
            stockpiles: Vec::new(),
        }];
        let mut first_parcel = vec![0];
        let mut thresholds = vec![Vec::new()];
        for (destination, name) in destinations.iter().enumerate().skip(1) {
            let feeding: Vec<usize> = (0..stockpiles.len()).filter(|&index| stockpiles[index].feeds.contains(name)).collect();
            let mut cuts: Vec<f64> = feeding.iter().map(|&index| stockpiles[index].min_grade).collect();
            cuts.sort_by(f64::total_cmp);
            cuts.dedup();
            first_parcel.push(parcels.len());
            // Bin 0 is below every cut; bin b is at or above cut b - 1.
            for bin in 0..=cuts.len() {
                let mut takers: Vec<usize> = match bin {
                    0 => Vec::new(),
                    _ => feeding.iter().copied().filter(|&index| stockpiles[index].min_grade <= cuts[bin - 1]).collect(),
                };
                takers.sort_by(|&a, &b| stockpiles[b].min_grade.total_cmp(&stockpiles[a].min_grade).then(a.cmp(&b)));
                parcels.push(Parcel { destination, stockpiles: takers });
            }
            thresholds.push(cuts);
        }
        Self {
            elements,
            stride: LEAD_SLOTS + 2 * elements,
            parcels,
            first_parcel,
            thresholds,
            method_destination,
        }
    }

    pub(crate) fn panel_len(&self) -> usize {
        self.parcels.len() * self.stride
    }

    /// The parcel ore of `destination` at `grade` belongs to.
    pub(crate) fn parcel_of(&self, destination: usize, grade: f64) -> usize {
        if destination == 0 {
            return 0;
        }
        let bin = self.thresholds[destination].iter().filter(|&&cut| cut <= grade).count();
        self.first_parcel[destination] + bin
    }

    /// Index of `slot` of `parcel`.
    pub(crate) fn at(&self, parcel: usize, slot: Slot) -> usize {
        parcel * self.stride + slot as usize
    }

    /// Index of element `element`'s number `offset` of `parcel`.
    pub(crate) fn element_at(&self, parcel: usize, element: usize, offset: usize) -> usize {
        parcel * self.stride + LEAD_SLOTS + 2 * element + offset
    }

    /// Tonnes of every parcel of a panel together.
    pub(crate) fn tonnes(&self, panel: &[f64]) -> f64 {
        (0..self.parcels.len()).map(|parcel| panel[self.at(parcel, Slot::Tonnes)]).sum()
    }
}

/// A pushback's (or shell's) panels: per bench that holds anything, its
/// numbers, in bench order.
pub(crate) type Panels = Vec<(u32, Vec<f64>)>;

/// `into += other`, bench by bench.
pub(crate) fn add_panels(into: &mut Panels, other: &Panels) {
    let mut merged = Vec::with_capacity(into.len().max(other.len()));
    let mut left = std::mem::take(into).into_iter().peekable();
    let mut right = other.iter().peekable();
    loop {
        match (left.peek(), right.peek()) {
            (Some((a, _)), Some((b, _))) if a == b => {
                let (bench, mut values) = left.next().expect("peeked");
                let (_, more) = right.next().expect("peeked");
                values.iter_mut().zip(more).for_each(|(value, more)| *value += more);
                merged.push((bench, values));
            }
            (Some((a, _)), Some((b, _))) if a < b => merged.push(left.next().expect("peeked")),
            (Some(_), Some(_)) | (None, Some(_)) => merged.push(right.next().expect("peeked").clone()),
            (Some(_), None) => merged.push(left.next().expect("peeked")),
            (None, None) => break,
        }
    }
    *into = merged;
}

/// Every shell's panels, `[shell]` 0-based. One pass over the grid, a block
/// layer at a time; each block is sent where it pays best at the base price,
/// as in the run's report.
pub(crate) fn build(economics: &Economics, grid: &Grid, block_of_cell: &[u32], values: &BlockValues, shells: &Shells, layout: &Layout, cancel: &CancelFlag) -> Result<Vec<Panels>> {
    let volume = grid.cell.x * grid.cell.y * grid.cell.z;
    let layer = grid.column_count();
    let top = grid.dims[2] - 1;
    let shell_count = shells.summaries.len();
    let layers: Vec<Vec<(usize, Vec<f64>)>> = (0..grid.dims[2])
        .into_par_iter()
        .map(|z| {
            if cancel.is_cancelled() {
                return Vec::new();
            }
            let mut by_shell: Vec<Option<Vec<f64>>> = vec![None; shell_count];
            for cell in z * layer..(z + 1) * layer {
                let shell = shells.label[cell];
                let block = block_of_cell[cell];
                if shell == OUTSIDE || block == NO_BLOCK || values.air[cell] {
                    continue;
                }
                let block = block as usize;
                let panel = by_shell[usize::from(shell) - 1].get_or_insert_with(|| vec![0.0; layout.panel_len()]);
                let base = block_base(economics, volume, block);
                let method = values.destination(cell, 1.0).map(usize::from);
                let destination = method.map_or(0, |method| layout.method_destination[method]);
                let parcel = layout.parcel_of(destination, finite_or_zero(economics.quality.get(block)));
                panel[layout.at(parcel, Slot::Tonnes)] += base.tonnes;
                for (index, element) in economics.elements.iter().enumerate() {
                    panel[layout.element_at(parcel, index, ELEMENT_GRADE_TONNES)] += base.tonnes * finite_or_zero(element.values.get(block));
                }
                match method {
                    None => panel[layout.at(parcel, Slot::MineCost)] += base.mining + base.waste_haulage + base.rehab,
                    Some(method) => {
                        let mut element_cost = 0.0;
                        let costs = route_costs(economics, method, block, &base, |index, take| {
                            panel[layout.element_at(parcel, index, ELEMENT_RECOVERED)] += take.recovered;
                            element_cost += take.cost;
                        })
                        .expect("the destination takes the block");
                        panel[layout.at(parcel, Slot::MineCost)] += base.mining + costs.ore_haulage;
                        panel[layout.at(parcel, Slot::ProcessCost)] += costs.processing + costs.ga + element_cost;
                        panel[layout.at(parcel, Slot::Revenue)] += costs.revenue;
                    }
                }
            }
            by_shell.into_iter().enumerate().filter_map(|(shell, panel)| panel.map(|panel| (shell, panel))).collect()
        })
        .collect();
    if cancel.is_cancelled() {
        bail!("Cancelled");
    }
    // Benches count down from the top layer.
    let mut panels: Vec<Panels> = vec![Vec::new(); shell_count];
    for (z, layer) in layers.into_iter().enumerate().rev() {
        for (shell, panel) in layer {
            panels[shell].push(((top - z) as u32, panel));
        }
    }
    Ok(panels)
}
