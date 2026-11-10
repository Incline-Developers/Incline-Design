//! Prismoid grid commands: building one from two surfaces, adding qualities
//! to one, removing one from the explorer, and preparing what the view draws
//! of each.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Context, Result};
use glam::DVec3;

use crate::{
    app::{
        App,
        commands::{
            raster::MAX_RASTER_PREVIEW_DIMENSION, solids::preview_triangulation, solids_view::next_view_id, thickness_points::not_a_grid_reason,
            triangulation::session::refuse_quality,
        },
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        Command, ItemRef, MemberKind, OpenItem, ReserveAggregation, SceneEntityId, SectionKind,
        formats::mesh_data::Triangulation,
        grid_surface::GridSurface,
        plot::format_quantity,
        prismoid_grid::{
            AddedValues, BuildRefusal, CellReading, CellValueRole, CellValues, DEFAULT_PRISMOID_GRID_COLOR, OpenPrismoidGrid, PrismoidBuild, PrismoidGrid, PrismoidGridId,
            PrismoidGridSummary, QualitySurface, THICKNESS_RAMP, ThicknessMap, ValuesRefusal, Weighting,
        },
        progress::{Phase, Progress},
        project::{ModelEnvelope, ProjectItemState, unique_item_name},
        raster::{OpenRasterTexture, RasterTextureId},
        rbf::NODE_BUDGET,
        solid_reserves::NumericTotal,
        spatial::TriangleBvh,
        triangulation::{OpenTriangulation, QualityMark, TriangulationId, spatial_surface_face_order},
    },
    rendering::{camera::SectionSlab, color::linear_to_srgb_byte, scene::drill_hole_cache::evaluate_stops},
    ui::state::{CellSelection, EditorState, InspectorRow, InspectorRows, SelectionMode, Workspace},
    userspace_error, userspace_log, userspace_warn,
};

/// The dark line round a drawn grid's roof and floor.
const PRISMOID_OUTLINE_COLOR: [f32; 4] = [0.05, 0.06, 0.08, 1.0];

/// Counts cell selection changes over the session; each keys its outline.
static NEXT_CELL_SELECTION_REVISION: AtomicU64 = AtomicU64::new(1);

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
            refuse_quality(&item.name, item.quality.as_ref())?;
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
                let summary = &item.summary;
                let totals = totals_lines(summary);
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
                for line in totals {
                    userspace_log!("{line}");
                }
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

    /// Read quality surfaces into a prismoid grid's cells, `density` as its
    /// density, as one undoable step; surfaces saved unloaded are loaded
    /// first.
    pub(crate) fn add_prismoid_qualities(&mut self, grid_id: PrismoidGridId, surfaces: Vec<TriangulationId>, density: Option<TriangulationId>) -> Result<()> {
        if !self.prismoid_grids.iter().any(|item| item.id == grid_id && item.state.loaded) {
            anyhow::bail!("{}", tr!("cmd-commands-select-one-loaded-prismoid-grid-qualities"));
        }
        if !surfaces.iter().all(|id| self.triangulations.iter().any(|item| item.id == *id && item.quality.is_some())) {
            anyhow::bail!("{}", tr!("cmd-thickness-points-surface-gone"));
        }
        let needed = surfaces.iter().map(|id| ItemRef::Triangulation(*id)).collect();
        let restoring = self.restore_items_for(needed, {
            let surfaces = surfaces.clone();
            move |app| {
                if let Err(refusal) = app.spawn_adding_prismoid_qualities(grid_id, &surfaces, density) {
                    userspace_warn!("{refusal:#}");
                }
            }
        });
        if restoring {
            Ok(())
        } else {
            self.spawn_adding_prismoid_qualities(grid_id, &surfaces, density)
        }
    }

    fn spawn_adding_prismoid_qualities(&mut self, grid_id: PrismoidGridId, surfaces: &[TriangulationId], density: Option<TriangulationId>) -> Result<()> {
        let gone = || anyhow::anyhow!("{}", tr!("cmd-commands-select-one-loaded-prismoid-grid-qualities"));
        let runtime_id = self.workspace.active_project().map(|project| project.runtime_id).ok_or_else(gone)?;
        let item = self.prismoid_grids.iter().find(|item| item.id == grid_id && item.state.loaded).ok_or_else(gone)?;
        let (source, name) = (Arc::clone(&item.grid), item.name.clone());
        let inputs = surfaces
            .iter()
            .map(|id| {
                let item = self
                    .triangulations
                    .iter()
                    .find(|item| item.id == *id && item.state.deferred.is_none())
                    .ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-thickness-points-surface-gone")))?;
                Ok(QualityInput {
                    surface: item.name.clone(),
                    mark: item.quality.clone().ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-thickness-points-surface-gone")))?,
                    density: density == Some(*id),
                    mesh: Arc::clone(&item.mesh),
                    spatial: Arc::clone(&item.spatial),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut keys = vec![JobKey::PrismoidGrid(grid_id)];
        keys.extend(surfaces.iter().map(|id| JobKey::Triangulation(*id)));
        let grid = Arc::clone(&source);
        let read_name = name.clone();
        let compute = move |cancel: &CancelFlag, progress: &Progress| add_qualities(&grid, &read_name, inputs, cancel, &progress.phase(0.0, 1.0));
        let apply = move |app: &mut App, result: Result<(PrismoidGrid, Vec<String>)>| {
            let (merged, lines) = match result {
                Ok(added) => added,
                Err(refusal) => {
                    userspace_warn!("{refusal:#}");
                    return;
                }
            };
            let current = app.workspace.active_project().is_some_and(|project| project.runtime_id == runtime_id);
            let Some(item) = app.prismoid_grids.iter().find(|item| item.id == grid_id).filter(|_| current) else {
                userspace_warn!("{}", tr!("cmd-prismoid-grid-qualities-discarded", grid = name));
                return;
            };
            if !Arc::ptr_eq(&item.grid, &source) {
                userspace_warn!("{}", tr!("cmd-prismoid-grid-changed-while-reading", grid = item.name.clone()));
                return;
            }
            let grid_name = item.name.clone();
            let mut replacement = item.clone();
            replacement.summary = merged.summary();
            let totals = totals_lines(&replacement.summary);
            replacement.grid = Arc::new(merged);
            app.execute_edit_for(
                runtime_id,
                Command::ReplaceItem {
                    item: ItemRef::PrismoidGrid(grid_id),
                    other: Some(OpenItem::PrismoidGrid(Box::new(replacement))),
                },
            );
            userspace_log!("{}", tr!("cmd-prismoid-grid-qualities-added", count = lines.len().to_string(), grid = grid_name));
            for line in lines.into_iter().chain(totals) {
                userspace_log!("{line}");
            }
            userspace_log!("{}", tr!("cmd-prismoid-grid-qualities-rederive"));
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-adding-qualities"), keys, compute, apply);
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
        if self
            .editor
            .cell_selection
            .as_ref()
            .is_some_and(|selection| !cell_selection_stands(selection, &self.prismoid_grids, &self.editor))
        {
            self.editor.cell_selection = None;
            self.invalidate_geometry();
        }
        if let Some(selection) = self.editor.cell_selection.as_mut() {
            follow_grid_rename(selection, &self.prismoid_grids);
        }
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

    /// Pick the prismoid a click on a drawn grid landed on, by Leo's
    /// selection modes one grain down: the grid stays selected while any of
    /// its cells is. False when the point is on no active cell.
    pub(crate) fn pick_prismoid_cell(&mut self, id: PrismoidGridId, world: DVec3) -> bool {
        let entity = SceneEntityId::PrismoidGrid(id);
        let Some((grid, name)) = self
            .prismoid_grids
            .iter()
            .find(|item| item.id == id && item.state.loaded)
            .map(|item| (Arc::clone(&item.grid), item.name.clone()))
        else {
            return false;
        };
        let Some(cell) = grid.picked_cell(world) else {
            return false;
        };
        let held = self
            .editor
            .cell_selection
            .take()
            .filter(|selection| selection.entity == entity && Arc::ptr_eq(&selection.grid, &grid));
        let (mut cells, count) = held.map_or_else(|| (vec![false; grid.lattice.cell_count()], 0), |selection| (selection.cells, selection.count));
        let mode = if self.modifiers.shift_key() {
            SelectionMode::Toggle
        } else if self.modifiers.control_key() {
            SelectionMode::Add
        } else if cells[cell] {
            SelectionMode::Toggle
        } else {
            SelectionMode::Replace
        };
        let count = apply_cell_pick(&mut cells, count, cell, mode);
        let item_mode = match mode {
            SelectionMode::Replace => Some(SelectionMode::Replace),
            _ if count > 0 => Some(SelectionMode::Add),
            _ => self.editor.selected_handles.contains(&entity).then_some(SelectionMode::Toggle),
        };
        if let Some(item_mode) = item_mode {
            self.editor.on_canvas_pick(entity, world, item_mode);
        }
        self.active_triangulation = None;
        self.editor.cell_selection = cell_selection(entity, grid, &name, cells, count);
        true
    }

    /// Apply a marquee to the cell selection after Leo's box has applied its
    /// own, in its modifier order: shift toggles each cell taken, ctrl adds,
    /// a plain box replaces. The grid is selected while any of its cells is.
    pub(crate) fn select_prismoid_cells_in_box(&mut self, start: (f32, f32), end: (f32, f32)) {
        let (toggle, add) = (self.modifiers.shift_key(), self.modifiers.control_key());
        let Some((id, taken)) = self.prismoid_cells_in_screen_rect(start, end, toggle || add) else {
            return;
        };
        let Some((grid, name)) = self
            .prismoid_grids
            .iter()
            .find(|item| item.id == id)
            .map(|item| (Arc::clone(&item.grid), item.name.clone()))
        else {
            return;
        };
        let entity = SceneEntityId::PrismoidGrid(id);
        let held = self
            .editor
            .cell_selection
            .take()
            .filter(|selection| (toggle || add) && selection.entity == entity && Arc::ptr_eq(&selection.grid, &grid));
        let (mut cells, mut count) = held.map_or_else(|| (vec![false; grid.lattice.cell_count()], 0), |selection| (selection.cells, selection.count));
        let mode = if toggle { SelectionMode::Toggle } else { SelectionMode::Add };
        for cell in taken {
            count = apply_cell_pick(&mut cells, count, cell, mode);
        }
        if count > 0 {
            self.editor.selected_handles.insert(entity);
        } else {
            self.editor.selected_handles.remove(&entity);
        }
        self.editor.cell_selection = cell_selection(entity, grid, &name, cells, count);
    }

    /// The cells a marquee takes, as Lucas's dig blocks ride Leo's box: of
    /// one drawn grid, shown and not frozen, each active cell whose box
    /// centre falls inside, kept to the section's slab when one is up. With
    /// `keep_held` the grid holding cells is asked first; otherwise, or with
    /// none of its cells inside, the first grid in project order with any.
    fn prismoid_cells_in_screen_rect(&self, start: (f32, f32), end: (f32, f32), keep_held: bool) -> Option<(PrismoidGridId, Vec<usize>)> {
        if self.editor.active_workspace != Workspace::Geology {
            return None;
        }
        let graphics = self.graphics.as_ref()?;
        let (view_proj, slab) = (graphics.view_proj(), graphics.section_slab());
        let editor = &self.editor;
        let drawn = self.prismoid_grids.iter().filter(|item| {
            let entity = item.entity_id();
            item.state.loaded
                && !editor.hidden_handles.contains(&entity)
                && !editor.frozen_handles.contains(&entity)
                && self
                    .prismoid_drawn
                    .iter()
                    .any(|drawn| drawn.surface.prismoid_grid == Some(item.id) && Arc::ptr_eq(&drawn.source, &item.grid))
        });
        let taken = |item: &OpenPrismoidGrid| {
            let cells = centres_in_screen_rect(
                item.grid.cell_centres_3d(),
                |world| graphics.world_to_window_px(&view_proj, world),
                slab.as_ref(),
                start,
                end,
            );
            (!cells.is_empty()).then_some((item.id, cells))
        };
        let held = editor.cell_selection.as_ref().filter(|_| keep_held).map(|selection| selection.entity);
        held.and_then(|held| drawn.clone().find(|item| item.entity_id() == held))
            .and_then(taken)
            .or_else(|| drawn.filter_map(taken).next())
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

/// Apply a pick of `cell` to `cells`, `count` of them selected: Replace
/// leaves that cell alone in the set, Add adds it, Toggle flips it. Returns
/// the count after.
fn apply_cell_pick(cells: &mut [bool], count: usize, cell: usize, mode: SelectionMode) -> usize {
    match mode {
        SelectionMode::Replace => {
            cells.fill(false);
            cells[cell] = true;
            1
        }
        SelectionMode::Add if cells[cell] => count,
        SelectionMode::Add => {
            cells[cell] = true;
            count + 1
        }
        SelectionMode::Toggle => {
            cells[cell] = !cells[cell];
            if cells[cell] { count + 1 } else { count - 1 }
        }
    }
}

/// The cells whose centres project inside the screen rectangle from `start`
/// to `end`, within `slab` when one is given; a centre that projects nowhere
/// (behind the camera, say) is out.
fn centres_in_screen_rect(
    centres: impl Iterator<Item = (usize, DVec3)>,
    project: impl Fn(DVec3) -> Option<(f32, f32)>,
    slab: Option<&SectionSlab>,
    start: (f32, f32),
    end: (f32, f32),
) -> Vec<usize> {
    let (min, max) = ((start.0.min(end.0), start.1.min(end.1)), (start.0.max(end.0), start.1.max(end.1)));
    let inside = |p: (f32, f32)| p.0 >= min.0 && p.0 <= max.0 && p.1 >= min.1 && p.1 <= max.1;
    centres
        .filter(|(_, centre)| slab.is_none_or(|slab| slab.contains(*centre)))
        .filter(|(_, centre)| project(*centre).is_some_and(inside))
        .map(|(cell, _)| cell)
        .collect()
}

/// A selection of `count` cells on the grid `name`, derived; `None` with
/// none.
fn cell_selection(entity: SceneEntityId, grid: Arc<PrismoidGrid>, name: &str, cells: Vec<bool>, count: usize) -> Option<CellSelection> {
    (count > 0).then(|| {
        let mut selection = CellSelection {
            entity,
            grid,
            name: name.to_owned(),
            cells,
            count,
            revision: 0,
            outline: Vec::new(),
            rows: InspectorRows::default(),
        };
        refresh_cell_selection(&mut selection);
        selection
    })
}

/// Derive what a changed selection shows, under a new revision.
fn refresh_cell_selection(selection: &mut CellSelection) {
    selection.revision = NEXT_CELL_SELECTION_REVISION.fetch_add(1, Ordering::Relaxed);
    selection.outline = selection.grid.selection_outline(&selection.cells);
    selection.rows = cell_selection_rows(selection);
}

/// The inspector's rows for one picked prismoid or for several.
fn cell_selection_rows(selection: &CellSelection) -> InspectorRows {
    let single = (selection.count == 1).then(|| selection.cells.iter().position(|selected| *selected)).flatten();
    match single {
        Some(cell) => prismoid_cell_rows(&selection.name, &selection.grid, cell),
        None => {
            let cells = selection.cells.iter().enumerate().filter(|(_, selected)| **selected).map(|(cell, _)| cell);
            prismoid_selection_rows(&selection.name, &selection.grid.summary_of(cells))
        }
    }
}

/// Carry a rename of the selection's grid into its rows.
fn follow_grid_rename(selection: &mut CellSelection, grids: &[OpenPrismoidGrid]) {
    if let Some(item) = grids.iter().find(|item| item.entity_id() == selection.entity && item.name != selection.name) {
        selection.name.clone_from(&item.name);
        selection.rows = cell_selection_rows(selection);
    }
}

/// The inspector's rows for several prismoids of the grid `name`: how many,
/// their thickness range and what the means are weighted by, then their
/// volume, tonnes and each quality's mean, each saying what it left out.
fn prismoid_selection_rows(name: &str, summary: &PrismoidGridSummary) -> InspectorRows {
    let absent = || tr!("inventory-portion-missing");
    let left_out = |total: &NumericTotal| match total.missing + total.unusable_weights {
        0 => None,
        left => Some(tr!("prismoid-inspector-partial", counted = total.contributions, left = left)),
    };
    let (min, max) = summary.vertical_thickness.unwrap_or_default();
    let weighted = match (summary.weighting, &summary.density) {
        (Weighting::ByTonnes, Some(density)) => tr!("prismoid-inspector-by-tonnes", density = density.clone()),
        _ => tr!("prismoid-inspector-by-volume"),
    };
    let properties = vec![
        inspector_row(tr!("common-prismoid-grid"), name.to_owned(), None, None),
        inspector_row(tr!("prismoid-inspector-cells"), summary.active_cells.to_string(), None, None),
        inspector_row(
            tr!("prismoid-inspector-vertical-thickness"),
            tr!("prismoid-inspector-range", min = format_quantity(min, 2), max = format_quantity(max, 2)),
            Some("m"),
            None,
        ),
        inspector_row(tr!("reserve-weighted"), weighted, None, None),
    ];
    let tonnes = match &summary.tonnes {
        Some(total) => match total.value(ReserveAggregation::Sum) {
            Some(sum) => inspector_row(tr!("prismoid-inspector-tonnes"), format_quantity(sum, 1), Some("t"), left_out(total)),
            None => inspector_row(tr!("prismoid-inspector-tonnes"), absent(), None, left_out(total)),
        },
        None => inspector_row(tr!("prismoid-inspector-tonnes"), absent(), None, Some(tr!("prismoid-inspector-no-density"))),
    };
    let mut contents = vec![inspector_row(tr!("tri-volume"), format_quantity(summary.volume, 1), Some("m³"), None), tonnes];
    for quality in &summary.qualities {
        let unit = (!quality.unit.is_empty()).then_some(quality.unit.as_str());
        // `value` reads sum over weight for either average.
        contents.push(match quality.total.value(ReserveAggregation::VolumeAverage) {
            Some(mean) => inspector_row(quality.name.clone(), format_quantity(mean, 2), unit, left_out(&quality.total)),
            None => inspector_row(quality.name.clone(), absent(), None, left_out(&quality.total)),
        });
    }
    InspectorRows {
        title: tr!("planning-properties"),
        properties,
        contents,
    }
}

fn inspector_row(key: String, value: String, unit: Option<&str>, note: Option<String>) -> InspectorRow {
    InspectorRow {
        key,
        value,
        unit: unit.map(str::to_owned),
        note,
    }
}

/// The inspector's rows for one prismoid of the grid `name`: where it is,
/// its size and thicknesses, then its volume, tonnes and every value it
/// holds. Columns, rows and corners count from 1 at the lattice origin.
fn prismoid_cell_rows(name: &str, grid: &PrismoidGrid, cell: usize) -> InspectorRows {
    let lattice = &grid.lattice;
    let columns = lattice.columns();
    let (column, row) = (cell % columns, cell / columns);
    let measure = grid.cell_measure(cell);
    let ordinal = |index: usize| (index + 1).to_string();
    let mut properties = vec![
        inspector_row(tr!("common-prismoid-grid"), name.to_owned(), None, None),
        inspector_row(
            tr!("prismoid-inspector-cell"),
            tr!("prismoid-inspector-cell-index", column = ordinal(column), row = ordinal(row)),
            None,
            None,
        ),
        inspector_row(
            tr!("solids-plan-area"),
            tr!(
                "prismoid-inspector-plan-size",
                along_u = cell_size(lattice.u_sizes[column]),
                along_v = cell_size(lattice.v_sizes[row])
            ),
            Some("m"),
            None,
        ),
        inspector_row(
            tr!("prismoid-inspector-mean-thickness"),
            format_quantity(measure.map_or(0.0, |(bounds, _)| bounds.upper.z - bounds.lower.z), 2),
            Some("m"),
            None,
        ),
    ];
    // Round the cell from its origin corner, as `closed_mesh` takes them.
    for (along_u, along_v) in [(column, row), (column + 1, row), (column + 1, row + 1), (column, row + 1)] {
        let corner = along_v * (columns + 1) + along_u;
        properties.push(inspector_row(
            tr!("prismoid-inspector-corner", column = ordinal(along_u), row = ordinal(along_v)),
            format_quantity(grid.roof[corner] - grid.floor[corner], 2),
            Some("m"),
            None,
        ));
    }
    let volume = measure.map_or(0.0, |(_, measure)| measure.volume());
    let tonnes = match grid.cell_values.iter().find(|values| values.role == CellValueRole::Density) {
        None => inspector_row(
            tr!("prismoid-inspector-tonnes"),
            tr!("inventory-portion-missing"),
            None,
            Some(tr!("prismoid-inspector-no-density")),
        ),
        Some(density) => match density.values[cell] {
            Some(value) => inspector_row(
                tr!("prismoid-inspector-tonnes"),
                format_quantity(measure.map_or(0.0, |(_, measure)| measure.per_volume(value)), 1),
                Some("t"),
                None,
            ),
            None => inspector_row(
                tr!("prismoid-inspector-tonnes"),
                tr!("inventory-portion-missing"),
                None,
                Some(tr!("prismoid-inspector-no-density-value", density = density.name.clone())),
            ),
        },
    };
    let mut contents = vec![inspector_row(tr!("tri-volume"), format_quantity(volume, 1), Some("m³"), None), tonnes];
    for values in &grid.cell_values {
        let unit = (!values.unit.is_empty()).then_some(values.unit.as_str());
        contents.push(match values.values[cell] {
            Some(value) => inspector_row(values.name.clone(), format_quantity(value, 2), unit, None),
            None => inspector_row(values.name.clone(), tr!("inventory-portion-missing"), None, Some(tr!("prismoid-inspector-no-value"))),
        });
    }
    InspectorRows {
        title: tr!("planning-properties"),
        properties,
        contents,
    }
}

/// Whether a cell selection still stands: in Geology, on a loaded, shown
/// grid whose cells have not been replaced, and the grid still selected.
fn cell_selection_stands(selection: &CellSelection, grids: &[OpenPrismoidGrid], editor: &EditorState) -> bool {
    editor.active_workspace == Workspace::Geology
        && editor.selected_handles.contains(&selection.entity)
        && !editor.hidden_handles.contains(&selection.entity)
        && grids
            .iter()
            .any(|item| item.entity_id() == selection.entity && item.state.loaded && Arc::ptr_eq(&item.grid, &selection.grid))
}

/// One ticked quality surface, as the job reads it.
struct QualityInput {
    surface: String,
    mark: QualityMark,
    density: bool,
    mesh: Arc<Triangulation>,
    spatial: Arc<TriangleBvh>,
}

/// `grid` with each surface read into its cells under the surface's quality
/// mark, the one marked `density` as its density, and a console line per
/// surface. An Err is a refusal in words.
fn add_qualities(grid: &PrismoidGrid, grid_name: &str, inputs: Vec<QualityInput>, cancel: &CancelFlag, progress: &Phase) -> Result<(PrismoidGrid, Vec<String>)> {
    let (count, mut added, mut lines) = (inputs.len(), Vec::new(), Vec::new());
    for (done, input) in inputs.into_iter().enumerate() {
        let read = grid
            .read_cells(&QualitySurface::read(input.mesh, input.spatial), cancel)
            .ok_or_else(|| anyhow::anyhow!("{}", tr!("common-cancelled")))?;
        let (quality, surface, cells, missing) = (input.mark.label(), input.surface.clone(), read.read.to_string(), read.missing.to_string());
        lines.push(match read.reading {
            CellReading::Corners => tr!("cmd-prismoid-grid-quality-corners", quality = quality, surface = surface, read = cells, missing = missing),
            CellReading::Centre => tr!("cmd-prismoid-grid-quality-centre", quality = quality, surface = surface, read = cells, missing = missing),
            CellReading::Triangles => tr!("cmd-prismoid-grid-quality-triangles", quality = quality, surface = surface, read = cells, missing = missing),
        });
        added.push(AddedValues {
            surface: input.surface,
            values: CellValues {
                name: input.mark.name,
                unit: input.mark.unit,
                role: if input.density { CellValueRole::Density } else { CellValueRole::Quality },
                values: read.values,
            },
        });
        progress.set_fraction((done + 1) as f32 / count as f32);
    }
    let merged = grid.with_values(added).map_err(|refusal| anyhow::anyhow!("{}", values_refusal_words(refusal, grid_name)))?;
    Ok((merged, lines))
}

/// A refusal to add values to the prismoid grid `grid_name`, in words.
pub(crate) fn values_refusal_words(refusal: ValuesRefusal, grid_name: &str) -> String {
    match refusal {
        ValuesRefusal::Twice { first, second, quality } => tr!("cmd-prismoid-grid-quality-twice", first = first, second = second, quality = quality),
        ValuesRefusal::Reserved { surface, quality } => tr!("cmd-prismoid-grid-quality-reserved", surface = surface, quality = quality),
        ValuesRefusal::TwoDensities { held, surface } => tr!("cmd-prismoid-grid-two-densities", held = held, surface = surface),
        ValuesRefusal::DensityInKg { surface, unit } => tr!("cmd-prismoid-grid-density-kg", surface = surface, unit = unit),
        ValuesRefusal::NoCover { surface } => tr!("cmd-prismoid-grid-quality-no-cover", surface = surface, grid = grid_name.to_owned()),
        ValuesRefusal::Malformed(fault) => format!("{fault:#}"),
    }
}

/// A cell size in metres to the centimetre: a size read off a surface's
/// vertices carries their rounding.
pub(super) fn cell_size(size: f64) -> String {
    ((size * 100.0).round() / 100.0).to_string()
}

/// A grid's volume and tonnes, then each quality's mean, as its explorer row
/// and the console show them.
pub(crate) fn totals_lines(summary: &PrismoidGridSummary) -> Vec<String> {
    let left_out = |total: &NumericTotal| match total.missing + total.unusable_weights {
        0 => String::new(),
        count => tr!("explorer-prismoid-grid-left-out", count = count.to_string()),
    };
    let volume = format_quantity(summary.volume, 0);
    let first = match (&summary.tonnes, &summary.density) {
        (Some(tonnes), Some(density)) => match tonnes.value(ReserveAggregation::Sum) {
            Some(sum) => tr!(
                "explorer-prismoid-grid-volume-tonnes",
                volume = volume,
                tonnes = format_quantity(sum, 0),
                density = density.clone()
            ),
            None => tr!("explorer-prismoid-grid-volume-no-density", volume = volume, density = density.clone()),
        },
        _ => tr!("explorer-prismoid-grid-volume-only", volume = volume),
    };
    let mut lines = vec![format!("{first}{}", summary.tonnes.as_ref().map(left_out).unwrap_or_default())];
    for quality in &summary.qualities {
        let (total, name) = (&quality.total, quality.name.clone());
        // `value` reads sum over weight for either average.
        let line = match (total.value(ReserveAggregation::VolumeAverage), &summary.density) {
            (Some(mean), _) => {
                let mean = format_quantity(mean, 2);
                let value = if quality.unit.is_empty() { mean } else { format!("{mean} {}", quality.unit) };
                match summary.weighting {
                    Weighting::ByTonnes => tr!("explorer-prismoid-grid-quality-by-tonnes", quality = name, value = value),
                    Weighting::ByVolume => tr!("explorer-prismoid-grid-quality-by-volume", quality = name, value = value),
                }
            }
            (None, Some(density)) if total.unusable_weights > 0 => tr!("explorer-prismoid-grid-quality-no-density", quality = name, density = density.clone()),
            (None, _) => tr!("explorer-prismoid-grid-quality-no-values", quality = name),
        };
        lines.push(format!("{line}{}", left_out(total)));
    }
    lines
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
