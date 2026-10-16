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
    // The hole's collar: a 2D section keeps the whole hole or none of it by
    // where this lies.
    @location(7) collar: vec3<f32>,
    // Bit 0: this piece starts its run (trace or disc); bit 1: it ends it.
    @location(8) caps: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    // Distance from the section plane; affine in position, so interpolation is exact.
    @location(2) section_offset: f32,
    // Model-space position, for the cinematic lighting's shadow lookups.
    @location(3) world: vec3<f32>,
    // 2D section only, in pixels: x across the ribbon from its centreline,
    // y along it from the piece's start.
    @location(4) ribbon: vec2<f32>,
    // 2D section only: x half the ribbon's width and y the piece's length in
    // pixels, z the caps, w 1 while selected.
    @location(5) @interpolate(flat) ribbon_size: vec4<f32>,
};

// A 2D section draws each hole as a flat ribbon on the plane: a white
// outline filled with the hole's colour.
const SECTION_HOLE_MIN_HALF_PIXELS: f32 = 5.0;
const SECTION_HOLE_OUTLINE_PIXELS: f32 = 1.5;
const SECTION_HOLE_OUTLINE_COLOR: vec3<f32> = vec3<f32>(1.0, 1.0, 1.0);

fn in_section_2d() -> bool {
    return camera.section_normal.w > 1.5;
}

fn onto_section(point: vec3<f32>) -> vec3<f32> {
    return point - camera.section_normal.xyz * section_plane_offset(point);
}

fn clip_pixels(clip: vec4<f32>) -> vec2<f32> {
    return clip.xy / max(abs(clip.w), 1.0e-6) * camera.viewport.xy * 0.5;
}

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

// The hole flattened onto the section plane as a ribbon: six vertices make
// the quad, the rest of the cylinder's collapse. Every piece of a hole
// collared inside the corridor is kept, however far the trace runs out of it,
// and every piece of one collared outside is dropped.
fn section_ribbon(instance: SegmentInput, vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let selected = selection_active(instance.selection_index);
    out.color = select(instance.color, selection.selection_color.rgb, selected);
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    out.section_offset = 0.0;
    out.ribbon = vec2<f32>(0.0);
    out.ribbon_size = vec4<f32>(0.0);
    if vertex_index >= 6u || abs(section_plane_offset(instance.collar)) > camera.section_plane.w {
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        out.world = vec3<f32>(0.0);
        return out;
    }
    let start = onto_section(instance.start_radius.xyz);
    let end = onto_section(instance.end_pad.xyz);
    let start_clip = camera.view_proj * vec4<f32>(start, 1.0);
    let end_clip = camera.view_proj * vec4<f32>(end, 1.0);
    let start_px = clip_pixels(start_clip);
    let end_px = clip_pixels(end_clip);
    let span = end_px - start_px;
    let length_px = length(span);
    let direction = select(vec2<f32>(0.0, -1.0), span / max(length_px, 1.0e-6), length_px > 1.0e-4);
    let across = vec2<f32>(-direction.y, direction.x);
    // The diameter on the horizontal scale, so vertical exaggeration cannot fatten a hole.
    let strike = normalize(cross(vec3<f32>(0.0, 0.0, 1.0), camera.section_normal.xyz));
    let scale = max(pixels_per_world(start_clip, camera.view_proj * vec4<f32>(strike, 0.0)), 1.0e-6);
    let half_px = max(max(instance.start_radius.w * scale, instance.end_pad.w * 0.5), SECTION_HOLE_MIN_HALF_PIXELS);
    // Two triangles: start-, start+, end+ and start-, end+, end-.
    let at_end = vertex_index == 2u || vertex_index == 4u || vertex_index == 5u;
    let side = select(-1.0, 1.0, vertex_index == 1u || vertex_index == 2u || vertex_index == 4u);
    // An open end runs on half a width into the next piece so a bend leaves
    // no notch; a capped end stops square.
    let capped = select((instance.caps & 1u) != 0u, (instance.caps & 2u) != 0u, at_end);
    let extend = select(half_px, 0.0, capped);
    let shift = across * side * half_px + direction * select(-1.0, 1.0, at_end) * extend;
    var clip = select(start_clip, end_clip, at_end);
    clip = vec4<f32>(clip.xy + shift / (camera.viewport.xy * 0.5) * clip.w, clip.zw);
    let pixel = select(start_px, end_px, at_end) + shift;
    out.position = clip;
    out.world = select(start, end, at_end);
    out.ribbon = vec2<f32>(side * half_px, dot(pixel - start_px, direction));
    out.ribbon_size = vec4<f32>(half_px, length_px, f32(instance.caps), select(0.0, 1.0, selected));
    return out;
}

fn section_ribbon_color(input: VertexOutput) -> vec4<f32> {
    let caps = u32(input.ribbon_size.z + 0.5);
    var edge = input.ribbon_size.x - abs(input.ribbon.x);
    if (caps & 1u) != 0u {
        edge = min(edge, input.ribbon.y);
    }
    if (caps & 2u) != 0u {
        edge = min(edge, input.ribbon_size.y - input.ribbon.y);
    }
    if edge < SECTION_HOLE_OUTLINE_PIXELS {
        return vec4<f32>(select(SECTION_HOLE_OUTLINE_COLOR, input.color, input.ribbon_size.w > 0.5), 1.0);
    }
    return vec4<f32>(input.color, 1.0);
}

@vertex
fn vs_main(instance: SegmentInput, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    if in_section_2d() {
        return section_ribbon(instance, vertex_index);
    }
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
    out.ribbon = vec2<f32>(0.0);
    out.ribbon_size = vec4<f32>(0.0);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if in_section_2d() {
        return section_ribbon_color(input);
    }
    if outside_section_slab(input.section_offset) {
        discard;
    }
    return shade_drill_hole(input.color, input.normal, input.world, input.position.xy);
}
