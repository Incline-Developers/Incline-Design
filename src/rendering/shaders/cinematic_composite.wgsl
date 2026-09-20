
// The one pass that produces the image the rest of the app sees. It reads the
// scene exactly as the ordinary pipeline drew it and adds what a screen-space
// pass can add on top: sun shadows, ambient occlusion, bloom and a grade.
//
// Working here rather than inside each material shader is deliberate. The
// scene is drawn by a dozen pipelines - surfaces, blocks, drill traces, point
// splats, strokes - and every one of them would otherwise need the sun, the
// shadow map and the occlusion term plumbed into it. Reconstructing position
// from the depth buffer gets all of them at once, and leaves the shading that
// engineers read the geometry by completely untouched when cinematic is off.

@group(2) @binding(0)
var scene_color: texture_2d<f32>;
@group(2) @binding(1)
var occlusion: texture_2d<f32>;
@group(2) @binding(2)
var bloom: texture_2d<f32>;
@group(2) @binding(3)
var bloom_sampler: sampler;
@group(2) @binding(4)
var scene_depth: texture_depth_multisampled_2d;
@group(2) @binding(5)
var shadow_map: texture_depth_2d;
@group(2) @binding(6)
var shadow_sampler: sampler_comparison;

/// What the surface keeps where the sun does not reach it. Shadows are tinted
/// cool rather than crushed to black, which is both what happens outdoors
/// under an open sky and what keeps a shadowed bench readable.
const SHADOW_TINT: vec3<f32> = vec3<f32>(0.42, 0.48, 0.62);
/// Occlusion is a contact term, so it tints cooler still and goes darker.
const OCCLUSION_TINT: vec3<f32> = vec3<f32>(0.38, 0.43, 0.55);

/// Fraction of the sun reaching this point, filtered over a 3x3 neighbourhood
/// so the shadow edge is soft rather than a staircase of shadow-map texels.
fn sun_visibility(world: vec3<f32>) -> f32 {
    if cine.params.w < 0.5 {
        return 1.0;
    }
    let clip = cine.light_view_proj * vec4<f32>(world, 1.0);
    if clip.w <= 0.0 {
        return 1.0;
    }
    let ndc = clip.xyz / clip.w;
    // Outside the fitted light volume there is nothing that could have cast,
    // so the sun is unobstructed rather than the surface being in shadow.
    if any(abs(ndc.xy) > vec2<f32>(1.0)) || ndc.z <= 0.0 || ndc.z >= 1.0 {
        return 1.0;
    }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_map, 0));

    var total = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            total += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + offset, ndc.z);
        }
    }
    return total / 9.0;
}

/// Ambient occlusion, blurred as it is read. The per-pixel rotation in the
/// occlusion pass turns its low tap count into noise rather than banding, and
/// noise is what a blur is for. Weighting by depth keeps the blur from pulling
/// occlusion across a silhouette onto whatever lies behind it.
fn filtered_occlusion(pixel: vec2<i32>, centre_depth: f32) -> f32 {
    var total = 0.0;
    var weight_total = 0.0;
    for (var y = -2; y <= 2; y = y + 1) {
        for (var x = -2; x <= 2; x = x + 1) {
            let tap = pixel + vec2<i32>(x, y);
            let depth = textureLoad(scene_depth, tap, 0);
            if depth <= 0.0 {
                continue;
            }
            // Reversed-Z is far from linear, but the comparison only has to
            // separate "same surface" from "different surface" locally.
            let weight = exp(-abs(depth - centre_depth) / max(centre_depth * 0.002, 1.0e-7));
            total += textureLoad(occlusion, tap, 0).r * weight;
            weight_total += weight;
        }
    }
    if weight_total <= 0.0 {
        return textureLoad(occlusion, pixel, 0).r;
    }
    return total / weight_total;
}

/// Filmic roll-off (the Narkowicz ACES fit). The scene arrives display-referred
/// rather than as true radiance, so this is doing less than it would in a full
/// HDR pipeline - it lifts the mid-tones and stops the sun glare and the bloom
/// from clipping flat where they overlap.
fn tonemap(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(position.xy);
    let scene = textureLoad(scene_color, pixel, 0);

    // The scene only fills the canvas sub-rect; the margin is clear colour
    // that the panels are painted over. Leave it exactly as it was, or the
    // grade would show through the rounded corners of the panel chrome.
    if !inside_viewport(position.xy) {
        return scene;
    }

    let depth = textureLoad(scene_depth, pixel, 0);
    let bloom_uv = position.xy / vec2<f32>(textureDimensions(scene_color, 0));
    let halo = textureSampleLevel(bloom, bloom_sampler, bloom_uv, 0.0).rgb * cine.grade.x;

    // Depth of zero means nothing was drawn here: this is the background, and
    // it stays the flat clear colour the ordinary renderer leaves it. The
    // grade is deliberately not applied to it - the background is a chosen UI
    // colour, not part of the photograph. Bloom still lands on it, or the halo
    // around bright geometry would be sliced off at the silhouette.
    if depth <= 0.0 {
        return vec4<f32>(scene.rgb + halo, scene.a);
    }

    var color = scene.rgb;
    let world = world_from_depth(position.xy, depth);
    let lit = sun_visibility(world);
    let visibility = filtered_occlusion(pixel, depth);

    color *= mix(OCCLUSION_TINT, vec3<f32>(1.0), visibility);
    color *= mix(SHADOW_TINT, vec3<f32>(1.0), mix(1.0, lit, cine.params.z));
    // A little of the sun's own colour where it lands, which is what makes
    // the lit ground read as warm against the cool shadow.
    color *= mix(vec3<f32>(1.0), cine.sun_color.rgb, 0.14 * lit);

    color = tonemap((color + halo) * cine.grade.y);

    // Gentle contrast and saturation. Small amounts: this is the difference
    // between "rendered" and "photographed", and more than a touch of either
    // starts to misrepresent the colours a block model or a grade ramp is
    // encoding. There is no vignette - it would darken the geometry towards
    // the corners while the background beside it stayed flat.
    color = clamp(mix(vec3<f32>(luminance(color)), color, 1.12), vec3<f32>(0.0), vec3<f32>(1.0));
    color = clamp(mix(color, color * color * (3.0 - 2.0 * color), 0.18), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, scene.a);
}
