//! Vertical slice view: two-click line placement and mode enter/exit.

use glam::DVec2;

use crate::{
    app::App,
    i18n::tr,
    model::triangulation::OpenTriangulation,
    rendering::{camera::SectionSlab, scene::section_surface_lines::SurfaceLines},
    ui::state::{ActiveTool, EditorState, Workspace},
    userspace_log,
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
                &self.triangulations,
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

    /// Cut the shown surfaces along the section again, only when the plane, a
    /// surface or the switch changed since the last cut: a still section
    /// costs a hash a frame, not a pass over every triangle. A colour change
    /// recolours the cached lines without cutting.
    pub(crate) fn refresh_section_surface_lines(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let slab = graphics.section_slab();
        let key = section_surface_lines_key(&self.editor, slab, &self.triangulations);
        if key != self.section_surface_lines_key {
            self.section_surface_lines_key = key;
            let mut surfaces = Vec::new();
            if let (Some(_), Some(slab)) = (key, slab) {
                for triangulation in shown_surfaces(&self.editor, &self.triangulations) {
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
            let shown = || shown_surfaces(&self.editor, &self.triangulations).map(surface_line_color);
            if !shown().eq(self.section_surface_colors.iter().copied()) {
                self.section_surface_colors = shown().collect();
                graphics.recolor_section_surface_lines(self.section_surface_colors.iter().copied());
            }
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

/// What the section's surface lines are cut from: the plane and each shown
/// surface's mesh, not its colour, which recolours without a cut. `None`
/// while none are drawn: the switch off, no section up, or a workspace other
/// than Geology, the only one with the row.
pub(crate) fn section_surface_lines_key(editor: &EditorState, slab: Option<SectionSlab>, triangulations: &[OpenTriangulation]) -> Option<u64> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    if !editor.section_surface_lines || editor.active_workspace != Workspace::Geology {
        return None;
    }
    let slab = slab?;
    let mut hasher = DefaultHasher::new();
    for value in slab.normal.to_array() {
        value.to_bits().hash(&mut hasher);
    }
    // Where the plane lies, to the micrometre, not where on it the section is
    // centred: a pan along the plane keeps the lines.
    ((slab.point.dot(slab.normal) * 1.0e6).round() as i64).hash(&mut hasher);
    for triangulation in shown_surfaces(editor, triangulations) {
        triangulation.id.hash(&mut hasher);
        triangulation.state.revision().hash(&mut hasher);
        (std::sync::Arc::as_ptr(&triangulation.mesh) as usize).hash(&mut hasher);
    }
    Some(hasher.finish())
}
