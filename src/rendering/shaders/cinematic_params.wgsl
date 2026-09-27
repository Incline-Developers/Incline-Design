// The cinematic view's parameter block, shared by the lit scene shaders (which
// bind it at group 0 beside the camera) and the post chain (group 1). Mirrors
// `CinematicUniform` in rendering/graphics/cinematic.rs.
//
// Positions throughout are *model space*: scene-origin-relative metres before
// vertical exaggeration, which is what vertex positions are and what the
// camera's inverse view-projection reconstructs depth into. Exaggeration lives
// in the camera matrix alone.

struct CinematicParams {
    // Model space -> shadow clip for each cascade: 0 is fitted to what the
    // camera sees, 1 to the whole scene.
    cascades: array<mat4x4<f32>, 2>,
    // x, y: one shadow-map texel in metres for cascades 0 and 1. z: 1 when the
    // shadow maps hold this frame's casters. w: unused.
    cascade_texel: vec4<f32>,
    // xyz: unit vector toward the sun. w: unused.
    sun: vec4<f32>,
    // rgb: the sun's radiance - colour times intensity.
    sun_color: vec4<f32>,
    // rgb: sky light reaching a face that looks straight up.
    sky_color: vec4<f32>,
    // rgb: light bounced back up onto a face that looks straight down.
    ground_color: vec4<f32>,
    // x: minimum occlusion radius (m). y: occlusion strength. z: unused.
    // w: eye-dome lighting strength.
    params: vec4<f32>,
    // x: unused. y: exposure. z: unused. w: vertical exaggeration.
    grade: vec4<f32>,
};

// Stable spatial noise for the AO kernel; no frame-dependent rotation.
fn cinematic_noise(pixel: vec2<f32>, channel: f32) -> f32 {
    let shifted = pixel + channel * 7.0 * vec2<f32>(5.588238, 3.717432);
    return fract(52.9829189 * fract(dot(shifted, vec2<f32>(0.06711056, 0.00583715))));
}
