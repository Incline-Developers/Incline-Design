//! HiGHS's `HighsHashHelpers::hash`, `vector_hash` and `HighsHashTable`:
//! the same hash values and the same Robin Hood table, so that iterating a
//! table visits its entries in the order HiGHS's does.

use crate::util::hash::{C, M61, multiply_mod_m61, pair_hash};

const FIBONACCI_MULTIPLIER: u64 = 0x9e3779b97f4a7c15;

/// HiGHS's `HighsHashHelpers::hash` of a value whose bytes, as C++ lays
/// them out, are `words` (little-endian 32-bit words; at most 64 bytes).
pub(crate) fn hash_words(words: &[u32]) -> u64 {
    let mut bytes = [0u32; 16];
    bytes[..words.len()].copy_from_slice(words);
    let p = |k: usize| pair_hash(k, bytes[2 * k], bytes[2 * k + 1]);
    match words.len() {
        0..=2 => pair_hash(1, bytes[0], bytes[1]) ^ (pair_hash(0, bytes[0], bytes[1]) >> 32),
        3..=4 => (p(0) ^ (p(1) >> 32)).wrapping_mul(FIBONACCI_MULTIPLIER),
        5..=6 => (p(0) ^ (p(1).wrapping_add(p(2)) >> 32)).wrapping_mul(FIBONACCI_MULTIPLIER),
        7..=8 => (p(0).wrapping_add(p(1)) ^ (p(2).wrapping_add(p(3)) >> 32)).wrapping_mul(FIBONACCI_MULTIPLIER),
        9..=10 => (p(0).wrapping_add(p(1)) ^ (p(2).wrapping_add(p(3)).wrapping_add(p(4)) >> 32)).wrapping_mul(FIBONACCI_MULTIPLIER),
        11..=12 => (p(0).wrapping_add(p(1)).wrapping_add(p(2)) ^ (p(3).wrapping_add(p(4)).wrapping_add(p(5)) >> 32)).wrapping_mul(FIBONACCI_MULTIPLIER),
        13..=14 => (p(0).wrapping_add(p(1)).wrapping_add(p(2)) ^ (p(3).wrapping_add(p(4)).wrapping_add(p(5)).wrapping_add(p(6)) >> 32)).wrapping_mul(FIBONACCI_MULTIPLIER),
        _ => (p(0).wrapping_add(p(1)).wrapping_add(p(2)).wrapping_add(p(3)) ^ (p(4).wrapping_add(p(5)).wrapping_add(p(6)).wrapping_add(p(7)) >> 32))
            .wrapping_mul(FIBONACCI_MULTIPLIER),
    }
}

/// HiGHS's `HighsHashHelpers::hash` of a `HighsInt` (32 bits).
pub(crate) fn hash_i32(value: i32) -> u64 {
    hash_words(&[value as u32])
}

/// HiGHS's `HighsHashHelpers::hash` of a 64-bit integer.
pub(crate) fn hash_u64(value: u64) -> u64 {
    hash_words(&[value as u32, (value >> 32) as u32])
}

/// HiGHS's `HighsHashHelpers::vector_hash` of `data`'s bytes.
pub(crate) fn vector_hash_bytes(data: &[u8]) -> u64 {
    // HiGHS copies each pair's bytes into one array that is never cleared,
    // so a short last pair keeps the bytes of the one before it.
    let mut pair = [0u8; 8];
    let mut hash: u64 = 0;
    let mut k = 0usize;
    let mut at = 0;
    while at != data.len() {
        let num_bytes = (data.len() - at).min(256);
        let num_pairs = num_bytes.div_ceil(8);
        let last_pair_bytes = num_bytes - (num_pairs - 1) * 8;
        let mut chunk_hash = [0u64; 2];
        if num_pairs == 32 && hash != 0 {
            if hash >= M61 {
                hash -= M61;
            }
            hash = multiply_mod_m61(hash, C[k & 63] & M61);
            k += 1;
        }
        for n in (1..=num_pairs).rev() {
            let length = if n == 1 { last_pair_bytes } else { 8 };
            pair[..length].copy_from_slice(&data[at..at + length]);
            let a = u32::from_le_bytes([pair[0], pair[1], pair[2], pair[3]]);
            let b = u32::from_le_bytes([pair[4], pair[5], pair[6], pair[7]]);
            chunk_hash[n & 1] = chunk_hash[n & 1].wrapping_add(pair_hash(32 - n, a, b));
            at += length;
        }
        hash = hash.wrapping_add((chunk_hash[0] >> 3) ^ (chunk_hash[1] >> 32));
    }
    hash.wrapping_mul(FIBONACCI_MULTIPLIER)
}

/// `vector_hash` of 32-bit values (`HighsInt` or `u32` arrays).
pub(crate) fn vector_hash_u32(values: impl IntoIterator<Item = u32>) -> u64 {
    let bytes: Vec<u8> = values.into_iter().flat_map(u32::to_le_bytes).collect();
    vector_hash_bytes(&bytes)
}

/// A key of a [`HashTable`]: its HiGHS hash.
pub(crate) trait HashKey: Clone + PartialEq {
    fn highs_hash(&self) -> u64;
}

impl HashKey for i32 {
    fn highs_hash(&self) -> u64 {
        hash_i32(*self)
    }
}

/// Columns and rows, kept as `usize` in solver, hash as HiGHS's 32-bit
/// `HighsInt`.
impl HashKey for usize {
    fn highs_hash(&self) -> u64 {
        hash_i32(*self as i32)
    }
}

impl HashKey for u64 {
    fn highs_hash(&self) -> u64 {
        hash_u64(*self)
    }
}

/// `std::pair<HighsInt, HighsInt>`.
impl HashKey for (usize, usize) {
    fn highs_hash(&self) -> u64 {
        hash_words(&[self.0 as u32, self.1 as u32])
    }
}

/// `std::tuple<HighsInt, HighsInt, HighsUInt>`: libstdc++ lays a tuple's
/// elements out last first.
impl HashKey for (usize, usize, u32) {
    fn highs_hash(&self) -> u64 {
        hash_words(&[self.2, self.1 as u32, self.0 as u32])
    }
}

const MAX_DISTANCE: u64 = 127;

/// HiGHS's `HighsHashTable`: open addressing with Robin Hood hashing and
/// one metadata byte per slot (its top bit marks it occupied, the rest
/// hold the low bits of the entry's ideal slot).
#[derive(Clone, Debug)]
pub(crate) struct HashTable<K, V> {
    entries: Vec<Option<(K, V)>>,
    metadata: Vec<u8>,
    table_size_mask: u64,
    num_hash_shift: u64,
    num_elements: usize,
}

impl<K: HashKey, V> Default for HashTable<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: HashKey, V> HashTable<K, V> {
    pub fn new() -> Self {
        let mut table = Self {
            entries: Vec::new(),
            metadata: Vec::new(),
            table_size_mask: 0,
            num_hash_shift: 0,
            num_elements: 0,
        };
        table.make_empty_table(128);
        table
    }

    /// Room for `min_capacity` entries without growing.
    pub fn with_capacity(min_capacity: usize) -> Self {
        let capacity = 1u64 << (128f64.max(8.0 * min_capacity as f64 / 7.0)).log2().ceil() as u64;
        let mut table = Self::new();
        table.make_empty_table(capacity);
        table
    }

    fn make_empty_table(&mut self, capacity: u64) {
        self.table_size_mask = capacity - 1;
        self.num_hash_shift = 64 - (63 - capacity.leading_zeros() as u64);
        self.num_elements = 0;
        self.metadata = vec![0; capacity as usize];
        self.entries = Vec::with_capacity(capacity as usize);
        self.entries.resize_with(capacity as usize, || None);
    }

    fn to_metadata(&self, hash: u64) -> u8 {
        ((hash >> self.num_hash_shift) | 0x80) as u8
    }

    fn occupied(meta: u8) -> bool {
        meta & 0x80 != 0
    }

    fn distance_from_ideal_slot(&self, pos: u64) -> u64 {
        pos.wrapping_sub(self.metadata[pos as usize] as u64) & 0x7f
    }

    fn grow_table(&mut self) {
        let old_entries = std::mem::take(&mut self.entries);
        let old_metadata = std::mem::take(&mut self.metadata);
        let old_capacity = self.table_size_mask + 1;
        self.make_empty_table(2 * old_capacity);
        for (meta, entry) in old_metadata.into_iter().zip(old_entries) {
            if Self::occupied(meta)
                && let Some((key, value)) = entry
            {
                self.insert(key, value);
            }
        }
    }

    fn shrink_table(&mut self) {
        let old_entries = std::mem::take(&mut self.entries);
        let old_metadata = std::mem::take(&mut self.metadata);
        let old_capacity = self.table_size_mask + 1;
        self.make_empty_table(old_capacity / 2);
        for (meta, entry) in old_metadata.into_iter().zip(old_entries) {
            if Self::occupied(meta)
                && let Some((key, value)) = entry
            {
                self.insert(key, value);
            }
        }
    }

    /// HiGHS's `findPosition`: `Ok(pos)` where `key` is, or `Err` with the
    /// metadata, start, last and stopping positions for an insertion.
    fn find_position(&self, key: &K) -> Result<u64, (u8, u64, u64, u64)> {
        let hash = key.highs_hash();
        let start_pos = hash >> self.num_hash_shift;
        let max_pos = (start_pos + MAX_DISTANCE) & self.table_size_mask;
        let meta = self.to_metadata(hash);
        let mut pos = start_pos;
        loop {
            let slot_meta = self.metadata[pos as usize];
            if !Self::occupied(slot_meta) {
                return Err((meta, start_pos, max_pos, pos));
            }
            if slot_meta == meta && self.entries[pos as usize].as_ref().is_some_and(|(stored, _)| stored == key) {
                return Ok(pos);
            }
            let current_distance = pos.wrapping_sub(start_pos) & self.table_size_mask;
            if current_distance > self.distance_from_ideal_slot(pos) {
                return Err((meta, start_pos, max_pos, pos));
            }
            pos = (pos + 1) & self.table_size_mask;
            if pos == max_pos {
                return Err((meta, start_pos, max_pos, pos));
            }
        }
    }

    pub fn clear(&mut self) {
        if self.num_elements > 0 {
            if self.table_size_mask + 1 == 128 {
                self.metadata.fill(0);
                self.entries.iter_mut().for_each(|entry| *entry = None);
                self.num_elements = 0;
            } else {
                self.make_empty_table(128);
            }
        }
    }

    pub fn find(&self, key: &K) -> Option<&V> {
        self.find_position(key).ok().map(|pos| &self.entries[pos as usize].as_ref().expect("an occupied slot").1)
    }

    pub fn find_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.find_position(key) {
            Ok(pos) => Some(&mut self.entries[pos as usize].as_mut().expect("an occupied slot").1),
            Err(_) => None,
        }
    }

    pub fn contains(&self, key: &K) -> bool {
        self.find_position(key).is_ok()
    }

    /// Robin Hood insertion of an entry known to be absent, from `pos` on.
    /// Returns where the entry went, or `None` when it ran out of room and
    /// the table grew instead.
    fn place(&mut self, mut entry: (K, V), mut meta: u8, mut start_pos: u64, mut max_pos: u64, mut pos: u64) -> Option<u64> {
        let first = pos;
        self.num_elements += 1;
        loop {
            if !Self::occupied(self.metadata[pos as usize]) {
                self.metadata[pos as usize] = meta;
                self.entries[pos as usize] = Some(entry);
                return Some(first);
            }
            let current_distance = pos.wrapping_sub(start_pos) & self.table_size_mask;
            let distance_of_current_occupant = self.distance_from_ideal_slot(pos);
            if current_distance > distance_of_current_occupant {
                let occupant = self.entries[pos as usize].replace(entry).expect("an occupied slot");
                entry = occupant;
                std::mem::swap(&mut meta, &mut self.metadata[pos as usize]);
                start_pos = pos.wrapping_sub(distance_of_current_occupant) & self.table_size_mask;
                max_pos = (start_pos + MAX_DISTANCE) & self.table_size_mask;
            }
            pos = (pos + 1) & self.table_size_mask;
            if pos == max_pos {
                break;
            }
        }
        self.grow_table();
        self.insert(entry.0, entry.1);
        None
    }

    /// HiGHS's `insert`: add `key` unless it is there; returns whether it
    /// was added.
    pub fn insert(&mut self, key: K, value: V) -> bool {
        let (meta, start_pos, max_pos, pos) = match self.find_position(&key) {
            Ok(_) => return false,
            Err(found) => found,
        };
        if self.num_elements == (((self.table_size_mask + 1) * 7) / 8) as usize || pos == max_pos {
            self.grow_table();
            return self.insert(key, value);
        }
        self.place((key, value), meta, start_pos, max_pos, pos);
        true
    }

    /// HiGHS's `operator[]`: `key`'s value, inserted as the default if it
    /// was not there.
    pub fn entry(&mut self, key: K) -> &mut V
    where
        V: Default,
    {
        let (meta, start_pos, max_pos, pos) = match self.find_position(&key) {
            Ok(pos) => return &mut self.entries[pos as usize].as_mut().expect("an occupied slot").1,
            Err(found) => found,
        };
        if self.num_elements == (((self.table_size_mask + 1) * 7) / 8) as usize || pos == max_pos {
            self.grow_table();
            return self.entry(key);
        }
        match self.place((key.clone(), V::default()), meta, start_pos, max_pos, pos) {
            Some(at) => &mut self.entries[at as usize].as_mut().expect("an occupied slot").1,
            None => self.entry(key),
        }
    }

    /// HiGHS's `erase`; returns whether `key` was there.
    pub fn erase(&mut self, key: &K) -> bool {
        let Ok(mut pos) = self.find_position(key) else {
            return false;
        };
        self.entries[pos as usize] = None;
        self.metadata[pos as usize] = 0;
        self.num_elements -= 1;
        let capacity = self.table_size_mask + 1;
        if capacity != 128 && (self.num_elements as u64) < capacity / 4 {
            self.shrink_table();
            return true;
        }
        loop {
            let shift = (pos + 1) & self.table_size_mask;
            if !Self::occupied(self.metadata[shift as usize]) {
                return true;
            }
            if self.distance_from_ideal_slot(shift) == 0 {
                return true;
            }
            self.entries[pos as usize] = self.entries[shift as usize].take();
            self.metadata[pos as usize] = self.metadata[shift as usize];
            self.metadata[shift as usize] = 0;
            pos = shift;
        }
    }

    pub fn len(&self) -> usize {
        self.num_elements
    }

    pub fn is_empty(&self) -> bool {
        self.num_elements == 0
    }

    /// The entries in slot order, as HiGHS's iterator visits them.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().filter_map(|entry| entry.as_ref().map(|(key, value)| (key, value)))
    }
}

/// HiGHS's `HighsHashTable<K>` with no values: a set.
pub(crate) type HashSet<K> = HashTable<K, ()>;
