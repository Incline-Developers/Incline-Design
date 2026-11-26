//! The tools that manage what is in the project, as the explorer does:
//! renaming, deleting, loading and unloading, locking, duplicating layers,
//! and the folders the explorer files items in.

use serde_json::{Value, json};

use super::{Target, entity, failed, ok};
use crate::{
    app::App,
    model::{FolderId, FolderMember, ItemRef, LayerId, MemberTarget, SectionKind},
    ui::state::{RenameTarget, UiCommand},
};

/// The explorer's sections, as the tools name them.
const SECTIONS: [(&str, SectionKind); 6] = [
    ("designs", SectionKind::Designs),
    ("surfaces", SectionKind::Triangulations),
    ("rasters", SectionKind::Rasters),
    ("point_clouds", SectionKind::PointClouds),
    ("block_models", SectionKind::BlockModels),
    ("drillholes", SectionKind::DrillHoles),
];

/// The name the tools give a section.
pub(super) fn section_name(section: SectionKind) -> &'static str {
    SECTIONS.iter().find(|(_, kind)| *kind == section).map_or("designs", |(name, _)| name)
}

/// The kinds an item tool takes: whole items and layers, not objects.
const ITEM_KINDS: [&str; 6] = ["surface", "block_model", "drillholes", "point_cloud", "raster", "design_layer"];

/// The schema of `kind` and `id` naming one item or layer.
fn item_properties() -> serde_json::Map<String, Value> {
    let mut properties = serde_json::Map::new();
    properties.insert("kind".to_owned(), json!({ "type": "string", "enum": ITEM_KINDS }));
    properties.insert("id".to_owned(), json!({ "type": "integer", "minimum": 0 }));
    properties
}

/// An item tool's schema: `kind`, `id` and `extra`.
fn item_tool(name: &str, title: &str, description: &str, extra: &[(&str, Value)], destructive: bool) -> Value {
    let mut properties = item_properties();
    let mut required = vec!["kind", "id"];
    for (key, schema) in extra {
        properties.insert((*key).to_owned(), schema.clone());
        required.push(key);
    }
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "annotations": { "readOnlyHint": false, "destructiveHint": destructive, "idempotentHint": true },
    })
}

pub(super) fn definitions() -> Vec<Value> {
    let section = json!({ "type": "string", "enum": SECTIONS.map(|(name, _)| name) });
    vec![
        item_tool(
            "rename_item",
            "Rename an item",
            "Rename a surface, block model, drillhole collection, point cloud, raster or design layer. One undo step.",
            &[("name", json!({ "type": "string", "minLength": 1 }))],
            false,
        ),
        item_tool(
            "delete_item",
            "Delete an item",
            "Delete a surface, block model, drillhole collection, point cloud, raster or design layer (with everything on it) from the project. One undo step: it can be undone.",
            &[],
            true,
        ),
        item_tool(
            "set_loaded",
            "Load or unload an item",
            "Load an item into memory, or unload it to free memory while keeping it in the project, as the explorer's load toggle does. Loading a large item takes a moment: get_project says when it is loaded.",
            &[("loaded", json!({ "type": "boolean" }))],
            false,
        ),
        item_tool(
            "set_locked",
            "Lock or unlock an item",
            "Lock an item, or a design layer's objects, against selection and editing, or unlock it, as the explorer's lock does.",
            &[("locked", json!({ "type": "boolean" }))],
            false,
        ),
        json!({
            "name": "duplicate_layer",
            "title": "Duplicate a design layer",
            "description": "Copy a design layer and everything on it as a new layer. One undo step. Returns the new layer's id.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "integer", "minimum": 0 } }, "required": ["id"], "additionalProperties": false },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        json!({
            "name": "create_folder",
            "title": "Create a folder",
            "description": "Add a folder to one of the explorer's sections, to file items in with move_to_folder. get_project lists the folders. Returns the new folder's id.",
            "inputSchema": {
                "type": "object",
                "properties": { "section": section, "name": { "type": "string", "minLength": 1 } },
                "required": ["section", "name"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        }),
        item_tool(
            "move_to_folder",
            "File an item in a folder",
            "Move an item or design layer into a folder of the explorer, or back to its section's top level.",
            &[(
                "folder",
                json!({ "type": ["integer", "null"], "minimum": 0, "description": "The folder's id, from get_project, or null for the section's top level." }),
            )],
            false,
        ),
        json!({
            "name": "delete_folder",
            "title": "Delete a folder",
            "description": "Delete one of the explorer's folders. What it holds goes back to the section's top level, unless with_contents, which deletes that too. One undo step.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "minimum": 0 },
                    "with_contents": { "type": "boolean", "description": "Delete what is in it too. False if left out." },
                },
                "required": ["id"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true },
        }),
        json!({
            "name": "rename_folder",
            "title": "Rename a folder",
            "description": "Rename one of the explorer's folders.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer", "minimum": 0 }, "name": { "type": "string", "minLength": 1 } },
                "required": ["id", "name"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
        }),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_item_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let result = match name {
            "rename_item" => self.mcp_rename_item(arguments),
            "delete_item" => self.mcp_delete_item(arguments),
            "set_loaded" => self.mcp_set_loaded(arguments),
            "set_locked" => self.mcp_set_locked(arguments),
            "duplicate_layer" => self.mcp_duplicate_layer(arguments),
            "create_folder" => self.mcp_create_folder(arguments),
            "move_to_folder" => self.mcp_move_to_folder(arguments),
            "delete_folder" => self.mcp_delete_folder(arguments),
            "rename_folder" => self.mcp_rename_folder(arguments),
            _ => return None,
        };
        Some(result.map_or_else(failed, |value| ok(&value)))
    }

    /// The item or layer `kind` and `id` name.
    fn mcp_item_target(&self, arguments: &Value) -> Result<Target, String> {
        let (Some(kind), Some(id)) = (arguments["kind"].as_str(), arguments["id"].as_u64()) else {
            return Err("Give the item's kind and id.".to_owned());
        };
        match self.mcp_target(kind, id)? {
            Target::Object(_) => Err("This works on whole items and layers. Design objects have their own tools.".to_owned()),
            target => Ok(target),
        }
    }

    /// Run `command`, failing as it does.
    fn mcp_run(&mut self, command: UiCommand) -> Result<(), String> {
        self.handle_reported_ui_command(command).map_err(|error| format!("{error:#}"))
    }

    fn mcp_rename_item(&mut self, arguments: &Value) -> Result<Value, String> {
        let target = self.mcp_item_target(arguments)?;
        let name = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty()).ok_or("Give the new name.")?;
        let target = match target {
            Target::Layer(id) => RenameTarget::Layer(id),
            Target::Item(ItemRef::Triangulation(id)) => RenameTarget::Triangulation(id),
            Target::Item(ItemRef::BlockModel(id)) => RenameTarget::BlockModel(id),
            Target::Item(ItemRef::DrillHole(id)) => RenameTarget::DrillHole(id),
            Target::Item(ItemRef::PointCloud(id)) => RenameTarget::PointCloud(id),
            Target::Item(ItemRef::Raster(id)) => RenameTarget::Raster(id),
            Target::Item(ItemRef::PrismoidGrid(id)) => RenameTarget::PrismoidGrid(id),
            Target::Object(_) => unreachable!("refused by mcp_item_target"),
        };
        self.mcp_run(UiCommand::RenameItem {
            target,
            new_name: name.to_owned(),
        })?;
        Ok(json!({ "renamed": name }))
    }

    fn mcp_delete_item(&mut self, arguments: &Value) -> Result<Value, String> {
        let command = match self.mcp_item_target(arguments)? {
            Target::Layer(id) => UiCommand::DeleteLayer(id),
            Target::Item(ItemRef::Triangulation(id)) => UiCommand::RemoveTriangulation(id),
            Target::Item(ItemRef::BlockModel(id)) => UiCommand::RemoveBlockModel(id),
            Target::Item(ItemRef::DrillHole(id)) => UiCommand::RemoveDrillHole(id),
            Target::Item(ItemRef::PointCloud(id)) => UiCommand::RemovePointCloud(id),
            Target::Item(ItemRef::Raster(id)) => UiCommand::RemoveRaster(id),
            Target::Item(ItemRef::PrismoidGrid(id)) => UiCommand::RemovePrismoidGrid(id),
            Target::Object(_) => unreachable!("refused by mcp_item_target"),
        };
        self.mcp_run(command)?;
        Ok(json!({ "deleted": true }))
    }

    fn mcp_set_loaded(&mut self, arguments: &Value) -> Result<Value, String> {
        let loaded = arguments["loaded"].as_bool().ok_or("Give loaded.")?;
        let command = match (self.mcp_item_target(arguments)?, loaded) {
            (Target::Layer(id), true) => UiCommand::LoadLayer(id),
            (Target::Layer(id), false) => UiCommand::UnloadLayer(id),
            (Target::Item(ItemRef::Triangulation(id)), true) => UiCommand::LoadTriangulation(id),
            (Target::Item(ItemRef::Triangulation(id)), false) => UiCommand::CloseTriangulation(id),
            (Target::Item(ItemRef::BlockModel(id)), true) => UiCommand::LoadBlockModel(id),
            (Target::Item(ItemRef::BlockModel(id)), false) => UiCommand::CloseBlockModel(id),
            (Target::Item(ItemRef::DrillHole(id)), true) => UiCommand::LoadDrillHole(id),
            (Target::Item(ItemRef::DrillHole(id)), false) => UiCommand::CloseDrillHole(id),
            (Target::Item(ItemRef::PointCloud(id)), true) => UiCommand::LoadPointCloud(id),
            (Target::Item(ItemRef::PointCloud(id)), false) => UiCommand::ClosePointCloud(id),
            (Target::Item(ItemRef::Raster(id)), true) => UiCommand::LoadRaster(id),
            (Target::Item(ItemRef::Raster(id)), false) => UiCommand::UnloadRaster(id),
            (Target::Item(ItemRef::PrismoidGrid(id)), true) => UiCommand::LoadPrismoidGrid(id),
            (Target::Item(ItemRef::PrismoidGrid(id)), false) => UiCommand::ClosePrismoidGrid(id),
            (Target::Object(_), _) => unreachable!("refused by mcp_item_target"),
        };
        self.mcp_run(command)?;
        Ok(json!({ "loaded": loaded }))
    }

    fn mcp_set_locked(&mut self, arguments: &Value) -> Result<Value, String> {
        let locked = arguments["locked"].as_bool().ok_or("Give locked.")?;
        let command = match self.mcp_item_target(arguments)? {
            Target::Layer(id) => (self.editor.locked_layers.contains(&id) != locked).then_some(UiCommand::ToggleLayerLocked(id)),
            target => {
                let handle = entity(target).ok_or("That cannot be locked.")?;
                let now = if self.editor.page_locks_items() {
                    self.editor.shows_locked(handle)
                } else {
                    self.editor.frozen_handles.contains(&handle)
                };
                (now != locked).then_some(UiCommand::ToggleEntityLocked(handle))
            }
        };
        if let Some(command) = command {
            self.mcp_run(command)?;
        }
        Ok(json!({ "locked": locked }))
    }

    fn mcp_duplicate_layer(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["id"].as_u64().map(LayerId).ok_or("Give the layer's id.")?;
        let layers = |app: &Self| -> Vec<LayerId> {
            app.workspace
                .active_document()
                .map(|document| document.layers().iter().map(|layer| layer.id).collect())
                .unwrap_or_default()
        };
        let before = layers(self);
        if !before.contains(&id) {
            return Err(format!("There is no design layer with id {}.", id.0));
        }
        self.mcp_run(UiCommand::DuplicateLayer(id))?;
        let new: Vec<u64> = layers(self).into_iter().filter(|layer| !before.contains(layer)).map(|layer| layer.0).collect();
        Ok(json!({ "new_layer": new.first() }))
    }

    /// Every folder's id, by section.
    fn mcp_folder_ids(&self) -> Vec<(SectionKind, FolderId)> {
        let Some(project) = self.workspace.active_project() else {
            return Vec::new();
        };
        SECTIONS
            .iter()
            .flat_map(|(_, section)| project.project.folders.folders(*section).iter().map(|folder| (*section, folder.id)))
            .collect()
    }

    /// The section a folder is in.
    fn mcp_folder_section(&self, id: u64) -> Result<SectionKind, String> {
        self.mcp_folder_ids()
            .into_iter()
            .find(|(_, folder)| folder.0 == id)
            .map(|(section, _)| section)
            .ok_or_else(|| format!("There is no folder with id {id}. get_project lists them."))
    }

    fn mcp_create_folder(&mut self, arguments: &Value) -> Result<Value, String> {
        let section = arguments["section"]
            .as_str()
            .and_then(|name| SECTIONS.iter().find(|(key, _)| *key == name))
            .map(|(_, section)| *section)
            .ok_or("section is designs, surfaces, rasters, point_clouds, block_models or drillholes.")?;
        let name = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty()).ok_or("Give the folder a name.")?;
        let before = self.mcp_folder_ids();
        self.mcp_run(UiCommand::CreateFolder(section))?;
        let new = self
            .mcp_folder_ids()
            .into_iter()
            .find(|folder| !before.contains(folder))
            .ok_or("The folder was not made.")?;
        self.mcp_run(UiCommand::RenameItem {
            target: RenameTarget::Folder(section, new.1),
            new_name: name.to_owned(),
        })?;
        Ok(json!({ "id": new.1.0, "section": section_name(section), "name": name }))
    }

    fn mcp_move_to_folder(&mut self, arguments: &Value) -> Result<Value, String> {
        let target = match self.mcp_item_target(arguments)? {
            Target::Layer(id) => MemberTarget::Layer(id),
            Target::Item(item) => MemberTarget::Item(item),
            Target::Object(_) => unreachable!("refused by mcp_item_target"),
        };
        let current = match target {
            MemberTarget::Layer(id) => self.workspace.active_document().and_then(|document| document.layer(id)).map(|layer| layer.section),
            MemberTarget::Item(item) => self.project_item_state(item).map(|state| state.section),
        }
        .ok_or("That item has gone.")?;
        let (section, folder) = match arguments["folder"].as_u64() {
            Some(id) => (self.mcp_folder_section(id)?, Some(FolderId(id))),
            None => (current, None),
        };
        if !section.admits(target.kind()) {
            return Err(format!("A folder in {} cannot hold that.", section_name(section)));
        }
        self.mcp_run(UiCommand::MoveToFolder {
            member: FolderMember::new(current, target),
            section,
            folder,
        })?;
        Ok(json!({ "folder": folder.map(|folder| folder.0), "section": section_name(section) }))
    }

    fn mcp_delete_folder(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["id"].as_u64().ok_or("Give the folder's id.")?;
        let section = self.mcp_folder_section(id)?;
        let folder = FolderId(id);
        self.mcp_run(if arguments["with_contents"].as_bool().unwrap_or(false) {
            UiCommand::DeleteFolderAndContents { section, folder }
        } else {
            UiCommand::DeleteFolder { section, folder }
        })?;
        Ok(json!({ "deleted": id }))
    }

    fn mcp_rename_folder(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["id"].as_u64().ok_or("Give the folder's id.")?;
        let name = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty()).ok_or("Give the new name.")?;
        let section = self.mcp_folder_section(id)?;
        self.mcp_run(UiCommand::RenameItem {
            target: RenameTarget::Folder(section, FolderId(id)),
            new_name: name.to_owned(),
        })?;
        Ok(json!({ "renamed": name }))
    }
}
