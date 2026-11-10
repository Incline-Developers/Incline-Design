//! Opening the tools that work on one named item, and pointing an open one at
//! the item chosen in its dialog.
//!
//! Each opens whatever is selected: on the item when the selection names
//! exactly one of the right kind, and otherwise empty, for its dialog to
//! choose. Either way the same code derives what follows from the item.
//!
//! The tools that work on a set of design objects or holes are here too.
//! Those have no names to list, so each opens on what the selection holds of
//! its kind, empty if nothing, and its dialog's Pick hands the viewport's
//! selection back until Done, when the same code reads it again.

use crate::{
    app::{App, commands::triangulation::reference_surface::surface_input},
    i18n::tr,
    model::{Axis, ObjectId, SceneEntityId, block_model::BlockModelId, point_cloud::PointCloudId, triangulation::TriangulationId},
    ui::{
        dialogs::MoveToAxisDialog,
        state::{ReferencePointsDraft, ReferenceSurfaceDraft, SelectionPick, SelectionPickTool, ToolSubject, TriCreatePhase, TriPolylineClipMode},
    },
};

/// The one item of a selection, when it holds exactly one.
fn only<T: Copy>(items: &[T]) -> Option<T> {
    match items {
        [item] => Some(*item),
        _ => None,
    }
}

impl App<'_> {
    pub(crate) fn set_tool_subject(&mut self, subject: ToolSubject) {
        match subject {
            ToolSubject::ClipSurface(id) => self.set_clip_surface(id),
            ToolSubject::SliceSurface(id) => self.set_slice_surface(id),
            ToolSubject::ContourSurface(id) => self.set_contour_surface(id),
            ToolSubject::OreBlockModel(id) => self.set_ore_block_model(id),
            ToolSubject::BlockModelDrillholes(id) => self.open_create_block_model_dialog(id, None),
            ToolSubject::TinPointCloud(id) => self.set_tin_point_cloud(id),
            ToolSubject::ClassifyPointClouds(ids) => self.set_classify_point_clouds(ids),
        }
    }

    /// Open Clip by Polyline. A closed polyline in the selection is taken as
    /// the boundary; otherwise the dialog picks one from the viewport.
    pub(crate) fn open_clip_surface(&mut self) {
        let polyline_id = self.selected_clip_boundary();
        let boundary_name = polyline_id.map_or_else(String::new, |polyline_id| {
            self.scene_document
                .get_object(polyline_id)
                .and_then(|object| self.scene_document.layer(object.layer()))
                .map(|layer| tr!("common-polyline-layer", layer = layer.name.to_string()))
                .unwrap_or_else(|| tr!("common-polyline"))
        });
        self.editor.tri_cut_poly_open = true;
        self.editor.tri_hover_handles.clear();
        self.editor.tri_cut_poly_tri_id = None;
        self.editor.tri_cut_poly_object_id = polyline_id;
        self.editor.tri_cut_poly_object_name = boundary_name;
        self.editor.tri_cut_poly_mode = TriPolylineClipMode::KeepInside;
        self.editor.tri_cut_poly_name_input.clear();
        self.editor.tri_cut_poly_unload_source = true;
        if let Some(id) = only(&self.selected_triangulations()) {
            self.set_clip_surface(id);
        }
    }

    fn set_clip_surface(&mut self, id: TriangulationId) {
        let Some(surface) = self.triangulations.iter().find(|surface| surface.id == id && surface.state.loaded) else {
            return;
        };
        self.editor.tri_cut_poly_name_input = crate::app::canvas::derived_triangulation_name(&surface.name, &tr!("cmd-commands-clipped"));
        self.editor.tri_cut_poly_tri_id = Some(id);
    }

    pub(crate) fn open_slice_surface(&mut self) {
        self.editor.tri_cut_z_open = true;
        self.editor.tri_cut_z_tri_id = None;
        self.editor.tri_cut_z_min_input = 0.0;
        self.editor.tri_cut_z_max_input = 0.0;
        self.editor.tri_cut_z_limits = (0.0, 0.0);
        self.editor.tri_cut_z_name_input.clear();
        self.editor.tri_cut_z_unload_source = true;
        if let Some(id) = only(&self.selected_triangulations()) {
            self.set_slice_surface(id);
        }
    }

    fn set_slice_surface(&mut self, id: TriangulationId) {
        let Some(surface) = self.triangulations.iter().find(|surface| surface.id == id && surface.state.loaded) else {
            return;
        };
        let bounds = surface.mesh.bounds();
        self.editor.tri_cut_z_name_input = crate::app::canvas::derived_triangulation_name(&surface.name, &tr!("cmd-commands-sliced"));
        self.editor.tri_cut_z_tri_id = Some(id);
        self.editor.tri_cut_z_min_input = bounds.min.z;
        self.editor.tri_cut_z_max_input = bounds.max.z;
        self.editor.tri_cut_z_limits = (bounds.min.z, bounds.max.z);
    }

    pub(crate) fn open_contour_surface(&mut self) {
        self.editor.tri_contour_open = true;
        self.editor.tri_contour_tri_id = None;
        self.editor.tri_contour_target_layer = None;
        self.editor.tri_contour_layer_name_auto = true;
        self.editor.tri_contour_layer_name_input = tr!("common-surface-contours");
        self.editor.tri_contour_z_limits = (0.0, 0.0);
        self.editor.tri_contour_z_min_input = 0.0;
        self.editor.tri_contour_z_max_input = 0.0;
        if let Some(id) = only(&self.selected_triangulations()) {
            self.set_contour_surface(id);
        }
    }

    fn set_contour_surface(&mut self, id: TriangulationId) {
        let Some(surface) = self.triangulations.iter().find(|surface| surface.id == id && surface.state.loaded) else {
            return;
        };
        let bounds = surface.mesh.bounds();
        let name = surface.name.clone();
        self.editor.tri_contour_tri_id = Some(id);
        self.editor.tri_contour_z_limits = (bounds.min.z, bounds.max.z);
        self.editor.tri_contour_z_min_input = bounds.min.z;
        self.editor.tri_contour_z_max_input = bounds.max.z;
        // Only the automatic layer name follows the surface; a typed one stays.
        self.editor.update_contour_layer_name_from_surface(&name);
    }

    pub(crate) fn open_ore_triangulation(&mut self) {
        self.editor.ore_triangulation_open = true;
        self.editor.ore_block_model_id = None;
        if let Some(id) = only(&self.selected_block_models()) {
            self.set_ore_block_model(id);
        }
    }

    fn set_ore_block_model(&mut self, id: BlockModelId) {
        let Some(model) = self.block_models.iter().find(|model| model.id == id && model.state.loaded) else {
            return;
        };
        self.editor.ore_block_model_id = Some(id);
        self.editor.ore_variable = model
            .active_color_variable
            .clone()
            .filter(|name| model.model.numeric_variables().into_iter().any(|variable| variable.name == *name))
            .or_else(|| model.model.numeric_variables().into_iter().find(|var| !var.special).map(|var| var.name.clone()))
            .unwrap_or_default();
        let stem = std::path::Path::new(&model.name).file_stem().and_then(|value| value.to_str()).unwrap_or(&model.name);
        self.editor.ore_name_input = tr!("err-surf-ore-name", stem = stem.to_string());
    }

    /// Open Create Block Model: on the one collection the selection reaches
    /// (whole, or only the holes picked in it), or empty for the dialog to
    /// choose a collection.
    pub(crate) fn open_create_block_model(&mut self) {
        match self.block_model_source() {
            Some((drill_hole_id, holes)) => self.open_create_block_model_dialog(drill_hole_id, holes),
            None => {
                self.editor.block_model_create_open = true;
                self.editor.kriging_drill_hole_id = None;
                self.editor.kriging_holes = None;
            }
        }
    }

    pub(crate) fn open_point_cloud_tin(&mut self) {
        self.editor.point_cloud_tin_open = true;
        self.editor.point_cloud_tin_cloud_id = None;
        self.editor.point_cloud_tin_ground_count = None;
        // Keep any name the user already typed; otherwise restore the
        // default rather than opening with an empty, un-runnable field.
        if self.editor.point_cloud_tin_name_input.trim().is_empty() {
            self.editor.point_cloud_tin_name_input = tr!("tri-type-open-surface");
        }
        if let Some(id) = only(&self.selected_point_clouds()) {
            self.set_tin_point_cloud(id);
        }
    }

    fn set_tin_point_cloud(&mut self, id: PointCloudId) {
        let Some(cloud) = self.point_clouds.iter().find(|cloud| cloud.id == id && cloud.state.loaded) else {
            return;
        };
        self.editor.point_cloud_tin_ground_count = cloud.classifications.as_deref().map(|codes| {
            use rayon::prelude::*;
            let ground = codes.par_iter().filter(|&&code| code == crate::model::point_cloud::CLASS_GROUND).count();
            (id, ground)
        });
        self.editor.point_cloud_tin_cloud_id = Some(id);
    }

    /// Open the Join dialog with the selected clouds ticked.
    pub(crate) fn open_point_cloud_join(&mut self) {
        self.editor.point_cloud_join_open = true;
        self.editor.point_cloud_join_sources = self.selected_point_clouds();
        if self.editor.point_cloud_join_name_input.trim().is_empty() {
            self.editor.point_cloud_join_name_input = tr!("common-joined-cloud");
        }
    }

    /// Open the Classify dialog with the selected clouds ticked.
    pub(crate) fn open_point_cloud_classify(&mut self) {
        self.editor.point_cloud_classify_open = true;
        self.set_classify_point_clouds(self.selected_point_clouds());
    }

    fn set_classify_point_clouds(&mut self, sources: Vec<PointCloudId>) {
        // The sparsest cloud sets the recommendation: a cloth fine enough for
        // the densest would leave the sparsest's particles with nothing under
        // them.
        self.editor.point_cloud_classify_spacing = self
            .point_clouds
            .iter()
            .filter(|cloud| sources.contains(&cloud.id))
            .filter_map(|cloud| crate::model::ground_filter::plan_spacing(&cloud.points))
            .reduce(f64::max);
        self.editor.point_cloud_classify_extents = self
            .point_clouds
            .iter()
            .filter(|cloud| sources.contains(&cloud.id))
            .map(|cloud| (cloud.id, (cloud.bounds.1 - cloud.bounds.0).truncate()))
            .collect();
        self.editor.point_cloud_classify_sources = sources;
    }

    /// Hide `tool`'s dialog and let the viewport select its input. The
    /// selection carries on from what the tool holds, which is what is
    /// selected already: nothing else could select while its dialog was up.
    pub(crate) fn begin_selection_pick(&mut self, tool: SelectionPickTool) {
        self.editor.cancel_triangulation_pick();
        self.editor.selection_pick = Some(SelectionPick {
            tool,
            handles_before: self.editor.selected_handles.clone(),
            holes_before: self.editor.selected_drill_holes.clone(),
        });
        self.invalidate_geometry();
    }

    /// Done: the tool reads the selection again, as it did when it opened.
    pub(crate) fn finish_selection_pick(&mut self) {
        let Some(pick) = self.editor.selection_pick.take() else {
            return;
        };
        match pick.tool {
            SelectionPickTool::SetAxis => {
                if let Some(axis) = self.editor.move_to_axis_dialog.as_ref().map(|dialog| dialog.axis) {
                    self.open_move_to_axis(axis);
                }
            }
            SelectionPickTool::InsertAtElevation => self.open_insert_point_at_elevation_dialog(),
            SelectionPickTool::InsertAtIntersection => self.insert_points_at_selected_intersections(),
            SelectionPickTool::CreateTriangulation => self.take_triangulation_sources(),
            SelectionPickTool::ReferencePoints => self.open_reference_points(),
            SelectionPickTool::ReferenceSurface => self.open_reference_surface(),
        }
        self.invalidate_geometry();
    }

    /// Put the pick down: the selection goes back to what it started from,
    /// and the tool keeps what it had.
    pub(crate) fn cancel_selection_pick(&mut self) {
        let Some(pick) = self.editor.selection_pick.take() else {
            return;
        };
        self.editor.selected_handles = pick.handles_before;
        self.editor.selected_drill_holes = pick.holes_before;
        self.invalidate_geometry();
    }

    pub(crate) fn clear_selection_pick(&mut self) {
        if self.editor.selection_pick.is_none() {
            return;
        }
        self.editor.selected_handles.clear();
        self.editor.selected_drill_holes.clear();
        self.active_triangulation = None;
        self.invalidate_geometry();
    }

    /// Open Set X/Y/Z on the selected objects, or point the open dialog at
    /// them, keeping the value it already holds.
    pub(crate) fn open_move_to_axis(&mut self, axis: Axis) {
        let object_ids: Vec<ObjectId> = self
            .editor
            .selected_handles
            .iter()
            .filter_map(|&handle| match handle {
                SceneEntityId::Object(id) => Some(id),
                _ => None,
            })
            .collect();
        // A value typed over an empty set was typed blind, so only one that
        // had objects under it is kept.
        let held = self
            .editor
            .move_to_axis_dialog
            .as_ref()
            .filter(|dialog| dialog.axis == axis && !dialog.object_ids.is_empty())
            .map(|dialog| dialog.value);
        // Seed Z from the toolbar plane, X and Y from the first selected object.
        let value = held.unwrap_or_else(|| {
            if axis == Axis::Z {
                self.editor.z_input
            } else {
                object_ids
                    .first()
                    .and_then(|id| self.workspace.active_project()?.project.document.get_object(*id))
                    .map_or(0.0, |object| object.axis_position(axis))
            }
        });
        self.editor.move_to_axis_dialog = Some(MoveToAxisDialog { object_ids, axis, value });
    }

    /// Open Create Triangulation on the selected objects that can contribute
    /// an edge, or on none, for its Pick to fill.
    pub(crate) fn open_create_triangulation(&mut self) {
        self.editor.tri_create_open = true;
        self.editor.tri_create_phase = TriCreatePhase::MainDialog;
        self.editor.tri_selected_layer_ids.clear();
        self.editor.tri_name_input = tr!("tri-type-open-surface");
        self.editor.tri_hover_handles.clear();
        self.take_triangulation_sources();
    }

    /// Objects that cannot contribute an edge are dropped from the selection
    /// as well as from the tool's list, keeping the viewport highlight and
    /// the dialog's count in agreement.
    fn take_triangulation_sources(&mut self) {
        let object_ids = self.selected_triangulation_sources();
        self.editor.selected_handles = object_ids.iter().map(|&object_id| SceneEntityId::Object(object_id)).collect();
        self.editor.tri_selected_object_ids = object_ids;
    }

    /// Open Reference Points on the selected holes, or point the open dialog
    /// at them. Both ways of naming holes feed it: a collection taken whole,
    /// and holes picked one at a time.
    pub(crate) fn open_reference_points(&mut self) {
        let mut holes = Vec::new();
        self.for_each_reference_hole(|hole| holes.push(hole));
        // The selection is two unordered sets; sorting here keeps the
        // layer's points and the flagged list in a settled order.
        holes.sort_unstable_by_key(|hole| (hole.dataset.0, hole.hole));
        match self.editor.reference_points_dialog.as_mut() {
            Some(draft) => draft.holes = holes,
            None => self.editor.reference_points_dialog = Some(ReferencePointsDraft { holes, ..Default::default() }),
        }
    }

    /// Open Build Surface on the selection, or point the open dialog at it.
    /// Its inputs are of different kinds, so one selection names them all
    /// without anything having to say which is which.
    pub(crate) fn open_reference_surface(&mut self) {
        let input = surface_input(&self.scene_document, &self.editor.selected_handles);
        let layer_name = |id| self.scene_document.layer(id).map(|layer| layer.name.clone()).unwrap_or_default();
        let points_label = match input.layers.as_slice() {
            [] => tr!("reference-surface-no-points"),
            [layer] => tr!(
                "cmd-commands-count-point-s-layer",
                count = input.points.len().to_string(),
                layer = layer_name(*layer).to_string()
            ),
            layers => tr!(
                "cmd-commands-count-point-s-across-layers",
                count = input.points.len().to_string(),
                layers = layers.len().to_string()
            ),
        };
        let extent_label = match input.extents.as_slice() {
            [] => tr!("cmd-commands-no-extent"),
            [id] => self
                .scene_document
                .get_object(*id)
                .map(|object| {
                    tr!(
                        "cmd-commands-kind-layer",
                        kind = object.kind_name().to_string(),
                        layer = layer_name(object.layer()).to_string()
                    )
                })
                .unwrap_or_else(|| tr!("cmd-commands-no-extent")),
            extents => tr!("reference-surface-count-closed-polylines", count = extents.len().to_string()),
        };
        let controls_label = match input.controls.len() {
            0 => tr!("cmd-commands-no-control-strings"),
            count => {
                let mut control_layers = input.controls.iter().map(|id| self.scene_document.get_object(*id).map(|object| object.layer()));
                match control_layers.next().flatten() {
                    Some(layer) if control_layers.all(|other| other == Some(layer)) => {
                        tr!(
                            "cmd-commands-count-control-string-s-layer",
                            count = count.to_string(),
                            layer = layer_name(layer).to_string()
                        )
                    }
                    _ => tr!("cmd-commands-count-control-string-s", count = count.to_string()),
                }
            }
        };
        self.editor.reference_surface_dialog = Some(ReferenceSurfaceDraft {
            problem: input.problem(),
            extent: only(&input.extents),
            points: input.points,
            controls: input.controls,
            points_label,
            controls_label,
            extent_label,
        });
    }
}
