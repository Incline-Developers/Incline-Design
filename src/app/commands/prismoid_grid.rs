//! Prismoid grid commands: building one from two surfaces, and removing
//! one from the explorer.

use std::sync::Arc;

use anyhow::Result;

use crate::{
    app::{
        App,
        commands::thickness_points::not_a_grid_reason,
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        ItemRef, MemberKind, SceneEntityId, SectionKind,
        grid_surface::GridSurface,
        prismoid_grid::{BuildRefusal, DEFAULT_PRISMOID_GRID_COLOR, OpenPrismoidGrid, PrismoidBuild, PrismoidGrid, PrismoidGridId},
        progress::Progress,
        project::{ProjectItemState, unique_item_name},
        triangulation::TriangulationId,
    },
    userspace_error, userspace_log, userspace_warn,
};

impl App<'_> {
    /// Remove a prismoid grid from the project as one undoable step.
    pub(crate) fn remove_prismoid_grid(&mut self, id: PrismoidGridId) {
        let entity = SceneEntityId::PrismoidGrid(id);
        self.editor.selected_handles.remove(&entity);
        self.editor.hidden_handles.remove(&entity);
        self.editor.explicitly_frozen.remove(&entity);
        self.editor.frozen_handles.remove(&entity);
        self.editor.translucent_handles.remove(&entity);
        self.delete_project_item(ItemRef::PrismoidGrid(id));
    }

    /// Build a prismoid grid between a roof and a floor surface on the job
    /// queue; a pair that makes none is refused in words naming both.
    pub(crate) fn build_prismoid_grid(&mut self, roof_id: TriangulationId, floor_id: TriangulationId, name: String) -> Result<()> {
        if roof_id == floor_id {
            anyhow::bail!("{}", tr!("cmd-prismoid-grid-same-surface"));
        }
        let read = |id: TriangulationId| -> Result<(
            String,
            Arc<crate::model::formats::mesh_data::Triangulation>,
            Arc<crate::model::spatial::TriangleBvh>,
            SectionKind,
        )> {
            let item = self
                .triangulations
                .iter()
                .find(|item| item.id == id && item.state.loaded)
                .ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-thickness-points-surface-gone")))?;
            Ok((item.name.clone(), Arc::clone(&item.mesh), Arc::clone(&item.spatial), item.state.section))
        };
        let (roof_name, roof_mesh, roof_spatial, roof_section) = read(roof_id)?;
        let (floor_name, floor_mesh, floor_spatial, floor_section) = read(floor_id)?;
        let section = SectionKind::derived_for(MemberKind::PrismoidGrid, [roof_section, floor_section]);
        let (roof_label, floor_label, grid_name) = (roof_name.clone(), floor_name.clone(), name.clone());
        // An Err is a refusal; an Ok(Err) is a built grid failing its own
        // check, a fault in the build rather than in the surfaces.
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<Result<PrismoidBuild>> {
            let roof = GridSurface::read(roof_mesh, roof_spatial)
                .map_err(|problem| anyhow::anyhow!("{}", tr!("cmd-prismoid-grid-not-a-grid", name = roof_name.clone(), reason = not_a_grid_reason(problem))))?;
            let floor = GridSurface::read(floor_mesh, floor_spatial)
                .map_err(|problem| anyhow::anyhow!("{}", tr!("cmd-prismoid-grid-not-a-grid", name = floor_name.clone(), reason = not_a_grid_reason(problem))))?;
            match PrismoidGrid::between(&roof, &floor, cancel, &progress.phase(0.0, 1.0)) {
                Ok(built) => Ok(Ok(built)),
                Err(BuildRefusal::Malformed(error)) => Ok(Err(error.context(grid_name))),
                Err(BuildRefusal::Cancelled) => anyhow::bail!("{}", tr!("common-cancelled")),
                Err(BuildRefusal::NotOneLattice) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-not-one-lattice", roof = roof_name, floor = floor_name)),
                Err(BuildRefusal::NoOverlap) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-no-overlap", roof = roof_name, floor = floor_name)),
                Err(BuildRefusal::RoofBelow) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-roof-below", roof = roof_name, floor = floor_name)),
                Err(BuildRefusal::NoThickness) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-no-thickness", roof = roof_name, floor = floor_name)),
            }
        };
        let apply = move |app: &mut App, result: Result<Result<PrismoidBuild>>| match result {
            Err(refusal) => userspace_warn!("{refusal:#}"),
            Ok(Err(fault)) => userspace_error!("{fault:#}"),
            Ok(Ok(PrismoidBuild { grid, crossed, holes })) => {
                let id = app.insert_prismoid_grid(name, grid, section);
                let Some(item) = app.prismoid_grids.iter().find(|item| item.id == id) else {
                    return;
                };
                let summary = item.summary;
                let (min, max) = summary.vertical_thickness.unwrap_or_default();
                userspace_log!(
                    "{}",
                    tr!(
                        "cmd-prismoid-grid-built",
                        name = item.name.clone(),
                        columns = summary.columns.to_string(),
                        rows = summary.rows.to_string(),
                        active = summary.active_cells.to_string(),
                        min = format!("{min:.2}"),
                        max = format!("{max:.2}")
                    )
                );
                if crossed > 0 {
                    userspace_warn!(
                        "{}",
                        tr!(
                            "cmd-prismoid-grid-crossed",
                            count = (crossed as i64),
                            roof = roof_label.clone(),
                            floor = floor_label.clone()
                        )
                    );
                }
                if holes > 0 {
                    userspace_log!("{}", tr!("cmd-prismoid-grid-holes", count = (holes as i64), roof = roof_label, floor = floor_label));
                }
            }
        };
        self.spawn_job_reporting_progress(
            tr!("cmd-prismoid-grid-building"),
            vec![JobKey::Triangulation(roof_id), JobKey::Triangulation(floor_id)],
            compute,
            apply,
        );
        Ok(())
    }

    /// Add a built prismoid grid to the project, dirty, at its section's
    /// root, and select it.
    pub(crate) fn insert_prismoid_grid(&mut self, name: String, grid: PrismoidGrid, section: SectionKind) -> PrismoidGridId {
        let id = PrismoidGridId(self.next_prismoid_grid_id);
        self.next_prismoid_grid_id += 1;
        let name = unique_item_name(name, self.prismoid_grids.iter().map(|item| item.name.as_str()));
        self.prismoid_grids.push(OpenPrismoidGrid {
            id,
            state: ProjectItemState::dirty(MemberKind::PrismoidGrid, None).with_section(section),
            name,
            color: DEFAULT_PRISMOID_GRID_COLOR,
            summary: grid.summary(),
            grid: Arc::new(grid),
        });
        self.touch_active_project_content();
        self.editor.selected_handles.clear();
        self.editor.selected_handles.insert(SceneEntityId::PrismoidGrid(id));
        self.invalidate_geometry();
        id
    }
}
