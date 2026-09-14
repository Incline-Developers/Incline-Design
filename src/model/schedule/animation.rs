//! Stateless sampling of a published dispatch schedule for the Animate view.
//!
//! The dispatch output has one segment per loader.  This index folds those
//! contributions into one rate curve per physical dig block once, so moving
//! the time cursor never scans every bar and never counts a shared rate twice.

use std::collections::{BTreeMap, HashMap};

use crate::model::{
    DigBlockId,
    schedule::{DispatchSchedule, dispatch::ExecutionSegment},
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
    pub(crate) fn build(schedule: &DispatchSchedule) -> Result<Self, AnimationError> {
        // Grouped in one pass rather than filtered per balance: the execution
        // log holds every loader's every interval, and walking all of it once
        // for each block in it is the whole run squared.
        let mut worked: HashMap<DigBlockId, Vec<&ExecutionSegment>> = HashMap::new();
        for segment in &schedule.execution {
            worked.entry(segment.resolved).or_default().push(segment);
        }
        let mut blocks = HashMap::new();
        for balance in &schedule.balances {
            if !balance.started_t.is_finite() || balance.started_t < 0.0 || !balance.remaining_t.is_finite() || balance.remaining_t < 0.0 {
                return Err(AnimationError::InvalidBalance(balance.resolved));
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
            for segment in worked.get(&balance.resolved).into_iter().flatten().copied() {
                let duration = segment.end_h - segment.start_h;
                if !segment.start_h.is_finite() || !segment.end_h.is_finite() || !segment.tonnes.is_finite() || segment.tonnes < 0.0 || duration < 0.0 {
                    return Err(AnimationError::InvalidSegment(balance.resolved));
                }
                // A segment that occupies no time and moves nothing is an
                // instant the dispatcher happened to record, not a defect:
                // it has no rate, contributes nothing, and skipping it says
                // exactly what it says. Tonnes over no time is the defect.
                if duration == 0.0 {
                    if segment.tonnes > 0.0 {
                        return Err(AnimationError::InvalidSegment(balance.resolved));
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
                    block: balance.resolved,
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
                balance.resolved,
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
