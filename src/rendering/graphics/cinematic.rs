// Continuous cinematic lighting: sun and sky, filtered shadow maps, half-resolution
// ambient occlusion and point-cloud eye-dome lighting. A single graded frame is
// cached until the scene changes. Designs and drill holes are drawn afterwards
// with their ordinary colours and depth rules. Native only.

use glam::camera as fcamera;

use super::{
    frustum::Frustum,
    scene_pipelines::{GROUND_COLOR, GROUND_INTENSITY, SCENE_EXPOSURE, SKY_COLOR, SKY_INTENSITY, SUN_COLOR, SUN_DIRECTION, SUN_INTENSITY},
    *,
};
use crate::userspace_log;

/// Side length of each (square) shadow-map cascade.
const SHADOW_MAP_DIMENSION: u32 = 2048;
/// Cascade 0 is fitted to what the camera sees, cascade 1 to the whole scene.
const SHADOW_CASCADES: u32 = 2;
/// Pad the visible footprint for nearby relief and the cascade blend band.
const SHADOW_FOCUS_PADDING: f32 = 1.4;

/// Smallest ambient-occlusion radius in metres. The shader scales the radius
/// with the view and clamps it into `[AO_MIN_RADIUS, AO_MIN_RADIUS * 30]`.
const AO_MIN_RADIUS: f32 = 0.35;

/// The format the lit scene is drawn in before tone mapping.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CinematicUniform {
    cascades: [[[f32; 4]; 4]; 2],
    cascade_texel: [f32; 4],
    sun: [f32; 4],
    sun_color: [f32; 4],
    sky_color: [f32; 4],
    ground_color: [f32; 4],
    params: [f32; 4],
    grade: [f32; 4],
}

/// Everything whose size does not depend on the window: pipelines, layouts,
/// the shadow maps and the uniforms. Built once, the
/// first time the view is turned on, so a session that never uses it pays
/// neither the shader compilation nor the shadow maps.
pub(crate) struct CinematicPipelines {
    /// The lit copy of the scene pipelines, drawing into `HDR_FORMAT`.
    scene: Arc<scene_pipelines::ScenePipelines>,
    /// Group 0 for `scene`: the camera, parameter block and shadow maps.
    scene_camera_bind_group: wgpu::BindGroup,

    params_buffer: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    /// Per cascade: the camera uniform, holding the light's matrix, that the
    /// shadow pass binds in place of the view camera.
    light_camera_buffers: [wgpu::Buffer; 2],
    light_camera_bind_groups: [wgpu::BindGroup; 2],
    shadow_keys: [Option<u64>; 2],
    light_matrices: [glam::Mat4; 2],

    depth_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,

    shadow_pipeline: wgpu::RenderPipeline,
    occlusion_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,

    _shadow_texture: wgpu::Texture,
    /// One render view per cascade; the scene bind group retains the array view.
    shadow_layer_views: [wgpu::TextureView; 2],
}

/// The screen-sized half, rebuilt whenever the surface is reconfigured. Held
/// in an `Option` and surrendered to the retirement queue on resize, exactly
/// like the block-model attachments beside it.
pub(crate) struct CinematicTargets {
    width: u32,
    height: u32,
    /// The lit scene pass's multisample attachment, and what it resolves to.
    msaa_texture: wgpu::Texture,
    msaa_view: wgpu::TextureView,
    scene_texture: wgpu::Texture,
    pub(super) scene_view: wgpu::TextureView,
    occlusion_texture: wgpu::Texture,
    occlusion_view: wgpu::TextureView,
    depth_bind_group: wgpu::BindGroup,
    composite_bind_group: wgpu::BindGroup,
}

impl CinematicTargets {
    /// Every texture this owns, for the resize retirement queue.
    pub(super) fn into_textures(self) -> Vec<wgpu::Texture> {
        vec![self.msaa_texture, self.scene_texture, self.occlusion_texture]
    }
}

/// Orthographic light matrix that sees everything inside `bounds` (model
/// space) - a box with the sun behind it - plus the model-space size of one
/// of its texels. `focus`, when given, narrows the light's footprint to a
/// sphere around what the camera sees, while the depth range still spans
/// all of `bounds` so that casters outside the view still cast into it.
fn fit_light_matrix(bounds_min: glam::Vec3, bounds_max: glam::Vec3, focus: Option<(glam::Vec3, f32)>) -> Option<(glam::Mat4, f32)> {
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
    let (mut left, mut right, mut bottom, mut top) = (minimum.x, maximum.x, minimum.y, maximum.y);
    if let Some((focus_centre, focus_radius)) = focus {
        // A sphere, not the frustum's own outline: its footprint does not
        // change as the camera turns, so the shadow edge does not crawl while
        // orbiting. The radius moves in steps and the centre snaps to whole
        // texels for the same reason under zoom and pan.
        let radius = 1.05_f32.powf(focus_radius.max(0.01).log(1.05).ceil());
        let texel = 2.0 * radius / SHADOW_MAP_DIMENSION as f32;
        let centre = view.transform_point3(focus_centre);
        let snapped = (centre.truncate() / texel).floor() * texel;
        // Keep the square's scale fixed at this zoom. Clipping it to scene
        // bounds changes texel spacing as the camera pans along an edge.
        left = snapped.x - radius;
        right = snapped.x + radius;
        bottom = snapped.y - radius;
        top = snapped.y + radius;
        if right <= left || top <= bottom {
            return None;
        }
    }
    let projection = fcamera::rh::proj::directx::orthographic(left, right, bottom, top, near, far);
    let texel_world = (right - left).max(top - bottom) / SHADOW_MAP_DIMENSION as f32;
    Some((projection * view, texel_world))
}

/// Fit receivers at the depth being inspected, not the entire camera depth
/// range. The latter can span kilometres even when a bench fills the screen.
/// Unprojecting a constant clip depth works for both camera projections and
/// includes vertical exaggeration through the actual render matrix.
fn near_cascade_focus(view_proj: glam::Mat4, focal_point: glam::Vec3) -> Option<(glam::Vec3, f32)> {
    let clip = view_proj * focal_point.extend(1.0);
    if !clip.is_finite() || clip.w <= 1.0e-6 {
        return None;
    }
    let depth = clip.z / clip.w;
    if !(0.0..=1.0).contains(&depth) {
        return None;
    }
    let inverse = view_proj.inverse();
    let unproject = |x, y| {
        let point = inverse * glam::vec4(x, y, depth, 1.0);
        (point.is_finite() && point.w.abs() > 1.0e-12).then(|| point.truncate() / point.w)
    };
    let centre = unproject(0.0, 0.0)?;
    let mut radius = 0.0_f32;
    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        radius = radius.max(unproject(x, y)?.distance(centre));
    }
    (radius.is_finite() && radius > 0.0).then_some((centre, radius * SHADOW_FOCUS_PADDING))
}

impl<'a> Graphics<'a> {
    /// Build whatever the cinematic chain is missing. Called once per frame
    /// while the view is on, and a no-op after the first.
    pub(super) fn prepare_cinematic(&mut self) {
        if self.cinematic.is_none() {
            self.cinematic = Some(self.create_cinematic_pipelines());
        }
        let width = self.config.width.max(1);
        let height = self.config.height.max(1);
        let stale = self.cinematic_targets.as_ref().is_none_or(|targets| targets.width != width || targets.height != height);
        if stale && let Some(pipelines) = self.cinematic.as_ref() {
            self.cinematic_targets = Some(Self::create_cinematic_targets(&self.device, &self.config, &self.depth_view, pipelines));
        }
    }

    fn create_cinematic_pipelines(&self) -> CinematicPipelines {
        let device = &self.device;
        let camera_layout = &self.camera_bind_group_layout;
        let scene_format = self.config.format.add_srgb_suffix();

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

        // The shadow pass binds these where the scene pass binds the view
        // camera, so each is a full `CameraUniform` even though only its
        // view-projection is ever read.
        let light_camera = |label| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size_of::<CameraUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: camera_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
                label: Some(label),
            });
            (buffer, bind_group)
        };
        let (near_light_buffer, near_light_bind_group) = light_camera("Cinematic Near Cascade Light Camera");
        let (far_light_buffer, far_light_bind_group) = light_camera("Cinematic Far Cascade Light Camera");

        // Group 0 of the lit scene pipelines: the camera exactly as the
        // ordinary pipelines see it, then what they light with.
        let mut scene_camera_entries = vec![uniform_entry(0), uniform_entry(1)];
        scene_camera_entries.extend([
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ]);
        let scene_camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &scene_camera_entries,
            label: Some("cinematic_scene_camera_bind_group_layout"),
        });
        let scene = Arc::new(scene_pipelines::create_scene_pipelines(
            device,
            &scene_pipelines::ScenePipelineLayouts {
                camera: &scene_camera_layout,
                grid: &self.scene_pipelines.grid_render_pipeline.get_bind_group_layout(1),
                surface_style: &self.surface_style_bind_group_layout,
                surface_chunk: &self.surface_chunk_bind_group_layout,
                raster_surface: &self.raster_surface_bind_group_layout,
                edge_style: &self.edge_style_bind_group_layout,
                block_model_transparency_composite: &self.block_model_transparency_composite_bind_group_layout,
                block_model_volume_upscale: &self.block_model_volume_upscale_bind_group_layout,
                drill_selection: self.drill_hole_gpu.selection_layout(),
            },
            HDR_FORMAT,
            MSAA_SAMPLE_COUNT,
            scene_pipelines::SceneShading::Cinematic,
        ));

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
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                sampled_texture_entry(0, false),
                sampled_texture_entry(1, false),
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
            ],
            label: Some("cinematic_composite_bind_group_layout"),
        });
        let occlusion_shader = init::make_cinematic_shader(device, "cinematic_ao.wgsl", include_str!("../shaders/cinematic_ao.wgsl"));
        let composite_shader = init::make_cinematic_shader(device, "cinematic_composite.wgsl", include_str!("../shaders/cinematic_composite.wgsl"));
        // The shadow pass binds a chunk offset at group 1, where the cinematic
        // prelude puts its parameters, so it takes the camera prelude alone.
        let shadow_shader = init::make_shader(device, "cinematic_shadow.wgsl", include_str!("../shaders/cinematic_shadow.wgsl"));

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cinematic Shadow Pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Cinematic Shadow Pipeline Layout"),
                bind_group_layouts: &[Some(camera_layout), Some(&self.surface_chunk_bind_group_layout)],
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
                // Slope-scaled bias beside the shader's normal offset. Batters
                // are steep enough relative to a high sun that a constant bias
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

        let fullscreen_pipeline = |label: &str,
                                   layouts: &[Option<&wgpu::BindGroupLayout>],
                                   module: &wgpu::ShaderModule,
                                   entry_point: &str,
                                   format: wgpu::TextureFormat,
                                   blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(label),
                    bind_group_layouts: layouts,
                    immediate_size: 0,
                })),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_fullscreen"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(entry_point),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
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

        let occlusion_pipeline = fullscreen_pipeline(
            "Cinematic Occlusion Pipeline",
            &[Some(camera_layout), Some(&params_layout), Some(&depth_layout)],
            &occlusion_shader,
            "fs_main",
            wgpu::TextureFormat::R8Unorm,
            None,
        );
        let composite_pipeline = fullscreen_pipeline(
            "Cinematic Composite Pipeline",
            &[Some(camera_layout), Some(&params_layout), Some(&composite_layout)],
            &composite_shader,
            "fs_main",
            scene_format,
            None,
        );

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Cinematic Shadow Maps"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_DIMENSION,
                height: SHADOW_MAP_DIMENSION,
                depth_or_array_layers: SHADOW_CASCADES,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_layer_view = |layer: u32| {
            shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Cinematic Shadow Cascade"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        };
        let shadow_layer_views = [shadow_layer_view(0), shadow_layer_view(1)];
        let shadow_array_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Cinematic Shadow Cascades"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        // Comparison sampling does the depth test in the sampler, so a
        // bilinear filter averages the *results* of four tests rather than
        // four depths - which is what makes a shadow edge soft instead of
        // stepped, and why the lit shaders' filter is a disc of these.
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

        let scene_camera_bind_group = Self::create_scene_camera_bind_group(device, &scene_camera_layout, &self.camera_buffer, &params_buffer, &shadow_array_view, &shadow_sampler);
        userspace_log!("{}", crate::i18n::tr_format!(literal = "Cinematic view shadows: %method%", method = "cascaded shadow maps"));

        CinematicPipelines {
            scene,
            scene_camera_bind_group,
            params_buffer,
            params_bind_group,
            light_camera_buffers: [near_light_buffer, far_light_buffer],
            light_camera_bind_groups: [near_light_bind_group, far_light_bind_group],
            shadow_keys: [None; 2],
            light_matrices: [glam::Mat4::IDENTITY; 2],
            depth_layout,
            composite_layout,
            shadow_pipeline,
            occlusion_pipeline,
            composite_pipeline,
            _shadow_texture: shadow_texture,
            shadow_layer_views,
        }
    }

    /// Group 0 of the lit scene pipelines.
    fn create_scene_camera_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        camera_buffer: &wgpu::Buffer,
        params_buffer: &wgpu::Buffer,
        shadow_array_view: &wgpu::TextureView,
        shadow_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: params_buffer.as_entire_binding(),
            },
        ];
        entries.extend([
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(shadow_array_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(shadow_sampler),
            },
        ]);
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &entries,
            label: Some("cinematic_scene_camera_bind_group"),
        })
    }

    fn create_cinematic_targets(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, depth_view: &wgpu::TextureView, pipelines: &CinematicPipelines) -> CinematicTargets {
        let width = config.width.max(1);
        let height = config.height.max(1);
        let screen_texture = |label, format, sample_count, divisor: u32| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: width.div_ceil(divisor),
                    height: height.div_ceil(divisor),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: if sample_count > 1 {
                    wgpu::TextureUsages::RENDER_ATTACHMENT
                } else {
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
                },
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (texture, view)
        };
        let (msaa_texture, msaa_view) = screen_texture("Cinematic Scene Multisample Target", HDR_FORMAT, MSAA_SAMPLE_COUNT, 1);
        let (scene_texture, scene_view) = screen_texture("Cinematic Scene Target", HDR_FORMAT, 1, 1);
        let (occlusion_texture, occlusion_view) = screen_texture("Cinematic Occlusion Target", wgpu::TextureFormat::R8Unorm, 1, 2);
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
            ],
            label: Some("cinematic_composite_bind_group"),
        });
        CinematicTargets {
            width,
            height,
            msaa_texture,
            msaa_view,
            scene_texture,
            scene_view,
            occlusion_texture,
            occlusion_view,
            depth_bind_group,
            composite_bind_group,
        }
    }

    /// The scene's bounds in model space: rebased onto the scene origin but
    /// not exaggerated - the space vertex positions and reconstructed depth
    /// are both in, and so the space the light has to be fitted in.
    fn cinematic_model_bounds(&self) -> Option<(glam::Vec3, glam::Vec3)> {
        let (minimum, maximum) = self.cached_scene_bounds?;
        Some(((minimum - self.scene_origin).as_vec3(), (maximum - self.scene_origin).as_vec3()))
    }

    /// Write the sun, cascades and grade into the parameter block,
    /// and the cascades' matrices into the light cameras the shadow pass
    /// binds. Returns whether the shadow maps should be drawn this frame.
    fn upload_cinematic_uniforms(&mut self, triangulations: &[OpenTriangulation], editor: &EditorState) -> bool {
        let Some(pipelines) = self.cinematic.as_ref() else {
            return false;
        };
        let bounds = self.cinematic_model_bounds();
        let far = bounds.and_then(|(minimum, maximum)| fit_light_matrix(minimum, maximum, None));
        let view_proj = glam::Mat4::from_cols_array_2d(&self.camera_uniform.view_proj);
        let inverse = view_proj.as_dmat4().inverse();
        let unproject = |depth| {
            let point = inverse * glam::dvec4(0.0, 0.0, depth, 1.0);
            point.truncate() / point.w
        };
        let ray_origin = unproject(1.0);
        let ray_direction = (unproject(0.5) - ray_origin).normalize_or_zero();
        // One existing spatial-index query, with no GPU readback. Frozen
        // surfaces still receive shadows; hidden and unloaded surfaces do not.
        let hit = if ray_origin.is_finite() && ray_direction.is_finite() && ray_direction != DVec3::ZERO {
            crate::rendering::query::SceneQuery::nearest_surface(triangulations, &editor.hidden_handles, None, ray_origin + self.scene_origin, ray_direction)
                .filter(|(_, point)| self.section_slab().is_none_or(|slab| slab.contains(*point)))
                .map(|(_, point)| (point - self.scene_origin).as_vec3())
        } else {
            None
        };
        // Looking into sky or at non-surface assets: keep detail around the
        // orbit target, transformed back from display to model coordinates.
        let target = (self.unexaggerate_point(self.camera.target()) - self.scene_origin).as_vec3();
        let near = bounds.and_then(|(minimum, maximum)| {
            let focus = near_cascade_focus(view_proj, hit.unwrap_or(target))?;
            fit_light_matrix(minimum, maximum, Some(focus))
        });
        // Without a near fit the whole-scene cascade stands in for both.
        let near = near.or(far);
        let shadow_maps = far.is_some();
        let sun = SUN_DIRECTION.normalize();
        let scale = |color: u32, intensity: f32| {
            let [r, g, b, _] = crate::rendering::color::hex_to_linear_rgba(color);
            [r * intensity, g * intensity, b * intensity, 0.0]
        };
        let [near_matrix, far_matrix] = [near, far].map(|fit| fit.map_or(glam::Mat4::IDENTITY, |(matrix, _)| matrix));

        let uniform = CinematicUniform {
            cascades: [near_matrix.to_cols_array_2d(), far_matrix.to_cols_array_2d()],
            cascade_texel: [
                near.map_or(0.0, |(_, texel)| texel),
                far.map_or(0.0, |(_, texel)| texel),
                if shadow_maps { 1.0 } else { 0.0 },
                0.0,
            ],
            sun: [sun.x, sun.y, sun.z, 0.0],
            sun_color: scale(SUN_COLOR, SUN_INTENSITY),
            sky_color: scale(SKY_COLOR, SKY_INTENSITY),
            ground_color: scale(GROUND_COLOR, GROUND_INTENSITY),
            // Occlusion radius and strength, padding, eye-dome lighting strength.
            params: [AO_MIN_RADIUS, 0.85, 0.0, 0.12],
            // Exposure and the exaggeration the post chain needs to
            // measure depth in the space the camera is in.
            // Exposure keeps flat, sunlit white ground a light grey, near
            // where the ordinary view puts it, rather than letting the tone
            // curve push it up against the white of the design strings.
            grade: [0.0, SCENE_EXPOSURE, 0.0, self.vertical_exaggeration as f32],
        };
        self.queue.write_buffer(&pipelines.params_buffer, 0, bytemuck::bytes_of(&uniform));
        if shadow_maps {
            for (buffer, matrix) in pipelines.light_camera_buffers.iter().zip([near_matrix, far_matrix]) {
                let mut light_camera = CameraUniform::new();
                light_camera.view_proj = matrix.to_cols_array_2d();
                self.queue.write_buffer(buffer, 0, bytemuck::bytes_of(&light_camera));
            }
        }
        self.cinematic.as_mut().unwrap().light_matrices = [near_matrix, far_matrix];
        shadow_maps
    }

    /// The sun's view of the scene, one depth buffer per cascade. Casters are
    /// the opaque triangulation surfaces: topography and design surfaces are
    /// what throw a shadow anyone would look for, and they are the only scene
    /// geometry that rasterises through a plain vertex buffer. Block models
    /// expand instanced cubes in their own vertex shader and volume-rendered
    /// ones are raycast, so neither casts - which is visible if you look for
    /// it, and much less visible than the pit walls not casting would be.
    fn render_cinematic_shadow_pass(&mut self, encoder: &mut wgpu::CommandEncoder, triangulations: &[OpenTriangulation], editor: &EditorState) {
        let Some(pipelines) = self.cinematic.as_ref() else {
            return;
        };
        use std::hash::{Hash, Hasher};
        let mut caster_hash = std::collections::hash_map::DefaultHasher::new();
        for triangulation in triangulations {
            if triangulation.state.loaded
                && !editor.hidden_handles.contains(&triangulation.entity_id())
                && let Some(cached) = self.triangulation_gpu.get(triangulation.id)
                && cached.color[3] >= 0.999
            {
                triangulation.id.hash(&mut caster_hash);
                triangulation.state.revision().hash(&mut caster_hash);
                cached.surface_chunks.len().hash(&mut caster_hash);
            }
        }
        self.scene_origin.to_array().map(f64::to_bits).hash(&mut caster_hash);
        let mut keys = [None; 2];
        for (cascade, (view, light_camera)) in pipelines.shadow_layer_views.iter().zip(&pipelines.light_camera_bind_groups).enumerate() {
            let mut hash = caster_hash.clone();
            pipelines.light_matrices[cascade].to_cols_array().map(f32::to_bits).hash(&mut hash);
            keys[cascade] = Some(hash.finish());
            if pipelines.shadow_keys[cascade] == keys[cascade] {
                continue;
            }
            let frustum = Frustum::from_view_proj(pipelines.light_matrices[cascade]);
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Cinematic Shadow Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        // Ordinary depth, not the scene's reversed-Z: cleared
                        // to the far plane at 1.0.
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
            shadow_pass.set_bind_group(0, light_camera, &[]);
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
                // Cull against the light, so off-camera shadow casters remain visible.
                for chunk in &cached.surface_chunks {
                    if !frustum.intersects_aabb(chunk.bounds_min, chunk.bounds_max) {
                        continue;
                    }
                    shadow_pass.set_bind_group(1, &chunk.chunk_bind_group, &[]);
                    shadow_pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                    shadow_pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    shadow_pass.draw_indexed(0..chunk.index_count, 0, 0..1);
                }
            }
        }
        self.cinematic.as_mut().unwrap().shadow_keys = keys;
    }

    /// Render a cinematic frame into the scene
    /// cache: the sun's shadows, the lit scene pass, and the post chain.
    /// Returns `false` - having drawn nothing - when the chain could not be
    /// built, and the caller should draw the ordinary view instead.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_cinematic_scene(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        editor: &EditorState,
        triangulations: &[OpenTriangulation],
        block_models: &[OpenBlockModel],
        drill_holes: &[OpenDrillHoleDataset],
        point_clouds: &[OpenPointCloud],
        rasters: &[OpenRasterTexture],
    ) -> bool {
        self.prepare_cinematic();
        if self.cinematic_targets.is_none() {
            return false;
        }
        if self.upload_cinematic_uniforms(triangulations, editor) {
            self.render_cinematic_shadow_pass(encoder, triangulations, editor);
        }
        let (Some(pipelines), Some(targets)) = (self.cinematic.as_ref(), self.cinematic_targets.as_ref()) else {
            return false;
        };
        let scene_view = targets.scene_view.clone();
        self.active_scene = Some(ActiveScene {
            pipelines: pipelines.scene.clone(),
            camera_bind_group: pipelines.scene_camera_bind_group.clone(),
            msaa_view: targets.msaa_view.clone(),
        });
        self.render_scene_pass(
            encoder,
            &scene_view,
            self.viewport_rect,
            editor,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            true,
        );
        self.active_scene = None;
        let cache_view = self.scene_cache.view.clone();
        self.render_cinematic_post(encoder, &cache_view);
        self.render_cinematic_documents(encoder, self.viewport_rect, editor, drill_holes, true);
        true
    }

    /// Half-resolution occlusion, then grading directly into the scene cache.
    fn render_cinematic_post(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        let (Some(pipelines), Some(targets)) = (self.cinematic.as_ref(), self.cinematic_targets.as_ref()) else {
            return;
        };

        let fullscreen = |encoder: &mut wgpu::CommandEncoder,
                          label: &str,
                          view: &wgpu::TextureView,
                          load: wgpu::LoadOp<wgpu::Color>,
                          pipeline: &wgpu::RenderPipeline,
                          groups: &[&wgpu::BindGroup]| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load,
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
        // Both passes replace their whole attachment.
        let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);

        fullscreen(
            encoder,
            "Cinematic Occlusion Pass",
            &targets.occlusion_view,
            clear,
            &pipelines.occlusion_pipeline,
            &[&self.camera_bind_group, &pipelines.params_bind_group, &targets.depth_bind_group],
        );
        fullscreen(
            encoder,
            "Cinematic Composite Pass",
            output,
            clear,
            &pipelines.composite_pipeline,
            &[&self.camera_bind_group, &pipelines.params_bind_group, &targets.composite_bind_group],
        );
    }
}
