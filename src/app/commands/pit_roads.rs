//! A pit design's ramps are always roads in the haul network: each road
//! made from one says whose it is, and the network is kept in step with the
//! designs, whatever changed them.

use crate::{
    app::App,
    i18n::tr,
    model::{
        Object,
        haulage::RoadId,
        pit_design::{Design, StringKind},
    },
};

/// A design's ramps as the roads they make: each its centreline, named
/// after the design.
fn pit_road_strings(name: &str, design: &Design) -> Vec<(String, Vec<glam::DVec3>)> {
    let centrelines: Vec<&[glam::DVec3]> = design
        .strings
        .iter()
        .filter(|string| string.kind == StringKind::RampCentreline)
        .map(|string| string.points.as_slice())
        .collect();
    let several = centrelines.len() > 1;
    centrelines
        .iter()
        .enumerate()
        .map(|(index, points)| {
            let name = if several {
                tr!("pit-design-haul-road-numbered", design = name.to_owned(), number = (index + 1).to_string())
            } else {
                tr!("pit-design-haul-road", design = name.to_owned())
            };
            (name, points.to_vec())
        })
        .collect()
}

/// What a design's roads are made from, to tell when they are out of step.
fn road_strings_key(strings: &[(String, Vec<glam::DVec3>)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for (name, points) in strings {
        name.hash(&mut hasher);
        for point in points {
            point.to_array().map(f64::to_bits).hash(&mut hasher);
        }
    }
    hasher.finish()
}

impl App<'_> {
    /// Bring the haul network into step with the project's pit designs:
    /// each current design's ramps roads along their centrelines, joined to
    /// the network where they come within its join tolerance, made again
    /// where the design has changed; and the roads of a design no longer
    /// there gone. Not an undo step: like a design worked out, it follows
    /// from the designs, whichever step put them as they are.
    pub(crate) fn keep_pit_roads(&mut self) {
        let Some(document) = self.workspace.active_document() else {
            return;
        };
        let mut network = document.haulage().clone();
        let before = network.clone();
        let designs: std::collections::HashMap<u64, &Object> = document
            .objects()
            .iter()
            .filter(|object| matches!(object, Object::PitDesign { .. }))
            .map(|object| (object.id().0, object))
            .collect();
        // Roads of designs gone, or no longer designs.
        let orphans: Vec<RoadId> = network
            .roads
            .iter()
            .filter(|road| road.pit.is_some_and(|pit| !designs.contains_key(&pit.design)))
            .map(|road| road.id)
            .collect();
        for road in orphans {
            network.delete_road(road);
        }
        for (&design, object) in &designs {
            let Object::PitDesign { verts, pit, .. } = object else {
                continue;
            };
            // A design out of date keeps its roads until it is worked out.
            let ground = self.pit_ground_version(&pit.settings());
            let Some(worked) = pit.design().filter(|_| pit.is_fresh(verts.as_slice(), ground)) else {
                continue;
            };
            let strings = pit_road_strings(&pit.name, worked);
            let from = road_strings_key(&strings);
            let owned: Vec<RoadId> = network
                .roads
                .iter()
                .filter(|road| road.pit.is_some_and(|pit| pit.design == design))
                .map(|road| road.id)
                .collect();
            let current = network
                .roads
                .iter()
                .filter(|road| road.pit.is_some_and(|pit| pit.design == design))
                .all(|road| road.pit.is_some_and(|pit| pit.from == from));
            if current && (owned.is_empty() == strings.is_empty()) {
                continue;
            }
            for road in owned {
                network.delete_road(road);
            }
            let existing: std::collections::HashSet<RoadId> = network.roads.iter().map(|road| road.id).collect();
            if network.convert(&strings).is_err() {
                continue;
            }
            for road in network.roads.iter_mut().filter(|road| !existing.contains(&road.id)) {
                road.pit = Some(crate::model::haulage::PitRoad { design, from });
            }
        }
        if network == before || network.validate().is_err() {
            return;
        }
        if let Some(document) = self.workspace.active_document_mut() {
            document.set_haulage(network);
        }
        self.editor.haul_route = None;
        self.refresh_haulage_view();
    }
}
