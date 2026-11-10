//! SCIP's priority queue and queue (`misc.c`).

use std::collections::VecDeque;

#[inline(always)]
fn pq_parent(q: isize) -> isize {
    (q + 1) / 2 - 1
}

#[inline(always)]
fn pq_leftchild(p: isize) -> isize {
    2 * p + 1
}

#[inline(always)]
fn pq_rightchild(p: isize) -> isize {
    2 * p + 2
}

/// `SCIP_PQUEUE`: a binary heap, smallest by the comparator first. The
/// comparator and the position-change callback (`SCIP_DECL_PQUEUEELEMCHGPOS`,
/// told an element's old and new slot, -1 for none) are passed to each call.
#[derive(Clone, Debug, Default)]
pub struct PQueue<E> {
    slots: Vec<E>,
}

impl<E: Copy> PQueue<E> {
    /// `SCIPpqueueCreate`.
    pub fn new(initsize: usize) -> Self {
        Self {
            slots: Vec::with_capacity(initsize.max(1)),
        }
    }

    /// `SCIPpqueueClear`.
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    /// `pqueueElemChgPos`.
    #[inline(always)]
    fn elem_chg_pos(&mut self, elem: E, oldpos: isize, newpos: isize, chgpos: &mut impl FnMut(E, isize, isize)) {
        self.slots[newpos as usize] = elem;
        chgpos(elem, oldpos, newpos);
    }

    /// `SCIPpqueueInsert`.
    pub fn insert(&mut self, elem: E, cmp: impl Fn(&E, &E) -> i32, mut chgpos: impl FnMut(E, isize, isize)) {
        let mut pos = self.slots.len() as isize;
        self.slots.push(elem);
        let mut parentpos = pq_parent(pos);
        while pos > 0 && cmp(&elem, &self.slots[parentpos as usize]) < 0 {
            let parent = self.slots[parentpos as usize];
            self.elem_chg_pos(parent, parentpos, pos, &mut chgpos);
            pos = parentpos;
            parentpos = pq_parent(pos);
        }
        self.elem_chg_pos(elem, -1, pos, &mut chgpos);
    }

    /// `SCIPpqueueDelPos`.
    pub fn del_pos(&mut self, pos: usize, cmp: impl Fn(&E, &E) -> i32, mut chgpos: impl FnMut(E, isize, isize)) {
        let mut pos = pos as isize;
        let len = self.slots.len() as isize - 1;
        if pos == len {
            self.slots.pop();
            return;
        }
        let last = self.slots[len as usize];
        while pos > 0 && cmp(&last, &self.slots[pq_parent(pos) as usize]) < 0 {
            let parent = self.slots[pq_parent(pos) as usize];
            self.elem_chg_pos(parent, pq_parent(pos), pos, &mut chgpos);
            pos = pq_parent(pos);
        }
        while pos <= pq_parent(len - 1) {
            let mut childpos = pq_leftchild(pos);
            let brotherpos = pq_rightchild(pos);
            if brotherpos < len && cmp(&self.slots[brotherpos as usize], &self.slots[childpos as usize]) < 0 {
                childpos = brotherpos;
            }
            if cmp(&last, &self.slots[childpos as usize]) <= 0 {
                break;
            }
            let child = self.slots[childpos as usize];
            self.elem_chg_pos(child, childpos, pos, &mut chgpos);
            pos = childpos;
        }
        self.elem_chg_pos(last, len, pos, &mut chgpos);
        self.slots.pop();
    }

    /// `SCIPpqueueRemove`.
    pub fn remove(&mut self, cmp: impl Fn(&E, &E) -> i32, chgpos: impl FnMut(E, isize, isize)) -> Option<E> {
        let root = *self.slots.first()?;
        self.del_pos(0, cmp, chgpos);
        Some(root)
    }

    /// `SCIPpqueueFirst`.
    pub fn first(&self) -> Option<E> {
        self.slots.first().copied()
    }

    /// `SCIPpqueueNElems`.
    pub fn n_elems(&self) -> usize {
        self.slots.len()
    }

    /// `SCIPpqueueElems`.
    pub fn elems(&self) -> &[E] {
        &self.slots
    }
}

impl<E: Copy + PartialEq> PQueue<E> {
    /// `SCIPpqueueFind`.
    pub fn find(&self, elem: E) -> Option<usize> {
        self.slots.iter().position(|&e| e == elem)
    }
}

/// `SCIP_QUEUE`: first in, first out. C's ring buffer only changes where
/// elements are kept, not their order, so this is a `VecDeque`.
pub type Queue<E> = VecDeque<E>;
