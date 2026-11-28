//! Prismoid grid commands: building one from two surfaces, adding qualities
//! to one, making a surface from one, removing one from the explorer, and
//! preparing what the view draws of each.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    sync::{
        Arc, Weak,
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
                chain_contour_segments, contour_point_key,
                cuts::{for_each_plan_overlap_about, reference_xy_overlap_area_tolerance},
                session::{build_generated_triangulation, refuse_quality},
                solid_between::ring_contains,
            },
        },
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        Command, FillStyle, FolderId, ItemRef, ItemStyle, Layer, MemberKind, Object, ObjectColor, ObjectId, OpenItem, PolyVertex, ReserveAggregation, SceneEntityId, SectionKind,
        drill_hole::DrillColorPreset,
        formats::mesh_data::{self, Triangulation},
        geometry::triangle_xy_area,
        grid_surface::{GridSurface, surface_at},
        numbers::format_quantity,
        prismoid_grid::{
            AddedValues, BuildRecord, BuildRefusal, CUT_BUDGET, CellReading, CellValueRole, CellValues, DEFAULT_PRISMOID_GRID_COLOR, DifferenceRefusal, DifferenceReport, Gone,
            HoleRecords, Lattice, Moved, OpenPrismoidGrid, PrismoidBuild, PrismoidColorBy, PrismoidGrid, PrismoidGridId, PrismoidGridSummary, ProjectSources, QualitySurface,
            Quantity, SectionCrossing, SectionPoint, SetPrint, SourceStamp, Staleness, Stratum, TrimRefusal, Trimmed, ValueFrom, ValueSource, ValuesRefusal, Weighting,
            build_strata, check_difference_name, hole_prints, names_once, present_name, ramp_position,
        },
        progress::{Phase, Progress},
        project::{ModelEnvelope, ProjectItemState, unique_item_name},
        raster::{OpenRasterTexture, RasterTextureId},
        rbf::NODE_BUDGET,
        solid_reserves::NumericTotal,
        spatial::TriangleBvh,
        triangulation::{GeneratedTriangulationLog, GeometryVersion, OpenTriangulation, QualityMark, TriangulationId, same_quality_name, spatial_surface_face_order, unit_label},
    },
    rendering::{
        camera::SectionSlab,
        color::linear_to_srgb_byte,
        graphics::{Graphics, PrismoidLattice, camera::ScenePick},
        scene::{
            drill_hole_cache::evaluate_stops,
            section_surface_lines::{SECTION_FILL_DEPTH_TIE_M, SectionCellsOfGrid},
        },
    },
    ui::state::{
        CellSelection, EditorState, InspectorRow, InspectorRows, SelectionMode, SelectionPickTool, TriPolylineClipMode, TriSurfaceCutSide, TriSurfaceType, UiPrismoidGridEntry,
        Workspace,
    },
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

/// Why a grid is not drawn as it is, kept so the warning comes once.
pub(crate) enum DrawRefusal {
    /// More corners than the node budget; holds while the lattice keeps
    /// this count, whatever its cells hold.
    Budget(usize),
    /// The mesh job failed on this read of the grid, which is not kept alive.
    Fault(Weak<PrismoidGrid>),
}

impl DrawRefusal {
    /// Whether the refusal still stands for `grid`.
    fn holds_for(&self, grid: &Arc<PrismoidGrid>) -> bool {
        match self {
            Self::Budget(corners) => grid.lattice.corner_count() == *corners,
            Self::Fault(read) => std::ptr::eq(read.as_ptr(), Arc::as_ptr(grid)),
        }
    }
}

/// A prismoid grid as the view draws it, for as long as `source` stands.
pub(crate) struct DrawnPrismoidGrid {
    source: Arc<PrismoidGrid>,
    surface: OpenTriangulation,
    /// `None` on a tensor lattice or when the chosen value has no range,
    /// which draw in the grid's flat colour.
    map: Option<OpenRasterTexture>,
    /// What `map` was made from.
    color_by: PrismoidColorBy,
    /// The rim round roof and floor, drawn in Solid view.
    outline: Arc<Vec<[u32; 2]>>,
    /// The lattice lines, drawn otherwise.
    lattice: Arc<Vec<[u32; 2]>>,
    /// The view `surface` is styled for.
    shaded: bool,
    /// The active cells sharing each lattice line.
    lattice_cells: Arc<Vec<[u32; 4]>>,
    /// One colour per lattice line, from what `map` was made from; `None`
    /// where the body draws flat.
    wire_colors: Option<Arc<Vec<[f32; 4]>>>,
}

impl DrawnPrismoidGrid {
    /// Style the surface for its view, under a new id so the view's cache
    /// uploads it afresh: the lattice over faces drawn clear, in the grid's
    /// colour, or left to the view's coloured strokes with `wire_colors`; or
    /// with `shaded` the shaded body and its dark rim.
    fn style(&mut self, color: [f32; 4], shaded: bool) {
        let [red, green, blue, _] = color;
        (self.surface.edges, self.surface.color, self.surface.line_color) = if shaded {
            (Arc::clone(&self.outline), color, PRISMOID_OUTLINE_COLOR)
        } else if self.wire_colors.is_some() {
            (Arc::new(Vec::new()), [red, green, blue, 0.0], color)
        } else {
            (Arc::clone(&self.lattice), [red, green, blue, 0.0], color)
        };
        self.surface.id = next_view_id();
        self.shaded = shaded;
    }
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
        let (roof_stamp, floor_stamp) = (self.prismoid_stamp(roof_id)?, self.prismoid_stamp(floor_id)?);
        let runtime_id = self.workspace.active_project().map(|project| project.runtime_id);
        // An Err is a refusal; an Ok(Err) is a built grid failing its own
        // check, a fault in the build rather than in the surfaces. A build
        // comes with the surfaces' spacing.
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<Result<(PrismoidBuild, f64)>> {
            let roof = read_grid_surface(&roof_name, roof_mesh, roof_spatial)?;
            let floor = read_grid_surface(&floor_name, floor_mesh, floor_spatial)?;
            match PrismoidGrid::between(&roof, &floor, step, envelope.as_ref(), cancel, &progress.phase(0.0, 1.0)) {
                Ok(built) => Ok(Ok((built, roof.spacing()))),
                Err(BuildRefusal::Malformed(error)) => Ok(Err(error.context(grid_name))),
                Err(refusal) => Err(refusal_words(refusal, &roof_name, &floor_name, roof.spacing() * step.get() as f64, envelope.as_ref())),
            }
        };
        let apply = move |app: &mut App, result: Result<Result<(PrismoidBuild, f64)>>| match result {
            Err(refusal) if !crate::app::jobs::is_cancelled(&refusal) => {
                userspace_warn!("{refusal:#}");
                app.reopen_refused(runtime_id, |editor| editor.tri_prismoid_open = true);
            }
            Err(_) => {}
            Ok(Err(fault)) => userspace_error!("{fault:#}"),
            Ok(Ok((PrismoidBuild { grid, crossed, holes }, spacing))) => {
                let prints = hole_prints(&grid.lattice, &app.hole_records());
                let id = app.insert_prismoid_grid(name, grid, section);
                if let Some(item) = app.prismoid_grids.iter_mut().find(|item| item.id == id) {
                    item.built_from = Some(BuildRecord::now(roof_stamp, floor_stamp, spacing, step, envelope, prints));
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
        if !names_once(stated.intervals.iter().map(String::as_str)) {
            anyhow::bail!("{}", tr!("tri-strata-needs-intervals-once"));
        }
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
        let stamps = horizons.iter().map(|(id, _)| self.prismoid_stamp(*id)).collect::<Result<Vec<_>>>()?;
        let runtime_id = self.workspace.active_project().map(|project| project.runtime_id);
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<Result<(Vec<Option<PrismoidBuild>>, f64)>> {
            let grids = sources
                .into_iter()
                .map(|(name, mesh, spatial, _)| read_grid_surface(&name, mesh, spatial))
                .collect::<Result<Vec<_>>>()?;
            let size = grids[0].spacing() * step.get() as f64;
            match build_strata(&grids, step, envelope.as_ref(), cancel, &progress.phase(0.0, 1.0)) {
                Ok(built) => Ok(Ok((built, grids[0].spacing()))),
                Err((index, BuildRefusal::Malformed(error))) => Ok(Err(error.context(words.intervals[index].clone()))),
                Err((_, BuildRefusal::Cancelled)) => anyhow::bail!(crate::app::jobs::Cancelled),
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
        let apply = move |app: &mut App, result: Result<Result<(Vec<Option<PrismoidBuild>>, f64)>>| {
            let (built, spacing) = match result {
                Ok(Ok(built)) => built,
                Ok(Err(fault)) => {
                    userspace_error!("{fault:#}");
                    return;
                }
                Err(refusal) if !crate::app::jobs::is_cancelled(&refusal) => {
                    userspace_warn!("{refusal:#}");
                    app.reopen_refused(runtime_id, |editor| editor.tri_strata_open = true);
                    return;
                }
                Err(_) => return,
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
                app.reopen_refused(runtime_id, |editor| editor.tri_strata_open = true);
                return;
            }
            let mut prints: Vec<Vec<SetPrint>> = {
                let records = app.hole_records();
                built
                    .iter()
                    .map(|build| build.as_ref().map(|build| hole_prints(&build.grid.lattice, &records)).unwrap_or_default())
                    .collect()
            };
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
                if let Some(item) = app.prismoid_grids.iter_mut().find(|item| item.id == id) {
                    let prints = std::mem::take(&mut prints[index]);
                    item.built_from = Some(BuildRecord::now(stamps[index].clone(), stamps[index + 1].clone(), spacing, step, envelope, prints));
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

    /// A surface's name and geometry token, for a grid's build record.
    fn prismoid_stamp(&self, id: TriangulationId) -> Result<SourceStamp> {
        self.triangulations
            .iter()
            .find(|item| item.id == id)
            .map(|item| SourceStamp {
                name: item.name.clone(),
                geometry: item.geometry,
            })
            .ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-thickness-points-surface-gone")))
    }

    /// The project as a grid's build record is checked against; the holes'
    /// records are read only when some grid has a record.
    pub(crate) fn prismoid_sources(&self) -> ProjectSources<'_> {
        let recorded = self.prismoid_grids.iter().any(|item| item.built_from.is_some());
        ProjectSources {
            surfaces: self.triangulations.iter().map(|item| (item.name.as_str(), item.geometry)).collect(),
            drill_holes: if recorded { self.hole_records() } else { Vec::new() },
            grids: self.prismoid_grids.iter().map(|item| item.name.as_str()).collect(),
            envelope: self.workspace.active_project().and_then(|project| project.project.metadata.modelling.envelope),
        }
    }

    /// Every drill-hole set, the loaded ones' correction records each with
    /// its hole's box in plan.
    fn hole_records(&self) -> Vec<HoleRecords<'_>> {
        self.drill_holes
            .iter()
            .map(|item| {
                if item.state.loaded && item.state.deferred.is_none() {
                    HoleRecords::of(&item.name, &item.dataset)
                } else {
                    HoleRecords::unloaded(&item.name)
                }
            })
            .collect()
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
        let sources = surfaces
            .iter()
            .filter_map(|id| self.triangulations.iter().find(|item| item.id == *id))
            .filter_map(|item| {
                Some(ValueSource {
                    value: item.quality.as_ref()?.name.clone(),
                    from: ValueFrom::Surface {
                        surface: SourceStamp {
                            name: item.name.clone(),
                            geometry: item.geometry,
                        },
                        density: density == Some(item.id),
                    },
                })
            })
            .collect();
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
            app.replace_prismoid_values(runtime_id, grid_id, &name, &source, result, sources, false);
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-adding-qualities"), keys, compute, apply);
        Ok(())
    }

    /// Put a grid's copy with values added in place as one undoable step,
    /// its record noting `sources`, and log `lines`, its totals and the next
    /// step; a refusal is warned, and nothing is replaced when the grid is
    /// gone or moved on from `source`.
    /// `difference` words the lines for a difference, not for qualities.
    /// True when the copy was put in place.
    #[allow(clippy::too_many_arguments)]
    fn replace_prismoid_values(
        &mut self,
        runtime_id: u32,
        grid_id: PrismoidGridId,
        name: &str,
        source: &Arc<PrismoidGrid>,
        result: Result<(PrismoidGrid, Vec<String>)>,
        sources: Vec<ValueSource>,
        difference: bool,
    ) -> bool {
        let (merged, lines) = match result {
            Ok(added) => added,
            Err(refusal) if !crate::app::jobs::is_cancelled(&refusal) => {
                userspace_warn!("{refusal:#}");
                return false;
            }
            Err(_) => return false,
        };
        let current = self.workspace.active_project().is_some_and(|project| project.runtime_id == runtime_id);
        let Some(item) = self.prismoid_grids.iter().find(|item| item.id == grid_id).filter(|_| current) else {
            if difference {
                userspace_warn!("{}", tr!("cmd-prismoid-grid-difference-discarded", grid = name.to_owned()));
            } else {
                userspace_warn!("{}", tr!("cmd-prismoid-grid-qualities-discarded", grid = name.to_owned()));
            }
            return false;
        };
        if !Arc::ptr_eq(&item.grid, source) {
            if difference {
                userspace_warn!("{}", tr!("cmd-prismoid-grid-changed-while-comparing", grid = item.name.clone()));
            } else {
                userspace_warn!("{}", tr!("cmd-prismoid-grid-changed-while-reading", grid = item.name.clone()));
            }
            return false;
        }
        let replacement = with_merged_values(item, merged, sources);
        let totals = totals_lines(&replacement.summary);
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
        if difference {
            userspace_log!("{}", tr!("cmd-prismoid-grid-difference-rederive"));
        } else {
            userspace_log!("{}", tr!("cmd-prismoid-grid-qualities-rederive"));
        }
        true
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
        check_difference_name(&names, &other_name, &name).map_err(|refusal| anyhow::anyhow!("{}", difference_name_words(refusal, &grid_name)))?;
        let replaces = names.iter().any(|(held, _)| same_quality_name(held, &name));
        let keys = vec![JobKey::PrismoidGrid(grid_id), JobKey::PrismoidGrid(other_id)];
        let (grid, name_a, name_b) = (Arc::clone(&source), grid_name.clone(), other_name.clone());
        let other_read = Arc::clone(&other);
        let compared_with = ValueSource {
            value: name.clone(),
            from: ValueFrom::Difference {
                other: other_name.clone(),
                of: quantity.clone(),
            },
        };
        let compute = move |cancel: &CancelFlag, _: &Progress| -> Result<(PrismoidGrid, Vec<String>)> {
            let compared = grid
                .difference(&other, &quantity, name.clone(), unit.clone(), cancel)
                .map_err(|refusal| difference_refusal_words(refusal, &grid, &other, &name_a, &name_b))?;
            let (values, report) = compared.ok_or_else(|| no_difference(&name_a, &name_b, &label))?;
            let lines = difference_lines(&name, &name_a, &name_b, &unit, other_unit.as_deref(), replaces, &report);
            let merged = grid
                .with_values(vec![AddedValues { surface: name_b.clone(), values }])
                .map_err(|refusal| anyhow::anyhow!("{}", difference_name_words(refusal, &name_a)))?;
            Ok((merged, lines))
        };
        let apply = move |app: &mut App, result: Result<(PrismoidGrid, Vec<String>)>| {
            if result.is_ok()
                && let Some(changed) = other_grid_changed(&app.prismoid_grids, other_id, &other_read, &other_name)
            {
                userspace_warn!("{changed}");
                app.reopen_refused(Some(runtime_id), |editor| editor.tri_prismoid_difference_open = true);
                return;
            }
            // A refusal, or a grid gone or changed: each asks for the
            // comparison again, so the dialog comes back with it.
            let cancelled = result.as_ref().is_err_and(crate::app::jobs::is_cancelled);
            if !app.replace_prismoid_values(runtime_id, grid_id, &grid_name, &source, result, vec![compared_with], true) && !cancelled {
                app.reopen_refused(Some(runtime_id), |editor| editor.tri_prismoid_difference_open = true);
            }
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
        let runtime_id = self.workspace.active_project().map(|project| project.runtime_id);
        let compute = move |cancel: &CancelFlag, _: &Progress| -> Result<GeneratedTriangulationLog> {
            if cancel.is_cancelled() {
                anyhow::bail!(crate::app::jobs::Cancelled);
            }
            let (vertices, faces) = grid.value_mesh(index).ok_or_else(|| no_value_surface(&grid_name, &quantity))?;
            let nodes = vertices.len();
            let generated = build_generated_triangulation(name, vertices, faces, TriSurfaceType::Surface, |_| Vec::new())?.with_quality(Some(mark));
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
            let cancelled = result.as_ref().is_err_and(crate::app::jobs::is_cancelled);
            app.apply_generated_triangulation_job(result, &[]);
            // Contour Lines reads z, so the lines of this surface are values;
            // said here, as the contour tool does not carry the mark.
            if made {
                userspace_log!("{}", tr!("cmd-prismoid-grid-surface-contours-are-values", quantity = contoured));
            } else if !cancelled {
                app.reopen_refused(runtime_id, |editor| editor.tri_prismoid_surface_open = true);
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-making-surface"), vec![JobKey::PrismoidGrid(grid_id)], compute, apply);
        Ok(())
    }

    /// Mask the selected prismoids by hand as one undoable step, or unmask
    /// them when every one is masked by hand; a mixed selection masks its
    /// active cells. With none selected the viewport picks them first, and
    /// the pick's Done comes back here.
    pub(crate) fn toggle_prismoid_mask(&mut self) {
        let Some(runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        let Some(selection) = &self.editor.cell_selection else {
            self.begin_selection_pick(SelectionPickTool::MaskPrismoids);
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

    /// Unmask every prismoid masked by hand in each selected loaded grid,
    /// one undoable step per grid.
    pub(crate) fn unmask_all_prismoids(&mut self) {
        let Some(runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        let selected = &self.editor.selected_handles;
        let grids: Vec<_> = self
            .prismoid_grids
            .iter()
            .filter(|item| item.state.loaded && item.summary.masked_cells > 0 && selected.contains(&item.entity_id()))
            .map(|item| (item.id, item.name.clone(), Arc::clone(&item.grid)))
            .collect();
        for (id, name, source) in grids {
            self.spawn_mask_edit(runtime_id, id, name, source, false, |grid, _, _| Ok(unmask_all(grid)));
        }
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
            Err(refusal) if !crate::app::jobs::is_cancelled(&refusal) => {
                userspace_warn!("{refusal:#}");
            }
            Err(_) => {}
        };
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-masking"), vec![JobKey::PrismoidGrid(grid_id)], compute, apply);
    }

    /// Put a grid's masked or unmasked copy in place as one undoable step,
    /// unless the grid is gone or moved on from `source`, nothing changed, or
    /// no active cell would be left. A cell selection on `source` follows to
    /// the copy, less the cells no longer active.
    fn replace_prismoid_mask(&mut self, runtime_id: u32, grid_id: PrismoidGridId, name: &str, source: &Arc<PrismoidGrid>, changed: Option<MaskedGrid>, mask: bool) {
        let current = self.workspace.active_project().is_some_and(|project| project.runtime_id == runtime_id);
        let item = match masked_item(&self.prismoid_grids, current, grid_id, source, name, mask) {
            Ok(item) => item,
            Err(refusal) => {
                userspace_warn!("{refusal}");
                return;
            }
        };
        let grid_name = item.name.clone();
        let Some((grid, summary, count)) = changed else {
            if mask {
                userspace_log!("{}", tr!("cmd-prismoid-grid-mask-nothing", grid = grid_name));
            } else {
                userspace_log!("{}", tr!("cmd-prismoid-grid-unmask-nothing", grid = grid_name));
            }
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
    /// `mode` does not keep; with `unmask`, unmask every one masked by hand
    /// on the side it keeps.
    pub(crate) fn mask_prismoid_grid_by_polyline(&mut self, grid_id: PrismoidGridId, polyline_id: ObjectId, mode: TriPolylineClipMode, unmask: bool) -> Result<()> {
        let gone = || anyhow::anyhow!("{}", tr!("cmd-commands-select-one-prismoid-grid-polyline"));
        let runtime_id = self.workspace.active_project().map(|project| project.runtime_id).ok_or_else(gone)?;
        let item = self.prismoid_grids.iter().find(|item| item.id == grid_id && item.state.loaded).ok_or_else(gone)?;
        let (source, name) = (Arc::clone(&item.grid), item.name.clone());
        let ring = mask_ring(self.scene_document.get_object(polyline_id).and_then(Object::closed_boundary))?;
        self.spawn_mask_edit(runtime_id, grid_id, name, source, !unmask, move |grid, cancel, progress| {
            let cells = cells_by_ring(grid, &ring, mode, unmask, cancel, &progress.phase(0.0, 1.0)).ok_or(crate::app::jobs::Cancelled)?;
            Ok(mask_cells(grid, cells, !unmask))
        });
        Ok(())
    }

    /// Trim each of `grids` to the topology `surface` on the job queue,
    /// keeping the ground on `side` of it, into a new grid per source; the
    /// sources stay as they are. A strata's intervals go into one new folder
    /// for it. `name` names the new grid when there is one source. A surface
    /// that folds over any of the grids refuses the whole run. Ground within
    /// `allowance` metres (0 to 1) of a roof or floor is taken as on it. An
    /// interval whose build record says its roof was built from `surface`
    /// itself is left out, with a line saying so; a grid with no record is
    /// trimmed.
    /// A source deleted while the job runs cancels the whole run, and
    /// nothing is made.
    pub(crate) fn trim_prismoid_grids(
        &mut self,
        grids: Vec<PrismoidGridId>,
        surface: TriangulationId,
        side: TriSurfaceCutSide,
        name: Option<String>,
        allowance: f64,
    ) -> Result<()> {
        anyhow::ensure!((0.0..=1.0).contains(&allowance), "{}", tr!("tri-prismoid-trim-touching-range"));
        let gone = || anyhow::anyhow!("{}", tr!("cmd-commands-select-prismoid-grids-trim"));
        let mut sources = grids
            .iter()
            .map(|id| {
                self.prismoid_grids.iter().find(|item| item.id == *id && item.state.loaded).map(|item| TrimSource {
                    id: item.id,
                    grid: Arc::clone(&item.grid),
                    name: item.name.clone(),
                    stratum: item.stratum.clone(),
                    section: item.state.section,
                    roof_geometry: item.built_from.as_ref().map(|record| record.roof.geometry),
                })
            })
            .collect::<Option<Vec<_>>>()
            .filter(|sources| !sources.is_empty())
            .ok_or_else(gone)?;
        // Each strata's intervals top first, as they are logged.
        sources.sort_by_key(|source| source.stratum.as_ref().map(|stratum| (stratum.strata.clone(), stratum.index)));
        let (surface_name, mesh, spatial, _) = self.prismoid_source(surface)?;
        let surface_geometry = self.prismoid_stamp(surface)?.geometry;
        sources.retain(|source| {
            let skip = roof_is_trimming_surface(source.roof_geometry, surface_geometry);
            if skip {
                userspace_log!(
                    "{}",
                    tr!(
                        "cmd-prismoid-grid-trim-interval-roof-is-surface",
                        grid = source.name.clone(),
                        surface = surface_name.clone()
                    )
                );
            }
            !skip
        });
        if sources.is_empty() {
            return Ok(());
        }
        let grids: Vec<PrismoidGridId> = sources.iter().map(|source| source.id).collect();
        let project_runtime_id = self.workspace.active_project().map(|project| project.runtime_id);
        let (keep_below, relation) = (side == TriSurfaceCutSide::CutTop, side.retained_relation());
        let reads: Vec<_> = sources
            .iter()
            .map(|source| {
                (
                    Arc::clone(&source.grid),
                    source.name.clone(),
                    source.stratum.as_ref().map(|stratum| stratum.category().to_owned()),
                )
            })
            .collect();
        let label = surface_name.clone();
        // An Err is a refusal; an Ok(Err) is a trimmed grid failing its own
        // check, a fault in the trim rather than in the surface.
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<Result<(Vec<TrimOutcome>, usize)>> {
            let count = reads.len() as f32;
            let slack = reference_xy_overlap_area_tolerance(&mesh);
            let read = |stack: &mut Vec<usize>, at: DVec2| surface_at(&mesh, &spatial, stack, at);
            let overlaps = trim_overlaps(&mesh, &spatial, slack);
            let mut outcomes = Vec::new();
            for (index, (grid, grid_name, interval)) in reads.iter().enumerate() {
                let phase = progress.phase(index as f32 / count, (index + 1) as f32 / count);
                let trimmed = match grid.trimmed(read, &overlaps, slack, keep_below, allowance, CUT_BUDGET, cancel, &phase) {
                    Ok(trimmed) => trimmed,
                    Err(TrimRefusal::Malformed(error)) => return Ok(Err(error.context(grid_name.clone()))),
                    Err(TrimRefusal::Cancelled) => anyhow::bail!(crate::app::jobs::Cancelled),
                    Err(refusal) => anyhow::bail!("{}", trim_refusal_words(refusal, grid_name, &surface_name)),
                };
                let lines = trim_lines(grid_name, &surface_name, &relation, interval.as_deref(), allowance, &trimmed);
                let reached = trimmed.reached.active_cells > 0;
                let (roof_strings, floor_strings) = (chain_contour_segments(trimmed.roof_contacts), chain_contour_segments(trimmed.floor_contacts));
                outcomes.push(TrimOutcome {
                    grid: trimmed.grid,
                    lines,
                    reached,
                    roof_strings,
                    floor_strings,
                });
            }
            let upright = upright_faces_over(&mesh, &spatial, slack, reads.iter().map(|(grid, ..)| &grid.lattice));
            Ok(Ok((outcomes, upright)))
        };
        let apply = move |app: &mut App, result: Result<Result<(Vec<TrimOutcome>, usize)>>| match result {
            Err(refusal) if !crate::app::jobs::is_cancelled(&refusal) => userspace_warn!("{refusal:#}"),
            Err(_) => {}
            Ok(Err(fault)) => userspace_error!("{fault:#}"),
            Ok(Ok((outcomes, upright))) => {
                if upright > 0 {
                    userspace_warn!("{}", tr!("cmd-cuts-ignored-vertical-faces", count = upright.to_string()));
                }
                app.place_trimmed_grids(sources, outcomes, &label, side, name, project_runtime_id)
            }
        };
        let mut keys: Vec<JobKey> = grids.iter().map(|id| JobKey::PrismoidGrid(*id)).collect();
        keys.push(JobKey::Triangulation(surface));
        self.spawn_job_reporting_progress(tr!("cmd-prismoid-grid-trimming"), keys, compute, apply);
        Ok(())
    }

    /// Log each trim and insert the grids it kept, selected, beside their
    /// sources; a strata's go into one new folder named for the trim, each
    /// carrying its interval's record. A source that moved on while it was
    /// trimmed is dropped with a warning and the rest are made; one deleted
    /// then is never seen here, the whole job being cancelled with it. A
    /// surface reaching none of the grids ends with a warning. Where it met the
    /// roofs and floors of the grids placed go as strings into one new layer
    /// of the project the trim was started in (`project_runtime_id`).
    fn place_trimmed_grids(
        &mut self,
        sources: Vec<TrimSource>,
        outcomes: Vec<TrimOutcome>,
        surface: &str,
        side: TriSurfaceCutSide,
        name: Option<String>,
        project_runtime_id: Option<u32>,
    ) {
        let mut kept = Vec::with_capacity(sources.len());
        let mut contacts: Vec<GridStrings> = Vec::new();
        for (source, outcome) in sources.iter().zip(&outcomes) {
            if let Some(changed) = trimmed_grid_changed(&self.prismoid_grids, source.id, &source.grid, &source.name) {
                userspace_warn!("{changed}");
                kept.push(false);
                continue;
            }
            contacts.push((&outcome.roof_strings, &outcome.floor_strings));
            for line in &outcome.lines {
                userspace_log!("{line}");
            }
            kept.push(outcome.grid.is_some());
        }
        let reached = outcomes.iter().any(|outcome| outcome.reached);
        let subcrop = subcrop_strings(&contacts);
        let placements = {
            let folders = self.workspace.active_project().map(|project| &project.project.folders);
            trim_placements(&sources, &kept, surface, side, name.as_deref(), |folder, section| {
                unique_item_name(folder, folders.map(|folders| folders.names(section)).unwrap_or_default().into_iter())
            })
        };
        // Each new folder by its name and section.
        let mut new_folders: Vec<(String, SectionKind, Option<FolderId>)> = Vec::new();
        let mut made = Vec::new();
        for ((source, outcome), placement) in sources.into_iter().zip(outcomes).zip(placements) {
            let (Some(placement), Some(grid)) = (placement, outcome.grid) else {
                continue;
            };
            let folder = match &placement.stratum {
                None => None,
                Some(stratum) => match new_folders.iter().find(|(named, section, _)| *named == stratum.strata && *section == source.section) {
                    Some((.., folder)) => *folder,
                    None => {
                        let folder = self
                            .workspace
                            .active_project_mut()
                            .and_then(|project| project.project.folders.add(source.section, stratum.strata.clone()));
                        new_folders.push((stratum.strata.clone(), source.section, folder));
                        folder
                    }
                },
            };
            made.push(self.insert_prismoid_grid_in(placement.name, grid, source.section, folder, placement.stratum));
        }
        if !made.is_empty() {
            self.editor.selected_handles.clear();
            self.editor.selected_handles.extend(made.into_iter().map(SceneEntityId::PrismoidGrid));
        }
        if !reached {
            userspace_warn!("{}", tr!("cmd-prismoid-grid-trim-reaches-none", surface = surface.to_owned()));
        }
        if let (Some(runtime_id), false) = (project_runtime_id, subcrop.is_empty()) {
            self.add_subcrop_layer(runtime_id, subcrop, surface);
        }
    }

    /// Add `subcrop` as a new layer of the project `runtime_id` in one
    /// undoable step, named for `surface` and not taken, and log it.
    fn add_subcrop_layer(&mut self, runtime_id: u32, subcrop: Subcrop, surface: &str) {
        let surface = surface.trim().to_owned();
        let Some(project) = self.workspace.projects.iter_mut().find(|project| project.runtime_id == runtime_id) else {
            userspace_warn!("{}", tr!("cmd-prismoid-grid-trim-subcrop-discarded-project-closed", surface = surface));
            return;
        };
        let document = &mut project.project.document;
        let layer_name = unique_item_name(
            tr!("cmd-prismoid-grid-trim-subcrop-layer", surface = surface.clone()),
            document.layers().iter().map(|layer| layer.name.as_str()),
        );
        let layer_id = document.allocate_layer_id();
        let (roof, floor) = (subcrop.roof.len(), subcrop.floor.len());
        let strings = subcrop
            .roof
            .into_iter()
            .map(|verts| (verts, SUBCROP_ROOF_COLOR))
            .chain(subcrop.floor.into_iter().map(|verts| (verts, SUBCROP_FLOOR_COLOR)));
        let objects = strings
            .map(|(verts, color)| Object::Polyline {
                id: document.allocate_object_id(),
                layer: layer_id,
                verts: verts.into_iter().map(PolyVertex::straight).collect(),
                closed: false,
                color: ObjectColor::Fixed(color),
                fill: FillStyle::Clear,
                line_weight: 1.0,
            })
            .collect();
        let layer = Layer {
            id: layer_id,
            name: layer_name.clone(),
            color_index: None,
            color: [1.0, 1.0, 1.0, 1.0],
            loaded: true,
            hidden: false,
            elevation: 0.0,
            folder: None,
            section: SectionKind::natural_layer(),
            quality: None,
        };
        self.execute_edit_for(runtime_id, Command::AddLayerSnapshot { layer, objects });
        userspace_log!(
            "{}",
            tr!(
                "cmd-prismoid-grid-trim-subcrop",
                layer = layer_name,
                roof = roof.to_string(),
                surface = surface,
                floor = floor.to_string()
            )
        );
        self.invalidate_geometry();
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
            built_from: None,
            shaded: false,
        });
        self.touch_active_project_content();
        self.invalidate_geometry();
        id
    }

    /// Change what a grid is coloured by, as one undoable style step; an
    /// unchanged choice records nothing.
    pub(crate) fn set_prismoid_grid_color_by(&mut self, id: PrismoidGridId, change: impl FnOnce(&mut PrismoidColorBy)) {
        let item = ItemRef::PrismoidGrid(id);
        let Some(ItemStyle::PrismoidGrid {
            loaded,
            hidden,
            mut color_by,
            shaded,
        }) = self.item_style(item)
        else {
            return;
        };
        change(&mut color_by);
        self.set_item_style(item, ItemStyle::PrismoidGrid { loaded, hidden, color_by, shaded });
    }

    /// Draw a grid as a shaded body or as its lattice lines, as one undoable
    /// style step; an unchanged choice records nothing.
    pub(crate) fn set_prismoid_grid_shaded(&mut self, id: PrismoidGridId, shaded: bool) {
        let item = ItemRef::PrismoidGrid(id);
        let Some(ItemStyle::PrismoidGrid { loaded, hidden, color_by, .. }) = self.item_style(item) else {
            return;
        };
        self.set_item_style(item, ItemStyle::PrismoidGrid { loaded, hidden, color_by, shaded });
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
            // The view checks records only when the project moves; a frame
            // here compares what it found and copies it on a change.
            if let Some((_, view)) = self.ui_project_view_cache.borrow().as_ref() {
                follow_grid_record(selection, &view.prismoid_grids);
            }
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
                .filter(|id| !self.prismoid_grid_refused(*id))
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
            let grids = &self.prismoid_grids;
            let mut restyled = false;
            for drawn in &mut self.prismoid_drawn {
                if let Some(item) = grids.iter().find(|item| Some(item.id) == drawn.surface.prismoid_grid && item.shaded != drawn.shaded) {
                    drawn.style(item.color, item.shaded);
                    restyled = true;
                }
            }
            if restyled {
                self.request_topology_redraw();
            }
        }
        self.refresh_prismoid_scene();
        self.refresh_prismoid_lattice_lines();
        self.refresh_masked_cell_outline();
    }

    /// Outline the drawn grids' prismoids masked by hand while Mask
    /// Prismoids' pick is up in Geology, only when the pick, a grid or what
    /// is hidden moved: a still frame costs a hash.
    fn refresh_masked_cell_outline(&mut self) {
        let picking = self.editor.active_workspace == Workspace::Geology && self.editor.selection_pick.as_ref().is_some_and(|pick| pick.tool == SelectionPickTool::MaskPrismoids);
        if !picking {
            if self.editor.masked_cell_outline.take().is_some() {
                self.request_topology_redraw();
            }
            return;
        }
        let shown = section_grids(&self.editor, &self.prismoid_grids, &self.prismoid_drawn);
        let mut hasher = DefaultHasher::new();
        for item in shown.clone() {
            item.id.hash(&mut hasher);
            (Arc::as_ptr(&item.grid) as usize).hash(&mut hasher);
        }
        let key = hasher.finish();
        if self.editor.masked_cell_outline.as_ref().is_some_and(|(held, _)| *held == key) {
            return;
        }
        let outline = shown.flat_map(|item| item.grid.selection_outline(&item.grid.masked)).collect();
        self.editor.masked_cell_outline = Some((key, outline));
        self.request_topology_redraw();
    }

    /// Pick the prismoid a click on a drawn grid landed on, by Leo's
    /// selection modes one grain down: the grid stays selected while any of
    /// its cells is. False when the point is on no active cell.
    pub(crate) fn pick_prismoid_cell(&mut self, id: PrismoidGridId, world: DVec3) -> bool {
        self.click_prismoid_cell(id, world, |grid| grid.picked_cell(world))
    }

    /// During Mask Prismoids' pick, pick the prismoid masked by hand nearest
    /// along the cursor's ray, on a drawn grid shown and not frozen, as
    /// [`Self::pick_prismoid_cell`] picks; a click's `hit` nearer than it
    /// wins. False with none. Not in a 2D section, which draws no hole.
    pub(crate) fn pick_masked_prismoid_cell(&mut self, hit: Option<&ScenePick>) -> bool {
        let picking = self.editor.active_workspace == Workspace::Geology && self.editor.selection_pick.as_ref().is_some_and(|pick| pick.tool == SelectionPickTool::MaskPrismoids);
        let Some(graphics) = self.graphics.as_ref().filter(|_| picking) else {
            return false;
        };
        let slab = graphics.section_slab();
        if slab.as_ref().is_some_and(|slab| slab.outlines) {
            return false;
        }
        let (origin, direction) = graphics.cursor_model_ray();
        let frozen = &self.editor.pick_frozen;
        let nearest = section_grids(&self.editor, &self.prismoid_grids, &self.prismoid_drawn)
            .filter(|item| !frozen.contains(&item.entity_id()))
            .filter_map(|item| {
                let shown = |at: DVec3| slab.as_ref().is_none_or(|slab| slab.contains(at));
                item.grid.picked_masked_cell(origin, direction, shown).map(|(cell, distance)| (item.id, cell, distance))
            })
            .min_by(|(.., one), (.., other)| one.total_cmp(other));
        let Some((id, cell, distance)) = nearest else {
            return false;
        };
        if hit.is_some_and(|hit| (hit.world - origin).dot(direction) < distance) {
            return false;
        }
        self.click_prismoid_cell(id, origin + direction * distance, |_| Some(cell))
    }

    /// Pick the cell `cell_of` finds on grid `id`, the click landing at
    /// `world`, by [`Self::pick_prismoid_cell`]'s modes. False with none.
    fn click_prismoid_cell(&mut self, id: PrismoidGridId, world: DVec3, cell_of: impl FnOnce(&PrismoidGrid) -> Option<usize>) -> bool {
        let entity = SceneEntityId::PrismoidGrid(id);
        let Some((grid, name, stratum)) = self
            .prismoid_grids
            .iter()
            .find(|item| item.id == id && item.state.loaded)
            .map(|item| (Arc::clone(&item.grid), item.name.clone(), item.stratum.clone()))
        else {
            return false;
        };
        let Some(cell) = cell_of(&grid) else {
            return false;
        };
        let held = self
            .editor
            .cell_selection
            .take()
            .filter(|selection| selection.entity == entity && Arc::ptr_eq(&selection.grid, &grid));
        let (mut cells, count) = held.map_or_else(|| (vec![false; grid.lattice.cell_count()], 0), |selection| (selection.cells, selection.count));
        let mode = cell_click_mode(self.modifiers.shift_key(), self.modifiers.control_key(), self.editor.selection_pick.is_some(), cells[cell]);
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
    /// as a plain box does during a tool's pick, and otherwise a plain box
    /// replaces. The grid is selected while any of its cells is.
    pub(crate) fn select_prismoid_cells_in_box(&mut self, start: (f32, f32), end: (f32, f32)) {
        let (toggle, add) = (self.modifiers.shift_key(), self.modifiers.control_key() || self.editor.selection_pick.is_some());
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

    /// Whether drawing grid `id` was refused for the grid it holds now: over
    /// the budget while its lattice keeps that size, or a fault until its
    /// content is replaced.
    fn prismoid_grid_refused(&self, id: PrismoidGridId) -> bool {
        let held = self.prismoid_grids.iter().find(|item| item.id == id);
        self.prismoid_refused.get(&id).is_some_and(|refusal| held.is_some_and(|item| refusal.holds_for(&item.grid)))
    }

    /// Prepare a grid's mesh on the job queue. A grid over the node budget is
    /// refused, not drawn; only a file can hold one.
    fn spawn_prismoid_drawing(&mut self, id: PrismoidGridId) {
        let Some(item) = self.prismoid_grids.iter().find(|item| item.id == id) else {
            return;
        };
        let (grid, name, color, color_by, shaded) = (Arc::clone(&item.grid), item.name.clone(), item.color, item.color_by.clone(), item.shaded);
        let (index, range) = map_inputs(item);
        let corners = grid.lattice.corner_count();
        if corners > NODE_BUDGET {
            self.prismoid_refused.insert(id, DrawRefusal::Budget(corners));
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
        let read = Arc::downgrade(&grid);
        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<DrawnPrismoidGrid> {
            let phase = progress.phase(0.0, 1.0);
            let proceed = || -> Result<()> {
                if cancel.is_cancelled() {
                    anyhow::bail!(crate::app::jobs::Cancelled);
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
            let lattice_cells = Arc::new(closed.lattice_cells);
            let wire_colors = lattice_colors(&grid, color, &lattice_cells, index, range, color_by.preset).map(Arc::new);
            let (outline, lattice) = (Arc::new(closed.outline), Arc::new(closed.lattice));
            let mut surface = preview_triangulation(name, Arc::new(mesh), spatial, Arc::clone(&outline), order, color, PRISMOID_OUTLINE_COLOR);
            surface.raster_texture = map.as_ref().map(|raster| raster.id);
            surface.always_show_edges = true;
            surface.prismoid_grid = Some(id);
            phase.finish();
            let mut drawn = DrawnPrismoidGrid {
                source: grid,
                surface,
                map,
                color_by,
                outline,
                lattice,
                shaded,
                lattice_cells,
                wire_colors,
            };
            drawn.style(color, shaded);
            Ok(drawn)
        };
        let apply = move |app: &mut App, result: Result<DrawnPrismoidGrid>| match result {
            Ok(drawn) => {
                let should_fit = !app.scene_has_renderables();
                app.prismoid_refused.remove(&id);
                app.prismoid_drawn.push(drawn);
                app.refresh_prismoid_scene();
                if should_fit {
                    app.fit_view_to_extents();
                }
                app.invalidate_topology_bounds_and_redraw();
            }
            // A second try would meet the same fault, every frame.
            Err(fault) if !crate::app::jobs::is_cancelled(&fault) => {
                app.prismoid_refused.insert(id, DrawRefusal::Fault(read));
                userspace_error!("{fault:#}");
            }
            Err(_) => {}
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
        type Recoloured = (Arc<PrismoidGrid>, PrismoidColorBy, Option<OpenRasterTexture>, Option<Arc<Vec<[f32; 4]>>>);
        let (grid, name, color, color_by) = (Arc::clone(&item.grid), item.name.clone(), item.color, item.color_by.clone());
        let (index, range) = map_inputs(item);
        let cells = self
            .prismoid_drawn
            .iter()
            .find(|drawn| drawn.surface.prismoid_grid == Some(id) && Arc::ptr_eq(&drawn.source, &grid))
            .map(|drawn| Arc::clone(&drawn.lattice_cells));
        let compute = move |_: &CancelFlag| -> Result<Recoloured> {
            let map = prismoid_map(&grid, &name, index, range, color_by.preset);
            let wire_colors = cells.and_then(|cells| lattice_colors(&grid, color, &cells, index, range, color_by.preset)).map(Arc::new);
            Ok((grid, color_by, map, wire_colors))
        };
        let apply = move |app: &mut App, result: Result<Recoloured>| {
            let Ok((grid, color_by, map, wire_colors)) = result else {
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
            // Only a flip between coloured and flat wires restyles the surface.
            let flips = drawn.wire_colors.is_some() != wire_colors.is_some();
            drawn.wire_colors = wire_colors;
            if flips {
                drawn.style(color, drawn.shaded);
            }
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
            drawn.surface.id.hash(&mut hasher);
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

    /// Hand the view the drawn grids whose lattice lines draw in colour,
    /// only when one of them or its colours moved: a still frame costs a
    /// hash. None off Geology.
    fn refresh_prismoid_lattice_lines(&mut self) {
        let key = prismoid_lattice_key(&self.editor, &self.prismoid_drawn);
        let Some(graphics) = self.graphics.as_mut().filter(|graphics| graphics.prismoid_lattices_key() != Some(key)) else {
            return;
        };
        let lattices = coloured_lattices(&self.editor, &self.prismoid_drawn)
            .map(|(drawn, colors)| PrismoidLattice {
                entity: drawn.surface.entity_id(),
                mesh: Arc::clone(&drawn.surface.mesh),
                lattice: Arc::clone(&drawn.lattice),
                colors: Arc::clone(colors),
                width: drawn.surface.line_weight.unwrap_or(1.0),
                revision: lattice_revision(drawn, colors),
            })
            .collect();
        graphics.set_prismoid_lattices(key, lattices);
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
                // Scanned once for the grid, not once per crossing.
                let regular = item.grid.lattice.is_regular();
                cut.push(SectionCellsOfGrid {
                    colors: crossings.iter().map(|crossing| section_colors_with(item, crossing, regular)).collect(),
                    selected: crossings.iter().map(|crossing| held.is_some_and(|selection| selection.cells[crossing.cell])).collect(),
                    crossings,
                });
            }
        }
        graphics.set_section_cells(cut);
    }

    /// Open a dialog again after its job refused, with what was typed in it
    /// (closing keeps it), while the project the job ran in is still active.
    fn reopen_refused(&mut self, runtime_id: Option<u32>, open: impl FnOnce(&mut EditorState)) {
        if runtime_id.is_some() && self.workspace.active_project().map(|project| project.runtime_id) == runtime_id {
            open(&mut self.editor);
        }
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
/// `regular` is whether the grid's lattice is regular, read once per grid.
fn section_colors_with(item: &OpenPrismoidGrid, crossing: &SectionCrossing, regular: bool) -> Vec<[f32; 4]> {
    let [red, green, blue, _] = item.color;
    let flat = vec![[red, green, blue, 1.0]; crossing.points.len()];
    let (index, range) = map_inputs(item);
    let Some((min, max)) = range.filter(|_| regular) else {
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

/// One colour per lattice line, `cells` listing each line's cells, from the
/// inputs the body's colours are made from: each line's value from
/// [`PrismoidGrid::wire_values`] on the ramp, or the grid's own colour,
/// opaque, where no cell gives it one. `None` where the body draws flat.
fn lattice_colors(grid: &PrismoidGrid, color: [f32; 4], cells: &[[u32; 4]], index: Option<usize>, range: Option<(f64, f64)>, preset: DrillColorPreset) -> Option<Vec<[f32; 4]>> {
    let (min, max) = range.filter(|_| grid.lattice.is_regular())?;
    let [red, green, blue, _] = color;
    let stops = preset.stops();
    let ramp = |value: f64| {
        let [red, green, blue] = evaluate_stops(ramp_position(value, (min, max)), &stops, preset.smooth());
        [red, green, blue, 1.0]
    };
    let values = grid.wire_values(cells, index);
    Some(values.into_iter().map(|value| value.map_or([red, green, blue, 1.0], ramp)).collect())
}

/// The drawn grids whose lattice lines draw in their own colours, with
/// those colours: in Geology, out of Solid view, and coloured.
fn coloured_lattices<'d>(editor: &EditorState, drawn: &'d [DrawnPrismoidGrid]) -> impl Iterator<Item = (&'d DrawnPrismoidGrid, &'d Arc<Vec<[f32; 4]>>)> {
    let geology = editor.active_workspace == Workspace::Geology;
    drawn
        .iter()
        .filter(move |drawn| geology && !drawn.shaded)
        .filter_map(|drawn| Some((drawn, drawn.wire_colors.as_ref()?)))
}

/// Moves with a coloured lattice's view id, mesh or colours.
fn lattice_revision(drawn: &DrawnPrismoidGrid, colors: &Arc<Vec<[f32; 4]>>) -> u64 {
    let mut hasher = DefaultHasher::new();
    drawn.surface.id.hash(&mut hasher);
    (Arc::as_ptr(&drawn.surface.mesh) as usize).hash(&mut hasher);
    (Arc::as_ptr(colors) as usize).hash(&mut hasher);
    hasher.finish()
}

/// What the view's coloured lattices are handed over from: each one's
/// revision.
fn prismoid_lattice_key(editor: &EditorState, drawn: &[DrawnPrismoidGrid]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (drawn, colors) in coloured_lattices(editor, drawn) {
        lattice_revision(drawn, colors).hash(&mut hasher);
    }
    hasher.finish()
}

/// How a click on a prismoid, `selected` or not, picks it, by the canvas's
/// rule: shift toggles, as any click does while a tool's pick builds its
/// set; ctrl adds; a plain click toggles a selected prismoid and replaces
/// the set with an unselected one.
fn cell_click_mode(shift: bool, control: bool, picking: bool, selected: bool) -> SelectionMode {
    if shift || picking {
        SelectionMode::Toggle
    } else if control {
        SelectionMode::Add
    } else if selected {
        SelectionMode::Toggle
    } else {
        SelectionMode::Replace
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
/// inside. With `unmask`, the cells masked by hand on the side `mode`
/// keeps instead. A centre on the ring takes the one answer
/// `ring_contains` gives it. `None` when cancelled.
fn cells_by_ring(grid: &PrismoidGrid, ring: &[mesh_data::Vertex], mode: TriPolylineClipMode, unmask: bool, cancel: &CancelFlag, progress: &Phase) -> Option<Vec<usize>> {
    let (low, high) = ring.iter().fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(low, high), point| {
        let point = DVec2::new(point.x, point.y);
        (low.min(point), high.max(point))
    });
    let keep_inside = mode == TriPolylineClipMode::KeepInside;
    let (candidates, side) = if unmask { (&grid.masked, keep_inside) } else { (&grid.active, !keep_inside) };
    let cells = grid.lattice.cell_count();
    let mut taken = Vec::new();
    for (cell, centre) in grid.cell_centres() {
        if cell % 4096 == 0 {
            if cancel.is_cancelled() {
                return None;
            }
            progress.set_fraction(cell as f32 / cells as f32);
        }
        if !candidates[cell] {
            continue;
        }
        let inside = centre.cmpge(low).all() && centre.cmple(high).all() && ring_contains(ring, mesh_data::Vertex::new(centre.x, centre.y, 0.0));
        if inside == side {
            taken.push(cell);
        }
    }
    Some(taken)
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
            built: Vec::new(),
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

/// [`mask_cells`] unmasking every cell of `grid` masked by hand.
fn unmask_all(grid: &PrismoidGrid) -> Option<MaskedGrid> {
    let masked = grid.masked.iter().enumerate().filter(|(_, masked)| **masked).map(|(cell, _)| cell);
    mask_cells(grid, masked, false)
}

/// The grid a mask edit was made on, still in the active project (`current`)
/// and still `source`; else why the edit is dropped: the grid is gone or its
/// project is no longer active (another project's grid can carry the same
/// id), or the grid moved on while the edit was made.
fn masked_item<'g>(
    grids: &'g [OpenPrismoidGrid],
    current: bool,
    grid_id: PrismoidGridId,
    source: &Arc<PrismoidGrid>,
    name: &str,
    mask: bool,
) -> Result<&'g OpenPrismoidGrid, String> {
    let Some(item) = grids.iter().find(|item| item.id == grid_id).filter(|_| current) else {
        return Err(mask_edit_dropped(mask, false, name));
    };
    if Arc::ptr_eq(&item.grid, source) {
        Ok(item)
    } else {
        Err(mask_edit_dropped(mask, true, &item.name))
    }
}

/// The words for a mask (`mask`) or unmask edit dropped because its grid
/// `changed` while it was made, or else because the grid is gone.
fn mask_edit_dropped(mask: bool, changed: bool, grid: &str) -> String {
    let grid = grid.to_owned();
    match (mask, changed) {
        (true, true) => tr!("cmd-prismoid-grid-changed-while-masking", grid = grid),
        (false, true) => tr!("cmd-prismoid-grid-changed-while-unmasking", grid = grid),
        (true, false) => tr!("cmd-prismoid-grid-mask-discarded", grid = grid),
        (false, false) => tr!("cmd-prismoid-grid-unmask-discarded", grid = grid),
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
            let active = &selection.grid.active;
            let cells = selection
                .cells
                .iter()
                .enumerate()
                .filter(|(cell, selected)| **selected && active[*cell])
                .map(|(cell, _)| cell);
            prismoid_selection_rows(&selection.name, &selection.grid.summary_of(cells))
        }
    };
    let placed = selection.stratum.as_ref().map(stratum_rows).unwrap_or_default();
    rows.properties.splice(1..1, placed.into_iter().chain(selection.built.iter().cloned()));
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

/// What a grid was built from and when, then each source moved since: the
/// inspector's rows, made with the project view.
pub(crate) fn built_rows(record: &BuildRecord, staleness: &Staleness, project: &ProjectSources<'_>) -> Vec<InspectorRow> {
    let (roof, floor) = (present_name(&record.roof, project), present_name(&record.floor, project));
    let surfaces = tr!("prismoid-inspector-built-from-surfaces", roof = roof.to_owned(), floor = floor.to_owned());
    let mut rows = vec![
        inspector_row(tr!("prismoid-inspector-built-from"), surfaces, None, None),
        inspector_row(tr!("prismoid-inspector-built"), built_date(&record.built), None, Some(record.application.clone())),
    ];
    rows.extend(
        staleness
            .moved
            .iter()
            .map(|moved| inspector_row(tr!("prismoid-inspector-changed"), moved_line(moved), None, None)),
    );
    rows
}

/// A grid's hover lines on its build, made with the project view: what it
/// was built from, surfaces by their present names, then each source moved
/// since and each no longer in the project.
pub(crate) fn record_lines(record: Option<&BuildRecord>, staleness: &Staleness, project: &ProjectSources<'_>) -> Vec<String> {
    let Some(record) = record else {
        return vec![tr!("explorer-prismoid-grid-no-record")];
    };
    let mut lines = vec![tr!(
        "explorer-prismoid-grid-built",
        roof = present_name(&record.roof, project).to_owned(),
        floor = present_name(&record.floor, project).to_owned(),
        spacing = cell_size(record.spacing),
        step = record.step.to_string(),
        date = built_date(&record.built),
        application = record.application.clone()
    )];
    lines.extend(record.envelope.map(|_| tr!("explorer-prismoid-grid-built-envelope")));
    lines.extend(record.values.iter().map(|source| match &source.from {
        ValueFrom::Surface { surface, .. } => tr!(
            "explorer-prismoid-grid-built-value",
            value = source.value.clone(),
            surface = present_name(surface, project).to_owned()
        ),
        ValueFrom::Difference { other, .. } => tr!("explorer-prismoid-grid-built-difference", value = source.value.clone(), other = other.clone()),
    }));
    lines.extend(staleness.moved.iter().map(moved_line));
    lines.extend(staleness.gone.iter().map(|gone| match gone {
        Gone::Surface(name) => tr!("explorer-prismoid-grid-source-gone", name = name.clone()),
        Gone::Grid(name) => tr!("explorer-prismoid-grid-no-grid-named", name = name.clone()),
        Gone::HoleSet(name) => tr!("explorer-prismoid-grid-hole-set-gone", name = name.clone()),
    }));
    lines
}

fn moved_line(moved: &Moved) -> String {
    match moved {
        Moved::Holes { count, latest } => tr!("explorer-prismoid-grid-holes-corrected", count = count.to_string(), date = built_date(latest)),
        Moved::HoleRecords => tr!("explorer-prismoid-grid-holes-changed"),
        Moved::Surface(name) => tr!("explorer-prismoid-grid-surface-moved", name = name.clone()),
        Moved::Envelope => tr!("explorer-prismoid-grid-envelope-moved"),
    }
}

/// A recorded instant to the minute on the viewer's clock, so dates written
/// under different offsets read in order; as written when it does not read.
fn built_date(date: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(date).map_or_else(|_| date.to_owned(), |date| date.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
}

/// Carry the inspector rows the project view last made for the selection's
/// grid into its rows when they changed.
fn follow_grid_record(selection: &mut CellSelection, entries: &[UiPrismoidGridEntry]) {
    let Some(entry) = entries.iter().find(|entry| SceneEntityId::PrismoidGrid(entry.id) == selection.entity) else {
        return;
    };
    if selection.built != entry.record_rows {
        selection.built.clone_from(&entry.record_rows);
        selection.rows = cell_selection_rows(selection);
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
    let weighted = match (summary.weighting, &summary.density) {
        (Weighting::ByTonnes, Some(density)) => tr!("prismoid-inspector-by-tonnes", density = density.clone()),
        _ => tr!("prismoid-inspector-by-volume"),
    };
    let properties = vec![
        inspector_row(tr!("common-prismoid-grid"), name.to_owned(), None, None),
        inspector_row(tr!("prismoid-inspector-cells"), summary.active_cells.to_string(), None, None),
        match summary.vertical_thickness {
            Some((min, max)) => inspector_row(
                tr!("prismoid-inspector-vertical-thickness"),
                tr!("prismoid-inspector-range", min = format_quantity(min, 2), max = format_quantity(max, 2)),
                Some("m"),
                None,
            ),
            None => inspector_row(tr!("prismoid-inspector-vertical-thickness"), absent(), None, None),
        },
        inspector_row(tr!("reserve-weighted"), weighted, None, None),
    ];
    let tonnes = match &summary.tonnes {
        Some(total) => match total.value(ReserveAggregation::Sum) {
            Some(sum) => inspector_row(tr!("prismoid-inspector-tonnes"), format_quantity(sum, 1), Some("t"), left_out(total)),
            None => inspector_row(tr!("prismoid-inspector-tonnes"), absent(), None, left_out(total)),
        },
        None => inspector_row(tr!("prismoid-inspector-tonnes"), absent(), None, Some(tr!("prismoid-inspector-no-density"))),
    };
    let volume = match summary.active_cells {
        0 => inspector_row(tr!("tri-volume"), absent(), None, None),
        _ => inspector_row(tr!("tri-volume"), format_quantity(summary.volume, 1), Some("m³"), None),
    };
    let mut contents = vec![volume, tonnes];
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
/// its size and thicknesses, then its volume (and, for a prismoid a trim cut,
/// from what), tonnes and every value it holds. Columns, rows and corners
/// count from 1 at the lattice origin.
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
            tr!("tri-prismoid-cell-size"),
            tr!(
                "prismoid-inspector-plan-size",
                along_u = cell_size(lattice.u_sizes[column]),
                along_v = cell_size(lattice.v_sizes[row])
            ),
            Some("m"),
            None,
        ),
        match measure {
            Some((bounds, _)) => inspector_row(
                tr!("prismoid-inspector-mean-thickness"),
                format_quantity(bounds.upper.z - bounds.lower.z, 2),
                Some("m"),
                None,
            ),
            None => inspector_row(tr!("prismoid-inspector-mean-thickness"), tr!("inventory-portion-missing"), None, None),
        },
    ];
    let heights = grid.cell_corner_heights(cell);
    // Round the cell from its origin corner, as `closed_mesh` takes them.
    for (index, (along_u, along_v)) in [(column, row), (column + 1, row), (column + 1, row + 1), (column, row + 1)].into_iter().enumerate() {
        properties.push(inspector_row(
            tr!("prismoid-inspector-corner", column = ordinal(along_u), row = ordinal(along_v)),
            format_quantity(heights[index] - heights[index + 4], 2),
            Some("m"),
            None,
        ));
    }
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
    let volume = match measure {
        Some((_, measure)) => inspector_row(tr!("tri-volume"), format_quantity(measure.volume(), 1), Some("m³"), None),
        None => inspector_row(tr!("tri-volume"), tr!("inventory-portion-missing"), None, None),
    };
    let mut contents = vec![volume, tonnes];
    if grid.cut.cell_index(cell).is_some() {
        contents.insert(
            1,
            inspector_row(tr!("prismoid-inspector-volume-from"), tr!("prismoid-inspector-cut-along-topography"), None, None),
        );
    }
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
            .ok_or(crate::app::jobs::Cancelled)?;
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

/// `item` holding `merged`, its build record, when it has one, noting
/// where the merged values came from.
fn with_merged_values(item: &OpenPrismoidGrid, merged: PrismoidGrid, sources: Vec<ValueSource>) -> OpenPrismoidGrid {
    let mut replacement = item.clone();
    replacement.summary = merged.summary();
    replacement.grid = Arc::new(merged);
    if let Some(record) = &mut replacement.built_from {
        record.note_values(sources);
    }
    replacement
}

/// The mark a surface made from a grid's `values` carries, or from its
/// vertical thickness with `None`.
pub(super) fn surface_mark(values: Option<&CellValues>) -> QualityMark {
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
        DifferenceRefusal::Cancelled => return anyhow::Error::new(crate::app::jobs::Cancelled),
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
    Some(tr!("cmd-prismoid-grid-changed-while-comparing", grid = name.to_owned()))
}

/// A selected grid as a trim takes it.
struct TrimSource {
    id: PrismoidGridId,
    grid: Arc<PrismoidGrid>,
    name: String,
    stratum: Option<Stratum>,
    section: SectionKind,
    /// The geometry its build record says its roof was built from.
    roof_geometry: Option<GeometryVersion>,
}

/// Whether a grid's roof was built from the trimming surface, by the
/// geometry its build record names; a grid with no record says nothing.
fn roof_is_trimming_surface(roof: Option<GeometryVersion>, surface: GeometryVersion) -> bool {
    roof == Some(surface)
}

/// What a trim makes of one grid: the grid kept, if any, its console lines,
/// and whether the surface reached any of its active cells.
struct TrimOutcome {
    grid: Option<PrismoidGrid>,
    lines: Vec<String>,
    reached: bool,
    /// Where the surface meets the roofs and floors of the cut cells, as
    /// chained strings.
    roof_strings: Vec<Vec<DVec3>>,
    floor_strings: Vec<Vec<DVec3>>,
}

/// The colour of the subcrop strings where the surface meets a roof.
const SUBCROP_ROOF_COLOR: [f32; 4] = [0.95, 0.55, 0.1, 1.0];

/// The colour of the subcrop strings where the surface meets a floor.
const SUBCROP_FLOOR_COLOR: [f32; 4] = [0.2, 0.7, 0.95, 1.0];

/// The strings of a trim run's subcrop layer.
#[derive(Debug, Default, PartialEq)]
struct Subcrop {
    roof: Vec<Vec<DVec3>>,
    floor: Vec<Vec<DVec3>>,
}

impl Subcrop {
    fn is_empty(&self) -> bool {
        self.roof.is_empty() && self.floor.is_empty()
    }
}

/// One grid's roof strings and floor strings.
type GridStrings<'a> = (&'a [Vec<DVec3>], &'a [Vec<DVec3>]);

/// The roof and floor strings of each grid of a run, as one layer holds them.
/// An interval's floor and the next one's roof are one horizon, so each floor
/// segment that is a segment of another grid's roof strings (both ends at the
/// same `contour_point_key`, either way round) is left out and what is left of
/// a floor string is chained again: the line is held once, also where the
/// lower interval is pinched or masked along part of it.
fn subcrop_strings(grids: &[GridStrings]) -> Subcrop {
    type Key = (i64, i64, i64);
    let key_of = |pair: &[DVec3]| {
        let (a, b) = (contour_point_key(pair[0]), contour_point_key(pair[1]));
        if a <= b { (a, b) } else { (b, a) }
    };
    let segments = |string: &Vec<DVec3>| -> Vec<(Key, Key)> { string.windows(2).map(key_of).collect() };
    let mut subcrop = Subcrop::default();
    for (index, (roof, floor)) in grids.iter().enumerate() {
        subcrop.roof.extend(roof.iter().cloned());
        let others: std::collections::HashSet<(Key, Key)> = grids
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .flat_map(|(_, (roof, _))| roof.iter().flat_map(segments))
            .collect();
        for string in floor.iter() {
            let unshared: Vec<[DVec3; 2]> = string.windows(2).filter(|pair| !others.contains(&key_of(pair))).map(|pair| [pair[0], pair[1]]).collect();
            if unshared.len() + 1 == string.len() || string.len() < 2 {
                subcrop.floor.push(string.clone());
            } else {
                subcrop.floor.extend(chain_contour_segments(unshared));
            }
        }
    }
    subcrop
}

/// Where a trim puts one grid it kept: the new grid's name and, for a
/// strata's interval, its record, whose strata names the new folder.
#[derive(Debug, PartialEq)]
struct TrimPlacement {
    name: String,
    stratum: Option<Stratum>,
}

/// Where a trim puts each grid it `kept`, in source order: a lone source
/// takes the typed `name`, the rest the default; each source strata gets one
/// folder, named by `unique_folder` from the trim's folder name in the
/// source's section, and its intervals keep their records with only the
/// strata renamed to it.
fn trim_placements(
    sources: &[TrimSource],
    kept: &[bool],
    surface: &str,
    side: TriSurfaceCutSide,
    name: Option<&str>,
    mut unique_folder: impl FnMut(String, SectionKind) -> String,
) -> Vec<Option<TrimPlacement>> {
    let lone = sources.len() == 1;
    // Each source strata and section with the name of its new folder.
    let mut folders: Vec<(&str, SectionKind, String)> = Vec::new();
    sources
        .iter()
        .zip(kept)
        .map(|(source, kept)| {
            if !*kept {
                return None;
            }
            let grid_name = match name {
                Some(name) if lone => name.to_owned(),
                _ => trim_name(&source.name, surface, side),
            };
            let stratum = source.stratum.as_ref().map(|stratum| {
                let held = folders.iter().find(|(strata, section, _)| *strata == stratum.strata && *section == source.section);
                let folder = match held {
                    Some((.., folder)) => folder.clone(),
                    None => {
                        let folder = unique_folder(trim_strata_name(&stratum.strata, surface, side), source.section);
                        folders.push((&stratum.strata, source.section, folder.clone()));
                        folder
                    }
                };
                Stratum {
                    strata: folder,
                    ..stratum.clone()
                }
            });
            Some(TrimPlacement { name: grid_name, stratum })
        })
        .collect()
}

/// Why a trim of grid `id` is dropped at its apply: the grid, `name` when
/// the job read it as `read`, is gone or no longer holds that grid.
fn trimmed_grid_changed(grids: &[OpenPrismoidGrid], id: PrismoidGridId, read: &Arc<PrismoidGrid>, name: &str) -> Option<String> {
    let item = grids.iter().find(|item| item.id == id);
    if item.is_some_and(|item| Arc::ptr_eq(&item.grid, read)) {
        return None;
    }
    let name = item.map_or(name, |item| item.name.as_str());
    Some(tr!("cmd-prismoid-grid-changed-while-trimming", grid = name.to_owned()))
}

/// The default name of `grid` trimmed to `surface`, saying the side kept.
pub(crate) fn trim_name(grid: &str, surface: &str, side: TriSurfaceCutSide) -> String {
    let (grid, surface) = (grid.to_owned(), surface.trim().to_owned());
    match side {
        TriSurfaceCutSide::CutTop => tr!("cmd-prismoid-grid-trim-name-below", grid = grid, surface = surface),
        TriSurfaceCutSide::CutBottom => tr!("cmd-prismoid-grid-trim-name-above", grid = grid, surface = surface),
    }
}

/// The name of the folder a strata's trimmed intervals go into.
fn trim_strata_name(strata: &str, surface: &str, side: TriSurfaceCutSide) -> String {
    let (strata, surface) = (strata.to_owned(), surface.trim().to_owned());
    match side {
        TriSurfaceCutSide::CutTop => tr!("cmd-prismoid-grid-trim-strata-name-below", strata = strata, surface = surface),
        TriSurfaceCutSide::CutBottom => tr!("cmd-prismoid-grid-trim-strata-name-above", strata = strata, surface = surface),
    }
}

/// What the trim hands each overlap to: the face and the overlap's points.
type OverlapVisit<'a> = &'a mut dyn FnMut([DVec3; 3], &[DVec3]);

/// What [`PrismoidGrid::trimmed`] takes to find the faces of `mesh` over a
/// triangle: each face with its plan overlap, relative to the lattice origin.
pub(crate) fn trim_overlaps<'a>(
    mesh: &'a Triangulation,
    spatial: &'a TriangleBvh,
    slack: f64,
) -> impl Fn(DVec2, &mut Vec<usize>, &mut Vec<DVec3>, [DVec2; 3], OverlapVisit<'_>) + Sync + 'a {
    move |origin: DVec2, stack: &mut Vec<usize>, overlap: &mut Vec<DVec3>, triangle: [DVec2; 3], visit: OverlapVisit<'_>| {
        for_each_plan_overlap_about(origin, triangle, mesh, spatial, slack, stack, overlap, |ground, shared| {
            visit(ground.map(|point| DVec3::new(point.x, point.y, point.z)), shared)
        });
    }
}

/// A trim refused, in words naming the grid and the surface.
fn trim_refusal_words(refusal: TrimRefusal, grid: &str, surface: &str) -> String {
    match refusal {
        TrimRefusal::Folded { count, first } => tr!(
            "cmd-prismoid-grid-trim-folded",
            surface = surface.to_owned(),
            count = (count as i64),
            grid = grid.to_owned(),
            x = format!("{:.2}", first.x),
            y = format!("{:.2}", first.y)
        ),
        TrimRefusal::Cancelled => tr!("common-cancelled"),
        TrimRefusal::Malformed(error) => format!("{:#}", error.context(grid.to_owned())),
        TrimRefusal::TooLarge { budget } => tr!(
            "cmd-prismoid-grid-trim-too-many-cuts",
            grid = grid.to_owned(),
            surface = surface.to_owned(),
            budget = budget.to_string()
        ),
    }
}

/// How many faces of `mesh` stand upright in plan (no more plan area than
/// `slack`, which a trim ignores) over the plan bounds of `lattices`.
fn upright_faces_over<'a>(mesh: &Triangulation, spatial: &TriangleBvh, slack: f64, lattices: impl Iterator<Item = &'a Lattice>) -> usize {
    let corners: Vec<DVec2> = lattices.flat_map(Lattice::corners).collect();
    let min = corners.iter().fold(DVec2::splat(f64::INFINITY), |min, corner| min.min(*corner));
    let max = corners.iter().fold(DVec2::splat(f64::NEG_INFINITY), |max, corner| max.max(*corner));
    let (mut count, mut stack) = (0, Vec::new());
    spatial.for_each_xy_bounds_candidate_index_with_stack(min, max, &mut stack, |face| {
        let upright = mesh
            .face_vertex_indices(face)
            .is_some_and(|face| triangle_xy_area(face.map(|index| mesh.vertices()[index])).abs() <= slack);
        count += usize::from(upright);
    });
    count
}

/// The console lines for `grid` trimmed to `surface`, keeping the ground
/// `relation` it: what became of its prismoids, or that nothing was kept (by
/// `interval` for a strata's interval); what was cut along the surface;
/// when ground was taken as touching a roof or floor within `allowance`
/// metres, the area and the most volume that could have shifted; the kept
/// grid's totals; then what was trimmed off where the surface reaches, and
/// the prismoids beyond it.
fn trim_lines(grid: &str, surface: &str, relation: &str, interval: Option<&str>, allowance: f64, trimmed: &Trimmed) -> Vec<String> {
    let (grid, surface, relation) = (grid.to_owned(), surface.to_owned(), relation.to_owned());
    let kept = trimmed.grid.as_ref().map(PrismoidGrid::summary);
    let mut lines = Vec::new();
    match (&kept, interval) {
        (Some(_), _) => lines.push(tr!(
            "cmd-prismoid-grid-trimmed",
            grid = grid,
            surface = surface.clone(),
            relation = relation,
            whole = trimmed.whole.to_string(),
            cut = trimmed.cut.to_string(),
            gone = trimmed.gone.to_string()
        )),
        (None, Some(interval)) => lines.push(tr!(
            "cmd-prismoid-grid-trim-interval-none",
            interval = interval.to_owned(),
            relation = relation,
            surface = surface.clone()
        )),
        (None, None) => lines.push(tr!("cmd-prismoid-grid-trim-nothing", grid = grid, relation = relation, surface = surface.clone())),
    }
    if trimmed.triangles > 0 {
        lines.push(tr!(
            "cmd-prismoid-grid-trim-cut",
            surface = surface.clone(),
            count = trimmed.cut.to_string(),
            triangles = trimmed.triangles.to_string()
        ));
    }
    if trimmed.touching_area > 0.0 {
        lines.push(tr!(
            "cmd-prismoid-grid-trim-touching",
            surface = surface.clone(),
            allowance = format_quantity(allowance, 3),
            area = format_quantity(trimmed.touching_area, 0),
            shift = format_quantity(allowance * trimmed.touching_area, 1)
        ));
    }
    if let Some(summary) = &kept {
        lines.extend(totals_lines(summary));
    }
    // The cuts are exact, so the source less the kept grid over the cells the
    // surface reaches is what was trimmed off.
    let reached = &trimmed.reached;
    let tonnes = |summary: &PrismoidGridSummary| summary.tonnes.as_ref().and_then(|tonnes| tonnes.value(ReserveAggregation::Sum));
    let volume = format_quantity(reached.volume - kept.as_ref().map_or(0.0, |kept| kept.volume), 0);
    lines.push(match tonnes(reached) {
        Some(off) => tr!(
            "cmd-prismoid-grid-trimmed-off-tonnes",
            volume = volume,
            tonnes = format_quantity(off - kept.as_ref().and_then(tonnes).unwrap_or(0.0), 0)
        ),
        None => tr!("cmd-prismoid-grid-trimmed-off", volume = volume),
    });
    if trimmed.beyond > 0 {
        lines.push(tr!("cmd-prismoid-grid-trimmed-beyond", count = trimmed.beyond.to_string(), surface = surface));
    }
    lines
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

/// [`values_refusal_words`] for a difference kept on `grid_name`, whose
/// refused name is the difference's own, not the other grid's.
pub(crate) fn difference_name_words(refusal: ValuesRefusal, grid_name: &str) -> String {
    match refusal {
        ValuesRefusal::Reserved { quality, .. } => tr!("cmd-prismoid-grid-difference-reserved", quality = quality),
        refusal => values_refusal_words(refusal, grid_name),
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
        BuildRefusal::Cancelled => return anyhow::Error::new(crate::app::jobs::Cancelled),
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
        userspace_log!(
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
