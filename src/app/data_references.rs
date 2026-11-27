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
        FolderId, ItemRef, LayerId, SceneEntityId, SectionKind,
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
}

impl DataReference {
    /// Its file's name, which links to its items are saved by.
    fn file_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.path.display().to_string())
    }

    fn name(&self) -> String {
        self.path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Whether `a` and `b` name one file, however each is spelt.
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
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
        });
        id
    }

    /// Show every reference's contents in the project just installed.
    pub(super) fn reload_data_references(&mut self) {
        let ids = self.data_references.iter().map(|reference| reference.id).collect::<Vec<_>>();
        for id in ids {
            self.read_data_reference(id);
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
        self.read_data_reference(id);
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
        self.read_data_reference(id);
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

    /// Read one reference's file on a worker and show what it holds.
    fn read_data_reference(&mut self, id: ReferenceId) {
        let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) else {
            return;
        };
        reference.generation += 1;
        let generation = reference.generation;
        let path = reference.path.clone();
        let name = reference.name();
        // Whatever it showed before goes first: what it holds now may be
        // different, and none of it is the project's to keep.
        self.clear_referenced_content(id);
        let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) else {
            return;
        };
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (generation, path, name);
            reference.status = ReferenceStatus::Failed(tr!("data-references-desktop-only"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !path.is_file() {
                reference.status = ReferenceStatus::Missing;
                return;
            }
            if self
                .workspace
                .active_project()
                .and_then(|project| project.path.as_deref())
                .is_some_and(|open| same_file(open, &path))
            {
                let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) else {
                    return;
                };
                reference.status = ReferenceStatus::OpenAsProject;
                return;
            }
            let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) else {
                return;
            };
            reference.status = ReferenceStatus::Loading;
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
    /// Show a decoded reference's contents: every layer and item in it,
    /// tagged with the reference and locked. Its names are its own: the
    /// project may have items of the same names, being another file.
    fn install_data_reference(&mut self, id: ReferenceId, bundle: crate::model::formats::omf::ImportBundle) {
        let Some(runtime_id) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        let was_dirty = self.project_content_is_dirty(runtime_id);
        // The view is left where it is: a reference comes in unloaded, so
        // there is nothing of it to frame, and its reads land a moment after
        // the project opens, when a fit would move the view under the user.

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
                crate::model::project::merge_reference_layers(&mut project.project.document, &design.document);
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

        let new_layers = self.layer_ids().into_iter().filter(|layer| !layers_before.contains(layer)).collect::<Vec<_>>();
        if let Some(project) = self.workspace.active_project_mut() {
            for layer in &new_layers {
                project.project.document.set_layer_reference(*layer, Some(id));
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
        }
        // The project's links to its items, by name, back on them.
        self.resolve_reference_links(id);
        // Showing a reference changes nothing the project saves. The project
        // content counter it advanced on the way in is set back to read as
        // saved, unless there was unsaved work already.
        if !was_dirty {
            self.mark_all_project_content_saved();
        }
        let shown = self.referenced_count(id);
        if let Some(reference) = self.data_references.iter_mut().find(|reference| reference.id == id) {
            reference.status = ReferenceStatus::Loaded { shown };
            userspace_log!("{}", tr!("data-reference-loaded-log", count = shown.to_string(), name = reference.name()));
        }
        self.invalidate_geometry();
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
        // What links to its items is kept by name, to match its items again
        // when it is read: their ids are new each time.
        self.capture_reference_links();
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

    /// Copy something shown from a reference into the project, as the
    /// project's own to edit, and unload the reference's: one undo step.
    /// The copy keeps its name, unless the project has one of that name
    /// already: it is "(2)" then, as anything else coming in would be. The
    /// reference's own stays the reference's, whatever becomes of the copy.
    /// `place` is the section and collection the copy goes in, where it was
    /// dragged to; the root of its own section, without one.
    pub(crate) fn bring_into_project(&mut self, target: ReferencedRef, place: Option<(SectionKind, Option<FolderId>)>) -> anyhow::Result<()> {
        // What is copied has to be in memory: read in first, then here again.
        let again = move |app: &mut App<'a>| {
            if let Err(error) = app.bring_into_project(target, place) {
                log::warn!("{error:#}");
            }
        };
        let restoring = match target {
            ReferencedRef::Layer(id) => self.restore_layers_for(vec![id], again),
            ReferencedRef::Item(item) => self.restore_items_for(vec![item], again),
        };
        if restoring {
            return Ok(());
        }
        let brought = match target {
            ReferencedRef::Layer(id) => self.brought_in_layer(id, place),
            ReferencedRef::Item(item) => self.brought_in_item(item, place),
        };
        let Some((reference, name, command)) = brought else {
            return Ok(());
        };
        self.execute_edit(command);
        let reference_name = self
            .data_references
            .iter()
            .find(|candidate| candidate.id == reference)
            .map(DataReference::name)
            .unwrap_or_default();
        userspace_log!("{}", tr!("cmd-brought-into-project", name = name, reference = reference_name));
        self.invalidate_geometry();
        Ok(())
    }

    /// A referenced layer's copy for the project, and the unloading of the
    /// reference's own.
    fn brought_in_layer(&mut self, id: LayerId, place: Option<(SectionKind, Option<FolderId>)>) -> Option<(ReferenceId, String, crate::model::Command)> {
        use crate::model::{Command, Layer};
        let project = self.workspace.active_project_mut()?;
        let document = &mut project.project.document;
        let source = document.layer(id).cloned()?;
        let reference = source.reference?;
        let objects = crate::app::commands::layer::objects_on_layer(document, id);
        let name = crate::model::project::unique_item_name(
            source.name.clone(),
            document.layers().iter().filter(|layer| layer.reference.is_none()).map(|layer| layer.name.as_str()),
        );
        let copy = document.allocate_layer_id();
        let layer = Layer {
            id: copy,
            name: name.clone(),
            color_index: source.color_index,
            color: source.color,
            loaded: true,
            hidden: false,
            elevation: source.elevation,
            folder: place.and_then(|(_, folder)| folder),
            section: place.map_or(source.section, |(section, _)| section),
            reference: None,
        };
        let objects = objects
            .into_iter()
            .map(|object| {
                let object_id = document.allocate_object_id();
                object.with_id_and_layer(object_id, copy)
            })
            .collect();
        let mut commands = vec![Command::AddLayerSnapshot { layer, objects }];
        if source.loaded {
            commands.push(Command::SetLayerLoaded { id, before: true, after: false });
        }
        Some((reference, name, Command::Batch(commands)))
    }

    /// A referenced item's copy for the project, under an id and a name of
    /// its own, and the unloading of the reference's own.
    fn brought_in_item(&mut self, item: ItemRef, place: Option<(SectionKind, Option<FolderId>)>) -> Option<(ReferenceId, String, crate::model::Command)> {
        use crate::model::{
            Command, OpenItem, block_model::BlockModelId, drill_hole::DrillHoleId, point_cloud::PointCloudId, raster::RasterTextureId, triangulation::TriangulationId,
        };
        let mut copy = self.clone_project_item(item)?;
        let reference = copy.state().reference?;
        fn fresh(next: &mut u64) -> u64 {
            let id = *next;
            *next += 1;
            id
        }
        let working = |name: &str, state: &crate::model::project::ProjectItemState| state.is_working().then_some(name.to_owned());
        let (copy_ref, index, names): (ItemRef, usize, Vec<String>) = match item {
            ItemRef::Triangulation(_) => (
                ItemRef::Triangulation(TriangulationId(fresh(&mut self.next_triangulation_id))),
                self.triangulations.len(),
                self.triangulations.iter().filter_map(|item| working(&item.name, &item.state)).collect(),
            ),
            ItemRef::BlockModel(_) => (
                ItemRef::BlockModel(BlockModelId(fresh(&mut self.next_block_model_id))),
                self.block_models.len(),
                self.block_models.iter().filter_map(|item| working(&item.name, &item.state)).collect(),
            ),
            ItemRef::DrillHole(_) => (
                ItemRef::DrillHole(DrillHoleId(fresh(&mut self.next_drill_hole_id))),
                self.drill_holes.len(),
                self.drill_holes.iter().filter_map(|item| working(&item.name, &item.state)).collect(),
            ),
            ItemRef::PointCloud(_) => (
                ItemRef::PointCloud(PointCloudId(fresh(&mut self.next_point_cloud_id))),
                self.point_clouds.len(),
                self.point_clouds.iter().filter_map(|item| working(&item.name, &item.state)).collect(),
            ),
            ItemRef::Raster(_) => (
                ItemRef::Raster(RasterTextureId(fresh(&mut self.next_raster_texture_id))),
                self.raster_textures.len(),
                self.raster_textures.iter().filter_map(|item| working(&item.name, &item.state)).collect(),
            ),
        };
        fn name_of(copy: &mut OpenItem) -> &mut String {
            match copy {
                OpenItem::Triangulation(item) => &mut item.name,
                OpenItem::BlockModel(item) => &mut item.name,
                OpenItem::DrillHole(item) => &mut item.name,
                OpenItem::PointCloud(item) => &mut item.name,
                OpenItem::Raster(item) => &mut item.name,
            }
        }
        let name = crate::model::project::unique_item_name(name_of(&mut copy).clone(), names.iter().map(String::as_str));
        *name_of(&mut copy) = name.clone();
        crate::app::explorer_preview::set_item_id(&mut copy, copy_ref);
        let state = copy.state_mut();
        state.reference = None;
        state.folder = place.and_then(|(_, folder)| folder);
        if let Some((section, _)) = place {
            state.section = section;
        }
        state.loaded = true;
        state.hidden = false;
        state.touch();
        let mut commands = vec![Command::AddItem {
            item: copy_ref,
            index,
            added: Some(copy),
        }];
        commands.extend(self.item_style_command(item, |style| style.with_loaded(false)));
        Some((reference, name, Command::Batch(commands)))
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
    /// Every place the project links to an item, and the item each links to
    /// now: the raster on each of its own surfaces, and each solid's
    /// surfaces and block model.
    fn link_sites(&self) -> Vec<(crate::model::project::LinkSite, Option<ItemRef>)> {
        use crate::model::project::LinkSite;
        let mut sites: Vec<(LinkSite, Option<ItemRef>)> = self
            .triangulations
            .iter()
            .filter(|surface| surface.state.is_working())
            .map(|surface| (LinkSite::Drape(surface.id), surface.raster_texture.map(ItemRef::Raster)))
            .collect();
        if let Some(document) = self.workspace.active_document() {
            for solid in document.solids() {
                sites.push((LinkSite::SolidSurface(solid.id), solid.surface.map(ItemRef::Triangulation)));
                sites.push((LinkSite::SolidTopography(solid.id), solid.topography.map(ItemRef::Triangulation)));
                sites.push((LinkSite::SolidBlockModel(solid.id), solid.block_model.map(ItemRef::BlockModel)));
            }
        }
        sites
    }

    /// The file and name an item a reference shows is saved by, or `None`
    /// for one of the project's own.
    fn referenced_by_name(&self, item: ItemRef) -> Option<(String, String)> {
        let reference = self.project_item_state(item)?.reference?;
        let name = self.item_is_referenced(item)?;
        let file = self.data_references.iter().find(|candidate| candidate.id == reference)?.file_name();
        Some((file, name))
    }

    /// Bring the project's links to referenced items up to date with what
    /// each site links to now: before a reference's items go (their ids are
    /// new when it is read again) and as the project is saved. A link to an
    /// item that has gone is kept, waiting for its reference; one the user
    /// pointed elsewhere, or cleared, goes.
    pub(crate) fn capture_reference_links(&mut self) {
        let sites = self.link_sites();
        let Some(mut links) = self.workspace.active_project_mut().map(|project| std::mem::take(&mut project.project.reference_links)) else {
            return;
        };
        links.retain(|link| sites.iter().any(|(site, _)| *site == link.site));
        for (site, target) in sites {
            let held = links.iter().position(|link| link.site == site);
            match (target.and_then(|item| self.referenced_by_name(item)), target, held) {
                (Some((file, name)), _, held) => {
                    let link = crate::model::project::ReferenceLink { site, file, name, resolved: true };
                    match held {
                        Some(index) => links[index] = link,
                        None => links.push(link),
                    }
                }
                // Linked to one of the project's own, or cleared since it
                // was matched: it is no reference's any longer.
                (None, Some(item), Some(index)) if self.project_item_state(item).is_some() => {
                    links.remove(index);
                }
                (None, None, Some(index)) if links[index].resolved => {
                    links.remove(index);
                }
                _ => {}
            }
        }
        if let Some(project) = self.workspace.active_project_mut() {
            project.project.reference_links = links;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Match the project's links naming reference `id`'s file to its items,
    /// by name, now that it is read. A site linked since to something else
    /// is left as it is.
    fn resolve_reference_links(&mut self, id: ReferenceId) {
        use crate::model::{MemberKind, project::LinkSite};
        let Some(file) = self.data_references.iter().find(|reference| reference.id == id).map(DataReference::file_name) else {
            return;
        };
        let sites: std::collections::HashMap<LinkSite, Option<ItemRef>> = self.link_sites().into_iter().collect();
        let mine = |state: &crate::model::project::ProjectItemState| state.reference == Some(id);
        let found = |kind: MemberKind, name: &str| -> Option<ItemRef> {
            match kind {
                MemberKind::Triangulation => self
                    .triangulations
                    .iter()
                    .find(|item| mine(&item.state) && item.name == name)
                    .map(|item| ItemRef::Triangulation(item.id)),
                MemberKind::Raster => self
                    .raster_textures
                    .iter()
                    .find(|item| mine(&item.state) && item.name == name)
                    .map(|item| ItemRef::Raster(item.id)),
                MemberKind::BlockModel => self
                    .block_models
                    .iter()
                    .find(|item| mine(&item.state) && item.name == name)
                    .map(|item| ItemRef::BlockModel(item.id)),
                MemberKind::Layer | MemberKind::PointCloud | MemberKind::DrillHole => None,
            }
        };
        let mut matched: Vec<(LinkSite, ItemRef)> = Vec::new();
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        for link in project.project.reference_links.iter().filter(|link| link.file == file) {
            // Waiting: linked to nothing, or to an item since gone.
            let waiting = sites
                .get(&link.site)
                .is_some_and(|target| target.is_none_or(|item| self.project_item_state(item).is_none()));
            if waiting && let Some(item) = found(link.site.kind(), &link.name) {
                matched.push((link.site, item));
            }
        }
        if let Some(project) = self.workspace.active_project_mut() {
            for link in project.project.reference_links.iter_mut().filter(|link| matched.iter().any(|(site, _)| *site == link.site)) {
                link.resolved = true;
            }
        }
        let mut solids = self.workspace.active_document().map(|document| document.solids().to_vec()).unwrap_or_default();
        let mut solids_changed = false;
        for (site, item) in matched {
            match (site, item) {
                (LinkSite::Drape(surface), ItemRef::Raster(raster)) => {
                    if let Some(surface) = self.triangulations.iter_mut().find(|candidate| candidate.id == surface) {
                        surface.raster_texture = Some(raster);
                    }
                }
                (LinkSite::SolidSurface(solid), ItemRef::Triangulation(surface)) => {
                    solids.iter_mut().filter(|candidate| candidate.id == solid).for_each(|solid| solid.surface = Some(surface));
                    solids_changed = true;
                }
                (LinkSite::SolidTopography(solid), ItemRef::Triangulation(surface)) => {
                    solids
                        .iter_mut()
                        .filter(|candidate| candidate.id == solid)
                        .for_each(|solid| solid.topography = Some(surface));
                    solids_changed = true;
                }
                (LinkSite::SolidBlockModel(solid), ItemRef::BlockModel(model)) => {
                    solids
                        .iter_mut()
                        .filter(|candidate| candidate.id == solid)
                        .for_each(|solid| solid.block_model = Some(model));
                    solids_changed = true;
                }
                _ => {}
            }
        }
        if solids_changed && let Some(document) = self.workspace.active_document_mut() {
            document.restore_solids(solids);
        }
    }

    /// Take the links to referenced items out of a save, which carries them
    /// by name instead: their ids are this session's alone, and read back
    /// could name one of the project's own items.
    pub(crate) fn detach_reference_links(&self, snapshot: &mut crate::model::formats::omf::ProjectSnapshot) {
        let referenced = |item: ItemRef| self.item_is_referenced(item).is_some();
        for surface in &mut snapshot.triangulations {
            if surface.raster_texture.is_some_and(|raster| referenced(ItemRef::Raster(raster))) {
                surface.raster_texture = None;
            }
        }
        if let Some(design) = snapshot.designs.as_mut() {
            let mut solids = design.document.solids().to_vec();
            let mut changed = false;
            for solid in &mut solids {
                for surface in [&mut solid.surface, &mut solid.topography] {
                    if surface.is_some_and(|id| referenced(ItemRef::Triangulation(id))) {
                        *surface = None;
                        changed = true;
                    }
                }
                if solid.block_model.is_some_and(|id| referenced(ItemRef::BlockModel(id))) {
                    solid.block_model = None;
                    changed = true;
                }
            }
            if changed {
                design.document.restore_solids(solids);
            }
        }
    }

    /// The name of what a data reference shows that `command` would change,
    /// if it would change any: objects added to, taken from or moved off a
    /// referenced layer, the layer itself renamed, moved or deleted, or a
    /// referenced item moved, renamed, replaced or deleted. Every edit is
    /// asked this before it runs, however it was reached - a tool, a
    /// selection, the assistant - so none of them can write into a reference.
    /// Showing, hiding, loading and styling are not changes to it: none of
    /// that is saved.
    pub(crate) fn referenced_changed_by(&self, command: &crate::model::Command) -> Option<String> {
        use crate::model::Command;
        let layer = |id: LayerId| self.layer_is_referenced(id);
        let item = |item: ItemRef| self.item_is_referenced(item);
        match command {
            Command::AddObject(object) | Command::DeleteObject { object, .. } => layer(object.layer()),
            Command::Replace { before, after } => layer(before.layer()).or_else(|| layer(after.layer())),
            Command::RenameLayer { id, .. } | Command::SetLayerPlacement { id, .. } | Command::SetLayerElevation { id, .. } => layer(*id),
            Command::DeleteLayerSnapshot { layer: deleted, .. } => layer(deleted.id),
            Command::SetItemPlacement { item: changed, .. }
            | Command::RenameItem { item: changed, .. }
            | Command::ReplaceItem { item: changed, .. }
            | Command::DeleteItem { item: changed, .. } => item(*changed),
            Command::MoveCollars { dataset, .. }
            | Command::RotateCollars { dataset, .. }
            | Command::SetTieIns { dataset, .. }
            | Command::SetInitiation { dataset, .. }
            | Command::SetCharges { dataset, .. } => item(ItemRef::DrillHole(*dataset)),
            Command::Batch(commands) => commands.iter().find_map(|command| self.referenced_changed_by(command)),
            _ => None,
        }
    }

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

    /// An OMF 2 file holding one surface, "Topography", named so it reads
    /// back the same in any language.
    fn survey_file(dir: &Path) -> PathBuf {
        use std::sync::Arc;
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        let mesh = crate::model::formats::mesh_data::Triangulation::from_vertices_and_faces(
            [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]
                .map(|(x, y)| crate::model::formats::mesh_data::Vertex::new(x, y, 50.0))
                .to_vec(),
            vec![[0, 1, 2], [0, 2, 3]],
        )
        .unwrap();
        let built = crate::model::triangulation::GeneratedTriangulation {
            name: "Topography".to_owned(),
            spatial: Arc::new(crate::model::spatial::TriangleBvh::build(&mesh)),
            surface_face_order: Arc::new((0..mesh.face_count() as u32).collect()),
            mesh: Arc::new(mesh),
            edges: Vec::new(),
            surface_type: crate::ui::state::TriSurfaceType::Surface,
        };
        let (_, commands) = app.add_triangulation_commands(vec![crate::app::commands::triangulation::session::new_triangulation(
            built,
            crate::model::SectionKind::Triangulations,
        )]);
        app.execute_edit(crate::model::Command::Batch(commands));
        let path = dir.join("survey.omf");
        omf::write_path(app.omf_save_snapshot().unwrap(), &path, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
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
    fn hovering_an_unloaded_layer_reads_its_strings_in_for_the_card_and_loading_it_takes_them() {
        use crate::ui::state::{PreviewStatus, PreviewSubject};
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        let lines = install(&mut app, &powerlines_file(dir.path()));
        let layer = app
            .workspace
            .active_document()
            .unwrap()
            .layers()
            .iter()
            .find(|layer| layer.reference == Some(lines))
            .unwrap()
            .id;
        assert!(app.workspace.active_document().unwrap().deferred_layers.contains_key(&layer));

        app.editor.explorer_preview.hovered = Some(PreviewSubject::Layer(layer));
        app.refresh_explorer_preview();
        let started = std::time::Instant::now();
        while app.editor.explorer_preview.status == PreviewStatus::Loading {
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "the card's read never landed");
            app.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(app.editor.explorer_preview.status, PreviewStatus::Ready);
        assert_eq!(app.editor.explorer_preview.lines.len(), 1, "the layer's one point");
        // Read in for the card only: the layer is as unloaded as it was.
        assert!(app.workspace.active_document().unwrap().deferred_layers.contains_key(&layer));

        // The card closes; loading the layer takes what it read.
        app.refresh_explorer_preview();
        assert!(app.restore_layers_for(vec![layer], |_| {}));
        assert!(app.explorer_preview_layer.is_none(), "the load took the card's copy");
        assert!(!app.workspace.active_document().unwrap().deferred_layers.contains_key(&layer));
        assert_eq!(
            app.workspace.active_document().unwrap().objects().iter().filter(|object| object.layer() == layer).count(),
            1
        );
    }

    #[test]
    fn nothing_a_reference_shows_can_be_selected_for_editing_or_changed() {
        use crate::model::Command;
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        let lines = install(&mut app, &powerlines_file(dir.path()));
        let layer = app
            .workspace
            .active_document()
            .unwrap()
            .layers()
            .iter()
            .find(|layer| layer.reference == Some(lines))
            .unwrap()
            .id;
        // Loaded, so it has objects to select and change.
        app.handle_ui_command(crate::ui::state::UiCommand::LoadLayer(layer)).unwrap();
        let started = std::time::Instant::now();
        while app.workspace.active_document().unwrap().deferred_layers.contains_key(&layer) {
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "the layer never loaded");
            app.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let objects = |app: &App| {
            app.workspace
                .active_document()
                .unwrap()
                .objects()
                .iter()
                .filter(|object| object.layer() == layer)
                .cloned()
                .collect::<Vec<_>>()
        };
        let point = objects(&app).pop().expect("the reference's point");

        // Its row and Select All Objects select none of it.
        app.handle_ui_command(crate::ui::state::UiCommand::SelectExplorerRow(crate::ui::state::ExplorerRow::Layer(layer)))
            .unwrap();
        assert!(app.editor.selected_handles.is_empty());
        app.select_all_objects_in_layer(layer);
        assert!(app.editor.selected_handles.is_empty());

        // An edit that would change it is refused, however it is reached.
        app.execute_edit(Command::delete_object(point.clone()));
        let moved = point.with_id_and_layer(point.id(), layer);
        app.execute_edit(Command::Replace {
            before: point.clone(),
            after: moved,
        });
        app.execute_edit(Command::RenameLayer {
            id: layer,
            before: "Powerlines".to_owned(),
            after: "Mine".to_owned(),
        });
        assert_eq!(objects(&app).len(), 1, "the reference's point is still there");
        assert!(app.workspace.active_document().unwrap().layer(layer).is_some_and(|layer| layer.name == "Powerlines"));
        let id = runtime_id(&app);
        assert!(!app.project_content_is_dirty(id));

        // The project's own layers are edited as ever.
        let own = app.create_named_layer("Mine".to_owned()).unwrap();
        app.execute_edit(Command::AddObject(point.with_id_and_layer(crate::model::ObjectId(999_999), own)));
        assert_eq!(app.workspace.active_document().unwrap().objects().iter().filter(|object| object.layer() == own).count(), 1);
    }

    /// A solid built on a referenced surface keeps it across a save, a
    /// reopen and a reload, though the surface's id is new each time: the
    /// link is saved by the reference's file and the surface's name.
    #[test]
    fn links_to_referenced_items_survive_saving_reopening_and_reloading() {
        crate::model::asset_storage::initialize_native();
        let dir = tempfile::tempdir().unwrap();
        let shared = survey_file(dir.path());
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        install(&mut app, &shared);
        let (surface, name) = app
            .triangulations
            .iter()
            .find(|item| !item.state.is_working())
            .map(|item| (item.id, item.name.clone()))
            .unwrap();
        let solid = app
            .workspace
            .active_document_mut()
            .unwrap()
            .add_solid("Pit".to_owned(), crate::model::SolidKind::Pit, None, Some(surface), None);

        // Saved by name, and never by this session's id.
        let snapshot = app.omf_save_snapshot().unwrap();
        let saved = snapshot.designs.as_ref().unwrap().document.solid(solid).unwrap();
        assert_eq!(saved.topography, None);
        assert_eq!(snapshot.reference_links.len(), 1);
        assert_eq!(
            (snapshot.reference_links[0].file.as_str(), snapshot.reference_links[0].name.as_str()),
            ("survey.omf", name.as_str())
        );
        let path = dir.path().join("mine.omf");
        omf::write_path(snapshot, &path, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();

        // Reopened elsewhere, it finds the surface again once the reference is read.
        let mut other = App::default();
        other.start_untitled_project().unwrap();
        let bundle = omf::from_path("mine.omf", &path, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        other.apply_opened_omf_bundle(Some(path.clone()), "mine.omf".to_owned(), bundle, crate::app::commands::omf::ViewOnOpen::Fit);
        let topography = |app: &App| app.workspace.active_document().unwrap().solid(solid).and_then(|solid| solid.topography);
        assert_eq!(topography(&other), None, "waiting for the reference");
        install(&mut other, &shared);
        let named = |app: &App, id| app.triangulations.iter().find(|item| item.id == id).map(|item| item.name.clone());
        let found = topography(&other).expect("matched by name");
        assert_eq!(named(&other, found), Some(name.clone()));
        let reference = other.data_references[0].id;
        assert!(!other.project_content_is_dirty(runtime_id(&other)), "matching it is no unsaved work");

        // Read again, its items have new ids, and the link follows.
        let bundle = omf::reference_from_path("reference", &shared, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        other.clear_referenced_content(reference);
        other.install_data_reference(reference, bundle);
        let again = topography(&other).expect("matched again");
        assert_ne!(again, found, "a reread reference's items are new");
        assert_eq!(named(&other, again), Some(name));

        // Pointed at nothing by the user, the link goes.
        let mut solids = other.workspace.active_document().unwrap().solids().to_vec();
        solids[0].topography = None;
        other.workspace.active_document_mut().unwrap().restore_solids(solids);
        other.capture_reference_links();
        assert!(other.workspace.active_project().unwrap().project.reference_links.is_empty());
    }

    #[test]
    fn a_reference_to_the_open_project_shows_nothing_twice() {
        let dir = tempfile::tempdir().unwrap();
        let file = omf1_fixture(dir.path());
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        app.workspace.active_project_mut().unwrap().path = Some(file.clone());
        // Spelt another way, it is still the same file.
        let id = app.push_data_reference(dir.path().join(".").join(file.file_name().unwrap()));
        app.reload_data_reference(id);
        let reference = app.data_references.iter().find(|reference| reference.id == id).unwrap();
        assert_eq!(reference.status, ReferenceStatus::OpenAsProject);
        assert!(app.triangulations.is_empty());
    }

    #[test]
    fn loading_a_referenced_surface_is_no_unsaved_work() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        install(&mut app, &omf1_fixture(dir.path()));
        let runtime = runtime_id(&app);
        assert!(!app.project_content_is_dirty(runtime));
        let id = app.triangulations.iter().find(|surface| surface.state.deferred.is_some()).unwrap().id;
        app.handle_ui_command(crate::ui::state::UiCommand::LoadTriangulation(id)).unwrap();
        let started = std::time::Instant::now();
        while !app.triangulations.iter().find(|surface| surface.id == id).unwrap().state.loaded {
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "the load never finished");
            app.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!app.project_content_is_dirty(runtime));
    }

    #[test]
    fn hovering_an_unloaded_item_reads_it_in_for_the_card_and_loading_it_takes_that_copy() {
        use crate::ui::state::{PreviewStatus, PreviewSubject};
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        install(&mut app, &omf1_fixture(dir.path()));
        let surface = app
            .triangulations
            .iter()
            .find(|surface| surface.state.deferred.is_some())
            .expect("an unloaded referenced surface");
        let (id, name) = (surface.id, surface.name.clone());
        let item = ItemRef::Triangulation(id);

        app.editor.explorer_preview.hovered = Some(PreviewSubject::Item(item));
        app.refresh_explorer_preview();
        let started = std::time::Instant::now();
        while app.editor.explorer_preview.status == PreviewStatus::Loading {
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "the card's read never landed");
            app.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(app.editor.explorer_preview.status, PreviewStatus::Ready);
        assert!(app.editor.explorer_preview.bounds.is_some());
        // Read in for the card only: the project's surface is as it was.
        let surface = app.triangulations.iter().find(|surface| surface.id == id).unwrap();
        assert!(!surface.state.loaded && surface.state.deferred.is_some(), "{name}");
        assert_eq!(app.triangulations.iter().filter(|surface| surface.name == name).count(), 1);

        // A click closes the card; the copy is kept, and coming back to the
        // row shows it again at once.
        app.refresh_explorer_preview();
        assert_eq!(app.editor.explorer_preview.subject, None);
        assert!(app.explorer_preview_item.is_some());
        app.editor.explorer_preview.hovered = Some(PreviewSubject::Item(item));
        app.refresh_explorer_preview();
        assert_eq!(app.editor.explorer_preview.status, PreviewStatus::Ready);
        app.refresh_explorer_preview();

        // Loading it takes the card's copy: nothing left to read, and the
        // load finishes as one that read it would.
        app.handle_ui_command(crate::ui::state::UiCommand::LoadTriangulation(id)).unwrap();
        assert!(app.explorer_preview_item.is_none(), "the load took the card's copy");
        let surface = app.triangulations.iter().find(|surface| surface.id == id).unwrap();
        assert!(surface.state.deferred.is_none() && surface.mesh.face_count() > 0);
        let started = std::time::Instant::now();
        while !app.triangulations.iter().find(|surface| surface.id == id).unwrap().state.loaded {
            assert!(started.elapsed() < std::time::Duration::from_secs(5), "the load never finished");
            app.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!app.hand_over_explorer_preview(item));
        let runtime = runtime_id(&app);
        assert!(
            !app.project_content_is_dirty(runtime),
            "loading a referenced surface from the card's copy is no unsaved work"
        );

        // The same, read from the file as ever.
        let other = app.triangulations.iter().find(|surface| surface.state.deferred.is_some()).unwrap().id;
        app.handle_ui_command(crate::ui::state::UiCommand::LoadTriangulation(other)).unwrap();
        let started = std::time::Instant::now();
        while !app.triangulations.iter().find(|surface| surface.id == other).unwrap().state.loaded {
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "the load never finished");
            app.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!app.project_content_is_dirty(runtime), "loading a referenced surface is no unsaved work");
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
    fn referenced_content_is_locked_and_never_saved_until_copied_in() {
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

        // Brought in, the layer is copied into the project, as its own to
        // edit. The project has a layer of that name already, its own and
        // nothing to do with the reference's: the copy is "(2)".
        let wait = |app: &mut App, done: &dyn Fn(&App) -> bool| {
            let started = std::time::Instant::now();
            while !done(app) {
                assert!(started.elapsed() < std::time::Duration::from_secs(30), "never finished");
                app.poll_jobs();
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        wait(&mut app, &|app| {
            !app.workspace.active_project().unwrap().project.document.deferred_layers.contains_key(&layer)
        });
        let own = app
            .workspace
            .active_project_mut()
            .unwrap()
            .project
            .document
            .add_layer("Powerlines".to_owned(), None, [1.0; 4], true, 0.0);
        app.bring_into_project(ReferencedRef::Layer(layer), None).unwrap();
        let layers = |app: &App| {
            app.workspace
                .active_project()
                .unwrap()
                .project
                .document
                .layers()
                .iter()
                .map(|layer| (layer.id, layer.name.clone(), layer.reference, layer.loaded))
                .collect::<Vec<_>>()
        };
        let all = layers(&app);
        let copy = all
            .iter()
            .find(|(_, name, ..)| name == "Powerlines (2)")
            .expect("the copy, named apart from the project's own");
        assert_eq!(copy.2, None);
        assert!(copy.3);
        assert!(!app.editor.locked_layers.contains(&copy.0));
        assert!(all.iter().any(|(id, name, reference, _)| *id == own && name == "Powerlines" && reference.is_none()));
        let theirs = all.iter().find(|(id, ..)| *id == layer).expect("the reference's own stays");
        assert_eq!(
            (theirs.1.as_str(), theirs.2, theirs.3),
            ("Powerlines", Some(lines), false),
            "the reference's own is unloaded"
        );
        assert!(app.editor.locked_layers.contains(&layer));
        assert!(app.project_content_is_dirty(id));
        let snapshot = app.omf_save_snapshot().unwrap();
        let mut saved_layers = snapshot
            .designs
            .as_ref()
            .unwrap()
            .document
            .layers()
            .iter()
            .map(|layer| layer.name.clone())
            .collect::<Vec<_>>();
        saved_layers.sort();
        assert_eq!(saved_layers, ["Powerlines", "Powerlines (2)"]);
        assert!(snapshot.triangulations.is_empty());

        // Read again, the reference shows all it holds, beside the copy.
        app.reload_data_reference(lines);
        let lines_bundle = omf::reference_from_path("reference", &infrastructure, &crate::model::progress::Progress::new().phase(0.0, 1.0)).unwrap();
        app.install_data_reference(lines, lines_bundle);
        let all = layers(&app);
        assert_eq!(all.iter().filter(|(_, name, reference, _)| name == "Powerlines" && *reference == Some(lines)).count(), 1);
        assert!(all.iter().any(|(_, name, reference, _)| name == "Powerlines (2)" && reference.is_none()));

        // The copy deleted, the reference's own is as it was.
        app.workspace.active_project_mut().unwrap().project.document.delete_layer(copy.0);
        assert_eq!(layers(&app).iter().filter(|(_, _, reference, _)| *reference == Some(lines)).count(), 1);

        // A surface brought in keeps its name, the project having none like
        // it; it is read in first, being unloaded.
        let (surface, surface_name) = app
            .triangulations
            .iter()
            .find(|item| item.state.reference == Some(survey))
            .map(|item| (item.id, item.name.clone()))
            .unwrap();
        // Dragged out onto a collection of the project's: the copy lands in it.
        let collection = app
            .workspace
            .active_project_mut()
            .unwrap()
            .project
            .folders
            .add(SectionKind::Triangulations, "Surveys".to_owned())
            .unwrap();
        app.handle_ui_command(crate::ui::state::UiCommand::BringIntoProjectAt {
            target: ReferencedRef::Item(ItemRef::Triangulation(surface)),
            section: SectionKind::Triangulations,
            folder: Some(collection),
        })
        .unwrap();
        wait(&mut app, &|app| app.triangulations.iter().any(|item| item.state.is_working()));
        let copy = app.triangulations.iter().find(|item| item.state.is_working()).unwrap();
        assert_eq!(copy.name, surface_name);
        assert!(copy.state.loaded && copy.id != surface);
        assert_eq!(copy.state.folder, Some(collection));
        let theirs = app.triangulations.iter().find(|item| item.id == surface).unwrap();
        assert!(theirs.state.reference == Some(survey) && !theirs.state.loaded);
        assert_eq!(app.omf_save_snapshot().unwrap().triangulations.len(), 1);

        // Removing a reference takes its content away and leaves the project as it was.
        let dirty_before = app.project_content_is_dirty(id);
        app.data_references.retain(|reference| reference.id != survey);
        app.clear_referenced_content(survey);
        assert!(app.triangulations.iter().all(|item| item.state.is_working()), "the project's copy stays");
        assert_eq!(app.project_content_is_dirty(id), dirty_before);
    }
}
