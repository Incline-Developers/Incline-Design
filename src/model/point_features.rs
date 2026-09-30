//! Per-point features for the learned point classifier: the shape of the
//! returns around each point at several scales.
//!
//! Neighbourhoods are voxels rather than radius searches. At each scale the
//! cloud is bucketed into cubes of that edge, and a point's neighbourhood is
//! its cube and the 26 around it - one sort and a few lookups per occupied
//! cube, where a radius search would be a tree query per point per scale.
//! Every point in a cube shares one neighbourhood, so its shape is worked out
//! once per cube; only where the point sits within it is per point.
//!
//! The cloud is processed in square tiles, each with a halo wide enough that
//! the coarsest neighbourhood of any point in the tile lies inside it, so
//! memory stays bounded by the tile however large the cloud and tiles run in
//! parallel. The voxel grid is aligned to the cloud, not the tile, so a point's
//! features do not depend on which tile it fell in.

use anyhow::{Result, ensure};
use glam::{DMat3, DVec2, DVec3};
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    model::{geometry::symmetric_eigen, progress::Phase},
};

/// Voxel edge, in metres, at each scale.
const SCALES: [f64; 4] = [1.0, 2.0, 4.0, 8.0];
/// Features each scale contributes; see [`Shape::features`].
const PER_SCALE: usize = 7;
/// Features the caller appends after the per-scale ones.
pub(crate) const EXTRA_FEATURES: usize = 4;
pub(crate) const FEATURE_COUNT: usize = SCALES.len() * PER_SCALE + EXTRA_FEATURES;
/// How far a neighbourhood reaches from its point: across the point's own
/// voxel and one more at the coarsest scale.
const HALO: f64 = 2.0 * SCALES[SCALES.len() - 1];
/// Returns a tile aims to hold, halo excluded, which sets the tile's side from
/// the cloud's density.
const TILE_POINTS: f64 = 400_000.0;
/// Bounds on the tile side in metres: not so small the halo dwarfs it, not so
/// large a sparse cloud runs on a handful of threads.
const TILE_SIDE: std::ops::RangeInclusive<f64> = 64.0..=512.0;
/// Value of a shape feature where too few returns share a neighbourhood to
/// give it a shape.
const NO_SHAPE: f32 = -1.0;
/// Returns a neighbourhood needs before its shape means anything.
const MIN_SHAPE_POINTS: u32 = 4;
/// Bits per axis in a packed voxel key.
const KEY_BITS: u32 = 21;

/// Work out the features of every point `wanted` picks and pass each through
/// `map`, returning `(index, map(features))` in no particular order.
///
/// Every point `member` flags shapes the neighbourhoods; `wanted` must be a
/// subset of it. `extra` supplies the last [`EXTRA_FEATURES`] per point.
pub(crate) fn point_features<T: Send>(
    points: &[DVec3],
    member: &[bool],
    wanted: impl Fn(usize) -> bool + Sync,
    extra: impl Fn(usize) -> [f32; EXTRA_FEATURES] + Sync,
    map: impl Fn(&[f32; FEATURE_COUNT]) -> T + Sync,
    cancel: &CancelFlag,
    progress: &Phase,
) -> Result<Vec<(u32, T)>> {
    ensure!(u32::try_from(points.len()).is_ok(), "The point cloud has too many points to classify in one pass");
    let Some((min, max)) = points
        .par_iter()
        .zip(member.par_iter())
        .filter(|(_, member)| **member)
        .map(|(point, _)| (*point, *point))
        .reduce_with(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)))
    else {
        return Ok(Vec::new());
    };
    ensure!(
        ((max - min) / SCALES[0]).max_element() < (1u64 << KEY_BITS) as f64,
        "The point cloud is too large to classify in one pass"
    );
    let members = member.par_iter().filter(|member| **member).count();
    let extent = (max - min).truncate().max(DVec2::ONE);
    let side = (TILE_POINTS * extent.x * extent.y / members as f64).sqrt().clamp(*TILE_SIDE.start(), *TILE_SIDE.end());
    let cols = (extent.x / side).floor() as usize + 1;
    let rows = (extent.y / side).floor() as usize + 1;
    let tile_of = |point: DVec3| {
        let cell = ((point.truncate() - min.truncate()) / side).floor();
        (cell.y as usize).min(rows - 1) * cols + (cell.x as usize).min(cols - 1)
    };

    // Point indices sorted by tile, and where each tile's run starts.
    let mut order: Vec<(u32, u32)> = (0..points.len())
        .into_par_iter()
        .filter(|&index| member[index])
        .map(|index| (tile_of(points[index]) as u32, index as u32))
        .collect();
    order.par_sort_unstable();
    let starts: Vec<usize> = (0..=cols * rows).map(|tile| order.partition_point(|(key, _)| (*key as usize) < tile)).collect();
    progress.set_fraction(0.05);
    ensure!(!cancel.is_cancelled(), "Cancelled");

    let done = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<Vec<(u32, T)>> = (0..cols * rows)
        .into_par_iter()
        .map(|tile| {
            if cancel.is_cancelled() {
                return Vec::new();
            }
            let own: Vec<u32> = order[starts[tile]..starts[tile + 1]]
                .iter()
                .map(|(_, index)| *index)
                .filter(|&index| wanted(index as usize))
                .collect();
            let output = if own.is_empty() {
                Vec::new()
            } else {
                let (column, row) = (tile % cols, tile / cols);
                let low = min.truncate() + DVec2::new(column as f64, row as f64) * side - DVec2::splat(HALO);
                let high = low + DVec2::splat(side + 2.0 * HALO);
                let mut local = Vec::new();
                for y in row.saturating_sub(1)..(row + 2).min(rows) {
                    for x in column.saturating_sub(1)..(column + 2).min(cols) {
                        let near = y * cols + x;
                        local.extend(order[starts[near]..starts[near + 1]].iter().map(|(_, index)| *index).filter(|&index| {
                            let point = points[index as usize].truncate();
                            point.cmpge(low).all() && point.cmplt(high).all()
                        }));
                    }
                }
                tile_features(points, min, &local, &own, &extra, &map)
            };
            let finished = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            progress.set_fraction(0.05 + 0.95 * finished as f32 / (cols * rows) as f32);
            output
        })
        .collect();
    ensure!(!cancel.is_cancelled(), "Cancelled");
    Ok(results.into_iter().flatten().collect())
}

/// Features of the `own` points of one tile, from the neighbourhoods the
/// `local` points (own and halo) make up.
fn tile_features<T>(
    points: &[DVec3],
    origin: DVec3,
    local: &[u32],
    own: &[u32],
    extra: &impl Fn(usize) -> [f32; EXTRA_FEATURES],
    map: &impl Fn(&[f32; FEATURE_COUNT]) -> T,
) -> Vec<(u32, T)> {
    // Moments are summed about a point inside the tile so they stay precise
    // in f64 however far the cloud sits from the world origin.
    let reference = points[own[0] as usize];
    let mut features = vec![[0.0f32; FEATURE_COUNT]; own.len()];
    for (scale_index, &scale) in SCALES.iter().enumerate() {
        let key_of = |point: DVec3| {
            let cell = ((point - origin) / scale).floor();
            ((cell.x as u64) << (2 * KEY_BITS)) | ((cell.y as u64) << KEY_BITS) | cell.z as u64
        };
        let mut keyed: Vec<(u64, u32)> = local.iter().map(|&index| (key_of(points[index as usize]), index)).collect();
        keyed.sort_unstable();
        let mut voxels: Vec<(u64, Moments)> = Vec::new();
        for &(key, index) in &keyed {
            let offset = points[index as usize] - reference;
            match voxels.last_mut() {
                Some((last, moments)) if *last == key => moments.add(offset),
                _ => voxels.push((key, Moments::of(offset))),
            }
        }
        let find = |key: u64| voxels.binary_search_by_key(&key, |(key, _)| *key).ok();
        let mut shapes: Vec<Option<Shape>> = vec![None; voxels.len()];
        for (slot, &index) in own.iter().enumerate() {
            let point = points[index as usize];
            let voxel = find(key_of(point)).expect("every own point is local");
            let shape = *shapes[voxel].get_or_insert_with(|| {
                let key = voxels[voxel].0;
                let [x, y, z] = [key >> (2 * KEY_BITS), (key >> KEY_BITS) & ((1 << KEY_BITS) - 1), key & ((1 << KEY_BITS) - 1)];
                let mut total = Moments::default();
                for dx in -1i64..=1 {
                    for dy in -1i64..=1 {
                        for dz in -1i64..=1 {
                            let neighbour = [x as i64 + dx, y as i64 + dy, z as i64 + dz];
                            if neighbour.iter().all(|axis| (0..1i64 << KEY_BITS).contains(axis))
                                && let Some(found) = find(((neighbour[0] as u64) << (2 * KEY_BITS)) | ((neighbour[1] as u64) << KEY_BITS) | neighbour[2] as u64)
                            {
                                total.merge(&voxels[found].1);
                            }
                        }
                    }
                }
                Shape::of(&total)
            });
            let start = scale_index * PER_SCALE;
            features[slot][start..start + PER_SCALE].copy_from_slice(&shape.features(point - reference, scale));
        }
    }
    own.iter()
        .zip(features.iter_mut())
        .map(|(&index, features)| {
            features[SCALES.len() * PER_SCALE..].copy_from_slice(&extra(index as usize));
            (index, map(features))
        })
        .collect()
}

/// Running sums over a set of returns, about a shared reference point.
#[derive(Clone, Copy)]
struct Moments {
    count: u32,
    sum: DVec3,
    /// xx, xy, xz, yy, yz, zz.
    products: [f64; 6],
    low: f64,
    high: f64,
}

impl Default for Moments {
    fn default() -> Self {
        Self {
            count: 0,
            sum: DVec3::ZERO,
            products: [0.0; 6],
            low: f64::INFINITY,
            high: f64::NEG_INFINITY,
        }
    }
}

impl Moments {
    fn of(offset: DVec3) -> Self {
        let mut moments = Self::default();
        moments.add(offset);
        moments
    }

    fn add(&mut self, p: DVec3) {
        self.count += 1;
        self.sum += p;
        self.products[0] += p.x * p.x;
        self.products[1] += p.x * p.y;
        self.products[2] += p.x * p.z;
        self.products[3] += p.y * p.y;
        self.products[4] += p.y * p.z;
        self.products[5] += p.z * p.z;
        self.low = self.low.min(p.z);
        self.high = self.high.max(p.z);
    }

    fn merge(&mut self, other: &Self) {
        self.count += other.count;
        self.sum += other.sum;
        for (mine, theirs) in self.products.iter_mut().zip(other.products) {
            *mine += theirs;
        }
        self.low = self.low.min(other.low);
        self.high = self.high.max(other.high);
    }
}

/// The shape of one neighbourhood: its centroid, the normal of the plane that
/// best fits it, and how line-, plane- or volume-like its spread is.
#[derive(Clone, Copy)]
struct Shape {
    mean: DVec3,
    /// Upward unit normal, or `None` where the neighbourhood is too sparse.
    normal: Option<DVec3>,
    linearity: f32,
    planarity: f32,
    scattering: f32,
    low: f64,
    high: f64,
}

impl Shape {
    fn of(moments: &Moments) -> Self {
        let n = f64::from(moments.count);
        let mean = moments.sum / n;
        let [xx, xy, xz, yy, yz, zz] = moments.products;
        let covariance = [
            [xx / n - mean.x * mean.x, xy / n - mean.x * mean.y, xz / n - mean.x * mean.z],
            [xy / n - mean.x * mean.y, yy / n - mean.y * mean.y, yz / n - mean.y * mean.z],
            [xz / n - mean.x * mean.z, yz / n - mean.y * mean.z, zz / n - mean.z * mean.z],
        ];
        let (values, vectors) = symmetric_eigen(DMat3::from_cols_array_2d(&covariance));
        let [largest, middle, smallest] = values.map(|value| value.max(0.0));
        let mut shape = Self {
            mean,
            normal: None,
            linearity: NO_SHAPE,
            planarity: NO_SHAPE,
            scattering: NO_SHAPE,
            low: moments.low,
            high: moments.high,
        };
        if moments.count >= MIN_SHAPE_POINTS && largest > 1e-12 {
            let normal = vectors[2];
            shape.normal = Some(if normal.z < 0.0 { -normal } else { normal });
            shape.linearity = ((largest - middle) / largest) as f32;
            shape.planarity = ((middle - smallest) / largest) as f32;
            shape.scattering = (smallest / largest) as f32;
        }
        shape
    }

    /// For a point at `offset` from the moments' reference, at a scale whose
    /// voxels are `scale` across:
    ///
    /// 0. linearity, 1. planarity, 2. scattering - the eigenvalue ratios;
    /// 3. verticality, one less the normal's upward component;
    /// 4. the point's height off the fitted plane along its normal;
    /// 5. its height over the lowest return around it;
    /// 6. its depth under the highest.
    ///
    /// Lengths are in voxels so a feature means the same at every scale.
    fn features(&self, offset: DVec3, scale: f64) -> [f32; PER_SCALE] {
        let (verticality, off_plane) = match self.normal {
            Some(normal) => ((1.0 - normal.z) as f32, ((offset - self.mean).dot(normal) / scale) as f32),
            None => (NO_SHAPE, NO_SHAPE),
        };
        [
            self.linearity,
            self.planarity,
            self.scattering,
            verticality,
            off_plane,
            ((offset.z - self.low) / scale) as f32,
            ((self.high - offset.z) / scale) as f32,
        ]
    }
}
