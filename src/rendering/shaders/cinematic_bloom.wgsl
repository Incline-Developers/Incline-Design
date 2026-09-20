// Bloom: isolate what is genuinely bright, blur it wide and cheaply at a
// quarter of the resolution, and hand it back to the composite to add.
//
// Three entry points share one bind group - the bright pass reads the scene,
// each blur pass reads the other half of a ping-pong pair.

@group(0) @binding(0)
var source: texture_2d<f32>;
@group(0) @binding(1)
var source_sampler: sampler;

struct BloomParams {
    // x: bright-pass threshold in linear luminance; y: the knee over which
    // pixels fade in above it. zw: unused.
    threshold: vec4<f32>,
};
@group(0) @binding(2)
var<uniform> bloom: BloomParams;

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

fn source_texel() -> vec2<f32> {
    return 1.0 / vec2<f32>(textureDimensions(source, 0));
}

@fragment
fn fs_bright(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    // The target is a quarter of the source, so a destination pixel centre
    // maps straight onto a source position; four bilinear taps around it
    // cover the 4x4 source footprint without a wide kernel.
    let texel = source_texel();
    let uv = position.xy * texel * 4.0;
    var total = vec3<f32>(0.0);
    total += textureSampleLevel(source, source_sampler, uv + vec2<f32>(-1.0, -1.0) * texel, 0.0).rgb;
    total += textureSampleLevel(source, source_sampler, uv + vec2<f32>(1.0, -1.0) * texel, 0.0).rgb;
    total += textureSampleLevel(source, source_sampler, uv + vec2<f32>(-1.0, 1.0) * texel, 0.0).rgb;
    total += textureSampleLevel(source, source_sampler, uv + vec2<f32>(1.0, 1.0) * texel, 0.0).rgb;
    let color = total * 0.25;

    // A soft knee rather than a hard cut: a hard threshold makes the bloom
    // pop on and off as the camera moves across a highlight.
    let brightness = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let weight = smoothstep(bloom.threshold.x, bloom.threshold.x + bloom.threshold.y, brightness);
    return vec4<f32>(color * weight, 1.0);
}

// Nine-tap Gaussian, separated into the two passes below. Held in a function
// `var` rather than a module `const` so the loop counter can index it.
fn blur(position: vec2<f32>, direction: vec2<f32>) -> vec4<f32> {
    var weights = array<f32, 5>(0.2270270, 0.1945946, 0.1216216, 0.0540541, 0.0162162);
    let texel = source_texel();
    let uv = position * texel;
    var total = textureSampleLevel(source, source_sampler, uv, 0.0).rgb * weights[0];
    for (var i = 1; i < 5; i = i + 1) {
        let offset = direction * texel * f32(i);
        total += textureSampleLevel(source, source_sampler, uv + offset, 0.0).rgb * weights[i];
        total += textureSampleLevel(source, source_sampler, uv - offset, 0.0).rgb * weights[i];
    }
    return vec4<f32>(total, 1.0);
}

@fragment
fn fs_blur_horizontal(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(position.xy, vec2<f32>(1.0, 0.0));
}

@fragment
fn fs_blur_vertical(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(position.xy, vec2<f32>(0.0, 1.0));
}
