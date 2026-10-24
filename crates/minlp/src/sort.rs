//! SCIP's sorting templates (`sorttpl.c`): shell sort below 26 elements,
//! quicksort with median-of-three or ninther pivots above, sorted vector
//! insertion and search, and weighted selection.
//!
//! C instantiates the template once per key and field combination
//! (`SCIPsortIntRealPtr`, `SCIPsortDownReal`, ...). Here the key array is
//! a slice, the arrays permuted alongside it are a tuple of slices
//! implementing [`Fields`], and the comparison is a closure with SCIP's
//! sign convention: negative when the first argument goes first. The
//! `Down` variants negate it, as `SORTTPL_BACKWARDS` does.

const SHELLSORTMAX: usize = 25;
const MINSIZENINTHER: usize = 729;

/// The arrays permuted like the key array: `()`, or a tuple of mutable
/// slices of `Copy` elements.
pub trait Fields {
    type Item: Copy;
    fn get(&self, i: usize) -> Self::Item;
    fn set(&mut self, i: usize, item: Self::Item);
    fn swap(&mut self, a: usize, b: usize);
}

impl Fields for () {
    type Item = ();
    #[inline(always)]
    fn get(&self, _: usize) {}
    #[inline(always)]
    fn set(&mut self, _: usize, _: ()) {}
    #[inline(always)]
    fn swap(&mut self, _: usize, _: usize) {}
}

macro_rules! fields_tuple {
    ($($t:ident $i:tt),+) => {
        impl<$($t: Copy),+> Fields for ($(&mut [$t],)+) {
            type Item = ($($t,)+);
            #[inline(always)]
            fn get(&self, i: usize) -> Self::Item {
                ($(self.$i[i],)+)
            }
            #[inline(always)]
            fn set(&mut self, i: usize, item: Self::Item) {
                $(self.$i[i] = item.$i;)+
            }
            #[inline(always)]
            fn swap(&mut self, a: usize, b: usize) {
                $(self.$i.swap(a, b);)+
            }
        }
    };
}

fields_tuple!(A 0);
fields_tuple!(A 0, B 1);
fields_tuple!(A 0, B 1, C 2);
fields_tuple!(A 0, B 1, C 2, D 3);
fields_tuple!(A 0, B 1, C 2, D 3, E 4);
fields_tuple!(A 0, B 1, C 2, D 3, E 4, F 5);

/// SCIP's comparison of two integer keys, `x - y`, as the C compiler
/// computes it: wrapping.
#[inline(always)]
pub fn cmp_int(x: &i32, y: &i32) -> i32 {
    x.wrapping_sub(*y)
}

/// SCIP's comparison of two `SCIP_Longint` keys, as the sign of `x - y`.
#[inline(always)]
pub fn cmp_long(x: &i64, y: &i64) -> i32 {
    let d = x.wrapping_sub(*y);
    (d > 0) as i32 - (d < 0) as i32
}

/// SCIP's comparison of two real keys, `x - y`: its sign, with NaN (both
/// infinite and equal) as zero.
#[inline(always)]
pub fn cmp_real(x: &f64, y: &f64) -> i32 {
    let d = *x - *y;
    (d > 0.0) as i32 - (d < 0.0) as i32
}

/// SCIP's comparison of two indices (pointer keys compared by `SCIPsortCompInt`-style
/// comparators are ported as their own closures).
#[inline(always)]
pub fn cmp_usize(x: &usize, y: &usize) -> i32 {
    (x > y) as i32 - (x < y) as i32
}

#[inline(always)]
fn shell_sort<K: Copy, F: Fields>(key: &mut [K], mut weights: Option<&mut [f64]>, fields: &mut F, cmp: &impl Fn(&K, &K) -> i32, start: usize, end: usize) {
    const INCS: [usize; 3] = [1, 5, 19];
    debug_assert!(start <= end);
    for k in (0..3).rev() {
        let h = INCS[k];
        let first = h + start;
        let mut i = first;
        while i <= end {
            let tempkey = key[i];
            let tmpweight = weights.as_ref().map_or(1.0, |w| w[i]);
            let tempfield = fields.get(i);
            let mut j = i;
            while j >= first && cmp(&tempkey, &key[j - h]) < 0 {
                key[j] = key[j - h];
                if let Some(w) = weights.as_mut() {
                    w[j] = w[j - h];
                }
                let moved = fields.get(j - h);
                fields.set(j, moved);
                j -= h;
            }
            key[j] = tempkey;
            if let Some(w) = weights.as_mut() {
                w[j] = tmpweight;
            }
            fields.set(j, tempfield);
            i += 1;
        }
    }
}

#[inline(always)]
fn median_three<K: Copy>(key: &[K], cmp: &impl Fn(&K, &K) -> i32, a: usize, b: usize, c: usize) -> usize {
    if cmp(&key[a], &key[b]) < 0 {
        if cmp(&key[b], &key[c]) < 0 {
            b
        } else if cmp(&key[a], &key[c]) < 0 {
            c
        } else {
            a
        }
    } else if cmp(&key[b], &key[c]) < 0 {
        if cmp(&key[a], &key[c]) < 0 { a } else { c }
    } else {
        b
    }
}

fn select_pivot_index<K: Copy>(key: &[K], cmp: &impl Fn(&K, &K) -> i32, start: usize, end: usize) -> usize {
    if end - start + 1 <= SHELLSORTMAX {
        (start + end) / 2
    } else if end - start + 1 < MINSIZENINTHER {
        let mid = (start + end) / 2;
        median_three(key, cmp, start, mid, end)
    } else {
        let gap = (end - start + 1) / 9;
        debug_assert!(start + 8 * gap <= end);
        let median1 = median_three(key, cmp, start, start + gap, start + 2 * gap);
        let median2 = median_three(key, cmp, start + 3 * gap, start + 4 * gap, start + 5 * gap);
        let median3 = median_three(key, cmp, start + 6 * gap, start + 7 * gap, start + 8 * gap);
        median_three(key, cmp, median1, median2, median3)
    }
}

fn q_sort<K: Copy, F: Fields>(key: &mut [K], fields: &mut F, cmp: &impl Fn(&K, &K) -> i32, mut start: usize, mut end: usize, mut ty: bool) {
    debug_assert!(start <= end);
    while end - start >= SHELLSORTMAX {
        let mid = select_pivot_index(key, cmp, start, end);
        let pivotkey = key[mid];
        // Signed, as in C: `hi` may pass below `start` only to `start - 1`
        // when `start` is 0, which the loop guards keep from happening.
        let mut lo = start;
        let mut hi = end;
        loop {
            if ty {
                while lo < end && cmp(&key[lo], &pivotkey) < 0 {
                    lo += 1;
                }
                while hi > start && !(cmp(&key[hi], &pivotkey) < 0) {
                    hi -= 1;
                }
            } else {
                while lo < end && !(cmp(&key[lo], &pivotkey) > 0) {
                    lo += 1;
                }
                while hi > start && cmp(&key[hi], &pivotkey) > 0 {
                    hi -= 1;
                }
            }
            if lo >= hi {
                break;
            }
            key.swap(lo, hi);
            fields.swap(lo, hi);
            lo += 1;
            hi -= 1;
        }
        if ty {
            while lo < end && !(cmp(&pivotkey, &key[lo]) < 0) {
                lo += 1;
            }
            if lo == start {
                key.swap(lo, mid);
                fields.swap(lo, mid);
                lo += 1;
            }
        } else {
            while hi > start && !(cmp(&pivotkey, &key[hi]) > 0) {
                hi -= 1;
            }
            if hi == end {
                key.swap(hi, mid);
                fields.swap(hi, mid);
                hi -= 1;
            }
        }
        // `hi - start` in C may be -1; compare as signed.
        if hi as isize - start as isize <= end as isize - lo as isize {
            if start < hi {
                q_sort(key, fields, cmp, start, hi, !ty);
            }
            start = lo;
        } else {
            if lo < end {
                q_sort(key, fields, cmp, lo, end, !ty);
            }
            end = hi;
        }
        ty = !ty;
    }
    if end > start {
        shell_sort(key, None, fields, cmp, start, end);
    }
}

/// `SCIPsort...()`: sorts `key` by `cmp` and permutes `fields` the same way.
pub fn sort<K: Copy, F: Fields>(key: &mut [K], mut fields: F, cmp: impl Fn(&K, &K) -> i32) {
    let len = key.len();
    if len <= 1 {
        return;
    }
    if len <= SHELLSORTMAX {
        shell_sort(key, None, &mut fields, &cmp, 0, len - 1);
    } else {
        q_sort(key, &mut fields, &cmp, 0, len - 1, true);
    }
    debug_assert!((0..len - 1).all(|i| !(cmp(&key[i + 1], &key[i]) < 0)));
}

/// `SCIPsortDown...()`: [`sort`] the other way round.
pub fn sort_down<K: Copy, F: Fields>(key: &mut [K], fields: F, cmp: impl Fn(&K, &K) -> i32) {
    sort(key, fields, |x, y| -cmp(x, y))
}

/// `SCIPsortedvecInsert...()`: inserts `keyval` and `fieldval` into the
/// sorted vectors, after any equal keys; returns the position.
pub fn sortedvec_insert<K: Copy, F: Fields>(key: &mut Vec<K>, fields: &mut F, cmp: impl Fn(&K, &K) -> i32, keyval: K, fieldval: F::Item, len: &mut usize) -> usize {
    if key.len() <= *len {
        key.resize(*len + 1, keyval);
    }
    let mut j = *len;
    while j > 0 && cmp(&keyval, &key[j - 1]) < 0 {
        key[j] = key[j - 1];
        let moved = fields.get(j - 1);
        fields.set(j, moved);
        j -= 1;
    }
    key[j] = keyval;
    fields.set(j, fieldval);
    *len += 1;
    j
}

/// `SCIPsortedvecDelPos...()`: deletes position `pos` from the sorted
/// vectors of length `len`.
pub fn sortedvec_del_pos<K: Copy, F: Fields>(key: &mut [K], fields: &mut F, pos: usize, len: &mut usize) {
    debug_assert!(pos < *len);
    *len -= 1;
    for j in pos..*len {
        key[j] = key[j + 1];
        let moved = fields.get(j + 1);
        fields.set(j, moved);
    }
}

/// `SCIPsortedvecFind...()`: binary search for `val` in the sorted `key`.
/// `Ok(pos)` where it is, `Err(pos)` where it would be inserted.
pub fn sortedvec_find<K: Copy>(key: &[K], cmp: impl Fn(&K, &K) -> i32, val: K) -> Result<usize, usize> {
    let mut left: isize = 0;
    let mut right: isize = key.len() as isize - 1;
    while left <= right {
        let middle = ((left + right) / 2) as usize;
        if cmp(&val, &key[middle]) < 0 {
            right = middle as isize - 1;
        } else if cmp(&key[middle], &val) < 0 {
            left = middle as isize + 1;
        } else {
            return Ok(middle);
        }
    }
    Err(left as usize)
}

/// `SCIPselectWeighted...()`: partially sorts `key` around the weighted
/// median with respect to `capacity`, permuting `weights` (all 1 when
/// `None`) and `fields` alike. Returns the median's position, `len` when
/// everything fits.
pub fn select_weighted<K: Copy, F: Fields>(key: &mut [K], mut fields: F, cmp: impl Fn(&K, &K) -> i32, mut weights: Option<&mut [f64]>, capacity: f64) -> Option<usize> {
    let len = key.len();
    let cmp = &cmp;
    let totalweightsum = match weights.as_ref() {
        Some(w) => {
            let mut sum = 0.0;
            for &weight in w.iter().take(len) {
                sum += weight;
            }
            sum
        }
        None => len as f64,
    };
    if totalweightsum <= capacity {
        return Some(len);
    }
    let mut lo: isize = 0;
    let mut hi: isize = len as isize - 1;
    let mut residualcapacity = capacity;
    macro_rules! exch {
        ($x:expr, $y:expr) => {{
            let (x, y) = ($x as usize, $y as usize);
            key.swap(x, y);
            if let Some(w) = weights.as_mut() {
                w.swap(x, y);
            }
            fields.swap(x, y);
        }};
    }
    while hi - lo + 1 > SHELLSORTMAX as isize {
        let pivotindex = select_pivot_index(key, cmp, lo as usize, hi as usize) as isize;
        let pivot = key[pivotindex as usize];
        if pivotindex != lo {
            exch!(lo, pivotindex);
        }
        let mut i = lo;
        let mut bt = lo;
        let mut wt = hi;
        while i <= wt {
            if cmp(&key[i as usize], &pivot) < 0 {
                exch!(i, bt);
                i += 1;
                bt += 1;
            } else if cmp(&key[i as usize], &pivot) > 0 {
                exch!(i, wt);
                wt -= 1;
            } else {
                i += 1;
            }
        }
        let betterweightsum = match weights.as_ref() {
            Some(w) => {
                let mut sum = 0.0;
                for i in lo..bt {
                    sum += w[i as usize];
                }
                sum
            }
            None => (bt - lo) as f64,
        };
        if betterweightsum > residualcapacity {
            hi = bt - 1;
        } else {
            let mut weightsum = betterweightsum;
            for p in bt..=wt {
                weightsum += weights.as_ref().map_or(1.0, |w| w[p as usize]);
                if weightsum > residualcapacity {
                    return Some(p as usize);
                }
            }
            residualcapacity -= weightsum;
            lo = wt + 1;
        }
    }
    if hi - lo + 1 > 1 {
        shell_sort(key, weights.as_deref_mut(), &mut fields, cmp, lo as usize, hi as usize);
    }
    let mut j = lo;
    while j <= lo.max(hi) {
        let weight = weights.as_ref().map_or(1.0, |w| w[j as usize]);
        if weight > residualcapacity {
            return Some(j as usize);
        }
        residualcapacity -= weight;
        j += 1;
    }
    None
}

/// `SCIPselect...()`: partially sorts `key` so that position `k` holds the
/// element a full sort would put there.
pub fn select<K: Copy, F: Fields>(key: &mut [K], fields: F, cmp: impl Fn(&K, &K) -> i32, k: usize) {
    if k >= key.len() {
        return;
    }
    let pos = select_weighted(key, fields, cmp, None, k as f64 + 0.5);
    debug_assert_eq!(pos, Some(k));
}

/// `SCIPselectDown...()`.
pub fn select_down<K: Copy, F: Fields>(key: &mut [K], fields: F, cmp: impl Fn(&K, &K) -> i32, k: usize) {
    select(key, fields, |x, y| -cmp(x, y), k)
}

/// `SCIPselectWeightedDown...()`.
pub fn select_weighted_down<K: Copy, F: Fields>(key: &mut [K], fields: F, cmp: impl Fn(&K, &K) -> i32, weights: Option<&mut [f64]>, capacity: f64) -> Option<usize> {
    select_weighted(key, fields, |x, y| -cmp(x, y), weights, capacity)
}
