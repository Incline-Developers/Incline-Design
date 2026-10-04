//! Step 2 of Clean Strings: bad heights in one string on its own.

use glam::DVec3;

use super::{BUST_OFFSET, BUST_SPREAD_SHARE, Change, ChangeKind, JOIN_ON_REQUEST};
use crate::model::{kernel, rbf::MERGE_DISTANCE};

/// Drop every vertex at z = 0 when the string is not all at z = 0, then
/// every single height far off its neighbours, listing each into `changes`.
pub(super) fn drop_bad_heights(source: usize, verts: &mut Vec<DVec3>, changes: &mut Vec<Change>) {
    drop_zero_heights(source, verts, changes);
    drop_single_heights(source, verts, changes);
}

/// A string with some height other than zero never means z = 0 as a height:
/// those vertices were left unset. A string wholly at zero is flat on
/// purpose and stays.
fn drop_zero_heights(source: usize, verts: &mut Vec<DVec3>, changes: &mut Vec<Change>) {
    let kept = verts.iter().filter(|vertex| vertex.z != 0.0).count();
    if kept == verts.len() || kept < 2 {
        return;
    }
    for vertex in verts.iter().filter(|vertex| vertex.z == 0.0) {
        changes.push(Change {
            source,
            at: *vertex,
            kind: ChangeKind::ZeroDropped,
        });
    }
    verts.retain(|vertex| vertex.z != 0.0);
}

/// The vertices whose height sits far off the heights around them, all
/// judged on the string as it is and dropped together.
fn drop_single_heights(source: usize, verts: &mut Vec<DVec3>, changes: &mut Vec<Change>) {
    let count = verts.len();
    if count < 3 {
        return;
    }
    // Judged on the vertices that shape the string: one lying on the line
    // between its neighbours, as a vertex put in where two strings meet
    // does, adds no height of its own and must not pass for a neighbour.
    let shape = shape_vertices(verts);
    if shape.len() < 3 {
        return;
    }
    let z: Vec<f64> = shape.iter().map(|&index| verts[index].z).collect();
    let bad: Vec<(usize, f64)> = (0..z.len()).filter_map(|place| bad_height(&z, place).map(|offset| (shape[place], offset))).collect();
    if bad.is_empty() || count - bad.len() < 2 {
        return;
    }
    for &(index, offset) in &bad {
        changes.push(Change {
            source,
            at: verts[index],
            kind: ChangeKind::HeightDropped { offset },
        });
    }
    let mut index = 0;
    verts.retain(|_| {
        index += 1;
        !bad.iter().any(|&(dropped, _)| dropped == index - 1)
    });
}

/// The vertices that shape the string, by place: walking along it, a
/// vertex is passed over when it lies on the line from the last one kept to
/// the next, in plan within the merge distance of that segment (a vertex
/// shared at a crossing sits on the other string's vertex, up to that far
/// off this one's line) and in height within half of [`JOIN_ON_REQUEST`] of
/// it, the most a join moves a vertex. Taken one at a time, so a corner
/// hugged by vertices put in close beside it stays a corner.
fn shape_vertices(verts: &[DVec3]) -> Vec<usize> {
    let mut kept: Vec<usize> = Vec::with_capacity(verts.len());
    for index in 0..verts.len() {
        while kept.len() >= 2 && on_the_line(verts[kept[kept.len() - 2]], verts[kept[kept.len() - 1]], verts[index]) {
            kept.pop();
        }
        kept.push(index);
    }
    kept
}

fn on_the_line(before: DVec3, at: DVec3, after: DVec3) -> bool {
    let (near, along) = kernel::project_onto_segment(at.truncate(), before.truncate(), after.truncate());
    near.distance(at.truncate()) < MERGE_DISTANCE && (at.z - (before.z + (after.z - before.z) * along)).abs() <= JOIN_ON_REQUEST / 2.0
}

/// How far vertex `index` sits off its neighbours, positive above them, when
/// that is a single bad height. An interior vertex must stand off both
/// neighbours while the two heights each side agree among themselves; an
/// end vertex is judged against the two next to it. Two each side keeps a
/// string zigzagging between two levels from reading as a row of bad heights.
fn bad_height(z: &[f64], index: usize) -> Option<f64> {
    let last = z.len() - 1;
    let (offset, around) = if index == 0 {
        (((z[0] - z[1]).abs()).min((z[0] - z[2]).abs()), vec![z[1], z[2]])
    } else if index == last {
        (((z[last] - z[last - 1]).abs()).min((z[last] - z[last - 2]).abs()), vec![z[last - 1], z[last - 2]])
    } else {
        let around: Vec<f64> = [index.checked_sub(2), index.checked_sub(1), Some(index + 1), Some(index + 2)]
            .into_iter()
            .flatten()
            .filter(|&other| other <= last)
            .map(|other| z[other])
            .collect();
        (((z[index] - z[index - 1]).abs()).min((z[index] - z[index + 1]).abs()), around)
    };
    let spread = around.iter().fold(f64::NEG_INFINITY, |high, value| high.max(*value)) - around.iter().fold(f64::INFINITY, |low, value| low.min(*value));
    if offset > BUST_OFFSET && spread < BUST_SPREAD_SHARE * offset {
        let above = z[index] > around[0];
        Some(if above { offset } else { -offset })
    } else {
        None
    }
}
