struct SegmentInput {
    // xyz: segment start, relative to the scene origin. w: world radius.
    @location(0) start_radius: vec4<f32>,
    // xyz: segment end. w: minimum screen diameter in physical pixels.
    @location(1) end_pad: vec4<f32>,
    @location(2) color: vec3<f32>,
    // This instance's slot in `selection.bits` - a hole index, or
    // `hole_count + tie index` for a tie-in.
    @location(3) selection_index: u32,
    // x: least on-screen length of the whole disc along the string, in
    // physical pixels, 0 for anything that is not a disc. y: the disc's rank
    // among its hole's discs, thinnest highest, in (0, 1].
    @location(4) axial: vec2<f32>,
    // The first and last point of the whole disc this piece is part of:
    // survey stations can cut a disc into pieces, and a thin disc is
    // lengthened as one. Zero outside discs.
    @location(5) disc_start: vec3<f32>,
    @location(6) disc_end: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    // Distance from the section plane; affine in position, so interpolation is exact.
    @location(2) section_offset: f32,
    // Model-space position, for the cinematic lighting's shadow lookups.
    @location(3) world: vec3<f32>,
};

// How far toward the eye, in pixels' worth of distance, the thinnest of a
// hole's lengthened discs is moved. Depth only: the move runs along the
// line of sight, so nothing shifts on screen.
const DISC_NUDGE_PIXELS: f32 = 1.0;

struct RingFrame {
    right: vec3<f32>,
    up: vec3<f32>,
};

fn ring(angle: f32) -> vec2<f32> { return vec2<f32>(cos(angle), sin(angle)); }

// The two directions a ring around `axis` is laid out in.
fn ring_frame(axis: vec3<f32>) -> RingFrame {
    let helper = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(axis.z) > 0.9);
    let right = normalize(cross(helper, axis));
    return RingFrame(right, cross(axis, right));
}

// Screen pixels one world unit along a direction covers at a point, both
// already in clip space (the direction with w = 0).
fn pixels_per_world(point_clip: vec4<f32>, direction_clip: vec4<f32>) -> f32 {
    let safe_w = max(abs(point_clip.w), 1.0e-6);
    let ndc_per_world = (direction_clip.xy * point_clip.w - point_clip.xy * direction_clip.w) / (safe_w * safe_w);
    return length(ndc_per_world * camera.viewport.xy * 0.5);
}

// One projected scale for a whole ring. A radial direction can point into
// the camera and have a zero screen-space derivative; at least one
// cross-section axis remains visible, so the larger scale gives a stable
// minimum.
fn ring_pixels_per_world(point_clip: vec4<f32>, right_clip: vec4<f32>, up_clip: vec4<f32>) -> f32 {
    return max(max(pixels_per_world(point_clip, right_clip), pixels_per_world(point_clip, up_clip)), 1.0e-6);
}

@vertex
fn vs_main(instance: SegmentInput, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let tau = 6.28318530718;
    var radial = vec2<f32>(0.0, 0.0);
    var axial = 0.0;
    var local_normal = vec3<f32>(0.0, 0.0, 0.0);
    if vertex_index < 72u {
        let side = vertex_index / 6u;
        let corner = vertex_index % 6u;
        let a = ring(f32(side) * tau / 12.0);
        let b = ring(f32(side + 1u) * tau / 12.0);
        radial = select(select(a, b, corner == 1u || corner == 2u || corner == 4u), a, corner == 3u || corner == 5u);
        // Two triangles per wall quad:
        // a0, b0, b1 and a0, b1, a1.
        axial = select(0.0, 1.0, corner == 2u || corner == 4u || corner == 5u);
        local_normal = vec3<f32>(normalize(radial), 0.0);
    } else {
        let cap_vertex = vertex_index - 72u;
        let top = (cap_vertex % 6u) >= 3u;
        let corner = cap_vertex % 3u;
        let side = cap_vertex / 6u;
        let a = ring(f32(side) * tau / 12.0);
        let b = ring(f32(side + 1u) * tau / 12.0);
        radial = select(select(a, b, corner == 1u), vec2<f32>(0.0), corner == 0u);
        axial = select(0.0, 1.0, top);
        local_normal = vec3<f32>(0.0, 0.0, select(-1.0, 1.0, top));
    }
    var start = instance.start_radius.xyz;
    var end = instance.end_pad.xyz;

    // A disc shorter on screen than its floor grows about its own middle
    // until it reaches it, so a thin ply still shows from far away. The
    // whole disc is measured and every piece of it stretched by the same
    // factor along the disc's chord, so its pieces stay joined and none
    // grows on its own past the disc's ends. It never grows longer than it
    // is drawn wide: seen end-on its face already shows. Grown discs can
    // overlap their neighbours, so each moves toward the eye by its rank
    // and by how far it grew: the thinner wins, the same way every frame.
    var nudge_pixels = 0.0;
    let chord = instance.disc_end - instance.disc_start;
    let span_length = length(chord);
    if instance.axial.x > 0.0 && span_length > 0.0 {
        let chord_axis = chord / span_length;
        let middle = (instance.disc_start + instance.disc_end) * 0.5;
        let middle_clip = camera.view_proj * vec4<f32>(middle, 1.0);
        let axial_pixels_per_world = pixels_per_world(middle_clip, camera.view_proj * vec4<f32>(chord_axis, 0.0));
        let span_pixels = span_length * axial_pixels_per_world;
        if span_pixels < instance.axial.x {
            let chord_frame = ring_frame(chord_axis);
            let chord_ring_scale = ring_pixels_per_world(middle_clip, camera.view_proj * vec4<f32>(chord_frame.right, 0.0), camera.view_proj * vec4<f32>(chord_frame.up, 0.0));
            let drawn_radius = max(instance.start_radius.w, instance.end_pad.w * 0.5 / chord_ring_scale);
            let longest = max(span_length, 2.0 * drawn_radius);
            let wanted = instance.axial.x / max(axial_pixels_per_world, 1.0e-6);
            let stretch = min(wanted, longest) / span_length - 1.0;
            start = start + chord_axis * (dot(start - middle, chord_axis) * stretch);
            end = end + chord_axis * (dot(end - middle, chord_axis) * stretch);
            nudge_pixels = DISC_NUDGE_PIXELS * instance.axial.y * (1.0 - span_pixels / instance.axial.x);
        }
    }

    let axis = normalize(end - start);
    let frame = ring_frame(axis);
    let center = start + (end - start) * axial;
    let radial_direction = frame.right * radial.x + frame.up * radial.y;
    var radius = instance.start_radius.w;
    var world = center + radial_direction * radius;
    var clip: vec4<f32>;
    if instance.end_pad.w > 0.0 || nudge_pixels > 0.0 {
        // The clip position is built once from the ring centre's: the ring
        // offset and the nudge are directions, so they project linearly.
        let center_clip = camera.view_proj * vec4<f32>(center, 1.0);
        let right_clip = camera.view_proj * vec4<f32>(frame.right, 0.0);
        let up_clip = camera.view_proj * vec4<f32>(frame.up, 0.0);
        let scale = ring_pixels_per_world(center_clip, right_clip, up_clip);
        if instance.end_pad.w > 0.0 {
            radius = max(radius, (instance.end_pad.w * 0.5) / scale);
        }
        world = center + radial_direction * radius;
        clip = center_clip + (right_clip * radial.x + up_clip * radial.y) * radius;
        if nudge_pixels > 0.0 {
            // Positions are stored before vertical exaggeration and the eye
            // after it, so the line of sight is taken back into model space.
            let exaggeration = vec3<f32>(1.0, 1.0, max(camera.cam_forward.w, 1.0e-6));
            var toward_eye = -camera.cam_forward.xyz / exaggeration;
            if camera.cam_position.w > 0.5 {
                toward_eye = camera.cam_position.xyz / exaggeration - world;
            }
            clip = clip + camera.view_proj * vec4<f32>(normalize(toward_eye) * (nudge_pixels / scale), 0.0);
        }
    } else {
        clip = camera.view_proj * vec4<f32>(world, 1.0);
    }
    let normal = normalize(frame.right * local_normal.x + frame.up * local_normal.y + axis * local_normal.z);
    var out: VertexOutput;
    out.position = clip;
    out.normal = normal;
    out.color = select(instance.color, selection.selection_color.rgb, selection_active(instance.selection_index));
    out.section_offset = section_plane_offset(world);
    out.world = world;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if outside_section_slab(input.section_offset) {
        discard;
    }
    return shade_drill_hole(input.color, input.normal, input.world, input.position.xy);
}
