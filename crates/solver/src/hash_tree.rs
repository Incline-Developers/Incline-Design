//! HiGHS's `HighsHashTree`: a hash trie of 64-way branch nodes over leaves
//! that keep up to 54 entries sorted by their next 16 hash bits, and lists
//! where all 54 hash bits collide. Its shape follows HiGHS's exactly, so a
//! walk over it visits entries in HiGHS's order.

use crate::hash_table::HashKey;

const BITS_PER_LEVEL: u32 = 6;
const MAX_DEPTH: usize = 9;
const MIN_LEAF_SIZE: usize = 6;
const LEAF_BURST_THRESHOLD: usize = 54;
const SIZE_STEP: usize = (LEAF_BURST_THRESHOLD - MIN_LEAF_SIZE) / 3;

fn get_hash_chunk(hash: u64, pos: usize) -> u8 {
    ((hash >> (64 - BITS_PER_LEVEL as usize - pos * BITS_PER_LEVEL as usize)) & 63) as u8
}

fn get_hash_chunks16(hash: u64, pos: usize) -> u16 {
    (hash >> (48 - pos * BITS_PER_LEVEL as usize)) as u16
}

fn get_first_chunk16(chunks: u16) -> u8 {
    (chunks >> (16 - BITS_PER_LEVEL)) as u8
}

fn num_set_until(occupation: u64, pos: u8) -> usize {
    (occupation >> pos).count_ones() as usize
}

fn num_set_after(occupation: u64, pos: u8) -> usize {
    (occupation << (63 - pos)).count_ones() as usize
}

fn test(occupation: u64, pos: u8) -> bool {
    occupation & (1u64 << pos) != 0
}

fn entries_to_size_class(num_entries: usize) -> usize {
    1 + (num_entries + (SIZE_STEP - MIN_LEAF_SIZE - 1)) / SIZE_STEP
}

fn leaf_capacity(size_class: usize) -> usize {
    MIN_LEAF_SIZE + (size_class - 1) * SIZE_STEP
}

/// An inner leaf (HiGHS's `InnerLeaf<kSizeClass>`): entries sorted by
/// decreasing 16-bit hash chunks, with a zero after the last.
#[derive(Clone, Debug)]
struct Leaf<K, V> {
    size_class: usize,
    occupation: u64,
    hashes: Vec<u16>,
    entries: Vec<(K, V)>,
}

impl<K: HashKey, V> Leaf<K, V> {
    fn new(size_class: usize) -> Self {
        let capacity = leaf_capacity(size_class);
        let mut hashes = Vec::with_capacity(capacity + 1);
        hashes.push(0);
        Self {
            size_class,
            occupation: 0,
            hashes,
            entries: Vec::with_capacity(capacity),
        }
    }

    fn size(&self) -> usize {
        self.entries.len()
    }

    fn capacity(&self) -> usize {
        leaf_capacity(self.size_class)
    }

    fn find_key(&self, key: &K, hash: u16, pos: &mut usize) -> bool {
        while *pos != self.size() && self.hashes[*pos] == hash {
            if *key == self.entries[*pos].0 {
                return true;
            }
            *pos += 1;
        }
        false
    }

    /// Returns where the entry is and whether it was added.
    fn insert_entry(&mut self, full_hash: u64, hash_pos: usize, entry: (K, V)) -> (usize, bool) {
        let hash = get_hash_chunks16(full_hash, hash_pos);
        let hash_chunk = get_first_chunk16(hash);
        let mut pos = num_set_until(self.occupation, hash_chunk);
        if test(self.occupation, hash_chunk) {
            pos -= 1;
            while self.hashes[pos] > hash {
                pos += 1;
            }
            if self.find_key(&entry.0, hash, &mut pos) {
                return (pos, false);
            }
        } else {
            self.occupation |= 1u64 << hash_chunk;
            if pos < self.size() {
                while self.hashes[pos] > hash {
                    pos += 1;
                }
            }
        }
        self.entries.insert(pos, entry);
        self.hashes.insert(pos, hash);
        (pos, true)
    }

    fn find_entry(&self, full_hash: u64, hash_pos: usize, key: &K) -> Option<usize> {
        let hash = get_hash_chunks16(full_hash, hash_pos);
        let hash_chunk = get_first_chunk16(hash);
        if !test(self.occupation, hash_chunk) {
            return None;
        }
        let mut pos = num_set_until(self.occupation, hash_chunk) - 1;
        while self.hashes[pos] > hash {
            pos += 1;
        }
        self.find_key(key, hash, &mut pos).then_some(pos)
    }

    fn erase_entry(&mut self, full_hash: u64, hash_pos: usize, key: &K) -> bool {
        let hash = get_hash_chunks16(full_hash, hash_pos);
        let hash_chunk = get_first_chunk16(hash);
        if !test(self.occupation, hash_chunk) {
            return false;
        }
        let mut start_pos = num_set_until(self.occupation, hash_chunk) - 1;
        while get_first_chunk16(self.hashes[start_pos]) > hash_chunk {
            start_pos += 1;
        }
        let mut pos = start_pos;
        while self.hashes[pos] > hash {
            pos += 1;
        }
        if !self.find_key(key, hash, &mut pos) {
            return false;
        }
        let new_size = self.size() - 1;
        self.entries.remove(pos);
        self.hashes.remove(pos);
        if pos < new_size {
            if get_first_chunk16(self.hashes[start_pos]) != hash_chunk {
                self.occupation ^= 1u64 << hash_chunk;
            }
        } else if start_pos == pos {
            self.occupation ^= 1u64 << hash_chunk;
        }
        true
    }

    fn rehash(&mut self, hash_pos: usize) {
        self.occupation = 0;
        let size = self.size();
        for i in 0..size {
            self.hashes[i] = get_hash_chunks16(self.entries[i].0.highs_hash(), hash_pos);
            self.occupation |= 1u64 << get_first_chunk16(self.hashes[i]);
        }
        let mut i = 0;
        while i < size {
            let hash_chunk = get_first_chunk16(self.hashes[i]);
            let mut pos = num_set_until(self.occupation, hash_chunk) - 1;
            if pos > i {
                self.hashes.swap(pos, i);
                self.entries.swap(pos, i);
                continue;
            }
            while pos < i && self.hashes[pos] >= self.hashes[i] {
                pos += 1;
            }
            if pos < i {
                let hash = self.hashes.remove(i);
                let entry = self.entries.remove(i);
                self.hashes.insert(pos, hash);
                self.entries.insert(pos, entry);
            }
            i += 1;
        }
    }
}

#[derive(Clone, Debug)]
enum Node<K, V> {
    Empty,
    /// Entries whose 54 hash bits collide, in HiGHS's list order.
    List(Vec<(K, V)>),
    Leaf(Leaf<K, V>),
    Branch {
        occupation: u64,
        children: Vec<Node<K, V>>,
    },
}

impl<K: HashKey, V> Node<K, V> {
    /// HiGHS's node type order, used by `find_common`.
    fn type_rank(&self) -> usize {
        match self {
            Node::Empty => 0,
            Node::List(_) => 1,
            Node::Leaf(leaf) => 1 + leaf.size_class,
            Node::Branch { .. } => 6,
        }
    }

    fn num_entries_estimate(&self) -> usize {
        match self {
            Node::Empty => 0,
            Node::List(_) => 1,
            Node::Leaf(leaf) => leaf.capacity(),
            Node::Branch { .. } => 64,
        }
    }

    fn num_entries(&self) -> usize {
        match self {
            Node::Empty => 0,
            Node::List(list) => list.len(),
            Node::Leaf(leaf) => leaf.size(),
            Node::Branch { .. } => 64,
        }
    }
}

/// HiGHS's `HighsHashTree<K, V>`.
#[derive(Clone, Debug)]
pub(crate) struct HashTree<K, V> {
    root: Node<K, V>,
}

impl<K: HashKey, V> Default for HashTree<K, V> {
    fn default() -> Self {
        Self { root: Node::Empty }
    }
}

impl<K: HashKey, V> HashTree<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.root, Node::Empty)
    }

    pub fn clear(&mut self) {
        self.root = Node::Empty;
    }

    /// Add `key` unless it is there; returns whether it was added.
    pub fn insert(&mut self, key: K, value: V) -> bool {
        let hash = key.highs_hash();
        insert_recurse(&mut self.root, hash, 0, (key, value)).1
    }

    /// `key`'s value, inserting `value` first if `key` was not there, and
    /// whether it was inserted.
    pub fn insert_or_get(&mut self, key: K, value: V) -> (&mut V, bool) {
        let hash = key.highs_hash();
        let (path, added) = insert_recurse(&mut self.root, hash, 0, (key, value));
        (value_at(&mut self.root, &path), added)
    }

    pub fn erase(&mut self, key: &K) {
        let hash = key.highs_hash();
        erase_recurse(&mut self.root, hash, 0, key);
    }

    pub fn contains(&self, key: &K) -> bool {
        self.find(key).is_some()
    }

    pub fn find(&self, key: &K) -> Option<&V> {
        find_recurse(&self.root, key.highs_hash(), 0, key).map(|(_, value)| value)
    }

    pub fn find_mut(&mut self, key: &K) -> Option<&mut V> {
        find_recurse_mut(&mut self.root, key.highs_hash(), 0, key)
    }

    /// An entry this tree and `other` share, as HiGHS finds it.
    pub fn find_common<'a>(&'a self, other: &'a HashTree<K, V>) -> Option<&'a (K, V)> {
        find_common_recurse(&self.root, &other.root, 0)
    }

    /// Visit every entry in HiGHS's order until `f` returns `Some`.
    pub fn for_each_until<R>(&self, mut f: impl FnMut(&K, &V) -> Option<R>) -> Option<R> {
        for_each_recurse(&self.root, &mut f)
    }

    /// Visit every entry in HiGHS's order.
    pub fn for_each(&self, mut f: impl FnMut(&K, &V)) {
        self.for_each_until(|key, value| {
            f(key, value);
            None::<()>
        });
    }

    /// Visit every entry in HiGHS's order, changing values.
    pub fn for_each_mut(&mut self, mut f: impl FnMut(&K, &mut V)) {
        for_each_recurse_mut(&mut self.root, &mut f);
    }
}

/// Where an inserted or found entry lives: child positions to its leaf,
/// then its position there.
type Path = Vec<usize>;

fn value_at<'a, K, V>(node: &'a mut Node<K, V>, path: &[usize]) -> &'a mut V {
    match node {
        Node::Branch { children, .. } => value_at(&mut children[path[0]], &path[1..]),
        Node::Leaf(leaf) => &mut leaf.entries[path[0]].1,
        Node::List(list) => &mut list[path[0]].1,
        Node::Empty => unreachable!("a path into an empty node"),
    }
}

fn insert_recurse<K: HashKey, V>(node: &mut Node<K, V>, hash: u64, hash_pos: usize, entry: (K, V)) -> (Path, bool) {
    match node {
        Node::Empty => {
            if hash_pos == MAX_DEPTH {
                *node = Node::List(vec![entry]);
                return (vec![0], true);
            }
            let mut leaf = Leaf::new(1);
            let (pos, added) = leaf.insert_entry(hash, hash_pos, entry);
            *node = Node::Leaf(leaf);
            (vec![pos], added)
        }
        Node::List(list) => {
            if let Some(pos) = list.iter().position(|(key, _)| *key == entry.0) {
                return (vec![pos], false);
            }
            list.push(entry);
            (vec![list.len() - 1], true)
        }
        Node::Leaf(leaf) if leaf.size_class < 4 => {
            if leaf.size() == leaf.capacity() {
                if let Some(pos) = leaf.find_entry(hash, hash_pos, &entry.0) {
                    return (vec![pos], false);
                }
                leaf.size_class += 1;
            }
            let (pos, added) = leaf.insert_entry(hash, hash_pos, entry);
            (vec![pos], added)
        }
        Node::Leaf(leaf) => {
            if leaf.size() < leaf.capacity() {
                let (pos, added) = leaf.insert_entry(hash, hash_pos, entry);
                return (vec![pos], added);
            }
            if let Some(pos) = leaf.find_entry(hash, hash_pos, &entry.0) {
                return (vec![pos], false);
            }
            // Burst the full leaf into a branch node.
            let Node::Leaf(leaf) = std::mem::replace(node, Node::Empty) else { unreachable!() };
            let hash_chunk = get_hash_chunk(hash, hash_pos);
            let occupation = leaf.occupation | (1u64 << hash_chunk);
            let branch_size = occupation.count_ones() as usize;
            let leaf_size = leaf.size();
            if hash_pos + 1 == MAX_DEPTH {
                let mut children: Vec<Node<K, V>> = (0..branch_size).map(|_| Node::Empty).collect();
                for (i, leaf_entry) in leaf.entries.into_iter().enumerate() {
                    let pos = num_set_until(occupation, get_first_chunk16(leaf.hashes[i])) - 1;
                    match &mut children[pos] {
                        Node::Empty => children[pos] = Node::List(vec![leaf_entry]),
                        // HiGHS puts each later entry first.
                        Node::List(list) => list.insert(0, leaf_entry),
                        _ => unreachable!(),
                    }
                }
                let pos = num_set_until(occupation, hash_chunk) - 1;
                match &mut children[pos] {
                    Node::Empty => children[pos] = Node::List(vec![entry]),
                    Node::List(list) => list.insert(0, entry),
                    _ => unreachable!(),
                }
                *node = Node::Branch { occupation, children };
                return (vec![pos, 0], true);
            }
            if branch_size > 1 {
                let max_entries_per_leaf = 2 + leaf_size - branch_size;
                let mut children: Vec<Node<K, V>>;
                if max_entries_per_leaf <= leaf_capacity(1) {
                    children = (0..branch_size).map(|_| Node::Leaf(Leaf::new(1))).collect();
                    for (i, leaf_entry) in leaf.entries.into_iter().enumerate() {
                        let pos = num_set_until(occupation, get_first_chunk16(leaf.hashes[i])) - 1;
                        let Node::Leaf(child) = &mut children[pos] else { unreachable!() };
                        let entry_hash = leaf_entry.0.highs_hash();
                        child.insert_entry(entry_hash, hash_pos + 1, leaf_entry);
                    }
                    let pos = num_set_until(occupation, hash_chunk) - 1;
                    let Node::Leaf(child) = &mut children[pos] else { unreachable!() };
                    let (at, added) = child.insert_entry(hash, hash_pos + 1, entry);
                    *node = Node::Branch { occupation, children };
                    return (vec![pos, at], added);
                }
                let mut sizes = vec![0usize; leaf_capacity(4) + 1];
                sizes[num_set_until(occupation, hash_chunk) - 1] += 1;
                for i in 0..leaf_size {
                    sizes[num_set_until(occupation, get_first_chunk16(leaf.hashes[i])) - 1] += 1;
                }
                children = (0..branch_size).map(|i| Node::Leaf(Leaf::new(entries_to_size_class(sizes[i])))).collect();
                for (i, leaf_entry) in leaf.entries.into_iter().enumerate() {
                    let pos = num_set_until(occupation, get_first_chunk16(leaf.hashes[i])) - 1;
                    let Node::Leaf(child) = &mut children[pos] else { unreachable!() };
                    let entry_hash = leaf_entry.0.highs_hash();
                    child.insert_entry(entry_hash, hash_pos + 1, leaf_entry);
                }
                let pos = num_set_until(occupation, hash_chunk) - 1;
                *node = Node::Branch { occupation, children };
                let Node::Branch { children, .. } = node else { unreachable!() };
                let (mut path, added) = insert_recurse(&mut children[pos], hash, hash_pos + 1, entry);
                path.insert(0, pos);
                return (path, added);
            }
            let mut leaf = leaf;
            leaf.rehash(hash_pos + 1);
            *node = Node::Branch {
                occupation,
                children: vec![Node::Leaf(leaf)],
            };
            let Node::Branch { children, .. } = node else { unreachable!() };
            let (mut path, added) = insert_recurse(&mut children[0], hash, hash_pos + 1, entry);
            path.insert(0, 0);
            (path, added)
        }
        Node::Branch { occupation, children } => {
            let chunk = get_hash_chunk(hash, hash_pos);
            let mut location = num_set_until(*occupation, chunk);
            if test(*occupation, chunk) {
                location -= 1;
            } else {
                debug_assert_eq!(num_set_after(*occupation, chunk) + location, occupation.count_ones() as usize);
                children.insert(location, Node::Empty);
                *occupation |= 1u64 << chunk;
            }
            let (mut path, added) = insert_recurse(&mut children[location], hash, hash_pos + 1, entry);
            path.insert(0, location);
            (path, added)
        }
    }
}

/// Merge `merge` into `leaf` (HiGHS's `mergeIntoLeaf`).
fn merge_into_leaf<K: HashKey, V>(leaf: &mut Leaf<K, V>, hash_pos: usize, merge: Node<K, V>) {
    match merge {
        Node::List(list) => {
            for entry in list {
                let hash = entry.0.highs_hash();
                leaf.insert_entry(hash, hash_pos, entry);
            }
        }
        Node::Leaf(other) => {
            for entry in other.entries {
                let hash = entry.0.highs_hash();
                leaf.insert_entry(hash, hash_pos, entry);
            }
        }
        Node::Empty | Node::Branch { .. } => {}
    }
}

/// HiGHS's `removeChildFromBranchNode`, after the child at `location` was
/// emptied and its occupation bit cleared.
fn remove_child_from_branch_node<K: HashKey, V>(node: &mut Node<K, V>, location: usize, hash_pos: usize) {
    let Node::Branch { occupation, children } = node else { unreachable!() };
    let new_num_child = occupation.count_ones() as usize;
    if new_num_child * leaf_capacity(1) <= LEAF_BURST_THRESHOLD {
        let mut child_entries = 0;
        for child in children.iter().take(new_num_child + 1) {
            child_entries += child.num_entries_estimate();
            if child_entries > LEAF_BURST_THRESHOLD {
                break;
            }
        }
        if child_entries < LEAF_BURST_THRESHOLD {
            child_entries = children.iter().take(new_num_child + 1).map(Node::num_entries).sum();
            if child_entries < LEAF_BURST_THRESHOLD {
                let mut leaf = Leaf::new(entries_to_size_class(child_entries));
                for child in std::mem::take(children) {
                    merge_into_leaf(&mut leaf, hash_pos, child);
                }
                *node = Node::Leaf(leaf);
                return;
            }
        }
    }
    children.remove(location);
}

fn erase_recurse<K: HashKey, V>(node: &mut Node<K, V>, hash: u64, hash_pos: usize, key: &K) {
    match node {
        Node::Empty => {}
        Node::List(list) => {
            if let Some(pos) = list.iter().position(|(stored, _)| stored == key) {
                list.remove(pos);
            }
            if list.is_empty() {
                *node = Node::Empty;
            }
        }
        Node::Leaf(leaf) => {
            if leaf.erase_entry(hash, hash_pos, key) {
                if leaf.size() == 0 {
                    *node = Node::Empty;
                } else if leaf.size_class > 1 && leaf.size() == leaf_capacity(leaf.size_class - 1) {
                    leaf.size_class -= 1;
                }
            }
        }
        Node::Branch { occupation, children } => {
            let chunk = get_hash_chunk(hash, hash_pos);
            if !test(*occupation, chunk) {
                return;
            }
            let location = num_set_until(*occupation, chunk) - 1;
            erase_recurse(&mut children[location], hash, hash_pos + 1, key);
            if !matches!(children[location], Node::Empty) {
                return;
            }
            *occupation ^= 1u64 << chunk;
            remove_child_from_branch_node(node, location, hash_pos);
        }
    }
}

fn find_recurse<'a, K: HashKey, V>(node: &'a Node<K, V>, hash: u64, hash_pos: usize, key: &K) -> Option<&'a (K, V)> {
    match node {
        Node::Empty => None,
        Node::List(list) => list.iter().find(|(stored, _)| stored == key),
        Node::Leaf(leaf) => leaf.find_entry(hash, hash_pos, key).map(|pos| &leaf.entries[pos]),
        Node::Branch { occupation, children } => {
            let chunk = get_hash_chunk(hash, hash_pos);
            if !test(*occupation, chunk) {
                return None;
            }
            find_recurse(&children[num_set_until(*occupation, chunk) - 1], hash, hash_pos + 1, key)
        }
    }
}

fn find_recurse_mut<'a, K: HashKey, V>(node: &'a mut Node<K, V>, hash: u64, hash_pos: usize, key: &K) -> Option<&'a mut V> {
    match node {
        Node::Empty => None,
        Node::List(list) => list.iter_mut().find(|(stored, _)| stored == key).map(|(_, value)| value),
        Node::Leaf(leaf) => leaf.find_entry(hash, hash_pos, key).map(|pos| &mut leaf.entries[pos].1),
        Node::Branch { occupation, children } => {
            let chunk = get_hash_chunk(hash, hash_pos);
            if !test(*occupation, chunk) {
                return None;
            }
            find_recurse_mut(&mut children[num_set_until(*occupation, chunk) - 1], hash, hash_pos + 1, key)
        }
    }
}

/// HiGHS's `findCommonInLeaf` for two leaves.
fn find_common_in_leaves<'a, K: HashKey, V>(leaf1: &'a Leaf<K, V>, leaf2: &Leaf<K, V>) -> Option<&'a (K, V)> {
    let mut match_mask = leaf1.occupation & leaf2.occupation;
    if match_mask == 0 {
        return None;
    }
    let mut offset1: isize = -1;
    let mut offset2: isize = -1;
    while match_mask != 0 {
        let pos = 63 - match_mask.leading_zeros() as u8;
        match_mask ^= 1u64 << pos;
        let mut i = (num_set_until(leaf1.occupation, pos) as isize + offset1) as usize;
        while get_first_chunk16(leaf1.hashes[i]) != pos {
            i += 1;
            offset1 += 1;
        }
        let mut j = (num_set_until(leaf2.occupation, pos) as isize + offset2) as usize;
        while get_first_chunk16(leaf2.hashes[j]) != pos {
            j += 1;
            offset2 += 1;
        }
        loop {
            if leaf1.hashes[i] > leaf2.hashes[j] {
                i += 1;
                if i == leaf1.size() || get_first_chunk16(leaf1.hashes[i]) != pos {
                    break;
                }
            } else if leaf2.hashes[j] > leaf1.hashes[i] {
                j += 1;
                if j == leaf2.size() || get_first_chunk16(leaf2.hashes[j]) != pos {
                    break;
                }
            } else {
                if leaf1.entries[i].0 == leaf2.entries[j].0 {
                    return Some(&leaf1.entries[i]);
                }
                i += 1;
                if i == leaf1.size() || get_first_chunk16(leaf1.hashes[i]) != pos {
                    break;
                }
                j += 1;
                if j == leaf2.size() || get_first_chunk16(leaf2.hashes[j]) != pos {
                    break;
                }
            }
        }
    }
    None
}

/// HiGHS's `findCommonInLeaf` for a leaf and any node at least as large.
fn find_common_in_leaf<'a, K: HashKey, V>(leaf: &'a Leaf<K, V>, n2: &Node<K, V>, hash_pos: usize) -> Option<&'a (K, V)> {
    match n2 {
        Node::Leaf(other) => find_common_in_leaves(leaf, other),
        Node::Branch { occupation, children } => {
            let mut match_mask = occupation & leaf.occupation;
            let mut offset: isize = -1;
            while match_mask != 0 {
                let pos = 63 - match_mask.leading_zeros() as u8;
                match_mask ^= 1u64 << pos;
                let mut i = (num_set_until(leaf.occupation, pos) as isize + offset) as usize;
                while get_first_chunk16(leaf.hashes[i]) != pos {
                    i += 1;
                    offset += 1;
                }
                let j = num_set_until(*occupation, pos) - 1;
                loop {
                    let key = &leaf.entries[i].0;
                    if find_recurse(&children[j], key.highs_hash(), hash_pos + 1, key).is_some() {
                        return Some(&leaf.entries[i]);
                    }
                    i += 1;
                    if !(i < leaf.size() && get_first_chunk16(leaf.hashes[i]) == pos) {
                        break;
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn find_common_recurse<'a, K: HashKey, V>(mut n1: &'a Node<K, V>, mut n2: &'a Node<K, V>, hash_pos: usize) -> Option<&'a (K, V)> {
    if n1.type_rank() > n2.type_rank() {
        std::mem::swap(&mut n1, &mut n2);
    }
    match n1 {
        Node::Empty => None,
        Node::List(list) => list.iter().find(|(key, _)| find_recurse(n2, key.highs_hash(), hash_pos, key).is_some()),
        Node::Leaf(leaf) => find_common_in_leaf(leaf, n2, hash_pos),
        Node::Branch {
            occupation: occupation1,
            children: children1,
        } => {
            let Node::Branch {
                occupation: occupation2,
                children: children2,
            } = n2
            else {
                unreachable!()
            };
            let mut match_mask = occupation1 & occupation2;
            while match_mask != 0 {
                let pos = 63 - match_mask.leading_zeros() as u8;
                match_mask ^= 1u64 << pos;
                let location1 = num_set_until(*occupation1, pos) - 1;
                let location2 = num_set_until(*occupation2, pos) - 1;
                if let Some(found) = find_common_recurse(&children1[location1], &children2[location2], hash_pos + 1) {
                    return Some(found);
                }
            }
            None
        }
    }
}

fn for_each_recurse<K, V, R>(node: &Node<K, V>, f: &mut impl FnMut(&K, &V) -> Option<R>) -> Option<R> {
    match node {
        Node::Empty => None,
        Node::List(list) => list.iter().find_map(|(key, value)| f(key, value)),
        Node::Leaf(leaf) => leaf.entries.iter().find_map(|(key, value)| f(key, value)),
        Node::Branch { children, .. } => children.iter().find_map(|child| for_each_recurse(child, f)),
    }
}

fn for_each_recurse_mut<K, V>(node: &mut Node<K, V>, f: &mut impl FnMut(&K, &mut V)) {
    match node {
        Node::Empty => {}
        Node::List(list) => list.iter_mut().for_each(|(key, value)| f(key, value)),
        Node::Leaf(leaf) => leaf.entries.iter_mut().for_each(|(key, value)| f(key, value)),
        Node::Branch { children, .. } => children.iter_mut().for_each(|child| for_each_recurse_mut(child, f)),
    }
}
