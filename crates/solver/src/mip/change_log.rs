//! A pool's record of its changes for the domains propagating its rows.
//!
//! HiGHS's pools call every registered domain back as a cut or conflict is
//! added or deleted. Here a pool appends an event instead, and each domain
//! replays the events it has not seen before it next reads or changes its
//! propagation state, which leaves it exactly as the callbacks would: no
//! domain acts between a pool's change and its replay. An added row's event
//! carries a copy of the row, so a replay sees the row the callback saw even
//! when the pool has since deleted it and reused its index.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

type Registry = Rc<RefCell<Vec<Weak<Cell<usize>>>>>;

#[derive(Debug)]
pub(crate) struct ChangeLog<E> {
    events: Vec<E>,
    /// How many events have been dropped from the front.
    base: usize,
    registry: Registry,
}

impl<E> Default for ChangeLog<E> {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            base: 0,
            registry: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl<E: Clone> Clone for ChangeLog<E> {
    /// A copied pool has no listeners: domains register with one pool.
    fn clone(&self) -> Self {
        Self {
            events: Vec::new(),
            base: self.base + self.events.len(),
            registry: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl<E> ChangeLog<E> {
    fn end(&self) -> usize {
        self.base + self.events.len()
    }

    /// Record an event, unless nobody listens.
    pub fn push(&mut self, event: E) {
        let mut registry = self.registry.borrow_mut();
        registry.retain(|cursor| cursor.strong_count() > 0);
        if registry.is_empty() {
            drop(registry);
            self.base += self.events.len() + 1;
            self.events.clear();
            return;
        }
        drop(registry);
        self.events.push(event);
    }

    /// A listener that will see every event from now on.
    pub fn listen(&self) -> Listener {
        let cursor = Rc::new(Cell::new(self.end()));
        self.registry.borrow_mut().push(Rc::downgrade(&cursor));
        Listener {
            cursor,
            registry: Rc::clone(&self.registry),
        }
    }

    /// The events `listener` has not seen, marking them seen. Events every
    /// listener has seen are dropped first.
    pub fn take_unseen(&mut self, listener: &Listener) -> std::ops::Range<usize> {
        self.trim();
        let from = listener.cursor.get();
        let to = self.end();
        listener.cursor.set(to);
        from - self.base..to - self.base
    }

    pub fn event(&self, index: usize) -> &E {
        &self.events[index]
    }

    fn trim(&mut self) {
        let min = {
            let mut registry = self.registry.borrow_mut();
            registry.retain(|cursor| cursor.strong_count() > 0);
            registry.iter().filter_map(Weak::upgrade).map(|cursor| cursor.get()).min()
        };
        let min = min.unwrap_or(self.end());
        if min > self.base && min - self.base >= 64.max(self.events.len() / 2) {
            self.events.drain(..min - self.base);
            self.base = min;
        }
    }
}

/// A domain's place in a pool's [`ChangeLog`].
#[derive(Debug)]
pub(crate) struct Listener {
    cursor: Rc<Cell<usize>>,
    registry: Registry,
}

impl Clone for Listener {
    /// A copy of a domain listens from where the original stands.
    fn clone(&self) -> Self {
        let cursor = Rc::new(Cell::new(self.cursor.get()));
        self.registry.borrow_mut().push(Rc::downgrade(&cursor));
        Self {
            cursor,
            registry: Rc::clone(&self.registry),
        }
    }
}
