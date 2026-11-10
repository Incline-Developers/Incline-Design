//! Point cloud readers: LAS/LAZ (ASPRS LiDAR), ASCII XYZ/PTS, and PCD.
//!
//! Every reader produces world-space `f64` positions plus optional packed
//! RGBA8 colours (r in the low byte) and, for LAS/LAZ, the ASPRS
//! classification byte. Attributes other than those (intensity, GPS time, …)
//! are intentionally ignored.

#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::{io::Cursor, mem::size_of, path::Path};

use anyhow::{Context, Result, bail};
use glam::DVec3;
use rayon::prelude::*;

use crate::i18n::tr;

/// Points decoded between progress reports in the row-at-a-time readers.
const DECODE_PROGRESS_STRIDE: usize = 4096;
/// LAZ is decoded in bounded batches rather than materializing every expanded
/// record at once. A 512 MiB ceiling gives the parallel decoder enough chunks
/// per call to avoid frequent Rayon synchronization on very large clouds.
const MAX_LAZ_BATCH_BYTES: usize = 512 << 20;
const LAS_MIN_HEADER_LEN: usize = 227;
const LAS_VLR_HEADER_LEN: usize = 54;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PointCloudFormat {
    Las,
    Laz,
    Xyz,
    Pcd,
}

impl PointCloudFormat {
    pub(crate) fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        let extension = path.as_ref().extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "las" => Some(Self::Las),
            "laz" => Some(Self::Laz),
            "xyz" | "pts" => Some(Self::Xyz),
            "pcd" => Some(Self::Pcd),
            _ => None,
        }
    }
}

pub(crate) struct PointCloudData {
    pub(crate) points: Vec<DVec3>,
    /// Packed RGBA8 per point, parallel to `points`; `None` when the file
    /// has no colour data.
    pub(crate) colors: Option<Vec<u32>>,
    /// ASPRS classification code per point, parallel to `points`. Only LAS and
    /// LAZ carry one, and only a file that classifies something beyond
    /// "created" and "unclassified" keeps it - see [`meaningful_classifications`].
    pub(crate) classifications: Option<Vec<u8>>,
    /// Validated file metadata bounds when the format supplies them. Callers
    /// retain a decoded-point scan fallback for formats without bounds and
    /// malformed metadata.
    pub(crate) bounds: Option<(DVec3, DVec3)>,
}

/// The browser has no filesystem to read a cloud from; it decodes the bytes
/// it was handed through [`read_point_cloud_bytes`] instead.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read_point_cloud(path: &Path, progress: &crate::model::progress::Phase) -> Result<PointCloudData> {
    let format = PointCloudFormat::from_path(path).with_context(|| tr!("err-cloud-unsupported-extension", name = path.display().to_string()))?;
    let storage = {
        let file = fs::File::open(path).with_context(|| tr!("err-cloud-open-failed", name = path.display().to_string()))?;
        // The map is read-only and lives until decoding finishes. This avoids
        // allocating and copying the entire (often multi-GB) source file before
        // LAZ decoding can begin; the OS faults source pages in as they are read.
        unsafe { memmap2::Mmap::map(&file) }.with_context(|| tr!("err-cloud-map-failed", name = path.display().to_string()))?
    };
    decode_point_cloud_bytes(format, storage.as_ref(), progress)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn read_point_cloud_bytes(source_name: &str, bytes: &[u8], progress: &crate::model::progress::Phase) -> Result<PointCloudData> {
    let format = PointCloudFormat::from_path(source_name).with_context(|| tr!("err-cloud-unsupported-extension", name = source_name.to_string()))?;
    decode_point_cloud_bytes(format, bytes, progress)
}

fn decode_point_cloud_bytes(format: PointCloudFormat, bytes: &[u8], progress: &crate::model::progress::Phase) -> Result<PointCloudData> {
    match format {
        PointCloudFormat::Las | PointCloudFormat::Laz => read_las_bytes(bytes, progress),
        PointCloudFormat::Xyz => read_xyz_bytes(bytes, progress),
        PointCloudFormat::Pcd => read_pcd_bytes(bytes, progress),
    }
}

// --- LAS / LAZ ---------------------------------------------------------

struct LasHeader {
    point_format: u8,
    record_length: usize,
    point_count: u64,
    offset_to_points: usize,
    header_size: usize,
    vlr_count: u32,
    scale: DVec3,
    offset: DVec3,
    bounds: Option<(DVec3, DVec3)>,
    compressed: bool,
}

fn le_u16(bytes: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(checked_las_slice(bytes, at, 2)?.try_into().unwrap()))
}

fn le_u32(bytes: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(checked_las_slice(bytes, at, 4)?.try_into().unwrap()))
}

fn le_u64(bytes: &[u8], at: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(checked_las_slice(bytes, at, 8)?.try_into().unwrap()))
}

fn le_f64(bytes: &[u8], at: usize) -> Result<f64> {
    Ok(f64::from_le_bytes(checked_las_slice(bytes, at, 8)?.try_into().unwrap()))
}

fn checked_las_slice(bytes: &[u8], at: usize, len: usize) -> Result<&[u8]> {
    let end = at.checked_add(len).with_context(|| tr!("err-cloud-las-range-overflow"))?;
    bytes.get(at..end).with_context(|| tr!("err-cloud-las-header-truncated"))
}

fn parse_las_header(bytes: &[u8]) -> Result<LasHeader> {
    if bytes.get(..4) != Some(b"LASF") {
        bail!(tr!("err-cloud-las-no-signature"));
    }
    let version_major = *bytes.get(24).with_context(|| tr!("err-cloud-las-header-truncated"))?;
    let version_minor = *bytes.get(25).with_context(|| tr!("err-cloud-las-header-truncated"))?;
    let header_size = le_u16(bytes, 94)? as usize;
    if header_size < LAS_MIN_HEADER_LEN {
        bail!(tr!(
            "err-cloud-las-header-small",
            header_size = header_size.to_string(),
            minimum = LAS_MIN_HEADER_LEN.to_string()
        ));
    }
    let offset_to_points = le_u32(bytes, 96)? as usize;
    let vlr_count = le_u32(bytes, 100)?;
    let raw_format = *bytes.get(104).with_context(|| tr!("err-cloud-las-header-truncated"))?;
    // LAZ files set the two high bits of the point format id to flag
    // compression; the low bits keep the real record layout.
    let compressed = raw_format & 0x80 != 0;
    let point_format = raw_format & 0x3f;
    let record_length = le_u16(bytes, 105)? as usize;
    let legacy_count = le_u32(bytes, 107)? as u64;
    let point_count = if legacy_count == 0 && (version_major, version_minor) >= (1, 4) {
        if header_size < 255 {
            bail!(tr!("err-cloud-las14-header-small"));
        }
        le_u64(bytes, 247)?
    } else {
        legacy_count
    };
    let scale = DVec3::new(le_f64(bytes, 131)?, le_f64(bytes, 139)?, le_f64(bytes, 147)?);
    let offset = DVec3::new(le_f64(bytes, 155)?, le_f64(bytes, 163)?, le_f64(bytes, 171)?);
    let bounds_max = DVec3::new(le_f64(bytes, 179)?, le_f64(bytes, 195)?, le_f64(bytes, 211)?);
    let bounds_min = DVec3::new(le_f64(bytes, 187)?, le_f64(bytes, 203)?, le_f64(bytes, 219)?);
    let bounds = (bounds_min.is_finite() && bounds_max.is_finite() && bounds_min.cmple(bounds_max).all()).then_some((bounds_min, bounds_max));
    if record_length < 12 {
        bail!(tr!("err-cloud-las-record-small", length = record_length.to_string()));
    }
    Ok(LasHeader {
        point_format,
        record_length,
        point_count,
        offset_to_points,
        header_size,
        vlr_count,
        scale,
        offset,
        bounds,
        compressed,
    })
}

/// Byte offset of the classification byte within a point record, with the mask
/// that isolates the code, per format.
///
/// Legacy formats 0-5 pack the code into the low five bits of byte 15 and
/// spend the top three on the synthetic/key-point/withheld flags. Formats 6-10
/// moved those flags into byte 15 and gave classification a whole byte of its
/// own at 16, which is what lets them carry the codes above 31.
fn las_classification_offset(point_format: u8) -> Option<(usize, u8)> {
    match point_format {
        0..=5 => Some((15, 0x1f)),
        6..=10 => Some((16, 0xff)),
        _ => None,
    }
}

/// Byte offset of the 16-bit RGB triplet within a point record, per format.
fn las_rgb_offset(point_format: u8) -> Option<usize> {
    match point_format {
        2 => Some(20),
        3 | 5 => Some(28),
        7 | 8 | 10 => Some(30),
        _ => None,
    }
}

/// The laszip VLR's record data, needed to decompress LAZ point records.
fn find_laszip_vlr<'a>(bytes: &'a [u8], header: &LasHeader) -> Result<&'a [u8]> {
    let mut at = header.header_size;
    for _ in 0..header.vlr_count {
        let header_end = at.checked_add(LAS_VLR_HEADER_LEN).with_context(|| tr!("err-cloud-laz-vlr-range"))?;
        if header_end > header.offset_to_points {
            bail!(tr!("err-cloud-laz-vlr-overlap"));
        }
        let vlr_header = bytes.get(at..header_end).with_context(|| tr!("err-cloud-laz-vlr-truncated"))?;
        let user_id = &vlr_header[2..18];
        let record_id = u16::from_le_bytes(vlr_header[18..20].try_into().unwrap());
        let record_length = u16::from_le_bytes(vlr_header[20..22].try_into().unwrap()) as usize;
        let data_start = header_end;
        let data_end = data_start.checked_add(record_length).with_context(|| tr!("err-cloud-laz-vlr-data-range"))?;
        if data_end > header.offset_to_points {
            bail!(tr!("err-cloud-laz-vlr-data-overlap"));
        }
        let record_data = bytes.get(data_start..data_end).with_context(|| tr!("err-cloud-laz-vlr-truncated"))?;
        if user_id.starts_with(b"laszip encoded") && record_id == 22204 {
            return Ok(record_data);
        }
        at = data_end;
    }
    bail!(tr!("err-cloud-laz-no-laszip"))
}

fn read_las_bytes(bytes: &[u8], progress: &crate::model::progress::Phase) -> Result<PointCloudData> {
    let header = parse_las_header(bytes)?;
    if header.header_size > header.offset_to_points {
        bail!(tr!("err-cloud-las-offset-before-header"));
    }
    if header.offset_to_points > bytes.len() {
        bail!(tr!("err-cloud-las-offset-past-end"));
    }
    let count = usize::try_from(header.point_count).with_context(|| tr!("err-cloud-las-count-overflow"))?;
    let expanded_bytes = count.checked_mul(header.record_length).with_context(|| tr!("err-cloud-las-size-overflow"))?;
    let rgb_offset = las_rgb_offset(header.point_format).filter(|rgb_at| rgb_at.checked_add(6).is_some_and(|end| end <= header.record_length));
    let class_offset = las_classification_offset(header.point_format).filter(|(class_at, _)| *class_at < header.record_length);

    let laz_vlr = if header.compressed {
        let record_data = find_laszip_vlr(bytes, &header)?;
        let vlr = laz::LazVlr::from_buffer(record_data).with_context(|| tr!("err-cloud-laszip-invalid"))?;
        // `laz` chunks output by this width. Validate it without calling the
        // crate's `items_size`, whose u16 summation can itself overflow on a
        // malicious item list.
        let item_size = vlr.items().iter().try_fold(0usize, |total, item| total.checked_add(usize::from(item.size())));
        let item_size = item_size.with_context(|| tr!("err-cloud-laszip-item-overflow"))?;
        if item_size != header.record_length {
            bail!(tr!(
                "err-cloud-laszip-size-mismatch",
                item_size = item_size.to_string(),
                record_length = header.record_length.to_string()
            ));
        }
        Some(vlr)
    } else {
        let end = header.offset_to_points.checked_add(expanded_bytes).with_context(|| tr!("err-cloud-las-size-overflow"))?;
        if end > bytes.len() {
            bail!(tr!("err-cloud-las-data-truncated"));
        }
        None
    };

    let mut points = try_vec_with_capacity(count, &tr!("err-cloud-label-las-points"))?;
    let mut attributes = LasAttributes {
        rgb_offset,
        class_offset,
        // 16-bit source triplets; scaled to 8-bit once the file-wide maximum is
        // known (files written with 8-bit colours in the u16 fields are common).
        raw_colors: if rgb_offset.is_some() {
            try_vec_with_capacity(count, &tr!("err-cloud-label-las-colours"))?
        } else {
            Vec::new()
        },
        classifications: if class_offset.is_some() {
            try_vec_with_capacity(count, &tr!("err-cloud-label-las-classes"))?
        } else {
            Vec::new()
        },
    };

    if header.compressed {
        if count > 0 {
            let mut source = Cursor::new(bytes);
            source.set_position(u64::try_from(header.offset_to_points).with_context(|| tr!("err-cloud-laz-offset-too-large"))?);
            let vlr = laz_vlr.expect("compressed LAS validated a laszip VLR");
            match laz::ParLasZipDecompressor::new(source.clone(), vlr.clone()) {
                Ok(mut decompressor) => decode_laz_batches(&mut decompressor, &header, count, &mut points, &mut attributes, progress)?,
                Err(_) => {
                    // Old point-wise LAZ files and files without a usable
                    // chunk table cannot be split safely; retain compatibility
                    // through the sequential decoder.
                    let mut decompressor = laz::LasZipDecompressor::new(source, vlr).with_context(|| tr!("err-cloud-laz-init-failed"))?;
                    decode_laz_batches(&mut decompressor, &header, count, &mut points, &mut attributes, progress)?;
                }
            }
        }
    } else {
        let end = header
            .offset_to_points
            .checked_add(expanded_bytes)
            .filter(|end| *end <= bytes.len())
            .with_context(|| tr!("err-cloud-las-data-truncated"))?;
        // One parallel pass over every record: nothing to report part-way.
        append_las_records_parallel(&bytes[header.offset_to_points..end], &header, &mut points, &mut attributes);
        progress.set_items(count as u64, count as u64);
    }

    let colors = rgb_offset.map(|_| pack_las_colors(&attributes.raw_colors)).transpose()?;
    let classifications = meaningful_classifications(attributes.classifications);
    Ok(PointCloudData {
        points,
        colors,
        classifications,
        bounds: header.bounds,
    })
}

/// Drop a classification column that says nothing.
///
/// A file whose every point is "created, never classified" (0) or
/// "unclassified" (1) has been through no ground filter, so keeping the column
/// would offer a bare-earth filter that discards the whole cloud and a
/// classification view that paints it one flat colour. Treating that as absent
/// is what makes "ground only" safe to default on.
fn meaningful_classifications(classifications: Vec<u8>) -> Option<Vec<u8>> {
    classifications.par_iter().any(|code| *code > 1).then_some(classifications)
}

/// Where each optional per-point attribute lives in a point record, and what
/// has been decoded of it so far. One value rather than loose parameters
/// because the compressed and uncompressed paths both fill it record by record.
struct LasAttributes {
    rgb_offset: Option<usize>,
    class_offset: Option<(usize, u8)>,
    raw_colors: Vec<[u16; 3]>,
    classifications: Vec<u8>,
}

fn decode_laz_batches(
    decompressor: &mut impl laz::LazDecompressor,
    header: &LasHeader,
    count: usize,
    points: &mut Vec<DVec3>,
    attributes: &mut LasAttributes,
    progress: &crate::model::progress::Phase,
) -> Result<()> {
    let batch_capacity = (MAX_LAZ_BATCH_BYTES / header.record_length).max(1).min(count);
    let buffer_len = header.record_length.checked_mul(batch_capacity).with_context(|| tr!("err-cloud-laz-batch-size"))?;
    let mut buffer = try_zeroed_bytes(buffer_len, &tr!("err-cloud-label-laz-batch"))?;
    let mut remaining = count;
    while remaining > 0 {
        let batch = remaining.min(batch_capacity);
        let out_len = header.record_length.checked_mul(batch).with_context(|| tr!("err-cloud-laz-batch-size"))?;
        let out = &mut buffer[..out_len];
        decompressor.decompress_many(out).with_context(|| tr!("err-cloud-laz-decompress-failed"))?;
        append_las_records_parallel(out, header, points, attributes);
        remaining -= batch;
        // The header's point count makes this exact.
        progress.set_items((count - remaining) as u64, count as u64);
    }
    Ok(())
}

fn append_las_records_parallel(records: &[u8], header: &LasHeader, points: &mut Vec<DVec3>, attributes: &mut LasAttributes) {
    points.par_extend(
        records
            .par_chunks_exact(header.record_length)
            .map(|record| las_record_position(record, header.scale, header.offset)),
    );
    if let Some(rgb_at) = attributes.rgb_offset {
        attributes
            .raw_colors
            .par_extend(records.par_chunks_exact(header.record_length).map(|record| las_record_color(record, rgb_at)));
    }
    if let Some((class_at, mask)) = attributes.class_offset {
        attributes
            .classifications
            .par_extend(records.par_chunks_exact(header.record_length).map(|record| record[class_at] & mask));
    }
}

fn las_record_position(record: &[u8], scale: DVec3, offset: DVec3) -> DVec3 {
    let x = i32::from_le_bytes(record[0..4].try_into().unwrap()) as f64;
    let y = i32::from_le_bytes(record[4..8].try_into().unwrap()) as f64;
    let z = i32::from_le_bytes(record[8..12].try_into().unwrap()) as f64;
    DVec3::new(x, y, z) * scale + offset
}

fn las_record_color(record: &[u8], rgb_at: usize) -> [u16; 3] {
    [
        u16::from_le_bytes(record[rgb_at..rgb_at + 2].try_into().unwrap()),
        u16::from_le_bytes(record[rgb_at + 2..rgb_at + 4].try_into().unwrap()),
        u16::from_le_bytes(record[rgb_at + 4..rgb_at + 6].try_into().unwrap()),
    ]
}

pub(crate) fn try_vec_with_capacity<T>(count: usize, label: &str) -> Result<Vec<T>> {
    let allocation_bytes = count
        .checked_mul(size_of::<T>())
        .with_context(|| tr!("err-cloud-alloc-overflow", label = label.to_string()))?;
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .with_context(|| tr!("err-cloud-alloc-failed", bytes = allocation_bytes.to_string(), label = label.to_string()))?;
    Ok(out)
}

fn try_zeroed_bytes(len: usize, label: &str) -> Result<Vec<u8>> {
    let mut out = try_vec_with_capacity(len, label)?;
    out.resize(len, 0);
    Ok(out)
}

/// LAS colours are nominally 16-bit, but files storing 8-bit values are
/// widespread; scale by the file-wide maximum so both render correctly.
fn pack_las_colors(raw: &[[u16; 3]]) -> Result<Vec<u32>> {
    let sixteen_bit = raw.par_iter().any(|rgb| rgb.iter().any(|component| *component > 255));
    let mut packed = try_vec_with_capacity(raw.len(), &tr!("err-cloud-label-las-packed"))?;
    packed.par_extend(raw.par_iter().map(|[r, g, b]| {
        let (r, g, b) = if sixteen_bit {
            ((r >> 8) as u32, (g >> 8) as u32, (b >> 8) as u32)
        } else {
            (*r as u32, *g as u32, *b as u32)
        };
        r | (g << 8) | (b << 16) | 0xff00_0000
    }));
    Ok(packed)
}

// --- ASCII XYZ / PTS ---------------------------------------------------

type ParsedXyzRow = Result<Option<(DVec3, Option<u32>)>>;

/// Whitespace- or comma-separated `x y z [... r g b]` lines. Leica PTS
/// (`x y z intensity r g b`) parses through the same rule: when a row ends
/// in three integral values within 0..=255, they are taken as its colour.
fn read_xyz_bytes(bytes: &[u8], progress: &crate::model::progress::Phase) -> Result<PointCloudData> {
    let text = std::str::from_utf8(bytes).with_context(|| tr!("err-cloud-ascii-utf8"))?;
    // Row count is unknown until the file has been walked, so report by text
    // consumed instead.
    let mut scanned_bytes = 0usize;
    let parsed: Vec<ParsedXyzRow> = text
        .lines()
        .enumerate()
        .map(|(index, line)| {
            scanned_bytes += line.len() + 1;
            if index.is_multiple_of(DECODE_PROGRESS_STRIDE) {
                progress.set_items(scanned_bytes as u64, text.len() as u64);
            }
            parse_xyz_line(line, index + 1)
        })
        .collect();
    let rows: Vec<(DVec3, Option<u32>)> = parsed.into_iter().collect::<Result<Vec<_>>>()?.into_iter().flatten().collect();
    if rows.is_empty() {
        bail!(tr!("err-cloud-ascii-no-points"));
    }
    let points: Vec<DVec3> = rows.iter().map(|(point, _)| *point).collect();
    let colors = if rows.iter().any(|(_, color)| color.is_some()) {
        Some(rows.iter().map(|(_, color)| color.unwrap_or(0xffff_ffff)).collect())
    } else {
        None
    };
    Ok(PointCloudData {
        points,
        colors,
        classifications: None,
        bounds: None,
    })
}

fn parse_xyz_line(line: &str, line_number: usize) -> Result<Option<(DVec3, Option<u32>)>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
        return Ok(None);
    }
    let tokens: Vec<&str> = line.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|field| !field.is_empty()).collect();
    // A lone non-negative integer is a point-count preamble (PTS), not a
    // coordinate row. Every other non-comment row must be diagnosed.
    if tokens.len() == 1 && tokens[0].parse::<usize>().is_ok() {
        return Ok(None);
    }
    if tokens.len() < 3 {
        bail!(tr!("err-cloud-ascii-bad-row", line = line_number.to_string()));
    }
    let fields: Vec<f64> = tokens
        .iter()
        .map(|token| {
            token
                .parse::<f64>()
                .with_context(|| tr!("err-cloud-ascii-bad-number", token = token.to_string(), line = line_number.to_string()))
        })
        .collect::<Result<_>>()?;
    let point = DVec3::new(fields[0], fields[1], fields[2]);
    if !point.is_finite() {
        bail!(tr!("err-cloud-ascii-non-finite", line = line_number.to_string()));
    }
    let color = (fields.len() >= 6)
        .then(|| &fields[fields.len() - 3..])
        .filter(|rgb| rgb.iter().all(|c| c.fract() == 0.0 && (0.0..=255.0).contains(c)))
        .map(|rgb| (rgb[0] as u32) | ((rgb[1] as u32) << 8) | ((rgb[2] as u32) << 16) | 0xff00_0000);
    Ok(Some((point, color)))
}

// --- PCD ---------------------------------------------------------------

struct PcdField {
    name: String,
    size: usize,
    kind: char,
    count: usize,
}

fn read_pcd_bytes(bytes: &[u8], progress: &crate::model::progress::Phase) -> Result<PointCloudData> {
    // Header is ASCII lines up to and including the DATA line.
    let mut fields: Vec<PcdField> = Vec::new();
    let mut point_count: Option<usize> = None;
    let mut data_kind: Option<String> = None;
    let mut at = 0usize;
    while at < bytes.len() {
        let line_end = bytes[at..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|relative| at + relative)
            .with_context(|| tr!("err-cloud-pcd-header-truncated"))?;
        let line = std::str::from_utf8(&bytes[at..line_end]).with_context(|| tr!("err-cloud-pcd-header-utf8"))?.trim();
        at = line_end + 1;
        let mut tokens = line.split_ascii_whitespace();
        match tokens.next() {
            Some("FIELDS") => {
                fields = tokens
                    .map(|name| PcdField {
                        name: name.to_owned(),
                        size: 4,
                        kind: 'F',
                        count: 1,
                    })
                    .collect();
            }
            Some("SIZE") => {
                let sizes = tokens
                    .map(|token| token.parse::<usize>().with_context(|| tr!("err-cloud-pcd-bad-size")))
                    .collect::<Result<Vec<_>>>()?;
                if sizes.len() != fields.len() {
                    bail!(tr!("err-cloud-pcd-size-count"));
                }
                for (field, size) in fields.iter_mut().zip(sizes) {
                    if size == 0 {
                        bail!(tr!("err-cloud-pcd-size-zero"));
                    }
                    field.size = size;
                }
            }
            Some("TYPE") => {
                let kinds: Vec<&str> = tokens.collect();
                if kinds.len() != fields.len() {
                    bail!(tr!("err-cloud-pcd-type-count"));
                }
                for (field, token) in fields.iter_mut().zip(kinds) {
                    let mut chars = token.chars();
                    let kind = chars.next().with_context(|| tr!("err-cloud-pcd-bad-type-line"))?;
                    if chars.next().is_some() || !matches!(kind, 'F' | 'I' | 'U') {
                        bail!(tr!("err-cloud-pcd-bad-type", token = token.to_string()));
                    }
                    field.kind = kind;
                }
            }
            Some("COUNT") => {
                let counts = tokens
                    .map(|token| token.parse::<usize>().with_context(|| tr!("err-cloud-pcd-bad-count")))
                    .collect::<Result<Vec<_>>>()?;
                if counts.len() != fields.len() {
                    bail!(tr!("err-cloud-pcd-count-count"));
                }
                for (field, count) in fields.iter_mut().zip(counts) {
                    if count == 0 {
                        bail!(tr!("err-cloud-pcd-count-zero"));
                    }
                    field.count = count;
                }
            }
            Some("POINTS") => {
                point_count = Some(tokens.next().with_context(|| tr!("err-cloud-pcd-bad-points"))?.parse()?);
            }
            Some("DATA") => {
                data_kind = Some(tokens.next().with_context(|| tr!("err-cloud-pcd-bad-data"))?.to_owned());
                break;
            }
            _ => {}
        }
    }
    let data_kind = data_kind.with_context(|| tr!("err-cloud-pcd-no-data"))?;
    let point_count = point_count.with_context(|| tr!("err-cloud-pcd-no-points"))?;
    if fields.is_empty() {
        bail!(tr!("err-cloud-pcd-no-fields"));
    }
    if fields.iter().any(|field| field.size == 0 || field.count == 0) {
        bail!(tr!("err-cloud-pcd-size-count-zero"));
    }
    let x = pcd_field_index(&fields, "x")?;
    let y = pcd_field_index(&fields, "y")?;
    let z = pcd_field_index(&fields, "z")?;
    let rgb = fields.iter().position(|field| field.name == "rgb" || field.name == "rgba");
    if let Some(rgb) = rgb {
        let field = &fields[rgb];
        if field.size != 4 || field.count != 1 || !matches!(field.kind, 'F' | 'I' | 'U') {
            bail!(tr!(
                "err-cloud-pcd-colour-field",
                name = field.name.to_string(),
                kind = field.kind.to_string(),
                size = field.size.to_string(),
                count = field.count.to_string()
            ));
        }
    }

    match data_kind.as_str() {
        "ascii" => read_pcd_ascii(&bytes[at..], &fields, point_count, [x, y, z], rgb, progress),
        "binary" => read_pcd_binary(&bytes[at..], &fields, point_count, [x, y, z], rgb, progress),
        other => bail!(tr!("err-cloud-pcd-encoding", encoding = other.to_string())),
    }
}

fn pcd_field_index(fields: &[PcdField], name: &str) -> Result<usize> {
    fields
        .iter()
        .position(|field| field.name == name)
        .with_context(|| tr!("err-cloud-pcd-no-field", name = name.to_string()))
}

/// PCD packs colour as one scalar whose low three bytes are b, g, r.
fn pcd_color_from_packed(packed: u32) -> u32 {
    let r = (packed >> 16) & 0xff;
    let g = (packed >> 8) & 0xff;
    let b = packed & 0xff;
    r | (g << 8) | (b << 16) | 0xff00_0000
}

fn read_pcd_ascii(bytes: &[u8], fields: &[PcdField], point_count: usize, xyz: [usize; 3], rgb: Option<usize>, progress: &crate::model::progress::Phase) -> Result<PointCloudData> {
    let text = std::str::from_utf8(bytes).with_context(|| tr!("err-cloud-pcd-ascii-utf8"))?;
    // Column index of each field's first token within a row.
    let mut column = 0usize;
    let columns: Vec<usize> = fields
        .iter()
        .map(|field| {
            let start = column;
            column = column.saturating_add(field.count);
            start
        })
        .collect();
    if column == usize::MAX {
        bail!(tr!("err-cloud-pcd-row-width"));
    }
    let mut points = Vec::with_capacity(point_count);
    let mut colors = rgb.map(|_| Vec::with_capacity(point_count));
    for line in text.lines().take(point_count) {
        if points.len().is_multiple_of(DECODE_PROGRESS_STRIDE) {
            progress.set_items(points.len() as u64, point_count as u64);
        }
        let tokens: Vec<&str> = line.split_ascii_whitespace().collect();
        let coordinate = |field: usize| -> Result<f64> {
            tokens
                .get(columns[field])
                .and_then(|token| token.parse::<f64>().ok())
                .with_context(|| tr!("err-cloud-pcd-bad-coordinate"))
        };
        points.push(DVec3::new(coordinate(xyz[0])?, coordinate(xyz[1])?, coordinate(xyz[2])?));
        if let (Some(colors), Some(rgb)) = (colors.as_mut(), rgb) {
            // Stored either as a float whose bits carry the packed colour, or
            // as a plain unsigned integer, depending on the writer.
            let token = tokens.get(columns[rgb]).with_context(|| tr!("err-cloud-pcd-bad-rgb"))?;
            let packed = if fields[rgb].kind == 'F' {
                token.parse::<f32>().map(f32::to_bits).ok()
            } else {
                token.parse::<u32>().ok()
            }
            .with_context(|| tr!("err-cloud-pcd-bad-rgb"))?;
            colors.push(pcd_color_from_packed(packed));
        }
    }
    if points.len() < point_count {
        bail!(tr!("err-cloud-pcd-data-truncated"));
    }
    Ok(PointCloudData {
        points,
        colors,
        classifications: None,
        bounds: None,
    })
}

fn read_pcd_binary(bytes: &[u8], fields: &[PcdField], point_count: usize, xyz: [usize; 3], rgb: Option<usize>, progress: &crate::model::progress::Phase) -> Result<PointCloudData> {
    let mut offset = 0usize;
    let offsets: Vec<usize> = fields
        .iter()
        .map(|field| {
            let start = offset;
            offset = field.size.checked_mul(field.count).and_then(|width| offset.checked_add(width)).unwrap_or(usize::MAX);
            start
        })
        .collect();
    let stride = offset;
    if stride == 0 || stride == usize::MAX {
        bail!(tr!("err-cloud-pcd-record-size"));
    }
    let required = stride.checked_mul(point_count).with_context(|| tr!("err-cloud-pcd-binary-size"))?;
    if bytes.len() < required {
        bail!(tr!("err-cloud-pcd-binary-truncated"));
    }
    let mut points = Vec::with_capacity(point_count);
    let mut colors = rgb.map(|_| Vec::with_capacity(point_count));
    for record in bytes.chunks_exact(stride).take(point_count) {
        if points.len().is_multiple_of(DECODE_PROGRESS_STRIDE) {
            progress.set_items(points.len() as u64, point_count as u64);
        }
        let coordinate = |field: usize| -> Result<f64> {
            let at = offsets[field];
            match (fields[field].kind, fields[field].size) {
                ('F', 4) => Ok(f32::from_le_bytes(record[at..at + 4].try_into().unwrap()) as f64),
                ('F', 8) => Ok(f64::from_le_bytes(record[at..at + 8].try_into().unwrap())),
                (kind, size) => bail!(tr!("err-cloud-pcd-coordinate-type", kind = kind.to_string(), size = size.to_string())),
            }
        };
        points.push(DVec3::new(coordinate(xyz[0])?, coordinate(xyz[1])?, coordinate(xyz[2])?));
        if let (Some(colors), Some(rgb)) = (colors.as_mut(), rgb) {
            let at = offsets[rgb];
            let packed = u32::from_le_bytes(record.get(at..at + 4).with_context(|| tr!("err-cloud-pcd-binary-truncated"))?.try_into().unwrap());
            colors.push(pcd_color_from_packed(packed));
        }
    }
    Ok(PointCloudData {
        points,
        colors,
        classifications: None,
        bounds: None,
    })
}
