use super::*;
use crate::rendering::scene::{
    build::{self, DocumentSceneBuildInput, DynamicSceneBuildInput, rebuild_document_scene, rebuild_dynamic_scene, restyle_document_scene},
    overlays::{OverlaySceneBuildInput, rebuild_editor_overlay},
};

const VOLUME_FEEDBACK_INTERVAL_FRAMES: u64 = 8;

#[allow(clippy::too_many_arguments)]
fn main_scene_cache_key(
    camera_uniform: &CameraUniform,
    editor: &EditorState,
    document: &Document,
    triangulations: &[OpenTriangulation],
    block_models: &[OpenBlockModel],
    drill_holes: &[OpenDrillHoleDataset],
    point_clouds: &[OpenPointCloud],
    rasters: &[OpenRasterTexture],
    drill_hole_content_key: u64,
) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    slice_preview::slice_preview_scene_key(editor, document, triangulations, block_models, drill_holes, point_clouds, rasters).hash(&mut hasher);
    // Every other item kind reaches the key above by the identity of the data
    // it is holding, but a drill hole dataset is edited in place - laying a
    // tie, turning a collar - so nothing about it here would change. Its
    // instance cache has already been resynced by the time this runs, and it
    // reports what it is holding: see `DrillHoleGpuCache::content_key`.
    drill_hole_content_key.hash(&mut hasher);
    EditorSceneState::of(editor).hash(&mut hasher);
    hasher.write(bytemuck::bytes_of(camera_uniform));
    hasher.finish()
}

/// The editor state a cached scene image is only valid for.
///
/// Most editor state reaches the renderer by being pushed: hiding an object,
/// changing a selection and the other ~90 callers of `App::invalidate_geometry`
/// all mark the scene dirty, and the cache is rebuilt on the next frame. What
/// cannot work that way is state [`Graphics::render_scene_pass`] reads for
/// itself at draw time - the renderer decides from `editor` how to draw, so
/// nothing upstream knows the scene changed. **Every such read belongs in this
/// struct**, which is the whole of the scene pass's editor dependency.
///
/// Arming Tie Holes is the case that proved it: it lifts drill traces above the
/// topology and draws the surface connectors purely by being armed, and while
/// it was missing here the switch showed nothing until the camera next moved.
#[derive(Hash)]
struct EditorSceneState {
    /// Bit pattern: the level the design plane and z-cut draw at.
    z_level: u64,
    show_xy_grid: bool,
    slice_grid_enabled: bool,
    section_grid_style: crate::ui::state::SectionGridStyle,
    xy_grid_style: crate::ui::state::PlanGridStyle,
    fly_mode_enabled: bool,
    /// Tie Holes draws drill traces without the depth test (`draw_drill_holes`).
    tying_holes: bool,
    /// Drill & Blast alone draws the surface tie-in connectors.
    shows_tie_ins: bool,
    /// Cinematic view changes what the scene pass draws and what happens to
    /// the image afterwards, so a cached frame from the other mode is wrong.
    cinematic_enabled: bool,
    /// Survey draws classified point clouds in their class colours, which
    /// `PointCloudGpuCache::sync` resolves from the editor at draw time.
    colors_points_by_classification: bool,
    /// The chunk-bounds outline is rebuilt by the scene pass, so switching
    /// either chunk-debug view on has to force one.
    debug_surface_chunks: bool,
    debug_point_cloud_chunks: bool,
}

impl EditorSceneState {
    fn of(editor: &EditorState) -> Self {
        Self {
            z_level: editor.z_level.to_bits(),
            show_xy_grid: editor.show_xy_grid,
            slice_grid_enabled: editor.slice_grid_enabled,
            section_grid_style: editor.section_grid_style,
            xy_grid_style: editor.xy_grid_style,
            fly_mode_enabled: editor.fly_mode_enabled,
            tying_holes: editor.tying_holes(),
            shows_tie_ins: editor.shows_tie_ins(),
            cinematic_enabled: editor.cinematic_enabled,
            colors_points_by_classification: editor.colors_points_by_classification(),
            debug_surface_chunks: editor.debug_surface_chunks,
            debug_point_cloud_chunks: editor.debug_point_cloud_chunks,
        }
    }
}

fn scene_cache_needs_render(cached_key: Option<u64>, next_key: u64, content_changed: bool, gpu_work_pending: bool) -> bool {
    cached_key != Some(next_key) || content_changed || gpu_work_pending
}

pub(crate) struct RenderInput<'frame> {
    pub(crate) editor: &'frame mut EditorState,
    pub(crate) document: &'frame mut Document,
    pub(crate) triangulations: &'frame [OpenTriangulation],
    pub(crate) block_models: &'frame [OpenBlockModel],
    pub(crate) drill_holes: &'frame [OpenDrillHoleDataset],
    /// Read by the borehole inspector's log, beside the inspected hole.
    pub(crate) well_logs: &'frame crate::model::geophysics::GeophysicsSession,
    pub(crate) point_clouds: &'frame [OpenPointCloud],
    pub(crate) rasters: &'frame [OpenRasterTexture],
    pub(crate) project: &'frame UiProjectView,
}

impl<'a> Graphics<'a> {
    pub(crate) fn render(&mut self, input: RenderInput<'_>) -> Result<UiFrameOutput, RenderSurfaceError> {
        let RenderInput {
            editor,
            document,
            triangulations,
            block_models,
            drill_holes,
            well_logs,
            point_clouds,
            rasters,
            project,
        } = input;
        // Acquire before any queue writes: a hidden/unavailable surface can fail
        // indefinitely, and write_buffer staging allocations live until submit.
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output) | wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
            wgpu::CurrentSurfaceTexture::Timeout => return Err(RenderSurfaceError::Timeout),
            wgpu::CurrentSurfaceTexture::Occluded => return Err(RenderSurfaceError::Occluded),
            wgpu::CurrentSurfaceTexture::Outdated => return Err(RenderSurfaceError::Outdated),
            wgpu::CurrentSurfaceTexture::Lost => return Err(RenderSurfaceError::Lost),
            wgpu::CurrentSurfaceTexture::Validation => return Err(RenderSurfaceError::Validation),
        };
        // Only scene content forces the cached scene to be re-rendered. The
        // editor overlay is drawn over the cache every frame by
        // `render_editor_overlay_pass`, so `overlay_dirty` deliberately does
        // not appear here - that is what keeps a cursor-following tool preview
        // off the critical path of a full scene render.
        let mut scene_content_changed = self.geometry_dirty;
        self.vertical_exaggeration = editor.vertical_exaggeration.clamp(0.1, 20.0);
        let slice_visible_half_length = slice_visible_half_length(self.projection.zoom, self.screen_size());
        if self.slice_view.is_some() {
            self.refresh_scene_bounds(document, triangulations, block_models, drill_holes, point_clouds, &editor.hidden_handles);
        }
        if let Some(slice) = self.slice_view.as_mut() {
            // Slice mode sets the clip planes itself; the scene-fitting passes below must not run.
            slice.width = editor.slice_width_input.clamp(0.1, 1.0e6);
            slice.move_speed = editor.slice_speed_input.clamp(0.0, 1.0e6);
            slice.rotate_speed = editor.slice_rotate_input.clamp(1.0, 720.0).to_radians();
            // Camera's own depth range, not the slab (fragment shaders clip to that directly) - kept wide enough a tilted or overhead view won't clip away geometry the slab would show.
            let forward = self.camera.forward();
            let strike = DVec3::new(slice.direction.x, slice.direction.y, 0.0);
            // Scene bounds are model elevations, the slice centre a display one - stretch bounds by the vertical exaggeration before comparing.
            let origin_z = self.scene_origin.z;
            let exaggeration = self.vertical_exaggeration;
            let display_z = |z: f64| origin_z + (z - origin_z) * exaggeration;
            let bounds = self
                .cached_scene_bounds
                .map(|(min, max)| (DVec3::new(min.x, min.y, display_z(min.z)), DVec3::new(max.x, max.y, display_z(max.z))));
            self.projection
                .set_symmetric_depth_extent(slice_depth_half_extent(slice.center, strike, forward, slice.width * 0.5, bounds) + slice.view_offset.dot(forward).abs());
            editor.slice_center = [slice.center.x, slice.center.y, slice.center.z];
            editor.slice_direction = [slice.direction.x, slice.direction.y];
            editor.slice_half_length = slice_visible_half_length;
        } else {
            self.fit_depth_to_scene(document, triangulations, block_models, drill_holes, point_clouds, &editor.hidden_handles);
            self.include_tool_previews_in_depth(editor);
            self.include_batter_berm_preview_in_depth(editor);
        }
        editor.debug_clip_plane_distances = Some(self.projection.clip_planes());
        // Uploaded every frame; outside a section this is `None`, which is what switches the shader clip off.
        self.upload_camera_uniform(editor.block_model_interaction_resolution_divisor, self.section_slab());
        let grid_uniform = GridUniform::new(
            self.scene_origin,
            editor.renderer_background_color,
            &self.camera,
            &self.projection,
            self.vertical_exaggeration,
            self.fly_mode_enabled,
            &editor.xy_grid_style,
            self.window.scale_factor(),
        );
        self.queue.write_buffer(&self.grid_buffer, 0, bytemuck::bytes_of(&grid_uniform));
        if editor.slice_grid_enabled
            && let Some((axis, axis_spacing, elevation_spacing)) = self.section_grid_spacing(editor.section_grid_style.level_spacing)
        {
            let section_grid_uniform = SectionGridUniform::new(
                self.scene_origin,
                editor.renderer_background_color,
                axis,
                axis_spacing,
                elevation_spacing,
                &editor.section_grid_style,
                self.window.scale_factor(),
            );
            self.queue.write_buffer(&self.section_grid_buffer, 0, bytemuck::bytes_of(&section_grid_uniform));
        }
        // Advance non-blocking volume-usage readbacks. Their callbacks only
        // send a small bitset through a channel; residency changes are applied
        // later by the normal streaming pass.
        if self.block_model_gpu.has_pending_feedback() {
            let _ = self.device.poll(wgpu::PollType::Poll);
            self.block_model_gpu.poll_volume_feedback();
        }
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.add_srgb_suffix()),
            ..Default::default()
        });
        // Non-sRGB view of the same texture for the egui pass; egui applies
        // gamma itself, so it wants raw byte writes (see `Gui::new`).
        let gui_view = output.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

        let scale_factor = self.window.scale_factor() as f32;
        let gpu_work_was_pending = self.point_cloud_gpu.has_pending_uploads() || self.block_model_gpu.has_pending_builds();
        self.raster_gpu
            .sync(&self.device, &self.queue, self.scene_origin, rasters, &self.raster_surface_bind_group_layout);
        self.triangulation_gpu.sync(
            &self.device,
            &self.queue,
            self.scene_origin,
            scale_factor,
            triangulations,
            rasters,
            editor,
            &self.surface_style_bind_group_layout,
            &self.surface_chunk_bind_group_layout,
            &self.edge_style_bind_group_layout,
        );
        self.block_model_gpu.sync(
            &self.device,
            &self.queue,
            self.scene_origin,
            scale_factor,
            block_models,
            editor,
            &self.surface_style_bind_group_layout,
            &self.block_model_volume_bind_group_layout,
            &self.edge_style_bind_group_layout,
        );
        self.drill_hole_gpu.sync(&self.device, &self.queue, self.scene_origin, drill_holes, editor);
        self.point_cloud_gpu.sync(
            &self.device,
            &self.queue,
            &mut encoder,
            self.scene_origin,
            scale_factor,
            glam::Mat4::from_cols_array_2d(&self.camera_uniform.view_proj),
            (self.camera.position - self.scene_origin).as_vec3(),
            point_clouds,
            editor,
            &self.point_cloud_style_bind_group_layout,
        );
        self.design_point_gpu.sync(
            &self.device,
            &self.queue,
            document,
            &editor.hidden_handles,
            self.scene_origin,
            scale_factor,
            editor.show_points || editor.active_tool == crate::ui::state::ActiveTool::DeletePoints,
            self.geometry_dirty || self.cached_document_revision != document.revision(),
        );
        // Selection, hover and translucency only restyle: the shaders read
        // them from the style buffer. The key also covers hiding and
        // translucency, which change what is tessellated and what the static
        // chunks claim, so a geometry pass still runs; `document_scene_key`
        // keeps it from re-tessellating when nothing it bakes has changed.
        let render_style_key = editor.render_style_key();
        if self.cached_render_style_key != Some(render_style_key) {
            self.cached_render_style_key = Some(render_style_key);
            self.geometry_dirty = true;
            self.document_style_dirty = true;
            self.overlay_dirty = true;
        }
        let needs_geometry_rebuild = self.geometry_dirty || self.cached_document_revision != document.revision() || (self.cached_scale_factor - scale_factor).abs() > f32::EPSILON;

        if needs_geometry_rebuild {
            scene_content_changed = true;
            // Reconcile the static stroke chunks first so the stream rebuild
            // below knows which objects they own and can skip them.
            self.static_strokes
                .sync(&self.device, &self.queue, document, editor, &mut self.document_style_slots, self.scene_origin, scale_factor);
            // Most geometry passes come from editor-state invalidation with
            // the document untouched; tessellate only when an input changed.
            let scene_key = build::document_scene_key(document, editor, self.static_strokes.claimed_key(), self.scene_origin, scale_factor);
            if self.cached_document_scene_key != Some(scene_key) {
                rebuild_document_scene(DocumentSceneBuildInput {
                    editor,
                    document,
                    static_ids: self.static_strokes.claimed(),
                    fill_cache: &mut self.polyline_fill_cache,
                    text_system: &mut self.text_system,
                    slots: &mut self.document_style_slots,
                    lyon_buffer: &mut self.lyon_buffer,
                    strokes: &mut self.strokes,
                    text_vertex_buf: &mut self.text_vertex_buf,
                    text_index_buf: &mut self.text_index_buf,
                    text_draw_batches: &mut self.text_draw_batches,
                    pick_records: &mut self.pick_records,
                    text_pick_records: &mut self.text_pick_records,
                    object_ranges: &mut self.document_object_ranges,
                    scene_origin: self.scene_origin,
                    scale_factor,
                });
                self.upload_scene_stream_buffers();
                self.cached_document_scene_key = Some(scene_key);
            }
            if self.cached_document_revision != document.revision() {
                // Both slot holders have rebuilt against this document.
                self.document_style_slots.retain(document);
            }
            self.document_style_dirty = true;

            self.cached_scale_factor = scale_factor;
            self.cached_document_revision = document.revision();
            self.geometry_dirty = false;
        }

        if self.document_style_dirty {
            restyle_document_scene(
                &mut self.document_draw_batches,
                &self.document_object_ranges,
                editor,
                &self.strokes,
                &self.lyon_buffer.vertices,
                &self.lyon_buffer.indices,
            );
            let flags = self.document_style_slots.flags(editor);
            self.document_style.write(&self.device, &self.queue, &flags);
            let claimed = self.static_strokes.claimed();
            self.document_style.static_highlighted = editor
                .selected_handles
                .iter()
                .chain(&editor.tri_hover_handles)
                .filter_map(|handle| match handle {
                    SceneEntityId::Object(id) => Some(*id),
                    _ => None,
                })
                .chain(editor.tool_highlight_id)
                .any(|id| claimed.contains(&id));
            self.document_style_dirty = false;
        }

        // Per-frame pass for the live drawing tools; the static scene above no
        // longer rebuilds while they run. Rebuilt while a tool is active and
        // once more after it deactivates (to clear the buffers).
        let dynamic_active = editor.batter_berm_dialog_open;
        if dynamic_active || !self.dynamic_strokes.is_empty() {
            rebuild_dynamic_scene(DynamicSceneBuildInput {
                editor,
                dynamic_strokes: &mut self.dynamic_strokes,
                scene_origin: self.scene_origin,
                scale_factor,
            });
            Self::upload_instance_stream(
                &self.device,
                &self.queue,
                &mut self.dynamic_stroke_gpu,
                &mut self.dynamic_stroke_capacity,
                &mut self.dynamic_strokes,
                "Dynamic Scene Stroke Buffer",
            );
        }

        let measurement_state = (
            matches!(
                editor.active_tool,
                crate::ui::state::ActiveTool::MeasureDistance | crate::ui::state::ActiveTool::MeasureBatterAngle
            ),
            editor.measurement_start,
            editor.measurement_end,
            editor.batter_angle_points.clone(),
        );
        if measurement_state != self.cached_measurement_state {
            self.cached_measurement_state = measurement_state;
            self.overlay_dirty = true;
        }
        if editor.poly_finish_dialog != self.cached_poly_finish_dialog {
            self.cached_poly_finish_dialog = editor.poly_finish_dialog;
            self.overlay_dirty = true;
        }

        if self.overlay_dirty {
            let overlay_vp = self.view_proj();
            let overlay_screen = self.screen_size();
            rebuild_editor_overlay(OverlaySceneBuildInput {
                editor,
                document,
                overlay_strokes: &mut self.overlay_strokes,
                view_proj: overlay_vp,
                screen_size: overlay_screen,
                scene_origin: self.scene_origin,
                scale_factor,
            });

            Self::upload_instance_stream(
                &self.device,
                &self.queue,
                &mut self.overlay_stroke_gpu,
                &mut self.overlay_stroke_capacity,
                &mut self.overlay_strokes,
                "Editor Overlay Stroke Buffer",
            );
            self.overlay_dirty = false;
        }

        if needs_geometry_rebuild && self.frame_index.wrapping_sub(self.last_text_cache_trim_frame) >= TEXT_CACHE_TRIM_INTERVAL_FRAMES {
            self.text_system.text_cache.trim();
            self.last_text_cache_trim_frame = self.frame_index;
        }

        let primary_frustum = frustum::Frustum::from_view_proj(glam::Mat4::from_cols_array_2d(&self.camera_uniform.view_proj));
        let scene_key = main_scene_cache_key(
            &self.camera_uniform,
            editor,
            document,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            self.drill_hole_gpu.content_key(),
        );
        let gpu_work_pending = gpu_work_was_pending
            || self.point_cloud_gpu.has_pending_uploads()
            || self.block_model_gpu.has_pending_builds()
            || self.block_model_gpu.has_visible_pending_streaming(&primary_frustum, &editor.hidden_handles);
        let scene_changed = scene_cache_needs_render(self.scene_cache_key, scene_key, scene_content_changed, gpu_work_pending);
        let render_scene = scene_changed;
        let sample_volume_feedback = render_scene && self.frame_index.is_multiple_of(VOLUME_FEEDBACK_INTERVAL_FRAMES);
        if sample_volume_feedback {
            let phase = (self.frame_index / VOLUME_FEEDBACK_INTERVAL_FRAMES) % 64;
            self.block_model_gpu
                .clear_visible_volume_feedback(&self.queue, &mut encoder, phase as u32, &primary_frustum, &editor.hidden_handles);
        }

        // The scene renders into its own cache texture and is reused for as
        // long as its key holds; the overlay pass then puts this frame's
        // editor content over it and resolves the result to the surface. On a
        // cache hit that is the whole of the scene's cost.
        // Cinematic view renders its lit scene into targets of its own and
        // puts the finished image into the cache instead, so everything
        // downstream - the overlay pass, the cache hit next frame - is
        // unchanged. Never set in the browser build, where it does not exist.
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut cinematic_post_ran = false;
        if render_scene {
            #[cfg(not(target_arch = "wasm32"))]
            if editor.cinematic_enabled {
                // The light is fitted to the scene's extent, which in every
                // other view is only computed when something needs it.
                self.refresh_scene_bounds(document, triangulations, block_models, drill_holes, point_clouds, &editor.hidden_handles);
                cinematic_post_ran = self.render_cinematic_scene(&mut encoder, editor, triangulations, block_models, drill_holes, point_clouds, rasters);
            }
            if !cinematic_post_ran {
                let cache_view = self.scene_cache.view.clone();
                self.render_scene_pass(
                    &mut encoder,
                    &cache_view,
                    self.viewport_rect,
                    editor,
                    triangulations,
                    block_models,
                    drill_holes,
                    point_clouds,
                    rasters,
                    true,
                );
            }
            self.scene_cache_key = Some(scene_key);
        }
        // The overlay pass draws over whatever the multisample target holds.
        // After an ordinary scene pass that is the scene itself; after a
        // cinematic one it is the *ungraded* scene, because the graded image
        // went to the cache - so restore from the cache exactly as a frame
        // that skipped the scene pass would.
        self.render_editor_overlay_pass(&mut encoder, &view, self.viewport_rect, editor, !render_scene || cinematic_post_ran);

        // One-shot viewport export: re-render the scene (without the egui
        // chrome) into an offscreen texture and queue a readback on this
        // frame's encoder; the PNG is written after submit below.
        let pending_screenshot = self
            .pending_screenshot
            .take()
            .map(|path| self.encode_screenshot_capture(&mut encoder, editor, triangulations, block_models, drill_holes, point_clouds, rasters, path));

        // Render the in-viewport plan preview through the same shaded scene
        // pass as the main and detached viewports. The egui panel samples this
        // offscreen texture below.
        self.render_embedded_slice_preview(document, triangulations, block_models, drill_holes, point_clouds, rasters, editor);

        // Keep the engineering-drawing dialog's map preview current.
        self.refresh_plot_preview(editor, document, triangulations, block_models, drill_holes, point_clouds, rasters);

        self.update_tool_projections(editor, document, drill_holes);

        let orbit_marker_screen = self.orbit_marker_screen_pos();
        let rotation_centre_screen = editor.rotation_centre.and_then(|centre| self.rotation_centre_screen_pos(centre));
        let camera_active = self.is_camera_active();
        let camera_forward = self.camera.forward();
        let camera_up = self.camera.up();
        let world_per_physical_pixel = (!self.projection.is_perspective()).then(|| 2.0 * self.projection.zoom / f64::from(self.viewport_rect.height.max(1)));
        let ui_output = self.gui.render(
            &self.window,
            &self.device,
            &self.queue,
            &mut encoder,
            &gui_view,
            editor,
            document,
            project,
            block_models,
            drill_holes,
            well_logs,
            [self.size.width, self.size.height],
            orbit_marker_screen,
            rotation_centre_screen,
            camera_active,
            [camera_forward.x as f32, camera_forward.y as f32, camera_forward.z as f32],
            [camera_up.x as f32, camera_up.y as f32, camera_up.z as f32],
            world_per_physical_pixel,
        );
        // Apply this frame's egui layout for next frame's scene pass - the
        // scene renders before egui lays out, so it's always one frame behind
        // (see `apply_canvas_rect`).
        self.apply_canvas_rect(ui_output.canvas_rect);
        // Keep the startup view framed on the window the splash is centred
        // on, until the splash goes - see `track_startup_view_framing`.
        self.track_startup_view_framing(project.needs_startup_dialog);
        if ui_output.geometry_dirty {
            self.invalidate_geometry();
        }
        // Keep redrawing through the interaction cooldown so the volume
        // raycaster's reduced-quality frames are always followed by a
        // full-quality one once the camera settles / resizing stops.
        if self.interaction_active() {
            self.window.request_redraw();
        }
        // Continue draining the bounded point-cloud upload queue without
        // treating background upload progress as camera interaction.
        if self.point_cloud_gpu.has_pending_uploads()
            || self.block_model_gpu.has_pending_builds()
            || self.block_model_gpu.has_pending_feedback()
            || self.block_model_gpu.has_visible_pending_streaming(&primary_frustum, &editor.hidden_handles)
        {
            self.window.request_redraw();
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        if sample_volume_feedback
            && self
                .block_model_gpu
                .schedule_visible_volume_feedback(&self.device, &self.queue, &primary_frustum, &editor.hidden_handles)
        {
            self.window.request_redraw();
        }
        self.queue.present(output);
        if let Some(capture) = pending_screenshot {
            self.finish_screenshot_capture(capture);
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        self.release_retired_attachments();

        Ok(ui_output)
    }
}
