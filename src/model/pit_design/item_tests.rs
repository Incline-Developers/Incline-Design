//! A pit design kept in the project: what it holds survives saving, its
//! scenarios add and remove as the window and MCP do them, and it says
//! truly whether its outputs still match it.

use glam::DVec3;

use super::{
    item::{Built, PitDesign, PitDesignId, PitDesigns, ScenarioId, Standing, strings_fingerprint},
    settings::{PitDesignSettings, StopMode},
};
use crate::model::{
    LayerId, ObjectId,
    triangulation::{GeometryVersion, TriangulationId},
};

fn square(level: f64) -> Vec<DVec3> {
    vec![
        DVec3::new(0.0, 0.0, level),
        DVec3::new(300.0, 0.0, level),
        DVec3::new(300.0, 200.0, level),
        DVec3::new(0.0, 200.0, level),
    ]
}

fn design(id: u64) -> PitDesign {
    PitDesign::new(PitDesignId(id), format!("Pit {id}"), square(100.0), Some(ObjectId(7)), "Base case".to_owned(), PitDesignSettings::default())
}

/// What a build of `design` as it stands would record.
fn built_from(design: &PitDesign, topography: Option<GeometryVersion>) -> Built {
    Built {
        scenario: design.active,
        settings: design.settings(),
        base: design.base.clone(),
        topography,
        layer_fingerprint: 0,
        surface_geometry: None,
    }
}

#[test]
fn saves_and_reads_back_whole() {
    let mut first = design(0);
    first.add_scenario("Steeper".to_owned());
    first.scenario_mut().expect("a scenario").settings.face_angle = 75.0;
    first.outputs.layer = Some(LayerId(3));
    first.outputs.surface = Some(TriangulationId(9));
    first.outputs.built = Some(built_from(&first, Some(GeometryVersion::mint())));
    let designs = PitDesigns::default().with(first).with(design(1));
    let json = serde_json::to_string(&designs).expect("designs serialise");
    let read: PitDesigns = serde_json::from_str(&json).expect("designs read back");
    assert_eq!(read, designs);
    // Settings missing from a file read as their defaults; unknown ones are refused.
    let partial: PitDesignSettings = serde_json::from_str(r#"{"bench_height": 10.0}"#).expect("partial settings read");
    assert_eq!(partial.bench_height, 10.0);
    assert_eq!(partial.face_angle, PitDesignSettings::default().face_angle);
    assert!(serde_json::from_str::<PitDesignSettings>(r#"{"bench_hieght": 10.0}"#).is_err());
}

#[test]
fn ids_are_never_reissued() {
    let mut designs = PitDesigns::default();
    let a = designs.allocate_id();
    designs = designs.with(design(a.0));
    let b = designs.allocate_id();
    designs = designs.with(design(b.0)).without(b);
    let c = designs.allocate_id();
    assert_ne!(c, b, "an id freed is not given out again");
    assert_ne!(c, a);
    let mut older = PitDesigns::default();
    older.raise_allocator_to(&designs);
    assert!(older.allocate_id().0 > c.0, "a list put back by undo keeps the allocator where it got to");
}

#[test]
fn scenarios_copy_and_remove() {
    let mut design = design(0);
    let base = design.active;
    design.scenario_mut().expect("a scenario").settings.berm_width = 12.0;
    let copy = design.add_scenario("Copy".to_owned());
    assert_eq!(design.active, copy, "a copy becomes the active scenario");
    assert_eq!(design.settings().berm_width, 12.0, "with the settings it was copied from");
    design.scenario_mut().expect("a scenario").settings.berm_width = 6.0;
    assert_eq!(design.scenarios.iter().find(|scenario| scenario.id == base).map(|scenario| scenario.settings.berm_width), Some(12.0));
    assert!(design.remove_scenario());
    assert_eq!(design.active, base);
    assert!(!design.remove_scenario(), "a design keeps one scenario");
    // An active id naming no scenario falls back to the first.
    design.active = ScenarioId(99);
    assert_eq!(design.scenario().map(|scenario| scenario.id), Some(base));
}

#[test]
fn says_when_its_outputs_no_longer_match() {
    let mut design = design(0);
    assert_eq!(design.standing(None, false, false), Standing::NotBuilt);
    design.scenario_mut().expect("a scenario").settings.surface = false;
    design.outputs.built = Some(built_from(&design, None));
    assert_eq!(design.standing(None, true, false), Standing::Current);
    assert_eq!(design.standing(None, false, false), Standing::NotBuilt, "outputs all deleted");

    // A setting, the string, or the active scenario changing.
    let mut changed = design.clone();
    changed.scenario_mut().expect("a scenario").settings.bench_height = 10.0;
    assert_eq!(changed.standing(None, true, false), Standing::OutOfDate);
    let mut moved = design.clone();
    moved.base[0].x += 1.0;
    assert_eq!(moved.standing(None, true, false), Standing::OutOfDate);
    let mut switched = design.clone();
    switched.add_scenario("Same settings".to_owned());
    assert_eq!(switched.standing(None, true, false), Standing::OutOfDate);

    // Asking for a surface it did not build.
    let mut wants_surface = design.clone();
    wants_surface.scenario_mut().expect("a scenario").settings.surface = true;
    wants_surface.outputs.built = Some(built_from(&wants_surface, None));
    assert_eq!(wants_surface.standing(None, true, false), Standing::OutOfDate);
    assert_eq!(wants_surface.standing(None, true, true), Standing::Current);

    // The ground it daylit against, edited since; only a design that
    // daylights depends on it.
    let ground = GeometryVersion::mint();
    let mut daylit = design.clone();
    {
        let settings = &mut daylit.scenario_mut().expect("a scenario").settings;
        settings.stop = StopMode::Daylight;
        settings.topography = Some(TriangulationId(4));
    }
    daylit.outputs.built = Some(built_from(&daylit, Some(ground)));
    assert_eq!(daylit.standing(Some(ground), true, false), Standing::Current);
    assert_eq!(daylit.standing(Some(GeometryVersion::mint()), true, false), Standing::OutOfDate);
    assert_eq!(design.standing(Some(GeometryVersion::mint()), true, false), Standing::Current);
}

#[test]
fn ids_are_namespaced_and_saved_local() {
    let mut first = design(0);
    first.outputs.layer = Some(LayerId(5));
    let mut designs = PitDesigns::default().with(first);
    designs.apply_namespace(3);
    let design = &designs.designs[0];
    assert_eq!(design.outputs.layer, Some(LayerId((3 << 32) | 5)));
    assert_eq!(design.base_source, Some(ObjectId((3 << 32) | 7)));
    let saved = designs.local_copy();
    assert_eq!(saved.designs[0].outputs.layer, Some(LayerId(5)));
    assert_eq!(saved.designs[0].base_source, Some(ObjectId(7)));
    // The content fingerprint does not see the namespace.
    use std::hash::{DefaultHasher, Hasher};
    let hash = |designs: &PitDesigns| {
        let mut hasher = DefaultHasher::new();
        designs.hash_content(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(&designs), hash(&saved));
}

#[test]
fn fingerprints_strings_to_the_millimetre() {
    let ring = square(100.0);
    let open = vec![DVec3::ZERO, DVec3::X];
    let fingerprint = strings_fingerprint([(ring.as_slice(), true), (open.as_slice(), false)]);
    let nudged: Vec<DVec3> = ring.iter().map(|point| *point + DVec3::splat(1.0e-5)).collect();
    assert_eq!(fingerprint, strings_fingerprint([(nudged.as_slice(), true), (open.as_slice(), false)]), "a round trip's rounding is not an edit");
    let moved: Vec<DVec3> = ring.iter().map(|point| *point + DVec3::new(0.01, 0.0, 0.0)).collect();
    assert_ne!(fingerprint, strings_fingerprint([(moved.as_slice(), true), (open.as_slice(), false)]));
    assert_ne!(fingerprint, strings_fingerprint([(ring.as_slice(), false), (open.as_slice(), false)]), "opening a ring is an edit");
    assert_ne!(fingerprint, strings_fingerprint([(ring.as_slice(), true)]), "deleting a string is an edit");
}
