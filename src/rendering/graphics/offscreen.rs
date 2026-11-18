//! Pictures of the scene drawn offscreen, at a size of their own, with no
//! window involved: the video export's frames, and the views an MCP client
//! asks for (`render_view`), which come from a camera of their own so the
//! window's view never moves for them.

#[cfg(not(target_arch = "wasm32"))]
use super::readback::RgbaReadback;
use super::*;

/// A target the renderer's own passes draw into in place of the window: its
/// camera, depth and multisample targets, swapped with the window's for the
/// length of a draw.
pub(crate) struct OffscreenTarget {
    pub(super) size: winit::dpi::PhysicalSize<u32>,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) texture: wgpu::Texture,
    /// The scene draws through the sRGB view, egui through the plain one,
    /// as they do on the window's surface.
    pub(super) view: wgpu::TextureView,
    pub(super) gui_view: wgpu::TextureView,
    pub(super) projection: Projection,
    pub(super) camera_uniform: CameraUniform,
    pub(super) camera_buffer: wgpu::Buffer,
    pub(super) camera_bind_group: wgpu::BindGroup,
    pub(super) msaa_color: wgpu::Texture,
    pub(super) msaa_view: wgpu::TextureView,
    pub(super) depth_texture: wgpu::Texture,
    pub(super) depth_view: wgpu::TextureView,
    pub(super) transparency_targets: Option<BlockModelTransparencyTargets>,
    pub(super) volume_target: Option<BlockModelVolumeTarget>,
    pub(super) viewport_rect: ViewportRect,
}

impl OffscreenTarget {
    pub(super) fn new(graphics: &Graphics<'_>, width: u32, height: u32, label: &str) -> Self {
        let size = winit::dpi::PhysicalSize::new(width.max(1), height.max(1));
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format: graphics.config.format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: graphics.config.view_formats.clone(),
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        let texture = graphics.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{label} target")),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format.add_srgb_suffix(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[config.format.remove_srgb_suffix()],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let gui_view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_viewport(size.width, size.height);
        let camera_buffer = {
            use wgpu::util::DeviceExt;
            graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label} camera")),
                contents: bytemuck::bytes_of(&camera_uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        };
        let camera_bind_group = graphics.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label} camera bind group")),
            layout: &graphics.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let (msaa_color, msaa_view) = Graphics::create_msaa_target(&graphics.device, &config, graphics.sample_count);
        let (depth_texture, depth_view) = Graphics::create_depth_target(&graphics.device, &config, graphics.sample_count);
        Self {
            size,
            projection: graphics.projection.cropped_to(size.width, size.height),
            config,
            texture,
            view,
            gui_view,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            msaa_color,
            msaa_view,
            depth_texture,
            depth_view,
            transparency_targets: None,
            volume_target: None,
            viewport_rect: ViewportRect::full(size.width, size.height),
        }
    }

    /// Trade this target's camera, targets and size with the renderer's, so
    /// the renderer's own passes draw into it. Called in pairs.
    pub(super) fn swap_into(&mut self, graphics: &mut Graphics<'_>) {
        std::mem::swap(&mut graphics.projection, &mut self.projection);
        std::mem::swap(&mut graphics.camera_uniform, &mut self.camera_uniform);
        std::mem::swap(&mut graphics.camera_buffer, &mut self.camera_buffer);
        std::mem::swap(&mut graphics.camera_bind_group, &mut self.camera_bind_group);
        std::mem::swap(&mut graphics.msaa_color, &mut self.msaa_color);
        std::mem::swap(&mut graphics.msaa_view, &mut self.msaa_view);
        std::mem::swap(&mut graphics.depth_texture, &mut self.depth_texture);
        std::mem::swap(&mut graphics.depth_view, &mut self.depth_view);
        std::mem::swap(&mut graphics.block_model_transparency_targets, &mut self.transparency_targets);
        std::mem::swap(&mut graphics.block_model_volume_target, &mut self.volume_target);
        std::mem::swap(&mut graphics.config, &mut self.config);
        std::mem::swap(&mut graphics.size, &mut self.size);
        std::mem::swap(&mut graphics.viewport_rect, &mut self.viewport_rect);
    }
}


/// A requested view as it was drawn, top row first, four bytes a pixel.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct ViewportPixels {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// How [`Graphics::render_view`] looks at the scene. The whole view is in
/// the request: nothing of it is kept between pictures.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct ViewRequest {
    pub(crate) aim: ViewAim,
    pub(crate) perspective: bool,
    /// The picture's size in pixels.
    pub(crate) size: (u32, u32),
}

/// Where a requested view looks from.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum ViewAim {
    /// Along `forward`, with `up` as near as it can be to up in the picture,
    /// framing `bounds`, or everything shown when `None`.
    Frame { forward: DVec3, up: DVec3, bounds: Option<(DVec3, DVec3)> },
    /// From the window's camera, as the user sees the scene; `perspective`
    /// is the window's too.
    Window,
    /// Square-on to a vertical section along the line `from`..`to` in plan,
    /// showing a slab `width` thick, framed on the line.
    Section { from: DVec3, to: DVec3, width: f64 },
}

#[cfg(not(target_arch = "wasm32"))]
impl Graphics<'_> {
    /// Draw the scene as `request` describes into a picture of its own and
    /// read it back. The window's camera, section and cached scene image are
    /// left as they were; its next frame redraws, since `content` may show
    /// and highlight things differently from it.
    pub(crate) fn render_view(&mut self, request: &ViewRequest, content: frame::SceneContent<'_>) -> Result<ViewportPixels, String> {
        let (width, height) = (request.size.0.max(1), request.size.1.max(1));
        let mut target = match self.view_target.take() {
            Some(target) if target.size.width == width && target.size.height == height => target,
            _ => OffscreenTarget::new(self, width, height, "Requested view"),
        };
        // The window's projection for its settings (field of view, clip
        // limits); the request says which kind and the framing sets the zoom.
        target.projection = match request.aim {
            ViewAim::Window => self.projection.cropped_to(width, height),
            ViewAim::Section { .. } => {
                let mut projection = self.projection.clone();
                projection.resize(width, height);
                projection.set_perspective(false);
                projection
            }
            ViewAim::Frame { .. } => {
                let mut projection = self.projection.clone();
                projection.resize(width, height);
                projection.set_perspective(request.perspective);
                projection
            }
        };
        let window_camera = self.camera.clone();
        // A picture is of the scene in 3D, never cut by the window's section.
        let section = self.slice_view.take();
        target.swap_into(self);
        let pixels = self.draw_requested_view(request, content, &target.view, &target.texture);
        target.swap_into(self);
        self.camera = window_camera;
        self.slice_view = section;
        self.view_target = Some(target);
        // The window's cached scene was drawn with its own visibility and
        // selection, which the shared GPU caches were just restyled away from.
        self.scene_cache_key = None;
        self.request_redraw();
        pixels.map(|rgba| ViewportPixels { rgba, width, height })
    }

    fn draw_requested_view(&mut self, request: &ViewRequest, content: frame::SceneContent<'_>, view: &wgpu::TextureView, texture: &wgpu::Texture) -> Result<Vec<u8>, String> {
        let (width, height) = (self.size.width, self.size.height);
        let frame::SceneContent {
            editor,
            document,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            ..
        } = content;
        let hidden = &editor.hidden_handles;
        if let ViewAim::Frame { forward, up, bounds } = request.aim {
            let bounds = match bounds {
                Some(bounds) => Some(bounds),
                None => {
                    self.invalidate_scene_bounds();
                    self.refresh_scene_bounds(document, triangulations, block_models, drill_holes, point_clouds, hidden);
                    self.cached_scene_bounds
                }
            };
            let (min, max) = bounds.ok_or("There is nothing shown to frame.")?;
            self.aim_camera_at(min, max, forward, up);
        }
        if let ViewAim::Section { from, to, width: slab } = request.aim {
            self.aim_at_section(from, to, slab)?;
            self.refresh_scene_bounds(document, triangulations, block_models, drill_holes, point_clouds, hidden);
            self.fit_depth_to_section();
        } else {
            self.fit_depth_to_scene(document, triangulations, block_models, drill_holes, point_clouds, hidden);
        }
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection, self.scene_origin, self.vertical_exaggeration);
        self.camera_uniform.set_section_slab(self.section_slab(), self.scene_origin);
        self.camera_uniform.update_viewport(width, height);
        self.camera_uniform.set_viewport_origin(0, 0);
        self.camera_uniform.set_interaction_quality(1.0, 1.0);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.camera_uniform));
        self.write_grid_uniforms(editor);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Requested view encoder") });
        let scale_factor = self.scale_factor() as f32;
        self.sync_scene_content(&mut encoder, content, scale_factor);
        let full = ViewportRect::full(width, height);
        self.render_scene_pass(&mut encoder, view, full, editor, triangulations, block_models, drill_holes, point_clouds, rasters, true);
        let readback = RgbaReadback::record(&self.device, &mut encoder, texture, "Requested view readback");
        self.queue.submit(std::iter::once(encoder.finish()));
        readback.map_blocking(&self.device).map_err(|error| format!("The picture could not be read back: {error:#}"))?;
        let rgba = readback.unpack().map_err(|error| format!("The picture could not be read back: {error:#}"));
        readback.buffer.unmap();
        rgba
    }

    /// Look square-on at the section along `from`..`to`, `width` thick,
    /// framing the line across the picture. Puts a section in place for the
    /// draw; `render_view` puts the window's back after.
    fn aim_at_section(&mut self, from: DVec3, to: DVec3, width: f64) -> Result<(), String> {
        let delta = (to - from).truncate();
        if delta.length_squared() <= 1.0e-12 {
            return Err("from and to are the same point seen from above: a section needs a line.".to_owned());
        }
        let screen = self.screen_size();
        let aspect = (f64::from(screen.0) / f64::from(screen.1.max(1.0))).max(1e-9);
        let half_length = delta.length() * 0.5;
        self.projection.zoom = (half_length.max(1.0) / aspect * 1.1).max(1.0e-4);
        let center = self.exaggerate_point((from + to) * 0.5);
        let slice = SliceViewState::entered(center, delta.normalize(), width.clamp(0.1, 1.0e6), 0.0, 0.0, self.camera.clone(), self.projection.zoom);
        let (forward, _, up) = slice.camera_basis();
        self.camera.look_to(slice.center, forward, up, self.projection.zoom);
        self.slice_view = Some(slice);
        Ok(())
    }

    /// The camera's depth range round the section in place, as the window
    /// sets it for one.
    fn fit_depth_to_section(&mut self) {
        let Some(slice) = self.slice_view.as_ref() else {
            return;
        };
        let forward = self.camera.forward();
        let strike = DVec3::new(slice.direction.x, slice.direction.y, 0.0);
        let origin_z = self.scene_origin.z;
        let exaggeration = self.vertical_exaggeration;
        let display_z = |z: f64| origin_z + (z - origin_z) * exaggeration;
        let bounds = self
            .cached_scene_bounds
            .map(|(min, max)| (DVec3::new(min.x, min.y, display_z(min.z)), DVec3::new(max.x, max.y, display_z(max.z))));
        let half_extent = super::slice_depth_half_extent(slice.center, strike, forward, slice.width * 0.5, bounds);
        self.projection.set_symmetric_depth_extent(half_extent);
    }

    /// Point the camera along `forward`, `up` as near as it can be to up in
    /// the picture, framing the box `min`..`max` with a little room round it.
    fn aim_camera_at(&mut self, min: DVec3, max: DVec3, forward: DVec3, up: DVec3) {
        let forward = forward.normalize_or(DVec3::NEG_Z);
        let right = forward.cross(up).normalize_or(DVec3::X);
        let up = right.cross(forward).normalize_or(DVec3::Y);
        let center = (min + max) * 0.5;
        let (mut half_w, mut half_h) = (0.0_f64, 0.0_f64);
        for i in 0..8 {
            let corner = DVec3::new(
                if (i & 1) == 0 { min.x } else { max.x },
                if (i & 2) == 0 { min.y } else { max.y },
                if (i & 4) == 0 { min.z } else { max.z },
            );
            let mut d = corner - center;
            d.z *= self.vertical_exaggeration;
            half_w = half_w.max(d.dot(right).abs());
            half_h = half_h.max(d.dot(up).abs());
        }
        let screen = self.screen_size();
        let aspect = (f64::from(screen.0) / f64::from(screen.1.max(1.0))).max(1e-9);
        // 10 % padding, as the window's framing has.
        self.projection.zoom = (half_h.max(half_w / aspect) * 1.1).max(1e-4);
        let distance = if self.projection.is_perspective() {
            // Far enough back that the box's nearest face is clear of the eye too.
            self.projection.zoom / (self.projection.perspective_fov_y() * 0.5).tan() + (max - min).length() * 0.5
        } else {
            (max - min).length().max(self.projection.zoom)
        };
        // The camera works in the stretched space the scene is drawn in.
        let center = self.exaggerate_point(center);
        self.camera.look_to(center - forward * distance, forward, up, distance);
    }
}
