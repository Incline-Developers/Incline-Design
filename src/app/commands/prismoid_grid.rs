//! Prismoid grid commands: building one from two surfaces, removing one
//! from the explorer, and preparing what the view draws of each.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    sync::Arc,
};

use anyhow::{Context, Result};

use crate::{
    app::{
        App,
        commands::{raster::MAX_RASTER_PREVIEW_DIMENSION, solids::preview_triangulation, solids_view::next_view_id, thickness_points::not_a_grid_reason},
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        ItemRef, MemberKind, SceneEntityId, SectionKind,
        formats::mesh_data::Triangulation,
        grid_surface::GridSurface,
        prismoid_grid::{BuildRefusal, DEFAULT_PRISMOID_GRID_COLOR, OpenPrismoidGrid, PrismoidBuild, PrismoidGrid, PrismoidGridId, THICKNESS_RAMP, ThicknessMap},
        progress::Progress,
        project::{ModelEnvelope, ProjectItemState, unique_item_name},
        raster::{OpenRasterTexture, RasterTextureId},
        rbf::NODE_BUDGET,
        spatial::TriangleBvh,
        triangulation::{OpenTriangulation, TriangulationId, spatial_surface_face_order},
    },
    rendering::{color::linear_to_srgb_byte, scene::drill_hole_cache::evaluate_stops},
    ui::state::Workspace,
    userspace_error, userspace_log, userspace_warn,
};

/// The dark line round a drawn grid's roof and floor.
const PRISMOID_OUTLINE_COLOR: [f32; 4] = [0.05, 0.06, 0.08, 1.0];

/// A prismoid grid as the view draws it, for as long as `source` stands.
pub(crate) struct DrawnPrismoidGrid {
    source: Arc<PrismoidGrid>,
    surface: OpenTriangulation,
    /// `None` on a tensor lattice, which draws in the grid's flat colour.
    thickness: Option<OpenRasterTexture>,
}

impl App<'_> {
    /// The loaded prismoid grids in the selection.
    pub(crate) fn selected_prismoid_grids(&self) -> Vec<PrismoidGridId> {
        self.prismoid_grids
            .iter()
            .filter(|item| item.state.loaded && self.editor.selected_handles.contains(&item.entity_id()))
            .map(|item| item.id)
            .collect()
    }

    /// Remove a prismoid grid from the project as one undoable step.
    pub(crate) fn remove_prismoid_grid(&mut self, id: PrismoidGridId) {
        let entity = SceneEntityId::PrismoidGrid(id);
        self.editor.selected_handles.remove(&entity);
        self.editor.hidden_handles.remove(&entity);
        self.editor.explicitly_frozen.remove(&entity);
        self.editor.frozen_handles.remove(&entity);
        self.editor.translucent_handles.remove(&entity);
        self.cancel_jobs(|key| *key == JobKey::PrismoidGrid(id));
        self.delete_project_item(ItemRef::PrismoidGrid(id));
    }

    /// Build a prismoid grid between a roof and a floor surface on the job
    /// queue; a pair that makes none is refused in words naming both.
    pub(crate) fn build_prismoid_grid(&mut self, roof_id: TriangulationId, floor_id: TriangulationId, step: NonZeroUsize, name: String) -> Result<()> {
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
        let envelope = self.workspace.active_project().and_then(|project| project.project.metadata.modelling.envelope);
        let (roof_label, floor_label, grid_name) = (roof_name.clone(), floor_name.clone(), name.clone());
        // An Err is a refusal; an Ok(Err) is a built grid failing its own
        // check, a fault in the build rather than in the surfaces.
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<Result<PrismoidBuild>> {
            let roof = GridSurface::read(roof_mesh, roof_spatial)
                .map_err(|problem| anyhow::anyhow!("{}", tr!("cmd-prismoid-grid-not-a-grid", name = roof_name.clone(), reason = not_a_grid_reason(problem))))?;
            let floor = GridSurface::read(floor_mesh, floor_spatial)
                .map_err(|problem| anyhow::anyhow!("{}", tr!("cmd-prismoid-grid-not-a-grid", name = floor_name.clone(), reason = not_a_grid_reason(problem))))?;
            match PrismoidGrid::between(&roof, &floor, step, envelope.as_ref(), cancel, &progress.phase(0.0, 1.0)) {
                Ok(built) => Ok(Ok(built)),
                Err(BuildRefusal::Malformed(error)) => Ok(Err(error.context(grid_name))),
                Err(BuildRefusal::Cancelled) => anyhow::bail!("{}", tr!("common-cancelled")),
                Err(BuildRefusal::NotOneLattice) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-not-one-lattice", roof = roof_name, floor = floor_name)),
                Err(BuildRefusal::NoOverlap) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-no-overlap", roof = roof_name, floor = floor_name)),
                Err(BuildRefusal::CellTooLarge) => anyhow::bail!(
                    "{}",
                    tr!(
                        "cmd-prismoid-grid-cell-too-large",
                        roof = roof_name,
                        floor = floor_name,
                        size = cell_size(roof.spacing() * step.get() as f64)
                    )
                ),
                Err(BuildRefusal::RoofBelow) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-roof-below", roof = roof_name, floor = floor_name)),
                Err(BuildRefusal::NoThickness) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-no-thickness", roof = roof_name, floor = floor_name)),
                Err(BuildRefusal::OffEnvelopeLattice) => anyhow::bail!("{}", tr!("cmd-prismoid-grid-off-envelope-lattice", roof = roof_name, floor = floor_name)),
                Err(BuildRefusal::OutsideEnvelope) => anyhow::bail!(
                    "{}",
                    tr!(
                        "cmd-prismoid-grid-outside-envelope",
                        roof = roof_name,
                        floor = floor_name,
                        envelope = envelope.as_ref().map_or_else(String::new, ModelEnvelope::summary)
                    )
                ),
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
                let size = cell_size(item.grid.lattice.u_sizes.first().copied().unwrap_or_default());
                userspace_log!(
                    "{}",
                    tr!(
                        "cmd-prismoid-grid-built",
                        name = item.name.clone(),
                        columns = summary.columns.to_string(),
                        rows = summary.rows.to_string(),
                        size = size,
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

    /// Keep the view's meshes in step with the open grids, in Geology. Runs
    /// each frame; nothing is prepared or rebuilt unless a grid or the
    /// surfaces beside it moved.
    pub(crate) fn sync_prismoid_drawing(&mut self) {
        self.editor.model_envelope = self.workspace.active_project().and_then(|project| project.project.metadata.modelling.envelope);
        if self.editor.active_workspace == Workspace::Geology {
            let grids = &self.prismoid_grids;
            self.prismoid_drawn.retain(|drawn| {
                grids
                    .iter()
                    .any(|item| Some(item.id) == drawn.surface.prismoid_grid && item.state.loaded && Arc::ptr_eq(&item.grid, &drawn.source))
            });
            let wanted: Vec<_> = grids
                .iter()
                .filter(|item| item.state.loaded && item.summary.active_cells > 0)
                .map(|item| item.id)
                .filter(|id| !self.prismoid_refused.contains(id))
                .filter(|id| !self.prismoid_drawn.iter().any(|drawn| drawn.surface.prismoid_grid == Some(*id)))
                .filter(|id| !self.job_pending(&JobKey::PrismoidGrid(*id)))
                .collect();
            for id in wanted {
                self.spawn_prismoid_drawing(id);
            }
        }
        self.refresh_prismoid_scene();
    }

    /// Prepare a grid's mesh on the job queue. A grid over the node budget is
    /// refused, not drawn; only a file can hold one.
    fn spawn_prismoid_drawing(&mut self, id: PrismoidGridId) {
        let Some(item) = self.prismoid_grids.iter().find(|item| item.id == id) else {
            return;
        };
        let (grid, name, color, range) = (Arc::clone(&item.grid), item.name.clone(), item.color, item.summary.vertical_thickness);
        let corners = grid.lattice.corner_count();
        if corners > NODE_BUDGET {
            self.prismoid_refused.insert(id);
            userspace_warn!(
                "{}",
                tr!(
                    "cmd-prismoid-grid-too-large-to-draw",
                    name = name,
                    corners = corners.to_string(),
                    budget = NODE_BUDGET.to_string()
                )
            );
            return;
        }
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<DrawnPrismoidGrid> {
            let phase = progress.phase(0.0, 1.0);
            let proceed = || -> Result<()> {
                if cancel.is_cancelled() {
                    anyhow::bail!("{}", tr!("common-cancelled"));
                }
                Ok(())
            };
            let failed = || tr!("cmd-prismoid-grid-mesh-failed", name = name.clone());
            let closed = grid.closed_mesh().with_context(failed)?;
            let mesh = Triangulation::from_vertices_and_faces(closed.vertices, closed.faces).with_context(failed)?;
            phase.set_fraction(0.3);
            proceed()?;
            let spatial = Arc::new(TriangleBvh::build(&mesh));
            phase.set_fraction(0.6);
            proceed()?;
            let order = Arc::new(spatial_surface_face_order(&mesh));
            phase.set_fraction(0.8);
            proceed()?;
            let thickness = range
                .and_then(|range| grid.thickness_map(range, MAX_RASTER_PREVIEW_DIMENSION))
                .map(|map| thickness_raster(id, &name, map));
            let mut surface = preview_triangulation(name, Arc::new(mesh), spatial, closed.outline, order, color, PRISMOID_OUTLINE_COLOR);
            surface.id = next_view_id();
            surface.raster_texture = thickness.as_ref().map(|raster| raster.id);
            surface.always_show_edges = true;
            surface.prismoid_grid = Some(id);
            phase.finish();
            Ok(DrawnPrismoidGrid { source: grid, surface, thickness })
        };
        let apply = move |app: &mut App, result: Result<DrawnPrismoidGrid>| match result {
            Ok(drawn) => {
                let should_fit = !app.scene_has_renderables();
                app.prismoid_drawn.push(drawn);
                app.refresh_prismoid_scene();
                if should_fit {
                    app.fit_view_to_extents();
                }
                app.invalidate_topology_bounds_and_redraw();
            }
            // A second try would meet the same fault, every frame.
            Err(fault) => {
                app.prismoid_refused.insert(id);
                userspace_error!("{fault:#}");
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-preparing"), vec![JobKey::PrismoidGrid(id)], compute, apply);
    }

    /// What the merged surface list is made of, so it is put together when
    /// one of those things moves rather than once a frame.
    fn prismoid_scene_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for mesh in &self.triangulations {
            mesh.id.0.hash(&mut hasher);
            mesh.state.revision().hash(&mut hasher);
            (Arc::as_ptr(&mesh.mesh) as usize).hash(&mut hasher);
        }
        for raster in &self.raster_textures {
            raster.id.hash(&mut hasher);
            raster.state.revision().hash(&mut hasher);
        }
        for drawn in &self.prismoid_drawn {
            drawn.surface.prismoid_grid.hash(&mut hasher);
            (Arc::as_ptr(&drawn.surface.mesh) as usize).hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Put the project's surfaces and rasters and the drawn grids into one
    /// list each. Empty off Geology and with nothing drawn, so every reader
    /// falls back to the project's own.
    fn refresh_prismoid_scene(&mut self) {
        if self.editor.active_workspace != Workspace::Geology || self.prismoid_drawn.is_empty() {
            self.prismoid_scene.clear();
            self.prismoid_rasters.clear();
            self.prismoid_scene_key = None;
            return;
        }
        let key = self.prismoid_scene_key();
        if self.prismoid_scene_key == Some(key) {
            return;
        }
        self.prismoid_scene = self.triangulations.iter().chain(self.prismoid_drawn.iter().map(|drawn| &drawn.surface)).cloned().collect();
        let maps = self.prismoid_drawn.iter().filter_map(|drawn| drawn.thickness.as_ref());
        self.prismoid_rasters = self.raster_textures.iter().chain(maps).cloned().collect();
        self.prismoid_scene_key = Some(key);
    }
}

/// A cell size in metres to the centimetre: a size read off a surface's
/// vertices carries their rounding.
pub(super) fn cell_size(size: f64) -> String {
    ((size * 100.0).round() / 100.0).to_string()
}

/// A thickness map in the ramp's colours, as a raster the drape can draw. Its
/// id counts down from the top, clear of the project's.
fn thickness_raster(id: PrismoidGridId, name: &str, map: ThicknessMap) -> OpenRasterTexture {
    let stops = THICKNESS_RAMP.stops();
    let rgba = map
        .positions
        .iter()
        .flat_map(|position| {
            let [red, green, blue] = evaluate_stops(*position, &stops, THICKNESS_RAMP.smooth()).map(linear_to_srgb_byte);
            [red, green, blue, u8::MAX]
        })
        .collect();
    let rgba = Arc::new(rgba);
    OpenRasterTexture {
        id: RasterTextureId(u64::MAX - id.0),
        state: ProjectItemState::dirty(MemberKind::Raster, None).with_loaded(true),
        name: name.to_owned(),
        source_size: map.size,
        preview_size: map.size,
        full_rgba: Arc::clone(&rgba),
        rgba,
        world_to_uv: map.world_to_uv,
        projection: String::new(),
        driver_name: String::new(),
    }
}

/// `merged` unless it is empty. A free function on the two fields, because
/// its answer is held across borrows of the editor and the renderer.
pub(crate) fn with_prismoid_scene<'s, T>(own: &'s [T], merged: &'s [T]) -> &'s [T] {
    if merged.is_empty() { own } else { merged }
}
