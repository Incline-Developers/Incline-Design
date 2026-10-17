//! The section's surface lines as their own cached stroke stream.

use glam::DVec3;

use crate::rendering::{
    StrokeInstance,
    geometry::{DrawContext, draw_line},
};

/// Width of a surface line on the section, in logical pixels.
pub(crate) const SECTION_SURFACE_LINE_WIDTH: f32 = 2.0;

/// Where one shown surface cuts the section, in its colour.
pub(crate) struct SurfaceLines {
    pub(crate) lines: Vec<Vec<DVec3>>,
    pub(crate) color: [f32; 4],
}

/// The cut lines and their strokes. The strokes are built when the lines or
/// their colours change, or the scene origin or scale moves, and not on an
/// editor overlay rebuild.
#[derive(Default)]
pub(crate) struct SectionSurfaceStrokes {
    surfaces: Vec<SurfaceLines>,
    strokes: Vec<StrokeInstance>,
    built_for: Option<(DVec3, u32)>,
    dirty: bool,
}

impl SectionSurfaceStrokes {
    pub(crate) fn set(&mut self, surfaces: Vec<SurfaceLines>) {
        self.surfaces = surfaces;
        self.dirty = true;
    }

    /// Recolour in place, one colour per surface in the order of `set`.
    pub(crate) fn recolor(&mut self, colors: impl Iterator<Item = [f32; 4]>) {
        for (surface, color) in self.surfaces.iter_mut().zip(colors) {
            surface.color = color;
        }
        self.dirty = true;
    }

    /// Rebuild the strokes if anything they depend on changed. True when
    /// they were rebuilt and the GPU copy is stale.
    pub(crate) fn refresh(&mut self, scene_origin: DVec3, scale_factor: f32) -> bool {
        let built_for = Some((scene_origin, scale_factor.to_bits()));
        if !self.dirty && self.built_for == built_for {
            return false;
        }
        self.strokes.clear();
        let mut unused_fill_vertices = Vec::new();
        let mut unused_fill_indices = Vec::new();
        let mut ctx = DrawContext::unstyled(&mut self.strokes, &mut unused_fill_vertices, &mut unused_fill_indices, scene_origin, scale_factor);
        for surface in &self.surfaces {
            for line in &surface.lines {
                for pair in line.windows(2) {
                    draw_line(&mut ctx, pair[0], pair[1], SECTION_SURFACE_LINE_WIDTH, surface.color);
                }
            }
        }
        self.built_for = built_for;
        self.dirty = false;
        true
    }

    pub(crate) fn surfaces(&self) -> &[SurfaceLines] {
        &self.surfaces
    }

    pub(crate) fn strokes(&self) -> &[StrokeInstance] {
        &self.strokes
    }

    pub(crate) fn strokes_mut(&mut self) -> &mut Vec<StrokeInstance> {
        &mut self.strokes
    }
}
