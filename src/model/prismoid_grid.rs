//! A prismoid grid: the prismoids between a roof horizon and a floor horizon
//! on one plan lattice, with a mask of the active cells and any number of
//! values per cell.

use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use anyhow::{Result, bail};
use glam::{DMat2, DMat3, DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::{
    app::jobs::CancelFlag,
    i18n::tr,
    model::{
        block_model::BlockBounds,
        drill_hole::{Correction, DrillColorPreset, DrillHoleDataset},
        formats::mesh_data,
        grid_surface::{GridSurface, HEIGHT_TOLERANCE, NODE_TOLERANCE, SurfaceAt, surface_z_single},
        progress::{Phase, Progress},
        project::{ModelEnvelope, ProjectItemState},
        rbf::NODE_BUDGET,
        solid_reserves::NumericTotal,
        spatial::{TriangleBvh, ray_triangle},
        triangulation::{GeometryVersion, quality_key, same_quality_name},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PrismoidGridId(pub(crate) u64);

/// The grey a built prismoid grid starts in.
pub(crate) const DEFAULT_PRISMOID_GRID_COLOR: [f32; 4] = [0.72, 0.72, 0.75, 1.0];

/// The ramp a grid is drawn on until another is chosen, low blue to high red.
pub(crate) const THICKNESS_RAMP: DrillColorPreset = DrillColorPreset::Rainbow;

/// What a prismoid grid is coloured by: one of its values by name, or its
/// vertical thickness with `None`, on one of the drill colour scales.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PrismoidColorBy {
    pub(crate) value: Option<String>,
    pub(crate) preset: DrillColorPreset,
}

impl Default for PrismoidColorBy {
    fn default() -> Self {
        Self {
            value: None,
            preset: THICKNESS_RAMP,
        }
    }
}

impl PrismoidColorBy {
    /// The position of the first of `names` that is the chosen value, by the
    /// rule qualities are matched by; `None` for thickness or a name not held.
    pub(crate) fn index<'a>(&self, mut names: impl Iterator<Item = &'a str>) -> Option<usize> {
        let value = self.value.as_deref()?;
        names.position(|name| same_quality_name(name, value))
    }
}

/// Whether no two of `names` are one name, as qualities are compared; a
/// blank one is no name and is not counted.
pub(crate) fn names_once<'a>(names: impl IntoIterator<Item = &'a str>) -> bool {
    let keys: Vec<String> = names.into_iter().map(quality_key).filter(|key| !key.is_empty()).collect();
    keys.iter().enumerate().all(|(index, key)| !keys[..index].contains(key))
}

/// Where a prismoid grid sits in the strata it was built in: the stack as
/// the user stated it, top first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Stratum {
    pub(crate) strata: String,
    /// This grid's interval, from 0 at the top.
    pub(crate) index: usize,
    pub(crate) horizons: Vec<String>,
    pub(crate) intervals: Vec<String>,
}

impl Stratum {
    /// One more horizon than intervals, this grid among them, and every
    /// name given. A name may repeat: a saved record is read as it was kept.
    pub(crate) fn is_whole(&self) -> bool {
        let blank = |name: &String| name.trim().is_empty();
        self.horizons.len() == self.intervals.len() + 1
            && self.index < self.intervals.len()
            && !blank(&self.strata)
            && !self.horizons.iter().any(blank)
            && !self.intervals.iter().any(blank)
    }

    /// This grid's interval name, the category its blocks carry.
    pub(crate) fn category(&self) -> &str {
        self.intervals.get(self.index).map_or("", String::as_str)
    }

    pub(crate) fn roof(&self) -> &str {
        self.horizons.get(self.index).map_or("", String::as_str)
    }

    pub(crate) fn floor(&self) -> &str {
        self.horizons.get(self.index + 1).map_or("", String::as_str)
    }

    /// The interval above this one, `None` at the top.
    pub(crate) fn above(&self) -> Option<&str> {
        self.intervals.get(self.index.checked_sub(1)?).map(String::as_str)
    }

    /// The interval below this one, `None` at the base.
    pub(crate) fn below(&self) -> Option<&str> {
        self.intervals.get(self.index + 1).map(String::as_str)
    }
}

/// Build the interval between each pair of neighbouring `horizons`, listed
/// top first, by the one rule a single grid is built by. An interval with no
/// thickness anywhere is `None`; any other refusal refuses the whole stack,
/// given with the interval's position.
pub(crate) fn build_strata(
    horizons: &[GridSurface],
    step: NonZeroUsize,
    envelope: Option<&ModelEnvelope>,
    cancel: &CancelFlag,
    progress: &Phase,
) -> Result<Vec<Option<PrismoidBuild>>, (usize, BuildRefusal)> {
    use rayon::prelude::*;
    let pairs: Vec<&[GridSurface]> = horizons.windows(2).collect();
    // The intervals build side by side, so the bar counts finished ones; a
    // lone interval reports its own steps.
    let finished = AtomicU64::new(0);
    let built: Vec<_> = pairs
        .par_iter()
        .map(|pair| {
            if cancel.is_cancelled() {
                return Err(BuildRefusal::Cancelled);
            }
            let own = if pairs.len() == 1 { progress.clone() } else { Progress::new().phase(0.0, 1.0) };
            let built = PrismoidGrid::between(&pair[0], &pair[1], step, envelope, cancel, &own);
            progress.set_items(finished.fetch_add(1, Ordering::Relaxed) + 1, pairs.len() as u64);
            built
        })
        .collect();
    built
        .into_iter()
        .enumerate()
        .map(|(index, built)| match built {
            Ok(built) => Ok(Some(built)),
            Err(BuildRefusal::NoThickness) => Ok(None),
            Err(refusal) => Err((index, refusal)),
        })
        .collect()
}

/// Why two grid surfaces make no prismoid grid.
#[derive(Debug)]
pub(crate) enum BuildRefusal {
    /// Different spacings, or nodes that do not line up.
    NotOneLattice,
    /// Fewer than two node columns or rows in common, or no cell in
    /// common that both cover.
    NoOverlap,
    /// Fewer than two kept nodes along a side at the chosen cell size.
    CellTooLarge,
    /// No active cell, and more cells crossed than closed.
    RoofBelow,
    /// No active cell: roof and floor meet wherever both cover.
    NoThickness,
    Cancelled,
    /// The built grid failed [`PrismoidGrid::check`].
    Malformed(anyhow::Error),
    /// The surfaces' nodes are no whole number of spacings from the model
    /// envelope's corner.
    OffEnvelopeLattice,
    /// An active cell leaves the model envelope.
    OutsideEnvelope,
}

/// A prismoid grid built between two surfaces, with the cells of the
/// overlap left out for a crossing and for a missing height or triangle.
#[derive(Debug)]
pub(crate) struct PrismoidBuild {
    pub(crate) grid: PrismoidGrid,
    pub(crate) crossed: usize,
    pub(crate) holes: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Corner {
    Missing,
    Crossed,
    Closed,
    Open,
}

/// A plan lattice: an origin, two horizontal unit axes and the cell sizes
/// along each.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Lattice {
    pub(crate) origin: DVec2,
    pub(crate) u: DVec2,
    pub(crate) v: DVec2,
    pub(crate) u_sizes: Vec<f64>,
    pub(crate) v_sizes: Vec<f64>,
}

impl Lattice {
    pub(crate) fn columns(&self) -> usize {
        self.u_sizes.len()
    }

    pub(crate) fn rows(&self) -> usize {
        self.v_sizes.len()
    }

    pub(crate) fn corner_count(&self) -> usize {
        (self.columns() + 1) * (self.rows() + 1)
    }

    pub(crate) fn cell_count(&self) -> usize {
        self.columns() * self.rows()
    }

    /// The plan position of every corner, in corner order.
    pub(crate) fn corners(&self) -> Vec<DVec2> {
        self.corners_from(self.origin)
    }

    /// The corners as `corners` gives them, with `base` for the origin: with
    /// zero, relative to the origin, exact where world coordinates are not.
    fn corners_from(&self, base: DVec2) -> Vec<DVec2> {
        let (u_edges, v_edges) = (edges(&self.u_sizes), edges(&self.v_sizes));
        v_edges
            .iter()
            .flat_map(|along_v| u_edges.iter().map(move |along_u| base + self.u * *along_u + self.v * *along_v))
            .collect()
    }

    /// The (column, row) of the cell holding a plan point; a point on a line
    /// between cells belongs to the cell beyond it, and the far edge to the
    /// last cell. `None` outside the lattice.
    pub(crate) fn cell_at(&self, at: DVec2) -> Option<(usize, usize)> {
        self.cell_at_in(at, &edges(&self.u_sizes), &edges(&self.v_sizes))
    }

    /// [`Self::cell_at`] from the lattice's edge distances.
    fn cell_at_in(&self, at: DVec2, u_edges: &[f64], v_edges: &[f64]) -> Option<(usize, usize)> {
        let DVec2 { x: s, y: t } = self.to_lattice(at);
        let find = |edges: &[f64], along: f64| {
            let total = edges[edges.len() - 1];
            // Written so NaN is outside; either edge as `corners` computes
            // it may lie a rounding error of the world coordinates away.
            let slack = (self.origin.abs().max_element() + total) * 64.0 * f64::EPSILON;
            if !(-slack..=total + slack).contains(&along) {
                return None;
            }
            Some(edges.partition_point(|edge| *edge <= along).saturating_sub(1).min(edges.len() - 2))
        };
        Some((find(u_edges, s)?, find(v_edges, t)?))
    }

    /// A plan point as distances along `u` and `v` from the origin: the exact
    /// inverse of `corners`, also for axes `check` admits a hair off unit and
    /// perpendicular.
    pub(crate) fn to_lattice(&self, at: DVec2) -> DVec2 {
        self.to_lattice_from_origin(at - self.origin)
    }

    /// [`Self::to_lattice`] of a plan point given relative to the origin.
    fn to_lattice_from_origin(&self, at: DVec2) -> DVec2 {
        DMat2::from_cols(self.u, self.v).inverse() * at
    }

    /// A point in the lattice frame (distances along `u` and `v` from the
    /// origin) as a plan point: the inverse of [`Self::to_lattice`].
    pub(crate) fn to_world(&self, at: DVec2) -> DVec2 {
        self.origin + self.u * at.x + self.v * at.y
    }

    /// The (column, row) of this lattice's cell (0, 0) in `other`'s cells
    /// when the two share one lattice: both regular, the same axes and cell
    /// size to `NODE_TOLERANCE` (the size's as a fraction), their origins a
    /// whole number of cells apart. Negative where this lattice starts before `other`; `None` for
    /// two lattices, a 10 m grid against a 5 m one included.
    pub(crate) fn offset_to(&self, other: &Lattice) -> Option<(i64, i64)> {
        if !self.is_regular() || !other.is_regular() {
            return None;
        }
        let same_axis = |mine: DVec2, theirs: DVec2| (mine - theirs).abs().max_element() <= NODE_TOLERANCE;
        if !same_axis(self.u, other.u) || !same_axis(self.v, other.v) {
            return None;
        }
        let mine = DVec2::new(*self.u_sizes.first()?, *self.v_sizes.first()?);
        let theirs = DVec2::new(*other.u_sizes.first()?, *other.v_sizes.first()?);
        if (mine - theirs).abs().cmpgt(mine * NODE_TOLERANCE).any() {
            return None;
        }
        let offset = other.to_lattice(self.origin) / theirs;
        let whole = offset.round();
        ((offset - whole).abs().max_element() <= NODE_TOLERANCE).then_some((whole.x as i64, whole.y as i64))
    }

    /// The plan position of the middle of a cell.
    pub(crate) fn cell_centre(&self, column: usize, row: usize) -> DVec2 {
        self.cell_centre_in(column, row, &edges(&self.u_sizes), &edges(&self.v_sizes))
    }

    /// [`Self::cell_centre`] from the lattice's edge distances.
    fn cell_centre_in(&self, column: usize, row: usize, u_edges: &[f64], v_edges: &[f64]) -> DVec2 {
        let s = (u_edges[column] + u_edges[column + 1]) / 2.0;
        let t = (v_edges[row] + v_edges[row + 1]) / 2.0;
        self.origin + self.u * s + self.v * t
    }

    /// Where a plan point lies in a cell, from 0 to 1 along `u` and along `v`.
    fn local_in(&self, column: usize, row: usize, at: DVec2, u_edges: &[f64], v_edges: &[f64]) -> DVec2 {
        let DVec2 { x: s, y: t } = self.to_lattice(at);
        DVec2::new((s - u_edges[column]) / self.u_sizes[column], (t - v_edges[row]) / self.v_sizes[row])
    }

    /// Whether every column has one size and every row one size.
    pub(crate) fn is_regular(&self) -> bool {
        let uniform = |sizes: &[f64]| sizes.iter().all(|size| Some(size) == sizes.first());
        uniform(&self.u_sizes) && uniform(&self.v_sizes)
    }

    /// The smallest cell side.
    pub(crate) fn least_size(&self) -> f64 {
        self.u_sizes.iter().chain(&self.v_sizes).copied().fold(f64::INFINITY, f64::min)
    }
}

/// Two triangles on the a-c diagonal of every quad of `columns` by `rows`
/// that `nodes` gives the four nodes of, a, b (along `u`), c and d.
fn quads(columns: usize, rows: usize, mut nodes: impl FnMut(usize, usize) -> Option<[u32; 4]>) -> Vec<[u32; 3]> {
    let mut faces = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            if let Some([a, b, c, d]) = nodes(column, row) {
                faces.extend([[a, b, c], [a, c, d]]);
            }
        }
    }
    faces
}

/// A height on the two triangles of a cell, from `z` at its corners a, b
/// (along `u`), c and d, at `local`, 0 to 1 across the cell along each.
fn height_in(z: &[f64], [a, b, c, d]: [usize; 4], local: DVec2) -> f64 {
    let (fs, ft) = (local.x, local.y);
    if fs >= ft {
        z[a] + fs * (z[b] - z[a]) + ft * (z[c] - z[b])
    } else {
        z[a] + ft * (z[d] - z[a]) + fs * (z[c] - z[d])
    }
}

/// Distances from the lattice origin to each line between cells, from 0.
fn edges(sizes: &[f64]) -> Vec<f64> {
    let mut edges = vec![0.0];
    for size in sizes {
        edges.push(edges[edges.len() - 1] + size);
    }
    edges
}

/// One named value per cell, `None` where a cell has none.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CellValues {
    pub(crate) name: String,
    /// As the user typed it; empty when the values have none.
    pub(crate) unit: String,
    pub(crate) role: CellValueRole,
    pub(crate) values: Vec<Option<f64>>,
}

/// What a grid's values are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellValueRole {
    Quality,
    /// The density tonnes are measured with, in t/m³; one per grid.
    Density,
}

/// How far the topography must pass a roof or floor for a prismoid to be
/// cut: nearer, it is taken as meeting it.
const CUT_CONTACT: f64 = 1e-6;

/// The most cut triangles one trimmed grid holds: as many as the corners a
/// grid is drawn with.
pub(crate) const CUT_BUDGET: usize = NODE_BUDGET;

/// A point of a cut prismoid's triangles: its plan position in the lattice's
/// frame (distances along `u` and `v` from the origin) and the kept roof and
/// kept floor there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CutPoint {
    pub(crate) plan: DVec2,
    pub(crate) roof: f64,
    pub(crate) floor: f64,
}

/// The prismoids a trim cut along a topography. A cut prismoid is kept as
/// triangles over the part of its plan where the topography leaves ground,
/// the kept roof and floor each linear on a triangle, so volumes sum exactly.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CutPrismoids {
    /// The cut cells, ascending.
    pub(crate) cells: Vec<u32>,
    /// Where each cut cell's triangles start in `triangles`: one more entry
    /// than `cells`, the last the triangle count; empty when nothing is cut.
    pub(crate) starts: Vec<u32>,
    /// The kept roof's and floor's means of each cut cell, as its box holds
    /// its exact volume at the kept part's mid height.
    pub(crate) means: Vec<[f64; 2]>,
    pub(crate) points: Vec<CutPoint>,
    /// Indices into `points`; a cell's triangles use only its own points.
    pub(crate) triangles: Vec<[u32; 3]>,
    /// Per lattice cell, its index in `cells` or `u32::MAX`; empty when
    /// nothing is cut.
    of_cell: Vec<u32>,
}

/// The barycentric weights of `at` on a triangle of plan points.
fn barycentric([a, b, c]: [DVec2; 3], at: DVec2) -> [f64; 3] {
    let det = (b - a).perp_dot(c - a);
    let (second, third) = ((at - a).perp_dot(c - a) / det, (b - a).perp_dot(at - a) / det);
    [1.0 - second - third, second, third]
}

/// The point of a triangle nearest to `at`: `at` itself when inside.
fn closest_in_triangle(triangle: [DVec2; 3], at: DVec2) -> DVec2 {
    if barycentric(triangle, at).iter().all(|weight| *weight >= 0.0) {
        return at;
    }
    let mut nearest = triangle[0];
    let mut least = f64::INFINITY;
    for (from, to) in [(0, 1), (1, 2), (2, 0)] {
        let (a, b) = (triangle[from], triangle[to]);
        let along = b - a;
        let candidate = a + along * ((at - a).dot(along) / along.length_squared()).clamp(0.0, 1.0);
        let distance = (candidate - at).length_squared();
        if distance < least {
            (nearest, least) = (candidate, distance);
        }
    }
    nearest
}

/// The plan area of a triangle of cut points.
fn cut_area([a, b, c]: [CutPoint; 3]) -> f64 {
    ((b.plan - a.plan).perp_dot(c.plan - a.plan) * 0.5).abs()
}

/// The roof and floor means a cut cell's triangles give its box: the volume
/// over the cell's plan area, centred on the kept part's mid height.
fn cut_means(triangles: impl Iterator<Item = [CutPoint; 3]>, cell_area: f64) -> [f64; 2] {
    let (mut volume, mut middle, mut area) = (0.0, 0.0, 0.0);
    for triangle in triangles {
        let share = cut_area(triangle);
        volume += share * triangle.iter().map(|point| point.roof - point.floor).sum::<f64>() / 3.0;
        middle += share * triangle.iter().map(|point| (point.roof + point.floor) / 2.0).sum::<f64>() / 3.0;
        area += share;
    }
    let centre = if area > 0.0 { middle / area } else { 0.0 };
    let height = volume / cell_area;
    [centre + height / 2.0, centre - height / 2.0]
}

impl CutPrismoids {
    /// The cut cells with their own points and triangles, cells ascending.
    /// Each cell's points are welded when their plan positions match at the
    /// kernel's tolerance and their kept roofs and kept floors each agree
    /// within it too, keeping the first; points over one plan position with
    /// different heights stay apart, as at an upright face of the ground,
    /// where the cliff draws as an open line. Triangles left with fewer than
    /// three points or no plan area are dropped, and cells left with no
    /// triangle.
    pub(crate) fn build(lattice: &Lattice, cells: Vec<(usize, Vec<CutPoint>, Vec<[u32; 3]>)>) -> CutPrismoids {
        let mut cut = CutPrismoids::default();
        let columns = lattice.columns();
        let quantum = crate::model::kernel::XY_TOL;
        let mut welded: std::collections::HashMap<(i64, i64), Vec<u32>> = std::collections::HashMap::new();
        for (cell, points, triangles) in cells {
            let first = cut.points.len();
            let triangles_first = cut.triangles.len();
            welded.clear();
            let mut at = Vec::with_capacity(points.len());
            for point in &points {
                let key = ((point.plan.x / quantum).round() as i64, (point.plan.y / quantum).round() as i64);
                // The first held point within the tolerance, in any of the
                // buckets the point could straddle.
                let same = (-1..=1)
                    .flat_map(|along_x| (-1..=1).map(move |along_y| (key.0.saturating_add(along_x), key.1.saturating_add(along_y))))
                    .filter_map(|near| welded.get(&near))
                    .flatten()
                    .copied()
                    .filter(|index| {
                        let other = cut.points[*index as usize];
                        (other.plan - point.plan).abs().max_element() <= quantum && (other.roof - point.roof).abs() <= quantum && (other.floor - point.floor).abs() <= quantum
                    })
                    .min();
                let index = same.unwrap_or_else(|| {
                    cut.points.push(*point);
                    let index = (cut.points.len() - 1) as u32;
                    welded.entry(key).or_default().push(index);
                    index
                });
                at.push(index);
            }
            for triangle in triangles {
                let [a, b, c] = triangle.map(|index| at[index as usize]);
                if a == b || b == c || a == c {
                    continue;
                }
                if cut_area([a, b, c].map(|index| cut.points[index as usize])) <= 0.0 {
                    continue;
                }
                cut.triangles.push([a, b, c]);
            }
            if cut.triangles.len() == triangles_first {
                cut.points.truncate(first);
                continue;
            }
            cut.cells.push(cell as u32);
            cut.starts.push(triangles_first as u32);
        }
        if cut.cells.is_empty() {
            return CutPrismoids::default();
        }
        cut.starts.push(cut.triangles.len() as u32);
        cut.of_cell = vec![u32::MAX; lattice.cell_count()];
        for (index, cell) in cut.cells.iter().enumerate() {
            cut.of_cell[*cell as usize] = index as u32;
        }
        let means: Vec<[f64; 2]> = (0..cut.cells.len())
            .map(|index| {
                let cell = cut.cells[index] as usize;
                cut_means(cut.triangles_at(index), lattice.u_sizes[cell % columns] * lattice.v_sizes[cell / columns])
            })
            .collect();
        cut.means = means;
        cut
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The position in `cells` of a lattice cell, when it is cut.
    pub(crate) fn cell_index(&self, cell: usize) -> Option<usize> {
        self.of_cell.get(cell).copied().filter(|index| *index != u32::MAX).map(|index| index as usize)
    }

    /// The kept roof's and floor's means of a cut cell.
    pub(crate) fn mean(&self, cell: usize) -> Option<[f64; 2]> {
        self.cell_index(cell).map(|index| self.means[index])
    }

    /// The triangles of a cut cell.
    pub(crate) fn triangles_of(&self, cell: usize) -> Option<impl Iterator<Item = [CutPoint; 3]> + '_> {
        self.cell_index(cell).map(|index| self.triangles_at(index))
    }

    fn triangles_at(&self, index: usize) -> impl Iterator<Item = [CutPoint; 3]> + '_ {
        let range = self.starts[index] as usize..self.starts[index + 1] as usize;
        self.triangles[range].iter().map(|triangle| triangle.map(|point| self.points[point as usize]))
    }

    pub(crate) fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// The point indices of the triangles of the cut cell at `index`.
    fn indices_at(&self, index: usize) -> &[[u32; 3]] {
        &self.triangles[self.starts[index] as usize..self.starts[index + 1] as usize]
    }

    /// The kept roof, kept floor and the roof's gradient (dz/ds, dz/dt, in
    /// the lattice frame) at a lattice-frame point of the cut cell at
    /// `index`: on the triangle holding it, else at the nearest point of the
    /// cell's triangles, where the kept part ends.
    fn read(&self, index: usize, at: DVec2) -> Option<(f64, f64, DVec2)> {
        let mut best: Option<(f64, [CutPoint; 3], DVec2)> = None;
        for triangle in self.triangles_at(index) {
            let nearest = closest_in_triangle(triangle.map(|point| point.plan), at);
            let distance = (nearest - at).length_squared();
            if best.is_none_or(|(least, ..)| distance < least) {
                best = Some((distance, triangle, nearest));
                if distance == 0.0 {
                    break;
                }
            }
        }
        let (_, triangle, nearest) = best?;
        let weights = barycentric(triangle.map(|point| point.plan), nearest);
        let blend = |value: fn(&CutPoint) -> f64| weights.iter().zip(&triangle).map(|(weight, point)| weight * value(point)).sum::<f64>();
        let [a, b, c] = triangle;
        let (e1, e2) = (b.plan - a.plan, c.plan - a.plan);
        let (d1, d2) = (b.roof - a.roof, c.roof - a.roof);
        let det = e1.perp_dot(e2);
        let gradient = DVec2::new((d1 * e2.y - d2 * e1.y) / det, (d2 * e1.x - d1 * e2.x) / det);
        Some((blend(|point| point.roof), blend(|point| point.floor), gradient))
    }

    /// Whether a lattice-frame point of the cut cell at `index` lies on a kept
    /// triangle, to `kernel::XY_TOL`; false where the trim took the ground away.
    fn covers(&self, index: usize, at: DVec2) -> bool {
        let tolerance = crate::model::kernel::XY_TOL;
        self.triangles_at(index)
            .any(|triangle| (closest_in_triangle(triangle.map(|point| point.plan), at) - at).length_squared() <= tolerance * tolerance)
    }

    /// The part of the cut cell at `index`'s boundary that lies on the line
    /// from `from` to `to` (lattice-frame points), as segments of cut
    /// points ordered from `from` towards `to`, each end on the line to
    /// `kernel::XY_TOL`.
    fn along_edge(&self, index: usize, from: DVec2, to: DVec2) -> Vec<[CutPoint; 2]> {
        let tolerance = crate::model::kernel::XY_TOL;
        let (length, direction) = ((to - from).length(), (to - from).normalize());
        let on_line = |point: &CutPoint| {
            let offset = point.plan - from;
            direction.perp_dot(offset).abs() <= tolerance && -tolerance <= offset.dot(direction) && offset.dot(direction) <= length + tolerance
        };
        let mut segments = Vec::new();
        for triangle in self.triangles_at(index) {
            for (first, second) in [(0, 1), (1, 2), (2, 0)] {
                let (one, other) = (triangle[first], triangle[second]);
                if on_line(&one) && on_line(&other) {
                    let ordered = if (one.plan - from).dot(direction) <= (other.plan - from).dot(direction) {
                        [one, other]
                    } else {
                        [other, one]
                    };
                    segments.push(ordered);
                }
            }
        }
        segments
    }

    /// Bytes held.
    fn estimated_bytes(&self) -> usize {
        (self.cells.len() + self.starts.len() + self.of_cell.len()) * size_of::<u32>()
            + self.means.len() * size_of::<[f64; 2]>()
            + self.points.len() * size_of::<CutPoint>()
            + self.triangles.len() * size_of::<[u32; 3]>()
    }
}

/// Roof and floor elevations at every corner, row by row from the first
/// row: corner (`column`, `row`) at `row * (columns + 1) + column`, cell
/// (`column`, `row`) at `row * columns + column`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PrismoidGrid {
    pub(crate) lattice: Lattice,
    pub(crate) roof: Vec<f64>,
    pub(crate) floor: Vec<f64>,
    pub(crate) active: Vec<bool>,
    /// The cells masked by hand: never active, and the only cells an unmask
    /// brings back.
    pub(crate) masked: Vec<bool>,
    pub(crate) cell_values: Vec<CellValues>,
    /// The prismoids a trim cut along a topography; empty when none is.
    pub(crate) cut: CutPrismoids,
}

impl PrismoidGrid {
    /// Whether the grid is well formed: unit, perpendicular, right-handed
    /// axes; at least one cell, each size finite and positive; finite heights
    /// with the roof never below the floor; both masks and values for every
    /// cell, no cell masked by hand still active, each set of values named,
    /// no reserved or column name, no name twice, at most one density; and
    /// cut prismoids, if any, well formed.
    pub(crate) fn check(&self) -> Result<()> {
        let lattice = &self.lattice;
        if lattice.columns() == 0 || lattice.rows() == 0 {
            bail!(tr!("err-prismoid-grid-no-cells"));
        }
        // The tolerances the OMF writer validates an orientation with.
        const THRESHOLD: f64 = 1e-6;
        if [lattice.u, lattice.v].iter().any(|axis| (1.0 - axis.length_squared()).abs() >= THRESHOLD) {
            bail!(tr!("err-prismoid-grid-axes-not-unit"));
        }
        let direction = |axis: DVec2| if axis.length() == 0.0 { DVec2::ZERO } else { axis / axis.length() };
        if direction(lattice.u).dot(direction(lattice.v)).abs() >= THRESHOLD {
            bail!(tr!("err-prismoid-grid-axes-not-square"));
        }
        if lattice.u.perp_dot(lattice.v) <= 0.0 {
            bail!(tr!("err-prismoid-grid-not-right-handed"));
        }
        if !lattice.u_sizes.iter().chain(&lattice.v_sizes).all(|size| size.is_finite() && *size > 0.0) {
            bail!(tr!("err-prismoid-grid-cell-size"));
        }
        let corners = lattice.corner_count();
        if self.roof.len() != corners || self.floor.len() != corners {
            bail!(tr!(
                "err-prismoid-grid-height-count",
                roof = self.roof.len().to_string(),
                floor = self.floor.len().to_string(),
                corners = corners.to_string()
            ));
        }
        if !self.roof.iter().chain(&self.floor).all(|height| height.is_finite()) {
            bail!(tr!("err-prismoid-grid-height-not-finite"));
        }
        if let Some(corner) = self.roof.iter().zip(&self.floor).position(|(roof, floor)| roof < floor) {
            bail!(tr!("err-prismoid-grid-floor-above-roof", corner = corner.to_string()));
        }
        let cells = lattice.cell_count();
        if self.active.len() != cells {
            bail!(tr!("err-prismoid-grid-mask-count", count = self.active.len().to_string(), cells = cells.to_string()));
        }
        if self.masked.len() != cells {
            bail!(tr!("err-prismoid-grid-hand-mask-count", count = self.masked.len().to_string(), cells = cells.to_string()));
        }
        if let Some(cell) = self.active.iter().zip(&self.masked).position(|(active, masked)| *active && *masked) {
            bail!(tr!("err-prismoid-grid-masked-and-active", cell = cell.to_string()));
        }
        if let Some(values) = self.cell_values.iter().find(|values| values.values.len() != cells) {
            bail!(tr!(
                "err-prismoid-grid-value-count",
                count = values.values.len().to_string(),
                name = values.name.clone(),
                cells = cells.to_string()
            ));
        }
        if self.cell_values.iter().any(|values| values.name.trim().is_empty()) {
            bail!(tr!("err-prismoid-grid-unnamed-values"));
        }
        if let Some(values) = self
            .cell_values
            .iter()
            .find(|values| RESERVED_NAMES.iter().chain(&CORNER_HEIGHT_COLUMNS).any(|name| same_quality_name(name, &values.name)))
        {
            bail!(tr!("err-prismoid-grid-reserved-name", name = values.name.clone()));
        }
        let mut names = std::collections::HashSet::new();
        if let Some(values) = self.cell_values.iter().find(|values| !names.insert(quality_key(&values.name))) {
            bail!(tr!("err-prismoid-grid-values-twice", name = values.name.clone()));
        }
        if self.cell_values.iter().filter(|values| values.role == CellValueRole::Density).count() > 1 {
            bail!(tr!("err-prismoid-grid-two-densities"));
        }
        self.check_cut()
    }

    /// Whether the cut prismoids are well formed: lists that agree, cells
    /// ascending and taken by a trim (active or masked by hand), each cell's
    /// triangles inside it with kept heights finite and the roof never below
    /// the floor, and means as its triangles give them.
    fn check_cut(&self) -> Result<()> {
        let cut = &self.cut;
        if cut.is_empty() {
            let bare = cut.starts.is_empty() && cut.means.is_empty() && cut.points.is_empty() && cut.triangles.is_empty() && cut.of_cell.is_empty();
            if !bare {
                bail!(tr!("err-prismoid-grid-cut-lists"));
            }
            return Ok(());
        }
        let lattice = &self.lattice;
        let (count, cells) = (cut.cells.len(), lattice.cell_count());
        let shaped = cut.starts.len() == count + 1
            && cut.means.len() == count
            && cut.of_cell.len() == cells
            && cut.starts[0] == 0
            && cut.starts[count] as usize == cut.triangle_count()
            && cut.starts.windows(2).all(|pair| pair[0] <= pair[1])
            && cut.triangles.iter().flatten().all(|point| (*point as usize) < cut.points.len())
            && cut.of_cell.iter().filter(|index| **index != u32::MAX).count() == count;
        if !shaped {
            bail!(tr!("err-prismoid-grid-cut-lists"));
        }
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let columns = lattice.columns();
        for (index, cell) in cut.cells.iter().map(|cell| *cell as usize).enumerate() {
            let named = || cell.to_string();
            let ascending = index == 0 || cut.cells[index - 1] < cut.cells[index];
            if !ascending || cell >= cells || !(self.active[cell] || self.masked[cell]) || cut.of_cell[cell] as usize != index {
                bail!(tr!("err-prismoid-grid-cut-cell", cell = named()));
            }
            let (column, row) = (cell % columns, cell / columns);
            let slack = crate::model::kernel::XY_TOL;
            let inside = |point: &CutPoint| {
                point.plan.x >= u_edges[column] - slack
                    && point.plan.x <= u_edges[column + 1] + slack
                    && point.plan.y >= v_edges[row] - slack
                    && point.plan.y <= v_edges[row + 1] + slack
            };
            let cell_area = lattice.u_sizes[column] * lattice.v_sizes[row];
            let mut area = 0.0;
            for triangle in cut.triangles_at(index) {
                if !triangle.iter().all(inside) {
                    bail!(tr!("err-prismoid-grid-cut-footprint", cell = named()));
                }
                area += cut_area(triangle);
            }
            if area > cell_area * (1.0 + 1e-9) {
                bail!(tr!("err-prismoid-grid-cut-footprint", cell = named()));
            }
            let points = cut.triangles[cut.starts[index] as usize..cut.starts[index + 1] as usize].iter().flatten();
            if !points
                .map(|point| cut.points[*point as usize])
                .all(|point| point.roof.is_finite() && point.floor.is_finite() && point.roof - point.floor >= -1e-9)
            {
                bail!(tr!("err-prismoid-grid-cut-heights", cell = named()));
            }
            let held = cut.means[index];
            let mean = cut_means(cut.triangles_at(index), cell_area);
            let same = |held: f64, mean: f64| held.is_finite() && (held - mean).abs() <= 1e-9 * mean.abs().max(1.0);
            if !same(held[0], mean[0]) || !same(held[1], mean[1]) {
                bail!(tr!("err-prismoid-grid-cut-means", cell = named()));
            }
        }
        Ok(())
    }

    /// The prismoids between `roof` and `floor` over the cells both cover,
    /// each `step` of the surfaces' cells along a side and read at its four
    /// corners only. A cell is active where both surfaces have triangles
    /// over every surface cell inside it, none of its corners lacks a height
    /// or has the floor above the roof, and at least one has thickness. With
    /// an `envelope`, every active cell must lie inside it.
    pub(crate) fn between(
        roof: &GridSurface,
        floor: &GridSurface,
        step: NonZeroUsize,
        envelope: Option<&ModelEnvelope>,
        cancel: &CancelFlag,
        progress: &Phase,
    ) -> Result<PrismoidBuild, BuildRefusal> {
        let spacing = roof.spacing();
        if (spacing - floor.spacing()).abs() > spacing * NODE_TOLERANCE {
            return Err(BuildRefusal::NotOneLattice);
        }
        let (roof_origin, roof_columns, roof_rows) = roof.lattice();
        let (floor_origin, floor_columns, floor_rows) = floor.lattice();
        let offset = (floor_origin - roof_origin) / spacing;
        let whole = offset.round();
        if (offset - whole).abs().max_element() > NODE_TOLERANCE {
            return Err(BuildRefusal::NotOneLattice);
        }
        // The shared node box, in roof node indices.
        let span = |shift: f64, roof_count: usize, floor_count: usize| {
            let shift = shift as i64;
            let low = shift.max(0);
            let high = (shift + floor_count as i64).min(roof_count as i64);
            (low, high - low, shift)
        };
        let (low_column, columns, shift_column) = span(whole.x, roof_columns, floor_columns);
        let (low_row, rows, shift_row) = span(whole.y, roof_rows, floor_rows);
        if columns < 2 || rows < 2 {
            return Err(BuildRefusal::NoOverlap);
        }
        let (low_column, low_row) = (low_column as usize, low_row as usize);
        let first = roof_origin + DVec2::new(low_column as f64, low_row as f64) * spacing;
        // Along each side, the first node kept and how many are kept: every
        // `step`-th node, from a whole multiple of the cell size measured
        // from the envelope's corner, else the coordinate origin. A lattice
        // off the spacing's multiples starts at its first node; under an
        // envelope it is refused.
        let step = step.get();
        let anchor = envelope.map_or(DVec2::ZERO, |envelope| envelope.lower().truncate());
        let kept_nodes = |at: f64, anchor: f64, count: i64| {
            let nodes = (at - anchor) / spacing;
            let whole = nodes.round();
            let skip = if (nodes - whole).abs() <= NODE_TOLERANCE {
                (step - (whole as i64).rem_euclid(step as i64) as usize) % step
            } else if envelope.is_none() {
                0
            } else {
                return None;
            };
            Some((skip, (count as usize).saturating_sub(skip).div_ceil(step)))
        };
        let (Some((skip_column, columns)), Some((skip_row, rows))) = (kept_nodes(first.x, anchor.x, columns), kept_nodes(first.y, anchor.y, rows)) else {
            return Err(BuildRefusal::OffEnvelopeLattice);
        };
        if columns < 2 || rows < 2 {
            return Err(BuildRefusal::CellTooLarge);
        }
        let size = spacing * step as f64;
        let origin = first + DVec2::new(skip_column as f64, skip_row as f64) * spacing;
        // Roof and floor heights at every kept node, NaN where missing.
        let heights: Vec<(f64, f64)> = (0..rows)
            .flat_map(|row| (0..columns).map(move |column| (column, row)))
            .map(|(column, row)| {
                let (roof_column, roof_row) = (low_column + skip_column + column * step, low_row + skip_row + row * step);
                let floor_column = (roof_column as i64 - shift_column) as usize;
                let floor_row = (roof_row as i64 - shift_row) as usize;
                let height = |surface: &GridSurface, column: usize, row: usize| surface.height(column, row).unwrap_or(f64::NAN);
                (height(roof, roof_column, roof_row), height(floor, floor_column, floor_row))
            })
            .collect();
        if cancel.is_cancelled() {
            return Err(BuildRefusal::Cancelled);
        }
        progress.set_fraction(0.3);
        // Every surface cell inside the cell, at a point inside each of its
        // two triangles, whichever way its diagonal runs.
        let covered = |column: usize, row: usize| {
            (0..step * step).all(|inside| {
                let (fine_column, fine_row) = ((column * step + inside % step) as f64, (row * step + inside / step) as f64);
                [0.25, 0.75].into_iter().all(|across| {
                    let at = origin + DVec2::new(fine_column + across, fine_row + 0.5) * spacing;
                    roof.covers(at) && floor.covers(at)
                })
            })
        };
        let (cell_columns, cell_rows) = (columns - 1, rows - 1);
        let (mut crossed, mut holes, mut closed) = (0, 0, 0);
        let mut active = vec![false; cell_columns * cell_rows];
        // The smallest box of cells holding every active cell.
        let mut kept: Option<[usize; 4]> = None;
        for row in 0..cell_rows {
            if cancel.is_cancelled() {
                return Err(BuildRefusal::Cancelled);
            }
            for column in 0..cell_columns {
                let at = [
                    row * columns + column,
                    row * columns + column + 1,
                    (row + 1) * columns + column,
                    (row + 1) * columns + column + 1,
                ]
                .map(|index| corner(heights[index]));
                if at.contains(&Corner::Crossed) {
                    crossed += 1;
                } else if at.contains(&Corner::Missing) || !covered(column, row) {
                    holes += 1;
                } else if at.contains(&Corner::Open) {
                    active[row * cell_columns + column] = true;
                    kept = Some(kept.map_or([column, row, column, row], |[c0, r0, c1, r1]| [c0.min(column), r0.min(row), c1.max(column), r1.max(row)]));
                } else {
                    closed += 1;
                }
            }
        }
        let Some([first_column, first_row, last_column, last_row]) = kept else {
            return Err(if crossed == 0 && closed == 0 {
                BuildRefusal::NoOverlap
            } else if crossed > closed {
                BuildRefusal::RoofBelow
            } else {
                BuildRefusal::NoThickness
            });
        };
        progress.set_fraction(0.6);
        let (kept_columns, kept_rows) = (last_column - first_column + 1, last_row - first_row + 1);
        let (corner_columns, corner_rows) = (kept_columns + 1, kept_rows + 1);
        let (mut roof_heights, mut floor_heights): (Vec<f64>, Vec<f64>) = (first_row..=last_row + 1)
            .flat_map(|row| (first_column..=last_column + 1).map(move |column| row * columns + column))
            .map(|index| heights[index])
            .unzip();
        for values in [&mut roof_heights, &mut floor_heights] {
            fill_missing(values, corner_columns, corner_rows);
        }
        // Every corner of an active cell is closed or open, so this moves
        // only a closed corner's floor, by at most the tolerance, and the
        // corners no active cell reads.
        for (top, bottom) in roof_heights.iter().zip(&mut floor_heights) {
            *bottom = bottom.min(*top);
        }
        let active = (first_row..=last_row)
            .flat_map(|row| (first_column..=last_column).map(move |column| row * cell_columns + column))
            .map(|cell| active[cell])
            .collect();
        let grid = PrismoidGrid {
            lattice: Lattice {
                origin: origin + DVec2::new(first_column as f64, first_row as f64) * size,
                u: DVec2::X,
                v: DVec2::Y,
                u_sizes: vec![size; kept_columns],
                v_sizes: vec![size; kept_rows],
            },
            roof: roof_heights,
            floor: floor_heights,
            active,
            masked: vec![false; kept_columns * kept_rows],
            cell_values: Vec::new(),
            cut: CutPrismoids::default(),
        };
        grid.check().map_err(BuildRefusal::Malformed)?;
        if envelope.is_some_and(|envelope| !grid.fits(envelope)) {
            return Err(BuildRefusal::OutsideEnvelope);
        }
        progress.set_fraction(1.0);
        Ok(PrismoidBuild { grid, crossed, holes })
    }

    /// Whether the grid lies in `envelope`: its lattice on the envelope's
    /// axes, and every active cell's corners and every cut cell's points
    /// inside it, roof and floor.
    pub(crate) fn fits(&self, envelope: &ModelEnvelope) -> bool {
        let lattice = &self.lattice;
        if (lattice.u, lattice.v) != (DVec2::X, DVec2::Y) {
            return false;
        }
        let (corners, columns) = (lattice.corners(), lattice.columns());
        let corners_fit = self.active.iter().enumerate().filter(|(_, active)| **active).all(|(cell, _)| {
            let first = cell / columns * (columns + 1) + cell % columns;
            [first, first + 1, first + columns + 1, first + columns + 2]
                .into_iter()
                .all(|corner| envelope.contains_xy(corners[corner]) && envelope.contains_z(self.roof[corner]) && envelope.contains_z(self.floor[corner]))
        });
        corners_fit
            && self
                .cut
                .points
                .iter()
                .all(|point| envelope.contains_xy(lattice.to_world(point.plan)) && envelope.contains_z(point.roof) && envelope.contains_z(point.floor))
    }

    /// The segments of a cut cell's stored triangles that lie on the lattice
    /// edge from corner `from` to corner `to` (each a (column, row) of
    /// corners), ordered from `from` towards `to`, leaving out segments whose
    /// ends both have no kept thickness. `None` when the cell is not cut.
    fn cut_side(&self, cell: usize, from: (usize, usize), to: (usize, usize), u_edges: &[f64], v_edges: &[f64]) -> Option<Vec<[CutPoint; 2]>> {
        const NONE: f64 = 1e-9;
        let index = self.cut.cell_index(cell)?;
        let [from, to] = [from, to].map(|(column, row)| DVec2::new(u_edges[column], v_edges[row]));
        let mut segments = self.cut.along_edge(index, from, to);
        segments.retain(|ends| ends.iter().any(|end| end.roof - end.floor > NONE));
        Some(segments)
    }

    /// The lines where a trim left a cell's kept part ending inside it: the
    /// edges of its stored triangles that only one triangle uses and that no
    /// other such edge lies over in plan (a step between two kept parts, as
    /// the touching allowance leaves, is no end). An edge on a side of the
    /// cell counts only where the kept part has no thickness, as
    /// [`Self::cut_side`] draws the rest. Empty when the cell is not cut.
    fn cut_lines(&self, cell: usize, u_edges: &[f64], v_edges: &[f64]) -> Vec<[CutPoint; 2]> {
        const NONE: f64 = 1e-9;
        let Some(index) = self.cut.cell_index(cell) else {
            return Vec::new();
        };
        let columns = self.lattice.columns();
        let (column, row) = (cell % columns, cell / columns);
        let slack = crate::model::kernel::XY_TOL;
        let on = |value: f64, edge: f64| (value - edge).abs() <= slack;
        let on_side = |[one, other]: [DVec2; 2]| {
            let (xs, ys) = ([u_edges[column], u_edges[column + 1]], [v_edges[row], v_edges[row + 1]]);
            xs.iter().any(|x| on(one.x, *x) && on(other.x, *x)) || ys.iter().any(|y| on(one.y, *y) && on(other.y, *y))
        };
        let point = |at: u32| self.cut.points[at as usize];
        // Every edge by its two points, sorted, so an edge two triangles
        // share sits twice in a row.
        let mut edges: Vec<[u32; 2]> = self
            .cut
            .indices_at(index)
            .iter()
            .flat_map(|triangle| [(0, 1), (1, 2), (2, 0)].map(|(one, other)| [triangle[one].min(triangle[other]), triangle[one].max(triangle[other])]))
            .collect();
        edges.sort_unstable();
        let once = edges.chunk_by(|one, other| one == other).filter(|run| run.len() == 1).map(|run| run[0]);
        // Each of those by where it lies in plan, ends in either order.
        let key = |at: u32| {
            let plan = point(at).plan;
            ((plan.x / slack).round() as i64, (plan.y / slack).round() as i64)
        };
        let mut placed: Vec<((_, _), [u32; 2])> = once
            .map(|edge| {
                let (one, other) = (key(edge[0]), key(edge[1]));
                ((one.min(other), one.max(other)), edge)
            })
            .collect();
        placed.sort_unstable();
        placed
            .chunk_by(|one, other| one.0 == other.0)
            .filter(|run| run.len() == 1)
            .map(|run| run[0].1.map(point))
            .filter(|ends| !on_side(ends.map(|end| end.plan)) || ends.iter().all(|end| end.roof - end.floor <= NONE))
            .collect()
    }

    /// A cut line's roof and floor in the world, one stroke where the two
    /// meet along the whole line.
    fn cut_line_strokes(&self, line: [CutPoint; 2]) -> Vec<[DVec3; 2]> {
        const NONE: f64 = 1e-9;
        let at = |height: fn(&CutPoint) -> f64| line.map(|end| self.lattice.to_world(end.plan).extend(height(&end)));
        let mut strokes = vec![at(|end| end.roof)];
        if line.iter().any(|end| end.roof - end.floor > NONE) {
            strokes.push(at(|end| end.floor));
        }
        strokes
    }

    /// The vertical walls along every edge of an active cell whose neighbour
    /// is inactive or off the lattice, two outward-facing triangles each; on
    /// a cut cell, one wall per segment of the cut's boundary on that edge,
    /// none where the kept ground has no thickness.
    pub(crate) fn outline_walls(&self) -> (Vec<DVec3>, Vec<[u32; 3]>) {
        let lattice = &self.lattice;
        let (columns, rows) = (lattice.columns(), lattice.rows());
        let corners = lattice.corners();
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let active = |column: Option<usize>, row: Option<usize>| match (column, row) {
            (Some(column), Some(row)) if column < columns && row < rows => self.active[row * columns + column],
            _ => false,
        };
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                if !active(Some(column), Some(row)) {
                    continue;
                }
                // Anticlockwise round the cell, each edge with the neighbour
                // across it.
                let edges = [
                    ((column, row), (column + 1, row), (Some(column), row.checked_sub(1))),
                    ((column + 1, row), (column + 1, row + 1), (Some(column + 1), Some(row))),
                    ((column + 1, row + 1), (column, row + 1), (Some(column), Some(row + 1))),
                    ((column, row + 1), (column, row), (column.checked_sub(1), Some(row))),
                ];
                for (from, to, (beside_column, beside_row)) in edges {
                    if active(beside_column, beside_row) {
                        continue;
                    }
                    let mut wall = |ends: [(DVec2, f64, f64); 2]| {
                        let first = vertices.len() as u32;
                        for (plan, roof, floor) in ends {
                            vertices.push(plan.extend(roof));
                            vertices.push(plan.extend(floor));
                        }
                        let [from_roof, from_floor, to_roof, to_floor] = [first, first + 1, first + 2, first + 3];
                        triangles.extend([[to_roof, from_roof, from_floor], [to_roof, from_floor, to_floor]]);
                    };
                    if let Some(segments) = self.cut_side(row * columns + column, from, to, &u_edges, &v_edges) {
                        for ends in segments {
                            wall(ends.map(|end| (lattice.to_world(end.plan), end.roof, end.floor)));
                        }
                        continue;
                    }
                    wall([from, to].map(|(corner_column, corner_row)| {
                        let index = corner_row * (columns + 1) + corner_column;
                        (corners[index], self.roof[index], self.floor[index])
                    }));
                }
            }
        }
        (vertices, triangles)
    }

    /// Roof and floor segments round the rim of a set of cells: `cells` is a
    /// mask in cell order, and each side of a selected cell whose neighbour
    /// is not selected (or is off the lattice) gives one roof and one floor
    /// segment; a cut cell's sides follow its cut, as [`Self::outline_walls`]
    /// does, and its cut lines are drawn as well.
    pub(crate) fn selection_outline(&self, cells: &[bool]) -> Vec<[DVec3; 2]> {
        let lattice = &self.lattice;
        let (columns, rows) = (lattice.columns(), lattice.rows());
        let corners = lattice.corners();
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let selected = |column: Option<usize>, row: Option<usize>| match (column, row) {
            (Some(column), Some(row)) if column < columns && row < rows => cells[row * columns + column],
            _ => false,
        };
        let mut segments = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                if !selected(Some(column), Some(row)) {
                    continue;
                }
                let sides = [
                    ((column, row), (column + 1, row), (Some(column), row.checked_sub(1))),
                    ((column + 1, row), (column + 1, row + 1), (Some(column + 1), Some(row))),
                    ((column + 1, row + 1), (column, row + 1), (Some(column), Some(row + 1))),
                    ((column, row + 1), (column, row), (column.checked_sub(1), Some(row))),
                ];
                for (from, to, (beside_column, beside_row)) in sides {
                    if selected(beside_column, beside_row) {
                        continue;
                    }
                    if let Some(rim) = self.cut_side(row * columns + column, from, to, &u_edges, &v_edges) {
                        for [one, other] in rim {
                            let [one_plan, other_plan] = [one, other].map(|end| lattice.to_world(end.plan));
                            segments.push([one_plan.extend(one.roof), other_plan.extend(other.roof)]);
                            segments.push([one_plan.extend(one.floor), other_plan.extend(other.floor)]);
                        }
                        continue;
                    }
                    let [from, to] = [from, to].map(|(corner_column, corner_row)| corner_row * (columns + 1) + corner_column);
                    segments.push([corners[from].extend(self.roof[from]), corners[to].extend(self.roof[to])]);
                    segments.push([corners[from].extend(self.floor[from]), corners[to].extend(self.floor[to])]);
                }
                for line in self.cut_lines(row * columns + column, &u_edges, &v_edges) {
                    segments.extend(self.cut_line_strokes(line));
                }
            }
        }
        segments
    }

    /// The closed body of the active cells: roof, floor facing down, and
    /// [`Self::outline_walls`] less the triangles a closed corner flattens.
    /// A cut cell gives its stored triangles twice, the kept roof facing up
    /// and the kept floor down, with no wall inside it; its wires follow the
    /// cut. `None` with no active cell.
    pub(crate) fn closed_mesh(&self) -> Option<PrismoidMesh> {
        const UNSET: u32 = u32::MAX;
        let columns = self.lattice.columns();
        let plan = self.lattice.corners();
        let mut vertices = Vec::new();
        let mut roof_at = vec![UNSET; plan.len()];
        let mut floor_at = vec![UNSET; plan.len()];
        let place = |vertices: &mut Vec<mesh_data::Vertex>, at: &mut [u32], heights: &[f64], corner: usize| {
            if at[corner] == UNSET {
                at[corner] = vertices.len() as u32;
                vertices.push(mesh_vertex(plan[corner].extend(heights[corner])));
            }
            at[corner]
        };
        let mut faces = Vec::new();
        let (mut cut_roof_at, mut cut_floor_at) = (vec![UNSET; self.cut.points.len()], vec![UNSET; self.cut.points.len()]);
        for (cell, _) in self.active.iter().enumerate().filter(|(_, active)| **active) {
            if let Some(index) = self.cut.cell_index(cell) {
                for triangle in self.cut.indices_at(index) {
                    let [a, b, c] = triangle.map(|point| self.cut.points[point as usize].plan);
                    // Anticlockwise in the lattice's frame faces up.
                    let [x, y, z] = if (b - a).perp_dot(c - a) >= 0.0 {
                        *triangle
                    } else {
                        [triangle[0], triangle[2], triangle[1]]
                    };
                    let mut at = |placed: &mut [u32], point: u32, height: fn(&CutPoint) -> f64| {
                        if placed[point as usize] == UNSET {
                            placed[point as usize] = vertices.len() as u32;
                            let held = self.cut.points[point as usize];
                            vertices.push(mesh_vertex(self.lattice.to_world(held.plan).extend(height(&held))));
                        }
                        placed[point as usize]
                    };
                    let [ra, rb, rc] = [x, y, z].map(|point| at(&mut cut_roof_at, point, |held| held.roof));
                    let [fa, fb, fc] = [x, y, z].map(|point| at(&mut cut_floor_at, point, |held| held.floor));
                    faces.extend([[ra, rb, rc], [fa, fc, fb]]);
                }
                continue;
            }
            let a = (cell / columns) * (columns + 1) + cell % columns;
            let d = a + columns + 1;
            let corners = [a, a + 1, d + 1, d];
            let [ra, rb, rc, rd] = corners.map(|corner| place(&mut vertices, &mut roof_at, &self.roof, corner));
            let [fa, fb, fc, fd] = corners.map(|corner| place(&mut vertices, &mut floor_at, &self.floor, corner));
            faces.extend([[ra, rb, rc], [ra, rc, rd], [fa, fc, fb], [fa, fd, fc]]);
        }
        if faces.is_empty() {
            return None;
        }
        // Each active cell's sides on roof and floor, a side two active
        // cells share by the lower or left one, and each corner's vertical,
        // less what a closed corner flattens.
        let flat = |corner: usize| self.roof[corner] == self.floor[corner];
        let active = |column: usize, row: usize| self.active[row * columns + column];
        let mut seen = vec![false; plan.len()];
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        let mut lattice = Vec::new();
        let mut lattice_cells: Vec<[u32; 4]> = Vec::new();
        // The strokes along the lines trims cut inside cells, which the
        // outline below draws too.
        let mut cut_strokes: Vec<[u32; 2]> = Vec::new();
        let rows = self.lattice.rows();
        for (cell, _) in self.active.iter().enumerate().filter(|(_, active)| **active) {
            let (column, row) = (cell % columns, cell / columns);
            let a = row * (columns + 1) + column;
            let d = a + columns + 1;
            let sides = [
                (a, a + 1, row == 0 || !active(column, row - 1)),
                (a + 1, d + 1, true),
                (d, d + 1, true),
                (a, d, column == 0 || !active(column - 1, row)),
            ];
            let neighbour = |column: usize, row: usize| {
                if column < columns && row < rows && active(column, row) {
                    (row * columns + column) as u32
                } else {
                    UNSET
                }
            };
            let side_cells = [
                [cell as u32, UNSET, UNSET, UNSET],
                [cell as u32, neighbour(column + 1, row), UNSET, UNSET],
                [cell as u32, neighbour(column, row + 1), UNSET, UNSET],
                [cell as u32, UNSET, UNSET, UNSET],
            ];
            for (side, (from, to, _)) in sides.into_iter().enumerate().filter(|(_, (_, _, drawn))| *drawn) {
                let node = |corner: usize| (corner % (columns + 1), corner / (columns + 1));
                if let Some(segments) = self.cut_side(cell, node(from), node(to), &u_edges, &v_edges) {
                    for ends in segments {
                        for height in [|end: &CutPoint| end.roof, |end: &CutPoint| end.floor] {
                            let [one, other] = ends.map(|end| {
                                vertices.push(mesh_vertex(self.lattice.to_world(end.plan).extend(height(&end))));
                                vertices.len() as u32 - 1
                            });
                            lattice.push([one, other]);
                            lattice_cells.push(side_cells[side]);
                        }
                    }
                    continue;
                }
                lattice.push([roof_at[from], roof_at[to]]);
                lattice_cells.push(side_cells[side]);
                if !(flat(from) && flat(to)) {
                    lattice.push([floor_at[from], floor_at[to]]);
                    lattice_cells.push(side_cells[side]);
                }
            }
            for line in self.cut_lines(cell, &u_edges, &v_edges) {
                for stroke in self.cut_line_strokes(line) {
                    let first = vertices.len() as u32;
                    vertices.extend(stroke.map(mesh_vertex));
                    lattice.push([first, first + 1]);
                    lattice_cells.push([cell as u32, UNSET, UNSET, UNSET]);
                    cut_strokes.push([first, first + 1]);
                }
            }
            for corner in [a, a + 1, d + 1, d] {
                if !std::mem::replace(&mut seen[corner], true) && !flat(corner) {
                    let roof = place(&mut vertices, &mut roof_at, &self.roof, corner);
                    let floor = place(&mut vertices, &mut floor_at, &self.floor, corner);
                    lattice.push([roof, floor]);
                    let (node_column, node_row) = (corner % (columns + 1), corner / (columns + 1));
                    let mut around = [UNSET; 4];
                    let mut taken = 0;
                    for (dc, dr) in [(1, 1), (0, 1), (1, 0), (0, 0)] {
                        if node_column >= dc && node_row >= dr {
                            let found = neighbour(node_column - dc, node_row - dr);
                            if found != UNSET {
                                around[taken] = found;
                                taken += 1;
                            }
                        }
                    }
                    lattice_cells.push(around);
                }
            }
        }
        let (walls, wall_faces) = self.outline_walls();
        let offset = vertices.len() as u32;
        vertices.extend(walls.iter().map(|position| mesh_vertex(*position)));
        faces.extend(
            wall_faces
                .into_iter()
                .filter(|face| {
                    let [a, b, c] = face.map(|index| walls[index as usize]);
                    a != b && b != c && a != c
                })
                .map(|face| face.map(|index| index + offset)),
        );
        // Each wall quad runs from-roof, from-floor, to-roof, to-floor.
        let mut outline: Vec<[u32; 2]> = (0..walls.len() as u32 / 4)
            .flat_map(|quad| {
                let first = offset + 4 * quad;
                [[first, first + 2], [first + 1, first + 3]]
            })
            .collect();
        // The rim also runs along every line a trim cut inside a cell.
        outline.extend(cut_strokes);
        Some(PrismoidMesh {
            vertices,
            faces,
            outline,
            lattice,
            lattice_cells,
        })
    }

    /// A surface whose height is the vertical thickness with `None`: the
    /// roof's triangles of [`Self::closed_mesh`] with z = roof less floor at
    /// each corner, or at each point of a cut cell's triangles. With
    /// `Some(index)`, one set of values: a node at the centre of every
    /// active cell holding a finite value, z the value, and
    /// two triangles on each 2 by 2 block of such cells, so a cell without a
    /// value leaves a hole. `None` with no face, or an `index` naming no
    /// values.
    pub(crate) fn value_mesh(&self, index: Option<usize>) -> Option<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> {
        const UNSET: u32 = u32::MAX;
        let lattice = &self.lattice;
        let (columns, rows) = (lattice.columns(), lattice.rows());
        let mut vertices = Vec::new();
        let faces = match index {
            None => {
                let plan = lattice.corners();
                let mut at = vec![UNSET; plan.len()];
                let mut place = |corner: usize| {
                    if at[corner] == UNSET {
                        at[corner] = vertices.len() as u32;
                        vertices.push(mesh_vertex(plan[corner].extend(self.roof[corner] - self.floor[corner])));
                    }
                    at[corner]
                };
                let mut faces = quads(columns, rows, |column, row| {
                    let cell = row * columns + column;
                    (self.active[cell] && self.cut.cell_index(cell).is_none()).then(|| self.cell_corners(cell).map(&mut place))
                });
                let mut cut_at = vec![UNSET; self.cut.points.len()];
                for cell in &self.cut.cells {
                    if !self.active[*cell as usize] {
                        continue;
                    }
                    let Some(index) = self.cut.cell_index(*cell as usize) else { continue };
                    for triangle in self.cut.indices_at(index) {
                        let [a, b, c] = triangle.map(|point| self.cut.points[point as usize].plan);
                        let [x, y, z] = if (b - a).perp_dot(c - a) >= 0.0 {
                            *triangle
                        } else {
                            [triangle[0], triangle[2], triangle[1]]
                        };
                        faces.push([x, y, z].map(|point| {
                            if cut_at[point as usize] == UNSET {
                                cut_at[point as usize] = vertices.len() as u32;
                                let held = self.cut.points[point as usize];
                                vertices.push(mesh_vertex(self.lattice.to_world(held.plan).extend(held.roof - held.floor)));
                            }
                            cut_at[point as usize]
                        }));
                    }
                }
                faces
            }
            Some(index) => {
                let values = &self.cell_values.get(index)?.values;
                let mut at = vec![UNSET; lattice.cell_count()];
                for (cell, centre) in self.cell_centres() {
                    if let Some(value) = values[cell].filter(|value| value.is_finite() && self.active[cell]) {
                        at[cell] = vertices.len() as u32;
                        vertices.push(mesh_vertex(centre.extend(value)));
                    }
                }
                let node = |column: usize, row: usize| at[row * columns + column];
                quads(columns.saturating_sub(1), rows.saturating_sub(1), |column, row| {
                    let nodes = [node(column, row), node(column + 1, row), node(column + 1, row + 1), node(column, row + 1)];
                    nodes.iter().all(|node| *node != UNSET).then_some(nodes)
                })
            }
        };
        (!faces.is_empty()).then_some((vertices, faces))
    }

    /// One box per active prismoid with its cell, in cell order, in the
    /// lattice's frame: x along `u` and y along `v` from the origin, z the
    /// elevation. Top and bottom are the roof's and floor's mean heights over
    /// the two triangles [`Self::closed_mesh`] draws, so a box holds its
    /// prismoid's volume; for a prismoid a trim cut, its cut's means. A cell
    /// whose box has no height is left out.
    pub(crate) fn blocks(&self) -> Vec<(usize, BlockBounds)> {
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        let mut blocks = Vec::new();
        for (cell, _) in self.active.iter().enumerate().filter(|(_, active)| **active) {
            if let Some(bounds) = self.cell_box(cell, &u_edges, &v_edges) {
                blocks.push((cell, bounds));
            }
        }
        blocks
    }

    /// The box of one cell as [`Self::blocks`] gives it, from the lattice's
    /// edge distances; `None` when it has no height.
    fn cell_box(&self, cell: usize, u_edges: &[f64], v_edges: &[f64]) -> Option<BlockBounds> {
        let columns = self.lattice.columns();
        let mean = |heights: &[f64], [a, b, c, d]: [usize; 4]| (2.0 * heights[a] + heights[b] + 2.0 * heights[c] + heights[d]) / 6.0;
        let (column, row) = (cell % columns, cell / columns);
        let corners = self.cell_corners(cell);
        let [top, bottom] = self.cut.mean(cell).unwrap_or_else(|| [mean(&self.roof, corners), mean(&self.floor, corners)]);
        (top > bottom).then(|| BlockBounds {
            lower: DVec3::new(u_edges[column], v_edges[row], bottom),
            upper: DVec3::new(u_edges[column + 1], v_edges[row + 1], top),
        })
    }

    /// A cell's eight corner heights, in [`CORNER_HEIGHT_COLUMNS`] order (roof
    /// a, b, c, d, then floor a, b, c, d): the nodes' heights, and for a cut
    /// cell its box, the kept roof's mean at all four roof corners and the
    /// kept floor's mean at all four floor corners, so its two prisms hold
    /// exactly the cell's kept volume.
    pub(crate) fn cell_corner_heights(&self, cell: usize) -> [f64; 8] {
        if let Some([roof, floor]) = self.cut.mean(cell) {
            return [roof, roof, roof, roof, floor, floor, floor, floor];
        }
        let [a, b, c, d] = self.cell_corners(cell);
        [
            self.roof[a],
            self.roof[b],
            self.roof[c],
            self.roof[d],
            self.floor[a],
            self.floor[b],
            self.floor[c],
            self.floor[d],
        ]
    }

    /// The corner nodes of a cell, in [`CORNER_HEIGHT_COLUMNS`] order.
    pub(crate) fn cell_corners(&self, cell: usize) -> [usize; 4] {
        let columns = self.lattice.columns();
        let a = (cell / columns) * (columns + 1) + cell % columns;
        let d = a + columns + 1;
        [a, a + 1, d + 1, d]
    }

    /// The active cell a hit point on the drawn body (roof, floor or an
    /// outline wall) belongs to. A wall stands only where the neighbour is
    /// inactive or off the lattice, so of its two sides only one is a body.
    pub(crate) fn picked_cell(&self, at: DVec3) -> Option<usize> {
        let lattice = &self.lattice;
        let columns = lattice.columns();
        let slack = 1e-6 * lattice.least_size();
        let plan = at.truncate();
        let (u, v) = (lattice.u * slack, lattice.v * slack);
        let mut found: Vec<usize> = [DVec2::ZERO, u, -u, v, -v, u + v, u - v, v - u, -u - v]
            .into_iter()
            .filter_map(|shift| lattice.cell_at(plan + shift))
            .map(|(column, row)| row * columns + column)
            .filter(|cell| self.active[*cell])
            .collect();
        found.sort_unstable();
        found.dedup();
        let mut best: Option<(usize, f64)> = None;
        for cell in found {
            let distance = (lattice.cell_centre(cell % columns, cell / columns) - plan).length_squared();
            if best.is_none_or(|(_, nearest)| distance < nearest) {
                best = Some((cell, distance));
            }
        }
        best.map(|(cell, _)| cell)
    }

    /// The cell masked by hand that the ray from `origin` along unit
    /// `direction` meets first, with how far along the ray: on its roof or
    /// floor as [`Self::closed_mesh`] would draw them were it active, a cut
    /// cell by its cut triangles. A masked cell is drawn as a hole. Only hits
    /// at points `shown` keeps count, as a section's slab clips the view.
    pub(crate) fn picked_masked_cell(&self, origin: DVec3, direction: DVec3, shown: impl Fn(DVec3) -> bool) -> Option<(usize, f64)> {
        let plan = self.lattice.corners();
        let hit = |triangle: [DVec3; 3]| ray_triangle(origin, direction, triangle).filter(|distance| shown(origin + direction * *distance));
        let mut best: Option<(usize, f64)> = None;
        for cell in (0..self.masked.len()).filter(|cell| self.masked[*cell]) {
            let nearest = match self.cut.triangles_of(cell) {
                Some(triangles) => triangles
                    .flat_map(|points| {
                        let at = |height: fn(&CutPoint) -> f64| points.map(|point| self.lattice.to_world(point.plan).extend(height(&point)));
                        [at(|point| point.roof), at(|point| point.floor)]
                    })
                    .filter_map(hit)
                    .min_by(f64::total_cmp),
                None => {
                    let [a, b, c, d] = self.cell_corners(cell);
                    let roof = |corner: usize| plan[corner].extend(self.roof[corner]);
                    let floor = |corner: usize| plan[corner].extend(self.floor[corner]);
                    [
                        [roof(a), roof(b), roof(c)],
                        [roof(a), roof(c), roof(d)],
                        [floor(a), floor(c), floor(b)],
                        [floor(a), floor(d), floor(c)],
                    ]
                    .into_iter()
                    .filter_map(hit)
                    .min_by(f64::total_cmp)
                }
            };
            if let Some(distance) = nearest
                && best.is_none_or(|(_, held)| distance < held)
            {
                best = Some((cell, distance));
            }
        }
        best
    }

    /// Where the vertical plane through `point` with horizontal unit `normal`
    /// crosses the active cells, in order along the plane: distances from
    /// `point` towards `(-normal.y, normal.x)`, as a section frames them.
    /// Heights lie on the triangles [`Self::closed_mesh`] draws, so a piece
    /// across a cell's a-c diagonal gains a point on it.
    pub(crate) fn section_crossings(&self, point: DVec2, normal: DVec2) -> Vec<SectionCrossing> {
        const LEVEL: f64 = 1e-12;
        let lattice = &self.lattice;
        let columns = lattice.columns();
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let along = DVec2::new(-normal.y, normal.x);
        let inverse = DMat2::from_cols(lattice.u, lattice.v).inverse();
        let (start, slope) = (inverse * (point - lattice.origin), inverse * along);
        let axes = [(start.x, slope.x, &u_edges), (start.y, slope.y, &v_edges)];
        let (mut enter, mut leave) = (f64::NEG_INFINITY, f64::INFINITY);
        for (from, rate, edges) in axes {
            let total = edges[edges.len() - 1];
            if rate.abs() <= LEVEL {
                if !(0.0..=total).contains(&from) {
                    return Vec::new();
                }
            } else {
                let (first, last) = (-from / rate, (total - from) / rate);
                enter = enter.max(first.min(last));
                leave = leave.min(first.max(last));
            }
        }
        let crossed = enter.is_finite() && leave.is_finite() && enter < leave;
        if !crossed {
            return Vec::new();
        }
        let mut stops = vec![enter, leave];
        for (from, rate, edges) in axes {
            if rate.abs() > LEVEL {
                stops.extend(edges.iter().map(|edge| (edge - from) / rate).filter(|stop| enter < *stop && *stop < leave));
            }
        }
        stops.sort_by(f64::total_cmp);
        let shortest = 1e-9 * lattice.least_size();
        let mut crossings = Vec::new();
        for pair in stops.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if to - from <= shortest {
                continue;
            }
            // The middle of the piece, so a trace on a lattice line takes the
            // cells beyond it, as `cell_at` does.
            let Some((column, row)) = lattice.cell_at_in(point + along * ((from + to) / 2.0), &u_edges, &v_edges) else {
                continue;
            };
            let cell = row * columns + column;
            if !self.active[cell] {
                continue;
            }
            if let Some(index) = self.cut.cell_index(cell) {
                crossings.extend(self.cut_crossings(cell, index, (point, along, start, slope), [from, to], shortest));
                continue;
            }
            let ends = [from, to].map(|stop| point + along * stop);
            let [first, last] = ends.map(|plan| {
                let local = lattice.local_in(column, row, plan, &u_edges, &v_edges);
                local.x - local.y
            });
            let mut plans = vec![ends[0]];
            if first * last < 0.0 {
                plans.push(point + along * (from + (to - from) * first / (first - last)));
            }
            plans.push(ends[1]);
            let points = plans
                .into_iter()
                .map(|plan| {
                    let (roof, floor) = self.heights_at(cell, plan, &u_edges, &v_edges);
                    SectionPoint { plan, roof, floor }
                })
                .collect();
            crossings.push(SectionCrossing { cell, along: [from, to], points });
        }
        crossings
    }

    /// The crossings of a trace through a cut cell, cut triangle by
    /// triangle: one per run of kept ground along it, in order. The trace is
    /// `(point, along, start, slope)`: its origin and unit direction in plan,
    /// and the same in the lattice's frame; `span` is the stretch of it inside
    /// the cell.
    fn cut_crossings(&self, cell: usize, index: usize, (point, along, start, slope): (DVec2, DVec2, DVec2, DVec2), span: [f64; 2], shortest: f64) -> Vec<SectionCrossing> {
        const LEVEL: f64 = 1e-12;
        let tolerance = crate::model::kernel::XY_TOL;
        // The stretch of the trace in each triangle, with the kept roof and
        // floor at each end.
        let mut stretches: Vec<(f64, f64, [f64; 2], [f64; 2])> = Vec::new();
        for triangle in self.cut.triangles_at(index) {
            let plans = triangle.map(|held| held.plan);
            let facing = (plans[1] - plans[0]).perp_dot(plans[2] - plans[0]).signum();
            let [mut low, mut high] = span;
            for (one, other) in [(0, 1), (1, 2), (2, 0)] {
                let edge = plans[other] - plans[one];
                let length = edge.length();
                let (level, rate) = (facing * edge.perp_dot(start - plans[one]) / length, facing * edge.perp_dot(slope) / length);
                if rate.abs() <= LEVEL {
                    if level < -tolerance {
                        high = f64::NEG_INFINITY;
                    }
                } else {
                    let bound = -level / rate;
                    if rate > 0.0 {
                        low = low.max(bound);
                    } else {
                        high = high.min(bound);
                    }
                }
            }
            if high - low <= shortest {
                continue;
            }
            let heights = [low, high].map(|stop| {
                let weights = barycentric(plans, start + slope * stop);
                let blend = |value: fn(&CutPoint) -> f64| weights.iter().zip(&triangle).map(|(weight, held)| weight * value(held)).sum::<f64>();
                (blend(|held| held.roof), blend(|held| held.floor))
            });
            stretches.push((low, high, [heights[0].0, heights[0].1], [heights[1].0, heights[1].1]));
        }
        stretches.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Stretches that meet or overlap are one run of kept ground.
        let mut runs: Vec<Vec<(f64, f64, f64)>> = Vec::new();
        for (low, high, [roof_low, floor_low], [roof_high, floor_high]) in stretches {
            match runs.last_mut() {
                Some(run) if low <= run.last().map_or(f64::NEG_INFINITY, |end| end.0) + tolerance => {
                    if run.last().is_some_and(|end| high > end.0) {
                        run.push((high, roof_high, floor_high));
                    }
                }
                _ => runs.push(vec![(low, roof_low, floor_low), (high, roof_high, floor_high)]),
            }
        }
        runs.into_iter()
            .map(|run| SectionCrossing {
                cell,
                along: [run[0].0, run[run.len() - 1].0],
                points: run
                    .into_iter()
                    .map(|(stop, roof, floor)| SectionPoint {
                        plan: point + along * stop,
                        roof,
                        floor,
                    })
                    .collect(),
            })
            .collect()
    }

    /// Roof and floor over a plan point in a cell, on the triangles
    /// [`Self::closed_mesh`] draws (on a cut cell, on its cut triangles, the
    /// nearest point of them where the trim took the ground away): with
    /// corners a, b (along `u`), c and d (along `v`), `[a, b, c]` where the
    /// point is at least as far across the cell along `u` as along `v`, else
    /// `[a, c, d]`.
    fn heights_at(&self, cell: usize, plan: DVec2, u_edges: &[f64], v_edges: &[f64]) -> (f64, f64) {
        let lattice = &self.lattice;
        if let Some((roof, floor, _)) = self.cut.cell_index(cell).and_then(|index| self.cut.read(index, lattice.to_lattice(plan))) {
            return (roof, floor);
        }
        let columns = lattice.columns();
        let (column, row) = (cell % columns, cell / columns);
        let local = lattice.local_in(column, row, plan, u_edges, v_edges).clamp(DVec2::ZERO, DVec2::ONE);
        let corners = self.cell_corners(cell);
        let height = |z: &[f64]| height_in(z, corners, local);
        (height(&self.roof), height(&self.floor))
    }

    /// The roof's slope (dz/dx, dz/dy) on the triangle [`Self::heights_at`]
    /// takes for a plan point in a cell.
    fn roof_slope_in(&self, cell: usize, plan: DVec2, u_edges: &[f64], v_edges: &[f64]) -> DVec2 {
        let lattice = &self.lattice;
        if let Some((_, _, gradient)) = self.cut.cell_index(cell).and_then(|index| self.cut.read(index, lattice.to_lattice(plan))) {
            return lattice.u * gradient.x + lattice.v * gradient.y;
        }
        let columns = lattice.columns();
        let (column, row) = (cell % columns, cell / columns);
        let local = lattice.local_in(column, row, plan, u_edges, v_edges).clamp(DVec2::ZERO, DVec2::ONE);
        let [a, b, c, d] = self.cell_corners(cell).map(|corner| self.roof[corner]);
        let (along_u, along_v) = if local.x >= local.y { (b - a, c - b) } else { (c - d, d - a) };
        lattice.u * (along_u / lattice.u_sizes[column]) + lattice.v * (along_v / lattice.v_sizes[row])
    }

    /// Whether a plan point lies in the part of a cut cell the trim took
    /// away, so that nothing is read there (the drawing's nearest-point read
    /// is for drawing only).
    fn cut_misses(&self, cell: usize, plan: DVec2) -> bool {
        self.cut.cell_index(cell).is_some_and(|index| !self.cut.covers(index, self.lattice.to_lattice(plan)))
    }

    /// The roof, floor and roof slope over a plan point, on the triangles
    /// [`Self::closed_mesh`] draws, and nothing in the part of a cut cell the
    /// trim took away; why not, off the lattice or on a cell that is not
    /// active.
    pub(crate) fn reading_at(&self, plan: DVec2) -> std::result::Result<PlanReading, PlanMiss> {
        let (u_edges, v_edges) = self.plan_edges();
        let (cell, roof, floor) = self.read_in(plan, &u_edges, &v_edges)?;
        Ok(PlanReading {
            cell,
            roof,
            floor,
            roof_slope: self.roof_slope_in(cell, plan, &u_edges, &v_edges),
        })
    }

    /// The lattice's edge distances, for [`Self::heights_in`] on many points.
    pub(crate) fn plan_edges(&self) -> (Vec<f64>, Vec<f64>) {
        (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes))
    }

    /// The roof and floor of [`Self::reading_at`] from the lattice's edge
    /// distances, without the slope.
    pub(crate) fn heights_in(&self, plan: DVec2, u_edges: &[f64], v_edges: &[f64]) -> std::result::Result<(f64, f64), PlanMiss> {
        self.read_in(plan, u_edges, v_edges).map(|(_, roof, floor)| (roof, floor))
    }

    /// The cell, roof and floor over a plan point, or why not.
    fn read_in(&self, plan: DVec2, u_edges: &[f64], v_edges: &[f64]) -> std::result::Result<(usize, f64, f64), PlanMiss> {
        let lattice = &self.lattice;
        let (column, row) = lattice.cell_at_in(plan, u_edges, v_edges).ok_or(PlanMiss::Outside)?;
        let cell = row * lattice.columns() + column;
        if self.masked[cell] {
            return Err(PlanMiss::Masked);
        }
        if !self.active[cell] {
            return Err(PlanMiss::Inactive);
        }
        if self.cut_misses(cell, plan) {
            return Err(PlanMiss::Inactive);
        }
        let (roof, floor) = self.heights_at(cell, plan, u_edges, v_edges);
        Ok((cell, roof, floor))
    }

    /// The centres of the cells that are active and not masked.
    pub(crate) fn ground_centres(&self) -> impl Iterator<Item = DVec2> + '_ {
        self.cell_centres().filter(|(cell, _)| self.active[*cell] && !self.masked[*cell]).map(|(_, centre)| centre)
    }

    /// The active cell a point on a section lies in, between the floor and
    /// the roof as drawn; `None` above the roof, below the floor, off the
    /// lattice or on an inactive cell.
    pub(crate) fn section_cell_at(&self, at: DVec3) -> Option<usize> {
        let lattice = &self.lattice;
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let plan = at.truncate();
        let (column, row) = lattice.cell_at_in(plan, &u_edges, &v_edges)?;
        let cell = row * lattice.columns() + column;
        if !self.active[cell] || self.cut_misses(cell, plan) {
            return None;
        }
        let (roof, floor) = self.heights_at(cell, plan, &u_edges, &v_edges);
        (floor <= at.z && at.z <= roof).then_some(cell)
    }

    /// The boxes [`Self::blocks`] gives in the frame of a block model made
    /// from the grid: the lattice's, or with an `envelope` (the grid on its
    /// axes) the envelope's, from its corner at the base RL.
    pub(crate) fn model_blocks(&self, envelope: Option<&ModelEnvelope>) -> Vec<(usize, BlockBounds)> {
        let blocks = self.blocks();
        let Some(envelope) = envelope else { return blocks };
        let shift = self.lattice.origin.extend(0.0) - envelope.lower();
        blocks
            .into_iter()
            .map(|(cell, block)| {
                let (lower, upper) = (block.lower + shift, block.upper + shift);
                (cell, BlockBounds { lower, upper })
            })
            .collect()
    }

    /// One active cell's box in the lattice's frame, as [`Self::blocks`]
    /// gives it, measured as `block_volumes` and `per_volume_values` measure
    /// a block model's; `None` when the box has no height.
    pub(crate) fn cell_measure(&self, cell: usize) -> Option<(BlockBounds, CellMeasure)> {
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        self.measure_in(cell, &u_edges, &v_edges, self.stretch())
    }

    /// [`Self::cell_measure`] from the lattice's edge distances and stretch.
    fn measure_in(&self, cell: usize, u_edges: &[f64], v_edges: &[f64], stretch: f64) -> Option<(BlockBounds, CellMeasure)> {
        let bounds = self.cell_box(cell, u_edges, v_edges)?;
        let measure = CellMeasure {
            cell,
            sides: (bounds.upper - bounds.lower).element_product(),
            stretch,
        };
        Some((bounds, measure))
    }

    /// Each active cell with its box's centre: the cell's plan centre at the
    /// middle of its box's height. A cell whose box has no height is left out.
    pub(crate) fn cell_centres_3d(&self) -> impl Iterator<Item = (usize, DVec3)> + '_ {
        self.centres_3d_of(&self.active)
    }

    /// [`Self::cell_centres_3d`] of the cells set in `cells`, a mask in cell
    /// order.
    pub(crate) fn centres_3d_of<'a>(&'a self, cells: &'a [bool]) -> impl Iterator<Item = (usize, DVec3)> + 'a {
        let lattice = &self.lattice;
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let columns = lattice.columns();
        cells.iter().enumerate().filter(|(_, set)| **set).filter_map(move |(cell, _)| {
            let bounds = self.cell_box(cell, &u_edges, &v_edges)?;
            let centre = lattice.cell_centre_in(cell % columns, cell / columns, &u_edges, &v_edges);
            Some((cell, centre.extend((bounds.lower.z + bounds.upper.z) / 2.0)))
        })
    }

    /// How much the lattice's frame scales a volume.
    fn stretch(&self) -> f64 {
        let lattice = &self.lattice;
        DMat3::from_cols(lattice.u.extend(0.0), lattice.v.extend(0.0), DVec3::Z).determinant().abs()
    }

    /// Thickness at every corner as a position from 0 to 1 across `range`,
    /// 0.5 where the range is flat. With a cut prismoid, instead the kept
    /// thickness at four texels along each cell side (centres on the quarter
    /// lines), so colour reaches zero where the kept ground ends. `None`
    /// unless the lattice is regular: the map to the texture is affine, and a
    /// tensor lattice's corners are not evenly spaced.
    pub(crate) fn thickness_map(&self, range: (f64, f64), max_side: u32) -> Option<ThicknessMap> {
        let lattice = &self.lattice;
        if !lattice.is_regular() {
            return None;
        }
        if !self.cut.is_empty() {
            return Some(self.cut_thickness_map(range, max_side));
        }
        // A side too long for a texture keeps every `step`-th corner, its
        // last texel clamped to the last corner.
        let (along_u, along_v) = (texel_fit(lattice.columns(), max_side, 1), texel_fit(lattice.rows(), max_side, 1));
        let ((column_step, width), (row_step, height)) = (along_u, along_v);
        let (columns, rows) = (lattice.columns() + 1, lattice.rows() + 1);
        let positions = (0..height)
            .flat_map(|row| (0..width).map(move |column| (column, row)))
            .map(|(column, row)| {
                let corner = (row * row_step).min(rows - 1) * columns + (column * column_step).min(columns - 1);
                ramp_position(self.roof[corner] - self.floor[corner], range)
            })
            .collect();
        // Texel centres on the corners kept.
        Some(ThicknessMap {
            size: [width as u32, height as u32],
            positions,
            world_to_uv: texel_world_to_uv(lattice, along_u, along_v, 1, 0.5),
        })
    }

    /// [`Self::thickness_map`] of a regular lattice with cut prismoids: a
    /// texel on every quarter line of a cell side, corners included; a cut
    /// cell's from its triangles, any other from its corners' triangles.
    fn cut_thickness_map(&self, range: (f64, f64), max_side: u32) -> ThicknessMap {
        use rayon::prelude::*;
        const QUARTERS: usize = 4;
        let lattice = &self.lattice;
        let (columns, rows) = (lattice.columns(), lattice.rows());
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let (along_u, along_v) = (texel_fit(QUARTERS * columns, max_side, 1), texel_fit(QUARTERS * rows, max_side, 1));
        let ((column_step, width), (row_step, height)) = (along_u, along_v);
        // The cell a quarter line lies in, the one beyond it on a cell edge,
        // and where in it.
        let place = |quarter: usize, cells: usize| {
            let cell = (quarter / QUARTERS).min(cells - 1);
            (cell, (quarter - QUARTERS * cell) as f64 / QUARTERS as f64)
        };
        let positions = (0..height)
            .into_par_iter()
            .flat_map_iter(|texel_row| {
                let (row, local_v) = place((texel_row * row_step).min(QUARTERS * rows), rows);
                let (u_edges, v_edges) = (&u_edges, &v_edges);
                (0..width).map(move |texel_column| {
                    let (column, local_u) = place((texel_column * column_step).min(QUARTERS * columns), columns);
                    let (cell, local) = (row * columns + column, DVec2::new(local_u, local_v));
                    let at = DVec2::new(u_edges[column] + local.x * lattice.u_sizes[column], v_edges[row] + local.y * lattice.v_sizes[row]);
                    let (roof, floor) = match self.cut.cell_index(cell) {
                        Some(index) => self.cut.read(index, at).map_or((0.0, 0.0), |(roof, floor, _)| (roof, floor)),
                        None => {
                            let corners = self.cell_corners(cell);
                            (height_in(&self.roof, corners, local), height_in(&self.floor, corners, local))
                        }
                    };
                    ramp_position(roof - floor, range)
                })
            })
            .collect();
        ThicknessMap {
            size: [width as u32, height as u32],
            positions,
            world_to_uv: texel_world_to_uv(lattice, along_u, along_v, QUARTERS, 0.5),
        }
    }

    /// One cell value as positions from 0 to 1 across `range` (0.5 where the
    /// range is flat), a texel per cell inside a ring of one. `None` unless
    /// the lattice is regular, or when `index` names no set of values.
    pub(crate) fn cell_value_map(&self, index: usize, range: (f64, f64), max_side: u32) -> Option<CellValueMap> {
        let lattice = &self.lattice;
        if !lattice.is_regular() {
            return None;
        }
        let values = &self.cell_values.get(index)?.values;
        let columns = lattice.columns();
        // A side too long for a texture keeps every `step`-th cell.
        let (along_u, along_v) = (texel_fit(columns, max_side, 2), texel_fit(lattice.rows(), max_side, 2));
        let ((column_step, width), (row_step, height)) = (along_u, along_v);
        let position = |cell: usize| Some(ramp_position(values[cell].filter(|value| value.is_finite())?, range));
        // Outer `None`: the texel is not an active cell.
        let held: Vec<Option<Option<f32>>> = (0..width * height)
            .map(|texel| {
                let (i, j) = (texel % width, texel / width);
                if i == 0 || j == 0 || i == width - 1 || j == height - 1 {
                    return None;
                }
                let cell = (j - 1) * row_step * columns + (i - 1) * column_step;
                self.active[cell].then(|| position(cell))
            })
            .collect();
        // A wall stands on a cell edge, a texel boundary, so its fragments
        // fall either side by rounding; the neighbour holding the same
        // colour makes a straight wall read as its cell.
        const AROUND: [(isize, isize); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
        let positions = (0..width * height)
            .map(|texel| {
                if let Some(own) = held[texel] {
                    return own;
                }
                let (i, j) = (texel % width, texel / width);
                AROUND
                    .iter()
                    .find_map(|(di, dj)| {
                        let (x, y) = (i.checked_add_signed(*di)?, j.checked_add_signed(*dj)?);
                        if x >= width || y >= height {
                            return None;
                        }
                        held[y * width + x]
                    })
                    .flatten()
            })
            .collect();
        // Texel i lies over kept column i - 1, so a cell's centre lands on
        // its texel's centre.
        Some(CellValueMap {
            size: [width as u32, height as u32],
            positions,
            world_to_uv: texel_world_to_uv(lattice, along_u, along_v, 1, 1.0),
        })
    }

    /// The lattice size, the active cells, the range of roof minus floor
    /// over every corner of an active cell, and the totals: a derived block
    /// model's bit for bit without an envelope, to rounding with one; and
    /// the cells masked by hand.
    pub(crate) fn summary(&self) -> PrismoidGridSummary {
        let masked_cells = self.masked.iter().filter(|masked| **masked).count();
        PrismoidGridSummary {
            masked_cells,
            ..self.summary_of(self.active.iter().enumerate().filter(|(_, active)| **active).map(|(cell, _)| cell))
        }
    }

    /// [`Self::summary`] over the given active cells, taken in cell order:
    /// `active_cells` counts them, and `masked_cells` is 0.
    pub(crate) fn summary_of(&self, cells: impl Iterator<Item = usize>) -> PrismoidGridSummary {
        let (columns, rows) = (self.lattice.columns(), self.lattice.rows());
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        let stretch = self.stretch();
        let mut thickness: Option<(f64, f64)> = None;
        let mut active_cells = 0;
        let density = self.cell_values.iter().find(|values| values.role == CellValueRole::Density);
        let mut tonnes = density.map(|_| NumericTotal::default());
        let mut qualities: Vec<(&CellValues, NumericTotal)> = self
            .cell_values
            .iter()
            .filter(|values| values.role == CellValueRole::Quality)
            .map(|values| (values, NumericTotal::default()))
            .collect();
        let mut spreads: Vec<ValueSpread> = self
            .cell_values
            .iter()
            .map(|values| ValueSpread {
                name: values.name.clone(),
                unit: values.unit.clone(),
                range: None,
                missing: 0,
            })
            .collect();
        let mut volume = 0.0;
        for cell in cells {
            active_cells += 1;
            let (column, row) = (cell % columns, cell / columns);
            for corner in [
                row * (columns + 1) + column,
                row * (columns + 1) + column + 1,
                (row + 1) * (columns + 1) + column,
                (row + 1) * (columns + 1) + column + 1,
            ] {
                let height = self.roof[corner] - self.floor[corner];
                thickness = Some(thickness.map_or((height, height), |(min, max)| (min.min(height), max.max(height))));
            }
            for point in self.cut.triangles_of(cell).into_iter().flatten().flatten() {
                let height = point.roof - point.floor;
                thickness = thickness.map(|(min, max)| (min.min(height), max.max(height)));
            }
            for (values, spread) in self.cell_values.iter().zip(&mut spreads) {
                spread.take(values.values[cell]);
            }
            // A box with no height holds nothing, as `blocks` leaves it out.
            let Some((_, measure)) = self.measure_in(cell, &u_edges, &v_edges, stretch) else {
                continue;
            };
            let cell_volume = measure.volume();
            volume += cell_volume;
            let cell_tonnes = density.map(|density| density.values[measure.cell].map(|value| measure.per_volume(value)));
            if let Some(total) = tonnes.as_mut() {
                add_summed(total, cell_tonnes.flatten());
            }
            // By tonnes where the grid has a density, never mixed with volume.
            let weight = cell_tonnes.unwrap_or(Some(cell_volume));
            for (values, total) in &mut qualities {
                add_weighted(total, values.values[measure.cell], weight);
            }
        }
        PrismoidGridSummary {
            columns,
            rows,
            active_cells,
            masked_cells: 0,
            vertical_thickness: thickness,
            volume,
            weighting: if tonnes.is_some() { Weighting::ByTonnes } else { Weighting::ByVolume },
            tonnes,
            density: density.map(|density| density.name.clone()),
            qualities: qualities
                .into_iter()
                .map(|(values, total)| QualityTotal {
                    name: values.name.clone(),
                    unit: values.unit.clone(),
                    total,
                })
                .collect(),
            spreads,
        }
    }

    /// The value each lattice line shows, from the active cells sharing it
    /// (`cells`, one entry per line as [`PrismoidMesh::lattice_cells`]): the
    /// mean of value `index` (or of thickness with `None`) weighted as
    /// [`Self::summary_of`] weights qualities, by tonnes where the grid has a
    /// density, else by volume. Signs are kept. `None` where no cell holds
    /// both a value and a usable weight.
    pub(crate) fn wire_values(&self, cells: &[[u32; 4]], index: Option<usize>) -> Vec<Option<f64>> {
        let (u_edges, v_edges) = (edges(&self.lattice.u_sizes), edges(&self.lattice.v_sizes));
        let stretch = self.stretch();
        let density = self.cell_values.iter().find(|values| values.role == CellValueRole::Density);
        let values = index.map(|index| self.cell_values.get(index));
        let per_cell: Vec<Option<(f64, f64)>> = (0..self.active.len())
            .map(|cell| {
                if !self.active[cell] {
                    return None;
                }
                let (bounds, measure) = self.measure_in(cell, &u_edges, &v_edges, stretch)?;
                let value = match values {
                    None => Some(bounds.upper.z - bounds.lower.z),
                    Some(values) => values?.values[cell],
                }
                .filter(|value| value.is_finite())?;
                let weight = match density {
                    Some(density) => density.values[cell].map(|value| measure.per_volume(value)),
                    None => Some(measure.volume()),
                }
                .filter(|weight| weight.is_finite() && *weight > 0.0)?;
                Some((value, weight))
            })
            .collect();
        cells
            .iter()
            .map(|line| {
                let (mut sum, mut weights) = (0.0, 0.0);
                let mut counted = false;
                for (value, weight) in line
                    .iter()
                    .filter(|cell| **cell != u32::MAX)
                    .filter_map(|cell| per_cell.get(*cell as usize).copied().flatten())
                {
                    sum += value * weight;
                    weights += weight;
                    counted = true;
                }
                counted.then_some(sum / weights)
            })
            .collect()
    }

    /// Bytes held by the heights, the masks, the cell values and the cut
    /// prismoids.
    pub(crate) fn estimated_bytes(&self) -> usize {
        let values: usize = self
            .cell_values
            .iter()
            .map(|cell_values| cell_values.name.len() + cell_values.unit.len() + cell_values.values.len() * size_of::<Option<f64>>())
            .sum();
        (self.roof.len() + self.floor.len()) * size_of::<f64>() + self.active.len() + self.masked.len() + values + self.cut.estimated_bytes()
    }

    /// A copy with `cells` masked by hand, or with `mask` false unmasked, and
    /// how many changed. A mask takes only active cells and an unmask only
    /// cells masked by hand, so a cell the build left out stays out. `None`
    /// when no cell changes.
    pub(crate) fn with_mask(&self, cells: impl IntoIterator<Item = usize>, mask: bool) -> Option<(PrismoidGrid, usize)> {
        let mut grid: Option<PrismoidGrid> = None;
        let mut changed = 0;
        for cell in cells {
            let held = grid.as_ref().unwrap_or(self);
            if (mask && held.active[cell]) || (!mask && held.masked[cell]) {
                let grid = grid.get_or_insert_with(|| self.clone());
                (grid.active[cell], grid.masked[cell]) = (!mask, mask);
                changed += 1;
            }
        }
        grid.map(|grid| (grid, changed))
    }

    /// The cells a trim takes: the active ones and those masked by hand.
    fn taken(&self) -> Vec<bool> {
        self.active.iter().zip(&self.masked).map(|(active, masked)| *active || *masked).collect()
    }

    /// `read` at every corner node (at `plan`) of the cells `taken` marks,
    /// rows in parallel.
    fn read_trim_nodes(
        &self,
        read: &(impl Fn(&mut Vec<usize>, DVec2) -> SurfaceAt + Sync),
        taken: &[bool],
        plan: &[DVec2],
        cancel: &CancelFlag,
    ) -> Result<Vec<Option<SurfaceAt>>, TrimRefusal> {
        use rayon::prelude::*;
        let (columns, rows) = (self.lattice.columns(), self.lattice.rows());
        let mut needed = vec![false; plan.len()];
        for cell in (0..taken.len()).filter(|cell| taken[*cell]) {
            for node in self.cell_corners(cell) {
                needed[node] = true;
            }
        }
        Ok((0..=rows)
            .into_par_iter()
            .map_init(Vec::new, |stack, row| {
                (!cancel.is_cancelled()).then(|| {
                    (row * (columns + 1)..(row + 1) * (columns + 1))
                        .map(|node| needed[node].then(|| read(stack, plan[node])))
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(TrimRefusal::Cancelled)?
            .concat())
    }

    /// A copy trimmed to a surface, cut exactly along it. `read` gives the
    /// surface at a plan point (with a search stack of its own per thread),
    /// and `overlaps` hands its visitor each face of the surface over a plan
    /// triangle with the exact plan overlap of the two (its points' heights
    /// mean nothing). The triangle and what the visitor gets are relative to
    /// the lattice origin, which `overlaps` takes first: small cells far from
    /// the world origin keep their precision. A plan overlap may exceed its
    /// triangle by `slack` of area before it is a fold.
    ///
    /// The surface is read at every corner node, held between the floor and
    /// the roof, and taken as the roof with `keep_below`, else as the floor.
    /// Each active or hand-masked cell is cut over its two lattice triangles
    /// (or the triangles an earlier trim cut it into) against every surface
    /// face over them: left out where the surface does not cover it, gone
    /// where the surface keeps none of it, kept as it was where it keeps
    /// all, and otherwise cut, its kept roof (or floor) following the
    /// surface inside it. The two sides' kept thicknesses add up to the
    /// source's at every point. A cell the surface covers twice over (it folds
    /// there), or a cut grid past `budget` triangles, refuses the whole trim;
    /// a step standing upright on a node line does not.
    ///
    /// Where the surface lies within `allowance` metres of a roof or floor it
    /// is taken as exactly on it (the nearer one where two are that close),
    /// before the cut; 0 takes nothing as touching. The plan area taken so
    /// is in [`Trimmed::touching_area`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn trimmed(
        &self,
        read: impl Fn(&mut Vec<usize>, DVec2) -> SurfaceAt + Sync,
        overlaps: impl Fn(DVec2, &mut Vec<usize>, &mut Vec<DVec3>, [DVec2; 3], &mut dyn FnMut([DVec3; 3], &[DVec3])) + Sync,
        slack: f64,
        keep_below: bool,
        allowance: f64,
        budget: usize,
        cancel: &CancelFlag,
        progress: &Phase,
    ) -> Result<Trimmed, TrimRefusal> {
        use std::sync::atomic::AtomicBool;

        use rayon::prelude::*;
        let lattice = &self.lattice;
        let (columns, rows, cells) = (lattice.columns(), lattice.rows(), lattice.cell_count());
        let plan = lattice.corners();
        let relative = lattice.corners_from(DVec2::ZERO);
        let taken = self.taken();
        let nodes = self.read_trim_nodes(&read, &taken, &plan, cancel)?;
        progress.set_fraction(0.1);
        let (mut roof, mut floor) = (self.roof.clone(), self.floor.clone());
        for (node, at) in nodes.iter().enumerate() {
            // Where the surface gives two heights the node takes the one that
            // leaves the whole cells beside it as they were: the highest to
            // keep below, the lowest to keep above.
            let level = match at {
                Some(SurfaceAt::At(z)) => Some(*z),
                Some(SurfaceAt::Folded { lowest, highest }) => Some(if keep_below { *highest } else { *lowest }),
                _ => None,
            };
            if let Some(z) = level {
                let (node_roof, node_floor) = (self.roof[node], self.floor[node]);
                let height = if allowance > 0.0 {
                    Snap::of(z, node_roof, node_floor, allowance).level_of(z, node_roof, node_floor)
                } else {
                    z
                };
                let height = height.clamp(node_floor, node_roof);
                if keep_below {
                    roof[node] = height;
                } else {
                    floor[node] = height;
                }
            }
        }
        let (stop, stored, swept) = (AtomicBool::new(false), AtomicUsize::new(0), AtomicUsize::new(0));
        let sweep = progress.phase(0.1, 0.9);
        let swept_rows: Option<Vec<Vec<(usize, CellCut, f64)>>> = (0..rows)
            .into_par_iter()
            .map_init(CutScratch::default, |scratch, row| {
                let mut found = Vec::new();
                for cell in (row * columns..(row + 1) * columns).filter(|cell| taken[*cell]) {
                    if stop.load(Ordering::Relaxed) || cancel.is_cancelled() {
                        return None;
                    }
                    let beyond = self.cell_corners(cell).iter().any(|node| nodes[*node] == Some(SurfaceAt::Beyond));
                    scratch.touching = 0.0;
                    let outcome = if beyond {
                        CellCut::Beyond
                    } else {
                        self.cut_cell(cell, &relative, &overlaps, slack, keep_below, allowance, scratch)
                    };
                    let held = match &outcome {
                        CellCut::Whole => self.cut.cell_index(cell).map_or(0, |index| (self.cut.starts[index + 1] - self.cut.starts[index]) as usize),
                        CellCut::Cut(_, triangles, _) => triangles.len(),
                        _ => 0,
                    };
                    if held > 0 && stored.fetch_add(held, Ordering::Relaxed) + held > budget {
                        stop.store(true, Ordering::Relaxed);
                        return None;
                    }
                    found.push((cell, outcome, scratch.touching));
                }
                sweep.set_items(swept.fetch_add(1, Ordering::Relaxed) as u64 + 1, rows as u64);
                Some(found)
            })
            .collect();
        if stop.load(Ordering::Relaxed) {
            return Err(TrimRefusal::TooLarge { budget });
        }
        let swept_rows = swept_rows.ok_or(TrimRefusal::Cancelled)?;
        let folded_cells = swept_rows
            .iter()
            .flatten()
            .filter(|(_, outcome, _)| matches!(outcome, CellCut::Folded))
            .map(|(cell, _, _)| *cell);
        let mut folds = folded_cells.map(|cell| lattice.cell_centre(cell % columns, cell / columns));
        if let Some(first) = folds.next() {
            return Err(TrimRefusal::Folded { count: folds.count() + 1, first });
        }
        let mut trimmed = Trimmed {
            grid: None,
            whole: 0,
            cut: 0,
            gone: 0,
            beyond: 0,
            reached: self.summary_of(std::iter::empty()),
            triangles: 0,
            roof_contacts: Vec::new(),
            floor_contacts: Vec::new(),
            touching_area: 0.0,
        };
        let (mut active, mut masked) = (vec![false; cells], vec![false; cells]);
        let mut reached = vec![false; cells];
        let mut cut_cells = Vec::new();
        let mut made = Vec::new();
        for (cell, outcome, touching) in swept_rows.into_iter().flatten() {
            let was_active = usize::from(self.active[cell]);
            if self.active[cell] {
                trimmed.touching_area += touching;
            }
            match outcome {
                CellCut::Beyond => {
                    trimmed.beyond += was_active;
                    continue;
                }
                CellCut::Folded => continue,
                CellCut::Gone => trimmed.gone += was_active,
                CellCut::Whole => {
                    (active[cell], masked[cell]) = (self.active[cell], self.masked[cell]);
                    trimmed.whole += was_active;
                    if let Some(triangles) = self.cut.triangles_of(cell) {
                        let points: Vec<CutPoint> = triangles.flatten().collect();
                        let indices = (0..points.len() as u32 / 3).map(|triangle| [3 * triangle, 3 * triangle + 1, 3 * triangle + 2]).collect();
                        cut_cells.push((cell, points, indices));
                    }
                }
                CellCut::Cut(points, triangles, contacts) => {
                    (active[cell], masked[cell]) = (self.active[cell], self.masked[cell]);
                    trimmed.cut += was_active;
                    cut_cells.push((cell, points, triangles));
                    made.push((cell, contacts));
                }
            }
            reached[cell] = self.active[cell];
        }
        let cut = CutPrismoids::build(lattice, cut_cells);
        // Triangles too small to keep leave a cut with nothing: gone.
        for (cell, contacts) in made {
            match cut.cell_index(cell) {
                Some(index) => {
                    trimmed.triangles += (cut.starts[index + 1] - cut.starts[index]) as usize;
                    if self.active[cell] {
                        trimmed.roof_contacts.extend(contacts.roof);
                        trimmed.floor_contacts.extend(contacts.floor);
                    }
                }
                None => {
                    let was_active = usize::from(self.active[cell]);
                    (active[cell], masked[cell]) = (false, false);
                    (trimmed.cut, trimmed.gone) = (trimmed.cut - was_active, trimmed.gone + was_active);
                }
            }
        }
        trimmed.reached = self.summary_of((0..cells).filter(|cell| reached[*cell]));
        if active.contains(&true) {
            let grid = PrismoidGrid {
                lattice: lattice.clone(),
                roof,
                floor,
                active,
                masked,
                cell_values: self.cell_values.clone(),
                cut,
            };
            grid.check().map_err(TrimRefusal::Malformed)?;
            trimmed.grid = Some(grid);
        }
        progress.set_fraction(1.0);
        Ok(trimmed)
    }

    /// What the surface does to one cell, over its pieces (see
    /// [`Self::trimmed`]): the plan overlaps of each piece with the surface's
    /// faces, then whole, gone or cut by what height the surface keeps at
    /// their points. With an `allowance` above 0 the surface is first taken
    /// as on a roof or floor within that distance (see [`Snap`]), and the plan
    /// area so taken is left in `scratch.touching`. `plan` holds the corners
    /// relative to the lattice origin, and the work is done there.
    #[allow(clippy::too_many_arguments)]
    fn cut_cell(
        &self,
        cell: usize,
        plan: &[DVec2],
        overlaps: &(impl Fn(DVec2, &mut Vec<usize>, &mut Vec<DVec3>, [DVec2; 3], &mut dyn FnMut([DVec3; 3], &[DVec3])) + Sync),
        slack: f64,
        keep_below: bool,
        allowance: f64,
        scratch: &mut CutScratch,
    ) -> CellCut {
        let lattice = &self.lattice;
        let CutScratch {
            stack,
            overlap,
            pieces,
            verts,
            polys,
            touching,
        } = scratch;
        pieces.clear();
        verts.clear();
        polys.clear();
        match self.cut.triangles_of(cell) {
            Some(triangles) => pieces.extend(triangles.map(|triangle| Piece {
                plan: triangle.map(|point| lattice.u * point.plan.x + lattice.v * point.plan.y),
                roof: triangle.map(|point| point.roof),
                floor: triangle.map(|point| point.floor),
            })),
            None => {
                let [a, b, c, d] = self.cell_corners(cell);
                pieces.extend([[a, b, c], [a, c, d]].map(|nodes| Piece {
                    plan: nodes.map(|node| plan[node]),
                    roof: nodes.map(|node| self.roof[node]),
                    floor: nodes.map(|node| self.floor[node]),
                }));
            }
        }
        let (mut least_over_roof, mut most_over_floor) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut folded, mut beyond) = (false, false);
        for (index, piece) in pieces.iter().enumerate() {
            let area = ((piece.plan[1] - piece.plan[0]).perp_dot(piece.plan[2] - piece.plan[0]) / 2.0).abs();
            let mut covered = 0.0;
            overlaps(lattice.origin, stack, overlap, piece.plan, &mut |ground, shared| {
                let ground_plan = ground.map(|point| point.truncate());
                let ground_z = ground.map(|point| point.z);
                let start = verts.len();
                for point in shared {
                    let at = point.truncate();
                    let vert = Vert {
                        plan: at,
                        roof: piece.at(&piece.roof, at),
                        floor: piece.at(&piece.floor, at),
                        ground: height_on(&ground_plan, &ground_z, at),
                    };
                    least_over_roof = least_over_roof.min(vert.ground - vert.roof);
                    most_over_floor = most_over_floor.max(vert.ground - vert.floor);
                    verts.push(vert);
                }
                covered += polygon_area(&verts[start..]);
                polys.push(Poly {
                    piece: index,
                    range: start..verts.len(),
                    ground_plan,
                    ground_z,
                    snap: Snap::None,
                });
            });
            if covered > area * (1.0 + 1e-9) + slack {
                folded = true;
                break;
            }
            beyond |= covered < area * (1.0 - 1e-9) - slack;
        }
        if folded {
            return CellCut::Folded;
        }
        if beyond {
            return CellCut::Beyond;
        }
        if allowance > 0.0 {
            // Split each overlap into parts on which the surface taken as
            // touching is linear, and read whole and gone off those.
            (least_over_roof, most_over_floor) = (f64::INFINITY, f64::NEG_INFINITY);
            for poly in std::mem::take(polys) {
                let piece = &pieces[poly.piece];
                let at = |plan: DVec2| Vert {
                    plan,
                    roof: piece.at(&piece.roof, plan),
                    floor: piece.at(&piece.floor, plan),
                    ground: height_on(&poly.ground_plan, &poly.ground_z, plan),
                };
                let polygon = verts[poly.range.clone()].to_vec();
                for (part, snap) in snap_parts(&polygon, allowance, &at) {
                    let start = verts.len();
                    for vert in part {
                        let level = snap.level(&vert);
                        least_over_roof = least_over_roof.min(level - vert.roof);
                        most_over_floor = most_over_floor.max(level - vert.floor);
                        verts.push(vert);
                    }
                    if snap != Snap::None {
                        *touching += polygon_area(&verts[start..]);
                    }
                    polys.push(Poly {
                        range: start..verts.len(),
                        snap,
                        ..poly.clone()
                    });
                }
            }
        }
        let (whole, gone) = if keep_below {
            (least_over_roof >= -CUT_CONTACT, most_over_floor <= CUT_CONTACT)
        } else {
            (most_over_floor <= CUT_CONTACT, least_over_roof >= -CUT_CONTACT)
        };
        if whole {
            return CellCut::Whole;
        }
        if gone {
            return CellCut::Gone;
        }
        let pick: fn(f64, f64) -> f64 = if keep_below { f64::min } else { f64::max };
        let kept = |vert: &Vert, snap: Snap| {
            let level = snap.level(vert);
            CutPoint {
                plan: lattice.to_lattice_from_origin(vert.plan),
                roof: pick(vert.roof, level),
                floor: pick(vert.floor, level),
            }
        };
        let (mut points, mut triangles) = (Vec::new(), Vec::new());
        let mut contacts = Contacts::default();
        for poly in polys.iter() {
            let piece = &pieces[poly.piece];
            let at = |plan: DVec2| Vert {
                plan,
                roof: piece.at(&piece.roof, plan),
                floor: piece.at(&piece.floor, plan),
                ground: height_on(&poly.ground_plan, &poly.ground_z, plan),
            };
            let whole = &verts[poly.range.clone()];
            let level = |vert: &Vert| poly.snap.level(vert);
            if allowance > 0.0 {
                snap_contacts(whole, poly, piece, allowance, &at, &mut contacts);
            }
            contacts.floor.extend(contact_of(whole, |vert| level(vert) - vert.floor, &at));
            let (under, rest) = split_convex(whole, |vert| level(vert) - vert.floor, &at);
            contacts.roof.extend(contact_of(&rest, |vert| level(vert) - vert.roof, &at));
            let (inside, over) = split_convex(&rest, |vert| level(vert) - vert.roof, &at);
            // Ground taken as on the roof keeps nothing above it: its part
            // is all `inside`, with no thickness.
            let keeps = match (keep_below, poly.snap) {
                (true, _) => [inside, over],
                (false, Snap::Roof) => [under, Vec::new()],
                (false, _) => [under, inside],
            };
            for part in keeps.iter().filter(|part| part.len() >= 3) {
                let first = points.len() as u32;
                points.extend(part.iter().map(|vert| kept(vert, poly.snap)));
                for index in 1..part.len() as u32 - 1 {
                    let triangle = [first, first + index, first + index + 1];
                    if cut_area(triangle.map(|point| points[point as usize])) > 0.0 {
                        triangles.push(triangle);
                    }
                }
            }
        }
        // The strings are drawn in the world.
        let world = lattice.origin.extend(0.0);
        for end in contacts.roof.iter_mut().chain(&mut contacts.floor).flatten() {
            *end += world;
        }
        if triangles.is_empty() {
            CellCut::Gone
        } else {
            CellCut::Cut(points, triangles, contacts)
        }
    }
}

/// What a trim does to one cell.
enum CellCut {
    Beyond,
    Folded,
    Gone,
    Whole,
    /// Cut: its own points (plan in the lattice frame) and triangles.
    Cut(Vec<CutPoint>, Vec<[u32; 3]>, Contacts),
}

/// Where the surface meets the roofs and floors inside one cut cell, as
/// world segments at the surface's height there.
#[derive(Default)]
struct Contacts {
    roof: Vec<[DVec3; 2]>,
    floor: Vec<[DVec3; 2]>,
}

/// One triangle of a cell in plan, relative to the lattice origin, with the
/// roof and floor at its points.
struct Piece {
    plan: [DVec2; 3],
    roof: [f64; 3],
    floor: [f64; 3],
}

impl Piece {
    /// The linear height `z` gives on this triangle at a plan point.
    fn at(&self, z: &[f64; 3], at: DVec2) -> f64 {
        height_on(&self.plan, z, at)
    }
}

/// A point of an overlap polygon with the roof, floor and ground there.
#[derive(Clone, Copy)]
struct Vert {
    plan: DVec2,
    roof: f64,
    floor: f64,
    ground: f64,
}

/// An overlap polygon: its piece, its points in `verts`, its ground face
/// and the horizon the ground is taken as touching on it.
#[derive(Clone)]
struct Poly {
    piece: usize,
    range: std::ops::Range<usize>,
    ground_plan: [DVec2; 3],
    ground_z: [f64; 3],
    snap: Snap,
}

/// The horizon a surface is taken as lying on, where it is within the
/// trim's allowance of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Snap {
    None,
    Roof,
    Floor,
}

impl Snap {
    /// What the ground height `ground` is taken as over a roof and floor:
    /// on a horizon it is within `allowance` of, the nearer where it is
    /// within both (the roof on a tie).
    fn of(ground: f64, roof: f64, floor: f64, allowance: f64) -> Snap {
        let (near_roof, near_floor) = ((ground - roof).abs() <= allowance, (ground - floor).abs() <= allowance);
        match (near_roof, near_floor) {
            (true, true) if ground >= (roof + floor) / 2.0 => Snap::Roof,
            (true, true) => Snap::Floor,
            (true, false) => Snap::Roof,
            (false, true) => Snap::Floor,
            (false, false) => Snap::None,
        }
    }

    /// The height taken for ground `ground` over a roof and floor.
    fn level_of(self, ground: f64, roof: f64, floor: f64) -> f64 {
        match self {
            Snap::None => ground,
            Snap::Roof => roof,
            Snap::Floor => floor,
        }
    }

    fn level(self, vert: &Vert) -> f64 {
        self.level_of(vert.ground, vert.roof, vert.floor)
    }
}

/// What each thread of a trim keeps between cells.
#[derive(Default)]
struct CutScratch {
    stack: Vec<usize>,
    overlap: Vec<DVec3>,
    pieces: Vec<Piece>,
    verts: Vec<Vert>,
    polys: Vec<Poly>,
    /// The plan area of the cell last cut taken as touching a horizon.
    touching: f64,
}

/// A convex overlap polygon cut into parts on each of which the ground taken
/// as touching a horizon (see [`Snap::of`]) is linear: split along the lines
/// where the ground is `allowance` over and under the roof and the floor,
/// and, where it is within both, along the line half way between them.
fn snap_parts(polygon: &[Vert], allowance: f64, at: &impl Fn(DVec2) -> Vert) -> Vec<(Vec<Vert>, Snap)> {
    let mut parts: Vec<(Vec<Vert>, bool, bool)> = vec![(polygon.to_vec(), false, false)];
    for roof in [true, false] {
        let gap = |vert: &Vert| vert.ground - if roof { vert.roof } else { vert.floor };
        let mut next = Vec::new();
        for (part, near_roof, near_floor) in parts {
            let (not_below, below) = split_convex(&part, |vert| -gap(vert) - allowance, at);
            let (near, above) = split_convex(&not_below, |vert| gap(vert) - allowance, at);
            let flags = |near_horizon: bool| if roof { (near_horizon, near_floor) } else { (near_roof, near_horizon) };
            for (piece, near_horizon) in [(near, true), (below, false), (above, false)] {
                if piece.len() >= 3 && polygon_area(&piece) > 0.0 {
                    let (a, b) = flags(near_horizon);
                    next.push((piece, a, b));
                }
            }
        }
        parts = next;
    }
    let mut snapped = Vec::new();
    for (part, near_roof, near_floor) in parts {
        match (near_roof, near_floor) {
            (false, false) => snapped.push((part, Snap::None)),
            (true, false) => snapped.push((part, Snap::Roof)),
            (false, true) => snapped.push((part, Snap::Floor)),
            (true, true) => {
                let (roof_nearer, floor_nearer) = split_convex(&part, |vert| -(vert.ground - (vert.roof + vert.floor) / 2.0), at);
                for (piece, snap) in [(roof_nearer, Snap::Roof), (floor_nearer, Snap::Floor)] {
                    if piece.len() >= 3 && polygon_area(&piece) > 0.0 {
                        snapped.push((piece, snap));
                    }
                }
            }
        }
    }
    snapped
}

/// Whether `point` is in the triangle `triangle` (its edges included).
fn in_triangle(triangle: &[DVec2; 3], point: DVec2) -> bool {
    let sides = [0, 1, 2].map(|index| (triangle[(index + 1) % 3] - triangle[index]).perp_dot(point - triangle[index]));
    sides.iter().all(|side| *side >= 0.0) || sides.iter().all(|side| *side <= 0.0)
}

/// The subcrop segments on the edges of a part of an overlap where the ground
/// taken as touching steps across a roof or floor: an edge inside the overlap
/// with the ground at or over the roof on one side and under it on the
/// other, or at or under the floor on one side and over it on the other,
/// where the kept seam begins. Each is given from the side at the horizon.
fn snap_contacts(part: &[Vert], poly: &Poly, piece: &Piece, allowance: f64, at: &impl Fn(DVec2) -> Vert, contacts: &mut Contacts) {
    const PROBE: f64 = 1e-6;
    let centre = part.iter().map(|vert| vert.plan).sum::<DVec2>() / part.len() as f64;
    for (index, from) in part.iter().enumerate() {
        let to = &part[(index + 1) % part.len()];
        let edge = to.plan - from.plan;
        let length = edge.length();
        if length < 1e-9 {
            continue;
        }
        let middle = (from.plan + to.plan) / 2.0;
        let normal = edge.perp() / length;
        let outward = if normal.dot(middle - centre) >= 0.0 { normal } else { -normal };
        let probe = middle + outward * PROBE;
        if !in_triangle(&piece.plan, probe) || !in_triangle(&poly.ground_plan, probe) {
            continue;
        }
        let (here, there) = (at(middle), at(probe));
        let there_snap = Snap::of(there.ground, there.roof, there.floor, allowance);
        let (here_level, there_level) = (poly.snap.level(&here), there_snap.level(&there));
        let end = |vert: &Vert| vert.plan.extend(poly.snap.level(vert));
        if here_level >= here.roof && there_level < there.roof {
            contacts.roof.push([end(from), end(to)]);
        }
        if here_level <= here.floor && there_level > there.floor {
            contacts.floor.push([end(from), end(to)]);
        }
    }
}

/// The linear height `z` gives on the triangle `plan` at a point in it.
fn height_on(plan: &[DVec2; 3], z: &[f64; 3], at: DVec2) -> f64 {
    let (e1, e2) = (plan[1] - plan[0], plan[2] - plan[0]);
    let det = e1.perp_dot(e2);
    if det == 0.0 {
        return (z[0] + z[1] + z[2]) / 3.0;
    }
    let d = at - plan[0];
    let (l1, l2) = (d.perp_dot(e2) / det, e1.perp_dot(d) / det);
    z[0] + l1 * (z[1] - z[0]) + l2 * (z[2] - z[0])
}

/// The plan area of a convex polygon.
fn polygon_area(points: &[Vert]) -> f64 {
    let Some(first) = points.first() else { return 0.0 };
    let twice: f64 = points.windows(2).skip(1).map(|pair| (pair[0].plan - first.plan).perp_dot(pair[1].plan - first.plan)).sum();
    (twice / 2.0).abs()
}

/// A convex polygon split by the sign of `d`, linear on it: the points with
/// `d` at most 0, and those above. A crossing lies on both, its heights read
/// again by `at` from the planes, so the parts tile the polygon exactly.
fn split_convex(polygon: &[Vert], d: impl Fn(&Vert) -> f64, at: &impl Fn(DVec2) -> Vert) -> (Vec<Vert>, Vec<Vert>) {
    let (mut low, mut high) = (Vec::new(), Vec::new());
    for (index, from) in polygon.iter().enumerate() {
        let to = &polygon[(index + 1) % polygon.len()];
        let (d_from, d_to) = (d(from), d(to));
        (if d_from <= 0.0 { &mut low } else { &mut high }).push(*from);
        if (d_from <= 0.0) != (d_to <= 0.0) {
            let crossing = if d_from == 0.0 {
                *from
            } else if d_to == 0.0 {
                *to
            } else {
                at(from.plan + (to.plan - from.plan) * (d_from / (d_from - d_to)))
            };
            low.push(crossing);
            high.push(crossing);
        }
    }
    (low, high)
}

/// The segment where `d`, linear on a convex polygon, is 0 across it, at the
/// ground height there; none unless `d` changes sign strictly inside the
/// polygon, or when the segment has no length.
fn contact_of(polygon: &[Vert], d: impl Fn(&Vert) -> f64, at: &impl Fn(DVec2) -> Vert) -> Option<[DVec3; 2]> {
    if polygon.len() < 3 || !polygon.iter().any(|vert| d(vert) < 0.0) || !polygon.iter().any(|vert| d(vert) > 0.0) {
        return None;
    }
    let mut ends: Vec<DVec3> = Vec::with_capacity(2);
    for (index, from) in polygon.iter().enumerate() {
        let to = &polygon[(index + 1) % polygon.len()];
        let (d_from, d_to) = (d(from), d(to));
        let vert = if d_from == 0.0 {
            *from
        } else if (d_from < 0.0 && d_to > 0.0) || (d_from > 0.0 && d_to < 0.0) {
            at(from.plan + (to.plan - from.plan) * (d_from / (d_from - d_to)))
        } else {
            continue;
        };
        let end = vert.plan.extend(vert.ground);
        if !ends.iter().any(|other| other.distance(end) < 1e-9) {
            ends.push(end);
        }
    }
    match ends[..] {
        [a, b] => Some([a, b]),
        _ => None,
    }
}

/// A prismoid grid trimmed to a surface, and how its active cells fared:
/// kept with no cut, cut, gone, or beyond the surface.
#[derive(Debug)]
pub(crate) struct Trimmed {
    /// `None` when no active cell is kept.
    pub(crate) grid: Option<PrismoidGrid>,
    pub(crate) whole: usize,
    pub(crate) cut: usize,
    pub(crate) gone: usize,
    pub(crate) beyond: usize,
    /// The source over its active cells the surface reaches.
    pub(crate) reached: PrismoidGridSummary,
    /// The triangles the cuts of this trim store, masked cells' too.
    pub(crate) triangles: usize,
    /// Where the surface meets the roofs of the cut active cells, in cell
    /// order, as world segments.
    pub(crate) roof_contacts: Vec<[DVec3; 2]>,
    /// Where it meets their floors.
    pub(crate) floor_contacts: Vec<[DVec3; 2]>,
    /// The plan area of the active cells over which the surface was taken as
    /// touching a roof or floor (m2); the volume that could shift is at most
    /// the allowance times it.
    pub(crate) touching_area: f64,
}

/// Why a grid is not trimmed to a surface.
#[derive(Debug)]
pub(crate) enum TrimRefusal {
    /// The surface covers part of `count` cells twice, folding over itself
    /// there; `first` is the first such cell's centre in plan.
    Folded {
        count: usize,
        first: DVec2,
    },
    Cancelled,
    /// The trimmed grid failed [`PrismoidGrid::check`].
    Malformed(anyhow::Error),
    /// The cuts would hold more than `budget` triangles.
    TooLarge {
        budget: usize,
    },
}

/// A prismoid grid as one closed mesh, with the line round its roof and floor
/// and its lattice lines.
pub(crate) struct PrismoidMesh {
    pub(crate) vertices: Vec<mesh_data::Vertex>,
    pub(crate) faces: Vec<[u32; 3]>,
    pub(crate) outline: Vec<[u32; 2]>,
    pub(crate) lattice: Vec<[u32; 2]>,
    /// The active cells sharing each `lattice` line, `u32::MAX` unused.
    pub(crate) lattice_cells: Vec<[u32; 4]>,
}

/// Where a vertical plane crosses one active prismoid: the cell, the span of
/// distances along the plane, and two points (three across the drawn
/// diagonal) in order along it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SectionCrossing {
    pub(crate) cell: usize,
    pub(crate) along: [f64; 2],
    pub(crate) points: Vec<SectionPoint>,
}

/// A point of a [`SectionCrossing`]: its plan position and the drawn roof
/// and floor over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SectionPoint {
    pub(crate) plan: DVec2,
    pub(crate) roof: f64,
    pub(crate) floor: f64,
}

/// The grid over one plan point: the cell, the drawn roof and floor, and the
/// roof's slope there (dz/dx, dz/dy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlanReading {
    pub(crate) cell: usize,
    pub(crate) roof: f64,
    pub(crate) floor: f64,
    pub(crate) roof_slope: DVec2,
}

/// Why a plan point has no reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlanMiss {
    /// Off the lattice.
    Outside,
    /// In a cell masked by hand.
    Masked,
    /// In a cell the grid leaves out: a horizon does not cover it, the
    /// horizons cross in it, or it has no thickness. Which is not kept.
    Inactive,
}

/// A grid's vertical thickness as positions on the grid's colour scale, row
/// by row, with the map from world XY to their texture coordinates.
pub(crate) struct ThicknessMap {
    pub(crate) size: [u32; 2],
    pub(crate) positions: Vec<f32>,
    pub(crate) world_to_uv: [f64; 6],
}

/// One value as positions on a colour scale, a texel per cell inside a ring
/// of one texel, row by row, `None` where a cell has no value, with the map
/// from world XY to texture coordinates.
pub(crate) struct CellValueMap {
    pub(crate) size: [u32; 2],
    pub(crate) positions: Vec<Option<f32>>,
    pub(crate) world_to_uv: [f64; 6],
}

/// Every `step`-th of a side's `cells` kept so that they, and `extra`
/// texels more, fit `max_side`: the step and the texels on that side.
fn texel_fit(cells: usize, max_side: u32, extra: usize) -> (usize, usize) {
    let step = cells.div_ceil((max_side as usize).saturating_sub(extra).max(1));
    (step, cells.div_ceil(step) + extra)
}

/// The map from world XY to texture coordinates for the steps and texel
/// counts `texel_fit` gave, a step being `parts`th of a cell, the lattice
/// origin `offset` texels in.
fn texel_world_to_uv(lattice: &Lattice, (column_step, width): (usize, usize), (row_step, height): (usize, usize), parts: usize, offset: f64) -> [f64; 6] {
    let along = |axis: DVec2, size: f64, step: usize, count: usize| {
        let (spacing, count) = (size / parts as f64 * step as f64, count as f64);
        [
            axis.x / (spacing * count),
            axis.y / (spacing * count),
            (offset - lattice.origin.dot(axis) / spacing) / count,
        ]
    };
    let [a, b, c] = along(lattice.u, lattice.u_sizes[0], column_step, width);
    let [d, e, f] = along(lattice.v, lattice.v_sizes[0], row_step, height);
    [a, b, c, d, e, f]
}

/// `value` as a position from 0 to 1 across `(min, max)`, 0.5 where the
/// range is flat, which is to say within `1e-9` of its own size, well above
/// the rounding left by subtracting heights.
pub(crate) fn ramp_position(value: f64, (min, max): (f64, f64)) -> f32 {
    if (max - min).abs() <= 1e-9 * min.abs().max(max.abs()) {
        0.5
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
    }
}

fn mesh_vertex(position: DVec3) -> mesh_data::Vertex {
    mesh_data::Vertex::new(position.x, position.y, position.z)
}

// One whole cell into a total by the rules the reserve scan's `add` adds a
// block with: a value that is absent is missing; a weight absent, zero or
// negative is unusable; a negative value reads as zero.
fn add_summed(total: &mut NumericTotal, value: Option<f64>) {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        total.missing += 1;
        return;
    };
    if value < 0.0 {
        total.negatives += 1;
    } else {
        total.sum += value;
    }
    total.contributions += 1;
}

fn add_weighted(total: &mut NumericTotal, value: Option<f64>, weight: Option<f64>) {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        total.missing += 1;
        return;
    };
    let Some(weight) = weight.filter(|weight| weight.is_finite() && *weight > 0.0) else {
        total.unusable_weights += 1;
        return;
    };
    if value < 0.0 {
        total.negatives += 1;
    } else {
        total.sum += value * weight;
    }
    total.weight += weight;
    total.contributions += 1;
}

/// One active cell's box, measured as a block model's: its sides'
/// product, then the frame's stretch.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CellMeasure {
    pub(crate) cell: usize,
    sides: f64,
    stretch: f64,
}

impl CellMeasure {
    pub(crate) fn volume(self) -> f64 {
        self.sides * self.stretch
    }

    /// A `value` per cubic metre over the box, multiplied in
    /// `per_volume_values`' order.
    pub(crate) fn per_volume(self, value: f64) -> f64 {
        value * self.sides * self.stretch
    }
}

/// What a grid's quality means are weighted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Weighting {
    /// Cell tonnes: the grid has a density.
    ByTonnes,
    ByVolume,
}

/// One quality's mean over a grid's cells.
#[derive(Clone, Debug)]
pub(crate) struct QualityTotal {
    pub(crate) name: String,
    pub(crate) unit: String,
    pub(crate) total: NumericTotal,
}

/// One set of cell values' smallest and largest over the cells summed, and
/// how many of those cells have none.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ValueSpread {
    pub(crate) name: String,
    pub(crate) unit: String,
    pub(crate) range: Option<(f64, f64)>,
    pub(crate) missing: usize,
}

impl ValueSpread {
    fn take(&mut self, value: Option<f64>) {
        match value.filter(|value| value.is_finite()) {
            Some(value) => self.range = Some(self.range.map_or((value, value), |(min, max)| (min.min(value), max.max(value)))),
            None => self.missing += 1,
        }
    }
}

/// What a prismoid grid's row reports about it.
#[derive(Clone, Debug)]
pub(crate) struct PrismoidGridSummary {
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    pub(crate) active_cells: usize,
    /// The cells masked by hand, which Unmask All Prismoids brings back.
    pub(crate) masked_cells: usize,
    /// Roof minus floor, smallest and largest; `None` with no active cell.
    pub(crate) vertical_thickness: Option<(f64, f64)>,
    /// Cubic metres in the active prismoids.
    pub(crate) volume: f64,
    /// Volume times density, summed; `None` without a density.
    pub(crate) tonnes: Option<NumericTotal>,
    /// The density's name, with `tonnes`.
    pub(crate) density: Option<String>,
    pub(crate) weighting: Weighting,
    /// Each quality's mean, density aside, in the grid's order.
    pub(crate) qualities: Vec<QualityTotal>,
    /// Every set of values, density included, in the grid's order.
    pub(crate) spreads: Vec<ValueSpread>,
}

/// A prismoid grid open in the project.
#[derive(Clone, Debug)]
pub(crate) struct OpenPrismoidGrid {
    pub(crate) id: PrismoidGridId,
    pub(crate) state: ProjectItemState,
    pub(crate) name: String,
    pub(crate) color: [f32; 4],
    pub(crate) grid: Arc<PrismoidGrid>,
    /// Computed once: `grid` is an immutable `Arc`, so it cannot go stale.
    pub(crate) summary: PrismoidGridSummary,
    pub(crate) color_by: PrismoidColorBy,
    /// `None` for a grid built on its own.
    pub(crate) stratum: Option<Stratum>,
    /// `None` for a grid built before records were kept.
    pub(crate) built_from: Option<BuildRecord>,
    /// Drawn as a shaded body rather than as its lattice lines.
    pub(crate) shaded: bool,
}

impl OpenPrismoidGrid {
    pub(crate) fn entity_id(&self) -> crate::model::SceneEntityId {
        crate::model::SceneEntityId::PrismoidGrid(self.id)
    }

    /// The category a derived block model gives its blocks: the interval's
    /// name in a strata, else the grid's own.
    pub(crate) fn category(&self) -> &str {
        self.stratum.as_ref().map_or(&self.name, Stratum::category)
    }
}

/// A prismoid grid read from a file, before the project gives it an id.
pub(crate) struct LoadedPrismoidGrid {
    pub(crate) name: String,
    pub(crate) color: [f32; 4],
    pub(crate) grid: Arc<PrismoidGrid>,
    pub(crate) summary: PrismoidGridSummary,
    pub(crate) color_by: PrismoidColorBy,
    pub(crate) stratum: Option<Stratum>,
    pub(crate) built_from: Option<BuildRecord>,
    pub(crate) shaded: bool,
}

/// How a node's roof and floor stand, from their heights (NaN where
/// missing).
fn corner((top, bottom): (f64, f64)) -> Corner {
    if top.is_nan() || bottom.is_nan() {
        Corner::Missing
    } else if top - bottom < -HEIGHT_TOLERANCE {
        Corner::Crossed
    } else if top - bottom <= HEIGHT_TOLERANCE {
        Corner::Closed
    } else {
        Corner::Open
    }
}

/// Give every NaN the value of the nearest known one in steps across the
/// lattice, breadth first from every known value at once.
fn fill_missing(values: &mut [f64], columns: usize, rows: usize) {
    let mut queue: std::collections::VecDeque<usize> = (0..values.len()).filter(|&index| !values[index].is_nan()).collect();
    while let Some(index) = queue.pop_front() {
        let (column, row) = (index % columns, index / columns);
        let neighbours = [
            column.checked_sub(1).map(|column| row * columns + column),
            (column + 1 < columns).then(|| row * columns + column + 1),
            row.checked_sub(1).map(|row| row * columns + column),
            (row + 1 < rows).then(|| (row + 1) * columns + column),
        ];
        for next in neighbours.into_iter().flatten() {
            if values[next].is_nan() {
                values[next] = values[index];
                queue.push_back(next);
            }
        }
    }
}

/// How a quality surface's values reach a grid's cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellReading {
    /// The mean of the surface's heights at a prismoid's four corners: the
    /// grid's corners are nodes of the surface.
    Corners,
    /// Bilinear at the cell's centre, the surface being a grid on another lattice.
    Centre,
    /// Off the surface's triangles at the cell's centre: it is no grid.
    Triangles,
}

/// A quality surface as the cells read it.
pub(crate) enum QualitySurface {
    Grid(GridSurface),
    Triangles { mesh: Arc<mesh_data::Triangulation>, spatial: Arc<TriangleBvh> },
}

impl QualitySurface {
    /// A grid where `GridSurface::read` takes the surface, else its triangles.
    pub(crate) fn read(mesh: Arc<mesh_data::Triangulation>, spatial: Arc<TriangleBvh>) -> Self {
        match GridSurface::read(Arc::clone(&mesh), Arc::clone(&spatial)) {
            Ok(grid) => Self::Grid(grid),
            Err(_) => Self::Triangles { mesh, spatial },
        }
    }
}

/// One quality surface read into a grid's cells.
#[derive(Debug)]
pub(crate) struct ReadCells {
    pub(crate) reading: CellReading,
    /// One per cell, in cell order; `None` for inactive cells too.
    pub(crate) values: Vec<Option<f64>>,
    /// Active cells given a value, and active cells left without one.
    pub(crate) read: usize,
    pub(crate) missing: usize,
}

/// Values read from one quality surface, with the surface's name for words.
#[derive(Clone, Debug)]
pub(crate) struct AddedValues {
    pub(crate) surface: String,
    pub(crate) values: CellValues,
}

/// What two prismoid grids are compared by.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Quantity {
    /// A cell's box height, the inspector's mean thickness.
    VerticalThickness,
    /// A value both grids hold, by name.
    Value(String),
}

/// What a comparison of two grids found, this grid less the other.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DifferenceReport {
    /// Cells given a difference.
    pub(crate) compared: usize,
    /// This grid's active cells given none: no active counterpart in the
    /// other grid, or no value on one side.
    pub(crate) left_out_a: usize,
    /// The other grid's active cells with no active cell of this grid over them.
    pub(crate) outside_a: usize,
    pub(crate) low: f64,
    pub(crate) high: f64,
    pub(crate) mean: f64,
    /// For vertical thickness, the compared cells' box volumes, this grid
    /// less the other, in cubic metres.
    pub(crate) volume: Option<f64>,
}

/// Why two grids cannot be compared.
#[derive(Debug)]
pub(crate) enum DifferenceRefusal {
    /// The lattices differ in axes, cell size or alignment.
    NotOneLattice,
    Cancelled,
}

/// Values about to join a grid as the refusals that need no cells see them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ValuesMark<'a> {
    pub(crate) surface: &'a str,
    pub(crate) name: &'a str,
    pub(crate) unit: &'a str,
    pub(crate) role: CellValueRole,
}

/// Why values cannot join a grid. Names are surface names unless said;
/// quality names match as `same_quality_name` does.
#[derive(Debug)]
pub(crate) enum ValuesRefusal {
    /// Two surfaces give one quality (by `CellValues::name`).
    Twice { first: String, second: String, quality: String },
    /// A name the grid keeps for its own values: a derived block model
    /// column, the file's mask, or `density` for anything but the density.
    Reserved { surface: String, quality: String },
    /// The grid already holds a density (`held`: that quality's name) and another is added.
    TwoDensities { held: String, surface: String },
    /// A density in a unit naming kg.
    DensityInKg { surface: String, unit: String },
    /// No active cell got a value.
    NoCover { surface: String },
    /// The merged grid failed [`PrismoidGrid::check`].
    Malformed(anyhow::Error),
}

/// Names the derived block model and the file keep for their own columns.
const RESERVED_NAMES: [&str; 5] = ["volume", "vertical_thickness", "prismoid_grid", "active", "masked"];

/// The derived block model's columns holding each block's prismoid: roof,
/// then floor RL at corners a, b, c, d, anticlockwise from above with a at
/// the lattice's lowest `u` and `v`; prisms abc and acd split on the a-c
/// diagonal, as [`PrismoidGrid::closed_mesh`] draws them. A block of a
/// prismoid a trim cut holds its box: the kept roof's and floor's means at
/// all four corners, as four corners cannot hold a cut shape (the cut shape
/// stays in the grid), so the two prisms hold the block's volume.
pub(crate) const CORNER_HEIGHT_COLUMNS: [&str; 8] = ["roof_a", "roof_b", "roof_c", "roof_d", "floor_a", "floor_b", "floor_c", "floor_d"];

/// Whether `value` is a whole number, zero included, within `NODE_TOLERANCE`.
fn is_whole_or_zero(value: f64) -> bool {
    (value - value.round()).abs() <= NODE_TOLERANCE
}

/// As [`is_whole_or_zero`], one or more.
fn is_whole(value: f64) -> bool {
    value.round() >= 1.0 && is_whole_or_zero(value)
}

impl PrismoidGrid {
    /// Reads `surface` into one value per cell, or `None` if cancelled.
    pub(crate) fn read_cells(&self, surface: &QualitySurface, cancel: &CancelFlag) -> Option<ReadCells> {
        match surface {
            QualitySurface::Triangles { mesh, spatial } => self.read_cells_triangles(mesh, spatial, cancel),
            QualitySurface::Grid(grid) if self.corners_are_nodes(grid) => self.read_cells_corners(grid, cancel),
            QualitySurface::Grid(grid) => self.read_cells_centre(grid, cancel),
        }
    }

    /// Whether the lattice is north-aligned and every corner can be a node of `grid`.
    fn corners_are_nodes(&self, grid: &GridSurface) -> bool {
        let lattice = &self.lattice;
        if (lattice.u, lattice.v) != (DVec2::X, DVec2::Y) {
            return false;
        }
        let spacing = grid.spacing();
        let offset = (lattice.origin - grid.lattice().0) / spacing;
        is_whole_or_zero(offset.x) && is_whole_or_zero(offset.y) && lattice.u_sizes.iter().chain(&lattice.v_sizes).all(|size| is_whole(size / spacing))
    }

    /// The plan position of every cell's centre, row by row.
    pub(crate) fn cell_centres(&self) -> impl Iterator<Item = (usize, DVec2)> + '_ {
        let lattice = &self.lattice;
        let (u_edges, v_edges) = (edges(&lattice.u_sizes), edges(&lattice.v_sizes));
        let columns = lattice.columns();
        (0..lattice.cell_count()).map(move |cell| {
            let (column, row) = (cell % columns, cell / columns);
            let along_u = u_edges[column] + lattice.u_sizes[column] / 2.0;
            let along_v = v_edges[row] + lattice.v_sizes[row] / 2.0;
            (cell, lattice.origin + lattice.u * along_u + lattice.v * along_v)
        })
    }

    /// Reads each active cell with `value`, checking `cancel` once per row.
    fn read_each(&self, reading: CellReading, cancel: &CancelFlag, mut value: impl FnMut(usize, DVec2) -> Option<f64>) -> Option<ReadCells> {
        let columns = self.lattice.columns();
        let mut values = vec![None; self.active.len()];
        let (mut read, mut missing) = (0, 0);
        for (cell, centre) in self.cell_centres() {
            if cell % columns == 0 && cancel.is_cancelled() {
                return None;
            }
            if !self.active[cell] {
                continue;
            }
            values[cell] = value(cell, centre);
            if values[cell].is_some() {
                read += 1;
            } else {
                missing += 1;
            }
        }
        Some(ReadCells { reading, values, read, missing })
    }

    fn read_cells_triangles(&self, mesh: &mesh_data::Triangulation, spatial: &TriangleBvh, cancel: &CancelFlag) -> Option<ReadCells> {
        let mut stack = Vec::new();
        self.read_each(CellReading::Triangles, cancel, |_, centre| surface_z_single(mesh, spatial, &mut stack, centre))
    }

    fn read_cells_centre(&self, grid: &GridSurface, cancel: &CancelFlag) -> Option<ReadCells> {
        self.read_each(CellReading::Centre, cancel, |_, centre| grid.bilinear(centre))
    }

    fn read_cells_corners(&self, grid: &GridSurface, cancel: &CancelFlag) -> Option<ReadCells> {
        let heights: Vec<Option<f64>> = self
            .lattice
            .corners()
            .into_iter()
            .map(|at| grid.node_at(at).and_then(|(column, row)| grid.height(column, row)))
            .collect();
        let columns = self.lattice.columns();
        self.read_each(CellReading::Corners, cancel, |cell, _| {
            let first = cell / columns * (columns + 1) + cell % columns;
            let corners = [heights[first], heights[first + 1], heights[first + columns + 1], heights[first + columns + 2]];
            corners.iter().try_fold(0.0, |sum, height| height.map(|height| sum + height)).map(|sum| sum / 4.0)
        })
    }

    /// This grid less `other`, cell by cell, as values named `name` in
    /// `unit`: for each active cell with an active counterpart on the shared
    /// lattice, the box height less the box height, or the value less the
    /// value where both are finite, exactly 0 where the two agree within 1e-9
    /// of the larger. Cells without a counterpart or a value on either side
    /// get none and are counted. `Ok(None)` when no cell compares; `Err` for
    /// two lattices or on cancel, checked once per row.
    pub(crate) fn difference(
        &self,
        other: &PrismoidGrid,
        quantity: &Quantity,
        name: String,
        unit: String,
        cancel: &CancelFlag,
    ) -> Result<Option<(CellValues, DifferenceReport)>, DifferenceRefusal> {
        let (shift_column, shift_row) = self.lattice.offset_to(&other.lattice).ok_or(DifferenceRefusal::NotOneLattice)?;
        let index_in = |grid: &PrismoidGrid| match quantity {
            Quantity::VerticalThickness => None,
            Quantity::Value(value) => grid.cell_values.iter().position(|values| same_quality_name(&values.name, value)),
        };
        let (mine, theirs) = (index_in(self), index_in(other));
        let thickness = *quantity == Quantity::VerticalThickness;
        // Each lattice's edge distances and stretch, taken once and only
        // where boxes are measured.
        let frame_of = |grid: &PrismoidGrid| thickness.then(|| (edges(&grid.lattice.u_sizes), edges(&grid.lattice.v_sizes), grid.stretch()));
        let (my_frame, their_frame) = (frame_of(self), frame_of(other));
        // An active cell's quantity and, for thickness, its box volume;
        // `None` without a value. A named value takes no measure.
        let read = |grid: &PrismoidGrid, frame: &Option<(Vec<f64>, Vec<f64>, f64)>, index: Option<usize>, cell: usize| -> Option<(f64, f64)> {
            let (value, volume) = match (index, frame) {
                (Some(index), _) => (grid.cell_values[index].values[cell]?, 0.0),
                (None, Some((u_edges, v_edges, stretch))) => {
                    let (bounds, measure) = grid.measure_in(cell, u_edges, v_edges, *stretch)?;
                    (bounds.upper.z - bounds.lower.z, measure.volume())
                }
                (None, None) => return None,
            };
            value.is_finite().then_some((value, volume))
        };
        let (columns, rows) = (self.lattice.columns() as i64, self.lattice.rows() as i64);
        let (other_columns, other_rows) = (other.lattice.columns() as i64, other.lattice.rows() as i64);
        let counterpart = |column: i64, row: i64| {
            let (column, row) = (column + shift_column, row + shift_row);
            ((0..other_columns).contains(&column) && (0..other_rows).contains(&row)).then_some((row * other_columns + column) as usize)
        };
        let mut values = vec![None; self.active.len()];
        let mut matched = vec![false; other.active.len()];
        let mut report = DifferenceReport {
            compared: 0,
            left_out_a: 0,
            outside_a: 0,
            low: f64::INFINITY,
            high: f64::NEG_INFINITY,
            mean: 0.0,
            volume: thickness.then_some(0.0),
        };
        for row in 0..rows {
            if cancel.is_cancelled() {
                return Err(DifferenceRefusal::Cancelled);
            }
            for column in 0..columns {
                let cell = (row * columns + column) as usize;
                if !self.active[cell] {
                    continue;
                }
                let their = counterpart(column, row).filter(|their| other.active[*their]);
                if let Some(their) = their {
                    matched[their] = true;
                }
                match their.and_then(|their| read(self, &my_frame, mine, cell).zip(read(other, &their_frame, theirs, their))) {
                    Some(((a, a_volume), (b, b_volume))) => {
                        // Values agreeing to within rounding differ by nothing.
                        let difference = if (a - b).abs() <= 1e-9 * a.abs().max(b.abs()) { 0.0 } else { a - b };
                        values[cell] = Some(difference);
                        report.compared += 1;
                        report.low = report.low.min(difference);
                        report.high = report.high.max(difference);
                        report.mean += difference;
                        if let Some(volume) = &mut report.volume {
                            *volume += a_volume - b_volume;
                        }
                    }
                    None => report.left_out_a += 1,
                }
            }
        }
        report.outside_a = other.active.iter().zip(&matched).filter(|(active, matched)| **active && !**matched).count();
        if report.compared == 0 {
            return Ok(None);
        }
        report.mean /= report.compared as f64;
        let values = CellValues {
            name,
            unit,
            role: CellValueRole::Quality,
            values,
        };
        Ok(Some((values, report)))
    }

    /// A copy of the grid with `added` merged in: a quality the grid already
    /// holds is replaced in place, new ones follow in order.
    pub(crate) fn with_values(&self, added: Vec<AddedValues>) -> Result<PrismoidGrid, ValuesRefusal> {
        let held: Vec<_> = self.cell_values.iter().map(|values| (values.name.as_str(), values.role)).collect();
        let marks: Vec<_> = added
            .iter()
            .map(|one| ValuesMark {
                surface: &one.surface,
                name: &one.values.name,
                unit: &one.values.unit,
                role: one.values.role,
            })
            .collect();
        check_marks(&held, &marks)?;
        if let Some(one) = added
            .iter()
            .find(|one| !self.active.iter().zip(&one.values.values).any(|(active, value)| *active && value.is_some()))
        {
            return Err(ValuesRefusal::NoCover { surface: one.surface.clone() });
        }
        let mut merged = self.clone();
        for one in added {
            match merged.cell_values.iter_mut().find(|held| same_quality_name(&held.name, &one.values.name)) {
                Some(held) => *held = one.values,
                None => merged.cell_values.push(one.values),
            }
        }
        merged.check().map_err(ValuesRefusal::Malformed)?;
        Ok(merged)
    }
}

/// The refusal `added` meets against the grid's `held` names and roles
/// before any cell is read, so a dialog can say it before the work starts.
pub(crate) fn check_marks(held: &[(&str, CellValueRole)], added: &[ValuesMark<'_>]) -> Result<(), ValuesRefusal> {
    for one in added {
        let reserved = RESERVED_NAMES.iter().chain(&CORNER_HEIGHT_COLUMNS).any(|name| same_quality_name(name, one.name));
        // A quality may not replace the held density by taking its name.
        // A batch that adds a density replaces the held one, whatever its name.
        let takes_density = one.role != CellValueRole::Density
            && !added.iter().any(|other| other.role == CellValueRole::Density)
            && held.iter().any(|(name, role)| *role == CellValueRole::Density && same_quality_name(name, one.name));
        if reserved || takes_density || (same_quality_name(one.name, "density") && one.role != CellValueRole::Density) {
            return Err(ValuesRefusal::Reserved {
                surface: one.surface.to_owned(),
                quality: one.name.to_owned(),
            });
        }
    }
    if let Some(one) = added.iter().find(|one| one.role == CellValueRole::Density && one.unit.to_ascii_lowercase().contains("kg")) {
        return Err(ValuesRefusal::DensityInKg {
            surface: one.surface.to_owned(),
            unit: one.unit.to_owned(),
        });
    }
    for (later, one) in added.iter().enumerate() {
        if let Some(earlier) = added[..later].iter().find(|earlier| same_quality_name(earlier.name, one.name)) {
            return Err(ValuesRefusal::Twice {
                first: earlier.surface.to_owned(),
                second: one.surface.to_owned(),
                quality: one.name.to_owned(),
            });
        }
    }
    // A held density stays unless one of `added` replaces it.
    let kept = held
        .iter()
        .filter(|(name, role)| *role == CellValueRole::Density && !added.iter().any(|one| same_quality_name(one.name, name)))
        .map(|(name, _)| *name);
    let mut densities = kept.chain(added.iter().filter(|one| one.role == CellValueRole::Density).map(|one| one.name));
    if let (Some(first), Some(_)) = (densities.next(), densities.next())
        && let Some(one) = added.iter().rev().find(|one| one.role == CellValueRole::Density)
    {
        return Err(ValuesRefusal::TwoDensities {
            held: first.to_owned(),
            surface: one.surface.to_owned(),
        });
    }
    Ok(())
}

/// The refusal a difference named `name` meets against the grid's `held`
/// names and roles, `other` the grid compared with: what [`check_marks`]
/// refuses a quality of that name, the name of the held density included.
pub(crate) fn check_difference_name(held: &[(&str, CellValueRole)], other: &str, name: &str) -> Result<(), ValuesRefusal> {
    let mark = ValuesMark {
        surface: other,
        name,
        unit: "",
        role: CellValueRole::Quality,
    };
    check_marks(held, &[mark])
}

/// A surface a grid was built from: its name for the reader, its geometry
/// token to find it again after a rename, a reopen or a merge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SourceStamp {
    pub(crate) name: String,
    pub(crate) geometry: GeometryVersion,
}

/// Where one of a grid's values came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ValueSource {
    pub(crate) value: String,
    pub(crate) from: ValueFrom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum ValueFrom {
    /// Read from a quality surface, as the grid's density when `density`.
    Surface { surface: SourceStamp, density: bool },
    /// A difference against the grid of this name, by `of`.
    Difference { other: String, of: Quantity },
}

/// What a prismoid grid was built from and when, as it stood then.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BuildRecord {
    pub(crate) roof: SourceStamp,
    pub(crate) floor: SourceStamp,
    /// The surfaces' node spacing, metres.
    pub(crate) spacing: f64,
    /// Surface nodes to a cell side.
    pub(crate) step: NonZeroUsize,
    pub(crate) envelope: Option<ModelEnvelope>,
    /// RFC 3339, local time with its offset.
    pub(crate) built: String,
    /// The application and its version, as a saved file names them.
    pub(crate) application: String,
    #[serde(default)]
    pub(crate) values: Vec<ValueSource>,
    /// The correction records bearing on the grid at the build, one print
    /// per drill-hole set read.
    pub(crate) holes: Vec<SetPrint>,
}

impl BuildRecord {
    /// A record dated now, holding no values yet.
    pub(crate) fn now(roof: SourceStamp, floor: SourceStamp, spacing: f64, step: NonZeroUsize, envelope: Option<ModelEnvelope>, holes: Vec<SetPrint>) -> Self {
        Self {
            roof,
            floor,
            spacing,
            step,
            envelope,
            built: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            application: format!("Incline {}", env!("CARGO_PKG_VERSION")),
            values: Vec::new(),
            holes,
        }
    }

    /// Note where `sources` came from, each taking the place of a held
    /// entry of the same value name, as values are merged.
    pub(crate) fn note_values(&mut self, sources: Vec<ValueSource>) {
        for source in sources {
            match self.values.iter_mut().find(|held| same_quality_name(&held.value, &source.value)) {
                Some(held) => *held = source,
                None => self.values.push(source),
            }
        }
    }
}

/// The correction records of one drill-hole set whose holes meet a grid's
/// plan rectangle: how many, and one hash over them in any order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SetPrint {
    pub(crate) set: String,
    pub(crate) count: usize,
    /// Hex, so a reader without 64-bit integers keeps it whole.
    pub(crate) print: String,
}

/// A source of a grid that has moved since the grid was built.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Moved {
    /// Drill holes inside the grid whose records changed, those dated after
    /// the build counted, with the latest date as written.
    Holes { count: usize, latest: String },
    /// Records inside the grid changed with none dated after the build: an
    /// earlier correction undone, or one undone before the build redone.
    HoleRecords,
    /// A surface of the recorded name holding other geometry.
    Surface(String),
    /// The model envelope is not the one the grid was built in.
    Envelope,
}

/// A recorded source the project no longer holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Gone {
    Surface(String),
    /// No grid of this name: a difference names its other grid only.
    Grid(String),
    /// No drill-hole set of this name, loaded or not.
    HoleSet(String),
}

/// What has become of a grid's recorded sources since its build.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Staleness {
    pub(crate) moved: Vec<Moved>,
    pub(crate) gone: Vec<Gone>,
}

/// The project a grid's sources are looked for in: every surface by name
/// and geometry token, loaded or not, every drill-hole set with the loaded
/// ones' records, every grid's name and the model envelope.
pub(crate) struct ProjectSources<'a> {
    pub(crate) surfaces: Vec<(&'a str, GeometryVersion)>,
    pub(crate) drill_holes: Vec<HoleRecords<'a>>,
    pub(crate) grids: Vec<&'a str>,
    pub(crate) envelope: Option<ModelEnvelope>,
}

/// One loaded drill-hole set's correction records, each with its hole's
/// box in plan, worked out once for every grid checked against them.
pub(crate) struct HoleRecords<'a> {
    pub(crate) set: &'a str,
    /// Whether the set is loaded; an unloaded one holds no records here.
    pub(crate) loaded: bool,
    pub(crate) records: Vec<HoleRecord<'a>>,
}

pub(crate) struct HoleRecord<'a> {
    /// The hole's index in its set.
    pub(crate) hole: usize,
    /// Least and greatest corner of the hole's box in plan.
    pub(crate) plan: (DVec2, DVec2),
    pub(crate) date: &'a str,
    pub(crate) when: Option<chrono::DateTime<chrono::FixedOffset>>,
    correction: &'a Correction,
    print: std::cell::OnceCell<u64>,
}

impl<'a> HoleRecords<'a> {
    /// The records of `dataset`, named `set`, on holes it still holds.
    pub(crate) fn of(set: &'a str, dataset: &'a DrillHoleDataset) -> Self {
        let holes: HashMap<&str, usize> = dataset.holes.iter().enumerate().map(|(index, hole)| (hole.dhid.as_str(), index)).collect();
        let records = dataset
            .corrections
            .iter()
            .filter_map(|correction| {
                let hole = *holes.get(correction.hole.as_str())?;
                let (min, max) = dataset.hole_box(hole)?;
                Some(HoleRecord {
                    hole,
                    plan: (min.truncate(), max.truncate()),
                    date: &correction.date,
                    when: chrono::DateTime::parse_from_rfc3339(&correction.date).ok(),
                    correction,
                    print: std::cell::OnceCell::new(),
                })
            })
            .collect();
        Self { set, loaded: true, records }
    }

    /// The set `set`, in the project but not loaded.
    pub(crate) fn unloaded(set: &'a str) -> Self {
        Self {
            set,
            loaded: false,
            records: Vec::new(),
        }
    }
}

impl HoleRecord<'_> {
    /// A hash of what the record changes, the same on every target and
    /// after a save and reopen: FNV-1a (Fowler, Noll and Vo) over the JSON
    /// of its hole, interval, kind, field, values, date and rows, worked
    /// out on first use.
    fn print(&self) -> u64 {
        *self.print.get_or_init(|| {
            let record = self.correction;
            let fields = (
                &record.hole,
                record.logged_from.to_bits(),
                record.logged_to.to_bits(),
                record.kind,
                &record.field,
                &record.before,
                &record.after,
                &record.date,
                &record.row,
                &record.removed,
            );
            fnv(&serde_json::to_vec(&fields).unwrap_or_default())
        })
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3))
}

/// The print of a set's records bearing on a grid.
fn set_print(set: &str, records: &[&HoleRecord<'_>]) -> SetPrint {
    let mut prints: Vec<u64> = records.iter().map(|record| record.print()).collect();
    prints.sort_unstable();
    let bytes: Vec<u8> = prints.iter().flat_map(|print| print.to_le_bytes()).collect();
    SetPrint {
        set: set.to_owned(),
        count: prints.len(),
        print: format!("{:016x}", fnv(&bytes)),
    }
}

/// The print, per loaded set, of the correction records whose hole meets a
/// grid's plan rectangle: kept on the record at the build.
pub(crate) fn hole_prints(lattice: &Lattice, holes: &[HoleRecords<'_>]) -> Vec<SetPrint> {
    holes.iter().filter(|set| set.loaded).map(|set| set_print(set.set, &records_inside(lattice, set))).collect()
}

fn records_inside<'s, 'a>(lattice: &Lattice, set: &'s HoleRecords<'a>) -> Vec<&'s HoleRecord<'a>> {
    set.records.iter().filter(|record| meets(lattice, record.plan)).collect()
}

/// The present name of the surface holding `stamp`'s token, else the
/// recorded one.
pub(crate) fn present_name<'s>(stamp: &'s SourceStamp, project: &ProjectSources<'s>) -> &'s str {
    project
        .surfaces
        .iter()
        .find(|(_, geometry)| *geometry == stamp.geometry)
        .map_or(stamp.name.as_str(), |(name, _)| name)
}

/// Which of `record`'s sources have moved since the build of a grid on
/// `lattice`, and which are gone. A surface is found by its token, else
/// by its name. The records of holes whose box meets the grid's plan
/// rectangle have moved when their print differs from the one kept at the
/// build, whatever they corrected; a set without a kept print falls back
/// to records dated after the build (an unreadable date counts as after).
/// The dated records word the line.
pub(crate) fn staleness(record: &BuildRecord, lattice: &Lattice, project: &ProjectSources<'_>) -> Staleness {
    let mut found = Staleness::default();
    let built = chrono::DateTime::parse_from_rfc3339(&record.built).ok();
    let (mut holes, mut latest, mut changed) = (HashSet::new(), None, false);
    for (index, set) in project.drill_holes.iter().enumerate().filter(|(_, set)| set.loaded) {
        let inside = records_inside(lattice, set);
        let later: Vec<_> = inside.iter().filter(|held| built.zip(held.when).is_none_or(|(built, when)| when > built)).collect();
        let kept = record.holes.iter().find(|print| print.set == set.set);
        let moved = match kept {
            Some(kept) => *kept != set_print(set.set, &inside),
            None => !later.is_empty(),
        };
        if !moved {
            continue;
        }
        changed = true;
        for held in later {
            holes.insert((index, held.hole));
            if latest.is_none_or(|(when, _)| held.when > when) {
                latest = Some((held.when, held.date));
            }
        }
    }
    if changed {
        found.moved.push(match latest {
            Some((_, latest)) => Moved::Holes {
                count: holes.len(),
                latest: latest.to_owned(),
            },
            None => Moved::HoleRecords,
        });
    }
    let surfaces = [&record.roof, &record.floor].into_iter().chain(record.values.iter().filter_map(|value| match &value.from {
        ValueFrom::Surface { surface, .. } => Some(surface),
        ValueFrom::Difference { .. } => None,
    }));
    for stamp in surfaces {
        if project.surfaces.iter().any(|(_, geometry)| *geometry == stamp.geometry) {
            continue;
        }
        if project.surfaces.iter().any(|(name, _)| *name == stamp.name) {
            found.moved.push(Moved::Surface(stamp.name.clone()));
        } else {
            found.gone.push(Gone::Surface(stamp.name.clone()));
        }
    }
    for print in &record.holes {
        if !project.drill_holes.iter().any(|set| set.set == print.set) {
            found.gone.push(Gone::HoleSet(print.set.clone()));
        }
    }
    if record.envelope != project.envelope {
        found.moved.push(Moved::Envelope);
    }
    for value in &record.values {
        if let ValueFrom::Difference { other, .. } = &value.from
            && !project.grids.contains(&other.as_str())
        {
            found.gone.push(Gone::Grid(other.clone()));
        }
    }
    found
}

/// Whether a box in plan, least and greatest corner, meets the lattice's
/// own rectangle, turned as the lattice is: the two overlap along each
/// side's normal.
fn meets(lattice: &Lattice, (low, high): (DVec2, DVec2)) -> bool {
    let (width, length) = (lattice.u_sizes.iter().sum::<f64>(), lattice.v_sizes.iter().sum::<f64>());
    let rectangle = [DVec2::ZERO, lattice.u * width, lattice.u * width + lattice.v * length, lattice.v * length].map(|offset| lattice.origin + offset);
    let corners = [low, DVec2::new(high.x, low.y), high, DVec2::new(low.x, high.y)];
    let span = |points: &[DVec2; 4], axis: DVec2| {
        points
            .iter()
            .map(|point| point.dot(axis))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(least, most), along| (least.min(along), most.max(along)))
    };
    [DVec2::X, DVec2::Y, lattice.u.perp(), lattice.v.perp()].into_iter().all(|axis| {
        let ((a_least, a_most), (b_least, b_most)) = (span(&rectangle, axis), span(&corners, axis));
        a_least <= b_most && b_least <= a_most
    })
}

/// `∫ max(0, L - h) dA` over a triangle of `area` on which `L` runs linearly
/// between `values` at its corners: the volume of `L` above the level `h`.
/// Above the lowest corner the part of the triangle where `L > h` is itself
/// a triangle at the highest corner, over which `L - h` is linear and zero
/// at two corners; each branch divides only by positive numbers.
fn above_integral(area: f64, values: [f64; 3], h: f64) -> f64 {
    let mut sorted = values;
    sorted.sort_by(f64::total_cmp);
    let [low, middle, high] = sorted;
    let mean = (low + middle + high) / 3.0;
    if h <= low {
        area * (mean - h)
    } else if h <= middle {
        let under = h - low;
        area * (mean - h) + area * under * under * under / (3.0 * (middle - low) * (high - low))
    } else if h < high {
        let over = high - h;
        area * over * over * over / (3.0 * (high - low) * (high - middle))
    } else {
        0.0
    }
}

impl PrismoidGrid {
    /// How much of one active prismoid lies in each of `layers` horizontal
    /// layers `thickness` thick upward from `base`, exactly: `add(layer,
    /// volume)` upward for each layer it reaches. The body is the two
    /// triangular prisms [`Self::closed_mesh`] draws, and its volume above a
    /// level is the roof's [`above_integral`] less the floor's. A cut
    /// prismoid sums its cut triangles the same way. Ground below the base
    /// or above the last layer is left out.
    fn layer_volumes(&self, cell: usize, stretch: f64, base: f64, thickness: f64, layers: usize, mut add: impl FnMut(usize, f64)) {
        let lattice = &self.lattice;
        let columns = lattice.columns();
        let area = lattice.u_sizes[cell % columns] * lattice.v_sizes[cell / columns] * stretch / 2.0;
        let [a, b, c, d] = self.cell_corners(cell);
        let triangles = [[a, b, c], [a, c, d]];
        let cut = self.cut.triangles_of(cell);
        let above = |level: f64| match self.cut.triangles_of(cell) {
            Some(pieces) => pieces
                .map(|[p, q, r]| {
                    let area = (q.plan - p.plan).perp_dot(r.plan - p.plan).abs() / 2.0 * stretch;
                    above_integral(area, [p.roof, q.roof, r.roof], level) - above_integral(area, [p.floor, q.floor, r.floor], level)
                })
                .sum::<f64>(),
            None => {
                let [one, two] = triangles
                    .map(|corners| above_integral(area, corners.map(|corner| self.roof[corner]), level) - above_integral(area, corners.map(|corner| self.floor[corner]), level));
                one + two
            }
        };
        let (low, high) = match cut {
            Some(pieces) => {
                let mut range = (f64::INFINITY, f64::NEG_INFINITY);
                for point in pieces.flatten() {
                    range = (range.0.min(point.floor), range.1.max(point.roof));
                }
                if range.0 > range.1 {
                    return;
                }
                range
            }
            None => (
                [a, b, c, d].map(|corner| self.floor[corner]).into_iter().fold(f64::INFINITY, f64::min),
                [a, b, c, d].map(|corner| self.roof[corner]).into_iter().fold(f64::NEG_INFINITY, f64::max),
            ),
        };
        // A layer either side of the division's rounding, then only the
        // layers the body reaches.
        let first = (((low - base) / thickness).floor().max(0.0) as usize).saturating_sub(1);
        let last = (((high - base) / thickness).ceil().max(0.0) as usize).saturating_add(1).min(layers);
        let mut below = above(base + first as f64 * thickness);
        for layer in first..last {
            let (bottom, top) = (base + layer as f64 * thickness, base + (layer + 1) as f64 * thickness);
            let over = above(top);
            let volume = below - over;
            if top > low && bottom < high && volume > 0.0 {
                add(layer, volume);
            }
            below = over;
        }
    }
}

/// What [`regular_boxes`] measured: the box lattice and, for each box
/// holding modelled ground, its place and numbers.
#[derive(Clone, Debug)]
pub(crate) struct RegularBoxes {
    /// Boxes along x, y and z, by the kriging grid's round-up.
    pub(crate) dims: [usize; 3],
    /// The categories in the order first met.
    pub(crate) labels: Vec<String>,
    /// The qualities, matched by `same_quality_name` and named as first met.
    pub(crate) qualities: Vec<String>,
    /// Each written box's (column, row, layer), x fastest, then z, then y.
    pub(crate) at: Vec<[usize; 3]>,
    /// Per written box: modelled volume, mass, each label's volume, each
    /// quality's mean.
    records: Vec<f64>,
}

impl RegularBoxes {
    fn record(&self, index: usize) -> &[f64] {
        let width = 2 + self.labels.len() + self.qualities.len();
        &self.records[index * width..(index + 1) * width]
    }

    /// Modelled cubic metres in the box.
    pub(crate) fn volume(&self, index: usize) -> f64 {
        self.record(index)[0]
    }

    /// Tonnes in the box; NaN where some of its ground has no density.
    pub(crate) fn mass(&self, index: usize) -> f64 {
        self.record(index)[1]
    }

    /// Cubic metres of each label in the box, in `labels` order.
    pub(crate) fn label_volumes(&self, index: usize) -> &[f64] {
        &self.record(index)[2..2 + self.labels.len()]
    }

    /// Each quality's mean over the ground carrying it, by tonnes where the
    /// box's mass is known, else by volume, of the values as they are, signs
    /// kept (a difference is signed); NaN where no ground carries it.
    pub(crate) fn quality_means(&self, index: usize) -> &[f64] {
        &self.record(index)[2 + self.labels.len()..]
    }
}

/// Why [`regular_boxes`] measured nothing.
#[derive(Debug)]
pub(crate) enum RegularRefusal {
    /// The kriging grid refuses the envelope and block size.
    Grid,
    /// The grid at this position is not regular on the envelope's lattice.
    OffLattice(usize),
    /// The block's plan size is no whole multiple of this grid's cells.
    NotMultiple(usize),
    /// Two grids, by position, give one quality, named as first met, in two
    /// units: `first` on `first_grid`, `second` on `second_grid`.
    UnitsDiffer {
        quality: String,
        first: String,
        first_grid: usize,
        second: String,
        second_grid: usize,
    },
    Cancelled,
}

/// One grid as [`regular_boxes`] reads it.
struct RegularSource<'a> {
    grid: &'a PrismoidGrid,
    stretch: f64,
    label: usize,
    /// The grid's first cell, in cells from the envelope's corner.
    offset: [i64; 2],
    /// Cells along a block's x and y.
    per_block: [i64; 2],
    density: Option<&'a CellValues>,
    /// Each quality with its place in the merged list.
    qualities: Vec<(&'a CellValues, usize)>,
}

/// The corners of the box lattice for cubes of `size` over `envelope`: x
/// and y at its plan corners, z moved outward from its base and top RL to
/// whole multiples of `size` from RL 0, a ratio within `NODE_TOLERANCE` of a
/// whole number taken as it.
pub(crate) fn regular_frame(envelope: &ModelEnvelope, size: f64) -> (DVec3, DVec3) {
    let snap = |rl: f64, outward: fn(f64) -> f64| {
        let ratio = rl / size;
        (if is_whole_or_zero(ratio) { ratio.round() } else { outward(ratio) }) * size
    };
    let (lower, upper) = (envelope.lower(), envelope.upper());
    (lower.with_z(snap(lower.z, f64::floor)), upper.with_z(snap(upper.z, f64::ceil)))
}

/// Cubes of `size` laid across `envelope` from its plan corner, in layers
/// cut at whole multiples of `size` from RL 0 by [`regular_frame`], and how
/// much of each active prismoid of `grids`, each with its category, lies in
/// each, exactly. A box holds its volume, its mass where all of its
/// ground has a density above 0, each label's volume and each quality's
/// mean; only boxes holding more than `1e-9` of their volume in modelled
/// ground are kept, and a part under that share counts as rounding for the
/// density too. Qualities of one name must agree in unit where both give
/// one. Every grid must be regular on the envelope's lattice with `size` a
/// whole multiple of its cells, so a cell lies in one column of
/// boxes; the caller checks [`PrismoidGrid::fits`] first, and a cell off the
/// box lattice is skipped. Box rows run side by side; each adds grids in
/// order, cells in cell order, layers upward, so the numbers do not depend
/// on the threads. A roof within the envelope's tolerance above its top RL
/// may leave a sliver above the last layer, out of every box.
pub(crate) fn regular_boxes(grids: &[(&PrismoidGrid, &str)], envelope: &ModelEnvelope, size: f64, cancel: &CancelFlag, progress: &Phase) -> Result<RegularBoxes, RegularRefusal> {
    use rayon::prelude::*;
    let (lower, upper) = regular_frame(envelope, size);
    let size = DVec3::splat(size);
    let dims = crate::model::kriging::KrigingGrid { lower, upper, cell: size }
        .dimensions()
        .map_err(|_| RegularRefusal::Grid)?;
    let [columns, rows, layers] = dims;
    let mut labels: Vec<String> = Vec::new();
    let mut slots = ValueSlots::default();
    let mut sources = Vec::with_capacity(grids.len());
    for (index, (grid, category)) in grids.iter().enumerate() {
        let lattice = &grid.lattice;
        let (Some(&cell_x), Some(&cell_y)) = (lattice.u_sizes.first(), lattice.v_sizes.first()) else {
            return Err(RegularRefusal::OffLattice(index));
        };
        let corner = Lattice {
            origin: lower.truncate(),
            u: DVec2::X,
            v: DVec2::Y,
            u_sizes: vec![cell_x],
            v_sizes: vec![cell_y],
        };
        let (offset_x, offset_y) = lattice.offset_to(&corner).ok_or(RegularRefusal::OffLattice(index))?;
        let per_block = [size.x / cell_x, size.y / cell_y];
        if !per_block.into_iter().all(is_whole) {
            return Err(RegularRefusal::NotMultiple(index));
        }
        let label = match labels.iter().position(|label| label == category) {
            Some(label) => label,
            None => {
                labels.push((*category).to_owned());
                labels.len() - 1
            }
        };
        let mut held = Vec::new();
        for values in grid.cell_values.iter().filter(|values| values.role == CellValueRole::Quality) {
            if let Some(merged) = slots.slot(values, index)? {
                held.push((values, merged));
            }
        }
        let density = grid.cell_values.iter().find(|values| values.role == CellValueRole::Density);
        if let Some(values) = density {
            slots.slot(values, index)?;
        }
        sources.push(RegularSource {
            grid,
            stretch: grid.stretch(),
            label,
            offset: [offset_x, offset_y],
            per_block: per_block.map(|cells| cells.round() as i64),
            density,
            qualities: held,
        });
    }
    let qualities = slots.quality_names();
    // Per box: volume, mass, volume without a density, each label's volume,
    // then per quality its sums of volume times value, volume, mass times
    // value and mass.
    let (label_count, quality_count) = (labels.len(), qualities.len());
    let stride = 3 + label_count + 4 * quality_count;
    let width = 2 + label_count + quality_count;
    let whole = size.x * size.y * size.z;
    let sliver = 1e-9 * whole;
    let finished = AtomicU64::new(0);
    // The grid's cell rows inside a box row.
    let cell_rows = |source: &RegularSource<'_>, row: usize| {
        let ([_, per_y], [_, offset_y]) = (source.per_block, source.offset);
        (row as i64 * per_y - offset_y).max(0)..((row as i64 + 1) * per_y - offset_y).min(source.grid.lattice.rows() as i64)
    };
    let measured: Vec<_> = (0..rows)
        .into_par_iter()
        .map(|row| {
            if cancel.is_cancelled() {
                return None;
            }
            if sources.iter().all(|source| cell_rows(source, row).is_empty()) {
                return Some((Vec::new(), Vec::new()));
            }
            let mut sums = vec![0.0; columns * layers * stride];
            for source in &sources {
                let grid = source.grid;
                let cell_columns = grid.lattice.columns() as i64;
                let [per_x, _] = source.per_block;
                let [offset_x, _] = source.offset;
                for cell_row in cell_rows(source, row) {
                    for cell_column in 0..cell_columns {
                        let cell = (cell_row * cell_columns + cell_column) as usize;
                        let column = (cell_column + offset_x).div_euclid(per_x);
                        if !grid.active[cell] || !(0..columns as i64).contains(&column) {
                            continue;
                        }
                        // As the grid's own summary weighs a cell, a density at
                        // or below 0 is none; values keep their sign, as the
                        // derived model of one grid carries them.
                        let density = source.density.and_then(|values| values.values[cell]).filter(|value| value.is_finite() && *value > 0.0);
                        grid.layer_volumes(cell, source.stretch, lower.z, size.z, layers, |layer, volume| {
                            let at = (layer * columns + column as usize) * stride;
                            let sum = &mut sums[at..at + stride];
                            sum[0] += volume;
                            match density {
                                Some(density) => sum[1] += volume * density,
                                None => sum[2] += volume,
                            }
                            sum[3 + source.label] += volume;
                            for (values, merged) in &source.qualities {
                                let Some(value) = values.values[cell].filter(|value| value.is_finite()) else {
                                    continue;
                                };
                                let at = 3 + label_count + 4 * merged;
                                sum[at] += volume * value;
                                sum[at + 1] += volume;
                                if let Some(density) = density {
                                    sum[at + 2] += volume * density * value;
                                    sum[at + 3] += volume * density;
                                }
                            }
                        });
                    }
                }
            }
            let (mut at, mut records) = (Vec::new(), Vec::new());
            for layer in 0..layers {
                for column in 0..columns {
                    let sum = &sums[(layer * columns + column) * stride..][..stride];
                    if sum[0] <= sliver {
                        continue;
                    }
                    let known = sum[2] <= sliver;
                    at.push([column, row, layer]);
                    records.push(sum[0]);
                    records.push(if known { sum[1] } else { f64::NAN });
                    records.extend_from_slice(&sum[3..3 + label_count]);
                    for quality in 0..quality_count {
                        let sums = &sum[3 + label_count + 4 * quality..][..4];
                        let (weighted, weight) = if known { (sums[2], sums[3]) } else { (sums[0], sums[1]) };
                        records.push(if weight > 0.0 { weighted / weight } else { f64::NAN });
                    }
                }
            }
            progress.set_items(finished.fetch_add(1, Ordering::Relaxed) + 1, rows as u64);
            Some((at, records))
        })
        .collect();
    // Rows joined in order, each freed as it is taken.
    let count = measured.iter().map(|row| row.as_ref().map_or(0, |(at, _)| at.len())).sum::<usize>();
    let mut at = Vec::with_capacity(count);
    let mut records = Vec::with_capacity(count * width);
    for row in measured {
        let (row_at, row_records) = row.ok_or(RegularRefusal::Cancelled)?;
        at.extend(row_at);
        records.extend(row_records);
    }
    Ok(RegularBoxes {
        dims,
        labels,
        qualities,
        at,
        records,
    })
}

/// Several grids' values as one block model's columns: qualities merged by
/// [`same_quality_name`] and named as first met, every grid's density in
/// one. One name may carry one unit only, a blank unit agreeing with any.
#[derive(Default)]
pub(crate) struct ValueSlots<'a> {
    /// Each quality's name as first met, its first unit given and that
    /// unit's grid, blank until one is.
    qualities: Vec<(&'a str, &'a str, usize)>,
    /// The same for the first density met.
    density: Option<(&'a str, &'a str, usize)>,
}

impl<'a> ValueSlots<'a> {
    /// The merged quality `values` of the grid at position `index` fill,
    /// `None` for its density; refused when its unit differs from the one
    /// given first for that name.
    pub(crate) fn slot(&mut self, values: &'a CellValues, index: usize) -> Result<Option<usize>, RegularRefusal> {
        let slot = match values.role {
            CellValueRole::Density => None,
            CellValueRole::Quality => Some(match self.qualities.iter().position(|(name, _, _)| same_quality_name(name, &values.name)) {
                Some(slot) => slot,
                None => {
                    self.qualities.push((values.name.as_str(), "", index));
                    self.qualities.len() - 1
                }
            }),
        };
        let held = match slot {
            Some(slot) => &mut self.qualities[slot],
            None => self.density.get_or_insert((values.name.as_str(), "", index)),
        };
        let unit = values.unit.trim();
        if held.1.is_empty() {
            (held.1, held.2) = (unit, index);
        } else if !unit.is_empty() && unit != held.1 {
            return Err(RegularRefusal::UnitsDiffer {
                quality: held.0.to_owned(),
                first: held.1.to_owned(),
                first_grid: held.2,
                second: unit.to_owned(),
                second_grid: index,
            });
        }
        Ok(slot)
    }

    /// Each merged quality's name, by slot.
    pub(crate) fn quality_names(&self) -> Vec<String> {
        self.qualities.iter().map(|(name, _, _)| (*name).to_owned()).collect()
    }
}

/// How many `blocks`, each its plan cell on one lattice, bottom and top,
/// reach below the top of another block in their cell: taken bottom up, a
/// block counts when its bottom lies below the highest top before it by
/// more than `1e-9` of its own height, so two blocks sharing a horizon do
/// not count.
pub(crate) fn stacked_overlaps(mut blocks: Vec<([i64; 2], f64, f64)>) -> usize {
    blocks.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut count = 0;
    let mut highest: Option<([i64; 2], f64)> = None;
    for (cell, bottom, top) in blocks {
        let reach = match highest {
            Some((held, reach)) if held == cell => {
                if reach - bottom > 1e-9 * (top - bottom) {
                    count += 1;
                }
                reach.max(top)
            }
            _ => top,
        };
        highest = Some((cell, reach));
    }
    count
}
