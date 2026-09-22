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

// Surface normals and positions are stored before vertical exaggeration.
// Transform the view direction into that space before choosing the visible
// side. Perspective rays vary across the screen; the central camera direction
// can classify a visible grazing face as its underside.
fn surface_geometric_normal(flat_normal: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let scale = vec3<f32>(1.0, 1.0, max(camera.cam_forward.w, 1.0e-6));
    var view = -camera.cam_forward.xyz / scale;
    if camera.cam_position.w > 0.5 {
        view = camera.cam_position.xyz / scale - world;
    }
    let normal = normalize(flat_normal);
    return select(-normal, normal, dot(normal, view) >= 0.0);
}

// Use the same crease-aware normals in both quality modes. Orient the smooth
// normal with the visible geometric side, not independently toward the eye.
fn surface_shading_normal(geometric: vec3<f32>, smooth_normal: vec3<f32>) -> vec3<f32> {
    var normal = geometric;
    if dot(smooth_normal, smooth_normal) > 0.25 {
        let shading = normalize(smooth_normal);
        normal = select(-shading, shading, dot(shading, geometric) >= 0.0);
    }
    return normal;
}

const GRADE_START_COMPRESSION: f32 = 0.76;
const GRADE_DESATURATION: f32 = 0.15;

// Applied inline in standard materials; cinematic applies it after AO in its
// composite. No HDR target or extra fullscreen pass is needed in standard mode.
//
// Khronos PBR Neutral: a surface's colour is data, so hue and saturation pass
// through untouched below the compression knee, and only highlights roll off
// toward white. A per-channel filmic curve would skew a grade ramp away from
// its legend.
fn grade_scene(color: vec3<f32>, exposure: f32) -> vec3<f32> {
    var mapped = max(color * exposure, vec3<f32>(0.0));
    let low = min(mapped.r, min(mapped.g, mapped.b));
    mapped -= select(0.04, low - 6.25 * low * low, low < 0.08);
    let peak = max(mapped.r, max(mapped.g, mapped.b));
    if peak < GRADE_START_COMPRESSION {
        return mapped;
    }
    let d = 1.0 - GRADE_START_COMPRESSION;
    let new_peak = 1.0 - d * d / (peak + d - GRADE_START_COMPRESSION);
    mapped *= new_peak / peak;
    let g = 1.0 - 1.0 / (GRADE_DESATURATION * (peak - new_peak) + 1.0);
    return mix(mapped, vec3<f32>(new_peak), g);
}
