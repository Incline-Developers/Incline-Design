// A rain simulation's water: clear over the shallows, dark in the pools,
// rippled along the flow, foaming where it runs fast, and ringed by the
// drops while it rains.
//
// The pattern is drawn at whatever size reads on screen: ripples a metre
// across up close, and the same ripples at twice, four times, ... the size
// as the camera pulls back, each size fading into the next. So the flow can
// be seen running from across the pit as well as from the bench.

struct WaterUniform {
    // xyz: the mesh's anchor relative to the scene origin; w: the ripple
    // clock, seconds.
    anchor: vec4<f32>,
    // x: how far from the earlier instant to the later, 0 to 1; y: 1 while
    // rain is falling; z: the lift off the ground, metres.
    params: vec4<f32>,
};
@group(1) @binding(0) var<uniform> water: WaterUniform;

struct VertexInput {
    // The ground, relative to the anchor.
    @location(0) position: vec3<f32>,
    // The water's depth at the earlier instant and the later.
    @location(1) depth: vec2<f32>,
    // Its velocity in plan at each, metres a second.
    @location(2) flow_from: vec2<f32>,
    @location(3) flow_to: vec2<f32>,
    // How far from the earlier instant to the later the water came here.
    @location(4) arrival: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // Relative to the scene origin, before vertical exaggeration.
    @location(0) world: vec3<f32>,
    @location(1) depth: f32,
    @location(2) flow: vec2<f32>,
    @location(3) section_offset: f32,
    // Plan position relative to the anchor: what the pattern is laid out
    // on, so it stays put when the scene's origin moves.
    @location(4) plan: vec2<f32>,
};

// The shallowest water drawn, and the depth by which it is fully there.
const FILM_DEPTH: f32 = 0.003;
const SHORE_DEPTH: f32 = 0.03;
// How long a copy of the carried pattern lives, seconds.
const CARRY_PERIOD: f32 = 2.5;
// The fastest the pattern is carried, in ripple cells a second: faster and
// it smears between one copy and the next.
const CARRY_LIMIT: f32 = 2.5;
// How many pixels the finest ripple has to span before it is drawn.
const RIPPLE_PIXELS: f32 = 12.0;
// And a rain ring's cell.
const RING_PIXELS: f32 = 60.0;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // Nothing changes here until the water comes; then it goes the whole way
    // to the later instant in what is left.
    let blend = clamp((water.params.x - in.arrival) / max(1.0 - in.arrival, 1.0e-3), 0.0, 1.0);
    let depth = mix(in.depth.x, in.depth.y, blend);
    let world = water.anchor.xyz + in.position + vec3<f32>(0.0, 0.0, max(depth, 0.0) + water.params.z);
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.depth = depth;
    out.flow = mix(in.flow_from, in.flow_to, blend);
    out.section_offset = section_plane_offset(world);
    out.plan = in.position.xy;
    return out;
}

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let b = mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let c = mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let d = mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 3; i++) {
        sum += amp * vnoise(q);
        q = q * 2.03 + vec3<f32>(11.0, 7.0, 3.0);
        amp *= 0.5;
    }
    return sum;
}

// Height of the small waves at a point of the surface.
fn ripples(q: vec2<f32>, t: f32) -> f32 {
    let a = vnoise(vec3<f32>(q * 1.1, t * 0.5));
    let b = vnoise(vec3<f32>(q * 2.9 + vec2<f32>(7.0, 3.0), t * 0.9 + 4.0));
    return 0.65 * a + 0.35 * b;
}

// The waves' slope in xy and the foam's lace in z, at one size of the
// pattern (`scale` metres to a ripple cell), carried along by the flow: the
// pattern is drawn twice, each copy sliding downstream and fading out as
// the other fades in, so it moves without ever stretching.
fn carried(plan: vec2<f32>, flow: vec2<f32>, scale: f32, t: f32) -> vec3<f32> {
    // A larger pattern is carried faster over the ground, or it would seem
    // to stand still from far off.
    var run = flow / sqrt(scale);
    run *= min(1.0, CARRY_LIMIT / max(length(run), 1.0e-5));
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 2; i++) {
        let phase = fract(t / CARRY_PERIOD + 0.5 * f32(i));
        let q = plan / scale - run * (phase - 0.5) * CARRY_PERIOD + vec2<f32>(13.0, 5.0) * f32(i);
        let e = 0.12;
        let h = ripples(q, t);
        let slope = vec2<f32>(ripples(q + vec2<f32>(e, 0.0), t) - h, ripples(q + vec2<f32>(0.0, e), t) - h) / e;
        let lace = fbm3(vec3<f32>(q * 1.7 + vec2<f32>(4.0, 9.0), 2.0 + t * 0.2));
        sum += vec3<f32>(slope, lace) * (1.0 - abs(2.0 * phase - 1.0));
    }
    return sum;
}

// Rain on the water, at one size: each drop that lands sends out a ring,
// spreading and dying as it goes. The landings are cells of a jittered
// grid, each coming back after its own while somewhere new in its cell.
// Returns the surface's slope.
fn rain_rings(plan: vec2<f32>, scale: f32, t: f32) -> vec2<f32> {
    let cell_size = 0.42;
    let q = plan / (scale * cell_size);
    let home = floor(q);
    var slope = vec2<f32>(0.0);
    for (var j = 0; j < 9; j++) {
        let id = home + vec2<f32>(f32(j % 3 - 1), f32(j / 3 - 1));
        let period = 0.55 + 0.6 * hash3(vec3<f32>(id, 11.0));
        let phase = t / period + hash3(vec3<f32>(id, 13.0));
        let cycle = floor(phase);
        if (hash3(vec3<f32>(id, cycle)) > 0.7) {
            continue;
        }
        let age = fract(phase) * period;
        let centre = id + vec2<f32>(hash3(vec3<f32>(id, cycle + 0.3)), hash3(vec3<f32>(id, cycle + 0.7)));
        let off = (q - centre) * cell_size;
        let d = length(off);
        let r = 0.015 + 0.32 * age;
        let width = 0.025 + 0.03 * age;
        let x = (d - r) / width;
        let amp = 1.6 * exp(-age * 3.2) / (1.0 + 10.0 * r);
        // A crest between two troughs, as its slope.
        slope += off / max(d, 1.0e-3) * (-amp * sin(x * 3.5) * exp(-x * x));
    }
    return slope;
}

// The sky a ray leaving the surface sees: pale at the horizon, blue
// overhead, and the pit's own walls for a ray that leaves low.
fn sky_toward(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(dir.z, 0.0, 1.0);
    let sky = mix(vec3<f32>(0.74, 0.82, 0.90), vec3<f32>(0.22, 0.44, 0.80), pow(up, 0.6));
    return mix(vec3<f32>(0.34, 0.31, 0.27), sky, smoothstep(0.03, 0.30, dir.z));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Derivatives before anything that can end the invocation.
    let exaggeration = max(camera.cam_forward.w, 1.0e-6);
    let shown = in.world * vec3<f32>(1.0, 1.0, exaggeration);
    let facet = cross(dpdx(shown), dpdy(shown));
    let footprint = max(length(dpdx(in.plan)), length(dpdy(in.plan)));
    if outside_section_slab(in.section_offset) {
        discard;
    }
    let depth = max(in.depth, 0.0);
    // Nothing at all where the water runs out to no depth.
    let edge = smoothstep(FILM_DEPTH, SHORE_DEPTH, depth);
    if edge <= 0.0 {
        discard;
    }
    let t = water.anchor.w;
    let speed = length(in.flow);

    // The water's own surface, level on a pool and lying along the ground
    // where it runs, seen from above.
    var level = facet / max(length(facet), 1.0e-20);
    if level.z < 0.0 {
        level = -level;
    }
    var view = -camera.cam_forward.xyz;
    if camera.cam_position.w > 0.5 {
        view = normalize(camera.cam_position.xyz - shown);
    }

    // The two sizes of the pattern either side of what reads at this
    // distance, and how far from the smaller to the larger.
    let size = log2(max(footprint * RIPPLE_PIXELS, 1.0));
    let step_down = floor(size);
    let between = smoothstep(0.0, 1.0, size - step_down);
    let wave = mix(carried(in.plan, in.flow, exp2(step_down), t), carried(in.plan, in.flow, exp2(step_down + 1.0), t), between);

    // Still pools are glassy; the faster the flow the rougher the surface.
    // A film a few centimetres deep has no waves to speak of, and waves
    // drawn large for a far camera stand lower than their size.
    let chop = (0.035 + 0.16 * smoothstep(0.2, 2.2, speed)) * smoothstep(0.02, 0.3, depth) / (1.0 + 0.3 * step_down);
    var slope = wave.xy * chop;
    if water.params.y > 0.5 {
        let ring_size = log2(max(footprint * RING_PIXELS, 1.0));
        let ring_down = floor(ring_size);
        let rings = mix(rain_rings(in.plan, exp2(ring_down), t), rain_rings(in.plan, exp2(ring_down + 1.0), t), smoothstep(0.0, 1.0, ring_size - ring_down));
        slope += rings * 0.6 * smoothstep(0.004, 0.03, depth);
    }
    let normal = normalize(level - vec3<f32>(slope, 0.0));
    let facing = max(dot(normal, view), 0.02);
    var fresnel = 0.02 + 0.98 * pow(1.0 - facing, 5.0);

    let sun = STANDARD_SUN;
    let light = STANDARD_SUN_COLOR * max(sun.z, 0.0) * 0.9 + STANDARD_SKY_COLOR;

    // What is under the surface, made to tell a puddle from a pond at a
    // glance: a film barely tints the ground, a few centimetres are a pale
    // wash, half a metre is clear turquoise with the ground plain through
    // it, and the ground goes on fading until some metres down. Two rates of
    // fading together, so neither the first centimetres nor the last metres
    // are all one colour; and the water's own colour darkens with its depth,
    // not with how thick the fading has made it. The slant of the view counts
    // for little, so the same depth reads the same from any side.
    let path = depth / max(facing, 0.6);
    let through = (0.5 * exp(-path * vec3<f32>(4.0, 2.6, 2.2)) + 0.5 * exp(-path * vec3<f32>(0.9, 0.5, 0.4))) * 0.97;
    let body = mix(vec3<f32>(0.16, 0.52, 0.56), vec3<f32>(0.01, 0.05, 0.22), 1.0 - exp(-depth * 0.7)) * light;
    // A film mirrors little: it is wet ground more than a sheet of water.
    fresnel *= 0.25 + 0.75 * smoothstep(0.01, 0.15, depth);

    // What is on it: the sky, and the sun broken into glints by the waves.
    let mirror = reflect(-view, normal);
    let glance = max(dot(mirror, sun), 0.0);
    let mirrored = sky_toward(mirror) + STANDARD_SUN_COLOR * (pow(glance, 600.0) * 18.0 + pow(glance, 60.0) * 0.25);

    // Foam where the water runs fast, or fast and shallow, and a thin line
    // of it along the shore of anything moving.
    let rush = clamp(smoothstep(1.5, 3.0, speed) + 0.8 * smoothstep(0.9, 1.8, speed) * (1.0 - smoothstep(0.15, 0.6, depth)), 0.0, 1.0);
    var foam = smoothstep(0.62 - 0.32 * rush, 0.75, wave.z) * rush;
    foam = max(foam, 0.5 * (1.0 - smoothstep(0.02, 0.10, depth)) * smoothstep(0.42, 0.6, wave.z) * smoothstep(0.25, 0.7, speed));
    foam *= smoothstep(0.02, 0.08, depth);
    let foam_color = vec3<f32>(0.90, 0.91, 0.88) * light;

    // Laid over the ground behind as light added and light held back.
    let seen = (through.r + through.g + through.b) / 3.0;
    var alpha = 1.0 - seen * (1.0 - fresnel);
    var color = body * (1.0 - through) * (1.0 - fresnel) + mirrored * fresnel;
    color = mix(color, foam_color, foam);
    alpha = mix(alpha, 1.0, foam) * edge;
    color *= edge;
    // Graded as the ground around it is.
    return vec4<f32>(grade_scene(color / max(alpha, 1.0e-3), STANDARD_EXPOSURE) * alpha, alpha);
}
