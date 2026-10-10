//! Integrate a closed, outward-oriented boundary's signed vertical columns over
//! a block. Each column is measured analytically - over an upright block in
//! plan, where the roof's height is linear, and otherwise clipped as a convex
//! polyhedron; no point sampling or centroid classification is involved.
//! Downward faces subtract the space below the floor, including cavities, from
//! the space below the roof.
use glam::{DVec2, DVec3};

use crate::{
    app::jobs::CancelFlag,
    model::{block_model::BlockBounds, formats::block_model_data::BlockModelData},
};

type Polyhedron = Vec<Vec<DVec3>>;

/// Buffers reused across every block/triangle pair in one reserve run.
///
/// The column integral clips a fresh copy of the block against four planes per
/// candidate triangle, and a block model puts millions of pairs through it.
/// Held here, the working polyhedron and the clipper's own vectors are
/// allocated once and overwritten in place; owned locally, they were six or
/// more mallocs per triangle.
#[derive(Default)]
pub(super) struct Scratch {
    stack: Vec<usize>,
    poly: Polyhedron,
    cap: Vec<DVec3>,
    output: Vec<DVec3>,
    column: Vec<DVec2>,
    part: Vec<DVec2>,
}

pub(super) struct Block {
    center: DVec3,
    scale: f64,
    faces: Polyhedron,
    /// The block's corners, in the same local frame as `faces`.
    corners: [DVec3; 8],
    /// For a block standing upright, its outline and levels in the same frame,
    /// so the columns over it can be measured in plan.
    footprint: Option<Footprint>,
    volume: f64,
    pub(super) min: DVec3,
    pub(super) max: DVec3,
}

impl Block {
    pub(super) fn new(model: &BlockModelData, bounds: BlockBounds) -> anyhow::Result<Self> {
        let size = bounds.upper - bounds.lower;
        anyhow::ensure!(bounds.lower.is_finite() && size.is_finite() && size.min_element() > 0.0, "Invalid block bounds");
        let center = model.local_to_world(bounds.lower + size * 0.5);
        let offsets = std::array::from_fn::<_, 8, _>(|i| {
            model.rotation()
                * (size
                    * DVec3::new(
                        if i & 1 == 0 { -0.5 } else { 0.5 },
                        if i & 2 == 0 { -0.5 } else { 0.5 },
                        if i & 4 == 0 { -0.5 } else { 0.5 },
                    ))
        });
        let scale = offsets.iter().map(|p| p.abs().max_element()).fold(0.0, f64::max);
        anyhow::ensure!(center.is_finite() && scale.is_finite() && scale > 0.0, "Invalid block transform");
        let points = offsets.map(|p| p / scale);
        let up = model.rotation() * DVec3::Z;
        let faces = [[0, 1, 3, 2], [4, 5, 7, 6], [0, 1, 5, 4], [2, 3, 7, 6], [0, 2, 6, 4], [1, 3, 7, 5]]
            .map(|face| face.map(|i| points[i]).to_vec())
            .to_vec();
        let volume = polyhedron_volume(&faces);
        anyhow::ensure!(
            volume.is_finite() && volume > 0.0 && model.rotation().determinant().abs() > 0.0,
            "Degenerate block transform"
        );
        Ok(Self {
            center,
            scale,
            faces,
            corners: points,
            footprint: (up.x == 0.0 && up.y == 0.0).then(|| Footprint {
                outline: [0, 1, 3, 2].map(|i| points[i].truncate()),
                floor: points.iter().map(|point| point.z).fold(f64::INFINITY, f64::min),
                roof: points.iter().map(|point| point.z).fold(f64::NEG_INFINITY, f64::max),
            }),
            volume,
            min: center + offsets.iter().copied().fold(DVec3::splat(f64::INFINITY), DVec3::min),
            max: center + offsets.iter().copied().fold(DVec3::splat(f64::NEG_INFINITY), DVec3::max),
        })
    }

    /// This block's own volume in world units, so a measured overlap
    /// fraction can be reported as cubic metres of covered ground.
    pub(super) fn world_volume(&self) -> f64 {
        self.volume * self.scale * self.scale * self.scale
    }

    pub(super) fn fraction(&self, piece: &super::ReservePiece, scratch: &mut Scratch, cancel: &CancelFlag) -> anyhow::Result<f64> {
        let bounds = piece.mesh.bounds();
        if self.max.x <= bounds.min.x || self.min.x >= bounds.max.x || self.max.y <= bounds.min.y || self.min.y >= bounds.max.y {
            return Ok(0.0);
        }
        let mut sum = 0.0;
        let mut correction = 0.0;
        let Scratch {
            stack,
            poly,
            cap,
            output,
            column,
            part,
        } = scratch;
        piece
            .spatial
            .for_each_xy_bounds_candidate_index_with_stack(self.min.truncate(), self.max.truncate(), stack, |index| {
                if cancel.is_cancelled() {
                    return;
                }
                let triangle = piece.mesh.triangle(index).expect("BVH triangle exists");
                let mut points = triangle.vertices.map(|p| (DVec3::new(p.x, p.y, p.z) - self.center) / self.scale);
                let normal = (points[1] - points[0]).cross(points[2] - points[0]);
                if normal.z == 0.0 {
                    return;
                }
                let sign = normal.z.signum();
                if sign < 0.0 {
                    points.swap(1, 2);
                }
                let roof = (normal * sign).normalize();
                // The column's three sides, then the roof, in the order they clip.
                let planes: [(DVec3, f64); 4] = std::array::from_fn(|i| {
                    if i == 3 {
                        return (roof, roof.dot(points[0]));
                    }
                    let a = points[i];
                    let edge = points[(i + 1) % 3] - a;
                    let outward = DVec3::new(edge.y, -edge.x, 0.0).normalize();
                    (outward, outward.dot(a))
                });
                // Most candidates only share bounds with the block: a block
                // wholly outside one side of the triangle's column is one the
                // clips below would empty, so it is passed over before them.
                if planes[..3]
                    .iter()
                    .any(|&(outward, distance)| self.corners.iter().all(|&corner| outward.dot(corner) - distance > MISSES_COLUMN))
                {
                    return;
                }
                // A column whose roof passes wholly beneath the block holds none
                // of it: the floors under a block are most of what is left.
                if self.corners.iter().all(|&corner| signed(roof, planes[3].1, corner) >= 0.0) {
                    return;
                }
                let volume = match &self.footprint {
                    Some(footprint) => footprint.column_volume(&planes, column, part),
                    None => {
                        // Overwrites the retained face buffers rather than allocating
                        // a new set for every triangle.
                        poly.clone_from(&self.faces);
                        for &(normal, distance) in &planes {
                            clip(poly, normal, distance, cap, output);
                            if poly.is_empty() {
                                return;
                            }
                        }
                        polyhedron_volume(poly)
                    }
                };
                // Compensated summation limits cancellation between floor and roof.
                let value = sign * volume - correction;
                let next = sum + value;
                correction = (next - sum) - value;
                sum = next;
            });
        anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
        let fraction = sum / self.volume;
        anyhow::ensure!(
            fraction.is_finite() && (-1e-7..=1.0 + 1e-7).contains(&fraction),
            "Invalid solid/block overlap fraction {fraction}; check mesh orientation and closure"
        );
        Ok(fraction.clamp(0.0, 1.0))
    }
}

/// An upright block seen from above: its plan outline and the elevations of
/// its floor and roof, in the block's scaled frame.
struct Footprint {
    outline: [DVec2; 4],
    floor: f64,
    roof: f64,
}

impl Footprint {
    /// The block's volume inside a triangle's column, given the column's three
    /// sides and its roof as outward planes. Within the column's plan outline
    /// the roof's elevation is linear, so the ground it stands over above any
    /// level is that ground's area times the roof's height over the level at
    /// its centroid. The block holds what stands above its floor less what
    /// stands above its roof: each part is nothing along its own boundary, so
    /// a roof lying exactly at either level is counted once.
    fn column_volume(&self, planes: &[(DVec3, f64); 4], column: &mut Vec<DVec2>, part: &mut Vec<DVec2>) -> f64 {
        column.clear();
        column.extend_from_slice(&self.outline);
        for &(normal, distance) in &planes[..3] {
            clip_ring(column, normal.truncate(), distance, part);
            std::mem::swap(column, part);
            if column.len() < 3 {
                return 0.0;
            }
        }
        // The roof plane n.p = d stands at least at elevation z where
        // n.xy.p <= d - n.z z, n.z being positive.
        let (normal, distance) = planes[3];
        let (plan, rise) = (normal.truncate(), normal.z);
        let mut above = |level: f64| {
            clip_ring(column, plan, distance - rise * level, part);
            let (area, centroid) = area_and_centroid(part);
            if area > 0.0 { area * ((distance - plan.dot(centroid)) / rise - level) } else { 0.0 }
        };
        above(self.floor) - above(self.roof)
    }
}

/// `ring` clipped to the half-plane `normal.p <= distance`, into `out`.
fn clip_ring(ring: &[DVec2], normal: DVec2, distance: f64, out: &mut Vec<DVec2>) {
    out.clear();
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (da, db) = (normal.dot(a) - distance, normal.dot(b) - distance);
        if da <= 0.0 {
            out.push(a);
        }
        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
            out.push(a + (b - a) * (da / (da - db)));
        }
    }
}

/// Unsigned area and centroid of a simple ring.
fn area_and_centroid(ring: &[DVec2]) -> (f64, DVec2) {
    if ring.len() < 3 {
        return (0.0, DVec2::ZERO);
    }
    let origin = ring[0];
    let (mut twice, mut moment) = (0.0, DVec2::ZERO);
    for i in 1..ring.len() - 1 {
        let (a, b) = (ring[i] - origin, ring[i + 1] - origin);
        let cross = a.perp_dot(b);
        twice += cross;
        moment += (a + b) * cross;
    }
    if twice == 0.0 {
        return (0.0, origin);
    }
    (twice.abs() / 2.0, origin + moment / (3.0 * twice))
}

/// How far, in the block's scaled frame, every corner must lie outside a side
/// of a triangle's column for the block to be passed over without clipping:
/// far above roundoff, so only blocks the clips would empty are skipped.
const MISSES_COLUMN: f64 = 1e-9;

/// Faces are convex and may have either winding. Tetrahedra measured from an
/// interior point partition the polyhedron, so each contributes positive volume.
fn polyhedron_volume(faces: &Polyhedron) -> f64 {
    let count = faces.iter().map(Vec::len).sum::<usize>();
    if count == 0 {
        return 0.0;
    }
    let center = faces.iter().flatten().copied().sum::<DVec3>() / count as f64;
    faces
        .iter()
        .map(|face| {
            (1..face.len().saturating_sub(1))
                .map(|i| (face[0] - center).dot((face[i] - center).cross(face[i + 1] - center)).abs() / 6.0)
                .sum::<f64>()
        })
        .sum()
}

/// How far `point` lies outside the plane `n.p = distance`, with roundoff-sized
/// distances snapped to zero so every test reads a point the same way.
fn signed(normal: DVec3, distance: f64, point: DVec3) -> f64 {
    let d = normal.dot(point) - distance;
    if d.abs() <= EPS { 0.0 } else { d }
}

const EPS: f64 = 1e-12;

/// Keep n.p <= distance. Working near the block origin avoids mine-coordinate
/// cancellation. Snap roundoff-sized plane distances consistently on all faces.
fn clip(faces: &mut Polyhedron, normal: DVec3, distance: f64, cap: &mut Vec<DVec3>, output: &mut Vec<DVec3>) {
    let signed = |point: DVec3| signed(normal, distance, point);
    // Most queried triangles cover the entire block in one or more planes.
    // Avoid rebuilding faces unless this plane actually cuts the block.
    if !faces.iter().flatten().any(|&point| signed(point) > 0.0) {
        return;
    }
    if !faces.iter().flatten().any(|&point| signed(point) < 0.0) {
        faces.clear();
        return;
    }
    cap.clear();
    let mut removed = false;
    for face in faces.iter_mut() {
        output.clear();
        output.reserve(face.len() + 1);
        for i in 0..face.len() {
            let a = face[i];
            let b = face[(i + 1) % face.len()];
            let da = signed(a);
            let db = signed(b);
            removed |= da > 0.0;
            if da <= 0.0 {
                output.push(a);
            }
            if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
                let point = a + (b - a) * (da / (da - db));
                output.push(point);
                cap.push(point);
            }
            if da == 0.0 {
                cap.push(a);
            }
        }
        // A face left lying in the plane - a block face the cut runs a
        // rounding error off - is the cap's own ground, and the cap is built
        // from its points; kept as well, it would count that slice twice.
        if output.iter().all(|&point| signed(point) == 0.0) {
            output.clear();
        }
        // Swapped rather than assigned: the face takes the list just built and
        // the scratch buffer inherits the face's allocation for the next one.
        std::mem::swap(face, output);
    }
    faces.retain(|face| face.len() >= 3);
    if !removed || faces.is_empty() {
        return;
    }
    let mut unique = Vec::<DVec3>::with_capacity(cap.len());
    for &p in cap.iter() {
        if !unique.iter().any(|q| p.distance_squared(*q) <= EPS * EPS) {
            unique.push(p);
        }
    }
    if unique.len() < 3 {
        return;
    }
    let center = unique.iter().copied().sum::<DVec3>() / unique.len() as f64;
    let axis = if normal.x.abs() < 0.8 { DVec3::X } else { DVec3::Y };
    let u = normal.cross(axis).normalize();
    let v = normal.cross(u);
    unique.sort_by(|a, b| pseudo_angle(*a - center, u, v).total_cmp(&pseudo_angle(*b - center, u, v)));
    faces.push(unique);
}

/// Orders a direction in the cap plane exactly as `atan2` would, without the
/// transcendental call.
///
/// Only the ordering of the cap's points matters - the angle itself is never
/// read - and this rises strictly with the true angle across the same
/// (-pi, pi] sweep, so the polygon comes out in the same rotation for a
/// division and a few absolute values. `atan2` was the single most expensive
/// thing in the whole reserve calculation.
fn pseudo_angle(offset: DVec3, u: DVec3, v: DVec3) -> f64 {
    let (x, y) = (offset.dot(u), offset.dot(v));
    let sum = x.abs() + y.abs();
    if sum == 0.0 {
        return 0.0;
    }
    let quadrant = x / sum;
    if y < 0.0 { quadrant - 1.0 } else { 1.0 - quadrant }
}
