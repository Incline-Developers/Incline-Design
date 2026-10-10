//! Step 1 of Clean Strings: hygiene of one string on its own.

use glam::{DVec2, DVec3};

use super::{Cancelled, Change, ChangeKind, REPEAT_HEIGHTS, stop_if};
use crate::model::{
    control_checks::{BoxGrid, SelfContact, elevation_along, segment_box, turning_back},
    kernel::{self, SegSeg},
    rbf::MERGE_DISTANCE,
};

/// What one rule did on a pass.
enum Outcome {
    /// The rule found nothing to fix.
    Nothing,
    /// A fix was made.
    Applied,
    /// A fix was due but would leave too little of the string, or cut too
    /// much of it: the string is left as it is.
    Refused,
}

/// Merge repeated points, drop spikes, cut out-and-back stretches back to
/// the way out and cut loops where the string meets itself, listing every
/// change made into `changes` under `source`. `Err` once `cancelled` says
/// so.
///
/// Each round merges repeats, then fixes every turn in one forward pass, then
/// cuts the first loop. A string still holding repeats too far apart in
/// height to merge is fixed one turn at a time instead, each fix followed
/// by a new merge, as a fix there can change what merges.
pub(super) fn clean_string(source: usize, verts: &mut Vec<DVec3>, changes: &mut Vec<Change>, cancelled: &dyn Fn() -> bool) -> Result<(), Cancelled> {
    loop {
        stop_if(cancelled)?;
        let mut merged = merge_repeats(source, verts, changes);
        let outcome = if has_repeats(verts) {
            let plan = plan_of(verts);
            match turning_back(&plan) {
                Some(turn) => fix_turn(source, verts, &plan, turn, changes),
                None => cut_loop(source, verts, &plan, changes),
            }
        } else {
            match fix_turns(source, verts, changes, cancelled)? {
                Sweep::Joined => continue,
                Sweep::Fixed(fixed) => {
                    // A fixed string would have been merged again before
                    // the loop is looked for, with nothing to merge.
                    merged &= !fixed;
                    let plan = plan_of(verts);
                    cut_loop(source, verts, &plan, changes)
                }
            }
        };
        match outcome {
            Outcome::Refused => return Ok(()),
            Outcome::Applied => {}
            Outcome::Nothing if merged => {}
            Outcome::Nothing => return Ok(()),
        }
    }
}

/// How a pass over the turns of a string ended.
enum Sweep {
    /// No turn is left; whether any was fixed.
    Fixed(bool),
    /// A fix left points within the merge distance of each other: the string
    /// is to be merged again before anything else.
    Joined,
}

/// Whether two consecutive points lie within the merge distance of each
/// other.
fn has_repeats(verts: &[DVec3]) -> bool {
    verts.windows(2).any(|pair| pair[0].truncate().distance(pair[1].truncate()) < MERGE_DISTANCE)
}

/// Fix every turn of a string with no points within the merge distance of
/// each other, first turn first, as fixing them one at a time would, in one
/// pass. The points already passed are kept as a stack: a fix only changes
/// the corners at its own end of it. Each point also keeps the box around
/// all before it, and the points ahead the box around all after them, so a
/// point far from a polyline is told so without walking it.
fn fix_turns(source: usize, verts: &mut Vec<DVec3>, changes: &mut Vec<Change>, cancelled: &dyn Fn() -> bool) -> Result<Sweep, Cancelled> {
    let plan = plan_of(verts);
    let mut ahead = vec![EMPTY_BOX; plan.len() + 1];
    for index in (0..plan.len()).rev() {
        ahead[index] = grow(ahead[index + 1], plan[index]);
    }
    let mut stack: Vec<DVec3> = Vec::with_capacity(verts.len());
    let mut behind: Vec<(DVec2, DVec2)> = Vec::with_capacity(verts.len());
    let mut fixed = false;
    for next in 0..plan.len() {
        let point = verts[next];
        if next % 1024 == 0 {
            stop_if(cancelled)?;
        }
        let mut joined = false;
        while stack.len() >= 2 {
            let turn = stack.len() - 1;
            let corner = [stack[turn - 1].truncate(), stack[turn].truncate(), plan[next]];
            if turning_back(&corner).is_none() {
                break;
            }
            let at = stack[turn];
            let box_behind = behind[turn];
            // The way back: every point still to come lies on the string so far.
            let back = (next..plan.len()).take_while(|&k| near_stack(plan[k], &stack, box_behind)).count();
            if back == plan.len() - next {
                changes.push(Change {
                    source,
                    at,
                    kind: ChangeKind::RetraceDropped { vertices: back },
                });
                *verts = stack;
                return Ok(Sweep::Fixed(true));
            }
            // The way out: every point so far lies on the string still to come.
            let ahead_box = grow(ahead[next], at.truncate());
            let out = (0..turn)
                .rev()
                .take_while(|&k| near_path(stack[k].truncate(), at.truncate(), &plan[next..], ahead_box))
                .count();
            fixed = true;
            if out == turn {
                changes.push(Change {
                    source,
                    at,
                    kind: ChangeKind::RetraceDropped { vertices: out },
                });
                stack.drain(..turn);
                behind.clear();
                behind.push(grow(EMPTY_BOX, at.truncate()));
            } else {
                changes.push(Change {
                    source,
                    at,
                    kind: ChangeKind::SpikeDropped,
                });
                stack.pop();
                behind.pop();
                joined = true;
            }
        }
        if joined && plan[next].distance(stack.last().unwrap().truncate()) < MERGE_DISTANCE {
            stack.extend_from_slice(&verts[next..]);
            *verts = stack;
            return Ok(Sweep::Joined);
        }
        behind.push(grow(behind.last().copied().unwrap_or(EMPTY_BOX), plan[next]));
        stack.push(point);
    }
    if fixed {
        *verts = stack;
    }
    Ok(Sweep::Fixed(fixed))
}

const EMPTY_BOX: (DVec2, DVec2) = (DVec2::INFINITY, DVec2::NEG_INFINITY);

/// The box widened to hold `point`.
fn grow(found: (DVec2, DVec2), point: DVec2) -> (DVec2, DVec2) {
    (found.0.min(point), found.1.max(point))
}

/// Whether `point` lies within the kernel's plan tolerance of the segment.
fn near_segment(point: DVec2, start: DVec2, end: DVec2) -> bool {
    kernel::project_onto_segment(point, start, end).0.distance(point) <= kernel::XY_TOL
}

/// Whether `point` lies in the box, widened by the kernel's plan tolerance.
fn in_box(point: DVec2, found: (DVec2, DVec2)) -> bool {
    let margin = DVec2::splat(kernel::XY_TOL);
    (found.0 - margin).cmple(point).all() && point.cmple(found.1 + margin).all()
}

/// Whether `point` lies on the string held as `stack`, whose points lie in
/// `found`, searching from its end.
fn near_stack(point: DVec2, stack: &[DVec3], found: (DVec2, DVec2)) -> bool {
    in_box(point, found) && stack.windows(2).rev().any(|pair| near_segment(point, pair[0].truncate(), pair[1].truncate()))
}

/// Whether `point` lies on the polyline from `start` through `rest`, whose
/// points lie in `found`.
fn near_path(point: DVec2, start: DVec2, rest: &[DVec2], found: (DVec2, DVec2)) -> bool {
    in_box(point, found) && (near_segment(point, start, rest[0]) || rest.windows(2).any(|pair| near_segment(point, pair[0], pair[1])))
}

fn plan_of(verts: &[DVec3]) -> Vec<DVec2> {
    verts.iter().map(|vertex| vertex.truncate()).collect()
}

/// Turn each run of points within the merge distance of the run's first
/// point into one, at that plan position and the run's mean height, unless
/// the heights spread wider than [`REPEAT_HEIGHTS`] or the string would be
/// left with fewer than two vertices. Whether anything merged.
fn merge_repeats(source: usize, verts: &mut Vec<DVec3>, changes: &mut Vec<Change>) -> bool {
    let mut kept: Vec<DVec3> = Vec::with_capacity(verts.len());
    let mut remaining = verts.len();
    let mut any = false;
    let mut start = 0;
    while start < verts.len() {
        let first = verts[start];
        let mut end = start + 1;
        while end < verts.len() && verts[end].truncate().distance(first.truncate()) < MERGE_DISTANCE {
            end += 1;
        }
        let run = &verts[start..end];
        let count = run.len();
        let low = run.iter().map(|vertex| vertex.z).fold(f64::INFINITY, f64::min);
        let high = run.iter().map(|vertex| vertex.z).fold(f64::NEG_INFINITY, f64::max);
        if count >= 2 && high - low <= REPEAT_HEIGHTS && remaining - (count - 1) >= 2 {
            let merged = first.truncate().extend(run.iter().map(|vertex| vertex.z).sum::<f64>() / count as f64);
            kept.push(merged);
            changes.push(Change {
                source,
                at: merged,
                kind: ChangeKind::RepeatsMerged { count },
            });
            remaining -= count - 1;
            any = true;
        } else {
            kept.extend_from_slice(run);
        }
        start = end;
    }
    if any {
        *verts = kept;
    }
    any
}

/// Whether `point` lies within the kernel's plan tolerance of the polyline.
fn on_polyline(point: DVec2, line: &[DVec2]) -> bool {
    line.windows(2).any(|segment| near_segment(point, segment[0], segment[1]))
}

/// Fix the turn at vertex `turn`: cut the way back off the end, cut the
/// dead-end leg off the start, or drop the tip of a spike.
fn fix_turn(source: usize, verts: &mut Vec<DVec3>, plan: &[DVec2], turn: usize, changes: &mut Vec<Change>) -> Outcome {
    let last = verts.len() - 1;
    let back = (turn + 1..=last).take_while(|&k| on_polyline(plan[k], &plan[..=turn])).count();
    let out = (0..turn).rev().take_while(|&k| on_polyline(plan[k], &plan[turn..])).count();
    let at = verts[turn];
    if back >= 1 && turn + back == last {
        if turn + 1 < 2 {
            return Outcome::Refused;
        }
        verts.truncate(turn + 1);
        changes.push(Change {
            source,
            at,
            kind: ChangeKind::RetraceDropped { vertices: back },
        });
    } else if out >= 1 && turn == out {
        if verts.len() - turn < 2 {
            return Outcome::Refused;
        }
        verts.drain(..turn);
        changes.push(Change {
            source,
            at,
            kind: ChangeKind::RetraceDropped { vertices: out },
        });
    } else {
        if verts.len() < 3 {
            return Outcome::Refused;
        }
        verts.remove(turn);
        changes.push(Change {
            source,
            at,
            kind: ChangeKind::SpikeDropped,
        });
    }
    Outcome::Applied
}

/// Where a string first meets itself, segments taken in order along it, as
/// [`self_intersection`] finds it but leaving out the touch of a segment by
/// the one after the next across a zero-length segment between them: that
/// is a repeated point, not a loop.
fn first_self_contact(plan: &[DVec2]) -> Option<SelfContact> {
    let segments = plan.len().saturating_sub(1);
    let grid = BoxGrid::new((0..segments).map(|segment| segment_box(plan[segment], plan[segment + 1])).collect());
    let mut near = Vec::new();
    (0..segments).find_map(|first| {
        let (low, high) = segment_box(plan[first], plan[first + 1]);
        grid.overlapping(low, high, &mut near);
        near.iter()
            .filter(|&&second| second >= first + 2 && !is_repeat_touch(plan, first, second))
            .find_map(|&second| match kernel::segment_segment(plan[first], plan[first + 1], plan[second], plan[second + 1]) {
                SegSeg::Disjoint => None,
                SegSeg::Crossing { point, t, .. } | SegSeg::Touching { point, t, .. } => Some(SelfContact { point, segment: first, along: t }),
                SegSeg::CollinearOverlap { t0, .. } => Some(SelfContact {
                    point: plan[first].lerp(plan[first + 1], t0),
                    segment: first,
                    along: t0,
                }),
            })
    })
}

/// Whether segments `first` and `second` meet only through a repeated point
/// between them: repeated points too far apart in height to merge touch
/// themselves through a zero-length segment, and are left for the final
/// check to name.
fn is_repeat_touch(plan: &[DVec2], first: usize, second: usize) -> bool {
    second == first + 2 && plan[first + 1].distance(plan[first + 2]) < MERGE_DISTANCE
}

/// Cut out the loop where the string first meets itself, putting the
/// meeting point in its place, unless the loop is longer than half the
/// string.
fn cut_loop(source: usize, verts: &mut Vec<DVec3>, plan: &[DVec2], changes: &mut Vec<Change>) -> Outcome {
    let Some(contact) = first_self_contact(plan) else {
        return Outcome::Nothing;
    };
    let i = contact.segment;
    let mut meeting = None;
    for j in i + 2..plan.len() - 1 {
        if is_repeat_touch(plan, i, j) {
            continue;
        }
        let met = match kernel::segment_segment(plan[i], plan[i + 1], plan[j], plan[j + 1]) {
            SegSeg::Disjoint => continue,
            SegSeg::Crossing { point, .. } | SegSeg::Touching { point, .. } => point,
            SegSeg::CollinearOverlap { .. } => contact.point,
        };
        meeting = Some((j, met));
        break;
    }
    let Some((j, second)) = meeting else {
        return Outcome::Refused;
    };
    let length = |from: usize, to: usize| plan[from..=to].windows(2).map(|pair| pair[0].distance(pair[1])).sum::<f64>();
    let whole = length(0, plan.len() - 1);
    let loop_length = contact.point.distance(plan[i + 1]) + length(i + 1, j) + plan[j].distance(second);
    if loop_length > whole / 2.0 {
        return Outcome::Refused;
    }
    let z = elevation_along(verts[i], verts[i + 1], contact.along);
    let point = contact.point.extend(z);
    let put_in = contact.point.distance(plan[i]) >= MERGE_DISTANCE && contact.point.distance(plan[j + 1]) >= MERGE_DISTANCE;
    let left = i + 1 + usize::from(put_in) + (verts.len() - j - 1);
    if left < 2 {
        return Outcome::Refused;
    }
    let mut cut: Vec<DVec3> = verts[..=i].to_vec();
    if put_in {
        cut.push(point);
    }
    cut.extend_from_slice(&verts[j + 1..]);
    *verts = cut;
    changes.push(Change {
        source,
        at: point,
        kind: ChangeKind::LoopCut { vertices: j - i },
    });
    Outcome::Applied
}
