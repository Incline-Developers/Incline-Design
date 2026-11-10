//! Renaming explorer items.
//!
//! Design layers live in the `Document` and project items live beside it, but
//! both renames are undoable `Command`s against the same history - the item
//! one through [`Command::RenameItem`], which bumps the item's revision and so
//! re-derives the `UiProjectView` as well as marking the project dirty.

use crate::{
    app::App,
    i18n::tr,
    model::{Command, ItemRef, LayerId, SceneEntityId, project::unique_item_name},
    ui::state::{ExplorerRow, RenameTarget},
    userspace_log, userspace_warn,
};

/// Names of the other items of the same kind, which a rename must not collide with.
macro_rules! sibling_names {
    ($collection:expr, $id:expr) => {
        $collection.iter().filter(|item| item.id != $id).map(|item| item.name.clone()).collect()
    };
}

impl<'a> App<'a> {
    /// Open the rename field on `target`, filled with its current name.
    pub(crate) fn begin_rename_item(&mut self, target: RenameTarget) {
        if let RenameTarget::Layer(layer_id) = target {
            self.activate_project_for_layer(layer_id);
        }
        let current_name = self.rename_target_name(target).unwrap_or_default();
        self.editor.renaming_item = Some((target, current_name));
    }

    /// F2: rename the explorer row last clicked, as its right-click Rename
    /// does. The row must still be in the tree and selected, so the key never
    /// renames something the user has since moved away from, and a locked row
    /// stays as it is, its menu entry being disabled too.
    pub(crate) fn begin_rename_from_explorer(&mut self) {
        let Some(row) = self.editor.explorer_anchor.filter(|row| self.editor.explorer_rows.contains(row)) else {
            return;
        };
        let editor = &self.editor;
        let target = match row {
            ExplorerRow::Layer(id) if editor.selected_layers.contains(&id) && !editor.locked_layers.contains(&id) => RenameTarget::Layer(id),
            ExplorerRow::Entity(handle) if editor.selected_handles.contains(&handle) && !editor.frozen_handles.contains(&handle) => match handle {
                SceneEntityId::Triangulation(id) => RenameTarget::Triangulation(id),
                SceneEntityId::Raster(id) if !editor.locked_rasters.contains(&id) => RenameTarget::Raster(id),
                SceneEntityId::PointCloud(id) => RenameTarget::PointCloud(id),
                SceneEntityId::BlockModel(id) => RenameTarget::BlockModel(id),
                SceneEntityId::DrillHole(id) => RenameTarget::DrillHole(id),
                _ => return,
            },
            _ => return,
        };
        self.begin_rename_item(target);
        self.redraw_requested = true;
    }

    /// Current display name of a rename target, or `None` once it is gone.
    pub(crate) fn rename_target_name(&self, target: RenameTarget) -> Option<String> {
        match target {
            RenameTarget::Layer(id) => self
                .workspace
                .active_project()
                .and_then(|project| project.project.document.layer(id))
                .map(|layer| layer.name.clone()),
            RenameTarget::Triangulation(id) => self.triangulations.iter().find(|item| item.id == id).map(|item| item.name.clone()),
            RenameTarget::Raster(id) => self.raster_textures.iter().find(|item| item.id == id).map(|item| item.name.clone()),
            RenameTarget::PointCloud(id) => self.point_clouds.iter().find(|item| item.id == id).map(|item| item.name.clone()),
            RenameTarget::BlockModel(id) => self.block_models.iter().find(|item| item.id == id).map(|item| item.name.clone()),
            RenameTarget::DrillHole(id) => self.drill_holes.iter().find(|item| item.id == id).map(|item| item.name.clone()),
            RenameTarget::ReserveField(id) => self
                .workspace
                .active_document()
                .and_then(|document| document.reserve_field(id))
                .map(|field| field.name.clone()),
            RenameTarget::Solid(id) => self.workspace.active_document().and_then(|document| document.solid(id)).map(|solid| solid.name.clone()),
            RenameTarget::BlastShape(blast) => self.blast_name(blast),
            RenameTarget::Folder(section, id) => self
                .workspace
                .active_project()
                .and_then(|project| project.project.folders.name(section, id))
                .map(ToOwned::to_owned),
        }
    }

    /// Rename a design layer through the undo history (or, for an unloaded
    /// layer, directly - the history only ever describes loaded geometry).
    pub(crate) fn rename_layer(&mut self, layer_id: LayerId, new_name: String) {
        let Some((before, is_loaded)) = self.workspace.active_project().and_then(|project| {
            project
                .project
                .document
                .layer(layer_id)
                .map(|layer| (layer.name.clone(), project.project.document.layer(layer_id).is_some_and(|layer| layer.loaded)))
        }) else {
            return;
        };
        if before == new_name {
            return;
        }
        if is_loaded {
            self.editor.active_layer = Some(layer_id);
        }
        self.execute_edit(Command::RenameLayer {
            id: layer_id,
            before,
            after: new_name,
        });
    }

    /// Rename a triangulation, raster, point cloud, block model, or drill hole
    /// dataset owned by the active project. Names stay unique within a kind so
    /// the explorer - and an OMF save's element names - stay unambiguous.
    pub(crate) fn rename_project_item(&mut self, target: RenameTarget, new_name: String) {
        let requested = new_name.trim().to_owned();
        if requested.is_empty() {
            return;
        }
        let Some(before) = self.rename_target_name(target) else {
            userspace_warn!("{}", tr!("cmd-rename-item-no-longer-belongs-active"));
            return;
        };
        if before == requested {
            return;
        }
        let siblings: Vec<String> = match target {
            RenameTarget::Layer(_) | RenameTarget::Folder(..) | RenameTarget::ReserveField(_) | RenameTarget::Solid(_) | RenameTarget::BlastShape(_) => return,
            RenameTarget::Triangulation(id) => sibling_names!(self.triangulations, id),
            RenameTarget::Raster(id) => sibling_names!(self.raster_textures, id),
            RenameTarget::PointCloud(id) => sibling_names!(self.point_clouds, id),
            RenameTarget::BlockModel(id) => sibling_names!(self.block_models, id),
            RenameTarget::DrillHole(id) => sibling_names!(self.drill_holes, id),
        };
        let name = unique_item_name(requested.clone(), siblings.iter().map(String::as_str));
        let item = match target {
            RenameTarget::Layer(_) | RenameTarget::Folder(..) | RenameTarget::ReserveField(_) | RenameTarget::Solid(_) | RenameTarget::BlastShape(_) => return,
            RenameTarget::Triangulation(id) => ItemRef::Triangulation(id),
            RenameTarget::Raster(id) => ItemRef::Raster(id),
            RenameTarget::PointCloud(id) => ItemRef::PointCloud(id),
            RenameTarget::BlockModel(id) => ItemRef::BlockModel(id),
            RenameTarget::DrillHole(id) => ItemRef::DrillHole(id),
        };
        self.execute_edit(Command::RenameItem {
            item,
            before: before.clone(),
            after: name.clone(),
        });
        if name == requested {
            userspace_log!("{}", tr!("cmd-rename-renamed-before-name", before = before.to_string(), name = name.to_string()));
        } else {
            userspace_log!(
                "{}",
                tr!(
                    "cmd-rename-renamed-name-taken",
                    before = before.to_string(),
                    name = name.to_string(),
                    requested = requested.to_string()
                )
            );
        }
        self.redraw_requested = true;
    }
}
