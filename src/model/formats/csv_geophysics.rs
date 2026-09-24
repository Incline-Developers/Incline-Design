//! Downhole geophysics from a CSV file: one row per sample, a hole id, a
//! depth and gamma or density columns, as a database exports the logs once
//! they are cleaned. Raw instrument formats are not read here or anywhere,
//! and no unit is converted: depth is metres, gamma API and density g/cc.
//!
//! A file runs to millions of rows, so it is never held as text. Rows are
//! read one at a time straight into numbers, and a hole's samples become
//! traces as soon as the rows move on to the next hole. A hole that comes
//! back for a curve it holds keeps what it holds: the later rows add only
//! depths it has no reading at, joined in batches so that rows interleaved
//! hole by hole are not a rebuild each.

use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap, HashSet},
    io::BufRead,
    ops::Range,
};

use crate::{
    app::memory::MemoryReservation,
    i18n::{tr, tr_format},
    model::{
        formats::csv_drill_hole::{self, CsvDrillColumnRole, CsvDrillError, CsvDrillFileMapping, SKIP_REPORT_LIMIT},
        geophysics::{LogKind, LogTrace, TraceError},
    },
    userspace_warn,
};

/// Hole ids named in one report before the rest are only counted.
const HOLE_LIST_LIMIT: usize = 5;

/// Longest record read, a quoted cell's line breaks and all. Checked while
/// reading, so a file with no line breaks, or not text at all, fails here
/// rather than being read whole into one line.
const MAX_RECORD_BYTES: usize = 1024 * 1024;

/// Fewest rows a hole's later runs are joined in: with batches at least as
/// large as what the hole holds, rows interleaved hole by hole cost a
/// rebuild per doubling rather than per row.
const REJOIN_BATCH_ROWS: usize = 4096;

/// Most bytes of rows waiting to be joined, across holes, before the hole
/// with the most waiting is joined early.
const WAITING_BYTES_LIMIT: usize = 64 * 1024 * 1024;

/// Bytes counted per depth held in the set a batch is checked against.
const COVERED_ENTRY_BYTES: usize = 32;

/// Rows between checks of the transient memory held.
const MEMORY_CHECK_ROWS: usize = 1 << 16;

/// Rows between asking whether the import was cancelled.
const CANCEL_CHECK_ROWS: usize = 4096;

/// Smallest step a memory reservation grows by, so a file does not reserve
/// once per hole.
const RESERVE_STEP: usize = 16 * 1024 * 1024;

/// Bytes read between progress reports.
const PROGRESS_STRIDE: u64 = 4 * 1024 * 1024;

/// Two readings this close are one depth.
const SAME_DEPTH: f64 = 1.0e-6;

/// Where a density curve's median has to lie for its unit to be g/cc.
const PLAUSIBLE_DENSITY: std::ops::RangeInclusive<f64> = 0.5..=5.0;

/// How a long read reports progress and learns it should stop.
#[derive(Clone, Copy)]
pub(crate) struct StreamControl<'a> {
    /// Told the share of the bundle's bytes read so far: the tables' all at
    /// once when they are parsed, then the geophysics as it streams.
    pub(crate) progress: &'a dyn Fn(f32),
    /// Asked every few thousand rows; true stops the read.
    pub(crate) cancelled: &'a dyn Fn() -> bool,
}

/// The curve a column role carries.
pub(crate) fn curve_kind(role: &CsvDrillColumnRole) -> Option<LogKind> {
    match role {
        CsvDrillColumnRole::Gamma => Some(LogKind::Gamma),
        CsvDrillColumnRole::LongDensity => Some(LogKind::LongDensity),
        CsvDrillColumnRole::ShortDensity => Some(LogKind::ShortDensity),
        _ => None,
    }
}

/// What a geophysics import leaves for the store, with the counts its
/// summary reports.
pub(crate) struct GeophysicsImport {
    /// Each hole with at least one trace, and its traces.
    pub(crate) holes: Vec<(String, Vec<LogTrace>)>,
    /// Data rows read, blank lines aside.
    pub(crate) rows_read: usize,
    /// Rows refused: wrong width, no hole id, no usable depth.
    pub(crate) rows_skipped: usize,
    /// Rows for holes the bundle does not define.
    pub(crate) orphan_rows: usize,
    /// Distinct holes the bundle does not define.
    pub(crate) orphan_holes: usize,
    /// Readings left empty for lying outside what the curve can hold:
    /// negative, which takes in the no-reading sentinels, or above the
    /// largest value a trace stores.
    pub(crate) out_of_range: usize,
    /// Readings of a later run left out because the hole already held a
    /// reading of that curve within half a step.
    pub(crate) already_held: usize,
    /// Readings left out as isolated: a small group far from the rest of
    /// its hole's curve.
    pub(crate) isolated: usize,
    /// Runs that could not be made a trace.
    pub(crate) runs_not_kept: usize,
    /// Density curves left out of a file because their unit looked wrong.
    pub(crate) curves_left_out: usize,
    /// Held against the browser's working-set budget for as long as the
    /// traces are; nothing on native.
    pub(crate) reservation: MemoryReservation,
}

impl GeophysicsImport {
    /// Traces held per curve, indexed by [`LogKind::index`].
    pub(crate) fn traces_per_curve(&self) -> [usize; LogKind::ALL.len()] {
        let mut counts = [0; LogKind::ALL.len()];
        for trace in self.holes.iter().flat_map(|(_, traces)| traces) {
            counts[trace.kind().index()] += 1;
        }
        counts
    }
}

/// The samples of one run of rows, a column per mapped curve.
#[derive(Default)]
struct Samples {
    depths: Vec<f64>,
    /// Empty for a curve the file does not map.
    values: [Vec<f64>; 3],
}

impl Samples {
    fn bytes(&self) -> usize {
        (self.depths.capacity() + self.values.iter().map(Vec::capacity).sum::<usize>()) * size_of::<f64>()
    }

    fn clear(&mut self) {
        self.depths.clear();
        self.values.iter_mut().for_each(Vec::clear);
    }

    /// Move `other`'s rows onto the end. Both come from one file, so they
    /// map the same curves.
    fn append(&mut self, other: &mut Samples) {
        self.depths.append(&mut other.depths);
        for (values, others) in self.values.iter_mut().zip(other.values.iter_mut()) {
            values.append(others);
        }
    }
}

/// A hole's rows waiting to be joined, and each run's row count in the order
/// the runs arrived.
#[derive(Default)]
struct Waiting {
    samples: Samples,
    runs: Vec<usize>,
}

impl Waiting {
    fn bytes(&self) -> usize {
        self.samples.bytes() + self.runs.capacity() * size_of::<usize>()
    }
}

/// The run of rows being read: consecutive rows for one hole.
struct Run {
    dhid: String,
    /// A hole the bundle does not define: its rows are counted, not kept.
    orphan: bool,
}

/// Memory held against the working-set budget, grown in steps.
struct Held {
    reservation: MemoryReservation,
    reserved: usize,
}

impl Held {
    fn new() -> Self {
        Self {
            reservation: MemoryReservation::untracked(),
            reserved: 0,
        }
    }

    fn cover(&mut self, bytes: usize) -> Result<(), CsvDrillError> {
        if bytes <= self.reserved {
            return Ok(());
        }
        let target = bytes.max(self.reserved.saturating_add(RESERVE_STEP));
        self.reservation.grow_to(target, &tr!(literal = "Downhole geophysics")).map_err(CsvDrillError::Invalid)?;
        self.reserved = target;
        Ok(())
    }

    /// The reservation, trimmed to the bytes actually held.
    fn settle(self, bytes: usize) -> MemoryReservation {
        self.reservation.shrink_to(bytes);
        self.reservation
    }
}

/// What one file has done so far, kept apart until the file is read to its
/// end: a file that fails, or a density curve whose unit looks wrong, comes
/// back out of every hole the file touched.
#[derive(Default)]
struct FileState {
    /// Set once the file's header is read and it may touch holes; until
    /// then there is nothing of it to take back out.
    started: bool,
    index: usize,
    name: String,
    /// The header each curve is read from, shown beside its trace.
    sources: [String; 3],
    /// Density readings below, within and above the plausible g/cc range.
    votes: [[usize; 3]; 3],
    out_of_range: [usize; 3],
    /// Each curve's trace before this file first touched it.
    before: HashMap<(String, LogKind), Option<LogTrace>>,
    before_bytes: usize,
    rows: usize,
    skipped: usize,
    orphans: HashSet<String>,
    orphan_rows: usize,
    /// Holes that came back for a curve they already held.
    rejoined: HashSet<String>,
    repeated: usize,
    already_held: [usize; 3],
    isolated: [usize; 3],
}

impl FileState {
    /// A curve cell as a reading, or NaN where it holds none.
    fn reading(&mut self, kind: LogKind, cell: &[u8]) -> f64 {
        let Some(value) = cell_number(cell) else {
            return f64::NAN;
        };
        // The usual no-reading sentinels, -999.25, -999 and -9999, are
        // negative and are caught here with every other impossible reading.
        if value < 0.0 {
            self.out_of_range[kind.index()] += 1;
            return f64::NAN;
        }
        if kind != LogKind::Gamma {
            let side = if value < *PLAUSIBLE_DENSITY.start() {
                0
            } else if value > *PLAUSIBLE_DENSITY.end() {
                2
            } else {
                1
            };
            self.votes[kind.index()][side] += 1;
        }
        if value > kind.max_value() {
            self.out_of_range[kind.index()] += 1;
            return f64::NAN;
        }
        value
    }
}

/// A run that could not be made a trace, reported once the import is done.
struct Note {
    file: usize,
    dhid: String,
    kind: LogKind,
    error: TraceError,
}

/// Reads the geophysics files of one bundle into traces, merging a hole's
/// runs as each ends.
pub(crate) struct GeophysicsReader {
    /// The holes the bundle defines; rows for any other are orphans.
    known: HashSet<String>,
    run: Option<Run>,
    samples: Samples,
    /// One curve's readings from the runs being settled.
    readings: Vec<(f64, f64)>,
    /// One run's readings, checked before they join `readings`.
    run_readings: Vec<(f64, f64)>,
    /// Depths, as bits, an earlier run of the batch already read.
    covered: BTreeSet<u64>,
    /// A held trace's samples and new readings, joined.
    joined: Vec<(f64, f64)>,
    /// Every hole whose run has ended, with its traces so far.
    built: HashMap<String, [Option<LogTrace>; 3]>,
    /// Rows of holes back for a curve they hold, waiting to be joined.
    pending: HashMap<String, Waiting>,
    pending_bytes: usize,
    /// Bytes of waiting rows past which the largest batch is joined early.
    pending_limit: usize,
    /// The most waiting rows held at once, in bytes, for the log.
    pending_peak: usize,
    /// The run buffer while it is taken out to be settled or queued.
    spare_bytes: usize,
    /// A batch while its traces are built from it.
    settling_bytes: usize,
    file: FileState,
    files: usize,
    /// Holes whose rows came in more than one run.
    rejoined: HashSet<String>,
    orphans: HashSet<String>,
    orphan_rows: usize,
    rows_read: usize,
    rows_skipped: usize,
    out_of_range: usize,
    /// Readings at a depth read earlier in the same file for that hole.
    repeated: usize,
    /// Readings left out for a reading the hole already held.
    already_held: usize,
    isolated: usize,
    notes: Vec<Note>,
    curves_left_out: usize,
    transient: Held,
    kept: Held,
    kept_bytes: usize,
}

impl GeophysicsReader {
    pub(crate) fn new(known: impl IntoIterator<Item = String>) -> Self {
        Self {
            known: known.into_iter().collect(),
            run: None,
            samples: Samples::default(),
            readings: Vec::new(),
            run_readings: Vec::new(),
            covered: BTreeSet::new(),
            joined: Vec::new(),
            built: HashMap::new(),
            pending: HashMap::new(),
            pending_bytes: 0,
            pending_limit: WAITING_BYTES_LIMIT,
            pending_peak: 0,
            spare_bytes: 0,
            settling_bytes: 0,
            file: FileState::default(),
            files: 0,
            rejoined: HashSet::new(),
            orphans: HashSet::new(),
            orphan_rows: 0,
            rows_read: 0,
            rows_skipped: 0,
            out_of_range: 0,
            repeated: 0,
            already_held: 0,
            isolated: 0,
            notes: Vec::new(),
            curves_left_out: 0,
            transient: Held::new(),
            kept: Held::new(),
            kept_bytes: 0,
        }
    }

    /// Read one geophysics file. A file that fails leaves no trace of itself:
    /// the holes it touched get back what they had, so the files around it
    /// still count. `progress` is told the bytes of this file read so far.
    pub(crate) fn read(&mut self, mapping: &CsvDrillFileMapping, input: impl BufRead, cancelled: &dyn Fn() -> bool, progress: &mut dyn FnMut(u64)) -> Result<(), CsvDrillError> {
        let read = self.read_file(mapping, input, cancelled, progress);
        if read.is_err() {
            self.abandon_file();
        }
        read
    }

    fn read_file(&mut self, mapping: &CsvDrillFileMapping, input: impl BufRead, cancelled: &dyn Fn() -> bool, progress: &mut dyn FnMut(u64)) -> Result<(), CsvDrillError> {
        let name = mapping.path.display().to_string();
        let mut records = Records::new(input)?;
        if !records.next()? {
            return Err(CsvDrillError::Invalid(tr_format!(literal = "%file% is empty", file = name.clone())));
        }
        let width = records.len();
        if mapping.columns.len() != width {
            return Err(CsvDrillError::Invalid(tr_format!(
                literal = "%file% mapping has %mapped% columns, CSV has %found%",
                file = name.clone(),
                mapped = mapping.columns.len().to_string(),
                found = width.to_string()
            )));
        }
        let column = |role: CsvDrillColumnRole| mapping.columns.iter().position(|mapped| *mapped == role);
        let (Some(dhid_column), Some(depth_column)) = (column(CsvDrillColumnRole::Dhid), column(CsvDrillColumnRole::Depth)) else {
            return Err(CsvDrillError::Invalid(tr_format!(
                literal = "%file% requires one DHID and one depth column",
                file = name.clone()
            )));
        };
        let curves = mapping
            .columns
            .iter()
            .enumerate()
            .filter_map(|(index, role)| curve_kind(role).map(|kind| (kind, index)))
            .collect::<Vec<_>>();
        if curves.is_empty() {
            return Err(CsvDrillError::Invalid(tr_format!(literal = "%file% maps no gamma or density column", file = name.clone())));
        }
        self.file = FileState {
            started: true,
            index: self.files,
            name: name.clone(),
            ..FileState::default()
        };
        self.files += 1;
        for &(kind, index) in &curves {
            self.file.sources[kind.index()] = cell_text(records.cell(index)).trim().to_owned();
        }

        let mut last_progress = 0u64;
        while records.next()? {
            if records.bytes_read() >= last_progress + PROGRESS_STRIDE {
                last_progress = records.bytes_read();
                progress(last_progress);
            }
            if records.is_blank() {
                continue;
            }
            self.file.rows += 1;
            if self.file.rows.is_multiple_of(CANCEL_CHECK_ROWS) && cancelled() {
                return Err(CsvDrillError::Cancelled);
            }
            let (dhid, depth) = match gate(&records, &name, width, dhid_column, depth_column) {
                Ok(gate) => gate,
                Err(reason) => {
                    self.file.skipped += 1;
                    if self.file.skipped <= SKIP_REPORT_LIMIT {
                        userspace_warn!("{}", tr_format!(literal = "Skipped a row: %reason%", reason = reason));
                    }
                    continue;
                }
            };
            if self.run.as_ref().is_none_or(|run| run.dhid != dhid) {
                self.end_run()?;
                let orphan = !self.known.contains(dhid.as_ref());
                if orphan && !self.file.orphans.contains(dhid.as_ref()) {
                    self.file.orphans.insert(dhid.clone().into_owned());
                }
                self.run = Some(Run { dhid: dhid.into_owned(), orphan });
            }
            if self.run.as_ref().is_some_and(|run| run.orphan) {
                self.file.orphan_rows += 1;
                continue;
            }
            self.samples.depths.push(depth);
            for &(kind, index) in &curves {
                let value = self.file.reading(kind, records.cell(index));
                self.samples.values[kind.index()].push(value);
            }
            if self.samples.depths.len().is_multiple_of(MEMORY_CHECK_ROWS) {
                self.cover_transient()?;
            }
        }
        // A run does not carry over into the next file, whose columns differ.
        self.end_run()?;
        self.join_pending()?;
        progress(records.bytes_read());
        let (rows, skipped) = (self.file.rows, self.file.skipped);

        if skipped > SKIP_REPORT_LIMIT {
            userspace_warn!(
                "{}",
                tr_format!(literal = "%count% rows were skipped in total in %file%", count = skipped.to_string(), file = name.clone())
            );
        }
        // Most of a file failing is a mapping mistake, not dirty data.
        if rows > 0 && skipped * 2 > rows {
            return Err(CsvDrillError::Invalid(tr_format!(
                literal = "%file%: %skipped% of %rows% rows could not be read; the reasons are in the console",
                file = name,
                skipped = skipped.to_string(),
                rows = rows.to_string()
            )));
        }
        self.close_file();
        Ok(())
    }

    /// Transient bytes: the run's samples, wherever they are, the rows
    /// waiting and the batch being built, the reading buffers with about as
    /// much again for [`LogTrace::from_samples`] working on them, and what
    /// the file must be able to put back.
    fn cover_transient(&mut self) -> Result<(), CsvDrillError> {
        let buffers = (self.readings.capacity() + self.run_readings.capacity() + self.joined.capacity()) * size_of::<(f64, f64)>();
        let covered = self.covered.len() * COVERED_ENTRY_BYTES;
        let rows = self.samples.bytes() + self.spare_bytes + self.pending_bytes + self.settling_bytes;
        self.transient.cover(rows + 2 * buffers + covered + self.file.before_bytes)
    }

    /// Close the current run. A hole's first run becomes its traces now; a
    /// run back for a curve the hole holds waits with any others it came
    /// back with, and they join the traces as a batch.
    fn end_run(&mut self) -> Result<(), CsvDrillError> {
        let Some(run) = self.run.take() else {
            return Ok(());
        };
        let mut samples = std::mem::take(&mut self.samples);
        self.spare_bytes = samples.bytes();
        let rows = samples.depths.len();
        let settled = if run.orphan {
            Ok(())
        } else if self.returning(&run.dhid, &samples) {
            let waiting = self.pending.entry(run.dhid.clone()).or_default();
            let before = waiting.bytes();
            waiting.samples.append(&mut samples);
            waiting.runs.push(rows);
            let (waiting_rows, after) = (waiting.samples.depths.len(), waiting.bytes());
            self.pending_bytes = self.pending_bytes + after - before;
            if waiting_rows >= REJOIN_BATCH_ROWS.max(self.held_samples(&run.dhid)) {
                self.join_hole(&run.dhid)
            } else {
                self.limit_waiting()
            }
        } else {
            self.settle_samples(&run.dhid, &samples, &[rows])
        };
        samples.clear();
        self.samples = samples;
        self.spare_bytes = 0;
        settled
    }

    /// Whether a run's hole already holds a curve the run has readings of,
    /// or has rows waiting, which the run must not overtake.
    fn returning(&self, dhid: &str, samples: &Samples) -> bool {
        self.pending.contains_key(dhid)
            || self.built.get(dhid).is_some_and(|traces| {
                LogKind::ALL
                    .iter()
                    .any(|kind| traces[kind.index()].is_some() && samples.values[kind.index()].iter().any(|value| value.is_finite()))
            })
    }

    /// Samples in the hole's longest trace.
    fn held_samples(&self, dhid: &str) -> usize {
        self.built.get(dhid).map_or(0, |traces| traces.iter().flatten().map(LogTrace::len).max().unwrap_or(0))
    }

    /// Keep the waiting rows within their limit, joining the hole with the
    /// most waiting first.
    fn limit_waiting(&mut self) -> Result<(), CsvDrillError> {
        while self.pending_bytes > self.pending_limit {
            let Some(largest) = self.pending.iter().max_by_key(|(_, waiting)| waiting.bytes()).map(|(dhid, _)| dhid.clone()) else {
                break;
            };
            self.join_hole(&largest)?;
        }
        self.pending_peak = self.pending_peak.max(self.pending_bytes);
        self.cover_transient()
    }

    /// Join a hole's waiting rows to its traces.
    fn join_hole(&mut self, dhid: &str) -> Result<(), CsvDrillError> {
        let Some(waiting) = self.pending.remove(dhid) else {
            return Ok(());
        };
        let bytes = waiting.bytes();
        self.pending_bytes -= bytes;
        // Still counted until its traces are built.
        self.settling_bytes = bytes;
        let settled = self.settle_samples(dhid, &waiting.samples, &waiting.runs);
        self.settling_bytes = 0;
        settled
    }

    /// Join every hole's waiting rows, as a file ends.
    fn join_pending(&mut self) -> Result<(), CsvDrillError> {
        let mut holes = self.pending.keys().cloned().collect::<Vec<_>>();
        holes.sort_unstable();
        for dhid in holes {
            self.join_hole(&dhid)?;
        }
        Ok(())
    }

    /// Settle each curve `samples` read into the hole's traces, taking the
    /// runs, of `runs` rows each, in the order they arrived. Readings held
    /// win: each run adds only depths with no reading within half a step,
    /// from the hole's trace or from an earlier run. That extends a trace,
    /// fills its gaps and leaves a repeat pass out, two passes never
    /// interleave, and the result does not depend on where a batch ends.
    fn settle_samples(&mut self, dhid: &str, samples: &Samples, runs: &[usize]) -> Result<(), CsvDrillError> {
        for kind in LogKind::ALL {
            let values = &samples.values[kind.index()];
            if values.is_empty() {
                continue;
            }
            self.readings.clear();
            self.covered.clear();
            let held = self.built.get(dhid).and_then(|traces| traces[kind.index()].as_ref());
            // Half the held trace's step counts as one depth; with no step to
            // go by, the first run to have one sets it.
            let mut step = held.filter(|trace| trace.len() > 1).map(LogTrace::step);
            let mut offered = 0;
            let mut start = 0;
            for (index, &rows) in runs.iter().enumerate() {
                let run = start..start + rows;
                start += rows;
                self.run_readings.clear();
                self.run_readings.extend(
                    samples.depths[run.clone()]
                        .iter()
                        .copied()
                        .zip(values[run].iter().copied())
                        .filter(|(_, value)| value.is_finite()),
                );
                if self.run_readings.is_empty() {
                    continue;
                }
                offered += self.run_readings.len();
                let reach = step.map_or(SAME_DEPTH, |step| 0.5 * step + SAME_DEPTH);
                let covered = &self.covered;
                self.run_readings
                    .retain(|(depth, _)| !held.is_some_and(|trace| holds_reading(trace, *depth)) && !covers(covered, *depth, reach));
                // Only a later run of the batch is checked against this one.
                if index + 1 < runs.len() {
                    // Stable, so at one depth the reading read first stays
                    // first.
                    self.run_readings.sort_by(|a, b| a.0.total_cmp(&b.0));
                    if step.is_none() && self.run_readings.len() > 1 {
                        step = median_spacing(&self.run_readings);
                    }
                    self.covered.extend(self.run_readings.iter().map(|(depth, _)| depth_key(*depth)));
                }
                self.readings.extend_from_slice(&self.run_readings);
            }
            if held.is_some() && offered > 0 {
                self.file.rejoined.insert(dhid.to_owned());
            }
            self.file.already_held[kind.index()] += offered - self.readings.len();
            if self.readings.is_empty() {
                continue;
            }
            self.file.repeated += settle_depths(&mut self.readings);
            self.cover_transient()?;
            self.settle_curve(dhid, kind)?;
        }
        self.covered.clear();
        Ok(())
    }

    /// Settle one curve's checked readings, in `self.readings`, into the
    /// hole's trace.
    fn settle_curve(&mut self, dhid: &str, kind: LogKind) -> Result<(), CsvDrillError> {
        if !self.built.contains_key(dhid) {
            self.built.insert(dhid.to_owned(), Default::default());
        }
        let traces = self.built.get_mut(dhid).expect("inserted above");
        if !self.file.before.contains_key(&(dhid.to_owned(), kind)) {
            let before = traces[kind.index()].clone();
            self.file.before_bytes += before.as_ref().map_or(0, LogTrace::memory_bytes);
            self.file.before.insert((dhid.to_owned(), kind), before);
        }
        let previous = traces[kind.index()].take();
        let previous_bytes = previous.as_ref().map_or(0, LogTrace::memory_bytes);
        let label = &self.file.sources[kind.index()];
        let mut error = None;
        let settled = match previous {
            None => match build(kind, &self.readings) {
                Ok((trace, isolated)) => {
                    self.file.isolated[kind.index()] += isolated;
                    Some(trace.with_source(label.clone()))
                }
                Err(refused) => {
                    error = Some(refused);
                    None
                }
            },
            Some(held) => {
                // The held samples are read back off the stored grid, which
                // is lossless at an unchanged step: each is an exact multiple
                // of the curve's resolution at an exact grid depth, so it
                // quantises back to itself.
                self.joined.clear();
                self.joined.extend(
                    held.samples(held.start_depth(), held.end_depth())
                        .filter_map(|(depth, value)| value.map(|value| (depth, f64::from(value)))),
                );
                self.joined.extend_from_slice(&self.readings);
                self.file.repeated += settle_depths(&mut self.joined);
                match build(kind, &self.joined) {
                    Ok((trace, isolated)) => {
                        self.file.isolated[kind.index()] += isolated;
                        let source = joined_label(held.source(), label);
                        Some(trace.with_source(source))
                    }
                    Err(refused) => {
                        error = Some(refused);
                        Some(held)
                    }
                }
            }
        };
        self.notes.extend(error.map(|error| Note {
            file: self.file.index,
            dhid: dhid.to_owned(),
            kind,
            error,
        }));
        self.kept_bytes = self.kept_bytes - previous_bytes + settled.as_ref().map_or(0, LogTrace::memory_bytes);
        self.built.get_mut(dhid).expect("inserted above")[kind.index()] = settled;
        self.kept.cover(self.kept_bytes)
    }

    /// End a file read to its end: its counts join the import's, and a
    /// density curve whose median is not a g/cc value comes back out of every
    /// hole the file touched, with a warning. Units are the exporting
    /// database's to set, so nothing is converted.
    fn close_file(&mut self) {
        let file = std::mem::take(&mut self.file);
        self.rows_read += file.rows;
        self.rows_skipped += file.skipped;
        self.orphan_rows += file.orphan_rows;
        self.orphans.extend(file.orphans);
        self.rejoined.extend(file.rejoined);
        self.repeated += file.repeated;
        let mut left_out = [false; 3];
        for kind in LogKind::ALL {
            let [below, within, above] = file.votes[kind.index()];
            let total = below + within + above;
            let side = if below * 2 > total {
                Some(tr!(literal = "below 0.5"))
            } else if above * 2 > total {
                Some(tr!(literal = "above 5"))
            } else {
                None
            };
            let Some(side) = side else {
                self.out_of_range += file.out_of_range[kind.index()];
                self.already_held += file.already_held[kind.index()];
                self.isolated += file.isolated[kind.index()];
                continue;
            };
            left_out[kind.index()] = true;
            self.curves_left_out += 1;
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "%curve% in %file% was left out: most of its readings are %side%, so its median is outside 0.5 to 5 g/cc and its unit looks wrong (g/cc expected). Incline converts no units; correct the export and import it again",
                    curve = curve_name(kind),
                    file = file.name.clone(),
                    side = side
                )
            );
        }
        if left_out.contains(&true) {
            self.restore(file.index, file.before, left_out);
        }
    }

    /// Take a file that failed back out: every hole it touched gets back
    /// what it had, and nothing it counted or noted stays.
    fn abandon_file(&mut self) {
        self.run = None;
        self.samples.clear();
        self.pending.clear();
        self.pending_bytes = 0;
        self.spare_bytes = 0;
        self.settling_bytes = 0;
        let file = std::mem::take(&mut self.file);
        // A file that failed in its header never touched a hole; the state
        // left behind is an empty one whose index is not this file's.
        if file.started {
            self.restore(file.index, file.before, [true; 3]);
        }
    }

    /// Put back the traces a file found, for the curves in `curves`, and
    /// drop its notes on them.
    fn restore(&mut self, file: usize, before: HashMap<(String, LogKind), Option<LogTrace>>, curves: [bool; 3]) {
        for ((dhid, kind), before) in before {
            if !curves[kind.index()] {
                continue;
            }
            let Some(traces) = self.built.get_mut(&dhid) else {
                continue;
            };
            let added = before.as_ref().map_or(0, LogTrace::memory_bytes);
            let removed = std::mem::replace(&mut traces[kind.index()], before);
            self.kept_bytes = self.kept_bytes + added - removed.as_ref().map_or(0, LogTrace::memory_bytes);
            // A hole only this file gave traces to is not one seen before.
            if traces.iter().all(Option::is_none) {
                self.built.remove(&dhid);
            }
        }
        self.notes.retain(|note| note.file != file || !curves[note.kind.index()]);
    }

    /// Report what was set aside and hand over the traces.
    pub(crate) fn finish(mut self) -> GeophysicsImport {
        log::debug!("geophysics import: waiting rows peaked at {} bytes", self.pending_peak);
        if !self.rejoined.is_empty() {
            let mut holes = self.rejoined.iter().map(String::as_str).collect::<Vec<_>>();
            holes.sort_unstable();
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "Geophysics for %count% hole(s) came in more than one run of the same curve, not grouped by hole or spread over files; each later run added only depths its hole had no reading at: %holes%",
                    count = holes.len().to_string(),
                    holes = hole_list(holes.into_iter())
                )
            );
        }
        self.notes.sort_by(|a, b| a.dhid.cmp(&b.dhid).then(a.kind.cmp(&b.kind)).then(a.file.cmp(&b.file)));
        for note in self.notes.iter().take(SKIP_REPORT_LIMIT) {
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "A run of %hole% %curve% was not kept (%reason%)",
                    hole = note.dhid.clone(),
                    curve = curve_name(note.kind),
                    reason = trace_error(&note.error)
                )
            );
        }
        if self.notes.len() > SKIP_REPORT_LIMIT {
            userspace_warn!(
                "{}",
                tr_format!(literal = "%runs% run(s) of geophysics were not kept in all", runs = self.notes.len().to_string())
            );
        }
        if self.repeated > 0 {
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "%count% geophysics reading(s) repeated a depth read earlier in the same file for that hole; the first was kept",
                    count = self.repeated.to_string()
                )
            );
        }
        if !self.orphans.is_empty() {
            let mut orphans = self.orphans.iter().map(String::as_str).collect::<Vec<_>>();
            orphans.sort_unstable();
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "%rows% geophysics row(s) for %count% hole(s) the bundle's geometry does not define were not kept: %holes%",
                    rows = self.orphan_rows.to_string(),
                    count = orphans.len().to_string(),
                    holes = hole_list(orphans.into_iter())
                )
            );
        }

        let mut holes = self
            .built
            .into_iter()
            .map(|(dhid, traces)| (dhid, traces.into_iter().flatten().collect::<Vec<_>>()))
            .filter(|(_, traces)| !traces.is_empty())
            .collect::<Vec<_>>();
        holes.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        GeophysicsImport {
            holes,
            rows_read: self.rows_read,
            rows_skipped: self.rows_skipped,
            orphan_rows: self.orphan_rows,
            orphan_holes: self.orphans.len(),
            out_of_range: self.out_of_range,
            already_held: self.already_held,
            isolated: self.isolated,
            runs_not_kept: self.notes.len(),
            curves_left_out: self.curves_left_out,
            reservation: self.kept.settle(self.kept_bytes),
        }
    }
}

/// A row's hole id and depth, or the reason it is refused.
fn gate<'r, R: BufRead>(records: &'r Records<R>, file: &str, width: usize, dhid_column: usize, depth_column: usize) -> Result<(Cow<'r, str>, f64), String> {
    let row = records.record_number().to_string();
    if records.len() != width {
        return Err(tr_format!(
            literal = "%file% row %row% has %found% columns; expected %expected%",
            file = file.to_owned(),
            row = row,
            found = records.len().to_string(),
            expected = width.to_string()
        ));
    }
    let dhid = match cell_text(records.cell(dhid_column)) {
        Cow::Borrowed(text) => Cow::Borrowed(text.trim()),
        Cow::Owned(text) => Cow::Owned(text.trim().to_owned()),
    };
    if dhid.is_empty() {
        return Err(tr_format!(literal = "%file% row %row% has a blank hole id", file = file.to_owned(), row = row));
    }
    match cell_number(records.cell(depth_column)) {
        None => Err(tr_format!(literal = "%file% row %row% has no readable depth", file = file.to_owned(), row = row)),
        Some(depth) if depth < 0.0 => Err(tr_format!(literal = "%file% row %row% has a negative depth", file = file.to_owned(), row = row)),
        Some(depth) => Ok((dhid, depth)),
    }
}

/// A trace from readings, and how many were left out as isolated.
fn build(kind: LogKind, readings: &[(f64, f64)]) -> Result<(LogTrace, usize), TraceError> {
    let (depths, values): (Vec<f64>, Vec<f64>) = readings.iter().copied().unzip();
    LogTrace::from_samples(kind, &depths, &values).map(|(trace, stats)| (trace, stats.isolated))
}

/// Whether `trace` has a reading within half a step of `depth`. A trace of
/// one sample has no step to speak of, so only its own depth counts.
fn holds_reading(trace: &LogTrace, depth: f64) -> bool {
    let (start, step, count) = (trace.start_depth(), trace.step(), trace.len());
    let reach = if count > 1 { 0.5 * step + SAME_DEPTH } else { SAME_DEPTH };
    if depth < start - reach || depth > trace.end_depth() + reach {
        return false;
    }
    let at = ((depth - start) / step).max(0.0);
    [at.floor(), at.ceil()]
        .into_iter()
        .map(|index| (index as usize).min(count - 1))
        .any(|index| (start + index as f64 * step - depth).abs() <= reach && trace.sample(index).is_some())
}

/// A depth as an ordered set key. Depths are never negative, and a
/// non-negative float's bits sort as the float does.
fn depth_key(depth: f64) -> u64 {
    depth.abs().to_bits()
}

/// Whether `covered` holds a depth within `reach` of `depth`.
fn covers(covered: &BTreeSet<u64>, depth: f64, reach: f64) -> bool {
    let (low, high) = ((depth - reach).max(0.0), depth + reach);
    covered.range(depth_key(low)..=depth_key(high)).next().is_some()
}

/// The median spacing of readings sorted by depth, if any is positive.
fn median_spacing(readings: &[(f64, f64)]) -> Option<f64> {
    let mut spacing = readings.windows(2).map(|pair| pair[1].0 - pair[0].0).filter(|gap| *gap > SAME_DEPTH).collect::<Vec<_>>();
    if spacing.is_empty() {
        return None;
    }
    let middle = spacing.len() / 2;
    Some(*spacing.select_nth_unstable_by(middle, f64::total_cmp).1)
}

/// The column a joined trace names: both, where two files named it apart.
fn joined_label(held: &str, new: &str) -> String {
    if held.is_empty() || held.split(", ").any(|part| part == new) {
        new.to_owned()
    } else {
        format!("{held}, {new}")
    }
}

/// Sort readings by depth and fold those at one depth into the first,
/// returning how many were folded away.
fn settle_depths(rows: &mut Vec<(f64, f64)>) -> usize {
    if !rows.is_sorted_by(|a, b| a.0 <= b.0) {
        // Stable, so the reading read first stays first.
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let before = rows.len();
    rows.dedup_by(|later, earlier| later.0 - earlier.0 <= SAME_DEPTH);
    before - rows.len()
}

/// A cell's text. Bytes that are not UTF-8 are repaired as the table reader
/// repairs them, so a damaged hole id reads the same in every file.
pub(super) fn cell_text(cell: &[u8]) -> Cow<'_, str> {
    match std::str::from_utf8(cell) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => Cow::Owned(
            csv_drill_hole::repair_utf8(cell, cell.len())
                .and_then(|(bytes, _)| String::from_utf8(bytes).ok())
                .unwrap_or_default(),
        ),
    }
}

fn cell_number(cell: &[u8]) -> Option<f64> {
    std::str::from_utf8(cell).ok().and_then(csv_drill_hole::finite_number)
}

fn curve_name(kind: LogKind) -> String {
    match kind {
        LogKind::Gamma => tr!(literal = "gamma"),
        LogKind::LongDensity => tr!(literal = "long-spaced density"),
        LogKind::ShortDensity => tr!(literal = "short-spaced density"),
    }
}

fn trace_error(error: &TraceError) -> String {
    match error {
        TraceError::NoSamples => tr!(literal = "no readings"),
        TraceError::BadStep => tr!(literal = "no usable depth step"),
        TraceError::SpanTooLong { rows, requested } => tr_format!(
            literal = "%rows% readings would need %samples% samples",
            rows = rows.to_string(),
            samples = requested.to_string()
        ),
    }
}

/// `a, b, c (+N more)` from a list of hole ids.
fn hole_list<'h>(holes: impl ExactSizeIterator<Item = &'h str>) -> String {
    let total = holes.len();
    let mut text = holes.take(HOLE_LIST_LIMIT).collect::<Vec<_>>().join(", ");
    if total > HOLE_LIST_LIMIT {
        text.push_str(&tr_format!(literal = " (+%count% more)", count = (total - HOLE_LIST_LIMIT).to_string()));
    }
    text
}

/// CSV records read one at a time from a byte stream, as the table reader
/// reads them: a quote opens a cell only at its start, a doubled quote in
/// one is a quote, and a CR outside quotes is dropped. A cell is a slice of
/// the line read, or of an unquoted copy when the record has quotes in it.
pub(super) struct Records<R> {
    reader: R,
    /// The record's bytes so far; more than one line when a quoted cell
    /// runs over a line break.
    line: Vec<u8>,
    unquoted: Vec<u8>,
    cells: Vec<Range<usize>>,
    /// The record has quotes, so its cells index `unquoted`.
    quoted: bool,
    /// Bytes of `line` scanned: a record continued over a line break is
    /// scanned on from here, not from its start.
    scanned: usize,
    /// Inside a quoted cell at `scanned`.
    open: bool,
    /// Start in `unquoted` of the cell being scanned.
    cell_start: usize,
    bytes: u64,
    record: usize,
}

impl<R: BufRead> Records<R> {
    pub(super) fn new(mut reader: R) -> Result<Self, CsvDrillError> {
        // Wide text is refused by name, as the table files are.
        let head = reader.fill_buf()?;
        csv_drill_hole::check_encoding(head)?;
        Ok(Self {
            reader,
            line: Vec::new(),
            unquoted: Vec::new(),
            cells: Vec::new(),
            quoted: false,
            scanned: 0,
            open: false,
            cell_start: 0,
            bytes: 0,
            record: 0,
        })
    }

    /// Read the next record; false at the end of the stream.
    pub(super) fn next(&mut self) -> Result<bool, CsvDrillError> {
        self.line.clear();
        self.unquoted.clear();
        self.cells.clear();
        self.quoted = false;
        self.scanned = 0;
        self.open = false;
        self.cell_start = 0;
        loop {
            let read = self.read_line()?;
            self.bytes += read as u64;
            if read == 0 && self.line.is_empty() {
                return Ok(false);
            }
            if self.record == 0 && self.scanned == 0 && self.line.starts_with(&[0xEF, 0xBB, 0xBF]) {
                self.line.drain(..3);
            }
            if self.scan(read == 0) {
                self.record += 1;
                return Ok(true);
            }
            // A quoted cell runs on over the line break.
            if read == 0 {
                return Err(CsvDrillError::Invalid(tr!(literal = "CSV has an unterminated quoted field")));
            }
        }
    }

    /// Append one line, through its line break, to the record; the bytes
    /// read, none at the end of the stream. A record past
    /// [`MAX_RECORD_BYTES`] is refused as it grows.
    fn read_line(&mut self) -> Result<usize, CsvDrillError> {
        let mut read = 0;
        loop {
            let available = match self.reader.fill_buf() {
                Ok(available) => available,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            };
            if available.is_empty() {
                return Ok(read);
            }
            let (take, ended) = match available.iter().position(|byte| *byte == b'\n') {
                Some(at) => (at + 1, true),
                None => (available.len(), false),
            };
            if self.line.len() + take > MAX_RECORD_BYTES {
                return Err(CsvDrillError::Invalid(tr_format!(
                    literal = "CSV has a record longer than %limit% MiB: the file has no line breaks where a CSV has them, or is not text",
                    limit = (MAX_RECORD_BYTES / (1024 * 1024)).to_string()
                )));
            }
            self.line.extend_from_slice(&available[..take]);
            self.reader.consume(take);
            read += take;
            if ended {
                return Ok(read);
            }
        }
    }

    /// Scan on from `scanned`; true once the record is complete, false if a
    /// quoted cell is still open where the bytes read so far end.
    fn scan(&mut self, at_end: bool) -> bool {
        if !self.quoted {
            // Only a record's first line gets here: a record runs on only
            // from inside quotes. Without a quote its cells are the line's.
            if !self.line.contains(&b'"') {
                let mut end = self.line.len();
                if self.line[..end].ends_with(b"\n") {
                    end -= 1;
                }
                if self.line[..end].ends_with(b"\r") {
                    end -= 1;
                }
                let mut start = 0;
                for (index, byte) in self.line[..end].iter().enumerate() {
                    if *byte == b',' {
                        self.cells.push(start..index);
                        start = index + 1;
                    }
                }
                self.cells.push(start..end);
                return true;
            }
            self.quoted = true;
        }
        let line = &self.line;
        let mut index = self.scanned;
        while index < line.len() {
            let byte = line[index];
            if self.open {
                if byte == b'"' {
                    if line.get(index + 1) == Some(&b'"') {
                        self.unquoted.push(b'"');
                        index += 1;
                    } else {
                        self.open = false;
                    }
                } else {
                    self.unquoted.push(byte);
                }
            } else {
                match byte {
                    b'"' if self.unquoted.len() == self.cell_start => self.open = true,
                    b',' => {
                        self.cells.push(self.cell_start..self.unquoted.len());
                        self.cell_start = self.unquoted.len();
                    }
                    b'\n' => {
                        self.cells.push(self.cell_start..self.unquoted.len());
                        self.scanned = index + 1;
                        return true;
                    }
                    b'\r' => {}
                    _ => self.unquoted.push(byte),
                }
            }
            index += 1;
        }
        self.scanned = index;
        if at_end && !self.open {
            self.cells.push(self.cell_start..self.unquoted.len());
            return true;
        }
        false
    }

    pub(super) fn len(&self) -> usize {
        self.cells.len()
    }

    pub(super) fn cell(&self, index: usize) -> &[u8] {
        let range = self.cells[index].clone();
        if self.quoted { &self.unquoted[range] } else { &self.line[range] }
    }

    fn is_blank(&self) -> bool {
        (0..self.len()).all(|index| self.cell(index).trim_ascii().is_empty())
    }

    /// The record's row number as a spreadsheet shows it, the header being 1.
    fn record_number(&self) -> usize {
        self.record
    }

    fn bytes_read(&self) -> u64 {
        self.bytes
    }
}
