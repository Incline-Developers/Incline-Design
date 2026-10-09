use anyhow::{Context, Result};

use crate::{
    app::App,
    i18n::tr,
    model::{Command, Document, Layer, LayerId, Object, SceneEntityId, SectionKind},
    userspace_log,
};

/// `preferred`, or `preferred` with the lowest ` N` (N >= 2) suffix not
/// already taken. Shared by layer duplication (here) and folder creation
/// (`app::commands::folder`), which sit side by side in the same menu and
/// must agree on the scheme.
pub(super) fn unique_name(preferred: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(preferred) {
        return preferred.to_string();
    }

    for index in 2.. {
        let candidate = format!("{preferred} {index}");
        if !taken(&candidate) {
            return candidate;
        }
    }

    unreachable!("unbounded iterator should always find a unique name")
}

fn objects_on_layer(document: &Document, layer_id: LayerId) -> Vec<Object> {
    document.objects().iter().filter(|object| object.layer() == layer_id).cloned().collect()
}

fn positioned_objects_on_layer(document: &Document, layer_id: LayerId) -> Vec<(usize, Object)> {
    document
        .objects()
        .iter()
        .enumerate()
        .filter(|(_, object)| object.layer() == layer_id)
        .map(|(index, object)| (index, object.clone()))
        .collect()
}

impl<'a> App<'a> {
    pub(crate) fn create_layer(&mut self, name: String) -> Result<()> {
        let Some(project) = self.workspace.active_project_mut() else {
            return Ok(());
        };
        let layer_id = project.project.document.allocate_layer_id();
        let layer = Layer {
            id: layer_id,
            name: name.clone(),
            color_index: None,
            color: [1.0, 1.0, 1.0, 1.0],
            loaded: true,
            hidden: false,
            elevation: 0.0,
            folder: None,
            section: SectionKind::natural_layer(),
        };
        self.execute_edit(Command::AddLayerSnapshot { layer, objects: Vec::new() });

        self.editor.selected_handles.clear();
        self.editor.active_layer = Some(layer_id);
        userspace_log!("{}", tr!("cmd-layer-created-layer-name", name = name.to_string()));
        self.invalidate_geometry();
        Ok(())
    }

    pub(crate) fn delete_layer(&mut self, layer_id: LayerId) -> Result<()> {
        if self.restore_layers_for(vec![layer_id], move |app| {
            if let Err(error) = app.delete_layer(layer_id) {
                crate::userspace_error!("{error:#}");
            }
        }) {
            return Ok(());
        }
        self.editor.pending_delete_layer = None;
        let Some(project) = self.workspace.active_project_mut() else {
            return Ok(());
        };
        let Some(layer) = project.project.document.layer(layer_id).cloned() else {
            return Ok(());
        };
        let layer_index = project
            .project
            .document
            .layers()
            .iter()
            .position(|candidate| candidate.id == layer_id)
            .context("Layer position disappeared")?;
        let on_layer = positioned_objects_on_layer(&project.project.document, layer_id);
        self.execute_edit(Command::DeleteLayerSnapshot {
            layer,
            layer_index,
            objects: on_layer,
        });

        if self.editor.active_layer == Some(layer_id) {
            self.editor.active_layer = None;
        }
        self.editor.selected_handles.clear();
        userspace_log!("{}", tr!("cmd-layer-deleted-with-objects", layer_id = format!("{layer_id:?}")));
        self.invalidate_geometry();
        Ok(())
    }

    pub(crate) fn duplicate_layer(&mut self, layer_id: LayerId) {
        if self.restore_layers_for(vec![layer_id], move |app| app.duplicate_layer(layer_id)) {
            return;
        }
        let Some(project) = self.workspace.active_project_mut() else {
            return;
        };
        let Some(source_layer) = project.project.document.layer(layer_id).cloned() else {
            return;
        };
        let source_objects = objects_on_layer(&project.project.document, layer_id);
        let duplicate_name = unique_name(&tr!("cmd-layer-name-copy", name = source_layer.name.to_string()), |candidate| {
            project.project.document.layer_id_by_name(candidate).is_some()
        });

        let doc = &mut project.project.document;
        let new_layer_id = doc.allocate_layer_id();
        let duplicate_layer = Layer {
            id: new_layer_id,
            name: duplicate_name.clone(),
            color_index: source_layer.color_index,
            color: source_layer.color,
            loaded: source_layer.loaded,
            hidden: source_layer.hidden,
            elevation: source_layer.elevation,
            folder: source_layer.folder,
            section: source_layer.section,
        };
        let duplicate_objects: Vec<Object> = source_objects
            .into_iter()
            .map(|object| {
                let object_id = doc.allocate_object_id();
                object.with_id_and_layer(object_id, new_layer_id)
            })
            .collect();

        self.execute_edit(Command::AddLayerSnapshot {
            layer: duplicate_layer,
            objects: duplicate_objects,
        });

        self.editor.selected_handles.clear();
        userspace_log!("{}", tr!("cmd-layer-duplicated-layer-duplicate-name", duplicate_name = duplicate_name.to_string()));
        self.invalidate_geometry();
    }

    pub(crate) fn load_layer(&mut self, layer_id: LayerId) {
        self.set_layer_loaded(layer_id, true);
    }

    fn set_layer_loaded(&mut self, layer_id: LayerId, loaded: bool) {
        self.activate_project_for_layer(layer_id);
        if !loaded {
            self.cancel_jobs(|key| matches!(key, crate::app::jobs::JobKey::LayerResidency { layer: pending, restoring: true, .. } if *pending == layer_id));
        } else if self.layer_load_pending(layer_id) {
            return;
        }
        let Some(layer) = self.workspace.active_document().and_then(|document| document.layer(layer_id)) else {
            return;
        };
        if layer.loaded == loaded {
            return;
        }
        let set_loaded = Command::SetLayerLoaded {
            id: layer_id,
            before: layer.loaded,
            after: loaded,
        };
        // Loading always brings a layer in visible, whatever its eye said
        // before it was unloaded.
        if loaded && layer.hidden {
            self.execute_edit(Command::Batch(vec![
                set_loaded,
                Command::SetLayerHidden {
                    id: layer_id,
                    before: true,
                    after: false,
                },
            ]));
        } else {
            self.execute_edit(set_loaded);
        }
    }

    /// The explorer's eye on a layer: hiding leaves it loaded, and showing
    /// one that is unloaded loads it.
    pub(crate) fn set_layer_visible(&mut self, layer_id: LayerId, visible: bool) {
        self.activate_project_for_layer(layer_id);
        let Some(layer) = self.workspace.active_document().and_then(|document| document.layer(layer_id)) else {
            return;
        };
        if visible && !layer.loaded {
            self.set_layer_loaded(layer_id, true);
            return;
        }
        if !layer.loaded || layer.hidden == !visible {
            return;
        }
        let before = layer.hidden;
        self.execute_edit(Command::SetLayerHidden {
            id: layer_id,
            before,
            after: !visible,
        });
        self.invalidate_geometry();
    }

    pub(crate) fn select_all_objects_in_layer(&mut self, layer_id: LayerId) {
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        if !project.project.document.layer(layer_id).is_some_and(Layer::is_visible) {
            return;
        }
        let handles: Vec<SceneEntityId> = project
            .project
            .document
            .objects()
            .iter()
            .filter(|object| object.layer() == layer_id)
            .map(|object| SceneEntityId::Object(object.id()))
            .collect();

        self.editor.active_layer = Some(layer_id);
        self.editor.selected_handles = handles.into_iter().collect();
        self.editor.tri_selected_object_ids.clear();
        self.editor.tri_selected_layer_ids.clear();
        self.editor.canvas_context_menu_open = false;
        let count = self.editor.selected_handles.len();
        userspace_log!(
            "{}",
            tr!("cmd-layer-selected-count-object-s-layer", count = count.to_string(), layer_id = format!("{layer_id:?}"))
        );
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Unload a layer from the scene while retaining its project membership.
    pub(crate) fn unload_layer(&mut self, layer_id: LayerId) {
        self.set_layer_loaded(layer_id, false);
    }

    /// Lock or unlock every object on a layer against selection and editing.
    ///
    /// The lock is held per layer; `invalidate_geometry` expands it onto the
    /// individual object handles that picking and snapping test.
    pub(crate) fn toggle_layer_locked(&mut self, layer_id: LayerId) {
        self.activate_project_for_layer(layer_id);
        let name = self
            .workspace
            .active_project()
            .and_then(|project| project.project.document.layer(layer_id))
            .map(|layer| layer.name.clone());
        let Some(name) = name else {
            return;
        };
        let locked = !self.editor.locked_layers.remove(&layer_id);
        if locked {
            self.editor.locked_layers.insert(layer_id);
            // A locked layer cannot be the drawing target, and nothing on it
            // may stay selected.
            if self.editor.active_layer == Some(layer_id) {
                self.editor.active_layer = None;
            }
        }
        let state = if locked { tr!("cmd-layer-locked") } else { tr!("cmd-layer-unlocked") };
        userspace_log!("{}", tr!("cmd-layer-state-layer-name", state = state.to_string(), name = name.to_string()));
        self.invalidate_geometry();
    }
}
