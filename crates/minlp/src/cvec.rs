//! `CVec`: a C array. A `Vec` that is also indexed by C's `int`, so ported
//! loops keep their `int` counters (and their -1 sentinels) and still read
//! `vars[v]` as the C does.

use std::ops::{Deref, DerefMut, Index, IndexMut, Range};

#[derive(Clone, Debug, PartialEq)]
pub struct CVec<T>(pub Vec<T>);

impl<T> Default for CVec<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> CVec<T> {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// C's `BMSreallocMemoryArray`: grows (or shrinks) to `size` slots,
    /// new slots defaulted.
    pub fn realloc(&mut self, size: i32)
    where
        T: Default + Clone,
    {
        self.0.resize(size.max(0) as usize, T::default());
    }

    /// A C array of `size` default slots.
    pub fn with_size(size: i32) -> Self
    where
        T: Default + Clone,
    {
        Self(vec![T::default(); size.max(0) as usize])
    }

    /// The first `n` slots (C's `array` with its count `n`).
    pub fn head(&self, n: i32) -> &[T] {
        &self.0[..n.max(0) as usize]
    }

    pub fn head_mut(&mut self, n: i32) -> &mut [T] {
        &mut self.0[..n.max(0) as usize]
    }
}

impl<T> From<Vec<T>> for CVec<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}

impl<T> Deref for CVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T> DerefMut for CVec<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

impl<T> Index<i32> for CVec<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, i: i32) -> &T {
        &self.0[i as usize]
    }
}

impl<T> IndexMut<i32> for CVec<T> {
    #[inline(always)]
    fn index_mut(&mut self, i: i32) -> &mut T {
        &mut self.0[i as usize]
    }
}

impl<T> Index<usize> for CVec<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, i: usize) -> &T {
        &self.0[i]
    }
}

impl<T> IndexMut<usize> for CVec<T> {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut T {
        &mut self.0[i]
    }
}

impl<T> Index<Range<i32>> for CVec<T> {
    type Output = [T];
    fn index(&self, r: Range<i32>) -> &[T] {
        &self.0[r.start as usize..r.end as usize]
    }
}

impl<T> IndexMut<Range<i32>> for CVec<T> {
    fn index_mut(&mut self, r: Range<i32>) -> &mut [T] {
        &mut self.0[r.start as usize..r.end as usize]
    }
}

/// A C array literal or a C array of `n` copies.
#[macro_export]
macro_rules! cvec {
    ($($x:tt)*) => { $crate::cvec::CVec(vec![$($x)*]) };
}
