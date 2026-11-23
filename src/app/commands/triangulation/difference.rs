use std::collections::HashMap;

use anyhow::Result;
use rayon::prelude::*;

use super::cuts::{PreparedReferenceSurface, for_each_plan_overlap_about, reference_xy_overlap_area_tolerance};
use crate::{
    app::{
        App,
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        formats::mesh_data,
        geometry::triangle_xy_area,
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

/// Output points as they are welded: the points kept, in plan about `origin`,
/// and the cells they sit in.
struct OutputPoints {
    origin: glam::DVec2,
    vertices: Vec<mesh_data::Vertex>,
    plan: Vec<[f64; 2]>,
    cells: HashMap<(i64, i64), Vec<u32>>,
}

impl OutputPoints {
    /// None yet, for points given about `origin`.
    fn new(origin: glam::DVec2) -> Self {
        Self {
            origin,
            vertices: Vec::new(),
            plan: Vec::new(),
            cells: HashMap::new(),
        }
    }

    /// The cell of one coordinate; cells are two welds wide, so a point's
    /// reach covers at most two per axis.
    fn cell(value: f64) -> i64 {
        (value / (2.0 * WELD)).floor() as i64
    }

    /// The index of the kept point within a weld of `point` (given about the
    /// origin) in plan and a step in height, or else of `point` kept anew.
    fn index(&mut self, point: [f64; 3]) -> u32 {
        let (xs, ys) = (
            Self::cell(point[0] - WELD)..=Self::cell(point[0] + WELD),
            Self::cell(point[1] - WELD)..=Self::cell(point[1] + WELD),
        );
        for key in xs.flat_map(|x| ys.clone().map(move |y| (x, y))) {
            let near = self.cells.get(&key).and_then(|sharing| {
                sharing.iter().copied().find(|&index| {
                    let at = self.plan[index as usize];
                    (at[0] - point[0]).abs() <= WELD && (at[1] - point[1]).abs() <= WELD && (self.vertices[index as usize].z - point[2]).abs() <= STEP
                })
            });
            if let Some(found) = near {
                return found;
            }
        }
        self.vertices.push(mesh_data::Vertex {
            x: point[0] + self.origin.x,
            y: point[1] + self.origin.y,
            z: point[2],
        });
        self.plan.push([point[0], point[1]]);
        let index = (self.vertices.len() - 1) as u32;
        self.cells.entry((Self::cell(point[0]), Self::cell(point[1]))).or_default().push(index);
        index
    }
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
    // Far from the origin the last bits of a coordinate are worth nanometres,
    // enough for two copies of one crossing point to weld apart, so triangles
    // are taken about the middle of the overlap as they are read (no surface
    // is copied) and the result moved back.
    let (above, below) = (upper.mesh.bounds(), lower.mesh.bounds());
    let origin = glam::DVec2::new(
        (above.min.x.max(below.min.x) + above.max.x.min(below.max.x)) / 2.0,
        (above.min.y.max(below.min.y) + above.max.y.min(below.max.y)) / 2.0,
    );
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
                let local = target.map(|vertex| mesh_data::Vertex::new(vertex.x - origin.x, vertex.y - origin.y, vertex.z));
                let plan = local.map(|vertex| glam::DVec2::new(vertex.x, vertex.y));
                for_each_plan_overlap_about(origin, plan, &lower.mesh, &lower.spatial, upright, &mut stack, &mut overlap, |lower_triangle, polygon| {
                    let at = |index: usize| {
                        let point = polygon[index];
                        [point.x, point.y, plane_z(local, point.x, point.y) - plane_z(lower_triangle, point.x, point.y)]
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

    let mut weld = OutputPoints::new(origin);
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for partial in partials {
        if cancel.is_cancelled() {
            anyhow::bail!(crate::app::jobs::Cancelled);
        }
        for piece in partial? {
            let face = piece.map(|point| weld.index(point));
            if face[0] != face[1] && face[1] != face[2] && face[0] != face[2] {
                faces.push(face);
            }
        }
    }
    Ok((weld.vertices, faces))
}

/// `mesh` ready to measure, or why it cannot be: no face with area in plan,
/// or a fold.
fn measured(mesh: &mesh_data::Triangulation, surface: &str) -> Result<PreparedReferenceSurface> {
    let tolerance = reference_xy_overlap_area_tolerance(mesh);
    if !mesh
        .face_indices()
        .any(|face| triangle_xy_area(face.map(|index| mesh.vertices()[index as usize])).abs() > tolerance)
    {
        anyhow::bail!(tr!("err-surf-difference-no-faces", surface = surface.to_owned()));
    }
    super::cuts::validate_reference_surface(mesh).map_err(|_| anyhow::anyhow!(tr!("err-surf-difference-folds", surface = surface.to_owned())))
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
            self.editor.tri_difference_name_input = crate::app::canvas::derived_surface_name(&surface.name, &tr!("cmd-commands-difference"));
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
            let upper = measured(&upper_mesh, &upper_name)?;
            let lower = measured(&lower_mesh, &lower_name)?;
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
            // Over the points the faces use: a piece too thin to keep leaves
            // its points behind.
            let (thinnest, thickest) = faces
                .iter()
                .flatten()
                .map(|&index| vertices[index as usize].z)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), z| (low.min(z), high.max(z)));
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
