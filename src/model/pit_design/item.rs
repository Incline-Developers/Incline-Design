//! Pit designs kept in the project.
//!
//! A pit design keeps what it is built from: its own copy of the string it
//! starts from, and its scenarios, each a full set of settings, one of them
//! active. Building it writes its strings to a design layer and, when asked,
//! its surface to a surface item; it remembers both, and rebuilding replaces
//! them in place. It also remembers what it was built from, so it can say
//! when its outputs no longer match it, and whether they were edited by hand
//! since.
//!
//! The whole list is swapped as one snapshot for undo, as the haul network
//! is: a design is a few kilobytes.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use super::settings::PitDesignSettings;
use crate::model::{
    LayerId, ObjectId,
    triangulation::{GeometryVersion, TriangulationId},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct PitDesignId(pub(crate) u64);

/// A scenario's id within its design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ScenarioId(pub(crate) u64);

/// The project's pit designs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PitDesigns {
    pub(crate) designs: Vec<PitDesign>,
    pub(crate) next_id: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PitDesign {
    pub(crate) id: PitDesignId,
    pub(crate) name: String,
    /// The closed string the design starts from: the floor's toe built up,
    /// the crest built down. A copy, so editing or deleting the string it was
    /// taken from never changes the design.
    pub(crate) base: Vec<DVec3>,
    /// The polyline it was copied from, to copy again on request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base_source: Option<ObjectId>,
    pub(crate) scenarios: Vec<Scenario>,
    pub(crate) active: ScenarioId,
    pub(crate) next_scenario_id: u64,
    #[serde(default)]
    pub(crate) outputs: Outputs,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Scenario {
    pub(crate) id: ScenarioId,
    pub(crate) name: String,
    pub(crate) settings: PitDesignSettings,
}

/// What a design has built, and what from.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Outputs {
    /// The design layer holding its strings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) layer: Option<LayerId>,
    /// Its surface.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) surface: Option<TriangulationId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) built: Option<Built>,
}

/// What the outputs were built from, and what they were when built.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Built {
    pub(crate) scenario: ScenarioId,
    pub(crate) settings: PitDesignSettings,
    pub(crate) base: Vec<DVec3>,
    /// The version of the topography it daylit against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) topography: Option<GeometryVersion>,
    /// The layer's strings as built ([`strings_fingerprint`]).
    pub(crate) layer_fingerprint: u64,
    /// The surface's geometry as built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) surface_geometry: Option<GeometryVersion>,
}

/// How a design's outputs stand against the design.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Standing {
    /// Never built, or its outputs have all been deleted.
    NotBuilt,
    /// Built from what the design is now.
    Current,
    /// The string, the active scenario, its settings or the ground have
    /// changed since it was built.
    OutOfDate,
}

impl PitDesigns {
    pub(crate) fn get(&self, id: PitDesignId) -> Option<&PitDesign> {
        self.designs.iter().find(|design| design.id == id)
    }

    pub(crate) fn get_mut(&mut self, id: PitDesignId) -> Option<&mut PitDesign> {
        self.designs.iter_mut().find(|design| design.id == id)
    }

    /// A new id, never one in use.
    pub(crate) fn allocate_id(&mut self) -> PitDesignId {
        let floor = self.designs.iter().map(|design| design.id.0 + 1).max().unwrap_or(0);
        let id = self.next_id.max(floor);
        self.next_id = id + 1;
        PitDesignId(id)
    }

    /// The list with `design` in place of the one with its id, or added.
    pub(crate) fn with(&self, design: PitDesign) -> Self {
        let mut list = self.clone();
        match list.get_mut(design.id) {
            Some(existing) => *existing = design,
            None => {
                list.next_id = list.next_id.max(design.id.0 + 1);
                list.designs.push(design);
            }
        }
        list
    }

    /// The list without the design with `id`.
    pub(crate) fn without(&self, id: PitDesignId) -> Self {
        let mut list = self.clone();
        list.designs.retain(|design| design.id != id);
        list
    }

    /// Never lower the allocator below one an earlier version reached, so an
    /// id freed on one branch of history is not reissued on another.
    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_id = self.next_id.max(other.next_id);
    }

    /// Put every layer and object id in `namespace`, as the document's own
    /// are when a project is opened.
    pub(crate) fn apply_namespace(&mut self, namespace: u32) {
        let id = |id: u64| (u64::from(namespace) << 32) | (id & u64::from(u32::MAX));
        for design in &mut self.designs {
            design.base_source = design.base_source.map(|object| ObjectId(id(object.0)));
            design.outputs.layer = design.outputs.layer.map(|layer| LayerId(id(layer.0)));
        }
    }

    /// The list with its ids local to its file, as it is saved.
    pub(crate) fn local_copy(&self) -> Self {
        let mut copy = self.clone();
        copy.apply_namespace(0);
        copy
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        let mut copy = self.local_copy();
        copy.next_id = 0;
        serde_json::to_vec(&copy).unwrap_or_default().hash(hasher);
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self
                .designs
                .iter()
                .map(|design| {
                    let built = design.outputs.built.as_ref().map_or(0, |built| built.base.len() * size_of::<DVec3>() + size_of::<Built>());
                    size_of::<PitDesign>() + design.name.len() + design.base.len() * size_of::<DVec3>() + design.scenarios.len() * size_of::<Scenario>() + built
                })
                .sum::<usize>()
    }
}

impl PitDesign {
    /// A design of `base` with one scenario.
    pub(crate) fn new(id: PitDesignId, name: String, base: Vec<DVec3>, base_source: Option<ObjectId>, scenario_name: String, settings: PitDesignSettings) -> Self {
        Self {
            id,
            name,
            base,
            base_source,
            scenarios: vec![Scenario {
                id: ScenarioId(0),
                name: scenario_name,
                settings,
            }],
            active: ScenarioId(0),
            next_scenario_id: 1,
            outputs: Outputs::default(),
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

    /// How the outputs stand, given the topography's version now. `has_layer`
    /// and `has_surface` say whether the outputs it names still exist.
    pub(crate) fn standing(&self, topography: Option<GeometryVersion>, has_layer: bool, has_surface: bool) -> Standing {
        let Some(built) = self.outputs.built.as_ref() else {
            return Standing::NotBuilt;
        };
        if !has_layer && !has_surface {
            return Standing::NotBuilt;
        }
        let settings = self.settings();
        let current = built.scenario == self.scenario().map_or(built.scenario, |scenario| scenario.id)
            && built.settings == settings
            && built.base == self.base
            && (settings.ground().is_none() || built.topography == topography)
            && (has_surface || !settings.surface);
        if current { Standing::Current } else { Standing::OutOfDate }
    }
}

/// A fingerprint of a layer's strings, to tell whether they were edited
/// after a design built them: their points to the millimetre and whether
/// each is closed, in order. Ids and colours are left out, and points are
/// rounded, so it holds across saving, opening and recolouring.
pub(crate) fn strings_fingerprint<'a>(strings: impl IntoIterator<Item = (&'a [DVec3], bool)>) -> u64 {
    // FNV-1a: the same on every build and platform, unlike the standard
    // library's hasher.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for (points, closed) in strings {
        mix(&[u8::from(closed)]);
        mix(&(points.len() as u64).to_le_bytes());
        for point in points {
            for value in [point.x, point.y, point.z] {
                mix(&((value * 1000.0).round() as i64).to_le_bytes());
            }
        }
    }
    hash
}
