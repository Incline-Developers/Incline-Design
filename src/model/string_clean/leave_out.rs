//! The strings Build Surface leaves out of one build when its own checks
//! refuse the control strings together, so it can build from the rest.
//!
//! A string with a fault of its own shape (too short, ending where it
//! starts, turning back, crossing itself, two of its points at one place
//! at two heights) is left out at once. Then, until the strings left pass
//! the build's checks, one string at a time is left out among those still
//! in a clash with another (meeting at two heights, running along each
//! other, passing close at two heights): the one whose clashes are the
//! largest share of all the places it meets or nears another string,
//! preferring a string on one side of every string it clashes with; ties go
//! to the string with fewer crossings agreeing, then the shorter in plan,
//! then the lower key. A string with no fault or clash is never left out.
//! Nothing here changes a string: the build reads the answer for one run
//! only.

use glam::DVec3;

use super::check::{self, Problem, ProblemKind};

/// One string left out, with what it was left out for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LeftOut {
    /// By place in the strings given.
    pub(crate) piece: usize,
    /// Its own shape faults, or every clash it had with the strings still
    /// in when it was chosen, in the order the checks list them: the
    /// crossings biggest miss first.
    pub(crate) problems: Vec<Problem>,
}

/// Why no set of strings could be left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotLeftOut {
    /// The checks refuse strings no clash or shape fault names.
    Unnamed,
    /// Leaving out every string that clashes or is misshapen leaves none.
    NoneLeft,
    Cancelled,
}

fn is_shape(kind: &ProblemKind) -> bool {
    matches!(
        kind,
        ProblemKind::TooShort | ProblemKind::EndsWhereItStarts | ProblemKind::TurnsBack | ProblemKind::CrossesItself | ProblemKind::PointsDisagree { .. }
    )
}

fn is_clash(kind: &ProblemKind) -> bool {
    matches!(kind, ProblemKind::Crossing { .. } | ProblemKind::NearMiss { .. } | ProblemKind::Along)
}

fn plan_length(string: &[DVec3]) -> f64 {
    string.windows(2).map(|pair| pair[0].truncate().distance(pair[1].truncate())).sum()
}

/// How one string stands among the clashes still open.
#[derive(Clone, Copy, Default)]
struct Tally {
    clashes: usize,
    /// Every place it meets or nears a string still in: its crossings, at
    /// one height or two, and its other clashes.
    places: usize,
    above: usize,
    below: usize,
}

/// The strings to leave out of `strings` so the rest pass Build Surface's
/// checks, in the order they were chosen; empty when the strings pass as
/// they are. `problems` are [`super::problems`] of `strings`; `keys` break
/// the last tie, lower first, a string past their end keyed by its place.
pub(crate) fn leave_out(strings: &[Vec<DVec3>], problems: &[Problem], keys: &[u64], cancelled: &dyn Fn() -> bool) -> Result<Vec<LeftOut>, NotLeftOut> {
    let stopped = |_| NotLeftOut::Cancelled;
    if check::build_refusal(strings, cancelled).map_err(stopped)?.is_none() {
        return Ok(Vec::new());
    }
    let count = strings.len();
    let mut live = vec![true; count];
    let places = check::crossing_places(strings, cancelled).map_err(stopped)?;
    let lengths: Vec<f64> = strings.iter().map(|string| plan_length(string)).collect();
    let key = |piece: usize| keys.get(piece).copied().unwrap_or(piece as u64);
    let mut problems = problems.to_vec();
    let mut left: Vec<LeftOut> = Vec::new();
    loop {
        if cancelled() {
            return Err(NotLeftOut::Cancelled);
        }
        let mut shaped: Vec<usize> = problems
            .iter()
            .filter(|problem| is_shape(&problem.kind))
            .filter_map(|problem| problem.sides.first().map(|side| side.piece))
            .filter(|&piece| live[piece])
            .collect();
        shaped.sort_unstable();
        shaped.dedup();
        for piece in shaped {
            live[piece] = false;
            left.push(LeftOut {
                piece,
                problems: problems
                    .iter()
                    .filter(|problem| is_shape(&problem.kind) && problem.sides.iter().any(|side| side.piece == piece))
                    .cloned()
                    .collect(),
            });
        }
        let clashes: Vec<&Problem> = problems
            .iter()
            .filter(|problem| is_clash(&problem.kind) && problem.sides.len() == 2 && problem.sides.iter().all(|side| live[side.piece]))
            .collect();
        if clashes.is_empty() {
            let rest: Vec<usize> = (0..count).filter(|&piece| live[piece]).collect();
            if rest.is_empty() {
                return Err(NotLeftOut::NoneLeft);
            }
            let kept: Vec<Vec<DVec3>> = rest.iter().map(|&piece| strings[piece].clone()).collect();
            if check::build_refusal(&kept, cancelled).map_err(stopped)?.is_none() {
                return Ok(left);
            }
            // What the first problems could not show among the strings
            // left: found again on those alone, by place in `strings`.
            let fresh: Vec<Problem> = check::problems(&kept, cancelled)
                .map_err(stopped)?
                .into_iter()
                .map(|mut problem| {
                    for side in &mut problem.sides {
                        side.piece = rest[side.piece];
                    }
                    problem
                })
                .collect();
            if !fresh.iter().any(|problem| is_shape(&problem.kind) || is_clash(&problem.kind)) {
                return Err(NotLeftOut::Unnamed);
            }
            problems = fresh;
            continue;
        }
        let culprit = culprit(&clashes, &places, &live, &lengths, &key);
        live[culprit] = false;
        left.push(LeftOut {
            piece: culprit,
            problems: clashes
                .iter()
                .filter(|problem| problem.sides.iter().any(|side| side.piece == culprit))
                .map(|problem| (*problem).clone())
                .collect(),
        });
    }
}

/// The string to leave out next among those in `clashes`.
fn culprit(clashes: &[&Problem], places: &[(usize, usize)], live: &[bool], lengths: &[f64], key: &dyn Fn(usize) -> u64) -> usize {
    let mut tallies = vec![Tally::default(); live.len()];
    for &(a, b) in places {
        if live[a] && live[b] {
            tallies[a].places += 1;
            tallies[b].places += 1;
        }
    }
    for problem in clashes {
        let crossing = matches!(problem.kind, ProblemKind::Crossing { .. });
        for (index, side) in problem.sides.iter().enumerate() {
            let other = &problem.sides[1 - index];
            let tally = &mut tallies[side.piece];
            tally.clashes += 1;
            if !crossing {
                tally.places += 1;
            }
            if let (Some(own), Some(theirs)) = (side.height, other.height) {
                if own < theirs {
                    tally.below += 1;
                } else if own > theirs {
                    tally.above += 1;
                }
            }
        }
    }
    for tally in &mut tallies {
        tally.places = tally.places.max(tally.clashes);
    }
    let in_clash: Vec<usize> = (0..live.len()).filter(|&piece| tallies[piece].clashes > 0).collect();
    let one_sided: Vec<usize> = in_clash.iter().copied().filter(|&piece| tallies[piece].above == 0 || tallies[piece].below == 0).collect();
    let candidates = if one_sided.is_empty() { in_clash } else { one_sided };
    candidates
        .into_iter()
        .min_by(|&a, &b| {
            let (one, other) = (tallies[a], tallies[b]);
            (other.clashes * one.places)
                .cmp(&(one.clashes * other.places))
                .then((one.places - one.clashes).cmp(&(other.places - other.clashes)))
                .then(lengths[a].total_cmp(&lengths[b]))
                .then(key(a).cmp(&key(b)))
        })
        .expect("a clash names a string")
}
