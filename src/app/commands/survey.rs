//! Reusable mine-grid definitions and atomic, backgrounded transformed copies.
use std::{collections::HashMap, sync::Arc};

use anyhow::{Result, anyhow, ensure};
use glam::DVec3;

use crate::{
    app::{
        App,
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{Command, OpenItem, SceneEntityId, crs::SurveyTransform, formats::mesh_data, point_cloud, survey::MineGridTransform, triangulation},
    userspace_log, userspace_warn,
};

impl App<'_> {
    /// Mark `system` as the site's mine coordinate system, or the reference
    /// frame with `None`, and write the choice to the config.
    pub(crate) fn set_survey_local_system(&mut self, system: Option<String>) -> Result<()> {
        if let Some(name) = &system {
            ensure!(self.editor.survey.definitions.iter().any(|d| &d.name == name), "{}", tr!("survey-system-missing"));
        }
        self.write_survey_config(self.editor.survey.definitions.clone(), system.clone())?;
        self.editor.survey.local_system = system;
        self.refresh_axis_names();
        Ok(())
    }

    /// Put the chosen system's axis names into effect across the interface.
    ///
    /// Called after anything that could change which system is chosen or what
    /// it calls its axes, so a rename of "Z" to "RL" reaches every readout in
    /// the same frame it is typed.
    pub(crate) fn refresh_axis_names(&self) {
        let names = self
            .editor
            .survey
            .local_system
            .as_ref()
            .and_then(|name| self.editor.survey.definitions.iter().find(|definition| &definition.name == name))
            .and_then(|definition| definition.axis_names.clone());
        crate::model::survey::set_axis_names(names);
    }

    /// Persist the coordinate systems alongside every other stored preference.
    fn write_survey_config(&self, definitions: Vec<crate::model::survey::SystemDefinition>, local: Option<String>) -> Result<()> {
        let config = super::view::config_from(
            &self.editor.current_preferences(),
            self.editor.workspace_order,
            self.editor.delay_products.iter().map(crate::ui::state::DelayProduct::to_stored).collect(),
            definitions,
            local,
        );
        Ok(crate::app::io::save_config(&config)?)
    }

    /// Write one coordinate system to the config, replacing `target` when it
    /// names an existing system and adding one otherwise.
    pub(crate) fn save_survey_definition(&mut self, target: Option<String>, definition: crate::model::survey::SystemDefinition) -> Result<()> {
        let name = definition.name.trim().to_owned();
        ensure!(!name.is_empty(), "{}", tr!("survey-name-required"));
        let mut definitions = self.editor.survey.definitions.clone();
        ensure!(
            !definitions.iter().any(|d| d.name == name && Some(&d.name) != target.as_ref()),
            "{}",
            tr!("survey-name-exists")
        );
        if let Some(old) = &target {
            ensure!(definitions.iter().any(|d| &d.name == old), "{}", tr!("survey-system-missing"));
            definitions.retain(|d| &d.name != old);
        }
        definitions.push(crate::model::survey::SystemDefinition {
            name: name.clone(),
            kind: definition.kind,
            axis_names: definition.axis_names,
        });
        // Grids name their parent, so a rename has to be written through to
        // the children before anything is saved - naming a parent is only
        // simpler than embedding it if this happens in one place.
        if let Some(old) = &target
            && old != &name
        {
            for definition in &mut definitions {
                if let crate::model::survey::SystemKind::Grid { parent, .. } = &mut definition.kind
                    && *parent == *old
                {
                    *parent = name.clone();
                }
            }
        }
        // A grid whose parent chain cannot be resolved - a missing parent, or
        // one that leads back here - would fail on every use, so it is caught
        // before it reaches the config rather than after.
        crate::model::survey::resolve_system(&name, &definitions)?;
        let first = definitions.len() == 1;
        definitions.sort_by(|a, b| a.name.cmp(&b.name));
        // A rename carries the mine-system mark with the system it belongs to,
        // and the first system a site defines takes it, so a fresh config is
        // never left transforming into a frame nobody chose.
        let mut local = self.editor.survey.local_system.clone();
        if (target.is_some() && local == target) || (first && local.is_none()) {
            local = Some(name.clone());
        }
        self.write_survey_config(definitions.clone(), local.clone())?;

        let state = &mut self.editor.survey;
        state.definitions = definitions;
        state.local_system = local;
        if target.is_some() {
            if state.source_system == target {
                state.source_system = Some(name.clone());
            }
            if state.target_system == target {
                state.target_system = Some(name.clone());
            }
        }
        // A new system is selected because the user just asked for it; a
        // rename is followed only while the list still stands on the system
        // being renamed, so clicking away mid-edit saves without yanking the
        // selection back.
        if target.is_none() || state.editing_name == target {
            state.edit_definition(Some(name));
        }
        self.refresh_axis_names();
        Ok(())
    }

    pub(crate) fn delete_survey_definition(&mut self, name: &str) -> Result<()> {
        let mut definitions = self.editor.survey.definitions.clone();
        ensure!(definitions.iter().any(|d| d.name == name), "{}", tr!("survey-system-missing"));
        // Deleting what other grids are defined against would leave them
        // pointing at nothing. Naming the dependants is more use than
        // refusing without saying why.
        let dependants: Vec<&str> = definitions
            .iter()
            .filter(|definition| matches!(&definition.kind, crate::model::survey::SystemKind::Grid { parent, .. } if parent == name))
            .map(|definition| definition.name.as_str())
            .collect();
        ensure!(
            dependants.is_empty(),
            "{}",
            tr!("survey-system-in-use", name = name.to_owned(), dependants = dependants.join(", "))
        );
        definitions.retain(|d| d.name != name);
        let mut local = self.editor.survey.local_system.clone();
        if local.as_deref() == Some(name) {
            local = None;
        }
        self.write_survey_config(definitions.clone(), local.clone())?;

        let state = &mut self.editor.survey;
        state.definitions = definitions;
        state.local_system = local;
        // Anything still pointing at the deleted system falls back to the
        // reference frame rather than to a name that no longer resolves.
        if state.source_system.as_deref() == Some(name) {
            state.source_system = None;
        }
        if state.target_system.as_deref() == Some(name) {
            state.target_system = None;
        }
        if state.editing_name.as_deref() == Some(name) {
            state.edit_definition(None);
        }
        self.refresh_axis_names();
        Ok(())
    }

    pub(crate) fn transform_survey_selection(&mut self) -> Result<()> {
        let transform = self.editor.survey.selected_survey_transform()?;
        let target_system = self.editor.survey.resolve(&self.editor.survey.target_system)?.to_stored();
        let project = self.workspace.active_project().ok_or_else(|| anyhow!("{}", tr!("survey-wrong-project")))?;
        let runtime_id = project.runtime_id;
        let mut keys = vec![JobKey::Project {
            runtime_id,
            document_revision: project.project.document.revision(),
        }];
        let mut objects = Vec::new();
        let mut layers = HashMap::new();
        let mut items = Vec::new();
        for handle in &self.editor.selected_handles {
            match *handle {
                SceneEntityId::Object(id) => {
                    let object = project.project.document.get_object(id).ok_or_else(|| anyhow!("{}", tr!("survey-wrong-project")))?;
                    let layer = project.project.document.layer(object.layer()).ok_or_else(|| anyhow!("{}", tr!("survey-unavailable")))?;
                    ensure!(layer.loaded, "{}", tr!("survey-unavailable"));
                    layers.insert(layer.id, layer.clone());
                    objects.push(object.clone());
                }
                SceneEntityId::Triangulation(id) => {
                    let item = self
                        .triangulations
                        .iter()
                        .find(|item| item.id == id)
                        .ok_or_else(|| anyhow!("{}", tr!("survey-unavailable")))?;
                    ensure!(item.state.loaded && item.state.deferred.is_none(), "{}", tr!("survey-unavailable"));
                    items.push(OpenItem::Triangulation(Box::new(item.clone())));
                    keys.push(JobKey::Triangulation(id));
                }
                SceneEntityId::BlockModel(id) => {
                    let item = self
                        .block_models
                        .iter()
                        .find(|item| item.id == id)
                        .ok_or_else(|| anyhow!("{}", tr!("survey-unavailable")))?;
                    ensure!(item.state.loaded && item.state.deferred.is_none(), "{}", tr!("survey-unavailable"));
                    items.push(OpenItem::BlockModel(Box::new(item.clone())));
                    keys.push(JobKey::BlockModel(id));
                }
                SceneEntityId::PointCloud(id) => {
                    let item = self
                        .point_clouds
                        .iter()
                        .find(|item| item.id == id)
                        .ok_or_else(|| anyhow!("{}", tr!("survey-unavailable")))?;
                    ensure!(item.state.loaded && item.state.deferred.is_none(), "{}", tr!("survey-unavailable"));
                    items.push(OpenItem::PointCloud(Box::new(item.clone())));
                    keys.push(JobKey::PointCloud(id));
                }
                SceneEntityId::Raster(id) => {
                    let item = self
                        .raster_textures
                        .iter()
                        .find(|item| item.id == id)
                        .ok_or_else(|| anyhow!("{}", tr!("survey-unavailable")))?;
                    ensure!(item.state.loaded && item.state.deferred.is_none(), "{}", tr!("survey-unavailable"));
                    items.push(OpenItem::Raster(Box::new(item.clone())));
                    keys.push(JobKey::Raster(id));
                }
                SceneEntityId::DrillHole(id) => {
                    let item = self.drill_holes.iter().find(|item| item.id == id).ok_or_else(|| anyhow!("{}", tr!("survey-unavailable")))?;
                    ensure!(item.state.loaded && item.state.deferred.is_none(), "{}", tr!("survey-unavailable"));
                    items.push(OpenItem::DrillHole(Box::new(item.clone())));
                    keys.push(JobKey::DrillHole(id));
                }
            }
        }
        ensure!(!objects.is_empty() || !items.is_empty(), "{}", tr!("survey-empty-selection"));
        let source_revisions = items
            .iter()
            .map(|item| {
                let revision = match item {
                    OpenItem::Triangulation(item) => item.state.revision(),
                    OpenItem::BlockModel(item) => item.state.revision(),
                    OpenItem::PointCloud(item) => item.state.revision(),
                    OpenItem::DrillHole(item) => item.state.revision(),
                    OpenItem::Raster(item) => item.state.revision(),
                };
                (item.item_ref(), revision)
            })
            .collect::<Vec<_>>();
        // A layer's elevation is a coordinate like any other on it, so it
        // moves with the objects rather than being left behind pointing at a
        // bench that is no longer there.
        let mut layer_elevations = Vec::new();
        for layer in layers.values() {
            let after = transform.point(DVec3::new(0.0, 0.0, f64::from(layer.elevation)))?.z as f32;
            ensure!(after.is_finite(), "{}", tr!("survey-invalid-transform"));
            if after != layer.elevation {
                layer_elevations.push((layer.id, layer.elevation, after));
            }
        }
        self.spawn_job(
            tr!("survey-working"),
            keys,
            move |cancel| {
                for object in &mut objects {
                    ensure!(!cancel.is_cancelled(), "Cancelled");
                    crate::model::survey::transform_object(&transform, object, cancel)?;
                }
                for item in &mut items {
                    ensure!(!cancel.is_cancelled(), "Cancelled");
                    transform_item(item, &transform, &target_system, cancel)?;
                }
                Ok((objects, items))
            },
            move |app, result| {
                if !app.workspace.active_project().is_some_and(|p| p.runtime_id == runtime_id)
                    || source_revisions.iter().any(|(item, revision)| {
                        !app.project_item_state(*item)
                            .is_some_and(|state| state.revision() == *revision && state.loaded && state.deferred.is_none())
                    })
                {
                    userspace_warn!("{}", tr!("survey-stale"));
                    return;
                }
                let (objects, items) = match result {
                    Ok(result) => result,
                    Err(error) => {
                        userspace_warn!("{}", tr!("survey-failed", error = error.to_string()));
                        return;
                    }
                };
                let Some(project) = app.workspace.projects.iter().find(|p| p.runtime_id == runtime_id) else {
                    return;
                };
                let (designs, mut meshes, mut models, mut clouds, mut holes, mut rasters) = (objects.len(), 0, 0, 0, 0, 0);
                for item in &items {
                    match item {
                        OpenItem::Triangulation(_) => meshes += 1,
                        OpenItem::BlockModel(_) => models += 1,
                        OpenItem::PointCloud(_) => clouds += 1,
                        OpenItem::DrillHole(_) => holes += 1,
                        OpenItem::Raster(_) => rasters += 1,
                    }
                }

                // Everything below rewrites what is already there. The
                // converted data is the same data in different numbers - the
                // same design on the same layer, the same mesh under the same
                // name - so it replaces itself rather than arriving beside
                // itself, and every id downstream stays pointed at it.
                let mut commands = Vec::new();
                for after in objects {
                    let Some(before) = project.project.document.get_object(after.id()).cloned() else {
                        userspace_warn!("{}", tr!("survey-stale"));
                        return;
                    };
                    commands.push(Command::Replace { before, after });
                }
                for (id, before, after) in layer_elevations {
                    commands.push(Command::SetLayerElevation { id, before, after });
                }
                for item in items {
                    commands.push(Command::ReplaceItem {
                        item: item.item_ref(),
                        other: Some(item),
                    });
                }
                app.execute_edit_for(runtime_id, Command::Batch(commands));
                let message = tr!(
                    "survey-completed",
                    items = crate::model::survey::describe_counts([designs, meshes, models, clouds, holes, rasters])
                );
                userspace_log!("{}", message);
            },
        );
        Ok(())
    }
}

fn transform_item(item: &mut OpenItem, transform: &SurveyTransform, target_system: &str, cancel: &CancelFlag) -> Result<()> {
    match item {
        OpenItem::Triangulation(item) => {
            let vertices = item
                .mesh
                .vertices()
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    if i % 4096 == 0 {
                        ensure!(!cancel.is_cancelled(), "Cancelled");
                    }
                    let p = transform.point(DVec3::from_array(v.as_array()))?;
                    Ok(mesh_data::Vertex::new(p.x, p.y, p.z))
                })
                .collect::<Result<Vec<_>>>()?;
            let faces = item
                .mesh
                .face_vertex_indices_iter()
                .enumerate()
                .map(|(i, face)| {
                    if i % 4096 == 0 {
                        ensure!(!cancel.is_cancelled(), "Cancelled");
                    }
                    Ok(face.map(|index| index as u32))
                })
                .collect::<Result<Vec<_>>>()?;
            let mesh = mesh_data::Triangulation::from_vertices_and_faces(vertices, faces)?;
            ensure!(!cancel.is_cancelled(), "Cancelled");
            item.spatial = Arc::new(crate::model::spatial::TriangleBvh::build(&mesh));
            ensure!(!cancel.is_cancelled(), "Cancelled");
            item.surface_face_order = Arc::new(triangulation::morton_surface_face_order(&mesh));
            item.mesh = Arc::new(mesh);
            item.raster_texture = None;
        }
        OpenItem::BlockModel(item) => {
            // A block model is a regular grid stated as an origin, a rotation
            // and a cell size. A reprojection does not keep it regular - cells
            // would have to be resampled into a new grid, losing the values
            // they carry - so it is refused rather than approximated.
            let grid = transform.grid().ok_or_else(|| anyhow!("{}", tr!("survey-needs-grid-block-model")))?;
            let origin = transform.point(item.model.origin())?;
            let rotation = grid.rotation() * item.model.rotation();
            item.model = item.model.clone().with_transform(origin, rotation)?;
            item.model.metadata.lower *= grid.scale;
            item.model.metadata.upper *= grid.scale;
            ensure!(
                item.model.metadata.lower.is_finite() && item.model.metadata.upper.is_finite(),
                "Invalid scaled model extent"
            );
            if grid.scale != 1.0 {
                item.blocks = Arc::new(item.blocks.scaled(grid.scale, cancel)?);
                item.uniform_grid = item.uniform_grid.as_ref().map(|cells| cells.scaled(grid.scale));
                if let Some(slice) = &mut item.slice {
                    slice.min *= grid.scale;
                    slice.max *= grid.scale;
                }
            }
            let mut min = DVec3::splat(f64::INFINITY);
            let mut max = DVec3::splat(f64::NEG_INFINITY);
            // Implicit full grids have an exact O(1) extent; otherwise inspect
            // renderable cells, polling cancellation between small batches.
            let implicit = item.blocks.implicit_local_bounds().filter(|_| item.renderable_block_indices.is_all());
            let count = if implicit.is_some() { 1 } else { item.renderable_block_indices.len() };
            for index in 0..count {
                if index % 4096 == 0 {
                    ensure!(!cancel.is_cancelled(), "Cancelled");
                }
                let bounds = implicit
                    .or_else(|| item.renderable_block_indices.get(index).and_then(|i| item.blocks.get(i)))
                    .ok_or_else(|| anyhow!("Missing block bounds"))?;
                for x in [bounds.lower.x, bounds.upper.x] {
                    for y in [bounds.lower.y, bounds.upper.y] {
                        for z in [bounds.lower.z, bounds.upper.z] {
                            let p = item.model.local_to_world(DVec3::new(x, y, z));
                            ensure!(p.is_finite(), "Invalid transformed block coordinate");
                            min = min.min(p);
                            max = max.max(p);
                        }
                    }
                }
            }
            item.world_bounds = (count > 0).then_some((min, max));
        }
        OpenItem::PointCloud(item) => {
            let points = item
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    if i % 4096 == 0 {
                        ensure!(!cancel.is_cancelled(), "Cancelled");
                    }
                    transform.point(*p)
                })
                .collect::<Result<Vec<_>>>()?;
            let bounds = point_cloud::finite_bounds(&points).ok_or_else(|| anyhow!("Empty point cloud"))?;
            ensure!(!cancel.is_cancelled(), "Cancelled");
            item.prepared = Arc::new(point_cloud::prepare_for_render(
                &points,
                item.colors.as_deref().map(Vec::as_slice),
                item.classifications.as_deref().map(Vec::as_slice),
                bounds,
            ));
            item.points = Arc::new(points);
            item.bounds = bounds;
            item.point_size *= transform.length_scale() as f32;
            ensure!(item.point_size.is_finite(), "Invalid transformed point size");
        }
        OpenItem::DrillHole(item) => {
            let dataset = Arc::make_mut(&mut item.dataset);
            for (index, hole) in dataset.holes.iter_mut().enumerate() {
                if index % 256 == 0 {
                    ensure!(!cancel.is_cancelled(), "Cancelled");
                }
                hole.collar = transform.point(hole.collar)?;
                for station in &mut hole.trace {
                    station.position = transform.point(station.position)?;
                    // Depth is measured along the hole, not across the grid,
                    // so it follows the scale and nothing else. At the scale
                    // of 1 a coordinate conversion normally carries, every
                    // depth, diameter and interval below is left exactly as
                    // drilled.
                    station.depth *= transform.length_scale();
                    ensure!(station.depth.is_finite(), "{}", tr!("survey-invalid-transform"));
                }
                if let Some(diameter) = &mut hole.diameter {
                    *diameter *= transform.length_scale();
                    ensure!(diameter.is_finite(), "{}", tr!("survey-invalid-transform"));
                }
                for (from, to) in &mut hole.render_ranges {
                    *from *= transform.length_scale();
                    *to *= transform.length_scale();
                }
                for interval in &mut hole.intervals {
                    interval.from *= transform.length_scale();
                    interval.to *= transform.length_scale();
                    ensure!(interval.from.is_finite() && interval.to.is_finite(), "{}", tr!("survey-invalid-transform"));
                }
            }
            // Rebuilt rather than reconstructed through `DrillHoleDataset::new`,
            // which sorts the holes: the ties and initiations beside them name
            // holes by index, so a re-sort here would repoint every one of them.
            dataset.refresh_bounds();
        }
        OpenItem::Raster(item) => {
            // A raster is not resampled. Its georeferencing is already a full
            // affine map from world XY to texture UV, so moving the image is
            // composing this transform's inverse into that map - six numbers,
            // and not one pixel read. The image stays exactly as sharp as it
            // was, which resampling could not promise.
            //
            // This holds only while the transform is affine, which a grid
            // shift is. A conversion between two curved-earth systems is not,
            // and would have to warp the pixels themselves.
            let grid = transform.grid().ok_or_else(|| anyhow!("{}", tr!("survey-needs-grid-raster")))?;
            item.world_to_uv = composed_world_to_uv(item.world_to_uv, grid)?;
            item.projection = target_system.to_owned();
        }
    }
    ensure!(!cancel.is_cancelled(), "Cancelled");
    Ok(())
}

/// Re-aim a raster's world-to-texture map through a grid transform.
///
/// `world_to_uv` reads `u = a*x + b*y + c`, `v = d*x + e*y + f`. After the
/// data moves, a world point `p'` has to land on the texel its original `p`
/// did, so the new map is the old one composed with the transform undone:
/// `new(p') = old(T-1(p'))`. Only the XY block of the transform matters -
/// a texture has no third axis to shear.
fn composed_world_to_uv(world_to_uv: [f64; 6], transform: MineGridTransform) -> Result<[f64; 6]> {
    let [a, b, c, d, e, f] = world_to_uv;
    // T-1(p') = source_origin + R^T (p' - target_origin) / scale, so its own
    // linear part is R^T / scale and its offset is what the origin leaves.
    let inverse_linear = transform.rotation().transpose() * (1.0 / transform.scale);
    let inverse_offset = transform.source_origin - inverse_linear * transform.target_origin;
    let composed = |row: [f64; 2], constant: f64| {
        [
            row[0] * inverse_linear.x_axis.x + row[1] * inverse_linear.x_axis.y,
            row[0] * inverse_linear.y_axis.x + row[1] * inverse_linear.y_axis.y,
            row[0] * inverse_offset.x + row[1] * inverse_offset.y + constant,
        ]
    };
    let [a, b, c] = composed([a, b], c);
    let [d, e, f] = composed([d, e], f);
    let out = [a, b, c, d, e, f];
    ensure!(out.iter().all(|value| value.is_finite()), "{}", tr!("survey-invalid-transform"));
    Ok(out)
}
