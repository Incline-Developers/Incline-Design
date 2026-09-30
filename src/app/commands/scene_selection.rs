//! What the selection-driven tools take from the current scene selection.
//!
//! Create Triangulation, the surface tools, the point-cloud tools and the
//! estimation tools run on whatever is selected when they are opened, rather
//! than on a pick list filled inside their own dialog: select first, then act.
//! That splits into three jobs handled here - counting the selection every
//! frame so the menu entries can enable themselves, gathering the ids once
//! when a dialog opens so it works on a set that cannot shift under it, and
//! turning an explorer row's click into the selection it names.

use std::collections::HashSet;

use crate::{
    app::{App, canvas::is_triangulation_polyline},
    model::{Object, ObjectId, SceneEntityId, block_model::BlockModelId, drill_hole::DrillHoleId, point_cloud::PointCloudId, triangulation::TriangulationId},
    ui::state::{ExplorerRow, SelectionCounts},
};

impl App<'_> {
    /// Selected design objects that can contribute an edge to a triangulation.
    ///
    /// Returned in document order, so a triangulation does not depend on the
    /// order the selection set happens to iterate in.
    pub(crate) fn selected_triangulation_sources(&self) -> Vec<ObjectId> {
        let selected: HashSet<ObjectId> = self
            .editor
            .selected_handles
            .iter()
            .filter_map(|handle| match handle {
                SceneEntityId::Object(object_id) => Some(*object_id),
                _ => None,
            })
            .collect();
        if selected.is_empty() {
            return Vec::new();
        }
        self.scene_document
            .objects()
            .iter()
            .filter(|object| selected.contains(&object.id()) && is_triangulation_polyline(object))
            .map(Object::id)
            .collect()
    }

    /// The selected closed polyline, when the selection holds exactly one.
    ///
    /// The clip tools take a single boundary rather than a set, so anything
    /// else is not an answer and the caller reports it as such.
    pub(crate) fn selected_clip_boundary(&self) -> Option<ObjectId> {
        let mut boundary = None;
        for handle in &self.editor.selected_handles {
            let SceneEntityId::Object(object_id) = handle else {
                continue;
            };
            if self.scene_document.get_object(*object_id).is_some_and(Object::encloses_area) {
                if boundary.is_some() {
                    return None;
                }
                boundary = Some(*object_id);
            }
        }
        boundary
    }

    /// Selected triangulations that are loaded, in project order.
    ///
    /// An unloaded surface has no mesh to work on, so it is not an input even
    /// though its explorer row can be selected.
    pub(crate) fn selected_triangulations(&self) -> Vec<TriangulationId> {
        self.triangulations
            .iter()
            .filter(|triangulation| triangulation.state.loaded && self.editor.selected_handles.contains(&SceneEntityId::Triangulation(triangulation.id)))
            .map(|triangulation| triangulation.id)
            .collect()
    }

    /// Selected point clouds that are loaded, in project order.
    pub(crate) fn selected_point_clouds(&self) -> Vec<PointCloudId> {
        self.point_clouds
            .iter()
            .filter(|cloud| cloud.state.loaded && self.editor.selected_handles.contains(&SceneEntityId::PointCloud(cloud.id)))
            .map(|cloud| cloud.id)
            .collect()
    }

    /// Selected drill-hole datasets that are loaded, in project order.
    pub(crate) fn selected_drill_hole_datasets(&self) -> Vec<DrillHoleId> {
        self.drill_holes
            .iter()
            .filter(|dataset| dataset.state.loaded && self.editor.selected_handles.contains(&SceneEntityId::DrillHole(dataset.id)))
            .map(|dataset| dataset.id)
            .collect()
    }

    /// Selected block models that are loaded, in project order.
    pub(crate) fn selected_block_models(&self) -> Vec<BlockModelId> {
        self.block_models
            .iter()
            .filter(|model| model.state.loaded && self.editor.selected_handles.contains(&SceneEntityId::BlockModel(model.id)))
            .map(|model| model.id)
            .collect()
    }

    /// Refresh the per-kind selection counts the menus read.
    ///
    /// Walks the selection rather than the project, so the cost is the size of
    /// what the user has picked and no caching is needed to run it each frame.
    pub(crate) fn refresh_selection_counts(&mut self) {
        let mut counts = SelectionCounts::default();
        for handle in &self.editor.selected_handles {
            match handle {
                SceneEntityId::Object(object_id) => {
                    if let Some(object) = self.scene_document.get_object(*object_id) {
                        if is_triangulation_polyline(object) {
                            counts.triangulation_sources += 1;
                        }
                        if object.encloses_area() {
                            counts.clip_boundaries += 1;
                        }
                    }
                }
                SceneEntityId::Triangulation(id) => {
                    if self.triangulations.iter().any(|triangulation| triangulation.id == *id && triangulation.state.loaded) {
                        counts.triangulations += 1;
                    }
                }
                SceneEntityId::PointCloud(id) if self.point_clouds.iter().any(|cloud| cloud.id == *id && cloud.state.loaded) => {
                    counts.point_clouds += 1;
                }
                SceneEntityId::DrillHole(id) if self.drill_holes.iter().any(|dataset| dataset.id == *id && dataset.state.loaded) => {
                    counts.drill_holes += 1;
                }
                SceneEntityId::BlockModel(id) if self.block_models.iter().any(|model| model.id == *id && model.state.loaded) => {
                    counts.block_models += 1;
                }
                _ => {}
            }
        }
        self.editor.selection_counts = counts;
    }

    /// Apply an explorer row's click to the scene selection.
    ///
    /// The tree is a second way into the same selection the viewport builds,
    /// and it reads the modifiers the way every other tree does: a plain click
    /// replaces the selection, Ctrl (Cmd on macOS) adds or drops one row, and
    /// Shift takes the whole run between the row last clicked and this one, in
    /// the order the tree shows them. A layer row stands for every object on
    /// it, so all three work the same whether the row names one entity or a
    /// layer's worth of them.
    pub(crate) fn select_from_explorer_row(&mut self, row: ExplorerRow) {
        // A tool open on a snapshot of the selection freezes it, in the tree
        // as well as in the viewport: a row that appeared to become the tool's
        // input would not reach the run. See `selection_locked_by_tool`.
        if self.editor.selection_locked_by_tool() {
            return;
        }
        // A layer belongs to one project, and its objects can only be read out
        // of that project's document - so clicking it makes that project the
        // active one, exactly as the row's own Select All Objects does.
        if let ExplorerRow::Layer(layer_id) = row {
            self.activate_project_for_layer(layer_id);
            self.editor.active_layer = Some(layer_id);
        }
        let toggle = self.modifiers.control_key() || (cfg!(target_os = "macos") && self.modifiers.super_key());
        if self.modifiers.shift_key() {
            // The anchor stays where it was, so walking a Shift-click up and
            // down the tree grows and shrinks one run rather than leaving a
            // trail. A row no longer in the tree - its section was collapsed,
            // or its entity removed - leaves the click to act on its own row.
            let run = explorer_run(&self.editor.explorer_rows, self.editor.explorer_anchor, row);
            if run.is_none() {
                self.editor.explorer_anchor = Some(row);
            }
            let handles = self.handles_for_rows(run.as_deref().unwrap_or(&[row]));
            self.clear_scene_selection();
            self.editor.selected_handles.extend(handles);
        } else if toggle {
            let handles = self.handles_for_rows(&[row]);
            let selected = &mut self.editor.selected_handles;
            // A row already wholly selected drops out; one only partly
            // selected - a layer some of whose objects are - fills in.
            if !handles.is_empty() && handles.iter().all(|handle| selected.contains(handle)) {
                for handle in &handles {
                    selected.remove(handle);
                }
            } else {
                selected.extend(handles);
            }
            // The anchor follows the last row the user pointed at, so a Shift
            // click after a Ctrl click runs from there rather than from
            // whatever was clicked before it.
            self.editor.explorer_anchor = Some(row);
        } else {
            let handles = self.handles_for_rows(&[row]);
            self.clear_scene_selection();
            self.editor.selected_handles.extend(handles);
            self.editor.explorer_anchor = Some(row);
        }
        self.sync_active_triangulation();
        self.invalidate_geometry();
    }

    /// The scene entities `rows` stand for, in tree order.
    ///
    /// An entity row is itself; a layer row is every object standing on it. An
    /// unloaded layer has nothing in the scene to select, so it contributes
    /// nothing rather than a selection that cannot be acted on.
    fn handles_for_rows(&self, rows: &[ExplorerRow]) -> Vec<SceneEntityId> {
        let mut handles = Vec::with_capacity(rows.len());
        for row in rows {
            match row {
                ExplorerRow::Entity(handle) => handles.push(*handle),
                ExplorerRow::Layer(layer_id) => {
                    // Resolved through the layer's own project rather than the
                    // active one: a run can cross rows the click never landed
                    // on, and those name the project they came from.
                    let Some(project) = self.workspace.project_index_for_layer(*layer_id).map(|index| &self.workspace.projects[index]) else {
                        continue;
                    };
                    if !project.project.document.layer(*layer_id).is_some_and(|layer| layer.loaded) {
                        continue;
                    }
                    handles.extend(
                        project
                            .project
                            .document
                            .objects()
                            .iter()
                            .filter(|object| object.layer() == *layer_id)
                            .map(|object| SceneEntityId::Object(object.id())),
                    );
                }
            }
        }
        handles
    }

    /// Drop everything the scene holds selected, including the individual
    /// holes and tie-ins Drill & Blast selects in place of whole datasets: a
    /// click that replaces the selection replaces all of it, as it does in the
    /// viewport.
    fn clear_scene_selection(&mut self) {
        self.editor.selected_handles.clear();
        self.editor.selected_drill_holes.clear();
        self.editor.selected_tie_ins.clear();
    }

    /// Point the active surface at the selection, the way a viewport pick does.
    ///
    /// The tools that default one of their inputs to "the surface you were
    /// last working on" read this. A selection naming more than one surface,
    /// or none, leaves them without a default rather than picking one.
    fn sync_active_triangulation(&mut self) {
        let mut selected = self.editor.selected_handles.iter().filter_map(|handle| match handle {
            SceneEntityId::Triangulation(id) => Some(*id),
            _ => None,
        });
        self.active_triangulation = match (selected.next(), selected.next()) {
            (Some(id), None) => Some(id),
            _ => None,
        };
    }
}

/// The rows from `anchor` to `target` inclusive, in tree order.
///
/// `None` when the target is not among the rows the tree last drew, which
/// leaves the caller to fall back to the target alone. An anchor that is no
/// longer there starts the run at the target, so the click selects one row.
fn explorer_run(rows: &[ExplorerRow], anchor: Option<ExplorerRow>, target: ExplorerRow) -> Option<Vec<ExplorerRow>> {
    let target = rows.iter().position(|row| *row == target)?;
    let anchor = anchor.and_then(|anchor| rows.iter().position(|row| *row == anchor)).unwrap_or(target);
    let (first, last) = if anchor <= target { (anchor, target) } else { (target, anchor) };
    Some(rows[first..=last].to_vec())
}
