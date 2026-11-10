//! Orson Peters' pattern-defeating quicksort, as HiGHS ships it
//! (`extern/pdqsort/pdqsort.h`), with libstdc++'s heap sort as its
//! fallback. HiGHS sorts with it where elements can compare equal, and
//! the order such elements end in decides which reductions presolve
//! makes, so it is ported step for step rather than replaced by
//! `sort_unstable_by`. HiGHS's `pdqsort` with a lambda uses the branching
//! partition; its explicit `pdqsort_branchless` uses the block partition.
//!
//! An altered version of pdqsort, Copyright (c) 2021 Orson Peters, under
//! the zlib licence in `LICENSE-pdqsort` at the crate's root.

const INSERTION_SORT_THRESHOLD: usize = 24;
const NINTHER_THRESHOLD: usize = 128;
const PARTIAL_INSERTION_SORT_LIMIT: usize = 8;
const BLOCK_SIZE: usize = 64;

/// Sort `v` so that `less` holds between no later and earlier element.
pub(crate) fn pdqsort<T: Copy>(v: &mut [T], mut less: impl FnMut(&T, &T) -> bool) {
    if v.is_empty() {
        return;
    }
    let bad_allowed = log2(v.len());
    pdqsort_loop(v, 0, v.len(), &mut less, bad_allowed, true, false);
}

/// HiGHS's `pdqsort_branchless`: [`pdqsort`] with the block partition.
pub(crate) fn pdqsort_branchless<T: Copy>(v: &mut [T], mut less: impl FnMut(&T, &T) -> bool) {
    if v.is_empty() {
        return;
    }
    let bad_allowed = log2(v.len());
    pdqsort_loop(v, 0, v.len(), &mut less, bad_allowed, true, true);
}

/// pdqsort's `swap_offsets`.
#[allow(clippy::too_many_arguments)]
fn swap_offsets<T: Copy>(v: &mut [T], first: usize, last: usize, offsets_l: &[u8], offsets_r: &[u8], num: usize, use_swaps: bool) {
    if use_swaps {
        for i in 0..num {
            v.swap(first + offsets_l[i] as usize, last - offsets_r[i] as usize);
        }
    } else if num > 0 {
        let mut l = first + offsets_l[0] as usize;
        let mut r = last - offsets_r[0] as usize;
        let tmp = v[l];
        v[l] = v[r];
        for i in 1..num {
            l = first + offsets_l[i] as usize;
            v[r] = v[l];
            r = last - offsets_r[i] as usize;
            v[l] = v[r];
        }
        v[r] = tmp;
    }
}

/// pdqsort's `partition_right_branchless`: [`partition_right`] by blocks
/// of offsets.
fn partition_right_branchless<T: Copy>(v: &mut [T], begin: usize, end: usize, less: &mut impl FnMut(&T, &T) -> bool) -> (usize, bool) {
    let pivot = v[begin];
    let mut first = begin;
    let mut last = end;
    loop {
        first += 1;
        if !less(&v[first], &pivot) {
            break;
        }
    }
    if first - 1 == begin {
        while first < last {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    } else {
        loop {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    }
    let already_partitioned = first >= last;
    if !already_partitioned {
        v.swap(first, last);
        first += 1;
        let mut offsets_l = [0u8; BLOCK_SIZE + 1];
        let mut offsets_r = [0u8; BLOCK_SIZE + 1];
        let mut offsets_l_base = first;
        let mut offsets_r_base = last;
        let (mut num_l, mut num_r, mut start_l, mut start_r) = (0usize, 0usize, 0usize, 0usize);
        while first < last {
            let num_unknown = last - first;
            let left_split = if num_l == 0 { if num_r == 0 { num_unknown / 2 } else { num_unknown } } else { 0 };
            let right_split = if num_r == 0 { num_unknown - left_split } else { 0 };
            let left_count = if left_split >= BLOCK_SIZE { BLOCK_SIZE } else { left_split };
            for i in 0..left_count {
                offsets_l[num_l] = i as u8;
                num_l += usize::from(!less(&v[first], &pivot));
                first += 1;
            }
            let right_count = if right_split >= BLOCK_SIZE { BLOCK_SIZE } else { right_split };
            for i in 0..right_count {
                offsets_r[num_r] = (i + 1) as u8;
                last -= 1;
                num_r += usize::from(less(&v[last], &pivot));
            }
            let num = num_l.min(num_r);
            swap_offsets(v, offsets_l_base, offsets_r_base, &offsets_l[start_l..], &offsets_r[start_r..], num, num_l == num_r);
            num_l -= num;
            num_r -= num;
            start_l += num;
            start_r += num;
            if num_l == 0 {
                start_l = 0;
                offsets_l_base = first;
            }
            if num_r == 0 {
                start_r = 0;
                offsets_r_base = last;
            }
        }
        if num_l > 0 {
            while num_l > 0 {
                num_l -= 1;
                last -= 1;
                v.swap(offsets_l_base + offsets_l[start_l + num_l] as usize, last);
            }
            first = last;
        }
        if num_r > 0 {
            while num_r > 0 {
                num_r -= 1;
                v.swap(offsets_r_base - offsets_r[start_r + num_r] as usize, first);
                first += 1;
            }
            last = first;
        }
        let _ = last;
    }
    let pivot_pos = first - 1;
    v[begin] = v[pivot_pos];
    v[pivot_pos] = pivot;
    (pivot_pos, already_partitioned)
}

fn log2(mut n: usize) -> i32 {
    let mut log = 0;
    while {
        n >>= 1;
        n != 0
    } {
        log += 1;
    }
    log
}

fn insertion_sort<T: Copy>(v: &mut [T], begin: usize, end: usize, less: &mut impl FnMut(&T, &T) -> bool) {
    if begin == end {
        return;
    }
    for cur in begin + 1..end {
        let mut sift = cur;
        if less(&v[sift], &v[sift - 1]) {
            let tmp = v[sift];
            loop {
                v[sift] = v[sift - 1];
                sift -= 1;
                if sift == begin || !less(&tmp, &v[sift - 1]) {
                    break;
                }
            }
            v[sift] = tmp;
        }
    }
}

/// Insertion sort that relies on an element no greater than all of
/// `begin..end` sitting just before `begin`.
fn unguarded_insertion_sort<T: Copy>(v: &mut [T], begin: usize, end: usize, less: &mut impl FnMut(&T, &T) -> bool) {
    if begin == end {
        return;
    }
    for cur in begin + 1..end {
        let mut sift = cur;
        if less(&v[sift], &v[sift - 1]) {
            let tmp = v[sift];
            loop {
                v[sift] = v[sift - 1];
                sift -= 1;
                if !less(&tmp, &v[sift - 1]) {
                    break;
                }
            }
            v[sift] = tmp;
        }
    }
}

/// Insertion sort that gives up once it has moved more than a few
/// elements, saying whether it finished.
fn partial_insertion_sort<T: Copy>(v: &mut [T], begin: usize, end: usize, less: &mut impl FnMut(&T, &T) -> bool) -> bool {
    if begin == end {
        return true;
    }
    let mut limit = 0;
    for cur in begin + 1..end {
        let mut sift = cur;
        if less(&v[sift], &v[sift - 1]) {
            let tmp = v[sift];
            loop {
                v[sift] = v[sift - 1];
                sift -= 1;
                if sift == begin || !less(&tmp, &v[sift - 1]) {
                    break;
                }
            }
            v[sift] = tmp;
            limit += cur - sift;
        }
        if limit > PARTIAL_INSERTION_SORT_LIMIT {
            return false;
        }
    }
    true
}

fn sort2<T: Copy>(v: &mut [T], a: usize, b: usize, less: &mut impl FnMut(&T, &T) -> bool) {
    if less(&v[b], &v[a]) {
        v.swap(a, b);
    }
}

fn sort3<T: Copy>(v: &mut [T], a: usize, b: usize, c: usize, less: &mut impl FnMut(&T, &T) -> bool) {
    sort2(v, a, b, less);
    sort2(v, b, c, less);
    sort2(v, a, b, less);
}

/// Partition `begin..end` around `v[begin]`, elements equal to it going
/// right; returns the pivot's place and whether nothing had to move.
fn partition_right<T: Copy>(v: &mut [T], begin: usize, end: usize, less: &mut impl FnMut(&T, &T) -> bool) -> (usize, bool) {
    let pivot = v[begin];
    let mut first = begin;
    let mut last = end;
    loop {
        first += 1;
        if !less(&v[first], &pivot) {
            break;
        }
    }
    if first - 1 == begin {
        while first < last {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    } else {
        loop {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    }
    let already_partitioned = first >= last;
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if !less(&v[first], &pivot) {
                break;
            }
        }
        loop {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    v[begin] = v[pivot_pos];
    v[pivot_pos] = pivot;
    (pivot_pos, already_partitioned)
}

/// Partition `begin..end` around `v[begin]`, elements equal to it going
/// left; used when many elements equal the pivot.
fn partition_left<T: Copy>(v: &mut [T], begin: usize, end: usize, less: &mut impl FnMut(&T, &T) -> bool) -> usize {
    let pivot = v[begin];
    let mut first = begin;
    let mut last = end;
    loop {
        last -= 1;
        if !less(&pivot, &v[last]) {
            break;
        }
    }
    if last + 1 == end {
        while first < last {
            first += 1;
            if less(&pivot, &v[first]) {
                break;
            }
        }
    } else {
        loop {
            first += 1;
            if less(&pivot, &v[first]) {
                break;
            }
        }
    }
    while first < last {
        v.swap(first, last);
        loop {
            last -= 1;
            if !less(&pivot, &v[last]) {
                break;
            }
        }
        loop {
            first += 1;
            if less(&pivot, &v[first]) {
                break;
            }
        }
    }
    let pivot_pos = last;
    v[begin] = v[pivot_pos];
    v[pivot_pos] = pivot;
    pivot_pos
}

fn pdqsort_loop<T: Copy>(v: &mut [T], mut begin: usize, end: usize, less: &mut impl FnMut(&T, &T) -> bool, mut bad_allowed: i32, mut leftmost: bool, branchless: bool) {
    loop {
        let size = end - begin;
        if size < INSERTION_SORT_THRESHOLD {
            if leftmost {
                insertion_sort(v, begin, end, less);
            } else {
                unguarded_insertion_sort(v, begin, end, less);
            }
            return;
        }

        let s2 = size / 2;
        if size > NINTHER_THRESHOLD {
            sort3(v, begin, begin + s2, end - 1, less);
            sort3(v, begin + 1, begin + (s2 - 1), end - 2, less);
            sort3(v, begin + 2, begin + (s2 + 1), end - 3, less);
            sort3(v, begin + (s2 - 1), begin + s2, begin + (s2 + 1), less);
            v.swap(begin, begin + s2);
        } else {
            sort3(v, begin + s2, begin, end - 1, less);
        }

        if !leftmost && !less(&v[begin - 1], &v[begin]) {
            begin = partition_left(v, begin, end, less) + 1;
            continue;
        }

        let (pivot_pos, already_partitioned) = if branchless {
            partition_right_branchless(v, begin, end, less)
        } else {
            partition_right(v, begin, end, less)
        };
        let l_size = pivot_pos - begin;
        let r_size = end - (pivot_pos + 1);
        let highly_unbalanced = l_size < size / 8 || r_size < size / 8;

        if highly_unbalanced {
            bad_allowed -= 1;
            if bad_allowed == 0 {
                make_heap(&mut v[begin..end], less);
                sort_heap(&mut v[begin..end], less);
                return;
            }
            if l_size >= INSERTION_SORT_THRESHOLD {
                v.swap(begin, begin + l_size / 4);
                v.swap(pivot_pos - 1, pivot_pos - l_size / 4);
                if l_size > NINTHER_THRESHOLD {
                    v.swap(begin + 1, begin + (l_size / 4 + 1));
                    v.swap(begin + 2, begin + (l_size / 4 + 2));
                    v.swap(pivot_pos - 2, pivot_pos - (l_size / 4 + 1));
                    v.swap(pivot_pos - 3, pivot_pos - (l_size / 4 + 2));
                }
            }
            if r_size >= INSERTION_SORT_THRESHOLD {
                v.swap(pivot_pos + 1, pivot_pos + (1 + r_size / 4));
                v.swap(end - 1, end - r_size / 4);
                if r_size > NINTHER_THRESHOLD {
                    v.swap(pivot_pos + 2, pivot_pos + (2 + r_size / 4));
                    v.swap(pivot_pos + 3, pivot_pos + (3 + r_size / 4));
                    v.swap(end - 2, end - (1 + r_size / 4));
                    v.swap(end - 3, end - (2 + r_size / 4));
                }
            }
        } else if already_partitioned && partial_insertion_sort(v, begin, pivot_pos, less) && partial_insertion_sort(v, pivot_pos + 1, end, less) {
            return;
        }

        pdqsort_loop(v, begin, pivot_pos, less, bad_allowed, leftmost, branchless);
        begin = pivot_pos + 1;
        leftmost = false;
    }
}

/// libstdc++'s `__adjust_heap`: sift the hole at `hole` down to a leaf,
/// then push `value` back up from there.
fn adjust_heap<T: Copy>(v: &mut [T], mut hole: usize, len: usize, value: T, less: &mut impl FnMut(&T, &T) -> bool) {
    let top = hole;
    let mut second = hole;
    while second < (len - 1) / 2 {
        second = 2 * (second + 1);
        if less(&v[second], &v[second - 1]) {
            second -= 1;
        }
        v[hole] = v[second];
        hole = second;
    }
    if len & 1 == 0 && second == (len - 2) / 2 {
        second = 2 * (second + 1);
        v[hole] = v[second - 1];
        hole = second - 1;
    }
    // __push_heap
    let mut parent = hole.wrapping_sub(1) / 2;
    while hole > top && less(&v[parent], &value) {
        v[hole] = v[parent];
        hole = parent;
        parent = hole.wrapping_sub(1) / 2;
    }
    v[hole] = value;
}

fn make_heap<T: Copy>(v: &mut [T], less: &mut impl FnMut(&T, &T) -> bool) {
    let len = v.len();
    if len < 2 {
        return;
    }
    let mut parent = (len - 2) / 2;
    loop {
        let value = v[parent];
        adjust_heap(v, parent, len, value, less);
        if parent == 0 {
            return;
        }
        parent -= 1;
    }
}

fn sort_heap<T: Copy>(v: &mut [T], less: &mut impl FnMut(&T, &T) -> bool) {
    let mut last = v.len();
    while last > 1 {
        last -= 1;
        // __pop_heap
        let value = v[last];
        v[last] = v[0];
        adjust_heap(v, 0, last, value, less);
    }
}
