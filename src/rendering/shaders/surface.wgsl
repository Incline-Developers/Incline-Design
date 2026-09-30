struct SurfaceStyle {
    color: vec4<f32>,
    // x: raster blend opacity; y: wireframe width in pixels, 0 for none;
    // z, w: scene-Z range to shade across, equal when depth shading is off.
    params: vec4<f32>,
    wire_color: vec4<f32>,
    pattern_color: vec4<f32>,
    // x: hatch pattern (0 clear, 1 slashes, 2 crosses); remaining lanes reserved.
    pattern: vec4<f32>,
};
@group(1) @binding(0)
var<uniform> surface_style: SurfaceStyle;

// Scene-origin-relative offset of this chunk's local origin. Vertices are
// stored relative to their chunk's AABB centre so interpolated positions keep
// f32 precision far from the scene origin.
struct SurfaceChunk {
    offset: vec4<f32>,
    pattern_phase: vec4<f32>,
};
@group(2) @binding(0)
var<uniform> chunk: SurfaceChunk;

@group(3) @binding(0)
var raster_texture: texture_2d<f32>;
@group(3) @binding(1)
var raster_sampler: sampler;
struct RasterMap {
    uv_x: vec4<f32>,
    uv_y: vec4<f32>,
};
@group(3) @binding(2)
var<uniform> raster_map: RasterMap;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Flat face normal from the triangle's provoking vertex, unit length and
    // pre-oriented with z >= 0 on the CPU. Using a stored normal instead of
    // dpdx/dpdy of the position avoids derivative cancellation noise when the
    // per-pixel position delta approaches the f32 ULP (static-like speckle,
    // worst close-up in fly mode).
    @location(1) @interpolate(flat) normal: vec3<f32>,
    @location(2) surface_xy: vec2<f32>,
    // Distance from the section plane; affine in position, so interpolation is exact.
    @location(3) section_offset: f32,
    // Scene-relative height, for the depth ramp. `pattern_position` cannot
    // stand in: its phase is deliberately wrapped to the hatch period.
    @location(4) scene_z: f32,
    // Model-space position, for the cinematic lighting's shadow lookups.
    @location(5) world: vec3<f32>,
    @location(6) pattern_position: vec3<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.color = surface_style.color;
    out.normal = model.normal;
    let scene_position = model.position + chunk.offset.xyz;
    out.world = scene_position;
    out.surface_xy = scene_position.xy;
    // Reduce each chunk origin in f64 on the CPU; interpolate only local
    // coordinates so close-up derivatives do not quantize at mine coordinates.
    out.pattern_position = model.position + chunk.pattern_phase.xyz;
    out.scene_z = scene_position.z;
    out.clip_position = camera.view_proj * vec4<f32>(scene_position, 1.0);
    out.section_offset = section_plane_offset(scene_position);
    return out;
}

// Coverage of the triangle's own edges, `width` pixels wide centred on each
// edge (half falls in each neighbour), antialiased over one pixel.
// `surface_shader_body` (scene_pipelines.rs) removes the `barycentric` input and supplies
// a constant with no edge in reach where the device lacks barycentrics; the
// cache then draws instanced edges instead and leaves `width` at zero.
fn wire_coverage(barycentric: vec3<f32>, width: f32) -> f32 {
    let pixels = barycentric / max(fwidth(barycentric), vec3<f32>(1e-6));
    let nearest = min(pixels.x, min(pixels.y, pixels.z));
    return select(0.0, 1.0 - smoothstep(width * 0.5 - 0.5, width * 0.5 + 0.5, nearest), width > 0.0);
}

@fragment
fn fs_main(in: VertexOutput, @builtin(barycentric) barycentric: vec3<f32>) -> @location(0) vec4<f32> {
    // Derivatives before anything that can end the invocation.
    let wire = wire_coverage(barycentric, surface_style.params.y) * surface_style.wire_color.a;
    // Project onto the dominant world plane, including vertical slab walls.
    var hatch_uv = in.pattern_position.xy;
    let axis = abs(in.normal);
    if axis.x > axis.y && axis.x > axis.z {
        hatch_uv = in.pattern_position.yz;
    } else if axis.y > axis.z {
        hatch_uv = in.pattern_position.xz;
    }
    let diagonal = vec2<f32>(hatch_uv.x + hatch_uv.y, hatch_uv.x - hatch_uv.y) / 5.0;
    let distance = abs(fract(diagonal + 0.5) - 0.5);
    let dx = dpdx(diagonal);
    let dy = dpdy(diagonal);
    let pixels = max(sqrt(dx * dx + dy * dy), vec2<f32>(1e-7));
    let pixel_distance = distance / pixels;
    // Constant on-screen stroke: a half-pixel solid core fading out by 1.25
    // pixels either side, so zoom changes its world width, not its apparent
    // one. Fade the whole pattern out once a period falls below a few pixels,
    // at grazing angles or far zoom, instead of producing moire.
    let fade = vec2<f32>(1.0) - smoothstep(vec2<f32>(0.2), vec2<f32>(0.5), pixels);
    let strokes = (vec2<f32>(1.0) - smoothstep(vec2<f32>(0.25), vec2<f32>(1.25), pixel_distance)) * fade;
    if outside_section_slab(in.section_offset) {
        discard;
    }
    var surface_color = in.color;
    // Depth ramp: lowest ground near black, highest at full colour, so a
    // plan view still reads as a pit. Raster and hatch composite over it.
    let depth_span = surface_style.params.w - surface_style.params.z;
    if depth_span > 0.0 {
        let depth = clamp((in.scene_z - surface_style.params.z) / depth_span, 0.0, 1.0);
        surface_color = vec4<f32>(surface_color.rgb * mix(0.25, 1.0, depth), surface_color.a);
    }
    if surface_style.params.x > 0.0 {
        let uv = vec2<f32>(
            dot(raster_map.uv_x.xyz, vec3<f32>(in.surface_xy, 1.0)),
            dot(raster_map.uv_y.xyz, vec3<f32>(in.surface_xy, 1.0)),
        );
        let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
        if inside {
            // The branch is fragment-varying, so implicit derivatives are not
            // available uniformly on WebGPU. Raster textures have one mip.
            let image = textureSampleLevel(raster_texture, raster_sampler, uv, 0.0);
            surface_color = vec4<f32>(
                mix(surface_color.rgb, image.rgb, clamp(surface_style.params.x * image.a, 0.0, 1.0)),
                surface_color.a,
            );
        }
    }
    if surface_style.pattern.x > 0.5 {
        var coverage = strokes.x;
        if surface_style.pattern.x > 1.5 {
            coverage = max(coverage, strokes.y);
        }
        surface_color = vec4<f32>(mix(surface_color.rgb, surface_style.pattern_color.rgb,
            coverage * surface_style.pattern_color.a), surface_color.a);
    }
    let shaded = shade_surface(surface_color, in.normal, in.world, in.clip_position.xy);
    // Unlit, like the instanced edges it replaces.
    return vec4<f32>(mix(shaded.rgb, surface_style.wire_color.rgb, wire), max(shaded.a, wire));
}
