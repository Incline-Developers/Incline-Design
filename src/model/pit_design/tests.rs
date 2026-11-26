//! Designs on outlines of every sort, held to what a sound design is: no two
//! strings crossing in plan at different heights, the ramp nowhere steeper
//! than its gradient nor narrower than its width, and a single-valued
//! surface from its strings.

use glam::{DVec2, DVec3};

use super::{
    Additions, Base, BenchRules, Design, DesignString, Direction, End, Kind, Layout, Problem, RampSpec, Spec, Stop, StringKind, Zone, design,
    walls::{Region, Sector},
};

/// Outlines in plan: a rectangle, a circle, an L, a star, a hand-drawn floor
/// with a re-entrant corner, and a big crest.
fn outline(kind: &str) -> Vec<DVec2> {
    let points: Vec<(f64, f64)> = match kind {
        "rect" => vec![(0.0, 0.0), (400.0, 0.0), (400.0, 250.0), (0.0, 250.0)],
        "circle" => {
            return (0..72).map(|i| DVec2::from_angle(f64::from(i) * std::f64::consts::TAU / 72.0) * 200.0).collect();
        }
        "ell" => vec![(0.0, 0.0), (500.0, 0.0), (500.0, 200.0), (200.0, 200.0), (200.0, 500.0), (0.0, 500.0)],
        "star" => {
            return (0..14)
                .map(|i| DVec2::from_angle(f64::from(i) * std::f64::consts::TAU / 14.0) * if i % 2 == 0 { 300.0 } else { 220.0 })
                .collect();
        }
        "floor" => vec![
            (0.0, 0.0),
            (120.0, -20.0),
            (220.0, 10.0),
            (260.0, 90.0),
            (200.0, 130.0),
            (210.0, 200.0),
            (120.0, 240.0),
            (20.0, 210.0),
            (-30.0, 110.0),
        ],
        "crest" => vec![
            (0.0, 0.0),
            (500.0, -60.0),
            (900.0, 80.0),
            (1000.0, 420.0),
            (760.0, 520.0),
            (820.0, 800.0),
            (400.0, 900.0),
            (60.0, 760.0),
            (-80.0, 380.0),
        ],
        _ => unreachable!(),
    };
    points.into_iter().map(|(x, y)| DVec2::new(x, y)).collect()
}

/// Gradients and berm widths designs are tried with: no berm, where the
/// road takes the whole bench, among them.
const RAMPS: [(f64, f64); 3] = [(0.1, 8.0), (0.08, 0.0), (0.12, 10.0)];

fn spec(direction: Direction, layout: Layout, start: DVec2, clockwise: bool, stop: Stop) -> Spec {
    with_ramp(direction, layout, start, clockwise, stop, RAMPS[0])
}

fn with_ramp(direction: Direction, layout: Layout, start: DVec2, clockwise: bool, stop: Stop, (gradient, berm_width): (f64, f64)) -> Spec {
    Spec {
        kind: Kind::Pit,
        direction,
        bench_height: 15.0,
        face_angle: 65.0,
        berm_width,
        rules: BenchRules::default(),
        wall_radius: 30.0,
        ramps: vec![RampSpec {
            width: 28.0,
            gradient,
            start,
            climbs_clockwise: clockwise,
            layout,
            from: None,
            to: None,
            bands: Vec::new(),
            exit_flat: 0.0,
            junction_flat: 0.0,
            turns_at: Vec::new(),
            turning_radius: 15.0,
        }],
        stop,
    }
}

fn built(kind: &str, elevation: f64, spec: &Spec, ground: Option<super::Ground<'_>>) -> Design {
    let points: Vec<DVec3> = outline(kind).iter().map(|point| point.extend(elevation)).collect();
    design(&Base::from_points(&points).expect("a closed outline"), spec, ground, &|| false).expect("settings that make a design")
}

/// Segments of every string but the centreline, which only repeats the
/// edges, with the string each is from.
fn segments(design: &Design) -> Vec<(DVec3, DVec3, usize)> {
    design
        .strings
        .iter()
        .enumerate()
        .filter(|(_, string)| string.kind != StringKind::RampCentreline)
        .flat_map(|(index, string)| {
            let count = string.points.len();
            let last = if string.closed { count } else { count - 1 };
            (0..last).map(move |at| (string.points[at], string.points[(at + 1) % count], index))
        })
        .collect()
}

/// How many times strings cross in plan at different heights.
fn crossings(design: &Design) -> usize {
    let segments = segments(design);
    let mut found = 0;
    for (i, (a, b, first)) in segments.iter().enumerate() {
        for (c, d, second) in &segments[i + 1..] {
            if first == second {
                continue;
            }
            if let crate::model::kernel::SegSeg::Crossing { t, u, .. } = crate::model::kernel::segment_segment(a.truncate(), b.truncate(), c.truncate(), d.truncate())
                && (a.z + (b.z - a.z) * t - (c.z + (d.z - c.z) * u)).abs() > 0.05
            {
                found += 1;
            }
        }
    }
    found
}

/// The steepest the ramp's edges get over any metre of their run.
fn steepest(design: &Design) -> f64 {
    let mut worst: f64 = 0.0;
    for string in design.strings.iter().filter(|string| string.kind == StringKind::RampEdge) {
        let mut anchor = string.points[0];
        let mut run = 0.0;
        for pair in string.points.windows(2) {
            run += pair[0].truncate().distance(pair[1].truncate());
            if run >= 1.0 {
                worst = worst.max((pair[1].z - anchor.z).abs() / run);
                anchor = pair[1];
                run = 0.0;
            }
        }
    }
    worst
}

/// The narrowest the road gets: from each point of one edge to the other
/// edge where it is at the same height.
fn narrowest(design: &Design) -> f64 {
    let edges: Vec<&DesignString> = design.strings.iter().filter(|string| string.kind == StringKind::RampEdge).collect();
    let mut low = f64::INFINITY;
    for point in &edges[0].points {
        let mut nearest = f64::INFINITY;
        for pair in edges[1].points.windows(2) {
            if point.z < pair[0].z.min(pair[1].z) - 0.05 || point.z > pair[0].z.max(pair[1].z) + 0.05 {
                continue;
            }
            let (foot, _) = crate::model::kernel::project_onto_segment(point.truncate(), pair[0].truncate(), pair[1].truncate());
            nearest = nearest.min(foot.distance(point.truncate()));
        }
        if nearest.is_finite() {
            low = low.min(nearest);
        }
    }
    low
}

/// The design's strings as a surface's breaklines, as Create Triangulation
/// takes them: its rings, closed, and its ramp's edges, open. The centreline
/// only repeats what the edges say.
fn breaklines(design: &Design) -> (Vec<Vec<DVec3>>, Vec<Vec<DVec3>>) {
    let (mut rings, mut edges) = (Vec::new(), Vec::new());
    for string in &design.strings {
        match (string.kind, string.closed) {
            (StringKind::RampCentreline, _) => {}
            (_, true) => rings.push(string.points.clone()),
            (_, false) => edges.push(string.points.clone()),
        }
    }
    (rings, edges)
}

/// Hold a design to what a sound one is.
fn assert_sound(name: &str, design: &Design, ramp: &RampSpec) {
    assert_sound_climbing(name, design, ramp, true);
}

/// Hold a design to what a sound one is, its stretches climbing at their
/// gradient where `at_gradient` (flatter only where they say so), or
/// otherwise no steeper.
fn assert_sound_climbing(name: &str, design: &Design, ramp: &RampSpec, at_gradient: bool) {
    assert!(design.benches > 0, "{name}: no bench built");
    assert_eq!(crossings(design), 0, "{name}: strings cross at different heights");
    assert!(steepest(design) <= ramp.gradient + 1.0e-6, "{name}: the ramp is steeper than its gradient");
    assert!(narrowest(design) >= ramp.width * 0.96, "{name}: the road narrows to {:.1} m", narrowest(design));
    // Each bench's stretch climbs at the gradient asked: a flat before it
    // where the bench needs more room, flatter only where it says so.
    assert_eq!(design.report.len(), design.benches as usize, "{name}: a report for every bench");
    for (index, bench) in design.report.iter().enumerate() {
        let road = bench.ramp().unwrap_or_else(|| panic!("{name}: bench {index} has no ramp"));
        // Over a ring that weaves, the edges take turns to be the shorter,
        // and each stretch between them climbs only what the shorter
        // allows: flatter, as the design's checks say where.
        let flagged_flatter = design.flags.iter().any(|flag| flag.check == super::checks::Check::Flatter && flag.bench == index);
        if road.lengthened == 0.0 && at_gradient && !flagged_flatter {
            assert!(
                road.steepest >= ramp.gradient - 0.005,
                "{name}: bench {index} climbs at {:.4}, not {}",
                road.steepest,
                ramp.gradient
            );
        }
        assert!(road.steepest <= ramp.gradient + 1.0e-6, "{name}: bench {index} is steeper than its gradient");
        assert!(road.narrowest >= ramp.width * 0.96, "{name}: bench {index} narrows to {:.1} m", road.narrowest);
    }
    let lengthened = design.report.iter().filter(|bench| bench.ramp().is_some_and(|road| road.lengthened > 0.0)).count();
    assert!(
        !at_gradient || lengthened * 10 <= design.report.len().max(10),
        "{name}: {lengthened} stretches climb flatter than asked"
    );
    let (rings, edges) = breaklines(design);
    if let Err(error) = crate::app::commands::triangulation::creation::cdt_surface_from_breaklines(&rings, &edges) {
        panic!("{name}: no single-valued surface: {error:#}");
    }
}

#[test]
fn spirals_up_are_sound_and_complete() {
    for kind in ["rect", "circle", "ell", "star", "floor"] {
        for (clockwise, ramp) in [false, true].into_iter().flat_map(|clockwise| RAMPS.map(|ramp| (clockwise, ramp))) {
            let start = outline(kind)[0];
            let spec = with_ramp(Direction::Up, Layout::Spiral, start, clockwise, Stop::Benches(10), ramp);
            let design = built(kind, 100.0, &spec, None);
            let name = format!("{kind} up, clockwise {clockwise}, gradient and berm {ramp:?}");
            assert_sound(&name, &design, &spec.ramps[0]);
            assert_eq!(design.end, End::Complete, "{name}: stopped at bench {}", design.benches);
        }
    }
}

#[test]
fn switchbacks_up_turn_back_and_are_sound() {
    for (kind, ramp) in ["rect", "circle", "floor"].into_iter().flat_map(|kind| RAMPS.map(|ramp| (kind, ramp))) {
        let start = outline(kind)[0];
        let layout = Layout::Switchback {
            every: 3,
            inside_radius: 15.0,
            widening: 0.0,
            grade: 0.0,
        };
        let spec = with_ramp(Direction::Up, layout, start, false, Stop::Benches(10), ramp);
        let design = built(kind, 100.0, &spec, None);
        assert_sound(&format!("{kind} switchback, gradient and berm {ramp:?}"), &design, &spec.ramps[0]);
        // Turned back at least once: the centreline heads both ways round.
        let centreline = design.strings.iter().find(|string| string.kind == StringKind::RampCentreline).unwrap();
        let centre = centreline.points.iter().fold(DVec2::ZERO, |sum, point| sum + point.truncate()) / centreline.points.len() as f64;
        let turning: Vec<f64> = centreline
            .points
            .windows(2)
            .map(|pair| (pair[0].truncate() - centre).perp_dot(pair[1].truncate() - pair[0].truncate()))
            .collect();
        assert!(
            turning.iter().any(|turn| *turn > 0.0) && turning.iter().any(|turn| *turn < 0.0),
            "{kind}: the ramp never turned back"
        );
    }
}

#[test]
fn designs_down_are_sound() {
    for (kind, benches) in [("crest", 12), ("rect", 4), ("circle", 5), ("star", 4), ("ell", 3)] {
        for (clockwise, ramp) in [false, true].into_iter().flat_map(|clockwise| RAMPS.map(|ramp| (clockwise, ramp))) {
            let start = outline(kind)[0];
            let spec = with_ramp(Direction::Down, Layout::Spiral, start, clockwise, Stop::Benches(40), ramp);
            let design = built(kind, 500.0, &spec, None);
            let name = format!("{kind} down, clockwise {clockwise}, gradient and berm {ramp:?}");
            assert_sound(&name, &design, &spec.ramps[0]);
            assert!(design.benches >= benches, "{name}: only {} benches", design.benches);
        }
    }
}

#[test]
fn stops_at_an_rl_and_at_daylight() {
    let start = outline("floor")[3];
    let at_rl = spec(Direction::Up, Layout::Spiral, start, false, Stop::Elevation(137.0));
    let design = built("floor", 100.0, &at_rl, None);
    assert_eq!(design.levels, vec![100.0, 115.0, 130.0, 137.0], "the last bench is cut short to meet the RL");
    assert_sound("floor to RL 137", &design, &at_rl.ramps[0]);

    // Ground rising to the east, from 140 at x = -200 to 220 at x = 600.
    let ground = |point: DVec2| Some(160.0 + point.x * 0.1);
    let daylight = spec(Direction::Up, Layout::Spiral, start, false, Stop::Daylight);
    let design = built("floor", 100.0, &daylight, Some(&ground));
    assert_eq!(design.end, End::Daylight);
    let last = design.strings.iter().rev().find(|string| string.closed).unwrap();
    assert!(
        last.points.iter().all(|point| point.z >= ground(point.truncate()).unwrap()),
        "the last ring stands clear of the ground"
    );

    let down = spec(Direction::Down, Layout::Spiral, outline("crest")[2], false, Stop::Elevation(448.0));
    let design = built("crest", 500.0, &down, None);
    assert_eq!(design.final_elevation(), 448.0);
}

#[test]
fn settings_that_make_no_design_say_why() {
    let points: Vec<DVec3> = outline("rect").iter().map(|point| point.extend(100.0)).collect();
    let base = Base::from_points(&points).unwrap();
    let start = DVec2::ZERO;
    let problem = |spec: Spec| design(&base, &spec, None, &|| false).err();
    assert_eq!(problem(spec(Direction::Down, Layout::Spiral, start, false, Stop::Daylight)), Some(Problem::DaylightDown));
    assert_eq!(
        problem(spec(Direction::Up, Layout::Spiral, start, false, Stop::Elevation(90.0))),
        Some(Problem::StopElevation)
    );
    assert_eq!(problem(spec(Direction::Up, Layout::Spiral, start, false, Stop::Benches(0))), Some(Problem::Benches));
    let mut steep = spec(Direction::Up, Layout::Spiral, start, false, Stop::Benches(3));
    steep.face_angle = 90.0;
    assert_eq!(problem(steep), Some(Problem::FaceAngle));
    let mut zoned = spec(Direction::Up, Layout::Spiral, start, false, Stop::Benches(3));
    zoned.rules.zones.push(Zone {
        from: 120.0,
        to: 120.0,
        ..Zone::default()
    });
    assert_eq!(problem(zoned.clone()), Some(Problem::Zone));
    zoned.rules.zones[0].to = 140.0;
    zoned.rules.zones[0].face_angle = Some(95.0);
    assert_eq!(problem(zoned.clone()), Some(Problem::FaceAngle));
    zoned.rules.zones[0].face_angle = None;
    zoned.rules.zones[0].wall_radius = Some(-1.0);
    assert_eq!(problem(zoned.clone()), Some(Problem::WallRadius));
    zoned.rules.zones[0].wall_radius = None;
    zoned.rules.height_cycle = vec![10.0, 0.0];
    assert_eq!(problem(zoned), Some(Problem::BenchHeight));
    let mut flats = spec(Direction::Up, Layout::Spiral, start, false, Stop::Benches(3));
    flats.ramps[0].exit_flat = -1.0;
    assert_eq!(problem(flats.clone()), Some(Problem::RampBand));
    flats.ramps[0].exit_flat = 0.0;
    flats.ramps[0].bands.push(super::RampBand {
        from: 120.0,
        to: 140.0,
        width: Some(0.0),
        flat: None,
    });
    assert_eq!(problem(flats), Some(Problem::RampWidth));
}

#[test]
fn dumps_build_up_stepping_in_and_down_to_the_ground() {
    let start = outline("crest")[2];
    for clockwise in [false, true] {
        let mut spec = with_ramp(Direction::Up, Layout::Spiral, start, clockwise, Stop::Benches(6), RAMPS[0]);
        spec.kind = Kind::Dump;
        let dump = built("crest", 300.0, &spec, None);
        let name = format!("dump up, clockwise {clockwise}");
        assert_sound(&name, &dump, &spec.ramps[0]);
        assert!(dump.dump);
        assert!(dump.benches >= 5, "{name}: only {} lifts", dump.benches);
        assert!(dump.levels.windows(2).all(|pair| pair[1] > pair[0]), "{name}: built up");
        assert!(dump.report.windows(2).all(|pair| pair[1].area < pair[0].area), "{name}: each lift steps in");
        // Climbing, the ramp goes the way round it was asked: seen from
        // above, the centreline sweeps clockwise or anticlockwise as it rises.
        let centreline = dump.strings.iter().find(|string| string.kind == StringKind::RampCentreline).unwrap();
        let mut points = centreline.points.clone();
        points.sort_by(|a, b| a.z.total_cmp(&b.z));
        let centre = points.iter().fold(DVec2::ZERO, |sum, point| sum + point.truncate()) / points.len() as f64;
        let turning: f64 = points
            .windows(2)
            .map(|pair| (pair[0].truncate() - centre).perp_dot(pair[1].truncate() - pair[0].truncate()))
            .sum();
        assert_eq!(turning < 0.0, clockwise, "{name}: the ramp climbs the wrong way round");
    }

    // Down from its top, stepping out until a whole lift meets ground that
    // falls away to the east.
    let ground = |point: DVec2| Some(260.0 - point.x * 0.02);
    let mut spec = with_ramp(Direction::Down, Layout::Spiral, start, false, Stop::Daylight, RAMPS[0]);
    spec.kind = Kind::Dump;
    let dump = built("rect", 300.0, &spec, Some(&ground));
    assert_sound("dump down", &dump, &spec.ramps[0]);
    assert_eq!(dump.end, End::Daylight);
    assert!(dump.levels.windows(2).all(|pair| pair[1] < pair[0]), "built down");
    let last = dump.strings.iter().rev().find(|string| string.closed).unwrap();
    assert!(
        last.points.iter().all(|point| point.z <= ground(point.truncate()).unwrap()),
        "the last lift's toe is on or below the ground"
    );

    // Built up, there is no ground to meet.
    let mut spec = with_ramp(Direction::Up, Layout::Spiral, start, false, Stop::Daylight, RAMPS[0]);
    spec.kind = Kind::Dump;
    let points: Vec<DVec3> = outline("rect").iter().map(|point| point.extend(300.0)).collect();
    assert_eq!(design(&Base::from_points(&points).unwrap(), &spec, None, &|| false).err(), Some(Problem::DaylightDumpUp));
}

/// Rules that vary every sort of thing: heights 10, 10, 15 in turn, a catch
/// berm 20 wide every third bench, and between RL 130 and 160 faces of 50°
/// and berms of 12.
fn varied() -> BenchRules {
    BenchRules {
        height_cycle: vec![10.0, 10.0, 15.0],
        catch_every: 3,
        catch_width: 20.0,
        zones: vec![Zone {
            from: 130.0,
            to: 160.0,
            face_angle: Some(50.0),
            berm_width: Some(12.0),
            ..Zone::default()
        }],
        ..BenchRules::default()
    }
}

#[test]
fn bench_rules_land_on_the_right_benches() {
    // Up from RL 100: the benches start at 100, 110, 120, 135, 145, 155,
    // 170 and 180. The catch berms come before the fourth and seventh; the
    // zone holds the three starting between 130 and 160.
    let mut flat = spec(Direction::Up, Layout::Spiral, outline("rect")[0], false, Stop::Benches(8));
    flat.ramps.clear();
    flat.wall_radius = 0.0;
    flat.rules = varied();
    let plain = built("rect", 100.0, &flat, None);
    let rows: Vec<(f64, f64, f64, f64)> = plain
        .report
        .iter()
        .map(|bench| {
            assert_eq!(bench.face_angle[0], bench.face_angle[1], "every wall of a bench alike");
            assert_eq!(bench.berm[0], bench.berm[1], "every berm of a bench alike");
            (bench.elevation, bench.height, bench.face_angle[0], bench.berm[0])
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (100.0, 10.0, 65.0, 0.0),
            (110.0, 10.0, 65.0, 8.0),
            (120.0, 15.0, 65.0, 8.0),
            (135.0, 10.0, 50.0, 20.0),
            (145.0, 10.0, 50.0, 12.0),
            (155.0, 15.0, 50.0, 12.0),
            (170.0, 10.0, 65.0, 20.0),
            (180.0, 10.0, 65.0, 8.0),
        ]
    );
    // And the rings stand that far apart: each berm, then each face's run.
    let rings: Vec<&DesignString> = plain.strings.iter().filter(|string| string.closed).collect();
    let mut gaps = Vec::new();
    for bench in &plain.report {
        if bench.berm[0] > 0.0 {
            gaps.push(bench.berm[0]);
        }
        gaps.push(bench.height / bench.face_angle[0].to_radians().tan());
    }
    assert_eq!(rings.len(), gaps.len() + 1);
    for (pair, gap) in rings.windows(2).zip(&gaps) {
        // Measured both ways: a ring's corners stand further off than its
        // sides, which may have no points of their own along them.
        let plan = |string: &DesignString| string.points.iter().map(|point| point.truncate()).collect::<Vec<DVec2>>();
        let (inner, outer) = (plan(pair[0]), plan(pair[1]));
        let from = |points: &[DVec2], ring: &[DVec2]| {
            points
                .iter()
                .map(|point| crate::model::pit_design::ring::nearest(ring, *point).1)
                .fold(f64::INFINITY, f64::min)
        };
        let apart = from(&outer, &inner).min(from(&inner, &outer));
        assert!((apart - gap).abs() < 0.01, "rings {apart:.3} m apart, not {gap:.3}");
    }

    // With a ramp, as sound as any design.
    for kind in ["rect", "ell", "floor"] {
        let mut ramped = spec(Direction::Up, Layout::Spiral, outline(kind)[0], false, Stop::Benches(8));
        ramped.rules = varied();
        let design = built(kind, 100.0, &ramped, None);
        assert_sound(&format!("{kind} with bench rules"), &design, &ramped.ramps[0]);
    }

    // A dump built up takes its zones by their RLs too.
    let mut dumped = spec(Direction::Up, Layout::Spiral, outline("crest")[2], false, Stop::Benches(6));
    dumped.kind = Kind::Dump;
    dumped.rules = BenchRules {
        zones: vec![Zone {
            from: 330.0,
            to: 360.0,
            face_angle: Some(40.0),
            ..Zone::default()
        }],
        ..BenchRules::default()
    };
    let dump = built("crest", 300.0, &dumped, None);
    for bench in &dump.report {
        let expected = if (330.0..360.0).contains(&bench.elevation) { 40.0 } else { 65.0 };
        assert_eq!(bench.face_angle, [expected; 2], "the lift from RL {}", bench.elevation);
    }
    assert!(dump.report.iter().any(|bench| bench.face_angle[0] == 40.0));
}

/// Walls facing north cut at 70° with 6 m berms, walls facing south at 45°
/// with 15 m berms, those between blended round the compass.
fn sectors() -> Vec<Sector> {
    vec![
        Sector {
            bearing: 0.0,
            face_angle: 70.0,
            berm_width: 6.0,
            inter_ramp: None,
        },
        Sector {
            bearing: 180.0,
            face_angle: 45.0,
            berm_width: 15.0,
            inter_ramp: None,
        },
    ]
}

/// Everything east of x = 250 cut at 40° with 20 m berms, blended over 40 m.
fn east_region() -> Region {
    Region {
        outline: vec![DVec2::new(250.0, -400.0), DVec2::new(900.0, -400.0), DVec2::new(900.0, 900.0), DVec2::new(250.0, 900.0)],
        face_angle: Some(40.0),
        berm_width: Some(20.0),
    }
}

/// How far apart two rings stand at `point` on the first.
fn apart_at(first: &DesignString, second: &DesignString, point: DVec2) -> f64 {
    let plan = |string: &DesignString| string.points.iter().map(|point| point.truncate()).collect::<Vec<DVec2>>();
    debug_assert!(crate::model::pit_design::ring::nearest(&plan(first), point).1 < 0.01, "{point} is on the first ring");
    crate::model::pit_design::ring::nearest(&plan(second), point).1
}

#[test]
fn walls_take_their_sector_and_region() {
    let run = |angle: f64| 15.0 / f64::to_radians(angle).tan();
    let mut flat = spec(Direction::Up, Layout::Spiral, outline("rect")[0], false, Stop::Benches(3));
    flat.ramps.clear();
    flat.wall_radius = 0.0;
    flat.rules.sectors = sectors();
    let design = built("rect", 100.0, &flat, None);
    let rings: Vec<&DesignString> = design.strings.iter().filter(|string| string.closed).collect();
    // The first bench's face: the south wall faces north, the north wall
    // south, the east wall east, half way between round the compass.
    for (point, angle) in [(DVec2::new(200.0, 0.0), 70.0), (DVec2::new(200.0, 250.0), 45.0), (DVec2::new(400.0, 125.0), 57.5)] {
        let apart = apart_at(rings[0], rings[1], point);
        assert!((apart - run(angle)).abs() < 0.01, "at {point}: {apart:.3} m, not {:.3}", run(angle));
    }
    // The second bench's berm, from the first's crest.
    let crest = |point: DVec2, angle: f64| point + DVec2::new(0.0, if point.y < 100.0 { -1.0 } else { 1.0 }) * run(angle);
    for (point, angle, berm) in [(DVec2::new(200.0, 0.0), 70.0, 6.0), (DVec2::new(200.0, 250.0), 45.0, 15.0)] {
        let apart = apart_at(rings[1], rings[2], crest(point, angle));
        assert!((apart - berm).abs() < 0.01, "berm at {point}: {apart:.3} m, not {berm}");
    }
    assert_eq!(design.report[0].face_angle, [45.0, 70.0]);
    assert_eq!(design.report[1].berm, [6.0, 15.0]);

    // A region takes over east of x = 250, blending in across 40 m: along
    // the south wall the face's run goes smoothly from the sector's to the
    // region's.
    flat.rules.regions = vec![east_region()];
    flat.rules.blend = 40.0;
    let design = built("rect", 100.0, &flat, None);
    let rings: Vec<&DesignString> = design.strings.iter().filter(|string| string.closed).collect();
    let runs: Vec<f64> = (0..=40).map(|step| apart_at(rings[0], rings[1], DVec2::new(150.0 + 5.0 * f64::from(step), 0.0))).collect();
    assert!((runs[0] - run(70.0)).abs() < 0.01, "west of the region: {:.3}", runs[0]);
    assert!((runs[40] - run(40.0)).abs() < 0.01, "inside it: {:.3}", runs[40]);
    // The angle blends evenly, 30° over 40 m; the run it makes grows
    // fastest at the flattest angle, 40°, where 5 m along adds
    // 5 × 0.75° × 15 / sin² 40° in radians.
    let most = 5.0 * 0.75_f64.to_radians() * 15.0 / 40.0_f64.to_radians().sin().powi(2);
    let steepest = runs.windows(2).map(|pair| (pair[1] - pair[0]).abs()).fold(0.0, f64::max);
    assert!(steepest <= most * 1.05, "the run jumps {steepest:.3} m in 5 m, more than {most:.3}");
    assert!(runs.windows(2).all(|pair| pair[1] >= pair[0] - 1.0e-6), "and only widens into the region");

    // A dump's walls face out: its north wall faces north.
    let mut dumped = flat.clone();
    dumped.kind = Kind::Dump;
    dumped.rules.regions.clear();
    let dump = built("rect", 300.0, &dumped, None);
    let rings: Vec<&DesignString> = dump.strings.iter().filter(|string| string.closed).collect();
    for (point, angle) in [(DVec2::new(200.0, 0.0), 45.0), (DVec2::new(200.0, 250.0), 70.0)] {
        let apart = apart_at(rings[0], rings[1], point);
        assert!((apart - run(angle)).abs() < 0.01, "dump at {point}: {apart:.3} m, not {:.3}", run(angle));
    }
}

#[test]
fn walls_cut_otherwise_still_make_sound_designs() {
    // Walls varying by the way they face, by where they stand, or both:
    // built up, every design is sound, and complete but for the two that
    // take both at once round a tight curve, where the walls widen fastest.
    let rules = [("sectors", true, false), ("a region", false, true), ("both", true, true)];
    for kind in ["rect", "circle", "ell", "floor", "star"] {
        for (layout, every) in [
            (Layout::Spiral, true),
            (
                Layout::Switchback {
                    every: 2,
                    inside_radius: 15.0,
                    widening: 0.0,
                    grade: 0.0,
                },
                false,
            ),
        ] {
            for (what, with_sectors, with_region) in rules {
                if !every && !matches!(kind, "rect" | "ell") {
                    continue;
                }
                let mut spec = with_ramp(Direction::Up, layout, outline(kind)[0], false, Stop::Benches(8), RAMPS[0]);
                if with_sectors {
                    spec.rules.sectors = sectors();
                }
                if with_region {
                    spec.rules.regions = vec![east_region()];
                }
                let design = built(kind, 100.0, &spec, None);
                let name = format!("{kind} up {layout:?} with {what}");
                assert_sound(&name, &design, &spec.ramps[0]);
                if !(what == "both" && matches!(kind, "circle" | "floor")) {
                    assert_eq!(design.end, End::Complete, "{name}: {} benches", design.benches);
                }
            }
        }
    }
    // Built down, a pit narrows bench by bench, and the faster its walls
    // widen the sooner the ramp runs out of ring: sound as far as it goes.
    for kind in ["rect", "circle", "star"] {
        for (what, with_sectors, with_region) in rules {
            let mut spec = with_ramp(Direction::Down, Layout::Spiral, outline(kind)[0], false, Stop::Benches(8), RAMPS[0]);
            if with_sectors {
                spec.rules.sectors = sectors();
            }
            if with_region {
                spec.rules.regions = vec![east_region()];
            }
            let design = built(kind, 300.0, &spec, None);
            let name = format!("{kind} down with {what}");
            // Built down where the walls vary, a stretch can come out a
            // little flatter than asked, never steeper.
            assert_sound_climbing(&name, &design, &spec.ramps[0], false);
            assert!(design.benches >= 2, "{name}: {} benches", design.benches);
        }
    }
}

#[test]
fn walls_come_out_no_steeper_than_cut() {
    let mut measured_in_all = 0;
    for kind in ["rect", "circle", "ell", "star", "floor"] {
        for with_sectors in [false, true] {
            for direction in [Direction::Up, Direction::Down] {
                let mut spec = with_ramp(direction, Layout::Spiral, outline(kind)[0], false, Stop::Benches(8), RAMPS[0]);
                if with_sectors {
                    spec.rules.sectors = sectors();
                }
                let elevation = if direction == Direction::Up { 100.0 } else { 300.0 };
                let design = built(kind, elevation, &spec, None);
                let name = format!("{kind} {direction:?}, sectors {with_sectors}");
                let measured: u32 = design.walls.iter().map(|wall| wall.samples).sum();
                // Beside the road the walls are left to its checks, so a
                // small pit the ramp spirals right round may have none to
                // measure; across them all, plenty.
                measured_in_all += measured;
                for wall in &design.walls {
                    // Walls cut alike come out as cut; walls that vary, as near
                    // as the report itself calls sound.
                    assert!(
                        wall.over_cut <= if with_sectors { super::walls::STEEPER_BY } else { 0.2 },
                        "{name}: the walls facing {} come out {:.2}° steeper than cut on average",
                        wall.bearing,
                        wall.over_cut
                    );
                    assert!(!wall.too_steep(), "{name}: the walls facing {} are too steep", wall.bearing);
                }
            }
        }
    }
    // Against a target: the walls are checked on average.
    let mut spec = with_ramp(Direction::Up, Layout::Spiral, outline("rect")[0], false, Stop::Benches(8), RAMPS[0]);
    spec.rules.inter_ramp = Some(40.0);
    let design = built("rect", 100.0, &spec, None);
    assert!(
        design.walls.iter().all(|wall| wall.target == Some(40.0) && wall.too_steep()),
        "walls near 47° are too steep for 40°"
    );
    spec.rules.inter_ramp = Some(60.0);
    let design = built("rect", 100.0, &spec, None);
    assert!(design.walls.iter().all(|wall| !wall.too_steep()), "and not for 60°");
    // A sector's own target counts for its walls.
    spec.rules.sectors = sectors();
    spec.rules.sectors[1].inter_ramp = Some(30.0);
    let design = built("rect", 100.0, &spec, None);
    let south_facing = design.walls.iter().find(|wall| wall.bearing == 180.0).expect("the north wall measured");
    assert_eq!(south_facing.target, Some(30.0));
    assert!(south_facing.too_steep());
    assert!(
        design
            .walls
            .iter()
            .filter(|wall| wall.bearing == 0.0)
            .all(|wall| wall.target == Some(60.0) && !wall.too_steep())
    );
    assert!(measured_in_all >= 200, "walls measured at only {measured_in_all} places in all");
}

/// The points of a design's rings closer than a metre to the one before.
fn crowded(design: &Design) -> usize {
    design
        .strings
        .iter()
        .filter(|string| string.closed)
        .map(|string| {
            let count = string.points.len();
            (0..count).filter(|&index| string.points[index].distance(string.points[(index + 1) % count]) < 1.0).count()
        })
        .sum()
}

/// How much nearer its toe any point of a crest comes than its face's run
/// (building up; building down, toe to crest): none where every face is as
/// steep as cut or flatter.
fn steeper_than_cut(design: &Design, face_angle: f64) -> f64 {
    let rings: Vec<&DesignString> = design.strings.iter().filter(|string| string.closed).collect();
    let mut worst: f64 = 0.0;
    for pair in rings.windows(2) {
        let rise = (pair[1].points[0].z - pair[0].points[0].z).abs();
        if rise < 1.0 {
            continue;
        }
        let run = rise / face_angle.to_radians().tan();
        let below: Vec<DVec2> = pair[0].points.iter().map(|point| point.truncate()).collect();
        for point in &pair[1].points {
            worst = worst.max(run - crate::model::pit_design::ring::nearest(&below, point.truncate()).1);
        }
    }
    worst
}

#[test]
fn conditioned_rings_are_lighter_and_no_steeper() {
    for kind in ["rect", "circle", "ell", "star", "floor"] {
        for (direction, layout) in [
            (Direction::Up, Layout::Spiral),
            (Direction::Down, Layout::Spiral),
            (
                Direction::Up,
                Layout::Switchback {
                    every: 2,
                    inside_radius: 15.0,
                    widening: 0.0,
                    grade: 0.0,
                },
            ),
        ] {
            let mut spec = with_ramp(direction, layout, outline(kind)[0], false, Stop::Benches(8), RAMPS[0]);
            spec.wall_radius = 30.0;
            let elevation = if direction == Direction::Up { 100.0 } else { 300.0 };
            let name = format!("{kind} {direction:?} {layout:?}");
            let design = built(kind, elevation, &spec, None);
            let points: Vec<DVec3> = outline(kind).iter().map(|point| point.extend(elevation)).collect();
            let raw = super::design_with(&Base::from_points(&points).unwrap(), &spec, None, &|| false, Additions::FlatsFirst, false)
                .unwrap()
                .0;
            assert_sound(&name, &design, &spec.ramps[0]);
            assert!(design.benches >= raw.benches, "{name}: {} benches conditioned, {} not", design.benches, raw.benches);
            let short = steeper_than_cut(&design, spec.face_angle);
            assert!(short <= 1.0e-3, "{name}: a crest {short:.4} m nearer its toe than the face's run");
            // Lighter wherever the design built with conditioned rings got
            // as far as one without.
            if crowded(&design) != crowded(&raw) {
                assert!(
                    crowded(&design) * 2 <= crowded(&raw),
                    "{name}: {} crowding points, {} unconditioned",
                    crowded(&design),
                    crowded(&raw)
                );
            }
        }
    }
}

#[test]
fn conditioning_moves_a_ring_only_the_way_asked() {
    use crate::model::pit_design::ring::{conditioned, signed_area};
    // A square whose bottom edge zig-zags 3 cm in and out every half metre.
    let mut ring: Vec<DVec2> = (0..=200).map(|step| DVec2::new(f64::from(step) * 0.5, if step % 2 == 0 { 0.0 } else { -0.03 })).collect();
    ring.extend([DVec2::new(100.0, 100.0), DVec2::new(0.0, 100.0)]);
    let outward = conditioned(&ring, 1.0, &|_| false);
    let inward = conditioned(&ring, -1.0, &|_| false);
    assert!(
        outward.len() < ring.len() / 2 && inward.len() < ring.len() / 2,
        "the zig-zag goes: {} and {} of {}",
        outward.len(),
        inward.len(),
        ring.len()
    );
    // Moved out, the ring takes in every point it had; moved in, none of it
    // reaches past where it was.
    let inside = |ring: &[DVec2], point: DVec2| crate::model::pit_design::ring::nearest(ring, point).1 < 1.0e-9 || winding_inside(ring, point);
    assert!(ring.iter().all(|point| inside(&outward, *point)), "moved out, it holds the old ring");
    assert!(inward.iter().all(|point| inside(&ring, *point)), "moved in, it lies within the old ring");
    assert!(signed_area(&outward) >= signed_area(&ring) && signed_area(&inward) <= signed_area(&ring));
    // Kept points stay.
    let kept = conditioned(&ring, 1.0, &|point| point.x < 50.0);
    assert!(ring.iter().filter(|point| point.x < 50.0).all(|point| kept.contains(point)));
}

/// Whether `point` lies inside `ring`, by crossings.
fn winding_inside(ring: &[DVec2], point: DVec2) -> bool {
    let mut inside = false;
    for index in 0..ring.len() {
        let (a, b) = (ring[index], ring[(index + 1) % ring.len()]);
        if (a.y > point.y) != (b.y > point.y) && point.x < a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
}

/// The length of `chain` it runs level at its top end, and at its bottom.
fn level_ends(chain: &[DVec3]) -> (f64, f64) {
    let level = |points: &mut dyn Iterator<Item = &DVec3>| {
        let mut points = points.peekable();
        let Some(first) = points.next() else {
            return 0.0;
        };
        let (mut length, mut last) = (0.0, *first);
        for point in points {
            if (point.z - first.z).abs() > 1.0e-6 {
                break;
            }
            length += point.truncate().distance(last.truncate());
            last = *point;
        }
        length
    };
    let top_first = chain.first().zip(chain.last()).is_some_and(|(first, last)| first.z > last.z);
    let (from_start, from_end) = (level(&mut chain.iter()), level(&mut chain.iter().rev()));
    if top_first { (from_start, from_end) } else { (from_end, from_start) }
}

#[test]
fn a_ramp_runs_where_and_as_its_table_says() {
    let start = outline("rect")[0];
    let centreline = |design: &Design| design.strings.iter().find(|string| string.kind == StringKind::RampCentreline).unwrap().points.clone();

    // Ending at RL 160: the four benches below it have the ramp, those
    // above none, and the road tops out at 160.
    let mut spec = super::tests::spec(Direction::Up, Layout::Spiral, start, false, Stop::Benches(7));
    spec.ramps[0].to = Some(160.0);
    let design = built("rect", 100.0, &spec, None);
    assert_eq!(design.benches, 7);
    let on: Vec<bool> = design.report.iter().map(|bench| bench.ramp().is_some()).collect();
    assert_eq!(on, [true, true, true, true, false, false, false]);
    let top = centreline(&design).iter().map(|point| point.z).fold(f64::NEG_INFINITY, f64::max);
    assert!((top - 160.0).abs() < 1.0e-6, "the road tops out at {top}");
    assert_eq!(crossings(&design), 0);

    // Beginning at RL 130: it starts from the third bench, as a ramp begins
    // anywhere, and climbs from there.
    let mut spec = super::tests::spec(Direction::Up, Layout::Spiral, start, false, Stop::Benches(7));
    spec.ramps[0].from = Some(130.0);
    let design = built("rect", 100.0, &spec, None);
    let on: Vec<bool> = design.report.iter().map(|bench| bench.ramp().is_some()).collect();
    assert_eq!(on, [false, false, true, true, true, true, true]);
    let bottom = centreline(&design).iter().map(|point| point.z).fold(f64::INFINITY, f64::min);
    assert!((bottom - 130.0).abs() < 1.0e-6, "the road starts at {bottom}");
    assert_eq!(crossings(&design), 0);

    // A one-lane bottom: the two benches below RL 130 a 15 m road with a
    // 20 m flat before each climb; and a 40 m flat where it leaves the pit.
    let mut spec = super::tests::spec(Direction::Up, Layout::Spiral, start, false, Stop::Benches(6));
    {
        let ramp = &mut spec.ramps[0];
        ramp.bands = vec![super::RampBand {
            from: 100.0,
            to: 130.0,
            width: Some(15.0),
            flat: Some(20.0),
        }];
        ramp.exit_flat = 40.0;
    }
    let design = built("rect", 100.0, &spec, None);
    assert_eq!(design.benches, 6);
    for (index, bench) in design.report.iter().enumerate() {
        let road = bench.ramp().unwrap();
        // Two benches one lane wide, then one tapering out to the full width
        // from where the last arrived, then the rest full width.
        if index < 2 {
            assert!(road.narrowest >= 15.0 * 0.96 && road.narrowest < 20.0, "bench {index}: {:.1} m wide", road.narrowest);
            assert!(road.flat >= 20.0, "bench {index}: a {:.1} m flat", road.flat);
        } else if index == 2 {
            assert!(road.narrowest >= 15.0 * 0.96, "bench {index}: {:.1} m wide", road.narrowest);
        } else {
            assert!(road.narrowest >= 28.0 * 0.96, "bench {index}: {:.1} m wide", road.narrowest);
        }
        assert!(road.steepest <= 0.1 + 1.0e-6, "bench {index} climbs at {:.4}", road.steepest);
    }
    let (top_level, _) = level_ends(&centreline(&design));
    assert!(top_level >= 40.0 * 0.95, "it leaves the pit on a {top_level:.1} m flat");
    assert_eq!(crossings(&design), 0);

    // Built down, the exit flat is at its first bench, where it leaves the
    // crest.
    let mut spec = super::tests::spec(Direction::Down, Layout::Spiral, outline("crest")[2], false, Stop::Benches(4));
    spec.ramps[0].exit_flat = 40.0;
    let design = built("crest", 300.0, &spec, None);
    assert!(design.report[0].ramp().unwrap().flat >= 40.0);
    let (top_level, _) = level_ends(&centreline(&design));
    assert!(top_level >= 40.0 * 0.95, "it leaves the pit on a {top_level:.1} m flat");
}

/// Hold each ramp of a design to what a sound one is: every stretch of it at
/// its gradient (flatter only where it says so) and its width.
fn assert_ramps_sound(name: &str, design: &Design, ramps: &[RampSpec]) {
    assert!(design.benches > 0, "{name}: no bench built");
    assert_eq!(crossings(design), 0, "{name}: strings cross at different heights");
    for bench in &design.report {
        for road in &bench.ramps {
            let ramp = &ramps[road.ramp];
            assert!(
                road.steepest <= ramp.gradient + 1.0e-6,
                "{name}: ramp {} at RL {} climbs at {:.4}",
                road.ramp,
                bench.elevation,
                road.steepest
            );
            if road.lengthened == 0.0 {
                assert!(
                    road.steepest >= ramp.gradient - 0.005,
                    "{name}: ramp {} at RL {} climbs at {:.4}",
                    road.ramp,
                    bench.elevation,
                    road.steepest
                );
            }
            assert!(
                road.narrowest >= ramp.width * 0.96,
                "{name}: ramp {} at RL {} narrows to {:.1}",
                road.ramp,
                bench.elevation,
                road.narrowest
            );
        }
    }
    let (rings, edges) = breaklines(design);
    if let Err(error) = crate::app::commands::triangulation::creation::cdt_surface_from_breaklines(&rings, &edges) {
        panic!("{name}: no single-valued surface: {error:#}");
    }
}

#[test]
fn two_ramps_climb_a_pit_side_by_side() {
    // On opposite walls of a rectangle, and of an ell, the whole way up.
    for (kind, starts) in [
        ("rect", [DVec2::new(150.0, 0.0), DVec2::new(250.0, 250.0)]),
        ("ell", [DVec2::new(250.0, 0.0), DVec2::new(100.0, 500.0)]),
    ] {
        let mut spec = super::tests::spec(Direction::Up, Layout::Spiral, starts[0], false, Stop::Benches(6));
        let mut second = spec.ramps[0].clone();
        second.start = starts[1];
        second.width = 20.0;
        spec.ramps.push(second);
        let design = built(kind, 100.0, &spec, None);
        let name = format!("{kind} with two ramps");
        assert_eq!(design.end, End::Complete, "{name}: {} benches, blocked {:?}", design.benches, design.blocked_ramp);
        assert_ramps_sound(&name, &design, &spec.ramps);
        assert!(design.report.iter().all(|bench| bench.ramps.len() == 2), "{name}: both ramps on every bench");
        let centrelines = design.strings.iter().filter(|string| string.kind == StringKind::RampCentreline).count();
        assert_eq!(centrelines, 2, "{name}: a centreline each");
        assert!(
            design.ramp_exits.iter().all(|exit| exit.is_some_and(|exit| (exit.z - 190.0).abs() < 1.0e-6)),
            "{name}: both top out"
        );
    }

    // A second ramp beginning at RL 145, on its berm, while the first
    // carries on past it.
    let mut spec = super::tests::spec(Direction::Up, Layout::Spiral, DVec2::new(150.0, 0.0), false, Stop::Benches(6));
    let mut second = spec.ramps[0].clone();
    second.start = DVec2::new(250.0, 250.0);
    second.from = Some(145.0);
    spec.ramps.push(second);
    let design = built("rect", 100.0, &spec, None);
    assert_eq!(design.end, End::Complete, "{} benches, blocked {:?}", design.benches, design.blocked_ramp);
    assert_ramps_sound("rect with a second ramp from RL 145", &design, &spec.ramps);
    let on: Vec<usize> = design.report.iter().map(|bench| bench.ramps.len()).collect();
    assert_eq!(on, [1, 1, 1, 2, 2, 2]);

    // Two ramps asked to start side by side cannot both be laid: the design
    // stops, and says which found no room.
    let mut spec = super::tests::spec(Direction::Up, Layout::Spiral, DVec2::new(150.0, 0.0), false, Stop::Benches(6));
    let mut second = spec.ramps[0].clone();
    second.start = DVec2::new(160.0, 0.0);
    spec.ramps.push(second);
    let design = built("rect", 100.0, &spec, None);
    assert_eq!(design.end, End::OutOfRoom);
    assert_eq!(design.blocked_ramp, Some(1));
}

#[test]
fn two_ramps_down_and_with_a_switchback() {
    // Built down from a crest, two ramps on opposite sides.
    let crest = outline("crest");
    let mut spec = super::tests::spec(Direction::Down, Layout::Spiral, crest[1], false, Stop::Benches(5));
    let mut second = spec.ramps[0].clone();
    second.start = crest[6];
    spec.ramps.push(second);
    let design = built("crest", 300.0, &spec, None);
    assert!(design.benches >= 4, "{} benches, ended {:?}, blocked {:?}", design.benches, design.end, design.blocked_ramp);
    assert_ramps_sound("crest down with two ramps", &design, &spec.ramps);
    assert!(design.report.iter().all(|bench| bench.ramps.len() == 2));

    // A spiral on one wall and a switchback on the other: the switchback
    // keeps to its wall while the spiral goes round, so the spiral climbs
    // into it in the end, and the design stops there saying so.
    let mut spec = super::tests::spec(Direction::Up, Layout::Spiral, DVec2::new(150.0, 0.0), false, Stop::Benches(6));
    let mut second = spec.ramps[0].clone();
    second.start = DVec2::new(250.0, 250.0);
    second.layout = Layout::Switchback {
        every: 2,
        inside_radius: 15.0,
        widening: 0.0,
        grade: 0.0,
    };
    spec.ramps.push(second);
    let design = built("rect", 100.0, &spec, None);
    assert!(design.benches >= 3, "{} benches, ended {:?}, blocked {:?}", design.benches, design.end, design.blocked_ramp);
    if design.end == End::OutOfRoom {
        assert!(design.blocked_ramp.is_some(), "a design stopped for want of room names the ramp");
    }
    assert_ramps_sound("rect with a spiral and a switchback", &design, &spec.ramps);
    assert!(
        design.report.iter().flat_map(|bench| &bench.ramps).any(|road| road.ramp == 1 && road.switchback),
        "the second ramp turns back"
    );
}

#[test]
fn every_stretch_leaves_its_berm_on_the_flat_asked() {
    for kind in ["rect", "circle", "ell"] {
        let mut spec = super::tests::spec(Direction::Up, Layout::Spiral, outline(kind)[0], false, Stop::Benches(6));
        spec.ramps[0].junction_flat = 15.0;
        let design = built(kind, 100.0, &spec, None);
        let name = format!("{kind} with 15 m junction flats");
        assert_eq!(design.end, End::Complete, "{name}: {} benches", design.benches);
        assert_ramps_sound(&name, &design, &spec.ramps);
        for bench in &design.report {
            assert!(
                bench.ramp().unwrap().flat >= 15.0,
                "{name}: at RL {} a {:.1} m flat",
                bench.elevation,
                bench.ramp().unwrap().flat
            );
        }
    }
}

/// The benches a design's first ramp turns back on, with where each turn
/// was asked if it was put off.
fn turns(design: &Design) -> Vec<(f64, Option<f64>)> {
    design
        .report
        .iter()
        .filter_map(|bench| bench.ramp().filter(|road| road.switchback).map(|road| (bench.elevation, road.turn_moved_from)))
        .collect()
}

/// The plan area of a design's toes, all told.
fn toe_area(design: &Design) -> f64 {
    design
        .strings
        .iter()
        .filter(|string| string.closed && string.kind == StringKind::Toe)
        .map(|string| super::ring::signed_area(&string.points.iter().map(|point| point.truncate()).collect::<Vec<_>>()))
        .sum()
}

#[test]
fn switchbacks_turn_where_asked_and_say_where_they_could_not() {
    // At RLs 130 and 175, and only there; wider round the turn, the pads
    // are bigger.
    for (kind, start) in [("rect", DVec2::new(150.0, 0.0)), ("ell", DVec2::new(250.0, 0.0)), ("circle", outline("circle")[0])] {
        let mut areas = Vec::new();
        for widening in [0.0, 8.0] {
            let mut spec = super::tests::spec(
                Direction::Up,
                Layout::Switchback {
                    every: 0,
                    inside_radius: 15.0,
                    widening,
                    grade: 0.0,
                },
                start,
                false,
                Stop::Benches(7),
            );
            spec.ramps[0].turns_at = vec![130.0, 175.0];
            let design = built(kind, 100.0, &spec, None);
            let name = format!("{kind}, turns widened {widening} m");
            assert_eq!(design.end, End::Complete, "{name}");
            assert_ramps_sound(&name, &design, &spec.ramps);
            assert_eq!(turns(&design), [(130.0, None), (175.0, None)], "{name}");
            areas.push(toe_area(&design));
        }
        assert!(areas[1] > areas[0] + 1000.0, "{kind}: widened turns take more room: {areas:?}");
    }

    // Where a turn will not fit at its RL it is made on the next bench that
    // has room, and says so: every turn asked is made, in turn.
    let mut spec = super::tests::spec(
        Direction::Up,
        Layout::Switchback {
            every: 0,
            inside_radius: 30.0,
            widening: 10.0,
            grade: 0.0,
        },
        outline("star")[0],
        false,
        Stop::Benches(7),
    );
    spec.ramps[0].turns_at = vec![115.0, 130.0, 145.0];
    let design = built("star", 100.0, &spec, None);
    assert_ramps_sound("star with three turns", &design, &spec.ramps);
    let made = turns(&design);
    assert_eq!(made.len(), 3, "{made:?}");
    for ((at, moved), asked) in made.iter().zip([115.0, 130.0, 145.0]) {
        assert!(*at >= asked, "the turn asked at {asked} made at {at}");
        assert_eq!(*moved, (*at > asked).then_some(asked), "the turn asked at {asked} made at {at}");
    }
    assert!(made.iter().any(|(_, moved)| moved.is_some()), "{made:?}: some turn had to move here");

    // A switchback with neither a spacing nor an RL to turn at is refused.
    let points: Vec<DVec3> = outline("rect").iter().map(|point| point.extend(100.0)).collect();
    let spec = super::tests::spec(
        Direction::Up,
        Layout::Switchback {
            every: 0,
            inside_radius: 15.0,
            widening: 0.0,
            grade: 0.0,
        },
        outline("rect")[0],
        false,
        Stop::Benches(3),
    );
    assert_eq!(design_from(&points, &spec), Some(Problem::Switchback));
}

/// Why a design from `points` cannot be made, if it cannot.
fn design_from(points: &[DVec3], spec: &Spec) -> Option<Problem> {
    design(&Base::from_points(points).unwrap(), spec, None, &|| false).err()
}

#[test]
fn switchbacks_climb_round_their_turns() {
    // Up every outline, a turn every two benches, widened, climbing at 7.5%
    // round its inside edge on a 10% ramp: as sound as level turns, the
    // stretch after each climbing what is left, and the ramp the shorter.
    let rise = std::f64::consts::PI * 15.0 * 0.075;
    for kind in ["rect", "circle", "ell", "floor", "star"] {
        let mut lengths = Vec::new();
        for grade in [0.0, 0.075] {
            let layout = Layout::Switchback {
                every: 2,
                inside_radius: 15.0,
                widening: 5.0,
                grade,
            };
            let spec = spec(Direction::Up, layout, outline(kind)[0], false, Stop::Benches(7));
            let design = built(kind, 100.0, &spec, None);
            let name = format!("{kind} turning at {grade}");
            assert_eq!(design.end, End::Complete, "{name}");
            assert_ramps_sound(&name, &design, &spec.ramps);
            let rises: Vec<f64> = design
                .report
                .iter()
                .flat_map(|bench| &bench.ramps)
                .filter(|road| road.switchback)
                .map(|road| road.turn_rise)
                .collect();
            assert!(rises.len() >= 2, "{name}: {rises:?}");
            for turn in &rises {
                if grade == 0.0 {
                    assert_eq!(*turn, 0.0, "{name}: a level turn climbs nothing");
                } else {
                    assert!(*turn > rise * 0.5 && *turn <= rise + 1.0e-9, "{name}: a turn climbs {turn:.2} m");
                }
            }
            lengths.push(design.ramp_length);
        }
        assert!(lengths[1] < lengths[0], "{kind}: climbing round its turns, the ramp is shorter: {lengths:?}");
    }
    // And down from a crest.
    for (kind, start) in [("crest", outline("crest")[1]), ("circle", outline("circle")[0])] {
        let layout = Layout::Switchback {
            every: 2,
            inside_radius: 15.0,
            widening: 5.0,
            grade: 0.075,
        };
        let spec = spec(Direction::Down, layout, start, false, Stop::Benches(6));
        let design = built(kind, 300.0, &spec, None);
        assert_ramps_sound(&format!("{kind} down, climbing round its turns"), &design, &spec.ramps);
        assert!(design.report.iter().flat_map(|bench| &bench.ramps).any(|road| road.turn_rise > 0.0));
    }
}

#[test]
fn a_sound_design_raises_no_flags() {
    let start = outline("circle")[0];
    let spec = with_ramp(Direction::Up, Layout::Spiral, start, false, Stop::Benches(10), RAMPS[2]);
    let design = built("circle", 100.0, &spec, None);
    assert!(design.flags.is_empty(), "flags on a sound circle: {:?}", design.flags);
    let mut no_ramp = spec;
    no_ramp.ramps.clear();
    let design = built("floor", 100.0, &no_ramp, None);
    assert!(design.flags.is_empty(), "flags on a floor with no ramp: {:?}", design.flags);
}

#[test]
fn checks_find_what_falls_short_and_where() {
    use super::checks::{Check, flags};
    let start = outline("rect")[0];
    let spec = with_ramp(Direction::Up, Layout::Spiral, start, false, Stop::Benches(6), RAMPS[2]);
    let design = built("rect", 100.0, &spec, None);
    // Asked for more than it was built to: a gentler road, wider berms and
    // flatter faces fall short everywhere.
    let mut stricter = spec.clone();
    stricter.ramps[0].gradient = 0.10;
    stricter.berm_width = 13.0;
    stricter.face_angle = 55.0;
    let found = flags(&stricter, &design, &[]);
    let count = |check: Check| found.iter().filter(|flag| flag.check == check).count();
    let stretches = design.report.iter().filter(|bench| !bench.ramps.is_empty()).count();
    assert_eq!(count(Check::Steep), stretches, "every stretch steeper than 10 %");
    assert_eq!(count(Check::Face), design.report.len(), "every face steeper than 55°");
    assert_eq!(count(Check::Berm), design.report.len() - 1, "every berm after the first narrower than 13 m");
    for flag in &found {
        let bench = &design.report[flag.bench];
        let (low, high) = (bench.elevation, bench.elevation + bench.height);
        assert!(flag.at.z >= low - 1.0e-6 && flag.at.z <= high + 1.0e-6, "{flag:?} off its bench {low}..{high}");
        match flag.check {
            Check::Steep => assert!(flag.value > 0.10 && (flag.asked - 0.10).abs() < 1.0e-12, "{flag:?}"),
            // With no road to leave out, its worst is where the road takes
            // the berm.
            Check::Berm => assert!(flag.value < 13.0 && flag.length > 100.0, "{flag:?}"),
            Check::Face => assert!(flag.value > 56.0 && flag.asked == 55.0 && flag.length > 100.0, "{flag:?}"),
            _ => {}
        }
    }
    // A road narrower than asked, where it is narrowest.
    let mut wider = spec.clone();
    wider.ramps[0].width = 32.0;
    let narrow: Vec<_> = flags(&wider, &design, &[]).into_iter().filter(|flag| flag.check == Check::Narrow).collect();
    assert_eq!(narrow.len(), stretches);
    for flag in &narrow {
        let road = design.report[flag.bench].ramps[0];
        assert_eq!(flag.value, road.narrowest);
        assert_eq!(Some(flag.at), road.narrowest_at);
    }
}

#[test]
fn a_ramp_round_sharp_tips_turns_too_tight_there() {
    use super::checks::Check;
    let start = outline("star")[0];
    let spec = with_ramp(Direction::Up, Layout::Spiral, start, false, Stop::Benches(10), RAMPS[0]);
    let design = built("star", 100.0, &spec, None);
    let tight: Vec<_> = design.flags.iter().filter(|flag| flag.check == Check::Tight).collect();
    assert!(!tight.is_empty(), "the road rounds the star's tips");
    let centreline = design.strings.iter().find(|string| string.kind == StringKind::RampCentreline).unwrap();
    let tips: Vec<DVec2> = outline("star").into_iter().step_by(2).collect();
    for flag in tight {
        assert!(flag.value < flag.asked && flag.asked == 15.0, "{flag:?}");
        assert!(centreline.points.contains(&flag.at), "{flag:?} is on the road's middle");
        let bearing = flag.at.truncate().to_angle();
        assert!(
            tips.iter()
                .any(|tip| (tip.to_angle() - bearing + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI < 0.2),
            "{flag:?} is at a tip"
        );
    }
}

#[test]
fn flags_move_and_turn_over_with_their_design() {
    let start = outline("floor")[0];
    let spec = with_ramp(Direction::Up, Layout::Spiral, start, false, Stop::Benches(6), RAMPS[0]);
    let design = built("floor", 100.0, &spec, None);
    assert!(!design.flags.is_empty(), "the floor's corners turn the road tight");
    let delta = DVec3::new(10.0, -5.0, 3.0);
    let moved = design.translated(delta);
    for (flag, after) in design.flags.iter().zip(&moved.flags) {
        assert_eq!(after.at, flag.at + delta);
    }
    let dump = Spec { kind: Kind::Dump, ..spec };
    let design = built("floor", 100.0, &dump, None);
    let (low, high) = (
        design.levels.iter().copied().fold(f64::INFINITY, f64::min),
        design.levels.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    );
    for flag in &design.flags {
        assert!(flag.at.z >= low - 1.0e-6 && flag.at.z <= high + 1.0e-6, "{flag:?} outside the dump's {low}..{high}");
    }
}

/// How long designs of every sort take, for profiling: run with
/// `cargo test --profile profiling --bin incline-design time_designs -- --ignored --nocapture`.
/// `PIT_CASE` runs only the cases whose name holds it.
#[test]
#[ignore = "a timing, not a test"]
fn time_designs() {
    let rolling = |point: DVec2| Some(170.0 + 12.0 * (point.x / 60.0).sin() + 8.0 * (point.y / 45.0).cos());
    let switchback = Layout::Switchback {
        every: 3,
        inside_radius: 15.0,
        widening: 0.0,
        grade: 0.0,
    };
    let mut cases: Vec<(String, &str, f64, Spec, bool)> = Vec::new();
    for kind in ["rect", "circle", "ell", "star", "floor"] {
        let start = outline(kind)[0];
        cases.push((
            format!("{kind} spiral up"),
            kind,
            100.0,
            spec(Direction::Up, Layout::Spiral, start, false, Stop::Benches(10)),
            false,
        ));
        cases.push((
            format!("{kind} switchback up"),
            kind,
            100.0,
            spec(Direction::Up, switchback, start, false, Stop::Benches(10)),
            false,
        ));
        cases.push((
            format!("{kind} daylight"),
            kind,
            100.0,
            spec(Direction::Up, Layout::Spiral, start, false, Stop::Daylight),
            true,
        ));
    }
    cases.push((
        "crest down".into(),
        "crest",
        500.0,
        spec(Direction::Down, Layout::Spiral, outline("crest")[2], false, Stop::Benches(40)),
        false,
    ));
    let mut walled = spec(Direction::Up, Layout::Spiral, outline("rect")[0], false, Stop::Benches(10));
    walled.rules.sectors = sectors();
    walled.rules.regions = vec![east_region()];
    cases.push(("rect sectors and region".into(), "rect", 100.0, walled, false));
    let mut two = spec(Direction::Up, Layout::Spiral, DVec2::new(150.0, 0.0), false, Stop::Benches(8));
    let mut second = two.ramps[0].clone();
    second.start = DVec2::new(250.0, 250.0);
    two.ramps.push(second);
    cases.push(("rect two ramps".into(), "rect", 100.0, two, false));

    let only = std::env::var("PIT_CASE").ok();
    let repeats: usize = std::env::var("PIT_REPEATS").ok().and_then(|text| text.parse().ok()).unwrap_or(3);
    let mut total = std::time::Duration::ZERO;
    for (name, kind, elevation, spec, daylight) in cases.iter().filter(|case| only.as_ref().is_none_or(|only| case.0.contains(only.as_str()))) {
        let points: Vec<DVec3> = outline(kind).iter().map(|point| point.extend(*elevation)).collect();
        let base = Base::from_points(&points).expect("a closed outline");
        let ground: Option<super::Ground<'_>> = daylight.then_some(&rolling as super::Ground<'_>);
        let mut best = std::time::Duration::MAX;
        let mut benches = 0;
        let mut print = 0u64;
        for _ in 0..repeats {
            let began = std::time::Instant::now();
            let made = design(&base, spec, ground, &|| false);
            best = best.min(began.elapsed());
            print = fingerprint(made.as_ref().ok());
            benches = made.map_or(0, |made| made.benches);
        }
        total += best;
        println!("{name:<28} {:>8.1} ms  {benches} benches  print {print:016x}", best.as_secs_f64() * 1000.0);
    }
    println!("{:<28} {:>8.1} ms", "all", total.as_secs_f64() * 1000.0);
}

/// Every number a design comes to, hashed: two designs alike to the bit
/// hash alike.
fn fingerprint(design: Option<&Design>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let Some(design) = design else {
        return 0;
    };
    for string in &design.strings {
        (string.kind as u8, string.closed).hash(&mut hasher);
        for point in &string.points {
            point.to_array().map(f64::to_bits).hash(&mut hasher);
        }
    }
    (design.benches, design.end as u8, design.volume.to_bits(), design.ramp_length.to_bits()).hash(&mut hasher);
    for flag in &design.flags {
        (
            flag.check as u8,
            flag.bench,
            flag.value.to_bits(),
            flag.asked.to_bits(),
            flag.at.to_array().map(f64::to_bits),
        )
            .hash(&mut hasher);
    }
    for wall in &design.walls {
        (wall.bearing.to_bits(), wall.designed.to_bits(), wall.achieved.to_bits(), wall.steepest.to_bits()).hash(&mut hasher);
    }
    hasher.finish()
}

/// Places strings cross or touch in plan at different heights: either makes
/// the design's surface fail to triangulate.
fn clashes(design: &Design) -> Vec<DVec3> {
    let segments = segments(design);
    let mut found = Vec::new();
    for (i, (a, b, first)) in segments.iter().enumerate() {
        for (c, d, second) in &segments[i + 1..] {
            if first == second {
                continue;
            }
            use crate::model::kernel::SegSeg;
            if let SegSeg::Crossing { t, u, point } | SegSeg::Touching { t, u, point } =
                crate::model::kernel::segment_segment(a.truncate(), b.truncate(), c.truncate(), d.truncate())
                && (a.z + (b.z - a.z) * t - (c.z + (d.z - c.z) * u)).abs() > 0.05
            {
                found.push(point.extend(a.z + (b.z - a.z) * t));
            }
        }
    }
    found
}

/// Rolling ground over a dome orebody's pit, the ground a search for the
/// best pit was tried on: floors from an optimised shell, the ramp started
/// at the bearings round them that once failed.
#[test]
fn pits_up_to_rolling_ground_daylight_soundly_from_any_ramp_start() {
    let ground = |point: DVec2| {
        let (u, v) = ((point.x - 10_000.0) / 2000.0, (point.y - 20_000.0) / 2000.0);
        let pi = std::f64::consts::PI;
        Some(
            420.0 + 25.0 * (2.1 * u * pi + 0.4).sin() * (1.6 * v * pi).cos() + 35.0 * (-((u - 0.75).powi(2) + (v - 0.7).powi(2)) / 0.03).exp()
                - 18.0 * (-((u + v - 1.0) / 0.08).powi(2)).exp()
                + 6.0 * (9.0 * u + 3.0 * v).sin() * (7.0 * v).sin(),
        )
    };
    /// A floor at its RL, and the ramp starts tried round it.
    struct Floor {
        elevation: f64,
        outline: &'static [(f64, f64)],
        /// In sixteenths of a turn from north, and whether the ramp climbs
        /// clockwise: the starts that once left a fold in a ring (strings
        /// touching at different heights), ran out of room, or found no
        /// stretch long enough to climb a bench.
        starts: &'static [(u32, bool)],
    }
    let floors = [
        Floor {
            elevation: 220.0,
            outline: &[
                (10890.0, 21090.0),
                (10900.0, 21060.0),
                (10930.0, 21000.0),
                (10970.0, 20990.0),
                (10980.0, 20990.0),
                (11020.0, 21030.0),
                (11020.0, 21070.0),
                (11010.0, 21100.0),
                (10980.0, 21130.0),
                (10950.0, 21140.0),
                (10930.0, 21140.0),
                (10910.0, 21130.0),
                (10890.0, 21110.0),
            ],
            starts: &[(4, false), (12, false), (14, false), (15, false), (12, true)],
        },
        Floor {
            elevation: 230.0,
            outline: &[
                (10880.0, 21090.0),
                (10890.0, 21050.0),
                (10920.0, 21000.0),
                (10930.0, 20990.0),
                (10960.0, 20980.0),
                (11000.0, 20980.0),
                (11030.0, 21020.0),
                (11030.0, 21070.0),
                (11020.0, 21100.0),
                (10980.0, 21140.0),
                (10950.0, 21150.0),
                (10930.0, 21150.0),
                (10900.0, 21140.0),
                (10890.0, 21130.0),
                (10880.0, 21110.0),
            ],
            starts: &[(0, false), (14, false)],
        },
    ];
    let mut failures = Vec::new();
    for Floor {
        elevation,
        outline: floor,
        starts,
    } in floors
    {
        let points: Vec<DVec3> = floor.iter().map(|&(x, y)| DVec3::new(x, y, elevation)).collect();
        let base = Base::from_points(&points).expect("a closed floor");
        let centre = floor.iter().fold(DVec2::ZERO, |sum, &(x, y)| sum + DVec2::new(x, y)) / floor.len() as f64;
        for &(step, clockwise) in starts {
            let bearing = f64::from(step) * std::f64::consts::TAU / 16.0;
            let start = centre + DVec2::new(bearing.sin(), bearing.cos()) * 300.0;
            let mut spec = with_ramp(Direction::Up, Layout::Spiral, start, clockwise, Stop::Daylight, (0.1, 5.0));
            spec.bench_height = 10.0;
            spec.face_angle = 75.0;
            spec.wall_radius = 20.0;
            spec.ramps[0].width = 25.0;
            let design = design(&base, &spec, Some(&ground), &|| false).expect("a design");
            let name = format!("floor {elevation}, start {:.1}°, clockwise {clockwise}", bearing.to_degrees());
            if design.end != End::Daylight {
                failures.push(format!("{name}: ended {:?} at RL {}", design.end, design.final_elevation()));
            }
            let clashes = clashes(&design);
            if let Some(first) = clashes.first() {
                failures.push(format!("{name}: {} clashes, first at {first:.2}", clashes.len()));
            }
        }
    }
    assert!(failures.is_empty(), "{} designs failed:\n{}", failures.len(), failures.join("\n"));
}

/// A census of what the checks flag across outlines and settings, by check
/// and by kind, way and layout, printed: run by hand with `--ignored
/// --nocapture` to measure the engine. Its counts are the yardstick for work
/// on what the designs fall short of.
#[test]
#[ignore = "a census to read, not a test"]
fn flag_census() {
    use std::collections::BTreeMap;
    let mut by_check: BTreeMap<String, (usize, f64, String)> = BTreeMap::new();
    let mut designs = 0;
    let mut flagged = 0;
    let mut turns = 0;
    let mut ends: BTreeMap<String, usize> = BTreeMap::new();
    let mut groups: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut berm_road: BTreeMap<&str, usize> = BTreeMap::new();
    for kind_name in ["rect", "circle", "ell", "star", "floor", "crest"] {
        let points = outline(kind_name);
        for direction in [Direction::Up, Direction::Down] {
            for dump in [false, true] {
                for layout in [
                    Layout::Spiral,
                    Layout::Switchback {
                        every: 3,
                        inside_radius: 15.0,
                        widening: 0.0,
                        grade: 0.0,
                    },
                ] {
                    for clockwise in [false, true] {
                        for (bench, face, berm) in [(10.0, 70.0, 6.0), (15.0, 60.0, 8.0)] {
                            let start = points[2];
                            let mut spec = with_ramp(direction, layout, start, clockwise, Stop::Benches(8), (0.1, berm));
                            spec.bench_height = bench;
                            spec.face_angle = face;
                            spec.wall_radius = 20.0;
                            spec.ramps[0].width = 25.0;
                            if dump {
                                spec.kind = Kind::Dump;
                            }
                            let elevation = if direction == Direction::Up { 100.0 } else { 500.0 };
                            let pts: Vec<DVec3> = points.iter().map(|p| p.extend(elevation)).collect();
                            let Ok(design) = design(&Base::from_points(&pts).unwrap(), &spec, None, &|| false) else {
                                continue;
                            };
                            designs += 1;
                            turns += design.report.iter().flat_map(|bench| &bench.ramps).filter(|road| road.switchback).count();
                            *ends.entry(format!("{:?}", design.end)).or_default() += 1;
                            if std::env::var("ZZ_LIST").is_ok() {
                                println!(
                                    "list {kind_name} {direction:?} dump={dump} {} cw={clockwise} {bench}: {:?} {} benches, {} flags, {} turns",
                                    matches!(layout, Layout::Spiral),
                                    design.end,
                                    design.benches,
                                    design.flags.len(),
                                    design.report.iter().flat_map(|bench| &bench.ramps).filter(|road| road.switchback).count()
                                );
                            }
                            if !design.flags.is_empty() {
                                flagged += 1;
                            }
                            let name = format!(
                                "{kind_name} {direction:?} {} {layout:?} cw={clockwise} {bench}/{face}/{berm}",
                                if dump { "dump" } else { "pit" }
                            );
                            for flag in design.flags.iter().filter(|flag| flag.check == super::checks::Check::Berm) {
                                let road: f64 = design
                                    .strings
                                    .iter()
                                    .filter(|string| string.kind == StringKind::RampEdge)
                                    .flat_map(|string| {
                                        string.points.windows(2).map(|pair| {
                                            crate::model::kernel::project_onto_segment(flag.at.truncate(), pair[0].truncate(), pair[1].truncate())
                                                .0
                                                .distance(flag.at.truncate())
                                        })
                                    })
                                    .fold(f64::INFINITY, f64::min);
                                let bucket = if road < 15.0 {
                                    "<15"
                                } else if road < 30.0 {
                                    "15-30"
                                } else if road < 60.0 {
                                    "30-60"
                                } else {
                                    ">60"
                                };
                                *berm_road.entry(bucket).or_default() += 1;
                            }
                            for flag in &design.flags {
                                *groups
                                    .entry((
                                        format!("{:?}", flag.check),
                                        format!(
                                            "{} {direction:?} {}",
                                            if dump { "dump" } else { "pit" },
                                            match layout {
                                                Layout::Spiral => "spiral",
                                                _ => "switchback",
                                            }
                                        ),
                                    ))
                                    .or_default() += 1;
                                let miss = (flag.value - flag.asked).abs() / flag.asked.abs().max(1.0e-9);
                                let entry = by_check.entry(format!("{:?}", flag.check)).or_insert((0, 0.0, String::new()));
                                entry.0 += 1;
                                if miss > entry.1 {
                                    *entry = (
                                        entry.0,
                                        miss,
                                        format!(
                                            "{name}: value {:.3} asked {:.3} len {:.1} bench {} at {:.1}",
                                            flag.value, flag.asked, flag.length, flag.bench, flag.at
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{designs} designs, {flagged} with flags, {turns} switchback turns; ends {ends:?}");
    for (check, (count, miss, worst)) in by_check {
        println!("{check}: {count} flags, worst {:.1}% off, {worst}", miss * 100.0);
    }
    for ((check, group), count) in groups {
        println!("  {check:8} {group:24} {count}");
    }
    println!("narrow berms by distance to the nearest road edge: {berm_road:?}");
}

/// Each flag of one check (ZZ_CHECK: Tight, Berm, Narrow, Flatter, Steep,
/// Face) for one way (ZZ_DIR: Up or Down) and layout (ZZ_LAYOUT: spiral or
/// switchback), with where it is: for following up the census.
#[test]
#[ignore = "a census to read, not a test"]
fn flag_detail() {
    let filter_check = std::env::var("ZZ_CHECK").unwrap_or_else(|_| "Tight".to_owned());
    let filter_dir = std::env::var("ZZ_DIR").unwrap_or_else(|_| "Up".to_owned());
    let filter_layout = std::env::var("ZZ_LAYOUT").unwrap_or_else(|_| "spiral".to_owned());
    for kind_name in ["rect", "circle", "ell", "star", "floor", "crest"] {
        let points = outline(kind_name);
        let direction = if filter_dir == "Up" { Direction::Up } else { Direction::Down };
        let layout = if filter_layout == "spiral" {
            Layout::Spiral
        } else {
            Layout::Switchback {
                every: 3,
                inside_radius: 15.0,
                widening: 0.0,
                grade: 0.0,
            }
        };
        for clockwise in [false, true] {
            for (bench, face, berm) in [(10.0, 70.0, 6.0), (15.0, 60.0, 8.0)] {
                let mut spec = with_ramp(direction, layout, points[2], clockwise, Stop::Benches(8), (0.1, berm));
                spec.bench_height = bench;
                spec.face_angle = face;
                spec.wall_radius = 20.0;
                spec.ramps[0].width = 25.0;
                let elevation = if direction == Direction::Up { 100.0 } else { 500.0 };
                let pts: Vec<DVec3> = points.iter().map(|p| p.extend(elevation)).collect();
                let Ok(design) = design(&Base::from_points(&pts).unwrap(), &spec, None, &|| false) else {
                    continue;
                };
                for flag in design.flags.iter().filter(|flag| format!("{:?}", flag.check) == filter_check) {
                    println!(
                        "{kind_name:6} cw={clockwise:5} {bench}/{face}/{berm}: value {:7.3} asked {:6.3} len {:6.1} bench {} at {:.1}",
                        flag.value, flag.asked, flag.length, flag.bench, flag.at
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "a look at one case, not a test"]
fn flag_look() {
    let kind = std::env::var("ZZ_OUTLINE").unwrap_or_else(|_| "rect".to_owned());
    let clockwise = std::env::var("ZZ_CW").is_ok();
    let down = std::env::var("ZZ_DOWN").is_ok();
    let switchback = std::env::var("ZZ_SPIRAL").is_err();
    let points = outline(&kind);
    let layout = if switchback {
        Layout::Switchback {
            every: 3,
            inside_radius: 15.0,
            widening: 0.0,
            grade: 0.0,
        }
    } else {
        Layout::Spiral
    };
    let direction = if down { Direction::Down } else { Direction::Up };
    let small = std::env::var("ZZ_SMALL").is_ok();
    let mut spec = with_ramp(direction, layout, points[2], clockwise, Stop::Benches(8), (0.1, if small { 6.0 } else { 8.0 }));
    spec.bench_height = if small { 10.0 } else { 15.0 };
    spec.face_angle = if small { 70.0 } else { 60.0 };
    spec.wall_radius = 20.0;
    spec.ramps[0].width = 25.0;
    if std::env::var("ZZ_DUMP").is_ok() {
        spec.kind = Kind::Dump;
    }
    let base: Vec<DVec3> = points.iter().map(|p| p.extend(if down { 500.0 } else { 100.0 })).collect();
    let design = design(&Base::from_points(&base).unwrap(), &spec, None, &|| false).unwrap();
    println!("end {:?} benches {}", design.end, design.benches);
    for (index, bench) in design.report.iter().enumerate() {
        for road in &bench.ramps {
            println!("bench {index} at {:.1}: switchback {} turn at {:?}", bench.elevation, road.switchback, road.turn_at);
        }
    }
    for string in &design.strings {
        let (lo, hi) = string.points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.z), hi.max(p.z)));
        println!("string {:?} closed {} z {lo:.1}..{hi:.1} points {}", string.kind, string.closed, string.points.len());
    }
    for flag in &design.flags {
        println!(
            "flag {:?} value {:.2} asked {:.2} len {:.1} bench {} at {:.1}",
            flag.check, flag.value, flag.asked, flag.length, flag.bench, flag.at
        );
    }
    for flag in design.flags.iter().filter(|flag| flag.check == super::checks::Check::Berm) {
        for string in design.strings.iter().filter(|string| string.kind == StringKind::RampCentreline) {
            let best = string
                .points
                .windows(2)
                .map(|pair| {
                    let (q, _) = crate::model::kernel::project_onto_segment(flag.at.truncate(), pair[0].truncate(), pair[1].truncate());
                    (q.distance(flag.at.truncate()), pair[0].z, pair[1].z)
                })
                .fold((f64::INFINITY, 0.0, 0.0), |a, b| if b.0 < a.0 { b } else { a });
            println!("berm flag at {:.1}: centreline {:.1} m away at z {:.1}..{:.1}", flag.at, best.0, best.1, best.2);
        }
    }
    // A section through `ZZ_SECTION=x,y,dx,dy`: every string it crosses,
    // by distance along it, with the string's kind and height there.
    if let Ok(text) = std::env::var("ZZ_SECTION") {
        let v: Vec<f64> = text.split(',').map(|v| v.parse().unwrap()).collect();
        let (origin, way) = (DVec2::new(v[0], v[1]), DVec2::new(v[2], v[3]).normalize());
        let mut hits = Vec::new();
        for string in &design.strings {
            let mut points = string.points.clone();
            if string.closed {
                points.push(points[0]);
            }
            for pair in points.windows(2) {
                let (a, b) = (pair[0].truncate() - origin, pair[1].truncate() - origin);
                let cross = |p: DVec2| way.perp_dot(p);
                let (ca, cb) = (cross(a), cross(b));
                if ca.signum() == cb.signum() {
                    continue;
                }
                let t = ca / (ca - cb);
                let along = a.lerp(b, t).dot(way);
                if along.abs() < 60.0 {
                    hits.push((along, string.kind, pair[0].z + (pair[1].z - pair[0].z) * t));
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (along, kind, z) in hits {
            println!("section {along:7.2} {kind:?} z {z:.2}");
        }
    }
    let focus: Vec<f64> = std::env::var("ZZ_AT")
        .ok()
        .map(|text| text.split(',').map(|v| v.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![0.0, 0.0, 2000.0]);
    let (cx, cy, size) = (focus[0], focus[1], focus[2]);
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='{} {} {size} {size}' width='1400' height='1400'><rect x='{}' y='{}' width='{size}' height='{size}' fill='white'/>",
        cx - size / 2.0,
        -cy - size / 2.0,
        cx - size / 2.0,
        -cy - size / 2.0
    );
    let stroke = size / 1500.0;
    for string in &design.strings {
        let colour = match string.kind {
            StringKind::Toe => "#2a6fdb",
            StringKind::Crest => "#d9452b",
            StringKind::RampEdge => "#2f9e44",
            _ => "#999",
        };
        let path: Vec<String> = string.points.iter().map(|p| format!("{:.2},{:.2}", p.x, -p.y)).collect();
        let tag = if string.closed { "polygon" } else { "polyline" };
        svg += &format!("<{tag} points='{}' fill='none' stroke='{colour}' stroke-width='{stroke}'/>", path.join(" "));
    }
    for flag in &design.flags {
        svg += &format!("<circle cx='{:.1}' cy='{:.1}' r='{}' fill='orange' opacity='0.6'/>", flag.at.x, -flag.at.y, size / 150.0);
    }
    svg += "</svg>";
    std::fs::write(std::env::var("ZZ_SVG").unwrap(), svg).unwrap();
}
