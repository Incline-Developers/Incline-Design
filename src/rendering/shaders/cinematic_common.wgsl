
// Shared state for the cinematic post chain, prefixed after the camera prelude
// onto every cinematic shader that binds a camera (see `make_cinematic_shader`
// in rendering/graphics/cinematic.rs).
//
// Everything here works in *display space*: scene-origin-relative metres with
// the vertical exaggeration already applied, which is the space the scene's
// vertices are drawn in and therefore the space depth reconstructs into.

struct CinematicParams {
    // Display-space world -> shadow map clip. Fitted to the whole scene rather
    // than the view frustum, so it does not shimmer as the camera moves.
    light_view_proj: mat4x4<f32>,
    // xyz: unit vector pointing *at* the sun. w: one shadow map texel in
    // metres, which sizes the comparison filter.
    sun: vec4<f32>,
    sun_color: vec4<f32>,
    // x: ambient occlusion radius in metres, y: its strength, z: shadow
    // strength, w: 1 when a shadow map was rendered for this frame.
    params: vec4<f32>,
    // x: bloom intensity, y: exposure. zw: unused.
    grade: vec4<f32>,
};
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

/// Display-space position of the fragment a depth sample came from.
/// Depth is reversed-Z: 1 at the near plane, 0 where nothing was drawn.
fn world_from_depth(target_pixel: vec2<f32>, depth: f32) -> vec3<f32> {
    let point = camera.inv_view_proj * vec4<f32>(ndc_from_target_pixel(target_pixel), depth, 1.0);
    return point.xyz / point.w;
}

/// Target-space pixel a display-space point projects to, with its reversed-Z
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
    return dot(world - camera.cam_position.xyz, camera.cam_forward.xyz);
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

/// Interleaved gradient noise - one cheap per-pixel rotation so the occlusion
/// sample pattern differs between neighbours and blurs out to smooth.
fn dither(target_pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(target_pixel, vec2<f32>(0.06711056, 0.00583715))));
}
