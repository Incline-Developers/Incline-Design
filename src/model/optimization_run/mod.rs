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
pub(crate) mod shells;
pub(crate) mod values;

use anyhow::{Result, bail};
use rayon::prelude::*;

use self::{
    grid::{BlockLayout, Grid},
    mesh::ShellMesh,
    prepare::{Air, Prepared},
    shells::Shells,
};
use crate::{app::jobs::CancelFlag, model::progress::Progress};

/// Everything a run needs, owned so it can go to a worker.
#[derive(Clone)]
pub(crate) struct RunInput {
    pub(crate) scenario_name: String,
    pub(crate) layer_name: String,
    pub(crate) prepared: Prepared,
    pub(crate) layout: BlockLayout,
    pub(crate) make_solids: bool,
    pub(crate) make_surfaces: bool,
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
    pub(crate) blocks_with_default_density: usize,
    pub(crate) blocks_with_default_angle: usize,
}

/// What a finished run keeps for later: the grid and every cell's shell.
#[derive(Debug)]
#[allow(dead_code, reason = "read back by stage 3: reports and the shell number field")]
pub(crate) struct RunResult {
    pub(crate) grid: Grid,
    pub(crate) shells: Shells,
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
    drop(block_of_cell);
    progress.set_fraction(0.2);

    let shells = shells::solve(&prepared.plan, &grid, &values, &slopes, cancel, &progress.phase(0.2, 0.85))?;

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
    })
}
