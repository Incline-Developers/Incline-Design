//! Designs on outlines of every sort, held to what a sound design is: no two
//! strings crossing in plan at different heights, the ramp nowhere steeper
//! than its gradient nor narrower than its width, and a single-valued
//! surface from its strings.

use glam::{DVec2, DVec3};

use super::{Base, BenchRules, Design, DesignString, Direction, End, Kind, Layout, Problem, RampSpec, Spec, Stop, StringKind, Zone, design};

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
        ramp: Some(RampSpec {
            width: 28.0,
            gradient,
            start,
            climbs_clockwise: clockwise,
            layout,
        }),
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
    assert!(design.benches > 0, "{name}: no bench built");
    assert_eq!(crossings(design), 0, "{name}: strings cross at different heights");
    assert!(steepest(design) <= ramp.gradient + 1.0e-6, "{name}: the ramp is steeper than its gradient");
    assert!(narrowest(design) >= ramp.width * 0.96, "{name}: the road narrows to {:.1} m", narrowest(design));
    // Each bench's stretch climbs at the gradient asked: a flat before it
    // where the bench needs more room, flatter only where it says so.
    assert_eq!(design.report.len(), design.benches as usize, "{name}: a report for every bench");
    for (index, bench) in design.report.iter().enumerate() {
        let road = bench.ramp.unwrap_or_else(|| panic!("{name}: bench {index} has no ramp"));
        if road.lengthened == 0.0 {
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
    let lengthened = design.report.iter().filter(|bench| bench.ramp.is_some_and(|road| road.lengthened > 0.0)).count();
    assert!(lengthened * 10 <= design.report.len().max(10), "{name}: {lengthened} stretches climb flatter than asked");
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
            assert_sound(&name, &design, spec.ramp.as_ref().unwrap());
            assert_eq!(design.end, End::Complete, "{name}: stopped at bench {}", design.benches);
        }
    }
}

#[test]
fn switchbacks_up_turn_back_and_are_sound() {
    for (kind, ramp) in ["rect", "circle", "floor"].into_iter().flat_map(|kind| RAMPS.map(|ramp| (kind, ramp))) {
        let start = outline(kind)[0];
        let layout = Layout::Switchback { every: 3, inside_radius: 15.0 };
        let spec = with_ramp(Direction::Up, layout, start, false, Stop::Benches(10), ramp);
        let design = built(kind, 100.0, &spec, None);
        assert_sound(&format!("{kind} switchback, gradient and berm {ramp:?}"), &design, spec.ramp.as_ref().unwrap());
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
            assert_sound(&name, &design, spec.ramp.as_ref().unwrap());
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
    assert_sound("floor to RL 137", &design, at_rl.ramp.as_ref().unwrap());

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
    zoned.rules.height_cycle = vec![10.0, 0.0];
    assert_eq!(problem(zoned), Some(Problem::BenchHeight));
}

#[test]
fn dumps_build_up_stepping_in_and_down_to_the_ground() {
    let start = outline("crest")[2];
    for clockwise in [false, true] {
        let mut spec = with_ramp(Direction::Up, Layout::Spiral, start, clockwise, Stop::Benches(6), RAMPS[0]);
        spec.kind = Kind::Dump;
        let dump = built("crest", 300.0, &spec, None);
        let name = format!("dump up, clockwise {clockwise}");
        assert_sound(&name, &dump, spec.ramp.as_ref().unwrap());
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
    assert_sound("dump down", &dump, spec.ramp.as_ref().unwrap());
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
    }
}

#[test]
fn bench_rules_land_on_the_right_benches() {
    // Up from RL 100: the benches start at 100, 110, 120, 135, 145, 155,
    // 170 and 180. The catch berms come before the fourth and seventh; the
    // zone holds the three starting between 130 and 160.
    let mut flat = spec(Direction::Up, Layout::Spiral, outline("rect")[0], false, Stop::Benches(8));
    flat.ramp = None;
    flat.wall_radius = 0.0;
    flat.rules = varied();
    let plain = built("rect", 100.0, &flat, None);
    let rows: Vec<(f64, f64, f64, f64)> = plain.report.iter().map(|bench| (bench.elevation, bench.height, bench.face_angle, bench.berm)).collect();
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
        if bench.berm > 0.0 {
            gaps.push(bench.berm);
        }
        gaps.push(bench.height / bench.face_angle.to_radians().tan());
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
        assert_sound(&format!("{kind} with bench rules"), &design, ramped.ramp.as_ref().unwrap());
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
        assert_eq!(bench.face_angle, expected, "the lift from RL {}", bench.elevation);
    }
    assert!(dump.report.iter().any(|bench| bench.face_angle == 40.0));
}
