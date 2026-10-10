//! Step 3 of Clean Strings: strings of one layer running along each other.

use glam::{DVec2, DVec3};

use super::{Cancelled, Change, ChangeKind, Piece, REPEAT_HEIGHTS, stop_if};
use crate::model::{
    control_checks::{BoxGrid, elevation_along, segment_box},
    kernel::{self, SegSeg},
    rbf::MERGE_DISTANCE,
};

/// A stretch of a losing string another string runs along: distances along
/// the loser, and the piece that wins it.
#[derive(Clone, Copy)]
struct Covered {
    from: f64,
    to: f64,
    winner: usize,
}

/// Cut every stretch two strings share out of the shorter of the two.
/// `Err` once `cancelled` says so, nothing cut.
pub(super) fn cut_shared_stretches(pieces: &mut Vec<Piece>, changes: &mut Vec<Change>, cancelled: &dyn Fn() -> bool) -> Result<(), Cancelled> {
    let walks: Vec<Walk> = pieces.iter().map(|piece| Walk::new(&piece.verts)).collect();
    let covered = covered_stretches(pieces, &walks, cancelled)?;
    if covered.iter().all(Vec::is_empty) {
        return Ok(());
    }
    let mut result = Vec::with_capacity(pieces.len());
    for (index, piece) in pieces.iter().enumerate() {
        stop_if(cancelled)?;
        if covered[index].is_empty() {
            result.push(piece.clone());
            continue;
        }
        let walk = &walks[index];
        let cuts = merged(&covered[index], pieces, &walks);
        let stretches = remaining(&piece.verts, walk, &cuts);
        if stretches.is_empty() {
            changes.push(Change {
                source: piece.source,
                at: piece.verts[0],
                kind: ChangeKind::Removed {
                    kept: pieces[cuts[0].winner].source,
                },
            });
        } else {
            for cut in &cuts {
                let kind = ChangeKind::SharedCut {
                    kept: pieces[cut.winner].source,
                    length: cut.to - cut.from,
                };
                changes.push(Change {
                    source: piece.source,
                    at: walk.point_at(&piece.verts, cut.from),
                    kind,
                });
            }
        }
        result.extend(stretches.into_iter().map(|verts| Piece { source: piece.source, verts }));
    }
    *pieces = result;
    Ok(())
}

/// Plan distances along one string at each of its vertices.
struct Walk {
    at: Vec<f64>,
}

impl Walk {
    fn new(verts: &[DVec3]) -> Self {
        let mut at = Vec::with_capacity(verts.len());
        let mut total = 0.0;
        at.push(total);
        for pair in verts.windows(2) {
            total += pair[0].truncate().distance(pair[1].truncate());
            at.push(total);
        }
        Self { at }
    }

    fn length(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }

    /// The segment holding distance `along`, and how far along it, as a
    /// fraction, that distance falls.
    fn locate(&self, along: f64) -> (usize, f64) {
        let segment = self.at.partition_point(|&start| start <= along).saturating_sub(1).min(self.at.len() - 2);
        let length = self.at[segment + 1] - self.at[segment];
        let fraction = if length > 0.0 { ((along - self.at[segment]) / length).clamp(0.0, 1.0) } else { 0.0 };
        (segment, fraction)
    }

    /// The point on the string `along` metres in: plan by interpolation,
    /// height by the build's own reading along a segment.
    fn point_at(&self, verts: &[DVec3], along: f64) -> DVec3 {
        let (segment, fraction) = self.locate(along);
        let (start, end) = (verts[segment], verts[segment + 1]);
        let plan = start.truncate().lerp(end.truncate(), fraction);
        plan.extend(elevation_along(start, end, fraction))
    }
}

/// For every piece, the stretches of it a longer (or lower numbered) piece
/// runs along, all read from the strings before any is cut.
fn covered_stretches(pieces: &[Piece], walks: &[Walk], cancelled: &dyn Fn() -> bool) -> Result<Vec<Vec<Covered>>, Cancelled> {
    let segments: Vec<(usize, usize)> = pieces
        .iter()
        .enumerate()
        .flat_map(|(index, piece)| (0..piece.verts.len().saturating_sub(1)).map(move |segment| (index, segment)))
        .collect();
    let plan = |index: usize, segment: usize| -> (DVec2, DVec2) { (pieces[index].verts[segment].truncate(), pieces[index].verts[segment + 1].truncate()) };
    let grid = BoxGrid::new(
        segments
            .iter()
            .map(|&(index, segment)| segment_box(plan(index, segment).0, plan(index, segment).1))
            .collect(),
    );
    let mut covered: Vec<Vec<Covered>> = vec![Vec::new(); pieces.len()];
    let mut near = Vec::new();
    for &(left, a) in &segments {
        stop_if(cancelled)?;
        let (from, to) = plan(left, a);
        let (low, high) = segment_box(from, to);
        grid.overlapping(low, high, &mut near);
        for &other in &near {
            let (right, b) = segments[other];
            if right <= left {
                continue;
            }
            let (loser, winner) = if shorter_loses(pieces, walks, left, right) { (left, right) } else { (right, left) };
            let (loser_segment, winner_segment) = if loser == left { (a, b) } else { (b, a) };
            let (start, end) = plan(loser, loser_segment);
            let (other_start, other_end) = plan(winner, winner_segment);
            if let SegSeg::CollinearOverlap { t0, t1 } = kernel::segment_segment(start, end, other_start, other_end) {
                let length = start.distance(end);
                let base = walks[loser].at[loser_segment];
                covered[loser].push(Covered {
                    from: base + t0.min(t1) * length,
                    to: base + t0.max(t1) * length,
                    winner,
                });
            }
        }
    }
    Ok(covered)
}

/// Whether `left` is the one to lose against `right`: the shorter in plan,
/// and on exactly equal lengths the higher source.
fn shorter_loses(pieces: &[Piece], walks: &[Walk], left: usize, right: usize) -> bool {
    let (one, other) = (walks[left].length(), walks[right].length());
    if one == other { pieces[left].source > pieces[right].source } else { one < other }
}

/// The covered stretches of one loser with the overlapping and touching ones
/// joined, in order along it. A joined stretch is won by the longest winner
/// at its start, the lower source on equal lengths.
fn merged(covered: &[Covered], pieces: &[Piece], walks: &[Walk]) -> Vec<Covered> {
    let mut sorted = covered.to_vec();
    sorted.sort_by(|a, b| a.from.total_cmp(&b.from).then(a.to.total_cmp(&b.to)));
    let mut cuts: Vec<Covered> = Vec::new();
    for stretch in sorted {
        match cuts.last_mut() {
            Some(last) if stretch.from <= last.to + kernel::XY_TOL => {
                if stretch.from <= last.from + kernel::XY_TOL && outranks(pieces, walks, stretch.winner, last.winner) {
                    last.winner = stretch.winner;
                }
                last.to = last.to.max(stretch.to);
            }
            _ => cuts.push(stretch),
        }
    }
    cuts
}

/// Whether piece `one` is the better one to name as the winner than `other`:
/// the longer, and on exactly equal lengths the lower source.
fn outranks(pieces: &[Piece], walks: &[Walk], one: usize, other: usize) -> bool {
    let (long, short) = (walks[one].length(), walks[other].length());
    if long == short { pieces[one].source < pieces[other].source } else { long > short }
}

/// What is left of a string after the `cuts` are taken out: each stretch
/// from the cut point at its start through the original vertices inside to
/// the cut point at its end. Stretches shorter than the build's merge
/// distance are dropped, and an original vertex within the merge distance
/// of a cut point is merged into it, as step 1 merges repeated points, so a
/// second clean finds no repeat to merge.
fn remaining(verts: &[DVec3], walk: &Walk, cuts: &[Covered]) -> Vec<Vec<DVec3>> {
    let mut gaps = Vec::new();
    let mut from = 0.0;
    for cut in cuts {
        gaps.push((from, cut.from));
        from = cut.to;
    }
    gaps.push((from, walk.length()));
    gaps.into_iter()
        .filter(|&(from, to)| to - from >= MERGE_DISTANCE)
        .map(|(from, to)| {
            let mut stretch = vec![walk.point_at(verts, from)];
            stretch.extend(
                (0..verts.len())
                    .filter(|&vertex| walk.at[vertex] > from + kernel::XY_TOL && walk.at[vertex] < to - kernel::XY_TOL)
                    .map(|vertex| verts[vertex]),
            );
            stretch.push(walk.point_at(verts, to));
            merge_into_cut_ends(&mut stretch);
            stretch
        })
        .collect()
}

/// Merge the original vertices next to each cut point and within the merge
/// distance of it in plan into the cut point, at their mean height, when
/// the heights are within [`REPEAT_HEIGHTS`]; further apart they are left,
/// as step 1 leaves them. The cut point keeps its plan position: it lies on
/// the string kept.
fn merge_into_cut_ends(stretch: &mut Vec<DVec3>) {
    let near = |stretch: &[DVec3], end: usize, others: &mut dyn Iterator<Item = usize>| -> usize {
        others
            .take_while(|&vertex| stretch[vertex].truncate().distance(stretch[end].truncate()) < MERGE_DISTANCE)
            .count()
    };
    let merge = |run: &[DVec3]| -> Option<f64> {
        let low = run.iter().map(|vertex| vertex.z).fold(f64::INFINITY, f64::min);
        let high = run.iter().map(|vertex| vertex.z).fold(f64::NEG_INFINITY, f64::max);
        (high - low <= REPEAT_HEIGHTS).then(|| run.iter().map(|vertex| vertex.z).sum::<f64>() / run.len() as f64)
    };
    let last = stretch.len() - 1;
    let count = near(stretch, 0, &mut (1..last));
    if count > 0
        && let Some(z) = merge(&stretch[..=count])
    {
        stretch[0].z = z;
        stretch.drain(1..=count);
    }
    let last = stretch.len() - 1;
    let count = near(stretch, last, &mut (1..last).rev());
    if count > 0
        && let Some(z) = merge(&stretch[last - count..])
    {
        stretch[last].z = z;
        stretch.drain(last - count..last);
    }
}
