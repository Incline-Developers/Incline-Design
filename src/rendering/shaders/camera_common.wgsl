// Shared camera uniform and section-slab helpers, prefixed to the source of
// every shader that binds a camera (see `make_shader` in rendering/graphics/init.rs).

struct CameraUniform {
    view_proj: mat4x4<f32>,
    // xyz: world-space forward; w: vertical exaggeration.
    cam_forward: vec4<f32>,
    // xyz: rebased world-space eye; w: 1 for perspective, 0 for orthographic.
    cam_position: vec4<f32>,
    viewport: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    // xy: the viewport rect's offset in physical pixels within its render
    // target, to be subtracted from `@builtin(position)`; zero except in the
    // main window's toolbar-bounded canvas. zw: padding.
    viewport_origin: vec4<f32>,
    // xyz: a point on the vertical section plane, relative to the scene
    // origin the vertex positions are relative to; w: half the slab width
    // in metres.
    section_plane: vec4<f32>,
    // xyz: the section plane's unit normal, horizontal by construction;
    // w: 1 while the section slab clips the scene, 0 in every other view.
    section_normal: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// How far a point lies from the section plane, signed by which wall it is on.
// The normal is horizontal, so vertical exaggeration cannot skew this.
fn section_plane_offset(world: vec3<f32>) -> f32 {
    return dot(world - camera.section_plane.xyz, camera.section_normal.xyz);
}

// A section shows a fixed slab of ground between two walls parallel to its
// plane. Outside a section the slab is off and this discards nothing.
fn outside_section_slab(offset: f32) -> bool {
    return camera.section_normal.w > 0.5 && abs(offset) > camera.section_plane.w;
}

// Coverage of a block's own outline at `p`, for the in-shader block-model
// wireframe. `lower`/`upper` bound the block (already clipped to any slice) in
// the same frame as `p`; `pixels_per_unit` converts a distance along each axis
// to screen pixels. The face `p` lies on is the axis it is relatively nearest
// a bound of; the outline is that face's border, `width` pixels wide centred
// on each edge (half falls on the neighbouring face), antialiased over one
// pixel. `faces` masks which faces draw it - bit 2a for the lower face across
// axis a, 2a + 1 for the upper - so a face shared with a visible neighbour
// stays bare and a translucent model shows only its skin.
fn block_wire_coverage(p: vec3<f32>, lower: vec3<f32>, upper: vec3<f32>, pixels_per_unit: vec3<f32>, width: f32, faces: u32) -> f32 {
    if (width <= 0.0) {
        return 0.0;
    }
    let to_lower = p - lower;
    let to_upper = upper - p;
    let distance = min(to_lower, to_upper);
    let relative = distance / max(upper - lower, vec3<f32>(1.0e-6));
    var axis = 0u;
    if (relative.y < relative.x && relative.y <= relative.z) {
        axis = 1u;
    } else if (relative.z < relative.x && relative.z < relative.y) {
        axis = 2u;
    }
    let face_bit = 1u << (axis * 2u + select(1u, 0u, to_lower[axis] <= to_upper[axis]));
    if ((faces & face_bit) == 0u) {
        return 0.0;
    }
    let pixels = distance * pixels_per_unit;
    var nearest = 1.0e9;
    for (var other = 0u; other < 3u; other++) {
        if (other != axis) {
            nearest = min(nearest, pixels[other]);
        }
    }
    return 1.0 - smoothstep(width * 0.5 - 0.5, width * 0.5 + 0.5, nearest);
}
