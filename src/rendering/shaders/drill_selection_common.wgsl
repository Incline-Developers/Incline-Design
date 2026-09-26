// Per-dataset selection bitset: one bit per hole (indices [0, hole_count)),
// then one bit per tie-in (indices [hole_count, hole_count + tie count)).
// See `drill_hole_cache::selection_bits_for`, which packs this buffer.
//
// A uniform, not a storage buffer: a vertex-stage storage buffer is not
// available on every WebGPU canvas this runs on, and the drill pipelines
// simply failed to create where it is not. A uniform array needs a 16-byte
// stride, so four bitset words ride in each element, and the whole binding
// is the 64 KiB WebGPU guarantees: a 32-byte header and 4094 elements.
struct Selection {
    selection_color: vec4<f32>,
    whole: u32,
    hole_count: u32,
    word_count: u32,
    _pad: u32,
    bits: array<vec4<u32>, 4094>,
};
@group(1) @binding(0)
var<uniform> selection: Selection;

// `whole` deliberately never reaches a tie index (only `index < hole_count`
// takes the fast path) - a whole-dataset selection highlights every hole but
// leaves tie colouring to its own bit, matching `HoleSelection::contains_tie`
// on the CPU side, which never consults `whole` either.
fn selection_active(index: u32) -> bool {
    if selection.whole != 0u && index < selection.hole_count {
        return true;
    }
    let word = index / 32u;
    if word >= selection.word_count {
        return false;
    }
    // The four words packed into one element are picked apart by hand rather
    // than by indexing the vector with a value, which not every backend
    // compiles the same way for a uniform.
    let packed = selection.bits[word / 4u];
    let lane = word % 4u;
    var bits = packed.x;
    if lane == 1u {
        bits = packed.y;
    } else if lane == 2u {
        bits = packed.z;
    } else if lane == 3u {
        bits = packed.w;
    }
    return (bits & (1u << (index % 32u))) != 0u;
}
