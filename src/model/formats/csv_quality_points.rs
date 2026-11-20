//! Composited quality values read from a CSV, one value per hole and seam.
//!
//! The wide shape holds one row per hole and seam and one column per quality,
//! its unit in a trailing bracket of the header (`Ash (%)`). The long shape
//! holds one row per hole, seam and quality, with Quality and Value columns
//! and an optional Unit; it is folded into the same sets as the wide one.

use std::{borrow::Cow, collections::HashMap, fmt, io};
#[cfg(not(target_arch = "wasm32"))]
use std::{fs::File, io::Read, path::Path};

use crate::{
    i18n::tr,
    model::formats::{
        csv_drill_hole::{PREVIEW_HEAD_BYTES, decode_text, normalize_header, strip_repair_marks},
        csv_records::{for_each_record, owned_fields},
    },
};

pub(crate) const PREVIEW_ROW_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum QualityColumnRole {
    Hole,
    East,
    North,
    Seam,
    /// Long shape: the quality's name.
    Quality,
    /// Long shape: the number. Wide shape: a quality column, named by its
    /// header.
    Value,
    /// Long shape only.
    Unit,
    Ignore,
}

impl QualityColumnRole {
    pub(crate) const ALL: [Self; 8] = [Self::Hole, Self::East, Self::North, Self::Seam, Self::Quality, Self::Value, Self::Unit, Self::Ignore];

    pub(crate) fn label(self, long: bool) -> String {
        match self {
            Self::Hole => tr!("common-hole-id"),
            Self::East => tr!("io-east-x"),
            Self::North => tr!("io-north-y"),
            Self::Seam => tr!("io-quality-role-seam"),
            Self::Quality => tr!("io-quality-role-quality"),
            Self::Value if long => tr!("io-quality-role-value"),
            Self::Value => tr!("io-quality-role-quality-column"),
            Self::Unit => tr!("io-quality-role-unit"),
            Self::Ignore => tr!("common-ignore"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QualityMapping {
    pub(crate) roles: Vec<QualityColumnRole>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QualityShape {
    Wide { columns: usize },
    Long,
}

impl QualityMapping {
    fn only(&self, role: QualityColumnRole) -> Option<usize> {
        let mut found = self.roles.iter().enumerate().filter(|(_, mapped)| **mapped == role).map(|(index, _)| index);
        let first = found.next();
        if found.next().is_some() { None } else { first }
    }

    fn count(&self, role: QualityColumnRole) -> usize {
        self.roles.iter().filter(|mapped| **mapped == role).count()
    }

    /// The shape the mapping reads, or `None` while a key column or the
    /// values are unmapped or mapped twice.
    pub(crate) fn shape(&self) -> Option<QualityShape> {
        use QualityColumnRole as Role;
        if [Role::Hole, Role::East, Role::North, Role::Seam].into_iter().any(|role| self.only(role).is_none()) || self.count(Role::Unit) > 1 {
            return None;
        }
        let values = self.count(Role::Value);
        match self.count(Role::Quality) {
            0 if values > 0 => Some(QualityShape::Wide { columns: values }),
            1 if values == 1 => Some(QualityShape::Long),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct QualityPreview {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
    pub(crate) mapping: QualityMapping,
}

/// One position and its value; the hole id is only a key while reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct QualityPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) value: f64,
}

/// Every value of one quality in one seam, in file order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct QualitySet {
    pub(crate) seam: String,
    pub(crate) quality: String,
    /// As written; blank when the file gives none.
    pub(crate) unit: String,
    pub(crate) points: Vec<QualityPoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParsedQualityPoints {
    /// In the order each seam and quality is first met.
    pub(crate) sets: Vec<QualitySet>,
    /// Values left blank: not analysed, so left out.
    pub(crate) blank: usize,
}

#[derive(Debug)]
pub(crate) enum QualityCsvError {
    Invalid(String),
    Io(io::Error),
}

impl fmt::Display for QualityCsvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for QualityCsvError {}

impl From<io::Error> for QualityCsvError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<csv::Error> for QualityCsvError {
    fn from(value: csv::Error) -> Self {
        Self::Invalid(value.to_string())
    }
}

pub(crate) fn preview(bytes: &[u8]) -> Result<QualityPreview, QualityCsvError> {
    preview_head(bytes, PREVIEW_HEAD_BYTES)
}

fn preview_head(bytes: &[u8], limit: usize) -> Result<QualityPreview, QualityCsvError> {
    let mut head = &bytes[..bytes.len().min(limit)];
    // A head cut short of the file ends at its last whole line.
    if head.len() < bytes.len()
        && let Some(end) = head.iter().rposition(|byte| *byte == b'\n')
    {
        head = &head[..=end];
    }
    let head = decoded(head)?;
    let mut records = Vec::new();
    for_each_record::<QualityCsvError>(&head, Some(PREVIEW_ROW_COUNT + 1), |_, record| {
        records.push(owned_fields(record));
        Ok(())
    })?;
    let headers = records.first().cloned().ok_or_else(|| QualityCsvError::Invalid(tr!("csv-drill-hole-csv-file-empty")))?;
    if headers.iter().all(|header| header.trim().is_empty()) {
        return Err(QualityCsvError::Invalid(tr!("csv-drill-hole-csv-header-has-no-columns")));
    }
    let rows = records.into_iter().skip(1).collect::<Vec<_>>();
    let mut mapping = auto_mapping(&headers);
    // A wide column of text only (a comment, a lithology) is no quality.
    if mapping.roles.contains(&QualityColumnRole::Quality) {
        return Ok(QualityPreview { headers, rows, mapping });
    }
    for (column, role) in mapping.roles.iter_mut().enumerate() {
        let mut cells = rows
            .iter()
            .filter_map(|row| row.get(column))
            .map(|cell| cell.trim())
            .filter(|cell| !cell.is_empty())
            .peekable();
        if *role == QualityColumnRole::Value && cells.peek().is_some() && cells.all(|cell| cell.parse::<f64>().is_err()) {
            *role = QualityColumnRole::Ignore;
        }
    }
    Ok(QualityPreview { headers, rows, mapping })
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn preview_path(path: &Path) -> Result<QualityPreview, QualityCsvError> {
    let mut head = Vec::new();
    File::open(path)?.take(PREVIEW_HEAD_BYTES as u64 + 1).read_to_end(&mut head)?;
    preview(&head)
}

fn role_alias(header: &str) -> Option<(QualityColumnRole, u8)> {
    use QualityColumnRole as Role;
    Some(match header {
        "hole" | "holeid" | "dhid" | "bhid" => (Role::Hole, 0),
        "id" => (Role::Hole, 1),
        "east" | "easting" | "x" => (Role::East, 0),
        "north" | "northing" | "y" => (Role::North, 0),
        "seam" | "seamname" | "seamcode" | "horizon" => (Role::Seam, 0),
        "quality" | "analyte" | "property" => (Role::Quality, 0),
        "value" | "result" => (Role::Value, 0),
        "unit" | "units" => (Role::Unit, 0),
        _ => return None,
    })
}

/// Each role goes to the first column of its best alias tier; the other
/// columns naming a role are ignored, as are blank headers. The rest are
/// quality columns in the wide shape and ignored in the long one.
pub(crate) fn auto_mapping(headers: &[String]) -> QualityMapping {
    use QualityColumnRole as Role;
    let aliases = headers.iter().map(|header| role_alias(&normalize_header(header))).collect::<Vec<_>>();
    let mut best: HashMap<Role, (u8, usize)> = HashMap::new();
    for (column, (role, tier)) in aliases.iter().enumerate().filter_map(|(column, alias)| alias.map(|alias| (column, alias))) {
        let entry = best.entry(role).or_insert((tier, column));
        if tier < entry.0 {
            *entry = (tier, column);
        }
    }
    let long = best.contains_key(&Role::Quality);
    let unclaimed = if long { Role::Ignore } else { Role::Value };
    let roles = (0..headers.len())
        .map(|column| match aliases[column] {
            Some((role, _)) if best.get(&role).is_some_and(|(_, chosen)| *chosen == column) => role,
            Some(_) => Role::Ignore,
            None if headers[column].trim().is_empty() => Role::Ignore,
            None => unclaimed,
        })
        .collect();
    QualityMapping { roles }
}

/// A wide header's name and the unit in its trailing bracket:
/// `Ash (%)` and `Ash [%]` are `Ash` in `%`.
pub(crate) fn split_unit(header: &str) -> (&str, &str) {
    let header = header.trim();
    for (open, close) in [('(', ')'), ('[', ']')] {
        if let Some(inner) = header.strip_suffix(close)
            && let Some(start) = inner.rfind(open)
            && !inner[..start].trim().is_empty()
        {
            return (inner[..start].trim(), inner[start + 1..].trim());
        }
    }
    (header, "")
}

/// A quality's text as shown: a byte the decode repaired keeps its mark and
/// loses its hex, as a drill-hole attribute's name does.
fn shown(text: &str) -> Cow<'_, str> {
    if text.contains(char::REPLACEMENT_CHARACTER) {
        Cow::Owned(strip_repair_marks(text))
    } else {
        Cow::Borrowed(text)
    }
}

/// The bytes as UTF-8, repaired as the drill-hole importer repairs them.
fn decoded(bytes: &[u8]) -> Result<Cow<'_, [u8]>, QualityCsvError> {
    decode_text(bytes).map(|(text, _)| text).map_err(|error| invalid(error.to_string()))
}

fn invalid(message: String) -> QualityCsvError {
    QualityCsvError::Invalid(message)
}

/// A number at or above zero; blank is `None`, everything else refuses the
/// file.
fn quality_value(field: &str, line: usize, column: &str) -> Result<Option<f64>, QualityCsvError> {
    let field = field.trim();
    if field.is_empty() {
        return Ok(None);
    }
    let value = coordinate(field, line, column)?;
    if value < 0.0 {
        return Err(invalid(tr!(
            "io-quality-points-negative",
            line = line.to_string(),
            column = column.to_owned(),
            value = field.to_owned()
        )));
    }
    Ok(Some(value))
}

fn coordinate(field: &str, line: usize, column: &str) -> Result<f64, QualityCsvError> {
    let field = field.trim();
    field.parse::<f64>().ok().filter(|value| value.is_finite()).ok_or_else(|| {
        invalid(tr!(
            "io-quality-points-not-a-number",
            line = line.to_string(),
            column = column.to_owned(),
            found = field.to_owned()
        ))
    })
}

fn key_text<'a>(field: &'a str, line: usize, column: &str) -> Result<&'a str, QualityCsvError> {
    let field = field.trim();
    if field.is_empty() {
        return Err(invalid(tr!("io-quality-points-blank", line = line.to_string(), column = column.to_owned())));
    }
    Ok(field)
}

/// Each name met, held once; values refer to it by its index.
#[derive(Default)]
struct Names(HashMap<String, usize>);

impl Names {
    fn id(&mut self, name: &str) -> usize {
        if let Some(&id) = self.0.get(name) {
            return id;
        }
        let id = self.0.len();
        self.0.insert(name.to_owned(), id);
        id
    }
}

/// Sets in first-met order, each value keyed by hole so a second one for
/// the same hole, seam and quality is refused rather than averaged.
#[derive(Default)]
struct Gather {
    sets: Vec<QualitySet>,
    holes: Names,
    seams: Names,
    qualities: Names,
    /// Seam and quality ids to the set.
    index: HashMap<(usize, usize), usize>,
    /// Hole, seam and quality ids to the line first holding them.
    lines: HashMap<(usize, usize, usize), usize>,
    /// One unit per quality id, whatever the seam; blank until one is given.
    units: Vec<String>,
    blank: usize,
}

impl Gather {
    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, line: usize, column: &str, hole: &str, seam: &str, quality: &str, unit: &str, x: f64, y: f64, value: Option<f64>) -> Result<(), QualityCsvError> {
        let (hole_id, seam_id, quality_id) = (self.holes.id(hole), self.seams.id(seam), self.qualities.id(quality));
        if let Some(first) = self.lines.insert((hole_id, seam_id, quality_id), line) {
            return Err(invalid(tr!(
                "io-quality-points-duplicate",
                first = first.to_string(),
                second = line.to_string(),
                quality = quality.to_owned(),
                hole = hole.to_owned(),
                seam = seam.to_owned()
            )));
        }
        let Some(value) = value else {
            self.blank += 1;
            return Ok(());
        };
        if self.units.len() <= quality_id {
            self.units.resize(quality_id + 1, String::new());
        }
        // A blank unit is one not given, never a second unit.
        let known = &mut self.units[quality_id];
        if known.is_empty() {
            unit.clone_into(known);
        } else if !unit.is_empty() && known != unit {
            return Err(unit_clash(line, column, quality, known, unit));
        }
        let next = self.sets.len();
        let set = *self.index.entry((seam_id, quality_id)).or_insert(next);
        if set == next {
            self.sets.push(QualitySet {
                seam: seam.to_owned(),
                quality: quality.to_owned(),
                unit: String::new(),
                points: Vec::new(),
            });
        }
        self.sets[set].points.push(QualityPoint { x, y, value });
        Ok(())
    }

    fn finish(mut self) -> ParsedQualityPoints {
        for set in &mut self.sets {
            set.unit = self.units[self.qualities.0[&set.quality]].clone();
        }
        ParsedQualityPoints {
            sets: self.sets,
            blank: self.blank,
        }
    }
}

fn unit_clash(line: usize, column: &str, quality: &str, first: &str, second: &str) -> QualityCsvError {
    invalid(tr!(
        "io-quality-points-unit-clash",
        line = line.to_string(),
        column = column.to_owned(),
        quality = quality.to_owned(),
        first = format!("\"{first}\""),
        second = format!("\"{second}\"")
    ))
}

/// The first bad cell refuses the whole file, naming its line and column: a
/// value dropped quietly would bend the surface built from the rest.
pub(crate) fn parse(bytes: &[u8], mapping: &QualityMapping) -> Result<ParsedQualityPoints, QualityCsvError> {
    use QualityColumnRole as Role;
    let shape = mapping.shape().ok_or_else(|| invalid(tr!("io-quality-points-needs")))?;
    let column = |role| mapping.roles.iter().position(|mapped| *mapped == role);
    let [hole, east, north, seam] = [Role::Hole, Role::East, Role::North, Role::Seam].map(|role| column(role).expect("the shape needs every key column"));
    let (quality, unit) = (column(Role::Quality), column(Role::Unit));
    let values = (0..mapping.roles.len()).filter(|index| mapping.roles[*index] == Role::Value).collect::<Vec<_>>();
    let bytes = decoded(bytes)?;
    let mut headers: Option<Vec<String>> = None;
    // Wide only: each value column with its quality's name and unit; a
    // column with a blank header is left out.
    let mut qualities: Vec<(usize, String, String)> = Vec::new();
    let mut gather = Gather::default();
    for_each_record(&bytes, None, |line, record| {
        let Some(headers) = headers.as_ref() else {
            let names = owned_fields(record);
            if names.len() != mapping.roles.len() {
                return Err(invalid(tr!(
                    "io-quality-points-column-count",
                    mapped = mapping.roles.len().to_string(),
                    found = names.len().to_string()
                )));
            }
            if shape != QualityShape::Long {
                for &index in &values {
                    let text = shown(&names[index]);
                    let (name, unit) = split_unit(&text);
                    if name.is_empty() {
                        continue;
                    }
                    if let Some((first, ..)) = qualities.iter().find(|(_, known, _)| known == name) {
                        return Err(invalid(tr!(
                            "io-quality-points-duplicate-column",
                            line = line.to_string(),
                            first = (first + 1).to_string(),
                            second = (index + 1).to_string(),
                            quality = name.to_owned()
                        )));
                    }
                    qualities.push((index, name.to_owned(), unit.to_owned()));
                }
            }
            headers = Some(names);
            return Ok(());
        };
        if record.iter().all(|field| field.trim().is_empty()) {
            return Ok(());
        }
        let cell = |index: usize| record.get(index).unwrap_or("");
        let hole_id = key_text(cell(hole), line, &headers[hole])?;
        let x = coordinate(cell(east), line, &headers[east])?;
        let y = coordinate(cell(north), line, &headers[north])?;
        let seam_name = key_text(cell(seam), line, &headers[seam])?;
        match quality {
            Some(quality) => {
                // The name loses a unit bracket as a wide header does; a
                // Unit cell, when filled, gives the unit.
                let text = shown(key_text(cell(quality), line, &headers[quality])?);
                let (name, bracket) = split_unit(&text);
                let given = unit.map_or("", |unit| cell(unit).trim());
                let unit_text = if given.is_empty() { bracket } else { given };
                let value = quality_value(cell(values[0]), line, &headers[values[0]])?;
                let named = match unit {
                    Some(unit) if !given.is_empty() => &headers[unit],
                    _ => &headers[quality],
                };
                gather.push(line, named, hole_id, seam_name, name, unit_text, x, y, value)?;
            }
            None => {
                for (index, name, unit) in &qualities {
                    let value = quality_value(cell(*index), line, &headers[*index])?;
                    gather.push(line, &headers[*index], hole_id, seam_name, name, unit, x, y, value)?;
                }
            }
        }
        Ok(())
    })?;
    if gather.sets.is_empty() {
        return Err(invalid(tr!("io-quality-points-empty")));
    }
    Ok(gather.finish())
}
