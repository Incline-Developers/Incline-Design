struct PointCloudStyle {
    color: vec4<f32>,
    // x: screen-facing splat width in world units; y: draw `color` in place of
    // the instance's own colour (a selected cloud, or one whose colour channel
    // holds classification colours while that view is off); z: fade splats by
    // distance from the eye (fly mode).
    options: vec4<f32>,
    // Fixed cloud origin relative to the current floating scene origin.
    origin: vec4<f32>,
};
@group(1) @binding(0)
var<uniform> style: PointCloudStyle;

struct ColoredPointInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Distance from the section plane, flat: the splat is kept or dropped whole.
    @location(1) @interpolate(flat) section_offset: f32,
    // Model-space splat centre, for the cinematic lighting.
    @location(2) @interpolate(flat) world: vec3<f32>,
    // Share of a pixel's samples the splat's true size would cover, below 1
    // only once `MIN_SPLAT_PIXELS` has grown it (see `coverage_mask`).
    @location(3) @interpolate(flat) coverage: f32,
    // Stable per-point hash that picks which samples a partial splat lights.
    @location(4) @interpolate(flat) seed: u32,
};

// Fly-mode depth cue: splats within `DEPTH_CUE_NEAR` metres of the eye are
// washed towards white by `DEPTH_CUE_NEAR_WHITE`, ease down to
// `DEPTH_CUE_FAR_BRIGHTNESS` of their colour by `DEPTH_CUE_FAR` metres, and
// hold there beyond it.
const DEPTH_CUE_NEAR: f32 = 50.0;
const DEPTH_CUE_FAR: f32 = 500.0;
const DEPTH_CUE_NEAR_WHITE: f32 = 0.7;
const DEPTH_CUE_FAR_BRIGHTNESS: f32 = 0.35;

fn depth_cue(color: vec4<f32>, splat_center: vec3<f32>) -> vec4<f32> {
    if style.options.z < 0.5 {
        return color;
    }
    // The eye is in exaggerated display space; bring it back to model space
    // so the fade distance stays in true metres.
    let eye = camera.cam_position.xyz / vec3<f32>(1.0, 1.0, max(camera.cam_forward.w, 1.0e-6));
    let t = smoothstep(DEPTH_CUE_NEAR, DEPTH_CUE_FAR, distance(splat_center, eye));
    let near = mix(color.rgb, vec3<f32>(1.0), DEPTH_CUE_NEAR_WHITE);
    let far = color.rgb * DEPTH_CUE_FAR_BRIGHTNESS;
    return vec4<f32>(mix(near, far, t), color.a);
}

// Smallest on-screen splat width, in physical pixels.
const MIN_SPLAT_PIXELS: f32 = 1.0;
// Matches `MSAA_SAMPLE_COUNT` in rendering/graphics/mod.rs.
const SAMPLE_COUNT: u32 = 4u;

fn hash_u32(value: u32) -> u32 {
    var x = value;
    x = (x ^ (x >> 16u)) * 0x7feb352du;
    x = (x ^ (x >> 15u)) * 0x846ca68bu;
    return x ^ (x >> 16u);
}

fn point_seed(pos: vec3<f32>) -> u32 {
    let bits = bitcast<vec3<u32>>(pos);
    return hash_u32(bits.x ^ hash_u32(bits.y ^ hash_u32(bits.z)));
}

// A grown splat stands in for one smaller than a pixel, so it lights only the
// share of samples its true size would cover: sub-pixel points resolve to a
// partial pixel rather than all-or-nothing, and a dense cloud builds up
// towards solid instead of filling it outright. The count rounds up or down
// by the seed so the average is exact; the seed also rotates which samples,
// so neighbouring points in one pixel fill different ones. Unlike blending
// this is order-independent and depth-tests per sample, and it leaves alpha
// free for the cinematic eye-dome mask.
fn coverage_mask(coverage: f32, seed: u32) -> u32 {
    let jitter = f32(seed >> 8u) * (1.0 / 16777216.0);
    let count = min(u32(coverage * f32(SAMPLE_COUNT) + jitter), SAMPLE_COUNT);
    let bits = (1u << count) - 1u;
    let rotate = seed % SAMPLE_COUNT;
    let all = (1u << SAMPLE_COUNT) - 1u;
    return ((bits << rotate) | (bits >> (SAMPLE_COUNT - rotate))) & all;
}

fn expand_point(pos: vec3<f32>, color: vec4<f32>, vertex_index: u32) -> VertexOutput {
    let right = vertex_index == 1u || vertex_index == 3u;
    let up = vertex_index >= 2u;
    let corner = vec2<f32>(select(-1.0, 1.0, right), select(-1.0, 1.0, up));
    // The inverse view-projection's clip-X/Y directions are the camera's
    // screen-space axes in the same scene coordinates as the point data.
    let world_right = normalize((camera.inv_view_proj * vec4<f32>(1.0, 0.0, 0.0, 0.0)).xyz);
    let world_up = normalize((camera.inv_view_proj * vec4<f32>(0.0, 1.0, 0.0, 0.0)).xyz);
    let half_size = style.options.x * 0.5;
    let splat_center = pos + style.origin.xyz;
    let center_clip = camera.view_proj * vec4<f32>(splat_center, 1.0);
    // The offsets are screen-parallel, so they project linearly and leave
    // clip W unchanged: the splat's on-screen half-width in pixels follows
    // directly. Below `MIN_SPLAT_PIXELS` a splat can miss every pixel centre
    // and drop out, so distant ones are grown to that floor.
    let right_clip = camera.view_proj * vec4<f32>(world_right * half_size, 0.0);
    let up_clip = camera.view_proj * vec4<f32>(world_up * half_size, 0.0);
    let half_pixels = min(abs(right_clip.x) * camera.viewport.x, abs(up_clip.y) * camera.viewport.y) * 0.5 / max(abs(center_clip.w), 1.0e-6);
    let grow = max(1.0, MIN_SPLAT_PIXELS * 0.5 / max(half_pixels, 1.0e-6));
    var out: VertexOutput;
    let clip = center_clip + (corner.x * right_clip + corner.y * up_clip) * grow;
    out.clip_position = clip;
    out.color = depth_cue(color, splat_center);
    out.section_offset = section_plane_offset(splat_center);
    out.world = splat_center;
    out.coverage = 1.0 / (grow * grow);
    out.seed = point_seed(pos);
    return out;
}

@vertex
fn vs_colored(point: ColoredPointInput, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let color = select(point.color, style.color, style.options.y > 0.5);
    return expand_point(point.pos, color, vertex_index);
}

@vertex
fn vs_uncolored(@location(0) pos: vec3<f32>, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    return expand_point(pos, style.color, vertex_index);
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let mask = coverage_mask(in.coverage, in.seed);
    if outside_section_slab(in.section_offset) || mask == 0u {
        discard;
    }
    return FragmentOutput(shade_point(in.color, in.world, in.clip_position.xy), mask);
}
