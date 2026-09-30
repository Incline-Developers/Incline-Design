use std::{collections::TryReserveError, sync::Arc};

use bytes::Bytes;
use parquet::{
    basic::Repetition,
    column::writer::{ColumnCloseResult, get_column_writer, get_typed_column_writer},
    data_type::DataType,
    errors::ParquetError,
    file::{
        properties::WriterPropertiesPtr,
        writer::{SerializedPageWriter, TrackedWrite},
    },
    schema::types::{ColumnDescPtr, Type},
};

use super::array_type::PqArrayType;

/// One column chunk, encoded and compressed into its own buffer, ready to be spliced into a
/// row group with `append_column`.
pub struct EncodedColumn {
    pub data: Bytes,
    pub close: ColumnCloseResult,
}

/// Encodes one column chunk. Owns its values, so the chunks of a row group - and of several row
/// groups - can be compressed on different threads.
pub type ColumnJob = Box<
    dyn FnOnce(ColumnDescPtr, WriterPropertiesPtr) -> Result<EncodedColumn, ParquetError> + Send,
>;

fn encode_column<T: DataType>(
    descr: ColumnDescPtr,
    props: WriterPropertiesPtr,
    values: &[T::T],
    def_levels: Option<&[i16]>,
) -> Result<EncodedColumn, ParquetError> {
    let mut sink = TrackedWrite::new(Vec::new());
    let mut column = get_typed_column_writer::<T>(get_column_writer(
        descr,
        props,
        Box::new(SerializedPageWriter::new(&mut sink)),
    ));
    column.write_batch(values, def_levels, None)?;
    let close = column.close()?;
    Ok(EncodedColumn {
        data: sink.into_inner()?.into(),
        close,
    })
}

fn column_job<T: DataType>(values: Vec<T::T>, def_levels: Option<Arc<Vec<i16>>>) -> ColumnJob
where
    T::T: 'static,
{
    Box::new(move |descr, props| {
        encode_column::<T>(
            descr,
            props,
            &values,
            def_levels.as_deref().map(Vec::as_slice),
        )
    })
}

pub trait Source {
    /// Parquet schema types that this source writes.
    fn types(&self) -> Vec<Type>;
    /// Read up to `size` items, returning how many were read and one encode job per leaf
    /// column, in schema order.
    fn take(&mut self, size: usize) -> Result<(usize, Vec<ColumnJob>), ParquetError>;
}

fn single_type<P: PqArrayType>(name: &str, nullable: bool) -> Type {
    Type::primitive_type_builder(name, P::physical_type())
        .with_repetition(if nullable {
            Repetition::OPTIONAL
        } else {
            Repetition::REQUIRED
        })
        .with_logical_type(P::logical_type())
        .build()
        .expect("valid type")
}

fn row_group_vec<T>(n: usize) -> Result<Vec<T>, TryReserveError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)?;
    Ok(v)
}

fn reserve_error(error: TryReserveError) -> ParquetError {
    ParquetError::General(format!("failed to reserve row group buffer: {error}"))
}

/// Capacity to reserve for buffers, from the iterator size hint.
///
/// Reserving a whole row group up front costs several megabytes per column, which is wasted
/// when the array is small. Most sources are backed by slices and report an exact size, so
/// this reserves only what is needed while still capping at the row group size.
fn buffer_capacity(size_hint: (usize, Option<usize>), row_group_size: usize) -> usize {
    size_hint.1.unwrap_or(row_group_size).min(row_group_size)
}

pub trait PqArrayRow: 'static {
    type Buffer: Sized;
    const WIDTH: usize;

    fn types(names: &[&str]) -> Vec<Type>;
    fn make_buffer(capacity: usize) -> Result<Self::Buffer, TryReserveError>;
    fn add_to_buffer(self, buffer: &mut Self::Buffer);
    /// Split the buffer into one encode job per column.
    ///
    /// `def_levels` is `None` for required columns, where Parquet doesn't need definition
    /// levels at all.
    fn column_jobs(buffer: Self::Buffer, def_levels: Option<Arc<Vec<i16>>>) -> Vec<ColumnJob>;
}

pub struct RowSource<R: PqArrayRow, I: Iterator<Item = R>> {
    row_types: Vec<Type>,
    iter: I,
    capacity: usize,
    _row: std::marker::PhantomData<R>,
}

impl<R: PqArrayRow, I: Iterator<Item = R>> RowSource<R, I> {
    pub fn new(names: &[&str], iter: I, row_group_size: usize) -> Result<Self, TryReserveError> {
        let capacity = buffer_capacity(iter.size_hint(), row_group_size);
        Ok(Self {
            row_types: R::types(names),
            iter,
            capacity,
            _row: std::marker::PhantomData,
        })
    }
}

impl<R: PqArrayRow, I: Iterator<Item = R>> Source for RowSource<R, I> {
    fn types(&self) -> Vec<Type> {
        self.row_types.clone()
    }

    fn take(&mut self, size: usize) -> Result<(usize, Vec<ColumnJob>), ParquetError> {
        let mut buffer = R::make_buffer(self.capacity.min(size)).map_err(reserve_error)?;
        let mut count = 0;
        for row in self.iter.by_ref().take(size) {
            row.add_to_buffer(&mut buffer);
            count += 1;
        }
        // These columns are all required, so Parquet doesn't need definition levels.
        Ok((count, R::column_jobs(buffer, None)))
    }
}

pub struct NullableRowSource<R: PqArrayRow, I: Iterator<Item = Option<R>>> {
    ty: Type,
    iter: I,
    capacity: usize,
    _row: std::marker::PhantomData<R>,
}

impl<R: PqArrayRow, I: Iterator<Item = Option<R>>> NullableRowSource<R, I> {
    pub fn new(
        group_name: &str,
        names: &[&str],
        iter: I,
        row_group_size: usize,
    ) -> Result<Self, TryReserveError> {
        let capacity = buffer_capacity(iter.size_hint(), row_group_size);
        Ok(Self {
            ty: Type::group_type_builder(group_name)
                .with_repetition(Repetition::OPTIONAL)
                .with_fields(R::types(names).into_iter().map(Into::into).collect())
                .build()
                .expect("valid type"),
            iter,
            capacity,
            _row: std::marker::PhantomData,
        })
    }

    pub fn new_single(name: &str, iter: I, row_group_size: usize) -> Result<Self, TryReserveError> {
        assert_eq!(R::WIDTH, 1);
        let ty = R::types(&["tmp"]).into_iter().next().unwrap();
        let capacity = buffer_capacity(iter.size_hint(), row_group_size);
        Ok(Self {
            ty: Type::primitive_type_builder(name, ty.get_physical_type())
                .with_repetition(Repetition::OPTIONAL)
                .with_logical_type(ty.get_basic_info().logical_type_ref().cloned())
                .build()
                .expect("valid type"),
            iter,
            capacity,
            _row: std::marker::PhantomData,
        })
    }
}

impl<R: PqArrayRow, I: Iterator<Item = Option<R>>> Source for NullableRowSource<R, I> {
    fn types(&self) -> Vec<Type> {
        vec![self.ty.clone()]
    }

    fn take(&mut self, size: usize) -> Result<(usize, Vec<ColumnJob>), ParquetError> {
        let capacity = self.capacity.min(size);
        let mut buffer = R::make_buffer(capacity).map_err(reserve_error)?;
        let mut def_levels = row_group_vec(capacity).map_err(reserve_error)?;
        for opt_row in self.iter.by_ref().take(size) {
            if let Some(row) = opt_row {
                row.add_to_buffer(&mut buffer);
                def_levels.push(1);
            } else {
                def_levels.push(0);
            }
        }
        let count = def_levels.len();
        Ok((count, R::column_jobs(buffer, Some(Arc::new(def_levels)))))
    }
}

impl<P: PqArrayType, const N: usize> PqArrayRow for [P; N] {
    type Buffer = [Vec<<P::DataType as DataType>::T>; N];
    const WIDTH: usize = N;

    fn types(names: &[&str]) -> Vec<Type> {
        assert_eq!(names.len(), N);
        names.iter().map(|n| single_type::<P>(n, false)).collect()
    }

    fn make_buffer(capacity: usize) -> Result<Self::Buffer, TryReserveError> {
        let mut buffer = std::array::from_fn(|_| Vec::new());
        for b in &mut buffer {
            *b = row_group_vec(capacity)?;
        }
        Ok(buffer)
    }

    fn add_to_buffer(self, buffer: &mut Self::Buffer) {
        for (b, a) in buffer.iter_mut().zip(self) {
            b.push(a.to_parquet());
        }
    }

    fn column_jobs(buffer: Self::Buffer, def_levels: Option<Arc<Vec<i16>>>) -> Vec<ColumnJob> {
        buffer
            .into_iter()
            .map(|values| column_job::<P::DataType>(values, def_levels.clone()))
            .collect()
    }
}

macro_rules! row {
    ($width:literal, { $($i:tt $P:ident),* }) => {
        impl<$( $P: PqArrayType ),*> PqArrayRow for ($( $P, )*) {
            type Buffer = (
                $( Vec<<$P::DataType as DataType>::T>, )*
            );
            const WIDTH: usize = $width;

            fn types(names: &[&str]) -> Vec<Type> {
                assert_eq!(names.len(), $width);
                let mut names_iter = names.into_iter();
                vec![$(
                    single_type::<$P>(names_iter.next().unwrap(), false),
                )*]
            }

            fn make_buffer(capacity: usize) -> Result<Self::Buffer, TryReserveError> {
                Ok(($(
                    row_group_vec::<<$P::DataType as DataType>::T>(capacity)?,
                )*))
            }

            fn add_to_buffer(self, buffer: &mut Self::Buffer) {
                $( buffer.$i.push(self.$i.to_parquet()); )*
            }

            fn column_jobs(buffer: Self::Buffer, def_levels: Option<Arc<Vec<i16>>>) -> Vec<ColumnJob> {
                vec![$(
                    column_job::<$P::DataType>(buffer.$i, def_levels.clone()),
                )*]
            }
        }
    };
}

row!(1, { 0 P0 });
row!(2, { 0 P0, 1 P1 });
row!(3, { 0 P0, 1 P1, 2 P2 });
row!(4, { 0 P0, 1 P1, 2 P2, 3 P3 });
row!(5, { 0 P0, 1 P1, 2 P2, 3 P3, 4 P4 });
row!(6, { 0 P0, 1 P1, 2 P2, 3 P3, 4 P4, 5 P5 });
row!(7, { 0 P0, 1 P1, 2 P2, 3 P3, 4 P4, 5 P5, 6 P6 });
row!(8, { 0 P0, 1 P1, 2 P2, 3 P3, 4 P4, 5 P5, 6 P6, 7 P7 });
row!(9, { 0 P0, 1 P1, 2 P2, 3 P3, 4 P4, 5 P5, 6 P6, 7 P7, 8 P8 });
row!(10, { 0 P0, 1 P1, 2 P2, 3 P3, 4 P4, 5 P5, 6 P6, 7 P7, 8 P8, 9 P9 });
