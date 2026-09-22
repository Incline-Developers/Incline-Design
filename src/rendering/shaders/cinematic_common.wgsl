// Shared state for the cinematic post chain, prefixed after the camera prelude
// and `cinematic_params.wgsl` onto every post shader that binds a camera (see
// `make_cinematic_shader` in rendering/graphics/init.rs).
//
// Positions are model space, as `cinematic_params.wgsl` describes: what depth
// reconstructs into through the camera's inverse view-projection. Distances
// *along the view* are measured in display space instead, with the vertical
// exaggeration applied, because that is the space the camera sits in.
//
// Every shader here reads `scene_depth`, which each declares at its own
// binding; the helpers below that load it rely on that shared name.

@group(1) @binding(0)
var<uniform> cine: CinematicParams;

// A single oversized triangle covering the viewport; no vertex buffer.
@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

// `@builtin(position)` reports pixels in the render *target*; the scene is
// drawn into a sub-rect of it (the toolbar-bounded canvas), so every screen
// mapping has to move into the viewport's own space first.
fn viewport_pixel(target_pixel: vec2<f32>) -> vec2<f32> {
    return target_pixel - camera.viewport_origin.xy;
}

fn inside_viewport(target_pixel: vec2<f32>) -> bool {
    let local = viewport_pixel(target_pixel);
    return all(local >= vec2<f32>(0.0)) && all(local < camera.viewport.xy);
}

fn ndc_from_target_pixel(target_pixel: vec2<f32>) -> vec2<f32> {
    let uv = viewport_pixel(target_pixel) / camera.viewport.xy;
    return vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
}

/// Model-space position of the fragment a depth sample came from.
/// Depth is reversed-Z: 1 at the near plane, 0 where nothing was drawn.
fn world_from_depth(target_pixel: vec2<f32>, depth: f32) -> vec3<f32> {
    let point = camera.inv_view_proj * vec4<f32>(ndc_from_target_pixel(target_pixel), depth, 1.0);
    return point.xyz / point.w;
}

/// Target-space pixel a model-space point projects to, with its reversed-Z
/// depth in `z`. The inverse of `world_from_depth`.
fn target_pixel_from_world(world: vec3<f32>) -> vec3<f32> {
    let clip = camera.view_proj * vec4<f32>(world, 1.0);
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    return vec3<f32>(uv * camera.viewport.xy + camera.viewport_origin.xy, ndc.z);
}

/// Distance along the view direction. Unlike distance from the eye this is
/// also correct under the orthographic projection, which has no eye point.
fn view_depth(world: vec3<f32>) -> f32 {
    let display = vec3<f32>(world.xy, world.z * cine.grade.w);
    return dot(display - camera.cam_position.xyz, camera.cam_forward.xyz);
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn load_depth(pixel: vec2<i32>) -> f32 {
    return textureLoad(scene_depth, pixel, 0);
}

/// Face orientation rebuilt from the depth buffer. Taking the *nearer* of the
/// two neighbours on each axis keeps the derivative on the near side of a
/// silhouette instead of straddling it, which would otherwise ring every edge
/// in the scene with a band of false occlusion.
fn reconstruct_normal(pixel: vec2<i32>, centre: vec3<f32>, centre_depth: f32) -> vec3<f32> {
    let left_depth = load_depth(pixel + vec2<i32>(-1, 0));
    let right_depth = load_depth(pixel + vec2<i32>(1, 0));
    let down_depth = load_depth(pixel + vec2<i32>(0, -1));
    let up_depth = load_depth(pixel + vec2<i32>(0, 1));

    let left = world_from_depth(vec2<f32>(pixel + vec2<i32>(-1, 0)) + 0.5, left_depth);
    let right = world_from_depth(vec2<f32>(pixel + vec2<i32>(1, 0)) + 0.5, right_depth);
    let down = world_from_depth(vec2<f32>(pixel + vec2<i32>(0, -1)) + 0.5, down_depth);
    let up = world_from_depth(vec2<f32>(pixel + vec2<i32>(0, 1)) + 0.5, up_depth);

    let dx = select(centre - left, right - centre, abs(right_depth - centre_depth) < abs(centre_depth - left_depth));
    let dy = select(centre - down, up - centre, abs(up_depth - centre_depth) < abs(centre_depth - down_depth));

    var normal = cross(dx, dy);
    let length_squared = dot(normal, normal);
    if length_squared < 1.0e-18 {
        return -camera.cam_forward.xyz;
    }
    normal = normal * inverseSqrt(length_squared);
    // Surfaces are two-sided and the reconstruction has no winding to go on,
    // so orient towards the viewer rather than trusting the cross product.
    return select(-normal, normal, dot(normal, -camera.cam_forward.xyz) > 0.0);
}
