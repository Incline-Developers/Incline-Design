// A surface drawn as its cut through the section plane: one instance per
// triangle, expanded into a screen-space line quad along the segment where the
// triangle crosses the plane, or into nothing where it does not. Done here
// rather than with a fragment test on the filled surface because a square-on
// section sees flat benches exactly edge-on, and those rasterise no fragments.

struct SurfaceStyle {
    color: vec4<f32>,
    // z: cut line width in pixels; w: 1 while the surface is selected.
    params: vec4<f32>,
    wire_color: vec4<f32>,
};
@group(1) @binding(0)
var<uniform> surface_style: SurfaceStyle;

struct SurfaceChunk {
    offset: vec4<f32>,
};
@group(2) @binding(0)
var<uniform> chunk: SurfaceChunk;

// The chunk's vertex buffer copied texel for texel into an `r32float` texture
// `SECTION_CUT_ROW` wide, so a vertex can be fetched by index from the vertex
// stage (`TriangulationGpuCache::sync_section_cuts`).
@group(3) @binding(0)
var vertex_texels: texture_2d<f32>;

const SECTION_CUT_ROW: u32 = 4096u;
// Floats per `SurfaceVertex`: position, then normal.
const VERTEX_FLOATS: u32 = 6u;

fn texel(index: u32) -> f32 {
    return textureLoad(vertex_texels, vec2<u32>(index % SECTION_CUT_ROW, index / SECTION_CUT_ROW), 0).x;
}

fn vertex_position(index: u32) -> vec3<f32> {
    let base = index * VERTEX_FLOATS;
    return vec3<f32>(texel(base), texel(base + 1u), texel(base + 2u)) + chunk.offset.xyz;
}

// Where `tail`-`head` meets the plane; only called across a sign change, so
// the denominator is never zero.
fn crossing(tail: vec3<f32>, head: vec3<f32>, tail_offset: f32, head_offset: f32) -> vec3<f32> {
    return tail + (head - tail) * (tail_offset / (tail_offset - head_offset));
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(@location(0) triangle: vec3<u32>, @builtin(vertex_index) corner: u32) -> VertexOutput {
    var out: VertexOutput;
    let a = vertex_position(triangle.x);
    let b = vertex_position(triangle.y);
    let c = vertex_position(triangle.z);
    let da = section_plane_offset(a);
    let db = section_plane_offset(b);
    let dc = section_plane_offset(c);
    // A vertex on the plane counts as in front, so a crossing is counted once
    // and a triangle meets the plane in exactly two edges or none.
    let sa = da >= 0.0;
    let sb = db >= 0.0;
    let sc = dc >= 0.0;
    if sa == sb && sb == sc {
        // Outside the clip volume and the same for all four corners: no area.
        out.clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    // The cut runs across the two edges that meet at the vertex alone on its side.
    var start: vec3<f32>;
    var end: vec3<f32>;
    if sa == sb {
        start = crossing(c, a, dc, da);
        end = crossing(c, b, dc, db);
    } else if sa == sc {
        start = crossing(b, a, db, da);
        end = crossing(b, c, db, dc);
    } else {
        start = crossing(a, b, da, db);
        end = crossing(a, c, da, dc);
    }

    let start_clip = camera.view_proj * vec4<f32>(start, 1.0);
    let end_clip = camera.view_proj * vec4<f32>(end, 1.0);
    let pixel_to_ndc = vec2<f32>(2.0 / camera.viewport.x, 2.0 / camera.viewport.y);
    let start_px = start_clip.xy / max(abs(start_clip.w), 1e-6) / pixel_to_ndc;
    let end_px = end_clip.xy / max(abs(end_clip.w), 1e-6) / pixel_to_ndc;
    let span = end_px - start_px;
    let direction = select(vec2<f32>(1.0, 0.0), span / length(span), length(span) > 1e-6);
    let normal = vec2<f32>(-direction.y, direction.x);
    let half_width = surface_style.params.z * 0.5;
    // Strip corners: start then end, each on both sides. Each end is pushed out
    // by half the width too, so neighbouring triangles' cuts overlap at the
    // shared crossing instead of leaving a notch at every bend.
    let at_end = corner >= 2u;
    let side = select(-1.0, 1.0, (corner & 1u) == 1u);
    var clip = select(start_clip, end_clip, at_end);
    let along = select(-1.0, 1.0, at_end);
    let shift_px = normal * side * half_width + direction * along * half_width;
    clip.x = clip.x + shift_px.x * pixel_to_ndc.x * clip.w;
    clip.y = clip.y + shift_px.y * pixel_to_ndc.y * clip.w;
    out.clip_position = clip;
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // A cut is a line, not a fill, so it draws solid even on a translucent surface.
    let color = select(surface_style.color.rgb, surface_style.wire_color.rgb, surface_style.params.w > 0.5);
    return vec4<f32>(color, 1.0);
}
