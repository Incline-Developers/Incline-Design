// Grade the scene with depth-aware half-resolution AO and point-cloud eye-dome lighting.

@group(2) @binding(0)
var scene_color: texture_2d<f32>;
@group(2) @binding(1)
var occlusion: texture_2d<f32>;
@group(2) @binding(4)
var scene_depth: texture_depth_multisampled_2d;

/// Occlusion is a contact term, so it tints cool - the colour of the sky it
/// is shutting out - rather than crushing to black.
const OCCLUSION_TINT: vec3<f32> = vec3<f32>(0.38, 0.43, 0.55);
/// Eye-dome lighting compares each point against neighbours this far away.
const EYE_DOME_RADIUS_PIXELS: f32 = 1.5;
/// Cap on one neighbour's depth step, in pixel widths, so a silhouette
/// against the far wall of the pit is a dark outline and not a hole.
const EYE_DOME_MAX_STEP: f32 = 16.0;

/// Ambient occlusion, blurred as it is read. The per-pixel rotation in the
/// occlusion pass turns its low tap count into noise rather than banding, and
/// noise is what a blur is for. Weighting by depth keeps the blur from pulling
/// occlusion across a silhouette onto whatever lies behind it.
fn filtered_occlusion(pixel: vec2<i32>, centre_depth: f32) -> f32 {
    var total = 0.0;
    var weight_total = 0.0;
    let size = vec2<i32>(textureDimensions(occlusion));
    let depth_size = vec2<i32>(textureDimensions(scene_depth));
    let centre = pixel / 2;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let tap = clamp(centre + vec2<i32>(x, y), vec2<i32>(0), size - 1);
            let full_pixel = min(tap * 2, depth_size - 1);
            if !inside_viewport(vec2<f32>(full_pixel) + 0.5) { continue; }
            let depth = textureLoad(scene_depth, full_pixel, 0);
            if depth <= 0.0 { continue; }
            let delta = (vec2<f32>(full_pixel) - vec2<f32>(pixel)) * 0.5;
            let spatial = exp(-dot(delta, delta) * 0.5);
            let weight = spatial * exp(-abs(depth - centre_depth) / max(centre_depth * 0.002, 1.0e-7));
            total += textureLoad(occlusion, tap, 0).r * weight;
            weight_total += weight;
        }
    }
    // No compatible neighbour: leave the silhouette unoccluded.
    if weight_total <= 1.0e-6 { return 1.0; }
    return total / weight_total;
}

/// Eye-dome lighting (Boucheny): shade each point by how far its neighbours
/// stand in front of it. A point cloud has no surfaces to light, and drawn
/// flat it is a coloured smear; this is what gives it its edges back - every
/// bench crest, every boulder - from the depth buffer alone. Steps are
/// measured in pixel widths at the point's depth, so the effect looks the same
/// zoomed in or out and under either projection.
fn eye_dome(position: vec2<f32>, depth: f32) -> f32 {
    let centre = world_from_depth(position, depth);
    let centre_depth = view_depth(centre);
    let pixel_world = max(length(world_from_depth(position + vec2<f32>(1.0, 0.0), depth) - centre), 1.0e-6);
    var response = 0.0;
    for (var tap = 0; tap < 8; tap = tap + 1) {
        let angle = f32(tap) * 0.78539816;
        let neighbour = position + vec2<f32>(cos(angle), sin(angle)) * EYE_DOME_RADIUS_PIXELS;
        let neighbour_depth = textureLoad(scene_depth, vec2<i32>(neighbour), 0);
        // An empty neighbour is the sky behind a silhouette: the strongest
        // edge there is.
        var step = EYE_DOME_MAX_STEP;
        if neighbour_depth > 0.0 {
            let rise = centre_depth - view_depth(world_from_depth(neighbour, neighbour_depth));
            step = clamp(rise / pixel_world, 0.0, EYE_DOME_MAX_STEP);
        }
        response += step;
    }
    return exp(-response / 8.0 * cine.params.w);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(position.xy);
    let scene = textureLoad(scene_color, pixel, 0);

    // The scene only fills the canvas sub-rect; the margin is clear colour
    // that the panels are painted over. Leave it exactly as it was, or the
    // grade would show through the rounded corners of the panel chrome.
    if !inside_viewport(position.xy) {
        return vec4<f32>(scene.rgb, 1.0);
    }

    let depth = textureLoad(scene_depth, pixel, 0);
    if depth <= 0.0 {
        return vec4<f32>(scene.rgb, 1.0);
    }

    var color = scene.rgb;
    color *= mix(OCCLUSION_TINT, vec3<f32>(1.0), filtered_occlusion(pixel, depth));

    // The scene's alpha marks point clouds with a zero (see
    // `SceneShading::blend`); a resolved edge sample is part point, part not.
    let point_coverage = clamp(1.0 - scene.a, 0.0, 1.0);
    if point_coverage > 0.0 && cine.params.w > 0.0 {
        color *= mix(1.0, eye_dome(position.xy, depth), point_coverage);
    }

    color = grade_scene(color, cine.grade.y);

    return vec4<f32>(color, 1.0);
}
