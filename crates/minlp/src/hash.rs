//! SCIP's hash tables, maps and sets (`misc.c`): Robin Hood hashing with
//! linear probing over SCIP's own hash values, so elements sit in the same
//! slots, and iterate in the same order, as in C.
//!
//! C stores the key callbacks in the table. Here they are a [`HashOps`]
//! passed to every call, so they can read solver data the table's owner
//! also owns. Maps and sets hash pointers in C; here they hash the ids that
//! stand for them.

/// `hashvalue`.
#[inline(always)]
fn hashvalue(input: u64) -> u32 {
    ((0x9e3779b97f4a7c15u64.wrapping_mul(input) >> 32) as u32) | 1
}

/// The callbacks of a hash table: `SCIP_DECL_HASHGETKEY`,
/// `SCIP_DECL_HASHKEYEQ` and `SCIP_DECL_HASHKEYVAL`.
pub trait HashOps<E> {
    type Key;
    fn get_key(&self, element: &E) -> Self::Key;
    fn key_eq(&self, key1: &Self::Key, key2: &Self::Key) -> bool;
    fn key_val(&self, key: &Self::Key) -> u64;
}

/// `SCIPhashKeyValString`, with C's signed `char`.
pub fn hash_key_val_string(s: &str) -> u64 {
    let mut hash: u64 = 37;
    for &c in s.as_bytes() {
        hash = hash.wrapping_mul(11);
        hash = hash.wrapping_add(c as i8 as i32 as u32 as u64);
    }
    hash
}

/// `SCIP_HASHTABLE`.
#[derive(Clone, Debug)]
pub struct HashTable<E> {
    slots: Vec<E>,
    hashes: Vec<u32>,
    shift: u32,
    mask: u32,
    nelements: u32,
}

impl<E: Copy + Default> HashTable<E> {
    /// `SCIPhashtableCreate`.
    pub fn new(tablesize: i32) -> Self {
        let shift = 32 - (crate::def::max(32.0, tablesize as f64 / 0.9)).log2().ceil() as u32;
        let nslots = 1u32 << (32 - shift);
        Self {
            slots: vec![E::default(); nslots as usize],
            hashes: vec![0; nslots as usize],
            shift,
            mask: nslots - 1,
            nelements: 0,
        }
    }

    #[inline(always)]
    fn elem_distance(&self, pos: u32) -> u32 {
        (pos.wrapping_add(self.mask).wrapping_add(1).wrapping_sub(self.hashes[pos as usize] >> self.shift)) & self.mask
    }

    /// `hashtableInsert`: `Err(())` is `SCIP_KEYALREADYEXISTING`.
    fn insert_hashed<O: HashOps<E>>(&mut self, ops: &O, mut element: E, mut key: O::Key, mut hashval: u32, override_: bool) -> Result<(), ()> {
        let mut pos = hashval >> self.shift;
        let mut elemdistance = 0u32;
        loop {
            if self.hashes[pos as usize] == 0 {
                self.slots[pos as usize] = element;
                self.hashes[pos as usize] = hashval;
                self.nelements += 1;
                return Ok(());
            }
            if self.hashes[pos as usize] == hashval && ops.key_eq(&ops.get_key(&self.slots[pos as usize]), &key) {
                if override_ {
                    self.slots[pos as usize] = element;
                    self.hashes[pos as usize] = hashval;
                    return Ok(());
                }
                return Err(());
            }
            let distance = self.elem_distance(pos);
            if distance < elemdistance {
                elemdistance = distance;
                std::mem::swap(&mut self.slots[pos as usize], &mut element);
                std::mem::swap(&mut self.hashes[pos as usize], &mut hashval);
                key = ops.get_key(&element);
            }
            pos = (pos + 1) & self.mask;
            elemdistance += 1;
        }
    }

    /// `hashtableCheckLoad`: doubles the table above a load of 0.9.
    fn check_load<O: HashOps<E>>(&mut self, ops: &O) {
        if ((self.nelements as u64) << 10) >> (32 - self.shift) > 921 {
            let nslots = self.mask + 1;
            let newnslots = 2 * nslots;
            self.mask = newnslots - 1;
            self.shift -= 1;
            let slots = std::mem::replace(&mut self.slots, vec![E::default(); newnslots as usize]);
            let hashes = std::mem::replace(&mut self.hashes, vec![0; newnslots as usize]);
            self.nelements = 0;
            for i in 0..nslots as usize {
                if hashes[i] != 0 {
                    let key = ops.get_key(&slots[i]);
                    self.insert_hashed(ops, slots[i], key, hashes[i], false).expect("rehashing a hash table");
                }
            }
        }
    }

    /// `SCIPhashtableInsert`: inserts or overrides.
    pub fn insert<O: HashOps<E>>(&mut self, ops: &O, element: E) {
        self.check_load(ops);
        let key = ops.get_key(&element);
        let hashval = hashvalue(ops.key_val(&key));
        let _ = self.insert_hashed(ops, element, key, hashval, true);
    }

    /// `SCIPhashtableSafeInsert`: `Err(())` if the key exists.
    #[allow(clippy::result_unit_err, reason = "SCIP_KEYALREADYEXISTING carries nothing more")]
    pub fn safe_insert<O: HashOps<E>>(&mut self, ops: &O, element: E) -> Result<(), ()> {
        self.check_load(ops);
        let key = ops.get_key(&element);
        let hashval = hashvalue(ops.key_val(&key));
        self.insert_hashed(ops, element, key, hashval, false)
    }

    /// `SCIPhashtableRetrieve`.
    pub fn retrieve<O: HashOps<E>>(&self, ops: &O, key: &O::Key) -> Option<E> {
        let hashval = hashvalue(ops.key_val(key));
        let mut pos = hashval >> self.shift;
        let mut elemdistance = 0u32;
        loop {
            if self.hashes[pos as usize] == 0 {
                return None;
            }
            let distance = self.elem_distance(pos);
            if elemdistance > distance {
                return None;
            }
            if self.hashes[pos as usize] == hashval && ops.key_eq(&ops.get_key(&self.slots[pos as usize]), key) {
                return Some(self.slots[pos as usize]);
            }
            pos = (pos + 1) & self.mask;
            elemdistance += 1;
        }
    }

    /// `SCIPhashtableExists`.
    pub fn exists<O: HashOps<E>>(&self, ops: &O, element: &E) -> bool {
        self.retrieve(ops, &ops.get_key(element)).is_some()
    }

    /// `SCIPhashtableRemove`.
    pub fn remove<O: HashOps<E>>(&mut self, ops: &O, element: &E) {
        let key = ops.get_key(element);
        let hashval = hashvalue(ops.key_val(&key));
        let mut elemdistance = 0u32;
        let mut pos = hashval >> self.shift;
        loop {
            if self.hashes[pos as usize] == 0 {
                return;
            }
            let distance = self.elem_distance(pos);
            if elemdistance > distance {
                return;
            }
            if self.hashes[pos as usize] == hashval && ops.key_eq(&ops.get_key(&self.slots[pos as usize]), &key) {
                break;
            }
            pos = (pos + 1) & self.mask;
            elemdistance += 1;
        }
        self.hashes[pos as usize] = 0;
        self.nelements -= 1;
        loop {
            let nextpos = (pos + 1) & self.mask;
            if self.hashes[nextpos as usize] == 0 {
                break;
            }
            if (self.hashes[nextpos as usize] >> self.shift) == nextpos {
                break;
            }
            self.slots[pos as usize] = self.slots[nextpos as usize];
            self.hashes[pos as usize] = self.hashes[nextpos as usize];
            self.hashes[nextpos as usize] = 0;
            pos = nextpos;
        }
    }

    /// `SCIPhashtableRemoveAll`.
    pub fn remove_all(&mut self) {
        self.hashes.fill(0);
        self.nelements = 0;
    }

    /// `SCIPhashtableGetNElements`.
    pub fn get_n_elements(&self) -> i64 {
        self.nelements as i64
    }

    /// `SCIPhashtableGetNEntries`.
    pub fn get_n_entries(&self) -> usize {
        self.mask as usize + 1
    }

    /// `SCIPhashtableGetEntry`.
    pub fn get_entry(&self, entryidx: usize) -> Option<E> {
        if self.hashes[entryidx] == 0 { None } else { Some(self.slots[entryidx]) }
    }
}

/// `SCIP_HASHMAP`, from an id to an image of type `V` (C's pointer, int,
/// long or real images).
#[derive(Clone, Debug)]
pub struct HashMap<V> {
    origins: Vec<u64>,
    images: Vec<V>,
    hashes: Vec<u32>,
    shift: u32,
    mask: u32,
    nelements: u32,
}

impl<V: Copy + Default> HashMap<V> {
    /// `SCIPhashmapCreate`.
    pub fn new(mapsize: i32) -> Self {
        let shift = 32 - ((crate::def::max(32.0, mapsize as f64 / 0.9)).ln() / 2.0f64.ln()).ceil() as u32;
        let nslots = 1u32 << (32 - shift);
        Self {
            origins: vec![0; nslots as usize],
            images: vec![V::default(); nslots as usize],
            hashes: vec![0; nslots as usize],
            shift,
            mask: nslots - 1,
            nelements: 0,
        }
    }

    #[inline(always)]
    fn elem_distance(&self, pos: u32) -> u32 {
        (pos.wrapping_add(self.mask).wrapping_add(1).wrapping_sub(self.hashes[pos as usize] >> self.shift)) & self.mask
    }

    /// `hashmapInsert`.
    fn insert_hashed(&mut self, mut origin: u64, mut image: V, mut hashval: u32, override_: bool) -> Result<(), ()> {
        let mut pos = hashval >> self.shift;
        let mut elemdistance = 0u32;
        loop {
            let p = pos as usize;
            if self.hashes[p] == 0 {
                self.origins[p] = origin;
                self.images[p] = image;
                self.hashes[p] = hashval;
                self.nelements += 1;
                return Ok(());
            }
            if hashval == self.hashes[p] && origin == self.origins[p] {
                if override_ {
                    self.images[p] = image;
                    self.hashes[p] = hashval;
                    return Ok(());
                }
                return Err(());
            }
            let distance = self.elem_distance(pos);
            if distance < elemdistance {
                elemdistance = distance;
                std::mem::swap(&mut self.hashes[p], &mut hashval);
                std::mem::swap(&mut self.origins[p], &mut origin);
                std::mem::swap(&mut self.images[p], &mut image);
            }
            pos = (pos + 1) & self.mask;
            elemdistance += 1;
        }
    }

    /// `hashmapLookup`.
    fn lookup(&self, origin: u64) -> Option<usize> {
        let hashval = hashvalue(origin);
        let mut pos = hashval >> self.shift;
        let mut elemdistance = 0u32;
        loop {
            let p = pos as usize;
            if self.hashes[p] == 0 {
                return None;
            }
            let distance = self.elem_distance(pos);
            if elemdistance > distance {
                return None;
            }
            if self.hashes[p] == hashval && self.origins[p] == origin {
                return Some(p);
            }
            pos = (pos + 1) & self.mask;
            elemdistance += 1;
        }
    }

    /// `hashmapCheckLoad`.
    fn check_load(&mut self) {
        if ((self.nelements as u64) << 10) >> (32 - self.shift) > 921 {
            let nslots = self.mask + 1;
            self.shift -= 1;
            let newnslots = 2 * nslots;
            self.mask = newnslots - 1;
            let origins = std::mem::replace(&mut self.origins, vec![0; newnslots as usize]);
            let images = std::mem::replace(&mut self.images, vec![V::default(); newnslots as usize]);
            let hashes = std::mem::replace(&mut self.hashes, vec![0; newnslots as usize]);
            self.nelements = 0;
            for i in 0..nslots as usize {
                if hashes[i] != 0 {
                    self.insert_hashed(origins[i], images[i], hashes[i], false).expect("rehashing a hash map");
                }
            }
        }
    }

    /// `SCIPhashmapInsert` (and its Int, Long and Real variants):
    /// `Err(())` if the origin is already mapped.
    #[allow(clippy::result_unit_err, reason = "SCIP_KEYALREADYEXISTING carries nothing more")]
    pub fn insert(&mut self, origin: u64, image: V) -> Result<(), ()> {
        self.check_load();
        self.insert_hashed(origin, image, hashvalue(origin), false)
    }

    /// `SCIPhashmapGetImage` and its variants.
    pub fn get_image(&self, origin: u64) -> Option<V> {
        self.lookup(origin).map(|pos| self.images[pos])
    }

    /// `SCIPhashmapSetImage`: inserts or overrides.
    pub fn set_image(&mut self, origin: u64, image: V) {
        self.check_load();
        let _ = self.insert_hashed(origin, image, hashvalue(origin), true);
    }

    /// `SCIPhashmapExists`.
    pub fn exists(&self, origin: u64) -> bool {
        self.lookup(origin).is_some()
    }

    /// `SCIPhashmapRemove`.
    pub fn remove(&mut self, origin: u64) {
        if let Some(mut pos) = self.lookup(origin) {
            self.hashes[pos] = 0;
            self.nelements -= 1;
            loop {
                let nextpos = (pos + 1) & self.mask as usize;
                if self.hashes[nextpos] == 0 {
                    return;
                }
                if (self.hashes[nextpos] >> self.shift) as usize == nextpos {
                    return;
                }
                self.origins[pos] = self.origins[nextpos];
                self.images[pos] = self.images[nextpos];
                self.hashes[pos] = self.hashes[nextpos];
                self.hashes[nextpos] = 0;
                pos = nextpos;
            }
        }
    }

    /// `SCIPhashmapRemoveAll`.
    pub fn remove_all(&mut self) {
        self.hashes.fill(0);
        self.nelements = 0;
    }

    /// `SCIPhashmapIsEmpty`.
    pub fn is_empty(&self) -> bool {
        self.nelements == 0
    }

    /// `SCIPhashmapGetNElements`.
    pub fn get_n_elements(&self) -> usize {
        self.nelements as usize
    }

    /// `SCIPhashmapGetNEntries`.
    pub fn get_n_entries(&self) -> usize {
        self.mask as usize + 1
    }

    /// `SCIPhashmapGetEntry`: the origin and image in slot `entryidx`.
    pub fn get_entry(&self, entryidx: usize) -> Option<(u64, V)> {
        if self.hashes[entryidx] == 0 {
            None
        } else {
            Some((self.origins[entryidx], self.images[entryidx]))
        }
    }

    /// `SCIPhashmapEntrySetImage` on slot `entryidx`.
    pub fn entry_set_image(&mut self, entryidx: usize, image: V) {
        self.images[entryidx] = image;
    }
}

/// `SCIP_HASHSET` of ids. C's empty slot is `NULL`; here an element is
/// stored as its id plus one, so id 0 can be a member.
#[derive(Clone, Debug)]
pub struct HashSet {
    slots: Vec<u64>,
    shift: u32,
    nelements: u32,
}

impl HashSet {
    /// `SCIPhashsetCreate`.
    pub fn new(size: i32) -> Self {
        let shift = 64 - ((crate::def::max(8.0, size as f64 / 0.9)).ln() / 2.0f64.ln()).ceil() as u32;
        let nslots = 1u32 << (64 - shift);
        Self {
            slots: vec![0; nslots as usize],
            shift,
            nelements: 0,
        }
    }

    /// `SCIPhashsetGetNSlots`.
    pub fn get_n_slots(&self) -> usize {
        1usize << (64 - self.shift)
    }

    /// `hashSetDesiredPos`, of a stored (id plus one) element.
    #[inline(always)]
    fn desired_pos(&self, stored: u64) -> u32 {
        (0x9e3779b97f4a7c15u64.wrapping_mul(stored) >> self.shift) as u32
    }

    #[inline(always)]
    fn elem_distance(&self, pos: u32, nslots: u32, mask: u32) -> u32 {
        (pos.wrapping_add(nslots).wrapping_sub(self.desired_pos(self.slots[pos as usize]))) & mask
    }

    /// `hashsetInsert`.
    fn insert_stored(&mut self, mut element: u64) {
        let mut pos = self.desired_pos(element);
        let nslots = self.get_n_slots() as u32;
        let mask = nslots - 1;
        let mut elemdistance = 0u32;
        loop {
            if self.slots[pos as usize] == 0 {
                self.slots[pos as usize] = element;
                self.nelements += 1;
                return;
            }
            if self.slots[pos as usize] == element {
                return;
            }
            let distance = self.elem_distance(pos, nslots, mask);
            if distance < elemdistance {
                elemdistance = distance;
                std::mem::swap(&mut self.slots[pos as usize], &mut element);
            }
            pos = (pos + 1) & mask;
            elemdistance += 1;
        }
    }

    /// `hashsetCheckLoad`.
    fn check_load(&mut self) {
        if ((self.nelements as u64) << 10) >> (64 - self.shift) > 921 {
            let nslots = self.get_n_slots();
            let newnslots = 2 * nslots;
            self.shift -= 1;
            let slots = std::mem::replace(&mut self.slots, vec![0; newnslots]);
            self.nelements = 0;
            for &slot in slots.iter().take(nslots) {
                if slot != 0 {
                    self.insert_stored(slot);
                }
            }
        }
    }

    /// `SCIPhashsetInsert`.
    pub fn insert(&mut self, element: u64) {
        self.check_load();
        self.insert_stored(element + 1);
    }

    /// `SCIPhashsetExists`.
    pub fn exists(&self, element: u64) -> bool {
        let element = element + 1;
        let mut pos = self.desired_pos(element);
        let nslots = self.get_n_slots() as u32;
        let mask = nslots - 1;
        let mut elemdistance = 0u32;
        loop {
            if self.slots[pos as usize] == element {
                return true;
            }
            if self.slots[pos as usize] == 0 {
                return false;
            }
            let distance = self.elem_distance(pos, nslots, mask);
            if elemdistance > distance {
                return false;
            }
            pos = (pos + 1) & mask;
            elemdistance += 1;
        }
    }

    /// `SCIPhashsetRemove`.
    pub fn remove(&mut self, element: u64) {
        let element = element + 1;
        let mut pos = self.desired_pos(element);
        let nslots = self.get_n_slots() as u32;
        let mask = nslots - 1;
        let mut elemdistance = 0u32;
        loop {
            if self.slots[pos as usize] == element {
                break;
            }
            if self.slots[pos as usize] == 0 {
                return;
            }
            let distance = self.elem_distance(pos, nslots, mask);
            if elemdistance > distance {
                return;
            }
            pos = (pos + 1) & mask;
            elemdistance += 1;
        }
        self.nelements -= 1;
        loop {
            let nextpos = (pos + 1) & mask;
            if self.slots[nextpos as usize] == 0 {
                self.slots[pos as usize] = 0;
                return;
            }
            if self.desired_pos(self.slots[nextpos as usize]) == nextpos {
                self.slots[pos as usize] = 0;
                return;
            }
            self.slots[pos as usize] = self.slots[nextpos as usize];
            pos = nextpos;
        }
    }

    /// `SCIPhashsetRemoveAll`.
    pub fn remove_all(&mut self) {
        self.slots.fill(0);
        self.nelements = 0;
    }

    /// `SCIPhashsetIsEmpty`.
    pub fn is_empty(&self) -> bool {
        self.nelements == 0
    }

    /// `SCIPhashsetGetNElements`.
    pub fn get_n_elements(&self) -> usize {
        self.nelements as usize
    }

    /// The members in slot order (`SCIPhashsetGetSlots`, skipping empty
    /// slots).
    pub fn elements(&self) -> impl Iterator<Item = u64> + '_ {
        self.slots.iter().filter(|&&s| s != 0).map(|&s| s - 1)
    }
}

/// `SCIP_MULTIHASH_MAXSIZE`: maximum size of a multihash table.
const MULTIHASH_MAXSIZE: i32 = 33554431; // 2^25 - 1
/// `SCIP_MULTIHASH_RESIZE_PERCENTAGE`: load in percent at which to resize.
const MULTIHASH_RESIZE_PERCENTAGE: f64 = 65.0;
/// `SCIP_MULTIHASH_GROW_FACTOR`.
const MULTIHASH_GROW_FACTOR: f64 = 1.31;

/// `SCIP_MULTIHASH`: a hash table of lists that may hold several elements
/// with equal keys. Each list is stored head first (C's singly linked
/// `SCIP_MULTIHASHLIST`).
#[derive(Clone, Debug)]
pub struct MultiHash<E> {
    lists: Vec<Vec<E>>,
    nelements: i64,
}

/// `SCIP_MULTIHASHLIST*` as a cursor for [`MultiHash::retrieve_next`]: the
/// list and the position of the next entry to look at, or `None` (NULL).
pub type MultiHashCursor = Option<(usize, usize)>;

impl<E: Copy + PartialEq> MultiHash<E> {
    /// `SCIPmultihashCreate`: creates a multihash table.
    pub fn new(tablesize: i32) -> Self {
        Self {
            lists: vec![Vec::new(); tablesize.max(0) as usize],
            nelements: 0,
        }
    }

    /// `SCIPmultihashGetLoad`: returns the load of the given multihash table
    /// in percentage.
    pub fn get_load(&self) -> f64 {
        self.nelements as f64 / self.lists.len() as f64 * 100.0
    }

    /// `SCIPmultihashGetNElements`.
    pub fn get_n_elements(&self) -> i64 {
        self.nelements
    }

    fn hashval<O: HashOps<E>>(_ops: &O, keyval: u64, nlists: usize) -> usize {
        (keyval % (nlists as u32 as u64)) as u32 as usize
    }

    /// `multihashResize`: resizes a multihash table.
    fn resize<O: HashOps<E>>(&mut self, ops: &O) {
        // Get new memeory for hash table lists.
        let nlists = self.lists.len() as i32;
        let mut nnewlists = crate::def::min(((nlists as u32) as f64 * MULTIHASH_GROW_FACTOR) as u32 as f64, MULTIHASH_MAXSIZE as f64) as i32;
        nnewlists = nnewlists.max(nlists);

        if nnewlists > nlists {
            let mut newlists: Vec<Vec<E>> = vec![Vec::new(); nnewlists as usize];

            // Move all lists.
            for l in (0..nlists as usize).rev() {
                let list = std::mem::take(&mut self.lists[l]);
                let mut onlyone = true;

                // Move all elements frmm the old lists into the new lists.
                for (i, &element) in list.iter().enumerate() {
                    // Get the hash key and its hash value.
                    let key = ops.get_key(&element);
                    let keyval = ops.key_val(&key);
                    let hashval = Self::hashval(ops, keyval, nnewlists as usize);

                    // If the old hash table list consists of only one entry, we
                    // still can use this old memory block instead of creating a
                    // new one: it goes to the end of the new list.
                    if i + 1 == list.len() && onlyone {
                        newlists[hashval].push(element);
                    } else {
                        // Append old element to the list at the hash position.
                        newlists[hashval].insert(0, element);
                    }
                    onlyone = false;
                }
            }

            // Set new data (the number of elements is unchanged).
            self.lists = newlists;
        }
    }

    /// `SCIPmultihashInsert`: inserts element in multihash table (multiple
    /// inserts of same element possible).
    pub fn insert<O: HashOps<E>>(&mut self, ops: &O, element: E) {
        // Dynamically resizing the hashtables.
        if self.get_load() > MULTIHASH_RESIZE_PERCENTAGE {
            self.resize(ops);
        }

        // Get the hash key and its hash value.
        let key = ops.get_key(&element);
        let keyval = ops.key_val(&key);
        let hashval = Self::hashval(ops, keyval, self.lists.len());

        // Append element to the list at the hash position.
        self.lists[hashval].insert(0, element);

        self.nelements += 1;
    }

    /// `multihashlistFind` from position `start` of list `l`: the position of
    /// the first matching entry.
    fn find<O: HashOps<E>>(&self, ops: &O, l: usize, start: usize, keyval: u64, key: &O::Key) -> Option<usize> {
        let list = &self.lists[l];
        (start..list.len()).find(|&p| {
            let currentkey = ops.get_key(&list[p]);
            let currentkeyval = ops.key_val(&currentkey);
            currentkeyval == keyval && ops.key_eq(&currentkey, key)
        })
    }

    /// `SCIPmultihashRetrieve`: retrieve element with key from multihash
    /// table, returns NULL if not existing.
    pub fn retrieve<O: HashOps<E>>(&self, ops: &O, key: &O::Key) -> Option<E> {
        // Get the hash value of the key.
        let keyval = ops.key_val(key);
        let l = Self::hashval(ops, keyval, self.lists.len());

        self.find(ops, l, 0, keyval, key).map(|p| self.lists[l][p])
    }

    /// `SCIPmultihashRetrieveNext`: retrieve element with key from multihash
    /// table, returns NULL if not existing; can be used to retrieve all
    /// entries with the same key (one-by-one).
    ///
    /// `cursor` is `None` to start; afterwards it is the entry after the
    /// retrieved one (`None` after the last entry of the list, as in C).
    pub fn retrieve_next<O: HashOps<E>>(&self, ops: &O, cursor: &mut MultiHashCursor, key: &O::Key) -> Option<E> {
        let keyval = ops.key_val(key);

        let (l, start) = match *cursor {
            Some(c) => c,
            None => {
                // Get the hash value of the key.
                (Self::hashval(ops, keyval, self.lists.len()), 0)
            }
        };

        // Find hash list entry.
        match self.find(ops, l, start, keyval, key) {
            Some(p) => {
                // `h->next`, which is NULL after the last entry.
                *cursor = if p + 1 < self.lists[l].len() { Some((l, p + 1)) } else { None };
                Some(self.lists[l][p])
            }
            None => {
                *cursor = None;
                None
            }
        }
    }

    /// `SCIPmultihashExists`: returns whether the given element exists in the
    /// multihash table.
    pub fn exists<O: HashOps<E>>(&self, ops: &O, element: E) -> bool {
        // Get the hash key and its hash value.
        let key = ops.get_key(&element);
        let keyval = ops.key_val(&key);
        let l = Self::hashval(ops, keyval, self.lists.len());

        self.find(ops, l, 0, keyval, &key).is_some()
    }

    /// `SCIPmultihashRemove`: removes element from the multihash table, if
    /// it exists.
    pub fn remove<O: HashOps<E>>(&mut self, ops: &O, element: E) {
        // Get the hash key for the element.
        let key = ops.get_key(&element);
        let keyval = ops.key_val(&key);
        let l = Self::hashval(ops, keyval, self.lists.len());

        // Remove element from the list at the hash position.
        if let Some(p) = self.lists[l].iter().position(|&e| e == element) {
            self.lists[l].remove(p);
            self.nelements -= 1;
        }
    }

    /// `SCIPmultihashRemoveAll`: removes all elements of the multihash
    /// table.
    pub fn remove_all(&mut self) {
        for l in self.lists.iter_mut() {
            l.clear();
        }
        self.nelements = 0;
    }
}
