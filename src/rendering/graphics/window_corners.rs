//! Rounded corners for a window with no system frame to round them.
//!
//! On Windows the compositor rounds the window and on macOS the window is the
//! system's; a Linux compositor rounds only the frames it draws itself, and
//! Incline's window has none (`app/window_frame.rs`). So there the window is
//! opened transparent, its surface is blended by its alpha, and the frame's
//! last draw cuts the corners away (`shaders/window_corners.wgsl`).
//!
//! The draw also makes every pixel inside the window opaque, so no pass
//! before it has to keep its alpha at one for the window to stay solid.
//!
//! The radius comes from `app/window_frame.rs`, which sizes it to match the
//! desktop's own windows; the UI strokes the window's border along the same
//! arc (`ui/elements/title_bar.rs::draw_window_border`).

pub(crate) struct WindowCorners {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    /// In physical pixels. Zero, square, while the window is maximised or
    /// fullscreen, as any window's corners are once they meet the screen's.
    radius: f32,
}

impl WindowCorners {
    /// Whether this surface can show the corners cut: only a transparent
    /// Wayland window, on a surface that composites premultiplied alpha.
    /// Elsewhere the corners are the system's business, or cannot be shown.
    pub(super) fn alpha_mode(window: &winit::window::Window, capabilities: &wgpu::SurfaceCapabilities) -> Option<wgpu::CompositeAlphaMode> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let wayland = matches!(window.window_handle().map(|handle| handle.as_raw()), Ok(RawWindowHandle::Wayland(_)));
        let mode = wgpu::CompositeAlphaMode::PreMultiplied;
        (wayland && capabilities.alpha_modes.contains(&mode)).then_some(mode)
    }

    /// `format` is the view the UI is drawn through.
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Window Corners Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Window Corners Uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Window Corners Bind Group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/window_corners.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Window Corners Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Window Corners Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Colour scaled by coverage, alpha replaced by it: see the shader.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::SrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            bind_group,
            uniform,
            radius: 0.0,
        }
    }

    /// The corners' radius in physical pixels; zero leaves them square.
    pub(super) fn set_radius(&mut self, radius: f32) {
        self.radius = radius;
    }

    /// Cut the corners of `view`, `size` physical pixels.
    pub(super) fn draw(&self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size: [u32; 2]) {
        let values = [size[0] as f32, size[1] as f32, self.radius, 0.0];
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&values));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Window Corners Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
