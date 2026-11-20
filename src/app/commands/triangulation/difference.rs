use std::collections::HashMap;

use anyhow::Result;
use rayon::prelude::*;

use super::cuts::{PreparedReferenceSurface, for_each_plan_overlap, reference_xy_overlap_area_tolerance};
use crate::{
    app::{
        App,
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        formats::mesh_data,
        progress::{Phase, Progress},
        triangulation::{GeneratedTriangulationLog, TriangulationId},
    },
    ui::state::TriSurfaceType,
    userspace_log,
};

/// Welding grid for output points in plan: a micrometre.
const WELD: f64 = 1.0e-6;

/// Heights at one plan point closer than this are one output point; further
/// apart they stay separate (a vertical step).
const STEP: f64 = 1.0e-3;

/// Height of the plane through `triangle` at (`x`, `y`).
fn plane_z(triangle: [mesh_data::Vertex; 3], x: f64, y: f64) -> f64 {
    let [a, b, c] = triangle;
    let (ux, uy, uz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    let (vx, vy, vz) = (c.x - a.x, c.y - a.y, c.z - a.z);
    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;
    a.z - (nx * (x - a.x) + ny * (y - a.y)) / nz
}

/// The height of `upper` above `lower` as a surface, exact on the inputs' own
/// triangles: each plan overlap of an upper and a lower triangle is convex
/// with both surfaces planar over it, so its difference is planar too. Where
/// `upper` is below `lower` the height is negative and kept.
pub(super) fn difference_mesh(
    upper: &PreparedReferenceSurface,
    lower: &PreparedReferenceSurface,
    cancel: &CancelFlag,
    progress: &Phase,
) -> Result<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> {
    let upright = reference_xy_overlap_area_tolerance(&lower.mesh);
    let task_count = rayon::current_num_threads().saturating_mul(4).max(1);
    let chunk_size = upper.triangles.len().div_ceil(task_count).max(1);
    let done = progress.counter(upper.triangles.len());
    let partials: Vec<Result<Vec<[[f64; 3]; 3]>>> = upper
        .triangles
        .par_chunks(chunk_size)
        .map(|chunk| {
            if cancel.is_cancelled() {
                anyhow::bail!(crate::app::jobs::Cancelled);
            }
            let mut pieces = Vec::new();
            let mut stack = Vec::new();
            let mut overlap = Vec::new();
            for &target in chunk {
                for_each_plan_overlap(target, &lower.mesh, &lower.spatial, upright, &mut stack, &mut overlap, |lower_triangle, polygon| {
                    let at = |index: usize| {
                        let point = polygon[index];
                        [point.x, point.y, plane_z(target, point.x, point.y) - plane_z(lower_triangle, point.x, point.y)]
                    };
                    for index in 1..polygon.len() - 1 {
                        let (a, b, c) = (at(0), at(index), at(index + 1));
                        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                        if area > 0.0 {
                            pieces.push([a, b, c]);
                        } else if area < 0.0 {
                            pieces.push([a, c, b]);
                        }
                    }
                });
            }
            done.advance_by(chunk.len());
            Ok(pieces)
        })
        .collect();

    let mut vertices: Vec<mesh_data::Vertex> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    let mut welded: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
    for partial in partials {
        for piece in partial? {
            let face = piece.map(|point| {
                let key = ((point[0] / WELD).round() as i64, (point[1] / WELD).round() as i64);
                let sharing = welded.entry(key).or_default();
                if let Some(&found) = sharing.iter().find(|&&index| (vertices[index as usize].z - point[2]).abs() <= STEP) {
                    return found;
                }
                vertices.push(mesh_data::Vertex {
                    x: point[0],
                    y: point[1],
                    z: point[2],
                });
                sharing.push((vertices.len() - 1) as u32);
                sharing[sharing.len() - 1]
            });
            if face[0] != face[1] && face[1] != face[2] && face[0] != face[2] {
                faces.push(face);
            }
        }
    }
    Ok((vertices, faces))
}

impl App<'_> {
    /// Open the dialog with the upper surface taken from the selection when
    /// it holds exactly one.
    pub(crate) fn open_difference_surfaces(&mut self) {
        self.editor.tri_difference_open = true;
        self.editor.tri_difference_upper_id = None;
        self.editor.tri_difference_lower_id = None;
        self.editor.tri_difference_name_auto = true;
        self.editor.tri_difference_name_input.clear();
        if let [id] = self.selected_triangulations()[..]
            && let Some(surface) = self.triangulations.iter().find(|surface| surface.id == id && surface.state.loaded)
        {
            self.editor.tri_difference_upper_id = Some(id);
            self.editor.tri_difference_name_input = crate::app::canvas::derived_triangulation_name(&surface.name, &tr!("cmd-commands-difference"));
        }
    }

    /// Make the height of `upper_id` above `lower_id` into a new surface
    /// whose z is the thickness, marked as a value and not an RL. The inputs
    /// stay as they are.
    pub(crate) fn difference_triangulations(&mut self, upper_id: TriangulationId, lower_id: TriangulationId, name: String) -> Result<()> {
        if upper_id == lower_id {
            anyhow::bail!(tr!("err-surf-difference-same"));
        }

        let snapshot = |app: &Self, id: TriangulationId| -> Result<(std::sync::Arc<mesh_data::Triangulation>, String)> {
            let item = app
                .triangulations
                .iter()
                .find(|triangulation| triangulation.id == id && triangulation.state.loaded)
                .ok_or_else(|| anyhow::anyhow!(tr!("err-surf-difference-not-found")))?;
            super::session::refuse_quality(&item.name, item.quality.as_ref())?;
            Ok((item.mesh.clone(), item.name.clone()))
        };
        let (upper_mesh, upper_name) = snapshot(self, upper_id)?;
        let (lower_mesh, lower_name) = snapshot(self, lower_id)?;
        let mark = crate::app::commands::prismoid_grid::surface_mark(None);
        let quantity = mark.name.clone();

        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<GeneratedTriangulationLog> {
            let folds = |surface: &str| anyhow::anyhow!(tr!("err-surf-difference-folds", surface = surface.to_owned()));
            let upper = super::cuts::validate_reference_surface(&upper_mesh).map_err(|_| folds(&upper_name))?;
            let lower = super::cuts::validate_reference_surface(&lower_mesh).map_err(|_| folds(&lower_name))?;
            if cancel.is_cancelled() {
                anyhow::bail!(crate::app::jobs::Cancelled);
            }
            let (vertices, faces) = difference_mesh(&upper, &lower, cancel, &progress.phase(0.1, 0.9))?;
            if cancel.is_cancelled() {
                anyhow::bail!(crate::app::jobs::Cancelled);
            }
            if faces.is_empty() {
                anyhow::bail!(tr!("err-surf-difference-no-overlap", upper = upper_name, lower = lower_name));
            }
            let (thinnest, thickest) = vertices
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), vertex| (low.min(vertex.z), high.max(vertex.z)));
            let generated = super::session::build_generated_triangulation(name, vertices, faces, TriSurfaceType::Surface, |_| Vec::new())?.with_quality(Some(mark));
            Ok(GeneratedTriangulationLog {
                generated,
                message: tr!(
                    "cmd-difference-surface-made",
                    upper = upper_name,
                    lower = lower_name,
                    min = format!("{thinnest:.2}"),
                    max = format!("{thickest:.2}")
                ),
            })
        };
        let apply = move |app: &mut App, result: Result<GeneratedTriangulationLog>| {
            let made = result.is_ok();
            app.apply_generated_triangulation_job(result, &[]);
            if made {
                userspace_log!("{}", tr!("cmd-prismoid-grid-surface-contours-are-values", quantity = quantity));
            }
        };
        self.spawn_job_reporting_progress(
            tr!("cmd-difference-surface-making"),
            vec![JobKey::Triangulation(upper_id), JobKey::Triangulation(lower_id)],
            compute,
            apply,
        );
        Ok(())
    }
}
