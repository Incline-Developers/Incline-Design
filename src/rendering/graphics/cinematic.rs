// Cinematic view: a presentation pass over the ordinary scene render.
//
// Nothing here changes how the scene is drawn. The scene pass runs exactly as
// it always does, into an offscreen colour target instead of the scene cache,
// and this module turns that image plus the depth buffer beside it into the
// one the cache finally holds - sun shadows, ambient occlusion, bloom and a
// grade. The editor overlay pass that follows never knows the difference.
//
// The background is left alone: where the scene drew nothing, the image keeps
// the flat clear colour the ordinary renderer produces. Cinematic view changes
// how the geometry is lit, not what the window looks like around it.
//
// Two properties of the renderer are what make this affordable. The scene is
// only re-rendered when its cache key changes (see `frame::render`), so the
// whole chain costs one camera movement and nothing while the view is parked;
// and the depth buffer is already bound as a texture elsewhere, so position
// can be reconstructed per pixel without a G-buffer.
//
// Native only. The chain adds four screen-sized attachments and a 4096-square
// shadow map, which is more GPU memory than the browser build can spare.

use glam::camera as fcamera;

use super::*;

/// Direction pointing *at* the sun, in display space.
///
/// This is `key_direction` from `surface.wgsl`, and it has to stay that way:
/// the surface shader already shades as though the light came from here, so
/// shadows cast along any other direction would contradict the shading of the
/// very faces casting them.
const SUN_DIRECTION: glam::Vec3 = glam::vec3(-0.55, -0.35, 0.76);

/// Side length of the (square) shadow map. Fitted to the whole scene rather
/// than to cascades, so this is the entire budget: over a two-kilometre pit it
/// works out around half a metre per texel.
const SHADOW_MAP_DIMENSION: u32 = 4096;

/// How much smaller the bloom chain is than the screen, per axis.
const BLOOM_DIVISOR: u32 = 4;

/// Smallest ambient-occlusion radius in metres. The shader scales the radius
/// with the view and clamps it into `[AO_MIN_RADIUS, AO_MIN_RADIUS * 40]`.
const AO_MIN_RADIUS: f32 = 0.35;

/// Warm, but only just: the tint it leaves on lit ground is what separates
/// those faces from the cool shadow beside them, and any more than this starts
/// to shift the colours a surface or a grade ramp is actually encoding.
const SUN_COLOR: u32 = 0xffd9a0;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct CinematicUniform {
    light_view_proj: [[f32; 4]; 4],
    /// xyz: unit vector toward the sun. w: one shadow-map texel in metres.
    sun: [f32; 4],
    sun_color: [f32; 4],
    /// x: minimum occlusion radius (m), y: occlusion strength, z: shadow
    /// strength, w: 1 when a shadow map was rendered for this frame.
    params: [f32; 4],
    /// x: bloom intensity, y: exposure. zw: unused.
    grade: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BloomUniform {
    /// x: bright-pass threshold, y: the knee above it. zw: padding.
    threshold: [f32; 4],
}

/// Everything whose size does not depend on the window: pipelines, layouts,
/// the shadow map and the uniforms. Built once, the first time the view is
/// turned on, so a session that never uses it pays neither the shader
/// compilation nor the 64 MB shadow map.
pub(crate) struct CinematicPipelines {
    params_buffer: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    /// The camera uniform, holding the light's matrix, that the shadow pass
    /// binds in place of the view camera.
    light_camera_buffer: wgpu::Buffer,
    light_camera_bind_group: wgpu::BindGroup,
    bloom_params_buffer: wgpu::Buffer,

    depth_layout: wgpu::BindGroupLayout,
    bloom_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,

    shadow_pipeline: wgpu::RenderPipeline,
    occlusion_pipeline: wgpu::RenderPipeline,
    bloom_bright_pipeline: wgpu::RenderPipeline,
    bloom_blur_horizontal_pipeline: wgpu::RenderPipeline,
    bloom_blur_vertical_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,

    _shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    linear_sampler: wgpu::Sampler,
    shadow_sampler: wgpu::Sampler,
}

/// The screen-sized half, rebuilt whenever the surface is reconfigured. Held
/// in an `Option` and surrendered to the retirement queue on resize, exactly
/// like the block-model attachments beside it.
pub(crate) struct CinematicTargets {
    width: u32,
    height: u32,
    pub(super) scene_texture: wgpu::Texture,
    /// What the scene pass resolves into while cinematic view is on, in place
    /// of the scene cache.
    pub(super) scene_view: wgpu::TextureView,
    occlusion_texture: wgpu::Texture,
    occlusion_view: wgpu::TextureView,
    /// Ping-pong pair for the bright pass and the two blur passes.
    bloom_textures: [wgpu::Texture; 2],
    bloom_views: [wgpu::TextureView; 2],
    bloom_bind_groups: [wgpu::BindGroup; 2],
    scene_bind_group: wgpu::BindGroup,
    depth_bind_group: wgpu::BindGroup,
    composite_bind_group: wgpu::BindGroup,
}

impl CinematicTargets {
    /// Every texture this owns, for the resize retirement queue.
    pub(super) fn into_textures(self) -> Vec<wgpu::Texture> {
        let [bloom_a, bloom_b] = self.bloom_textures;
        vec![self.scene_texture, self.occlusion_texture, bloom_a, bloom_b]
    }
}

/// Orthographic light matrix fitted to `bounds`, plus the world size of one of
/// its texels.
///
/// Fitting to the scene rather than to the view frustum is a deliberate trade:
/// a frustum fit would spend far more of the map on what is actually visible,
/// but it also moves whenever the camera does, and a shadow edge that crawls
/// while you orbit is much more distracting in a design tool than a soft one.
fn fit_light_matrix(bounds_min: glam::Vec3, bounds_max: glam::Vec3) -> Option<(glam::Mat4, f32)> {
    let extent = bounds_max - bounds_min;
    if !extent.is_finite() || extent.max_element() <= 0.0 {
        return None;
    }
    let centre = (bounds_min + bounds_max) * 0.5;
    let radius = extent.length() * 0.5;
    let sun = SUN_DIRECTION.normalize();
    // `look_at_rh` degenerates when the up vector is parallel to the view
    // direction; the sun is high but never exactly overhead, so Z serves
    // except in the case where someone changes `SUN_DIRECTION`.
    let up = if sun.z.abs() > 0.99 { glam::Vec3::Y } else { glam::Vec3::Z };
    let view = fcamera::rh::view::look_at_mat4(centre + sun * (radius + 1.0), centre, up);

    // Bound the scene in the light's own space rather than assuming the
    // bounding sphere: a pit is far wider than it is deep, and a sphere fit
    // would waste most of the map on empty air above and below it.
    let mut minimum = glam::Vec3::splat(f32::INFINITY);
    let mut maximum = glam::Vec3::splat(f32::NEG_INFINITY);
    for index in 0..8 {
        let corner = glam::vec3(
            if index & 1 == 0 { bounds_min.x } else { bounds_max.x },
            if index & 2 == 0 { bounds_min.y } else { bounds_max.y },
            if index & 4 == 0 { bounds_min.z } else { bounds_max.z },
        );
        let light_space = view.transform_point3(corner);
        minimum = minimum.min(light_space);
        maximum = maximum.max(light_space);
    }
    // A right-handed view looks down -Z, so the near plane is the *greatest*
    // light-space Z. Both are padded so a caster sitting exactly on the bound
    // is not clipped out of the map it is supposed to cast into.
    let near = (-maximum.z - 1.0).max(0.01);
    let far = -minimum.z + 1.0;
    // A scene flat enough that the light's near and far planes coincide has
    // nothing to cast into.
    if far <= near || !far.is_finite() {
        return None;
    }
    let projection = fcamera::rh::proj::directx::orthographic(minimum.x, maximum.x, minimum.y, maximum.y, near, far);
    let texel_world = (maximum.x - minimum.x).max(maximum.y - minimum.y) / SHADOW_MAP_DIMENSION as f32;
    Some((projection * view, texel_world))
}

impl<'a> Graphics<'a> {
    /// Build whatever the cinematic chain is missing. Called once per frame
    /// while the view is on, and a no-op after the first.
    pub(super) fn prepare_cinematic(&mut self) {
        if self.cinematic.is_none() {
            self.cinematic = Some(Self::create_cinematic_pipelines(
                &self.device,
                &self.camera_bind_group_layout,
                &self.surface_chunk_bind_group_layout,
                self.config.format,
            ));
        }
        let width = self.config.width.max(1);
        let height = self.config.height.max(1);
        let stale = self.cinematic_targets.as_ref().is_none_or(|targets| targets.width != width || targets.height != height);
        if stale && let Some(pipelines) = self.cinematic.as_ref() {
            self.cinematic_targets = Some(Self::create_cinematic_targets(&self.device, &self.config, &self.depth_view, pipelines));
        }
    }

    /// The target the scene pass should resolve into this frame, or `None`
    /// when the chain is not ready and the caller should render as usual.
    pub(super) fn cinematic_scene_view(&self) -> Option<wgpu::TextureView> {
        self.cinematic_targets.as_ref().map(|targets| targets.scene_view.clone())
    }

    fn create_cinematic_pipelines(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        chunk_layout: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> CinematicPipelines {
        let scene_format = surface_format.add_srgb_suffix();

        let uniform_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[uniform_entry(0)],
            label: Some("cinematic_params_bind_group_layout"),
        });
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cinematic Params"),
            size: size_of::<CinematicUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
            label: Some("cinematic_params_bind_group"),
        });

        // The shadow pass binds this where the scene pass binds the view
        // camera, so it is a full `CameraUniform` even though only its
        // view-projection is ever read.
        let light_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cinematic Light Camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_camera_buffer.as_entire_binding(),
            }],
            label: Some("cinematic_light_camera_bind_group"),
        });

        let depth_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: true,
                },
                count: None,
            }],
            label: Some("cinematic_depth_bind_group_layout"),
        });

        let sampled_texture_entry = |binding: u32, filterable: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let bloom_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                sampled_texture_entry(0, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                uniform_entry(2),
            ],
            label: Some("cinematic_bloom_bind_group_layout"),
        });
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                sampled_texture_entry(0, false),
                sampled_texture_entry(1, false),
                sampled_texture_entry(2, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: true,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
            label: Some("cinematic_composite_bind_group_layout"),
        });

        let occlusion_shader = init::make_cinematic_shader(device, "cinematic_ao.wgsl", include_str!("../shaders/cinematic_ao.wgsl"));
        let composite_shader = init::make_cinematic_shader(device, "cinematic_composite.wgsl", include_str!("../shaders/cinematic_composite.wgsl"));
        // The shadow pass binds a chunk offset at group 1, where the cinematic
        // prelude puts its parameters, so it takes the camera prelude alone.
        let shadow_shader = init::make_shader(device, "cinematic_shadow.wgsl", include_str!("../shaders/cinematic_shadow.wgsl"));
        let bloom_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/cinematic_bloom.wgsl"));

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cinematic Shadow Pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Cinematic Shadow Pipeline Layout"),
                bind_group_layouts: &[Some(camera_layout), Some(chunk_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<SurfaceVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                })],
                compilation_options: Default::default(),
            },
            // Depth-only: there is no colour attachment to write.
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                // Triangulations are open, two-sided surfaces - a topography
                // has no inside - so back-face culling would punch holes in
                // the shadow rather than halving its cost.
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            // The shadow map is its own depth space, with no reason to follow
            // the scene's reversed-Z: an orthographic projection spreads
            // precision evenly, so the ordinary near-is-zero convention holds.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                // Slope-scaled bias in place of a shader-side one. Batters are
                // steep enough relative to a high sun that a constant bias
                // large enough to stop acne there would detach the shadow from
                // its caster everywhere flat.
                bias: wgpu::DepthBiasState {
                    constant: 4,
                    slope_scale: 2.5,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let occlusion_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cinematic Occlusion Pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Cinematic Occlusion Pipeline Layout"),
                bind_group_layouts: &[Some(camera_layout), Some(&params_layout), Some(&depth_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &occlusion_shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &occlusion_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let bloom_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cinematic Bloom Pipeline Layout"),
            bind_group_layouts: &[Some(&bloom_layout)],
            immediate_size: 0,
        });
        let bloom_pipeline = |label: &str, entry_point: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&bloom_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &bloom_shader,
                    entry_point: Some("vs_fullscreen"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &bloom_shader,
                    entry_point: Some(entry_point),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let bloom_bright_pipeline = bloom_pipeline("Cinematic Bloom Bright Pipeline", "fs_bright");
        let bloom_blur_horizontal_pipeline = bloom_pipeline("Cinematic Bloom Blur H Pipeline", "fs_blur_horizontal");
        let bloom_blur_vertical_pipeline = bloom_pipeline("Cinematic Bloom Blur V Pipeline", "fs_blur_vertical");

        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cinematic Composite Pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Cinematic Composite Pipeline Layout"),
                bind_group_layouts: &[Some(camera_layout), Some(&params_layout), Some(&composite_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &composite_shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &composite_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: scene_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Cinematic Shadow Map"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_DIMENSION,
                height: SHADOW_MAP_DIMENSION,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cinematic Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Comparison sampling does the depth test in the sampler, so a
        // bilinear filter averages the *results* of four tests rather than
        // four depths - which is what makes a shadow edge soft instead of
        // stepped, and why the filter below is a 3x3 of these.
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cinematic Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let bloom_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cinematic Bloom Params"),
            size: size_of::<BloomUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        CinematicPipelines {
            params_buffer,
            params_bind_group,
            light_camera_buffer,
            light_camera_bind_group,
            bloom_params_buffer,
            depth_layout,
            bloom_layout,
            composite_layout,
            shadow_pipeline,
            occlusion_pipeline,
            bloom_bright_pipeline,
            bloom_blur_horizontal_pipeline,
            bloom_blur_vertical_pipeline,
            composite_pipeline,
            _shadow_texture: shadow_texture,
            shadow_view,
            linear_sampler,
            shadow_sampler,
        }
    }
}

impl<'a> Graphics<'a> {
    fn create_cinematic_targets(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, depth_view: &wgpu::TextureView, pipelines: &CinematicPipelines) -> CinematicTargets {
        let width = config.width.max(1);
        let height = config.height.max(1);
        let scene_format = config.format.add_srgb_suffix();
        // Reading the scene back through an sRGB view is what lets the whole
        // chain work in linear light: the hardware decodes on the way in and
        // re-encodes on the way out, so the grade is applied to radiance
        // rather than to gamma-encoded bytes.
        let view_formats = (scene_format != config.format).then_some(scene_format).into_iter().collect::<Vec<_>>();
        let scene_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Cinematic Scene Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &view_formats,
        });
        let scene_view = scene_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Cinematic Scene Target View"),
            format: Some(scene_format),
            ..Default::default()
        });

        let occlusion_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Cinematic Occlusion Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let occlusion_view = occlusion_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bloom_texture = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: width.div_ceil(BLOOM_DIVISOR),
                    height: height.div_ceil(BLOOM_DIVISOR),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Half floats, not the surface format: the bright pass keeps
                // values above 1.0 so a blurred highlight still reads as a
                // highlight instead of clipping the moment it is isolated.
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let bloom_textures = [bloom_texture("Cinematic Bloom A"), bloom_texture("Cinematic Bloom B")];
        let bloom_views = [
            bloom_textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            bloom_textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        let sampled_bind_group = |label, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &pipelines.bloom_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&pipelines.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: pipelines.bloom_params_buffer.as_entire_binding(),
                    },
                ],
                label: Some(label),
            })
        };
        let scene_bind_group = sampled_bind_group("cinematic_scene_bind_group", &scene_view);
        let bloom_bind_groups = [
            sampled_bind_group("cinematic_bloom_a_bind_group", &bloom_views[0]),
            sampled_bind_group("cinematic_bloom_b_bind_group", &bloom_views[1]),
        ];

        let depth_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &pipelines.depth_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth_view),
            }],
            label: Some("cinematic_depth_bind_group"),
        });

        let composite_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &pipelines.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&occlusion_view),
                },
                // The blur chain finishes back in A, so that is what the
                // composite reads - see `render_cinematic_post`.
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&bloom_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&pipelines.linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&pipelines.shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&pipelines.shadow_sampler),
                },
            ],
            label: Some("cinematic_composite_bind_group"),
        });

        CinematicTargets {
            width,
            height,
            scene_texture,
            scene_view,
            occlusion_texture,
            occlusion_view,
            bloom_textures,
            bloom_views,
            bloom_bind_groups,
            scene_bind_group,
            depth_bind_group,
            composite_bind_group,
        }
    }

    /// The scene's bounds in display space: rebased onto the scene origin and
    /// stretched by the vertical exaggeration, which is the space the depth
    /// buffer reconstructs into and so the space the light has to be fitted in.
    fn cinematic_display_bounds(&self) -> Option<(glam::Vec3, glam::Vec3)> {
        let (minimum, maximum) = self.cached_scene_bounds?;
        let origin = self.scene_origin;
        let exaggeration = self.vertical_exaggeration;
        let to_display = |point: DVec3| glam::vec3((point.x - origin.x) as f32, (point.y - origin.y) as f32, ((point.z - origin.z) * exaggeration) as f32);
        Some((to_display(minimum), to_display(maximum)))
    }

    /// Write this frame's sun and grade into the uniform the occlusion pass
    /// and the composite read. Returns the fitted light matrix, or `None` when
    /// there is nothing in the scene to cast a shadow.
    pub(super) fn upload_cinematic_uniforms(&self) -> Option<glam::Mat4> {
        let pipelines = self.cinematic.as_ref()?;
        let fit = self.cinematic_display_bounds().and_then(|(minimum, maximum)| fit_light_matrix(minimum, maximum));
        let (light_view_proj, texel_world) = fit.unwrap_or((glam::Mat4::IDENTITY, 0.0));
        let sun = SUN_DIRECTION.normalize();

        let uniform = CinematicUniform {
            light_view_proj: light_view_proj.to_cols_array_2d(),
            sun: [sun.x, sun.y, sun.z, texel_world],
            sun_color: crate::rendering::color::hex_to_linear_rgba(SUN_COLOR),
            params: [AO_MIN_RADIUS, 0.90, 1.00, if fit.is_some() { 1.0 } else { 0.0 }],
            grade: [0.45, 0.88, 0.0, 0.0],
        };
        self.queue.write_buffer(&pipelines.params_buffer, 0, bytemuck::bytes_of(&uniform));
        self.queue.write_buffer(
            &pipelines.bloom_params_buffer,
            0,
            bytemuck::bytes_of(&BloomUniform {
                threshold: [0.62, 0.25, 0.0, 0.0],
            }),
        );

        if let Some((matrix, _)) = fit {
            let mut light_camera = CameraUniform::new();
            light_camera.view_proj = matrix.to_cols_array_2d();
            self.queue.write_buffer(&pipelines.light_camera_buffer, 0, bytemuck::bytes_of(&light_camera));
        }
        fit.map(|(matrix, _)| matrix)
    }

    /// The sun's view of the scene, as a depth buffer. Casters are the opaque
    /// triangulation surfaces: topography and design surfaces are what throw a
    /// shadow anyone would look for, and they are the only scene geometry that
    /// rasterises through a plain vertex buffer. Block models expand instanced
    /// cubes in their own vertex shader and volume-rendered ones are raycast,
    /// so neither casts - which is visible if you look for it, and much less
    /// visible than the pit walls not casting would be.
    pub(super) fn render_cinematic_shadow_pass(&self, encoder: &mut wgpu::CommandEncoder, triangulations: &[OpenTriangulation], editor: &EditorState) {
        let Some(pipelines) = self.cinematic.as_ref() else {
            return;
        };
        let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Cinematic Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &pipelines.shadow_view,
                depth_ops: Some(wgpu::Operations {
                    // Ordinary depth, not the scene's reversed-Z: cleared to
                    // the far plane at 1.0.
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        shadow_pass.set_pipeline(&pipelines.shadow_pipeline);
        shadow_pass.set_bind_group(0, &pipelines.light_camera_bind_group, &[]);
        for triangulation in triangulations {
            let entity = triangulation.entity_id();
            if !triangulation.state.loaded || editor.hidden_handles.contains(&entity) {
                continue;
            }
            let Some(cached) = self.triangulation_gpu.get(triangulation.id) else {
                continue;
            };
            // A surface you can see through should not lay down an opaque
            // shadow, and this pass has no way to lay down any other kind.
            if cached.color[3] < 0.999 {
                continue;
            }
            // No frustum culling here: the light sees the whole scene, and
            // geometry behind the camera casts into what is in front of it.
            for chunk in &cached.surface_chunks {
                shadow_pass.set_bind_group(1, &chunk.chunk_bind_group, &[]);
                shadow_pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                shadow_pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                shadow_pass.draw_indexed(0..chunk.index_count, 0, 0..1);
            }
        }
    }

    /// Occlusion, bloom and the composite that writes the finished image into
    /// `output` - the scene cache, from which the editor overlay pass carries
    /// on exactly as it does without any of this.
    pub(super) fn render_cinematic_post(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        let (Some(pipelines), Some(targets)) = (self.cinematic.as_ref(), self.cinematic_targets.as_ref()) else {
            return;
        };

        let fullscreen = |encoder: &mut wgpu::CommandEncoder, label: &str, view: &wgpu::TextureView, pipeline: &wgpu::RenderPipeline, groups: &[&wgpu::BindGroup]| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        // Every one of these writes its whole attachment, so
                        // there is nothing to preserve and the clear is free.
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            for (index, group) in groups.iter().enumerate() {
                pass.set_bind_group(index as u32, *group, &[]);
            }
            pass.draw(0..3, 0..1);
        };

        fullscreen(
            encoder,
            "Cinematic Occlusion Pass",
            &targets.occlusion_view,
            &pipelines.occlusion_pipeline,
            &[&self.camera_bind_group, &pipelines.params_bind_group, &targets.depth_bind_group],
        );
        fullscreen(
            encoder,
            "Cinematic Bloom Bright Pass",
            &targets.bloom_views[0],
            &pipelines.bloom_bright_pipeline,
            &[&targets.scene_bind_group],
        );
        fullscreen(
            encoder,
            "Cinematic Bloom Blur H Pass",
            &targets.bloom_views[1],
            &pipelines.bloom_blur_horizontal_pipeline,
            &[&targets.bloom_bind_groups[0]],
        );
        // Back into A, which is what the composite bind group reads.
        fullscreen(
            encoder,
            "Cinematic Bloom Blur V Pass",
            &targets.bloom_views[0],
            &pipelines.bloom_blur_vertical_pipeline,
            &[&targets.bloom_bind_groups[1]],
        );
        fullscreen(
            encoder,
            "Cinematic Composite Pass",
            output,
            &pipelines.composite_pipeline,
            &[&self.camera_bind_group, &pipelines.params_bind_group, &targets.composite_bind_group],
        );
    }
}
