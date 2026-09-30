// Instanced document and editor strokes: one `StrokeInstance` per line,
// round disc or screen-aligned bar, each expanded here into a screen-space
// quad of six vertices. Widths are in physical pixels, so the CPU stores a
// segment once instead of four widened vertices and a join polygon.

struct StrokeInstance {
    @location(0) start: vec3<f32>,
    @location(1) half_width: f32,
    @location(2) end: vec3<f32>,
    @location(3) style: u32,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Distance from the section plane, taken at the line endpoint before the
    // screen-space widening below; affine in position, so interpolation is exact.
    @location(1) section_offset: f32,
    // A disc's pixel offset from its centre (y up); zero for other primitives.
    @location(2) disc_px: vec2<f32>,
    // A disc's radius in pixels; negative for other primitives.
    @location(3) @interpolate(flat) disc_radius: f32,
};

// Two triangles over (end, side): end 0 is `start`, end 1 is `end`.
const QUAD = array<vec2<f32>, 6>(
    vec2<f32>(0.0, -1.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, -1.0),
);

// Matches `MSAA_SAMPLE_COUNT` in rendering/graphics/mod.rs.
const SAMPLE_COUNT: u32 = 4u;
// The standard 4x sample positions, in pixels from the pixel centre (y down).
const SAMPLE_OFFSETS = array<vec2<f32>, 4>(
    vec2<f32>(-0.125, -0.375),
    vec2<f32>(0.375, -0.125),
    vec2<f32>(-0.375, 0.125),
    vec2<f32>(0.125, 0.375),
);

fn clip_ndc(clip: vec4<f32>) -> vec2<f32> {
    return clip.xy / max(abs(clip.w), 1e-6);
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, stroke: StrokeInstance) -> VertexOutput {
    var out: VertexOutput;
    let flags = document_style_flags(stroke.style);
    if document_style_culled(stroke.style, flags) {
        out.clip_position = CULLED_POSITION;
        return out;
    }
    out.color = document_styled_color(stroke.color, flags);
    out.disc_radius = -1.0;

    let corner = QUAD[vertex_index];
    let pixel_to_ndc = vec2<f32>(2.0 / camera.viewport.x, 2.0 / camera.viewport.y);
    var anchor = stroke.start;
    var offset_px: vec2<f32>;
    if (stroke.style & STROKE_ROUND) != 0u {
        // One pixel of margin keeps every sample the disc covers inside the quad.
        let square = vec2<f32>(corner.x * 2.0 - 1.0, corner.y) * (stroke.half_width + 1.0);
        offset_px = square;
        out.disc_px = square;
        out.disc_radius = stroke.half_width;
    } else if (stroke.style & STROKE_SCREEN_AXIS) != 0u {
        let axis = stroke.end.xy;
        let direction = axis / max(length(axis), 1e-6);
        offset_px = axis * (corner.x * 2.0 - 1.0) + vec2<f32>(-direction.y, direction.x) * corner.y * stroke.half_width;
    } else {
        let at_end = corner.x > 0.5;
        anchor = select(stroke.start, stroke.end, at_end);
        let start_clip = camera.view_proj * vec4<f32>(stroke.start, 1.0);
        let end_clip = camera.view_proj * vec4<f32>(stroke.end, 1.0);
        // The line's direction in pixels, so the width stays true on a
        // non-square viewport.
        let delta = (clip_ndc(end_clip) - clip_ndc(start_clip)) / pixel_to_ndc;
        let direction = select(vec2<f32>(1.0, 0.0), delta / length(delta), length(delta) > 1e-6);
        offset_px = vec2<f32>(-direction.y, direction.x) * corner.y * stroke.half_width;
    }

    out.section_offset = section_plane_offset(anchor);
    var clip = camera.view_proj * vec4<f32>(anchor, 1.0);
    clip = vec4<f32>(clip.xy + offset_px * pixel_to_ndc * clip.w, clip.z, clip.w);
    out.clip_position = clip;
    return out;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    if outside_section_slab(in.section_offset) {
        discard;
    }
    var out: FragmentOutput;
    out.color = in.color;
    out.mask = 0xffffffffu;
    if in.disc_radius >= 0.0 {
        // Cover exactly the samples inside the disc, which antialiases its
        // rim through the multisample resolve like any polygon edge.
        var mask = 0u;
        let radius_sq = in.disc_radius * in.disc_radius;
        for (var sample = 0u; sample < SAMPLE_COUNT; sample++) {
            let offset = SAMPLE_OFFSETS[sample];
            let at = in.disc_px + vec2<f32>(offset.x, -offset.y);
            if dot(at, at) <= radius_sq {
                mask |= 1u << sample;
            }
        }
        if mask == 0u {
            discard;
        }
        out.mask = mask;
    }
    return out;
}
