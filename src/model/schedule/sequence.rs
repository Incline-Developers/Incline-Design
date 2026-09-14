//! Dig sequences: the ordered ground a loader is asked to work through, and
//! the references that have to survive a save, a reopen and a rerun.
//!
//! # The identity contract
//!
//! A dig block cannot be referenced by its id: [`crate::model::DigBlockId`]s
//! come from a counter that restarts with the process, and the ledger that
//! hands the same ground the same id across a rerun lives in memory only.
//! Reopen a project, rerun Solids, and block 7 is whatever the partition
//! produced seventh - most likely someone else's ground.
//!
//! Nor can it be referenced by a point and an area: a 10 × 10 square and a
//! 20 × 5 rectangle share a centre and an area and are different ground, and
//! a flitch re-cut from RL 0–6 to RL 0–3 keeps its plan footprint while
//! holding half the material. A reference has to name the *whole* authored
//! ground, or a schedule planned against one shape silently executes
//! another.
//!
//! So a stored reference names the ground it was captured from, in two
//! layers, each answering a question the other cannot:
//!
//! - **Provenance**: a [`GroundSourceStamp`] naming the source surfaces the
//!   Solids run cut the block from, and which version of each. Two
//!   cuts of one surface can agree in outline, band and volume while holding
//!   different material - a slope and its mirror are the standing example -
//!   so no geometric number, alone, can identify ground. The stamp can: it
//!   changes exactly when the sources are re-cut, and it is deliberately
//!   conservative - a source edit invalidates every member of that solid,
//!   even one whose ground happened not to move, because "happened not to
//!   move" is not something this layer can prove.
//! - **Geometry**: the solid by its durable id; the flitch's base **and
//!   top** RL; a point inside the footprint, used only to locate candidates;
//!   a canonical digest of the footprint (every ring, holes included,
//!   quantized to a 1 µm lattice with ring order, winding and collinear
//!   vertices normalized away); and the block's clipped volume. These catch
//!   the changes a stamp cannot see - re-benching and strip edits re-cut the
//!   *same* sources into different ground.
//!
//! Resolving, in order: is the solid still there; are the sources still the
//! sources (stamp comparison); and then does exactly one block of this solid,
//! in this band, cover the stored point with this footprint and this volume?
//! Only an unqualified yes resolves. Every other answer keeps the member in
//! place and unresolved, with the most specific true reason. Nothing is ever
//! dropped, renamed, matched by display name, or rebound through a `replaces`
//! lineage.
//!
//! Every field of a reference is required. Scheduling is in development and
//! carries no backwards compatibility: a reference is captured whole by
//! [`App::dig_block_reference`](crate::app::App::dig_block_reference) or it
//! does not exist, so there is no weaker kind to resolve, and no branch that
//! has to decide how much of a reference it can trust.
//!
//! Measured grades and tonnages are deliberately *not* part of identity: a
//! block whose model was re-measured is the same ground with new figures,
//! and the next readiness report reads the new figures without the schedule
//! having to be re-authored.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::{
    i18n::tr,
    model::{
        SolidId,
        arrangement::{Face, point_in_face},
        triangulation::GroundSourceStamp,
    },
};

/// How far a flitch's base or top RL may move and still be the same band.
///
/// The same figure the Solids partition matches its own identities with, so a
/// reference and a rerun agree on which band they are talking about.
const FLITCH_TOLERANCE: f64 = 1e-6;

/// How far a block's clipped volume may move and still be the same ground.
///
/// Like the footprint digest, this is a same-ground check rather than a
/// similarity one: rebuilding unchanged inputs reproduces the volume to the
/// last bits, so the tolerance only has to absorb floating-point noise. A
/// surface edit that moves material fails it and leaves the member
/// unresolved, which is the answer the schedule needs: its tonnes are no
/// longer the tonnes it was planned against.
const VOLUME_RELATIVE_TOLERANCE: f64 = 1e-6;
const VOLUME_ABSOLUTE_TOLERANCE: f64 = 1e-6;

/// The lattice, in metres, that footprint vertices are quantized to before
/// hashing. Small enough that any real boundary sits far from a cell edge at
/// mine scale, coarse enough that sub-micron floating-point noise in an
/// unchanged rebuild lands in the same cell.
const FOOTPRINT_QUANTUM: f64 = 1e-6;

/// A canonical digest of a block's plan footprint: every ring, holes
/// included.
///
/// Equivalence is by construction: both sides are reduced to the same
/// canonical form - vertices quantized to the [`FOOTPRINT_QUANTUM`] lattice,
/// consecutive duplicates and collinear vertices dropped, winding turned to
/// counter-clockwise, each ring rotated to start at its least vertex, rings
/// sorted - and then fingerprinted with two FNV-1a lanes, one over the bytes
/// forward and one backward, so no single lane decides a match. This is a
/// fingerprint for exact canonical-form comparison, not a cryptographic
/// commitment; it is one input to identity, never the whole proof of it -
/// the source stamp in [`DigBlockRef`](super::DigBlockRef) carries what
/// geometry cannot. Ring order, winding and vertex-count differences in an
/// unchanged rerun hash identically; any moved, added or removed boundary
/// does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct Footprint(#[serde(with = "footprint_bytes")] pub(crate) [u64; 2]);

/// Two lanes of little-endian bytes in a JSON array; spelled out so the wire
/// form of a digest cannot drift between builds.
mod footprint_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: &[u64; 2], serializer: S) -> std::result::Result<S::Ok, S::Error> {
        [value[0].to_le_bytes(), value[1].to_le_bytes()].serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<[u64; 2], D::Error> {
        let bytes = <[[u8; 8]; 2]>::deserialize(deserializer)?;
        Ok([u64::from_le_bytes(bytes[0]), u64::from_le_bytes(bytes[1])])
    }
}

impl Footprint {
    pub(crate) fn of(face: &[Vec<DVec2>]) -> Self {
        let rings = canonical_rings(face);
        let mut stream = Vec::new();
        stream.extend_from_slice(&(rings.len() as u64).to_le_bytes());
        for ring in &rings {
            stream.extend_from_slice(&(ring.len() as u64).to_le_bytes());
            for [x, y] in ring {
                stream.extend_from_slice(&x.to_le_bytes());
                stream.extend_from_slice(&y.to_le_bytes());
            }
        }
        Self([
            fnv1a(stream.iter().copied(), 0xcbf2_9ce4_8422_2325),
            fnv1a(stream.iter().rev().copied(), 0x9e37_79b9_7f4a_7c15),
        ])
    }
}

/// FNV-1a over a byte stream: small, fixed, and owned by this file, so a
/// digest written by one build reads the same in the next.
fn fnv1a(bytes: impl IntoIterator<Item = u8>, mut hash: u64) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    for byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// One footprint reduced to its canonical integer form. See [`Footprint`].
fn canonical_rings(face: &[Vec<DVec2>]) -> Vec<Vec<[i64; 2]>> {
    let quantize = |value: f64| (value / FOOTPRINT_QUANTUM).round() as i64;
    let mut rings = Vec::with_capacity(face.len());
    for ring in face {
        let mut points: Vec<[i64; 2]> = ring.iter().map(|point| [quantize(point.x), quantize(point.y)]).collect();
        // Quantization can fuse neighbours, including across the wrap.
        while points.len() > 1 && points[0] == points[points.len() - 1] {
            points.pop();
        }
        points.dedup_by(|a, b| a == b);
        if points.len() > 1 && points[0] == points[points.len() - 1] {
            points.pop();
        }
        // Drop collinear vertices until none remain: the same ground can be
        // emitted with or without the points where a cut met a boundary.
        // Integer lattice coordinates make the cross product exact. A spike -
        // a vertex doubling straight back on itself - is equally redundant,
        // and dropping it leaves the same segment either way.
        if points.len() >= 3 {
            loop {
                let len = points.len();
                let mut kept = Vec::with_capacity(len);
                for index in 0..len {
                    let previous = points[(index + len - 1) % len];
                    let current = points[index];
                    let next = points[(index + 1) % len];
                    if cross_z(previous, current, next) != 0 {
                        kept.push(current);
                    }
                }
                points = kept;
                points.dedup_by(|a, b| a == b);
                if points.len() == len || points.len() < 3 {
                    break;
                }
            }
        }
        if points.len() < 3 {
            // Fewer than three distinct cells enclose nothing.
            continue;
        }
        // Canonical winding: counter-clockwise, whatever the arrangement
        // emitted.
        if signed_area(&points) < 0 {
            points.reverse();
        }
        // Canonical start: the least vertex, so a rotation of the same cycle
        // hashes the same.
        if let Some(least) = points.iter().enumerate().min_by_key(|(_, point)| **point).map(|(index, _)| index) {
            points.rotate_left(least);
        }
        rings.push(points);
    }
    rings.sort();
    rings
}

/// The Z component of (b - a) × (c - b), on lattice coordinates.
fn cross_z(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> i128 {
    ((b[0] - a[0]) as i128) * ((c[1] - b[1]) as i128) - ((b[1] - a[1]) as i128) * ((c[0] - b[0]) as i128)
}

/// Twice the signed area of a lattice ring, exact in i128.
fn signed_area(ring: &[[i64; 2]]) -> i128 {
    ring.iter()
        .enumerate()
        .map(|(index, a)| {
            let b = ring[(index + 1) % ring.len()];
            (a[0] as i128) * (b[1] as i128) - (a[1] as i128) * (b[0] as i128)
        })
        .sum()
}

/// One dig block's ground as a current run describes it, which is what a
/// stored reference is matched against.
///
/// Built from the planning snapshot by the app layer; the model never sees a
/// mesh, a cache or a session id.
#[derive(Clone)]
pub(crate) struct BlockGround {
    pub(crate) solid: SolidId,
    /// Which source geometry this block was cut from.
    pub(crate) source: GroundSourceStamp,
    /// Base and top RL of the flitch the block was cut from.
    pub(crate) flitch_base: f64,
    pub(crate) flitch_top: f64,
    /// The block's footprint: outer ring first, then holes. Shared with the
    /// run that produced it rather than copied - see
    /// [`crate::app::commands::solids_view::DigPiece`].
    pub(crate) outline: std::sync::Arc<Face>,
    /// The footprint's canonical digest, computed once per report.
    pub(crate) footprint: Footprint,
    /// The block's clipped volume; `None` when it did not close.
    pub(crate) volume: Option<f64>,
    pub(crate) plan_area: f64,
}

/// A stored reference to one dig block's ground.
///
/// Every field is durable project data or plain geometry - nothing here is
/// allocated at runtime, so nothing here can alias a later session's block -
/// and every field is required. A reference either names its ground
/// completely or is not a reference.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DigBlockRef {
    /// The solid the block was cut from. Persistent document data.
    pub(crate) solid: SolidId,
    /// The source geometry the block was cut from, as the run that produced
    /// it stood when the reference was captured. Never refreshed: it records
    /// the provenance of the ground the user selected, and a later source
    /// edit is exactly what it exists to catch.
    pub(crate) source: GroundSourceStamp,
    /// Base RL of its flitch.
    pub(crate) flitch_base: f64,
    /// Top RL of its flitch.
    pub(crate) flitch_top: f64,
    /// A point inside the block's footprint, in plan. Taken from the
    /// partition's own anchor for the block, and used only as a probe: the
    /// current block's anchor may sit elsewhere, so long as this point is
    /// still inside the same ground.
    pub(crate) anchor: [f64; 2],
    /// The plan area the reference was captured against. Diagnostic only -
    /// identity is decided by the footprint digest, not the area.
    pub(crate) plan_area: f64,
    /// The canonical digest of the footprint.
    pub(crate) footprint: Footprint,
    /// The clipped volume the reference was captured against, `null` when the
    /// block did not close - the one field that may legitimately be empty,
    /// because an unclosed piece has no volume. Still *required* on the wire:
    /// serde would otherwise read a reference with no `volume` key at all as
    /// one that did not close, which is a statement about the ground and not
    /// something a missing key is entitled to make.
    #[serde(deserialize_with = "Option::deserialize")]
    pub(crate) volume: Option<f64>,
}

impl DigBlockRef {
    /// The ground this reference names, ignoring where in it the pick sat.
    ///
    /// Two references to one block differ in their anchor (and nothing
    /// else), so membership checks compare this rather than the whole
    /// reference.
    pub(crate) fn ground_identity(&self) -> GroundIdentity {
        GroundIdentity {
            solid: self.solid,
            source: self.source,
            flitch_base: self.flitch_base,
            flitch_top: self.flitch_top,
            footprint: self.footprint,
        }
    }
}

/// What [`DigBlockRef::ground_identity`] compares on. Private on purpose:
/// this is the one notion of "the same ground" the schedule has.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct GroundIdentity {
    solid: SolidId,
    source: GroundSourceStamp,
    flitch_base: f64,
    flitch_top: f64,
    footprint: Footprint,
}

/// A hashable stand-in for [`GroundIdentity`], so resolving many references
/// against many blocks is one lookup each rather than a walk of the list.
///
/// Equality here is equality there. The RLs go in as bits with zero
/// normalised, so `-0.0` and `0.0` still name one flitch; a non-finite RL is
/// the single divergence, and benching produces none.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GroundKey {
    solid: SolidId,
    source: GroundSourceStamp,
    flitch_base: u64,
    flitch_top: u64,
    footprint: Footprint,
}

impl GroundIdentity {
    pub(crate) fn key(&self) -> GroundKey {
        let bits = |value: f64| if value == 0.0 { 0.0_f64.to_bits() } else { value.to_bits() };
        GroundKey {
            solid: self.solid,
            source: self.source,
            flitch_base: bits(self.flitch_base),
            flitch_top: bits(self.flitch_top),
            footprint: self.footprint,
        }
    }
}

/// What a current run says about one stored reference.
///
/// Everything but [`Self::Resolved`] keeps the member in the sequence and
/// explains itself; none of them removes it, guesses a replacement, or
/// substitutes a tonnage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RefStatus {
    /// Index into the ground list this was resolved against.
    Resolved(usize),
    /// The solid itself is gone from the project.
    SolidMissing,
    /// The solid is there, but no flitch sits at that RL any more - a
    /// benching change, most often.
    FlitchMissing,
    /// The flitch still starts at that RL but runs to a different top: the
    /// ground was re-flitched, and this band is not the band it was.
    FlitchTopChanged { now: f64 },
    /// The flitch is there and no block covers the stored point: the ground
    /// was cut away, or the footprint moved out from under it.
    GroundMissing,
    /// More than one block covers the stored point, so which one is meant
    /// cannot be decided. Overlapping footprints should not happen; if they
    /// do, saying so beats picking one.
    GroundAmbiguous(usize),
    /// One block covers the point, but it is not the same ground: its
    /// footprint has been split, merged or reshaped since. `was`/`now` are
    /// plan areas.
    GroundChanged { was: f64, now: f64 },
    /// The footprint is unchanged but the ground's volume moved: the ground
    /// was re-cut within the same band. `was`/`now` are cubic metres, or
    /// `None` when one side did not close.
    VolumeChanged { was: Option<f64>, now: Option<f64> },
    /// The sources the ground was cut from have been re-cut or replaced
    /// since the reference was captured. The block may even look the same -
    /// a slope and its mirror share every number but the material's - so the
    /// reference stands down until it is reselected or reconfirmed.
    SourceChanged,
}

impl RefStatus {
    pub(crate) fn resolved(self) -> Option<usize> {
        match self {
            Self::Resolved(index) => Some(index),
            _ => None,
        }
    }

    /// What to tell the user about a reference that did not resolve.
    pub(crate) fn message(self) -> Option<String> {
        Some(match self {
            Self::Resolved(_) => return None,
            Self::SolidMissing => tr!("sequence-unresolved-solid"),
            Self::FlitchMissing => tr!("sequence-unresolved-flitch"),
            Self::FlitchTopChanged { now } => tr!("sequence-unresolved-flitch-top", now = format!("{now:.1}")),
            Self::GroundMissing => tr!("sequence-unresolved-ground"),
            Self::GroundAmbiguous(count) => tr!("sequence-unresolved-ambiguous", count = count.to_string()),
            Self::GroundChanged { was, now } => tr!("sequence-unresolved-changed", was = format!("{was:.1}"), now = format!("{now:.1}")),
            Self::VolumeChanged { was, now } => tr!(
                "sequence-unresolved-volume",
                was = was.map_or_else(|| tr!(literal = "—").to_owned(), |value| format!("{value:.1}")),
                now = now.map_or_else(|| tr!(literal = "—").to_owned(), |value| format!("{value:.1}"))
            ),
            Self::SourceChanged => tr!("sequence-unresolved-source"),
        })
    }
}

impl DigBlockRef {
    /// Which block of the current run this reference means, or why none of
    /// them does.
    ///
    /// The order of the checks is the order of the questions a person would
    /// ask, so the reason handed back is the most specific true one: a
    /// deleted solid does not report as missing ground, a re-cut source does
    /// not report as changed ground, and a re-flitched band does not report
    /// as missing ground.
    pub(crate) fn resolve(&self, ground: &[BlockGround]) -> RefStatus {
        // The sources first, before any geometric question: whatever the
        // current run holds for this solid, it was cut from the current
        // sources, and a reference cut from earlier sources stands down even
        // when every number that follows would agree - a slope and its
        // mirror agree on all of them.
        let mut seen_solid = false;
        let mut current_source = None;
        for block in ground {
            if block.solid == self.solid {
                seen_solid = true;
                current_source.get_or_insert(block.source);
            }
        }
        if !seen_solid {
            return RefStatus::SolidMissing;
        }
        if current_source.is_some_and(|current| current != self.source) {
            return RefStatus::SourceChanged;
        }
        let anchor = glam::DVec2::new(self.anchor[0], self.anchor[1]);
        let mut seen_base = false;
        let mut band_top = None;
        let mut covering = Vec::new();
        for (index, block) in ground.iter().enumerate() {
            if block.solid != self.solid {
                continue;
            }
            if (block.flitch_base - self.flitch_base).abs() > FLITCH_TOLERANCE {
                continue;
            }
            seen_base = true;
            band_top.get_or_insert(block.flitch_top);
            let band = (block.flitch_top - self.flitch_top).abs() <= FLITCH_TOLERANCE;
            if band && point_in_face(&block.outline, anchor) {
                covering.push(index);
            }
        }
        if !seen_base {
            return RefStatus::FlitchMissing;
        }
        // A band whose top moved: the ground at this base is no longer cut
        // the way this reference was captured against. `seen_band` false
        // means every flitch at this base runs to some other top, so this is
        // a re-flitch rather than missing ground.
        if covering.is_empty()
            && let Some(now) = band_top.filter(|now| (now - self.flitch_top).abs() > FLITCH_TOLERANCE)
        {
            return RefStatus::FlitchTopChanged { now };
        }
        match covering.as_slice() {
            [] => RefStatus::GroundMissing,
            [index] => {
                let block = &ground[*index];
                if block.footprint != self.footprint {
                    return RefStatus::GroundChanged {
                        was: self.plan_area,
                        now: block.plan_area,
                    };
                }
                if !same_volume(self.volume, block.volume) {
                    return RefStatus::VolumeChanged {
                        was: self.volume,
                        now: block.volume,
                    };
                }
                RefStatus::Resolved(*index)
            }
            many => RefStatus::GroundAmbiguous(many.len()),
        }
    }

    /// Whether this reference is well formed enough to be worth resolving.
    /// Checked on load: a reference with a non-finite anchor would match
    /// nothing for reasons no diagnostic could explain.
    pub(crate) fn is_well_formed(&self) -> bool {
        self.flitch_base.is_finite()
            && self.flitch_top.is_finite()
            && self.anchor.iter().all(|value| value.is_finite())
            && self.plan_area.is_finite()
            && self.plan_area > 0.0
            && self.volume.is_none_or(|volume| volume.is_finite() && volume >= 0.0)
    }
}

/// Whether two clipped volumes describe the same ground. See
/// [`VOLUME_RELATIVE_TOLERANCE`]. Two unclosed pieces are alike; a closed
/// piece and an unclosed one are not.
fn same_volume(was: Option<f64>, now: Option<f64>) -> bool {
    match (was, now) {
        (None, None) => true,
        (Some(was), Some(now)) => {
            let tolerance = VOLUME_ABSOLUTE_TOLERANCE.max(was.abs() * VOLUME_RELATIVE_TOLERANCE);
            (was - now).abs() <= tolerance
        }
        _ => false,
    }
}

/// An ordered run of ground, named by the user: the dig order one Gantt bar
/// works through.
///
/// Order is the whole point: it is the order the blocks are dug in, so
/// membership is a `Vec` rather than a set, and the same ground cannot appear
/// in it twice.
///
/// Exactly one thing owns one of these: a [`ScheduleBar`](super::ScheduleBar),
/// and nothing else. The stage 2A standalone sequence is gone and its
/// membership moved here unchanged, so the project holds one notion of "an
/// ordered run of ground" rather than two that could drift apart.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DigOrder {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) members: Vec<DigBlockRef>,
}

impl DigOrder {
    pub(crate) fn new(name: String) -> Self {
        Self { name, members: Vec::new() }
    }

    pub(crate) fn members(&self) -> &[DigBlockRef] {
        &self.members
    }

    /// Whether this order already holds the ground `block` names.
    ///
    /// By ground identity - where in the ground the pick sat is not part of
    /// what an order claims - so picking the same block twice through
    /// different anchors is still a duplicate. A file's duplicate ground is
    /// caught the same way by the load check.
    pub(crate) fn contains(&self, block: &DigBlockRef) -> bool {
        self.members.iter().any(|member| member.ground_identity() == block.ground_identity())
    }

    /// Put ground at `position`, clamped to the end.
    pub(crate) fn insert(&mut self, position: usize, block: DigBlockRef) -> super::ScheduleResult {
        if !block.is_well_formed() {
            return Err(super::ScheduleError::MalformedReference);
        }
        if self.contains(&block) {
            return Err(super::ScheduleError::DuplicateMember);
        }
        let position = position.min(self.members.len());
        self.members.insert(position, block);
        Ok(())
    }

    pub(crate) fn remove(&mut self, position: usize) -> super::ScheduleResult {
        if position >= self.members.len() {
            return Err(super::ScheduleError::UnknownMember);
        }
        self.members.remove(position);
        Ok(())
    }

    /// Move one block to a different place in the dig order.
    pub(crate) fn move_member(&mut self, from: usize, to: usize) -> super::ScheduleResult {
        if from >= self.members.len() || to >= self.members.len() {
            return Err(super::ScheduleError::UnknownMember);
        }
        let block = self.members.remove(from);
        self.members.insert(to, block);
        Ok(())
    }

    /// Check membership read back from a file.
    ///
    /// Shape only. Whether the ground is still there is a question for the
    /// current run, asked every time an order is measured - a file that opens
    /// on a project whose Solids have not been rerun is not a broken file.
    ///
    /// Duplicates are judged on exact stored equality, which is file
    /// corruption; the same *ground* held under two anchors is not a broken
    /// file - capture refuses it, but a saved plan can carry it - so it
    /// passes here and the readiness report names it.
    pub(crate) fn check_loaded(&self) -> super::ScheduleResult {
        for (position, member) in self.members.iter().enumerate() {
            if !member.is_well_formed() {
                return Err(super::ScheduleError::MalformedReference);
            }
            if self.members[..position].contains(member) {
                return Err(super::ScheduleError::DuplicateMember);
            }
        }
        Ok(())
    }
}
