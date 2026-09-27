// Scene lighting for the cinematic view, prefixed to the material shaders in
// place of `scene_lighting_standard.wgsl` and defining the same functions.
// Follows `scene_shadow_map.wgsl`, which
// provide `sun_visibility` and bind the parameter block.
//
// Output is linear radiance, free to go above 1: the half-float target keeps
// it, and the composite exposes and tone maps
// the lot. Albedo is the colour the ordinary view would have shown, so a grade
// ramp or a surface colour keeps its hue - light scales it, and tints it only
// as far as a warm sun and a cool sky do.

fn cinematic_light(albedo: vec3<f32>, normal: vec3<f32>, geometric: vec3<f32>, world: vec3<f32>, pixel: vec2<f32>, shininess: f32) -> vec3<f32> {
    var visibility = 0.0;
    if dot(normal, cine.sun.xyz) > 0.0 && dot(geometric, cine.sun.xyz) > 0.0 {
        visibility = sun_visibility(world, geometric, pixel);
    }
    return scene_light(albedo, normal, geometric, shininess, visibility, cine.sun.xyz, cine.sun_color.rgb, cine.sky_color.rgb, cine.ground_color.rgb);
}

// --- Materials -------------------------------------------------------------

fn shade_surface(color: vec4<f32>, flat_normal: vec3<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    let normal = surface_geometric_normal(flat_normal, world);
    return vec4<f32>(cinematic_light(color.rgb, normal, normal, world, pixel, 24.0), color.a);
}

// `normal` already faces the camera, and a block's faces are flat.
fn shade_block(color: vec4<f32>, normal: vec3<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(cinematic_light(color.rgb, normal, normal, world, pixel, 16.0), color.a);
}

fn shade_drill_hole(color: vec3<f32>, normal: vec3<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    let facing = toward_viewer(normalize(normal));
    return vec4<f32>(cinematic_light(color, facing, facing, world, pixel, 48.0), 1.0);
}

/// Scanned colour already has the day's light in it, so a point is not lit
/// again - its depth comes from eye-dome lighting instead, which finds it by
/// the zero this writes to the target's alpha (see `SceneShading::blend`).
fn shade_point(color: vec4<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(color.rgb, 0.0);
}
