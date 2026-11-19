//! Designs on outlines of every sort, held to what a sound design is: no two
//! strings crossing in plan at different heights, the ramp nowhere steeper
//! than its gradient nor narrower than its width, and a single-valued
//! surface from its strings.

use glam::{DVec2, DVec3};

use super::{Base, Design, DesignString, Direction, End, Layout, Problem, RampSpec, Spec, Stop, StringKind, design};

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
        direction,
        bench_height: 15.0,
        face_angle: 65.0,
        berm_width,
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

/// Hold a design to what a sound one is.
fn assert_sound(name: &str, design: &Design, ramp: &RampSpec) {
    assert!(design.benches > 0, "{name}: no bench built");
    assert_eq!(crossings(design), 0, "{name}: strings cross at different heights");
    assert!(steepest(design) <= ramp.gradient + 1.0e-6, "{name}: the ramp is steeper than its gradient");
    assert!(narrowest(design) >= ramp.width * 0.9, "{name}: the road narrows to {:.1} m", narrowest(design));
    let (rings, edges) = design.breaklines();
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
}
