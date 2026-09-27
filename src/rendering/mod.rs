pub(crate) mod camera;
pub(crate) mod color;
pub(crate) mod geometry;
pub(crate) mod graphics;
pub(crate) mod pick;
pub(crate) mod query;
pub(crate) mod scene;
pub(crate) mod section_grid;
pub(crate) mod snap;
pub(crate) mod text;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub(crate) struct Vertex {
    pub(crate) pos: [f32; 3],
    pub(crate) color: [f32; 4],
    /// Document style slot the shader restyles this vertex by (selection,
    /// hover, translucency); `scene::document_style::STYLE_SLOT_NONE` for
    /// geometry that is never restyled, such as text.
    pub(crate) style: u32,
}

/// Vertex for triangulation surfaces - colour comes from a per-draw uniform.
/// `pos` is chunk-origin-relative (see `CachedSurfaceChunk`) so f32 stays
/// precise far from the scene origin. `normal` is the flat face normal of the
/// triangle this vertex provokes (`@interpolate(flat)` in the shader), already
/// oriented with `z >= 0` for the two-sided surface lighting.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub(crate) struct SurfaceVertex {
    pub(crate) pos: [f32; 3],
    pub(crate) normal: [f32; 3],
}

/// One instanced block: local-space axis-aligned bounds plus grade. The
/// vertex shader expands this into a full cube and applies the model's
/// local->scene rotation, so a block costs 32 B instead of eight vertices plus
/// indices. Grade is a normalized 0..1 value, or a negative sentinel when no
/// numeric colour variable is active / the block is hidden.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub(crate) struct BlockInstance {
    pub(crate) lower: [f32; 3],
    pub(crate) grade: f32,
    pub(crate) upper: [f32; 3],
    pub(crate) _pad: f32,
}

/// One stroke primitive, expanded by `stroke.wgsl` into a screen-space quad:
/// a world-space line, a round disc (a join, cap or marker), or a
/// screen-aligned bar anchored at a world point. `style` carries the
/// document style slot in its low bits and the `STROKE_*` flags of
/// `scene::document_style` in its high bits.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub(crate) struct StrokeInstance {
    pub(crate) start: [f32; 3],
    /// Half the stroke width, or a disc's radius, in physical pixels.
    pub(crate) half_width_px: f32,
    /// The line's far end. A disc ignores it; a screen-aligned bar reads its
    /// `xy` as the bar's half-extent in physical pixels.
    pub(crate) end: [f32; 3],
    pub(crate) style: u32,
    pub(crate) color: [f32; 4],
}

impl StrokeInstance {
    /// The world positions the primitive covers: both ends of a line, or
    /// the anchor twice for a disc or screen-aligned bar.
    pub(crate) fn world_ends(&self) -> ([f32; 3], [f32; 3]) {
        if self.style & (scene::document_style::STROKE_ROUND | scene::document_style::STROKE_SCREEN_AXIS) != 0 {
            (self.start, self.start)
        } else {
            (self.start, self.end)
        }
    }

    /// Drawn only while its owner is selected; never picked.
    pub(crate) fn selection_only(&self) -> bool {
        self.style & scene::document_style::STROKE_SELECTION_ONLY != 0
    }
}
