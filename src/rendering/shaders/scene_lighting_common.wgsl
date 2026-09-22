// In-pass lighting and grading shared by standard and cinematic materials.

const SCENE_PI: f32 = 3.14159265;

/// Hemisphere light: the sky for a face that looks up, the ground's bounce for
/// one that looks down, blended by how far the face tips between them.
fn sky_light(normal: vec3<f32>, sky: vec3<f32>, ground: vec3<f32>) -> vec3<f32> {
    return mix(ground, sky, normal.z * 0.5 + 0.5);
}

/// Surfaces are two-sided sheets - a topography has no inside - so a face is
/// lit on whichever side the camera sees.
fn toward_viewer(normal: vec3<f32>) -> vec3<f32> {
    return select(-normal, normal, dot(normal, -camera.cam_forward.xyz) >= 0.0);
}

/// The lighting every lit material shares. `normal` shades; `geometric` is the
/// face's true orientation, which decides whether the sun can reach it at all
/// and which way a shadow lookup steps off the surface.
fn scene_light(albedo: vec3<f32>, normal: vec3<f32>, geometric: vec3<f32>, shininess: f32,
               visibility: f32, sun: vec3<f32>, sun_color: vec3<f32>, sky: vec3<f32>, ground: vec3<f32>) -> vec3<f32> {
    let view = -camera.cam_forward.xyz;
    let n_dot_l = max(dot(normal, sun), 0.0);
    let facing = smoothstep(0.0, 0.1, dot(geometric, sun));
    let sun_light = sun_color * (n_dot_l * facing * visibility);
    // Energy-normalised Blinn-Phong with Schlick's Fresnel: faint face-on, as
    // rock is, and a sheen at grazing angles that picks out a bench edge.
    let half_vector = normalize(sun + view);
    let fresnel = 0.04 + 0.96 * pow(1.0 - clamp(dot(half_vector, view), 0.0, 1.0), 5.0);
    let highlight = fresnel * (shininess + 8.0) / (8.0 * SCENE_PI) * pow(max(dot(normal, half_vector), 0.0), shininess);
    return albedo * (sun_light + sky_light(normal, sky, ground)) + sun_light * highlight;
}

// Use the same crease-aware normals in both quality modes.
fn surface_shading_normal(flat_normal: vec3<f32>, smooth_normal: vec3<f32>) -> vec3<f32> {
    let geometric = toward_viewer(normalize(flat_normal));
    var normal = geometric;
    if dot(smooth_normal, smooth_normal) > 0.25 {
        let shading = normalize(smooth_normal);
        // Both normals were oriented z >= 0 on the CPU; keep them on the same
        // side after `toward_viewer` has flipped the face.
        normal = select(-shading, shading, dot(shading, geometric) >= 0.0);
    }
    return normal;
}

// Applied inline in standard materials; cinematic applies it after AO in its
// composite. No HDR target or extra fullscreen pass is needed in standard mode.
fn grade_scene(color: vec3<f32>, exposure: f32) -> vec3<f32> {
    let exposed = color * exposure;
    let mapped = clamp((exposed * (2.51 * exposed + 0.03)) / (exposed * (2.43 * exposed + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
    return clamp(mix(mapped, mapped * mapped * (3.0 - 2.0 * mapped), 0.18), vec3<f32>(0.0), vec3<f32>(1.0));
}
