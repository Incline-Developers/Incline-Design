//! Vertical slice view: two-click line placement and mode enter/exit.

use glam::{DVec2, DVec3};

use crate::{
    app::{App, BackgroundTaskTicket, jobs::JobKey},
    i18n::tr,
    model::{
        Command, Document, FillStyle, Layer, Object, ObjectColor, PolyVertex, SectionKind,
        kernel::XY_TOL,
        project::unique_item_name,
        spatial::TriangleBvh,
        triangulation::{OpenTriangulation, TriangulationId},
    },
    rendering::{camera::SectionSlab, scene::section_surface_lines::SurfaceLines},
    ui::state::{ActiveTool, EditorState},
    userspace_log, userspace_warn,
};

impl<'a> App<'a> {
    /// Canvas click while the Vertical Slice tool is armed. First click
    /// stores the line start; the second computes the slice frame and enters
    /// slice mode. Z of the picks only seeds the initial view elevation -
    /// the line is flat in XY by construction.
    pub(crate) fn slice_line_click(&mut self) {
        if self.editor.snapping_active() && !self.editor.cursor_snapped {
            return;
        }
        let Some(point) = self.editor.cursor_world else {
            return;
        };

        let Some(start) = self.editor.slice_pending_start else {
            self.editor.slice_pending_start = Some(point);
            self.invalidate_overlay();
            return;
        };

        let delta = (point - start).truncate();
        if delta.length_squared() <= 1.0e-12 {
            return;
        }
        let direction: DVec2 = delta.normalize();
        let center = (start + point) * 0.5;
        let half_length = delta.length() * 0.5;

        self.editor.slice_pending_start = None;
        self.editor.active_tool = ActiveTool::None;
        self.editor.slice_preview_navigation.reset();
        self.slice_preview_cursor_px = None;
        self.slice_preview_middle_down = false;
        self.set_fly_mode_enabled(false);
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.set_fly_mode_enabled(false);
            graphics.enter_slice_mode(
                center,
                direction,
                half_length,
                self.editor.slice_width_input,
                self.editor.slice_speed_input,
                self.editor.slice_rotate_input.to_radians(),
            );
            self.editor.slice_mode_enabled = true;
        }
        // No mouse event behind this camera swap; ending any orbit lets the cursor land on the section.
        self.end_right_orbit();
        self.invalidate_overlay();
        self.redraw_requested = true;
        userspace_log!(
            "{}",
            tr!(
                "cmd-slice-entered-slice-view-cx-cy",
                cx = format!("{:.3}", center.x),
                cy = format!("{:.3}", center.y),
                cz = format!("{:.3}", center.z),
                dx = format!("{:.3}", direction.x),
                dy = format!("{:.3}", direction.y),
                length = (half_length * 2.0).to_string()
            )
        );
    }

    /// Toggle the vertical slice viewing mode. Enabling without a drawn line
    /// is meaningless, so `true` only arms the placement tool.
    pub(crate) fn set_slice_mode_enabled(&mut self, enabled: bool) {
        if enabled {
            if !self.editor.slice_mode_enabled {
                self.set_active_tool_from_toolbar(ActiveTool::VerticalSlice);
            }
            return;
        }
        self.leave_slice_mode();
    }

    /// Single exit point for slice mode; a half-drawn stroke, the active tool and any picks carry over into plan view.
    pub(crate) fn leave_slice_mode(&mut self) {
        if !self.editor.slice_mode_enabled {
            return;
        }
        self.forget_section_picks();
        self.editor.slice_mode_enabled = false;
        self.editor.slice_preview_detached = false;
        self.editor.slice_preview_navigation.reset();
        self.slice_preview_cursor_px = None;
        self.slice_preview_middle_down = false;
        self.editor.selection_box_start_px = None;
        self.editor.selection_box_current_px = None;
        self.pending_selection_click = None;
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.close_slice_preview();
            graphics.exit_slice_mode();
        }
        self.end_right_orbit();
        self.clear_rotation_centre();
        self.redraw_requested = true;
        userspace_log!("{}", tr!("cmd-slice-exited-slice-view"));
    }

    /// Offset, Chamfer and Relimit picks made in a section's plane mean nothing
    /// in plan, so each tool stays armed one step back: Offset at its dialog,
    /// Chamfer and Relimit waiting for the corner or target pick.
    fn forget_section_picks(&mut self) {
        if self.drawing_section().is_none() {
            return;
        }
        let editor = &mut self.editor;
        if editor.offset_awaiting_side_pick {
            editor.offset_awaiting_side_pick = false;
            editor.offset_dialog_open = true;
            editor.offset_preview_world.clear();
            editor.offset_preview_screen_px.clear();
            editor.offset_preview_ranges.clear();
        }
        editor.chamfer_poly_id = None;
        editor.chamfer_corner_index = None;
        editor.chamfer_gizmo_drag_start_px = None;
        if editor.relimit_confirming_end {
            editor.relimit_confirming_end = false;
            editor.relimit_waiting_for_pick = true;
            editor.relimit_second_id = None;
            editor.relimit_candidates.clear();
        }
        self.invalidate_overlay();
    }

    /// Reset View while sliced: square the camera to the section plane, then
    /// frame what is visible on it. The section itself - its direction and
    /// where the slab sits along its normal - is left exactly where it is,
    /// which is what lets Reset View mean something here without dropping the
    /// mode to get back to a plan view.
    pub(crate) fn reset_slice_view(&mut self) {
        if let Some(graphics) = self.graphics.as_mut() {
            if !graphics.reset_slice_view(self.editor.rotation_centre) {
                return;
            }
            // Squared up first, so the fit measures the section as it will be seen.
            graphics.zoom_to_extents(
                &self.scene_document,
                crate::app::commands::prismoid_grid::with_prismoid_scene(&self.triangulations, &self.prismoid_scene),
                &self.block_models,
                &self.drill_holes,
                &self.point_clouds,
                &self.editor.hidden_handles,
            );
        }
        self.end_right_orbit();
        self.redraw_requested = true;
        userspace_log!("{}", tr!("cmd-slice-reset-section-view-fit-extents"));
    }

    /// Whether the cursor may be re-projected onto the section: yes when the section moves with no mouse event behind it, not while a right drag is orbiting it.
    pub(crate) fn slice_cursor_tracks_section(&self) -> bool {
        self.editor.slice_mode_enabled && !self.right_orbit_active
    }

    /// The RL grid in a section, the XY grid in plan. One button drives both,
    /// so which one it means is decided here.
    pub(crate) fn set_grid_shown(&mut self, shown: bool) -> anyhow::Result<()> {
        if self.editor.slice_mode_enabled {
            self.set_slice_grid_enabled(shown);
        } else if self.editor.show_xy_grid != shown {
            self.set_xy_grid_shown(shown);
        }
        Ok(())
    }

    /// Toggles the section grid: elevation levels plus easting/northing lines where the cut face crosses them. Not persisted with the project.
    pub(crate) fn set_slice_grid_enabled(&mut self, enabled: bool) {
        self.editor.slice_grid_enabled = enabled;
        self.redraw_requested = true;
        userspace_log!("{}", tr!("cmd-slice-set-section-grid-enabled", enabled = enabled.to_string()));
    }

    /// Surface Lines on Section: switch the lines on or off. Not persisted
    /// with the project.
    pub(crate) fn toggle_section_surface_lines(&mut self) {
        self.editor.section_surface_lines = !self.editor.section_surface_lines;
        self.redraw_requested = true;
        userspace_log!("{}", tr!("cmd-slice-set-surface-lines", enabled = self.editor.section_surface_lines.to_string()));
    }

    /// Save Surface Lines as Strings: the lines the section draws, written
    /// to a new layer as ordinary strings, one per line in its surface's
    /// colour, so they can be edited and plotted. One undo step.
    pub(crate) fn save_section_surface_lines(&mut self) {
        let section_up = self.graphics.as_ref().is_some_and(|graphics| graphics.section_slab().is_some());
        if !section_up || !self.editor.section_surface_lines_on() {
            userspace_warn!("{}", tr!("cmd-slice-save-surface-lines-off"));
            return;
        }
        // What is saved is what is drawn: bring the drawn lines up to date,
        // which cuts only if the plane or a surface changed, then save them.
        self.refresh_section_surface_lines();
        if self.section_surface_lines_waiting() {
            userspace_warn!("{}", tr!("cmd-slice-surface-lines-still-preparing"));
            return;
        }
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let surfaces = graphics.section_surface_lines();
        let Some(project) = self.workspace.active_project_mut() else {
            userspace_warn!("{}", tr!("cmd-slice-open-project-save-surface-lines"));
            return;
        };
        let Some((layer, objects)) = section_lines_layer(&mut project.project.document, surfaces) else {
            userspace_warn!("{}", tr!("cmd-slice-no-surface-crosses-section"));
            return;
        };
        let crossing = surfaces.iter().filter(|surface| !surface.lines.is_empty()).count();
        let (layer_id, name, count) = (layer.id, layer.name.clone(), objects.len());
        self.execute_edit(Command::AddLayerSnapshot { layer, objects });
        self.editor.active_layer = Some(layer_id);
        userspace_log!(
            "{}",
            tr!("cmd-slice-saved-surface-lines", count = count.to_string(), surfaces = crossing.to_string(), layer = name)
        );
        self.invalidate_geometry();
    }

    /// Cut the shown surfaces along the section again, only when the plane, a
    /// surface or the switch changed since the last cut: a still section
    /// costs a hash a frame, not a pass over every triangle. A colour change
    /// recolours the cached lines without cutting.
    pub(crate) fn refresh_section_surface_lines(&mut self) {
        if self.index_surfaces_for_section_lines() {
            return;
        }
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let slab = graphics.section_slab();
        let scene = crate::app::commands::prismoid_grid::with_prismoid_scene(&self.triangulations, &self.prismoid_scene);
        let key = section_surface_lines_key(&self.editor, slab, scene);
        if key != self.section_surface_lines_key {
            self.section_surface_lines_key = key;
            let mut surfaces = Vec::new();
            if let (Some(_), Some(slab)) = (key, slab) {
                for triangulation in shown_surfaces(&self.editor, scene) {
                    let lines = crate::app::commands::triangulation::section_surface_lines(&triangulation.mesh, &triangulation.spatial, &slab);
                    surfaces.push(SurfaceLines {
                        lines,
                        color: surface_line_color(triangulation),
                    });
                }
            }
            self.section_surface_colors = surfaces.iter().map(|surface| surface.color).collect();
            graphics.set_section_surface_lines(surfaces);
        } else if key.is_some() {
            let shown = || shown_surfaces(&self.editor, scene).map(surface_line_color);
            if !shown().eq(self.section_surface_colors.iter().copied()) {
                self.section_surface_colors = shown().collect();
                graphics.recolor_section_surface_lines(self.section_surface_colors.iter().copied());
            }
        }
    }

    /// Whether a shown surface of a section that wants lines still has no
    /// tree to cut with.
    fn section_surface_lines_waiting(&self) -> bool {
        let Some(graphics) = self.graphics.as_ref() else {
            return false;
        };
        let scene = crate::app::commands::prismoid_grid::with_prismoid_scene(&self.triangulations, &self.prismoid_scene);
        !surfaces_awaiting_tree(&self.editor, graphics.section_slab(), scene).is_empty()
    }

    /// A big surface has no tree until asked, and building one here would
    /// hold the frame. When the lines are due a cut and a shown surface has
    /// none, build them in the background instead, one job a surface, and
    /// draw nothing until they are there; the finished job asks for a redraw
    /// and the next refresh cuts. A tree someone else is building is waited
    /// for, and a status row says so while the frames keep coming. True while
    /// it waits.
    fn index_surfaces_for_section_lines(&mut self) -> bool {
        let Some(graphics) = self.graphics.as_ref() else {
            self.show_section_lines_wait(&[]);
            return false;
        };
        let scene = crate::app::commands::prismoid_grid::with_prismoid_scene(&self.triangulations, &self.prismoid_scene);
        let key = section_surface_lines_key(&self.editor, graphics.section_slab(), scene);
        if key.is_none() || key == self.section_surface_lines_key {
            self.show_section_lines_wait(&[]);
            return false;
        }
        let waiting = surfaces_awaiting_tree(&self.editor, graphics.section_slab(), scene);
        if waiting.is_empty() {
            self.show_section_lines_wait(&[]);
            return false;
        }
        let waiting_ids: Vec<_> = waiting.iter().map(|(id, _)| *id).collect();
        self.spawn_surface_index_jobs(waiting);
        self.show_section_lines_wait(&waiting_ids);
        // Whatever was drawn belongs to another plane: clear it once, and
        // forget its key so the same cut asked for later is made again.
        self.section_surface_lines_key = None;
        if let Some(graphics) = self.graphics.as_mut()
            && !graphics.section_surface_lines().is_empty()
        {
            self.section_surface_colors.clear();
            graphics.set_section_surface_lines(Vec::new());
        }
        true
    }

    /// Keeps a status row up while `waiting` holds: begins one when the wait
    /// starts, ends it when the wait is over, and returns what is shown now.
    /// The row is withdrawn, not finished: the build it waited on is not ours.
    pub(crate) fn show_build_wait(&mut self, shown: Option<BackgroundTaskTicket>, waiting: bool, label: impl FnOnce() -> String) -> Option<BackgroundTaskTicket> {
        match (shown, waiting) {
            (None, true) => Some(self.begin_reported_task(label()).0),
            (Some(ticket), false) => {
                self.cancel_background_task(ticket);
                None
            }
            (shown, _) => shown,
        }
    }

    /// Shows the wait in the status bar while a surface in `waiting` has a
    /// tree a query is building in the background, which has no job row of
    /// its own.
    fn show_section_lines_wait(&mut self, waiting: &[TriangulationId]) {
        if waiting.is_empty() && self.section_lines_wait_shown.is_none() {
            return;
        }
        let quiet = self
            .triangulations
            .iter()
            .any(|surface| waiting.contains(&surface.id) && !surface.spatial.is_built() && !self.job_pending(&JobKey::Triangulation(surface.id)));
        let shown = self.section_lines_wait_shown.take();
        self.section_lines_wait_shown = self.show_build_wait(shown, quiet, || tr!("cmd-slice-indexing-surfaces-section-lines"));
    }

    /// One background build for each surface whose tree no one is building yet.
    /// A tree another job or a query is already building is waited for, not
    /// built twice, and another tool's job on a surface does not hold up its
    /// tree.
    fn spawn_surface_index_jobs(&mut self, waiting: Vec<(TriangulationId, TriangleBvh)>) {
        for (id, spatial) in waiting {
            let Some(claim) = spatial.claim_build() else {
                continue;
            };
            self.spawn_job(
                tr!("cmd-slice-indexing-surfaces-section-lines"),
                vec![JobKey::Triangulation(id)],
                move |_cancel| {
                    spatial.ensure_built();
                    drop(claim);
                    Ok(())
                },
                |app, result| {
                    if let Err(error) = result {
                        userspace_warn!("{error}");
                    }
                    app.redraw_requested = true;
                },
            );
        }
    }

    /// Re-places the cursor after a camera move with no mouse event behind it.
    /// Walking the section carries its targets past a still mouse, so the snap
    /// is asked again here rather than dropped; off any target the cursor
    /// falls back to the section plane under it.
    pub(crate) fn refresh_slice_cursor(&mut self) {
        if !self.slice_cursor_tracks_section() {
            return;
        }
        let snapped = self.poly_close_target().or_else(|| {
            if self.editor.active_tool.snaps_cursor() && self.editor.snapping_active() {
                self.cursor_snap_point(web_time::Instant::now())
            } else {
                None
            }
        });
        let Some(world) = snapped.or_else(|| self.graphics.as_ref().and_then(|graphics| graphics.cursor_world(self.editor.z_level))) else {
            return;
        };
        if self.editor.cursor_world == Some(world) && self.editor.cursor_snapped == snapped.is_some() {
            return;
        }
        self.editor.cursor_world = Some(world);
        self.editor.cursor_snapped = snapped.is_some();
        if self.editor.overlay_follows_cursor() {
            self.invalidate_overlay();
        }
    }
}

/// The shown surfaces whose tree is not built yet, with a handle on it, while
/// the section wants lines drawn: what a cut would otherwise have to build.
fn surfaces_awaiting_tree(editor: &EditorState, slab: Option<SectionSlab>, triangulations: &[OpenTriangulation]) -> Vec<(TriangulationId, TriangleBvh)> {
    if section_surface_lines_key(editor, slab, triangulations).is_none() {
        return Vec::new();
    }
    shown_surfaces(editor, triangulations)
        .filter(|surface| !surface.spatial.is_built())
        .map(|surface| (surface.id, (*surface.spatial).clone()))
        .collect()
}

/// The surfaces a section draws: loaded and not hidden.
fn shown_surfaces<'t>(editor: &'t EditorState, triangulations: &'t [OpenTriangulation]) -> impl Iterator<Item = &'t OpenTriangulation> {
    triangulations
        .iter()
        .filter(|triangulation| triangulation.state.loaded && !editor.hidden_handles.contains(&triangulation.entity_id()))
}

/// The colour a surface's line is drawn in.
fn surface_line_color(triangulation: &OpenTriangulation) -> [f32; 4] {
    let [red, green, blue, _] = triangulation.color;
    [red, green, blue, 1.0]
}

/// The layer Save Surface Lines as Strings adds, made as the contour command
/// makes its own, and one string per cut line on it; `None` when no surface
/// crosses the section.
fn section_lines_layer(document: &mut Document, surfaces: &[SurfaceLines]) -> Option<(Layer, Vec<Object>)> {
    if surfaces.iter().all(|surface| surface.lines.is_empty()) {
        return None;
    }
    let layer_id = document.allocate_layer_id();
    let layer = Layer {
        id: layer_id,
        name: unique_item_name(tr!("cmd-slice-section-lines-layer"), document.layers().iter().map(|layer| layer.name.as_str())),
        color_index: None,
        color: [1.0, 1.0, 1.0, 1.0],
        loaded: true,
        hidden: false,
        elevation: 0.0,
        folder: None,
        section: SectionKind::natural_layer(),
        quality: None,
    };
    let objects = surfaces
        .iter()
        .flat_map(|surface| surface.lines.iter().map(move |line| (line, surface.color)))
        .map(|(line, color)| {
            let (verts, closed) = section_line_string(line);
            Object::Polyline {
                id: document.allocate_object_id(),
                layer: layer_id,
                verts,
                closed,
                color: ObjectColor::Fixed(color),
                fill: FillStyle::Clear,
                line_weight: 1.0,
            }
        })
        .collect();
    Some((layer, objects))
}

/// One cut line as a string's vertices, each where the surface meets the
/// plane. A line that comes back to its start is a ring: saved closed,
/// without the repeated point.
fn section_line_string(line: &[DVec3]) -> (Vec<PolyVertex>, bool) {
    let returns = line.first().zip(line.last()).is_some_and(|(first, last)| (*first - *last).abs().max_element() < XY_TOL);
    let closed = line.len() >= 4 && returns;
    // A line that goes out and straight back is the one segment it covers.
    let points = if closed || (line.len() == 3 && returns) { &line[..line.len() - 1] } else { line };
    (points.iter().copied().map(PolyVertex::straight).collect(), closed)
}

/// What the section's surface lines are cut from: the plane and each shown
/// surface's mesh, not its colour, which recolours without a cut. `None`
/// while none are drawn: the switch off, no section up, or a workspace other
/// than Geology, the only one with the row.
pub(crate) fn section_surface_lines_key(editor: &EditorState, slab: Option<SectionSlab>, triangulations: &[OpenTriangulation]) -> Option<u64> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    if !editor.section_surface_lines_on() {
        return None;
    }
    let slab = slab?;
    let mut hasher = DefaultHasher::new();
    hash_section_plane(&slab, &mut hasher);
    for triangulation in shown_surfaces(editor, triangulations) {
        triangulation.id.hash(&mut hasher);
        triangulation.state.revision().hash(&mut hasher);
        (std::sync::Arc::as_ptr(&triangulation.mesh) as usize).hash(&mut hasher);
    }
    Some(hasher.finish())
}

/// The section's plane into `hasher`: its normal, and where it lies to the
/// micrometre, not where on it the section is centred, so a pan along the
/// plane keeps what was cut.
pub(crate) fn hash_section_plane(slab: &SectionSlab, hasher: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    for value in slab.normal.to_array() {
        value.to_bits().hash(hasher);
    }
    ((slab.point.dot(slab.normal) * 1.0e6).round() as i64).hash(hasher);
}
