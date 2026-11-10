//! HiGHS's `highs::RbTree` and `CacheMinRbTree`: an intrusive red-black
//! tree whose links live in the elements it orders, addressed by index.

/// No element (HiGHS's `noLink`, -1).
pub(crate) const NO_LINK: usize = usize::MAX;

const LEFT: usize = 0;
const RIGHT: usize = 1;

/// HiGHS's `RbTreeLinks`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RbLinks {
    pub child: [usize; 2],
    parent: usize,
    red: bool,
}

impl Default for RbLinks {
    fn default() -> Self {
        Self {
            child: [NO_LINK; 2],
            parent: NO_LINK,
            red: false,
        }
    }
}

/// A collection whose elements are kept in a red-black tree. With
/// [`RbTree::cached_first`], it is HiGHS's `CacheMinRbTree`.
pub(crate) trait RbTree {
    type Key: PartialOrd;

    fn root(&self) -> usize;
    fn set_root(&mut self, root: usize);
    fn links(&self, node: usize) -> &RbLinks;
    fn links_mut(&mut self, node: usize) -> &mut RbLinks;
    fn key(&self, node: usize) -> Self::Key;

    /// The cached first element, for a `CacheMinRbTree`.
    fn cached_first(&mut self) -> Option<&mut usize> {
        None
    }

    fn is_red(&self, node: usize) -> bool {
        node != NO_LINK && self.links(node).red
    }

    fn is_black(&self, node: usize) -> bool {
        node == NO_LINK || !self.links(node).red
    }

    fn parent(&self, node: usize) -> usize {
        self.links(node).parent
    }

    fn get_child(&self, node: usize, dir: usize) -> usize {
        self.links(node).child[dir]
    }

    fn set_child(&mut self, node: usize, dir: usize, child: usize) {
        self.links_mut(node).child[dir] = child;
    }

    fn set_parent(&mut self, node: usize, parent: usize) {
        self.links_mut(node).parent = parent;
    }

    fn rotate(&mut self, x: usize, dir: usize) {
        let y = self.get_child(x, 1 - dir);
        let y_dir = self.get_child(y, dir);
        self.set_child(x, 1 - dir, y_dir);
        if y_dir != NO_LINK {
            self.set_parent(y_dir, x);
        }
        let p_x = self.parent(x);
        self.set_parent(y, p_x);
        if p_x == NO_LINK {
            self.set_root(y);
        } else {
            let side = usize::from(x != self.get_child(p_x, dir)) ^ dir;
            self.set_child(p_x, side, y);
        }
        self.set_child(y, dir, x);
        self.set_parent(x, y);
    }

    fn insert_fixup(&mut self, mut z: usize) {
        let mut p_z = self.parent(z);
        while self.is_red(p_z) {
            let mut z_grand_parent = self.parent(p_z);
            let dir = usize::from(self.get_child(z_grand_parent, LEFT) == p_z);
            let y = self.get_child(z_grand_parent, dir);
            if self.is_red(y) {
                self.links_mut(p_z).red = false;
                self.links_mut(y).red = false;
                self.links_mut(z_grand_parent).red = true;
                z = z_grand_parent;
            } else {
                if z == self.get_child(p_z, dir) {
                    z = p_z;
                    self.rotate(z, 1 - dir);
                    p_z = self.parent(z);
                    z_grand_parent = self.parent(p_z);
                }
                self.links_mut(p_z).red = false;
                self.links_mut(z_grand_parent).red = true;
                self.rotate(z_grand_parent, dir);
            }
            p_z = self.parent(z);
        }
        let root = self.root();
        self.links_mut(root).red = false;
    }

    fn transplant(&mut self, u: usize, v: usize, nil_parent: &mut usize) {
        let p = self.parent(u);
        if p == NO_LINK {
            self.set_root(v);
        } else {
            let side = usize::from(u != self.get_child(p, LEFT));
            self.set_child(p, side, v);
        }
        if v == NO_LINK {
            *nil_parent = p;
        } else {
            self.set_parent(v, p);
        }
    }

    fn delete_fixup(&mut self, mut x: usize, nil_parent: usize) {
        while x != self.root() && self.is_black(x) {
            let p = if x == NO_LINK { nil_parent } else { self.parent(x) };
            let dir = usize::from(x == self.get_child(p, LEFT));
            let mut w = self.get_child(p, dir);
            if self.is_red(w) {
                self.links_mut(w).red = false;
                self.links_mut(p).red = true;
                self.rotate(p, 1 - dir);
                w = self.get_child(p, dir);
            }
            if self.is_black(self.get_child(w, LEFT)) && self.is_black(self.get_child(w, RIGHT)) {
                self.links_mut(w).red = true;
                x = p;
            } else {
                if self.is_black(self.get_child(w, dir)) {
                    let other = self.get_child(w, 1 - dir);
                    self.links_mut(other).red = false;
                    self.links_mut(w).red = true;
                    self.rotate(w, dir);
                    w = self.get_child(p, dir);
                }
                let color = self.links(p).red;
                self.links_mut(w).red = color;
                self.links_mut(p).red = false;
                let child = self.get_child(w, dir);
                self.links_mut(child).red = false;
                self.rotate(p, 1 - dir);
                x = self.root();
            }
        }
        if x != NO_LINK {
            self.links_mut(x).red = false;
        }
    }

    fn is_empty(&self) -> bool {
        self.root() == NO_LINK
    }

    fn first_of(&self, mut x: usize) -> usize {
        if x == NO_LINK {
            return NO_LINK;
        }
        loop {
            let l_x = self.get_child(x, LEFT);
            if l_x == NO_LINK {
                return x;
            }
            x = l_x;
        }
    }

    fn last_of(&self, mut x: usize) -> usize {
        if x == NO_LINK {
            return NO_LINK;
        }
        loop {
            let r_x = self.get_child(x, RIGHT);
            if r_x == NO_LINK {
                return x;
            }
            x = r_x;
        }
    }

    /// The first element (not the cached one; see [`RbTree::cached_first`]).
    fn first(&self) -> usize {
        self.first_of(self.root())
    }

    fn last(&self) -> usize {
        self.last_of(self.root())
    }

    fn successor(&self, mut x: usize) -> usize {
        let mut y = self.get_child(x, RIGHT);
        if y != NO_LINK {
            return self.first_of(y);
        }
        y = self.parent(x);
        while y != NO_LINK && x == self.get_child(y, RIGHT) {
            x = y;
            y = self.parent(x);
        }
        y
    }

    fn predecessor(&self, mut x: usize) -> usize {
        let mut y = self.get_child(x, LEFT);
        if y != NO_LINK {
            return self.last_of(y);
        }
        y = self.parent(x);
        while y != NO_LINK && x == self.get_child(y, LEFT) {
            x = y;
            y = self.parent(x);
        }
        y
    }

    /// HiGHS's `find`: the element with `key` (`true`), or the last one
    /// the search passed (`false`).
    fn find_in(&self, key: &Self::Key, tree_root: usize) -> (usize, bool) {
        let mut y = NO_LINK;
        let mut x = tree_root;
        while x != NO_LINK {
            let node_key = self.key(x);
            let cmp = 1 - i32::from(node_key < *key) + i32::from(*key < node_key);
            match cmp {
                0 => {
                    y = x;
                    x = self.get_child(y, RIGHT);
                }
                1 => return (x, true),
                _ => {
                    y = x;
                    x = self.get_child(y, LEFT);
                }
            }
        }
        (y, false)
    }

    fn find(&self, key: &Self::Key) -> (usize, bool) {
        self.find_in(key, self.root())
    }

    /// HiGHS's `link(z, parent)`, keeping the cached first element.
    fn link_at(&mut self, z: usize, parent: usize) {
        let key_less = parent == NO_LINK || self.key(z) < self.key(parent);
        if let Some(first) = self.cached_first()
            && *first == parent
            && key_less
        {
            *first = z;
        }
        self.set_parent(z, parent);
        if parent == NO_LINK {
            self.set_root(z);
        } else {
            let side = usize::from(self.key(parent) < self.key(z));
            self.set_child(parent, side, z);
        }
        self.set_child(z, LEFT, NO_LINK);
        self.set_child(z, RIGHT, NO_LINK);
        self.links_mut(z).red = true;
        self.insert_fixup(z);
    }

    fn link(&mut self, z: usize) {
        let mut y = NO_LINK;
        let mut x = self.root();
        while x != NO_LINK {
            y = x;
            let side = usize::from(self.key(x) < self.key(z));
            x = self.get_child(y, side);
        }
        self.link_at(z, y);
    }

    fn unlink(&mut self, z: usize) {
        if self.cached_first().is_some_and(|first| *first == z) {
            let successor = self.successor(z);
            if let Some(first) = self.cached_first() {
                *first = successor;
            }
        }
        let mut nil_parent = NO_LINK;
        let mut y = z;
        let mut y_was_black = self.is_black(y);
        let x;
        if self.get_child(z, LEFT) == NO_LINK {
            x = self.get_child(z, RIGHT);
            self.transplant(z, x, &mut nil_parent);
        } else if self.get_child(z, RIGHT) == NO_LINK {
            x = self.get_child(z, LEFT);
            self.transplant(z, x, &mut nil_parent);
        } else {
            y = self.first_of(self.get_child(z, RIGHT));
            y_was_black = self.is_black(y);
            x = self.get_child(y, RIGHT);
            if self.parent(y) == z {
                if x == NO_LINK {
                    nil_parent = y;
                } else {
                    self.set_parent(x, y);
                }
            } else {
                let y_right = self.get_child(y, RIGHT);
                self.transplant(y, y_right, &mut nil_parent);
                let z_right = self.get_child(z, RIGHT);
                self.set_child(y, RIGHT, z_right);
                self.set_parent(z_right, y);
            }
            self.transplant(z, y, &mut nil_parent);
            let z_left = self.get_child(z, LEFT);
            self.set_child(y, LEFT, z_left);
            self.set_parent(z_left, y);
            let color = self.links(z).red;
            self.links_mut(y).red = color;
        }
        if y_was_black {
            self.delete_fixup(x, nil_parent);
        }
    }
}
