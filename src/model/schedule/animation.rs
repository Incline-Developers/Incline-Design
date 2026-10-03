//! Stateless sampling of a calculated schedule for the Animate view.
//!
//! The result has one execution span per loader. This index folds the *dig*
//! spans into one rate curve per physical dig block once, so moving the time
//! cursor never scans every bar and never counts a shared rate twice. Reclaim
//! spans take material out of a stockpile, not out of the pit, so they never
//! enter a ground curve - the tonnes they move were already depleted from the
//! ground when they were dug.

use std::collections::{BTreeMap, HashMap};

use crate::model::{
    DigBlockId,
    schedule::result::{CalculatedSchedule, Execution, WorkSource},
};

const TONNE_TOLERANCE: f64 = 1.0e-7;

#[derive(Clone, Debug)]
struct Interval {
    start_h: f64,
    end_h: f64,
    mined_before_t: f64,
    rate_tph: f64,
}

#[derive(Clone, Debug)]
struct BlockCurve {
    started_t: f64,
    /// What the ledger says has gone by the end of the run.
    ///
    /// The intervals reconstruct the same number by summing rate x duration,
    /// and a block dug across a dozen of them accumulates tens of ulps doing
    /// it - enough to land short of whole. The ledger is the only thing
    /// entitled to say a block is empty, and a block the ledger has emptied
    /// has to read as emptied exactly, or the view keeps a fragment of ground
    /// that is no longer there.
    final_fraction: f64,
    intervals: Vec<Interval>,
}

/// A validation failure in calculated output.  These are surfaced instead of
/// being hidden by a clamp, because animation must not make invalid reserves
/// look plausible.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnimationError {
    InvalidBalance(DigBlockId),
    InvalidSegment(DigBlockId),
    Conservation { block: DigBlockId, expected_t: f64, actual_t: f64 },
}

impl std::fmt::Display for AnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBalance(block) => write!(f, "block {} has an invalid starting balance", block.0),
            Self::InvalidSegment(block) => write!(f, "block {} has an invalid execution segment", block.0),
            Self::Conservation { block, expected_t, actual_t } => write!(
                f,
                "block {} execution does not conserve tonnes ({actual_t:.6} t recorded, {expected_t:.6} t expected)",
                block.0
            ),
        }
    }
}

/// Immutable per-calculation depletion curves.
#[derive(Clone, Debug, Default)]
pub(crate) struct AnimationIndex {
    blocks: HashMap<DigBlockId, BlockCurve>,
}

impl AnimationIndex {
    pub(crate) fn build(schedule: &CalculatedSchedule) -> Result<Self, AnimationError> {
        // Grouped in one pass rather than filtered per balance: the execution
        // log holds every loader's every span, and walking all of it once for
        // each block in it is the whole run squared.
        let mut worked: HashMap<DigBlockId, Vec<&Execution>> = HashMap::new();
        for execution in &schedule.executions {
            if let WorkSource::Block(block) = execution.source {
                worked.entry(block).or_default().push(execution);
            }
        }
        let mut blocks = HashMap::new();
        for balance in &schedule.ground {
            if !balance.started_t.is_finite() || balance.started_t < 0.0 || !balance.remaining_t.is_finite() || balance.remaining_t < 0.0 {
                return Err(AnimationError::InvalidBalance(balance.block));
            }
            // A zero-tonne block deliberately has no curve.  Its geometry is
            // retained because 0/0 has no useful visual interpretation.
            if balance.started_t == 0.0 {
                continue;
            }

            // Delta-rate events create intervals over which all loader
            // contributions on this shared block have one constant sum.
            let mut events: BTreeMap<u64, f64> = BTreeMap::new();
            let mut segment_total = 0.0;
            for segment in worked.get(&balance.block).into_iter().flatten().copied() {
                let duration = segment.end_h - segment.start_h;
                if !segment.start_h.is_finite() || !segment.end_h.is_finite() || !segment.tonnes.is_finite() || segment.tonnes < 0.0 || duration < 0.0 {
                    return Err(AnimationError::InvalidSegment(balance.block));
                }
                // A segment that occupies no time and moves nothing is an
                // instant the dispatcher happened to record, not a defect:
                // it has no rate, contributes nothing, and skipping it says
                // exactly what it says. Tonnes over no time is the defect.
                if duration == 0.0 {
                    if segment.tonnes > 0.0 {
                        return Err(AnimationError::InvalidSegment(balance.block));
                    }
                    continue;
                }
                let rate = segment.tonnes / duration;
                *events.entry(segment.start_h.to_bits()).or_default() += rate;
                *events.entry(segment.end_h.to_bits()).or_default() -= rate;
                segment_total += segment.tonnes;
            }

            let expected = balance.started_t - balance.remaining_t;
            let tolerance = TONNE_TOLERANCE * balance.started_t.max(1.0);
            if (segment_total - expected).abs() > tolerance {
                return Err(AnimationError::Conservation {
                    block: balance.block,
                    expected_t: expected,
                    actual_t: segment_total,
                });
            }

            let mut points: Vec<(f64, f64)> = events.into_iter().map(|(time, delta)| (f64::from_bits(time), delta)).collect();
            points.sort_by(|left, right| left.0.total_cmp(&right.0));
            let mut intervals = Vec::new();
            let mut rate = 0.0;
            let mut mined = 0.0;
            for pair in points.windows(2) {
                rate += pair[0].1;
                let (start_h, end_h) = (pair[0].0, pair[1].0);
                if end_h > start_h && rate > 0.0 {
                    intervals.push(Interval {
                        start_h,
                        end_h,
                        mined_before_t: mined,
                        rate_tph: rate,
                    });
                    mined += rate * (end_h - start_h);
                }
            }
            blocks.insert(
                balance.block,
                BlockCurve {
                    started_t: balance.started_t,
                    final_fraction: if balance.remaining_t <= 0.0 {
                        1.0
                    } else {
                        (expected / balance.started_t).clamp(0.0, 1.0)
                    },
                    intervals,
                },
            );
        }
        Ok(Self { blocks })
    }

    /// Fraction removed at elapsed project hours `time_h`.
    pub(crate) fn depleted_fraction(&self, block: DigBlockId, time_h: f64) -> f64 {
        let Some(curve) = self.blocks.get(&block) else { return 0.0 };
        let at = time_h.max(0.0);
        let index = curve.intervals.partition_point(|interval| interval.end_h <= at);
        let Some(interval) = curve.intervals.get(index) else {
            // Past the last interval nothing more is dug, and what the run
            // took out is on the ledger rather than in the arithmetic.
            return curve.final_fraction;
        };
        let mined = if at > interval.start_h {
            interval.mined_before_t + interval.rate_tph * (at.min(interval.end_h) - interval.start_h)
        } else {
            interval.mined_before_t
        };
        // Validation above rejects substantive disagreement.  This clamp is
        // only the final floating-point guard.
        (mined / curve.started_t).clamp(0.0, 1.0)
    }
}

/// What Animate needs to draw one of the run's drill and blast blasts, by
/// its position in the run's blasts.
#[derive(Clone, Debug)]
pub(crate) struct AnimatedBlast {
    /// The ground it stands on, which its marks are draped over; `None`
    /// when none of its blocks were found.
    pub(crate) top: Option<std::sync::Arc<BlastTop>>,
}

/// The top of the ground inside a blast, sampled on a grid in plan: the
/// highest upward-facing triangle of the blocks it holds at each node.
#[derive(Clone, Debug)]
pub(crate) struct BlastTop {
    origin: glam::DVec2,
    cell: f64,
    columns: usize,
    rows: usize,
    /// Row-major heights; NaN where no ground was found.
    z: Vec<f64>,
}

impl BlastTop {
    /// Rasterizes the meshes' upward-facing triangles over the plan box
    /// `min`..`max`. A ring of nodes just outside the ground borrows its
    /// neighbours' heights, so a mark on the blast's edge still finds it.
    pub(crate) fn rasterize<'a>(min: glam::DVec2, max: glam::DVec2, meshes: impl IntoIterator<Item = &'a crate::model::formats::mesh_data::Triangulation>) -> Option<Self> {
        use glam::DVec2;
        let extent = max - min;
        if !extent.is_finite() || extent.min_element() < 0.0 {
            return None;
        }
        let cell = (extent.max_element() / 48.0).clamp(0.5, 4.0);
        let origin = min - DVec2::splat(cell);
        let columns = (extent.x / cell).ceil() as usize + 3;
        let rows = (extent.y / cell).ceil() as usize + 3;
        let mut z = vec![f64::NAN; columns * rows];
        let node = |column: usize, row: usize| origin + DVec2::new(column as f64, row as f64) * cell;
        for mesh in meshes {
            for triangle in mesh.triangles() {
                let [a, b, c] = triangle.vertices.map(|vertex| glam::DVec3::new(vertex.x, vertex.y, vertex.z));
                let normal = (b - a).cross(c - a);
                // Walls and undersides: only the ground's top carries marks.
                if normal.z <= 0.05 * normal.length() {
                    continue;
                }
                let area = normal.z;
                let low = ((a.truncate().min(b.truncate()).min(c.truncate()) - origin) / cell).ceil().max(DVec2::ZERO);
                let high = ((a.truncate().max(b.truncate()).max(c.truncate()) - origin) / cell).floor();
                if high.x < 0.0 || high.y < 0.0 {
                    continue;
                }
                let (low, high) = (low.as_uvec2(), high.min(DVec2::new((columns - 1) as f64, (rows - 1) as f64)).as_uvec2());
                for row in low.y..=high.y {
                    for column in low.x..=high.x {
                        let p = node(column as usize, row as usize);
                        let edge = |from: glam::DVec3, to: glam::DVec3| (to.x - from.x) * (p.y - from.y) - (to.y - from.y) * (p.x - from.x);
                        let (wa, wb, wc) = (edge(b, c) / area, edge(c, a) / area, edge(a, b) / area);
                        if wa < -1e-9 || wb < -1e-9 || wc < -1e-9 {
                            continue;
                        }
                        let height = wa * a.z + wb * b.z + wc * c.z;
                        let held = &mut z[row as usize * columns + column as usize];
                        *held = if held.is_nan() { height } else { held.max(height) };
                    }
                }
            }
        }
        if z.iter().all(|height| height.is_nan()) {
            return None;
        }
        let filled = z.clone();
        for row in 0..rows {
            for column in 0..columns {
                if !filled[row * columns + column].is_nan() {
                    continue;
                }
                let (mut sum, mut count) = (0.0, 0);
                for (dr, dc) in [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
                    let (r, c) = (row as isize + dr, column as isize + dc);
                    if r < 0 || c < 0 || r as usize >= rows || c as usize >= columns {
                        continue;
                    }
                    let height = filled[r as usize * columns + c as usize];
                    if !height.is_nan() {
                        sum += height;
                        count += 1;
                    }
                }
                if count > 0 {
                    z[row * columns + column] = sum / f64::from(count);
                }
            }
        }
        Some(Self { origin, cell, columns, rows, z })
    }

    /// Spacing of the samples: marks are drawn in steps no longer than this,
    /// so they follow the ground between them.
    pub(crate) fn cell(&self) -> f64 {
        self.cell
    }

    /// The ground's height at `point`, from the samples around it; `None`
    /// off the ground.
    pub(crate) fn height(&self, point: glam::DVec2) -> Option<f64> {
        let at = (point - self.origin) / self.cell;
        if !at.is_finite() || at.x < 0.0 || at.y < 0.0 {
            return None;
        }
        let (column, row) = (at.x.floor() as usize, at.y.floor() as usize);
        let (fx, fy) = (at.x - column as f64, at.y - row as f64);
        let (mut sum, mut weight) = (0.0, 0.0);
        for (dc, dr, w) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
            let (c, r) = (column + dc, row + dr);
            if c >= self.columns || r >= self.rows {
                continue;
            }
            let height = self.z[r * self.columns + c];
            if !height.is_nan() && w > 0.0 {
                sum += height * w;
                weight += w;
            }
        }
        (weight > 1e-9).then(|| sum / weight)
    }
}
