// Sun visibility from two cached shadow-map cascades. Cascade 0 is fitted to what the camera sees and resolves a bench's
// shadow up close; cascade 1 covers the whole scene, so anything beyond the
// first still casts. Binds the cinematic parameter block for the lighting
// prelude that follows.

@group(0) @binding(1)
var<uniform> cine: CinematicParams;
@group(0) @binding(2)
var shadow_maps: texture_depth_2d_array;
@group(0) @binding(3)
var shadow_sampler: sampler_comparison;

/// The fraction of the sun reaching `world` according to one cascade, or -1
/// when the point lies outside the part of it trusted (`edge`, in NDC).
fn cascade_visibility(cascade: i32, world: vec3<f32>, geometric: vec3<f32>, pixel: vec2<f32>, edge: f32) -> f32 {
    // Step the lookup off the surface along its own normal, a texel and a
    // half: what stops a grazing batter shadowing itself, without the large
    // constant bias that would lift every shadow off its caster.
    let texel_world = cine.cascade_texel[cascade];
    let clip = cine.cascades[cascade] * vec4<f32>(world + geometric * texel_world * 1.5, 1.0);
    let ndc = clip.xyz / clip.w;
    if any(abs(ndc.xy) > vec2<f32>(edge)) || ndc.z <= 0.0 || ndc.z >= 1.0 {
        return -1.0;
    }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_maps, 0).xy);
    // Four bilinear comparison taps: stable soft edges without stochastic
    // noise or still-frame accumulation (each tap filters four depth tests).
    var lit = 0.0;
    for (var y = 0; y < 2; y = y + 1) {
        for (var x = 0; x < 2; x = x + 1) {
            let offset = (vec2<f32>(f32(x), f32(y)) - 0.5) * 1.5 * texel;
            lit += textureSampleCompareLevel(shadow_maps, shadow_sampler, uv + offset, cascade, ndc.z);
        }
    }
    return lit * 0.25;
}

fn sun_visibility(world: vec3<f32>, geometric: vec3<f32>, pixel: vec2<f32>) -> f32 {
    if cine.cascade_texel.z < 0.5 {
        return 1.0;
    }
    // Blend only in the outer band; most pixels still pay for one cascade.
    // Use the receiver position without normal bias so adjacent faces agree
    // on the transition even when their normals differ at a bench crest.
    let clip = cine.cascades[0] * vec4<f32>(world, 1.0);
    let ndc = clip.xyz / clip.w;
    let edge = max(abs(ndc.x), abs(ndc.y));
    var near = -1.0;
    if edge < 0.96 {
        near = cascade_visibility(0, world, geometric, pixel, 0.98);
        if near >= 0.0 && edge <= 0.75 { return near; }
    }
    let far = cascade_visibility(1, world, geometric, pixel, 1.0);
    let broad = select(far, 1.0, far < 0.0);
    if near < 0.0 { return broad; }
    return mix(near, broad, smoothstep(0.75, 0.96, edge));
}
