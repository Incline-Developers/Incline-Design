//! Running an optimization scenario (stage 2): from settings and a block
//! model to nested pit shells.
//!
//! [`prepare`] checks and resolves a scenario on the UI thread. [`run`] does
//! the rest on a worker: lays the blocks onto the optimizer's grid
//! ([`grid`]), values every cell ([`values`]), solves the shells with mineflow
//! ([`shells`]) and meshes them ([`mesh`]). See
//! `.claude/skills/optimization/SKILL.md` for the value formula and the shell
//! algorithm.

pub(crate) mod grid;
pub(crate) mod mesh;
pub(crate) mod prepare;
pub(crate) mod report;
pub(crate) mod shells;
pub(crate) mod values;

use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Result, bail};
use rayon::prelude::*;

use self::{
    grid::{BlockLayout, Grid},
    mesh::ShellMesh,
    prepare::{Air, Prepared},
    report::OptimizationReport,
    shells::Shells,
};
use crate::{
    app::jobs::CancelFlag,
    model::{optimization::ShellFieldValue, progress::Progress},
};

/// Everything a run needs, owned so it can go to a worker.
#[derive(Clone)]
pub(crate) struct RunInput {
    pub(crate) scenario_name: String,
    pub(crate) layer_name: String,
    pub(crate) prepared: Prepared,
    pub(crate) layout: BlockLayout,
    pub(crate) make_solids: bool,
    pub(crate) make_surfaces: bool,
    /// What to write per block into the block model, if anything.
    pub(crate) field_value: Option<ShellFieldValue>,
}

/// One shell's mesh, to become a triangulation.
pub(crate) struct ShellTriangulation {
    pub(crate) name: String,
    /// 1-based.
    pub(crate) shell: usize,
    pub(crate) solid: bool,
    pub(crate) mesh: ShellMesh,
}

/// What a run produced.
pub(crate) struct RunOutcome {
    pub(crate) grid: Grid,
    pub(crate) shells: Shells,
    pub(crate) meshes: Vec<ShellTriangulation>,
    /// The shell field to write into the block model (see [`block_field`]).
    pub(crate) field: Option<ShellField>,
    pub(crate) report: OptimizationReport,
    pub(crate) blocks_with_default_density: usize,
    pub(crate) blocks_with_default_angle: usize,
}

/// A categorical block model field of shells: per block a category code (the
/// shell's 1-based number, `NaN` blank) and each code's name.
#[derive(Clone)]
pub(crate) struct ShellField {
    pub(crate) codes: Arc<Vec<f64>>,
    pub(crate) categories: BTreeMap<u32, String>,
}

/// What a finished run keeps for later: the grid and every cell's shell.
#[derive(Debug)]
pub(crate) struct RunResult {
    #[allow(dead_code, reason = "kept for later reports (bench by bench, scheduling)")]
    pub(crate) grid: Grid,
    #[allow(dead_code, reason = "kept for later reports (bench by bench, scheduling)")]
    pub(crate) shells: Shells,
    /// Per shell and destination, at the base price; the Results view reads it.
    pub(crate) report: std::sync::Arc<OptimizationReport>,
}

/// The name of a shell's triangulation: `<scenario> <layer>_RAF 0.85`, with
/// ` DIR 50%` after it for a directional shell (its share of the distance).
pub(crate) fn shell_name(scenario: &str, layer: &str, factor: f64, distance: Option<f64>) -> String {
    let factor = crate::model::optimization::format_factor(factor);
    match distance {
        None => format!("{scenario} {layer}_RAF {factor}"),
        Some(share) => format!("{scenario} {layer}_RAF {factor} DIR {:.0}%", share * 100.0),
    }
}

pub(crate) fn run(input: RunInput, cancel: &CancelFlag, progress: &Progress) -> Result<RunOutcome> {
    let RunInput {
        scenario_name,
        layer_name,
        prepared,
        layout,
        make_solids,
        make_surfaces,
        field_value,
    } = input;
    let grid = layout.grid;
    for warning in &prepared.warnings {
        crate::userspace_warn!("{warning}");
    }

    let block_of_cell = layout.block_of_cell(cancel)?;
    progress.set_fraction(0.05);
    let heights = match &prepared.economics.air {
        Air::Topography(surface) => Some(surface.column_heights(&grid)),
        _ => None,
    };
    if cancel.is_cancelled() {
        bail!("Cancelled");
    }
    progress.set_fraction(0.1);
    let values = values::compute(&prepared.economics, &grid, &block_of_cell, heights.as_deref(), cancel)?;
    progress.set_fraction(0.15);
    let slopes = shells::slopes(&grid, &prepared.slope, &block_of_cell)?;
    progress.set_fraction(0.2);

    let shells = shells::solve(&prepared.plan, &grid, &values, &slopes, cancel, &progress.phase(0.2, 0.85))?;
    let field = field_value.map(|kind| block_field(&shells, &values.air, &block_of_cell, layout.blocks.len(), kind));
    let report = report::build(&scenario_name, &prepared.economics, &grid, &block_of_cell, &values, &shells, cancel)?;
    drop(block_of_cell);

    // One mesh per shell and kind, built side by side.
    let wanted: Vec<(usize, bool)> = shells
        .summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| summary.blocks > 0)
        .flat_map(|(index, _)| [(index + 1, true), (index + 1, false)])
        .filter(|&(_, solid)| if solid { make_solids } else { make_surfaces })
        .collect();
    if cancel.is_cancelled() {
        bail!("Cancelled");
    }
    let both = make_solids && make_surfaces;
    let meshes = wanted
        .into_par_iter()
        .map(|(shell, solid)| {
            let summary = &shells.summaries[shell - 1];
            let name = shell_name(&scenario_name, &layer_name, summary.factor, summary.distance);
            let (name, mesh) = if solid {
                (name, mesh::solid(&grid, &shells.label, &values.air, shell as u16))
            } else {
                let name = if both {
                    format!("{name} {}", crate::i18n::tr!("opt-shell-surface-suffix"))
                } else {
                    name
                };
                (name, mesh::surface(&grid, &shells.label, &values.air, shell as u16))
            };
            ShellTriangulation { name, shell, solid, mesh }
        })
        .collect();
    progress.set_fraction(1.0);

    Ok(RunOutcome {
        grid,
        blocks_with_default_density: values.blocks_with_default_density,
        blocks_with_default_angle: slopes.blocks_with_default_angle,
        shells,
        meshes,
        field,
        report,
    })
}

/// Per block, the innermost shell holding it - what writing the outer shell
/// first and each smaller one over it would leave - as a category: the code is
/// the shell's 1-based number, the name that number or the shell's revenue
/// factor. Categories, not numbers, so each shell takes its own colour without
/// a ramp to set up. Blocks in no shell, and air, are blank (`NaN`), so a rerun
/// leaves nothing of an earlier one behind; only shells holding a block are named.
fn block_field(shells: &Shells, air: &[bool], block_of_cell: &[u32], block_count: usize, kind: ShellFieldValue) -> ShellField {
    let mut codes = vec![f64::NAN; block_count];
    let mut categories = BTreeMap::new();
    for (cell, &block) in block_of_cell.iter().enumerate() {
        let label = shells.label[cell];
        if block == grid::NO_BLOCK || label == shells::OUTSIDE || air[cell] {
            continue;
        }
        codes[block as usize] = f64::from(label);
        categories.entry(u32::from(label)).or_insert_with(|| match kind {
            ShellFieldValue::Sequence => label.to_string(),
            ShellFieldValue::Factor => crate::model::optimization::format_factor(shells.summaries[usize::from(label) - 1].factor),
        });
    }
    ShellField {
        codes: Arc::new(codes),
        categories,
    }
}
