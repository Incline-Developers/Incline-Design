//! Mapped CSV drillhole import. A bundle may describe relational collar,
//! survey and interval tables, or explicit measured-depth line segments.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::{
    collections::{BTreeMap, HashMap, hash_map::Entry},
    fmt,
    path::PathBuf,
};

use glam::DVec3;
use serde::{Deserialize, Serialize};

use super::csv_geophysics::{GeophysicsImport, GeophysicsReader, Records, StreamControl, cell_text};
use crate::{
    model::drill_hole::{
        DrillHole, DrillHoleDataset, DrillInterval, DrillValue, HoleOrientation, OrientationSource, SurveyObservation, TraceStation, direction_orientation, resolve_trace,
    },
    userspace_warn,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CsvDrillFileRole {
    Unassigned,
    Collar,
    Survey,
    Interval,
    ExplicitSegments,
    /// Downhole geophysics: one row per sample, read as a stream.
    Geophysics,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CsvDrillColumnRole {
    Ignore,
    Dhid,
    East,
    North,
    Elevation,
    Depth,
    Azimuth,
    Dip,
    Inclination,
    From,
    To,
    StartEast,
    StartNorth,
    StartElevation,
    EndEast,
    EndNorth,
    EndElevation,
    Diameter,
    /// Natural gamma, API units.
    Gamma,
    /// Long-spaced density, g/cc.
    LongDensity,
    /// Short-spaced density, g/cc.
    ShortDensity,
    Attribute(String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CsvDrillFileMapping {
    pub(crate) path: PathBuf,
    pub(crate) role: CsvDrillFileRole,
    pub(crate) columns: Vec<CsvDrillColumnRole>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CsvDrillPreview {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
}

#[derive(Debug)]
pub(crate) enum CsvDrillError {
    Io(std::io::Error),
    Invalid(String),
    /// The import was cancelled while a file streamed.
    Cancelled,
}

impl fmt::Display for CsvDrillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Invalid(message) => f.write_str(message),
            Self::Cancelled => f.write_str(&crate::i18n::tr!(literal = "Cancelled")),
        }
    }
}

impl std::error::Error for CsvDrillError {}
impl From<std::io::Error> for CsvDrillError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Rows shown under a file's headers while it is mapped.
const PREVIEW_ROWS: usize = 8;

/// Most of a file read for its preview. A geophysics export runs to hundreds
/// of megabytes and the preview shows its first rows only.
const PREVIEW_HEAD_BYTES: usize = 1024 * 1024;

/// Read buffer for a geophysics file streamed from disk.
#[cfg(not(target_arch = "wasm32"))]
const STREAM_BUFFER_BYTES: usize = 1024 * 1024;

/// The header and first rows of a file, read from its head only.
pub(crate) fn preview(bytes: &[u8]) -> Result<CsvDrillPreview, CsvDrillError> {
    let mut head = &bytes[..bytes.len().min(PREVIEW_HEAD_BYTES)];
    // A head cut short of the file ends at its last whole line.
    if head.len() < bytes.len()
        && let Some(end) = head.iter().rposition(|byte| *byte == b'\n')
    {
        head = &head[..=end];
    }
    // Repaired as a whole, as an import would be, so a legacy encoding or a
    // binary file is refused here rather than shown cell by cell.
    let (text, _) = decode_text(head)?;
    let mut records = Records::new(&text[..])?;
    let mut rows = Vec::new();
    while rows.len() <= PREVIEW_ROWS && records.next()? {
        rows.push((0..records.len()).map(|index| cell_text(records.cell(index)).into_owned()).collect::<Vec<_>>());
    }
    let mut rows = rows.into_iter();
    let headers = rows.next().ok_or_else(|| CsvDrillError::Invalid(crate::i18n::tr!(literal = "CSV file is empty")))?;
    if headers.is_empty() || headers.iter().all(|header| header.trim().is_empty()) {
        return Err(CsvDrillError::Invalid(crate::i18n::tr!(literal = "CSV header has no columns")));
    }
    let mut seen = std::collections::HashSet::new();
    for header in &headers {
        let key = strip_repair_marks(header).trim().to_ascii_lowercase();
        if key.is_empty() || !seen.insert(key) {
            return Err(CsvDrillError::Invalid(crate::i18n::tr!(literal = "CSV headers must be nonblank and unique")));
        }
    }
    Ok(CsvDrillPreview { headers, rows: rows.collect() })
}

/// A file's mapping before the user touches it: its purpose if the headers
/// name one, with the columns that purpose recognises.
pub(crate) fn initial_mapping(path: PathBuf, preview: &CsvDrillPreview) -> CsvDrillFileMapping {
    let role = guess_role(&preview.headers);
    let columns = if role == CsvDrillFileRole::Unassigned {
        vec![CsvDrillColumnRole::Ignore; preview.headers.len()]
    } else {
        default_columns(role, &preview.headers)
    };
    CsvDrillFileMapping { path, role, columns }
}

pub(crate) fn default_columns(role: CsvDrillFileRole, headers: &[String]) -> Vec<CsvDrillColumnRole> {
    let fallback = |header: &String| {
        if matches!(role, CsvDrillFileRole::Interval | CsvDrillFileRole::ExplicitSegments) {
            CsvDrillColumnRole::Attribute(strip_repair_marks(header).trim().to_owned())
        } else {
            CsvDrillColumnRole::Ignore
        }
    };
    // A wide alias table matches two columns for one role sooner or later. The
    // specific name wins; a loser is ignored, a tie left to `validate_roles`.
    let matched = headers.iter().map(|header| role_alias(role, &normalize_header(header))).collect::<Vec<_>>();
    let mut best: HashMap<CsvDrillColumnRole, (u8, usize, usize)> = HashMap::new();
    for (index, candidate) in matched.iter().enumerate() {
        let Some((found, tier)) = candidate else { continue };
        best.entry(found.clone())
            .and_modify(|entry| {
                if *tier < entry.0 {
                    *entry = (*tier, 1, index);
                } else if *tier == entry.0 {
                    entry.1 += 1;
                }
            })
            .or_insert((*tier, 1, index));
    }
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| match &matched[index] {
            Some((found, tier)) if best.get(found).is_some_and(|entry| entry.0 == *tier && entry.1 == 1 && entry.2 == index) => found.clone(),
            Some(_) => CsvDrillColumnRole::Ignore,
            None => fallback(header),
        })
        .collect()
}

/// A normalised header's role and how specific the name was: tier 0 names
/// the role outright, tier 1 a generic word used only when nothing better is.
fn role_alias(role: CsvDrillFileRole, header: &str) -> Option<(CsvDrillColumnRole, u8)> {
    if role == CsvDrillFileRole::Unassigned {
        return None;
    }
    match header {
        "dhid" | "holeid" | "hole" => return Some((CsvDrillColumnRole::Dhid, 0)),
        "id" => return Some((CsvDrillColumnRole::Dhid, 1)),
        _ => {}
    }
    match role {
        CsvDrillFileRole::Unassigned => None,
        CsvDrillFileRole::Collar => match header {
            "east" | "easting" | "x" => Some((CsvDrillColumnRole::East, 0)),
            "north" | "northing" | "y" => Some((CsvDrillColumnRole::North, 0)),
            "elevation" | "elev" | "rl" | "z" => Some((CsvDrillColumnRole::Elevation, 0)),
            "height" => Some((CsvDrillColumnRole::Elevation, 1)),
            "diameter" | "diam" => Some((CsvDrillColumnRole::Diameter, 0)),
            _ => None,
        },
        CsvDrillFileRole::Survey => match header {
            "depth" | "at" | "md" => Some((CsvDrillColumnRole::Depth, 0)),
            "dep" => Some((CsvDrillColumnRole::Depth, 1)),
            "east" | "easting" | "x" => Some((CsvDrillColumnRole::East, 0)),
            "north" | "northing" | "y" => Some((CsvDrillColumnRole::North, 0)),
            "elevation" | "elev" | "rl" | "z" => Some((CsvDrillColumnRole::Elevation, 0)),
            "height" => Some((CsvDrillColumnRole::Elevation, 1)),
            "azimuth" | "azi" | "bearing" => Some((CsvDrillColumnRole::Azimuth, 0)),
            "dip" => Some((CsvDrillColumnRole::Dip, 0)),
            "inc" | "incl" | "incline" | "inclination" => Some((CsvDrillColumnRole::Inclination, 0)),
            _ => None,
        },
        CsvDrillFileRole::Interval => match header {
            "from" => Some((CsvDrillColumnRole::From, 0)),
            "depfr" | "depthfrom" => Some((CsvDrillColumnRole::From, 1)),
            "to" => Some((CsvDrillColumnRole::To, 0)),
            "depto" | "depthto" => Some((CsvDrillColumnRole::To, 1)),
            _ => None,
        },
        CsvDrillFileRole::ExplicitSegments => match header {
            "from" => Some((CsvDrillColumnRole::From, 0)),
            "to" => Some((CsvDrillColumnRole::To, 0)),
            "starteast" | "startx" | "x1" => Some((CsvDrillColumnRole::StartEast, 0)),
            "startnorth" | "starty" | "y1" => Some((CsvDrillColumnRole::StartNorth, 0)),
            "startelevation" | "startz" | "z1" => Some((CsvDrillColumnRole::StartElevation, 0)),
            "endeast" | "endx" | "x2" => Some((CsvDrillColumnRole::EndEast, 0)),
            "endnorth" | "endy" | "y2" => Some((CsvDrillColumnRole::EndNorth, 0)),
            "endelevation" | "endz" | "z2" => Some((CsvDrillColumnRole::EndElevation, 0)),
            "diameter" | "diam" => Some((CsvDrillColumnRole::Diameter, 0)),
            _ => None,
        },
        CsvDrillFileRole::Geophysics => match header {
            "depth" => Some((CsvDrillColumnRole::Depth, 0)),
            "dep" | "md" => Some((CsvDrillColumnRole::Depth, 1)),
            "gam" | "gamma" | "gr" | "grde" | "gamm" => Some((CsvDrillColumnRole::Gamma, 0)),
            "lsd" | "denl" | "longdensity" | "longspaceddensity" => Some((CsvDrillColumnRole::LongDensity, 0)),
            "ssd" | "denb" | "shortdensity" | "shortspaceddensity" => Some((CsvDrillColumnRole::ShortDensity, 0)),
            _ => None,
        },
    }
}

/// The purpose a file's headers point to, if they point to one: a depth and
/// a gamma or density column make a geophysics file. Other purposes are
/// left for the user to choose.
pub(crate) fn guess_role(headers: &[String]) -> CsvDrillFileRole {
    let roles = headers
        .iter()
        .filter_map(|header| role_alias(CsvDrillFileRole::Geophysics, &normalize_header(header)))
        .map(|(role, _)| role)
        .collect::<Vec<_>>();
    if roles.contains(&CsvDrillColumnRole::Depth) && roles.iter().any(|role| crate::model::formats::csv_geophysics::curve_kind(role).is_some()) {
        CsvDrillFileRole::Geophysics
    } else {
        CsvDrillFileRole::Unassigned
    }
}

/// Drops a repair marker's two hex digits, keeping the mark to show the damage.
fn strip_repair_marks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text.chars();
    while let Some(character) = rest.next() {
        if character == REPAIR_MARK {
            let mut ahead = rest.clone();
            if [ahead.next(), ahead.next()].iter().all(|digit| digit.is_some_and(|digit| digit.is_ascii_hexdigit())) {
                rest = ahead;
            }
        }
        out.push(character);
    }
    out
}

/// The repair marker and its two hex digits are dropped as one unit.
fn normalize_header(header: &str) -> String {
    let mut out = String::with_capacity(header.len());
    let mut rest = header.chars();
    while let Some(character) = rest.next() {
        if character == REPAIR_MARK {
            let mut ahead = rest.clone();
            if [ahead.next(), ahead.next()].iter().all(|digit| digit.is_some_and(|digit| digit.is_ascii_hexdigit())) {
                rest = ahead;
            }
            continue;
        }
        if character.is_ascii_alphanumeric() {
            out.extend(character.to_lowercase());
        }
    }
    out
}

/// Only the head of the file is read: enough for the header and the rows
/// the preview shows.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn preview_path(path: &Path) -> Result<CsvDrillPreview, CsvDrillError> {
    use std::io::Read;

    let mut head = Vec::new();
    std::fs::File::open(path)?.take(PREVIEW_HEAD_BYTES as u64 + 1).read_to_end(&mut head)?;
    preview(&head)
}

/// The file a bundle's holes are defined by, and its dataset named after:
/// the collar file, else the explicit-segment file. A bundle with neither
/// has no holes for geophysics to attach to.
pub(crate) fn bundle_anchor<'a>(files: impl IntoIterator<Item = &'a CsvDrillFileMapping>) -> Option<&'a CsvDrillFileMapping> {
    let mut segments = None;
    for mapping in files {
        match mapping.role {
            CsvDrillFileRole::Collar => return Some(mapping),
            CsvDrillFileRole::ExplicitSegments => segments = segments.or(Some(mapping)),
            _ => {}
        }
    }
    segments
}

/// A bundle's dataset and, when it carries geophysics files, their traces.
pub(crate) struct ParsedBundle {
    pub(crate) dataset: DrillHoleDataset,
    pub(crate) geophysics: Option<GeophysicsImport>,
}

/// Every mapping is checked before any file is read, so a bad one fails the
/// import at once; geophysics hangs off the holes the [`bundle_anchor`]
/// defines, the same rule the import dialog applies.
fn check_purposes<'a>(files: impl IntoIterator<Item = &'a CsvDrillFileMapping>) -> Result<(), CsvDrillError> {
    let files: Vec<&CsvDrillFileMapping> = files.into_iter().collect();
    for mapping in &files {
        validate_roles(mapping)?;
    }
    let has_geophysics = files.iter().any(|mapping| mapping.role == CsvDrillFileRole::Geophysics);
    if has_geophysics && bundle_anchor(files.iter().copied()).is_none() {
        return Err(CsvDrillError::Invalid(crate::i18n::tr!(
            literal = "Downhole geophysics needs a collar or explicit-segment file in the bundle, whose holes it attaches to"
        )));
    }
    Ok(())
}

/// Tables are read whole; geophysics files are streamed from disk, never
/// held whole.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn parse_paths(files: &[CsvDrillFileMapping], control: StreamControl<'_>) -> Result<ParsedBundle, CsvDrillError> {
    check_purposes(files)?;
    let (streams, tables): (Vec<_>, Vec<_>) = files.iter().partition(|mapping| mapping.role == CsvDrillFileRole::Geophysics);
    let buffers = tables
        .iter()
        .map(|mapping| std::fs::read(&mapping.path).map_err(CsvDrillError::Io))
        .collect::<Result<Vec<_>, _>>()?;
    let sizes = streams
        .iter()
        .map(|mapping| std::fs::metadata(&mapping.path).map_or(0, |metadata| metadata.len()))
        .collect::<Vec<_>>();
    let mut work = BundleWork::new(buffers.iter().map(|bytes| bytes.len() as u64).sum(), sizes.iter().sum(), control);
    let dataset = parse_tables(tables.iter().copied().zip(buffers.iter().map(Vec::as_slice)))?;
    drop(buffers);
    work.tables_done();
    let opened = streams.into_iter().zip(sizes).map(|(mapping, size)| {
        let reader = std::fs::File::open(&mapping.path)
            .map(|file| std::io::BufReader::with_capacity(STREAM_BUFFER_BYTES, file))
            .map_err(CsvDrillError::Io);
        (mapping, size, reader)
    });
    let geophysics = read_geophysics(&dataset, opened, &mut work)?;
    Ok(ParsedBundle { dataset, geophysics })
}

/// A bundle whose files are all in memory, as the browser hands them over.
#[cfg(any(target_arch = "wasm32", test))]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code, reason = "the browser import uses this; native builds it only for tests"))]
pub(crate) fn parse_bundle<'a>(inputs: impl IntoIterator<Item = (&'a CsvDrillFileMapping, &'a [u8])>, control: StreamControl<'_>) -> Result<ParsedBundle, CsvDrillError> {
    let inputs = inputs.into_iter().collect::<Vec<_>>();
    check_purposes(inputs.iter().map(|(mapping, _)| *mapping))?;
    let (streams, tables): (Vec<_>, Vec<_>) = inputs.into_iter().partition(|(mapping, _)| mapping.role == CsvDrillFileRole::Geophysics);
    let bytes = |files: &[(&CsvDrillFileMapping, &[u8])]| files.iter().map(|(_, bytes)| bytes.len() as u64).sum();
    let mut work = BundleWork::new(bytes(&tables), bytes(&streams), control);
    let dataset = parse_tables(tables)?;
    work.tables_done();
    let streams = streams.into_iter().map(|(mapping, bytes)| (mapping, bytes.len() as u64, Ok(bytes)));
    let geophysics = read_geophysics(&dataset, streams, &mut work)?;
    Ok(ParsedBundle { dataset, geophysics })
}

/// A bundle's progress, as the share of its bytes read: the tables all at
/// once when they are parsed, then the geophysics as it streams. A bundle
/// without geophysics is done when its tables are.
struct BundleWork<'c> {
    control: StreamControl<'c>,
    tables: u64,
    whole: u64,
    /// Geophysics bytes of the files already read.
    streamed: u64,
}

impl<'c> BundleWork<'c> {
    fn new(tables: u64, geophysics: u64, control: StreamControl<'c>) -> Self {
        Self {
            control,
            tables,
            whole: (tables + geophysics).max(1),
            streamed: 0,
        }
    }

    fn report(&self, geophysics: u64) {
        (self.control.progress)(((self.tables + geophysics) as f64 / self.whole as f64).min(1.0) as f32);
    }

    fn tables_done(&self) {
        self.report(0);
    }
}

/// Stream every geophysics file into traces for the dataset's holes. The
/// geophysics hangs off the dataset, not the other way round, and each file
/// stands alone: one that fails is left out with a warning, the traces of
/// the others are kept, and the drillholes load either way.
fn read_geophysics<'m, R: std::io::BufRead>(
    dataset: &DrillHoleDataset,
    streams: impl IntoIterator<Item = (&'m CsvDrillFileMapping, u64, Result<R, CsvDrillError>)>,
    work: &mut BundleWork<'_>,
) -> Result<Option<GeophysicsImport>, CsvDrillError> {
    let mut reader = None;
    let mut any_read = false;
    for (mapping, size, input) in streams {
        if (work.control.cancelled)() {
            return Err(CsvDrillError::Cancelled);
        }
        let reader = reader.get_or_insert_with(|| GeophysicsReader::new(dataset.holes.iter().map(|hole| hole.dhid.clone())));
        let streamed = work.streamed;
        let outcome = input.and_then(|input| reader.read(mapping, input, work.control.cancelled, &mut |bytes| work.report(streamed + bytes)));
        work.streamed += size;
        work.report(work.streamed);
        match outcome {
            Ok(()) => any_read = true,
            Err(CsvDrillError::Cancelled) => return Err(CsvDrillError::Cancelled),
            Err(error) => userspace_warn!(
                "{}",
                crate::i18n::tr_format!(
                    literal = "%file% was left out of the downhole geophysics: %error%",
                    file = mapping.path.display().to_string(),
                    error = error.to_string()
                )
            ),
        }
    }
    Ok(reader.filter(|_| any_read).map(GeophysicsReader::finish))
}

/// Enough to find the bad rows without burying the console.
pub(super) const SKIP_REPORT_LIMIT: usize = 10;

/// Hole names on an overlap report are a pointer to go and look, not a census.
const OVERLAP_EXAMPLE_LIMIT: usize = 3;

/// A short file may carry a handful of stray bytes outright.
const MINIMUM_REPAIR_BUDGET: usize = 16;

/// Enough of the head to tell wide text from a stray NUL.
const NUL_SAMPLE_BYTES: usize = 4096;

/// Written for an unreadable byte; in a repaired file it is never a value.
const REPAIR_MARK: char = '\u{FFFD}';

/// The collar, survey, interval and segment tables of a bundle.
fn parse_tables<'a>(inputs: impl IntoIterator<Item = (&'a CsvDrillFileMapping, &'a [u8])>) -> Result<DrillHoleDataset, CsvDrillError> {
    let mut inputs = inputs.into_iter().collect::<Vec<_>>();
    if inputs.is_empty() {
        return Err(CsvDrillError::Invalid("Select at least one CSV file".into()));
    }
    let collar_files = inputs.iter().filter(|(mapping, _)| mapping.role == CsvDrillFileRole::Collar).count();
    let survey_files = inputs.iter().filter(|(mapping, _)| mapping.role == CsvDrillFileRole::Survey).count();
    let segment_files = inputs.iter().filter(|(mapping, _)| mapping.role == CsvDrillFileRole::ExplicitSegments).count();
    if collar_files > 1 || survey_files > 1 || segment_files > 1 {
        return Err(CsvDrillError::Invalid("Use at most one collar, survey, and explicit-segment CSV".into()));
    }
    if segment_files == 0 && collar_files != 1 {
        return Err(CsvDrillError::Invalid("A relational CSV bundle requires one collar file".into()));
    }
    if segment_files == 1 && collar_files + survey_files != 0 {
        return Err(CsvDrillError::Invalid("Explicit-segment geometry cannot be combined with collar or survey geometry".into()));
    }

    // Geometry is read before the rows that hang off it, so a row naming a
    // hole the bundle does not define is known as out of scope when it is read.
    inputs.sort_by_key(|(mapping, _)| !matches!(mapping.role, CsvDrillFileRole::Collar | CsvDrillFileRole::ExplicitSegments));

    let segment_scoped = segment_files == 1;
    let mut skipped = 0usize;
    let mut orphaned = 0usize;
    let mut collars: HashMap<String, Collar> = HashMap::new();
    let mut surveys: HashMap<String, Vec<SurveyObservation>> = HashMap::new();
    let mut intervals: HashMap<String, Vec<DrillInterval>> = HashMap::new();
    let mut segment_stations: HashMap<String, Vec<TraceStation>> = HashMap::new();
    let mut segment_ranges: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    let mut segment_diameters: HashMap<String, Option<f64>> = HashMap::new();
    let mut attribute_counts: HashMap<String, usize> = HashMap::new();
    for (mapping, bytes) in &inputs {
        let rows = parse_csv(bytes)?.rows;
        let headers = rows.first().ok_or_else(|| CsvDrillError::Invalid(format!("{} is empty", mapping.path.display())))?;
        for (index, role) in mapping.columns.iter().enumerate() {
            if let CsvDrillColumnRole::Attribute(mapped) = role {
                let label = if mapped.trim().is_empty() { headers[index].trim() } else { mapped.trim() };
                *attribute_counts.entry(label.to_owned()).or_default() += 1;
            }
        }
    }

    for (mapping, bytes) in &inputs {
        let parsed = parse_csv(bytes)?;
        // One warning per file, though `parse_csv` runs three times over it.
        let repaired = parsed.repaired_bytes > 0;
        if repaired {
            userspace_warn!(
                "{}",
                crate::i18n::tr_format!(
                    literal = "%file% is not valid UTF-8; %count% unreadable byte(s) were replaced in %cells% cell(s); a damaged cell is not read as data",
                    file = mapping.path.display().to_string(),
                    count = parsed.repaired_bytes.to_string(),
                    cells = parsed.repaired_cells.to_string()
                )
            );
        }
        let rows = parsed.rows;
        let headers = rows.first().ok_or_else(|| CsvDrillError::Invalid(format!("{} is empty", mapping.path.display())))?;
        if mapping.columns.len() != headers.len() {
            return Err(CsvDrillError::Invalid(format!(
                "{} mapping has {} columns, CSV has {}",
                mapping.path.display(),
                mapping.columns.len(),
                headers.len()
            )));
        }
        let file = CsvFile { mapping, headers, repaired };
        let stem = mapping.path.file_stem().and_then(|value| value.to_str()).unwrap_or("interval");
        // Values decide the sign: a column headed inclination is often signed
        // as dip already, and negating it would stand every hole on its head.
        let angles = if mapping.role == CsvDrillFileRole::Survey {
            scan_survey_angles(&file, &rows)
        } else {
            SurveyAngleScan::default()
        };
        let inclination_as_dip = angles.inclination_is_dip;
        if inclination_as_dip {
            userspace_warn!(
                "{}",
                crate::i18n::tr_format!(
                    literal = "%file% inclination values that could be an angle are all at or below zero, so the column was read as dip, negative downward",
                    file = mapping.path.display().to_string()
                )
            );
        }
        if angles.azimuth_off_compass > 0 {
            userspace_warn!(
                "{}",
                crate::i18n::tr_format!(
                    literal = "%file% holds %count% rows whose azimuth is not between 0 and 360",
                    file = mapping.path.display().to_string(),
                    count = angles.azimuth_off_compass.to_string()
                )
            );
        }
        if angles.tilt_not_an_angle > 0 {
            userspace_warn!(
                "{}",
                crate::i18n::tr_format!(
                    literal = "%file% holds %count% rows whose dip is not between -90 and 90; those rows were read without a direction",
                    file = mapping.path.display().to_string(),
                    count = angles.tilt_not_an_angle.to_string()
                )
            );
        }
        // Once per row, not per attribute column, and only for interval files.
        let gates = (mapping.role == CsvDrillFileRole::Interval).then(|| {
            rows.iter()
                .enumerate()
                .skip(1)
                .map(|(row_index, row)| interval_gate(&file, row, row_index))
                .collect::<Vec<_>>()
        });
        let numeric_attributes = mapping
            .columns
            .iter()
            .enumerate()
            .filter_map(|(index, role)| matches!(role, CsvDrillColumnRole::Attribute(_)).then_some(index))
            .filter(|&index| {
                rows.iter()
                    .enumerate()
                    .skip(1)
                    // Rows the import will drop must not vote, or one junk
                    // cell would demote a numeric column to text.
                    .filter(|(row_index, _)| {
                        gates.as_ref().is_none_or(|gates| gates[row_index - 1].as_ref().is_ok_and(|keys| hole_known(segment_scoped, &collars, &segment_stations, &keys.dhid)))
                    })
                    .filter_map(|(_, row)| row.get(index).map(|value| value.trim()))
                    .filter(|value| !value.is_empty() && !file.damaged(value))
                    .all(|value| finite_number(value).is_some())
            })
            .collect::<std::collections::HashSet<_>>();
        let mut row_count = 0usize;
        let skipped_before = skipped;
        // The reason names its own file and row.
        let mut report_skip = |error: &CsvDrillError| {
            skipped += 1;
            if skipped <= SKIP_REPORT_LIMIT {
                userspace_warn!("{}", crate::i18n::tr_format!(literal = "Skipped a row: %reason%", reason = error.to_string()));
            }
        };
        // Each gate rides with the row it was read from, so the two cannot
        // come apart; a file without gates yields None for every row.
        let gates = gates.into_iter().flatten().map(Some).chain(std::iter::repeat_with(|| None));
        for ((row_index, row), gate) in rows.iter().enumerate().skip(1).zip(gates) {
            if row.iter().all(|value| value.trim().is_empty()) {
                continue;
            }
            // A missing segment breaks the trace, so a segment row still
            // refuses the file rather than being skipped.
            if mapping.role == CsvDrillFileRole::ExplicitSegments {
                check_width(&file, row, row_index)?;
                let dhid = required_text(&file, row, &CsvDrillColumnRole::Dhid, row_index)?;
                let from = required_number(&file, row, &CsvDrillColumnRole::From, row_index)?;
                let to = required_number(&file, row, &CsvDrillColumnRole::To, row_index)?;
                validate_segment(mapping, from, to, &dhid, row_index)?;
                let start = DVec3::new(
                    required_number(&file, row, &CsvDrillColumnRole::StartEast, row_index)?,
                    required_number(&file, row, &CsvDrillColumnRole::StartNorth, row_index)?,
                    required_number(&file, row, &CsvDrillColumnRole::StartElevation, row_index)?,
                );
                let end = DVec3::new(
                    required_number(&file, row, &CsvDrillColumnRole::EndEast, row_index)?,
                    required_number(&file, row, &CsvDrillColumnRole::EndNorth, row_index)?,
                    required_number(&file, row, &CsvDrillColumnRole::EndElevation, row_index)?,
                );
                let diameter = optional_number(&file, row, &CsvDrillColumnRole::Diameter, row_index)?;
                validate_diameter(diameter, &dhid)?;
                if let Some(existing) = segment_diameters.get(&dhid).copied() {
                    let consistent = match (existing, diameter) {
                        (None, None) => true,
                        (Some(a), Some(b)) => (a - b).abs() <= 1.0e-9,
                        _ => false,
                    };
                    if !consistent {
                        return Err(CsvDrillError::Invalid(format!("inconsistent diameter for DHID '{dhid}'")));
                    }
                } else {
                    segment_diameters.insert(dhid.clone(), diameter);
                }
                segment_stations
                    .entry(dhid.clone())
                    .or_default()
                    .extend([TraceStation { depth: from, position: start }, TraceStation { depth: to, position: end }]);
                segment_ranges.entry(dhid.clone()).or_default().push((from, to));
                let keys = IntervalKeys { dhid, from, to };
                let interval = interval_row(&file, row, &keys, stem, &numeric_attributes, &attribute_counts);
                intervals.entry(keys.dhid).or_default().push(interval);
                continue;
            }
            // Every malformed row is skipped, not only an unreadable attribute:
            // a blank hole ID or a dropped comma once cost the whole file.
            let parsed = match (gate, mapping.role) {
                (Some(gate), _) => gate.map(RowValue::Interval),
                (None, CsvDrillFileRole::Collar) => collar_gate(&file, row, row_index).map(|(dhid, collar)| RowValue::Collar(dhid, collar)),
                (None, CsvDrillFileRole::Survey) => survey_gate(&file, row, row_index, inclination_as_dip).map(|(dhid, observation)| RowValue::Survey(dhid, observation)),
                // Segment rows return above; unassigned is refused.
                (None, _) => continue,
            };
            row_count += 1;
            match parsed {
                Ok(value) => {
                    // A row for a hole this bundle never defines is out of
                    // scope, not unreadable, so it is its own report.
                    if let Some(dhid) = value.hole_id()
                        && !hole_known(segment_scoped, &collars, &segment_stations, dhid)
                    {
                        orphaned += 1;
                        if orphaned <= SKIP_REPORT_LIMIT {
                            userspace_warn!(
                                "{}",
                                crate::i18n::tr_format!(
                                    literal = "%file% row %row% is for DHID '%dhid%', a hole the bundle's geometry does not define",
                                    file = mapping.path.display().to_string(),
                                    row = (row_index + 1).to_string(),
                                    dhid = dhid.clone()
                                )
                            );
                        }
                        continue;
                    }
                    match value {
                        RowValue::Collar(dhid, collar) => match collars.entry(dhid) {
                            // A second row for one hole is unusable.
                            Entry::Occupied(entry) => report_skip(&CsvDrillError::Invalid(format!(
                                "{} row {} repeats DHID '{}'",
                                mapping.path.display(),
                                row_index + 1,
                                entry.key()
                            ))),
                            Entry::Vacant(entry) => {
                                entry.insert(collar);
                            }
                        },
                        RowValue::Survey(dhid, observation) => surveys.entry(dhid).or_default().push(observation),
                        RowValue::Interval(keys) => {
                            let interval = interval_row(&file, row, &keys, stem, &numeric_attributes, &attribute_counts);
                            intervals.entry(keys.dhid).or_default().push(interval);
                        }
                    }
                }
                Err(error) => report_skip(&error),
            }
        }
        // A few unreadable rows are dirty data; most of a file failing is a
        // mapping mistake; carrying on would report success and draw nothing.
        let row_skipped = skipped - skipped_before;
        if row_count > 0 && row_skipped * 2 > row_count {
            return Err(CsvDrillError::Invalid(format!(
                "{}: {row_skipped} of {row_count} rows could not be read; the reasons are in the console",
                mapping.path.display()
            )));
        }
    }

    if skipped > SKIP_REPORT_LIMIT {
        userspace_warn!("{}", crate::i18n::tr_format!(literal = "%count% rows were skipped in total", count = skipped.to_string()));
    }
    if orphaned > SKIP_REPORT_LIMIT {
        userspace_warn!(
            "{}",
            crate::i18n::tr_format!(literal = "%count% rows were for a hole the bundle's geometry does not define", count = orphaned.to_string())
        );
    }

    report_overlaps(&intervals);

    let mut holes = Vec::new();
    if segment_files == 1 {
        for (dhid, mut trace) in segment_stations {
            trace.sort_by(|a, b| a.depth.total_cmp(&b.depth));
            trace.dedup_by(|a, b| (a.depth - b.depth).abs() <= 1.0e-9 && (a.position - b.position).length() <= 1.0e-7);
            if trace.windows(2).any(|pair| (pair[0].depth - pair[1].depth).abs() <= 1.0e-9) {
                return Err(CsvDrillError::Invalid(format!("DHID '{dhid}' has inconsistent coordinates at the same measured depth")));
            }
            let collar = trace
                .first()
                .map(|station| station.position)
                .ok_or_else(|| CsvDrillError::Invalid(format!("DHID '{dhid}' has no segments")))?;
            let hole_intervals = intervals.remove(&dhid).unwrap_or_default();
            let render_ranges = segment_ranges.remove(&dhid).unwrap_or_default();
            holes.push(DrillHole {
                dhid: dhid.clone(),
                collar,
                diameter: segment_diameters.remove(&dhid).flatten(),
                trace,
                render_ranges,
                intervals: hole_intervals,
                // Explicit segments carry file positions, not a projection.
                orientation_source: OrientationSource::Measured,
            });
        }
    } else {
        for (dhid, collar) in collars {
            let hole_intervals = intervals.remove(&dhid).unwrap_or_default();
            let target_depth = hole_intervals.iter().map(|interval| interval.to).fold(0.0, f64::max);
            let observations = surveys.entry(dhid.clone()).or_default();
            let resolved = resolve_trace(collar.position, observations, target_depth);
            // Measured only if an observation steered the trace past collar.
            let orientation_source = if resolved.steered { OrientationSource::Measured } else { OrientationSource::Unknown };
            holes.push(DrillHole {
                dhid,
                collar: collar.position,
                diameter: collar.diameter,
                trace: resolved.stations,
                render_ranges: Vec::new(),
                intervals: hole_intervals,
                orientation_source,
            });
        }
    }
    Ok(DrillHoleDataset::new(holes))
}

#[derive(Clone, Copy)]
struct Collar {
    position: DVec3,
    diameter: Option<f64>,
}

fn validate_roles(mapping: &CsvDrillFileMapping) -> Result<(), CsvDrillError> {
    let count = |role: &CsvDrillColumnRole| mapping.columns.iter().filter(|value| *value == role).count();
    let require = |role: CsvDrillColumnRole, label: &str| -> Result<(), CsvDrillError> {
        if count(&role) == 1 {
            Ok(())
        } else {
            Err(CsvDrillError::Invalid(format!("{} requires exactly one {label} column", mapping.path.display())))
        }
    };
    match mapping.role {
        CsvDrillFileRole::Unassigned => {
            return Err(CsvDrillError::Invalid(format!("Choose a file purpose for {}", mapping.path.display())));
        }
        CsvDrillFileRole::Collar => {
            require(CsvDrillColumnRole::Dhid, "DHID")?;
            require(CsvDrillColumnRole::East, "east/X")?;
            require(CsvDrillColumnRole::North, "north/Y")?;
            require(CsvDrillColumnRole::Elevation, "elevation/Z")?;
        }
        CsvDrillFileRole::Survey => {
            require(CsvDrillColumnRole::Dhid, "DHID")?;
            require(CsvDrillColumnRole::Depth, "depth")?;
            let xyz = count(&CsvDrillColumnRole::East) == 1 && count(&CsvDrillColumnRole::North) == 1 && count(&CsvDrillColumnRole::Elevation) == 1;
            // Dip and inclination may both be mapped: a row reads whichever
            // cell it carries, so either column alone satisfies the file.
            let tilt = count(&CsvDrillColumnRole::Dip) + count(&CsvDrillColumnRole::Inclination);
            if count(&CsvDrillColumnRole::Dip) > 1 || count(&CsvDrillColumnRole::Inclination) > 1 {
                return Err(CsvDrillError::Invalid(format!("{} maps a dip or inclination column twice", mapping.path.display())));
            }
            let angles = count(&CsvDrillColumnRole::Azimuth) == 1 && tilt >= 1;
            if !xyz && !angles {
                return Err(CsvDrillError::Invalid(format!("{} survey requires XYZ or azimuth and dip", mapping.path.display())));
            }
        }
        CsvDrillFileRole::Interval => {
            require(CsvDrillColumnRole::Dhid, "DHID")?;
            require(CsvDrillColumnRole::From, "FROM")?;
            require(CsvDrillColumnRole::To, "TO")?;
        }
        CsvDrillFileRole::ExplicitSegments => {
            require(CsvDrillColumnRole::Dhid, "DHID")?;
            for (role, label) in [
                (CsvDrillColumnRole::From, "FROM"),
                (CsvDrillColumnRole::To, "TO"),
                (CsvDrillColumnRole::StartEast, "start east/X"),
                (CsvDrillColumnRole::StartNorth, "start north/Y"),
                (CsvDrillColumnRole::StartElevation, "start elevation/Z"),
                (CsvDrillColumnRole::EndEast, "end east/X"),
                (CsvDrillColumnRole::EndNorth, "end north/Y"),
                (CsvDrillColumnRole::EndElevation, "end elevation/Z"),
            ] {
                require(role, label)?;
            }
        }
        CsvDrillFileRole::Geophysics => {
            require(CsvDrillColumnRole::Dhid, "DHID")?;
            require(CsvDrillColumnRole::Depth, "depth")?;
            let curves = [CsvDrillColumnRole::Gamma, CsvDrillColumnRole::LongDensity, CsvDrillColumnRole::ShortDensity].map(|role| count(&role));
            if curves.iter().any(|mapped| *mapped > 1) {
                return Err(CsvDrillError::Invalid(crate::i18n::tr_format!(
                    literal = "%file% maps a gamma or density column twice",
                    file = mapping.path.display().to_string()
                )));
            }
            if curves.iter().all(|mapped| *mapped == 0) {
                return Err(CsvDrillError::Invalid(crate::i18n::tr_format!(
                    literal = "%file% requires a gamma or density column",
                    file = mapping.path.display().to_string()
                )));
            }
        }
    }
    if count(&CsvDrillColumnRole::Diameter) > 1 {
        return Err(CsvDrillError::Invalid(format!("{} has multiple diameter mappings", mapping.path.display())));
    }
    Ok(())
}

fn interval_row(
    file: &CsvFile<'_>,
    row: &[String],
    keys: &IntervalKeys,
    stem: &str,
    numeric_attributes: &std::collections::HashSet<usize>,
    attribute_counts: &HashMap<String, usize>,
) -> DrillInterval {
    let mut values = BTreeMap::new();
    for (index, role) in file.mapping.columns.iter().enumerate() {
        let CsvDrillColumnRole::Attribute(mapped_name) = role else {
            continue;
        };
        let raw = row[index].trim();
        if raw.is_empty() || file.damaged(raw) {
            continue;
        }
        let label = if mapped_name.trim().is_empty() {
            file.headers[index].trim()
        } else {
            mapped_name.trim()
        };
        let key = if attribute_counts.get(label).copied().unwrap_or(0) > 1 {
            format!("{stem}.{label}")
        } else {
            label.to_owned()
        };
        let value = if numeric_attributes.contains(&index) {
            DrillValue::Numeric(finite_number(raw).expect("column-wide numeric inference validated this value"))
        } else {
            DrillValue::Category(raw.to_owned())
        };
        values.insert(key, value);
    }
    // Nothing is copied: `logged` stays empty until a correction parts them.
    DrillInterval {
        from: keys.from,
        to: keys.to,
        values,
        logged: None,
    }
}

/// What one collar, survey or interval row became once it passed its gate.
enum RowValue {
    Collar(String, Collar),
    Survey(String, SurveyObservation),
    Interval(IntervalKeys),
}

impl RowValue {
    /// The hole this row hangs off; a collar row is the hole itself.
    fn hole_id(&self) -> Option<&String> {
        match self {
            Self::Collar(..) => None,
            Self::Survey(dhid, _) => Some(dhid),
            Self::Interval(keys) => Some(&keys.dhid),
        }
    }
}

/// Whether the bundle's geometry defines a hole, so a row may hang off it.
fn hole_known(segment_scoped: bool, collars: &HashMap<String, Collar>, segments: &HashMap<String, Vec<TraceStation>>, dhid: &str) -> bool {
    if segment_scoped { segments.contains_key(dhid) } else { collars.contains_key(dhid) }
}

/// A dip or inclination is an angle from one vertical to the other. A number
/// outside that is a column that did not line up, not a steep hole.
fn is_dip_angle(value: f64) -> bool {
    (-90.0..=90.0).contains(&value)
}

fn is_azimuth(value: f64) -> bool {
    (0.0..=360.0).contains(&value)
}

fn angle_at(file: &CsvFile<'_>, row: &[String], index: usize) -> Option<f64> {
    let value = row.get(index).map(|value| value.trim()).unwrap_or("");
    if value.is_empty() || file.damaged(value) {
        return None;
    }
    finite_number(value)
}

/// A cell as a finite number, or `None` if it is blank or not one.
pub(super) fn finite_number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|number| number.is_finite())
}

/// What one pass over a survey file's angle columns found.
#[derive(Default)]
struct SurveyAngleScan {
    /// An inclination column is signed as dip already: every angle at or
    /// below zero, one of them below it. Exports name the column either way.
    /// Only a number that could be an angle votes, so a handful of corrupt
    /// rows cannot stand a whole file on its head. A hole driven along a seam
    /// crosses zero honestly, roof to floor and back, so signs on both sides
    /// are data rather than damage and leave the column read as its role.
    inclination_is_dip: bool,
    /// Rows whose azimuth is off the compass. They are counted and kept:
    /// squaring them away belongs to whatever wrote the file.
    azimuth_off_compass: usize,
    /// Rows whose tilt is not an angle at all, and whose direction is left
    /// out for that reason.
    tilt_not_an_angle: usize,
}

fn scan_survey_angles(file: &CsvFile<'_>, rows: &[Vec<String>]) -> SurveyAngleScan {
    let inclination = role_index(file.mapping, &CsvDrillColumnRole::Inclination);
    let dip = role_index(file.mapping, &CsvDrillColumnRole::Dip);
    let azimuth = role_index(file.mapping, &CsvDrillColumnRole::Azimuth);
    if inclination.is_none() && dip.is_none() && azimuth.is_none() {
        return SurveyAngleScan::default();
    }
    let mut any_negative = false;
    let mut any_positive = false;
    let mut azimuth_off_compass = 0usize;
    let mut tilt_not_an_angle = 0usize;
    for row in rows.iter().skip(1) {
        if let Some(number) = inclination.and_then(|index| angle_at(file, row, index)) {
            if is_dip_angle(number) {
                any_negative |= number < 0.0;
                any_positive |= number > 0.0;
            } else {
                tilt_not_an_angle += 1;
            }
        }
        if let Some(number) = dip.and_then(|index| angle_at(file, row, index))
            && !is_dip_angle(number)
        {
            tilt_not_an_angle += 1;
        }
        if let Some(number) = azimuth.and_then(|index| angle_at(file, row, index))
            && !is_azimuth(number)
        {
            azimuth_off_compass += 1;
        }
    }
    SurveyAngleScan {
        inclination_is_dip: any_negative && !any_positive,
        azimuth_off_compass,
        tilt_not_an_angle,
    }
}

/// A collar row's hole and position, or the reason the row is refused.
fn collar_gate(file: &CsvFile<'_>, row: &[String], row_index: usize) -> Result<(String, Collar), CsvDrillError> {
    check_width(file, row, row_index)?;
    let dhid = required_text(file, row, &CsvDrillColumnRole::Dhid, row_index)?;
    let collar = Collar {
        position: DVec3::new(
            required_number(file, row, &CsvDrillColumnRole::East, row_index)?,
            required_number(file, row, &CsvDrillColumnRole::North, row_index)?,
            required_number(file, row, &CsvDrillColumnRole::Elevation, row_index)?,
        ),
        diameter: optional_number(file, row, &CsvDrillColumnRole::Diameter, row_index)?,
    };
    validate_diameter(collar.diameter, &dhid)?;
    Ok((dhid, collar))
}

/// A survey row's hole and observation, or the reason the row is refused.
fn survey_gate(file: &CsvFile<'_>, row: &[String], row_index: usize, inclination_as_dip: bool) -> Result<(String, SurveyObservation), CsvDrillError> {
    check_width(file, row, row_index)?;
    let dhid = required_text(file, row, &CsvDrillColumnRole::Dhid, row_index)?;
    let position = optional_xyz(file, row, row_index, CsvDrillColumnRole::East, CsvDrillColumnRole::North, CsvDrillColumnRole::Elevation)?;
    let azimuth = optional_number(file, row, &CsvDrillColumnRole::Azimuth, row_index)?;
    let dip = match optional_number(file, row, &CsvDrillColumnRole::Dip, row_index)? {
        Some(dip) => Some(dip),
        // Inclination is read positive downward; dip is negative downward.
        None => optional_number(file, row, &CsvDrillColumnRole::Inclination, row_index)?.map(|value| if inclination_as_dip { value } else { -value }),
    };
    // A tilt that is not an angle leaves the row without a direction rather
    // than without a station: a surveyed position on the same row is still a
    // position, and the count is reported once per file.
    let dip = dip.filter(|dip| is_dip_angle(*dip));
    // A complete position stands without angles and a complete pair without a
    // position; only a row holding neither is refused.
    let angles = azimuth.zip(dip);
    if position.is_none() && angles.is_none() {
        return Err(CsvDrillError::Invalid(format!(
            "{} row {} has no complete XYZ or azimuth/dip geometry",
            file.mapping.path.display(),
            row_index + 1
        )));
    }
    let (azimuth, dip) = angles.unzip();
    let depth = required_number(file, row, &CsvDrillColumnRole::Depth, row_index)?;
    Ok((dhid, SurveyObservation { depth, azimuth, dip, position }))
}

/// A row is read by column index, so it must be as wide as its header.
fn check_width(file: &CsvFile<'_>, row: &[String], row_index: usize) -> Result<(), CsvDrillError> {
    if row.len() != file.headers.len() {
        return Err(CsvDrillError::Invalid(format!(
            "{} row {} has {} columns; expected {}",
            file.mapping.path.display(),
            row_index + 1,
            row.len(),
            file.headers.len()
        )));
    }
    Ok(())
}

/// What an interval row is keyed and placed by, read once per row.
struct IntervalKeys {
    dhid: String,
    from: f64,
    to: f64,
}

/// `to == from` is allowed: a picked horizon is a depth, not a range, and
/// refusing it would drop the pick that a surface is built from.
fn validate_interval(mapping: &CsvDrillFileMapping, from: f64, to: f64, dhid: &str, row_index: usize) -> Result<(), CsvDrillError> {
    if !from.is_finite() || !to.is_finite() || from < 0.0 || to < from {
        return Err(CsvDrillError::Invalid(format!(
            "{} row {} has invalid interval {from}..{to} for DHID '{dhid}'",
            mapping.path.display(),
            row_index + 1
        )));
    }
    Ok(())
}

/// What an interval row must satisfy before its attributes are read, and the
/// keys it is placed by; separate so the inference can ask it per row.
fn interval_gate(file: &CsvFile<'_>, row: &[String], row_index: usize) -> Result<IntervalKeys, CsvDrillError> {
    check_width(file, row, row_index)?;
    let dhid = required_text(file, row, &CsvDrillColumnRole::Dhid, row_index)?;
    let from = required_number(file, row, &CsvDrillColumnRole::From, row_index)?;
    let to = required_number(file, row, &CsvDrillColumnRole::To, row_index)?;
    validate_interval(file.mapping, from, to, &dhid, row_index)?;
    Ok(IntervalKeys { dhid, from, to })
}

/// A segment is geometry, not a log entry, so zero length is refused here: it
/// would leave the trace with two stations claiming one depth.
fn validate_segment(mapping: &CsvDrillFileMapping, from: f64, to: f64, dhid: &str, row_index: usize) -> Result<(), CsvDrillError> {
    validate_interval(mapping, from, to, dhid, row_index)?;
    if to <= from {
        return Err(CsvDrillError::Invalid(format!(
            "{} row {} has a zero-length segment at {from} for DHID '{dhid}'",
            mapping.path.display(),
            row_index + 1
        )));
    }
    Ok(())
}

fn validate_diameter(diameter: Option<f64>, dhid: &str) -> Result<(), CsvDrillError> {
    if diameter.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Err(CsvDrillError::Invalid(format!("DHID '{dhid}' has an invalid diameter")));
    }
    Ok(())
}

/// Overlap is reported, not refused: a seam is logged both whole and as its
/// plies, so a parent containing its own splits is correct data. Duplicates
/// overlap the same way, so the count is a prompt to look, not a verdict.
fn report_overlaps(intervals: &HashMap<String, Vec<DrillInterval>>) {
    // Reported by field: which column overlaps, across how many holes, is the
    // shape of the problem; the hole names are there to open one and look.
    let mut by_field: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut affected: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (dhid, rows) in intervals {
        let mut spans: HashMap<&str, Vec<(f64, f64)>> = HashMap::new();
        for row in rows {
            for key in row.values.keys() {
                spans.entry(key).or_default().push((row.from, row.to));
            }
        }
        for (field, mut ranges) in spans {
            ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
            if ranges.windows(2).any(|pair| pair[1].0 < pair[0].1 - 1.0e-9) {
                by_field.entry(field).or_default().push(dhid.as_str());
                affected.insert(dhid.as_str());
            }
        }
    }
    if by_field.is_empty() {
        return;
    }
    let mut fields = by_field.into_iter().collect::<Vec<_>>();
    // Worst field first, name within a tie, so a report is stable.
    fields.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
    let truncated = fields.len() > SKIP_REPORT_LIMIT;
    let mut summary = fields
        .iter_mut()
        .take(SKIP_REPORT_LIMIT)
        .map(|(field, holes)| {
            holes.sort_unstable();
            let examples = holes.iter().take(OVERLAP_EXAMPLE_LIMIT).copied().collect::<Vec<_>>().join(", ");
            format!("{field} in {} hole(s), e.g. {examples}", holes.len())
        })
        .collect::<Vec<_>>()
        .join("; ");
    if truncated {
        summary.push_str(", …");
    }
    userspace_warn!(
        "{}",
        crate::i18n::tr_format!(
            literal = "%holes% hole(s) carry overlapping intervals, such as a seam logged alongside its splits: %summary%",
            holes = affected.len().to_string(),
            summary = summary
        )
    );
}

/// One file as its rows are read: a mark is damage only if repair ran.
struct CsvFile<'a> {
    mapping: &'a CsvDrillFileMapping,
    headers: &'a [String],
    repaired: bool,
}

impl CsvFile<'_> {
    fn damaged(&self, value: &str) -> bool {
        self.repaired && value.contains(REPAIR_MARK)
    }
}

fn role_index(mapping: &CsvDrillFileMapping, role: &CsvDrillColumnRole) -> Option<usize> {
    mapping.columns.iter().position(|value| value == role)
}

fn required_text(file: &CsvFile<'_>, row: &[String], role: &CsvDrillColumnRole, row_index: usize) -> Result<String, CsvDrillError> {
    let value = role_index(file.mapping, role).and_then(|index| row.get(index)).map(|value| value.trim()).unwrap_or("");
    if file.damaged(value) {
        Err(CsvDrillError::Invalid(format!(
            "{} row {} has an unreadable value",
            file.mapping.path.display(),
            row_index + 1
        )))
    } else if value.is_empty() {
        Err(CsvDrillError::Invalid(format!(
            "{} row {} has a blank required value",
            file.mapping.path.display(),
            row_index + 1
        )))
    } else {
        Ok(value.to_owned())
    }
}

fn required_number(file: &CsvFile<'_>, row: &[String], role: &CsvDrillColumnRole, row_index: usize) -> Result<f64, CsvDrillError> {
    let value = required_text(file, row, role, row_index)?;
    finite_number(&value).ok_or_else(|| CsvDrillError::Invalid(format!("{} row {} has invalid number '{value}'", file.mapping.path.display(), row_index + 1)))
}

fn optional_number(file: &CsvFile<'_>, row: &[String], role: &CsvDrillColumnRole, row_index: usize) -> Result<Option<f64>, CsvDrillError> {
    let Some(index) = role_index(file.mapping, role) else {
        return Ok(None);
    };
    let value = row[index].trim();
    if value.is_empty() || file.damaged(value) {
        return Ok(None);
    }
    finite_number(value)
        .map(Some)
        .ok_or_else(|| CsvDrillError::Invalid(format!("{} row {} has invalid number '{value}'", file.mapping.path.display(), row_index + 1)))
}

fn optional_xyz(
    file: &CsvFile<'_>,
    row: &[String],
    row_index: usize,
    x_role: CsvDrillColumnRole,
    y_role: CsvDrillColumnRole,
    z_role: CsvDrillColumnRole,
) -> Result<Option<DVec3>, CsvDrillError> {
    if role_index(file.mapping, &x_role).is_none() || role_index(file.mapping, &y_role).is_none() || role_index(file.mapping, &z_role).is_none() {
        return Ok(None);
    }
    let values = [
        optional_number(file, row, &x_role, row_index)?,
        optional_number(file, row, &y_role, row_index)?,
        optional_number(file, row, &z_role, row_index)?,
    ];
    Ok(match values {
        [Some(x), Some(y), Some(z)] => Some(DVec3::new(x, y, z)),
        _ => None,
    })
}

struct ParsedCsv {
    rows: Vec<Vec<String>>,
    repaired_bytes: usize,
    repaired_cells: usize,
}

/// Refuse wide text from the head of a file that is streamed, not held.
pub(super) fn check_encoding(head: &[u8]) -> Result<(), CsvDrillError> {
    decode_entry(head).map(|_| ())
}

/// UTF-16 and UTF-32 exports are refused by name; a UTF-8 mark is dropped.
fn decode_entry(bytes: &[u8]) -> Result<&[u8], CsvDrillError> {
    const WIDE_MARKS: [&[u8]; 4] = [&[0xFF, 0xFE, 0x00, 0x00], &[0x00, 0x00, 0xFE, 0xFF], &[0xFF, 0xFE], &[0xFE, 0xFF]];
    if WIDE_MARKS.iter().any(|mark| bytes.starts_with(mark)) {
        return Err(CsvDrillError::Invalid("CSV is UTF-16 or UTF-32 text; save it as UTF-8 and import it again".into()));
    }
    // Wide text without a mark is NUL every other byte; a stray NUL is not.
    let head = &bytes[..bytes.len().min(NUL_SAMPLE_BYTES)];
    if head.iter().filter(|byte| **byte == 0).count() * 8 > head.len() {
        return Err(CsvDrillError::Invalid(
            "CSV holds NUL bytes throughout, so it is not UTF-8 text; if it was written as UTF-16 or UTF-32, save it as UTF-8 and import it again".into(),
        ));
    }
    Ok(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
}

/// Repairs invalid UTF-8 keeping bad bytes distinct: a lossy decode would fold
/// two hole IDs one stray byte apart into one key; each becomes mark plus hex.
pub(super) fn repair_utf8(bytes: &[u8], budget: usize) -> Option<(Vec<u8>, usize)> {
    const HEX: [u8; 16] = *b"0123456789ABCDEF";
    const MARK: [u8; 3] = [0xEF, 0xBF, 0xBD];
    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    let mut repaired = 0usize;
    loop {
        match std::str::from_utf8(rest) {
            Ok(text) => {
                out.extend_from_slice(text.as_bytes());
                return Some((out, repaired));
            }
            Err(error) => {
                let (valid, tail) = rest.split_at(error.valid_up_to());
                out.extend_from_slice(valid);
                // `error_len` is None at a truncated tail: all of it is bad.
                let bad = error.error_len().unwrap_or(tail.len());
                for byte in &tail[..bad] {
                    repaired += 1;
                    if repaired > budget {
                        return None;
                    }
                    out.extend_from_slice(&[MARK[0], MARK[1], MARK[2], HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0F)]]);
                }
                rest = &tail[bad..];
            }
        }
    }
}

/// Text as UTF-8: wide text is refused by name and a UTF-8 mark dropped.
/// Old exports carry stray bytes from whatever encoding the logger used; one
/// must not cost the file, so they are repaired, up to a tenth of the text:
/// past that it is a legacy encoding, or not text at all.
fn decode_text(bytes: &[u8]) -> Result<(std::borrow::Cow<'_, [u8]>, usize), CsvDrillError> {
    let bytes = decode_entry(bytes)?;
    if std::str::from_utf8(bytes).is_ok() {
        return Ok((std::borrow::Cow::Borrowed(bytes), 0));
    }
    let budget = (bytes.len() / 10).max(MINIMUM_REPAIR_BUDGET);
    let (fixed, count) = repair_utf8(bytes, budget).ok_or_else(|| {
        CsvDrillError::Invalid(crate::i18n::tr!(
            literal = "CSV has too many unreadable bytes to repair; it is probably in a legacy encoding, so save it as UTF-8 and import it again"
        ))
    })?;
    Ok((std::borrow::Cow::Owned(fixed), count))
}

fn parse_csv(bytes: &[u8]) -> Result<ParsedCsv, CsvDrillError> {
    // The caller reports a repair once per file.
    let (bytes, repaired_bytes) = decode_text(bytes)?;
    let bytes = &bytes[..];
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = Vec::new();
    let mut quoted = false;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if quoted {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    field.push(b'"');
                    index += 1;
                } else {
                    quoted = false;
                }
            } else {
                field.push(byte);
            }
        } else {
            match byte {
                b'"' if field.is_empty() => quoted = true,
                b',' => {
                    row.push(String::from_utf8(std::mem::take(&mut field)).expect("validated UTF-8"));
                }
                b'\n' => {
                    row.push(String::from_utf8(std::mem::take(&mut field)).expect("validated UTF-8"));
                    rows.push(std::mem::take(&mut row));
                }
                b'\r' => {}
                _ => field.push(byte),
            }
        }
        index += 1;
    }
    if quoted {
        return Err(CsvDrillError::Invalid("CSV has an unterminated quoted field".into()));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(String::from_utf8(field).expect("validated UTF-8"));
        rows.push(row);
    }
    let repaired_cells = if repaired_bytes > 0 {
        rows.iter().flatten().filter(|cell| cell.contains(REPAIR_MARK)).count()
    } else {
        0
    };
    Ok(ParsedCsv {
        rows,
        repaired_bytes,
        repaired_cells,
    })
}

/// The three tables one dataset is written back out as, in the shape this
/// module reads: every column naming a role is one the mapper matches
/// outright, and an interval's own fields arrive as attributes, so an export
/// re-imports with no mapping asked for.
pub(crate) struct CsvDrillBundle {
    pub(crate) collars: String,
    pub(crate) survey: String,
    pub(crate) intervals: String,
}

/// One cell, quoted only where a comma, quote or newline would otherwise end
/// it early.
fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn csv_row(cells: impl IntoIterator<Item = String>) -> String {
    let mut row = cells.into_iter().map(|cell| csv_cell(&cell)).collect::<Vec<_>>().join(",");
    row.push('\n');
    row
}

/// One dataset as collar, survey and interval tables.
///
/// Intervals carry the interpreted values, which is what every other reader
/// of a hole sees, and numbers are written at full precision because a
/// database reads this back rather than a person. A hole with no trace still
/// writes its collar and one vertical survey row, so a set round-trips
/// without losing the holes that are only a collar.
pub(crate) fn write_bundle(dataset: &DrillHoleDataset) -> CsvDrillBundle {
    // No depth column: the collar mapper has no role for one, so writing it
    // would be a column this module reads back as nothing.
    let mut collars = csv_row(["HOLEID", "EAST", "NORTH", "RL", "DIAMETER"].map(str::to_owned));
    let mut survey = csv_row(["HOLEID", "DEPTH", "AZIMUTH", "DIP"].map(str::to_owned));
    let mut interval_header = vec!["HOLEID".to_owned(), "FROM".to_owned(), "TO".to_owned()];
    interval_header.extend(dataset.fields.iter().map(|field| field.label.clone()));
    let mut intervals = csv_row(interval_header);

    for hole in &dataset.holes {
        let collar = hole.collar_position();
        // Diameter is a column the reader takes, so a hole that has one keeps
        // it; a hole without leaves the cell empty rather than inventing a size.
        let diameter = hole.diameter.map(|value| value.to_string()).unwrap_or_default();
        collars.push_str(&csv_row([hole.dhid.clone(), collar.x.to_string(), collar.y.to_string(), collar.z.to_string(), diameter]));

        // The reader steers each segment by the row above it, so a station
        // writes the bearing of the segment leaving it and the toe repeats the
        // one it arrived on. A hole with no downhole survey runs straight
        // down, and writes the one row that says so.
        let collar_only = [TraceStation { depth: 0.0, position: collar }];
        let trace = if hole.trace.is_empty() { &collar_only[..] } else { &hole.trace[..] };
        for (index, station) in trace.iter().enumerate() {
            let orientation = segment_orientation(trace, index).unwrap_or(STRAIGHT_DOWN);
            survey.push_str(&csv_row([
                hole.dhid.clone(),
                station.depth.to_string(),
                orientation.azimuth.to_string(),
                orientation.dip.to_string(),
            ]));
        }

        for interval in &hole.intervals {
            let mut row = vec![hole.dhid.clone(), interval.from.to_string(), interval.to.to_string()];
            row.extend(dataset.fields.iter().map(|field| match interval.values.get(&field.key) {
                Some(DrillValue::Numeric(number)) => number.to_string(),
                Some(DrillValue::Category(code)) => code.clone(),
                None => String::new(),
            }));
            intervals.push_str(&csv_row(row));
        }
    }

    CsvDrillBundle { collars, survey, intervals }
}

/// What a hole with no downhole survey is taken to do.
const STRAIGHT_DOWN: HoleOrientation = HoleOrientation { azimuth: 0.0, dip: -90.0 };

/// The bearing the survey row at `index` carries: the segment leaving the
/// station, or for the toe the one arriving. `None` where neither has a
/// length to take a bearing from.
fn segment_orientation(trace: &[TraceStation], index: usize) -> Option<HoleOrientation> {
    let leaving = trace.get(index + 1).map(|next| next.position - trace[index].position);
    let arriving = index.checked_sub(1).map(|above| trace[index].position - trace[above].position);
    leaving
        .into_iter()
        .chain(arriving)
        .find(|delta| delta.length_squared() > 1.0e-18)
        .map(direction_orientation)
}
