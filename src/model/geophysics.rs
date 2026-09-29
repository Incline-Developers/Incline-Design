//! Downhole geophysics: the traces of one hole, and the link that finds a
//! hole's rows in a CSV file too large to hold. The file is read through
//! once for an index saved with the dataset, then a hole at a time.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ops::Range,
    path::PathBuf,
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    i18n::tr_format,
    model::drill_hole::{DrillHoleId, OpenDrillHoleDataset},
};

/// The curves the log draws, each on a fixed scale of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

/// Quantise one value to the curve's fixed-point encoding.
fn quantize(kind: LogKind, v: f64) -> u16 {
    if !v.is_finite() {
        return NULL_RAW;
    }
    let resolution = kind.resolution();
    let max_value = kind.max_value();
    let bounded = if v < 0.0 {
        0.0
    } else if v > max_value {
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

/// How a trace's readings map to its stored samples.
#[derive(Clone, Copy, Debug)]
enum Scale {
    /// A drawn curve, on its kind's fixed resolution from zero.
    Fixed(LogKind),
    /// Any other curve: its unit is unknown, so its samples span the
    /// readings it had, lowest to highest.
    Fitted { offset: f64, resolution: f64 },
}

impl Scale {
    fn fitted(values: impl Iterator<Item = f64>) -> Self {
        let (low, high) = values
            .filter(|value| value.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| (low.min(value), high.max(value)));
        let span = high - low;
        Scale::Fitted {
            offset: if low.is_finite() { low } else { 0.0 },
            resolution: if span.is_finite() && span > 0.0 { span / f64::from(NULL_RAW - 1) } else { 1.0 },
        }
    }

    fn encode(self, value: f64) -> u16 {
        match self {
            Scale::Fixed(kind) => quantize(kind, value),
            Scale::Fitted { .. } if !value.is_finite() => NULL_RAW,
            Scale::Fitted { offset, resolution } => ((value - offset) / resolution).round().clamp(0.0, f64::from(NULL_RAW - 1)) as u16,
        }
    }

    fn decode(self, raw: u16) -> Option<f32> {
        match self {
            Scale::Fixed(kind) => decode(kind, raw),
            Scale::Fitted { .. } if raw == NULL_RAW => None,
            Scale::Fitted { offset, resolution } => Some((offset + raw as f64 * resolution) as f32),
        }
    }
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
/// be the curve.
fn drop_isolated_readings(rows: &mut Vec<(f64, f64)>) {
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
        return;
    }
    groups.retain(|group| group.2 < MIN_GROUP_READINGS);
    if groups.is_empty() {
        return;
    }
    let mut next = groups.iter().peekable();
    rows.retain(|(depth, _)| {
        while next.peek().is_some_and(|group| group.1 < *depth) {
            next.next();
        }
        !next.peek().is_some_and(|group| group.0 <= *depth)
    });
}

/// One downhole curve: evenly spaced quantised samples plus a min/max pyramid
/// for fast zoomed-out queries.
#[derive(Clone)]
pub(crate) struct LogTrace {
    scale: Scale,
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
    /// refused. A curve of no `kind` is scaled to its own readings.
    pub(crate) fn from_samples(kind: Option<LogKind>, depths: &[f64], values: &[f64]) -> Result<LogTrace, TraceError> {
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
        let resampled = rows.windows(2).any(|pair| pair[1].0 < pair[0].0);
        if resampled {
            rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        }

        drop_isolated_readings(&mut rows);
        let scale = kind.map_or_else(|| Scale::fitted(rows.iter().map(|&(_, value)| value)), Scale::Fixed);

        let (start, step, raw_values) = if rows.len() == 1 {
            (rows[0].0, 1.0, vec![scale.encode(rows[0].1)])
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
                (rows[0].0, median, rows.iter().map(|&(_, value)| scale.encode(value)).collect())
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
                        raw.push(NULL_RAW);
                    } else {
                        raw.push(scale.encode(rows[nearest].1));
                    }
                }
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
        Ok(LogTrace {
            scale,
            start,
            step,
            values: values_trimmed,
            pyramid,
            robust,
            source: String::new(),
        })
    }

    /// The drawn curve this is, if it is one.
    pub(crate) fn kind(&self) -> Option<LogKind> {
        match self.scale {
            Scale::Fixed(kind) => Some(kind),
            Scale::Fitted { .. } => None,
        }
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
        self.values.get(i).and_then(|&raw| self.scale.decode(raw))
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
            return self.scale.decode(self.values[nearest]).map(|v| (v, v));
        }
        let (mn, mx) = self.range_min_max(idx_from, idx_to_excl);
        match (self.scale.decode(mn), self.scale.decode(mx)) {
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
        let (low, high) = self.robust?;
        self.scale.decode(low).zip(self.scale.decode(high))
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

/// Every trace read for one drill hole: the drawn curves and any other
/// curve column its files carry.
#[derive(Default)]
pub(crate) struct HoleLogs {
    traces: Vec<LogTrace>,
}

impl HoleLogs {
    pub(crate) fn new(traces: Vec<LogTrace>) -> Self {
        Self { traces }
    }

    pub(crate) fn trace(&self, kind: LogKind) -> Option<&LogTrace> {
        self.traces.iter().find(|trace| trace.kind() == Some(kind))
    }

    pub(crate) fn traces(&self) -> &[LogTrace] {
        &self.traces
    }

    pub(crate) fn memory_bytes(&self) -> usize {
        size_of::<Self>() + self.traces.iter().map(LogTrace::memory_bytes).sum::<usize>()
    }
}

/// A hole's rows past this many bytes are not read: tens of thousands of
/// metres at a centimetre step, so a file not grouped as its index says.
pub(crate) const MAX_HOLE_BYTES: u64 = 256 * 1024 * 1024;

/// Geophysics files linked to a drill-hole dataset, saved with it. Only the
/// index is kept: the readings stay in the files, read a hole at a time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct GeophysicsLink {
    /// In the order they were read: a later file adds only depths an
    /// earlier one has no reading at.
    pub(crate) files: Vec<LinkedFile>,
}

impl GeophysicsLink {
    /// The runs of `dhid`'s rows, by file, in the order they are read.
    pub(crate) fn runs_of<'a>(&'a self, dhid: &'a str) -> impl Iterator<Item = (usize, [u64; 2])> + 'a {
        self.files
            .iter()
            .enumerate()
            .flat_map(move |(index, file)| file.hole(dhid).into_iter().flat_map(move |hole| hole.runs.iter().map(move |run| (index, *run))))
    }

    pub(crate) fn holds(&self, dhid: &str) -> bool {
        self.files.iter().any(|file| file.hole(dhid).is_some())
    }

    /// Which drawn curves `dhid` has readings for, by [`LogKind::index`],
    /// across its files as a read joins them.
    pub(crate) fn kinds_of(&self, dhid: &str) -> [bool; 3] {
        let mut kinds = [false; 3];
        for (file, hole) in self.files.iter().filter_map(|file| file.hole(dhid).map(|hole| (file, hole))) {
            for (held, has) in kinds.iter_mut().zip(file.kinds(hole)) {
                *held |= has;
            }
        }
        kinds
    }

    /// The shallowest and deepest depth `dhid`'s drawn curves have readings
    /// at across its files, known before any is read.
    pub(crate) fn depths_of(&self, dhid: &str) -> Option<[f64; 2]> {
        self.files
            .iter()
            .filter_map(|file| file.hole(dhid).filter(|hole| file.kinds(hole).contains(&true)))
            .map(|hole| hole.depths)
            .reduce(|[top, bottom], [from, to]| [top.min(from), bottom.max(to)])
    }

    /// Put the holes back in the order lookups rely on, whatever wrote them.
    pub(crate) fn sort(&mut self) {
        for file in &mut self.files {
            file.holes.sort_unstable_by(|a, b| a.dhid.cmp(&b.dhid));
        }
    }
}

/// One linked file and where each hole's rows are in it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct LinkedFile {
    /// Where the file was linked from: its full path on the desktop, only
    /// its name in the browser.
    pub(crate) path: PathBuf,
    pub(crate) identity: FileIdentity,
    /// Every column of the header, in order.
    pub(crate) columns: Vec<LinkedColumn>,
    /// Sorted by hole id.
    pub(crate) holes: Vec<HoleRuns>,
}

impl LinkedFile {
    pub(crate) fn hole(&self, dhid: &str) -> Option<&HoleRuns> {
        self.holes.binary_search_by(|hole| hole.dhid.as_str().cmp(dhid)).ok().map(|index| &self.holes[index])
    }

    /// Which drawn curves `hole` has in this file, by [`LogKind::index`].
    fn kinds(&self, hole: &HoleRuns) -> [bool; 3] {
        let mut kinds = [false; 3];
        for (index, column) in self.columns.iter().enumerate() {
            if let ColumnRole::Curve { kind: Some(kind) } = column.role
                && hole.curves.contains(index)
            {
                kinds[kind.index()] = true;
            }
        }
        kinds
    }
}

/// What tells one version of a file from another without reading it all.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct FileIdentity {
    pub(crate) name: String,
    pub(crate) size: u64,
    /// Milliseconds since 1970, where the platform gives one.
    pub(crate) modified: Option<i64>,
    /// XXH64 of the file's first and last [`FileIdentity::END_BYTES`].
    pub(crate) ends_hash: u64,
}

impl FileIdentity {
    pub(crate) const END_BYTES: u64 = 64 * 1024;

    /// The head and tail hashed, as byte ranges of a file of `size` bytes.
    /// A small file's tail starts where its head ends.
    pub(crate) fn end_ranges(size: u64) -> [Range<u64>; 2] {
        let head = 0..size.min(Self::END_BYTES);
        let tail = size.saturating_sub(Self::END_BYTES).max(head.end)..size;
        [head, tail]
    }

    pub(crate) fn new(name: String, size: u64, modified: Option<i64>, head: &[u8], tail: &[u8]) -> Self {
        use std::hash::Hasher;

        let mut hasher = twox_hash::XxHash64::with_seed(0);
        hasher.write(head);
        hasher.write(tail);
        Self {
            name,
            size,
            modified,
            ends_hash: hasher.finish(),
        }
    }

    /// Whether `other` is this file unchanged. The name is left out: a file
    /// renamed or moved is still the file its index describes.
    pub(crate) fn matches(&self, other: &FileIdentity) -> bool {
        self.size == other.size && self.modified == other.modified && self.ends_hash == other.ends_hash
    }

    /// A file's last-modified time in milliseconds since 1970, where the
    /// platform gives one.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn modified_millis(metadata: &std::fs::Metadata) -> Option<i64> {
        metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_millis() as i64)
    }

    /// Read a file's identity from disk: its metadata and its two ends.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn of_path(path: &std::path::Path) -> std::io::Result<Self> {
        use std::io::{Read, Seek, SeekFrom};

        let mut file = std::fs::File::open(path)?;
        let metadata = file.metadata()?;
        let modified = Self::modified_millis(&metadata);
        let mut ends = [Vec::new(), Vec::new()];
        for (range, bytes) in Self::end_ranges(metadata.len()).into_iter().zip(&mut ends) {
            file.seek(SeekFrom::Start(range.start))?;
            (&mut file).take(range.end - range.start).read_to_end(bytes)?;
        }
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(Self::new(name, metadata.len(), modified, &ends[0], &ends[1]))
    }
}

/// One header column of a linked file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct LinkedColumn {
    pub(crate) header: String,
    pub(crate) role: ColumnRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ColumnRole {
    Dhid,
    Depth,
    /// A numeric column, read as a curve; `kind` when the log draws it.
    Curve {
        kind: Option<LogKind>,
    },
    /// A density curve whose readings were mostly outside the g/cc range,
    /// so its unit looked wrong and it is not read.
    LeftOut {
        kind: LogKind,
    },
    /// Text, or never a number: not a curve.
    Skipped,
}

/// Where one hole's rows are in a linked file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct HoleRuns {
    pub(crate) dhid: String,
    /// Byte ranges, `[start, end)`, of each run of the hole's rows, in file
    /// order. A run is read on its own: a later one adds only depths the
    /// hole has no reading at.
    pub(crate) runs: Vec<[u64; 2]>,
    /// Shallowest and deepest depth a drawn curve has a reading at, or of
    /// any row where none has.
    pub(crate) depths: [f64; 2],
    /// The curve columns with a reading for the hole.
    pub(crate) curves: ColumnSet,
}

/// Columns of a linked file, a bit each in header order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ColumnSet(Vec<u8>);

impl ColumnSet {
    pub(crate) fn insert(&mut self, column: usize) {
        let byte = column / 8;
        if byte >= self.0.len() {
            self.0.resize(byte + 1, 0);
        }
        self.0[byte] |= 1 << (column % 8);
    }

    pub(crate) fn contains(&self, column: usize) -> bool {
        self.0.get(column / 8).is_some_and(|byte| byte & 1 << (column % 8) != 0)
    }

    pub(crate) fn union(&mut self, other: &ColumnSet) {
        if self.0.len() < other.0.len() {
            self.0.resize(other.0.len(), 0);
        }
        self.0.iter_mut().zip(&other.0).for_each(|(byte, other)| *byte |= other);
    }

    pub(crate) fn intersect(&mut self, other: &ColumnSet) {
        self.0.truncate(other.0.len());
        self.0.iter_mut().zip(&other.0).for_each(|(byte, other)| *byte &= other);
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
}

/// Holes kept parsed at once.
const CACHED_HOLES: usize = 8;

/// Bytes of parsed holes kept at once; the hole read last is kept whatever
/// its size.
const CACHE_BYTES: usize = 128 * 1024 * 1024;

/// Whether this session can read a dataset's linked files.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LinkState {
    /// Making sure the files are where the link says, and unchanged.
    Checking,
    /// Reading a file through for a fresh index.
    Indexing,
    Ready,
    /// A file is not where the link says. Only the desktop finds this: the
    /// browser asks for the file again instead.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code, reason = "a page has no path to find a file missing from"))]
    Missing {
        file: String,
    },
    /// The browser has to be given the file again this session.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code, reason = "the desktop reopens a file by its path"))]
    NeedsPick {
        file: String,
    },
    Failed(String),
}

static CHECKING: LinkState = LinkState::Checking;

/// What the log can show of one hole's geophysics.
pub(crate) enum HoleView<'a> {
    /// The dataset has no geophysics linked.
    Unlinked,
    /// The linked files cannot be read, yet or at all.
    Link(&'a LinkState),
    /// The linked files have no rows for the hole.
    NotInFiles,
    Reading,
    /// Not read yet, and nothing is reading it.
    Wanted,
    Failed(&'a str),
    Shown(&'a HoleLogs),
}

/// The session's side of every linked dataset: whether its files can be
/// read, and the holes read lately. Nothing here is saved.
#[derive(Default)]
pub(crate) struct GeophysicsSession {
    links: HashMap<DrillHoleId, LinkSession>,
    /// The holes read most recently first.
    cache: VecDeque<CachedHole>,
    cache_bytes: usize,
    /// Never reused, so work for a link since replaced is known stale.
    next_generation: u64,
}

struct LinkSession {
    /// The link as the session took it up; a dataset holding another one
    /// has been relinked, reopened or reindexed.
    link: Arc<GeophysicsLink>,
    generation: u64,
    state: LinkState,
    reading: HashSet<String>,
    failed: HashMap<String, String>,
}

struct CachedHole {
    dataset: DrillHoleId,
    generation: u64,
    dhid: String,
    logs: HoleLogs,
    bytes: usize,
    /// Holds `bytes` of the browser's budget until the hole is dropped.
    _reservation: crate::app::memory::MemoryReservation,
}

/// Claim `bytes` of the browser's working set for `dhid`'s geophysics, or
/// say why not. Always granted on the desktop.
pub(crate) fn reserve_hole(dhid: &str, bytes: usize) -> Result<crate::app::memory::MemoryReservation, String> {
    crate::app::memory::reserve(bytes, &format!("{dhid} geophysics")).map_err(|_| {
        tr_format!(
            literal = "%hole% needs %size% MiB for its geophysics, more than the browser has left: unload other items, then unload and load this dataset again",
            hole = dhid.to_owned(),
            size = bytes.div_ceil(1024 * 1024)
        )
    })
}

impl GeophysicsSession {
    pub(crate) fn view(&self, dataset: &OpenDrillHoleDataset, dhid: &str) -> HoleView<'_> {
        let session = self.links.get(&dataset.id);
        // Before the dataset's first link is indexed too.
        if let Some(session) = session.filter(|session| session.state != LinkState::Ready) {
            return HoleView::Link(&session.state);
        }
        let Some(link) = dataset.geophysics.as_deref() else {
            return HoleView::Unlinked;
        };
        let Some(session) = session else {
            return HoleView::Link(&CHECKING);
        };
        if !link.holds(dhid) {
            return HoleView::NotInFiles;
        }
        if let Some(hole) = self
            .cache
            .iter()
            .find(|hole| hole.dataset == dataset.id && hole.generation == session.generation && hole.dhid == dhid)
        {
            return HoleView::Shown(&hole.logs);
        }
        if session.reading.contains(dhid) {
            return HoleView::Reading;
        }
        match session.failed.get(dhid) {
            Some(error) => HoleView::Failed(error),
            None => HoleView::Wanted,
        }
    }

    pub(crate) fn generation(&self, dataset: DrillHoleId) -> Option<u64> {
        self.links.get(&dataset).map(|session| session.generation)
    }

    /// The state of `dataset`'s link, if the session has taken one up.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code, reason = "the browser finds links waiting for a pick with it"))]
    pub(crate) fn state(&self, dataset: DrillHoleId) -> Option<&LinkState> {
        self.links.get(&dataset).map(|session| &session.state)
    }

    /// Let go of datasets gone, unloaded or unlinked, and return the loaded
    /// ones holding a link the session has not taken up. A dataset with no
    /// link yet is kept while its first link is indexed, and once that has
    /// failed, so the failure stays shown until it is linked again.
    pub(crate) fn sync(&mut self, datasets: &[OpenDrillHoleDataset]) -> Vec<DrillHoleId> {
        let loaded = |id: DrillHoleId| datasets.iter().find(|dataset| dataset.id == id && dataset.state.loaded);
        let before = self.links.len();
        self.links
            .retain(|id, session| loaded(*id).is_some_and(|dataset| dataset.geophysics.is_some() || matches!(session.state, LinkState::Indexing | LinkState::Failed(_))));
        if self.links.len() != before {
            self.trim_cache();
        }
        datasets
            .iter()
            .filter(|dataset| dataset.state.loaded)
            .filter(|dataset| {
                dataset
                    .geophysics
                    .as_ref()
                    .is_some_and(|link| !self.links.get(&dataset.id).is_some_and(|session| Arc::ptr_eq(&session.link, link)))
            })
            .map(|dataset| dataset.id)
            .collect()
    }

    /// Take up `link` for `dataset`, forgetting what was read under an
    /// earlier one; returns the generation work for it runs under.
    pub(crate) fn adopt(&mut self, dataset: DrillHoleId, link: &Arc<GeophysicsLink>, state: LinkState) -> u64 {
        self.next_generation += 1;
        self.links.insert(
            dataset,
            LinkSession {
                link: Arc::clone(link),
                generation: self.next_generation,
                state,
                reading: HashSet::new(),
                failed: HashMap::new(),
            },
        );
        self.trim_cache();
        self.next_generation
    }

    /// Set the state of `dataset`'s link, if `generation` is still its.
    pub(crate) fn set_state(&mut self, dataset: DrillHoleId, generation: u64, state: LinkState) {
        if let Some(session) = self.links.get_mut(&dataset).filter(|session| session.generation == generation) {
            session.state = state;
        }
    }

    /// Mark `dhid` as being read and return the generation to read it
    /// under, or `None` when it is read, being read, failed or cannot be.
    pub(crate) fn begin_read(&mut self, dataset: &OpenDrillHoleDataset, dhid: &str) -> Option<u64> {
        if !matches!(self.view(dataset, dhid), HoleView::Wanted) {
            return None;
        }
        let session = self.links.get_mut(&dataset.id)?;
        session.reading.insert(dhid.to_owned());
        Some(session.generation)
    }

    /// Keep a hole read under `generation`, or why it could not be.
    pub(crate) fn finish_read(&mut self, dataset: DrillHoleId, generation: u64, dhid: String, result: Result<HoleLogs, String>) {
        let Some(session) = self.links.get_mut(&dataset).filter(|session| session.generation == generation) else {
            return;
        };
        session.reading.remove(&dhid);
        match result {
            Ok(logs) => {
                let bytes = logs.memory_bytes() + dhid.capacity();
                match reserve_hole(&dhid, bytes) {
                    Ok(reservation) => {
                        self.cache_bytes += bytes;
                        self.cache.push_front(CachedHole {
                            dataset,
                            generation,
                            dhid,
                            logs,
                            bytes,
                            _reservation: reservation,
                        });
                        self.trim_cache();
                    }
                    Err(error) => {
                        session.failed.insert(dhid, error);
                    }
                }
            }
            Err(error) => {
                session.failed.insert(dhid, error);
            }
        }
    }

    /// Drop holes of links let go of or replaced, then the oldest past the
    /// budget.
    fn trim_cache(&mut self) {
        let links = &self.links;
        let mut freed = 0;
        self.cache.retain(|hole| {
            let current = links.get(&hole.dataset).is_some_and(|session| session.generation == hole.generation);
            freed += if current { 0 } else { hole.bytes };
            current
        });
        self.cache_bytes -= freed;
        while self.cache.len() > CACHED_HOLES || (self.cache.len() > 1 && self.cache_bytes > CACHE_BYTES) {
            let Some(oldest) = self.cache.pop_back() else { break };
            self.cache_bytes -= oldest.bytes;
        }
    }

    /// Bytes of parsed holes held.
    pub(crate) fn memory_bytes(&self) -> usize {
        self.cache_bytes
    }

    pub(crate) fn clear(&mut self) {
        self.links.clear();
        self.cache.clear();
        self.cache_bytes = 0;
    }
}
