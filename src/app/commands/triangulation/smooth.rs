use super::{include::topology_boundary_edges, *};
use crate::{
    app::jobs::CancelFlag,
    model::{progress::Progress, triangulation::unique_edges},
};

/// The welded vertices and faces, how many heights moved and how many points
/// were held.
pub(super) type SmoothedSurface = (Vec<mesh_data::Vertex>, Vec<[u32; 3]>, usize, usize);

/// Merge points that coincide, to within a weld as Clean Surface takes it,
/// and remap the faces onto them, dropping any face left with two corners on
/// one point. Points no face uses are left out.
fn weld_points(mesh: &mesh_data::Triangulation) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    let (points, remap) = super::clean::welded_points(mesh.vertices());
    let faces: Vec<[u32; 3]> = mesh
        .face_indices()
        .map(|face| face.map(|index| remap[index as usize]))
        .filter(|face| face[0] != face[1] && face[1] != face[2] && face[0] != face[2])
        .collect();
    super::clean::used_points(&points, faces)
}

// Taubin's two-step pass: a shrinking step then a slightly larger swelling
// one, so the surface is smoothed without shrinking (Taubin 1995, "A signal
// processing approach to fair surface design").
const SHRINK: f64 = 0.5;
const SWELL: f64 = -0.53;

/// Smooth the heights of `mesh` with `passes` Taubin passes over the mesh's
/// edges. Points at the same position are welded first, so a surface stored
/// one copy of each point per face still has its interior. x and y never
/// move, and every vertex on the rim of the surface or of a hole is held.
/// Returns the welded vertices and faces, how many heights changed and how
/// many vertices were held.
pub(super) fn smooth_heights(mesh: &mesh_data::Triangulation, passes: u32, cancel: &CancelFlag, progress: &Progress) -> Result<SmoothedSurface> {
    let (mut vertices, faces) = weld_points(mesh);
    let mesh = &mesh_data::Triangulation::from_vertices_and_faces(vertices.clone(), faces.clone())?;
    let count = vertices.len();

    let mut held = vec![false; count];
    for [a, b] in topology_boundary_edges(mesh) {
        held[a] = true;
        held[b] = true;
    }

    // Neighbours of each vertex, in one flat list.
    let edges = unique_edges(mesh);
    let mut start = vec![0usize; count + 1];
    for [a, b] in &edges {
        start[*a as usize + 1] += 1;
        start[*b as usize + 1] += 1;
    }
    for index in 0..count {
        start[index + 1] += start[index];
    }
    let mut fill = start.clone();
    let mut neighbours = vec![0u32; edges.len() * 2];
    for [a, b] in &edges {
        neighbours[fill[*a as usize]] = *b;
        fill[*a as usize] += 1;
        neighbours[fill[*b as usize]] = *a;
        fill[*b as usize] += 1;
    }

    let mut heights: Vec<f64> = vertices.iter().map(|vertex| vertex.z).collect();
    let mut delta = vec![0.0f64; count];
    for pass in 0..passes {
        if cancel.is_cancelled() {
            anyhow::bail!(crate::app::jobs::Cancelled);
        }
        for factor in [SHRINK, SWELL] {
            for index in 0..count {
                let around = &neighbours[start[index]..start[index + 1]];
                delta[index] = if held[index] || around.is_empty() {
                    0.0
                } else {
                    let sum: f64 = around.iter().map(|&other| heights[other as usize]).sum();
                    factor * (sum / around.len() as f64 - heights[index])
                };
            }
            for (height, change) in heights.iter_mut().zip(&delta) {
                *height += change;
            }
        }
        progress.set_fraction((pass + 1) as f32 / passes as f32);
    }

    let mut moved = 0;
    for (vertex, height) in vertices.iter_mut().zip(&heights) {
        if vertex.z != *height {
            moved += 1;
        }
        vertex.z = *height;
    }
    Ok((vertices, faces, moved, held.iter().filter(|&&rim| rim).count()))
}

impl App<'_> {
    /// Smooth a surface's heights into a new surface; the rim and the rims of
    /// holes stay as they are, and so does every x and y.
    pub(crate) fn smooth_triangulation(&mut self, id: TriangulationId, passes: u32, name: String) -> Result<()> {
        if passes == 0 {
            anyhow::bail!(tr!("err-surf-smooth-no-passes"));
        }
        let (mesh, source_name, quality) = {
            let source = self
                .triangulations
                .iter()
                .find(|triangulation| triangulation.id == id && triangulation.state.loaded)
                .ok_or_else(|| anyhow::anyhow!(tr!("err-surf-smooth-not-found")))?;
            (source.mesh.clone(), source.name.clone(), source.quality.clone())
        };

        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<crate::model::triangulation::GeneratedTriangulationLog> {
            let (vertices, faces, moved, held) = smooth_heights(&mesh, passes, cancel, progress)?;
            let generated = session::build_generated_triangulation(name, vertices, faces, TriSurfaceType::Surface, |_| Vec::new())?.with_quality(quality);
            Ok(crate::model::triangulation::GeneratedTriangulationLog {
                generated,
                message: tr!(
                    "cmd-smooth-surface-smoothed",
                    surface = source_name.to_string(),
                    passes = passes.to_string(),
                    moved = moved.to_string(),
                    held = held.to_string()
                ),
            })
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulationLog>| {
            app.apply_generated_triangulation_job(result, &[]);
        };
        self.spawn_job_reporting_progress(tr!("cmd-smooth-surface-smoothing"), vec![crate::app::jobs::JobKey::Triangulation(id)], compute, apply);
        Ok(())
    }
}
