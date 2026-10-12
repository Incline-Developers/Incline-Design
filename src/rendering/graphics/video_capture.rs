//! The video export's frames, rendered offscreen at the video's own size.
//!
//! Each frame draws the scene from the viewport's camera into a target of
//! exactly the requested size - framing the largest centred part of what the
//! viewport shows that has the video's shape - then paints the parts of
//! Animate the viewport draws through egui (destination labels, machine
//! icons, the flows' key) over it with an egui context of its own. The
//! window's size, shape and chrome never reach the video.
//!
//! Readback does not wait: each frame's copy is mapped in the background and
//! collected on a later frame, a few in flight at once, so the GPU and the
//! app overlap rather than taking turns.

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

use super::{readback::RgbaReadback, *};

/// Frames copied back but not yet collected before the next render waits
/// for the oldest.
const MAX_IN_FLIGHT: usize = 3;

const MAP_PENDING: u8 = 0;
const MAP_DONE: u8 = 1;
const MAP_FAILED: u8 = 2;

/// What one video frame is drawn from.
pub(crate) struct VideoFrameInput<'frame> {
    pub(crate) editor: &'frame mut EditorState,
    pub(crate) document: &'frame Document,
    pub(crate) triangulations: &'frame [OpenTriangulation],
    pub(crate) block_models: &'frame [OpenBlockModel],
    pub(crate) drill_holes: &'frame [OpenDrillHoleDataset],
    pub(crate) point_clouds: &'frame [OpenPointCloud],
    pub(crate) rasters: &'frame [OpenRasterTexture],
    pub(crate) project: &'frame crate::ui::state::UiProjectView,
    /// The video's size in pixels.
    pub(crate) size: (u32, u32),
    /// Seconds into the video, which sets how far the flows' stripes have
    /// travelled, so they move at the video's pace rather than the window's.
    pub(crate) clock_s: f64,
}

struct InFlight {
    readback: RgbaReadback,
    state: Arc<AtomicU8>,
}

pub(crate) struct VideoTarget {
    size: winit::dpi::PhysicalSize<u32>,
    config: wgpu::SurfaceConfiguration,
    texture: wgpu::Texture,
    /// The scene draws through the sRGB view, egui through the plain one,
    /// as they do on the window's surface.
    view: wgpu::TextureView,
    gui_view: wgpu::TextureView,
    projection: Projection,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    msaa_color: wgpu::Texture,
    msaa_view: wgpu::TextureView,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    transparency_targets: Option<BlockModelTransparencyTargets>,
    volume_target: Option<BlockModelVolumeTarget>,
    viewport_rect: ViewportRect,
    overlay: crate::ui::VideoOverlay,
    in_flight: VecDeque<InFlight>,
}

impl VideoTarget {
    fn new(graphics: &Graphics<'_>, width: u32, height: u32) -> Self {
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
            label: Some("Video frame target"),
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
                label: Some("Video frame camera"),
                contents: bytemuck::bytes_of(&camera_uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        };
        let camera_bind_group = graphics.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Video frame camera bind group"),
            layout: &graphics.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let (msaa_color, msaa_view) = Graphics::create_msaa_target(&graphics.device, &config, graphics.sample_count);
        let (depth_texture, depth_view) = Graphics::create_depth_target(&graphics.device, &config, graphics.sample_count);
        let overlay = crate::ui::VideoOverlay::new(&graphics.device, config.format.remove_srgb_suffix(), graphics.gui.visuals());
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
            overlay,
            in_flight: VecDeque::new(),
        }
    }

    /// Trade this target's camera, targets and size with the renderer's, so
    /// the renderer's own passes draw into it. Called in pairs.
    fn swap_into(&mut self, graphics: &mut Graphics<'_>) {
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

impl Graphics<'_> {
    /// Render one video frame and start copying it back; collect it with
    /// [`Self::take_video_frames`]. Waits only when [`MAX_IN_FLIGHT`] frames
    /// are still being copied.
    pub(crate) fn render_video_frame(&mut self, input: VideoFrameInput<'_>) {
        let VideoFrameInput {
            editor,
            document,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            project,
            size: (width, height),
            clock_s,
        } = input;
        let mut target = match self.video_target.take() {
            Some(target) if target.size.width == width && target.size.height == height => target,
            _ => VideoTarget::new(self, width, height),
        };
        if target.in_flight.len() >= MAX_IN_FLIGHT {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        }
        // The window's projection this frame - its clip planes fitted to the
        // scene - reshaped to the video.
        target.projection = self.projection.cropped_to(width, height);
        // Labels and icons as large against the frame as they are against the
        // viewport: the video is as many points tall as the viewport is.
        let window_ppp = self.gui.pixels_per_point().max(0.1);
        let viewport_points = (self.viewport_rect.height as f32 / window_ppp).max(1.0);
        let shrink = ((self.viewport_rect.width as f32 / self.viewport_rect.height.max(1) as f32) / (width as f32 / height as f32)).min(1.0);
        let pixels_per_point = height as f32 / (viewport_points * shrink);

        // The window's own projections of the flows, pins and icons; put back
        // once this frame is drawn, so the window's next frame is untouched.
        let window_flows = std::mem::take(&mut editor.animation_flows);
        let window_pins = std::mem::take(&mut editor.haul_pins);
        let window_icons = std::mem::take(&mut editor.animation_machine_icons);

        target.swap_into(self);
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection, self.scene_origin, self.vertical_exaggeration);
        self.camera_uniform.set_section_slab(self.section_slab(), self.scene_origin);
        self.camera_uniform.update_viewport(width, height);
        self.camera_uniform.set_viewport_origin(0, 0);
        self.camera_uniform.set_interaction_quality(1.0, 1.0);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.camera_uniform));

        self.project_haul_flows(editor);
        let view_proj = self.view_proj();
        editor.haul_pins = if editor.shows_haul_network() {
            document
                .haulage()
                .nodes
                .iter()
                .filter_map(|node| Some((self.world_to_window_px(&view_proj, node.pos)?, node.role?)))
                .collect()
        } else {
            Vec::new()
        };
        self.upload_flow_scene(editor, pixels_per_point, clock_s);
        let (near, icons) = self.split_machines(&editor.animation_machines, pixels_per_point);
        editor.animation_machine_icons = icons;
        if !near.is_empty() && self.machines.is_none() {
            self.machines = Some(crate::rendering::machines::MachineGpu::new(&self.device).map_err(|error| {
                crate::userspace_error!("{error:#}");
            }));
        }
        if let Some(Ok(machines)) = &mut self.machines {
            machines.place(&self.device, &self.queue, &near, self.scene_origin);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Video frame encoder"),
        });
        let full = ViewportRect::full(width, height);
        // The viewport's view, overlays included, as the image export draws it.
        self.render_scene_pass(
            &mut encoder,
            &target.view,
            full,
            editor,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            true,
        );
        self.render_editor_overlay_pass(&mut encoder, &target.view, full, editor, false);
        target.overlay.paint(
            &self.device,
            &self.queue,
            &mut encoder,
            &target.gui_view,
            [width, height],
            pixels_per_point,
            editor,
            document,
            project,
        );
        let readback = RgbaReadback::record(&self.device, &mut encoder, &target.texture, "Video frame readback");
        self.queue.submit(std::iter::once(encoder.finish()));
        target.swap_into(self);

        let state = Arc::new(AtomicU8::new(MAP_PENDING));
        let signal = Arc::clone(&state);
        readback.buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            signal.store(if result.is_ok() { MAP_DONE } else { MAP_FAILED }, Ordering::Release);
        });
        target.in_flight.push_back(InFlight { readback, state });

        editor.animation_flows = window_flows;
        editor.haul_pins = window_pins;
        editor.animation_machine_icons = window_icons;
        self.video_target = Some(target);
    }

    /// The frames whose copies have arrived, oldest first; with `wait`, every
    /// frame still in flight, waiting for them. `Err` when a copy failed.
    pub(crate) fn take_video_frames(&mut self, wait: bool) -> anyhow::Result<Vec<Vec<u8>>> {
        let Some(target) = self.video_target.as_mut() else {
            return Ok(Vec::new());
        };
        let _ = self.device.poll(if wait { wgpu::PollType::wait_indefinitely() } else { wgpu::PollType::Poll });
        let mut frames = Vec::new();
        while let Some(front) = target.in_flight.front() {
            match front.state.load(Ordering::Acquire) {
                MAP_PENDING => break,
                MAP_FAILED => {
                    target.in_flight.clear();
                    anyhow::bail!("A video frame could not be read back from the GPU");
                }
                _ => {
                    let done = target.in_flight.pop_front().expect("checked above");
                    let pixels = done.readback.unpack();
                    done.readback.buffer.unmap();
                    frames.push(pixels?);
                }
            }
        }
        Ok(frames)
    }

    /// Drop the video target and whatever it still has in flight.
    pub(crate) fn release_video_target(&mut self) {
        self.video_target = None;
    }
}
