//! A pit design as a design object.
//!
//! A pit design lives in a design layer like any polyline: its own vertices
//! are the closed string it is built from (the floor's toe built up, the
//! crest built down), and it draws as that string and the benches and ramp
//! designed from it. What it is built with is its scenarios, each a full set
//! of settings, one active.
//!
//! The benches are worked out from the string and the active scenario, in
//! the background, and kept on the object with what they were worked out
//! from ([`Derived`]): moving the object moves them with it, and any other
//! change leaves them to be worked out again. They are derived, so working
//! them out again is not an edit of its own: undo takes back the change that
//! made them stale, with the benches it had before.
//!
//! The pit's surface is a surface item, which a design layer cannot hold:
//! the object remembers which one it built, and from what.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use super::{Design, Problem, settings::PitDesignSettings};
use crate::model::{
    PolyVertex,
    triangulation::{GeometryVersion, TriangulationId},
};

/// A scenario's id within its design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ScenarioId(pub(crate) u64);

/// What a pit design object holds beyond its string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PitObject {
    /// What it is called: its surface is named after it.
    pub(crate) name: String,
    pub(crate) scenarios: Vec<Scenario>,
    pub(crate) active: ScenarioId,
    pub(crate) next_scenario_id: u64,
    /// The design as last worked out, and what from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) derived: Option<Derived>,
    /// The surface it built, and what from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) surface: Option<SurfaceLink>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Scenario {
    pub(crate) id: ScenarioId,
    pub(crate) name: String,
    pub(crate) settings: PitDesignSettings,
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

/// The surface a pit design built.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SurfaceLink {
    pub(crate) id: TriangulationId,
    /// [`inputs_key`] of what it was built from.
    pub(crate) inputs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ground: Option<GeometryVersion>,
    /// Whether it was trimmed to the topography.
    pub(crate) trimmed: bool,
    /// Its geometry as built: any other version is an edit made since.
    pub(crate) geometry: GeometryVersion,
}

/// How a pit design's surface stands against the design.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SurfaceStanding {
    /// None built, or the one built has been deleted.
    #[default]
    NotBuilt,
    Current,
    /// The design has changed since it was built.
    OutOfDate,
    /// The surface has been edited since it was built.
    Edited,
}

impl PitObject {
    /// A design with one scenario.
    pub(crate) fn new(name: String, scenario_name: String, settings: PitDesignSettings) -> Self {
        Self {
            name,
            scenarios: vec![Scenario {
                id: ScenarioId(0),
                name: scenario_name,
                settings,
            }],
            active: ScenarioId(0),
            next_scenario_id: 1,
            derived: None,
            surface: None,
        }
    }

    /// The active scenario: the one `active` names, or the first if it names
    /// none (a design always has one).
    pub(crate) fn scenario(&self) -> Option<&Scenario> {
        self.scenarios.iter().find(|scenario| scenario.id == self.active).or_else(|| self.scenarios.first())
    }

    pub(crate) fn scenario_mut(&mut self) -> Option<&mut Scenario> {
        let active = self.scenario()?.id;
        self.scenarios.iter_mut().find(|scenario| scenario.id == active)
    }

    /// The active scenario's settings.
    pub(crate) fn settings(&self) -> PitDesignSettings {
        self.scenario().map(|scenario| scenario.settings.clone()).unwrap_or_default()
    }

    /// Add a copy of the active scenario under `name` and make it active.
    pub(crate) fn add_scenario(&mut self, name: String) -> ScenarioId {
        let id = ScenarioId(self.next_scenario_id.max(self.scenarios.iter().map(|scenario| scenario.id.0 + 1).max().unwrap_or(0)));
        self.next_scenario_id = id.0 + 1;
        let settings = self.settings();
        self.scenarios.push(Scenario { id, name, settings });
        self.active = id;
        id
    }

    /// Remove the active scenario, unless it is the last, and make its
    /// neighbour active.
    pub(crate) fn remove_scenario(&mut self) -> bool {
        if self.scenarios.len() < 2 {
            return false;
        }
        let Some(index) = self.scenarios.iter().position(|scenario| scenario.id == self.active) else {
            return false;
        };
        self.scenarios.remove(index);
        self.active = self.scenarios[index.min(self.scenarios.len() - 1)].id;
        true
    }

    /// The design worked out, if it has been and there is one.
    pub(crate) fn design(&self) -> Option<&Design> {
        match &self.derived.as_ref()?.outcome {
            Outcome::Design(design) => Some(design),
            _ => None,
        }
    }

    /// Whether what was worked out is what `verts` and the active scenario
    /// come to now, against the ground at version `ground` (for a design that
    /// daylights).
    pub(crate) fn is_fresh(&self, verts: &[PolyVertex], ground: Option<GeometryVersion>) -> bool {
        let settings = self.settings();
        self.derived
            .as_ref()
            .is_some_and(|derived| derived.inputs == inputs_key(verts, &settings) && (settings.ground().is_none() || derived.ground == ground))
    }

    /// Move it by `delta` with its string: its ramp starts, and the design
    /// worked out, which moves with the string unless it daylights against
    /// ground that stays put. `verts` is the string after the move.
    pub(crate) fn translate(&mut self, delta: DVec3, verts: &[PolyVertex]) {
        for scenario in &mut self.scenarios {
            if let Some(start) = &mut scenario.settings.ramp_start {
                *start += delta.truncate();
            }
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

    /// How its surface stands, given the surface's geometry now (if it still
    /// exists) and the ground's version now.
    pub(crate) fn surface_standing(&self, verts: &[PolyVertex], surface: Option<GeometryVersion>, ground: Option<GeometryVersion>) -> SurfaceStanding {
        let (Some(link), Some(geometry)) = (&self.surface, surface) else {
            return SurfaceStanding::NotBuilt;
        };
        if geometry != link.geometry {
            return SurfaceStanding::Edited;
        }
        let settings = self.settings();
        let current = link.inputs == inputs_key(verts, &settings) && (settings.ground().is_none() || link.ground == ground) && link.trimmed == settings.trims();
        if current { SurfaceStanding::Current } else { SurfaceStanding::OutOfDate }
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
    mix(&serde_json::to_vec(settings).unwrap_or_default());
    hash
}
