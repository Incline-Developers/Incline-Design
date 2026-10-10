//! Clean Strings: one pass over the open strings of one layer that fixes
//! what Build Surface would refuse and is plain hygiene, and names the rest.
//!
//! Every layer is cleaned on its own: no string or point is ever compared
//! with another layer's. Within a layer the steps run in this order, each on
//! what the one before left:
//!
//! 1. hygiene, string by string: repeated points, spikes, out-and-back
//!    stretches and loops where a string crosses or touches itself;
//! 2. bad heights, string by string: a vertex at z = 0 in a string that is
//!    not all at z = 0, and a single height far off its neighbours;
//! 3. strings running along each other: the shared stretch is cut out of the
//!    shorter;
//! 4. crossings: a shared vertex at every place two strings meet, on both,
//!    joined at the halfway height when they miss by [`JOIN_AUTOMATIC`] or
//!    less;
//! 5. a final check with Build Surface's own checks, naming everything still
//!    in the way of a build, and each string that sits on one side of every
//!    string it misses by more than [`JOIN_ON_REQUEST`].
//!
//! The heights at crossings are read after steps 1 to 3, so a crossing is
//! judged on the strings as they will be. Nothing here touches a document:
//! the functions take vertices and give back vertices, the changes made, and
//! what is left for the geologist.

use glam::{DVec2, DVec3};

mod along;
mod check;
mod crossings;
mod heights;
mod hygiene;
mod leave_out;

pub(crate) use check::{Problem, ProblemKind};
pub(crate) use leave_out::{LeftOut, NotLeftOut, leave_out};

/// How far apart, in metres, the heights of consecutive points within the
/// build's merge distance may be and still merge into one point at their
/// mean height. Further apart, the points are left for hand fixing.
pub(crate) const REPEAT_HEIGHTS: f64 = 0.5;

/// Two strings meeting at heights this far apart or less, in metres, are
/// joined at the halfway height by Clean Strings itself.
pub(crate) const JOIN_AUTOMATIC: f64 = 0.5;

/// Two strings meeting at heights up to this far apart, in metres, are
/// joined at the halfway height when Join all at halfway is asked for.
/// Further apart they are left for hand fixing: usually two fault blocks.
pub(crate) const JOIN_ON_REQUEST: f64 = 5.0;

/// How far, in metres, a single height must sit off both its neighbours to
/// be taken for a bad height and dropped.
pub(crate) const BUST_OFFSET: f64 = 20.0;

/// The share of that offset the heights around a bad height may spread over
/// and still count as sitting close together, so a string that is steep or
/// zigzags all along is never read as one bad height.
pub(crate) const BUST_SPREAD_SHARE: f64 = 0.25;

/// How far apart, as a share of the smallest, the misses of one string may
/// spread and still count as one amount when it sits on one side of every
/// string it misses by more than [`JOIN_ON_REQUEST`].
pub(crate) const ODD_ONE_OUT_SPREAD_SHARE: f64 = 0.25;

/// The most rounds of steps 3 and 4 one clean runs.
const SETTLING_ROUNDS: usize = 4;

/// The run was cancelled before it finished: nothing it made is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cancelled;

/// `Err(Cancelled)` once `cancelled` says so, for the long loops to poll.
fn stop_if(cancelled: &dyn Fn() -> bool) -> Result<(), Cancelled> {
    if cancelled() { Err(Cancelled) } else { Ok(()) }
}

/// One string as the clean holds it: the input string it came from, by its
/// place in the layer's input, and its vertices. A string cut in two by
/// step 3 becomes two pieces from the same source.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Piece {
    pub(crate) source: usize,
    pub(crate) verts: Vec<DVec3>,
}

/// One change the clean made to a string, named by the input string it came
/// from, at the position where it was made.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Change {
    pub(crate) source: usize,
    pub(crate) at: DVec3,
    pub(crate) kind: ChangeKind,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ChangeKind {
    /// `count` consecutive points within the merge distance became one, at
    /// the first one's plan position and their mean height.
    RepeatsMerged { count: usize },
    /// The vertex where the string turned straight back was dropped.
    SpikeDropped,
    /// An out-and-back stretch ending the string (or starting it) was cut
    /// back to the way out: `vertices` dropped.
    RetraceDropped { vertices: usize },
    /// The loop where the string crossed or touched itself was cut out:
    /// `vertices` dropped, the meeting point put in their place.
    LoopCut { vertices: usize },
    /// A vertex at z = 0 in a string not all at z = 0 was dropped.
    ZeroDropped,
    /// A single height `offset` metres off both its neighbours was dropped;
    /// positive when it sat above them.
    HeightDropped { offset: f64 },
    /// The stretch this string shared with input string `kept` was cut out,
    /// `length` metres of it in plan.
    SharedCut { kept: usize, length: f64 },
    /// The whole string ran along input string `kept` and was removed.
    Removed { kept: usize },
    /// Where this string met input strings `others`, each got a shared
    /// vertex and every one was set to their mean height; they had missed
    /// by `miss` metres.
    Joined { others: Vec<usize>, miss: f64 },
    /// Where this string met input strings `others`, each got a shared
    /// vertex at its own height; they miss by `miss` metres, too far to join
    /// here, and the meeting is named for the geologist.
    VertexShared { others: Vec<usize>, miss: f64 },
}

/// What cleaning one layer gave.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayerClean {
    /// The strings after the clean, in input order; a string cut in two
    /// gives its pieces in order along it, a removed one none.
    pub(crate) pieces: Vec<Piece>,
    pub(crate) changes: Vec<Change>,
    /// What is still in the way of a build, by place in `pieces`.
    pub(crate) problems: Vec<Problem>,
    /// Build Surface's own checks run on the cleaned strings exactly as the
    /// build runs them: `None` when they pass, else the build's refusal.
    pub(crate) refusal: Option<String>,
    /// The strings on one side of every string they miss by more than
    /// [`JOIN_ON_REQUEST`], by place in `pieces`.
    pub(crate) odd_ones: Vec<OddOneOut>,
    /// The strings Build Surface would leave out of a build of these, by
    /// place in `pieces`, in the order it would choose them; empty when it
    /// would leave none out or would still refuse.
    pub(crate) left_out: Vec<usize>,
}

/// The strings [`leave_out`] would leave out of `strings`, by place,
/// ties to the earlier place; none when it cannot choose.
fn would_leave_out(strings: &[Vec<DVec3>], problems: &[Problem], cancelled: &dyn Fn() -> bool) -> Result<Vec<usize>, Cancelled> {
    match leave_out(strings, problems, &[], cancelled) {
        Ok(left) => Ok(left.iter().map(|left| left.piece).collect()),
        Err(NotLeftOut::Cancelled) => Err(Cancelled),
        Err(_) => Ok(Vec::new()),
    }
}

/// Clean the open strings of one layer, given in a fixed order (the lower
/// object id first): the order breaks ties between strings of one length.
/// `Err` once `cancelled` says so.
pub(crate) fn clean_layer(strings: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<LayerClean, Cancelled> {
    let (pieces, changes) = clean_steps(strings, cancelled)?;
    let strings: Vec<Vec<DVec3>> = pieces.iter().map(|piece| piece.verts.clone()).collect();
    let problems = check::problems(&strings, cancelled)?;
    let refusal = check::build_refusal(&strings, cancelled)?;
    Ok(LayerClean {
        odd_ones: odd_ones_out(&strings, &problems, cancelled)?,
        left_out: if refusal.is_some() {
            would_leave_out(&strings, &problems, cancelled)?
        } else {
            Vec::new()
        },
        problems,
        refusal,
        pieces,
        changes,
    })
}

/// Steps 1 to 4 of [`clean_layer`]: the strings after them and the changes
/// made, nothing checked.
fn clean_steps(strings: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<(Vec<Piece>, Vec<Change>), Cancelled> {
    let mut pieces: Vec<Piece> = strings.iter().enumerate().map(|(source, verts)| Piece { source, verts: verts.clone() }).collect();
    let mut changes = Vec::new();
    for piece in &mut pieces {
        hygiene::clean_string(piece.source, &mut piece.verts, &mut changes, cancelled)?;
    }
    for piece in &mut pieces {
        stop_if(cancelled)?;
        heights::drop_bad_heights(piece.source, &mut piece.verts, &mut changes);
    }
    // Steps 3 and 4 take turns until a round cuts nothing more: a shared
    // vertex put at a cut end can lay the next segment along the other
    // string. A round only removes overlap or shares vertices, so it settles
    // within a few; anything left is named by the final check.
    for round in 0..SETTLING_ROUNDS {
        let before = changes.len();
        along::cut_shared_stretches(&mut pieces, &mut changes, cancelled)?;
        if round > 0 && changes.len() == before {
            break;
        }
        crossings::share_crossings(&mut pieces, JOIN_AUTOMATIC, None, &mut changes, cancelled)?;
    }
    Ok((pieces, changes))
}

/// What joining crossings in one layer gave: the strings in the order given,
/// none added or removed, the changes, and what the joined strings still
/// show.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayerJoin {
    pub(crate) pieces: Vec<Piece>,
    pub(crate) changes: Vec<Change>,
    pub(crate) problems: Vec<Problem>,
    pub(crate) odd_ones: Vec<OddOneOut>,
    /// As [`LayerClean::left_out`].
    pub(crate) left_out: Vec<usize>,
}

/// Join at the halfway height every crossing between the strings within
/// the merge distance of one of `at` and missing by [`JOIN_ON_REQUEST`] or
/// less, all at once: first a shared vertex at every one of them on every
/// string meeting there, heights read from the strings as they are, then
/// each set to its mean, so no join moves another. `Err` once `cancelled`
/// says so.
pub(crate) fn join_layer(strings: &[Vec<DVec3>], at: &[DVec2], cancelled: &dyn Fn() -> bool) -> Result<LayerJoin, Cancelled> {
    let (pieces, changes) = join_steps(strings, at, cancelled)?;
    let strings: Vec<Vec<DVec3>> = pieces.iter().map(|piece| piece.verts.clone()).collect();
    let problems = check::problems(&strings, cancelled)?;
    Ok(LayerJoin {
        odd_ones: odd_ones_out(&strings, &problems, cancelled)?,
        left_out: would_leave_out(&strings, &problems, cancelled)?,
        problems,
        pieces,
        changes,
    })
}

/// The joins of [`join_layer`]: the strings after them and the changes
/// made, nothing checked.
fn join_steps(strings: &[Vec<DVec3>], at: &[DVec2], cancelled: &dyn Fn() -> bool) -> Result<(Vec<Piece>, Vec<Change>), Cancelled> {
    let mut pieces: Vec<Piece> = strings.iter().enumerate().map(|(source, verts)| Piece { source, verts: verts.clone() }).collect();
    let mut changes = Vec::new();
    crossings::share_crossings(&mut pieces, JOIN_ON_REQUEST, Some(at), &mut changes, cancelled)?;
    Ok((pieces, changes))
}

/// What cleaning a copy of a build's control strings gave: the strings
/// after Clean Strings and Join all at halfway, each piece's `source` its
/// string's place in the strings given, and every change, its `source`
/// likewise.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CopyClean {
    pub(crate) pieces: Vec<Piece>,
    pub(crate) changes: Vec<Change>,
}

/// Clean a copy of `strings` as Clean Strings and then Join all at halfway
/// would, at every crossing Clean Strings leaves missing by
/// [`JOIN_ON_REQUEST`] or less: each layer on its own, `layers` giving each
/// string's, its strings in the order of `keys`, lower first, as Clean
/// Strings takes them in object id order. Pieces come back layer by layer
/// in the order the layers first appear in `strings`. `Err` once
/// `cancelled` says so.
pub(crate) fn clean_copy(strings: &[Vec<DVec3>], layers: &[u64], keys: &[u64], cancelled: &dyn Fn() -> bool) -> Result<CopyClean, Cancelled> {
    let key = |index: usize| keys.get(index).copied().unwrap_or(index as u64);
    let layer = |index: usize| layers.get(index).copied().unwrap_or(0);
    let mut order: Vec<u64> = Vec::new();
    for index in 0..strings.len() {
        if !order.contains(&layer(index)) {
            order.push(layer(index));
        }
    }
    let mut copy = CopyClean {
        pieces: Vec::new(),
        changes: Vec::new(),
    };
    for group in order {
        let mut members: Vec<usize> = (0..strings.len()).filter(|&index| layer(index) == group).collect();
        members.sort_by_key(|&index| (key(index), index));
        let given: Vec<Vec<DVec3>> = members.iter().map(|&index| strings[index].clone()).collect();
        let (cleaned, changes) = clean_steps(&given, cancelled)?;
        let to_given = |source: usize| members[source];
        let renamed = |change: &Change, name: &dyn Fn(usize) -> usize| {
            let mut change = change.clone();
            change.source = name(change.source);
            match &mut change.kind {
                ChangeKind::SharedCut { kept, .. } | ChangeKind::Removed { kept } => *kept = name(*kept),
                ChangeKind::Joined { others, .. } | ChangeKind::VertexShared { others, .. } => others.iter_mut().for_each(|other| *other = name(*other)),
                _ => {}
            }
            change
        };
        copy.changes.extend(changes.iter().map(|change| renamed(change, &to_given)));
        let pieces: Vec<Vec<DVec3>> = cleaned.iter().map(|piece| piece.verts.clone()).collect();
        let at: Vec<DVec2> = check::problems(&pieces, cancelled)?
            .iter()
            .filter(|problem| matches!(problem.kind, ProblemKind::Crossing { miss } if miss <= JOIN_ON_REQUEST))
            .map(|problem| problem.at.truncate())
            .collect();
        if at.is_empty() {
            copy.pieces.extend(cleaned.iter().map(|piece| Piece {
                source: to_given(piece.source),
                verts: piece.verts.clone(),
            }));
            continue;
        }
        // Join reads the cleaned pieces as its strings: a source there is
        // a place among them, named back through the piece's own source.
        let (joined, changes) = join_steps(&pieces, &at, cancelled)?;
        let through = |piece: usize| to_given(cleaned[piece].source);
        copy.changes.extend(changes.iter().map(|change| renamed(change, &through)));
        copy.pieces.extend(joined.iter().map(|piece| Piece {
            source: through(piece.source),
            verts: piece.verts.clone(),
        }));
    }
    Ok(copy)
}

/// Everything Build Surface's checks find in `strings`, each place once, as
/// step 5 finds it: the strings are only read. The crossings come first,
/// biggest miss first. `Err` once `cancelled` says so.
pub(crate) fn problems(strings: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<Vec<Problem>, Cancelled> {
    check::problems(strings, cancelled)
}

/// A string that sits on one side of every string it misses by more than
/// [`JOIN_ON_REQUEST`], by a similar amount, and misses two strings or
/// more so: likely the one to fix.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OddOneOut {
    /// By place in the strings checked.
    pub(crate) piece: usize,
    /// Below the strings it misses, else above them.
    pub(crate) below: bool,
    /// The smallest and the biggest of those misses, in metres.
    pub(crate) low: f64,
    pub(crate) high: f64,
    /// How many crossings miss by more than [`JOIN_ON_REQUEST`], and how
    /// many places the string meets another in all.
    pub(crate) count: usize,
    pub(crate) total: usize,
}

/// The strings of `strings` whose every crossing missing by more than
/// [`JOIN_ON_REQUEST`] in `problems` puts them on the same side of the
/// other string, such crossings with two other strings or more, their
/// misses spread by no more than [`ODD_ONE_OUT_SPREAD_SHARE`] of the
/// smallest. A string missing only one other, however often, is not named:
/// the pair alone cannot say which of the two is out. In string order.
/// Facts only: nothing is changed. `Err` once `cancelled` says so.
pub(crate) fn odd_ones_out(strings: &[Vec<DVec3>], problems: &[Problem], cancelled: &dyn Fn() -> bool) -> Result<Vec<OddOneOut>, Cancelled> {
    let mut offsets: Vec<Vec<(usize, f64)>> = vec![Vec::new(); strings.len()];
    for problem in problems {
        let ProblemKind::Crossing { miss } = problem.kind else {
            continue;
        };
        if miss <= JOIN_ON_REQUEST {
            continue;
        }
        if let [one, other] = problem.sides.as_slice()
            && let (Some(high_one), Some(high_other)) = (one.height, other.height)
            && one.piece != other.piece
            && one.piece < strings.len()
            && other.piece < strings.len()
        {
            offsets[one.piece].push((other.piece, high_one - high_other));
            offsets[other.piece].push((one.piece, high_other - high_one));
        }
    }
    let mut counts: Option<Vec<usize>> = None;
    let mut odd = Vec::new();
    for (piece, offsets) in offsets.iter().enumerate() {
        let Some(&(first, offset)) = offsets.first() else {
            continue;
        };
        let below = offset < 0.0;
        if offsets.iter().all(|&(other, _)| other == first) || !offsets.iter().all(|&(_, offset)| (offset < 0.0) == below) {
            continue;
        }
        let low = offsets.iter().map(|(_, offset)| offset.abs()).fold(f64::INFINITY, f64::min);
        let high = offsets.iter().map(|(_, offset)| offset.abs()).fold(0.0, f64::max);
        if high - low > ODD_ONE_OUT_SPREAD_SHARE * low {
            continue;
        }
        if counts.is_none() {
            counts = Some(check::crossing_counts(strings, cancelled)?);
        }
        let total = counts.as_ref().map_or(0, |counts| counts[piece]);
        odd.push(OddOneOut {
            piece,
            below,
            low,
            high,
            count: offsets.len(),
            total: total.max(offsets.len()),
        });
    }
    Ok(odd)
}
