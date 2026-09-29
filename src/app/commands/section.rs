//! Bulk show/hide/lock actions for one explorer section, as offered by the
//! right-click menu on each section heading.
//!
//! Eye and lock actions apply to every row, including unloaded entries.

use crate::{
    app::App,
    i18n::tr_format,
    model::{Command, ItemRef, LayerId, MemberKind, SceneEntityId, SectionKind},
    ui::state::ExplorerSection,
    userspace_log,
};

/// The tag a section's layer rows carry, for the sections that hold layers
/// rather than project items.
///
/// Asked of the section rather than listed here, so a section that starts
/// showing layers is covered by having said so in `admitted`.
fn section_layer_tag(section: ExplorerSection) -> Option<SectionKind> {
    let tag = section.kind();
    tag.admits(MemberKind::Layer).then_some(tag)
}

impl<'a> App<'a> {
    /// Ids of the active project's layers tagged `tag`, in document order.
    fn section_layer_ids(&self, tag: SectionKind) -> Vec<LayerId> {
        let Some(project) = self.workspace.active_project() else {
            return Vec::new();
        };
        project
            .project
            .document
            .layers()
            .iter()
            .filter(|layer| layer.section == tag)
            .map(|layer| layer.id)
            .collect()
    }

    /// Scene entities in `section`, derived from [`Self::section_items`].
    ///
    /// Rasters map to nothing here even though they are project items: a
    /// raster has no scene entity of its own, and its lock lives in
    /// `EditorState::locked_rasters` instead.
    fn section_entities(&self, section: ExplorerSection) -> Vec<SceneEntityId> {
        self.section_items(section)
            .into_iter()
            .filter_map(|item| match item {
                ItemRef::Triangulation(id) => Some(SceneEntityId::Triangulation(id)),
                ItemRef::PointCloud(id) => Some(SceneEntityId::PointCloud(id)),
                ItemRef::BlockModel(id) => Some(SceneEntityId::BlockModel(id)),
                ItemRef::DrillHole(id) => Some(SceneEntityId::DrillHole(id)),
                ItemRef::Raster(_) => None,
            })
            .collect()
    }

    /// Project items tagged `section`, following each item's own
    /// [`SectionKind`] tag rather than its kind - a triangulation tagged
    /// Modelling turns up here under Modelling, not under Triangulations.
    /// Empty for the layer sections, since a layer is never an `ItemRef`.
    fn section_items(&self, section: ExplorerSection) -> Vec<ItemRef> {
        let tag = section.kind();
        let tagged = |state: &crate::model::project::ProjectItemState| state.section == tag;
        let triangulations = self.triangulations.iter().filter(|item| tagged(&item.state)).map(|item| ItemRef::Triangulation(item.id));
        let rasters = self.raster_textures.iter().filter(|item| tagged(&item.state)).map(|item| ItemRef::Raster(item.id));
        let point_clouds = self.point_clouds.iter().filter(|item| tagged(&item.state)).map(|item| ItemRef::PointCloud(item.id));
        let block_models = self.block_models.iter().filter(|item| tagged(&item.state)).map(|item| ItemRef::BlockModel(item.id));
        let drill_holes = self.drill_holes.iter().filter(|item| tagged(&item.state)).map(|item| ItemRef::DrillHole(item.id));
        triangulations.chain(rasters).chain(point_clouds).chain(block_models).chain(drill_holes).collect()
    }

    /// Load or unload every item in one explorer section, as a single
    /// undo step.
    pub(crate) fn set_section_visible(&mut self, section: ExplorerSection, visible: bool) {
        let layer_tag = section_layer_tag(section);
        if let (Some(tag), true) = (layer_tag, visible) {
            let needed = self
                .workspace
                .active_document()
                .map(|document| {
                    document
                        .deferred_layers
                        .keys()
                        .copied()
                        .filter(|id| document.layer(*id).is_some_and(|layer| layer.section == tag))
                        .collect()
                })
                .unwrap_or_default();
            if self.restore_layers_for(needed, move |app| app.set_section_visible(section, visible)) {
                return;
            }
        }
        let mut commands = Vec::new();
        if let Some(tag) = layer_tag
            && let Some(document) = self.workspace.active_document()
        {
            commands.extend(document.layers().iter().filter(|layer| layer.section == tag).map(|layer| layer.id).filter_map(|id| {
                document.layer(id).filter(|layer| layer.loaded != visible).map(|_| Command::SetLayerLoaded {
                    id,
                    before: !visible,
                    after: visible,
                })
            }));
            // Individual objects hidden from the canvas menu own no
            // explorer row, so this is where they come back.
            if visible && tag == SectionKind::Designs {
                commands.extend(document.hidden_object_ids().map(|id| Command::SetObjectHidden { id, before: true, after: false }));
            }
        }
        // A section can hold both layers and items (Modelling), so this half
        // always runs; it is empty for the layer-only sections.
        commands.extend(
            self.section_items(section)
                .into_iter()
                .filter_map(|item| self.item_style_command(item, |style| style.with_loaded(visible))),
        );

        let changed = commands.len();
        if visible {
            // Clear any remaining transient overrides when revealing rows.
            for entity in self.section_entities(section) {
                self.editor.hidden_handles.remove(&entity);
            }
        }
        if !commands.is_empty() {
            self.execute_edit(Command::Batch(commands));
        }
        userspace_log!(
            "{}",
            tr_format!(
                literal = "%verb% %count% item(s) in %section%",
                verb = if visible { "Revealed" } else { "Hid" },
                count = changed,
                section = section.label()
            )
        );
        self.invalidate_geometry();
    }

    /// Lock or unlock every item in one explorer section.
    pub(crate) fn set_section_locked(&mut self, section: ExplorerSection, locked: bool) {
        let mut changed = 0usize;
        if let Some(tag) = section_layer_tag(section) {
            for id in self.section_layer_ids(tag) {
                let already = self.editor.locked_layers.contains(&id);
                if already == locked {
                    continue;
                }
                changed += 1;
                if locked {
                    self.editor.locked_layers.insert(id);
                    if self.editor.active_layer == Some(id) {
                        self.editor.active_layer = None;
                    }
                } else {
                    self.editor.locked_layers.remove(&id);
                }
            }
            // Same for objects locked from the canvas menu: releasing the
            // layers has to release those too, or they stay stuck.
            if !locked && section == ExplorerSection::Designs {
                self.editor.explicitly_frozen.retain(|handle| !matches!(handle, SceneEntityId::Object(_)));
            }
        }
        // Filtered by tag rather than gated on `section == Rasters`: only that
        // section admits rasters, so the filter is naturally empty elsewhere.
        for raster in self.raster_textures.iter().filter(|raster| raster.state.section == section.kind()) {
            let already = self.editor.locked_rasters.contains(&raster.id);
            if already == locked {
                continue;
            }
            changed += 1;
            if locked {
                self.editor.locked_rasters.insert(raster.id);
            } else {
                self.editor.locked_rasters.remove(&raster.id);
            }
        }
        for entity in self.section_entities(section) {
            if self.editor.frozen_handles.contains(&entity) != locked {
                changed += 1;
            }
            self.editor.set_entity_locked(entity, locked);
        }
        userspace_log!(
            "{}",
            tr_format!(
                literal = "%verb% %count% item(s) in %section%",
                verb = if locked { "Locked" } else { "Unlocked" },
                count = changed,
                section = section.label()
            )
        );
        self.invalidate_geometry();
    }
}
