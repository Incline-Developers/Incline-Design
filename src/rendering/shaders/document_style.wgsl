// Editor styling of document geometry, prefixed to the document shaders
// after the constants `scene::document_style::shader_prelude` generates.
// One flag word per style slot; see rendering/scene/document_style.rs.

@group(1) @binding(0)
var<storage, read> document_style: array<u32>;
// x: 1 while the static chunks redraw only their highlighted members.
@group(1) @binding(1)
var<uniform> document_style_pass: vec4<u32>;

fn document_style_flags(style: u32) -> u32 {
    let slot = style & STYLE_SLOT_MASK;
    if slot >= arrayLength(&document_style) {
        return 0u;
    }
    return document_style[slot];
}

fn document_style_highlighted(flags: u32) -> bool {
    return (flags & (STYLE_SELECTED | STYLE_HOVER)) != 0u;
}

// Whether this pass skips the primitive entirely.
fn document_style_culled(style: u32, flags: u32) -> bool {
    if document_style_pass.x == 1u && !document_style_highlighted(flags) {
        return true;
    }
    return (style & STROKE_SELECTION_ONLY) != 0u && (flags & STYLE_SELECTED) == 0u;
}

fn document_styled_color(color: vec4<f32>, flags: u32) -> vec4<f32> {
    if (flags & STYLE_SELECTED) != 0u {
        return SELECTION_COLOR;
    }
    if (flags & STYLE_HOVER) != 0u {
        return HOVER_COLOR;
    }
    if (flags & STYLE_TRANSLUCENT) != 0u {
        return vec4<f32>(color.rgb, color.a * TRANSLUCENT_ALPHA);
    }
    return color;
}

// A clip position outside the view volume: every triangle made of it is culled.
const CULLED_POSITION: vec4<f32> = vec4<f32>(2.0, 2.0, 2.0, 1.0);
