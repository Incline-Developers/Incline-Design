//! What the selection-driven tools take from the current scene selection.
//!
//! Create Triangulation, the surface tools and the point-cloud tools run on
//! whatever is selected when they are opened, rather than on a pick list
//! filled inside their own dialog: select first, then act. That splits into
//! two jobs handled here - counting the selection every frame so the menu
//! entries can enable themselves, and gathering the ids once when a dialog
//! opens so it works on a set that cannot shift under it.

use std::collections::HashSet;

use crate::{
    app::{App, canvas::is_triangulation_polyline},
    model::{Object, ObjectId, SceneEntityId, point_cloud::PointCloudId, triangulation::TriangulationId},
    ui::state::SelectionCounts,
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
                _ => {}
            }
        }
        self.editor.selection_counts = counts;
    }

    /// Apply an explorer row's click to the scene selection.
    ///
    /// The tree is a second way into the same selection the viewport builds,
    /// so it follows the same rules: a plain click replaces the selection,
    /// Ctrl or Shift extends it, and clicking a selected row drops it.
    pub(crate) fn select_from_explorer_row(&mut self, handle: SceneEntityId) {
        if self.editor.selected_handles.contains(&handle) {
            self.editor.selected_handles.remove(&handle);
        } else if self.modifiers.shift_key() || self.modifiers.control_key() {
            self.editor.selected_handles.insert(handle);
        } else {
            self.editor.selected_handles.clear();
            self.editor.selected_handles.insert(handle);
        }
        self.invalidate_geometry();
    }
}
