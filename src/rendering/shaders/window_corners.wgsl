// Cuts the window's corners round, on a surface the compositor blends by its
// alpha: the last draw of the frame, over everything the scene and the UI put
// down. See `graphics/window_corners.rs`.
//
// The pipeline blends colour as `dst * src.a` and replaces alpha with
// `src.a`, so what this returns is the coverage of each pixel by the rounded
// window: the corners fade out, premultiplied, and every other pixel keeps
// its colour and is made fully opaque, whatever alpha the passes before left.

struct Corners {
    // The surface in physical pixels.
    size: vec2<f32>,
    // Zero leaves the corners square.
    radius: f32,
    _padding: f32,
};

@group(0) @binding(0) var<uniform> corners: Corners;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One oversized triangle covering the clip volume.
    let x = f32(i32(index) / 2) * 4.0 - 1.0;
    let y = f32(i32(index) & 1) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    if corners.radius <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    // The centre of the nearest corner's arc, or the pixel itself away from
    // the corners; the pixel's distance past that arc is what is cut away.
    let centre = clamp(position.xy, vec2<f32>(corners.radius), corners.size - vec2<f32>(corners.radius));
    let outside = length(position.xy - centre) - corners.radius;
    return vec4<f32>(0.0, 0.0, 0.0, clamp(0.5 - outside, 0.0, 1.0));
}
