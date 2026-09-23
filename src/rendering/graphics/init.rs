use super::*;
use crate::{i18n::tr_format, userspace_log};

/// Compiles a shader whose body is prefixed with the shared camera prelude `camera_common.wgsl`, so the camera struct, its binding, and the section-slab helpers exist once.
/// `label` carries the module's own path, matching what `wgpu::include_wgsl!` would have labelled it.
pub(super) fn make_shader(device: &wgpu::Device, label: &str, body: &'static str) -> wgpu::ShaderModule {
    let source = format!("{}{body}", include_str!("../shaders/camera_common.wgsl"));
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(source)),
    })
}

/// As [`make_shader`], plus the cinematic preludes `cinematic_params.wgsl` and
/// `cinematic_common.wgsl`: the post chain's parameter block, the fullscreen
/// vertex stage and screen-to-world helpers, plus shared material grading.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn make_cinematic_shader(device: &wgpu::Device, label: &str, body: &'static str) -> wgpu::ShaderModule {
    let source = format!(
        "{}{}{}{}{body}",
        include_str!("../shaders/camera_common.wgsl"),
        include_str!("../shaders/cinematic_params.wgsl"),
        include_str!("../shaders/cinematic_common.wgsl"),
        include_str!("../shaders/scene_lighting_common.wgsl")
    );
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(source)),
    })
}

impl<'a> Graphics<'a> {
    pub(crate) async fn new(window: Arc<Window>) -> Result<Graphics<'a>> {
        let window_size = window.inner_size();
        // The web backend's ResizeObserver may not have delivered its first
        // event when asynchronous WebGPU initialization begins. Read the
        // attached canvas directly so the first surface is not permanently
        // configured as the 1x1 placeholder below.
        #[cfg(target_arch = "wasm32")]
        let window_size = {
            use winit::platform::web::WindowExtWebSys;

            window.canvas().map_or(window_size, |canvas| {
                let scale_factor = window.scale_factor();
                winit::dpi::PhysicalSize::new(
                    (f64::from(canvas.client_width().max(1)) * scale_factor).round() as u32,
                    (f64::from(canvas.client_height().max(1)) * scale_factor).round() as u32,
                )
            })
        };
        // Minimized/hidden Wayland windows may initially report 0×0. wgpu
        // surfaces, projection aspect ratios and attachments all require a
        // non-zero placeholder until the first real resize arrives.
        let size = winit::dpi::PhysicalSize::new(window_size.width.max(1), window_size.height.max(1));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| anyhow!("No compatible GPU adapter found: {e:?}"))?;

        let adapter_info = adapter.get_info();
        userspace_log!(
            "{}",
            tr_format!(
                literal = "GPU adapter: %vendor% / %name% / %backend% / %device_type%",
                vendor = adapter_info.vendor,
                name = &adapter_info.name,
                backend = format!("{:?}", adapter_info.backend),
                device_type = format!("{:?}", adapter_info.device_type)
            )
        );
        userspace_log!(
            "{}",
            tr_format!(
                literal = "GPU driver: %driver% %driver_info%",
                driver = &adapter_info.driver,
                driver_info = &adapter_info.driver_info
            )
        );

        let adapter_limits = adapter.limits();
        // Take everything the adapter offers for buffer size: large surfaces
        // can tessellate to multi-GiB vertex streams.
        let required_limits = wgpu::Limits {
            max_buffer_size: adapter_limits.max_buffer_size,
            // Take the adapter's real storage-buffer binding size instead of
            // wgpu's conservative 128 MiB default. The block-model volume
            // raycaster binds dense per-brick tables and a cell pool as single
            // storage buffers; at 128 MiB a large translucent model overflows
            // them and silently falls back to the far slower instanced-cube
            // path. The per-stage storage-buffer *count* limit is untouched
            // (the volume bind group already sits at the default 8).
            max_storage_buffer_binding_size: adapter_limits.max_storage_buffer_binding_size,
            // Full-resolution raster previews may opt into the adapter's
            // larger texture limit instead of wgpu's conservative default.
            max_texture_dimension_2d: adapter_limits.max_texture_dimension_2d,
            ..wgpu::Limits::default()
        };
        userspace_log!(
            "{}",
            tr_format!(
                literal = "GPU limits: max_buffer_size=%max_buffer_size% MiB, max_storage_buffer_binding_size=%max_storage_buffer_binding_size% MiB, max_texture_dimension_2d=%max_texture_dimension_2d%, max_bind_groups=%max_bind_groups%",
                max_buffer_size = required_limits.max_buffer_size / (1024 * 1024),
                max_storage_buffer_binding_size = required_limits.max_storage_buffer_binding_size / (1024 * 1024),
                max_texture_dimension_2d = adapter_limits.max_texture_dimension_2d,
                max_bind_groups = adapter_limits.max_bind_groups
            )
        );
        if required_limits.max_buffer_size < COMFORTABLE_MAX_BUFFER_SIZE {
            crate::userspace_warn!(
                "{}",
                tr_format!(
                    literal = "GPU supports a maximum buffer size of %size% MiB; large scenes may not display fully",
                    size = required_limits.max_buffer_size / (1024 * 1024)
                )
            );
        }

        // Fragment barycentrics let a surface draw its own wireframe instead of
        // six instanced vertices per edge. Optional: the surface shader falls
        // back to instanced edges where it is missing (always, on WebGPU).
        let required_features = adapter.features() & wgpu::Features::SHADER_BARYCENTRICS;
        let experimental_features = wgpu::ExperimentalFeatures::disabled();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features,
                required_limits,
                label: None,
                experimental_features,
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        // wgpu treats uncaptured errors as fatal panics by default; a
        // validation failure (e.g. an oversized allocation) should degrade to
        // missing geometry, not lose the user's session.
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| {
            crate::userspace_error!("{}", tr_format!(literal = "wgpu error (continuing): %error%", error = error));
        }));
        #[cfg(target_arch = "wasm32")]
        device.set_device_lost_callback(|reason, message| {
            if reason != wgpu::DeviceLostReason::Destroyed {
                let message = crate::i18n::tr!("browser-graphics-device-lost", message = message);
                crate::userspace_error!("{message}");
                crate::show_web_startup_error(&message);
            }
        });

        let surface_caps = surface.get_capabilities(&adapter);
        // Browser WebGPU surfaces expose only the base `*Unorm` canvas
        // formats, while native backends commonly expose their `*UnormSrgb`
        // counterparts directly. The configured format must come from the
        // capability list, but a compatible view may differ in sRGB-ness.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|format| matches!(*format, wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb))
            .or_else(|| {
                surface_caps
                    .formats
                    .iter()
                    .copied()
                    .find(|format| matches!(*format, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm))
            })
            .ok_or_else(|| anyhow!("Surface reports no supported 8-bit sRGB-compatible format"))?;
        let present_mode = surface_caps
            .present_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::PresentMode::Fifo)
            .or_else(|| surface_caps.present_modes.first().copied())
            .ok_or_else(|| anyhow!("Surface reports no supported present modes"))?;
        // What "vsync off" means on this adapter. Mailbox renders freely and
        // presents the newest frame each refresh, so it drops the wait without
        // tearing; Immediate is the tearing fallback. Neither is guaranteed -
        // a browser surface offers only Fifo - and where there is none the
        // preference is not offered at all (`supports_vsync_off`).
        let no_vsync_present_mode = [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
            .into_iter()
            .find(|mode| surface_caps.present_modes.contains(mode));
        userspace_log!("{}", tr_format!(literal = "Surface presentation mode: %mode%", mode = format!("{present_mode:?}")));
        let alpha_mode = surface_caps
            .alpha_modes
            .first()
            .copied()
            .ok_or_else(|| anyhow!("Surface reports no supported alpha modes"))?;
        // Scene shaders output linear colour and therefore render through an
        // sRGB view. egui applies gamma itself and renders through the base
        // unorm view of the same surface texture.
        let scene_format = surface_format.add_srgb_suffix();
        let gui_format = surface_format.remove_srgb_suffix();
        let view_formats = vec![if surface_format.is_srgb() { gui_format } else { scene_format }];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode,
            alpha_mode,
            view_formats,
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        let sample_count = MSAA_SAMPLE_COUNT;
        let (msaa_color, msaa_view) = Self::create_msaa_target(&device, &config, sample_count);
        let (scene_cache_blit_layout, scene_cache_blit_pipeline) = Self::create_scene_cache_blit(&device, scene_format, sample_count);
        let scene_cache = Self::create_scene_cache_target(&device, &config, &scene_cache_blit_layout);
        let (depth_texture, depth_view) = Self::create_depth_target(&device, &config, sample_count);

        surface.configure(&device, &config);

        let block_model_volume_shader = make_shader(&device, "../shaders/block_model_volume.wgsl", include_str!("../shaders/block_model_volume.wgsl"));
        let block_model_transparency_fallback_shader = make_shader(
            &device,
            "../shaders/block_model_transparency_fallback.wgsl",
            include_str!("../shaders/block_model_transparency_fallback.wgsl"),
        );

        let camera = Camera::new(DVec3::new(0.0, 0.0, 10.0), (-90.0_f64).to_radians(), 0.0);
        let projection = Projection::new(config.width, config.height, INITIAL_CAMERA_Z_NEAR, INITIAL_CAMERA_Z_FAR);
        let camera_controller = CameraController::new(0.6, 0.005, CAMERA_ROTATE_SENSITIVITY);
        let fly_camera_controller = FlyCameraController::new(232., CAMERA_ROTATE_SENSITIVITY);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera, &projection, DVec3::ZERO, 1.0);
        camera_uniform.update_viewport(config.width, config.height);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("camera_bind_group_layout"),
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let grid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("XY Grid Uniform Buffer"),
            contents: bytemuck::bytes_of(&<GridUniform as bytemuck::Zeroable>::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let grid_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("XY Grid Bind Group Layout"),
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
        let grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("XY Grid Bind Group"),
            layout: &grid_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: grid_buffer.as_entire_binding(),
            }],
        });

        let section_grid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Section Grid Uniform Buffer"),
            contents: bytemuck::bytes_of(&<SectionGridUniform as bytemuck::Zeroable>::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let section_grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Section Grid Bind Group"),
            layout: &grid_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: section_grid_buffer.as_entire_binding(),
            }],
        });

        let style_bind_group_layout_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let surface_style_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[style_bind_group_layout_entry],
            label: Some("surface_style_bind_group_layout"),
        });
        // Per-chunk rebase offset for triangulation surfaces (group 2); block
        // model pipelines keep the plain two-group surface layout above.
        let surface_chunk_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: Some("surface_chunk_bind_group_layout"),
        });
        let raster_surface_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raster_surface_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let block_model_transparency_fallback_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: Some("block_model_transparency_fallback_bind_group_layout"),
        });
        let block_model_transparency_fallback_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Block Model Transparency Fallback Pipeline Layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&surface_style_bind_group_layout),
                Some(&block_model_transparency_fallback_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let block_model_transparency_composite_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
            label: Some("block_model_transparency_composite_bind_group_layout"),
        });
        let block_model_volume_upscale_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("block_model_volume_upscale_bind_group_layout"),
        });
        let block_model_volume_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("block_model_volume_bind_group_layout"),
        });
        // Beam pre-pass output, read by the main raycast (group 3):
        // R32Float, textureLoad only, so unfilterable.
        let block_model_beam_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
            label: Some("block_model_beam_bind_group_layout"),
        });
        let block_model_volume_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Block Model Volume Pipeline Layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&block_model_volume_bind_group_layout),
                Some(&block_model_transparency_fallback_bind_group_layout),
                Some(&block_model_beam_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let block_model_beam_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Block Model Beam Pipeline Layout"),
            bind_group_layouts: &[Some(&camera_bind_group_layout), Some(&block_model_volume_bind_group_layout)],
            immediate_size: 0,
        });
        let edge_style_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[style_bind_group_layout_entry],
            label: Some("edge_style_bind_group_layout"),
        });

        // One instance per block: lower.xyz + grade, then upper.xyz + pad.
        // The shader expands vertex_index 0..36 into the cube's faces.
        let block_model_vertex_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: size_of::<BlockInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x3],
        })];
        // wgpu handles are not Send or Sync on wasm, where nothing crosses threads.
        #[cfg_attr(target_arch = "wasm32", allow(clippy::arc_with_non_send_sync))]
        let scene_pipelines = Arc::new(scene_pipelines::create_scene_pipelines(
            &device,
            &scene_pipelines::ScenePipelineLayouts {
                camera: &camera_bind_group_layout,
                grid: &grid_bind_group_layout,
                surface_style: &surface_style_bind_group_layout,
                surface_chunk: &surface_chunk_bind_group_layout,
                raster_surface: &raster_surface_bind_group_layout,
                edge_style: &edge_style_bind_group_layout,
                block_model_transparency_composite: &block_model_transparency_composite_bind_group_layout,
                block_model_volume_upscale: &block_model_volume_upscale_bind_group_layout,
            },
            scene_format,
            sample_count,
            scene_pipelines::SceneShading::Standard,
        ));
        let block_model_volume_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Block Model Volume Raycast Pipeline"),
            layout: Some(&block_model_volume_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &block_model_volume_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &block_model_volume_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: scene_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            // The raycast now renders into the single-sample off-screen
            // volume target (upscaled afterwards), not the MSAA surface, so
            // this must be 1 to match the attachment.
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        let block_model_beam_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Block Model Volume Beam Pipeline"),
            layout: Some(&block_model_beam_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &block_model_volume_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &block_model_volume_shader,
                entry_point: Some("fs_beam"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        let block_model_transparency_fallback_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Block Model Transparency Fallback Pipeline"),
            layout: Some(&block_model_transparency_fallback_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &block_model_transparency_fallback_shader,
                entry_point: Some("vs_main"),
                buffers: &block_model_vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &block_model_transparency_fallback_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let lyon_buffer: VertexBuffers<Vertex, u32> = VertexBuffers::new();
        let lyon_vertex_gpu = Self::create_stream_buffer(&device, "Lyon Vertex Buffer", size_of::<Vertex>(), wgpu::BufferUsages::VERTEX);
        let lyon_index_gpu = Self::create_stream_buffer(&device, "Lyon Index Buffer", size_of::<u32>(), wgpu::BufferUsages::INDEX);
        let stroke_vertex_gpu = Self::create_stream_buffer(&device, "Stroke Vertex Buffer", size_of::<StrokeVertex>(), wgpu::BufferUsages::VERTEX);
        let stroke_index_gpu = Self::create_stream_buffer(&device, "Stroke Index Buffer", size_of::<u32>(), wgpu::BufferUsages::INDEX);
        let overlay_vertex_gpu = Self::create_stream_buffer(&device, "Editor Overlay Vertex Buffer", size_of::<StrokeVertex>(), wgpu::BufferUsages::VERTEX);
        let overlay_index_gpu = Self::create_stream_buffer(&device, "Editor Overlay Index Buffer", size_of::<u32>(), wgpu::BufferUsages::INDEX);
        let dynamic_vertex_gpu = Self::create_stream_buffer(&device, "Dynamic Scene Vertex Buffer", size_of::<StrokeVertex>(), wgpu::BufferUsages::VERTEX);
        let dynamic_index_gpu = Self::create_stream_buffer(&device, "Dynamic Scene Index Buffer", size_of::<u32>(), wgpu::BufferUsages::INDEX);
        let text_vertex_gpu = Self::create_stream_buffer(&device, "Document Text Vertex Buffer", size_of::<Vertex>(), wgpu::BufferUsages::VERTEX);
        let text_index_gpu = Self::create_stream_buffer(&device, "Document Text Index Buffer", size_of::<u32>(), wgpu::BufferUsages::INDEX);

        let text_system = TextSystem::new();
        let gui = Gui::new(&window, &device, gui_format);
        // High-precision block-model attachments are created on first visible
        // use; ordinary document and topology views pay no full-screen VRAM
        // cost for them.
        let block_model_transparency_targets = None;
        let block_model_volume_target = None;
        let design_point_gpu = DesignPointGpuCache::new(&device, &edge_style_bind_group_layout);
        Ok(Self {
            gui,
            text_system,
            scene_pipelines,
            active_scene: None,
            block_model_volume_pipeline,
            block_model_beam_pipeline,
            block_model_beam_bind_group_layout,
            block_model_transparency_fallback_pipeline,
            block_model_volume_upscale_bind_group_layout,
            block_model_transparency_fallback_bind_group_layout,
            block_model_transparency_composite_bind_group_layout,
            block_model_volume_bind_group_layout,
            surface_style_bind_group_layout,
            surface_chunk_bind_group_layout,
            raster_surface_bind_group_layout,
            edge_style_bind_group_layout,
            lyon_vertex_gpu,
            lyon_index_gpu,
            stroke_vertex_gpu,
            stroke_index_gpu,
            overlay_vertex_gpu,
            overlay_index_gpu,
            dynamic_vertex_gpu,
            dynamic_index_gpu,
            text_vertex_gpu,
            text_index_gpu,
            camera_buffer,
            camera_bind_group,
            camera_bind_group_layout,
            grid_buffer,
            grid_bind_group,
            section_grid_buffer,
            section_grid_bind_group,
            msaa_color,
            msaa_view,
            scene_cache,
            scene_cache_blit_layout,
            scene_cache_blit_pipeline,
            no_vsync_present_mode,
            scene_cache_key: None,
            depth_texture,
            depth_view,
            block_model_transparency_targets,
            block_model_volume_target,
            #[cfg(not(target_arch = "wasm32"))]
            instance,
            #[cfg(not(target_arch = "wasm32"))]
            adapter,
            window,
            surface,
            queue,
            device,
            config,
            sample_count,
            size,
            viewport_rect: ViewportRect::full(size.width, size.height),
            startup_view_offset: Some(DVec2::ZERO),
            lyon_buffer,
            lyon_vertex_capacity: 1,
            lyon_index_capacity: 1,
            camera,
            camera_uniform,
            camera_controller,
            fly_camera_controller,
            projection,
            mouse_pressed: None,
            touch_gesture: Default::default(),
            fly_mode_enabled: false,
            slice_view: None,
            stroke_index_buf: Vec::new(),
            stroke_vertex_buf: Vec::new(),
            stroke_vertex_capacity: 1,
            stroke_index_capacity: 1,
            overlay_vertex_buf: Vec::new(),
            overlay_index_buf: Vec::new(),
            overlay_vertex_capacity: 1,
            overlay_index_capacity: 1,
            dynamic_vertex_buf: Vec::new(),
            dynamic_index_buf: Vec::new(),
            dynamic_vertex_capacity: 1,
            dynamic_index_capacity: 1,
            text_vertex_buf: Vec::new(),
            text_index_buf: Vec::new(),
            text_vertex_capacity: 1,
            text_index_capacity: 1,
            text_draw_batches: Vec::new(),
            frame_index: 0,
            retired_attachments: Vec::new(),
            last_text_cache_trim_frame: 0,
            last_interaction: None,
            geometry_dirty: true,
            polyline_fill_cache: Default::default(),
            cached_document_revision: u64::MAX,
            cached_render_style_key: None,
            cached_bounds_document_revision: u64::MAX,
            cached_scene_bounds: None,
            cached_object_aabbs: Vec::new(),
            overlay_dirty: true,
            cached_scale_factor: 0.0,
            cached_measurement_state: (false, None, None, Vec::new()),
            cached_poly_finish_dialog: false,
            pick_records: Vec::new(),
            text_pick_records: Vec::new(),
            document_draw_batches: Vec::new(),
            orbit_marker: None,
            scene_origin: DVec3::ZERO,
            vertical_exaggeration: 1.0,
            triangulation_gpu: TriangulationGpuCache::default(),
            static_strokes: StaticStrokeCache::default(),
            block_model_gpu: BlockModelGpuCache::default(),
            point_cloud_gpu: PointCloudGpuCache::default(),
            drill_hole_gpu: DrillHoleGpuCache::default(),
            design_point_gpu,
            raster_gpu: RasterGpuCache::default(),
            chunk_render_stats: (0, 0),
            plot_preview: None,
            plot_preview_key: None,
            pending_screenshot: None,
            slice_preview: None,
            embedded_slice_preview: None,
            embedded_preview_scene_key: None,
            detached_preview_scene_key: None,
            #[cfg(not(target_arch = "wasm32"))]
            cinematic: None,
            #[cfg(not(target_arch = "wasm32"))]
            cinematic_targets: None,
        })
    }

    pub(crate) fn reconfigure(&mut self) {
        // Surface recovery does not change attachment dimensions. Rebuilding
        // full-resolution MSAA/depth targets on every failure wastes GPU memory.
        self.surface.configure(&self.device, &self.config);
    }

    pub(crate) fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 && new_size != self.size {
            self.mark_interaction();
            // Free what previous resizes replaced before allocating this
            // resize's attachments, so a drag holds one extra set rather than
            // one per event.
            self.release_retired_attachments();
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            let (msaa_color, msaa_view) = Self::create_msaa_target(&self.device, &self.config, self.sample_count);
            self.msaa_view = msaa_view;
            let (depth_texture, depth_view) = Self::create_depth_target(&self.device, &self.config, self.sample_count);
            self.depth_view = depth_view;
            let scene_cache = Self::create_scene_cache_target(&self.device, &self.config, &self.scene_cache_blit_layout);
            self.scene_cache_key = None;
            // Hand the attachments this resize replaced to the retirement queue
            // rather than dropping them for the browser's collector to find.
            // `release_retired_attachments` destroys them once the frames that
            // drew into them have been presented.
            let mut retired = RetiredAttachments {
                retired_at_frame: self.frame_index,
                retired_at: Instant::now(),
                textures: Vec::new(),
                buffers: Vec::new(),
            };
            retired.textures.push(std::mem::replace(&mut self.msaa_color, msaa_color));
            retired.textures.push(std::mem::replace(&mut self.depth_texture, depth_texture));
            retired.textures.push(std::mem::replace(&mut self.scene_cache, scene_cache).texture);
            // Lazily-created block-model attachments are recreated only if
            // the resized viewport actually renders a block model.
            if let Some(targets) = self.block_model_transparency_targets.take() {
                retired.textures.extend(targets._accum_textures);
            }
            if let Some(target) = self.block_model_volume_target.take() {
                retired.textures.push(target._texture);
                retired.textures.push(target._beam_texture);
                retired.buffers.push(target.params_buffer);
            }
            // The cinematic chain's attachments go the same way. Its pipelines
            // and shadow map are size-independent and stay put.
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(targets) = self.cinematic_targets.take() {
                retired.textures.extend(targets.into_textures());
            }
            self.retired_attachments.push(retired);
            // Document geometry is stored in world space and screen-space stroke
            // sizing is handled by the viewport uniform. Resizing therefore only
            // requires new surface-sized attachments; rebuilding and re-uploading
            // the entire document here makes interactive resize needlessly laggy.
            self.overlay_dirty = true;
        }
    }

    /// Apply the visible scene sub-rect computed by this frame's egui layout,
    /// for use starting next frame (the scene pass runs before egui layout,
    /// so it's always one frame behind - see `frame::render`).
    pub(super) fn apply_canvas_rect(&mut self, rect: ViewportRect) {
        if rect.width == 0 || rect.height == 0 || rect == self.viewport_rect {
            return;
        }
        self.viewport_rect = rect;
        self.projection.resize(rect.width, rect.height);
        self.camera_uniform.update_viewport(rect.width, rect.height);
        self.camera_uniform.set_viewport_origin(rect.x, rect.y);
    }
}
