//! CSV block-model import/export.
//!
//! Geometry columns describe block centroids (`x`, `y`, `z`) and full block
//! dimensions (`dx`, `dy`, `dz`). `Value` and optional `ID` columns become
//! numeric block-model variables, `Category` columns preserve text labels,
//! and `Ignore` columns are discarded.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    io::{self, Read, Write},
    sync::Arc,
};
#[cfg(not(target_arch = "wasm32"))]
use std::{fs::File, path::Path};

use csv::StringRecord;
use glam::DVec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::model::{
    block_model::{BlockBounds, BlockBoundsSource, BlockGeometryBuilder, RenderableBlockIndices, color_variable_default, is_no_data_sentinel},
    formats::{
        block_column::BlockColumnBuilder,
        block_model_data::{BlockModelColumn, BlockModelData},
        csv_records::{first_record, for_each_record, owned_fields},
    },
};

pub(crate) const PREVIEW_ROW_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CsvColumnRole {
    #[serde(alias = "Resource")]
    Value,
    Category,
    Ignore,
    Id,
    X,
    Y,
    Z,
    Dx,
    Dy,
    Dz,
}

impl CsvColumnRole {
    pub(crate) const ALL: [Self; 10] = [Self::Value, Self::Category, Self::Ignore, Self::Id, Self::X, Self::Y, Self::Z, Self::Dx, Self::Dy, Self::Dz];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Value => crate::i18n::tr!("csv-block-model-value"),
            Self::Category => crate::i18n::tr!("csv-block-model-category"),
            Self::Ignore => crate::i18n::tr!("common-ignore"),
            Self::Id => "ID".to_owned(),
            Self::X => "x".to_owned(),
            Self::Y => "y".to_owned(),
            Self::Z => "z".to_owned(),
            Self::Dx => "dx".to_owned(),
            Self::Dy => "dy".to_owned(),
            Self::Dz => "dz".to_owned(),
        }
    }
}

/// A block dimension the CSV doesn't carry a column for, entered by hand
/// instead (e.g. every block is a fixed 30m x 30m x 30m). Wraps `f64` with a
/// bit-pattern `Eq`/`Ord`/`Hash` so `CsvColumnMapping` keeps deriving them -
/// it's part of `BlockModelSource`, used as a job-dedup key.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct FixedDimension(pub(crate) f64);

impl PartialEq for FixedDimension {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for FixedDimension {}

impl PartialOrd for FixedDimension {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FixedDimension {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.to_bits().cmp(&other.0.to_bits())
    }
}

impl std::hash::Hash for FixedDimension {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CsvColumnMapping {
    pub(crate) roles: Vec<CsvColumnRole>,
    /// Fixed block sizes for the `Dx`/`Dy`/`Dz` axes, used when no column is
    /// mapped to that role - for models whose CSV only carries centroids.
    #[serde(default)]
    pub(crate) fixed_dx: Option<FixedDimension>,
    #[serde(default)]
    pub(crate) fixed_dy: Option<FixedDimension>,
    #[serde(default)]
    pub(crate) fixed_dz: Option<FixedDimension>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CsvPreview {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
    pub(crate) mapping: CsvColumnMapping,
}

#[derive(Debug)]
pub(crate) enum CsvBlockModelError {
    Invalid(String),
    Io(io::Error),
}

impl fmt::Display for CsvBlockModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CsvBlockModelError {}

impl From<io::Error> for CsvBlockModelError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<csv::Error> for CsvBlockModelError {
    fn from(value: csv::Error) -> Self {
        Self::Invalid(value.to_string())
    }
}

#[derive(Debug)]
pub(crate) struct ParsedCsvBlockModel {
    pub(crate) model: BlockModelData,
    pub(crate) blocks: BlockBoundsSource,
}

pub(crate) fn preview(bytes: &[u8]) -> Result<CsvPreview, CsvBlockModelError> {
    let mut records = Vec::new();
    for_each_record::<CsvBlockModelError>(bytes, Some(PREVIEW_ROW_COUNT + 1), |_, record| {
        records.push(owned_fields(record));
        Ok(())
    })?;
    let headers = records.first().cloned().ok_or_else(|| CsvBlockModelError::Invalid("CSV file is empty".to_owned()))?;
    if headers.is_empty() {
        return Err(CsvBlockModelError::Invalid("CSV header has no columns".to_owned()));
    }
    let rows = records.into_iter().skip(1).collect::<Vec<_>>();
    let mut mapping = auto_mapping(&headers);
    // Text columns are overwhelmingly likely to be categories. Infer those
    // from the preview while leaving numeric-looking category codes as Value;
    // users can explicitly select Category for that ambiguous case.
    for (column, role) in mapping.roles.iter_mut().enumerate() {
        if *role == CsvColumnRole::Value
            && rows
                .iter()
                .filter_map(|row| row.get(column))
                .map(|field| field.trim())
                .any(|field| !field.is_empty() && field.parse::<f64>().is_err())
        {
            *role = CsvColumnRole::Category;
        }
    }
    Ok(CsvPreview { headers, rows, mapping })
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn preview_path(path: &Path) -> Result<CsvPreview, CsvBlockModelError> {
    // A preview needs only the header and five records. Cap the initial read
    // so selecting a multi-gigabyte model never duplicates the whole file on
    // the UI thread; `preview` stops parsing as soon as those rows are found.
    const PREVIEW_READ_LIMIT: u64 = 8 * 1024 * 1024;
    let mut bytes = Vec::new();
    File::open(path)?.take(PREVIEW_READ_LIMIT).read_to_end(&mut bytes)?;
    preview(&bytes)
}

pub(crate) fn auto_mapping(headers: &[String]) -> CsvColumnMapping {
    let mut claimed = HashSet::new();
    let roles = headers
        .iter()
        .map(|header| {
            let role = match header.trim().to_ascii_lowercase().as_str() {
                "x" => CsvColumnRole::X,
                "y" => CsvColumnRole::Y,
                "z" => CsvColumnRole::Z,
                "dx" => CsvColumnRole::Dx,
                "dy" => CsvColumnRole::Dy,
                "dz" => CsvColumnRole::Dz,
                "id" => CsvColumnRole::Id,
                _ => CsvColumnRole::Value,
            };
            if matches!(role, CsvColumnRole::Value | CsvColumnRole::Category | CsvColumnRole::Ignore) || claimed.insert(role) {
                role
            } else {
                // A second identically named semantic column remains a value.
                // This preserves round-tripping when a value itself is named x.
                CsvColumnRole::Value
            }
        })
        .collect();
    CsvColumnMapping {
        roles,
        fixed_dx: None,
        fixed_dy: None,
        fixed_dz: None,
    }
}

pub(crate) fn validate_mapping(mapping: &CsvColumnMapping, column_count: usize) -> Result<(), CsvBlockModelError> {
    if mapping.roles.len() != column_count {
        return Err(CsvBlockModelError::Invalid(format!(
            "CSV mapping has {} columns but the file has {column_count}",
            mapping.roles.len()
        )));
    }
    for required in [CsvColumnRole::X, CsvColumnRole::Y, CsvColumnRole::Z] {
        let count = mapping.roles.iter().filter(|role| **role == required).count();
        if count != 1 {
            return Err(CsvBlockModelError::Invalid(format!(
                "Select exactly one '{}' column (currently selected {count})",
                required.label()
            )));
        }
    }
    for (required, fixed) in [
        (CsvColumnRole::Dx, mapping.fixed_dx),
        (CsvColumnRole::Dy, mapping.fixed_dy),
        (CsvColumnRole::Dz, mapping.fixed_dz),
    ] {
        let count = mapping.roles.iter().filter(|role| **role == required).count();
        if count > 1 {
            return Err(CsvBlockModelError::Invalid(format!(
                "Select at most one '{}' column (currently selected {count})",
                required.label()
            )));
        }
        if count == 0 && !fixed.is_some_and(|fixed| fixed.0.is_finite() && fixed.0 > 0.0) {
            return Err(CsvBlockModelError::Invalid(format!(
                "Select a '{}' column, or set a fixed size greater than zero",
                required.label()
            )));
        }
    }
    let id_count = mapping.roles.iter().filter(|role| **role == CsvColumnRole::Id).count();
    if id_count > 1 {
        return Err(CsvBlockModelError::Invalid(format!("Select at most one 'ID' column (currently selected {id_count})")));
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn parse(bytes: &[u8], mapping: &CsvColumnMapping) -> Result<ParsedCsvBlockModel, CsvBlockModelError> {
    parse_stream(bytes, mapping, |_| {})
}

/// How much text one thread parses at a time.
const CHUNK_BYTES: usize = 16 * 1024 * 1024;

/// Which columns of the file mean what, settled from its header.
struct CsvLayout<'a> {
    mapping: &'a CsvColumnMapping,
    headers: Vec<String>,
    role_indices: BTreeMap<CsvColumnRole, usize>,
    resource_indices: Vec<usize>,
    resource_roles: Vec<CsvColumnRole>,
}

impl CsvLayout<'_> {
    fn has_ids(&self) -> bool {
        self.resource_roles.contains(&CsvColumnRole::Id)
    }
}

/// What one chunk of text held, up to its first bad row.
#[derive(Default)]
struct ParsedChunk {
    blocks: Vec<BlockBounds>,
    columns: Vec<Vec<f64>>,
    /// Per column, the labels this chunk's codes in `columns` count from.
    labels: Vec<Vec<String>>,
    /// The line each row started on, kept only to name a duplicate ID.
    lines: Vec<usize>,
    error: Option<CsvBlockModelError>,
}

/// Read a CSV model a window of text at a time, parsing each window's chunks
/// side by side and encoding its rows before the next window is read, so what
/// is held at once is the finished part of the model and one window.
pub(crate) fn parse_stream(mut input: impl Read, mapping: &CsvColumnMapping, mut report_read: impl FnMut(u64)) -> Result<ParsedCsvBlockModel, CsvBlockModelError> {
    let mut window_bytes = CHUNK_BYTES * rayon::current_num_threads().max(1);
    let mut buffer: Vec<u8> = Vec::new();
    let mut read_so_far = 0u64;
    let mut at_end = false;
    let mut next_line = 1usize;
    let mut layout: Option<CsvLayout<'_>> = None;
    let mut resource_names = Vec::new();
    let mut geometry = BlockGeometryBuilder::default();
    let mut builders: Vec<BlockColumnBuilder> = Vec::new();
    let mut categories: Vec<HashMap<String, u32>> = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut block_count = 0usize;
    let mut overall_lower = DVec3::splat(f64::INFINITY);
    let mut overall_upper = DVec3::splat(f64::NEG_INFINITY);

    while !at_end {
        let held = buffer.len();
        buffer.resize(window_bytes.max(held + CHUNK_BYTES), 0);
        let mut filled = held;
        while filled < buffer.len() {
            let read = input.read(&mut buffer[filled..])?;
            if read == 0 {
                at_end = true;
                break;
            }
            filled += read;
        }
        buffer.truncate(filled);
        read_so_far += (filled - held) as u64;
        report_read(read_so_far);

        let mut ends = record_ends(&buffer, CHUNK_BYTES);
        if at_end && ends.last() != Some(&buffer.len()) && !buffer.is_empty() {
            ends.push(buffer.len());
        }
        let Some(&usable) = ends.last() else {
            // One record longer than the window, or no line ends at all: the
            // next pass reads more before looking again.
            window_bytes = window_bytes.saturating_mul(2);
            continue;
        };

        let mut start = 0;
        if layout.is_none() {
            let Some((record, used)) = first_record(&buffer[..usable])? else {
                buffer.clear();
                continue;
            };
            let found = CsvLayout::from_header(&record, mapping)?;
            resource_names = unique_resource_names(&found.headers, mapping);
            builders = (0..found.resource_indices.len()).map(|_| BlockColumnBuilder::default()).collect();
            categories = (0..found.resource_indices.len()).map(|_| HashMap::new()).collect();
            layout = Some(found);
            next_line += count_newlines(&buffer[..used]);
            start = used;
        }
        let layout_ref = layout.as_ref().expect("the header is settled above");

        let mut lines = Vec::new();
        let mut line = next_line;
        let mut chunks = Vec::new();
        let mut from = start;
        for &end in &ends {
            if end > from {
                chunks.push(&buffer[from..end]);
                lines.push(line);
                line += count_newlines(&buffer[from..end]);
                from = end;
            }
        }
        next_line = line;
        let parsed: Vec<ParsedChunk> = chunks.par_iter().zip(&lines).map(|(text, &first_line)| parse_chunk(text, first_line, layout_ref)).collect();

        for mut chunk in parsed {
            merge_categories(layout_ref, &mut chunk, &mut categories)?;
            if layout_ref.has_ids() {
                check_ids(layout_ref, &chunk, &mut seen_ids)?;
            }
            for block in &chunk.blocks {
                overall_lower = overall_lower.min(block.lower);
                overall_upper = overall_upper.max(block.upper);
            }
            block_count += chunk.blocks.len();
            geometry.extend(&chunk.blocks);
            builders.par_iter_mut().zip(&chunk.columns).for_each(|(builder, values)| builder.extend_slice(values));
            if let Some(error) = chunk.error {
                return Err(error);
            }
        }
        buffer.drain(..usable);
    }

    let layout = layout.ok_or_else(|| CsvBlockModelError::Invalid("CSV file is empty".to_owned()))?;
    if block_count == 0 {
        return Err(CsvBlockModelError::Invalid("CSV file has no block rows".to_owned()));
    }
    let geometry = geometry.finish();
    let retained_geometry_bytes = geometry.heap_bytes();
    let columns = resource_names
        .into_iter()
        .zip(builders)
        .zip(layout.resource_roles.iter().copied())
        .zip(categories)
        .map(|(((name, values), role), categories)| BlockModelColumn {
            name,
            values: Arc::new(values.finish()),
            categories: (role == CsvColumnRole::Category).then(|| categories.into_iter().map(|(label, code)| (code, label)).collect()),
            category_colors: BTreeMap::new(),
        })
        .collect();
    let model = BlockModelData::from_columns(block_count, overall_lower, overall_upper, columns, retained_geometry_bytes)
        .map_err(|error| CsvBlockModelError::Invalid(error.to_string()))?;
    Ok(ParsedCsvBlockModel {
        model,
        blocks: BlockBoundsSource::Explicit(geometry),
    })
}

fn count_newlines(text: &[u8]) -> usize {
    text.iter().filter(|&&byte| byte == b'\n').count()
}

/// Where the records of `text` can be cut into pieces of at least `chunk`
/// bytes: after line ends outside quotes, the last of them included. A file
/// with no quotes at all needs no walking over.
fn record_ends(text: &[u8], chunk: usize) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut last_cut = 0;
    if text.contains(&b'"') {
        let (mut quoted, mut last_end) = (false, 0);
        for (index, &byte) in text.iter().enumerate() {
            if byte == b'"' {
                quoted = !quoted;
            } else if byte == b'\n' && !quoted {
                last_end = index + 1;
                if last_end - last_cut >= chunk {
                    ends.push(last_end);
                    last_cut = last_end;
                }
            }
        }
        if last_end > last_cut {
            ends.push(last_end);
        }
        return ends;
    }
    while let Some(found) = text.get(last_cut + chunk..).and_then(|rest| rest.iter().position(|&byte| byte == b'\n')) {
        last_cut += chunk + found + 1;
        ends.push(last_cut);
    }
    if let Some(last) = text.iter().rposition(|&byte| byte == b'\n')
        && last + 1 > last_cut
    {
        ends.push(last + 1);
    }
    ends
}

impl<'a> CsvLayout<'a> {
    fn from_header(record: &StringRecord, mapping: &'a CsvColumnMapping) -> Result<Self, CsvBlockModelError> {
        validate_mapping(mapping, record.len())?;
        let headers = owned_fields(record);
        let (mut role_indices, mut resource_indices, mut resource_roles) = (BTreeMap::new(), Vec::new(), Vec::new());
        for (index, role) in mapping.roles.iter().copied().enumerate() {
            if matches!(role, CsvColumnRole::Value | CsvColumnRole::Category | CsvColumnRole::Id) {
                resource_indices.push(index);
                resource_roles.push(role);
            } else if role != CsvColumnRole::Ignore {
                role_indices.insert(role, index);
            }
        }
        Ok(Self {
            mapping,
            headers,
            role_indices,
            resource_indices,
            resource_roles,
        })
    }
}

fn parse_chunk(text: &[u8], first_line: usize, layout: &CsvLayout<'_>) -> ParsedChunk {
    let CsvLayout {
        mapping,
        headers,
        role_indices,
        resource_indices,
        resource_roles,
    } = layout;
    let columns = resource_indices.len();
    // Sized for this chunk's rows from its first line's length, so the columns
    // are not grown, and overshot, by doubling.
    let row_guess = text.iter().position(|&byte| byte == b'\n').map_or(1, |length| text.len() / (length + 1).max(1) + 1);
    let mut chunk = ParsedChunk {
        blocks: Vec::with_capacity(row_guess),
        columns: (0..columns).map(|_| Vec::with_capacity(row_guess)).collect(),
        labels: vec![Vec::new(); columns],
        ..ParsedChunk::default()
    };
    let mut local: Vec<HashMap<String, u32>> = vec![HashMap::new(); columns];
    let has_ids = layout.has_ids();
    let result = for_each_record(text, None, |line, record| {
        let record_number = first_line + line - 1;
        if record.iter().all(|field| field.trim().is_empty()) {
            return Ok(());
        }
        if record.len() > headers.len() {
            return Err(CsvBlockModelError::Invalid(format!(
                "CSV row {record_number} has {} fields; the header has {}",
                record.len(),
                headers.len()
            )));
        }
        let value = |role| {
            let index = role_indices[&role];
            parse_required_number(record.get(index).unwrap_or(""), record_number, &headers[index])
        };
        let dimension = |role, fixed: Option<FixedDimension>| -> Result<f64, CsvBlockModelError> {
            if role_indices.contains_key(&role) {
                value(role)
            } else {
                // Validated by `validate_mapping`: a role with no mapped column
                // always has a fixed size set.
                Ok(fixed.expect("validate_mapping requires a fixed size when the column is unmapped").0)
            }
        };
        let center = DVec3::new(value(CsvColumnRole::X)?, value(CsvColumnRole::Y)?, value(CsvColumnRole::Z)?);
        let size = DVec3::new(
            dimension(CsvColumnRole::Dx, mapping.fixed_dx)?,
            dimension(CsvColumnRole::Dy, mapping.fixed_dy)?,
            dimension(CsvColumnRole::Dz, mapping.fixed_dz)?,
        );
        if !center.is_finite() {
            return Err(CsvBlockModelError::Invalid(format!("CSV row {record_number} contains a non-finite block position")));
        }
        if !size.is_finite() || size.min_element() <= 0.0 {
            return Err(CsvBlockModelError::Invalid(format!(
                "CSV row {record_number} block sizes must all be finite and greater than zero"
            )));
        }
        // Each field is checked before any of the row is kept, so a bad row
        // leaves the columns the same length as the blocks.
        let row_start = chunk.columns.first().map_or(0, Vec::len);
        for (resource, &index) in resource_indices.iter().enumerate() {
            let field = record.get(index).unwrap_or("").trim();
            let role = resource_roles[resource];
            let number = if role == CsvColumnRole::Category {
                if field.is_empty() {
                    f64::NAN
                } else if let Some(&code) = local[resource].get(field) {
                    f64::from(code)
                } else {
                    let code = chunk.labels[resource].len() as u32;
                    local[resource].insert(field.to_owned(), code);
                    chunk.labels[resource].push(field.to_owned());
                    f64::from(code)
                }
            } else {
                let parsed = if field.is_empty() && role == CsvColumnRole::Value {
                    f64::NAN
                } else {
                    match field.parse::<f64>() {
                        Ok(parsed) => parsed,
                        Err(_) => {
                            chunk.columns[..resource].iter_mut().for_each(|column| column.truncate(row_start));
                            let expectation = if role == CsvColumnRole::Id { "a numeric ID" } else { "a number or blank" };
                            return Err(CsvBlockModelError::Invalid(format!(
                                "CSV row {record_number}, column '{}': expected {expectation}, found '{field}'",
                                headers[index]
                            )));
                        }
                    }
                };
                if role == CsvColumnRole::Id && !parsed.is_finite() {
                    chunk.columns[..resource].iter_mut().for_each(|column| column.truncate(row_start));
                    return Err(CsvBlockModelError::Invalid(format!(
                        "CSV row {record_number}, column '{}': ID must be finite",
                        headers[index]
                    )));
                }
                parsed
            };
            chunk.columns[resource].push(number);
        }
        let half = size * 0.5;
        chunk.blocks.push(BlockBounds {
            lower: center - half,
            upper: center + half,
        });
        if has_ids {
            chunk.lines.push(record_number);
        }
        Ok(())
    });
    chunk.error = result.err();
    chunk
}

/// Give a chunk's categories their codes for the whole file, numbered in the
/// order the file first mentions them, whichever thread parsed what.
fn merge_categories(layout: &CsvLayout<'_>, chunk: &mut ParsedChunk, categories: &mut [HashMap<String, u32>]) -> Result<(), CsvBlockModelError> {
    for resource in 0..layout.resource_indices.len() {
        if layout.resource_roles[resource] != CsvColumnRole::Category {
            continue;
        }
        let known = &mut categories[resource];
        let codes = chunk.labels[resource]
            .iter()
            .map(|label| {
                if let Some(&code) = known.get(label) {
                    return Ok(code);
                }
                let code = u32::try_from(known.len()).map_err(|_| {
                    CsvBlockModelError::Invalid(format!(
                        "CSV column '{}' has more categories than Incline Design can represent",
                        layout.headers[layout.resource_indices[resource]]
                    ))
                })?;
                known.insert(label.clone(), code);
                Ok(code)
            })
            .collect::<Result<Vec<u32>, CsvBlockModelError>>()?;
        for value in &mut chunk.columns[resource] {
            if !value.is_nan() {
                *value = f64::from(codes[*value as usize]);
            }
        }
    }
    Ok(())
}

fn check_ids(layout: &CsvLayout<'_>, chunk: &ParsedChunk, seen: &mut HashSet<u64>) -> Result<(), CsvBlockModelError> {
    let Some(resource) = layout.resource_roles.iter().position(|role| *role == CsvColumnRole::Id) else {
        return Ok(());
    };
    for (&id, &line) in chunk.columns[resource].iter().zip(&chunk.lines) {
        let key = if id == 0.0 { 0 } else { id.to_bits() };
        if !seen.insert(key) {
            return Err(CsvBlockModelError::Invalid(format!(
                "CSV row {line}, column '{}': duplicate ID '{id}'",
                layout.headers[layout.resource_indices[resource]]
            )));
        }
    }
    Ok(())
}

pub(crate) fn write<W: Write>(model: &BlockModelData, blocks: &BlockBoundsSource, renderable: &RenderableBlockIndices, output: W) -> Result<(), CsvBlockModelError> {
    if blocks.len() != model.metadata.n_blocks {
        return Err(CsvBlockModelError::Invalid(format!(
            "block model has {} geometry rows but {} data rows",
            blocks.len(),
            model.metadata.n_blocks
        )));
    }
    let variables: Vec<_> = model.color_variables().into_iter().filter(|variable| !variable.special).collect();
    let empty_values: Vec<_> = variables.iter().map(|variable| color_variable_default(variable)).collect();
    let mut output = csv::Writer::from_writer(output);
    output.write_record(["x", "y", "z", "dx", "dy", "dz"].into_iter().chain(variables.iter().map(|variable| variable.name.as_str())))?;

    const ROW_CHUNK: usize = 16_384;
    let mut renderable_rows = renderable.iter().peekable();
    let mut start = 0;
    while start < blocks.len() {
        let end = start.saturating_add(ROW_CHUNK).min(blocks.len());
        let columns: Vec<Vec<f64>> = variables
            .iter()
            .map(|variable| {
                model
                    .color_values_range(&variable.name, start, end)
                    .map_err(|error| CsvBlockModelError::Invalid(error.to_string()))
            })
            .collect::<Result<_, _>>()?;
        while renderable_rows.peek().is_some_and(|&row| row < end) {
            let row = renderable_rows.next().expect("peeked renderable row exists");
            if row < start {
                return Err(CsvBlockModelError::Invalid("renderable block indices must be ordered and contain no duplicates".to_owned()));
            }
            let block = blocks.get(row).ok_or_else(|| CsvBlockModelError::Invalid(format!("missing block geometry row {row}")))?;
            let center = model.local_to_world((block.lower + block.upper) * 0.5);
            let size = block.upper - block.lower;
            let geometry = [center.x, center.y, center.z, size.x, size.y, size.z];
            for value in geometry {
                output.write_field(value.to_string())?;
            }
            for ((column, variable), empty_value) in columns.iter().zip(&variables).zip(&empty_values) {
                let value = column[row - start];
                let categorical = matches!(variable.physical_type.as_str(), "namedbyte" | "namedshort");
                let is_empty = !value.is_finite() || empty_value.is_some_and(|empty| (value - empty).abs() < 1e-8) || (!categorical && is_no_data_sentinel(value));
                if is_empty {
                    output.write_field("")?;
                } else if categorical
                    && value.fract() == 0.0
                    && (0.0..=u32::MAX as f64).contains(&value)
                    && let Some(label) = variable.strings.get(&(value as u32))
                {
                    output.write_field(label)?;
                } else {
                    output.write_field(value.to_string())?;
                }
            }
            output.write_record(None::<&[u8]>)?;
        }
        start = end;
    }
    if let Some(row) = renderable_rows.next() {
        return Err(CsvBlockModelError::Invalid(format!("renderable block index {row} is outside the model")));
    }
    output.flush()?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn to_bytes(model: &BlockModelData, blocks: &BlockBoundsSource, renderable: &RenderableBlockIndices) -> Result<Vec<u8>, CsvBlockModelError> {
    let mut bytes = Vec::new();
    write(model, blocks, renderable, &mut bytes)?;
    Ok(bytes)
}

fn unique_resource_names(headers: &[String], mapping: &CsvColumnMapping) -> Vec<String> {
    let mut used = HashSet::new();
    headers
        .iter()
        .enumerate()
        .filter(|(index, _)| matches!(mapping.roles[*index], CsvColumnRole::Value | CsvColumnRole::Category | CsvColumnRole::Id))
        .map(|(index, header)| {
            let base = if header.trim().is_empty() {
                format!("resource_{}", index + 1)
            } else {
                header.trim().to_owned()
            };
            let mut name = base.clone();
            let mut suffix = 2;
            while !used.insert(name.clone()) {
                name = format!("{base}_{suffix}");
                suffix += 1;
            }
            name
        })
        .collect()
}

fn parse_required_number(field: &str, row: usize, header: &str) -> Result<f64, CsvBlockModelError> {
    field
        .trim()
        .parse::<f64>()
        .map_err(|_| CsvBlockModelError::Invalid(format!("CSV row {row}, column '{header}': expected a number, found '{}'", field.trim())))
}
