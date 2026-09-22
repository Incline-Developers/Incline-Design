
// Screen-space ambient occlusion over the scene depth buffer.
//
// This is the effect that does most of the work: it is what puts contact
// darkening into the corner where a batter meets a berm, under a block model
// sitting on topography, and along every crest and toe. Nothing about it needs
// the scene re-drawn - the depth buffer the scene pass already filled is the
// only input - so it costs one half-resolution pass per changed frame and nothing
// once the view has settled on the cached scene image.

@group(2) @binding(0)
var scene_depth: texture_depth_multisampled_2d;

const SAMPLE_COUNT: i32 = 12;
// Golden-angle spiral: successive samples land as far from their predecessors
// as a fixed turn can put them, which keeps a low tap count from banding.
const GOLDEN_ANGLE: f32 = 2.39996323;
/// Scales the cosine-weighted sum into the 0..1 the curve below expects.
const OCCLUSION_GAIN: f32 = 4.0;

@fragment
fn fs_main(@builtin(position) half_position: vec4<f32>) -> @location(0) f32 {
    // Match the full-resolution depth sample used by the bilateral upsample.
    let pixel = min(vec2<i32>(half_position.xy) * 2, vec2<i32>(textureDimensions(scene_depth)) - 1);
    let position = vec4<f32>(vec2<f32>(pixel) + 0.5, 0.0, 1.0);
    let centre_depth = load_depth(pixel);
    // Nothing was drawn here: sky, and sky is never occluded.
    if centre_depth <= 0.0 || !inside_viewport(position.xy) {
        return 1.0;
    }

    let centre = world_from_depth(position.xy, centre_depth);
    let normal = reconstruct_normal(pixel, centre, centre_depth);

    // A radius fixed in metres would be a boulder-sized effect on a bench and
    // invisible from the top of the pit, so it is fixed in *pixels* instead
    // and converted back to metres per fragment - the world span of one pixel
    // at this depth, which holds under both projections. The clamp is what
    // stops a grazing view from reaching halfway across the scene.
    let pixel_world = length(world_from_depth(position.xy + vec2<f32>(1.0, 0.0), centre_depth) - centre);
    let radius = clamp(pixel_world * 32.0, cine.params.x, cine.params.x * 30.0);

    // Zoomed far enough out, the radius covers less than a pixel: there is no
    // longer any detail at that scale to occlude.
    if radius / max(pixel_world, 1.0e-9) < 1.5 {
        return 1.0;
    }

    // A fixed spatial rotation avoids temporal flicker without accumulation.
    let rotation = cinematic_noise(position.xy, 0.0) * 6.2831853;
    var occlusion = 0.0;
    for (var i = 0; i < SAMPLE_COUNT; i = i + 1) {
        let index = f32(i) + 0.5;
        let angle = index * GOLDEN_ANGLE + rotation;
        // sqrt spacing distributes the taps evenly over the disc's *area*
        // instead of crowding them at the centre.
        let distance = radius * sqrt(index / f32(SAMPLE_COUNT));
        // A cosine-weighted point above the surface: a disc tangent to the
        // face, lifted along the normal so the sample sits in the hemisphere.
        let tangent = normalize(cross(normal, select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(normal.z) > 0.9)));
        let bitangent = cross(normal, tangent);
        let offset = (tangent * cos(angle) + bitangent * sin(angle)) * distance + normal * (radius * 0.15);
        let sample_world = centre + offset;

        let projected = target_pixel_from_world(sample_world);
        let sample_pixel = vec2<i32>(projected.xy);
        if !inside_viewport(projected.xy) { continue; }
        let occluder_depth = load_depth(sample_pixel);
        if occluder_depth <= 0.0 {
            continue;
        }
        let occluder = world_from_depth(projected.xy, occluder_depth);
        let occluder_view_depth = view_depth(occluder);
        let sample_view_depth = view_depth(sample_world);

        // Real geometry has to sit in front of where the sample would have
        // been for anything to be occluding it. The bias keeps a flat face
        // from shadowing itself.
        if sample_view_depth - occluder_view_depth <= radius * 0.02 {
            continue;
        }

        // Weight by where the occluder sits relative to the surface rather
        // than counting it outright. A depth test alone reports occlusion for
        // any neighbour that happens to be nearer the camera, which on a face
        // seen close to edge-on is most of them - so a bench berm in a section
        // view would come out uniformly grey instead of dark only in its
        // corners. The cosine term is zero for an occluder lying in the
        // surface's own plane and one for something directly above it, which
        // is the difference between a flat wall and the toe of a batter.
        let to_occluder = occluder - centre;
        let separation = length(to_occluder);
        if separation < 1.0e-4 {
            continue;
        }
        let cosine = max(dot(normal, to_occluder / separation), 0.0);
        // Falls off to nothing at the sampling radius, so an occluder at the
        // edge of the neighbourhood cannot pop in and out as the view moves.
        // Quadratic rather than linear: a linear falloff spends most of its
        // weight near the edge of the neighbourhood, where the cosine term is
        // already small, and the toe of a batter comes out at a tenth of the
        // occlusion it should have.
        let ratio = separation / radius;
        occlusion += cosine * clamp(1.0 - ratio * ratio, 0.0, 1.0);
    }

    // Gain, then a curve: the raw cosine-weighted sum is small even where the
    // geometry is deeply concave, because only a few of the samples in a
    // hemisphere can be occluded at once. The curve then pushes shallow,
    // broad occlusion back towards white and leaves the deep corners dark -
    // contact shading rather than an overall dimming.
    let raw = clamp(occlusion / f32(SAMPLE_COUNT) * OCCLUSION_GAIN, 0.0, 1.0);
    let visibility = 1.0 - pow(raw, 0.8) * cine.params.y;
    return clamp(visibility, 0.0, 1.0);
}
