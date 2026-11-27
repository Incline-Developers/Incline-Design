#![allow(dead_code)] // Many attributes in here exist for the JSON loading, but aren't used.
use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::model::{ColorArrays, Data, Elements, LegendArrays, SurfaceGeometries};

/// Stores a string, while `T` can control what types of model as accepted.
#[derive(Debug)]
pub struct Key<T> {
    pub value: String,
    _phantom: PhantomData<T>,
}

impl Key<Project> {
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        let mut value = format!("{:032x}", u128::from_be_bytes(bytes));
        for i in [20, 16, 12, 8] {
            value.insert(i, '-');
        }
        Self {
            value,
            _phantom: Default::default(),
        }
    }
}

struct KeyVisitor<T> {
    _phantom: PhantomData<T>,
}

impl<T> serde::de::Visitor<'_> for KeyVisitor<T> {
    type Value = Key<T>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an id string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Key {
            value: v.to_owned(),
            _phantom: Default::default(),
        })
    }
}

impl<'de, T> Deserialize<'de> for Key<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(KeyVisitor::<T> { _phantom: Default::default() })
    }
}

#[derive(Debug, Deserialize)]
pub struct UidModel {
    #[serde(default, deserialize_with = "text")]
    pub date_created: String,
    #[serde(default, deserialize_with = "text")]
    pub date_modified: String,
}

#[derive(Debug, Deserialize)]
pub struct ContentModel {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(default, deserialize_with = "text")]
    pub name: String,
    #[serde(default, deserialize_with = "text")]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    #[serde(flatten)]
    pub content: ContentModel,
    #[serde(default, deserialize_with = "text")]
    pub author: String,
    #[serde(default, deserialize_with = "text")]
    pub revision: String,
    #[serde(default, deserialize_with = "date_time")]
    pub date: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "text")]
    pub units: String,
    #[serde(default)]
    pub origin: [f64; 3],
    #[serde(default)]
    pub elements: Vec<Key<Elements>>,
}

// Elements

#[derive(Debug, Deserialize)]
pub struct PointSetElement {
    #[serde(flatten)]
    pub content: ContentModel,
    #[serde(default, deserialize_with = "color")]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub data: Vec<Key<Data>>,
    #[serde(default)]
    pub textures: Vec<Key<ImageTexture>>,
    #[serde(default)]
    pub subtype: String,
    pub geometry: Key<PointSetGeometry>,
}

#[derive(Debug, Deserialize)]
pub struct PointSetGeometry {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(default)]
    pub origin: [f64; 3],
    pub vertices: Key<Vector3Array>,
}

#[derive(Debug, Deserialize)]
pub struct LineSetElement {
    #[serde(flatten)]
    pub content: ContentModel,
    #[serde(default, deserialize_with = "color")]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub data: Vec<Key<Data>>,
    #[serde(default)]
    pub subtype: String,
    pub geometry: Key<LineSetGeometry>,
}

#[derive(Debug, Deserialize)]
pub struct LineSetGeometry {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(default)]
    pub origin: [f64; 3],
    pub vertices: Key<Vector3Array>,
    pub segments: Key<Int2Array>,
}

#[derive(Debug, Deserialize)]
pub struct SurfaceElement {
    #[serde(flatten)]
    pub content: ContentModel,
    #[serde(default, deserialize_with = "color")]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub data: Vec<Key<Data>>,
    #[serde(default)]
    pub textures: Vec<Key<ImageTexture>>,
    #[serde(default)]
    pub subtype: String,
    pub geometry: Key<SurfaceGeometries>,
}

#[derive(Debug, Deserialize)]
pub struct SurfaceGeometry {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(default)]
    pub origin: [f64; 3],
    pub vertices: Key<Vector3Array>,
    pub triangles: Key<Int3Array>,
}

#[derive(Debug, Deserialize)]
pub struct SurfaceGridGeometry {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(default)]
    pub origin: [f64; 3],
    pub tensor_u: Vec<f64>,
    pub tensor_v: Vec<f64>,
    #[serde(default = "i")]
    pub axis_u: [f64; 3],
    #[serde(default = "j")]
    pub axis_v: [f64; 3],
    #[serde(default)]
    pub offset_w: Option<Key<ScalarArray>>,
}

#[derive(Debug, Deserialize)]
pub struct VolumeElement {
    #[serde(flatten)]
    pub content: ContentModel,
    #[serde(default, deserialize_with = "color")]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub data: Vec<Key<Data>>,
    #[serde(default)]
    pub subtype: String,
    pub geometry: Key<VolumeGridGeometry>,
}

#[derive(Debug, Deserialize)]
pub struct VolumeGridGeometry {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(default)]
    pub origin: [f64; 3],
    pub tensor_u: Vec<f64>,
    pub tensor_v: Vec<f64>,
    pub tensor_w: Vec<f64>,
    #[serde(default = "i")]
    pub axis_u: [f64; 3],
    #[serde(default = "j")]
    pub axis_v: [f64; 3],
    #[serde(default = "k")]
    pub axis_w: [f64; 3],
    #[serde(default)]
    pub offset_w: Option<Key<ScalarArray>>,
}

// Colormaps

#[derive(Debug, Deserialize)]
pub struct ScalarColormap {
    #[serde(flatten)]
    pub content: ContentModel,
    pub gradient: Key<ColorArray>,
    pub limits: [f64; 2],
}

#[derive(Debug, Deserialize)]
pub struct DateTimeColormap {
    #[serde(flatten)]
    pub content: ContentModel,
    pub gradient: Key<ColorArray>,
    #[serde(deserialize_with = "date_time_limits")]
    pub limits: [DateTime<Utc>; 2],
}

#[derive(Debug, Deserialize)]
pub struct Legend {
    #[serde(flatten)]
    pub content: ContentModel,
    pub values: Key<LegendArrays>,
}

// Data

#[derive(Debug, Deserialize)]
pub struct ScalarData {
    #[serde(flatten)]
    pub content: ContentModel,
    pub location: DataLocation,
    pub colormap: Option<Key<ScalarColormap>>,
    pub array: Key<ScalarArray>,
}

#[derive(Debug, Deserialize)]
pub struct DateTimeData {
    #[serde(flatten)]
    pub content: ContentModel,
    pub location: DataLocation,
    pub colormap: Option<Key<DateTimeColormap>>,
    pub array: Key<DateTimeArray>,
}

#[derive(Debug, Deserialize)]
pub struct Vector2Data {
    #[serde(flatten)]
    pub content: ContentModel,
    pub location: DataLocation,
    pub array: Key<Vector2Array>,
}

#[derive(Debug, Deserialize)]
pub struct Vector3Data {
    #[serde(flatten)]
    pub content: ContentModel,
    pub location: DataLocation,
    pub array: Key<Vector3Array>,
}

#[derive(Debug, Deserialize)]
pub struct ColorData {
    #[serde(flatten)]
    pub content: ContentModel,
    pub location: DataLocation,
    pub array: Key<ColorArrays>,
}

#[derive(Debug, Deserialize)]
pub struct StringData {
    #[serde(flatten)]
    pub content: ContentModel,
    pub location: DataLocation,
    pub array: Key<StringArray>,
}

#[derive(Debug, Deserialize)]
pub struct MappedData {
    #[serde(flatten)]
    pub content: ContentModel,
    pub location: DataLocation,
    pub array: Key<ScalarArray>,
    #[serde(default)]
    pub legends: Vec<Key<Legend>>,
}

// Texture

#[derive(Debug, Deserialize)]
pub struct ImageTexture {
    #[serde(flatten)]
    pub content: ContentModel,
    #[serde(default)]
    pub origin: [f64; 3],
    #[serde(default = "i")]
    pub axis_u: [f64; 3],
    #[serde(default = "j")]
    pub axis_v: [f64; 3],
    pub image: Image,
}

// Arrays

#[derive(Debug, Deserialize)]
pub struct Array {
    pub start: u64,
    pub length: u64,
    pub dtype: DataType,
}

#[derive(Debug, Deserialize)]
pub struct Image {
    pub start: u64,
    pub length: u64,
    pub dtype: ImageType,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
pub enum DataType {
    #[serde(rename = "<f8")]
    Float,
    #[serde(rename = "<i8")]
    Int,
}

#[derive(Debug, Deserialize)]
pub enum ImageType {
    #[serde(rename = "image/png")]
    Png,
}

#[derive(Debug, Deserialize)]
pub struct ScalarArray {
    #[serde(flatten)]
    pub uid: UidModel,
    pub array: Array,
}

#[derive(Debug, Deserialize)]
pub struct Vector2Array {
    #[serde(flatten)]
    pub uid: UidModel,
    pub array: Array,
}

#[derive(Debug, Deserialize)]
pub struct Vector3Array {
    #[serde(flatten)]
    pub uid: UidModel,
    pub array: Array,
}

#[derive(Debug, Deserialize)]
pub struct Int2Array {
    #[serde(flatten)]
    pub uid: UidModel,
    pub array: Array,
}

#[derive(Debug, Deserialize)]
pub struct Int3Array {
    #[serde(flatten)]
    pub uid: UidModel,
    pub array: Array,
}

#[derive(Debug, Deserialize)]
pub struct StringArray {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(deserialize_with = "strings")]
    pub array: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DateTimeArray {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(deserialize_with = "date_times")]
    pub array: Vec<Option<DateTime<Utc>>>,
}

#[derive(Debug, Deserialize)]
pub struct ColorArray {
    #[serde(flatten)]
    pub uid: UidModel,
    #[serde(deserialize_with = "colors")]
    pub array: Vec<[u8; 3]>,
}

// Enums

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
pub enum DataLocation {
    #[serde(rename = "vertices")]
    Vertices,
    #[serde(rename = "segments")]
    Segments,
    #[serde(rename = "faces")]
    Faces,
    #[serde(rename = "cells")]
    Cells,
}

// Default factories

fn i() -> [f64; 3] {
    [1.0, 0.0, 0.0]
}

fn j() -> [f64; 3] {
    [0.0, 1.0, 0.0]
}

fn k() -> [f64; 3] {
    [0.0, 0.0, 1.0]
}

// Lenient field readers. OMF 1 had one reference writer, but files from other
// applications vary in how they wrote dates, colors and empty fields; a field
// that is merely written differently should not stop the whole file opening.

/// Text that may be written as `null`.
fn text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

/// A list of text where an entry may be `null`, read as empty.
fn strings<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    Ok(Vec::<Option<String>>::deserialize(deserializer)?.into_iter().map(Option::unwrap_or_default).collect())
}

/// Parse an OMF 1 date-time. The reference writer used `2017-10-04T21:46:12Z`;
/// others wrote fractional seconds, offsets, a space instead of the `T`, or
/// slashes, and an empty string for none.
pub fn parse_date_time(text: &str) -> Option<DateTime<Utc>> {
    use chrono::{NaiveDate, NaiveDateTime};
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(value) = DateTime::parse_from_rfc3339(text) {
        return Some(value.with_timezone(&Utc));
    }
    let naive = text.trim_end_matches('Z');
    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f", "%Y/%m/%d %H:%M:%S%.f", "%Y/%m/%dT%H:%M:%S%.f"] {
        if let Ok(value) = NaiveDateTime::parse_from_str(naive, format) {
            return Some(value.and_utc());
        }
    }
    for format in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(value) = NaiveDate::parse_from_str(naive, format) {
            return value.and_hms_opt(0, 0, 0).map(|value| value.and_utc());
        }
    }
    None
}

fn date_time<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.as_deref().and_then(parse_date_time))
}

fn date_times<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Option<DateTime<Utc>>>, D::Error> {
    Ok(Vec::<Option<String>>::deserialize(deserializer)?
        .iter()
        .map(|text| text.as_deref().and_then(parse_date_time))
        .collect())
}

fn date_time_limits<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<[DateTime<Utc>; 2], D::Error> {
    let [min, max] = <[String; 2]>::deserialize(deserializer)?;
    match (parse_date_time(&min), parse_date_time(&max)) {
        (Some(min), Some(max)) => Ok([min, max]),
        _ => Err(serde::de::Error::custom(format!("unreadable date-time limits '{min}' to '{max}'"))),
    }
}

/// A color as `[r, g, b]` (integers, or fractions of one), `#rrggbb`, or
/// `null`.
fn color_value(value: &serde_json::Value) -> Option<[u8; 3]> {
    match value {
        serde_json::Value::Array(items) if items.len() >= 3 => {
            let numbers = items.iter().take(3).map(serde_json::Value::as_f64).collect::<Option<Vec<_>>>()?;
            let fractions = numbers.iter().all(|x| (0.0..=1.0).contains(x)) && numbers.iter().any(|x| x.fract() != 0.0);
            let scale = if fractions { 255.0 } else { 1.0 };
            let channel = |x: f64| (x * scale).round().clamp(0.0, 255.0) as u8;
            Some([channel(numbers[0]), channel(numbers[1]), channel(numbers[2])])
        }
        serde_json::Value::String(text) => {
            let hex = text.trim().trim_start_matches('#');
            if hex.len() != 6 {
                return None;
            }
            let channel = |range: std::ops::Range<usize>| u8::from_str_radix(hex.get(range)?, 16).ok();
            Some([channel(0..2)?, channel(2..4)?, channel(4..6)?])
        }
        _ => None,
    }
}

fn color<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<[u8; 3]>, D::Error> {
    Ok(color_value(&serde_json::Value::deserialize(deserializer)?))
}

/// A list of colors, an unreadable entry read as gray.
fn colors<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<[u8; 3]>, D::Error> {
    Ok(Vec::<serde_json::Value>::deserialize(deserializer)?
        .iter()
        .map(|value| color_value(value).unwrap_or([128, 128, 128]))
        .collect())
}
