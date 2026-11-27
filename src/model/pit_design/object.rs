//! A pit design as a design object.
//!
//! A pit design lives in a design layer like any polyline: its own vertices
//! are the closed string it is built from (the floor's toe built up, the
//! crest built down), and it draws as that string and the benches and ramp
//! designed from it, with its settings.
//!
//! The benches are worked out from the string and the settings, in
//! the background, and kept on the object with what they were worked out
//! from ([`Derived`]): moving the object moves them with it, and any other
//! change leaves them to be worked out again. They are derived, so working
//! them out again is not an edit of its own: undo takes back the change that
//! made them stale, with the benches it had before.
//!
//! Its surface is made as any other's, with Create Triangulation, which takes
//! every string it draws.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use super::{Design, Problem, settings::PitDesignSettings};
use crate::model::{PolyVertex, triangulation::GeometryVersion};

/// What a pit design object holds beyond its string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PitObject {
    /// What it is called.
    pub(crate) name: String,
    /// What it is designed with.
    pub(crate) settings: PitDesignSettings,
    /// The design as last worked out, and what from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) derived: Option<Derived>,
    /// What the design takes out of the ground, as last measured, and what
    /// from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) evaluation: Option<super::evaluation::Evaluation>,
}

/// What the string and settings came to when last worked out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Derived {
    /// [`inputs_key`] of the string and settings it was worked out from.
    pub(crate) inputs: u64,
    /// The version of the topography it daylit against, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ground: Option<GeometryVersion>,
    pub(crate) outcome: Outcome,
}

/// A design, or why the settings make none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    Design(Box<Design>),
    /// The string is not closed round some area.
    NotClosed,
    Problem(Problem),
}

impl PitObject {
    pub(crate) fn new(name: String, settings: PitDesignSettings) -> Self {
        Self {
            name,
            settings,
            derived: None,
            evaluation: None,
        }
    }

    /// What it is designed with, as a copy.
    pub(crate) fn settings(&self) -> PitDesignSettings {
        self.settings.clone()
    }

    /// Where the design meets the topography, as last measured for the
    /// design it has now: none where it has not been measured since.
    pub(crate) fn daylight(&self) -> &[super::DesignString] {
        match (&self.derived, &self.evaluation) {
            (Some(derived), Some(evaluation)) if evaluation.design == derived.inputs => &evaluation.daylight,
            _ => &[],
        }
    }

    /// Its rehabilitated landform's contours, as last measured for the design
    /// it has now: none where it has not been, or is not rehabilitated.
    pub(crate) fn rehab_strings(&self) -> &[super::DesignString] {
        match (&self.derived, &self.evaluation) {
            (Some(derived), Some(evaluation)) if evaluation.design == derived.inputs => evaluation.rehab.as_ref().map_or(&[][..], |rehab| &rehab.strings),
            _ => &[],
        }
    }

    /// The design worked out, if it has been and there is one.
    pub(crate) fn design(&self) -> Option<&Design> {
        match &self.derived.as_ref()?.outcome {
            Outcome::Design(design) => Some(design),
            _ => None,
        }
    }

    /// Whether what was worked out is what `verts` and the settings
    /// come to now, against the ground at version `ground` (for a design that
    /// daylights).
    pub(crate) fn is_fresh(&self, verts: &[PolyVertex], ground: Option<GeometryVersion>) -> bool {
        let settings = self.settings();
        self.derived
            .as_ref()
            .is_some_and(|derived| derived.inputs == inputs_key(verts, &settings) && (settings.ground().is_none() || derived.ground == ground))
    }

    /// It as it stands once designed: no longer tied to the topography it
    /// was designed against, nor the block model and shell it was measured
    /// against. A design that daylit keeps the bench count it reached, so it
    /// comes out the same without the ground; the design and its measure,
    /// both current, are kept as they are, under the settled settings.
    /// `None` when there is nothing to settle, or it cannot be yet: a
    /// daylighting design not worked out for what it is now has no count to
    /// keep.
    pub(crate) fn settled(&self, verts: &[PolyVertex]) -> Option<PitObject> {
        if !self.has_links() {
            return None;
        }
        let before = inputs_key(verts, &self.settings);
        let current = self.derived.as_ref().filter(|derived| derived.inputs == before);
        let mut settled = self.clone();
        if self.settings.daylights() {
            let benches = match &current?.outcome {
                Outcome::Design(design) => design.benches,
                // Settings that make no design have no count to keep: what
                // they make without the ground is the same nothing.
                Outcome::NotClosed | Outcome::Problem(_) => self.settings.benches,
            };
            settled.settings.stop = super::settings::StopMode::Benches;
            settled.settings.benches = benches.max(1);
        }
        settled.settings.topography = None;
        settled.settings.evaluation.block_model = None;
        settled.settings.evaluation.shell = None;
        let after = inputs_key(verts, &settled.settings);
        if current.is_some()
            && let Some(derived) = &mut settled.derived
        {
            derived.inputs = after;
            derived.ground = None;
        }
        if let Some(evaluation) = settled.evaluation.as_mut().filter(|evaluation| evaluation.design == before) {
            evaluation.design = after;
        }
        Some(settled)
    }

    /// Whether it links to anything it is designed or measured against: what
    /// it lets go of as it settles.
    pub(crate) fn has_links(&self) -> bool {
        self.settings.topography.is_some() || self.settings.evaluation.block_model.is_some() || self.settings.evaluation.shell.is_some()
    }

    /// Move it by `delta` with its string: its ramp starts, and the design
    /// worked out, which moves with the string unless it daylights against
    /// ground that stays put. `verts` is the string after the move.
    pub(crate) fn translate(&mut self, delta: DVec3, verts: &[PolyVertex]) {
        for start in self.settings.ramps.iter_mut().filter_map(|ramp| ramp.start.as_mut()) {
            *start += delta.truncate();
        }
        let settings = self.settings();
        if let Some(derived) = &mut self.derived {
            if let Outcome::Design(design) = &mut derived.outcome {
                **design = design.translated(delta);
            }
            // Against ground, the moved design is no longer what it comes to:
            // it is left stale, to be worked out again.
            if settings.ground().is_none() {
                derived.inputs = inputs_key(verts, &settings);
            }
        }
    }
}

/// A fingerprint of what a design is worked out from: its string and its
/// settings. FNV-1a, the same on every build and platform.
pub(crate) fn inputs_key(verts: &[PolyVertex], settings: &PitDesignSettings) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    mix(&(verts.len() as u64).to_le_bytes());
    for vertex in verts {
        for value in [vertex.pos.x, vertex.pos.y, vertex.pos.z, vertex.bulge] {
            mix(&value.to_bits().to_le_bytes());
        }
    }
    // What it is measured against changes nothing designed.
    let designed = PitDesignSettings {
        evaluation: super::evaluation::EvaluationSettings::default(),
        rehab: None,
        ..settings.clone()
    };
    mix(&serde_json::to_vec(&designed).unwrap_or_default());
    hash
}

/// The string a pit design is worked out from, as points: arcs followed at
/// the engine's own step round a bend, so a rounded toe gives the benches no
/// more points than they would make rounding it themselves.
pub(crate) fn base_points(verts: &[PolyVertex]) -> Vec<DVec3> {
    let count = verts.len();
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let (start, end) = (verts[index], verts[(index + 1) % count].pos);
        points.push(start.pos);
        let sweep = 4.0 * start.bulge.atan();
        let steps = (sweep.abs() / super::ring::ARC_STEP).ceil() as usize;
        if steps < 2 {
            continue;
        }
        let (a, b) = (start.pos.truncate(), end.truncate());
        let chord = b - a;
        let length = chord.length();
        if length <= f64::EPSILON {
            continue;
        }
        // The arc's centre, left of the chord for a positive bulge.
        let sagitta_ratio = (1.0 - start.bulge * start.bulge) / (4.0 * start.bulge);
        let centre = a.lerp(b, 0.5) + chord.perp() * sagitta_ratio;
        let from = (a - centre).to_angle();
        let radius = (a - centre).length();
        for step in 1..steps {
            let t = step as f64 / steps as f64;
            let point = centre + glam::DVec2::from_angle(from + sweep * t) * radius;
            points.push(point.extend(start.pos.z + (end.z - start.pos.z) * t));
        }
    }
    points
}

/// A closed string through `points` with each corner rounded to an arc of
/// `radius`, or as large an arc as its two edges leave room for: each corner
/// becomes the arc's two ends, the first carrying its bulge. Corners all but
/// straight are left as they are.
pub(crate) fn rounded_corners(points: &[DVec3], radius: f64) -> Vec<PolyVertex> {
    let count = points.len();
    if count < 3 || radius <= 0.0 {
        return points.iter().copied().map(PolyVertex::straight).collect();
    }
    (0..count)
        .flat_map(|index| rounded_corner(points[(index + count - 1) % count], points[index], points[(index + 1) % count], radius))
        .collect()
}

/// [`rounded_corners`] for a string not yet closed, as it is being drawn:
/// its corners between its first and last points rounded, those two left
/// as they are until it closes.
pub(crate) fn rounded_open(points: &[DVec3], radius: f64) -> Vec<PolyVertex> {
    let count = points.len();
    if count < 3 || radius <= 0.0 {
        return points.iter().copied().map(PolyVertex::straight).collect();
    }
    let mut verts = vec![PolyVertex::straight(points[0])];
    for index in 1..count - 1 {
        verts.extend(rounded_corner(points[index - 1], points[index], points[index + 1], radius));
    }
    verts.push(PolyVertex::straight(points[count - 1]));
    verts
}

/// The corner at `corner`, between edges from `before` and to `after`, as
/// an arc of `radius` (or as large a one as half of each edge leaves room
/// for): its two ends, the first carrying its bulge; the corner itself
/// where it is all but straight.
fn rounded_corner(before: DVec3, corner: DVec3, after: DVec3, radius: f64) -> Vec<PolyVertex> {
    let (back, ahead) = ((before - corner).truncate(), (after - corner).truncate());
    let (back_length, ahead_length) = (back.length(), ahead.length());
    if back_length <= f64::EPSILON || ahead_length <= f64::EPSILON {
        return vec![PolyVertex::straight(corner)];
    }
    let (u, v) = (back / back_length, ahead / ahead_length);
    let opening = u.dot(v).clamp(-1.0, 1.0).acos();
    // The turn the arc makes, and whether to the left.
    let turn = std::f64::consts::PI - opening;
    if turn < 1.0e-3 || opening < 1.0e-6 {
        return vec![PolyVertex::straight(corner)];
    }
    // Each edge gives half its length at most, so neighbouring arcs meet
    // and never cross.
    let reach = (radius / (opening * 0.5).tan()).min(back_length * 0.5).min(ahead_length * 0.5);
    let left = (-u).perp_dot(v) > 0.0;
    let entry = corner + (before - corner) * (reach / back_length);
    let exit = corner + (after - corner) * (reach / ahead_length);
    let bulge = (turn / 4.0).tan() * if left { 1.0 } else { -1.0 };
    vec![PolyVertex { pos: entry, bulge }, PolyVertex::straight(exit)]
}
