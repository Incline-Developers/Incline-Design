//! Downhole geophysics traces, stored compactly and keyed by the drill-hole
//! dataset and hole id they were matched to.

use std::{
    collections::{BTreeMap, HashMap},
    ops::Range,
};

use crate::model::drill_hole::DrillHoleId;

/// The curve kinds Incline keeps from downhole geophysics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum LogKind {
    Gamma,
    LongDensity,
    ShortDensity,
}

impl LogKind {
    pub(crate) const ALL: [LogKind; 3] = [LogKind::Gamma, LogKind::LongDensity, LogKind::ShortDensity];

    /// Dense index for array-backed per-kind storage.
    pub(crate) fn index(self) -> usize {
        match self {
            LogKind::Gamma => 0,
            LogKind::LongDensity => 1,
            LogKind::ShortDensity => 2,
        }
    }

    /// Quantisation step for this curve, in its native unit.
    pub(crate) fn resolution(self) -> f64 {
        match self {
            LogKind::Gamma => 0.1,
            LogKind::LongDensity | LogKind::ShortDensity => 0.001,
        }
    }

    /// Largest reading a trace of this curve can hold.
    pub(crate) fn max_value(self) -> f64 {
        f64::from(NULL_RAW - 1) * self.resolution()
    }

    pub(crate) fn unit(self) -> &'static str {
        match self {
            LogKind::Gamma => "API",
            LogKind::LongDensity | LogKind::ShortDensity => "g/cc",
        }
    }
}

/// Sentinel raw value meaning "no reading" (NaN input, or an
/// out-of-range/missing sample).
const NULL_RAW: u16 = u16::MAX;

/// Quantise one value to the curve's fixed-point encoding, tracking clamp/null
/// events.
fn quantize(kind: LogKind, v: f64, clamped: &mut usize, nulls: &mut usize) -> u16 {
    if !v.is_finite() {
        *nulls += 1;
        return NULL_RAW;
    }
    let resolution = kind.resolution();
    let max_value = kind.max_value();
    let bounded = if v < 0.0 {
        *clamped += 1;
        0.0
    } else if v > max_value {
        *clamped += 1;
        max_value
    } else {
        v
    };
    ((bounded / resolution).round() as u32).min(NULL_RAW as u32 - 1) as u16
}

/// Decode one raw sample back to its physical value, or `None` for the null
/// sentinel.
fn decode(kind: LogKind, raw: u16) -> Option<f32> {
    if raw == NULL_RAW { None } else { Some((raw as f64 * kind.resolution()) as f32) }
}

/// Min/max (ignoring nulls) over a slice of base-level raw samples; the null
/// sentinel pair if all null.
fn combine_values(vals: &[u16]) -> (u16, u16) {
    let mut min = None;
    let mut max = None;
    for &v in vals {
        if v == NULL_RAW {
            continue;
        }
        min = Some(min.map_or(v, |m: u16| m.min(v)));
        max = Some(max.map_or(v, |m: u16| m.max(v)));
    }
    match (min, max) {
        (Some(mn), Some(mx)) => (mn, mx),
        _ => (NULL_RAW, NULL_RAW),
    }
}

/// Min/max (ignoring null-sentinel blocks) over a slice of pyramid entries from
/// the level below.
fn combine_pairs(level: &[(u16, u16)]) -> Vec<(u16, u16)> {
    level
        .chunks(2)
        .map(|chunk| {
            let mut min = None;
            let mut max = None;
            for &(mn, mx) in chunk {
                if mn == NULL_RAW && mx == NULL_RAW {
                    continue;
                }
                min = Some(min.map_or(mn, |m: u16| m.min(mn)));
                max = Some(max.map_or(mx, |m: u16| m.max(mx)));
            }
            match (min, max) {
                (Some(mn), Some(mx)) => (mn, mx),
                _ => (NULL_RAW, NULL_RAW),
            }
        })
        .collect()
}

/// Nearest-rank percentiles used for [`LogTrace::robust_range`]: wide enough
/// to keep the bulk of the curve, narrow enough to drop spikes at either end.
const ROBUST_LOW_PERCENTILE: f64 = 0.01;
const ROBUST_HIGH_PERCENTILE: f64 = 0.99;

/// Nearest-rank low/high percentile raw values over `raws` (consumed and
/// dropped here), or `None` if empty. Raw u16 order equals value order since
/// quantisation is monotonic, so this needs no decoding. O(n) via two
/// `select_nth_unstable` partitions rather than a full sort.
fn robust_percentiles(mut raws: Vec<u16>) -> Option<(u16, u16)> {
    let n = raws.len();
    if n == 0 {
        return None;
    }
    let low_idx = (((n - 1) as f64) * ROBUST_LOW_PERCENTILE).round() as usize;
    let high_idx = (((n - 1) as f64) * ROBUST_HIGH_PERCENTILE).round() as usize;
    let (_, &mut low, _) = raws.select_nth_unstable(low_idx);
    // Everything from low_idx on is now at least `low`, so the high
    // percentile lies in that right partition, its index shifted by low_idx.
    let (_, &mut high, _) = raws[low_idx..].select_nth_unstable(high_idx - low_idx);
    Some((low, high))
}

/// The lowest pyramid level kept in memory. Levels below it cover at most
/// `2^(FIRST_STORED_LEVEL - 1)` samples per node and are read straight from the
/// base values instead, which cuts the pyramid from twice the base array's
/// size to half of it.
const FIRST_STORED_LEVEL: usize = 3;

/// Build the min/max pyramid bottom-up from base samples. `pyramid[i]` holds
/// level `i + FIRST_STORED_LEVEL` (blocks of `2^(i + FIRST_STORED_LEVEL)` base
/// samples). Stops once a level has a single entry.
fn build_pyramid(values: &[u16]) -> Vec<Vec<(u16, u16)>> {
    let mut pyramid = Vec::new();
    if values.len() < 2 {
        return pyramid;
    }
    let mut level: Vec<(u16, u16)> = values.chunks(1 << FIRST_STORED_LEVEL).map(combine_values).collect();
    loop {
        let is_last = level.len() <= 1;
        pyramid.push(level.clone());
        if is_last {
            break;
        }
        level = combine_pairs(&level);
    }
    pyramid
}

/// Stats returned alongside a freshly built [`LogTrace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TraceStats {
    pub(crate) clamped: usize,
    pub(crate) resampled: bool,
    pub(crate) nulls: usize,
    /// Resampled depths left null because no reading lay within half a step.
    pub(crate) gaps: usize,
    /// Readings left out as isolated, far from the rest of the curve.
    pub(crate) isolated: usize,
}

/// Why [`LogTrace::from_samples`] built nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraceError {
    /// No sample with a finite depth, or every value null.
    NoSamples,
    /// The median depth step is not finite, or finer than
    /// [`MIN_TRACE_STEP`].
    BadStep,
    /// Resampling to the median step would need more than
    /// [`MAX_TRACE_SAMPLES`] samples.
    SpanTooLong { rows: usize, requested: u64 },
}

/// Most samples one trace may hold. The cap is on the trace, not scaled to
/// the rows read, so a continuation across a long unlogged gap is kept.
const MAX_TRACE_SAMPLES: usize = 20_000_000;

/// Whether a trace of `samples` samples is within [`MAX_TRACE_SAMPLES`].
fn fits_cap(samples: u64) -> bool {
    samples <= MAX_TRACE_SAMPLES as u64
}

/// Finest depth step, in metres, a trace is built at.
const MIN_TRACE_STEP: f64 = 0.001;

/// Readings further apart than this, in metres, are in separate groups.
const ISOLATION_GAP: f64 = 50.0;

/// Fewest readings a separate group needs to be kept.
const MIN_GROUP_READINGS: usize = 10;

/// Drop from `rows`, sorted by depth, each group of fewer than
/// [`MIN_GROUP_READINGS`] readings lying more than [`ISOLATION_GAP`] from
/// the rest of the curve, nulls within its span included: a stray depth, not
/// a continuation. Nothing is dropped unless some group is large enough to
/// be the curve. Returns the readings dropped.
fn drop_isolated_readings(rows: &mut Vec<(f64, f64)>) -> usize {
    // First depth, last depth and readings of each group.
    let mut groups: Vec<(f64, f64, usize)> = Vec::new();
    for &(depth, _) in rows.iter().filter(|(_, value)| value.is_finite()) {
        match groups.last_mut() {
            Some(group) if depth - group.1 <= ISOLATION_GAP => {
                group.1 = depth;
                group.2 += 1;
            }
            _ => groups.push((depth, depth, 1)),
        }
    }
    if !groups.iter().any(|group| group.2 >= MIN_GROUP_READINGS) {
        return 0;
    }
    groups.retain(|group| group.2 < MIN_GROUP_READINGS);
    let isolated = groups.iter().map(|group| group.2).sum();
    if isolated > 0 {
        let mut next = groups.iter().peekable();
        rows.retain(|(depth, _)| {
            while next.peek().is_some_and(|group| group.1 < *depth) {
                next.next();
            }
            !next.peek().is_some_and(|group| group.0 <= *depth)
        });
    }
    isolated
}

/// One downhole curve: evenly spaced quantised samples plus a min/max pyramid
/// for fast zoomed-out queries.
#[derive(Clone)]
pub(crate) struct LogTrace {
    kind: LogKind,
    start: f64,
    step: f64,
    values: Vec<u16>,
    pyramid: Vec<Vec<(u16, u16)>>,
    /// Nearest-rank 1st/99th percentile raw values over the non-null
    /// samples, precomputed at import. `None` if there was no valid sample.
    robust: Option<(u16, u16)>,
    /// The curve mnemonic this trace was read from, e.g. `"GAMM"`. Empty
    /// unless set with [`LogTrace::with_source`].
    source: String,
}

impl LogTrace {
    /// Build a trace from raw (depth, value) pairs, in any depth order. A
    /// column not evenly stepped is resampled to its median step, each depth
    /// taking the nearest reading within half a step and staying null where
    /// there is none, so gaps are never filled with invented values. A few
    /// readings far from the rest of the curve are left out first (see
    /// [`drop_isolated_readings`]). A median step finer than
    /// [`MIN_TRACE_STEP`], or a trace past [`MAX_TRACE_SAMPLES`] samples, is
    /// refused.
    pub(crate) fn from_samples(kind: LogKind, depths: &[f64], values: &[f64]) -> Result<(LogTrace, TraceStats), TraceError> {
        let mut rows: Vec<(f64, f64)> = depths
            .iter()
            .zip(values)
            .filter(|(depth, _)| depth.is_finite())
            .map(|(&depth, &value)| (depth, value))
            .collect();
        if rows.iter().all(|(_, value)| !value.is_finite()) {
            return Err(TraceError::NoSamples);
        }
        if rows[0].0 > rows[rows.len() - 1].0 {
            rows.reverse();
        }
        // Out of order even after that: sort, and resample below.
        let mut resampled = rows.windows(2).any(|pair| pair[1].0 < pair[0].0);
        if resampled {
            rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        }

        let isolated = drop_isolated_readings(&mut rows);

        let mut clamped = 0usize;
        let mut nulls = 0usize;
        let mut gaps = 0usize;
        let (start, step, raw_values) = if rows.len() == 1 {
            (rows[0].0, 1.0, vec![quantize(kind, rows[0].1, &mut clamped, &mut nulls)])
        } else {
            let mut diffs: Vec<f64> = rows.windows(2).map(|pair| pair[1].0 - pair[0].0).collect();
            let (even, middle) = (diffs.len().is_multiple_of(2), diffs.len() / 2);
            let (lower, upper, _) = diffs.select_nth_unstable_by(middle, f64::total_cmp);
            let median = if even {
                (lower.iter().copied().fold(f64::NEG_INFINITY, f64::max) + *upper) / 2.0
            } else {
                *upper
            };
            if !median.is_finite() || median < MIN_TRACE_STEP {
                return Err(TraceError::BadStep);
            }
            let tolerance = (1e-6_f64).max(1e-3 * median);
            let uniform = !resampled && rows.windows(2).all(|pair| (pair[1].0 - pair[0].0 - median).abs() <= tolerance);
            if uniform {
                if !fits_cap(rows.len() as u64) {
                    return Err(TraceError::SpanTooLong {
                        rows: rows.len(),
                        requested: rows.len() as u64,
                    });
                }
                (rows[0].0, median, rows.iter().map(|&(_, value)| quantize(kind, value, &mut clamped, &mut nulls)).collect())
            } else {
                let first = rows[0].0;
                let span = (rows[rows.len() - 1].0 - first) / median;
                // `span + 1` samples: refused past the absolute cap, or when
                // not finite.
                if !span.is_finite() || !fits_cap((span.floor() + 1.0) as u64) {
                    return Err(TraceError::SpanTooLong {
                        rows: rows.len(),
                        requested: (span.floor() + 1.0) as u64,
                    });
                }
                // A hair of slack so rounding in the step cannot drop the last
                // depth.
                let count = (span + 1e-6).floor() as usize + 1;
                let reach = 0.5 * median * (1.0 + 1e-9);
                let mut raw = Vec::with_capacity(count);
                let mut nearest = 0usize;
                for j in 0..count {
                    let target = first + j as f64 * median;
                    while nearest + 1 < rows.len() && (rows[nearest + 1].0 - target).abs() <= (rows[nearest].0 - target).abs() {
                        nearest += 1;
                    }
                    if (rows[nearest].0 - target).abs() > reach {
                        gaps += 1;
                        raw.push(NULL_RAW);
                    } else {
                        raw.push(quantize(kind, rows[nearest].1, &mut clamped, &mut nulls));
                    }
                }
                resampled = true;
                (first, median, raw)
            }
        };

        let first_non_null = raw_values.iter().position(|&v| v != NULL_RAW);
        let last_non_null = raw_values.iter().rposition(|&v| v != NULL_RAW);
        let (start, values_trimmed) = match (first_non_null, last_non_null) {
            (Some(f), Some(l)) => (start + f as f64 * step, raw_values[f..=l].to_vec()),
            _ => (start, raw_values),
        };

        let robust_scratch: Vec<u16> = values_trimmed.iter().copied().filter(|&v| v != NULL_RAW).collect();
        let robust = robust_percentiles(robust_scratch);
        let pyramid = build_pyramid(&values_trimmed);
        let trace = LogTrace {
            kind,
            start,
            step,
            values: values_trimmed,
            pyramid,
            robust,
            source: String::new(),
        };
        Ok((
            trace,
            TraceStats {
                clamped,
                resampled,
                nulls,
                gaps,
                isolated,
            },
        ))
    }

    pub(crate) fn kind(&self) -> LogKind {
        self.kind
    }

    pub(crate) fn start_depth(&self) -> f64 {
        self.start
    }

    pub(crate) fn step(&self) -> f64 {
        self.step
    }

    /// Depth of the last sample.
    pub(crate) fn end_depth(&self) -> f64 {
        self.start + self.values.len().saturating_sub(1) as f64 * self.step
    }

    /// Samples on the trace's grid, nulls included.
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }

    pub(crate) fn sample(&self, i: usize) -> Option<f32> {
        self.values.get(i).and_then(|&raw| decode(self.kind, raw))
    }

    /// Base-sample index range `[from, to)` covering depths in the closed
    /// window `[from, to]`, clipped to the trace.
    fn index_range_closed(&self, from: f64, to: f64) -> (usize, usize) {
        if self.values.is_empty() || !from.is_finite() || !to.is_finite() || to < from || self.step <= 0.0 {
            return (0, 0);
        }
        let end = self.end_depth();
        if to < self.start || from > end {
            return (0, 0);
        }
        let idx_from = if from <= self.start { 0 } else { ((from - self.start) / self.step).ceil() as isize };
        let idx_to_excl = if to >= end {
            self.values.len() as isize
        } else {
            ((to - self.start) / self.step).floor() as isize + 1
        };
        let idx_from = idx_from.clamp(0, self.values.len() as isize) as usize;
        let idx_to_excl = idx_to_excl.clamp(0, self.values.len() as isize) as usize;
        if idx_from >= idx_to_excl { (0, 0) } else { (idx_from, idx_to_excl) }
    }

    /// Base-sample index range `[from, to)` covering depths in the half-open
    /// window `[from, to)`, clipped to the trace.
    fn index_range_half_open(&self, from: f64, to: f64) -> (usize, usize) {
        if self.values.is_empty() || self.step <= 0.0 {
            return (0, 0);
        }
        let end = self.end_depth();
        let clipped_from = from.max(self.start);
        let clipped_to = to.min(end + self.step);
        if clipped_to <= clipped_from {
            return (0, 0);
        }
        let idx_from = ((clipped_from - self.start) / self.step).ceil().max(0.0) as usize;
        let idx_to_excl = ((clipped_to - self.start) / self.step).ceil().max(0.0) as usize;
        let idx_from = idx_from.min(self.values.len());
        let idx_to_excl = idx_to_excl.min(self.values.len());
        if idx_from >= idx_to_excl { (0, 0) } else { (idx_from, idx_to_excl) }
    }

    /// Exact decoded samples in the closed depth window, clipped to the trace.
    pub(crate) fn samples(&self, depth_from: f64, depth_to: f64) -> impl Iterator<Item = (f64, Option<f32>)> + '_ {
        let (from, to) = if depth_to < depth_from { (depth_to, depth_from) } else { (depth_from, depth_to) };
        let (idx_from, idx_to_excl) = self.index_range_closed(from, to);
        (idx_from..idx_to_excl).map(move |i| (self.start + i as f64 * self.step, self.sample(i)))
    }

    /// Min/max over one bucket `[bucket_from, bucket_to)`, exact: the bucket's
    /// base-sample index range is decomposed into O(log n) disjoint pyramid
    /// nodes (dyadic/segment-tree range decomposition, see `range_min_max`) and
    /// combined, so the result never bleeds samples from outside the bucket.
    fn bucket_value(&self, bucket_from: f64, bucket_to: f64) -> Option<(f32, f32)> {
        if self.values.is_empty() {
            return None;
        }
        let end = self.end_depth();
        if bucket_to <= self.start || bucket_from > end {
            return None;
        }
        let (idx_from, idx_to_excl) = self.index_range_half_open(bucket_from, bucket_to);
        if idx_from >= idx_to_excl {
            // Narrower than a sample, or falls in a gap between samples: use
            // the nearest sample to the bucket centre.
            let centre = (bucket_from + bucket_to) * 0.5;
            let nearest = ((centre - self.start) / self.step).round();
            let nearest = (nearest.max(0.0) as usize).min(self.values.len() - 1);
            return decode(self.kind, self.values[nearest]).map(|v| (v, v));
        }
        let (mn, mx) = self.range_min_max(idx_from, idx_to_excl);
        match (decode(self.kind, mn), decode(self.kind, mx)) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }

    /// Exact min/max (ignoring nulls) over base samples `[i0, i1)`.
    /// Segment-tree walk: at each level an odd left end takes its node and
    /// steps right, an odd right end steps left and takes its node, then both
    /// halve. The range is covered by O(log n) disjoint nodes, never spilling
    /// past either end.
    fn range_min_max(&self, i0: usize, i1: usize) -> (u16, u16) {
        let mut min: Option<u16> = None;
        let mut max: Option<u16> = None;
        let mut take = |mn: u16, mx: u16| {
            if mn == NULL_RAW {
                return;
            }
            min = Some(min.map_or(mn, |m: u16| m.min(mn)));
            max = Some(max.map_or(mx, |m: u16| m.max(mx)));
        };
        let mut l = i0;
        let mut r = i1;
        let mut level = 0usize;
        // Node `index` at `level`: stored levels come from the pyramid, the
        // ones below it are recombined from the (at most four) base samples.
        let node = |level: usize, index: usize| {
            if level >= FIRST_STORED_LEVEL {
                self.pyramid[level - FIRST_STORED_LEVEL][index]
            } else {
                let from = index << level;
                combine_values(&self.values[from..(from + (1 << level)).min(self.values.len())])
            }
        };
        while l < r {
            if level >= FIRST_STORED_LEVEL && level - FIRST_STORED_LEVEL >= self.pyramid.len() {
                break;
            }
            if l % 2 == 1 {
                let (mn, mx) = node(level, l);
                take(mn, mx);
                l += 1;
            }
            if r % 2 == 1 {
                r -= 1;
                let (mn, mx) = node(level, r);
                take(mn, mx);
            }
            l /= 2;
            r /= 2;
            level += 1;
        }
        match (min, max) {
            (Some(mn), Some(mx)) => (mn, mx),
            _ => (NULL_RAW, NULL_RAW),
        }
    }

    /// Yields exactly `buckets` items, one min/max (or `None`) per bucket
    /// covering `[depth_from, depth_to)` split evenly. Each bucket's min/max is
    /// exact - the dyadic pyramid decomposition in `range_min_max` combines
    /// precisely the base samples whose depth falls in the bucket's half-open
    /// window, never samples from outside it. `depth_to < depth_from` is
    /// normalised by swapping; `buckets == 0` or non-finite depths yield an
    /// empty iterator. Allocation-free beyond the returned iterator itself.
    pub(crate) fn envelope(&self, depth_from: f64, depth_to: f64, buckets: usize) -> Envelope<'_> {
        if buckets == 0 || !depth_from.is_finite() || !depth_to.is_finite() {
            return Envelope {
                trace: self,
                remaining: 0..0,
                from: 0.0,
                w: 0.0,
            };
        }
        let (from, to) = if depth_to < depth_from { (depth_to, depth_from) } else { (depth_from, depth_to) };
        let w = (to - from) / buckets as f64;
        Envelope {
            trace: self,
            remaining: 0..buckets,
            from,
            w,
        }
    }

    /// Nearest-rank 1st/99th percentile decoded values, excluding spikes at
    /// either end; `None` if the trace has no valid sample.
    pub(crate) fn robust_range(&self) -> Option<(f32, f32)> {
        self.robust
            .map(|(low, high)| ((low as f64 * self.kind.resolution()) as f32, (high as f64 * self.kind.resolution()) as f32))
    }

    /// Attach the curve mnemonic this trace was read from.
    pub(crate) fn with_source(mut self, mnemonic: impl Into<String>) -> Self {
        self.source = mnemonic.into();
        self
    }

    /// The curve mnemonic this trace was read from, empty if never set.
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn memory_bytes(&self) -> usize {
        let values_bytes = self.values.capacity() * std::mem::size_of::<u16>();
        let pyramid_bytes: usize = self.pyramid.iter().map(|level| level.capacity() * std::mem::size_of::<(u16, u16)>()).sum();
        let pyramid_vec_overhead = self.pyramid.capacity() * std::mem::size_of::<Vec<(u16, u16)>>();
        std::mem::size_of::<Self>() + values_bytes + pyramid_bytes + pyramid_vec_overhead + self.source.capacity()
    }
}

/// Iterator returned by [`LogTrace::envelope`]. Always yields exactly as many
/// items as requested.
pub(crate) struct Envelope<'a> {
    trace: &'a LogTrace,
    remaining: Range<usize>,
    from: f64,
    w: f64,
}

impl<'a> Iterator for Envelope<'a> {
    type Item = Option<(f32, f32)>;

    fn next(&mut self) -> Option<Self::Item> {
        let b = self.remaining.next()?;
        let bucket_from = self.from + b as f64 * self.w;
        let bucket_to = self.from + (b + 1) as f64 * self.w;
        Some(self.trace.bucket_value(bucket_from, bucket_to))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining.size_hint()
    }
}

impl<'a> ExactSizeIterator for Envelope<'a> {}

/// The (up to) three traces held for one drill hole.
#[derive(Default)]
pub(crate) struct HoleLogs {
    traces: [Option<LogTrace>; 3],
}

impl HoleLogs {
    pub(crate) fn trace(&self, kind: LogKind) -> Option<&LogTrace> {
        self.traces[kind.index()].as_ref()
    }
}

/// The session's downhole geophysics traces, keyed by the drill-hole dataset
/// and hole id (compared exactly) each was matched to, so a dataset's traces go
/// with it. Not saved with the project.
#[derive(Default)]
pub(crate) struct GeophysicsStore {
    datasets: HashMap<DrillHoleId, BTreeMap<String, HoleLogs>>,
    /// A dataset's share of the browser's working-set budget, released with
    /// its traces.
    reservations: HashMap<DrillHoleId, crate::app::memory::MemoryReservation>,
}

impl GeophysicsStore {
    /// Store `trace` for its hole and kind, returning the trace it replaced:
    /// a later import replaces an earlier one.
    pub(crate) fn insert(&mut self, dataset: DrillHoleId, dhid: &str, trace: LogTrace) -> Option<LogTrace> {
        let kind = trace.kind;
        let hole = self.datasets.entry(dataset).or_default().entry(dhid.to_owned()).or_default();
        hole.traces[kind.index()].replace(trace)
    }

    /// Hold `holes` as all of `dataset`'s traces, replacing any it had, with
    /// the reservation that covers them.
    pub(crate) fn replace_dataset(&mut self, dataset: DrillHoleId, holes: Vec<(String, Vec<LogTrace>)>, reservation: crate::app::memory::MemoryReservation) {
        self.drop_dataset(dataset);
        for (dhid, traces) in holes {
            for trace in traces {
                self.insert(dataset, &dhid, trace);
            }
        }
        if self.datasets.contains_key(&dataset) {
            self.reservations.insert(dataset, reservation);
        }
    }

    /// Bytes held for one dataset's traces.
    pub(crate) fn dataset_memory_bytes(&self, dataset: DrillHoleId) -> usize {
        self.datasets.get(&dataset).map_or(0, |holes| {
            holes
                .iter()
                .map(|(dhid, hole)| dhid.capacity() + std::mem::size_of::<HoleLogs>() + hole.traces.iter().flatten().map(LogTrace::memory_bytes).sum::<usize>())
                .sum()
        })
    }

    pub(crate) fn hole(&self, dataset: DrillHoleId, dhid: &str) -> Option<&HoleLogs> {
        self.datasets.get(&dataset).and_then(|holes| holes.get(dhid))
    }

    /// True when at least one trace is held for `dataset`. A hole entry only
    /// exists because `insert` created it, and `insert` always stores a
    /// trace into the hole it creates, so a non-empty holes map already
    /// means at least one trace.
    pub(crate) fn has_dataset(&self, dataset: DrillHoleId) -> bool {
        self.datasets.get(&dataset).is_some_and(|holes| !holes.is_empty())
    }

    pub(crate) fn holes(&self) -> impl Iterator<Item = (DrillHoleId, &str, &HoleLogs)> + '_ {
        self.datasets
            .iter()
            .flat_map(|(dataset, holes)| holes.iter().map(move |(dhid, hole)| (*dataset, dhid.as_str(), hole)))
    }

    /// Drop every trace matched to `dataset`; true if there were any.
    pub(crate) fn drop_dataset(&mut self, dataset: DrillHoleId) -> bool {
        self.reservations.remove(&dataset);
        self.datasets.remove(&dataset).is_some()
    }

    /// Keep only the traces of `datasets`, the loaded ones: a dataset closed
    /// or removed gives back its traces and their share of the budget.
    pub(crate) fn keep_datasets(&mut self, datasets: impl IntoIterator<Item = DrillHoleId>) {
        let keep = datasets.into_iter().collect::<std::collections::HashSet<_>>();
        self.datasets.retain(|dataset, _| keep.contains(dataset));
        self.reservations.retain(|dataset, _| keep.contains(dataset));
    }

    /// Holes with at least one trace, across datasets.
    pub(crate) fn hole_count(&self) -> usize {
        self.datasets.values().map(BTreeMap::len).sum()
    }

    pub(crate) fn trace_count(&self, kind: LogKind) -> usize {
        self.holes().filter(|(_, _, hole)| hole.trace(kind).is_some()).count()
    }

    pub(crate) fn memory_bytes(&self) -> usize {
        let per_dataset = std::mem::size_of::<BTreeMap<String, HoleLogs>>();
        std::mem::size_of::<Self>() + self.datasets.keys().map(|dataset| per_dataset + self.dataset_memory_bytes(*dataset)).sum::<usize>()
    }

    pub(crate) fn clear(&mut self) {
        self.datasets.clear();
        self.reservations.clear();
    }
}
