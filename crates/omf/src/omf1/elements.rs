use std::io::{Seek, Write};

use super::{
    array::{scalars_array, segments_array, triangles_array, vertices_array},
    model::*,
    objects::{parse_date_time, *},
    reader::Omf1Reader,
};
use crate::{
    date_time::utc_now,
    error::Error,
    file::{ReadAt, Writer},
};

impl UidModel {
    /// Nothing: an OMF 1 record's creation and modification times describe
    /// the file it was in, not the data, and the reader of the conversion
    /// has no use for them.
    pub fn metadata(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::new()
    }
}

impl Project {
    pub fn convert<W: Write + Seek + Send, R: ReadAt>(&self, r: &Omf1Reader<R>, w: &mut Writer<W>) -> Result<crate::Project, Error> {
        let metadata = self.content.uid.metadata();
        Ok(crate::Project {
            name: self.content.name.clone(),
            description: self.content.description.clone(),
            coordinate_reference_system: Default::default(),
            units: self.units.clone(),
            origin: self.origin,
            author: self.author.clone(),
            application: Default::default(),
            date: self.date.or_else(|| parse_date_time(&self.content.uid.date_created)).unwrap_or_else(utc_now),
            metadata,
            elements: self
                .elements
                .iter()
                .map(|k| {
                    let element = r.model(k)?;
                    element.convert(r, w)
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

impl ElementModel<'_> {
    pub fn convert<W: Write + Seek + Send, R: ReadAt>(&self, r: &Omf1Reader<R>, w: &mut Writer<W>) -> Result<crate::Element, Error> {
        match *self {
            Self::PointSetElement(x) => x.convert(r, w),
            Self::LineSetElement(x) => x.convert(r, w),
            Self::SurfaceElement(x) => x.convert(r, w),
            Self::VolumeElement(x) => x.convert(r, w),
        }
    }
}

impl PointSetElement {
    pub fn convert<W: Write + Seek + Send, R: ReadAt>(&self, r: &Omf1Reader<R>, w: &mut Writer<W>) -> Result<crate::Element, Error> {
        let geometry = r.model(&self.geometry)?;
        element(
            &self.content,
            self.color,
            attributes_and_textures(r, w, &self.data, &self.textures)?,
            crate::PointSet {
                origin: geometry.origin,
                vertices: vertices_array(r, w, &geometry.vertices)?,
            },
        )
    }
}

impl LineSetElement {
    pub fn convert<W: Write + Seek + Send, R: ReadAt>(&self, r: &Omf1Reader<R>, w: &mut Writer<W>) -> Result<crate::Element, Error> {
        let geometry = r.model(&self.geometry)?;
        element(
            &self.content,
            self.color,
            attributes(r, w, &self.data)?,
            crate::LineSet {
                origin: geometry.origin,
                vertices: vertices_array(r, w, &geometry.vertices)?,
                segments: segments_array(r, w, &geometry.segments)?,
            },
        )
    }
}

impl SurfaceElement {
    pub fn convert<W: Write + Seek + Send, R: ReadAt>(&self, r: &Omf1Reader<R>, w: &mut Writer<W>) -> Result<crate::Element, Error> {
        match r.model(&self.geometry)? {
            SurfaceGeometryModel::SurfaceGeometry(geometry) => element(
                &self.content,
                self.color,
                attributes_and_textures(r, w, &self.data, &self.textures)?,
                crate::Surface {
                    origin: geometry.origin,
                    vertices: vertices_array(r, w, &geometry.vertices)?,
                    triangles: triangles_array(r, w, &geometry.triangles)?,
                },
            ),
            SurfaceGeometryModel::SurfaceGridGeometry(geometry) => element(
                &self.content,
                self.color,
                attributes_and_textures(r, w, &self.data, &self.textures)?,
                crate::GridSurface {
                    orient: crate::Orient2 {
                        origin: geometry.origin,
                        u: unit(geometry.axis_u),
                        v: unit(geometry.axis_v),
                    },
                    grid: crate::Grid2::Tensor {
                        u: w.array_scalars(geometry.tensor_u.iter().copied())?,
                        v: w.array_scalars(geometry.tensor_v.iter().copied())?,
                    },
                    heights: geometry.offset_w.as_ref().map(|a| scalars_array(r, w, a)).transpose()?,
                },
            ),
        }
    }
}

impl VolumeElement {
    pub fn convert<W: Write + Seek + Send, R: ReadAt>(&self, r: &Omf1Reader<R>, w: &mut Writer<W>) -> Result<crate::Element, Error> {
        let geometry = r.model(&self.geometry)?;
        element(
            &self.content,
            self.color,
            attributes(r, w, &self.data)?,
            crate::BlockModel {
                orient: crate::Orient3 {
                    origin: geometry.origin,
                    u: unit(geometry.axis_u),
                    v: unit(geometry.axis_v),
                    w: unit(geometry.axis_w),
                },
                grid: crate::Grid3::Tensor {
                    u: Box::new(w.array_scalars(geometry.tensor_u.iter().copied())?),
                    v: Box::new(w.array_scalars(geometry.tensor_v.iter().copied())?),
                    w: Box::new(w.array_scalars(geometry.tensor_w.iter().copied())?),
                },
                subblocks: None,
            },
        )
    }
}

impl DataModel<'_> {
    pub fn convert<W: Write + Seek + Send, R: ReadAt>(&self, r: &Omf1Reader<R>, w: &mut Writer<W>) -> Result<crate::Attribute, Error> {
        match *self {
            DataModel::ScalarData(x) => x.convert(r, w),
            DataModel::DateTimeData(x) => x.convert(r, w),
            DataModel::Vector2Data(x) => x.convert(r, w),
            DataModel::Vector3Data(x) => x.convert(r, w),
            DataModel::ColorData(x) => x.convert(r, w),
            DataModel::StringData(x) => x.convert(r, w),
            DataModel::MappedData(x) => x.convert(r, w),
        }
    }
}

fn element(content: &ContentModel, color: Option<[u8; 3]>, attributes: Vec<crate::Attribute>, geometry: impl Into<crate::Geometry>) -> Result<crate::Element, Error> {
    Ok(crate::Element {
        name: content.name.clone(),
        description: content.description.clone(),
        color: color.map(|[r, g, b]| [r, g, b, u8::MAX]),
        metadata: content.uid.metadata(),
        attributes,
        geometry: geometry.into(),
    })
}

fn attributes<W: Write + Seek + Send, R: ReadAt>(r: &Omf1Reader<R>, w: &mut Writer<W>, data: &[Key<Data>]) -> Result<Vec<crate::Attribute>, Error> {
    data.iter().map(|key| r.model(key)?.convert(r, w)).collect::<Result<Vec<_>, _>>()
}

fn attributes_and_textures<W: Write + Seek + Send, R: ReadAt>(
    r: &Omf1Reader<R>,
    w: &mut Writer<W>,
    data: &[Key<Data>],
    textures: &[Key<ImageTexture>],
) -> Result<Vec<crate::Attribute>, Error> {
    let mut data_attributes = data.iter().map(|key| r.model(key)?.convert(r, w)).collect::<Result<Vec<_>, _>>()?;
    let mut texture_attributes = textures.iter().map(|key| r.model(key)?.convert(r, w)).collect::<Result<Vec<_>, _>>()?;
    data_attributes.append(&mut texture_attributes);
    Ok(data_attributes)
}

/// `axis` scaled to unit length. OMF 1 asked for unit axes but wrote what it
/// was given; OMF 2 refuses anything else.
fn unit(axis: [f64; 3]) -> [f64; 3] {
    let length = axis.iter().map(|x| x * x).sum::<f64>().sqrt();
    if length > 0.0 && length.is_finite() { axis.map(|x| x / length) } else { axis }
}
