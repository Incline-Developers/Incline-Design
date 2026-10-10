//! The order of a categorical field's codes down the deposit, as most holes
//! put them, and the holes that disagree.
//!
//! Each hole votes for the order in which it first meets every pair of codes.
//! The votes are merged with Ranked Pairs (T. N. Tideman, 1987, "Independence
//! of clones as a criterion for voting rules", Social Choice and Welfare
//! 4:185-206): strongest majorities first, each kept unless a stronger chain
//! already runs the other way. The kept majorities are laid out with a
//! topological sort (A. B. Kahn, 1962, "Topological sorting of large
//! networks", Communications of the ACM 5:558-562).

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

use rayon::prelude::*;

use crate::model::drill_hole::{DrillFieldKind, DrillHoleDataset, DrillValue, OpenDrillHoleDataset, UNKNOWN_NAME};

/// Most codes a field may hold and still be ordered.
pub(crate) const MAX_ORDERED_CODES: usize = 1024;

/// Largest total size of the per-chunk pair tables, in counters.
const PAIR_TABLE_BUDGET: usize = 16 * 1024 * 1024;

/// How a hole disagrees with a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum FlagKind {
    /// Two codes in the wrong order, and no more.
    OutOfPlace,
    /// Three or more codes in a row in reverse order.
    Overturned,
    /// One code met again after another code came between.
    Repeat,
}

/// Whether a flag is about the groups (the parent field's values) or about
/// the names within one group, or within a set with no groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum FlagLevel {
    Group,
    Name,
}

/// One disagreement in one hole: the hole's index in the dataset, what kind,
/// at which level, and the codes involved top to bottom as the hole has
/// them (group names for a group-level flag).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HoleFlag {
    pub(crate) hole: usize,
    pub(crate) kind: FlagKind,
    pub(crate) level: FlagLevel,
    pub(crate) codes: Vec<String>,
}

/// A coarser categorical field each of the field's codes belongs to, and how
/// many interval rows went against that.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParentField {
    pub(crate) field: String,
    /// Rows of a parented code whose parent value is missing or is not the
    /// one most rows give that code. A code no row gives a parent is
    /// unparented, a group of its own, and its rows are not bad.
    pub(crate) bad_rows: usize,
    /// Rows holding a code of the field.
    pub(crate) rows: usize,
}

/// Keys a lithology is usually written under. A lithology field is never
/// taken as a parent, nor read as the strat field unless picked by hand.
pub(crate) const LITHOLOGY_KEYS: [&str; 6] = ["lith", "litho", "lithology", "lithtype", "lith_1", "rock"];

/// Whether `key` names a lithology field, by [`LITHOLOGY_KEYS`].
pub(crate) fn is_lithology_key(key: &str) -> bool {
    LITHOLOGY_KEYS.iter().any(|lithology| key.eq_ignore_ascii_case(lithology))
}

/// Whether `code` repeats another code `present` holds: that code with a
/// trailing R, as DRTR beside DRT. A repeat never enters a strat column; its
/// intervals keep their name. A code ending in R with no such base is not
/// one.
pub(crate) fn is_repeat_code(code: &str, present: impl Fn(&str) -> bool) -> bool {
    code.strip_suffix('R').is_some_and(|base| !base.is_empty() && present(base))
}

/// The field a hole's strat column reads in `dataset`: `chosen` while it is
/// one of the dataset's and still categorical, else
/// [`default_strat_field`]. The log and the strat column tab both read
/// through here, so the two never disagree.
pub(crate) fn strat_field_of<'a>(dataset: &'a OpenDrillHoleDataset, chosen: Option<&str>) -> Option<&'a crate::model::drill_hole::DrillField> {
    chosen
        .and_then(|key| dataset.dataset.field(key))
        .filter(|field| is_categorical(field))
        .or_else(|| default_strat_field(&dataset.dataset.fields, &dataset.color.working_sections, dataset.color.strat_field.as_deref()))
}

/// Parts of a key a stratigraphic field is usually written under, the most
/// detailed first.
const STRAT_KEY_HINTS: [&str; 3] = ["code", "seam", "ply"];

fn is_categorical(field: &crate::model::drill_hole::DrillField) -> bool {
    matches!(field.kind, crate::model::drill_hole::DrillFieldKind::Categorical { .. })
}

/// The strat field when none is picked: `recorded`, the field an import
/// noted (the parent, seam field when it found one); else the categorical
/// field holding working sections; else the first whose key holds a part of
/// [`STRAT_KEY_HINTS`], tried in order; else the first categorical field.
/// A lithology field is never taken unless it holds sections: it is read
/// only when picked by hand.
pub(crate) fn default_strat_field<'a>(
    fields: &'a [crate::model::drill_hole::DrillField],
    sections: &[crate::model::drill_hole::WorkingSection],
    recorded: Option<&str>,
) -> Option<&'a crate::model::drill_hole::DrillField> {
    let lithology = |field: &crate::model::drill_hole::DrillField| crate::model::strat_order::is_lithology_key(&field.key);
    let mut candidates = fields.iter().filter(|field| is_categorical(field));
    let noted = recorded.and_then(|key| fields.iter().find(|field| field.key == key && is_categorical(field) && !lithology(field)));
    let sectioned = || {
        fields
            .iter()
            .filter(|field| is_categorical(field))
            .find(|field| sections.iter().any(|section| section.field == field.key))
    };
    noted
        .or_else(sectioned)
        .or_else(|| {
            STRAT_KEY_HINTS.iter().find_map(|hint| {
                fields
                    .iter()
                    .filter(|field| is_categorical(field) && !lithology(field))
                    .find(|field| field.key.to_ascii_lowercase().contains(hint))
            })
        })
        .or_else(|| candidates.find(|field| !lithology(field)))
}

/// A parent field is taken when at most one row in this many goes against it.
pub(crate) const PARENT_TOLERANCE_ONE_IN: usize = 50;

/// One parent value and its codes top first; `name` is `None` for a code no
/// row gives a parent, which stands as a group of its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Group {
    pub(crate) name: Option<String>,
    pub(crate) codes: Vec<String>,
}

/// The majority order of a field and the holes that disagree with it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MajorityOrder {
    /// Every code the field holds in any hole, top first, repeats aside.
    pub(crate) order: Vec<String>,
    /// The holes that disagree with `order`, by hole then depth.
    pub(crate) flags: Vec<HoleFlag>,
    /// Majorities set aside because a stronger chain of majorities ran the
    /// other way.
    pub(crate) overruled: usize,
    /// Holes holding at least one code of the field.
    pub(crate) holes: usize,
    /// The parent field the order was built under, if one was found.
    pub(crate) parent: Option<ParentField>,
    /// The groups top first, each with its codes top first; `order` is
    /// their codes end to end. Empty when there is no parent.
    pub(crate) groups: Vec<Group>,
    /// Of `overruled`, the majorities between groups.
    pub(crate) group_overruled: usize,
}

/// One working section of `field` per named group, called after the group
/// and holding its codes. A group whose name a section could not carry (a
/// code of another group, or a name already taken) is left out.
pub(crate) fn seeded_sections(found: &MajorityOrder, field: &str) -> Vec<crate::model::drill_hole::WorkingSection> {
    let lower = |text: &str| text.trim().chars().flat_map(char::to_lowercase).collect::<String>();
    let mut sections: Vec<crate::model::drill_hole::WorkingSection> = Vec::new();
    for (at, group) in found.groups.iter().enumerate() {
        let Some(name) = group.name.as_deref().map(str::trim).filter(|name| !name.is_empty()) else {
            continue;
        };
        let key = lower(name);
        let is_foreign_code = found
            .groups
            .iter()
            .enumerate()
            .any(|(other, rest)| other != at && rest.codes.iter().any(|code| lower(code) == key));
        if is_foreign_code || sections.iter().any(|section| lower(&section.name) == key) {
            continue;
        }
        sections.push(crate::model::drill_hole::WorkingSection {
            name: name.to_owned(),
            field: field.to_owned(),
            codes: group.codes.clone(),
        });
    }
    sections
}

/// Per code of `field` seen in the intervals, how many rows gave each value of
/// a candidate field, and how many gave none.
struct Tally<'a> {
    values: Vec<HashMap<&'a str, u32>>,
    missing: Vec<u32>,
}

impl<'a> Tally<'a> {
    fn new(n: usize) -> Self {
        Self {
            values: vec![HashMap::new(); n],
            missing: vec![0; n],
        }
    }

    fn merge(mut self, other: Self) -> Self {
        for (mine, theirs) in self.values.iter_mut().zip(other.values) {
            for (value, count) in theirs {
                *mine.entry(value).or_insert(0) += count;
            }
        }
        for (mine, theirs) in self.missing.iter_mut().zip(other.missing) {
            *mine += theirs;
        }
        self
    }
}

/// A candidate parent so far: bad rows, parent values, key, summary, and each
/// code's parent.
type Candidate<'a> = (usize, usize, &'a str, ParentField, HashMap<&'a str, &'a str>);

/// The parent field, and for each code of `field` that has one, its parent
/// value.
fn parent_detail<'a>(dataset: &'a DrillHoleDataset, field: &str) -> Option<(ParentField, HashMap<&'a str, &'a str>)> {
    let code_of = |interval: &'a crate::model::drill_hole::DrillInterval| -> Option<&'a str> {
        if !interval.from.is_finite() || !interval.to.is_finite() {
            return None;
        }
        match interval.values.get(field) {
            Some(DrillValue::Category(code)) if !code.trim().is_empty() => Some(code.as_str()),
            _ => None,
        }
    };
    let mut names: Vec<&'a str> = dataset
        .holes
        .par_iter()
        .fold(std::collections::HashSet::<&'a str>::new, |mut set, hole| {
            set.extend(hole.intervals.iter().filter_map(code_of));
            set
        })
        .reduce(std::collections::HashSet::new, |mut a, b| {
            a.extend(b);
            a
        })
        .into_iter()
        .collect();
    if names.len() < 3 {
        return None;
    }
    names.sort_by(|a, b| crate::natural_sort::natural_cmp(a, b));
    let map: HashMap<&str, u32> = names.iter().enumerate().map(|(at, name)| (*name, at as u32)).collect();
    let n = names.len();

    let mut best: Option<Candidate<'a>> = None;
    for candidate in &dataset.fields {
        if candidate.key == field || is_lithology_key(&candidate.key) || !matches!(candidate.kind, DrillFieldKind::Categorical { .. }) {
            continue;
        }
        let key = candidate.key.as_str();
        let tally = dataset
            .holes
            .par_iter()
            .fold(
                || Tally::new(n),
                |mut tally, hole| {
                    for interval in &hole.intervals {
                        let Some(code) = code_of(interval) else { continue };
                        let Some(&at) = map.get(code) else { continue };
                        match interval.values.get(key) {
                            Some(DrillValue::Category(value)) if !value.trim().is_empty() && value != UNKNOWN_NAME => {
                                *tally.values[at as usize].entry(value.as_str()).or_insert(0) += 1;
                            }
                            _ => tally.missing[at as usize] += 1,
                        }
                    }
                    tally
                },
            )
            .reduce(|| Tally::new(n), Tally::merge);
        // A repeat parent value counts as the value it repeats.
        let values: std::collections::HashSet<&'a str> = tally.values.iter().flat_map(|counts| counts.keys().copied()).collect();
        let tally = Tally {
            values: tally
                .values
                .into_iter()
                .map(|counts| {
                    let mut merged: HashMap<&'a str, u32> = HashMap::with_capacity(counts.len());
                    for (value, count) in counts {
                        let value = match value.strip_suffix('R').and_then(|base| values.get(base)) {
                            Some(&base) if !base.is_empty() => base,
                            _ => value,
                        };
                        *merged.entry(value).or_insert(0) += count;
                    }
                    merged
                })
                .collect(),
            missing: tally.missing,
        };
        let mut rows = 0usize;
        let mut parented = 0usize;
        let mut good = 0usize;
        let mut codes = 0usize;
        let mut parents: HashMap<&'a str, &'a str> = HashMap::new();
        for code in 0..n {
            let total: usize = tally.values[code].values().map(|&count| count as usize).sum::<usize>() + tally.missing[code] as usize;
            if total == 0 {
                continue;
            }
            codes += 1;
            rows += total;
            let top = tally.values[code]
                .iter()
                .min_by(|a, b| b.1.cmp(a.1).then_with(|| crate::natural_sort::natural_cmp(a.0, b.0)));
            if let Some((&value, &count)) = top {
                good += count as usize;
                parented += total;
                parents.insert(names[code], value);
            }
        }
        let bad_rows = parented - good;
        let distinct: std::collections::HashSet<&str> = parents.values().copied().collect();
        // Most rows must name a parent, or the field is not one.
        if rows == 0 || parented * 2 < rows || bad_rows * PARENT_TOLERANCE_ONE_IN > rows || distinct.len() < 2 || distinct.len() >= codes {
            continue;
        }
        let better = match &best {
            None => true,
            Some((bad, count, other, _, _)) => bad_rows
                .cmp(bad)
                .then_with(|| count.cmp(&distinct.len()))
                .then_with(|| crate::natural_sort::natural_cmp(key, other))
                .is_lt(),
        };
        if better {
            let info = ParentField {
                field: candidate.key.clone(),
                bad_rows,
                rows,
            };
            best = Some((bad_rows, distinct.len(), key, info, parents));
        }
    }
    best.map(|(_, _, _, info, parents)| (info, parents))
}

/// The groups of `names` under `parents`: each code's group index, each
/// group's label (the parent value, or the code of a group of its own) and
/// whether it is a named group.
fn assign_groups<'a>(names: &[&'a str], parents: &HashMap<&'a str, &'a str>) -> (Vec<u32>, Vec<&'a str>, Vec<bool>) {
    let mut ids: HashMap<&str, u32> = HashMap::new();
    let mut labels: Vec<&'a str> = Vec::new();
    let mut named: Vec<bool> = Vec::new();
    let mut group_of = Vec::with_capacity(names.len());
    for &name in names {
        let id = match parents.get(name) {
            Some(&parent) => *ids.entry(parent).or_insert_with(|| {
                labels.push(parent);
                named.push(true);
                labels.len() as u32 - 1
            }),
            None => {
                labels.push(name);
                named.push(false);
                labels.len() as u32 - 1
            }
        };
        group_of.push(id);
    }
    (group_of, labels, named)
}

/// What two-level flagging needs: each code's group, each group's label and
/// place, each code's place within its group, and the code names.
struct Layout<'a> {
    group_of: &'a [u32],
    group_rank: &'a [u32],
    group_labels: &'a [&'a str],
    code_rank: &'a [u32],
    names: &'a [&'a str],
}

const NONE: u32 = u32::MAX;

/// One hole's codes top to bottom as indices, with runs of one code joined
/// and codes missing from `map` passed over. The flag says one was missing,
/// UNK and a repeat of a code in `map` aside.
fn hole_codes(hole: &crate::model::drill_hole::DrillHole, field: &str, map: &HashMap<&str, u32>, repeats: &RepeatRows) -> (Vec<u32>, bool) {
    let mut items: Vec<(f64, u32)> = Vec::with_capacity(hole.intervals.len());
    let mut missing = false;
    for interval in &hole.intervals {
        if !interval.from.is_finite() || !interval.to.is_finite() || repeats.holds(interval) {
            continue;
        }
        let Some(DrillValue::Category(code)) = interval.values.get(field) else {
            continue;
        };
        if code.trim().is_empty() {
            continue;
        }
        match map.get(code.as_str()) {
            Some(&index) => items.push((interval.from, index)),
            None if code == UNKNOWN_NAME || is_repeat_code(code, |base| map.contains_key(base)) => {}
            None => missing = true,
        }
    }
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut codes: Vec<u32> = items.into_iter().map(|item| item.1).collect();
    codes.dedup();
    (codes, missing)
}

/// The rows of a repeat parent value: the parent field and its values that
/// repeat another of its values. Such rows are passed over in ordering and
/// checking, as a repeat code is.
#[derive(Default)]
struct RepeatRows<'a> {
    field: Option<String>,
    values: std::collections::HashSet<&'a str>,
}

impl<'a> RepeatRows<'a> {
    fn of(dataset: &'a DrillHoleDataset, parent: Option<&ParentField>) -> Self {
        let Some(parent) = parent else {
            return Self::default();
        };
        let values = match dataset.field(&parent.field).map(|found| &found.kind) {
            Some(DrillFieldKind::Categorical { categories }) => {
                let declared: std::collections::HashSet<&str> = categories.iter().map(String::as_str).collect();
                declared.iter().copied().filter(|value| is_repeat_code(value, |base| declared.contains(base))).collect()
            }
            _ => std::collections::HashSet::new(),
        };
        Self {
            field: Some(parent.field.clone()),
            values,
        }
    }

    fn holds(&self, interval: &crate::model::drill_hole::DrillInterval) -> bool {
        self.field
            .as_ref()
            .is_some_and(|field| matches!(interval.values.get(field), Some(DrillValue::Category(value)) if self.values.contains(value.as_str())))
    }
}

fn all_codes(dataset: &DrillHoleDataset, field: &str, map: &HashMap<&str, u32>, repeats: &RepeatRows) -> (Vec<Vec<u32>>, bool) {
    let found: Vec<(Vec<u32>, bool)> = dataset.holes.par_iter().map(|hole| hole_codes(hole, field, map, repeats)).collect();
    let missing = found.iter().any(|item| item.1);
    (found.into_iter().map(|item| item.0).collect(), missing)
}

/// `names` indexed in first-met order, blanks, UNK and repeats of another
/// of them left out.
fn index_of<'a>(names: impl Iterator<Item = &'a str>) -> (HashMap<&'a str, u32>, Vec<&'a str>) {
    let mut seen = std::collections::HashSet::new();
    let mut list = Vec::new();
    for name in names {
        if !name.trim().is_empty() && name != UNKNOWN_NAME && seen.insert(name) {
            list.push(name);
        }
    }
    list.retain(|name| !is_repeat_code(name, |base| seen.contains(base)));
    let map = list.iter().enumerate().map(|(at, name)| (*name, at as u32)).collect();
    (map, list)
}

/// Every code of `field` found in the intervals, in first-met order.
fn collect_names<'a>(dataset: &'a DrillHoleDataset, field: &str) -> (HashMap<&'a str, u32>, Vec<&'a str>) {
    let names = dataset
        .holes
        .iter()
        .flat_map(|hole| &hole.intervals)
        .filter_map(|interval| match interval.values.get(field) {
            Some(DrillValue::Category(code)) => Some(code.as_str()),
            _ => None,
        });
    index_of(names)
}

/// The order most holes agree on for `field`, and the holes that disagree.
/// `None` when the field holds more than [`MAX_ORDERED_CODES`] codes.
pub(crate) fn majority_order(dataset: &DrillHoleDataset, field: &str) -> Option<MajorityOrder> {
    let declared = dataset.field(field).and_then(|found| match &found.kind {
        DrillFieldKind::Categorical { categories } => Some(index_of(categories.iter().map(String::as_str))),
        DrillFieldKind::Numeric { .. } => None,
    });
    let parent = parent_detail(dataset, field);
    let repeats = RepeatRows::of(dataset, parent.as_ref().map(|found| &found.0));
    let mut found = None;
    if let Some((map, names)) = declared {
        if names.len() > MAX_ORDERED_CODES {
            return None;
        }
        let (sequences, missing) = all_codes(dataset, field, &map, &repeats);
        if !missing {
            found = Some((names, sequences));
        }
    }
    let (names, sequences) = match found {
        Some(found) => found,
        None => {
            let (map, names) = collect_names(dataset, field);
            if names.len() > MAX_ORDERED_CODES {
                return None;
            }
            let (sequences, _) = all_codes(dataset, field, &map, &repeats);
            (names, sequences)
        }
    };
    let n = names.len();

    let (above, position_sum, seen, holes) = pair_counts(&sequences, n);
    let Some((info, parents)) = parent else {
        let (order, overruled) = ranked_order(&names, &above, &position_sum, &seen);
        let mut rank = vec![NONE; n];
        for (place, &code) in order.iter().enumerate() {
            rank[code as usize] = place as u32;
        }
        let flags = flag_sequences(&sequences, &rank, &names);
        return Some(MajorityOrder {
            order: order.iter().map(|&code| names[code as usize].to_owned()).collect(),
            flags,
            overruled,
            holes,
            ..MajorityOrder::default()
        });
    };

    let (group_of, labels, named) = assign_groups(&names, &parents);
    let g = labels.len();
    let group_sequences: Vec<Vec<u32>> = sequences
        .par_iter()
        .map(|codes| {
            let mut groups: Vec<u32> = codes.iter().map(|&code| group_of[code as usize]).collect();
            groups.dedup();
            groups
        })
        .collect();
    let (g_above, g_position, g_seen, _) = pair_counts(&group_sequences, g);
    let (group_order, group_overruled) = ranked_order(&labels, &g_above, &g_position, &g_seen);

    let mut members: Vec<Vec<u32>> = vec![Vec::new(); g];
    for code in 0..n {
        if seen[code] > 0 {
            members[group_of[code] as usize].push(code as u32);
        }
    }
    let mut group_rank = vec![NONE; g];
    let mut code_rank = vec![NONE; n];
    let mut overruled = group_overruled;
    let mut order = Vec::new();
    let mut groups = Vec::with_capacity(group_order.len());
    for (place, &id) in group_order.iter().enumerate() {
        group_rank[id as usize] = place as u32;
        let list = &members[id as usize];
        let m = list.len();
        let mut sub_above = vec![0u32; m * m];
        for (i, &a) in list.iter().enumerate() {
            for (j, &b) in list.iter().enumerate() {
                sub_above[i * m + j] = above[a as usize * n + b as usize];
            }
        }
        let sub_names: Vec<&str> = list.iter().map(|&code| names[code as usize]).collect();
        let sub_position: Vec<f64> = list.iter().map(|&code| position_sum[code as usize]).collect();
        let sub_seen: Vec<u32> = list.iter().map(|&code| seen[code as usize]).collect();
        let (local, set_aside) = ranked_order(&sub_names, &sub_above, &sub_position, &sub_seen);
        overruled += set_aside;
        let mut codes = Vec::with_capacity(local.len());
        for (at, &index) in local.iter().enumerate() {
            let code = list[index as usize];
            code_rank[code as usize] = at as u32;
            codes.push(names[code as usize].to_owned());
        }
        order.extend(codes.iter().cloned());
        groups.push(Group {
            name: named[id as usize].then(|| labels[id as usize].to_owned()),
            codes,
        });
    }
    let layout = Layout {
        group_of: &group_of,
        group_rank: &group_rank,
        group_labels: &labels,
        code_rank: &code_rank,
        names: &names,
    };
    let flags = flag_two_level(&sequences, &layout);
    Some(MajorityOrder {
        order,
        flags,
        overruled,
        holes,
        parent: Some(info),
        groups,
        group_overruled,
    })
}

/// The holes that disagree with `column` for `field`. Codes the column does
/// not hold, repeats it holds and rows of a repeat parent are passed over.
/// Under a parent field, groups are judged first and the codes within each
/// group after.
pub(crate) fn flags_against(dataset: &DrillHoleDataset, field: &str, column: &[String]) -> Vec<HoleFlag> {
    let (map, names) = index_of(column.iter().map(String::as_str));
    let rank: Vec<u32> = (0..names.len() as u32).collect();
    let parent = parent_detail(dataset, field);
    let repeats = RepeatRows::of(dataset, parent.as_ref().map(|found| &found.0));
    let (sequences, _) = all_codes(dataset, field, &map, &repeats);
    let Some((_, parents)) = parent else {
        return flag_sequences(&sequences, &rank, &names);
    };
    let (group_of, labels, _) = assign_groups(&names, &parents);
    let mut group_rank = vec![NONE; labels.len()];
    for (code, &id) in group_of.iter().enumerate() {
        group_rank[id as usize] = group_rank[id as usize].min(code as u32);
    }
    let layout = Layout {
        group_of: &group_of,
        group_rank: &group_rank,
        group_labels: &labels,
        code_rank: &rank,
        names: &names,
    };
    flag_two_level(&sequences, &layout)
}

/// One chunk's or the whole set's counts, see [`pair_counts`].
type PairCounts = (Vec<u32>, Vec<f64>, Vec<u32>, usize);

/// For every ordered pair of codes, in how many holes the first is met above
/// the second; the summed relative position of each code; how often each was
/// met; and the number of holes holding any code.
fn pair_counts(sequences: &[Vec<u32>], n: usize) -> PairCounts {
    let table = (n * n).max(1);
    let chunks = (rayon::current_num_threads() * 2).min(PAIR_TABLE_BUDGET / table).max(1);
    let size = sequences.len().div_ceil(chunks).max(1);
    let parts: Vec<PairCounts> = sequences
        .par_chunks(size)
        .map(|part| {
            let mut above = vec![0u32; n * n];
            let mut position = vec![0.0f64; n];
            let mut seen = vec![0u32; n];
            let mut stamp = vec![0usize; n];
            let mut firsts: Vec<u32> = Vec::new();
            let mut holes = 0;
            for (number, codes) in part.iter().enumerate() {
                if codes.is_empty() {
                    continue;
                }
                holes += 1;
                firsts.clear();
                for (place, &code) in codes.iter().enumerate() {
                    let code = code as usize;
                    if stamp[code] == number + 1 {
                        continue;
                    }
                    stamp[code] = number + 1;
                    firsts.push(code as u32);
                    position[code] += (place as f64 + 0.5) / codes.len() as f64;
                    seen[code] += 1;
                }
                for (a, &upper) in firsts.iter().enumerate() {
                    let row = upper as usize * n;
                    for &lower in &firsts[a + 1..] {
                        above[row + lower as usize] += 1;
                    }
                }
            }
            (above, position, seen, holes)
        })
        .collect();
    let mut total: PairCounts = (vec![0u32; n * n], vec![0.0f64; n], vec![0u32; n], 0);
    for (above, position, seen, holes) in parts {
        for (sum, add) in total.0.iter_mut().zip(&above) {
            *sum += add;
        }
        for (sum, add) in total.1.iter_mut().zip(&position) {
            *sum += add;
        }
        for (sum, add) in total.2.iter_mut().zip(&seen) {
            *sum += add;
        }
        total.3 += holes;
    }
    total
}

/// Ranked Pairs over the pair counts, then a topological sort that breaks
/// ties by mean position and natural name. Returns the codes top first and
/// the number of majorities set aside.
fn ranked_order(names: &[&str], above: &[u32], position_sum: &[f64], seen: &[u32]) -> (Vec<u32>, usize) {
    let n = names.len();
    let mut edges: Vec<(u32, u32, u32, u32)> = Vec::new();
    for a in 0..n {
        for b in 0..n {
            let (win, lose) = (above[a * n + b], above[b * n + a]);
            if win > lose {
                edges.push((a as u32, b as u32, win, lose));
            }
        }
    }
    edges.sort_by(|x, y| (y.2 - y.3).cmp(&(x.2 - x.3)).then(y.2.cmp(&x.2)).then(x.0.cmp(&y.0)).then(x.1.cmp(&y.1)));

    let words = n.div_ceil(64).max(1);
    let mut reach = vec![0u64; n * words];
    let bit = |row: &[u64], code: usize| row[code / 64] >> (code % 64) & 1 == 1;
    let mut followers: Vec<Vec<u32>> = vec![Vec::new(); n];
    let mut inbound = vec![0u32; n];
    let mut overruled = 0;
    for &(a, b, _, _) in &edges {
        let (a, b) = (a as usize, b as usize);
        if bit(&reach[b * words..(b + 1) * words], a) {
            overruled += 1;
            continue;
        }
        followers[a].push(b as u32);
        inbound[b] += 1;
        if bit(&reach[a * words..(a + 1) * words], b) {
            continue;
        }
        let mut below: Vec<u64> = reach[b * words..(b + 1) * words].to_vec();
        below[b / 64] |= 1 << (b % 64);
        for x in 0..n {
            if x == a || bit(&reach[x * words..(x + 1) * words], a) {
                for (word, add) in reach[x * words..(x + 1) * words].iter_mut().zip(&below) {
                    *word |= add;
                }
            }
        }
    }

    let mean = |code: usize| position_sum[code] / seen[code].max(1) as f64;
    let mut used: Vec<u32> = (0..n as u32).filter(|&code| seen[code as usize] > 0).collect();
    used.sort_by(|&x, &y| {
        mean(x as usize)
            .total_cmp(&mean(y as usize))
            .then_with(|| crate::natural_sort::natural_cmp(names[x as usize], names[y as usize]))
    });
    let mut tie = vec![0u32; n];
    for (place, &code) in used.iter().enumerate() {
        tie[code as usize] = place as u32;
    }
    let mut ready: BinaryHeap<Reverse<(u32, u32)>> = used
        .iter()
        .filter(|&&code| inbound[code as usize] == 0)
        .map(|&code| Reverse((tie[code as usize], code)))
        .collect();
    let mut order = Vec::with_capacity(used.len());
    while let Some(Reverse((_, code))) = ready.pop() {
        order.push(code);
        for &next in &followers[code as usize] {
            inbound[next as usize] -= 1;
            if inbound[next as usize] == 0 {
                ready.push(Reverse((tie[next as usize], next)));
            }
        }
    }
    (order, overruled)
}

/// Flags for every hole against `rank`, which gives each code index its place
/// in the column. Holes are worked in parallel and returned in hole order.
fn flag_sequences(sequences: &[Vec<u32>], rank: &[u32], names: &[&str]) -> Vec<HoleFlag> {
    let per_hole: Vec<Vec<HoleFlag>> = sequences
        .par_iter()
        .enumerate()
        .map(|(hole, codes)| hole_flags(hole, codes, rank, names, FlagLevel::Name))
        .collect();
    per_hole.into_iter().flatten().collect()
}

/// Two-level flags for every hole: the hole's groups against the group order,
/// then, group by group, its codes against the order within the group.
fn flag_two_level(sequences: &[Vec<u32>], layout: &Layout) -> Vec<HoleFlag> {
    let per_hole: Vec<Vec<HoleFlag>> = sequences
        .par_iter()
        .enumerate()
        .map(|(hole, codes)| {
            if codes.len() < 2 {
                return Vec::new();
            }
            let mut groups: Vec<u32> = codes.iter().map(|&code| layout.group_of[code as usize]).collect();
            groups.dedup();
            let mut flags = hole_flags(hole, &groups, layout.group_rank, layout.group_labels, FlagLevel::Group);
            let mut tagged: Vec<(u32, u32)> = codes.iter().map(|&code| (layout.group_rank[layout.group_of[code as usize] as usize], code)).collect();
            tagged.sort_by_key(|item| item.0);
            let mut start = 0;
            while start < tagged.len() {
                let end = start + tagged[start..].iter().take_while(|item| item.0 == tagged[start].0).count();
                let mut part: Vec<u32> = tagged[start..end].iter().map(|item| item.1).collect();
                part.dedup();
                flags.extend(hole_flags(hole, &part, layout.code_rank, layout.names, FlagLevel::Name));
                start = end;
            }
            flags
        })
        .collect();
    per_hole.into_iter().flatten().collect()
}

fn hole_flags(hole: usize, codes: &[u32], rank: &[u32], names: &[&str], level: FlagLevel) -> Vec<HoleFlag> {
    let mut flags = Vec::new();
    if codes.len() < 2 {
        return flags;
    }
    let name = |code: u32| names[code as usize].to_owned();
    let mut sorted = codes.to_vec();
    sorted.sort_unstable();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        let mut done: Vec<u32> = Vec::new();
        for &code in codes {
            if sorted
                .binary_search(&code)
                .map(|at| sorted.get(at + 1) == Some(&code) || (at > 0 && sorted[at - 1] == code))
                .unwrap_or(false)
                && !done.contains(&code)
            {
                done.push(code);
                flags.push(HoleFlag {
                    hole,
                    kind: FlagKind::Repeat,
                    level,
                    codes: vec![name(code)],
                });
            }
        }
    }
    let down = |at: usize| rank[codes[at + 1] as usize] < rank[codes[at] as usize];
    let mut start = 0;
    while start + 1 < codes.len() {
        if !down(start) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end + 1 < codes.len() && down(end) {
            end += 1;
        }
        let kind = if end - start == 1 { FlagKind::OutOfPlace } else { FlagKind::Overturned };
        flags.push(HoleFlag {
            hole,
            kind,
            level,
            codes: codes[start..=end].iter().map(|&code| name(code)).collect(),
        });
        start = end;
    }
    flags
}
