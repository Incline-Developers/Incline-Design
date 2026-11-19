//! A pit design object: what it holds survives saving, it says truly whether
//! its benches still match it, and it moves and explodes whole.

use glam::{DVec2, DVec3};

use super::{
    Base, DesignString, StringKind, design,
    object::{Derived, Outcome, PitObject, inputs_key},
    settings::{PitDesignSettings, StopMode},
};
use crate::model::{
    LayerId, Object, ObjectColor, ObjectId, PolyVertex,
    triangulation::{GeometryVersion, TriangulationId},
};

fn square(level: f64) -> Vec<PolyVertex> {
    [(0.0, 0.0), (300.0, 0.0), (300.0, 200.0), (0.0, 200.0)]
        .into_iter()
        .map(|(x, y)| PolyVertex::straight(DVec3::new(x, y, level)))
        .collect()
}

fn settings() -> PitDesignSettings {
    PitDesignSettings {
        benches: 3,
        ramps: vec![super::settings::RampSettings {
            start: Some(DVec2::new(300.0, 100.0)),
            ..Default::default()
        }],
        ..PitDesignSettings::default()
    }
}

/// A pit design object on `verts`, its design worked out.
fn pit_object(verts: Vec<PolyVertex>, settings: PitDesignSettings) -> Object {
    let mut pit = PitObject::new("North pit".to_owned(), settings.clone());
    let points: Vec<DVec3> = verts.iter().map(|vertex| vertex.pos).collect();
    let base = Base::from_points(&points).expect("a closed string");
    let spec = settings.spec(points[0].truncate());
    let worked = design(&base, &spec, None, &|| false).expect("a design");
    pit.derived = Some(Derived {
        inputs: inputs_key(&verts, &settings),
        ground: None,
        outcome: Outcome::Design(Box::new(worked)),
    });
    Object::PitDesign {
        id: ObjectId(7),
        layer: LayerId(1),
        verts,
        color: ObjectColor::ByLayer,
        line_weight: 1.0,
        pit: Box::new(pit),
    }
}

fn parts(object: &Object) -> (&[PolyVertex], &PitObject) {
    match object {
        Object::PitDesign { verts, pit, .. } => (verts, pit),
        _ => panic!("not a pit design"),
    }
}

#[test]
fn saves_and_reads_back_whole() {
    let mut object = pit_object(square(100.0), settings());
    if let Object::PitDesign { pit, .. } = &mut object {
        pit.settings.face_angle = 75.0;
    }
    let json = serde_json::to_string(&object).expect("an object serialises");
    let read: Object = serde_json::from_str(&json).expect("and reads back");
    assert_eq!(read, object);
    // Settings missing from a file read as their defaults; unknown ones are refused.
    let partial: PitDesignSettings = serde_json::from_str(r#"{"bench_height": 10.0}"#).expect("partial settings read");
    assert_eq!(partial.bench_height, 10.0);
    assert_eq!(partial.face_angle, PitDesignSettings::default().face_angle);
    assert!(serde_json::from_str::<PitDesignSettings>(r#"{"bench_hieght": 10.0}"#).is_err());
}

#[test]
fn says_when_its_benches_are_stale() {
    let object = pit_object(square(100.0), settings());
    let (verts, pit) = parts(&object);
    assert!(pit.is_fresh(verts, None));
    let mut moved_vertex = verts.to_vec();
    moved_vertex[1].pos.x += 5.0;
    assert!(!pit.is_fresh(&moved_vertex, None), "an edited string makes the design stale");
    let mut changed = pit.clone();
    changed.settings.bench_height = 10.0;
    assert!(!changed.is_fresh(verts, None), "so does a changed setting");

    // Against ground, the ground's version counts too; without, it does not.
    assert!(pit.is_fresh(verts, Some(GeometryVersion::mint())));
    let mut daylit = pit.clone();
    {
        let settings = &mut daylit.settings;
        settings.stop = StopMode::Daylight;
        settings.topography = Some(TriangulationId(4));
    }
    let ground = GeometryVersion::mint();
    daylit.derived = Some(Derived {
        inputs: inputs_key(verts, &daylit.settings()),
        ground: Some(ground),
        outcome: Outcome::NotClosed,
    });
    assert!(daylit.is_fresh(verts, Some(ground)));
    assert!(!daylit.is_fresh(verts, Some(GeometryVersion::mint())), "the ground edited since");
}

#[test]
fn moves_whole() {
    let mut object = pit_object(square(100.0), settings());
    let before = object.clone();
    let delta = DVec3::new(50.0, -20.0, 5.0);
    object.translate(delta);
    let (verts, pit) = parts(&object);
    assert!(pit.is_fresh(verts, None), "a design that does not daylight moves with its string, still current");
    assert_eq!(pit.settings().ramps[0].start, Some(DVec2::new(350.0, 80.0)), "its ramp start moves with it");
    let moved = pit.design().expect("a design");
    let original = parts(&before).1.design().expect("a design");
    assert_eq!(moved.strings[1].points[0], original.strings[1].points[0] + delta);
    assert_eq!(moved.final_elevation(), original.final_elevation() + delta.z);
}

#[test]
fn draws_and_explodes_as_its_strings() {
    let object = pit_object(square(100.0), settings());
    let (verts, pit) = parts(&object);
    let design = pit.design().expect("a design");
    // Its own string stands for the design's first ring.
    assert_eq!(object.pit_strings(), &design.strings[1..]);
    let polylines = object.pit_polylines();
    assert_eq!(polylines.len(), design.strings.len());
    match &polylines[0] {
        Object::Polyline { verts: first, closed, .. } => {
            assert_eq!(first.as_slice(), verts, "the string comes first, as it was");
            assert!(closed);
        }
        _ => panic!("a polyline"),
    }
    let ramp = design.strings.iter().position(|string: &DesignString| string.kind == StringKind::RampEdge).expect("a ramp");
    match &polylines[ramp] {
        Object::Polyline { color, closed, .. } => {
            assert_eq!(*color, ObjectColor::Fixed(StringKind::RampEdge.colour()));
            assert!(!closed);
        }
        _ => panic!("a polyline"),
    }
    assert!(polylines.iter().all(|polyline| polyline.id() == object.id() && polyline.layer() == object.layer()));
}

#[test]
fn rounds_corners_into_arcs() {
    use super::object::{base_points, rounded_corners};
    let square: Vec<DVec3> = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]
        .into_iter()
        .map(|(x, y)| DVec3::new(x, y, 50.0))
        .collect();
    let area = |points: &[DVec3]| {
        (0..points.len())
            .map(|i| points[i].truncate().perp_dot(points[(i + 1) % points.len()].truncate()))
            .sum::<f64>()
            * 0.5
    };
    for (points, sign) in [(square.clone(), 1.0), (square.iter().rev().copied().collect::<Vec<_>>(), -1.0)] {
        let verts = rounded_corners(&points, 10.0);
        assert_eq!(verts.len(), 8, "each corner becomes the two ends of its arc");
        // Exact arcs, followed finely: the square less four corners' worth of
        // a radius-10 square that is not quarter circle.
        let fine = crate::model::geometry::tessellate_polyline_bulges(&verts, true);
        let expected = 100.0 * 100.0 - (4.0 - std::f64::consts::PI) * 100.0;
        assert!(
            (area(&fine) * sign - expected).abs() < 1.0,
            "the arcs bulge outward, as a rounded corner does: {} vs {expected}",
            area(&fine) * sign
        );
        // The engine's points follow the same arcs, coarser.
        let coarse = base_points(&verts);
        assert!(coarse.len() < fine.len() / 4);
        assert!((area(&coarse) * sign - expected).abs() < 15.0);
        assert!(coarse.iter().all(|point| point.z == 50.0));
    }
    // A corner tighter than its edges allow is rounded as far as they do.
    let thin: Vec<DVec3> = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)].into_iter().map(|(x, y)| DVec3::new(x, y, 0.0)).collect();
    let verts = rounded_corners(&thin, 100.0);
    assert!(verts.iter().all(|vertex| vertex.pos.is_finite() && vertex.bulge.is_finite()));
}
