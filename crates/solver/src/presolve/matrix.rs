//! HiGHS's presolve storage of the constraint matrix: one triplet per
//! nonzero, linked into a doubly linked list per column and a splay tree
//! per row keyed by column (`util/HighsSplay.h`), so entries can come and
//! go as reductions remove and fill them.

use super::NONE;

/// Where the splay walk keeps a link it will write later: one of its
/// two working trees, or a child slot of a node.
#[derive(Clone, Copy)]
enum Slot {
    LeftTree,
    RightTree,
    Left(usize),
    Right(usize),
}

/// The row trees' child links and keys.
pub(crate) struct Trees<'a> {
    pub left: &'a mut [usize],
    pub right: &'a mut [usize],
    pub key: &'a [usize],
}

impl Trees<'_> {
    fn set(&mut self, slot: Slot, value: usize, left_tree: &mut usize, right_tree: &mut usize) {
        match slot {
            Slot::LeftTree => *left_tree = value,
            Slot::RightTree => *right_tree = value,
            Slot::Left(node) => self.left[node] = value,
            Slot::Right(node) => self.right[node] = value,
        }
    }

    /// HiGHS's `highs_splay`: bring the node with `key` (or the last one
    /// on the way to it) to the root of the tree at `root`.
    pub fn splay(&mut self, key: usize, mut root: usize) -> usize {
        if root == NONE {
            return NONE;
        }
        // HiGHS's Nleft and Nright: the trees of nodes greater and less
        // than the key, built from their leftmost and rightmost slots.
        let mut n_left = NONE;
        let mut n_right = NONE;
        let mut l_right = Slot::RightTree;
        let mut r_left = Slot::LeftTree;
        loop {
            if key < self.key[root] {
                let left = self.left[root];
                if left == NONE {
                    break;
                }
                if key < self.key[left] {
                    let y = left;
                    self.left[root] = self.right[y];
                    self.right[y] = root;
                    root = y;
                    if self.left[root] == NONE {
                        break;
                    }
                }
                self.set(r_left, root, &mut n_left, &mut n_right);
                r_left = Slot::Left(root);
                root = self.left[root];
            } else if key > self.key[root] {
                let right = self.right[root];
                if right == NONE {
                    break;
                }
                if key > self.key[right] {
                    let y = right;
                    self.right[root] = self.left[y];
                    self.left[y] = root;
                    root = y;
                    if self.right[root] == NONE {
                        break;
                    }
                }
                self.set(l_right, root, &mut n_left, &mut n_right);
                l_right = Slot::Right(root);
                root = self.right[root];
            } else {
                break;
            }
        }
        let root_left = self.left[root];
        self.set(l_right, root_left, &mut n_left, &mut n_right);
        let root_right = self.right[root];
        self.set(r_left, root_right, &mut n_left, &mut n_right);
        self.left[root] = n_right;
        self.right[root] = n_left;
        root
    }

    /// HiGHS's `highs_splay_link`: add `node` to the tree at `root`,
    /// returning the new root.
    pub fn link(&mut self, node: usize, root: usize) -> usize {
        if root == NONE {
            self.left[node] = NONE;
            self.right[node] = NONE;
            return node;
        }
        let root = self.splay(self.key[node], root);
        if self.key[node] < self.key[root] {
            self.left[node] = self.left[root];
            self.right[node] = root;
            self.left[root] = NONE;
        } else {
            self.right[node] = self.right[root];
            self.left[node] = root;
            self.right[root] = NONE;
        }
        node
    }

    /// HiGHS's `highs_splay_unlink`: take `node` out of the tree at
    /// `root`, returning the new root.
    pub fn unlink(&mut self, node: usize, root: usize) -> usize {
        let root = self.splay(self.key[node], root);
        if root != node {
            let right = self.right[root];
            self.right[root] = self.unlink(node, right);
            return root;
        }
        if self.left[node] == NONE {
            self.right[node]
        } else {
            let root = self.splay(self.key[node], self.left[node]);
            self.right[root] = self.right[node];
            root
        }
    }
}

/// A row's positions in pre-order (HiGHS's `getRowVector`).
pub(crate) fn pre_order(root: usize, left: &[usize], right: &[usize], out: &mut Vec<usize>) {
    out.clear();
    if root == NONE {
        return;
    }
    let mut stack: Vec<usize> = Vec::with_capacity(16);
    stack.push(NONE);
    let mut current = root;
    while current != NONE {
        out.push(current);
        if left[current] != NONE {
            if right[current] != NONE {
                stack.push(right[current]);
            }
            current = left[current];
        } else if right[current] != NONE {
            current = right[current];
        } else {
            current = stack.pop().unwrap();
        }
    }
}

/// A row's positions in column order (HiGHS's `getSortedRowVector`).
pub(crate) fn in_order(root: usize, left: &[usize], right: &[usize], out: &mut Vec<usize>) {
    out.clear();
    let mut stack: Vec<usize> = Vec::with_capacity(16);
    stack.push(NONE);
    let mut current = root;
    if current == NONE {
        return;
    }
    while left[current] != NONE {
        stack.push(current);
        current = left[current];
    }
    while current != NONE {
        out.push(current);
        if right[current] != NONE {
            current = right[current];
            while left[current] != NONE {
                stack.push(current);
                current = left[current];
            }
        } else {
            current = stack.pop().unwrap();
        }
    }
}
