//! The section's surface lines as their own cached stroke stream, and the
//! prismoids it crosses as filled shapes with their lines.

use glam::DVec3;

use crate::{
    model::prismoid_grid::{SectionCrossing, SectionPoint},
    rendering::{
        StrokeInstance, Vertex,
        geometry::{DrawContext, draw_line},
        scene::document_style::STYLE_SLOT_NONE,
    },
};

/// How far behind the plane a crossed prismoid is filled, in metres. The
/// square-on view moves no pixel for it, and the cut line on the plane stays
/// in front of the fill instead of tying with it.
pub(crate) const SECTION_FILL_DEPTH_TIE_M: f64 = 0.01;

/// Width of the line between two crossed prismoids, in logical pixels.
pub(crate) const SECTION_CELL_LINE_WIDTH: f32 = 1.0;

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

/// Where the section crosses one drawn prismoid grid: the crossings in order,
/// one colour per point of each, and whether each crossed cell is selected.
pub(crate) struct SectionCellsOfGrid {
    pub(crate) crossings: Vec<SectionCrossing>,
    pub(crate) colors: Vec<Vec<[f32; 4]>>,
    pub(crate) selected: Vec<bool>,
}

/// The crossed prismoids as fills, with the lines between them and round the
/// selected ones. The fill is built when the cells change, or the scene
/// origin or the view direction moves; the lines go with `revision`.
#[derive(Default)]
pub(crate) struct SectionCells {
    grids: Vec<SectionCellsOfGrid>,
    revision: u64,
    walls: Vec<(DVec3, DVec3)>,
    rim: Vec<(DVec3, DVec3)>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    built_for: Option<([u64; 3], [u64; 3])>,
    dirty: bool,
}

impl SectionCells {
    pub(crate) fn set(&mut self, grids: Vec<SectionCellsOfGrid>) {
        self.walls.clear();
        self.rim.clear();
        for grid in &grids {
            cell_lines(grid, &mut self.walls, &mut self.rim);
        }
        self.grids = grids;
        self.revision += 1;
        self.dirty = true;
    }

    /// Rebuild the fill if anything it depends on changed, each point moved
    /// `away` from the viewer by [`SECTION_FILL_DEPTH_TIE_M`]. True when it
    /// was rebuilt and the GPU copy is stale.
    pub(crate) fn refresh(&mut self, scene_origin: DVec3, away: DVec3) -> bool {
        let built_for = Some((scene_origin.to_array().map(f64::to_bits), away.to_array().map(f64::to_bits)));
        if !self.dirty && self.built_for == built_for {
            return false;
        }
        self.vertices.clear();
        self.indices.clear();
        let vertex = |point: &SectionPoint, z: f64, color: [f32; 4]| Vertex {
            pos: (point.plan.extend(z) - scene_origin + away * SECTION_FILL_DEPTH_TIE_M).as_vec3().to_array(),
            color,
            style: STYLE_SLOT_NONE,
        };
        for grid in &self.grids {
            for (crossing, colors) in grid.crossings.iter().zip(&grid.colors) {
                for (points, colors) in crossing.points.windows(2).zip(colors.windows(2)) {
                    let base = self.vertices.len() as u32;
                    self.vertices.extend([
                        vertex(&points[0], points[0].roof, colors[0]),
                        vertex(&points[1], points[1].roof, colors[1]),
                        vertex(&points[1], points[1].floor, colors[1]),
                        vertex(&points[0], points[0].floor, colors[0]),
                    ]);
                    self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                }
            }
        }
        self.built_for = built_for;
        self.dirty = false;
        true
    }

    /// Counts up on every [`Self::set`].
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// Floor-to-roof lines where two crossed cells touch.
    pub(crate) fn walls(&self) -> &[(DVec3, DVec3)] {
        &self.walls
    }

    /// The lines round the selected cells.
    pub(crate) fn rim(&self) -> &[(DVec3, DVec3)] {
        &self.rim
    }

    pub(crate) fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    pub(crate) fn indices(&self) -> &[u32] {
        &self.indices
    }
}

/// One grid's lines: a wall where two crossed cells touch, and round the
/// selected cells their roof, their floor and each end not shared with a
/// selected neighbour. The outer ends are the cut's own wall lines.
fn cell_lines(grid: &SectionCellsOfGrid, walls: &mut Vec<(DVec3, DVec3)>, rim: &mut Vec<(DVec3, DVec3)>) {
    let upright = |point: &SectionPoint| (point.plan.extend(point.floor), point.plan.extend(point.roof));
    let crossings = &grid.crossings;
    for (index, crossing) in crossings.iter().enumerate() {
        let (Some(first), Some(last)) = (crossing.points.first(), crossing.points.last()) else {
            continue;
        };
        let before = index.checked_sub(1).filter(|before| crossings[*before].along[1] == crossing.along[0]);
        let after = Some(index + 1).filter(|after| crossings.get(*after).is_some_and(|next| next.along[0] == crossing.along[1]));
        if before.is_some() {
            walls.push(upright(first));
        }
        if !grid.selected[index] {
            continue;
        }
        for pair in crossing.points.windows(2) {
            rim.push((pair[0].plan.extend(pair[0].roof), pair[1].plan.extend(pair[1].roof)));
            rim.push((pair[0].plan.extend(pair[0].floor), pair[1].plan.extend(pair[1].floor)));
        }
        if before.is_none_or(|before| !grid.selected[before]) {
            rim.push(upright(first));
        }
        if after.is_none_or(|after| !grid.selected[after]) {
            rim.push(upright(last));
        }
    }
}
