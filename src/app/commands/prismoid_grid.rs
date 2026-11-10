//! Prismoid grid commands: building one from two surfaces, adding qualities
//! to one, making a surface from one, removing one from the explorer, and
//! preparing what the view draws of each.

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
use glam::{DVec2, DVec3};

use crate::{
    app::{
        App,
        commands::{
            raster::MAX_RASTER_PREVIEW_DIMENSION,
            solids::preview_triangulation,
            solids_view::next_view_id,
            thickness_points::not_a_grid_reason,
            triangulation::{
                session::{build_generated_triangulation, refuse_quality},
                solid_between::ring_contains,
            },
        },
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        Command, FolderId, ItemRef, ItemStyle, MemberKind, Object, ObjectId, OpenItem, ReserveAggregation, SceneEntityId, SectionKind,
        drill_hole::DrillColorPreset,
        formats::mesh_data::{self, Triangulation},
        grid_surface::GridSurface,
        plot::format_quantity,
        prismoid_grid::{
            AddedValues, BuildRefusal, CellReading, CellValueRole, CellValues, DEFAULT_PRISMOID_GRID_COLOR, DifferenceRefusal, DifferenceReport, OpenPrismoidGrid, PrismoidBuild,
            PrismoidColorBy, PrismoidGrid, PrismoidGridId, PrismoidGridSummary, QualitySurface, Quantity, SectionCrossing, SectionPoint, Stratum, ValuesRefusal, Weighting,
            build_strata, check_difference_name, ramp_position,
        },
        progress::{Phase, Progress},
        project::{ModelEnvelope, ProjectItemState, unique_item_name},
        raster::{OpenRasterTexture, RasterTextureId},
        rbf::NODE_BUDGET,
        solid_reserves::NumericTotal,
        spatial::TriangleBvh,
        triangulation::{GeneratedTriangulationLog, OpenTriangulation, QualityMark, TriangulationId, same_quality_name, spatial_surface_face_order, unique_edges, unit_label},
    },
    rendering::{
        camera::SectionSlab,
        color::linear_to_srgb_byte,
        graphics::{Graphics, camera::ScenePick},
        scene::{
            drill_hole_cache::evaluate_stops,
            section_surface_lines::{SECTION_FILL_DEPTH_TIE_M, SectionCellsOfGrid},
        },
    },
    ui::state::{CellSelection, EditorState, InspectorRow, InspectorRows, SelectionMode, TriPolylineClipMode, TriSurfaceType, Workspace},
    userspace_error, userspace_log, userspace_warn,
};

/// The dark line round a drawn grid's roof and floor.
pub(crate) const PRISMOID_OUTLINE_COLOR: [f32; 4] = [0.05, 0.06, 0.08, 1.0];

/// Counts cell selection changes over the session; each keys its outline.
static NEXT_CELL_SELECTION_REVISION: AtomicU64 = AtomicU64::new(1);

/// Counts down from the top, clear of the project's rasters: the raster
/// cache uploads an id's pixels once, so every map built takes a new one.
static NEXT_PRISMOID_MAP_ID: AtomicU64 = AtomicU64::new(u64::MAX);

/// A surface's name, mesh, index and section, taken to build from.
type PrismoidSource = (String, Arc<Triangulation>, Arc<TriangleBvh>, SectionKind);

/// A prismoid grid as the view draws it, for as long as `source` stands.
pub(crate) struct DrawnPrismoidGrid {
    source: Arc<PrismoidGrid>,
    surface: OpenTriangulation,
    /// `None` on a tensor lattice or when the chosen value has no range,
    /// which draw in the grid's flat colour.
    map: Option<OpenRasterTexture>,
    /// What `map` was made from.
    color_by: PrismoidColorBy,
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
        let (roof_name, roof_mesh, roof_spatial, roof_section) = self.prismoid_source(roof_id)?;
        let (floor_name, floor_mesh, floor_spatial, floor_section) = self.prismoid_source(floor_id)?;
        let section = SectionKind::derived_for(MemberKind::PrismoidGrid, [roof_section, floor_section]);
        let envelope = self.workspace.active_project().and_then(|project| project.project.metadata.modelling.envelope);
        let (roof_label, floor_label, grid_name) = (roof_name.clone(), floor_name.clone(), name.clone());
        // An Err is a refusal; an Ok(Err) is a built grid failing its own
        // check, a fault in the build rather than in the surfaces.
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<Result<PrismoidBuild>> {
            let roof = read_grid_surface(&roof_name, roof_mesh, roof_spatial)?;
            let floor = read_grid_surface(&floor_name, floor_mesh, floor_spatial)?;
            match PrismoidGrid::between(&roof, &floor, step, envelope.as_ref(), cancel, &progress.phase(0.0, 1.0)) {
                Ok(built) => Ok(Ok(built)),
                Err(BuildRefusal::Malformed(error)) => Ok(Err(error.context(grid_name))),
                Err(refusal) => Err(refusal_words(refusal, &roof_name, &floor_name, roof.spacing() * step.get() as f64, envelope.as_ref())),
            }
        };
        let apply = move |app: &mut App, result: Result<Result<PrismoidBuild>>| match result {
            Err(refusal) => userspace_warn!("{refusal:#}"),
            Ok(Err(fault)) => userspace_error!("{fault:#}"),
            Ok(Ok(PrismoidBuild { grid, crossed, holes })) => {
                let id = app.insert_prismoid_grid(name, grid, section);
                if let Some(item) = app.prismoid_grids.iter().find(|item| item.id == id) {
                    log_built_prismoid_grid(item, crossed, holes, &roof_label, &floor_label);
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

    /// Build a prismoid grid for every interval between neighbouring
    /// `horizons`, each a surface and its name, listed top first, into a new
    /// folder named after the strata. An interval with no thickness anywhere
    /// is skipped; any other refused interval refuses the strata.
    pub(crate) fn build_strata(&mut self, horizons: Vec<(TriangulationId, String)>, intervals: Vec<String>, step: NonZeroUsize, name: String) -> Result<()> {
        let names: Vec<String> = horizons.iter().map(|(_, horizon)| horizon.clone()).collect();
        let stated = Stratum {
            strata: name,
            index: 0,
            horizons: names,
            intervals,
        };
        let named_once = stated.horizons.iter().enumerate().all(|(index, horizon)| !stated.horizons[..index].contains(horizon));
        if !stated.is_whole() || !named_once {
            anyhow::bail!("{}", tr!("tri-strata-list-help"));
        }
        if horizons
            .iter()
            .enumerate()
            .any(|(index, (id, _))| horizons[..index].iter().any(|(earlier, _)| earlier == id))
        {
            anyhow::bail!("{}", tr!("cmd-prismoid-grid-strata-same-surface"));
        }
        let sources = horizons.iter().map(|(id, _)| self.prismoid_source(*id)).collect::<Result<Vec<_>>>()?;
        let section = SectionKind::derived_for(MemberKind::PrismoidGrid, sources.iter().map(|source| source.3));
        let envelope = self.workspace.active_project().and_then(|project| project.project.metadata.modelling.envelope);
        let surfaces: Vec<String> = sources.iter().map(|source| source.0.clone()).collect();
        let (labels, words) = (surfaces.clone(), stated.clone());
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<Result<Vec<Option<PrismoidBuild>>>> {
            let grids = sources
                .into_iter()
                .map(|(name, mesh, spatial, _)| read_grid_surface(&name, mesh, spatial))
                .collect::<Result<Vec<_>>>()?;
            let size = grids[0].spacing() * step.get() as f64;
            match build_strata(&grids, step, envelope.as_ref(), cancel, &progress.phase(0.0, 1.0)) {
                Ok(built) => Ok(Ok(built)),
                Err((index, BuildRefusal::Malformed(error))) => Ok(Err(error.context(words.intervals[index].clone()))),
                Err((_, BuildRefusal::Cancelled)) => anyhow::bail!("{}", tr!("common-cancelled")),
                Err((index, refusal)) => {
                    let reason = refusal_words(refusal, &surfaces[index], &surfaces[index + 1], size, envelope.as_ref());
                    anyhow::bail!(
                        "{}",
                        tr!(
                            "cmd-prismoid-grid-strata-refused",
                            name = words.strata.clone(),
                            interval = words.intervals[index].clone(),
                            reason = format!("{reason:#}")
                        )
                    )
                }
            }
        };
        let apply = move |app: &mut App, result: Result<Result<Vec<Option<PrismoidBuild>>>>| {
            let built = match result {
                Ok(Ok(built)) => built,
                Ok(Err(fault)) => {
                    userspace_error!("{fault:#}");
                    return;
                }
                Err(refusal) => {
                    userspace_warn!("{refusal:#}");
                    return;
                }
            };
            let count = built.len();
            for (index, _) in built.iter().enumerate().filter(|(_, build)| build.is_none()) {
                userspace_log!(
                    "{}",
                    tr!(
                        "cmd-prismoid-grid-strata-skipped",
                        interval = stated.intervals[index].clone(),
                        roof = labels[index].clone(),
                        floor = labels[index + 1].clone()
                    )
                );
            }
            if built.iter().all(Option::is_none) {
                userspace_warn!("{}", tr!("cmd-prismoid-grid-strata-none-built", name = stated.strata.clone()));
                return;
            }
            // A strata is never rebuilt in place, so each build has a folder
            // of its own.
            let Some(project) = app.workspace.active_project_mut() else {
                return;
            };
            let folders = &mut project.project.folders;
            let strata = unique_item_name(stated.strata.clone(), folders.names(section).into_iter());
            let folder = folders.add(section, strata.clone());
            let stratum = Stratum { strata, ..stated };
            app.editor.selected_handles.clear();
            let mut made = 0;
            for (index, build) in built.into_iter().enumerate() {
                let Some(PrismoidBuild { grid, crossed, holes }) = build else {
                    continue;
                };
                let record = Stratum { index, ..stratum.clone() };
                let id = app.insert_prismoid_grid_in(stratum.intervals[index].clone(), grid, section, folder, Some(record));
                app.editor.selected_handles.insert(SceneEntityId::PrismoidGrid(id));
                made += 1;
                if let Some(item) = app.prismoid_grids.iter().find(|item| item.id == id) {
                    log_built_prismoid_grid(item, crossed, holes, &labels[index], &labels[index + 1]);
                }
                if crossed > 0 && index > 0 && index + 1 < count {
                    userspace_warn!(
                        "{}",
                        tr!(
                            "cmd-prismoid-grid-strata-overlap",
                            floor = labels[index + 1].clone(),
                            roof = labels[index].clone(),
                            interval = stratum.intervals[index].clone()
                        )
                    );
                }
            }
            userspace_log!(
                "{}",
                tr!(
                    "cmd-prismoid-grid-strata-built",
                    name = stratum.strata.clone(),
                    built = made.to_string(),
                    count = count.to_string()
                )
            );
        };
        let keys = horizons.iter().map(|(id, _)| JobKey::Triangulation(*id)).collect();
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-strata-building"), keys, compute, apply);
        Ok(())
    }

    /// A loaded surface's name, mesh, index and section, to build a prismoid
    /// grid from; a quality surface is refused.
    fn prismoid_source(&self, id: TriangulationId) -> Result<PrismoidSource> {
        let item = self
            .triangulations
            .iter()
            .find(|item| item.id == id && item.state.loaded)
            .ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-thickness-points-surface-gone")))?;
        refuse_quality(&item.name, item.quality.as_ref())?;
        Ok((item.name.clone(), Arc::clone(&item.mesh), Arc::clone(&item.spatial), item.state.section))
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
            let result = result.map(|(merged, lines)| {
                let added = tr!("cmd-prismoid-grid-qualities-added", count = lines.len().to_string(), grid = name.clone());
                (merged, std::iter::once(added).chain(lines).collect())
            });
            app.replace_prismoid_values(runtime_id, grid_id, &name, &source, result);
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-adding-qualities"), keys, compute, apply);
        Ok(())
    }

    /// Put a grid's copy with values added in place as one undoable step and
    /// log `lines`, its totals and the next step; a refusal is warned, and
    /// nothing is replaced when the grid is gone or moved on from `source`.
    fn replace_prismoid_values(&mut self, runtime_id: u32, grid_id: PrismoidGridId, name: &str, source: &Arc<PrismoidGrid>, result: Result<(PrismoidGrid, Vec<String>)>) {
        let (merged, lines) = match result {
            Ok(added) => added,
            Err(refusal) => {
                userspace_warn!("{refusal:#}");
                return;
            }
        };
        let current = self.workspace.active_project().is_some_and(|project| project.runtime_id == runtime_id);
        let Some(item) = self.prismoid_grids.iter().find(|item| item.id == grid_id).filter(|_| current) else {
            userspace_warn!("{}", tr!("cmd-prismoid-grid-qualities-discarded", grid = name.to_owned()));
            return;
        };
        if !Arc::ptr_eq(&item.grid, source) {
            userspace_warn!("{}", tr!("cmd-prismoid-grid-changed-while-reading", grid = item.name.clone()));
            return;
        }
        let mut replacement = item.clone();
        replacement.summary = merged.summary();
        let totals = totals_lines(&replacement.summary);
        replacement.grid = Arc::new(merged);
        self.execute_edit_for(
            runtime_id,
            Command::ReplaceItem {
                item: ItemRef::PrismoidGrid(grid_id),
                other: Some(OpenItem::PrismoidGrid(Box::new(replacement))),
            },
        );
        for line in lines.into_iter().chain(totals) {
            userspace_log!("{line}");
        }
        userspace_log!("{}", tr!("cmd-prismoid-grid-qualities-rederive"));
    }

    /// Compare two loaded prismoid grids cell by cell, this grid less the
    /// other, by `quantity` (a value both hold, or vertical thickness with
    /// `None`), and keep the difference on this grid as a value named `name`,
    /// as one undoable step. The lattice test runs on the worker.
    pub(crate) fn difference_prismoid_grids(&mut self, grid_id: PrismoidGridId, other_id: PrismoidGridId, quantity: Option<String>, name: String) -> Result<()> {
        if grid_id == other_id {
            anyhow::bail!("{}", tr!("cmd-prismoid-grid-difference-same"));
        }
        let gone = || anyhow::anyhow!("{}", tr!("ws-menubar-modelling-select-prismoid-grid"));
        let runtime_id = self.workspace.active_project().map(|project| project.runtime_id).ok_or_else(gone)?;
        let loaded = |id: PrismoidGridId| {
            self.prismoid_grids
                .iter()
                .find(|item| item.id == id && item.state.loaded)
                .map(|item| (Arc::clone(&item.grid), item.name.clone()))
                .ok_or_else(gone)
        };
        let (source, grid_name) = loaded(grid_id)?;
        let (other, other_name) = loaded(other_id)?;
        let held = |grid: &PrismoidGrid, value: &str| {
            grid.cell_values
                .iter()
                .find(|values| same_quality_name(&values.name, value))
                .map(|values| values.unit.clone())
        };
        let (quantity, label, unit, other_unit) = match quantity {
            None => (Quantity::VerticalThickness, tr!("thickness-points-column-vertical"), "m".to_owned(), None),
            Some(value) => {
                let (Some(unit), Some(other_unit)) = (held(&source, &value), held(&other, &value)) else {
                    return Err(no_difference(&grid_name, &other_name, &value));
                };
                (Quantity::Value(value.clone()), value, unit, Some(other_unit))
            }
        };
        // A name the grid keeps, or its density's, is refused before the
        // work; any other held name is replaced, and the line says so.
        let names: Vec<_> = source.cell_values.iter().map(|values| (values.name.as_str(), values.role)).collect();
        check_difference_name(&names, &other_name, &name).map_err(|refusal| anyhow::anyhow!("{}", values_refusal_words(refusal, &grid_name)))?;
        let replaces = names.iter().any(|(held, _)| same_quality_name(held, &name));
        let keys = vec![JobKey::PrismoidGrid(grid_id), JobKey::PrismoidGrid(other_id)];
        let (grid, name_a, name_b) = (Arc::clone(&source), grid_name.clone(), other_name.clone());
        let other_read = Arc::clone(&other);
        let compute = move |cancel: &CancelFlag, _: &Progress| -> Result<(PrismoidGrid, Vec<String>)> {
            let compared = grid
                .difference(&other, &quantity, name.clone(), unit.clone(), cancel)
                .map_err(|refusal| difference_refusal_words(refusal, &grid, &other, &name_a, &name_b))?;
            let (values, report) = compared.ok_or_else(|| no_difference(&name_a, &name_b, &label))?;
            let lines = difference_lines(&name, &name_a, &name_b, &unit, other_unit.as_deref(), replaces, &report);
            let merged = grid
                .with_values(vec![AddedValues { surface: name_b.clone(), values }])
                .map_err(|refusal| anyhow::anyhow!("{}", values_refusal_words(refusal, &name_a)))?;
            Ok((merged, lines))
        };
        let apply = move |app: &mut App, result: Result<(PrismoidGrid, Vec<String>)>| {
            if result.is_ok()
                && let Some(changed) = other_grid_changed(&app.prismoid_grids, other_id, &other_read, &other_name)
            {
                userspace_warn!("{changed}");
                return;
            }
            app.replace_prismoid_values(runtime_id, grid_id, &grid_name, &source, result);
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-differencing"), keys, compute, apply);
        Ok(())
    }

    /// Make a surface whose height is a prismoid grid's vertical thickness,
    /// or one of its values named by `value`, marked as that quality and so
    /// inserted hidden, for Generate Contour Lines to contour.
    pub(crate) fn prismoid_grid_to_surface(&mut self, grid_id: PrismoidGridId, value: Option<String>, name: String) -> Result<()> {
        let item = self
            .prismoid_grids
            .iter()
            .find(|item| item.id == grid_id && item.state.loaded)
            .ok_or_else(|| anyhow::anyhow!("{}", tr!("ws-menubar-modelling-select-prismoid-grid")))?;
        let (grid, grid_name) = (Arc::clone(&item.grid), item.name.clone());
        let index = match value.as_deref() {
            None => None,
            Some(value) => Some(
                grid.cell_values
                    .iter()
                    .position(|held| same_quality_name(&held.name, value))
                    .ok_or_else(|| no_value_surface(&grid_name, value))?,
            ),
        };
        let mark = surface_mark(index.map(|index| &grid.cell_values[index]));
        let (quantity, label) = (mark.name.clone(), name.clone());
        let contoured = quantity.clone();
        let compute = move |cancel: &CancelFlag, _: &Progress| -> Result<GeneratedTriangulationLog> {
            if cancel.is_cancelled() {
                anyhow::bail!("{}", tr!("common-cancelled"));
            }
            let (vertices, faces) = grid.value_mesh(index).ok_or_else(|| no_value_surface(&grid_name, &quantity))?;
            let nodes = vertices.len();
            let generated = build_generated_triangulation(name, vertices, faces, TriSurfaceType::Surface, unique_edges)?.with_quality(Some(mark));
            let message = tr!(
                "cmd-prismoid-grid-surface-made",
                name = label,
                grid = grid_name,
                quantity = quantity,
                nodes = nodes.to_string()
            );
            Ok(GeneratedTriangulationLog { generated, message })
        };
        let apply = move |app: &mut App, result: Result<GeneratedTriangulationLog>| {
            let made = result.is_ok();
            app.apply_generated_triangulation_job(result, &[]);
            // Contour Lines reads z, so the lines of this surface are values;
            // said here, as the contour tool does not carry the mark.
            if made {
                userspace_log!("{}", tr!("cmd-prismoid-grid-surface-contours-are-values", quantity = contoured));
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-making-surface"), vec![JobKey::PrismoidGrid(grid_id)], compute, apply);
        Ok(())
    }

    /// Mask the selected prismoids by hand as one undoable step, or unmask
    /// them when every one is masked by hand; a mixed selection masks its
    /// active cells.
    pub(crate) fn toggle_prismoid_mask(&mut self) {
        let Some(runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        let Some(selection) = &self.editor.cell_selection else {
            return;
        };
        let SceneEntityId::PrismoidGrid(id) = selection.entity else {
            return;
        };
        let mask = selection.masked < selection.count;
        let cells: Vec<usize> = selection.cells.iter().enumerate().filter(|(_, selected)| **selected).map(|(cell, _)| cell).collect();
        let (source, name) = (Arc::clone(&selection.grid), selection.name.clone());
        self.spawn_mask_edit(runtime_id, id, name, source, mask, move |grid, _, _| Ok(mask_cells(grid, cells, mask)));
    }

    /// Run a mask edit's `compute` on the job queue keyed by the grid, as
    /// the grid is copied whole however few cells change, and put its result
    /// in place with [`Self::replace_prismoid_mask`].
    fn spawn_mask_edit(
        &mut self,
        runtime_id: u32,
        grid_id: PrismoidGridId,
        name: String,
        source: Arc<PrismoidGrid>,
        mask: bool,
        compute: impl FnOnce(&PrismoidGrid, &CancelFlag, &Progress) -> Result<Option<MaskedGrid>> + Send + 'static,
    ) {
        let grid = Arc::clone(&source);
        let compute = move |cancel: &CancelFlag, progress: &Progress| compute(&grid, cancel, progress);
        let apply = move |app: &mut App, result: Result<Option<MaskedGrid>>| match result {
            Ok(changed) => app.replace_prismoid_mask(runtime_id, grid_id, &name, &source, changed, mask),
            Err(refusal) => {
                userspace_warn!("{refusal:#}");
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-masking"), vec![JobKey::PrismoidGrid(grid_id)], compute, apply);
    }

    /// Put a grid's masked or unmasked copy in place as one undoable step,
    /// unless the grid is gone or moved on from `source`, nothing changed, or
    /// no active cell would be left. A cell selection on `source` follows to
    /// the copy, less the cells no longer active.
    fn replace_prismoid_mask(&mut self, runtime_id: u32, grid_id: PrismoidGridId, name: &str, source: &Arc<PrismoidGrid>, changed: Option<MaskedGrid>, mask: bool) {
        let current = self.workspace.active_project().is_some_and(|project| project.runtime_id == runtime_id);
        let item = match masked_item(&self.prismoid_grids, current, grid_id, source, name) {
            Ok(item) => item,
            Err(refusal) => {
                userspace_warn!("{refusal}");
                return;
            }
        };
        let grid_name = item.name.clone();
        let Some((grid, summary, count)) = changed else {
            userspace_log!("{}", tr!("cmd-prismoid-grid-mask-nothing", grid = grid_name));
            return;
        };
        let active = summary.active_cells;
        if active == 0 {
            userspace_warn!("{}", tr!("cmd-prismoid-grid-mask-leaves-none", grid = grid_name));
            return;
        }
        let mut replacement = item.clone();
        replacement.summary = summary;
        let totals = totals_lines(&replacement.summary);
        let grid = Arc::new(grid);
        replacement.grid = Arc::clone(&grid);
        self.execute_edit_for(
            runtime_id,
            Command::ReplaceItem {
                item: ItemRef::PrismoidGrid(grid_id),
                other: Some(OpenItem::PrismoidGrid(Box::new(replacement))),
            },
        );
        let entity = SceneEntityId::PrismoidGrid(grid_id);
        if let Some(selection) = self
            .editor
            .cell_selection
            .take_if(|selection| selection.entity == entity && Arc::ptr_eq(&selection.grid, source))
        {
            self.editor.cell_selection = carry_cell_selection(&selection, grid);
        }
        let (count, active) = (count.to_string(), active.to_string());
        if mask {
            userspace_log!("{}", tr!("cmd-prismoid-grid-masked", count = count, grid = grid_name, active = active));
        } else {
            userspace_log!("{}", tr!("cmd-prismoid-grid-unmasked", count = count, grid = grid_name, active = active));
        }
        for line in totals {
            userspace_log!("{line}");
        }
        userspace_log!("{}", tr!("cmd-prismoid-grid-mask-rederive"));
    }

    /// Mask by hand, on the job queue and as one undoable step, every active
    /// prismoid whose centre lies on the side of a closed polyline that
    /// `mode` does not keep.
    pub(crate) fn mask_prismoid_grid_by_polyline(&mut self, grid_id: PrismoidGridId, polyline_id: ObjectId, mode: TriPolylineClipMode) -> Result<()> {
        let gone = || anyhow::anyhow!("{}", tr!("cmd-commands-select-one-prismoid-grid-polyline"));
        let runtime_id = self.workspace.active_project().map(|project| project.runtime_id).ok_or_else(gone)?;
        let item = self.prismoid_grids.iter().find(|item| item.id == grid_id && item.state.loaded).ok_or_else(gone)?;
        let (source, name) = (Arc::clone(&item.grid), item.name.clone());
        let ring = mask_ring(self.scene_document.get_object(polyline_id).and_then(Object::closed_boundary))?;
        self.spawn_mask_edit(runtime_id, grid_id, name, source, true, move |grid, cancel, progress| {
            let cells = cells_by_ring(grid, &ring, mode, cancel, &progress.phase(0.0, 1.0)).ok_or_else(|| anyhow::anyhow!("{}", tr!("common-cancelled")))?;
            Ok(mask_cells(grid, cells, true))
        });
        Ok(())
    }

    /// Add a built prismoid grid to the project, dirty, at its section's
    /// root, and select it.
    pub(crate) fn insert_prismoid_grid(&mut self, name: String, grid: PrismoidGrid, section: SectionKind) -> PrismoidGridId {
        let id = self.insert_prismoid_grid_in(name, grid, section, None, None);
        self.editor.selected_handles.clear();
        self.editor.selected_handles.insert(SceneEntityId::PrismoidGrid(id));
        id
    }

    /// Add a built prismoid grid to the project, dirty, in `folder` of its
    /// section, leaving the selection to the caller.
    pub(crate) fn insert_prismoid_grid_in(&mut self, name: String, grid: PrismoidGrid, section: SectionKind, folder: Option<FolderId>, stratum: Option<Stratum>) -> PrismoidGridId {
        let id = PrismoidGridId(self.next_prismoid_grid_id);
        self.next_prismoid_grid_id += 1;
        let name = unique_item_name(name, self.prismoid_grids.iter().map(|item| item.name.as_str()));
        self.prismoid_grids.push(OpenPrismoidGrid {
            id,
            state: ProjectItemState::dirty(MemberKind::PrismoidGrid, None).with_section(section).with_folder(folder),
            name,
            color: DEFAULT_PRISMOID_GRID_COLOR,
            summary: grid.summary(),
            grid: Arc::new(grid),
            color_by: PrismoidColorBy::default(),
            stratum,
        });
        self.touch_active_project_content();
        self.invalidate_geometry();
        id
    }

    /// Change what a grid is coloured by, as one undoable style step; an
    /// unchanged choice records nothing.
    pub(crate) fn set_prismoid_grid_color_by(&mut self, id: PrismoidGridId, change: impl FnOnce(&mut PrismoidColorBy)) {
        let item = ItemRef::PrismoidGrid(id);
        let Some(ItemStyle::PrismoidGrid { loaded, hidden, mut color_by }) = self.item_style(item) else {
            return;
        };
        change(&mut color_by);
        self.set_item_style(item, ItemStyle::PrismoidGrid { loaded, hidden, color_by });
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
            let grids = &self.prismoid_grids;
            let recolour: Vec<_> = self
                .prismoid_drawn
                .iter()
                .filter_map(|drawn| {
                    let id = drawn.surface.prismoid_grid?;
                    grids.iter().any(|item| item.id == id && item.color_by != drawn.color_by).then_some(id)
                })
                .filter(|id| !self.job_pending(&JobKey::PrismoidGrid(*id)))
                .collect();
            for id in recolour {
                self.spawn_prismoid_map(id);
            }
        }
        self.refresh_prismoid_scene();
    }

    /// Pick the prismoid a click on a drawn grid landed on, by Leo's
    /// selection modes one grain down: the grid stays selected while any of
    /// its cells is. False when the point is on no active cell.
    pub(crate) fn pick_prismoid_cell(&mut self, id: PrismoidGridId, world: DVec3) -> bool {
        let entity = SceneEntityId::PrismoidGrid(id);
        let Some((grid, name, stratum)) = self
            .prismoid_grids
            .iter()
            .find(|item| item.id == id && item.state.loaded)
            .map(|item| (Arc::clone(&item.grid), item.name.clone(), item.stratum.clone()))
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
        self.editor.cell_selection = cell_selection(entity, grid, &name, stratum, cells, count);
        true
    }

    /// The prismoid under a 2D section's plane point `world`, as a pick on
    /// its grid: of the drawn grids, shown and not frozen, the first in
    /// explorer order with a cell there between the floor and the roof.
    pub(crate) fn section_cell_pick(&self, world: DVec3) -> Option<ScenePick> {
        if self.editor.active_workspace != Workspace::Geology {
            return None;
        }
        section_grids(&self.editor, &self.prismoid_grids, &self.prismoid_drawn)
            .filter(|item| !self.editor.frozen_handles.contains(&item.entity_id()))
            .find(|item| item.grid.section_cell_at(world).is_some())
            .map(|item| ScenePick {
                entity: item.entity_id(),
                world,
                hole: None,
            })
    }

    /// What a click in a 2D section picks, from Leo's ray `pick`: the
    /// prismoid under the plane point when `accepts` takes it and nothing
    /// drawn in front of its fill was hit. Both mouse buttons pick here.
    pub(crate) fn with_section_cell_pick(&self, graphics: &Graphics<'_>, pick: Option<ScenePick>, accepts: impl Fn(&ScenePick) -> bool) -> Option<ScenePick> {
        match graphics.section_slab().filter(|slab| slab.outlines) {
            Some(slab) if section_cell_wins(pick.as_ref(), &slab, graphics.camera_orientation().0) => graphics
                .cursor_world(self.editor.z_level)
                .and_then(|world| self.section_cell_pick(world))
                .filter(|pick| accepts(pick))
                .or(pick.filter(|pick| !matches!(pick.entity, SceneEntityId::PrismoidGrid(_)))),
            _ => pick,
        }
    }

    /// Apply a marquee to the cell selection after Leo's box has applied its
    /// own, in its modifier order: shift toggles each cell taken, ctrl adds,
    /// a plain box replaces. The grid is selected while any of its cells is.
    pub(crate) fn select_prismoid_cells_in_box(&mut self, start: (f32, f32), end: (f32, f32)) {
        let (toggle, add) = (self.modifiers.shift_key(), self.modifiers.control_key());
        let Some((id, taken)) = self.prismoid_cells_in_screen_rect(start, end, toggle || add) else {
            return;
        };
        let Some((grid, name, stratum)) = self
            .prismoid_grids
            .iter()
            .find(|item| item.id == id)
            .map(|item| (Arc::clone(&item.grid), item.name.clone(), item.stratum.clone()))
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
        self.editor.cell_selection = cell_selection(entity, grid, &name, stratum, cells, count);
    }

    /// The cells a marquee takes, as Lucas's dig blocks ride Leo's box: of
    /// one drawn grid, shown and not frozen, each active cell whose box
    /// centre falls inside, or with none the cells masked by hand there,
    /// kept to the section's slab when one is up; in a 2D section each
    /// crossed cell by the middle of its crossing. With
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
            let project = |world| graphics.world_to_window_px(&view_proj, world);
            // A 2D section takes the cells it draws, a 3D slab those whose
            // centres it holds.
            let cells = match slab.filter(|slab| slab.outlines) {
                Some(section) => centres_in_screen_rect(crossing_centres(&item.grid, &section), project, slab.as_ref(), start, end),
                None => box_cells(&item.grid, project, slab.as_ref(), start, end),
            };
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
        let (grid, name, color, color_by) = (Arc::clone(&item.grid), item.name.clone(), item.color, item.color_by.clone());
        let (index, range) = map_inputs(item);
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
            let map = prismoid_map(&grid, &name, index, range, color_by.preset);
            let mut surface = preview_triangulation(name, Arc::new(mesh), spatial, closed.outline, order, color, PRISMOID_OUTLINE_COLOR);
            surface.id = next_view_id();
            surface.raster_texture = map.as_ref().map(|raster| raster.id);
            surface.always_show_edges = true;
            surface.prismoid_grid = Some(id);
            phase.finish();
            Ok(DrawnPrismoidGrid {
                source: grid,
                surface,
                map,
                color_by,
            })
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

    /// Rebuild only a drawn grid's colours, for a new choice of what it is
    /// coloured by. A result that no longer matches the grid is dropped; the
    /// next frame asks again.
    fn spawn_prismoid_map(&mut self, id: PrismoidGridId) {
        let Some(item) = self.prismoid_grids.iter().find(|item| item.id == id) else {
            return;
        };
        let (grid, name, color_by) = (Arc::clone(&item.grid), item.name.clone(), item.color_by.clone());
        let (index, range) = map_inputs(item);
        let compute = move |_: &CancelFlag| -> Result<(Arc<PrismoidGrid>, PrismoidColorBy, Option<OpenRasterTexture>)> {
            let map = prismoid_map(&grid, &name, index, range, color_by.preset);
            Ok((grid, color_by, map))
        };
        let apply = move |app: &mut App, result: Result<(Arc<PrismoidGrid>, PrismoidColorBy, Option<OpenRasterTexture>)>| {
            let Ok((grid, color_by, map)) = result else {
                return;
            };
            let current = app
                .prismoid_grids
                .iter()
                .any(|item| item.id == id && Arc::ptr_eq(&item.grid, &grid) && item.color_by == color_by);
            let Some(drawn) = app
                .prismoid_drawn
                .iter_mut()
                .find(|drawn| drawn.surface.prismoid_grid == Some(id) && Arc::ptr_eq(&drawn.source, &grid))
                .filter(|_| current)
            else {
                return;
            };
            drawn.surface.raster_texture = map.as_ref().map(|raster| raster.id);
            drawn.map = map;
            drawn.color_by = color_by;
            app.refresh_prismoid_scene();
            app.invalidate_topology_bounds_and_redraw();
        };
        self.spawn_job_quietly(tr!("cmd-prismoid-grid-preparing"), vec![JobKey::PrismoidGrid(id)], compute, apply);
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
            drawn.map.as_ref().map(|map| map.id).hash(&mut hasher);
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
        let maps = self.prismoid_drawn.iter().filter_map(|drawn| drawn.map.as_ref());
        self.prismoid_rasters = self.raster_textures.iter().chain(maps).cloned().collect();
        self.prismoid_scene_key = Some(key);
    }

    /// Cut the drawn grids along a 2D section again, only when the plane, a
    /// grid, its colours or the cell selection moved since the last cut: a
    /// still section costs a hash a frame.
    pub(crate) fn refresh_section_prismoid_cells(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let slab = graphics.section_slab();
        let key = section_cells_key(&self.editor, slab, &self.prismoid_grids, &self.prismoid_drawn);
        if key == self.section_cells_key {
            return;
        }
        self.section_cells_key = key;
        let mut cut = Vec::new();
        if let (Some(_), Some(slab)) = (key, slab) {
            for item in section_grids(&self.editor, &self.prismoid_grids, &self.prismoid_drawn) {
                let crossings = item.grid.section_crossings(slab.point.truncate(), slab.normal.truncate());
                if crossings.is_empty() {
                    continue;
                }
                let held = self
                    .editor
                    .cell_selection
                    .as_ref()
                    .filter(|selection| selection.entity == item.entity_id() && Arc::ptr_eq(&selection.grid, &item.grid));
                cut.push(SectionCellsOfGrid {
                    colors: crossings.iter().map(|crossing| section_colors(item, crossing)).collect(),
                    selected: crossings.iter().map(|crossing| held.is_some_and(|selection| selection.cells[crossing.cell])).collect(),
                    crossings,
                });
            }
        }
        graphics.set_section_cells(cut);
    }
}

/// The grids a section shows cells of: loaded, not hidden, and drawn from
/// the grid they hold now.
fn section_grids<'g>(editor: &'g EditorState, grids: &'g [OpenPrismoidGrid], drawn: &'g [DrawnPrismoidGrid]) -> impl Iterator<Item = &'g OpenPrismoidGrid> + Clone {
    grids.iter().filter(move |item| {
        item.state.loaded
            && !editor.hidden_handles.contains(&item.entity_id())
            && drawn
                .iter()
                .any(|drawn| drawn.surface.prismoid_grid == Some(item.id) && Arc::ptr_eq(&drawn.source, &item.grid))
    })
}

/// What the section's prismoid cells are cut and coloured from: the plane,
/// then each shown grid's id, grid, colour choice and flat colour, and the
/// revision of a cell selection on it. The slab's width is not in it, as the
/// cut is on the plane. `None` with no cells to draw: off Geology, no 2D
/// section, or no grid drawn.
fn section_cells_key(editor: &EditorState, slab: Option<SectionSlab>, grids: &[OpenPrismoidGrid], drawn: &[DrawnPrismoidGrid]) -> Option<u64> {
    if editor.active_workspace != Workspace::Geology {
        return None;
    }
    let slab = slab.filter(|slab| slab.outlines)?;
    let shown = section_grids(editor, grids, drawn);
    shown.clone().next()?;
    let mut hasher = DefaultHasher::new();
    crate::app::commands::slice::hash_section_plane(&slab, &mut hasher);
    for item in shown {
        item.id.hash(&mut hasher);
        (Arc::as_ptr(&item.grid) as usize).hash(&mut hasher);
        item.color_by.value.hash(&mut hasher);
        std::mem::discriminant(&item.color_by.preset).hash(&mut hasher);
        item.color.map(f32::to_bits).hash(&mut hasher);
        let selection = editor.cell_selection.as_ref().filter(|selection| selection.entity == item.entity_id());
        selection.map(|selection| selection.revision).hash(&mut hasher);
    }
    Some(hasher.finish())
}

/// One colour per point of a crossing, from the inputs the body's colours
/// are made from: a value gives the cell one colour, thickness each point its
/// own. The grid's own colour, opaque, where the body draws flat or the cell
/// has no value.
fn section_colors(item: &OpenPrismoidGrid, crossing: &SectionCrossing) -> Vec<[f32; 4]> {
    let [red, green, blue, _] = item.color;
    let flat = vec![[red, green, blue, 1.0]; crossing.points.len()];
    let (index, range) = map_inputs(item);
    let Some((min, max)) = range.filter(|_| item.grid.lattice.is_regular()) else {
        return flat;
    };
    let preset = item.color_by.preset;
    let stops = preset.stops();
    let ramp = |value: f64| {
        let [red, green, blue] = evaluate_stops(ramp_position(value, (min, max)), &stops, preset.smooth());
        [red, green, blue, 1.0]
    };
    match index {
        Some(index) => match item.grid.cell_values[index].values[crossing.cell].filter(|value| value.is_finite()) {
            Some(value) => vec![ramp(value); crossing.points.len()],
            None => flat,
        },
        None => crossing.points.iter().map(|point| ramp(point.roof - point.floor)).collect(),
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

/// A closed polyline's outline as the ring a mask is judged by; an open
/// string, a circle too small to bound a region or a deleted object is
/// refused.
fn mask_ring(boundary: Option<Vec<DVec3>>) -> Result<Vec<mesh_data::Vertex>> {
    let boundary = boundary.ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-commands-select-one-prismoid-grid-polyline")))?;
    Ok(boundary.into_iter().map(|point| mesh_data::Vertex::new(point.x, point.y, point.z)).collect())
}

/// The active cells `mode` masks against `ring`, each judged by its plan
/// centre: Keep inside masks those outside the ring, Keep outside those
/// inside. A centre on the ring takes the one answer `ring_contains` gives
/// it. `None` when cancelled.
fn cells_by_ring(grid: &PrismoidGrid, ring: &[mesh_data::Vertex], mode: TriPolylineClipMode, cancel: &CancelFlag, progress: &Phase) -> Option<Vec<usize>> {
    let (low, high) = ring.iter().fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(low, high), point| {
        let point = DVec2::new(point.x, point.y);
        (low.min(point), high.max(point))
    });
    let keep_inside = mode == TriPolylineClipMode::KeepInside;
    let cells = grid.lattice.cell_count();
    let mut masked = Vec::new();
    for (cell, centre) in grid.cell_centres() {
        if cell % 4096 == 0 {
            if cancel.is_cancelled() {
                return None;
            }
            progress.set_fraction(cell as f32 / cells as f32);
        }
        if !grid.active[cell] {
            continue;
        }
        let inside = centre.cmpge(low).all() && centre.cmple(high).all() && ring_contains(ring, mesh_data::Vertex::new(centre.x, centre.y, 0.0));
        if inside != keep_inside {
            masked.push(cell);
        }
    }
    Some(masked)
}

/// The cells a marquee takes off a grid in 3D: the active cells whose
/// centres project inside, or with none, the cells masked by hand there. A
/// masked cell is drawn as a hole, and a box over it is how it is picked.
fn box_cells(grid: &PrismoidGrid, project: impl Fn(DVec3) -> Option<(f32, f32)>, slab: Option<&SectionSlab>, start: (f32, f32), end: (f32, f32)) -> Vec<usize> {
    let active = centres_in_screen_rect(grid.cell_centres_3d(), &project, slab, start, end);
    if !active.is_empty() {
        return active;
    }
    centres_in_screen_rect(grid.centres_3d_of(&grid.masked), project, slab, start, end)
}

/// Each cell a 2D section crosses, by a point on the plane in the middle of
/// its crossing: halfway along it, halfway between the floor and the roof
/// there.
fn crossing_centres(grid: &PrismoidGrid, slab: &SectionSlab) -> impl Iterator<Item = (usize, DVec3)> {
    let (point, normal) = (slab.point.truncate(), slab.normal.truncate());
    let along = DVec2::new(-normal.y, normal.x);
    grid.section_crossings(point, normal).into_iter().filter_map(move |crossing| {
        let middle = (crossing.along[0] + crossing.along[1]) / 2.0;
        let at = |point_of: &SectionPoint| ((point_of.plan - point).dot(along), (point_of.roof + point_of.floor) / 2.0);
        // Rounding can leave the middle of a sliver past its last point: the
        // nearest point's height stands in.
        let height = crossing
            .points
            .windows(2)
            .map(|pair| (at(&pair[0]), at(&pair[1])))
            .find(|(_, (to, _))| middle <= *to)
            .map(|((from, low), (to, high))| low + (high - low) * ((middle - from) / (to - from).max(f64::MIN_POSITIVE)))
            .or_else(|| {
                crossing
                    .points
                    .iter()
                    .map(at)
                    .min_by(|(a, _), (b, _)| (a - middle).abs().total_cmp(&(b - middle).abs()))
                    .map(|(_, height)| height)
            })?;
        Some((crossing.cell, (point + along * middle).extend(height)))
    })
}

/// Whether, in a 2D section, a prismoid under the plane point takes a click
/// over Leo's `pick`: with nothing picked, a pick on a surface or a grid,
/// both drawn here only as cut lines, or a pick behind the cell's fill,
/// which sits [`SECTION_FILL_DEPTH_TIE_M`] behind the plane and hides it.
pub(crate) fn section_cell_wins(pick: Option<&ScenePick>, slab: &SectionSlab, forward: DVec3) -> bool {
    pick.is_none_or(|pick| {
        matches!(pick.entity, SceneEntityId::Triangulation(_) | SceneEntityId::PrismoidGrid(_))
            || slab.signed_distance(pick.world) * slab.normal.dot(forward) > SECTION_FILL_DEPTH_TIE_M
    })
}

/// A selection of `count` cells on the grid `name`, derived; `None` with
/// none.
fn cell_selection(entity: SceneEntityId, grid: Arc<PrismoidGrid>, name: &str, stratum: Option<Stratum>, cells: Vec<bool>, count: usize) -> Option<CellSelection> {
    (count > 0).then(|| {
        let mut selection = CellSelection {
            entity,
            grid,
            name: name.to_owned(),
            stratum,
            cells,
            count,
            masked: 0,
            revision: 0,
            outline: Vec::new(),
            rows: InspectorRows::default(),
        };
        refresh_cell_selection(&mut selection);
        selection
    })
}

/// A grid's masked or unmasked copy with its summary, both made off the UI
/// thread, and the count of cells changed.
type MaskedGrid = (PrismoidGrid, PrismoidGridSummary, usize);

/// [`PrismoidGrid::with_mask`] with the copy's summary; `None` when no cell
/// changed.
fn mask_cells(grid: &PrismoidGrid, cells: impl IntoIterator<Item = usize>, mask: bool) -> Option<MaskedGrid> {
    grid.with_mask(cells, mask).map(|(grid, count)| {
        let summary = grid.summary();
        (grid, summary, count)
    })
}

/// The grid a mask edit was made on, still in the active project (`current`)
/// and still `source`; else why the edit is dropped: the grid is gone or its
/// project is no longer active (another project's grid can carry the same
/// id), or the grid moved on while the edit was made.
fn masked_item<'g>(grids: &'g [OpenPrismoidGrid], current: bool, grid_id: PrismoidGridId, source: &Arc<PrismoidGrid>, name: &str) -> Result<&'g OpenPrismoidGrid, String> {
    let Some(item) = grids.iter().find(|item| item.id == grid_id).filter(|_| current) else {
        return Err(tr!("cmd-prismoid-grid-mask-discarded", grid = name.to_owned()));
    };
    if Arc::ptr_eq(&item.grid, source) {
        Ok(item)
    } else {
        Err(tr!("cmd-prismoid-grid-changed-while-masking", grid = item.name.clone()))
    }
}

/// A selection carried to its grid's masked or unmasked copy: the cells
/// still active stay selected, the rest leave; `None` with none left.
fn carry_cell_selection(selection: &CellSelection, grid: Arc<PrismoidGrid>) -> Option<CellSelection> {
    let cells: Vec<bool> = selection.cells.iter().zip(&grid.active).map(|(selected, active)| *selected && *active).collect();
    let count = cells.iter().filter(|selected| **selected).count();
    cell_selection(selection.entity, grid, &selection.name, selection.stratum.clone(), cells, count)
}

/// Derive what a changed selection shows, under a new revision.
fn refresh_cell_selection(selection: &mut CellSelection) {
    selection.revision = NEXT_CELL_SELECTION_REVISION.fetch_add(1, Ordering::Relaxed);
    selection.masked = selection
        .cells
        .iter()
        .zip(&selection.grid.masked)
        .filter(|(selected, masked)| **selected && **masked)
        .count();
    selection.outline = selection.grid.selection_outline(&selection.cells);
    selection.rows = cell_selection_rows(selection);
}

/// The inspector's rows for one picked prismoid or for several, the grid's
/// place in its strata after its name.
fn cell_selection_rows(selection: &CellSelection) -> InspectorRows {
    let single = (selection.count == 1).then(|| selection.cells.iter().position(|selected| *selected)).flatten();
    let mut rows = match single {
        Some(cell) => prismoid_cell_rows(&selection.name, &selection.grid, cell),
        None => {
            let cells = selection.cells.iter().enumerate().filter(|(_, selected)| **selected).map(|(cell, _)| cell);
            prismoid_selection_rows(&selection.name, &selection.grid.summary_of(cells))
        }
    };
    if let Some(stratum) = &selection.stratum {
        rows.properties.splice(1..1, stratum_rows(stratum));
    }
    if selection.masked > 0 {
        rows.properties
            .push(inspector_row(tr!("prismoid-inspector-masked"), selection.masked.to_string(), None, None));
    }
    rows
}

/// Which strata a grid was built in, its interval's position, the two
/// horizons bounding it and the intervals either side.
fn stratum_rows(stratum: &Stratum) -> Vec<InspectorRow> {
    let neighbour = |name: Option<&str>| name.map_or_else(|| tr!("common-none"), str::to_owned);
    vec![
        inspector_row(tr!("prismoid-inspector-strata"), stratum.strata.clone(), None, None),
        inspector_row(
            tr!("prismoid-inspector-interval"),
            tr!(
                "prismoid-inspector-interval-of",
                position = (stratum.index + 1).to_string(),
                count = stratum.intervals.len().to_string()
            ),
            None,
            None,
        ),
        inspector_row(tr!("prismoid-inspector-roof-horizon"), stratum.roof().to_owned(), None, None),
        inspector_row(tr!("prismoid-inspector-floor-horizon"), stratum.floor().to_owned(), None, None),
        inspector_row(tr!("prismoid-inspector-above"), neighbour(stratum.above()), None, None),
        inspector_row(tr!("prismoid-inspector-below"), neighbour(stratum.below()), None, None),
    ]
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

/// The mark a surface made from a grid's `values` carries, or from its
/// vertical thickness with `None`.
fn surface_mark(values: Option<&CellValues>) -> QualityMark {
    match values {
        Some(values) => QualityMark {
            name: values.name.clone(),
            unit: values.unit.clone(),
        },
        None => QualityMark {
            name: tr!("thickness-points-column-vertical"),
            unit: "m".to_owned(),
        },
    }
}

/// Why `quantity` of the grid `name` makes no surface.
fn no_value_surface(name: &str, quantity: &str) -> anyhow::Error {
    anyhow::anyhow!("{}", tr!("cmd-prismoid-grid-surface-empty", grid = name.to_owned(), quantity = quantity.to_owned()))
}

/// Why no prismoid of `grid` compares with one of `other` by `quantity`.
fn no_difference(grid: &str, other: &str, quantity: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "{}",
        tr!(
            "cmd-prismoid-grid-difference-none",
            grid = grid.to_owned(),
            other = other.to_owned(),
            quantity = quantity.to_owned()
        )
    )
}

/// Why `grid` and `other` cannot be compared, in words naming both and
/// their cell sizes.
fn difference_refusal_words(refusal: DifferenceRefusal, grid: &PrismoidGrid, other: &PrismoidGrid, grid_name: &str, other_name: &str) -> anyhow::Error {
    let words = match refusal {
        DifferenceRefusal::Cancelled => tr!("common-cancelled"),
        DifferenceRefusal::NotOneLattice => tr!(
            "cmd-prismoid-grid-difference-lattices",
            grid = grid_name.to_owned(),
            size = cell_size(grid.lattice.least_size()),
            other = other_name.to_owned(),
            other_size = cell_size(other.lattice.least_size())
        ),
    };
    anyhow::anyhow!("{words}")
}

/// Why a difference is dropped at its apply: the other grid, `name` when
/// the job read it as `read`, is gone or no longer holds that grid.
fn other_grid_changed(grids: &[OpenPrismoidGrid], other_id: PrismoidGridId, read: &Arc<PrismoidGrid>, name: &str) -> Option<String> {
    let item = grids.iter().find(|item| item.id == other_id);
    if item.is_some_and(|item| Arc::ptr_eq(&item.grid, read)) {
        return None;
    }
    let name = item.map_or(name, |item| item.name.as_str());
    Some(tr!("cmd-prismoid-grid-changed-while-reading", grid = name.to_owned()))
}

/// The console lines for a difference `name` kept on `grid`, `grid` less
/// `other`: the other grid's unit when it differs from `unit`, the cells
/// compared and left out on each side (as replaced when `replaces` a held
/// value), the range and mean, and for vertical thickness the volume of
/// the difference.
fn difference_lines(name: &str, grid: &str, other: &str, unit: &str, other_unit: Option<&str>, replaces: bool, report: &DifferenceReport) -> Vec<String> {
    let (name, grid, other) = (name.to_owned(), grid.to_owned(), other.to_owned());
    let mut lines = Vec::new();
    if let Some(other_unit) = other_unit.filter(|other_unit| *other_unit != unit) {
        lines.push(tr!(
            "cmd-prismoid-grid-difference-units",
            name = name.clone(),
            other = other.clone(),
            other_unit = unit_label(other_unit),
            grid = grid.clone(),
            unit = unit_label(unit)
        ));
    }
    let (compared, left_out, outside) = (report.compared.to_string(), report.left_out_a.to_string(), report.outside_a.to_string());
    lines.push(if replaces {
        tr!(
            "cmd-prismoid-grid-difference-replaced",
            name = name.clone(),
            grid = grid.clone(),
            other = other.clone(),
            compared = compared,
            left_out = left_out,
            outside = outside
        )
    } else {
        tr!(
            "cmd-prismoid-grid-difference-added",
            name = name.clone(),
            grid = grid.clone(),
            other = other.clone(),
            compared = compared,
            left_out = left_out,
            outside = outside
        )
    });
    let range = tr!(
        "cmd-prismoid-grid-difference-range",
        name = name,
        low = format_quantity(report.low, 2),
        high = format_quantity(report.high, 2),
        mean = format_quantity(report.mean, 2),
        unit = unit.to_owned()
    );
    lines.push(range.trim_end().to_owned());
    if let Some(volume) = report.volume {
        lines.push(tr!("cmd-prismoid-grid-difference-volume", volume = format_quantity(volume, 1)));
    }
    lines
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

/// A surface read as a grid, refused in words naming it.
fn read_grid_surface(name: &str, mesh: Arc<Triangulation>, spatial: Arc<TriangleBvh>) -> Result<GridSurface> {
    GridSurface::read(mesh, spatial).map_err(|problem| anyhow::anyhow!("{}", tr!("cmd-prismoid-grid-not-a-grid", name = name.to_string(), reason = not_a_grid_reason(problem))))
}

/// Why `roof` and `floor` make no prismoid grid at cells of `size` metres,
/// in words naming both.
fn refusal_words(refusal: BuildRefusal, roof: &str, floor: &str, size: f64, envelope: Option<&ModelEnvelope>) -> anyhow::Error {
    let (roof, floor) = (roof.to_string(), floor.to_string());
    let words = match refusal {
        BuildRefusal::Malformed(error) => return error,
        BuildRefusal::Cancelled => tr!("common-cancelled"),
        BuildRefusal::NotOneLattice => tr!("cmd-prismoid-grid-not-one-lattice", roof = roof, floor = floor),
        BuildRefusal::NoOverlap => tr!("cmd-prismoid-grid-no-overlap", roof = roof, floor = floor),
        BuildRefusal::CellTooLarge => tr!("cmd-prismoid-grid-cell-too-large", roof = roof, floor = floor, size = cell_size(size)),
        BuildRefusal::RoofBelow => tr!("cmd-prismoid-grid-roof-below", roof = roof, floor = floor),
        BuildRefusal::NoThickness => tr!("cmd-prismoid-grid-no-thickness", roof = roof, floor = floor),
        BuildRefusal::OffEnvelopeLattice => tr!("cmd-prismoid-grid-off-envelope-lattice", roof = roof, floor = floor),
        BuildRefusal::OutsideEnvelope => tr!(
            "cmd-prismoid-grid-outside-envelope",
            roof = roof,
            floor = floor,
            envelope = envelope.map_or_else(String::new, ModelEnvelope::summary)
        ),
    };
    anyhow::anyhow!("{words}")
}

/// The console lines for a built grid: its size and thickness, its totals,
/// then any cells crossed or missing between `roof` and `floor`.
fn log_built_prismoid_grid(item: &OpenPrismoidGrid, crossed: usize, holes: usize, roof: &str, floor: &str) {
    let summary = &item.summary;
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
    for line in totals_lines(summary) {
        userspace_log!("{line}");
    }
    if crossed > 0 {
        userspace_warn!(
            "{}",
            tr!("cmd-prismoid-grid-crossed", count = (crossed as i64), roof = roof.to_string(), floor = floor.to_string())
        );
    }
    if holes > 0 {
        userspace_log!(
            "{}",
            tr!("cmd-prismoid-grid-holes", count = (holes as i64), roof = roof.to_string(), floor = floor.to_string())
        );
    }
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

/// Which of a grid's values its colours follow, `None` for thickness, and
/// that one's range. A chosen value the grid no longer holds draws thickness.
fn map_inputs(item: &OpenPrismoidGrid) -> (Option<usize>, Option<(f64, f64)>) {
    let index = item.color_by.index(item.grid.cell_values.iter().map(|values| values.name.as_str()));
    let range = match index {
        Some(index) => item.summary.spreads.get(index).and_then(|spread| spread.range),
        None => item.summary.vertical_thickness,
    };
    (index, range)
}

/// A grid's colours as a raster: thickness smooth across each cell, a value
/// one block per cell. `None` with no range, so the body draws flat.
fn prismoid_map(grid: &PrismoidGrid, name: &str, index: Option<usize>, range: Option<(f64, f64)>, preset: DrillColorPreset) -> Option<OpenRasterTexture> {
    let range = range?;
    match index {
        None => grid
            .thickness_map(range, MAX_RASTER_PREVIEW_DIMENSION)
            .map(|map| ramp_raster(name, map.positions.into_iter().map(Some), map.size, map.world_to_uv, preset, false)),
        Some(index) => grid
            .cell_value_map(index, range, MAX_RASTER_PREVIEW_DIMENSION)
            .map(|map| ramp_raster(name, map.positions.into_iter(), map.size, map.world_to_uv, preset, true)),
    }
}

/// Positions on a scale as a raster the drape can draw, under a new id. A
/// cell without a value is clear, so the body's own colour shows there.
fn ramp_raster(name: &str, positions: impl Iterator<Item = Option<f32>>, size: [u32; 2], world_to_uv: [f64; 6], preset: DrillColorPreset, nearest: bool) -> OpenRasterTexture {
    let stops = preset.stops();
    let rgba = positions
        .flat_map(|position| match position {
            Some(position) => {
                let [red, green, blue] = evaluate_stops(position, &stops, preset.smooth()).map(linear_to_srgb_byte);
                [red, green, blue, u8::MAX]
            }
            None => [0; 4],
        })
        .collect();
    let rgba = Arc::new(rgba);
    OpenRasterTexture {
        id: RasterTextureId(NEXT_PRISMOID_MAP_ID.fetch_sub(1, Ordering::Relaxed)),
        state: ProjectItemState::dirty(MemberKind::Raster, None).with_loaded(true),
        name: name.to_owned(),
        source_size: size,
        preview_size: size,
        full_rgba: Arc::clone(&rgba),
        rgba,
        world_to_uv,
        projection: String::new(),
        driver_name: String::new(),
        nearest,
    }
}

/// A scale's stops for the key's bar, which blends between neighbours: a
/// stepped scale gives each band two stops of its colour, so the bar steps
/// where the body does.
pub(crate) fn ramp_bar_stops(preset: DrillColorPreset) -> Vec<(f32, [f32; 3])> {
    let stops = preset.stops();
    if preset.smooth() {
        return stops.iter().map(|stop| (stop.t, stop.color)).collect();
    }
    let ends = stops.iter().skip(1).map(|stop| stop.t).chain([1.0]);
    stops.iter().zip(ends).flat_map(|(stop, end)| [(stop.t, stop.color), (end, stop.color)]).collect()
}

/// `merged` unless it is empty. A free function on the two fields, because
/// its answer is held across borrows of the editor and the renderer.
pub(crate) fn with_prismoid_scene<'s, T>(own: &'s [T], merged: &'s [T]) -> &'s [T] {
    if merged.is_empty() { own } else { merged }
}
