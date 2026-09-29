//! Explorer folder commands, shared by every section.
//!
//! Design layers and the five project-item kinds each get their own folder
//! list under [`SectionKind`], but the create/delete/rename/move mechanics
//! are the same shape everywhere - this module is that shared
//! implementation, generalised from the Designs-only slice it replaced.

use anyhow::Result;

use crate::{
    app::App,
    i18n::{tr, tr_format},
    model::{Command, Folder, FolderId, FolderMember, ItemRef, MemberTarget, Placement, SectionKind},
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
        let name = unique_collection_name(&tr!(literal = "Collection"), |candidate| registry.has_name(section, candidate));
        let id = registry.allocate_id();
        let folder = Folder { id, name: name.clone() };
        self.execute_edit(Command::AddFolder { section, folder });
        userspace_log!("{}", tr_format!(literal = "Created collection '%name%'", name = name));
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
        userspace_log!("{}", tr_format!(literal = "Deleted collection '%name%'", name = name));
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
            userspace_warn!("{}", tr_format!(literal = "A collection named '%name%' already exists", name = requested));
            return;
        }
        self.execute_edit(Command::RenameFolder {
            section,
            id: folder,
            before: before.clone(),
            after: requested.clone(),
        });
        userspace_log!("{}", tr_format!(literal = "Renamed collection '%before%' to '%after%'", before = before, after = requested));
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
            userspace_warn!("{}", tr!(literal = "That section cannot hold this item"));
            return;
        }
        if let Some(id) = folder
            && !project.project.folders.contains(section, id)
        {
            userspace_warn!("{}", tr!(literal = "That collection no longer exists"));
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
                Some(name) => tr_format!(literal = "Moved item into collection '%name%'", name = name),
                None => tr_format!(literal = "Moved item to the root of %section%", section = section_name),
            }
        );
    }
}

// `App` is not constructible in a unit test (it borrows a live wgpu/winit
// context), so its guards (the admission check in `move_to_folder`, the
// missing-folder-name refusal in `rename_folder`) are verified by reading.
