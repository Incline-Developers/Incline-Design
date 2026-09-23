// The pipelines the scene pass draws with, built once per way of shading it.
//
// The ordinary view draws into the 8-bit multisample target the editor
// overlay shares. The cinematic view draws the same geometry into a half-float
// target with the lighting done in the material shaders, and that needs a
// second copy of every pipeline: a pipeline is fixed to the format of the
// attachment it writes, and the lit ones bind the sun and shadow maps beside
// the camera at group 0. Both copies are built here from one description, so
// the two views cannot drift apart in anything but how they shade.

use super::{init::make_shader, *};

/// Direction pointing *at* the sun, in model space.
///
/// Both quality modes use the same material lighting.
pub(super) const SUN_DIRECTION: glam::Vec3 = glam::vec3(-0.55, -0.35, 0.76);

/// Very nearly white. A surface's colour is data - a grade ramp, a domain, the
/// white the user picked for a pit shell - so the light must not repaint it;
/// a warmer sun turned every white surface beige. The faint warmth left is
/// only enough to part lit faces from the cool shadow beside them.
pub(super) const SUN_COLOR: u32 = 0xfff8f0;
/// Sunlit rock facing the sun lands a little under one, so a white surface
/// tone maps to a light grey and white design strings stay legible over it.
pub(super) const SUN_INTENSITY: f32 = 0.9;
pub(super) const SKY_COLOR: u32 = 0xe4ecf8;
pub(super) const SKY_INTENSITY: f32 = 0.3;
/// Light the ground bounces back up onto overhangs and the undersides of
/// things: neutral, dim, and never zero, so nothing reads as a hole.
pub(super) const GROUND_COLOR: u32 = 0xd8d4cc;
pub(super) const GROUND_INTENSITY: f32 = 0.12;

/// PBR Neutral is linear below its knee, so this lands flat, sunlit white
/// ground (about 0.93 before exposure) near 0.7: a light grey under the white
/// design strings.
pub(super) const SCENE_EXPOSURE: f32 = 0.8;

const DRILL_SEGMENT_ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x3, 3 => Uint32];
const DRILL_COLLAR_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Uint32, 4 => Float32];

/// The bind group layouts the scene pipelines are laid out against. Everything
/// but `camera` is shared by both copies; `camera` is the plain camera layout
/// for the ordinary view and the camera-plus-lighting layout for the cinematic
/// one (created by `cinematic::create_cinematic_pipelines`).
pub(crate) struct ScenePipelineLayouts<'a> {
    pub(crate) camera: &'a wgpu::BindGroupLayout,
    pub(crate) grid: &'a wgpu::BindGroupLayout,
    pub(crate) surface_style: &'a wgpu::BindGroupLayout,
    pub(crate) surface_chunk: &'a wgpu::BindGroupLayout,
    pub(crate) raster_surface: &'a wgpu::BindGroupLayout,
    pub(crate) edge_style: &'a wgpu::BindGroupLayout,
    pub(crate) point_cloud_style: &'a wgpu::BindGroupLayout,
    pub(crate) block_model_transparency_composite: &'a wgpu::BindGroupLayout,
    pub(crate) block_model_volume_upscale: &'a wgpu::BindGroupLayout,
    /// The drill selection bitset, group 1 of both drill pipelines.
    pub(crate) drill_selection: &'a wgpu::BindGroupLayout,
}

/// How the scene pass shades what it draws.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SceneShading {
    /// Smooth sun/sky lighting graded directly into the 8-bit target, without
    /// shadows or screen-space effects.
    Standard,
    /// The cinematic view: a sun with real shadows, sky light and a specular
    /// term, into a half-float target the post chain then exposes and grades.
    /// Native only, like the cinematic view itself.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Cinematic,
}

impl SceneShading {
    /// `state`, adjusted for the target it blends into. The cinematic
    /// target's alpha channel is not coverage but a mask - 0 where a point
    /// cloud drew, 1 wherever something that hides it did - which the
    /// eye-dome lighting pass reads to find the points. Geometry that
    /// `occludes` (it writes depth, so it hides what is behind it) lays its
    /// coverage over the mask; everything else leaves the mask as it was, so
    /// a translucent surface over a cloud does not hide the cloud from it.
    fn blend(self, state: wgpu::BlendState, occludes: bool) -> Option<wgpu::BlendState> {
        match self {
            Self::Standard => Some(state),
            Self::Cinematic => Some(wgpu::BlendState {
                color: state.color,
                alpha: if occludes {
                    wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    }
                } else {
                    wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    }
                },
            }),
        }
    }

    /// A shader that shades through the scene lighting prelude: the
    /// `shade_*` functions the material shaders call are the ordinary view's
    /// sun/sky lighting in both, with shadow sampling added in cinematic.
    fn lit_shader(self, device: &wgpu::Device, label: &str, body: &str) -> wgpu::ShaderModule {
        let camera = include_str!("../shaders/camera_common.wgsl");
        let common = include_str!("../shaders/scene_lighting_common.wgsl");
        let source = match self {
            Self::Standard => {
                let vector = |value: glam::Vec3| format!("vec3<f32>({:?}, {:?}, {:?})", value.x, value.y, value.z);
                let color = |hex, intensity| {
                    let [r, g, b, _] = crate::rendering::color::hex_to_linear_rgba(hex);
                    vector(glam::vec3(r, g, b) * intensity)
                };
                let settings = format!(
                    "const STANDARD_SUN = {}; const STANDARD_SUN_COLOR = {}; const STANDARD_SKY_COLOR = {}; const STANDARD_GROUND_COLOR = {}; const STANDARD_EXPOSURE = {SCENE_EXPOSURE:?};",
                    vector(SUN_DIRECTION.normalize()),
                    color(SUN_COLOR, SUN_INTENSITY),
                    color(SKY_COLOR, SKY_INTENSITY),
                    color(GROUND_COLOR, GROUND_INTENSITY),
                );
                format!("{camera}{common}{settings}{}{body}", include_str!("../shaders/scene_lighting_standard.wgsl"))
            }
            Self::Cinematic => format!(
                "{camera}{common}{}{}{}{body}",
                include_str!("../shaders/cinematic_params.wgsl"),
                include_str!("../shaders/scene_shadow_map.wgsl"),
                include_str!("../shaders/scene_lighting_cinematic.wgsl"),
            ),
        };
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(source)),
        })
    }
}

/// `surface.wgsl`, with its fragment barycentrics swapped for a constant that
/// never reaches an edge when the device cannot provide them: naga rejects the
/// builtin outright without the feature, even left unused.
fn surface_shader_body(device: &wgpu::Device) -> String {
    let body = include_str!("../shaders/surface.wgsl");
    if device.features().contains(wgpu::Features::SHADER_BARYCENTRICS) {
        return body.to_owned();
    }
    const INPUT: &str = ", @builtin(barycentric) barycentric: vec3<f32>";
    debug_assert!(body.contains(INPUT), "surface.wgsl's barycentric input moved");
    format!("const barycentric = vec3<f32>(1.0);\n{}", body.replace(INPUT, ""))
}

/// Every pipeline that draws into the scene pass's colour attachment. The
/// volume raycast, its beam pre-pass and the transparency fallback write
/// intermediate targets of their own and stay on [`Graphics`].
pub(crate) struct ScenePipelines {
    pub(crate) surface_render_pipeline: wgpu::RenderPipeline,
    pub(crate) transparent_surface_render_pipeline: wgpu::RenderPipeline,
    pub(crate) grid_render_pipeline: wgpu::RenderPipeline,
    pub(crate) section_grid_render_pipeline: wgpu::RenderPipeline,
    pub(crate) raster_plane_render_pipeline: wgpu::RenderPipeline,
    pub(crate) block_model_render_pipeline: wgpu::RenderPipeline,
    pub(crate) block_model_transparency_composite_pipeline: wgpu::RenderPipeline,
    pub(crate) block_model_volume_upscale_pipeline: wgpu::RenderPipeline,
    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) transparent_document_fill_pipeline: wgpu::RenderPipeline,
    pub(crate) xray_render_pipeline: wgpu::RenderPipeline,
    pub(crate) opaque_stroke_render_pipeline: wgpu::RenderPipeline,
    pub(crate) stroke_render_pipeline: wgpu::RenderPipeline,
    pub(crate) edge_render_pipeline: wgpu::RenderPipeline,
    pub(crate) point_cloud_colored_render_pipeline: wgpu::RenderPipeline,
    pub(crate) point_cloud_uncolored_render_pipeline: wgpu::RenderPipeline,
    pub(crate) drill_hole_render_pipeline: wgpu::RenderPipeline,
    pub(crate) xray_drill_hole_render_pipeline: wgpu::RenderPipeline,
    pub(crate) drill_collar_render_pipeline: wgpu::RenderPipeline,
    pub(crate) xray_drill_collar_render_pipeline: wgpu::RenderPipeline,
    pub(crate) design_point_render_pipeline: wgpu::RenderPipeline,
    /// Only ever drawn by the editor overlay pass, which is always the
    /// ordinary view's; the cinematic copy exists only so the two sets share
    /// one shape.
    pub(crate) overlay_render_pipeline: wgpu::RenderPipeline,
}

pub(crate) fn create_scene_pipelines(
    device: &wgpu::Device,
    layouts: &ScenePipelineLayouts<'_>,
    format: wgpu::TextureFormat,
    sample_count: u32,
    shading: SceneShading,
) -> ScenePipelines {
    let shader = make_shader(device, "../shaders/shader.wgsl", include_str!("../shaders/shader.wgsl"));
    let surface_shader = shading.lit_shader(device, "../shaders/surface.wgsl", &surface_shader_body(device));
    let grid_shader = make_shader(device, "../shaders/grid.wgsl", include_str!("../shaders/grid.wgsl"));
    let section_grid_shader = make_shader(device, "../shaders/section_grid.wgsl", include_str!("../shaders/section_grid.wgsl"));
    let block_model_shader = shading.lit_shader(device, "../shaders/block_model.wgsl", include_str!("../shaders/block_model.wgsl"));
    let block_model_transparency_composite_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/block_model_transparency_composite.wgsl"));
    let block_model_volume_upscale_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/block_model_volume_upscale.wgsl"));
    let stroke_shader = make_shader(device, "../shaders/stroke.wgsl", include_str!("../shaders/stroke.wgsl"));
    let edge_shader = make_shader(device, "../shaders/edge.wgsl", include_str!("../shaders/edge.wgsl"));
    let point_cloud_shader = shading.lit_shader(device, "../shaders/point_cloud.wgsl", include_str!("../shaders/point_cloud.wgsl"));
    // The selection block is one file both drill shaders take as a prelude,
    // the way `make_shader` hands every shader the camera one.
    let drill_selection = include_str!("../shaders/drill_selection_common.wgsl");
    let drill_hole_body = format!("{drill_selection}{}", include_str!("../shaders/drill_hole.wgsl"));
    let drill_hole_shader = shading.lit_shader(device, "../shaders/drill_hole.wgsl", &drill_hole_body);
    let drill_collar_body = format!("{drill_selection}{}", include_str!("../shaders/drill_collar.wgsl"));
    let drill_collar_shader = make_shader(device, "../shaders/drill_collar.wgsl", &drill_collar_body);
    let design_point_shader = make_shader(device, "../shaders/design_point.wgsl", include_str!("../shaders/design_point.wgsl"));
    let raster_plane_shader = make_shader(device, "../shaders/raster_plane.wgsl", include_str!("../shaders/raster_plane.wgsl"));

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.camera)],
        immediate_size: 0,
    });
    let drill_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Drill Hole Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.camera), Some(layouts.drill_selection)],
        immediate_size: 0,
    });
    let grid_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("XY Grid Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.camera), Some(layouts.grid)],
        immediate_size: 0,
    });
    let surface_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Surface Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.camera), Some(layouts.surface_style)],
        immediate_size: 0,
    });
    let tri_surface_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Triangulation Surface Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.camera), Some(layouts.surface_style), Some(layouts.surface_chunk), Some(layouts.raster_surface)],
        immediate_size: 0,
    });
    let block_model_transparency_composite_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Block Model Transparency Composite Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.block_model_transparency_composite)],
        immediate_size: 0,
    });
    let block_model_volume_upscale_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Block Model Volume Upscale Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.block_model_volume_upscale)],
        immediate_size: 0,
    });
    let edge_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Edge Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.camera), Some(layouts.edge_style)],
        immediate_size: 0,
    });
    let point_cloud_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Point Cloud Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.camera), Some(layouts.point_cloud_style)],
        immediate_size: 0,
    });

    let vertex_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
    })];
    let surface_vertex_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<SurfaceVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
    })];
    // One instance per block: lower.xyz + grade, then upper.xyz + pad.
    // The shader expands vertex_index 0..36 into the cube's faces.
    let block_model_vertex_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<BlockInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x3],
    })];

    let stroke_vertex_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<StrokeVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4, 2 => Float32x3, 3 => Float32x2, 4 => Float32],
    })];
    let edge_instance_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<EdgeInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
    })];
    let point_uncolored_instance_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<PointPosition>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
    })];
    let point_colored_instance_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<PointInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4],
    })];
    let drill_hole_instance_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<DrillSegmentInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &DRILL_SEGMENT_ATTRIBUTES,
    })];
    let drill_collar_instance_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<DrillCollarInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &DRILL_COLLAR_ATTRIBUTES,
    })];

    let create_stroke_pipeline = |label, depth_stencil: Option<wgpu::DepthStencilState>| {
        let occludes = depth_stencil.as_ref().is_some_and(|depth| depth.depth_write_enabled == Some(true));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &stroke_shader,
                entry_point: Some("vs_main"),
                buffers: &stroke_vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &stroke_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, occludes),
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
            depth_stencil,
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    };
    let stroke_render_pipeline = create_stroke_pipeline("Depth-tested Stroke Render Pipeline", Some(Graphics::depth_state(false, -1)));
    let opaque_stroke_render_pipeline = create_stroke_pipeline("Opaque Depth-writing Stroke Render Pipeline", Some(Graphics::depth_state(true, -1)));
    let edge_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Instanced Triangulation Edge Pipeline"),
        layout: Some(&edge_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &edge_shader,
            entry_point: Some("vs_main"),
            buffers: &edge_instance_buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &edge_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, false),
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
        depth_stencil: Some(Graphics::depth_state(false, -1)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    // Point splats write depth so clouds occlude correctly against meshes
    // and themselves.
    let point_cloud_colored_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Colored Point Cloud Pipeline"),
        layout: Some(&point_cloud_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &point_cloud_shader,
            entry_point: Some("vs_colored"),
            buffers: &point_colored_instance_buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &point_cloud_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                // Imported point colors are opaque. Avoiding the blend
                // unit preserves the result while allowing the
                // depth/color path to use the cheapest opaque writes.
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(Graphics::depth_state(true, 0)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    let point_cloud_uncolored_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Uncolored Point Cloud Pipeline"),
        layout: Some(&point_cloud_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &point_cloud_shader,
            entry_point: Some("vs_uncolored"),
            buffers: &point_uncolored_instance_buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &point_cloud_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                // The fallback point-cloud color is currently opaque.
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(Graphics::depth_state(true, 0)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    let create_drill_hole_pipeline = |label, depth_stencil| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&drill_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &drill_hole_shader,
                entry_point: Some("vs_main"),
                buffers: &drill_hole_instance_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &drill_hole_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    };
    let drill_hole_render_pipeline = create_drill_hole_pipeline("Opaque Drillhole Cylinder Pipeline", Graphics::depth_state(true, 0));
    let mut xray_drill_hole_depth = Graphics::depth_state(false, 0);
    xray_drill_hole_depth.depth_compare = Some(wgpu::CompareFunction::Always);
    let xray_drill_hole_render_pipeline = create_drill_hole_pipeline("X-Ray Drillhole Cylinder Pipeline", xray_drill_hole_depth);
    let create_drill_collar_pipeline = |label, depth_stencil| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&drill_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &drill_collar_shader,
                entry_point: Some("vs_main"),
                buffers: &drill_collar_instance_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &drill_collar_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // The disc's rim is antialiased by coverage, not by
                    // blending: a blended rim would also write depth at
                    // partial opacity, and the scene geometry that draws
                    // after the collars would then be depth-rejected in
                    // that one-pixel ring, leaving it composited against
                    // the clear colour as a dark outline. Alpha is the
                    // coverage mask below and never reaches the target,
                    // so this writes colour only.
                    blend: None,
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                // The rim's fractional alpha becomes a sample mask, so an
                // uncovered sample keeps both the colour and the depth of
                // whatever stands behind the marker.
                alpha_to_coverage_enabled: true,
            },
            multiview_mask: None,
            cache: None,
        })
    };
    let drill_collar_render_pipeline = create_drill_collar_pipeline("Opaque Drillhole Collar Pipeline", Graphics::depth_state(true, 0));
    let mut xray_drill_collar_depth = Graphics::depth_state(false, 0);
    xray_drill_collar_depth.depth_compare = Some(wgpu::CompareFunction::Always);
    let xray_drill_collar_render_pipeline = create_drill_collar_pipeline("X-Ray Drillhole Collar Pipeline", xray_drill_collar_depth);
    let mut overlay_depth = Graphics::depth_state(false, 0);
    overlay_depth.depth_compare = Some(wgpu::CompareFunction::Always);
    let design_point_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Design Point Overlay Pipeline"),
        layout: Some(&edge_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &design_point_shader,
            entry_point: Some("vs_main"),
            buffers: &point_uncolored_instance_buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &design_point_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, false),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(overlay_depth.clone()),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    let overlay_render_pipeline = create_stroke_pipeline("Editor Overlay Render Pipeline", Some(overlay_depth));

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &vertex_buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, true),
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
        depth_stencil: Some(Graphics::depth_state(true, 0)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });

    // Triangulation surface pipelines use position-only vertices with a per-draw colour
    // uniform.
    let create_tri_surface_pipeline = |label, write_depth, depth_compare| {
        let mut depth = Graphics::depth_state(write_depth, 0);
        depth.depth_compare = Some(depth_compare);
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&tri_surface_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &surface_shader,
                entry_point: Some("vs_main"),
                buffers: &surface_vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &surface_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, write_depth),
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
            depth_stencil: Some(depth),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    };
    let surface_render_pipeline = create_tri_surface_pipeline("Opaque Triangulation Surface Pipeline", true, wgpu::CompareFunction::GreaterEqual);
    let transparent_surface_render_pipeline = create_tri_surface_pipeline("Transparent Triangulation Surface Pipeline", false, wgpu::CompareFunction::GreaterEqual);
    let grid_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Infinite XY Grid Pipeline"),
        layout: Some(&grid_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &grid_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &grid_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, false),
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
        // The grid is a background construction overlay, not an opaque
        // floor: it never writes depth and is drawn before scene
        // geometry, so nothing in the scene is ever occluded by it.
        depth_stencil: Some(Graphics::depth_state(false, 0)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    // The section grid shares the XY grid's shape and layout but not its
    // rule: it is depth tested and writes depth on its lines, so geometry
    // in front of the plane hides it and geometry behind sits under it.
    let section_grid_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Section Grid Pipeline"),
        layout: Some(&grid_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &section_grid_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &section_grid_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, true),
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
        depth_stencil: Some(Graphics::depth_state(true, 0)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    // Flat plan-view images for undraped rasters: drawn first, pinned to
    // the far plane, no depth writes, so all scene geometry covers them.
    let raster_plane_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Raster Plane Pipeline Layout"),
        bind_group_layouts: &[Some(layouts.camera), Some(layouts.raster_surface)],
        immediate_size: 0,
    });
    let raster_plane_vertex_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: (size_of::<f32>() * 4) as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
    })];
    let raster_plane_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Raster Plane Pipeline"),
        layout: Some(&raster_plane_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &raster_plane_shader,
            entry_point: Some("vs_main"),
            buffers: &raster_plane_vertex_buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &raster_plane_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, false),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(Graphics::depth_state(false, 0)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    let create_block_model_surface_pipeline = |label, write_depth, depth_compare| {
        let mut depth = Graphics::depth_state(write_depth, 0);
        depth.depth_compare = Some(depth_compare);
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&surface_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &block_model_shader,
                entry_point: Some("vs_main"),
                buffers: &block_model_vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &block_model_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Every fragment this pipeline draws is opaque (the
                    // chunk builder routes alpha < 0.98 to the translucent
                    // path), so skip blending entirely: zoomed-in views
                    // are fill-bound and blending doubles the per-sample
                    // colour traffic at 4x MSAA for no visual effect.
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                // The cube corner tables in block_model.wgsl deliberately
                // wind every face inward. Exterior faces are therefore
                // classified as back-facing and the far/interior faces as
                // front-facing. Cull the latter so the camera-facing cube
                // shells remain visible. The fragment shader derives its
                // normal from derivatives and does not rely on the winding.
                cull_mode: Some(wgpu::Face::Front),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(depth),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    };
    let block_model_render_pipeline = create_block_model_surface_pipeline("Opaque Block Model Surface Pipeline", true, wgpu::CompareFunction::GreaterEqual);
    let block_model_transparency_composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Block Model Transparency Composite Pipeline"),
        layout: Some(&block_model_transparency_composite_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &block_model_transparency_composite_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &block_model_transparency_composite_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: shading.blend(
                    wgpu::BlendState {
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
                    },
                    false,
                ),
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
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    let block_model_volume_upscale_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Block Model Volume Upscale Pipeline"),
        layout: Some(&block_model_volume_upscale_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &block_model_volume_upscale_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &block_model_volume_upscale_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                // Premultiplied over: matches the direct volume pass
                // this replaces.
                blend: shading.blend(
                    wgpu::BlendState {
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
                    },
                    false,
                ),
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
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    });
    // Document-object xray pipeline keeps position+colour per-vertex.
    let create_surface_pipeline = |label, write_depth, depth_compare, shader_module: &wgpu::ShaderModule| {
        let mut depth = Graphics::depth_state(write_depth, 0);
        depth.depth_compare = Some(depth_compare);
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: shading.blend(wgpu::BlendState::ALPHA_BLENDING, false),
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
            depth_stencil: Some(depth),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    };
    let xray_render_pipeline = create_surface_pipeline("X-Ray Document Fill Pipeline", false, wgpu::CompareFunction::Always, &shader);
    let transparent_document_fill_pipeline = create_surface_pipeline("Transparent Document Fill Pipeline", false, wgpu::CompareFunction::GreaterEqual, &shader);

    ScenePipelines {
        surface_render_pipeline,
        transparent_surface_render_pipeline,
        grid_render_pipeline,
        section_grid_render_pipeline,
        raster_plane_render_pipeline,
        block_model_render_pipeline,
        block_model_transparency_composite_pipeline,
        block_model_volume_upscale_pipeline,
        render_pipeline,
        transparent_document_fill_pipeline,
        xray_render_pipeline,
        opaque_stroke_render_pipeline,
        stroke_render_pipeline,
        edge_render_pipeline,
        point_cloud_colored_render_pipeline,
        point_cloud_uncolored_render_pipeline,
        drill_hole_render_pipeline,
        xray_drill_hole_render_pipeline,
        drill_collar_render_pipeline,
        xray_drill_collar_render_pipeline,
        design_point_render_pipeline,
        overlay_render_pipeline,
    }
}
