//! Ground classification for point clouds: an isolated-point noise pass, then
//! a cloth simulation filter after Zhang et al. (2016).
//!
//! A cloth filter stands the problem on its head. A stiff cloth is pressed up
//! against the cloud from beneath, rests on the lowest surface it can reach,
//! and cannot bend far enough to climb into canopy, plant or buildings.
//! Whatever lies within a threshold of where it settled is ground.
//!
//! Four departures from the published filter, each for open-pit ground:
//!
//! - The cloth's rest shape is solved for directly (see [`settle_membrane`])
//!   rather than stepped towards, so a 300 m deep pit settles as cleanly as a
//!   paddock and the stiffness means the same thing in both.
//! - Each particle collides with a plane fitted through the returns around it
//!   rather than the nearest return, which on a 70 degree wall can sit a metre
//!   off the face.
//! - Slope recovery walks down faces as steep as a wall stands but climbs only
//!   gently, so it recovers crests without stepping up onto plant.
//! - Returns on a face too steep for any height field - vertical or
//!   overhanging - are ground when touched ground brackets them above and
//!   below (see [`FaceBounds`]), which a roof or canopy never does.
//!
//! Ground points are then those within the threshold of the cloth measured
//! across it, not straight down, so a wall is judged by its face.

use std::{
    collections::VecDeque,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
};

use anyhow::{Result, bail, ensure};
use glam::{DVec2, DVec3};
use rayon::prelude::*;

use crate::{
    app::jobs::CancelFlag,
    model::{
        point_cloud::{CLASS_GROUND, CLASS_UNCLASSIFIED},
        progress::Phase,
    },
};

/// ASPRS "low point (noise)", which is what an isolated return is written as.
pub(crate) const CLASS_LOW_NOISE: u8 = 7;

/// Upper bound on cloth particles. Each costs a few dozen bytes while the
/// filter runs, so this caps the working set in the hundreds of megabytes
/// whatever the cloud's extent.
const MAX_CLOTH_NODES: usize = 16_000_000;
/// Upper bound on noise voxels per axis, set by the 21 bits each axis gets in
/// the packed voxel key.
const MAX_VOXELS_PER_AXIS: u64 = 1 << 21;
/// Red-black sweeps per pyramid level before it is taken as settled.
const MAX_SWEEPS: usize = 400;
/// Over-relaxation factor for the projected sweeps.
const RELAXATION: f64 = 1.85;
/// Side, in particles, below which the pyramid stops coarsening.
const COARSEST_SIDE: usize = 32;
/// Particles of cloth kept beyond the cloud's footprint on every side, so edge
/// points interpolate between particles that were settled like any other.
const CLOTH_MARGIN: usize = 2;
/// Furthest above the settled cloth, in ground thresholds, slope recovery
/// will lift it onto a continuing slope. A stiff cloth hangs this far under a
/// tall bench's crest.
const RECOVERY_GAP: f64 = 10.0;
/// How many particles either side face recovery looks for the ground a face
/// joins. A vertical face spans at most a particle or two in plan; reaching
/// further would start bracketing plant parked near the foot of a wall.
const FACE_REACH: usize = 3;
/// Smallest step, in ground thresholds, between the ground levels either side
/// that counts as a face rather than rough ground the cloth already covers.
const FACE_MIN_STEP: f64 = 2.0;
/// Steepest climb slope recovery takes from one particle to the next, in
/// ground thresholds. Plant and structures are only ever met climbing.
const RECOVERY_RISE: f64 = 0.5;

/// The ground a run is tuned for: how stiff the cloth is, and how steep a
/// face slope recovery will follow down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClothTerrain {
    /// Pit walls, benches and stockpiles.
    Steep,
    /// Rolling ground and waste dumps.
    Relief,
    /// Flat ground with large structures: a stiffer cloth that bridges them.
    Flat,
}

impl ClothTerrain {
    /// Tightest curvature (1/m) the cloth takes where it hangs free. It sets
    /// both how closely the cloth rounds a crest - to a radius of the inverse -
    /// and how far it bulges up into a structure it cannot reach the ground
    /// around, `curvature * width^2 / 8` over a span `width` across.
    fn curvature(self) -> f64 {
        match self {
            Self::Steep | Self::Relief => 0.1,
            Self::Flat => 0.04,
        }
    }

    /// Steepest ground, in degrees, slope recovery walks the cloth down.
    fn max_slope_degrees(self) -> f64 {
        match self {
            Self::Steep => 80.0,
            Self::Relief => 60.0,
            Self::Flat => 40.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GroundFilterParams {
    pub(crate) terrain: ClothTerrain,
    /// Cloth particle spacing in metres.
    pub(crate) cloth_resolution: f64,
    /// Greatest distance from the settled cloth a ground point may lie.
    pub(crate) class_threshold: f64,
    /// Let the cloth settle onto steep ground its stiffness held it off.
    pub(crate) slope_recovery: bool,
    /// Mark isolated returns as noise before the cloth runs.
    pub(crate) remove_noise: bool,
    /// Neighbourhood size the noise pass counts within, in metres.
    pub(crate) noise_radius: f64,
    /// Neighbours a return needs within `noise_radius` not to be noise.
    pub(crate) noise_min_neighbours: u32,
}

impl Default for GroundFilterParams {
    fn default() -> Self {
        Self {
            terrain: ClothTerrain::Steep,
            cloth_resolution: 0.5,
            class_threshold: 0.5,
            slope_recovery: true,
            remove_noise: true,
            noise_radius: 1.0,
            noise_min_neighbours: 3,
        }
    }
}

pub(crate) struct GroundFilterOutput {
    /// ASPRS codes in source order.
    pub(crate) codes: Vec<u8>,
    pub(crate) ground: usize,
    pub(crate) noise: usize,
}

/// Classify `points` into ground, noise and unclassified.
pub(crate) fn classify_ground(points: &[DVec3], params: &GroundFilterParams, cancel: &CancelFlag, progress: &Phase) -> Result<GroundFilterOutput> {
    ensure!(
        params.cloth_resolution.is_finite() && params.cloth_resolution > 0.0,
        "The cloth resolution must be greater than zero"
    );
    ensure!(
        params.class_threshold.is_finite() && params.class_threshold > 0.0,
        "The ground threshold must be greater than zero"
    );
    // Every point is relabelled; non-finite ones take no part and stay
    // unclassified.
    let mut codes = vec![CLASS_UNCLASSIFIED; points.len()];
    let finite: Vec<bool> = points.par_iter().map(|point| point.is_finite()).collect();

    let mut noise = 0;
    if params.remove_noise {
        ensure!(params.noise_radius.is_finite() && params.noise_radius > 0.0, "The noise radius must be greater than zero");
        let isolated = isolated_points(points, &finite, params.noise_radius, params.noise_min_neighbours, cancel, &progress.phase(0.0, 0.2))?;
        codes.par_iter_mut().zip(isolated.par_iter()).for_each(|(code, isolated)| {
            if *isolated {
                *code = CLASS_LOW_NOISE;
            }
        });
        noise = isolated.par_iter().filter(|isolated| **isolated).count();
    }
    ensure!(!cancel.is_cancelled(), "Cancelled");

    let cloth_points = |index: usize| finite[index] && codes[index] != CLASS_LOW_NOISE;
    let cloth = Cloth::settle(points, cloth_points, params, cancel, &progress.phase(0.2, 0.95))?;

    codes.par_iter_mut().enumerate().for_each(|(index, code)| {
        if *code == CLASS_LOW_NOISE || !finite[index] {
            return;
        }
        let point = points[index];
        let (height, gradient) = cloth.surface_at(point.truncate());
        // Distance across the cloth rather than straight down to it: on a 70
        // degree wall a return a hand's width off a particle sits a metre
        // above the cloth vertically while lying right on the face.
        let distance = (point.z - height).abs() / (1.0 + gradient.length_squared()).sqrt();
        *code = if distance < params.class_threshold || cloth.on_face(point, params.class_threshold) {
            CLASS_GROUND
        } else {
            CLASS_UNCLASSIFIED
        };
    });
    let ground = codes.par_iter().filter(|code| **code == CLASS_GROUND).count();
    progress.set_fraction(1.0);
    Ok(GroundFilterOutput { codes, ground, noise })
}

/// Returns sampled when measuring spacing. Occupancy on a grid a few spacings
/// wide settles long before this many, however large the cloud.
const SPACING_SAMPLES: usize = 500_000;
/// Cloth spacing recommended per point spacing. A cloth much finer than the
/// returns leaves particles with nothing under them, which on a face steps the
/// cloth instead of sloping it.
const RECOMMENDED_SPACINGS: f64 = 1.5;
/// Cloth resolutions offered as recommendations, in metres.
const RESOLUTION_STEPS: [f64; 17] = [0.1, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 7.5, 10.0, 20.0];

/// Typical plan distance between neighbouring returns: the covered area over
/// the point count, the covered area being the cells of a coarse grid that
/// hold any return. Taking the bounding box instead would count every hole
/// and ragged edge of a survey as sparse ground.
pub(crate) fn plan_spacing(points: &[DVec3]) -> Option<f64> {
    let stride = points.len().div_ceil(SPACING_SAMPLES).max(1);
    let sample: Vec<DVec2> = points.par_iter().step_by(stride).filter(|point| point.is_finite()).map(|point| point.truncate()).collect();
    if sample.len() < 16 {
        return None;
    }
    let (min, max) = sample
        .par_iter()
        .map(|point| (*point, *point))
        .reduce_with(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)))?;
    let extent = max - min;
    if extent.x <= 0.0 || extent.y <= 0.0 {
        return None;
    }
    // Cells about four sampled spacings wide hold some sixteen samples where
    // the survey covers them, so an occupied cell is a covered one.
    let cell = 4.0 * (extent.x * extent.y / sample.len() as f64).sqrt();
    let mut cells: Vec<u64> = sample
        .par_iter()
        .map(|point| {
            let grid = ((*point - min) / cell).floor();
            ((grid.x as u64) << 32) | grid.y as u64
        })
        .collect();
    cells.par_sort_unstable();
    cells.dedup();
    let covered = cells.len() as f64 * cell * cell;
    let counted = sample.len() * stride;
    Some((covered / counted.min(points.len()) as f64).sqrt())
}

/// The cloth resolution to suggest for returns `spacing` apart.
pub(crate) fn recommended_cloth_resolution(spacing: f64) -> f64 {
    let target = spacing * RECOMMENDED_SPACINGS;
    RESOLUTION_STEPS
        .into_iter()
        .find(|step| *step >= target)
        .unwrap_or(RESOLUTION_STEPS[RESOLUTION_STEPS.len() - 1])
}

/// Flag returns with fewer than `min_neighbours` others near them.
///
/// Points are bucketed into cubes of `radius` and a point's neighbours are
/// everything in its own cube and the 26 around it, so "near" means somewhere
/// between one and two radii. That is loose, but it is one sort and a lookup
/// per occupied cube, where a true radius search would be a tree query per
/// point - and a blunder is usually tens of metres from anything.
fn isolated_points(points: &[DVec3], eligible: &[bool], radius: f64, min_neighbours: u32, cancel: &CancelFlag, progress: &Phase) -> Result<Vec<bool>> {
    let Some((min, max)) = points
        .par_iter()
        .zip(eligible.par_iter())
        .filter(|(_, eligible)| **eligible)
        .map(|(point, _)| (*point, *point))
        .reduce_with(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)))
    else {
        return Ok(vec![false; points.len()]);
    };
    let span = ((max - min) / radius).ceil();
    if span.max_element() >= MAX_VOXELS_PER_AXIS as f64 {
        bail!("The noise radius of {radius} m is too small for a cloud this large; raise it");
    }
    let voxel = |point: DVec3| -> [u64; 3] {
        let cell = ((point - min) / radius).floor();
        [cell.x as u64, cell.y as u64, cell.z as u64]
    };
    let pack = |[x, y, z]: [u64; 3]| (x << 42) | (y << 21) | z;

    let keys: Vec<Option<u64>> = points
        .par_iter()
        .zip(eligible.par_iter())
        .map(|(point, eligible)| eligible.then(|| pack(voxel(*point))))
        .collect();
    let mut sorted: Vec<u64> = keys.par_iter().filter_map(|key| *key).collect();
    sorted.par_sort_unstable();
    progress.set_fraction(0.4);
    ensure!(!cancel.is_cancelled(), "Cancelled");

    let mut occupied: Vec<(u64, u32)> = Vec::new();
    for key in sorted {
        match occupied.last_mut() {
            Some((last, count)) if *last == key => *count += 1,
            _ => occupied.push((key, 1)),
        }
    }
    let count_of = |key: u64| occupied.binary_search_by_key(&key, |(key, _)| *key).map_or(0, |index| occupied[index].1);
    // Everything in a cube shares one neighbourhood, so it is counted once per
    // cube rather than once per point.
    let neighbourhoods: Vec<u32> = occupied
        .par_iter()
        .map(|&(key, _)| {
            let [x, y, z] = [key >> 42, (key >> 21) & (MAX_VOXELS_PER_AXIS - 1), key & (MAX_VOXELS_PER_AXIS - 1)];
            let mut total = 0;
            for dx in -1i64..=1 {
                for dy in -1i64..=1 {
                    for dz in -1i64..=1 {
                        let neighbour = [x as i64 + dx, y as i64 + dy, z as i64 + dz];
                        if neighbour.iter().all(|axis| (0..MAX_VOXELS_PER_AXIS as i64).contains(axis)) {
                            total += count_of(pack(neighbour.map(|axis| axis as u64)));
                        }
                    }
                }
            }
            total
        })
        .collect();
    progress.set_fraction(0.8);
    ensure!(!cancel.is_cancelled(), "Cancelled");

    let isolated = keys
        .par_iter()
        .map(|key| {
            key.is_some_and(|key| {
                let index = occupied.binary_search_by_key(&key, |(key, _)| *key).expect("every key was bucketed");
                // The count includes the point itself.
                neighbourhoods[index].saturating_sub(1) < min_neighbours
            })
        })
        .collect();
    progress.set_fraction(1.0);
    Ok(isolated)
}

/// A settled cloth: particle heights on a regular XY grid.
struct Cloth {
    origin: DVec2,
    resolution: f64,
    cols: usize,
    rows: usize,
    heights: Vec<f64>,
    /// Per particle, the lowest and highest ground the cloth touched within
    /// [`FACE_REACH`] of it, when face recovery is on.
    faces: Option<FaceBounds>,
}

impl Cloth {
    fn settle(points: &[DVec3], include: impl Fn(usize) -> bool + Sync, params: &GroundFilterParams, cancel: &CancelFlag, progress: &Phase) -> Result<Self> {
        ensure!(u32::try_from(points.len()).is_ok(), "The point cloud has too many points to classify in one pass");
        let resolution = params.cloth_resolution;
        let Some((min, max)) = (0..points.len())
            .into_par_iter()
            .filter(|&index| include(index))
            .map(|index| (points[index].truncate(), points[index].truncate()))
            .reduce_with(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)))
        else {
            bail!("The point cloud has no points left to find the ground in");
        };
        let span = ((max - min) / resolution).ceil();
        let (cols, rows) = (span.x as usize + 1 + 2 * CLOTH_MARGIN, span.y as usize + 1 + 2 * CLOTH_MARGIN);
        if cols.saturating_mul(rows) > MAX_CLOTH_NODES {
            bail!(
                "A {resolution} m cloth over this cloud would need {} million particles; raise the cloth resolution",
                cols.saturating_mul(rows) / 1_000_000
            );
        }
        let origin = min - DVec2::splat(CLOTH_MARGIN as f64 * resolution);

        let floor = collision_heights(points, &include, origin, resolution, cols, rows);
        let floor = fill_empty_nodes(floor, cols, rows);
        progress.set_fraction(0.1);
        ensure!(!cancel.is_cancelled(), "Cancelled");

        let tolerance = params.class_threshold / 100.0;
        let mut heights = settle_membrane(&floor, cols, rows, resolution, params.terrain.curvature(), tolerance, cancel, &progress.phase(0.1, 0.95))?;
        if params.slope_recovery {
            let max_step = resolution * params.terrain.max_slope_degrees().to_radians().tan();
            recover_slopes(
                &mut heights,
                &floor,
                cols,
                rows,
                max_step,
                params.class_threshold * RECOVERY_RISE,
                params.class_threshold * RECOVERY_GAP,
                tolerance,
            );
        }
        let faces = params.slope_recovery.then(|| FaceBounds::new(&heights, &floor, cols, rows, tolerance));
        progress.set_fraction(1.0);
        Ok(Self {
            origin,
            resolution,
            cols,
            rows,
            heights,
            faces,
        })
    }

    /// Whether `point` stands on a face joining two ground levels: touched
    /// ground lies both at or below it and at or above it close by in plan,
    /// and those levels are a real step apart rather than rough ground.
    fn on_face(&self, point: DVec3, threshold: f64) -> bool {
        let Some(faces) = &self.faces else {
            return false;
        };
        let grid = ((point.truncate() - self.origin) / self.resolution).round();
        let (x, y) = (grid.x.clamp(0.0, (self.cols - 1) as f64) as usize, grid.y.clamp(0.0, (self.rows - 1) as f64) as usize);
        let (lowest, highest) = (faces.lowest[y * self.cols + x], faces.highest[y * self.cols + x]);
        highest - lowest >= FACE_MIN_STEP * threshold && point.z >= lowest - threshold && point.z <= highest + threshold
    }

    /// Cloth height under `point`, bilinear between the four particles around
    /// it, and the cloth's slope there as rise per metre along X and Y.
    fn surface_at(&self, point: DVec2) -> (f64, DVec2) {
        let grid = (point - self.origin) / self.resolution;
        let x = grid.x.clamp(0.0, (self.cols - 1) as f64);
        let y = grid.y.clamp(0.0, (self.rows - 1) as f64);
        let (x0, y0) = ((x.floor() as usize).min(self.cols.saturating_sub(2)), (y.floor() as usize).min(self.rows.saturating_sub(2)));
        let (x1, y1) = ((x0 + 1).min(self.cols - 1), (y0 + 1).min(self.rows - 1));
        let (tx, ty) = (x - x0 as f64, y - y0 as f64);
        let at = |x: usize, y: usize| self.heights[y * self.cols + x];
        let bottom = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * tx;
        let top = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * tx;
        let along_x = (at(x1, y0) - at(x0, y0)) * (1.0 - ty) + (at(x1, y1) - at(x0, y1)) * ty;
        (bottom + (top - bottom) * ty, DVec2::new(along_x, top - bottom) / self.resolution)
    }
}

/// What each particle collides with: the ground height at the particle itself,
/// or `None` where no return falls in its cell.
///
/// The lowest or nearest return is the obvious choice and is wrong on a wall:
/// a return a hand's width off the particle on a 70 degree face sits most of a
/// metre below the face at the particle, and a cloth that must pass under it
/// sags off the crest above. Instead a plane is fitted through the returns in
/// the particle's cell and the eight around it and read off at the particle,
/// clamped to the particle's own returns so a slope cannot extrapolate it
/// anywhere they do not reach.
fn collision_heights(points: &[DVec3], include: &(impl Fn(usize) -> bool + Sync), origin: DVec2, resolution: f64, cols: usize, rows: usize) -> Vec<Option<f64>> {
    let node_of = |point: DVec3| {
        let grid = ((point.truncate() - origin) / resolution).round();
        grid.y as usize * cols + grid.x as usize
    };
    // Counting sort of point indices by particle: one pass to count, a prefix
    // sum for where each particle's run starts, one pass to scatter.
    let counts: Vec<AtomicU32> = (0..cols * rows).map(|_| AtomicU32::new(0)).collect();
    (0..points.len()).into_par_iter().filter(|&index| include(index)).for_each(|index| {
        counts[node_of(points[index])].fetch_add(1, Ordering::Relaxed);
    });
    let mut starts = Vec::with_capacity(cols * rows + 1);
    let mut total = 0u32;
    starts.push(0);
    for count in &counts {
        total += count.load(Ordering::Relaxed);
        starts.push(total);
    }
    let cursors: Vec<AtomicU32> = starts[..cols * rows].iter().map(|&start| AtomicU32::new(start)).collect();
    let order: Vec<AtomicU32> = (0..total).map(|_| AtomicU32::new(0)).collect();
    (0..points.len()).into_par_iter().filter(|&index| include(index)).for_each(|index| {
        let slot = cursors[node_of(points[index])].fetch_add(1, Ordering::Relaxed);
        order[slot as usize].store(index as u32, Ordering::Relaxed);
    });
    drop(cursors);
    let mut order: Vec<u32> = order.into_iter().map(AtomicU32::into_inner).collect();
    // The scatter lands each run in whatever order threads reached it; sorting
    // the runs keeps the fits, and so the classification, reproducible.
    {
        let mut runs: Vec<&mut [u32]> = Vec::with_capacity(cols * rows);
        let mut rest = order.as_mut_slice();
        for node in 0..cols * rows {
            let (run, tail) = rest.split_at_mut((starts[node + 1] - starts[node]) as usize);
            runs.push(run);
            rest = tail;
        }
        runs.par_iter_mut().for_each(|run| run.sort_unstable());
    }
    let run = |node: usize| &order[starts[node] as usize..starts[node + 1] as usize];

    (0..cols * rows)
        .into_par_iter()
        .map(|node| {
            let own = run(node);
            let (&first, rest) = own.split_first()?;
            let (column, row) = (node % cols, node / cols);
            let centre = origin + DVec2::new(column as f64, row as f64) * resolution;
            let (lowest, highest) = rest.iter().fold((points[first as usize].z, points[first as usize].z), |(lowest, highest), &index| {
                (lowest.min(points[index as usize].z), highest.max(points[index as usize].z))
            });
            // Normal equations for z = a + b*dx + c*dy, with offsets in
            // spacings so the system stays well scaled at any resolution.
            let mut sums = [0.0f64; 9];
            for y in row.saturating_sub(1)..(row + 2).min(rows) {
                for x in column.saturating_sub(1)..(column + 2).min(cols) {
                    for &index in run(y * cols + x) {
                        let point = points[index as usize];
                        let offset = (point.truncate() - centre) / resolution;
                        let z = point.z - lowest;
                        sums[0] += 1.0;
                        sums[1] += offset.x;
                        sums[2] += offset.y;
                        sums[3] += offset.x * offset.x;
                        sums[4] += offset.x * offset.y;
                        sums[5] += offset.y * offset.y;
                        sums[6] += z;
                        sums[7] += offset.x * z;
                        sums[8] += offset.y * z;
                    }
                }
            }
            let [n, sx, sy, sxx, sxy, syy, sz, sxz, syz] = sums;
            let determinant = n * (sxx * syy - sxy * sxy) - sx * (sx * syy - sxy * sy) + sy * (sx * sxy - sxx * sy);
            let height = if n >= 3.0 && determinant.abs() > 1e-9 * n * n * n {
                // Cramer's rule for the intercept, which is the plane at the
                // particle.
                let intercept = sz * (sxx * syy - sxy * sxy) - sx * (sxz * syy - sxy * syz) + sy * (sxz * sxy - sxx * syz);
                lowest + intercept / determinant
            } else {
                sz / n + lowest
            };
            Some(height.clamp(lowest, highest))
        })
        .collect()
}

/// Give every particle with no return near it the height of the nearest
/// particle that has one, so the cloth meets holes in the cloud at the ground
/// around them rather than rising through them.
fn fill_empty_nodes(floor: Vec<Option<f64>>, cols: usize, rows: usize) -> Vec<f64> {
    let mut queue: VecDeque<usize> = floor.iter().enumerate().filter_map(|(index, height)| height.map(|_| index)).collect();
    let mut filled: Vec<f64> = floor.iter().map(|height| height.unwrap_or(f64::NAN)).collect();
    while let Some(index) = queue.pop_front() {
        let (column, row) = (index % cols, index / cols);
        let neighbours = [
            (column > 0).then(|| index - 1),
            (column + 1 < cols).then(|| index + 1),
            (row > 0).then(|| index - cols),
            (row + 1 < rows).then(|| index + cols),
        ];
        for neighbour in neighbours.into_iter().flatten() {
            if filled[neighbour].is_nan() {
                filled[neighbour] = filled[index];
                queue.push_back(neighbour);
            }
        }
    }
    filled
}

/// The shape CSF's cloth comes to rest in, solved for directly.
///
/// A cloth pressed up against the cloud settles as the highest surface that
/// stays on or under every particle's collision height and, wherever it hangs
/// free, curves no tighter than its stiffness allows - a membrane under uniform
/// pressure meeting an obstacle. Solving that rest state rather than stepping
/// the cloth there makes the result independent of how far it had to travel:
/// the floor of a 300 m deep pit and a hilltop come out the same.
///
/// Projected over-relaxation on a red-black ordering, run coarse to fine: each
/// level starts from the one below it and only has to settle local detail.
/// Every coarse particle collides with the lowest of the four it covers, so a
/// coarse cloth never passes through ground a finer one has to stay under.
#[allow(clippy::too_many_arguments)]
fn settle_membrane(floor: &[f64], cols: usize, rows: usize, resolution: f64, curvature: f64, tolerance: f64, cancel: &CancelFlag, progress: &Phase) -> Result<Vec<f64>> {
    let mut levels = vec![(floor.to_vec(), cols, rows)];
    while let Some((finer, cols, rows)) = levels.last()
        && (*cols > COARSEST_SIDE || *rows > COARSEST_SIDE)
    {
        let (coarse_cols, coarse_rows) = (cols.div_ceil(2), rows.div_ceil(2));
        let coarse = (0..coarse_cols * coarse_rows)
            .into_par_iter()
            .map(|index| {
                let (x, y) = (2 * (index % coarse_cols), 2 * (index / coarse_cols));
                let mut lowest = f64::INFINITY;
                for fy in y..(y + 2).min(*rows) {
                    for fx in x..(x + 2).min(*cols) {
                        lowest = lowest.min(finer[fy * cols + fx]);
                    }
                }
                lowest
            })
            .collect();
        levels.push((coarse, coarse_cols, coarse_rows));
    }

    let mut settled: Option<(Vec<AtomicF64>, usize)> = None;
    let total = levels.len();
    for (level, (floor, cols, rows)) in levels.iter().enumerate().rev() {
        let (cols, rows) = (*cols, *rows);
        let heights: Vec<AtomicF64> = match settled.take() {
            // Start beneath everything: the cloth only ever rises.
            None => {
                let lowest = floor.par_iter().copied().reduce(|| f64::INFINITY, f64::min);
                (0..cols * rows).map(|_| AtomicF64::new(lowest)).collect()
            }
            Some((coarse, coarse_cols)) => (0..cols * rows)
                .into_par_iter()
                .map(|index| {
                    let (x, y) = (index % cols, index / cols);
                    AtomicF64::new(coarse[(y / 2) * coarse_cols + x / 2].load().min(floor[index]))
                })
                .collect(),
        };
        let spacing = resolution * f64::from(1u32 << level);
        relax(&heights, floor, cols, rows, curvature * spacing * spacing, tolerance, cancel)?;
        settled = Some((heights, cols));
        progress.set_items((total - level) as u64, total as u64);
    }
    let (heights, _) = settled.expect("the pyramid has at least one level");
    Ok(heights.into_iter().map(AtomicF64::into_inner).collect())
}

/// Red-black projected SOR sweeps on one level until nothing moves by more
/// than `tolerance`. A free particle settles where its four neighbours' pull
/// balances the pressure - `lift` is the curvature allowance times the particle
/// spacing squared - and is never let above its collision height.
fn relax(heights: &[AtomicF64], floor: &[f64], cols: usize, rows: usize, lift: f64, tolerance: f64, cancel: &CancelFlag) -> Result<()> {
    for sweep in 0..MAX_SWEEPS {
        if sweep % 16 == 0 {
            ensure!(!cancel.is_cancelled(), "Cancelled");
        }
        let mut largest = 0.0f64;
        // Red and black particles only neighbour each other, so each half
        // reads settled values from the other and races nothing.
        for colour in 0..2 {
            let moved = (0..rows)
                .into_par_iter()
                .map(|row| {
                    let mut largest = 0.0f64;
                    for column in ((row + colour) % 2..cols).step_by(2) {
                        let index = row * cols + column;
                        let (mut sum, mut count) = (0.0, 0.0);
                        for neighbour in [
                            (column > 0).then(|| index - 1),
                            (column + 1 < cols).then(|| index + 1),
                            (row > 0).then(|| index - cols),
                            (row + 1 < rows).then(|| index + cols),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            sum += heights[neighbour].load();
                            count += 1.0;
                        }
                        let current = heights[index].load();
                        let target = (sum + lift) / count;
                        let next = (current + RELAXATION * (target - current)).min(floor[index]);
                        largest = largest.max((next - current).abs());
                        heights[index].store(next);
                    }
                    largest
                })
                .reduce(|| 0.0, f64::max);
            largest = largest.max(moved);
        }
        if largest < tolerance {
            break;
        }
    }
    Ok(())
}

/// The eight particles around a particle, with their distance in spacings.
const NEIGHBOURS: [(isize, isize, f64); 8] = [
    (-1, -1, std::f64::consts::SQRT_2),
    (0, -1, 1.0),
    (1, -1, std::f64::consts::SQRT_2),
    (-1, 0, 1.0),
    (1, 0, 1.0),
    (-1, 1, std::f64::consts::SQRT_2),
    (0, 1, 1.0),
    (1, 1, std::f64::consts::SQRT_2),
];

/// Slope post-processing, extending CSF's. The stiffness that keeps the cloth
/// out of plant also stops it turning sharply, so under a crest it sags away
/// from the top of the wall for metres. Starting from where the cloth touched
/// down, walk onto neighbours still hanging free and settle the cloth onto
/// them when their collision height continues the ground.
///
/// The walk is lopsided on purpose. Ground falls away from a crest as steeply
/// as a wall can stand, so stepping down is allowed up to `max_step` per
/// spacing. Anything standing on the ground - plant, a building, canopy - is
/// met by stepping up, so that is held to CSF's own rule of less than the
/// ground threshold. Either way the neighbour may lie no more than `max_gap`
/// above where the cloth hangs, which keeps a downhill walk off canopy that
/// happens to sit below the ground it started from.
#[allow(clippy::too_many_arguments)]
fn recover_slopes(heights: &mut [f64], floor: &[f64], cols: usize, rows: usize, max_step: f64, max_rise: f64, max_gap: f64, tolerance: f64) {
    let mut ground: Vec<bool> = heights.iter().zip(floor).map(|(height, floor)| floor - height <= tolerance).collect();
    let mut queue: VecDeque<usize> = ground.iter().enumerate().filter_map(|(index, ground)| ground.then_some(index)).collect();
    while let Some(index) = queue.pop_front() {
        let (column, row) = (index % cols, index / cols);
        for (dx, dy, distance) in NEIGHBOURS {
            let (Some(x), Some(y)) = (column.checked_add_signed(dx), row.checked_add_signed(dy)) else {
                continue;
            };
            if x >= cols || y >= rows {
                continue;
            }
            let neighbour = y * cols + x;
            if ground[neighbour] || floor[neighbour] - heights[neighbour] > max_gap {
                continue;
            }
            let rise = floor[neighbour] - floor[index];
            if rise <= max_rise * distance && -rise <= max_step * distance {
                heights[neighbour] = floor[neighbour];
                ground[neighbour] = true;
                queue.push_back(neighbour);
            }
        }
    }
}

/// The ground a face can join, per particle: the lowest and highest height
/// the cloth touched down on within [`FACE_REACH`] particles, or an empty
/// range (`+inf..-inf`) where it touched none.
///
/// A height field cannot hold a vertical face - the cloth steps from toe to
/// crest between two particles and the returns on the face lie between them,
/// nowhere near it. What sets a berm face apart from a building wall or a
/// trunk is what is on top: a crest is ground the cloth touched, a roof or a
/// canopy is not. So a return bracketed by touched ground above and below is
/// ground whatever the face's angle, overhangs included.
struct FaceBounds {
    lowest: Vec<f64>,
    highest: Vec<f64>,
}

impl FaceBounds {
    fn new(heights: &[f64], floor: &[f64], cols: usize, rows: usize, tolerance: f64) -> Self {
        let touched = |index: usize| floor[index] - heights[index] <= tolerance;
        let lowest: Vec<f64> = (0..cols * rows).map(|index| if touched(index) { floor[index] } else { f64::INFINITY }).collect();
        let highest: Vec<f64> = (0..cols * rows).map(|index| if touched(index) { floor[index] } else { f64::NEG_INFINITY }).collect();
        Self {
            lowest: window_extreme(&lowest, cols, rows, f64::min),
            highest: window_extreme(&highest, cols, rows, f64::max),
        }
    }
}

/// Running `pick` (min or max) over the square of [`FACE_REACH`] particles
/// around each particle, as a row pass then a column pass.
fn window_extreme(values: &[f64], cols: usize, rows: usize, pick: fn(f64, f64) -> f64) -> Vec<f64> {
    let across: Vec<f64> = (0..cols * rows)
        .into_par_iter()
        .map(|index| {
            let (column, row) = (index % cols, index / cols);
            (column.saturating_sub(FACE_REACH)..(column + FACE_REACH + 1).min(cols))
                .map(|x| values[row * cols + x])
                .reduce(pick)
                .expect("the window holds the particle itself")
        })
        .collect();
    (0..cols * rows)
        .into_par_iter()
        .map(|index| {
            let (column, row) = (index % cols, index / cols);
            (row.saturating_sub(FACE_REACH)..(row + FACE_REACH + 1).min(rows))
                .map(|y| across[y * cols + column])
                .reduce(pick)
                .expect("the window holds the particle itself")
        })
        .collect()
}

/// A particle height shared across the red-black passes. Relaxed loads and
/// stores compile to plain moves; the ordering is what keeps them apart.
struct AtomicF64(AtomicU64);

impl AtomicF64 {
    fn new(value: f64) -> Self {
        Self(AtomicU64::new(value.to_bits()))
    }

    fn load(&self) -> f64 {
        f64::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn store(&self, value: f64) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }

    fn into_inner(self) -> f64 {
        f64::from_bits(self.0.into_inner())
    }
}
