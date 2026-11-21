use super::*;

impl<'a> App<'a> {
    /// Thin a surface to a share of its triangles, as a new surface.
    pub(crate) fn simplify_triangulation(&mut self, tri_id: TriangulationId, keep: f64, max_deviation: Option<f64>, name: String, unload_source: bool) -> Result<()> {
        let (mesh, tri_name) = {
            let tri = self
                .triangulations
                .iter()
                .find(|t| t.id == tri_id)
                .ok_or_else(|| anyhow::anyhow!(tr!("err-tri-triangulation-not-found")))?;
            (tri.mesh.clone(), tri.name.clone())
        };

        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<crate::model::triangulation::GeneratedTriangulationLog> {
            // The collapse is nearly all of it; the mesh and BVH build after
            // it are of the thinned surface.
            let settings = crate::model::simplify::Settings { keep, max_deviation };
            let Some(simplified) = crate::model::simplify::simplify(&mesh, &settings, &cancel.signal(), &progress.phase(0.0, 0.95)) else {
                anyhow::bail!(crate::app::jobs::Cancelled);
            };
            let from = mesh.face_count();
            let to = simplified.faces.len();
            if to >= from {
                anyhow::bail!(tr!("err-simplify-nothing-to-do"));
            }
            drop(mesh);
            let generated = session::build_generated_triangulation(name, simplified.vertices, simplified.faces, TriSurfaceType::Surface, |_| Vec::new())?;
            Ok(crate::model::triangulation::GeneratedTriangulationLog {
                generated,
                message: tr!(
                    "simplify-done",
                    name = tri_name.to_string(),
                    from = crate::model::numbers::format_quantity(from as f64, 0),
                    to = crate::model::numbers::format_quantity(to as f64, 0),
                    deviation = format!("{:.3}", simplified.deviation)
                ),
            })
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulationLog>| {
            app.apply_generated_triangulation_job(result, unload_source.then_some(tri_id).as_slice());
        };
        self.spawn_job_reporting_progress(tr!("simplify-working"), vec![crate::app::jobs::JobKey::Triangulation(tri_id)], compute, apply);
        Ok(())
    }
}
