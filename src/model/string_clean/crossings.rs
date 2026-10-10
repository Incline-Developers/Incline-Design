//! Step 4 of Clean Strings, and Join all at halfway: crossings between the
//! strings of one layer.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use glam::{DVec2, DVec3};

use super::{Cancelled, Change, ChangeKind, Piece, stop_if};
use crate::model::{
    control_checks::{BoxGrid, crossing_side, segment_box},
    kernel::{self, SegSeg},
    rbf::MERGE_DISTANCE,
};

/// How far a height may differ from its target and still count as already
/// there, so a mean of equal heights is never taken for a change.
const HEIGHT_NOISE: f64 = 1e-9;

/// One place a segment of one piece meets a segment of a later piece, with
/// both heights as the strings stand before anything changes.
struct Contact {
    point: DVec2,
    a: usize,
    i: usize,
    b: usize,
    j: usize,
    za: f64,
    zb: f64,
}

/// One meeting place: the contacts within the merge distance of each other,
/// where the shared vertex goes and the heights of the strings meeting there.
struct Cluster {
    contacts: Vec<usize>,
    at: DVec2,
    /// The strings meeting here in piece order, each with the height it
    /// reads from its first contact.
    heights: Vec<(usize, f64)>,
    miss: f64,
    mean: f64,
}

/// A vertex one piece gets for one cluster.
enum Op {
    /// The piece's own vertex at the place, moved onto it in plan.
    Existing(usize),
    /// A new vertex in `segment` at the fraction `along`, at `z`.
    Insert { segment: usize, along: f64, z: f64 },
}

/// Put a shared vertex at every place two strings meet, and set the shared
/// vertices to their mean height where the strings miss by `join_up_to` or
/// less. With `only_near`, only meetings within the merge distance of one
/// of those positions are touched. `Err` once `cancelled` says so,
/// nothing changed.
pub(super) fn share_crossings(
    pieces: &mut [Piece],
    join_up_to: f64,
    only_near: Option<&[DVec2]>,
    changes: &mut Vec<Change>,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Cancelled> {
    let contacts = find_contacts(pieces, cancelled)?;
    if contacts.is_empty() {
        return Ok(());
    }
    let groups = group_contacts(pieces, &contacts);
    stop_if(cancelled)?;
    let clusters: Vec<Cluster> = groups
        .into_iter()
        .map(|group| build_cluster(pieces, &contacts, group))
        .filter(|cluster| only_near.is_none_or(|targets| targets.iter().any(|&target| near_cluster(cluster, &contacts, target))))
        .collect();
    if clusters.is_empty() {
        return Ok(());
    }
    stop_if(cancelled)?;

    // Everything is read on the strings as they stand, then applied piece by
    // piece, so no vertex placed for one meeting moves another.
    let mut ops: BTreeMap<usize, Vec<(usize, Op)>> = BTreeMap::new();
    for (index, cluster) in clusters.iter().enumerate() {
        for &(piece, _) in &cluster.heights {
            ops.entry(piece)
                .or_default()
                .extend(piece_ops(pieces, &contacts, cluster, piece).into_iter().map(|op| (index, op)));
        }
    }
    let mut changed = vec![false; clusters.len()];
    let mut placed: Vec<Vec<(usize, usize)>> = vec![Vec::new(); clusters.len()];
    for (piece, ops) in ops {
        apply_ops(&mut pieces[piece].verts, piece, ops, &clusters, &mut changed, &mut placed);
    }

    for (index, cluster) in clusters.iter().enumerate() {
        let joined = cluster.miss <= join_up_to + HEIGHT_NOISE;
        if joined {
            for &(piece, vertex) in &placed[index] {
                let z = &mut pieces[piece].verts[vertex].z;
                if (*z - cluster.mean).abs() > HEIGHT_NOISE {
                    changed[index] = true;
                }
                *z = cluster.mean;
            }
        }
        if !changed[index] {
            continue;
        }
        let sources: Vec<usize> = cluster.heights.iter().map(|&(piece, _)| pieces[piece].source).collect();
        let others = sources[1..].to_vec();
        let (z, kind) = if joined {
            (cluster.mean, ChangeKind::Joined { others, miss: cluster.miss })
        } else {
            (cluster.heights[0].1, ChangeKind::VertexShared { others, miss: cluster.miss })
        };
        changes.push(Change {
            source: sources[0],
            at: cluster.at.extend(z),
            kind,
        });
    }
    Ok(())
}

/// Every crossing or touching between a segment of one piece and a segment
/// of a later piece, in the order of the pieces and segments meeting.
fn find_contacts(pieces: &[Piece], cancelled: &dyn Fn() -> bool) -> Result<Vec<Contact>, Cancelled> {
    let segments: Vec<(usize, usize)> = pieces
        .iter()
        .enumerate()
        .flat_map(|(piece, one)| (0..one.verts.len().saturating_sub(1)).map(move |segment| (piece, segment)))
        .collect();
    let grid = BoxGrid::new(
        segments
            .iter()
            .map(|&(piece, segment)| segment_box(pieces[piece].verts[segment].truncate(), pieces[piece].verts[segment + 1].truncate()))
            .collect(),
    );
    let mut contacts = Vec::new();
    let mut near = Vec::new();
    for (a, first) in pieces.iter().enumerate() {
        stop_if(cancelled)?;
        for i in 0..first.verts.len().saturating_sub(1) {
            let (low, high) = segment_box(first.verts[i].truncate(), first.verts[i + 1].truncate());
            grid.overlapping(low, high, &mut near);
            for &other in &near {
                let (b, j) = segments[other];
                if b <= a {
                    continue;
                }
                let second = &pieces[b].verts;
                let meeting = kernel::segment_segment(first.verts[i].truncate(), first.verts[i + 1].truncate(), second[j].truncate(), second[j + 1].truncate());
                let (point, t, u) = match meeting {
                    SegSeg::Crossing { point, t, u } | SegSeg::Touching { point, t, u } => (point, t, u),
                    SegSeg::Disjoint | SegSeg::CollinearOverlap { .. } => continue,
                };
                let za = crossing_side(&first.verts, a, i, t, point).1;
                let zb = crossing_side(second, b, j, u, point).1;
                contacts.push(Contact { point, a, i, b, j, za, zb });
            }
        }
    }
    contacts.sort_by_key(|contact| (contact.a, contact.i, contact.b, contact.j));
    Ok(contacts)
}

/// The contacts grouped into meeting places, each group ascending and the
/// groups in order of their first contact. Contacts within the merge
/// distance are one place; two places that would use the same existing
/// vertex are one too.
fn group_contacts(pieces: &[Piece], contacts: &[Contact]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..contacts.len()).collect();
    let mut by_x: Vec<usize> = (0..contacts.len()).collect();
    by_x.sort_by(|&one, &two| contacts[one].point.x.total_cmp(&contacts[two].point.x));
    for (position, &one) in by_x.iter().enumerate() {
        for &two in &by_x[position + 1..] {
            if contacts[two].point.x - contacts[one].point.x >= MERGE_DISTANCE {
                break;
            }
            if contacts[one].point.distance(contacts[two].point) < MERGE_DISTANCE {
                join(&mut parent, one, two);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for index in 0..contacts.len() {
        let root = find(&mut parent, index);
        groups.entry(root).or_default().push(index);
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().collect();
    groups.sort_by_key(|group| group[0]);
    merge_by_anchor(pieces, contacts, groups)
}

/// Groups filed by the vertex they would use, merged where two use the same
/// one. The pair merged next is always the first group, in order of first
/// contact, that shares its vertex with a later group, and the first such
/// later one; the later joins the earlier. The earlier keeps its first
/// contact, so its reach stays, and the vertex it uses can only move to a
/// lower one.
fn merge_by_anchor(pieces: &[Piece], contacts: &[Contact], groups: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
    let mut anchors: Vec<Option<(usize, usize)>> = groups.iter().map(|group| anchor(pieces, contacts, group)).collect();
    let mut groups: Vec<Option<Vec<usize>>> = groups.into_iter().map(Some).collect();
    let mut users = AnchorUsers::default();
    for (index, found) in anchors.iter().enumerate() {
        if let &Some(found) = found {
            users.add(found, index);
        }
    }
    while let Some(&(one, found)) = users.shared.first() {
        let two = users.second(found);
        users.remove(found, two);
        let merged = groups[two].take().unwrap();
        let reach = contacts[groups[one].as_ref().unwrap()[0]].point;
        let moved = anchor_among(pieces, contacts, &merged, reach).filter(|&moved| moved < found);
        groups[one].as_mut().unwrap().extend(merged);
        anchors[two] = None;
        if let Some(moved) = moved {
            users.remove(found, one);
            users.add(moved, one);
            anchors[one] = Some(moved);
        }
    }
    groups
        .into_iter()
        .flatten()
        .map(|mut group| {
            group.sort_unstable();
            group
        })
        .collect()
}

/// The groups using each vertex, and which vertices more than one uses, by
/// their first user.
#[derive(Default)]
struct AnchorUsers {
    by_vertex: HashMap<(usize, usize), BTreeSet<usize>>,
    shared: BTreeSet<(usize, (usize, usize))>,
}

impl AnchorUsers {
    fn add(&mut self, vertex: (usize, usize), group: usize) {
        let users = self.by_vertex.entry(vertex).or_default();
        if users.len() >= 2 {
            self.shared.remove(&(*users.first().unwrap(), vertex));
        }
        users.insert(group);
        if users.len() >= 2 {
            self.shared.insert((*users.first().unwrap(), vertex));
        }
    }

    fn remove(&mut self, vertex: (usize, usize), group: usize) {
        let users = self.by_vertex.get_mut(&vertex).unwrap();
        if users.len() >= 2 {
            self.shared.remove(&(*users.first().unwrap(), vertex));
        }
        users.remove(&group);
        if users.len() >= 2 {
            self.shared.insert((*users.first().unwrap(), vertex));
        }
    }

    /// The second group using the vertex.
    fn second(&self, vertex: (usize, usize)) -> usize {
        *self.by_vertex[&vertex].iter().nth(1).unwrap()
    }
}

fn find(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

fn join(parent: &mut [usize], one: usize, two: usize) {
    let (one, two) = (find(parent, one), find(parent, two));
    parent[one.max(two)] = one.min(two);
}

/// The existing vertex a meeting place sits on: the first, by piece then
/// position along it, among the ends of the segments that meet there and
/// within the merge distance of the first contact.
fn anchor(pieces: &[Piece], contacts: &[Contact], group: &[usize]) -> Option<(usize, usize)> {
    anchor_among(pieces, contacts, group, contacts[group[0]].point)
}

/// The same, judged on only some of a group's contacts, from `first`.
fn anchor_among(pieces: &[Piece], contacts: &[Contact], among: &[usize], first: DVec2) -> Option<(usize, usize)> {
    among
        .iter()
        .flat_map(|&index| {
            let contact = &contacts[index];
            [(contact.a, contact.i), (contact.b, contact.j)]
        })
        .flat_map(|(piece, segment)| [(piece, segment), (piece, segment + 1)])
        .filter(|&(piece, vertex)| pieces[piece].verts[vertex].truncate().distance(first) < MERGE_DISTANCE)
        .min()
}

fn build_cluster(pieces: &[Piece], contacts: &[Contact], group: Vec<usize>) -> Cluster {
    let at = match anchor(pieces, contacts, &group) {
        Some((piece, vertex)) => pieces[piece].verts[vertex].truncate(),
        None => contacts[group[0]].point,
    };
    let mut heights: BTreeMap<usize, f64> = BTreeMap::new();
    for &index in &group {
        let contact = &contacts[index];
        heights.entry(contact.a).or_insert(contact.za);
        heights.entry(contact.b).or_insert(contact.zb);
    }
    let heights: Vec<(usize, f64)> = heights.into_iter().collect();
    let low = heights.iter().map(|&(_, z)| z).fold(f64::INFINITY, f64::min);
    let high = heights.iter().map(|&(_, z)| z).fold(f64::NEG_INFINITY, f64::max);
    let mean = heights.iter().map(|&(_, z)| z).sum::<f64>() / heights.len() as f64;
    Cluster {
        contacts: group,
        at,
        heights,
        miss: high - low,
        mean,
    }
}

fn near_cluster(cluster: &Cluster, contacts: &[Contact], target: DVec2) -> bool {
    target.distance(cluster.at) <= MERGE_DISTANCE || cluster.contacts.iter().any(|&index| target.distance(contacts[index].point) <= MERGE_DISTANCE)
}

/// The vertices one piece gets at a meeting place: its own where a segment
/// meeting there ends within the merge distance of it, else one inserted in
/// the segment at the height of the string's own line.
fn piece_ops(pieces: &[Piece], contacts: &[Contact], cluster: &Cluster, piece: usize) -> Vec<Op> {
    let verts = &pieces[piece].verts;
    let segments: BTreeSet<usize> = cluster
        .contacts
        .iter()
        .flat_map(|&index| {
            let contact = &contacts[index];
            [(contact.a == piece).then_some(contact.i), (contact.b == piece).then_some(contact.j)]
        })
        .flatten()
        .collect();
    let mut taken = BTreeSet::new();
    let mut ops = Vec::new();
    for segment in segments {
        let end = [segment, segment + 1]
            .into_iter()
            .map(|vertex| (verts[vertex].truncate().distance(cluster.at), vertex))
            .filter(|&(distance, _)| distance < MERGE_DISTANCE)
            .min_by(|one, two| one.0.total_cmp(&two.0))
            .map(|(_, vertex)| vertex);
        match end {
            Some(vertex) => {
                if taken.insert(vertex) {
                    ops.push(Op::Existing(vertex));
                }
            }
            None => {
                let along = kernel::project_onto_segment(cluster.at, verts[segment].truncate(), verts[segment + 1].truncate()).1;
                ops.push(Op::Insert {
                    segment,
                    along,
                    z: verts[segment].z + (verts[segment + 1].z - verts[segment].z) * along,
                });
            }
        }
    }
    ops
}

/// Move the piece's own vertices onto their places and insert the new ones,
/// noting where every vertex of a cluster ended up.
fn apply_ops(verts: &mut Vec<DVec3>, piece: usize, ops: Vec<(usize, Op)>, clusters: &[Cluster], changed: &mut [bool], placed: &mut [Vec<(usize, usize)>]) {
    let mut inserts: BTreeMap<usize, Vec<(f64, f64, usize)>> = BTreeMap::new();
    let mut existing = Vec::new();
    for (cluster, op) in ops {
        match op {
            Op::Existing(vertex) => {
                let at = clusters[cluster].at;
                if verts[vertex].truncate() != at {
                    changed[cluster] = true;
                    verts[vertex].x = at.x;
                    verts[vertex].y = at.y;
                }
                existing.push((cluster, vertex));
            }
            Op::Insert { segment, along, z } => {
                changed[cluster] = true;
                inserts.entry(segment).or_default().push((along, z, cluster));
            }
        }
    }
    let mut rebuilt = Vec::with_capacity(verts.len() + inserts.len());
    let mut new_index = vec![0; verts.len()];
    for (vertex, &point) in verts.iter().enumerate() {
        new_index[vertex] = rebuilt.len();
        rebuilt.push(point);
        if let Some(list) = inserts.get_mut(&vertex) {
            list.sort_by(|one, two| one.0.total_cmp(&two.0));
            for &(_, z, cluster) in list.iter() {
                placed[cluster].push((piece, rebuilt.len()));
                rebuilt.push(clusters[cluster].at.extend(z));
            }
        }
    }
    for (cluster, vertex) in existing {
        placed[cluster].push((piece, new_index[vertex]));
    }
    *verts = rebuilt;
}
