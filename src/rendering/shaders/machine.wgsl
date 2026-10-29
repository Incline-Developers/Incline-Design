// Animate's machines: one low-poly model per instance, turned to its heading
// and held at a least size on screen so a machine stays readable from across
// the pit.

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct InstanceInput {
    // Relative to the scene origin.
    @location(3) origin: vec3<f32>,
    // The model's longest plan extent in metres.
    @location(4) length: f32,
    // Unit vector in plan the model's +X turns to.
    @location(5) heading: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) section_offset: f32,
};

// The least a machine spans on screen, in physical pixels, however far out
// the camera is.
const MIN_LENGTH_PIXELS: f32 = 28.0;

const SUN: vec3<f32> = vec3<f32>(-0.55, -0.35, 0.76);

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let exaggeration = max(camera.cam_forward.w, 1.0e-6);
    let origin_clip = camera.view_proj * vec4<f32>(instance.origin, 1.0);
    // Pixels one metre covers at the machine, measured across the view so
    // it does not vanish when the camera looks along it.
    let across = normalize(select(cross(camera.cam_forward.xyz, vec3<f32>(0.0, 0.0, 1.0)), vec3<f32>(1.0, 0.0, 0.0),
        length(cross(camera.cam_forward.xyz, vec3<f32>(0.0, 0.0, 1.0))) < 1.0e-4));
    let across_clip = camera.view_proj * vec4<f32>(across, 0.0);
    let w = max(abs(origin_clip.w), 1.0e-6);
    let ndc_per_metre = (across_clip.xy * origin_clip.w - origin_clip.xy * across_clip.w) / (w * w);
    let pixels_per_metre = max(length(ndc_per_metre * camera.viewport.xy * 0.5), 1.0e-6);
    let scale = max(1.0, MIN_LENGTH_PIXELS / (pixels_per_metre * max(instance.length, 1.0e-3)));

    let c = instance.heading.x;
    let s = instance.heading.y;
    let turn = mat3x3<f32>(vec3<f32>(c, s, 0.0), vec3<f32>(-s, c, 0.0), vec3<f32>(0.0, 0.0, 1.0));
    // The scene stretches heights by the vertical exaggeration; a machine
    // keeps its own proportions, so its height is shrunk by the same.
    let local = turn * (vertex.position * scale) * vec3<f32>(1.0, 1.0, 1.0 / exaggeration);
    let world = instance.origin + local;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world, 1.0);
    out.normal = turn * vertex.normal;
    out.color = vertex.color;
    out.section_offset = section_plane_offset(world);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if outside_section_slab(in.section_offset) {
        discard;
    }
    let normal = normalize(in.normal);
    let sun = max(dot(normal, normalize(SUN)), 0.0);
    let sky = 0.35 + 0.15 * normal.z;
    return vec4<f32>(in.color * (sky + 0.75 * sun), 1.0);
}
