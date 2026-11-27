//! Data references: OMF files named in Preferences > Data whose contents are
//! shown, read-only, beside the open project's own.
//!
//! See [`crate::model::project::ReferenceId`] for what a referenced layer or
//! item is. Every reference is read again whenever a project is installed,
//! because installing one clears every item the last project had, the
//! referenced ones with them.

use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use crate::userspace_warn;
use crate::{
    app::App,
    i18n::tr,
    model::{
        ItemRef, LayerId, SceneEntityId,
        project::{ReferenceId, ReferenceStatus},
    },
    ui::state::UiDataReference,
    userspace_log,
};

/// One data reference.
#[derive(Debug)]
pub(crate) struct DataReference {
    pub(crate) id: ReferenceId,
    pub(crate) path: PathBuf,
    pub(crate) status: ReferenceStatus,
    /// Counts reads of this reference, so a read finishing after a newer one
    /// started - or after the reference was removed - is thrown away.
    generation: u64,
    /// What this reference holds but does not show, because a project layer
    /// or item of the same kind and name stands in for it. Once nothing does
    /// (deleted, or renamed), the reference shows its own again.
    #[cfg(not(target_arch = "wasm32"))]
    shadows: Vec<(crate::model::MemberKind, String)>,
}

impl DataReference {
    fn name(&self) -> String {
        self.path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

/// Something shown from a data reference that can be brought into the
/// project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ReferencedRef {
    Layer(LayerId),
    Item(ItemRef),
}

impl<'a> App<'a> {
    /// The references as Preferences and the explorer show them.
    pub(crate) fn ui_data_references(&self) -> Vec<UiDataReference> {
        self.data_references
            .iter()
            .map(|reference| UiDataReference {
                id: reference.id,
                name: reference.name(),
                path: reference.path.clone(),
                #[cfg(not(target_arch = "wasm32"))]
                status: reference.status.clone(),
                #[cfg(not(target_arch = "wasm32"))]
                sections: self.reference_sections(reference.id),
            })
            .collect()
    }

    /// What reference `id` shows, by kind, in the explorer's section order,
    /// each with its section's name.
    #[cfg(not(target_arch = "wasm32"))]
    fn reference_sections(&self, id: ReferenceId) -> Vec<(crate::model::MemberKind, String)> {
        use crate::model::MemberKind;
        let has_layers = self
            .workspace
            .active_project()
            .is_some_and(|project| project.project.document.layers().iter().any(|layer| layer.reference == Some(id)));
        let mine = |state: &crate::model::project::ProjectItemState| state.reference == Some(id);
        [
            (MemberKind::Layer, has_layers, tr!("common-designs")),
            (
                MemberKind::Triangulation,
                self.triangulations.iter().any(|item| mine(&item.state)),
                tr!("common-triangulations"),
            ),
            (MemberKind::Raster, self.raster_textures.iter().any(|item| mine(&item.state)), tr!("common-rasters")),
            (MemberKind::PointCloud, self.point_clouds.iter().any(|item| mine(&item.state)), tr!("common-point-clouds")),
            (MemberKind::BlockModel, self.block_models.iter().any(|item| mine(&item.state)), tr!("common-block-models")),
            (MemberKind::DrillHole, self.drill_holes.iter().any(|item| mine(&item.state)), tr!("ws-menubar-drillholes")),
        ]
        .into_iter()
        .filter(|(_, present, _)| *present)
        .map(|(kind, _, label)| (kind, label))
        .collect()
    }

    pub(super) fn hash_data_references(&self, hasher: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        for reference in &self.data_references {
            reference.id.hash(hasher);
            reference.path.hash(hasher);
            reference.status.hash(hasher);
        }
    }

    /// The reference files, in the order they were added, for the config.
    pub(crate) fn data_reference_paths(&self) -> Vec<PathBuf> {
        self.data_references.iter().map(|reference| reference.path.clone()).collect()
    }

    /// Take the reference list from the config. Read once a project is
    /// installed, by [`Self::reload_data_references`].
    pub(super) fn set_data_reference_paths(&mut self, paths: Vec<PathBuf>) {
        self.data_references.clear();
        for path in paths {
            if !self.data_references.iter().any(|reference| reference.path == path) {
                self.push_data_reference(path);
            }
        }
    }

    fn push_data_reference(&mut self, path: PathBuf) -> ReferenceId {
        self.next_reference_id = self.next_reference_id.wrapping_add(1).max(1);
        let id = ReferenceId(self.next_reference_id);
        self.data_references.push(DataReference {
            id,
            path,
            status: ReferenceStatus::Loading,
            generation: 0,
            #[cfg(not(target_arch = "wasm32"))]
            shadows: Vec::new(),
        });
        id
    }

    /// Show every reference's contents in the project just installed.
    pub(super) fn reload_data_references(&mut self) {
        let ids = self.data_references.iter().map(|reference| reference.id).collect::<Vec<_>>();
        for id in ids {
            self.read_data_reference(id, false);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Show again what a reference holds and the project no longer stands in
    /// for: once the project's own copy is deleted or renamed, the shared one
    /// is back. Called every frame; it reads nothing unless a shadow is gone.
    pub(crate) fn refill_freed_data_references(&mut self) {
        for id in self.freed_data_references() {
            if let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) {
                // Read once: the install records the shadows anew.
                reference.shadows.clear();
            }
            self.read_data_reference(id, true);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// The references something of which the project no longer stands in for.
    fn freed_data_references(&self) -> Vec<ReferenceId> {
        self.data_references
            .iter()
            .filter(|reference| matches!(reference.status, ReferenceStatus::Loaded { .. }))
            .filter(|reference| reference.shadows.iter().any(|(kind, name)| !self.project_has_working(*kind, name)))
            .map(|reference| reference.id)
            .collect()
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Whether the project has its own layer or item of `kind` named `name`.
    fn project_has_working(&self, kind: crate::model::MemberKind, name: &str) -> bool {
        use crate::model::MemberKind;
        let working = |state: &crate::model::project::ProjectItemState, item: &str| state.is_working() && item == name;
        match kind {
            MemberKind::Layer => self
                .workspace
                .active_project()
                .is_some_and(|project| project.project.document.layers().iter().any(|layer| layer.reference.is_none() && layer.name == name)),
            MemberKind::Triangulation => self.triangulations.iter().any(|item| working(&item.state, &item.name)),
            MemberKind::Raster => self.raster_textures.iter().any(|item| working(&item.state, &item.name)),
            MemberKind::PointCloud => self.point_clouds.iter().any(|item| working(&item.state, &item.name)),
            MemberKind::BlockModel => self.block_models.iter().any(|item| working(&item.state, &item.name)),
            MemberKind::DrillHole => self.drill_holes.iter().any(|item| working(&item.state, &item.name)),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Add `path` as a data reference, from Preferences > Data.
    pub(crate) fn add_data_reference(&mut self, path: PathBuf) -> anyhow::Result<()> {
        if self.data_references.iter().any(|reference| reference.path == path) {
            userspace_warn!("{}", tr!("data-reference-already", path = path.display().to_string()));
            return Ok(());
        }
        let id = self.push_data_reference(path);
        self.save_data_reference_config()?;
        self.read_data_reference(id, false);
        Ok(())
    }

    /// Stop referencing a file, taking everything shown from it away.
    pub(crate) fn remove_data_reference(&mut self, id: ReferenceId) -> anyhow::Result<()> {
        self.clear_referenced_content(id);
        self.data_references.retain(|reference| reference.id != id);
        self.invalidate_geometry();
        self.save_data_reference_config()
    }

    /// Read a reference's file again, for one that has changed since.
    pub(crate) fn reload_data_reference(&mut self, id: ReferenceId) {
        self.read_data_reference(id, false);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn choose_data_reference(&mut self) {
        self.spawn_file_dialog(async move {
            let path = rfd::AsyncFileDialog::new().add_filter("Open Mining Format", &["omf"]).pick_file().await?.path().to_owned();
            Some(crate::app::commands::file::FileDialogAction::AddDataReference(path))
        });
    }

    fn save_data_reference_config(&self) -> anyhow::Result<()> {
        let config = crate::app::commands::view::config_from(
            &self.editor.current_preferences(),
            self.editor.delay_products.iter().map(crate::ui::state::DelayProduct::to_stored).collect(),
            self.editor.blast_library.clone(),
            self.editor.survey.definitions.clone(),
            self.editor.survey.local_system.clone(),
            self.editor.recent_functions.clone(),
            self.data_reference_paths(),
        );
        crate::app::io::save_config(&config)?;
        Ok(())
    }

    /// Read one reference's file on a worker and show what it holds. With
    /// `refill`, what it shows already stays, and only what it is missing is
    /// added.
    fn read_data_reference(&mut self, id: ReferenceId, refill: bool) {
        let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) else {
            return;
        };
        reference.generation += 1;
        let generation = reference.generation;
        let path = reference.path.clone();
        let name = reference.name();
        // Whatever it showed before goes first: what it holds now may be
        // different, and none of it is the project's to keep.
        if !refill {
            self.clear_referenced_content(id);
        }
        let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) else {
            return;
        };
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (generation, path, name, refill);
            reference.status = ReferenceStatus::Failed(tr!("data-references-desktop-only"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !path.is_file() {
                reference.status = ReferenceStatus::Missing;
                return;
            }
            if !refill {
                reference.status = ReferenceStatus::Loading;
            }
            let Some(runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
                return;
            };
            let source_name = path.file_name().and_then(|name| name.to_str()).map(ToOwned::to_owned).unwrap_or_else(|| name.clone());
            self.spawn_job_reporting_progress(
                tr!("data-references-title"),
                vec![crate::app::jobs::JobKey::Anonymous],
                move |cancel, progress| {
                    if cancel.is_cancelled() {
                        anyhow::bail!(crate::app::jobs::Cancelled);
                    }
                    crate::model::formats::omf::reference_from_path(&source_name, &path, &progress.phase(0.0, 1.0))
                },
                move |app, result| {
                    let current = app.data_references.iter().any(|reference| reference.id == id && reference.generation == generation);
                    let same_project = app.workspace.active_project().is_some_and(|project| project.runtime_id == runtime_id);
                    if !current || !same_project {
                        return;
                    }
                    match result {
                        Ok(bundle) => app.install_data_reference(id, bundle),
                        Err(error) if error.is::<crate::app::jobs::Cancelled>() => {}
                        Err(error) => {
                            let message = format!("{error:#}");
                            userspace_warn!("{}", tr!("data-reference-load-failed", name = name.clone(), error = message.clone()));
                            if let Some(reference) = app.data_references.iter_mut().find(|reference| reference.id == id) {
                                reference.status = ReferenceStatus::Failed(message);
                            }
                        }
                    }
                },
            );
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Show a decoded reference's contents: every layer and item the project
    /// has none of the same name of, tagged with the reference and locked.
    fn install_data_reference(&mut self, id: ReferenceId, mut bundle: crate::model::formats::omf::ImportBundle) {
        let Some(runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        let was_dirty = self.project_content_is_dirty(runtime_id);
        // Into an empty scene, the reference is what there is to look at.
        let should_fit = !self.scene_has_renderables();

        // A project item of the same kind and name shadows the referenced
        // one: it is the project's own copy, brought in to be edited. What
        // this reference shows already (on a refill) is not shown twice.
        let mut shadows = Vec::new();
        {
            use crate::model::MemberKind;
            let working_layers = self
                .workspace
                .active_project()
                .map(|project| {
                    project
                        .project
                        .document
                        .layers()
                        .iter()
                        .filter(|layer| layer.reference.is_none() || layer.reference == Some(id))
                        .map(|layer| (layer.name.clone(), layer.reference.is_none()))
                        .collect::<std::collections::HashMap<_, _>>()
                })
                .unwrap_or_default();
            for design in &mut bundle.designs {
                let present = design
                    .document
                    .layers()
                    .iter()
                    .filter_map(|layer| Some((layer.id, layer.name.clone(), *working_layers.get(&layer.name)?)))
                    .collect::<Vec<_>>();
                for (layer, name, working) in present {
                    if working {
                        shadows.push((MemberKind::Layer, name));
                    }
                    design.document.delete_layer(layer);
                }
            }
            macro_rules! drop_present {
                ($kind:expr, $imported:expr, $working:expr) => {{
                    let names = $working
                        .iter()
                        .filter(|item| item.state.is_working() || item.state.reference == Some(id))
                        .map(|item| (item.name.clone(), item.state.is_working()))
                        .collect::<std::collections::HashMap<_, _>>();
                    $imported.retain(|item| match names.get(&item.loaded.name) {
                        None => true,
                        Some(working) => {
                            if *working {
                                shadows.push(($kind, item.loaded.name.clone()));
                            }
                            false
                        }
                    });
                }};
            }
            drop_present!(MemberKind::Triangulation, bundle.triangulations, self.triangulations);
            drop_present!(MemberKind::BlockModel, bundle.block_models, self.block_models);
            drop_present!(MemberKind::DrillHole, bundle.drill_holes, self.drill_holes);
            drop_present!(MemberKind::PointCloud, bundle.point_clouds, self.point_clouds);
            drop_present!(MemberKind::Raster, bundle.rasters, self.raster_textures);
        }

        let layers_before = self.layer_ids();
        let triangulations_before = self.triangulations.iter().map(|item| item.id).collect::<std::collections::HashSet<_>>();
        let block_models_before = self.block_models.iter().map(|item| item.id).collect::<std::collections::HashSet<_>>();
        let drill_holes_before = self.drill_holes.iter().map(|item| item.id).collect::<std::collections::HashSet<_>>();
        let point_clouds_before = self.point_clouds.iter().map(|item| item.id).collect::<std::collections::HashSet<_>>();
        let rasters_before = self.raster_textures.iter().map(|item| item.id).collect::<std::collections::HashSet<_>>();

        // No folders: a reference's folders are its own, and the explorer
        // shows its contents under the reference instead.
        let no_folders = std::collections::HashMap::new();
        let crate::model::formats::omf::ImportBundle {
            designs,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            ..
        } = bundle;
        for design in designs {
            if let Some(project) = self.workspace.active_project_mut() {
                crate::model::project::merge_document_unique_layers(&mut project.project.document, &design.document, &no_folders);
            }
        }
        self.install_bundle_items(
            crate::app::commands::omf::BundleIds::Fresh,
            &no_folders,
            rasters,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
        );

        let mut added = 0usize;
        let new_layers = self.layer_ids().into_iter().filter(|layer| !layers_before.contains(layer)).collect::<Vec<_>>();
        if let Some(project) = self.workspace.active_project_mut() {
            for layer in &new_layers {
                if project.project.document.set_layer_reference(*layer, Some(id)) {
                    added += 1;
                }
            }
        }
        for layer in &new_layers {
            self.editor.locked_layers.insert(*layer);
        }
        macro_rules! tag {
            ($items:expr, $before:expr) => {{
                let mut handles = Vec::new();
                for item in $items.iter_mut().filter(|item| !$before.contains(&item.id)) {
                    item.state.reference = Some(id);
                    item.state.folder = None;
                    handles.push(item.entity_id());
                    added += 1;
                }
                handles
            }};
        }
        let mut handles = tag!(self.triangulations, triangulations_before);
        handles.extend(tag!(self.block_models, block_models_before));
        handles.extend(tag!(self.drill_holes, drill_holes_before));
        handles.extend(tag!(self.point_clouds, point_clouds_before));
        for handle in handles {
            self.editor.set_entity_locked(handle, true);
        }
        for raster in self.raster_textures.iter_mut().filter(|item| !rasters_before.contains(&item.id)) {
            raster.state.reference = Some(id);
            raster.state.folder = None;
            self.editor.locked_rasters.insert(raster.id);
            added += 1;
        }
        // Showing a reference changes nothing the project saves. The project
        // content counter it advanced on the way in is set back to read as
        // saved, unless there was unsaved work already.
        if !was_dirty {
            self.mark_all_project_content_saved();
        }
        let shown = self.referenced_count(id);
        if let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) {
            reference.status = ReferenceStatus::Loaded { shown, shadowed: shadows.len() };
            reference.shadows = shadows;
            userspace_log!("{}", tr!("data-reference-loaded-log", count = shown.to_string(), name = reference.name()));
        }
        self.invalidate_geometry();
        if should_fit && added > 0 {
            self.fit_view_to_extents();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// How many layers and items reference `id` shows.
    fn referenced_count(&self, id: ReferenceId) -> usize {
        let mine = |state: &crate::model::project::ProjectItemState| state.reference == Some(id);
        self.workspace
            .active_project()
            .map_or(0, |project| project.project.document.layers().iter().filter(|layer| layer.reference == Some(id)).count())
            + self.triangulations.iter().filter(|item| mine(&item.state)).count()
            + self.block_models.iter().filter(|item| mine(&item.state)).count()
            + self.drill_holes.iter().filter(|item| mine(&item.state)).count()
            + self.point_clouds.iter().filter(|item| mine(&item.state)).count()
            + self.raster_textures.iter().filter(|item| mine(&item.state)).count()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn layer_ids(&self) -> std::collections::HashSet<LayerId> {
        self.workspace
            .active_project()
            .map(|project| project.project.document.layers().iter().map(|layer| layer.id).collect())
            .unwrap_or_default()
    }

    /// Take away everything shown from reference `id`.
    fn clear_referenced_content(&mut self, id: ReferenceId) {
        let Some(runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        let was_dirty = self.project_content_is_dirty(runtime_id);
        let layers = self
            .workspace
            .active_project()
            .map(|project| {
                project
                    .project
                    .document
                    .layers()
                    .iter()
                    .filter(|layer| layer.reference == Some(id))
                    .map(|layer| layer.id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut removed = false;
        if let Some(project) = self.workspace.active_project_mut() {
            for layer in &layers {
                let objects = project
                    .project
                    .document
                    .objects()
                    .iter()
                    .filter(|object| object.layer() == *layer)
                    .map(|object| object.id())
                    .collect::<Vec<_>>();
                for object in objects {
                    project.project.document.remove_object(object);
                }
                removed |= project.project.document.delete_layer(*layer);
            }
        }
        for layer in &layers {
            self.editor.locked_layers.remove(layer);
            self.editor.selected_layers.remove(layer);
        }
        let mut handles = Vec::new();
        macro_rules! drop_from {
            ($items:expr) => {{
                $items.retain(|item| {
                    let keep = item.state.reference != Some(id);
                    if !keep {
                        handles.push(item.entity_id());
                    }
                    keep
                });
            }};
        }
        drop_from!(self.triangulations);
        drop_from!(self.block_models);
        drop_from!(self.drill_holes);
        drop_from!(self.point_clouds);
        let rasters = self
            .raster_textures
            .iter()
            .filter(|item| item.state.reference == Some(id))
            .map(|item| item.id)
            .collect::<Vec<_>>();
        self.raster_textures.retain(|item| item.state.reference != Some(id));
        for raster in &rasters {
            self.editor.locked_rasters.remove(raster);
            for triangulation in &mut self.triangulations {
                if triangulation.raster_texture == Some(*raster) {
                    triangulation.raster_texture = None;
                }
            }
        }
        removed |= !handles.is_empty() || !rasters.is_empty();
        for handle in handles {
            self.editor.set_entity_locked(handle, false);
            self.editor.selected_handles.remove(&handle);
            if self.active_triangulation.is_some_and(|active| SceneEntityId::Triangulation(active) == handle) {
                self.active_triangulation = None;
            }
        }
        if removed {
            if !was_dirty {
                self.mark_all_project_content_saved();
            }
            self.invalidate_geometry();
        }
    }

    /// Make something shown from a reference the project's own: no longer
    /// locked, saved with the project, and unsaved work until it is.
    pub(crate) fn bring_into_project(&mut self, target: ReferencedRef) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        let kind = {
            use crate::model::MemberKind;
            match target {
                ReferencedRef::Layer(_) => MemberKind::Layer,
                ReferencedRef::Item(ItemRef::Triangulation(_)) => MemberKind::Triangulation,
                ReferencedRef::Item(ItemRef::BlockModel(_)) => MemberKind::BlockModel,
                ReferencedRef::Item(ItemRef::DrillHole(_)) => MemberKind::DrillHole,
                ReferencedRef::Item(ItemRef::PointCloud(_)) => MemberKind::PointCloud,
                ReferencedRef::Item(ItemRef::Raster(_)) => MemberKind::Raster,
            }
        };
        let (name, reference) = match target {
            ReferencedRef::Layer(layer_id) => {
                let project = self.workspace.active_project_mut().ok_or_else(|| anyhow::anyhow!(tr!("err-file-project-closed")))?;
                let Some((name, reference)) = project
                    .project
                    .document
                    .layers()
                    .iter()
                    .find(|layer| layer.id == layer_id)
                    .and_then(|layer| Some((layer.name.clone(), layer.reference?)))
                else {
                    return Ok(());
                };
                project.project.document.set_layer_reference(layer_id, None);
                project.touch_content();
                self.editor.locked_layers.remove(&layer_id);
                (name, reference)
            }
            ReferencedRef::Item(item) => {
                macro_rules! bring {
                    ($items:expr, $id:expr) => {{
                        let Some(open) = $items.iter_mut().find(|open| open.id == $id) else {
                            return Ok(());
                        };
                        let Some(reference) = open.state.reference.take() else { return Ok(()) };
                        open.state.touch();
                        (open.name.clone(), reference, Some(open.entity_id()))
                    }};
                }
                let (name, reference, handle) = match item {
                    ItemRef::Triangulation(id) => bring!(self.triangulations, id),
                    ItemRef::BlockModel(id) => bring!(self.block_models, id),
                    ItemRef::DrillHole(id) => bring!(self.drill_holes, id),
                    ItemRef::PointCloud(id) => bring!(self.point_clouds, id),
                    ItemRef::Raster(id) => {
                        let Some(open) = self.raster_textures.iter_mut().find(|open| open.id == id) else {
                            return Ok(());
                        };
                        let Some(reference) = open.state.reference.take() else { return Ok(()) };
                        open.state.touch();
                        self.editor.locked_rasters.remove(&id);
                        (open.name.clone(), reference, None)
                    }
                };
                if let Some(handle) = handle {
                    self.editor.set_entity_locked(handle, false);
                }
                self.touch_active_project_content();
                (name, reference)
            }
        };
        let reference_name = self
            .data_references
            .iter()
            .find(|candidate| candidate.id == reference)
            .map(DataReference::name)
            .unwrap_or_default();
        // The reference shows one fewer of its own, and shadows it from now
        // on: until the project's copy is deleted or renamed, when it shows
        // its own again.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(candidate) = self.data_references.iter_mut().find(|candidate| candidate.id == reference) {
            candidate.shadows.push((kind, name.clone()));
            if let ReferenceStatus::Loaded { shown, shadowed } = &mut candidate.status {
                *shown = shown.saturating_sub(1);
                *shadowed += 1;
            }
        }
        userspace_log!("{}", tr!("cmd-brought-into-project", name = name, reference = reference_name));
        self.invalidate_geometry();
        Ok(())
    }

    /// The name of what `command` would change, when that is shown from a
    /// data reference and so may not be. Locked as referenced rows are,
    /// most edits pass them by already; these are the ones that would take
    /// the lock off, or act on a row whatever its lock.
    pub(crate) fn referenced_target_of(&self, command: &crate::ui::state::UiCommand) -> Option<String> {
        use crate::{
            model::MemberTarget,
            ui::state::{RenameTarget, UiCommand},
        };
        let rename_target = |target: &RenameTarget| match *target {
            RenameTarget::Layer(id) => self.layer_is_referenced(id),
            RenameTarget::Triangulation(id) => self.item_is_referenced(ItemRef::Triangulation(id)),
            RenameTarget::Raster(id) => self.item_is_referenced(ItemRef::Raster(id)),
            RenameTarget::PointCloud(id) => self.item_is_referenced(ItemRef::PointCloud(id)),
            RenameTarget::BlockModel(id) => self.item_is_referenced(ItemRef::BlockModel(id)),
            RenameTarget::DrillHole(id) => self.item_is_referenced(ItemRef::DrillHole(id)),
            _ => None,
        };
        match command {
            UiCommand::ToggleLayerLocked(id) | UiCommand::RequestDeleteLayer(id) | UiCommand::DeleteLayer(id) => self.layer_is_referenced(*id),
            UiCommand::ToggleRasterLocked(id) => self.item_is_referenced(ItemRef::Raster(*id)),
            UiCommand::ToggleEntityLocked(handle) => match *handle {
                SceneEntityId::Triangulation(id) => self.item_is_referenced(ItemRef::Triangulation(id)),
                SceneEntityId::BlockModel(id) => self.item_is_referenced(ItemRef::BlockModel(id)),
                SceneEntityId::DrillHole(id) => self.item_is_referenced(ItemRef::DrillHole(id)),
                SceneEntityId::PointCloud(id) => self.item_is_referenced(ItemRef::PointCloud(id)),
                SceneEntityId::Raster(id) => self.item_is_referenced(ItemRef::Raster(id)),
                _ => None,
            },
            UiCommand::BeginRenameItem(target) | UiCommand::RequestDeleteItem(target) => rename_target(target),
            UiCommand::MoveToFolder { member, .. } => match member.target() {
                MemberTarget::Layer(id) => self.layer_is_referenced(id),
                MemberTarget::Item(item) => self.item_is_referenced(item),
            },
            _ => None,
        }
    }

    /// Whether `item` is shown from a data reference.
    pub(crate) fn item_is_referenced(&self, item: ItemRef) -> Option<String> {
        match item {
            ItemRef::Triangulation(id) => self
                .triangulations
                .iter()
                .find(|open| open.id == id && !open.state.is_working())
                .map(|open| open.name.clone()),
            ItemRef::BlockModel(id) => self
                .block_models
                .iter()
                .find(|open| open.id == id && !open.state.is_working())
                .map(|open| open.name.clone()),
            ItemRef::DrillHole(id) => self.drill_holes.iter().find(|open| open.id == id && !open.state.is_working()).map(|open| open.name.clone()),
            ItemRef::PointCloud(id) => self
                .point_clouds
                .iter()
                .find(|open| open.id == id && !open.state.is_working())
                .map(|open| open.name.clone()),
            ItemRef::Raster(id) => self
                .raster_textures
                .iter()
                .find(|open| open.id == id && !open.state.is_working())
                .map(|open| open.name.clone()),
        }
    }

    /// Whether layer `id` is shown from a data reference, by its name.
    pub(crate) fn layer_is_referenced(&self, id: LayerId) -> Option<String> {
        self.workspace
            .active_project()?
            .project
            .document
            .layers()
            .iter()
            .find(|layer| layer.id == id && layer.reference.is_some())
            .map(|layer| layer.name.clone())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::path::{Path, PathBuf};

    use glam::DVec3;

    use super::*;
    use crate::model::{Object, ObjectColor, formats::omf, project::ReadOnlyReason};

    /// The OMF 1 file the reference OMF 1 writer produced: two triangulated
    /// surfaces.
    fn omf1_fixture(dir: &Path) -> PathBuf {
        use base64::Engine as _;
        let text = include_str!("../../crates/omf/src/omf1/fixtures/omf1_test_proj.b64");
        let bytes = base64::engine::general_purpose::STANDARD.decode(text.split_whitespace().collect::<String>()).unwrap();
        let path = dir.join("old.omf");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// An OMF 2 file holding one design layer, "Powerlines", with a point.
    fn powerlines_file(dir: &Path) -> PathBuf {
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        let project = app.workspace.active_project_mut().unwrap();
        let layer = project.project.document.add_layer("Powerlines".to_owned(), None, [1.0; 4], true, 0.0);
        project.project.document.add_object(|id| Object::Point {
            id,
            layer,
            pos: DVec3::new(1.0, 2.0, 3.0),
            color: ObjectColor::ByLayer,
        });
        let snapshot = app.omf_save_snapshot().unwrap();
        let path = dir.join("infrastructure.omf");
        omf::write_path(snapshot, &path, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        path
    }

    fn install(app: &mut App, path: &Path) -> ReferenceId {
        let id = app.push_data_reference(path.to_owned());
        let bundle = omf::reference_from_path("reference", path, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        app.install_data_reference(id, bundle);
        id
    }

    fn runtime_id(app: &App) -> u32 {
        app.workspace.active_project().unwrap().runtime_id
    }

    #[test]
    fn omf1_opens_read_only_with_its_surfaces() {
        crate::model::asset_storage::initialize_native();
        let dir = tempfile::tempdir().unwrap();
        let path = omf1_fixture(dir.path());
        let bundle = omf::from_path("old.omf", &path, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        assert_eq!(bundle.omf1_version.as_deref(), Some("OMF-v0.9.0"));
        assert_eq!(bundle.triangulations.len(), 2);
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        app.apply_opened_omf_bundle(Some(path.clone()), "old.omf".to_owned(), bundle, crate::app::commands::omf::ViewOnOpen::Fit);
        assert_eq!(app.workspace.active_project().unwrap().read_only, Some(ReadOnlyReason::Omf1));
        assert!(app.project_lock.is_none(), "an OMF 1 file is never written, so never locked");
        // Save asks where to save the OMF 2 copy instead of writing the file.
        let id = runtime_id(&app);
        app.save_project(id).unwrap();
        assert_eq!(app.editor.read_only_save_prompt, Some(id));
        assert!(app.pending_saves.is_empty());
        // The title bar marks it read-only (the words are the language's).
        assert!(app.project_view().titled_project().unwrap().read_only.is_some());
    }

    /// Every element type the reference OMF 1 writer produces reaches the
    /// project as something Incline shows.
    #[test]
    fn omf1_from_the_reference_writer_opens_every_type() {
        use base64::Engine as _;
        crate::model::asset_storage::initialize_native();
        let dir = tempfile::tempdir().unwrap();
        let text = include_str!("../../crates/omf/src/omf1/fixtures/every_type_v1.b64");
        let path = dir.path().join("every_type.omf");
        std::fs::write(
            &path,
            base64::engine::general_purpose::STANDARD.decode(text.split_whitespace().collect::<String>()).unwrap(),
        )
        .unwrap();
        let bundle = omf::from_path("every_type.omf", &path, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        for warning in &bundle.warnings {
            eprintln!("warning: {warning}");
        }
        eprintln!(
            "designs={} layers={} triangulations={} block_models={} point_clouds={} drill_holes={} rasters={}",
            bundle.designs.len(),
            bundle.designs.iter().map(|design| design.document.layers().len()).sum::<usize>(),
            bundle.triangulations.len(),
            bundle.block_models.len(),
            bundle.point_clouds.len(),
            bundle.drill_holes.len(),
            bundle.rasters.len()
        );
        assert!(bundle.item_count() >= 4, "points, lines, surfaces and blocks all come through");
        assert!(!bundle.block_models.is_empty());
        assert!(!bundle.triangulations.is_empty());
    }

    #[test]
    fn open_project_is_locked_and_a_second_session_gets_it_read_only() {
        crate::model::asset_storage::initialize_native();
        let dir = tempfile::tempdir().unwrap();
        let path = powerlines_file(dir.path());
        let open = |app: &mut App| {
            let bundle = omf::from_path("infrastructure.omf", &path, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
            app.apply_opened_omf_bundle(Some(path.clone()), "infrastructure.omf".to_owned(), bundle, crate::app::commands::omf::ViewOnOpen::Fit);
        };
        let mut first = App::default();
        first.start_untitled_project().unwrap();
        open(&mut first);
        assert!(first.project_lock.is_some());
        assert!(first.workspace.active_project().unwrap().read_only.is_none());
        assert!(crate::model::file_lock::lock_path(&path).exists());

        let mut second = App::default();
        second.start_untitled_project().unwrap();
        open(&mut second);
        assert!(matches!(second.workspace.active_project().unwrap().read_only, Some(ReadOnlyReason::LockedBy(_))));
        let id = runtime_id(&second);
        second.save_project(id).unwrap();
        assert_eq!(second.editor.read_only_save_prompt, Some(id), "Save offers Save As or Take Over");

        // Closing the project lets the file go.
        first.start_untitled_project().unwrap();
        assert!(!crate::model::file_lock::lock_path(&path).exists());
    }

    #[test]
    fn referenced_content_is_locked_shadowed_and_never_saved_until_brought_in() {
        crate::model::asset_storage::initialize_native();
        let dir = tempfile::tempdir().unwrap();
        let surfaces = omf1_fixture(dir.path());
        let infrastructure = powerlines_file(dir.path());

        let mut app = App::default();
        app.start_untitled_project().unwrap();
        let survey = install(&mut app, &surfaces);
        let lines = install(&mut app, &infrastructure);
        let id = runtime_id(&app);
        assert!(!app.project_content_is_dirty(id), "showing references is not unsaved work");

        let referenced = app.triangulations.iter().filter(|item| item.state.reference == Some(survey)).count();
        assert_eq!(referenced, 2);
        // Shown but unloaded, whatever the reference file recorded: nothing
        // of it is read until it is loaded.
        assert!(app.triangulations.iter().all(|item| !item.state.loaded && !item.state.hidden));
        assert!(
            app.workspace
                .active_project()
                .unwrap()
                .project
                .document
                .layers()
                .iter()
                .filter(|layer| layer.reference.is_some())
                .all(|layer| !layer.loaded && !layer.hidden)
        );
        // Read-only, but still something to snap to.
        app.page_lock_key = None;
        app.sync_page_locks();
        assert!(app.triangulations.iter().all(|item| app.editor.pick_frozen.contains(&item.entity_id())));
        assert!(app.triangulations.iter().all(|item| !app.editor.snap_frozen.contains(&item.entity_id())));
        assert!(app.triangulations.iter().all(|item| !app.editor.orbit_frozen.contains(&item.entity_id())));
        assert!(app.triangulations.iter().all(|item| app.editor.frozen_handles.contains(&item.entity_id())));
        let layer = app
            .workspace
            .active_project()
            .unwrap()
            .project
            .document
            .layers()
            .iter()
            .find(|layer| layer.reference == Some(lines))
            .map(|layer| layer.id)
            .expect("the referenced layer is shown");
        assert!(app.editor.locked_layers.contains(&layer));

        // Loading it reads it back from the reference: the residency job is
        // keyed by the layer's content hash, which a referenced layer has too.
        assert!(app.workspace.active_project().unwrap().project.document.deferred_layers.contains_key(&layer));
        assert!(app.restore_layers_for(vec![layer], |_| {}), "a referenced layer is read back when loaded");
        assert!(!app.project_content_is_dirty(id), "loading a referenced layer is not unsaved work");

        // A save writes none of it.
        let snapshot = app.omf_save_snapshot();
        assert!(snapshot.is_err() || snapshot.as_ref().is_ok_and(|snapshot| snapshot.triangulations.is_empty()));

        // Edits that would take the lock off are refused.
        let unlock = crate::ui::state::UiCommand::ToggleLayerLocked(layer);
        assert!(app.referenced_target_of(&unlock).is_some());

        // Brought in, the layer is the project's: unlocked, unsaved, saved.
        app.bring_into_project(ReferencedRef::Layer(layer)).unwrap();
        assert!(!app.editor.locked_layers.contains(&layer));
        assert!(app.project_content_is_dirty(id));
        let snapshot = app.omf_save_snapshot().unwrap();
        let saved_layers = snapshot
            .designs
            .as_ref()
            .unwrap()
            .document
            .layers()
            .iter()
            .map(|layer| layer.name.clone())
            .collect::<Vec<_>>();
        assert!(saved_layers.contains(&"Powerlines".to_owned()), "{saved_layers:?}");
        assert!(snapshot.triangulations.is_empty());

        // Read again, the reference no longer shows what the project now has.
        app.reload_data_reference(lines);
        let lines_bundle = omf::reference_from_path("reference", &infrastructure, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        app.install_data_reference(lines, lines_bundle);
        let powerlines = app
            .workspace
            .active_project()
            .unwrap()
            .project
            .document
            .layers()
            .iter()
            .filter(|layer| layer.name.starts_with("Powerlines"))
            .count();
        assert_eq!(powerlines, 1, "the project's copy shadows the referenced one");
        assert!(matches!(
            app.data_references.iter().find(|reference| reference.id == lines).unwrap().status,
            ReferenceStatus::Loaded { shadowed: 1, .. }
        ));
        assert!(app.freed_data_references().is_empty());

        // The project's copy deleted, the reference shows its own again, and
        // what it showed already stays as it was.
        let working = app
            .workspace
            .active_project()
            .unwrap()
            .project
            .document
            .layers()
            .iter()
            .find(|layer| layer.name == "Powerlines" && layer.reference.is_none())
            .unwrap()
            .id;
        app.workspace.active_project_mut().unwrap().project.document.delete_layer(working);
        assert_eq!(app.freed_data_references(), vec![lines]);
        let shown_before = app.referenced_count(lines);
        app.data_references.iter_mut().find(|reference| reference.id == lines).unwrap().shadows.clear();
        let lines_bundle = omf::reference_from_path("reference", &infrastructure, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        app.install_data_reference(lines, lines_bundle);
        let referenced = app
            .workspace
            .active_project()
            .unwrap()
            .project
            .document
            .layers()
            .iter()
            .filter(|layer| layer.name == "Powerlines" && layer.reference == Some(lines))
            .count();
        assert_eq!(referenced, 1, "the referenced copy is back, once");
        assert_eq!(app.referenced_count(lines), shown_before + 1);
        assert!(matches!(
            app.data_references.iter().find(|reference| reference.id == lines).unwrap().status,
            ReferenceStatus::Loaded { shadowed: 0, .. }
        ));
        assert!(app.freed_data_references().is_empty());

        // Removing a reference takes its content away and leaves the project as it was.
        let dirty_before = app.project_content_is_dirty(id);
        app.data_references.retain(|reference| reference.id != survey);
        app.clear_referenced_content(survey);
        assert!(app.triangulations.is_empty());
        assert_eq!(app.project_content_is_dirty(id), dirty_before);
    }
}
