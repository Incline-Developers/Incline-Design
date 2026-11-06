// Procedural infinite grid on the world XY plane at Z=0. A fullscreen
// triangle casts each pixel's ray at the plane, so the grid has no mesh edge
// and stays stable while panning or zooming.
//
// Everything here is measured from the eye. The ray is built from the
// camera's axes, it meets the plane at an offset from the eye, and the lines'
// phase about the eye arrives already reduced in f64. No f32 value is larger
// than the distance to the pixel's plane point, so the lines stay exact at
// any zoom and at survey coordinates in the millions.
//
// Each pixel picks its own powers of ten from its footprint on the plane, so
// a perspective view draws fine lines underfoot and coarser ones further off,
// crossfading between them, and the plane fades out towards its horizon.

struct GridUniform {
    // Clip transform of displayed points measured from the eye.
    eye_view_proj: mat4x4<f32>,
    // xyz: the view's right axis scaled to half the viewport's width, per
    // unit of depth in perspective or in metres in orthographic;
    // w: 1 for a perspective view, 0 for orthographic.
    ray_right: vec4<f32>,
    // xyz: the up axis scaled to half the viewport's height likewise;
    // w: the displayed plane's height above the eye (negative below it).
    ray_up: vec4<f32>,
    // xyz: the view direction; w: extra line width in pixels beyond the
    // one-pixel default.
    ray_forward: vec4<f32>,
    // x: base-10 exponent of the finest level in `phases`; y: the pixel
    // spacing at which a level is ten times too coarse to be the finest;
    // zw: the eye's world XY, which places the world axes.
    level_params: vec4<f32>,
    // Each level's line phase at the eye, two levels to a vector:
    // fract(eye.xy / spacing) for spacings 10^x, 10^(x+1), ...
    phases: array<vec4<f32>, 6>,
    minor_color: vec4<f32>,
    major_color: vec4<f32>,
    x_axis_color: vec4<f32>,
    y_axis_color: vec4<f32>,
};
@group(1) @binding(0)
var<uniform> grid: GridUniform;

// Levels in `phases`; GRID_LEVELS in rendering/graphics/mod.rs.
const LEVELS: f32 = 12.0;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // One oversized triangle covers the viewport without a vertex buffer.
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

fn level_phase(level: i32) -> vec2<f32> {
    let pair = grid.phases[level / 2];
    return select(pair.xy, pair.zw, (level & 1) == 1);
}

fn level_spacing(level: i32) -> f32 {
    return exp2((grid.level_params.x + f32(level)) * log2(10.0));
}

// Antialiased coverage of one level's two families of lines at `hit`, the
// plane point relative to the eye, whose pixel covers `footprint` metres of
// the plane along X and Y: x for the lines of constant X, y for constant Y.
// Each family is measured against its own footprint, so a family squashed
// by a grazing view keeps one-pixel lines.
fn level_coverage(hit: vec2<f32>, footprint: vec2<f32>, level: i32) -> vec2<f32> {
    let spacing = level_spacing(level);
    let cell = level_phase(level) + hit / spacing;
    let cells_per_pixel = max(footprint / spacing, vec2<f32>(1.0e-6));
    let pixel_distance = max(abs(fract(cell - 0.5) - 0.5) / cells_per_pixel - grid.ray_forward.w * 0.5, vec2<f32>(0.0));
    return vec2<f32>(1.0) - smoothstep(vec2<f32>(0.30), vec2<f32>(0.90), pixel_distance);
}

// How far each family's lines at `level` stand apart on screen, as an
// opacity: they fade as they crowd closer than a few pixels, thinning to
// nothing instead of piling into moire or a bright sheet. Measured against
// the line's own width, so thick lines thin out sooner.
fn level_density(footprint: vec2<f32>, level: i32) -> vec2<f32> {
    let spacing_pixels = level_spacing(level) / footprint / (1.0 + grid.ray_forward.w);
    return smoothstep(vec2<f32>(3.0), vec2<f32>(8.0), spacing_pixels);
}

fn clip_to_screen_homogeneous(clip: vec4<f32>) -> vec3<f32> {
    // Linear homogeneous form of NDC -> framebuffer pixels. Keeping W avoids
    // divisions near the perspective horizon.
    return vec3<f32>(
        camera.viewport.x * 0.5 * (clip.x + clip.w),
        camera.viewport.y * 0.5 * (clip.w - clip.y),
        clip.w,
    );
}

fn projected_axis_coverage(fragment_xy: vec2<f32>, base: vec3<f32>, direction: vec3<f32>) -> f32 {
    // A 3D line projects to a 2D homogeneous line. Transform one point and
    // its direction separately, then take their cross product; unlike
    // derivatives of the plane point, this remains a single stable line
    // where plane intersections approach infinity at the horizon.
    let point_h = clip_to_screen_homogeneous(grid.eye_view_proj * vec4<f32>(base, 1.0));
    let direction_h = clip_to_screen_homogeneous(grid.eye_view_proj * vec4<f32>(direction, 0.0));
    let line = cross(point_h, direction_h);
    let line_length = length(line.xy);
    if line_length < 1.0e-6 {
        return 0.0;
    }
    let pixel_distance = max(abs(dot(line, vec3<f32>(fragment_xy, 1.0))) / line_length - grid.ray_forward.w * 0.5, 0.0);
    return 1.0 - smoothstep(0.35, 0.95, pixel_distance);
}

@fragment
fn fs_main(@builtin(position) fragment_position: vec4<f32>) -> FragmentOutput {
    // `fragment_position` is relative to the render target, which for the
    // main window is the whole (toolbar-occluded) surface - rebase it to the
    // viewport sub-rect `camera.viewport` itself describes before using it as
    // a 0-based screen coordinate.
    let screen_xy = fragment_position.xy - camera.viewport_origin.xy;
    let viewport = max(camera.viewport.xy, vec2<f32>(1.0));
    let ndc = vec2<f32>(
        screen_xy.x / viewport.x * 2.0 - 1.0,
        1.0 - screen_xy.y / viewport.y * 2.0,
    );

    // The pixel's ray from the eye: fanned out in perspective, parallel and
    // offset sideways in orthographic.
    let perspective = grid.ray_right.w > 0.5;
    let lateral = grid.ray_right.xyz * ndc.x + grid.ray_up.xyz * ndc.y;
    let origin = select(lateral, vec3<f32>(0.0), perspective);
    let direction = select(grid.ray_forward.xyz, grid.ray_forward.xyz + lateral, perspective);
    let height = grid.ray_up.w;
    // A perspective ray reaches the plane only ahead of the eye. An
    // orthographic camera's position is nominal (its depth range runs either
    // side of it), so its rays meet the plane wherever they cross it.
    let crosses = abs(direction.z) > 1.0e-9;
    let t = select(0.0, (height - origin.z) / direction.z, crosses);
    let reaches = crosses && (!perspective || t > 0.0);
    let hit = origin + direction * t;

    // Taken before any discard, while the whole quad is still running.
    let dx = dpdx(hit.xy);
    let dy = dpdy(hit.xy);
    let footprint = max(abs(dx) + abs(dy), vec2<f32>(1.0e-30));
    if !reaches {
        discard;
    }

    // Fade the plane out as the ray grazes it. In perspective this is per
    // pixel - the grid stays under the eye and dissolves towards the
    // horizon, however level the view; in orthographic every ray is
    // parallel, so the whole plane fades as it turns edge-on.
    let grazing = abs(direction.z) / length(direction);
    // Orthographic: full above a 45 degree dip, gone below 5; the power
    // pulls the halfway point down to 15 degrees (sines 0.707, 0.087, 0.259).
    let ortho_fade = pow(smoothstep(0.087, 0.707, grazing), 0.414);
    let horizon_fade = select(ortho_fade, smoothstep(0.004, 0.12, grazing), perspective);
    if horizon_fade < 0.001 {
        discard;
    }

    // Pick this pixel's level from its footprint: the finest level whose
    // lines lie a tenth of `level_params.y` pixels apart or more, with the
    // next two above it as the coarser and major lines. The fraction
    // crossfades between levels, so the change is continuous across the
    // plane and as the view zooms.
    //
    // The footprint is the pixel's extent on the plane, which unlike the
    // per-axis `footprint` does not change as a plan view turns. Perspective
    // takes its longest side, so the levels coarsen with distance and the
    // lines running away stay as far apart as those across the view. In
    // orthographic the whole plane is squashed alike as it tilts, and there
    // the shortest side, which tilting leaves alone, keeps the spacing the
    // plan view has: stepping coarser with the tilt would read as the grid
    // zooming out while the view only turns.
    let longest = max(max(length(dx), length(dy)), 1.0e-30);
    let shortest = abs(dx.x * dy.y - dx.y * dy.x) / longest;
    let pixel_size = max(select(shortest, longest, perspective), 1.0e-30);
    let base = grid.level_params.x;
    let lod = clamp(log2(pixel_size * grid.level_params.y) / log2(10.0), base + 1.0, base + LEVELS - 1.001);
    let transition = fract(lod);
    let fine = i32(floor(lod) - base) - 1;

    // A family fades as a whole, its major lines with its minor ones, by how
    // far apart its finest visible lines stand; crossfaded like the levels
    // so it stays continuous through a change of level. Fading each level
    // by its own crowding would leave a squashed family as a few major lines
    // with nothing between them.
    let density = mix(level_density(footprint, fine), level_density(footprint, fine + 1), transition);
    let fine_lines = level_coverage(hit.xy, footprint, fine) * density * (grid.minor_color.a * (1.0 - transition));
    let coarse_lines = level_coverage(hit.xy, footprint, fine + 1) * density * mix(grid.major_color.a, grid.minor_color.a, transition);
    let major_lines = level_coverage(hit.xy, footprint, fine + 2) * density * grid.major_color.a;
    let fine_alpha = max(fine_lines.x, fine_lines.y);
    let coarse_alpha = max(coarse_lines.x, coarse_lines.y);
    let major_alpha = max(major_lines.x, major_lines.y);
    var alpha = max(max(fine_alpha, coarse_alpha), major_alpha);
    var premultiplied = mix(grid.minor_color.rgb, grid.major_color.rgb, clamp(max(coarse_alpha, major_alpha) / max(alpha, 1.0e-6), 0.0, 1.0)) * alpha;

    // Project the two infinite world axes directly, from points on them
    // level with the eye's own X or Y.
    let eye_xy = grid.level_params.zw;
    let x_axis_base = vec3<f32>(0.0, -eye_xy.y, height);
    let y_axis_base = vec3<f32>(-eye_xy.x, 0.0, height);
    let y_axis_alpha = projected_axis_coverage(screen_xy, y_axis_base, vec3<f32>(0.0, 1.0, 0.0)) * grid.y_axis_color.a;
    premultiplied = grid.y_axis_color.rgb * y_axis_alpha + premultiplied * (1.0 - y_axis_alpha);
    alpha = y_axis_alpha + alpha * (1.0 - y_axis_alpha);
    let x_axis_alpha = projected_axis_coverage(screen_xy, x_axis_base, vec3<f32>(1.0, 0.0, 0.0)) * grid.x_axis_color.a;
    premultiplied = grid.x_axis_color.rgb * x_axis_alpha + premultiplied * (1.0 - x_axis_alpha);
    alpha = x_axis_alpha + alpha * (1.0 - x_axis_alpha);

    premultiplied *= horizon_fade;
    alpha *= horizon_fade;
    if alpha < 0.001 {
        discard;
    }

    let clip = grid.eye_view_proj * vec4<f32>(hit, 1.0);
    if clip.w <= 0.0 {
        discard;
    }

    var out: FragmentOutput;
    out.color = vec4<f32>(premultiplied / alpha, alpha);
    // Keep the construction plane continuous even when the scene-fitted clip
    // range is tighter than the visible floor, including plane points
    // between the eye and the near plane. At far depth it still remains
    // behind every scene object because this renderer uses reversed-Z.
    out.depth = clamp(clip.z / clip.w, 0.0, 1.0);
    return out;
}
