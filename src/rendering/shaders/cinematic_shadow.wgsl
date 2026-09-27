
// Depth-only render of the shadow casters from the sun's point of view.
//
// The camera prelude's uniform is bound to the *light's* matrix for this pass,
// so the vertex transform is the ordinary surface one: the chunk rebase that
// keeps positions in f32 range, then a view-projection. Triangulation surfaces
// are the only casters - block models draw through their own instanced cube
// expansion, and a volume-rendered one has no surface to rasterise at all.

struct ShadowChunk {
    offset: vec4<f32>,
};
@group(1) @binding(0)
var<uniform> chunk: ShadowChunk;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> @builtin(position) vec4<f32> {
    return camera.view_proj * vec4<f32>(model.position + chunk.offset.xyz, 1.0);
}
