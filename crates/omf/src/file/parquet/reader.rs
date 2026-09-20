use crate::{
    Array, ArrayType, array_type,
    data::*,
    error::{Error, InvalidData},
    file::{ReadAt, SubFile},
    pqarray::PqArrayReader,
};

use super::{super::Reader, schemas};

impl<R: ReadAt> Reader<R> {
    fn array_reader(
        &self,
        array: &Array<impl ArrayType>,
    ) -> Result<PqArrayReader<SubFile<R>>, Error> {
        let f = self.array_bytes_reader(array)?;
        let reader = PqArrayReader::new(f)?;
        if array.item_count() != reader.len() {
            Err(InvalidData::LengthMismatch {
                found: array.item_count(),
                expected: reader.len(),
            }
            .into())
        } else {
            Ok(reader)
        }
    }

    /// Read an [`array_type::Scalar`](crate::array_type::Scalar) array.
    pub fn array_scalars(&self, array: &Array<array_type::Scalar>) -> Result<Scalars<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Scalar::check(&reader)? {
            schemas::Scalar::F32 => {
                let inner = reader.iter_column::<f32>("scalar")?;
                Scalars::F32(GenericScalars::new(inner, array.constraint()))
            }
            schemas::Scalar::F64 => {
                let inner = reader.iter_column::<f64>("scalar")?;
                Scalars::F64(GenericScalars::new(inner, array.constraint()))
            }
        })
    }

    /// Read an [`array_type::Vertex`](crate::array_type::Vertex) array.
    pub fn array_vertices(&self, array: &Array<array_type::Vertex>) -> Result<Vertices<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Vertex::check(&reader)? {
            schemas::Vertex::F32 => {
                Vertices::F32(GenericArrays(reader.iter_multi_column(["x", "y", "z"])?))
            }
            schemas::Vertex::F64 => {
                Vertices::F64(GenericArrays(reader.iter_multi_column(["x", "y", "z"])?))
            }
        })
    }

    /// Read an [`array_type::Vertex`](crate::array_type::Vertex) array into a
    /// single vector.
    ///
    /// Equivalent to collecting [`Self::array_vertices`], but the columns are
    /// decoded in bulk and row groups run in parallel. Prefer it whenever the
    /// whole array is wanted in memory anyway.
    pub fn array_vertices_vec(
        &self,
        array: &Array<array_type::Vertex>,
    ) -> Result<Vec<[f64; 3]>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Vertex::check(&reader)? {
            schemas::Vertex::F32 => reader
                .read_multi_column::<f32, 3>(["x", "y", "z"])?
                .into_iter()
                .map(|vertex| vertex.map(f64::from))
                .collect(),
            schemas::Vertex::F64 => reader.read_multi_column::<f64, 3>(["x", "y", "z"])?,
        })
    }

    /// Read an [`array_type::Segment`](crate::array_type::Segment) array.
    pub fn array_segments(
        &self,
        array: &Array<array_type::Segment>,
    ) -> Result<GenericPrimitives<2, R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Segment::check(&reader)? {
            schemas::Segment::U32 => {
                GenericPrimitives::new(reader.iter_multi_column(["a", "b"])?, array.constraint())
            }
        })
    }

    /// Read an [`array_type::Segment`](crate::array_type::Segment) array into a
    /// single vector, checking the indices as [`Self::array_segments`] does.
    pub fn array_segments_vec(
        &self,
        array: &Array<array_type::Segment>,
    ) -> Result<Vec<[u32; 2]>, Error> {
        let reader = self.array_reader(array)?;
        let schemas::Segment::U32 = schemas::Segment::check(&reader)?;
        check_indices(
            reader.read_multi_column::<u32, 2>(["a", "b"])?,
            array.constraint(),
        )
    }

    /// Read an [`array_type::Triangle`](crate::array_type::Triangle) array into
    /// a single vector, checking the indices as [`Self::array_triangles`] does.
    pub fn array_triangles_vec(
        &self,
        array: &Array<array_type::Triangle>,
    ) -> Result<Vec<[u32; 3]>, Error> {
        let reader = self.array_reader(array)?;
        let schemas::Triangle::U32 = schemas::Triangle::check(&reader)?;
        check_indices(
            reader.read_multi_column::<u32, 3>(["a", "b", "c"])?,
            array.constraint(),
        )
    }

    /// Read an [`array_type::Triangle`](crate::array_type::Triangle) array.
    pub fn array_triangles(
        &self,
        array: &Array<array_type::Triangle>,
    ) -> Result<GenericPrimitives<3, R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Triangle::check(&reader)? {
            schemas::Triangle::U32 => GenericPrimitives::new(
                reader.iter_multi_column(["a", "b", "c"])?,
                array.constraint(),
            ),
        })
    }

    /// Read an [`array_type::Name`](crate::array_type::Name) array.
    pub fn array_names(&self, array: &Array<array_type::Name>) -> Result<Names<R>, Error> {
        let reader = self.array_reader(array)?;
        schemas::Name::check(&reader)?;
        reader.iter_column("name").map(Names)
    }

    /// Read an [`array_type::Gradient`](crate::array_type::Gradient) array.
    pub fn array_gradient(
        &self,
        array: &Array<array_type::Gradient>,
    ) -> Result<Gradient<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Gradient::check(&reader)? {
            schemas::Gradient::Rgba8 => Gradient(reader.iter_multi_column(["r", "g", "b", "a"])?),
        })
    }

    /// Read an [`array_type::Texcoord`](crate::array_type::Texcoord) array.
    pub fn array_texcoords(
        &self,
        array: &Array<array_type::Texcoord>,
    ) -> Result<Texcoords<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Texcoord::check(&reader)? {
            schemas::Texcoord::F32 => {
                Texcoords::F32(GenericArrays(reader.iter_multi_column(["u", "v"])?))
            }
            schemas::Texcoord::F64 => {
                Texcoords::F64(GenericArrays(reader.iter_multi_column(["u", "v"])?))
            }
        })
    }

    /// Read an [`array_type::Boundary`](crate::array_type::Boundary) array.
    pub fn array_boundaries(
        &self,
        array: &Array<array_type::Boundary>,
    ) -> Result<Boundaries<R>, Error> {
        let reader = self.array_reader(array)?;
        let m = schemas::Boundary::check(&reader)?;
        let inclusive = reader.iter_column("inclusive")?;
        Ok(match m {
            schemas::Boundary::F32 => Boundaries::F32(GenericBoundaries::new(
                reader.iter_column("value")?,
                inclusive,
            )),
            schemas::Boundary::F64 => Boundaries::F64(GenericBoundaries::new(
                reader.iter_column("value")?,
                inclusive,
            )),
            schemas::Boundary::I64 => Boundaries::I64(GenericBoundaries::new(
                reader.iter_column("value")?,
                inclusive,
            )),
            schemas::Boundary::Date => Boundaries::Date(GenericBoundaries::new(
                reader.iter_column("value")?,
                inclusive,
            )),
            schemas::Boundary::DateTime => Boundaries::DateTime(GenericBoundaries::new(
                reader.iter_column("value")?,
                inclusive,
            )),
        })
    }

    /// Read an [`array_type::RegularSubblock`](crate::array_type::RegularSubblock) array.
    pub fn array_regular_subblocks(
        &self,
        array: &Array<array_type::RegularSubblock>,
    ) -> Result<RegularSubblocks<R>, Error> {
        let reader = self.array_reader(array)?;
        schemas::RegularSubblock::check(&reader)?;
        let parents = reader.iter_multi_column(["parent_u", "parent_v", "parent_w"])?;
        let corners = reader.iter_multi_column([
            "corner_min_u",
            "corner_min_v",
            "corner_min_w",
            "corner_max_u",
            "corner_max_v",
            "corner_max_w",
        ])?;
        Ok(RegularSubblocks::new(parents, corners, array.constraint()))
    }

    /// Read an [`array_type::FreeformSubblock`](crate::array_type::FreeformSubblock) array.
    pub fn array_freeform_subblocks(
        &self,
        array: &Array<array_type::FreeformSubblock>,
    ) -> Result<FreeformSubblocks<R>, Error> {
        let reader = self.array_reader(array)?;
        let m = schemas::FreeformSubblock::check(&reader)?;
        let parents = reader.iter_multi_column(["parent_u", "parent_v", "parent_w"])?;
        Ok(match m {
            schemas::FreeformSubblock::U32F32 => {
                FreeformSubblocks::F32(GenericFreeformSubblocks::new(
                    parents,
                    reader.iter_multi_column([
                        "corner_min_u",
                        "corner_min_v",
                        "corner_min_w",
                        "corner_max_u",
                        "corner_max_v",
                        "corner_max_w",
                    ])?,
                    array.constraint(),
                ))
            }
            schemas::FreeformSubblock::U32F64 => {
                FreeformSubblocks::F64(GenericFreeformSubblocks::new(
                    parents,
                    reader.iter_multi_column([
                        "corner_min_u",
                        "corner_min_v",
                        "corner_min_w",
                        "corner_max_u",
                        "corner_max_v",
                        "corner_max_w",
                    ])?,
                    array.constraint(),
                ))
            }
        })
    }

    /// Read an [`array_type::Number`](crate::array_type::Number) array.
    pub fn array_numbers(&self, array: &Array<array_type::Number>) -> Result<Numbers<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Number::check(&reader)? {
            schemas::Number::F32 => {
                Numbers::F32(GenericNumbers(reader.iter_nullable_column("number")?))
            }
            schemas::Number::F64 => {
                Numbers::F64(GenericNumbers(reader.iter_nullable_column("number")?))
            }
            schemas::Number::I64 => {
                Numbers::I64(GenericNumbers(reader.iter_nullable_column("number")?))
            }
            schemas::Number::Date => {
                Numbers::Date(GenericNumbers(reader.iter_nullable_column("number")?))
            }
            schemas::Number::DateTime => {
                Numbers::DateTime(GenericNumbers(reader.iter_nullable_column("number")?))
            }
        })
    }

    /// Read an [`array_type::Index`](crate::array_type::Index) array.
    pub fn array_indices(&self, array: &Array<array_type::Index>) -> Result<Indices<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Index::check(&reader)? {
            schemas::Index::U32 => {
                Indices::new(reader.iter_nullable_column("index")?, array.constraint())
            }
        })
    }

    /// Read an [`array_type::Vector`](crate::array_type::Vector) array.
    pub fn array_vectors(&self, array: &Array<array_type::Vector>) -> Result<Vectors<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Vector::check(&reader)? {
            schemas::Vector::F32x2 => Vectors::F32x2(GenericOptionalArrays(
                reader.iter_nullable_group_column("vector", ["x", "y"])?,
            )),
            schemas::Vector::F64x2 => Vectors::F64x2(GenericOptionalArrays(
                reader.iter_nullable_group_column("vector", ["x", "y"])?,
            )),
            schemas::Vector::F32x3 => Vectors::F32x3(GenericOptionalArrays(
                reader.iter_nullable_group_column("vector", ["x", "y", "z"])?,
            )),
            schemas::Vector::F64x3 => Vectors::F64x3(GenericOptionalArrays(
                reader.iter_nullable_group_column("vector", ["x", "y", "z"])?,
            )),
        })
    }

    /// Read a [`array_type::Text](crate::array_type::Text) array.
    pub fn array_text(&self, array: &Array<array_type::Text>) -> Result<Text<R>, Error> {
        let reader = self.array_reader(array)?;
        schemas::Text::check(&reader)?;
        reader.iter_nullable_column("text").map(Text)
    }

    /// Read an [`array_type::Boolean`](crate::array_type::Boolean) array.
    pub fn array_booleans(&self, array: &Array<array_type::Boolean>) -> Result<Booleans<R>, Error> {
        let reader = self.array_reader(array)?;
        schemas::Boolean::check(&reader)?;
        reader.iter_nullable_column("bool").map(Booleans)
    }

    /// Read an [`array_type::Color`](crate::array_type::Color) array.
    pub fn array_colors(&self, array: &Array<array_type::Color>) -> Result<Colors<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Color::check(&reader)? {
            schemas::Color::Rgba8 => {
                Colors(reader.iter_nullable_group_column("color", ["r", "g", "b", "a"])?)
            }
        })
    }
}

/// The bulk counterpart of the per-item range check in
/// [`GenericPrimitives`](crate::data::GenericPrimitives).
fn check_indices<const N: usize>(
    primitives: Vec<[u32; N]>,
    constraint: &crate::array::Constraint,
) -> Result<Vec<[u32; N]>, Error> {
    use crate::array::Constraint;
    let vertex_count = match constraint {
        Constraint::Segment(n) | Constraint::Triangle(n) => *n,
        _ => panic!("invalid constraint"),
    };
    for primitive in &primitives {
        for index in primitive {
            let index: u64 = (*index).into();
            if index >= vertex_count {
                return Err(InvalidData::IndexOutOfRange {
                    value: index,
                    maximum: vertex_count.saturating_add(1),
                }
                .into());
            }
        }
    }
    Ok(primitives)
}
