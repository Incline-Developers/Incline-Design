use rayon::prelude::*;

use super::{super::Reader, schemas};
use crate::{
    Array, ArrayType, array_type,
    data::*,
    error::{Error, InvalidData},
    file::{ReadAt, SubFile},
    pqarray::PqArrayReader,
};

impl<R: ReadAt> Reader<R> {
    fn array_reader(&self, array: &Array<impl ArrayType>) -> Result<PqArrayReader<SubFile<R>>, Error> {
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
            schemas::Vertex::F32 => Vertices::F32(GenericArrays(reader.iter_multi_column(["x", "y", "z"])?)),
            schemas::Vertex::F64 => Vertices::F64(GenericArrays(reader.iter_multi_column(["x", "y", "z"])?)),
        })
    }

    /// Read an [`array_type::Vertex`](crate::array_type::Vertex) array into a
    /// single vector.
    ///
    /// Equivalent to collecting [`Self::array_vertices`], but the columns are
    /// decoded in bulk and row groups run in parallel. Prefer it whenever the
    /// whole array is wanted in memory anyway.
    pub fn array_vertices_vec(&self, array: &Array<array_type::Vertex>) -> Result<Vec<[f64; 3]>, Error> {
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
    pub fn array_segments(&self, array: &Array<array_type::Segment>) -> Result<GenericPrimitives<2, R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Segment::check(&reader)? {
            schemas::Segment::U32 => GenericPrimitives::new(reader.iter_multi_column(["a", "b"])?, array.constraint()),
        })
    }

    /// Read an [`array_type::Segment`](crate::array_type::Segment) array into a
    /// single vector, checking the indices as [`Self::array_segments`] does.
    pub fn array_segments_vec(&self, array: &Array<array_type::Segment>) -> Result<Vec<[u32; 2]>, Error> {
        let reader = self.array_reader(array)?;
        let schemas::Segment::U32 = schemas::Segment::check(&reader)?;
        check_indices(reader.read_multi_column::<u32, 2>(["a", "b"])?, array.constraint())
    }

    /// Read an [`array_type::Triangle`](crate::array_type::Triangle) array into
    /// a single vector, checking the indices as [`Self::array_triangles`] does.
    pub fn array_triangles_vec(&self, array: &Array<array_type::Triangle>) -> Result<Vec<[u32; 3]>, Error> {
        let reader = self.array_reader(array)?;
        let schemas::Triangle::U32 = schemas::Triangle::check(&reader)?;
        check_indices(reader.read_multi_column::<u32, 3>(["a", "b", "c"])?, array.constraint())
    }

    /// Read an [`array_type::Triangle`](crate::array_type::Triangle) array.
    pub fn array_triangles(&self, array: &Array<array_type::Triangle>) -> Result<GenericPrimitives<3, R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Triangle::check(&reader)? {
            schemas::Triangle::U32 => GenericPrimitives::new(reader.iter_multi_column(["a", "b", "c"])?, array.constraint()),
        })
    }

    /// Read an [`array_type::Name`](crate::array_type::Name) array.
    pub fn array_names(&self, array: &Array<array_type::Name>) -> Result<Names<R>, Error> {
        let reader = self.array_reader(array)?;
        schemas::Name::check(&reader)?;
        reader.iter_column("name").map(Names)
    }

    /// Read an [`array_type::Gradient`](crate::array_type::Gradient) array.
    pub fn array_gradient(&self, array: &Array<array_type::Gradient>) -> Result<Gradient<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Gradient::check(&reader)? {
            schemas::Gradient::Rgba8 => Gradient(reader.iter_multi_column(["r", "g", "b", "a"])?),
        })
    }

    /// Read an [`array_type::Texcoord`](crate::array_type::Texcoord) array.
    pub fn array_texcoords(&self, array: &Array<array_type::Texcoord>) -> Result<Texcoords<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Texcoord::check(&reader)? {
            schemas::Texcoord::F32 => Texcoords::F32(GenericArrays(reader.iter_multi_column(["u", "v"])?)),
            schemas::Texcoord::F64 => Texcoords::F64(GenericArrays(reader.iter_multi_column(["u", "v"])?)),
        })
    }

    /// Read an [`array_type::Boundary`](crate::array_type::Boundary) array.
    pub fn array_boundaries(&self, array: &Array<array_type::Boundary>) -> Result<Boundaries<R>, Error> {
        let reader = self.array_reader(array)?;
        let m = schemas::Boundary::check(&reader)?;
        let inclusive = reader.iter_column("inclusive")?;
        Ok(match m {
            schemas::Boundary::F32 => Boundaries::F32(GenericBoundaries::new(reader.iter_column("value")?, inclusive)),
            schemas::Boundary::F64 => Boundaries::F64(GenericBoundaries::new(reader.iter_column("value")?, inclusive)),
            schemas::Boundary::I64 => Boundaries::I64(GenericBoundaries::new(reader.iter_column("value")?, inclusive)),
            schemas::Boundary::Date => Boundaries::Date(GenericBoundaries::new(reader.iter_column("value")?, inclusive)),
            schemas::Boundary::DateTime => Boundaries::DateTime(GenericBoundaries::new(reader.iter_column("value")?, inclusive)),
        })
    }

    /// Read an [`array_type::RegularSubblock`](crate::array_type::RegularSubblock) array.
    pub fn array_regular_subblocks(&self, array: &Array<array_type::RegularSubblock>) -> Result<RegularSubblocks<R>, Error> {
        let reader = self.array_reader(array)?;
        schemas::RegularSubblock::check(&reader)?;
        let parents = reader.iter_multi_column(["parent_u", "parent_v", "parent_w"])?;
        let corners = reader.iter_multi_column(["corner_min_u", "corner_min_v", "corner_min_w", "corner_max_u", "corner_max_v", "corner_max_w"])?;
        Ok(RegularSubblocks::new(parents, corners, array.constraint()))
    }

    /// Open an [`array_type::FreeformSubblock`](crate::array_type::FreeformSubblock)
    /// array to be read a row group at a time, so the groups can be decoded
    /// side by side. Each group is checked as [`Self::array_freeform_subblocks`]
    /// checks every item.
    pub fn array_freeform_subblock_groups(&self, array: &Array<array_type::FreeformSubblock>) -> Result<FreeformSubblockGroups<R>, Error> {
        let reader = self.array_reader(array)?;
        let single = matches!(schemas::FreeformSubblock::check(&reader)?, schemas::FreeformSubblock::U32F32);
        let block_count = match array.constraint() {
            &crate::array::Constraint::RegularSubblock { block_count, .. } | &crate::array::Constraint::FreeformSubblock { block_count } => block_count,
            _ => panic!("invalid constraint"),
        };
        Ok(FreeformSubblockGroups { reader, single, block_count })
    }

    /// Read an [`array_type::FreeformSubblock`](crate::array_type::FreeformSubblock) array.
    pub fn array_freeform_subblocks(&self, array: &Array<array_type::FreeformSubblock>) -> Result<FreeformSubblocks<R>, Error> {
        let reader = self.array_reader(array)?;
        let m = schemas::FreeformSubblock::check(&reader)?;
        let parents = reader.iter_multi_column(["parent_u", "parent_v", "parent_w"])?;
        Ok(match m {
            schemas::FreeformSubblock::U32F32 => FreeformSubblocks::F32(GenericFreeformSubblocks::new(
                parents,
                reader.iter_multi_column(["corner_min_u", "corner_min_v", "corner_min_w", "corner_max_u", "corner_max_v", "corner_max_w"])?,
                array.constraint(),
            )),
            schemas::FreeformSubblock::U32F64 => FreeformSubblocks::F64(GenericFreeformSubblocks::new(
                parents,
                reader.iter_multi_column(["corner_min_u", "corner_min_v", "corner_min_w", "corner_max_u", "corner_max_v", "corner_max_w"])?,
                array.constraint(),
            )),
        })
    }

    /// Open an [`array_type::Number`](crate::array_type::Number) array to be read
    /// a row group at a time, so the groups can be decoded side by side.
    pub fn array_number_groups(&self, array: &Array<array_type::Number>) -> Result<NumberGroups<R>, Error> {
        let reader = self.array_reader(array)?;
        let kind = schemas::Number::check(&reader)?;
        Ok(NumberGroups { reader, kind })
    }

    /// Open an [`array_type::Index`](crate::array_type::Index) array to be read a
    /// row group at a time, each checked as [`Self::array_indices`] checks every item.
    pub fn array_index_groups(&self, array: &Array<array_type::Index>) -> Result<IndexGroups<R>, Error> {
        let reader = self.array_reader(array)?;
        let schemas::Index::U32 = schemas::Index::check(&reader)?;
        let &crate::array::Constraint::Index(category_count) = array.constraint() else {
            panic!("invalid constraint");
        };
        Ok(IndexGroups { reader, category_count })
    }

    /// Read an [`array_type::Number`](crate::array_type::Number) array.
    pub fn array_numbers(&self, array: &Array<array_type::Number>) -> Result<Numbers<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Number::check(&reader)? {
            schemas::Number::F32 => Numbers::F32(GenericNumbers(reader.iter_nullable_column("number")?)),
            schemas::Number::F64 => Numbers::F64(GenericNumbers(reader.iter_nullable_column("number")?)),
            schemas::Number::I64 => Numbers::I64(GenericNumbers(reader.iter_nullable_column("number")?)),
            schemas::Number::Date => Numbers::Date(GenericNumbers(reader.iter_nullable_column("number")?)),
            schemas::Number::DateTime => Numbers::DateTime(GenericNumbers(reader.iter_nullable_column("number")?)),
        })
    }

    /// Read an [`array_type::Index`](crate::array_type::Index) array.
    pub fn array_indices(&self, array: &Array<array_type::Index>) -> Result<Indices<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Index::check(&reader)? {
            schemas::Index::U32 => Indices::new(reader.iter_nullable_column("index")?, array.constraint()),
        })
    }

    /// Read an [`array_type::Index`](crate::array_type::Index) array into a single
    /// vector, checking the indices as [`Self::array_indices`] does.
    pub fn array_indices_vec(&self, array: &Array<array_type::Index>) -> Result<Vec<Option<u32>>, Error> {
        let reader = self.array_reader(array)?;
        let schemas::Index::U32 = schemas::Index::check(&reader)?;
        let &crate::array::Constraint::Index(category_count) = array.constraint() else {
            panic!("invalid constraint");
        };
        let indices = reader.read_nullable_column::<u32>("index")?;
        if let Some(index) = indices.par_iter().flatten().map(|index| u64::from(*index)).find_any(|index| *index >= category_count) {
            return Err(InvalidData::IndexOutOfRange {
                value: index,
                maximum: category_count.saturating_add(1),
            }
            .into());
        }
        Ok(indices)
    }

    /// Read an [`array_type::Vector`](crate::array_type::Vector) array.
    pub fn array_vectors(&self, array: &Array<array_type::Vector>) -> Result<Vectors<R>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Vector::check(&reader)? {
            schemas::Vector::F32x2 => Vectors::F32x2(GenericOptionalArrays(reader.iter_nullable_group_column("vector", ["x", "y"])?)),
            schemas::Vector::F64x2 => Vectors::F64x2(GenericOptionalArrays(reader.iter_nullable_group_column("vector", ["x", "y"])?)),
            schemas::Vector::F32x3 => Vectors::F32x3(GenericOptionalArrays(reader.iter_nullable_group_column("vector", ["x", "y", "z"])?)),
            schemas::Vector::F64x3 => Vectors::F64x3(GenericOptionalArrays(reader.iter_nullable_group_column("vector", ["x", "y", "z"])?)),
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
            schemas::Color::Rgba8 => Colors(reader.iter_nullable_group_column("color", ["r", "g", "b", "a"])?),
        })
    }

    /// Read an [`array_type::Color`](crate::array_type::Color) array into a single vector.
    ///
    /// Equivalent to collecting [`Self::array_colors`], but decoded in bulk with row
    /// groups in parallel, like [`Self::array_vertices_vec`].
    pub fn array_colors_vec(&self, array: &Array<array_type::Color>) -> Result<Vec<Option<[u8; 4]>>, Error> {
        let reader = self.array_reader(array)?;
        Ok(match schemas::Color::check(&reader)? {
            schemas::Color::Rgba8 => reader.read_nullable_group_column::<u8, 4>("color", ["r", "g", "b", "a"])?,
        })
    }
}

/// The bulk counterpart of the per-item range check in
/// [`GenericPrimitives`](crate::data::GenericPrimitives).
fn check_indices<const N: usize>(primitives: Vec<[u32; N]>, constraint: &crate::array::Constraint) -> Result<Vec<[u32; N]>, Error> {
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

/// One row group of sub-blocks: each one's parent index, and its corners a
/// column each.
pub type SubblockGroup = (Vec<[u32; 3]>, [Vec<f64>; 6]);

/// A free-form sub-block array, read a row group at a time: see
/// [`Reader::array_freeform_subblock_groups`].
pub struct FreeformSubblockGroups<R: ReadAt> {
    reader: PqArrayReader<SubFile<R>>,
    /// Corners stored as `f32` rather than `f64`.
    single: bool,
    block_count: [u32; 3],
}

impl<R: ReadAt> FreeformSubblockGroups<R> {
    /// How many groups there are.
    pub fn len(&self) -> usize {
        self.reader.row_group_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One group's sub-blocks, in order: each one's parent index, and its
    /// corners a column each (minimum u, v, w, then maximum u, v, w). Columns
    /// rather than rows, so a caller can turn them into what it keeps in place.
    pub fn read(&self, group: usize) -> Result<SubblockGroup, Error> {
        const CORNERS: [&str; 6] = ["corner_min_u", "corner_min_v", "corner_min_w", "corner_max_u", "corner_max_v", "corner_max_w"];
        let parents = self.reader.read_multi_column_group::<u32, 3>(["parent_u", "parent_v", "parent_w"], group)?;
        let mut corners: [Vec<f64>; 6] = Default::default();
        for (column, name) in corners.iter_mut().zip(CORNERS) {
            *column = if self.single {
                self.reader.read_column_group::<f32>(name, group)?.into_iter().map(f64::from).collect()
            } else {
                self.reader.read_column_group::<f64>(name, group)?
            };
            if column.len() != parents.len() {
                return Err(InvalidData::LengthMismatch {
                    found: column.len() as u64,
                    expected: parents.len() as u64,
                }
                .into());
            }
        }
        for (row, parent) in parents.iter().enumerate() {
            check_parent(self.block_count, *parent)?;
            check_freeform_corners(std::array::from_fn(|axis| corners[axis][row]))?;
        }
        Ok((parents, corners))
    }
}

/// A number array, read a row group at a time: see [`Reader::array_number_groups`].
pub struct NumberGroups<R: ReadAt> {
    reader: PqArrayReader<SubFile<R>>,
    kind: schemas::Number,
}

impl<R: ReadAt> NumberGroups<R> {
    /// How many groups there are.
    pub fn len(&self) -> usize {
        self.reader.row_group_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the numbers are 64-bit integers, which [`Self::read_f64`] refuses.
    pub fn is_integer(&self) -> bool {
        self.kind == schemas::Number::I64
    }

    /// One group's numbers as `f64`, cast as [`Numbers::try_into_f64`] casts them.
    pub fn read_f64(&self, group: usize) -> Result<Vec<Option<f64>>, Error> {
        use crate::date_time::{date_time_to_f64, date_to_f64};
        let reader = &self.reader;
        Ok(match self.kind {
            schemas::Number::F32 => cast(reader.read_nullable_column_group::<f32>("number", group)?, f64::from),
            schemas::Number::F64 => reader.read_nullable_column_group::<f64>("number", group)?,
            schemas::Number::Date => cast(reader.read_nullable_column_group("number", group)?, date_to_f64),
            schemas::Number::DateTime => cast(reader.read_nullable_column_group("number", group)?, date_time_to_f64),
            schemas::Number::I64 => return Err(Error::UnsafeCast("64-bit integer", "64-bit float")),
        })
    }

    /// One group's numbers as `f64`, as [`Self::read_f64`] gives them but with
    /// `missing` in place of each null.
    pub fn read_f64_or(&self, group: usize, missing: f64) -> Result<Vec<f64>, Error> {
        Ok(match self.kind {
            schemas::Number::F64 => self.reader.read_nullable_column_group_or::<f64>("number", group, missing)?,
            _ => self.read_f64(group)?.into_iter().map(|value| value.unwrap_or(missing)).collect(),
        })
    }

    /// One group's numbers as `i64`, cast as [`Numbers::try_into_i64`] casts them.
    pub fn read_i64(&self, group: usize) -> Result<Vec<Option<i64>>, Error> {
        use crate::date_time::{date_time_to_i64, date_to_i64};
        let reader = &self.reader;
        Ok(match self.kind {
            schemas::Number::F32 => return Err(Error::UnsafeCast("32-bit float", "64-bit integer")),
            schemas::Number::F64 => return Err(Error::UnsafeCast("64-bit float", "64-bit integer")),
            schemas::Number::I64 => reader.read_nullable_column_group::<i64>("number", group)?,
            schemas::Number::Date => cast(reader.read_nullable_column_group("number", group)?, date_to_i64),
            schemas::Number::DateTime => cast(reader.read_nullable_column_group("number", group)?, date_time_to_i64),
        })
    }
}

fn cast<T, U>(values: Vec<Option<T>>, convert: impl Fn(T) -> U) -> Vec<Option<U>> {
    values.into_iter().map(|value| value.map(&convert)).collect()
}

/// An index array, read a row group at a time: see [`Reader::array_index_groups`].
pub struct IndexGroups<R: ReadAt> {
    reader: PqArrayReader<SubFile<R>>,
    category_count: u64,
}

impl<R: ReadAt> IndexGroups<R> {
    /// How many groups there are.
    pub fn len(&self) -> usize {
        self.reader.row_group_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One group's indices.
    pub fn read(&self, group: usize) -> Result<Vec<Option<u32>>, Error> {
        let indices = self.reader.read_nullable_column_group::<u32>("index", group)?;
        for index in indices.iter().flatten() {
            let index = u64::from(*index);
            if index >= self.category_count {
                return Err(InvalidData::IndexOutOfRange {
                    value: index,
                    maximum: self.category_count.saturating_add(1),
                }
                .into());
            }
        }
        Ok(indices)
    }
}
