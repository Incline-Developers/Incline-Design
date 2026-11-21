//! Native, full-scene top-down window for the vertical slice tool.

use std::sync::Arc;

use glam::DVec3;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

use super::{BlockModelTransparencyTargets, BlockModelVolumeTarget, Graphics};
use crate::{
    model::{Document, block_model::OpenBlockModel, drill_hole::OpenDrillHoleDataset, point_cloud::OpenPointCloud, raster::OpenRasterTexture, triangulation::OpenTriangulation},
    rendering::camera::{Camera, CameraUniform, Projection},
    ui::state::{EditorState, ViewportRect, fitted_slice_preview_zoom},
};

/// Fingerprint of everything a slice preview renders besides its own camera:
/// document content, selection/style state, the slice definition, and the
/// visible topology items with their display styles. Preview re-renders are
/// skipped while this key (plus the preview's own view state) is unchanged,
/// instead of performing a second full scene render per main-viewport frame.
pub(super) fn slice_preview_scene_key(
    editor: &EditorState,
    document: &Document,
    triangulations: &[OpenTriangulation],
    block_models: &[OpenBlockModel],
    drill_holes: &[OpenDrillHoleDataset],
    point_clouds: &[OpenPointCloud],
    rasters: &[OpenRasterTexture],
) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    document.revision().hash(&mut hasher);
    editor.render_style_key().hash(&mut hasher);
    editor.topology_wireframes_enabled.hash(&mut hasher);
    editor.topology_contours_enabled.hash(&mut hasher);
    editor.xray_enabled.hash(&mut hasher);
    editor.debug_surface_chunks.hash(&mut hasher);
    editor.vertical_exaggeration.to_bits().hash(&mut hasher);
    for value in editor.slice_center {
        value.to_bits().hash(&mut hasher);
    }
    for value in editor.slice_direction {
        value.to_bits().hash(&mut hasher);
    }
    editor.slice_half_length.to_bits().hash(&mut hasher);
    editor.slice_width_input.to_bits().hash(&mut hasher);
    for channel in editor.renderer_background_color {
        channel.to_bits().hash(&mut hasher);
    }
    for triangulation in triangulations {
        triangulation.id.hash(&mut hasher);
        triangulation.state.loaded.hash(&mut hasher);
        (Arc::as_ptr(&triangulation.mesh) as usize).hash(&mut hasher);
        triangulation.state.loaded.hash(&mut hasher);
        for channel in triangulation.color.iter().chain(&triangulation.line_color) {
            channel.to_bits().hash(&mut hasher);
        }
        triangulation.line_weight.map(f32::to_bits).hash(&mut hasher);
        triangulation.raster_opacity.to_bits().hash(&mut hasher);
        triangulation.raster_texture.hash(&mut hasher);
    }
    for block_model in block_models {
        block_model.id.hash(&mut hasher);
        block_model.state.loaded.hash(&mut hasher);
        (Arc::as_ptr(&block_model.blocks) as usize).hash(&mut hasher);
        block_model.state.loaded.hash(&mut hasher);
        if let Some(slice) = block_model.slice {
            for value in slice.min.to_array().into_iter().chain(slice.max.to_array()) {
                value.to_bits().hash(&mut hasher);
            }
        }
        block_model.active_color_variable.hash(&mut hasher);
        block_model.hide_empty_color_values.hash(&mut hasher);
        for channel in block_model.color {
            channel.to_bits().hash(&mut hasher);
        }
        let ramp = block_model.normalized_ramp();
        for channel in ramp.below {
            channel.to_bits().hash(&mut hasher);
        }
        for band in &ramp.bands {
            band.t.to_bits().hash(&mut hasher);
            band.inclusive.hash(&mut hasher);
            for channel in band.color {
                channel.to_bits().hash(&mut hasher);
            }
        }
    }
    for dataset in drill_holes {
        dataset.id.hash(&mut hasher);
        dataset.state.loaded.hash(&mut hasher);
        dataset.state.loaded.hash(&mut hasher);
        dataset.color.active_field.hash(&mut hasher);
        dataset.color.smooth.hash(&mut hasher);
        dataset.color.radius_scale.to_bits().hash(&mut hasher);
        dataset.color.min_pixel_diameter.to_bits().hash(&mut hasher);
        for stop in &dataset.color.stops {
            stop.t.to_bits().hash(&mut hasher);
            for value in stop.color {
                value.to_bits().hash(&mut hasher);
            }
        }
        dataset.color.categories.content_hash().hash(&mut hasher);
        dataset.color.working_sections.hash(&mut hasher);
        dataset.color.by_working_section.hash(&mut hasher);
        dataset.color.hole_style.hash(&mut hasher);
        dataset.color.disc_diameter.to_bits().hash(&mut hasher);
        dataset.color.string_pixel_width.to_bits().hash(&mut hasher);
    }
    for point_cloud in point_clouds {
        point_cloud.id.hash(&mut hasher);
        point_cloud.state.loaded.hash(&mut hasher);
        (Arc::as_ptr(&point_cloud.prepared) as usize).hash(&mut hasher);
        point_cloud.state.loaded.hash(&mut hasher);
        point_cloud.point_size.to_bits().hash(&mut hasher);
        for channel in point_cloud.color {
            channel.to_bits().hash(&mut hasher);
        }
    }
    for raster in rasters {
        raster.id.hash(&mut hasher);
        raster.state.loaded.hash(&mut hasher);
        raster.state.loaded.hash(&mut hasher);
        (Arc::as_ptr(&raster.rgba) as usize).hash(&mut hasher);
    }
    hasher.finish()
}

const OVERLAY_SHADER: &str = r#"
struct Transform {
    center: vec2<f32>,
    scale: vec2<f32>,
}
@group(0) @binding(0) var<uniform> transform: Transform;
struct In {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
}
struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}
@vertex fn vs_main(input: In) -> Out {
    var output: Out;
    output.pos = vec4<f32>((input.pos - transform.center) * transform.scale, 0.0, 1.0);
    output.color = input.color;
    return output;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> { return input.color; }
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct OverlayVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct OverlayTransform {
    center: [f32; 2],
    scale: [f32; 2],
}

/// Slice-overview strokes use the same logical-point convention as the main
/// viewport's screen-space strokes. WebGPU line primitives are only one
/// physical pixel wide, so the overview tessellates each segment into a quad.
const SLICE_OVERLAY_LINE_WIDTH_POINTS: f64 = 2.0;

pub(super) struct EmbeddedSlicePreview {
    /// Counts the colour targets made, so a popped-out preview's window
    /// knows to register the new one (`panel_window`).
    generation: u64,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    gui_view: wgpu::TextureView,
    texture_id: egui::TextureId,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    camera: Camera,
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
    overlay_pipeline: wgpu::RenderPipeline,
    overlay_transform_buffer: wgpu::Buffer,
    overlay_bind_group: wgpu::BindGroup,
    overlay_buffer: wgpu::Buffer,
    overlay_capacity: usize,
}

/// The pipeline the preview's overlay strokes (the slice's outline and
/// line) are drawn with, and its transform.
fn create_overlay_pipeline(graphics: &Graphics<'_>, format: wgpu::TextureFormat) -> (wgpu::RenderPipeline, wgpu::Buffer, wgpu::BindGroup) {
    let device = &graphics.device;
    let transform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Top-down slice overlay transform"),
        contents: bytemuck::bytes_of(&OverlayTransform {
            center: [0.0; 2],
            scale: [1.0; 2],
        }),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Top-down slice overlay layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Top-down slice overlay bind group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: transform_buffer.as_entire_binding(),
        }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Top-down slice overlay shader"),
        source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Top-down slice overlay pipeline layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Top-down slice overlay pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<OverlayVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    (pipeline, transform_buffer, bind_group)
}

impl EmbeddedSlicePreview {
    fn new(graphics: &mut Graphics<'_>, size: PhysicalSize<u32>) -> Self {
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            format: graphics.config.format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: graphics.config.view_formats.clone(),
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        let (texture, view, gui_view) = Self::create_color_target(&graphics.device, &config);
        // Shown in the window's UI, so registered with it; a renderer with no
        // window has no UI to show it in.
        let texture_id = graphics
            .gui
            .as_mut()
            .map(|gui| gui.register_native_texture(&graphics.device, &gui_view))
            .unwrap_or_default();
        let camera = Camera::new(DVec3::new(0.0, 0.0, 10.0), 0.0, 0.0);
        let projection = Projection::new(size.width, size.height, -1000.0, 1000.0);
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_viewport(size.width, size.height);
        let camera_buffer = graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Embedded top-down preview camera"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_layout = graphics.scene_pipelines.render_pipeline.get_bind_group_layout(0);
        let camera_bind_group = graphics.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Embedded top-down preview camera bind group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let (msaa_color, msaa_view) = Graphics::create_msaa_target(&graphics.device, &config, graphics.sample_count);
        let (depth_texture, depth_view) = Graphics::create_depth_target(&graphics.device, &config, graphics.sample_count);
        let transparency_targets = None;
        let volume_target = None;
        let (overlay_pipeline, overlay_transform_buffer, overlay_bind_group) = create_overlay_pipeline(graphics, config.format.add_srgb_suffix());
        let overlay_buffer = graphics.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Embedded top-down slice overlay"),
            size: size_of::<OverlayVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            generation: 0,
            texture,
            view,
            gui_view,
            texture_id,
            config,
            size,
            camera,
            projection,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            msaa_color,
            msaa_view,
            depth_texture,
            depth_view,
            transparency_targets,
            volume_target,
            overlay_pipeline,
            overlay_transform_buffer,
            overlay_bind_group,
            overlay_buffer,
            overlay_capacity: 1,
        }
    }

    fn create_color_target(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> (wgpu::Texture, wgpu::TextureView, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Embedded shaded top-down preview"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format.add_srgb_suffix(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[config.format.remove_srgb_suffix()],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // egui treats sampled image bytes as gamma encoded. Sampling through
        // the non-sRGB view prevents an unwanted hardware decode first.
        let gui_view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        (texture, view, gui_view)
    }

    fn recreate_targets(&mut self, graphics: &mut Graphics<'_>, size: PhysicalSize<u32>) {
        self.size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        self.config.width = self.size.width;
        self.config.height = self.size.height;
        self.projection.resize(self.size.width, self.size.height);
        self.camera_uniform.update_viewport(self.size.width, self.size.height);
        (self.texture, self.view, self.gui_view) = Self::create_color_target(&graphics.device, &self.config);
        self.generation += 1;
        if let Some(gui) = graphics.gui.as_mut() {
            gui.update_native_texture(&graphics.device, self.texture_id, &self.gui_view);
        }
        let (msaa_color, msaa_view) = Graphics::create_msaa_target(&graphics.device, &self.config, graphics.sample_count);
        let (depth_texture, depth_view) = Graphics::create_depth_target(&graphics.device, &self.config, graphics.sample_count);
        self.msaa_color = msaa_color;
        self.msaa_view = msaa_view;
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.transparency_targets = None;
        self.volume_target = None;
    }

    fn ensure_overlay_capacity(&mut self, device: &wgpu::Device, required: usize) {
        if required <= self.overlay_capacity {
            return;
        }
        self.overlay_capacity = required.next_power_of_two();
        self.overlay_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Embedded top-down slice overlay"),
            size: (self.overlay_capacity * size_of::<OverlayVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }
}

impl<'a> Graphics<'a> {
    /// The section preview's picture, for a window showing it, and the
    /// generation of the texture it is in.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn section_preview_view(&self) -> Option<(u64, wgpu::TextureView)> {
        self.embedded_slice_preview.as_ref().map(|preview| (preview.generation, preview.gui_view.clone()))
    }

    /// Whether the section preview's picture was drawn again since this was
    /// last asked.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn take_section_preview_rendered(&mut self) -> bool {
        std::mem::take(&mut self.section_preview_rendered)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_embedded_slice_preview(
        &mut self,
        document: &Document,
        triangulations: &[OpenTriangulation],
        block_models: &[OpenBlockModel],
        drill_holes: &[OpenDrillHoleDataset],
        point_clouds: &[OpenPointCloud],
        rasters: &[OpenRasterTexture],
        editor: &mut EditorState,
    ) {
        // Popped out too: its window shows the same picture.
        if !editor.slice_mode_enabled {
            editor.slice_preview_texture = None;
            return;
        }

        let requested = PhysicalSize::new(editor.slice_preview_size_px[0].clamp(160, 2048), editor.slice_preview_size_px[1].clamp(160, 2048));
        let mut preview = self.embedded_slice_preview.take().unwrap_or_else(|| EmbeddedSlicePreview::new(self, requested));
        let resized = preview.size != requested;
        if resized {
            preview.recreate_targets(self, requested);
        }
        editor.slice_preview_texture = Some(preview.texture_id);
        let scale_factor = self.scale_factor();

        // Skip the second full scene render while nothing it depends on has
        // changed. Interaction (camera/zoom/style drags) always refreshes,
        // covering style state the key intentionally approximates.
        let (center, zoom) = slice_preview_view(editor, requested, scale_factor);
        let key = {
            use std::hash::{DefaultHasher, Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            slice_preview_scene_key(editor, document, triangulations, block_models, drill_holes, point_clouds, rasters).hash(&mut hasher);
            (requested.width, requested.height).hash(&mut hasher);
            center.x.to_bits().hash(&mut hasher);
            center.y.to_bits().hash(&mut hasher);
            zoom.to_bits().hash(&mut hasher);
            hasher.finish()
        };
        if resized || self.interaction_active() || self.point_cloud_gpu.has_pending_uploads() || self.embedded_preview_scene_key != Some(key) {
            self.embedded_preview_scene_key = Some(key);
            self.section_preview_rendered = true;
            self.render_embedded_slice_preview_inner(
                &mut preview,
                document,
                triangulations,
                block_models,
                drill_holes,
                point_clouds,
                rasters,
                editor,
                scale_factor,
            );
        }
        self.embedded_slice_preview = Some(preview);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_embedded_slice_preview_inner(
        &mut self,
        preview: &mut EmbeddedSlicePreview,
        document: &Document,
        triangulations: &[OpenTriangulation],
        block_models: &[OpenBlockModel],
        drill_holes: &[OpenDrillHoleDataset],
        point_clouds: &[OpenPointCloud],
        rasters: &[OpenRasterTexture],
        editor: &EditorState,
        scale_factor: f64,
    ) {
        let aspect = (preview.config.width as f64 / preview.config.height.max(1) as f64).max(1e-9);
        let (center, zoom) = slice_preview_view(editor, preview.size, scale_factor);
        preview.projection.zoom = zoom;
        preview.camera.reset_to_plan_view(center, zoom);

        std::mem::swap(&mut self.camera, &mut preview.camera);
        std::mem::swap(&mut self.projection, &mut preview.projection);
        std::mem::swap(&mut self.camera_uniform, &mut preview.camera_uniform);
        std::mem::swap(&mut self.camera_buffer, &mut preview.camera_buffer);
        std::mem::swap(&mut self.camera_bind_group, &mut preview.camera_bind_group);
        std::mem::swap(&mut self.msaa_color, &mut preview.msaa_color);
        std::mem::swap(&mut self.msaa_view, &mut preview.msaa_view);
        std::mem::swap(&mut self.depth_texture, &mut preview.depth_texture);
        std::mem::swap(&mut self.depth_view, &mut preview.depth_view);
        std::mem::swap(&mut self.block_model_transparency_targets, &mut preview.transparency_targets);
        std::mem::swap(&mut self.block_model_volume_target, &mut preview.volume_target);
        std::mem::swap(&mut self.config, &mut preview.config);
        std::mem::swap(&mut self.size, &mut preview.size);

        self.fit_depth_to_scene(document, triangulations, block_models, drill_holes, point_clouds, &editor.hidden_handles);
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection, self.scene_origin, self.vertical_exaggeration);
        self.camera_uniform.update_viewport(self.size.width, self.size.height);
        self.camera_uniform.set_interaction_quality(1.0, 1.0);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.camera_uniform));

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Embedded top-down full scene encoder"),
        });
        self.render_scene_pass(
            &mut encoder,
            &preview.view,
            ViewportRect::full(self.size.width, self.size.height),
            editor,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            false,
        );

        let plan_center = [center.x - self.scene_origin.x, center.y - self.scene_origin.y];
        let transform = OverlayTransform {
            center: [plan_center[0] as f32, plan_center[1] as f32],
            scale: [(1.0 / (aspect * zoom)) as f32, (1.0 / zoom) as f32],
        };
        self.queue.write_buffer(&preview.overlay_transform_buffer, 0, bytemuck::bytes_of(&transform));
        let overlay = slice_overlay_vertices(editor, self.scene_origin, zoom, self.config.height, scale_factor);
        preview.ensure_overlay_capacity(&self.device, overlay.len());
        self.queue.write_buffer(&preview.overlay_buffer, 0, bytemuck::cast_slice(&overlay));
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Embedded top-down slice indicator pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &preview.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&preview.overlay_pipeline);
            pass.set_bind_group(0, &preview.overlay_bind_group, &[]);
            pass.set_vertex_buffer(0, preview.overlay_buffer.slice(..));
            pass.draw(0..overlay.len() as u32, 0..1);
        }

        std::mem::swap(&mut self.camera, &mut preview.camera);
        std::mem::swap(&mut self.projection, &mut preview.projection);
        std::mem::swap(&mut self.camera_uniform, &mut preview.camera_uniform);
        std::mem::swap(&mut self.camera_buffer, &mut preview.camera_buffer);
        std::mem::swap(&mut self.camera_bind_group, &mut preview.camera_bind_group);
        std::mem::swap(&mut self.msaa_color, &mut preview.msaa_color);
        std::mem::swap(&mut self.msaa_view, &mut preview.msaa_view);
        std::mem::swap(&mut self.depth_texture, &mut preview.depth_texture);
        std::mem::swap(&mut self.depth_view, &mut preview.depth_view);
        std::mem::swap(&mut self.block_model_transparency_targets, &mut preview.transparency_targets);
        std::mem::swap(&mut self.block_model_volume_target, &mut preview.volume_target);
        std::mem::swap(&mut self.config, &mut preview.config);
        std::mem::swap(&mut self.size, &mut preview.size);

        self.queue.submit([encoder.finish()]);
    }
}

/// Frame around the drawn slice at a stable world-units-per-logical-point
/// scale. Increasing either preview dimension therefore reveals more terrain
/// instead of re-fitting the same scene into a larger rectangle.
fn slice_preview_view(editor: &EditorState, size: PhysicalSize<u32>, scale_factor: f64) -> (DVec3, f64) {
    let navigation = editor.slice_preview_navigation;
    let offset = navigation.center_offset_xy();
    let fitted_zoom = fitted_slice_preview_zoom(editor.slice_half_length, size.height, scale_factor);
    let zoom = fitted_zoom * navigation.zoom_multiplier();
    (
        DVec3::new(editor.slice_center[0] + offset[0], editor.slice_center[1] + offset[1], editor.slice_center[2]),
        zoom,
    )
}

fn slice_overlay_vertices(editor: &EditorState, scene_origin: DVec3, zoom: f64, viewport_height_px: u32, scale_factor: f64) -> Vec<OverlayVertex> {
    let center = [editor.slice_center[0], editor.slice_center[1]];
    let direction = editor.slice_direction;
    let normal = [-direction[1], direction[0]];
    let half_length = editor.slice_half_length.max(1.0);
    let half_width = editor.slice_width_input.max(0.0) * 0.5;
    let a = [center[0] - direction[0] * half_length, center[1] - direction[1] * half_length];
    let b = [center[0] + direction[0] * half_length, center[1] + direction[1] * half_length];
    let offset = [normal[0] * half_width, normal[1] * half_width];
    let ao = [a[0] + offset[0], a[1] + offset[1]];
    let bo = [b[0] + offset[0], b[1] + offset[1]];
    let ai = [a[0] - offset[0], a[1] - offset[1]];
    let bi = [b[0] - offset[0], b[1] - offset[1]];
    let accent = [0.0953, 0.3662, 1.0, 1.0];
    let band = [0.0953, 0.3662, 1.0, 0.65];
    let mut vertices = Vec::with_capacity(48);
    for (p, q, color) in [(ao, bo, band), (ai, bi, band), (ao, ai, band), (bo, bi, band), (a, b, accent)] {
        push_overlay_segment(
            &mut vertices,
            p,
            q,
            color,
            scene_origin,
            zoom,
            viewport_height_px,
            scale_factor,
            SLICE_OVERLAY_LINE_WIDTH_POINTS,
        );
    }
    let arrow_length = (half_length * 0.12).clamp(1.0, half_length);
    let head = [center[0] + normal[0] * arrow_length, center[1] + normal[1] * arrow_length];
    push_overlay_segment(
        &mut vertices,
        center,
        head,
        accent,
        scene_origin,
        zoom,
        viewport_height_px,
        scale_factor,
        SLICE_OVERLAY_LINE_WIDTH_POINTS,
    );
    let wing = arrow_length * 0.22;
    let back = [head[0] - normal[0] * wing, head[1] - normal[1] * wing];
    for side in [-1.0, 1.0] {
        push_overlay_segment(
            &mut vertices,
            head,
            [back[0] + direction[0] * wing * side, back[1] + direction[1] * wing * side],
            accent,
            scene_origin,
            zoom,
            viewport_height_px,
            scale_factor,
            SLICE_OVERLAY_LINE_WIDTH_POINTS,
        );
    }
    vertices
}

#[allow(clippy::too_many_arguments)]
fn push_overlay_segment(
    vertices: &mut Vec<OverlayVertex>,
    start: [f64; 2],
    end: [f64; 2],
    color: [f32; 4],
    scene_origin: DVec3,
    zoom: f64,
    viewport_height_px: u32,
    scale_factor: f64,
    width_points: f64,
) {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    if length <= f64::EPSILON {
        return;
    }

    // In this plan-view projection, one world unit spans
    // `viewport_height_px / (2 * zoom)` physical pixels on both axes. Convert
    // the requested logical-point width back to a world-space half width so
    // zoom and display scale cannot change the visible stroke thickness.
    let viewport_height_px = f64::from(viewport_height_px.max(1));
    let half_width_world = width_points.max(0.0) * scale_factor.max(1.0e-6) * zoom.max(1.0e-9) / viewport_height_px;
    if half_width_world <= f64::EPSILON {
        return;
    }
    let offset = [-delta[1] / length * half_width_world, delta[0] / length * half_width_world];
    let start_left = [start[0] + offset[0], start[1] + offset[1]];
    let start_right = [start[0] - offset[0], start[1] - offset[1]];
    let end_left = [end[0] + offset[0], end[1] + offset[1]];
    let end_right = [end[0] - offset[0], end[1] - offset[1]];
    let vertex = |point: [f64; 2]| OverlayVertex {
        pos: [(point[0] - scene_origin.x) as f32, (point[1] - scene_origin.y) as f32],
        color,
    };
    vertices.extend_from_slice(&[
        vertex(start_left),
        vertex(start_right),
        vertex(end_right),
        vertex(start_left),
        vertex(end_right),
        vertex(end_left),
    ]);
}
