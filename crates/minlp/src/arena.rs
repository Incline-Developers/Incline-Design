//! Storage for objects SCIP refers to by pointer: a slot per id, empty
//! once the object is freed.

use std::ops::{Index, IndexMut};

#[derive(Clone, Debug)]
pub struct Arena<T> {
    slots: Vec<Option<T>>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> Arena<T> {
    /// Stores `value` at a fresh id past every id used so far.
    pub fn push(&mut self, value: T) -> usize {
        self.slots.push(Some(value));
        self.slots.len() - 1
    }

    /// Stores `value` at `id`, SCIP's own number for it.
    pub fn insert_at(&mut self, id: usize, value: T) {
        if self.slots.len() <= id {
            self.slots.resize_with(id + 1, || None);
        }
        debug_assert!(self.slots[id].is_none(), "id {id} is in use");
        self.slots[id] = Some(value);
    }

    /// Frees the object at `id`.
    pub fn remove(&mut self, id: usize) -> T {
        self.slots[id].take().expect("freeing a freed object")
    }

    pub fn get(&self, id: usize) -> Option<&T> {
        self.slots.get(id).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut T> {
        self.slots.get_mut(id).and_then(Option::as_mut)
    }

    pub fn contains(&self, id: usize) -> bool {
        self.get(id).is_some()
    }

    /// One past the largest id ever used.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
}

impl<T> Index<usize> for Arena<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, id: usize) -> &T {
        self.slots[id].as_ref().expect("using a freed object")
    }
}

impl<T> IndexMut<usize> for Arena<T> {
    #[inline(always)]
    fn index_mut(&mut self, id: usize) -> &mut T {
        self.slots[id].as_mut().expect("using a freed object")
    }
}
