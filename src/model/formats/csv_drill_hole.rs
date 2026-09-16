//! Mapped CSV drillhole import. A bundle may describe relational collar,
//! survey and interval tables, or explicit measured-depth line segments.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    path::PathBuf,
};

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::{
    model::drill_hole::{DrillHole, DrillHoleDataset, DrillInterval, DrillValue, OrientationSource, SurveyObservation, TraceStation, resolve_trace},
    userspace_warn,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CsvDrillFileRole {
    Unassigned,
    Collar,
    Survey,
    Interval,
    ExplicitSegments,
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
    From,
    To,
    StartEast,
    StartNorth,
    StartElevation,
    EndEast,
    EndNorth,
    EndElevation,
    Diameter,
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
}

impl fmt::Display for CsvDrillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CsvDrillError {}
impl From<std::io::Error> for CsvDrillError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub(crate) fn preview(bytes: &[u8]) -> Result<CsvDrillPreview, CsvDrillError> {
    let rows = parse_csv(bytes)?.rows;
    let headers = rows.first().cloned().ok_or_else(|| CsvDrillError::Invalid("CSV file is empty".into()))?;
    if headers.is_empty() || headers.iter().all(|header| header.trim().is_empty()) {
        return Err(CsvDrillError::Invalid("CSV header has no columns".into()));
    }
    let mut seen = std::collections::HashSet::new();
    for header in &headers {
        let key = strip_repair_marks(header).trim().to_ascii_lowercase();
        if key.is_empty() || !seen.insert(key) {
            return Err(CsvDrillError::Invalid("CSV headers must be nonblank and unique".into()));
        }
    }
    Ok(CsvDrillPreview {
        headers,
        rows: rows.into_iter().skip(1).take(8).collect(),
    })
}

pub(crate) fn unassigned_mapping(path: PathBuf, preview: &CsvDrillPreview) -> CsvDrillFileMapping {
    CsvDrillFileMapping {
        path,
        columns: vec![CsvDrillColumnRole::Ignore; preview.headers.len()],
        role: CsvDrillFileRole::Unassigned,
    }
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
            "dip" | "inclination" => Some((CsvDrillColumnRole::Dip, 0)),
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

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn preview_path(path: &Path) -> Result<CsvDrillPreview, CsvDrillError> {
    preview(&std::fs::read(path)?)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn parse_paths(files: &[CsvDrillFileMapping]) -> Result<DrillHoleDataset, CsvDrillError> {
    let buffers = files
        .iter()
        .map(|mapping| std::fs::read(&mapping.path).map_err(CsvDrillError::Io))
        .collect::<Result<Vec<_>, _>>()?;
    parse_bundle(files.iter().zip(buffers.iter()).map(|(mapping, bytes)| (mapping, bytes.as_slice())))
}

/// Enough to find the bad rows without burying the console.
const SKIP_REPORT_LIMIT: usize = 10;

/// Hole names on an overlap report are a pointer to go and look, not a census.
const OVERLAP_EXAMPLE_LIMIT: usize = 3;

/// A short file may carry a handful of stray bytes outright.
const MINIMUM_REPAIR_BUDGET: usize = 16;

/// Enough of the head to tell wide text from a stray NUL.
const NUL_SAMPLE_BYTES: usize = 4096;

/// Written for an unreadable byte; in a repaired file it is never a value.
const REPAIR_MARK: char = '\u{FFFD}';

pub(crate) fn parse_bundle<'a>(inputs: impl IntoIterator<Item = (&'a CsvDrillFileMapping, &'a [u8])>) -> Result<DrillHoleDataset, CsvDrillError> {
    let inputs = inputs.into_iter().collect::<Vec<_>>();
    if inputs.is_empty() {
        return Err(CsvDrillError::Invalid("Select at least one CSV file".into()));
    }
    if let Some((mapping, _)) = inputs.iter().find(|(mapping, _)| mapping.role == CsvDrillFileRole::Unassigned) {
        return Err(CsvDrillError::Invalid(format!("Choose a file purpose for {}", mapping.path.display())));
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

    let mut skipped = 0usize;
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
        validate_roles(mapping)?;
        let file = CsvFile { mapping, headers, repaired };
        let stem = mapping.path.file_stem().and_then(|value| value.to_str()).unwrap_or("interval");
        // Once per row, not per attribute column, and only for FROM/TO files.
        let gates = matches!(mapping.role, CsvDrillFileRole::Interval | CsvDrillFileRole::ExplicitSegments).then(|| {
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
                    .filter(|(row_index, _)| gates.as_ref().is_none_or(|gates| gates[row_index - 1].is_ok()))
                    .filter_map(|(_, row)| row.get(index).map(|value| value.trim()))
                    .filter(|value| !value.is_empty() && !file.damaged(value))
                    .all(|value| value.parse::<f64>().is_ok_and(f64::is_finite))
            })
            .collect::<std::collections::HashSet<_>>();
        let mut interval_rows = 0usize;
        let mut interval_skipped = 0usize;
        let mut gates = gates.unwrap_or_default().into_iter();
        for (row_index, row) in rows.iter().enumerate().skip(1) {
            let gate = gates.next();
            if row.iter().all(|value| value.trim().is_empty()) {
                continue;
            }
            // Every malformed interval row is skipped, not only an unreadable
            // attribute: a blank hole ID or a dropped comma once cost the file.
            if mapping.role == CsvDrillFileRole::Interval {
                interval_rows += 1;
                match gate.expect("an interval file gates every row") {
                    Ok(keys) => {
                        let interval = interval_row(&file, row, &keys, stem, &numeric_attributes, &attribute_counts);
                        intervals.entry(keys.dhid).or_default().push(interval);
                    }
                    Err(error) => {
                        skipped += 1;
                        interval_skipped += 1;
                        if skipped <= SKIP_REPORT_LIMIT {
                            // The reason names its own file and row.
                            userspace_warn!("{}", crate::i18n::tr_format!(literal = "Skipped a row: %reason%", reason = error.to_string()));
                        }
                    }
                }
                continue;
            }
            if row.len() != headers.len() {
                return Err(CsvDrillError::Invalid(format!(
                    "{} row {} has {} columns; expected {}",
                    mapping.path.display(),
                    row_index + 1,
                    row.len(),
                    headers.len()
                )));
            }
            let dhid = required_text(&file, row, &CsvDrillColumnRole::Dhid, row_index)?;
            match mapping.role {
                CsvDrillFileRole::Unassigned => unreachable!("unassigned mappings are rejected before rows are parsed"),
                CsvDrillFileRole::Interval => unreachable!("interval rows are handled above so one bad row can be skipped"),
                CsvDrillFileRole::Collar => {
                    let collar = Collar {
                        position: DVec3::new(
                            required_number(&file, row, &CsvDrillColumnRole::East, row_index)?,
                            required_number(&file, row, &CsvDrillColumnRole::North, row_index)?,
                            required_number(&file, row, &CsvDrillColumnRole::Elevation, row_index)?,
                        ),
                        diameter: optional_number(&file, row, &CsvDrillColumnRole::Diameter, row_index)?,
                    };
                    validate_diameter(collar.diameter, &dhid)?;
                    if collars.insert(dhid.clone(), collar).is_some() {
                        return Err(CsvDrillError::Invalid(format!("duplicate collar DHID '{dhid}'")));
                    }
                }
                CsvDrillFileRole::Survey => {
                    let position = optional_xyz(&file, row, row_index, CsvDrillColumnRole::East, CsvDrillColumnRole::North, CsvDrillColumnRole::Elevation)?;
                    let azimuth = optional_number(&file, row, &CsvDrillColumnRole::Azimuth, row_index)?;
                    let dip = optional_number(&file, row, &CsvDrillColumnRole::Dip, row_index)?;
                    if azimuth.is_some() != dip.is_some() {
                        return Err(CsvDrillError::Invalid(format!(
                            "{} row {} has a partial azimuth/dip orientation",
                            mapping.path.display(),
                            row_index + 1
                        )));
                    }
                    if position.is_none() && azimuth.is_none() {
                        return Err(CsvDrillError::Invalid(format!(
                            "{} row {} has no XYZ or azimuth/dip geometry",
                            mapping.path.display(),
                            row_index + 1
                        )));
                    }
                    surveys.entry(dhid).or_default().push(SurveyObservation {
                        depth: required_number(&file, row, &CsvDrillColumnRole::Depth, row_index)?,
                        azimuth,
                        dip,
                        position,
                    });
                }
                CsvDrillFileRole::ExplicitSegments => {
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
                }
            }
        }
        // A few unreadable rows are dirty data; most of a file failing is a
        // mapping mistake; carrying on would report success and draw nothing.
        if interval_rows > 0 && interval_skipped * 2 > interval_rows {
            return Err(CsvDrillError::Invalid(format!(
                "{}: {interval_skipped} of {interval_rows} interval rows could not be read; check the FROM and TO column mapping",
                mapping.path.display()
            )));
        }
    }

    if skipped > SKIP_REPORT_LIMIT {
        userspace_warn!("{}", crate::i18n::tr_format!(literal = "%count% rows were skipped in total", count = skipped.to_string()));
    }

    let known = if segment_files == 1 {
        segment_stations.keys().collect::<std::collections::HashSet<_>>()
    } else {
        collars.keys().collect()
    };
    for dhid in surveys.keys().chain(intervals.keys()) {
        if !known.contains(dhid) {
            return Err(CsvDrillError::Invalid(format!("rows reference unknown DHID '{dhid}'")));
        }
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
            let angles = count(&CsvDrillColumnRole::Azimuth) == 1 && count(&CsvDrillColumnRole::Dip) == 1;
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
            DrillValue::Numeric(raw.parse::<f64>().expect("column-wide numeric inference validated this value"))
        } else {
            DrillValue::Category(raw.to_owned())
        };
        values.insert(key, value);
    }
    DrillInterval {
        from: keys.from,
        to: keys.to,
        values,
    }
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
    if row.len() != file.headers.len() {
        return Err(CsvDrillError::Invalid(format!(
            "{} row {} has {} columns; expected {}",
            file.mapping.path.display(),
            row_index + 1,
            row.len(),
            file.headers.len()
        )));
    }
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
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| CsvDrillError::Invalid(format!("{} row {} has invalid number '{value}'", file.mapping.path.display(), row_index + 1)))
}

fn optional_number(file: &CsvFile<'_>, row: &[String], role: &CsvDrillColumnRole, row_index: usize) -> Result<Option<f64>, CsvDrillError> {
    let Some(index) = role_index(file.mapping, role) else {
        return Ok(None);
    };
    let value = row[index].trim();
    if value.is_empty() || file.damaged(value) {
        return Ok(None);
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
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
    match values {
        [Some(x), Some(y), Some(z)] => Ok(Some(DVec3::new(x, y, z))),
        [None, None, None] => Ok(None),
        _ => Err(CsvDrillError::Invalid(format!(
            "{} row {} has a partial XYZ coordinate",
            file.mapping.path.display(),
            row_index + 1
        ))),
    }
}

struct ParsedCsv {
    rows: Vec<Vec<String>>,
    repaired_bytes: usize,
    repaired_cells: usize,
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
fn repair_utf8(bytes: &[u8], budget: usize) -> Option<(Vec<u8>, usize)> {
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

fn parse_csv(bytes: &[u8]) -> Result<ParsedCsv, CsvDrillError> {
    // Old exports carry stray bytes from whatever encoding the logger used; one
    // must not cost the file, so text is repaired; the caller reports it once.
    let bytes = decode_entry(bytes)?;
    let repaired;
    let (bytes, repaired_bytes) = match std::str::from_utf8(bytes) {
        Ok(_) => (bytes, 0),
        Err(_) => {
            // A legacy encoding marks most rows; a binary is far past a tenth.
            let budget = (bytes.len() / 10).max(MINIMUM_REPAIR_BUDGET);
            let (fixed, count) = repair_utf8(bytes, budget).ok_or_else(|| {
                CsvDrillError::Invalid("CSV has too many unreadable bytes to repair; it is probably in a legacy encoding, so save it as UTF-8 and import it again".into())
            })?;
            repaired = fixed;
            (repaired.as_slice(), count)
        }
    };
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
