//! Explorer folder commands, shared by all six sections.
//!
//! Design layers and the five project-item kinds each get their own folder
//! list under [`SectionKind`], but the create/delete/rename/move mechanics
//! are the same shape everywhere - this module is that shared
//! implementation, generalised from the Designs-only slice it replaced.

use anyhow::Result;

use crate::{
    app::App,
    i18n::{tr, tr_format},
    model::{Command, Folder, FolderId, FolderMember, ItemRef, MemberTarget, SectionKind},
    userspace_log, userspace_warn,
};

impl<'a> App<'a> {
    pub(crate) fn create_folder(&mut self, section: SectionKind) -> Result<()> {
        let Some(project) = self.workspace.active_project_mut() else {
            return Ok(());
        };
        let registry = &mut project.project.folders;
        let name = super::layer::unique_name(&tr!(literal = "New Folder"), |candidate| registry.has_name(section, candidate));
        let id = registry.allocate_id();
        let folder = Folder { id, name: name.clone() };
        self.execute_edit(Command::AddFolder { section, folder });
        userspace_log!("{}", tr_format!(literal = "Created folder '%name%'", name = name));
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
        userspace_log!("{}", tr_format!(literal = "Deleted folder '%name%'", name = name));
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
            userspace_warn!("{}", tr_format!(literal = "A folder named '%name%' already exists", name = requested));
            return;
        }
        self.execute_edit(Command::RenameFolder {
            section,
            id: folder,
            before: before.clone(),
            after: requested.clone(),
        });
        userspace_log!("{}", tr_format!(literal = "Renamed folder '%before%' to '%after%'", before = before, after = requested));
    }

    /// Move a layer or a project item into `folder`, or back to the root of
    /// the section it is shown under with `None`.
    ///
    /// The section used is read fresh from the layer or item itself, not
    /// from `member`'s carried tag, which a stale view may have outlived.
    pub(crate) fn move_to_folder(&mut self, member: FolderMember, folder: Option<FolderId>) {
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        let (section, before) = match member.target() {
            MemberTarget::Layer(layer_id) => match project.project.document.layer(layer_id) {
                Some(layer) => (layer.section, layer.folder),
                None => return,
            },
            MemberTarget::Item(item) => match self.project_item_state(item) {
                Some(state) => (state.section, state.folder),
                None => return,
            },
        };
        if let Some(id) = folder
            && !project.project.folders.contains(section, id)
        {
            userspace_warn!("{}", tr!(literal = "That folder no longer exists"));
            return;
        }
        let folder_name = folder.and_then(|id| project.project.folders.name(section, id)).map(ToOwned::to_owned);
        if before == folder {
            return;
        }
        match member.target() {
            MemberTarget::Layer(layer_id) => self.execute_edit(Command::SetLayerFolder {
                id: layer_id,
                before,
                after: folder,
            }),
            MemberTarget::Item(item) => self.execute_edit(Command::SetItemFolder { item, before, after: folder }),
        }
        userspace_log!(
            "{}",
            match &folder_name {
                Some(name) => tr_format!(literal = "Moved item into folder '%name%'", name = name),
                None => tr!(literal = "Moved item to root"),
            }
        );
    }
}

// `App` is not constructible in a unit test (it borrows a live wgpu/winit
// context), so its guards (the same-kind check in `move_to_folder`, the
// missing-folder-name refusal in `rename_folder`) are verified by reading.
