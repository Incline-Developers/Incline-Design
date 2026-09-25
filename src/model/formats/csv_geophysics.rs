//! Downhole geophysics from a CSV file: one row per sample, a hole id, a
//! depth and curve columns, as a database exports the logs once they are
//! cleaned. Raw instrument formats are not read here or anywhere, and no
//! unit is converted: depth is metres, gamma API and density g/cc.
//!
//! A file runs to gigabytes, so it is never held. Linking reads it through
//! once, a row at a time, for where each hole's rows are ([`index_file`]);
//! showing a hole reads those rows alone ([`read_hole`]). A file not
//! grouped by hole, one sorted by depth say, is refused as it is linked:
//! its index would outgrow what a project can save and reopen. A hole that
//! comes back later, for a repeat pass, keeps what it holds: the later rows
//! add only depths it has no reading at, joined in batches.

use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap, HashSet},
    io::BufRead,
    ops::Range,
    path::PathBuf,
};

// Not on the web, where rayon's pool is the job queue: a thread waiting on
// its share of a parallel loop may take up a queued job instead, and an
// index pass there blocks for minutes on the page feeding it.
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

use crate::{
    i18n::{tr, tr_format},
    model::{
        formats::csv_drill_hole::{self, CsvDrillColumnRole, CsvDrillError, CsvDrillFileRole, SKIP_REPORT_LIMIT},
        geophysics::{ColumnRole, ColumnSet, FileIdentity, GeophysicsLink, HoleLogs, HoleRuns, LinkedColumn, LinkedFile, LogKind, LogTrace, TraceError},
    },
    userspace_log, userspace_warn,
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

/// Most bytes of rows waiting to be joined before they are joined early.
const WAITING_BYTES_LIMIT: usize = 64 * 1024 * 1024;

/// Rows between asking whether the index pass was cancelled.
const CANCEL_CHECK_ROWS: usize = 4096;

/// Bytes read between progress reports.
const PROGRESS_STRIDE: u64 = 4 * 1024 * 1024;

/// Runs past each hole's first that any file may have, however few its
/// holes.
const SPLIT_RUN_FLOOR: usize = 1024;

/// Runs past its first that each hole adds to what a file may have.
const SPLIT_RUNS_PER_HOLE: usize = 4;

/// Most runs past each hole's first a file may have. The index is saved
/// with the project, which does not reopen past 64 MiB of index; this many
/// runs are a few MiB of it.
const MAX_SPLIT_RUNS: usize = 50_000;

/// Two readings this close are one depth.
const SAME_DEPTH: f64 = 1.0e-6;

/// Where a density curve's median has to lie for its unit to be g/cc.
const PLAUSIBLE_DENSITY: std::ops::RangeInclusive<f64> = 0.5..=5.0;

/// The no-reading values exports write. A drawn curve is never negative, so
/// there every negative is one; a curve of unknown unit may be.
const NO_READING: [f64; 3] = [-999.25, -999.0, -9999.0];

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

/// A linked file's index, and what reading it through found.
pub(crate) struct IndexedFile {
    pub(crate) file: LinkedFile,
    pub(crate) report: IndexReport,
}

/// What an index pass counted, for the console once the file is linked.
pub(crate) struct IndexReport {
    rows: usize,
    skipped: usize,
    orphan_rows: usize,
    orphans: Vec<String>,
    /// Holes whose rows came in more than one run.
    split: Vec<String>,
    /// Density curves left out, and where most of their readings lay.
    left_out: Vec<(String, String)>,
}

impl IndexReport {
    /// Say what `file` links, and what was set aside.
    pub(crate) fn log(&self, file: &LinkedFile) {
        let curves = file
            .columns
            .iter()
            .filter(|column| matches!(column.role, ColumnRole::Curve { .. }))
            .map(|column| column.header.as_str())
            .collect::<Vec<_>>();
        userspace_log!(
            "{}",
            tr_format!(
                literal = "Linked downhole geophysics from %file%: %holes% hole(s), curves %curves%; %rows% row(s) read, %skipped% skipped. The readings stay in the file and are read a hole at a time",
                file = file.identity.name.clone(),
                holes = file.holes.len(),
                curves = curves.join(", "),
                rows = self.rows,
                skipped = self.skipped
            )
        );
        if !self.split.is_empty() {
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "Geophysics for %count% hole(s) comes in more than one run, not grouped by hole; each later run adds only depths its hole has no reading at: %holes%",
                    count = self.split.len(),
                    holes = hole_list(self.split.iter().map(String::as_str))
                )
            );
        }
        if !self.orphans.is_empty() {
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "%rows% geophysics row(s) for %count% hole(s) the dataset does not define are not linked: %holes%",
                    rows = self.orphan_rows,
                    count = self.orphans.len(),
                    holes = hole_list(self.orphans.iter().map(String::as_str))
                )
            );
        }
        for (curve, side) in &self.left_out {
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "%curve% in %file% was left out: most of its readings are %side%, so its median is outside 0.5 to 5 g/cc and its unit looks wrong (g/cc expected). Incline converts no units; correct the export and link it again",
                    curve = curve.clone(),
                    file = file.identity.name.clone(),
                    side = side.clone()
                )
            );
        }
    }
}

/// Where an index pass takes the file's column roles from.
#[derive(Clone, Copy)]
pub(crate) enum ColumnSource<'a> {
    /// As an import mapped them.
    Mapped(&'a [CsvDrillColumnRole]),
    /// As an earlier index of the file found them, while its header is the
    /// same; guessed from the header when it is not.
    Previous(&'a [LinkedColumn]),
    Guessed,
}

impl ColumnSource<'_> {
    fn roles(self, headers: &[String]) -> Vec<CsvDrillColumnRole> {
        match self {
            ColumnSource::Mapped(roles) => roles.to_vec(),
            ColumnSource::Previous(columns) if columns.iter().map(|column| &column.header).eq(headers) => columns
                .iter()
                .map(|column| match column.role {
                    ColumnRole::Dhid => CsvDrillColumnRole::Dhid,
                    ColumnRole::Depth => CsvDrillColumnRole::Depth,
                    ColumnRole::Curve { kind: Some(kind) } | ColumnRole::LeftOut { kind } => kind_role(kind),
                    ColumnRole::Curve { kind: None } | ColumnRole::Skipped => CsvDrillColumnRole::Ignore,
                })
                .collect(),
            ColumnSource::Previous(_) | ColumnSource::Guessed => csv_drill_hole::default_columns(CsvDrillFileRole::Geophysics, headers),
        }
    }
}

fn kind_role(kind: LogKind) -> CsvDrillColumnRole {
    match kind {
        LogKind::Gamma => CsvDrillColumnRole::Gamma,
        LogKind::LongDensity => CsvDrillColumnRole::LongDensity,
        LogKind::ShortDensity => CsvDrillColumnRole::ShortDensity,
    }
}

/// The run of rows being indexed: consecutive rows for one hole.
struct RunScan {
    dhid: String,
    /// A hole the dataset does not define: its rows are counted, not
    /// indexed.
    orphan: bool,
    bytes: [u64; 2],
    depths: [f64; 2],
    /// Columns with a reading in the run, as a read takes a cell.
    curves: ColumnSet,
    /// Depths each drawn curve has readings at, by [`LogKind::index`].
    drawn: [[f64; 2]; 3],
}

/// No depths yet.
const NO_DEPTHS: [f64; 2] = [f64::INFINITY, f64::NEG_INFINITY];

fn widen([top, bottom]: [f64; 2], [from, to]: [f64; 2]) -> [f64; 2] {
    [top.min(from), bottom.max(to)]
}

impl RunScan {
    /// Store the run under `holes`, unless it is an orphan's. Returns
    /// whether a known hole's run was stored.
    fn close(self, holes: &mut HashMap<String, (HoleRuns, [[f64; 2]; 3])>) -> bool {
        if self.orphan {
            return false;
        }
        let (hole, drawn) = holes.entry(self.dhid).or_insert_with_key(|dhid| {
            let hole = HoleRuns {
                dhid: dhid.clone(),
                runs: Vec::new(),
                depths: NO_DEPTHS,
                curves: ColumnSet::default(),
            };
            (hole, [NO_DEPTHS; 3])
        });
        hole.runs.push(self.bytes);
        hole.depths = widen(hole.depths, self.depths);
        hole.curves.union(&self.curves);
        for (held, run) in drawn.iter_mut().zip(self.drawn) {
            *held = widen(*held, run);
        }
        true
    }
}

/// Close `ended`'s run, then refuse the file `name` once its holes have
/// more runs past their first than it may: a file grouped by hole never
/// does, one sorted by depth soon does, long before it is read through.
fn close_run(ended: RunScan, holes: &mut HashMap<String, (HoleRuns, [[f64; 2]; 3])>, known_runs: &mut usize, name: &str) -> Result<(), CsvDrillError> {
    if !ended.close(holes) {
        return Ok(());
    }
    *known_runs += 1;
    let seen = holes.len();
    let split = *known_runs - seen;
    let allowed = (SPLIT_RUN_FLOOR + SPLIT_RUNS_PER_HOLE * seen).min(MAX_SPLIT_RUNS);
    if split > allowed {
        return Err(CsvDrillError::Invalid(tr_format!(
            literal = "%file% is not grouped by hole: its holes' rows are split across too many runs. Sort it by hole id, then depth, and link it again",
            file = name.to_owned()
        )));
    }
    Ok(())
}

/// Read buffer for a file indexed from disk.
#[cfg(not(target_arch = "wasm32"))]
const INDEX_BUFFER_BYTES: usize = 1024 * 1024;

/// Open the file at `path` for [`index_file`]: its identity, then a reader.
/// A path that is not UTF-8 is refused, since the link that saves it is
/// text.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn open_for_index(path: &std::path::Path) -> Result<(FileIdentity, std::io::BufReader<std::fs::File>), CsvDrillError> {
    if path.to_str().is_none() {
        return Err(CsvDrillError::Invalid(tr!(
            literal = "the file's path is not valid UTF-8, which a project cannot save: rename the file or its folder and link it again"
        )));
    }
    let identity = FileIdentity::of_path(path)?;
    Ok((identity, std::io::BufReader::with_capacity(INDEX_BUFFER_BYTES, std::fs::File::open(path)?)))
}

/// Read a geophysics file through, never holding it, for where the rows of
/// each `known` hole are. A column not the hole id, the depth or a drawn
/// curve is a curve if most of its cells are numbers. A file most of whose
/// rows cannot be read is refused, as is a density curve whose unit looks
/// wrong.
pub(crate) fn index_file(
    path: PathBuf,
    identity: FileIdentity,
    columns: ColumnSource<'_>,
    known: &HashSet<String>,
    input: impl BufRead,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64),
) -> Result<IndexedFile, CsvDrillError> {
    let name = identity.name.clone();
    let mut records = Records::new(input)?;
    if !records.next()? {
        return Err(CsvDrillError::Invalid(tr_format!(literal = "%file% is empty", file = name.clone())));
    }
    let headers = (0..records.len()).map(|index| cell_text(records.cell(index)).trim().to_owned()).collect::<Vec<_>>();
    let roles = columns.roles(&headers);
    let width = headers.len();
    if roles.len() != width {
        return Err(CsvDrillError::Invalid(tr_format!(
            literal = "%file% mapping has %mapped% columns, CSV has %found%",
            file = name.clone(),
            mapped = roles.len().to_string(),
            found = width.to_string()
        )));
    }
    let column = |role: CsvDrillColumnRole| roles.iter().position(|mapped| *mapped == role);
    let (Some(dhid_column), Some(depth_column)) = (column(CsvDrillColumnRole::Dhid), column(CsvDrillColumnRole::Depth)) else {
        return Err(CsvDrillError::Invalid(tr_format!(
            literal = "%file% requires one DHID and one depth column",
            file = name.clone()
        )));
    };
    let kinds = roles.iter().map(curve_kind).collect::<Vec<_>>();
    let (mut numbers, mut texts) = (vec![0usize; width], vec![0usize; width]);
    // Density readings below, within and above the plausible g/cc range.
    let mut votes = [[0usize; 3]; 3];
    let mut holes = HashMap::new();
    let mut known_runs = 0usize;
    let mut run: Option<RunScan> = None;
    let (mut rows, mut skipped, mut orphan_rows) = (0usize, 0usize, 0usize);
    let mut orphans = HashSet::new();
    let mut last_progress = 0u64;
    let mut start = records.bytes_read();
    while records.next()? {
        let row = [start, records.bytes_read()];
        start = row[1];
        if row[1] >= last_progress + PROGRESS_STRIDE {
            last_progress = row[1];
            progress(last_progress);
        }
        if records.is_blank() {
            continue;
        }
        rows += 1;
        if rows.is_multiple_of(CANCEL_CHECK_ROWS) && cancelled() {
            return Err(CsvDrillError::Cancelled);
        }
        let (dhid, depth) = match gate(&records, &name, width, dhid_column, depth_column) {
            Ok(gate) => gate,
            Err(reason) => {
                skipped += 1;
                if skipped <= SKIP_REPORT_LIMIT {
                    userspace_warn!("{}", tr_format!(literal = "Skipped a row: %reason%", reason = reason));
                }
                continue;
            }
        };
        if run.as_ref().is_none_or(|run| run.dhid != dhid) {
            if let Some(ended) = run.take() {
                close_run(ended, &mut holes, &mut known_runs, &name)?;
            }
            let orphan = !known.contains(dhid.as_ref());
            if orphan && !orphans.contains(dhid.as_ref()) {
                orphans.insert(dhid.clone().into_owned());
            }
            run = Some(RunScan {
                dhid: dhid.into_owned(),
                orphan,
                bytes: row,
                depths: [depth, depth],
                curves: ColumnSet::default(),
                drawn: [NO_DEPTHS; 3],
            });
        }
        let Some(run) = run.as_mut() else { continue };
        run.bytes[1] = row[1];
        run.depths = [run.depths[0].min(depth), run.depths[1].max(depth)];
        if run.orphan {
            orphan_rows += 1;
            continue;
        }
        for (index, kind) in kinds.iter().enumerate() {
            if index == dhid_column || index == depth_column {
                continue;
            }
            let cell = records.cell(index);
            let Some(kind) = *kind else {
                if !run.curves.contains(index) && could_be_number(cell) && reading(None, cell).is_finite() {
                    run.curves.insert(index);
                }
                if looks_numeric(cell) {
                    numbers[index] += 1;
                } else if !cell.trim_ascii().is_empty() {
                    texts[index] += 1;
                }
                continue;
            };
            // Every row, for the depths the curve has readings at.
            let value = cell_number(cell);
            // As a read takes it: a negative is no reading and no vote.
            if kind != LogKind::Gamma
                && let Some(value) = value.filter(|value| *value >= 0.0)
            {
                votes[kind.index()][density_side(value)] += 1;
            }
            if value.is_some_and(|value| is_reading(Some(kind), value)) {
                run.curves.insert(index);
                run.drawn[kind.index()] = widen(run.drawn[kind.index()], [depth, depth]);
            }
        }
    }
    if let Some(ended) = run.take() {
        close_run(ended, &mut holes, &mut known_runs, &name)?;
    }
    progress(records.bytes_read());

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
    let mut left_out = Vec::new();
    let columns = headers
        .into_iter()
        .enumerate()
        .map(|(index, header)| {
            let role = match kinds[index] {
                _ if index == dhid_column => ColumnRole::Dhid,
                _ if index == depth_column => ColumnRole::Depth,
                Some(kind) => match unit_side(votes[kind.index()]) {
                    Some(side) => {
                        left_out.push((header.clone(), side));
                        ColumnRole::LeftOut { kind }
                    }
                    None => ColumnRole::Curve { kind: Some(kind) },
                },
                None if numbers[index] > texts[index] => ColumnRole::Curve { kind: None },
                None => ColumnRole::Skipped,
            };
            LinkedColumn { header, role }
        })
        .collect::<Vec<_>>();
    if !columns.iter().any(|column| matches!(column.role, ColumnRole::Curve { .. } | ColumnRole::LeftOut { .. })) {
        return Err(CsvDrillError::Invalid(tr_format!(
            literal = "%file% has no curve: no column besides the hole id and depth holds numbers",
            file = name
        )));
    }
    let mut curves = ColumnSet::default();
    for (index, column) in columns.iter().enumerate() {
        if matches!(column.role, ColumnRole::Curve { .. }) {
            curves.insert(index);
        }
    }
    let kept = columns
        .iter()
        .filter_map(|column| match column.role {
            ColumnRole::Curve { kind: Some(kind) } => Some(kind.index()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut holes = holes
        .into_values()
        .map(|(mut hole, drawn)| {
            // Only what a read takes: text and left-out columns go.
            hole.curves.intersect(&curves);
            let read = kept.iter().map(|&kind| drawn[kind]).fold(NO_DEPTHS, widen);
            if read[0] <= read[1] {
                hole.depths = read;
            }
            hole
        })
        .collect::<Vec<_>>();
    holes.sort_unstable_by(|a, b| a.dhid.cmp(&b.dhid));
    let split = holes.iter().filter(|hole| hole.runs.len() > 1).map(|hole| hole.dhid.clone()).collect();
    let mut orphans = orphans.into_iter().collect::<Vec<_>>();
    orphans.sort_unstable();
    Ok(IndexedFile {
        file: LinkedFile { path, identity, columns, holes },
        report: IndexReport {
            rows,
            skipped,
            orphan_rows,
            orphans,
            split,
            left_out,
        },
    })
}

/// Which side of the plausible g/cc range a density reading lies.
fn density_side(value: f64) -> usize {
    if value < *PLAUSIBLE_DENSITY.start() {
        0
    } else if value > *PLAUSIBLE_DENSITY.end() {
        2
    } else {
        1
    }
}

/// Where most of a density curve's readings lie, when that is outside the
/// plausible range. Units are the exporting database's to set, so such a
/// curve is left out rather than converted.
fn unit_side([below, within, above]: [usize; 3]) -> Option<String> {
    let total = below + within + above;
    if below * 2 > total {
        Some(tr!(literal = "below 0.5"))
    } else if above * 2 > total {
        Some(tr!(literal = "above 5"))
    } else {
        None
    }
}

/// Whether a cell reads as a number, judged by its bytes alone: cheap
/// enough for every cell of a file of billions.
fn looks_numeric(cell: &[u8]) -> bool {
    let cell = cell.trim_ascii();
    cell.iter().any(u8::is_ascii_digit) && cell.iter().all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'+' | b'e' | b'E'))
}

/// Whether a cell could read as a number: a digit, and only bytes a number
/// or the space around one is written with. Spares text a failed parse.
fn could_be_number(cell: &[u8]) -> bool {
    cell.iter()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'+' | b'e' | b'E' | b'\x0B') || byte.is_ascii_whitespace() || !byte.is_ascii())
        && cell.iter().any(u8::is_ascii_digit)
}

/// A curve cell as a reading, or NaN where it holds none.
fn reading(kind: Option<LogKind>, cell: &[u8]) -> f64 {
    cell_number(cell).filter(|value| is_reading(kind, *value)).unwrap_or(f64::NAN)
}

/// Whether a number in a curve's cell is a reading. A drawn curve holds
/// none outside what its trace can store.
fn is_reading(kind: Option<LogKind>, value: f64) -> bool {
    match kind {
        Some(kind) => (0.0..=kind.max_value()).contains(&value),
        None => !NO_READING.contains(&value),
    }
}

/// The samples of one run of rows, a column per curve.
struct Samples {
    depths: Vec<f64>,
    /// Empty for a curve the file does not carry.
    values: Vec<Vec<f64>>,
}

impl Samples {
    fn new(curves: usize) -> Self {
        Self {
            depths: Vec::new(),
            values: vec![Vec::new(); curves],
        }
    }

    fn bytes(&self) -> usize {
        (self.depths.capacity() + self.values.iter().map(Vec::capacity).sum::<usize>()) * size_of::<f64>()
    }

    fn clear(&mut self) {
        self.depths.clear();
        self.values.iter_mut().for_each(Vec::clear);
    }

    /// Move `other`'s rows onto the end. Both come from one file, so they
    /// carry the same curves.
    fn append(&mut self, other: &mut Samples) {
        self.depths.append(&mut other.depths);
        for (values, others) in self.values.iter_mut().zip(other.values.iter_mut()) {
            values.append(others);
        }
    }
}

/// Rows back for a curve the hole holds, waiting to be joined, and each
/// run's row count in the order the runs arrived.
struct Waiting {
    samples: Samples,
    runs: Vec<usize>,
}

/// One curve as the hole's files carry it: a drawn curve is one whichever
/// column holds it, any other is one per header.
#[derive(PartialEq)]
enum CurveKey {
    Kind(LogKind),
    Header(String),
}

/// Build one hole's traces from the bytes of its runs, `runs` in the order
/// [`GeophysicsLink::runs_of`] gives them. A row for another hole means the
/// file is not the one indexed.
pub(crate) fn read_hole(link: &GeophysicsLink, dhid: &str, runs: &[Vec<u8>]) -> Result<HoleLogs, CsvDrillError> {
    let files = link.runs_of(dhid).map(|(file, _)| file).collect::<Vec<_>>();
    if files.len() != runs.len() {
        return Err(CsvDrillError::Invalid(tr_format!(
            literal = "Read %read% run(s) of %hole%, the link has %runs%",
            read = runs.len(),
            hole = dhid.to_owned(),
            runs = files.len()
        )));
    }
    let mut reader = HoleReader::new(dhid, link);
    let mut at = 0;
    for (index, file) in link.files.iter().enumerate() {
        let count = files.iter().filter(|of| **of == index).count();
        if count > 0 {
            reader.read_file(file, &runs[at..at + count])?;
            at += count;
        }
    }
    Ok(reader.finish())
}

/// Reads one hole's runs into traces, file by file, merging each run as it
/// ends.
struct HoleReader<'a> {
    dhid: &'a str,
    curves: Vec<Curve>,
    samples: Samples,
    pending: Option<Waiting>,
}

/// One curve of the hole, its trace so far and the buffers it is settled
/// in. Curves settle apart from each other, so they settle in parallel.
struct Curve {
    key: CurveKey,
    kind: Option<LogKind>,
    /// The header the curve is read from in the file being read.
    label: String,
    built: Option<LogTrace>,
    /// Readings from the runs being settled.
    readings: Vec<(f64, f64)>,
    /// One run's readings, checked before they join `readings`.
    run_readings: Vec<(f64, f64)>,
    /// Depths, as bits, an earlier run of the batch already read.
    covered: BTreeSet<u64>,
    /// The held trace's samples and new readings, joined.
    joined: Vec<(f64, f64)>,
    /// Why runs could not be made a trace, with the header they were read
    /// from at the time: `label` moves on to a later file, so a note keeps
    /// its own.
    notes: Vec<(String, TraceError)>,
}

impl<'a> HoleReader<'a> {
    fn new(dhid: &'a str, link: &GeophysicsLink) -> Self {
        let mut curves: Vec<Curve> = Vec::new();
        for column in link.files.iter().flat_map(|file| &file.columns) {
            if let ColumnRole::Curve { kind } = column.role {
                let key = curve_key(kind, &column.header);
                if !curves.iter().any(|curve| curve.key == key) {
                    curves.push(Curve {
                        key,
                        kind,
                        label: String::new(),
                        built: None,
                        readings: Vec::new(),
                        run_readings: Vec::new(),
                        covered: BTreeSet::new(),
                        joined: Vec::new(),
                        notes: Vec::new(),
                    });
                }
            }
        }
        let count = curves.len();
        Self {
            dhid,
            curves,
            samples: Samples::new(count),
            pending: None,
        }
    }

    /// Read one file's runs of the hole. Its rows waiting are joined as it
    /// ends: a run does not carry over into the next file, whose columns
    /// differ.
    fn read_file(&mut self, file: &LinkedFile, runs: &[Vec<u8>]) -> Result<(), CsvDrillError> {
        let name = &file.identity.name;
        let width = file.columns.len();
        let column = |role: ColumnRole| file.columns.iter().position(|column| column.role == role);
        let (Some(dhid_column), Some(depth_column)) = (column(ColumnRole::Dhid), column(ColumnRole::Depth)) else {
            return Err(CsvDrillError::Invalid(tr_format!(
                literal = "%file% requires one DHID and one depth column",
                file = name.clone()
            )));
        };
        let mut read = Vec::new();
        self.curves.iter_mut().for_each(|curve| curve.label.clear());
        for (index, linked) in file.columns.iter().enumerate() {
            if let ColumnRole::Curve { kind } = linked.role
                && let Some(at) = self.curves.iter().position(|curve| curve.key == curve_key(kind, &linked.header))
            {
                read.push((index, at, kind));
                self.curves[at].label = linked.header.clone();
            }
        }
        let format = RowFormat {
            name,
            width,
            dhid_column,
            depth_column,
            read,
            curves: self.curves.len(),
        };
        for bytes in runs {
            let pieces = row_pieces(bytes);
            #[cfg(not(target_arch = "wasm32"))]
            let pieces = pieces.par_iter();
            #[cfg(target_arch = "wasm32")]
            let pieces = pieces.iter();
            let pieces = pieces.map(|piece| format.parse(self.dhid, piece)).collect::<Result<Vec<_>, _>>()?;
            for mut piece in pieces {
                self.samples.append(&mut piece);
            }
            self.end_run();
        }
        self.join_pending();
        Ok(())
    }

    /// Close the current run. The hole's first run of a curve becomes its
    /// trace now; a run back for a curve the hole holds waits with any
    /// others it came back with, and they join the traces as a batch.
    fn end_run(&mut self) {
        let mut samples = std::mem::replace(&mut self.samples, Samples::new(0));
        let rows = samples.depths.len();
        if self.returning(&samples) {
            let curves = self.curves.len();
            let waiting = self.pending.get_or_insert_with(|| Waiting {
                samples: Samples::new(curves),
                runs: Vec::new(),
            });
            waiting.samples.append(&mut samples);
            waiting.runs.push(rows);
            let (waiting_rows, waiting_bytes) = (waiting.samples.depths.len(), waiting.samples.bytes());
            if waiting_rows >= REJOIN_BATCH_ROWS.max(self.held_samples()) || waiting_bytes > WAITING_BYTES_LIMIT {
                self.join_pending();
            }
        } else {
            self.settle_samples(&samples, &[rows]);
        }
        samples.clear();
        self.samples = samples;
    }

    /// Whether a run is back for a curve the hole already holds, or rows
    /// are waiting, which the run must not overtake.
    fn returning(&self, samples: &Samples) -> bool {
        self.pending.is_some()
            || self
                .curves
                .iter()
                .zip(&samples.values)
                .any(|(curve, values)| curve.built.is_some() && values.iter().any(|value| value.is_finite()))
    }

    /// Samples in the hole's longest trace.
    fn held_samples(&self) -> usize {
        self.curves.iter().filter_map(|curve| curve.built.as_ref()).map(LogTrace::len).max().unwrap_or(0)
    }

    fn join_pending(&mut self) {
        if let Some(waiting) = self.pending.take() {
            self.settle_samples(&waiting.samples, &waiting.runs);
        }
    }

    /// Settle each curve `samples` read into the hole's traces, taking the
    /// runs, of `runs` rows each, in the order they arrived.
    fn settle_samples(&mut self, samples: &Samples, runs: &[usize]) {
        let settle = |(curve, values): (&mut Curve, &Vec<f64>)| {
            if !values.is_empty() {
                curve.settle(&samples.depths, values, runs);
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        self.curves.par_iter_mut().zip(samples.values.par_iter()).for_each(settle);
        #[cfg(target_arch = "wasm32")]
        self.curves.iter_mut().zip(samples.values.iter()).for_each(settle);
    }

    /// Report the runs not kept, each with the header it was read from, and
    /// hand over the traces.
    fn finish(self) -> HoleLogs {
        let notes = self.curves.iter().flat_map(|curve| curve.notes.iter());
        for (label, error) in notes.take(SKIP_REPORT_LIMIT) {
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "A run of %hole% %curve% was not kept (%reason%)",
                    hole = self.dhid.to_owned(),
                    curve = label.clone(),
                    reason = trace_error(error)
                )
            );
        }
        HoleLogs::new(self.curves.into_iter().filter_map(|curve| curve.built).collect())
    }
}

impl Curve {
    /// Settle this curve's `values` from runs of `runs` rows each. Readings
    /// held win: each run adds only depths with no reading within half a
    /// step, from the trace or from an earlier run. That extends a trace,
    /// fills its gaps and leaves a repeat pass out, two passes never
    /// interleave, and the result does not depend on where a batch ends.
    fn settle(&mut self, depths: &[f64], values: &[f64], runs: &[usize]) {
        self.readings.clear();
        self.covered.clear();
        let held = self.built.as_ref();
        // Half the held trace's step counts as one depth; with no step to go
        // by, the first run to have one sets it.
        let mut step = held.filter(|trace| trace.len() > 1).map(LogTrace::step);
        let mut start = 0;
        for (index, &rows) in runs.iter().enumerate() {
            let run = start..start + rows;
            start += rows;
            self.run_readings.clear();
            self.run_readings
                .extend(depths[run.clone()].iter().copied().zip(values[run].iter().copied()).filter(|(_, value)| value.is_finite()));
            if self.run_readings.is_empty() {
                continue;
            }
            // A hole's first run has nothing to be checked against.
            if held.is_some() || !self.covered.is_empty() {
                let reach = step.map_or(SAME_DEPTH, |step| 0.5 * step + SAME_DEPTH);
                let covered = &self.covered;
                self.run_readings
                    .retain(|(depth, _)| !held.is_some_and(|trace| holds_reading(trace, *depth)) && !covers(covered, *depth, reach));
            }
            // Only a later run of the batch is checked against this one.
            if index + 1 < runs.len() {
                // Stable, so at one depth the reading read first stays first.
                self.run_readings.sort_by(|a, b| a.0.total_cmp(&b.0));
                if step.is_none() && self.run_readings.len() > 1 {
                    step = median_spacing(&self.run_readings);
                }
                self.covered.extend(self.run_readings.iter().map(|(depth, _)| depth_key(*depth)));
            }
            self.readings.extend_from_slice(&self.run_readings);
        }
        self.covered.clear();
        if self.readings.is_empty() {
            return;
        }
        settle_depths(&mut self.readings);
        self.settle_trace();
    }

    /// Settle the checked readings, in `self.readings`, into the trace.
    fn settle_trace(&mut self) {
        let settled = match self.built.take() {
            None => build(self.kind, &self.readings)
                .map(|trace| trace.with_source(self.label.clone()))
                .map_err(|error| (None, error)),
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
                settle_depths(&mut self.joined);
                let source = joined_label(held.source(), &self.label);
                build(self.kind, &self.joined).map(|trace| trace.with_source(source)).map_err(|error| (Some(held), error))
            }
        };
        self.built = match settled {
            Ok(trace) => Some(trace),
            Err((held, error)) => {
                self.notes.push((self.label.clone(), error));
                held
            }
        };
    }
}

/// How one linked file's rows are read into a hole's curves.
struct RowFormat<'f> {
    name: &'f str,
    width: usize,
    dhid_column: usize,
    depth_column: usize,
    /// Each curve column read: its index, its curve, and its kind.
    read: Vec<(usize, usize, Option<LogKind>)>,
    curves: usize,
}

impl RowFormat<'_> {
    /// The rows of `bytes`, which are all `dhid`'s, a column per curve.
    fn parse(&self, dhid: &str, bytes: &[u8]) -> Result<Samples, CsvDrillError> {
        let mut samples = Samples::new(self.curves);
        let mut records = Records::new(bytes)?;
        while records.next()? {
            if records.is_blank() {
                continue;
            }
            // Rows the index pass refused are refused again, unreported.
            let Ok((id, depth)) = gate(&records, self.name, self.width, self.dhid_column, self.depth_column) else {
                continue;
            };
            if id != dhid {
                return Err(CsvDrillError::Invalid(tr_format!(
                    literal = "%file% no longer matches its index: link it again",
                    file = self.name.to_owned()
                )));
            }
            samples.depths.push(depth);
            for &(index, at, kind) in &self.read {
                samples.values[at].push(reading(kind, records.cell(index)));
            }
        }
        Ok(samples)
    }
}

/// Bytes of rows parsed as one piece.
const PIECE_BYTES: usize = 256 * 1024;

/// A run's bytes cut at line breaks into pieces parsed side by side; whole
/// when a quote could carry a row over a line break.
fn row_pieces(bytes: &[u8]) -> Vec<&[u8]> {
    if bytes.contains(&b'"') {
        return vec![bytes];
    }
    let mut pieces = Vec::new();
    let mut rest = bytes;
    while rest.len() > PIECE_BYTES {
        let cut = rest[PIECE_BYTES..].iter().position(|byte| *byte == b'\n').map_or(rest.len(), |at| PIECE_BYTES + at + 1);
        let (piece, tail) = rest.split_at(cut);
        pieces.push(piece);
        rest = tail;
    }
    if !rest.is_empty() {
        pieces.push(rest);
    }
    pieces
}

fn curve_key(kind: Option<LogKind>, header: &str) -> CurveKey {
    kind.map_or_else(|| CurveKey::Header(header.trim().to_lowercase()), CurveKey::Kind)
}

/// A row's hole id and depth, or the reason it is refused.
fn gate<'r, R: BufRead>(records: &'r Records<R>, file: &str, width: usize, dhid_column: usize, depth_column: usize) -> Result<(Cow<'r, str>, f64), String> {
    // Named only when refused: a file of a hundred million rows would pay
    // for the text on every one.
    let row = || records.record_number().to_string();
    if records.len() != width {
        return Err(tr_format!(
            literal = "%file% row %row% has %found% columns; expected %expected%",
            file = file.to_owned(),
            row = row(),
            found = records.len().to_string(),
            expected = width.to_string()
        ));
    }
    let dhid = match cell_text(records.cell(dhid_column)) {
        Cow::Borrowed(text) => Cow::Borrowed(text.trim()),
        Cow::Owned(text) => Cow::Owned(text.trim().to_owned()),
    };
    if dhid.is_empty() {
        return Err(tr_format!(literal = "%file% row %row% has a blank hole id", file = file.to_owned(), row = row()));
    }
    match cell_number(records.cell(depth_column)) {
        None => Err(tr_format!(literal = "%file% row %row% has no readable depth", file = file.to_owned(), row = row())),
        Some(depth) if depth < 0.0 => Err(tr_format!(literal = "%file% row %row% has a negative depth", file = file.to_owned(), row = row())),
        Some(depth) => Ok((dhid, depth)),
    }
}

/// A trace from readings.
fn build(kind: Option<LogKind>, readings: &[(f64, f64)]) -> Result<LogTrace, TraceError> {
    let (depths, values): (Vec<f64>, Vec<f64>) = readings.iter().copied().unzip();
    LogTrace::from_samples(kind, &depths, &values)
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

/// Sort readings by depth and fold those at one depth into the first.
fn settle_depths(rows: &mut Vec<(f64, f64)>) {
    if !rows.is_sorted_by(|a, b| a.0 <= b.0) {
        // Stable, so the reading read first stays first.
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    rows.dedup_by(|later, earlier| later.0 - earlier.0 <= SAME_DEPTH);
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
    plain_decimal(cell.trim_ascii()).or_else(|| std::str::from_utf8(cell).ok().and_then(csv_drill_hole::finite_number))
}

/// Powers of ten a double holds exactly.
const EXACT_POWERS_OF_TEN: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// A cell like `-12.345`, read without the general parser, which is most
/// of the cost of a hole's rows. Its digits and its power of ten are both
/// exact as doubles, so one division rounds as `str::parse` rounds (the
/// fast path in Clinger, "How to Read Floating Point Numbers Accurately",
/// 1990). Anything else is left to the general parser.
fn plain_decimal(cell: &[u8]) -> Option<f64> {
    let (negative, digits) = match cell.split_first()? {
        (b'-', rest) => (true, rest),
        (b'+', rest) => (false, rest),
        _ => (false, cell),
    };
    let (mut mantissa, mut count, mut decimals, mut point) = (0u64, 0usize, 0usize, false);
    for &byte in digits {
        match byte {
            b'0'..=b'9' if count < 19 => {
                mantissa = mantissa * 10 + u64::from(byte - b'0');
                count += 1;
                decimals += usize::from(point);
            }
            b'.' if !point => point = true,
            _ => return None,
        }
    }
    if count == 0 || mantissa > 1 << 53 {
        return None;
    }
    let value = mantissa as f64 / EXACT_POWERS_OF_TEN.get(decimals)?;
    Some(if negative { -value } else { value })
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
