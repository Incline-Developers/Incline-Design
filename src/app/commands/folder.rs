//! Explorer folder commands, shared by every section.
//!
//! Design layers and the five project-item kinds each get their own folder
//! list under [`SectionKind`], but the create/delete/rename/move mechanics
//! are the same shape everywhere - this module is that shared
//! implementation, generalised from the Designs-only slice it replaced.

use anyhow::Result;

use crate::{
    app::App,
    i18n::tr,
    model::{Command, Folder, FolderId, FolderMember, ItemRef, LayerId, MemberTarget, Placement, SceneEntityId, SectionKind},
    ui::state::ExplorerSection,
    userspace_log, userspace_warn,
};

fn unique_collection_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    let mut number = 2_u64;
    loop {
        let name = format!("{base} ({number})");
        if !taken(&name) {
            return name;
        }
        number += 1;
    }
}

impl<'a> App<'a> {
    pub(crate) fn create_folder(&mut self, section: SectionKind) -> Result<()> {
        let Some(project) = self.workspace.active_project_mut() else {
            return Ok(());
        };
        let registry = &mut project.project.folders;
        let name = unique_collection_name(&tr!("common-collection"), |candidate| registry.has_name(section, candidate));
        let id = registry.allocate_id();
        let folder = Folder { id, name: name.clone() };
        self.execute_edit(Command::AddFolder { section, folder });
        userspace_log!("{}", tr!("cmd-folder-created-collection-name", name = name.to_string()));
        Ok(())
    }

    pub(crate) fn delete_folder(&mut self, section: SectionKind, folder: FolderId) -> Result<()> {
        let Some(project) = self.workspace.active_project() else {
            return Ok(());
        };
        let Some(index) = project.project.folders.index_of(section, folder) else {
            return Ok(());
        };
        let folder_data = project.project.folders.folders(section)[index].clone();
        // Deleting a folder returns its members to the section root, never
        // deletes them - ids are unique registry-wide across both halves.
        let layers = project.project.document.layers_in_folder(folder);
        let items = self.project_items_in_folder(folder);
        let name = folder_data.name.clone();
        self.execute_edit(Command::DeleteFolder {
            section,
            folder: folder_data,
            index,
            layers,
            items,
        });
        userspace_log!("{}", tr!("cmd-folder-deleted-collection-name", name = name.to_string()));
        Ok(())
    }

    /// Delete a folder together with everything in it - its layers with their
    /// objects, and its project items - as one undo step.
    pub(crate) fn delete_folder_and_contents(&mut self, section: SectionKind, folder: FolderId) -> Result<()> {
        let Some(project) = self.workspace.active_project() else {
            return Ok(());
        };
        let layers = project.project.document.layers_in_folder(folder);
        if self.restore_layers_for(layers.clone(), move |app| {
            if let Err(error) = app.delete_folder_and_contents(section, folder) {
                crate::userspace_error!("{error:#}");
            }
        }) {
            return Ok(());
        }
        let Some(project) = self.workspace.active_project() else {
            return Ok(());
        };
        let Some(index) = project.project.folders.index_of(section, folder) else {
            return Ok(());
        };
        let folder_data = project.project.folders.folders(section)[index].clone();
        let document = &project.project.document;
        let items = self.project_items_in_folder(folder);

        let mut commands = Vec::new();
        // A raster leaving takes its drapes with it, the way a lone delete
        // does, so undo puts the raster back before the drapes that need it.
        for item in &items {
            if let ItemRef::Raster(raster) = *item {
                let draped: Vec<_> = self.triangulations.iter().filter(|tri| tri.raster_texture == Some(raster)).map(|tri| tri.id).collect();
                commands.extend(
                    draped
                        .into_iter()
                        .filter_map(|tri| self.item_style_command(ItemRef::Triangulation(tri), |style| style.with_raster_texture(None))),
                );
            }
        }
        commands.extend(items.iter().map(|item| Command::DeleteItem {
            item: *item,
            index: 0,
            removed: None,
        }));

        // Layers go last-first in the stack, and each records its objects'
        // positions as they stand once the layers before it are gone, so undo
        // walking the batch backwards puts every object back where it was.
        let mut ordered: Vec<(usize, LayerId)> = layers
            .iter()
            .filter_map(|id| document.layers().iter().position(|layer| layer.id == *id).map(|position| (position, *id)))
            .collect();
        ordered.sort_unstable_by_key(|&(position, _)| std::cmp::Reverse(position));
        let mut removed = std::collections::HashSet::new();
        for (layer_index, layer_id) in ordered {
            let Some(layer) = document.layer(layer_id).cloned() else {
                continue;
            };
            let objects = document
                .objects()
                .iter()
                .filter(|object| !removed.contains(&object.layer()))
                .enumerate()
                .filter(|(_, object)| object.layer() == layer_id)
                .map(|(position, object)| (position, object.clone()))
                .collect();
            removed.insert(layer_id);
            commands.push(Command::DeleteLayerSnapshot { layer, layer_index, objects });
        }
        let name = folder_data.name.clone();
        commands.push(Command::DeleteFolder {
            section,
            folder: folder_data,
            index,
            layers: Vec::new(),
            items: Vec::new(),
        });

        for item in &items {
            let entity = match *item {
                ItemRef::Triangulation(id) => SceneEntityId::Triangulation(id),
                ItemRef::PointCloud(id) => SceneEntityId::PointCloud(id),
                ItemRef::BlockModel(id) => SceneEntityId::BlockModel(id),
                ItemRef::DrillHole(id) => SceneEntityId::DrillHole(id),
                ItemRef::Raster(id) => SceneEntityId::Raster(id),
            };
            self.editor.selected_handles.remove(&entity);
            self.editor.hidden_handles.remove(&entity);
            self.editor.explicitly_frozen.remove(&entity);
            self.editor.frozen_handles.remove(&entity);
            self.editor.translucent_handles.remove(&entity);
            if let ItemRef::Triangulation(id) = *item
                && self.active_triangulation == Some(id)
            {
                self.active_triangulation = None;
            }
        }
        if self.editor.active_layer.is_some_and(|active| layers.contains(&active)) {
            self.editor.active_layer = None;
        }
        for layer in &layers {
            self.editor.locked_layers.remove(layer);
        }
        self.cancel_jobs(|key| {
            use crate::app::jobs::JobKey;
            items.iter().any(|item| match (*item, key) {
                (ItemRef::Triangulation(id), JobKey::Triangulation(job)) => id == *job,
                (ItemRef::PointCloud(id), JobKey::PointCloud(job)) => id == *job,
                (ItemRef::BlockModel(id), JobKey::BlockModel(job)) => id == *job,
                (ItemRef::DrillHole(id), JobKey::DrillHole(job)) => id == *job,
                _ => false,
            })
        });

        self.execute_edit(Command::Batch(commands));
        userspace_log!("{}", tr!("cmd-folder-deleted-collection-contents", name = name.to_string()));
        self.invalidate_geometry();
        self.request_topology_redraw();
        Ok(())
    }

    /// Project items currently sitting in `folder`, whichever section shows
    /// them. Layers are the caller's other half, read straight off the
    /// document.
    fn project_items_in_folder(&self, folder: FolderId) -> Vec<ItemRef> {
        let in_folder = |state: &crate::model::project::ProjectItemState| state.folder == Some(folder);
        let triangulations = self.triangulations.iter().filter(|item| in_folder(&item.state)).map(|item| ItemRef::Triangulation(item.id));
        let rasters = self.raster_textures.iter().filter(|item| in_folder(&item.state)).map(|item| ItemRef::Raster(item.id));
        let point_clouds = self.point_clouds.iter().filter(|item| in_folder(&item.state)).map(|item| ItemRef::PointCloud(item.id));
        let block_models = self.block_models.iter().filter(|item| in_folder(&item.state)).map(|item| ItemRef::BlockModel(item.id));
        let drill_holes = self.drill_holes.iter().filter(|item| in_folder(&item.state)).map(|item| ItemRef::DrillHole(item.id));
        triangulations.chain(rasters).chain(point_clouds).chain(block_models).chain(drill_holes).collect()
    }

    /// Rename an explorer folder through the undo history. Blank names and a
    /// collision with another folder in the same section are refused before
    /// the edit is recorded.
    pub(crate) fn rename_folder(&mut self, section: SectionKind, folder: FolderId, name: String) {
        let requested = name.trim().to_owned();
        if requested.is_empty() {
            return;
        }
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        let registry = &project.project.folders;
        let Some(before) = registry.name(section, folder).map(ToOwned::to_owned) else {
            return;
        };
        if before == requested {
            return;
        }
        if registry.has_name(section, &requested) {
            userspace_warn!("{}", tr!("cmd-folder-collection-named-name-already-exists", name = requested.to_string()));
            return;
        }
        self.execute_edit(Command::RenameFolder {
            section,
            id: folder,
            before: before.clone(),
            after: requested.clone(),
        });
        userspace_log!(
            "{}",
            tr!("cmd-folder-renamed-collection-before-after", before = before.to_string(), after = requested.to_string())
        );
    }

    /// Move a layer or a project item into `folder` under `section`, or to
    /// that section's root with `None`.
    ///
    /// Where the member sits now is read fresh from the layer or item itself,
    /// not from `member`'s carried tag, which a stale view may have outlived.
    /// Where it may go is decided by `section` and the member's kind: a kind
    /// never changes, and any section admitting it may hold it, so a target in
    /// another section is a move between sections rather than a refusal.
    pub(crate) fn move_to_folder(&mut self, member: FolderMember, section: SectionKind, folder: Option<FolderId>) {
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        let before = match member.target() {
            MemberTarget::Layer(layer_id) => match project.project.document.layer(layer_id) {
                Some(layer) => Placement::new(layer.section, layer.folder),
                None => return,
            },
            MemberTarget::Item(item) => match self.project_item_state(item) {
                Some(state) => Placement::new(state.section, state.folder),
                None => return,
            },
        };
        // A section with no row for this kind would show the member nowhere.
        if !section.admits(member.kind()) {
            userspace_warn!("{}", tr!("cmd-folder-section-cannot-hold-item"));
            return;
        }
        if let Some(id) = folder
            && !project.project.folders.contains(section, id)
        {
            userspace_warn!("{}", tr!("cmd-folder-collection-no-longer-exists"));
            return;
        }
        let after = Placement::new(section, folder);
        if before == after {
            return;
        }
        let folder_name = folder.and_then(|id| project.project.folders.name(section, id)).map(ToOwned::to_owned);
        let section_name = ExplorerSection::from_kind(section).label();
        match member.target() {
            MemberTarget::Layer(id) => self.execute_edit(Command::SetLayerPlacement { id, before, after }),
            MemberTarget::Item(item) => self.execute_edit(Command::SetItemPlacement { item, before, after }),
        }
        userspace_log!(
            "{}",
            match &folder_name {
                Some(name) => tr!("cmd-folder-moved-item-into-collection-name", name = name.to_string()),
                None => tr!("cmd-folder-moved-item-root-section", section = section_name.to_string()),
            }
        );
    }
}

// `App` is not constructible in a unit test (it borrows a live wgpu/winit
// context), so its guards (the admission check in `move_to_folder`, the
// missing-folder-name refusal in `rename_folder`) are verified by reading.
