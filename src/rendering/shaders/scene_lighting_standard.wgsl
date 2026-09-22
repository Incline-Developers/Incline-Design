// Standard mode shares cinematic's inexpensive material lighting, but assumes
// full sun visibility. Grading happens before the ordinary 8-bit attachment;
// there are no shadow lookups, HDR attachments, or extra rendering passes.
// STANDARD_* constants are supplied by SceneShading::lit_shader from the same
// settings used to upload cinematic's lighting uniforms.
fn standard_light(color: vec3<f32>, normal: vec3<f32>, geometric: vec3<f32>, shininess: f32) -> vec3<f32> {
    return grade_scene(scene_light(color, normal, geometric, shininess, 1.0,
        STANDARD_SUN, STANDARD_SUN_COLOR, STANDARD_SKY_COLOR, STANDARD_GROUND_COLOR), STANDARD_EXPOSURE);
}

fn shade_surface(color: vec4<f32>, flat_normal: vec3<f32>, smooth_normal: vec3<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    let geometric = surface_geometric_normal(flat_normal, world);
    let normal = surface_shading_normal(geometric, smooth_normal);
    return vec4<f32>(standard_light(color.rgb, normal, geometric, 24.0), color.a);
}

fn shade_block(color: vec4<f32>, normal: vec3<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(standard_light(color.rgb, normal, normal, 16.0), color.a);
}

// Drill traces are annotations in both modes, and keep their existing lighting.
fn shade_drill_hole(color: vec3<f32>, normal: vec3<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    let light = normalize(vec3<f32>(0.35, 0.45, 0.82));
    let diffuse = 0.38 + 0.62 * abs(dot(normalize(normal), light));
    return vec4<f32>(color * diffuse, 1.0);
}

fn shade_point(color: vec4<f32>, world: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    return color;
}
